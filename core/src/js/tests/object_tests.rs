#[cfg(test)]
mod test_object {
    use crate::js::array::{ArrayBuiltins, GetArrayElement, ParseArray};
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
                ArrayBuiltins::default(),
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
    fn test_array_string_key_write_then_read() {
        assert_eq!(
            deobfuscate("var jt = new Array(); jt['a'] = 'hello'; console.log(jt['a']);"),
            "var jt = []; jt['a'] = 'hello'; console.log('hello');"
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
    fn test_global_number_string_concat() {
        assert_eq!(
            deobfuscate("console.log('false0'+Number);"),
            "console.log('false0function Number() { [native code] }');"
        );
    }
}
