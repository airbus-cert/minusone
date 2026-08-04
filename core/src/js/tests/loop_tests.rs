#[cfg(test)]
mod tests_js_loop {
    use crate::js::array::ParseArray;
    use crate::js::forward::Forward;
    use crate::js::functions::function::ParseFunction;
    use crate::js::integer::ParseInt;
    use crate::js::linter::Linter;
    use crate::js::r#loop::ArrayMapFilter;
    use crate::js::strategy::JavaScriptStrategy;
    use crate::js::string::ParseString;
    use crate::js::var::Var;
    use crate::js::{JavaScriptRuleSet, build_javascript_tree};
    use crate::rule::RuleSetBuilderType;

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

    fn deobfuscate_for_loop(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut_with_strategy(
            &mut JavaScriptRuleSet::new(RuleSetBuilderType::WithoutRules(vec![])),
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

    #[test]
    fn test_counter_propagated_after_loop() {
        let out = deobfuscate_for_loop("for(var i = 0; i < 5; i++) {} var x = i;");
        assert!(out.ends_with("var x = 5;"));
    }

    #[test]
    fn test_string_accumulation() {
        let out = deobfuscate_for_loop(
            "var s = ''; for(var i = 0; i < 3; i++) { s = s + String.fromCharCode(65 + i); } var out = s;",
        );
        assert!(out.ends_with("var out = 'ABC';"));
    }

    #[test]
    fn test_loop_never_runs() {
        let out = deobfuscate_for_loop(
            "var s = 'x'; for(var i = 0; i < 0; i++) { s = s + 'y'; } var out = s;",
        );
        assert!(out.ends_with("var out = 'x';"));
    }

    #[test]
    fn test_bail_on_break() {
        let src = "var s = ''; for(var i = 0; i < 3; i++) { if(i == 1) break; s = s + 'x'; } var out = s;";
        assert!(deobfuscate_for_loop(src).ends_with("var out = s;"));
    }

    #[test]
    fn test_bail_on_return() {
        let src = "function f() { var s = ''; for(var i = 0; i < 3; i++) { if(i == 1) return s; s = s + 'x'; } }";
        assert!(deobfuscate_for_loop(src).contains("for"));
    }

    #[test]
    fn test_non_deterministic_condition_bails() {
        let src = "var s = ''; for(var i = 0; i < unknown; i++) { s = s + 'x'; } var out = s;";
        assert!(deobfuscate_for_loop(src).ends_with("var out = s;"));
    }

    #[test]
    fn test_bare_counter_without_var() {
        let out = deobfuscate_for_loop(
            "var s = ''; for(i = 0; i < 3; i++) { s = s + 'x'; } var out = s;",
        );
        assert!(out.ends_with("var out = 'xxx';"));
    }

    #[test]
    fn test_array_length_in_condition() {
        let out = deobfuscate_for_loop(
            "var a = ['p', 'q', 'r']; var s = ''; for(i = 0; i < a.length; i++) { s = s + a[i]; } var out = s;",
        );
        assert!(out.ends_with("var out = 'pqr';"));
    }

    #[test]
    fn test_hoisted_invariant_array_reads() {
        let out = deobfuscate_for_loop(
            "var a = ['a', 'b', 'c', 'd']; var s = ''; for(j = 0; j < a.length; j++) { s = s + a[j] + a[0]; } var out = s;",
        );
        assert!(out.ends_with("var out = 'aabacada';"));
    }

    #[test]
    fn test_for_in_array_indices() {
        let out = deobfuscate_for_loop(
            "var a = ['x', 'y', 'z']; var s = ''; var k; for (k in a) { s = s + a[k]; } var out = s;",
        );
        assert!(out.ends_with("var out = 'xyz';"));
    }

    #[test]
    fn test_for_of_array_values() {
        let out = deobfuscate_for_loop(
            "var a = ['x', 'y', 'z']; var s = ''; var v; for (v of a) { s = s + v; } var out = s;",
        );
        assert!(out.ends_with("var out = 'xyz';"));
    }

    #[test]
    fn test_for_in_bare_loop_var() {
        let out = deobfuscate_for_loop(
            "var a = ['a', 'b']; var s = ''; for (k in a) { s = s + a[k]; } var out = s;",
        );
        assert!(out.ends_with("var out = 'ab';"));
    }

    #[test]
    fn test_for_in_object_indexed_lookup() {
        let out = deobfuscate_for_loop(
            "var keys = ['p', 'q']; var m = { p: 'A', q: 'B' }; var s = ''; for (i in keys) { s = s + m[keys[i]]; } var out = s;",
        );
        assert!(out.ends_with("var out = 'AB';"));
    }

    #[test]
    fn test_free_var_from_outer_scope() {
        let out = deobfuscate_for_loop(
            "var key = 10; var s = ''; for(var i = 0; i < 3; i++) { s = s + String.fromCharCode(55 + i + key); } var out = s;",
        );
        assert!(out.ends_with("var out = 'ABC';"));
    }
}
