use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, Node, NodeMut};
use log::trace;
use std::collections::HashMap;

/// Tracks function declarations with predictable return values
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::forward::Forward;
/// use minusone::js::integer::ParseInt;
/// use minusone::js::string::ParseString;
/// use minusone::js::var::Var;
/// use minusone::js::fncall::FnCall;
/// use minusone::js::linter::Linter;
/// use minusone::js::strategy::JavaScriptStrategy;
///
/// let mut tree = build_javascript_tree("function test() { return 'hello'; } console.log(test());").unwrap();
/// tree.apply_mut_with_strategy(
///     &mut (ParseString::default(), ParseInt::default(), Forward::default(), Var::default(), FnCall::default()),
///     JavaScriptStrategy::default(),
/// ).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "function test() { return 'hello'; } console.log('hello');");
/// ```
pub struct FnCall {
    functions: HashMap<String, JavaScript>,
}

impl Default for FnCall {
    fn default() -> Self {
        FnCall {
            functions: HashMap::new(),
        }
    }
}

impl FnCall {
    fn find_single_return_value(body: &Node<JavaScript>) -> Option<JavaScript> {
        let mut return_value: Option<JavaScript> = None;
        let mut found_count = 0;

        Self::walk_for_returns(body, &mut return_value, &mut found_count);

        if found_count == 1 { return_value } else { None }
    }

    fn walk_for_returns<'a>(
        node: &Node<'a, JavaScript>,
        return_value: &mut Option<JavaScript>,
        found_count: &mut usize,
    ) {
        for child in node.iter() {
            match child.kind() {
                "return_statement" => {
                    *found_count += 1;
                    if *found_count == 1 {
                        // first named child after "return"
                        for i in 0..child.child_count() {
                            if let Some(c) = child.child(i) {
                                if c.kind() != "return" && c.kind() != ";" {
                                    if let Some(data) = c.data() {
                                        *return_value = Some(data.clone());
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
                "function_declaration"
                | "function"
                | "arrow_function"
                | "generator_function_declaration"
                | "generator_function" => {
                    // skip nested fn having their own returns
                }
                // skip loops and conditionals
                "if_statement" | "while_statement" | "do_statement" | "for_statement"
                | "for_in_statement" | "switch_statement" | "try_statement" => {
                    let mut inner_count = 0;
                    Self::count_returns_in_subtree(&child, &mut inner_count);
                    if inner_count > 0 {
                        *found_count += inner_count;
                    }
                }
                _ => {
                    Self::walk_for_returns(&child, return_value, found_count);
                }
            }
        }
    }

    fn count_returns_in_subtree<'a>(node: &Node<'a, JavaScript>, count: &mut usize) {
        for child in node.iter() {
            match child.kind() {
                "return_statement" => {
                    *count += 1;
                }
                "function_declaration"
                | "function"
                | "arrow_function"
                | "generator_function_declaration"
                | "generator_function" => {
                    // skip nested fn
                }
                _ => {
                    Self::count_returns_in_subtree(&child, count);
                }
            }
        }
    }
}

impl<'a> RuleMut<'a> for FnCall {
    type Language = JavaScript;

    fn enter(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() == "program" {
            self.functions.clear();
        }
        Ok(())
    }

    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        let view = node.view();
        match view.kind() {
            "function_declaration" => {
                if let Some(name_node) = view.named_child("name") {
                    if name_node.kind() == "identifier" {
                        let fn_name = name_node.text()?.to_string();

                        if let Some(body) = view.named_child("body") {
                            if let Some(return_data) = Self::find_single_return_value(&body) {
                                trace!(
                                    "FnCall (L): Recorded function '{}' with return value: {:?}",
                                    fn_name, return_data
                                );
                                self.functions.insert(fn_name, return_data);
                            }
                        }
                    }
                }
            }
            "call_expression" => {
                // check known fn
                if let Some(func_node) = view.named_child("function") {
                    if func_node.kind() == "identifier" {
                        let fn_name = func_node.text()?.to_string();

                        if let Some(return_data) = self.functions.get(&fn_name) {
                            trace!(
                                "FnCall (L): Resolving call to '{}' with: {:?}",
                                fn_name, return_data
                            );
                            node.reduce(return_data.clone());
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::js::build_javascript_tree;
    use crate::js::fncall::FnCall;
    use crate::js::forward::Forward;
    use crate::js::integer::{ParseInt, SubAddInt};
    use crate::js::linter::Linter;
    use crate::js::strategy::JavaScriptStrategy;
    use crate::js::string::ParseString;
    use crate::js::var::Var;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                SubAddInt::default(),
                ParseString::default(),
                Forward::default(),
                Var::default(),
                FnCall::default(),
            ),
            JavaScriptStrategy::default(),
        )
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_fncall_simple_string_return() {
        assert_eq!(
            deobfuscate("function test() { return 'hello'; } console.log(test());"),
            "function test() { return 'hello'; } console.log('hello');"
        );
    }

    #[test]
    fn test_fncall_simple_int_return() {
        assert_eq!(
            deobfuscate("function getValue() { return 42; } var x = getValue();"),
            "function getValue() { return 42; } var x = 42;"
        );
    }

    #[test]
    fn test_fncall_with_var_inside() {
        assert_eq!(
            deobfuscate("function test() { var a = 'hello'; return a; } console.log(test());"),
            "function test() { var a = 'hello'; return 'hello'; } console.log('hello');"
        );
    }

    #[test]
    fn test_fncall_does_not_resolve_param_dependent_return() {
        assert_eq!(
            deobfuscate("function test(x) { return x; } console.log(test('hello'));"),
            "function test(x) { return x; } console.log(test('hello'));"
        );
    }

    #[test]
    fn test_fncall_resolve_param_independent_return() {
        assert_eq!(
            deobfuscate("function test(x) { console.log(x); return 1; } var a = test(7);"),
            "function test(x) { console.log(x); return 1; } var a = 1;"
        );
    }

    #[test]
    fn test_fncall_resolve_with_args_when_return_is_constant() {
        assert_eq!(
            deobfuscate("function test() { return 'hello'; } console.log(test('unused'));"),
            "function test() { return 'hello'; } console.log('hello');"
        );
    }

    #[test]
    fn test_fncall_multiple_returns_not_resolved() {
        assert_eq!(
            deobfuscate(
                "function test() { if (true) { return 'a'; } return 'b'; } console.log(test());"
            ),
            "function test() { if (true) { return 'a'; } return 'b'; } console.log(test());"
        );
    }

    #[test]
    fn test_fncall_no_return_not_resolved() {
        assert_eq!(
            deobfuscate("function test() { var a = 1; } console.log(test());"),
            "function test() { var a = 1; } console.log(test());"
        );
    }

    #[test]
    fn test_fncall_nested_function_scope() {
        assert_eq!(
            deobfuscate(
                "function outer() { function inner() { return 'inner'; } return 'outer'; } console.log(outer());"
            ),
            "function outer() { function inner() { return 'inner'; } return 'outer'; } console.log('outer');"
        );
    }

    #[test]
    fn test_fncall_expression_return() {
        assert_eq!(
            deobfuscate("function test() { return 1 + 2; } console.log(test());"),
            "function test() { return 3; } console.log(3);"
        );
    }

    #[test]
    fn test_fncall_unknown_return_not_resolved() {
        assert_eq!(
            deobfuscate("function test() { return foo(); } console.log(test());"),
            "function test() { return foo(); } console.log(test());"
        );
    }
}
