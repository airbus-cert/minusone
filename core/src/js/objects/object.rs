use crate::error::MinusOneResult;
use crate::js::JavaScript;
use crate::js::JavaScript::*;
use crate::js::functions::function::function_value_from_node;
use crate::js::globals::inject_js_globals;
use crate::js::objects::objectify::as_object;
use crate::js::utils::{get_positional_arguments, is_write_target};
use crate::rule::RuleMut;
use crate::scope::ScopeManager;
use crate::tree::{ControlFlow, Node, NodeMut};
use log::{trace, warn};
use std::collections::HashMap;

/// Parses JavaScript objects into `Object(_)`.
#[derive(Default)]
pub struct ParseObject;

fn number_key(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{:.0}", n)
    } else {
        n.to_string()
    }
}

fn key_from_node(node: &Node<JavaScript>) -> Option<String> {
    if let Some(data) = node.data() {
        return match data {
            Raw(crate::js::Value::Str(s)) => Some(s.clone()),
            Raw(crate::js::Value::Num(n)) => Some(number_key(*n)),
            Raw(crate::js::Value::Bool(b)) => Some(b.to_string()),
            _ => None,
        };
    }

    match node.kind() {
        "identifier" | "property_identifier" => node.text().ok().map(|s| s.to_string()),
        _ => None,
    }
}

impl<'a> RuleMut<'a> for ParseObject {
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
        if view.kind() == "object" {
            let mut map = HashMap::new();
            for child in view.iter() {
                if child.kind() == "pair" {
                    if let (Some(key), Some(value)) = (child.child(0), child.child(2)) {
                        if let Some(key) = key_from_node(&key) {
                            if let Some(value_data) = value.data() {
                                map.insert(key, value_data.clone());
                            } else if let Some(function_value) = function_value_from_node(&value) {
                                map.insert(key, function_value);
                            } else {
                                warn!(
                                    "ParseObject: failed to parse value for pair: {:?}",
                                    child.text()
                                );
                            }
                        } else {
                            warn!(
                                "ParseObject: failed to parse key or value for pair: {:?}",
                                child.text()
                            );
                        }
                    }
                }
            }
            trace!("ParseObject: map = {:?}", map);
            node.reduce(Object {
                map,
                to_string_override: None,
            });
        }

        Ok(())
    }
}

/// ObjectField is a field manager for objects. It allows the engine to redefine fields and read them
///
/// # Example
/// ```
/// use minusone::js::build_javascript_tree;
/// use minusone::js::forward::Forward;
/// use minusone::js::integer::ParseInt;
/// use minusone::js::string::ParseString;
/// use minusone::js::var::Var;
/// use minusone::js::linter::Linter;
/// use minusone::js::strategy::JavaScriptStrategy;
///
/// let mut tree = build_javascript_tree("var a = 'hello'; console.log(a);").unwrap();
/// tree.apply_mut_with_strategy(
///     &mut (ParseString::default(), Forward::default(), Var::default()),
///     JavaScriptStrategy::default(),
/// ).unwrap();
///
/// let mut linter = Linter::default();
/// tree.apply(&mut linter).unwrap();
///
/// assert_eq!(linter.output, "var a = 'hello'; console.log('hello');");
/// ```
#[derive(Default)]
pub struct ObjectField {
    scope_manager: ScopeManager<JavaScript>,
}

struct MemberAccess {
    base_name: Option<String>,
    base_value: Option<JavaScript>,
    keys: Vec<String>,
}

impl ObjectField {
    fn extract_member_access(node: &Node<JavaScript>) -> Option<MemberAccess> {
        match node.kind() {
            "member_expression" => {
                let object = node.named_child("object")?;
                let property = node.named_child("property")?;
                let mut access = Self::extract_member_access(&object)?;
                access.keys.push(key_from_node(&property)?);
                Some(access)
            }
            "subscript_expression" => {
                let object = node.named_child("object")?;
                let index = node.named_child("index")?;
                let mut access = Self::extract_member_access(&object)?;
                access.keys.push(key_from_node(&index)?);
                Some(access)
            }
            "identifier" => Some(MemberAccess {
                base_name: node.text().ok().map(|s| s.to_string()),
                base_value: node.data().cloned(),
                keys: vec![],
            }),
            _ => Some(MemberAccess {
                base_name: None,
                base_value: node.data().cloned(),
                keys: vec![],
            }),
        }
    }

    fn is_string_concat_target(node: &Node<JavaScript>) -> bool {
        let mut current_start = node.start_abs();
        let mut current_end = node.end_abs();
        let mut parent = node.parent();

        while let Some(p) = parent {
            if p.kind() == "parenthesized_expression" {
                current_start = p.start_abs();
                current_end = p.end_abs();
                parent = p.parent();
                continue;
            }

            if p.kind() == "binary_expression"
                && let Some(op) = p.child(1)
                && op.text().ok() == Some("+")
            {
                return p
                    .child(0)
                    .map(|c| c.start_abs() == current_start && c.end_abs() == current_end)
                    .unwrap_or(false)
                    || p.child(2)
                        .map(|c| c.start_abs() == current_start && c.end_abs() == current_end)
                        .unwrap_or(false);
            }

            break;
        }

        if let Some(parent) = node.parent() {
            if parent.kind() == "binary_expression"
                && let Some(op) = parent.child(1)
                && op.text().ok() == Some("+")
            {
                return parent
                    .child(0)
                    .map(|c| c.start_abs() == node.start_abs() && c.end_abs() == node.end_abs())
                    .unwrap_or(false)
                    || parent
                        .child(2)
                        .map(|c| c.start_abs() == node.start_abs() && c.end_abs() == node.end_abs())
                        .unwrap_or(false);
            }
        }

        false
    }

    fn get_by_path(root: &JavaScript, keys: &[String]) -> Option<JavaScript> {
        if keys.is_empty() {
            return Some(root.clone());
        }

        let mut current = root.clone();
        for key in keys {
            current = match current {
                Object { map, .. } => map.get(key).cloned()?,
                value => match as_object(&value) {
                    Some(Object { map, .. }) => map.get(key).cloned()?,
                    _ => return None,
                },
            };
        }

        Some(current)
    }

    fn set_in_map(
        map: &mut HashMap<String, JavaScript>,
        keys: &[String],
        value: JavaScript,
    ) -> bool {
        if keys.is_empty() {
            return false;
        }

        if keys.len() == 1 {
            map.insert(keys[0].clone(), value);
            return true;
        }

        let head = keys[0].clone();
        let entry = map.entry(head).or_insert_with(|| Object {
            map: HashMap::new(),
            to_string_override: None,
        });
        match entry {
            Object { map, .. } => Self::set_in_map(map, &keys[1..], value),
            _ => false,
        }
    }

    fn set_by_path(root: &mut JavaScript, keys: &[String], value: JavaScript) -> bool {
        if keys.is_empty() {
            return false;
        }

        match root {
            Object { map, .. } => Self::set_in_map(map, keys, value),
            _ => false,
        }
    }
}

impl<'a> RuleMut<'a> for ObjectField {
    type Language = JavaScript;

    fn enter(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        let view = node.view();
        match view.kind() {
            "program" => {
                self.scope_manager.reset();
                inject_js_globals(self.scope_manager.current_mut(), false);
            }
            "function_declaration"
            | "function"
            | "arrow_function"
            | "method_definition"
            | "generator_function_declaration"
            | "generator_function"
            | "statement_block" => {
                self.scope_manager.enter();
            }
            "}" => {
                if let Some(parent) = view.parent()
                    && parent.kind() == "statement_block"
                {
                    self.scope_manager.leave();
                }
            }
            _ => {}
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
            "variable_declarator" => {
                if let Some(name_node) = view.named_child("name") {
                    if name_node.kind() == "identifier" {
                        let var_name = name_node.text()?.to_string();
                        if let Some(value_node) =
                            view.named_child("value").or_else(|| view.child(2))
                        {
                            let value_data = value_node
                                .data()
                                .cloned()
                                .or_else(|| function_value_from_node(&value_node));

                            if let Some(value_data @ (Object { .. } | Function { .. })) = value_data
                            {
                                self.scope_manager.current_mut().assign(
                                    &var_name,
                                    value_data,
                                    node.is_ongoing_transaction(),
                                );
                            } else {
                                self.scope_manager
                                    .current_mut()
                                    .forget(&var_name, node.is_ongoing_transaction());
                            }
                        }
                    }
                }
            }
            "assignment_expression" => {
                if let (Some(left), Some(right)) = (view.child(0), view.child(2)) {
                    if left.kind() == "identifier" {
                        let var_name = left.text()?.to_string();
                        let right_data = right
                            .data()
                            .cloned()
                            .or_else(|| function_value_from_node(&right));

                        if let Some(right_data @ (Object { .. } | Function { .. })) = right_data {
                            self.scope_manager.current_mut().assign(
                                &var_name,
                                right_data,
                                node.is_ongoing_transaction(),
                            );
                        } else {
                            self.scope_manager
                                .current_mut()
                                .forget(&var_name, node.is_ongoing_transaction());
                        }
                    } else if let Some(access) = Self::extract_member_access(&left) {
                        if let Some(base_name) = access.base_name {
                            if access.keys.is_empty() {
                                return Ok(());
                            }

                            let rhs_data = right
                                .data()
                                .cloned()
                                .or_else(|| {
                                    if right.kind() == "identifier" {
                                        right.text().ok().and_then(|name| {
                                            self.scope_manager.current().get_var(name).cloned()
                                        })
                                    } else {
                                        None
                                    }
                                })
                                .or_else(|| function_value_from_node(&right));
                            if let Some(data) = rhs_data {
                                if self.scope_manager.current().get_var(&base_name).is_none()
                                    && matches!(access.base_value, Some(Object { .. }))
                                {
                                    self.scope_manager.current_mut().assign(
                                        &base_name,
                                        access.base_value.clone().unwrap(),
                                        node.is_ongoing_transaction(),
                                    );
                                }

                                let mut root_value = self
                                    .scope_manager
                                    .current()
                                    .get_var(&base_name)
                                    .cloned()
                                    .or(access.base_value.clone());

                                if let Some(mut root) = root_value.take()
                                    && Self::set_by_path(&mut root, &access.keys, data)
                                {
                                    self.scope_manager.current_mut().assign(
                                        &base_name,
                                        root,
                                        node.is_ongoing_transaction(),
                                    );
                                }
                            } else {
                                self.scope_manager
                                    .current_mut()
                                    .forget(&base_name, node.is_ongoing_transaction());
                            }
                        }
                    }
                }
            }
            "member_expression" | "subscript_expression" => {
                if is_write_target(&view) {
                    return Ok(());
                }

                if let Some(access) = Self::extract_member_access(&view) {
                    if access.keys.is_empty() {
                        return Ok(());
                    }

                    let base = if let Some(base_name) = access.base_name {
                        self.scope_manager
                            .current()
                            .get_var(&base_name)
                            .cloned()
                            .or(access.base_value)
                    } else {
                        access.base_value
                    };

                    if let Some(base) = base
                        && let Some(value) = Self::get_by_path(&base, &access.keys)
                    {
                        if access.keys.last().map(|k| k.as_str()) == Some("toString")
                            && let Some(parent) = view.parent()
                            && parent.kind() == "call_expression"
                            && parent
                                .named_child("function")
                                .or_else(|| parent.child(0))
                                .map(|callee| {
                                    callee.start_abs() == view.start_abs()
                                        && callee.end_abs() == view.end_abs()
                                })
                                .unwrap_or(false)
                            && !get_positional_arguments(parent.named_child("arguments")).is_empty()
                        {
                            // let the dedicated ToString rule resolve radix-aware calls for integers
                            return Ok(());
                        }

                        if matches!(&value, Function { source, .. } if source.contains("[native code]"))
                            && !Self::is_string_concat_target(&view)
                        {
                            // keep original constructor/at access except in + coercion contexts.
                            return Ok(());
                        }

                        trace!(
                            "ObjectVar (L): Propagating object access {:?} => {:?}",
                            view.text(),
                            value
                        );
                        node.reduce(value);
                    }
                }
            }
            "identifier" => {
                if !is_write_target(&view) {
                    if let Some(parent) = view.parent()
                        && matches!(parent.kind(), "member_expression" | "subscript_expression")
                        && parent
                            .named_child("object")
                            .map(|obj| {
                                obj.start_abs() == view.start_abs()
                                    && obj.end_abs() == view.end_abs()
                            })
                            .unwrap_or(false)
                    {
                        return Ok(());
                    }

                    let var_name = view.text()?.to_string();
                    if let Some(value @ (Object { .. } | Function { .. })) =
                        self.scope_manager.current().get_var(&var_name)
                    {
                        node.set(value.clone());
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
    use crate::js::array::{GetArrayElement, ParseArray};
    use crate::js::bool::ParseBool;
    use crate::js::build_javascript_tree;
    use crate::js::forward::Forward;
    use crate::js::functions::fncall::FnCall;
    use crate::js::functions::function::{ConcatFunction, ParseFunction};
    use crate::js::integer::ParseInt;
    use crate::js::linter::Linter;
    use crate::js::objects::object::{ObjectField, ParseObject};
    use crate::js::specials::{AddSubSpecials, ParseSpecials};
    use crate::js::strategy::JavaScriptStrategy;
    use crate::js::string::Concat;
    use crate::js::string::ParseString;
    use crate::js::var::Var;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                ParseString::default(),
                ParseBool::default(),
                ParseArray::default(),
                ParseFunction::default(),
                ParseObject::default(),
                ParseSpecials::default(),
                Forward::default(),
                GetArrayElement::default(),
                ObjectField::default(),
                AddSubSpecials::default(),
                Concat::default(),
                ConcatFunction::default(),
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
    fn test_object_property_read() {
        assert_eq!(
            deobfuscate("var obj = { a: 'hello' }; console.log(obj.a);"),
            "var obj = {a: 'hello'}; console.log('hello');"
        );
    }

    #[test]
    fn test_object_property_write_then_read() {
        assert_eq!(
            deobfuscate("var obj = {}; obj.a = 'hello'; console.log(obj.a);"),
            "var obj = {}; obj.a = 'hello'; console.log('hello');"
        );
    }

    #[test]
    fn test_object_nested_write_then_read() {
        assert_eq!(
            deobfuscate("var obj = {}; obj.a = {}; obj.a.b = 1; console.log(obj.a.b);"),
            "var obj = {}; obj.a = {}; obj.a.b = 1; console.log(1);"
        );
    }

    #[test]
    fn test_object_full_value_after_property_write() {
        assert_eq!(
            deobfuscate("var my_obj = {}; my_obj.a = 'a'; console.log(my_obj);"),
            "var my_obj = {}; my_obj.a = 'a'; console.log({a: 'a'});"
        );
    }

    #[test]
    fn test_object_full_value_after_property_update() {
        assert_eq!(
            deobfuscate("var my_obj = { a: 'a' }; my_obj.a = 'b'; console.log(my_obj);"),
            "var my_obj = {a: 'a'}; my_obj.a = 'b'; console.log({a: 'b'});"
        );
    }

    #[test]
    fn test_number_builtin_field_access() {
        assert_eq!(deobfuscate("console.log(Number.NaN);"), "console.log(NaN);");
    }

    #[test]
    fn test_number_builtin_field_access_in_function_scope() {
        assert_eq!(
            deobfuscate("function f(){ console.log(Number.NaN); }"),
            "function f(){ console.log(NaN); }"
        );
    }

    #[test]
    fn test_object_function_field_read() {
        assert_eq!(
            deobfuscate("var obj = { a: function(){return 1;} }; console.log(obj.a);"),
            "var obj = {a: function(){return 1;}}; console.log(function(){return 1;});"
        );
    }

    #[test]
    fn test_object_arrow_function_field_read() {
        assert_eq!(
            deobfuscate("var obj = { a: () => 1 }; console.log(obj.a);"),
            "var obj = {a: () => 1}; console.log(() => 1);"
        );
    }

    #[test]
    fn test_array_at_native_function_stringification() {
        assert_eq!(
            deobfuscate("var x = []['at'] + 'hello';"),
            "var x = 'function at() { [native code] }hello';"
        );
    }

    #[test]
    fn test_constructor_name_access_via_object_coercion() {
        assert_eq!(
            deobfuscate("var x = ''['constructor']['name'];"),
            "var x = 'String';"
        );
    }

    #[test]
    fn test_array_constructor_function_stringification() {
        assert_eq!(
            deobfuscate("var x = []['constructor'] + '';"),
            "var x = 'function Array() { [native code] }';"
        );
    }

    #[test]
    fn test_constructor_stringification_var_propagation() {
        assert_eq!(
            deobfuscate("let a = ([]['constructor']) + ''; console.log(a);"),
            "let a = 'function Array() { [native code] }'; console.log('function Array() { [native code] }');"
        );
    }

    #[test]
    fn test_native_constructor_callee_is_not_inlined() {
        assert_eq!(
            deobfuscate("[]['constructor']('return eval')();"),
            "[]['constructor']('return eval')();"
        );
    }

    #[test]
    fn test_native_at_constructor_chain_is_not_inlined() {
        assert_eq!(
            deobfuscate("[]['at']['constructor']('return eval')();"),
            "[]['at']['constructor']('return eval')();"
        );
    }

    #[test]
    fn test_number_literal_to_string_dot_call() {
        assert_eq!(deobfuscate("var x = (1).toString();"), "var x = '1';");
    }

    #[test]
    fn test_string_fontcolor_objectified_call() {
        assert_eq!(
            deobfuscate("var x = 'abc'['fontcolor']();"),
            "var x = '<font color=\"undefined\">abc</font>';"
        );
    }

    #[test]
    fn test_global_number_string_concat() {
        assert_eq!(
            deobfuscate("console.log('false0'+Number);"),
            "console.log('false0function Number() { [native code] }');"
        );
    }
}
