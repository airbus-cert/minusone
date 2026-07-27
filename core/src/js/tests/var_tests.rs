#[cfg(test)]
mod test_vars {
    use crate::js::build_javascript_tree;
    use crate::js::forward::Forward;
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
                Forward::default(),
                Var::default(),
            ),
            JavaScriptStrategy::default(),
        )
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_var_simple_string() {
        assert_eq!(
            deobfuscate("var a = 'hello'; console.log(a);"),
            "var a = 'hello'; console.log('hello');"
        );
    }

    #[test]
    fn test_var_simple_int() {
        assert_eq!(
            deobfuscate("var x = 42; console.log(x);"),
            "var x = 42; console.log(42);"
        );
    }

    #[test]
    fn test_let_simple() {
        assert_eq!(
            deobfuscate("let a = 'world'; console.log(a);"),
            "let a = 'world'; console.log('world');"
        );
    }

    #[test]
    fn test_const_simple() {
        assert_eq!(
            deobfuscate("const a = 'test'; console.log(a);"),
            "const a = 'test'; console.log('test');"
        );
    }

    #[test]
    fn test_var_function_scope() {
        assert_eq!(
            deobfuscate("function test() { var a = 'hello'; console.log(a); } console.log(a);"),
            "function test() { var a = 'hello'; console.log('hello'); } console.log(a);"
        );
    }

    #[test]
    fn test_var_reassignment() {
        assert_eq!(
            deobfuscate("var a = 'hello'; a = 'world'; console.log(a);"),
            "var a = 'hello'; a = 'world'; console.log('world');"
        );
    }

    #[test]
    fn test_var_unknown_reassignment() {
        assert_eq!(
            deobfuscate("var a = 'hello'; a = foo(); console.log(a);"),
            "var a = 'hello'; a = foo(); console.log(a);"
        );
    }

    #[test]
    fn test_multiple_vars() {
        assert_eq!(
            deobfuscate("var a = 'hello'; var b = 'world'; console.log(a, b);"),
            "var a = 'hello'; var b = 'world'; console.log('hello', 'world');"
        );
    }

    #[test]
    fn test_var_in_nested_block() {
        assert_eq!(
            deobfuscate("var x = 10; { console.log(x); }"),
            "var x = 10; { console.log(10); }"
        );
    }

    #[test]
    fn test_let_block_scope() {
        assert_eq!(
            deobfuscate("{ let x = 10; console.log(x); } console.log(x);"),
            "{ let x = 10; console.log(10); } console.log(x);"
        );
    }

    #[test]
    fn test_var_hoists_out_of_block() {
        assert_eq!(
            deobfuscate("{ var x = 10; } console.log(x);"),
            "{ var x = 10; } console.log(10);"
        );
    }

    #[test]
    fn test_let_does_not_hoist_out_of_block() {
        assert_eq!(
            deobfuscate("{ let x = 10; } console.log(x);"),
            "{ let x = 10; } console.log(x);"
        );
    }

    #[test]
    fn test_const_does_not_hoist_out_of_block() {
        assert_eq!(
            deobfuscate("{ const x = 10; } console.log(x);"),
            "{ const x = 10; } console.log(x);"
        );
    }

    #[test]
    fn test_postfix_update_expression() {
        assert_eq!(
            deobfuscate("var x = 5; var y = x++; console.log(x, y);"),
            "var x = 5; var y = 5; console.log(6, 5);"
        );
    }

    #[test]
    fn test_prefix_update_expression() {
        assert_eq!(
            deobfuscate("var x = 5; var y = ++x; console.log(x, y);"),
            "var x = 5; var y = 6; console.log(6, 6);"
        );
    }
}
