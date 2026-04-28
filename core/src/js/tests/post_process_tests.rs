#[cfg(test)]
mod test_js_post_process {
    use crate::js::build_javascript_tree_for_storage;
    use crate::js::post_process::*;
    use crate::tree::EmptyStorage;

    fn clean(input: &str) -> String {
        let tree = build_javascript_tree_for_storage::<EmptyStorage>(input).unwrap();
        let mut inline_iife = InlineIife::default();
        tree.apply(&mut inline_iife).unwrap();
        let rewritten = inline_iife.clear().unwrap();

        let tree = build_javascript_tree_for_storage::<EmptyStorage>(&rewritten).unwrap();
        let mut rewrite = ExpandAugmentedAssignment::default();
        tree.apply(&mut rewrite).unwrap();
        let rewritten = rewrite.clear().unwrap();

        let tree = build_javascript_tree_for_storage::<EmptyStorage>(&rewritten).unwrap();
        let mut reduce_sequence = ReduceSequenceExpression::default();
        tree.apply(&mut reduce_sequence).unwrap();
        let rewritten = reduce_sequence.clear().unwrap();

        let tree = build_javascript_tree_for_storage::<EmptyStorage>(&rewritten).unwrap();
        let mut for_to_while = ForToWhile::default();
        tree.apply(&mut for_to_while).unwrap();
        let rewritten = for_to_while.clear().unwrap();

        let tree = build_javascript_tree_for_storage::<EmptyStorage>(&rewritten).unwrap();
        let mut bracket_to_member = BracketCallToMember::default();
        tree.apply(&mut bracket_to_member).unwrap();
        let rewritten = bracket_to_member.clear().unwrap();

        let tree = build_javascript_tree_for_storage::<EmptyStorage>(&rewritten).unwrap();
        let mut unused = UnusedVar::default();
        tree.apply(&mut unused).unwrap();
        let mut remover = RemoveUnused::new(unused);
        tree.apply(&mut remover).unwrap();
        remover.clear().unwrap()
    }

    #[test]
    fn test_remove_unused_var() {
        assert_eq!(
            clean("var a = 'hello'; console.log('world');"),
            "console.log('world');"
        );
    }

    #[test]
    fn test_keep_used_var() {
        assert_eq!(
            clean("var a = 'hello'; console.log(a);"),
            "var a = 'hello'; console.log(a);"
        );
    }

    #[test]
    fn test_remove_unused_assignment() {
        assert_eq!(
            clean("var a = 1; a = 2; console.log('ok');"),
            "console.log('ok');"
        );
    }

    #[test]
    fn test_remove_unused_function() {
        assert_eq!(
            clean("function unused() { return 1; } console.log('hello');"),
            "console.log('hello');"
        );
    }

    #[test]
    fn test_keep_used_function() {
        assert_eq!(
            clean("function test() { return 1; } test();"),
            "function test() { return 1; } test();"
        );
    }

    #[test]
    fn test_remove_multiple_unused() {
        assert_eq!(
            clean("var a = 1; var b = 2; console.log('ok');"),
            "console.log('ok');"
        );
    }

    #[test]
    fn test_keep_mixed() {
        assert_eq!(
            clean("var a = 1; var b = 2; console.log(a);"),
            "var a = 1; console.log(a);"
        );
    }

    #[test]
    fn test_remove_unused_let_const() {
        assert_eq!(
            clean("let a = 1; const b = 2; console.log('ok');"),
            "console.log('ok');"
        );
    }

    #[test]
    fn test_full_pipeline_dead_code() {
        assert_eq!(
            clean("function test() { return 'hello'; } console.log('hello');"),
            "console.log('hello');"
        );
    }

    #[test]
    fn test_remove_bare_number() {
        assert_eq!(clean("1; console.log('ok');"), "console.log('ok');");
    }

    #[test]
    fn test_remove_bare_string() {
        assert_eq!(clean("'hello'; console.log('ok');"), "console.log('ok');");
    }

    #[test]
    fn test_remove_bare_bool() {
        assert_eq!(
            clean("true; false; console.log('ok');"),
            "console.log('ok');"
        );
    }

    #[test]
    fn test_remove_bare_literal_after_fncall_inlining() {
        assert_eq!(clean("1; console.log('world');"), "console.log('world');");
    }

    #[test]
    fn test_remove_if_false() {
        assert_eq!(
            clean("if (false) { console.log('no'); } console.log('yes');"),
            "console.log('yes');"
        );
    }

    #[test]
    fn test_if_false_with_else_keeps_else_body() {
        assert_eq!(
            clean("if (false) { console.log('no'); } else { console.log('yes'); }"),
            "console.log('yes');"
        );
    }

    #[test]
    fn test_if_true_keeps_if_body() {
        assert_eq!(
            clean("if (true) { console.log('yes'); }"),
            "console.log('yes');"
        );
    }

    #[test]
    fn test_if_true_with_else_keeps_if_body() {
        assert_eq!(
            clean("if (true) { console.log('yes'); } else { console.log('no'); }"),
            "console.log('yes');"
        );
    }

    #[test]
    fn test_keep_if_variable() {
        assert_eq!(
            clean("if (x) { console.log('maybe'); }"),
            "if (x) { console.log('maybe'); }"
        );
    }

    #[test]
    fn test_no_panic_when_parent_removed_before_children() {
        assert_eq!(
            clean("function drop() { var a = 1; a = 2; 1; } console.log('ok');"),
            "console.log('ok');"
        );
    }

    #[test]
    fn test_split_chained_let_declaration() {
        assert_eq!(
            clean("let r = 4027, E = 'A', S = 'B';"),
            "let r = 4027; let E = 'A'; let S = 'B';"
        );
    }

    #[test]
    fn test_split_chained_const_declaration() {
        assert_eq!(clean("const a = 1, b = 2;"), "const a = 1; const b = 2;");
    }

    #[test]
    fn test_keep_for_header_chained_declaration() {
        assert_eq!(
            clean("for (let i = 0, j = 1; i < 2; i++) { console.log(i, j); }"),
            "for (let i = 0, j = 1; i < 2; i++) { console.log(i, j); }"
        );
    }

    #[test]
    fn test_rewrite_for_ever_to_while_true() {
        assert_eq!(
            clean("for (;;) { console.log('x'); }"),
            "while (true) { console.log('x'); }"
        );
    }

    #[test]
    fn test_rewrite_for_condition_only_to_while() {
        assert_eq!(clean("for (; a != b;) { x(); }"), "while (a != b) { x(); }");
    }

    #[test]
    fn test_keep_for_with_increment() {
        assert_eq!(
            clean("for (; i < 10; i++) { x(); }"),
            "for (; i < 10; i++) { x(); }"
        );
    }

    #[test]
    fn test_keep_for_with_initializer() {
        assert_eq!(
            clean("for (let i = 0; i < 10;) { x(); }"),
            "for (let i = 0; i < 10;) { x(); }"
        );
    }

    #[test]
    fn test_split_sequence_expression_statement() {
        assert_eq!(
            clean("a = 1, b = 2, console.log(b);"),
            "a = 1; b = 2; console.log(b);"
        );
    }

    #[test]
    fn test_split_sequence_expression_statement_with_calls() {
        assert_eq!(
            clean(
                "S = S.replaceAll(a, q), S = S.replaceAll(E, r), t.writeFileSync(r, S), n = transform(stq[10], ord), n = n.replaceAll(E, r);"
            ),
            "S = S.replaceAll(a, q); S = S.replaceAll(E, r); t.writeFileSync(r, S); n = transform(stq[10], ord); n = n.replaceAll(E, r);"
        );
    }

    #[test]
    fn test_rewrite_bracket_call_to_member_call() {
        assert_eq!(clean("console['log'](a);"), "console.log(a);");
    }

    #[test]
    fn test_keep_bracket_call_when_key_not_identifier() {
        assert_eq!(clean("console['x-y'](a);"), "console['x-y'](a);");
    }

    #[test]
    fn test_stress_many_removals_no_slice_panic() {
        let mut input = String::new();
        for i in 0..200 {
            input += &format!("function f{}() {{ var a = 1; a = 2; 1; }} ", i);
        }
        input += "console.log('ok');";

        assert_eq!(clean(&input), "console.log('ok');");
    }

    #[test]
    fn test_inline_anonymous_iife() {
        assert_eq!(
            clean(
                "(function () { var a = 123; var b = 'Hello, world! '; console.log(b + a); })();"
            ),
            "var a = 123; var b = 'Hello, world! '; console.log(b + a);"
        );
    }

    #[test]
    fn test_do_not_inline_iife_with_arguments() {
        assert_eq!(
            clean("(function () { console.log('x'); })(foo());"),
            "(function () { console.log('x'); })(foo());"
        );
    }

    #[test]
    fn test_do_not_inline_iife_with_return() {
        assert_eq!(
            clean("(function () { return 1; })(); console.log('ok');"),
            "(function () { return 1; })(); console.log('ok');"
        );
    }

    #[test]
    fn test_do_not_inline_iife_with_name() {
        assert_eq!(
            clean(
                "(function foo() { var a = 123; var b = 'Hello, world! '; console.log(b + a); })();"
            ),
            "(function foo() { var a = 123; var b = 'Hello, world! '; console.log(b + a); })();"
        );
    }

    #[test]
    fn test_rewrite_augmented_plus_equals() {
        assert_eq!(clean("a += 2;"), "a = a + 2;");
        assert_eq!(clean("a -= 2;"), "a = a - 2;");
        assert_eq!(clean("a *= 2;"), "a = a * 2;");
        assert_eq!(clean("a /= 2;"), "a = a / 2;");
        assert_eq!(clean("a %= 2;"), "a = a % 2;");
        assert_eq!(clean("obj.x += 2;"), "obj.x += 2;");
    }

    #[test]
    fn test_reduce_sequence_expression_to_last_value() {
        assert_eq!(
            clean("console.log((\"a\",\"b\"));"),
            "console.log((\"b\"));"
        );
    }

    #[test]
    fn test_reduce_sequence_expression_with_safe_prefixes() {
        assert_eq!(
            clean("console.log((1, null, \"b\"));"),
            "console.log((\"b\"));"
        );
    }

    #[test]
    fn test_keep_sequence_expression_with_side_effects() {
        assert_eq!(
            clean("console.log((foo(), \"b\"));"),
            "console.log((foo(), \"b\"));"
        );
    }

    #[test]
    fn test_simplify_switch_case_match() {
        assert_eq!(
            clean(
                "switch (2) { case 1: console.log('no'); break; case 2: console.log('yes'); break; default: console.log('d'); }"
            ),
            "console.log('yes');"
        );
    }

    #[test]
    fn test_simplify_switch_case_match_with_early_break() {
        assert_eq!(
            clean(
                "switch (2) {
                        case 1:
                            console.log('no');
                            break;
                        case 2:
                            console.log('yes');
                            break;
                            console.log('no');
                        default:
                            console.log('d');
                    }"
            ),
            "console.log('yes');"
        );
    }

    #[test]
    fn test_simplify_switch_case_default() {
        assert_eq!(
            clean("switch (3) { case 1: a(); break; case 2: b(); break; default: c(); }"),
            "c();"
        );
    }

    #[test]
    fn test_simplify_switch_with_fallthrough_until_switch_end() {
        assert_eq!(
            clean("switch (1) { case 1: a(); case 2: b(); break; default: c(); }"),
            "a(); b();"
        );
    }

    #[test]
    fn test_simplify_switch_with_fallthrough_to_the_end() {
        assert_eq!(
            clean(
                "switch (2) {
                        case 1:
                            console.log('no');
                            break;
                        case 2:
                            console.log('yes');
                        case 3:
                            console.log('yes');
                        default:
                            console.log('yes');
                    }"
            ),
            "console.log('yes'); console.log('yes'); console.log('yes');"
        );
    }

    #[test]
    fn test_simplify_switch_with_fallthrough_until_late_break() {
        assert_eq!(
            clean(
                "switch (2) {
                        case 1:
                            console.log('no');
                            break;
                        case 2:
                            console.log('yes');
                        case 3:
                            console.log('yes');
                            break
                        default:
                            console.log('no');
                    }"
            ),
            "console.log('yes'); console.log('yes');"
        );
    }

    #[test]
    fn test_keep_switch_with_non_literal_case() {
        assert_eq!(
            clean("switch (1) { case x: a(); break; default: b(); }"),
            "switch (1) { case x: a(); break; default: b(); }"
        );
    }

    #[test]
    fn test_remove_while_false() {
        assert_eq!(
            clean("while (false) { console.log('no'); } console.log('yes');"),
            "console.log('yes');"
        );
    }

    #[test]
    fn test_keep_while_variable() {
        assert_eq!(
            clean("while (x) { console.log('maybe'); }"),
            "while (x) { console.log('maybe'); }"
        );
    }
}
