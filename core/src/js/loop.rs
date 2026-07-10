use crate::error::MinusOneResult;
use crate::js::JavaScript::*;
use crate::js::Value::Str;
use crate::js::array::flatten_array;
use crate::js::build_javascript_tree;
use crate::js::strategy::JavaScriptStrategy;
use crate::js::utils::{get_positional_arguments, method_name};
use crate::js::{JavaScript, JavaScriptRuleSet};
use crate::rule::{RuleMut, RuleSetBuilderType};
use crate::tree::{ControlFlow, Node, NodeMut};
use log::{trace, warn};
use std::cell::Cell;

const MAX_MAP_FILTER_DEPTH: usize = 4;

thread_local! {
    static MAP_FILTER_DEPTH: Cell<usize> = const { Cell::new(0) };
}

#[derive(Clone, Copy, PartialEq)]
enum MapFilterKind {
    Map,
    Filter,
}

enum Callback {
    UserFunction {
        params: Vec<String>,
        body_source: String,
        free_var_bindings: String,
    },
    Native(NativeConversion),
}

enum NativeConversion {
    Number,
    String,
}

impl NativeConversion {
    fn apply(&self, element: &JavaScript) -> JavaScript {
        match self {
            NativeConversion::Number => element.as_js_num(),
            NativeConversion::String => Raw(Str(match element {
                Raw(Str(s)) => s.clone(),
                Array(a) => flatten_array(a, None),
                v => v.to_string(),
            })),
        }
    }
}

/// Infers deterministic `Array.prototype.map` and `Array.prototype.filter` calls
/// on array literals. Also implement callback that is a bare reference to `Number`
/// or `String` (e.g. `arr.map(Number)`)
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::integer::{AddInt, ParseInt};
/// use minusone::js::array::ParseArray;
/// use minusone::js::functions::function::ParseFunction;
/// use minusone::js::r#loop::ArrayMapFilter;
/// use minusone::js::linter::Linter;
///
/// let mut tree = build_javascript_tree("var x = [0, 1, 2].map(e => e + 1);").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(), AddInt::default(), ParseArray::default(),
///     ParseFunction::default(), ArrayMapFilter::default()
/// )).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var x = [1, 2, 3];");
/// ```
#[derive(Default)]
pub struct ArrayMapFilter;

impl ArrayMapFilter {
    fn unwrap_parens(mut node: Node<JavaScript>) -> Node<JavaScript> {
        while node.kind() == "parenthesized_expression"
            && let Some(inner) = node.child(1)
        {
            node = inner;
        }
        node
    }

    fn callback_params(cb: &Node<JavaScript>) -> Vec<String> {
        if let Some(param) = cb.named_child("parameter") {
            return param
                .text()
                .map(|s| vec![s.to_string()])
                .unwrap_or_default();
        }

        if let Some(params) = cb.named_child("parameters") {
            return params
                .iter()
                .filter(|child| child.kind() == "identifier")
                .filter_map(|child| child.text().ok().map(|s| s.to_string()))
                .collect();
        }

        vec![]
    }

    fn callback_body_source(body: &Node<JavaScript>) -> Option<String> {
        if body.kind() != "statement_block" {
            let expr = body.text().ok()?;
            return Some(format!("({expr})"));
        }

        let mut prefix_statements = Vec::new();
        let mut return_expr: Option<String> = None;
        let mut return_count = 0usize;

        for statement in body.iter() {
            match statement.kind() {
                "{" | "}" => {}
                "if_statement" | "while_statement" | "do_statement" | "for_statement"
                | "for_in_statement" | "switch_statement" | "try_statement" => return None,
                "return_statement" => {
                    return_count += 1;
                    if return_count > 1 {
                        return None;
                    }
                    for i in 0..statement.child_count() {
                        if let Some(c) = statement.child(i)
                            && c.kind() != "return"
                            && c.kind() != ";"
                        {
                            return_expr = c.text().ok().map(|s| s.to_string());
                            break;
                        }
                    }
                }
                _ => {
                    if let Ok(text) = statement.text() {
                        prefix_statements.push(text.to_string());
                    }
                }
            }
        }

        if return_count != 1 {
            return None;
        }

        prefix_statements.push(format!("({})", return_expr?));
        Some(prefix_statements.join("\n"))
    }

    fn collect_free_var_bindings(body: &Node<JavaScript>, params: &[String]) -> String {
        let mut seen: std::collections::HashSet<String> = params.iter().cloned().collect();
        let mut bindings = String::new();
        Self::collect_free_vars_rec(body, &mut seen, &mut bindings);
        bindings
    }

    fn collect_free_vars_rec(
        node: &Node<JavaScript>,
        seen: &mut std::collections::HashSet<String>,
        bindings: &mut String,
    ) {
        for child in node.iter() {
            if child.kind() == "identifier"
                && let Ok(name) = child.text()
                && !seen.contains(name)
                && let Some(value) = child.data()
            {
                bindings.push_str(&format!("var {name} = {value};\n"));
                seen.insert(name.to_string());
            }
            Self::collect_free_vars_rec(&child, seen, bindings);
        }
    }

    /// Builds `<free var bindings> var <element_param> = <element>; var <index_param> = <index>; var <array_param> = <array>; <body>`
    fn evaluate(
        free_var_bindings: &str,
        params: &[String],
        body_source: &str,
        element: &JavaScript,
        index: usize,
        input: &[JavaScript],
    ) -> Option<JavaScript> {
        let mut bindings = free_var_bindings.to_string();
        if let Some(p) = params.first() {
            bindings.push_str(&format!("var {p} = {element};\n"));
        }
        if let Some(p) = params.get(1) {
            bindings.push_str(&format!("var {p} = {index};\n"));
        }
        if let Some(p) = params.get(2) {
            bindings.push_str(&format!("var {p} = {};\n", Array(input.to_vec())));
        }

        let program_source = format!("{bindings}{body_source}");

        let mut tree = build_javascript_tree(&program_source).ok()?;
        tree.apply_mut_with_strategy(
            &mut JavaScriptRuleSet::new(RuleSetBuilderType::WithoutRules(vec![])),
            JavaScriptStrategy,
        )
        .ok()?;

        let root = tree.root().ok()?;
        let mut result = None;
        for statement in root.iter() {
            if statement.kind() == "expression_statement" {
                result = statement
                    .iter()
                    .find(|c| c.kind() != ";")
                    .and_then(|c| c.data().cloned());
            }
        }

        result
    }

    fn collect_locals(node: &Node<JavaScript>, locals: &mut std::collections::HashSet<String>) {
        for child in node.iter() {
            if child.kind() == "variable_declarator"
                && let Some(name_node) = child.named_child("name")
                && name_node.kind() == "identifier"
                && let Ok(name) = name_node.text()
            {
                locals.insert(name.to_string());
            }
            if matches!(
                child.kind(),
                "arrow_function" | "function" | "function_expression" | "function_declaration"
            ) {
                for name in Self::callback_params(&child) {
                    locals.insert(name);
                }
            }
            Self::collect_locals(&child, locals);
        }
    }

    fn assignment_target_base_name(node: &Node<JavaScript>) -> Option<String> {
        match node.kind() {
            "identifier" => node.text().ok().map(|s| s.to_string()),
            "member_expression" | "subscript_expression" => {
                let object = node.child(0).or_else(|| node.named_child("object"))?;
                Self::assignment_target_base_name(&object)
            }
            _ => None,
        }
    }

    fn mutates_free_var(
        node: &Node<JavaScript>,
        locals: &std::collections::HashSet<String>,
    ) -> bool {
        for child in node.iter() {
            match child.kind() {
                "assignment_expression" | "augmented_assignment_expression" => {
                    if let Some(left) = child.child(0)
                        && let Some(name) = Self::assignment_target_base_name(&left)
                        && !locals.contains(&name)
                    {
                        return true;
                    }
                }
                "update_expression" => {
                    if let Some(operand) = child.iter().find(|c| c.kind() == "identifier")
                        && let Ok(name) = operand.text()
                        && !locals.contains(name)
                    {
                        return true;
                    }
                }
                _ => {}
            }
            if Self::mutates_free_var(&child, locals) {
                return true;
            }
        }
        false
    }

    /// Resolves the callback argument node into either a user function to inline evaluate or a recognized native conversion (`Number`, `String`)
    fn resolve_callback(cb: &Node<JavaScript>) -> Option<Callback> {
        if cb.kind() == "identifier" {
            return match cb.text().ok()? {
                "Number" => Some(Callback::Native(NativeConversion::Number)),
                "String" => Some(Callback::Native(NativeConversion::String)),
                _ => None,
            };
        }

        if matches!(
            cb.kind(),
            "arrow_function" | "function_expression" | "function"
        ) {
            let params = Self::callback_params(cb);
            let body = cb.named_child("body")?;

            let mut locals: std::collections::HashSet<String> = params.iter().cloned().collect();
            Self::collect_locals(&body, &mut locals);
            if Self::mutates_free_var(&body, &locals) {
                return None;
            }

            let body_source = Self::callback_body_source(&body)?;
            let free_var_bindings = Self::collect_free_var_bindings(&body, &params);
            return Some(Callback::UserFunction {
                params,
                body_source,
                free_var_bindings,
            });
        }

        None
    }

    fn apply_callback(
        kind: MapFilterKind,
        input: &[JavaScript],
        cb: &Node<JavaScript>,
    ) -> Option<Vec<JavaScript>> {
        if input.is_empty() {
            return Some(vec![]);
        }

        let callback = Self::resolve_callback(cb)?;

        let mut out = Vec::with_capacity(input.len());
        for (index, element) in input.iter().enumerate() {
            let value = match &callback {
                Callback::Native(conversion) => conversion.apply(element),
                Callback::UserFunction {
                    params,
                    body_source,
                    free_var_bindings,
                } => Self::evaluate(
                    free_var_bindings,
                    params,
                    body_source,
                    element,
                    index,
                    input,
                )?,
            };
            match kind {
                MapFilterKind::Map => out.push(value),
                MapFilterKind::Filter => {
                    if value.as_bool() {
                        out.push(element.clone());
                    }
                }
            }
        }

        Some(out)
    }
}

impl<'a> RuleMut<'a> for ArrayMapFilter {
    type Language = JavaScript;

    fn enter(
        &mut self,
        _node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        let view = node.view();
        if view.kind() != "call_expression" {
            return Ok(());
        }

        let Some(callee) = view.named_child("function").or_else(|| view.child(0)) else {
            return Ok(());
        };
        let Some(method) = method_name(&callee) else {
            return Ok(());
        };

        let kind = match method.as_str() {
            "map" => MapFilterKind::Map,
            "filter" => MapFilterKind::Filter,
            _ => return Ok(()),
        };

        let Some(object) = callee.child(0).or_else(|| callee.named_child("object")) else {
            return Ok(());
        };
        let Some(Array(input)) = object.data() else {
            return Ok(());
        };

        let args = view.named_child("arguments");
        let positional_args = get_positional_arguments(args);
        let Some(cb) = positional_args.into_iter().next() else {
            return Ok(());
        };
        // callbacks are sometimes redundantly parenthesized, e.g. `.map(((e) => e))`
        let cb = Self::unwrap_parens(cb);

        if !matches!(
            cb.kind(),
            "arrow_function" | "function_expression" | "function" | "identifier"
        ) {
            return Ok(());
        }

        if MAP_FILTER_DEPTH.get() >= MAX_MAP_FILTER_DEPTH {
            warn!("ArrayMapFilter: max recursion depth reached, leaving call unresolved");
            return Ok(());
        }

        MAP_FILTER_DEPTH.with(|d| d.set(d.get() + 1));
        let result = Self::apply_callback(kind, input, &cb);
        MAP_FILTER_DEPTH.with(|d| d.set(d.get() - 1));

        if let Some(values) = result {
            trace!(
                "ArrayMapFilter: reducing '{}'.{}(...) to {} elements",
                Array(input.clone()),
                method,
                values.len()
            );
            node.reduce(Array(values));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests_js_loop {
    use super::*;
    use crate::js::array::ParseArray;
    use crate::js::build_javascript_tree;
    use crate::js::forward::Forward;
    use crate::js::functions::function::ParseFunction;
    use crate::js::integer::ParseInt;
    use crate::js::linter::Linter;
    use crate::js::strategy::JavaScriptStrategy;
    use crate::js::string::ParseString;
    use crate::js::var::Var;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                ParseString::default(),
                ParseArray::default(),
                ParseFunction::default(),
                Forward::default(),
                Var::default(),
                ArrayMapFilter::default(),
            ),
            JavaScriptStrategy,
        )
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_map_arrow_expression_body() {
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4].map((e) => e.toString());"),
            "var x = ['0', '1', '2', '3', '4'];"
        );
    }

    #[test]
    fn test_map_bare_arrow_param() {
        assert_eq!(
            deobfuscate("var x = [0, 1, 2].map(e => e + 1);"),
            "var x = [1, 2, 3];"
        );
    }

    #[test]
    fn test_map_block_body_single_return() {
        assert_eq!(
            deobfuscate("var x = [1, 2, 3].map(function (e) { return e * 2; });"),
            "var x = [2, 4, 6];"
        );
    }

    #[test]
    fn test_filter_keeps_original_elements() {
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3, 4].filter((e) => e == 1);"),
            "var x = [1];"
        );
    }

    #[test]
    fn test_filter_empty_array() {
        assert_eq!(
            deobfuscate("var x = [].filter(e => e == 1);"),
            "var x = [];"
        );
    }

    #[test]
    fn test_map_does_not_mutate_original_array() {
        assert_eq!(
            deobfuscate("var x = [0, 1, 2]; var y = x.map(e => e.toString()); var z = x;"),
            "var x = [0, 1, 2]; var y = ['0', '1', '2']; var z = [0, 1, 2];"
        );
    }

    #[test]
    fn test_map_zero_arg_callback() {
        assert_eq!(
            deobfuscate("var x = [1, 2, 3].map(() => 9);"),
            "var x = [9, 9, 9];"
        );
    }

    #[test]
    fn test_map_unresolvable_callback_leaves_call_untouched() {
        assert_eq!(
            deobfuscate("var x = [1, 2, 3].map(e => foo(e));"),
            "var x = [1, 2, 3].map(e => foo(e));"
        );
    }

    #[test]
    fn test_map_multiple_returns_leaves_call_untouched() {
        assert_eq!(
            deobfuscate("var x = [1, 2, 3].map(function (e) { if (e) { return 1; } return 2; });"),
            "var x = [1, 2, 3].map(function (e) { if (e) { return 1; } return 2; });"
        );
    }

    #[test]
    fn test_map_chained_with_filter() {
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, 3].map(e => e + 1).filter(e => e == 2);"),
            "var x = [2];"
        );
    }

    #[test]
    fn test_map_number_callback() {
        assert_eq!(
            deobfuscate("var x = ['3', '1', '2'].map(Number);"),
            "var x = [3, 1, 2];"
        );
    }

    #[test]
    fn test_map_string_callback() {
        assert_eq!(
            deobfuscate("var x = [1, 2, 0].map(String);"),
            "var x = ['1', '2', '0'];"
        );
    }

    #[test]
    fn test_map_string_callback_flattens_arrays() {
        assert_eq!(
            deobfuscate("var x = [[1, 2], [3]].map(String);"),
            "var x = ['1,2', '3'];"
        );
    }

    #[test]
    fn test_filter_number_callback_drops_falsy() {
        assert_eq!(
            deobfuscate("var x = [0, 1, 2, '', '3'].filter(Number);"),
            "var x = [1, 2, '3'];"
        );
    }

    #[test]
    fn test_map_unknown_identifier_callback_leaves_call_untouched() {
        assert_eq!(
            deobfuscate("var x = [1, 2, 3].map(foo);"),
            "var x = [1, 2, 3].map(foo);"
        );
    }

    #[test]
    fn test_map_redundant_parens_around_callback() {
        assert_eq!(
            deobfuscate("var x = [1, 2, 3].map(((e) => e + 1));"),
            "var x = [2, 3, 4];"
        );
    }

    #[test]
    fn test_map_closure_over_outer_const() {
        assert_eq!(
            deobfuscate("var offset = 10; var x = [1, 2, 3].map(e => e + offset);"),
            "var offset = 10; var x = [11, 12, 13];"
        );
    }

    #[test]
    fn test_map_mutating_outer_var_leaves_call_untouched() {
        assert_eq!(
            deobfuscate("var x = 0; [1, 2, 3].map(() => x = x + 1);"),
            "var x = 0; [1, 2, 3].map(() => x = x + 1);"
        );
    }
}
