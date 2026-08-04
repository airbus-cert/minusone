#[cfg(test)]
mod tests_ps_var {
    use crate::ps::Powershell::Array;
    use crate::ps::Value::Num;
    use crate::ps::access::AccessHashMap;
    use crate::ps::array::ParseArrayLiteral;
    use crate::ps::bool::ParseBool;
    use crate::ps::build_powershell_tree;
    use crate::ps::forward::Forward;
    use crate::ps::hash::ParseHash;
    use crate::ps::integer::{AddInt, ParseInt};
    use crate::ps::linter::Linter;
    use crate::ps::strategy::PowershellStrategy;
    use crate::ps::string::ParseString;
    use crate::ps::var::{StaticVar, Var};

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_powershell_tree(input).unwrap();
        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                ParseString::default(),
                ParseBool::default(),
                AddInt::default(),
                Forward::default(),
                ParseArrayLiteral::default(),
                ParseHash::default(),
                AccessHashMap::default(),
                Var::default(),
                StaticVar::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_static_replacement() {
        assert_eq!(
            deobfuscate("$foo = 4\nWrite-Debug $foo"),
            "$foo = 4\nWrite-Debug 4"
        );
    }

    #[test]
    fn test_unfollow_var_use_unknow_var() {
        assert_eq!(
            deobfuscate("$foo = $toto\nWrite-Debug $foo"),
            "$foo = $toto\nWrite-Debug $foo"
        );
    }

    #[test]
    fn test_static_var_shell_id() {
        assert_eq!(deobfuscate("$shellid"), "\"Microsoft.Powershell\"");
    }

    #[test]
    fn test_unfollow_var_use_in_if_statement() {
        assert_eq!(
            deobfuscate("$foo = 0\nif(unknown) { $foo = 5 }\n White-Debug $foo"),
            "$foo = 0\nif (unknown){\n $foo = 5\n}\nWhite-Debug $foo"
        );
    }

    #[test]
    fn test_infer_var_use_in_if_statement_predictable() {
        assert_eq!(
            deobfuscate("$foo = 0\nif($true) { $foo = 5 }\nWhite-Debug $foo"),
            "$foo = 0\n$foo = 5\nWhite-Debug 5"
        );
    }

    #[test]
    fn test_infer_var_use_in_if_statement_predictable_false() {
        assert_eq!(
            deobfuscate("$foo = 0\nif($false) { $foo = 5 }\nWhite-Debug $foo"),
            "$foo = 0\nWhite-Debug 0"
        );
    }

    #[test]
    fn test_infer_var_use_in_if_else_statement_predictable() {
        assert_eq!(
            deobfuscate("$foo = 0\nif($false) { $foo = 5 }else { $foo = 8 }\nWhite-Debug $foo"),
            "$foo = 0\n$foo = 8\nWhite-Debug 8"
        );
    }

    #[test]
    fn test_infer_var_use_in_if_elseif_else_statement_predictable() {
        assert_eq!(
            deobfuscate(
                "$foo = 0\nif($false) { $foo = 5 }elseif($true) { $foo = 6 } else {$foo = 7}\nWhite-Debug $foo"
            ),
            "$foo = 0\n$foo = 7\nWhite-Debug 6"
        );
    }

    #[test]
    fn test_infer_var_use_in_if_elseif_else_statement_unpredictable() {
        assert_eq!(
            deobfuscate(
                "$foo = 0\nif($false) { $foo = 5 }elseif(unknown) { $foo = 6 } else {$foo = 7}\nWhite-Debug $foo"
            ),
            "$foo = 0\n$foo = 7\nWhite-Debug $foo"
        );
    }

    #[test]
    fn test_infer_var_use_in_if_elseif_else_statement_predictable_in_else() {
        assert_eq!(
            deobfuscate(
                "$foo = 0\nif($false) { $foo = 5 }elseif($false) { $foo = 6 } else {$foo = 7}\nWhite-Debug $foo"
            ),
            "$foo = 0\n$foo = 7\nWhite-Debug 7"
        );
    }

    #[test]
    fn test_infer_var_use_in_while_statement_use_in_statement() {
        // var is used in the loop statement -> not inferred in the condition and forget
        assert_eq!(
            deobfuscate("$a = 1\nwhile($a -gt 0) { $a = $a + 1 }\nWhite-Debug $a"),
            "$a = 1\nwhile (1 -gt 0){\n $a = $a + 1\n}\nWhite-Debug $a"
        );
    }

    #[test]
    fn test_infer_var_use_in_while_statement_not_use_in_statement() {
        // $a is not modified in the loop body, so its value stays trackable
        assert_eq!(
            deobfuscate("$a = 1\nwhile($a -gt 0) { $b = $a + 1 }\nWhite-Debug $a"),
            "$a = 1\nwhile (1 -gt 0){\n $b = 2\n}\nWhite-Debug 1"
        );
    }

    #[test]
    fn test_infer_var_use_in_function_statement() {
        // infer global var in function statement
        assert_eq!(
            deobfuscate("$a = 1\nFunction invoke { $a }"),
            "$a = 1\nFunction invoke {\n 1\n}"
        );
    }

    #[test]
    fn test_wildcarded_variable() {
        assert_eq!(
            deobfuscate("sV my-var 1\n(varIable M*ar).vaLue"),
            "sv my-var 1\n1"
        );
    }

    #[test]
    fn test_wildcarded_getvariable() {
        assert_eq!(deobfuscate("sV my-var 1\ngV M*ar -vaL"), "sv my-var 1\n1");
    }

    #[test]
    fn test_wildcarded_getsetitem() {
        assert_eq!(
            deobfuscate("sv mYVAr 1\nsi variable:/M*ar 2\n(ls variable:*y*ar).value"),
            "sv mYVAr 1\nsi variable:/M*ar 2\n2"
        );
    }

    #[test]
    fn test_add_assignment_operator_int_int() {
        assert_eq!(deobfuscate("$a=1;$a+=1;$a"), "$a = 1\n$a += 1\n2");
    }

    #[test]
    fn test_add_assignment_operator_int_str() {
        assert_eq!(deobfuscate("$a=1;$a+=\"1\";$a"), "$a = 1\n$a += \"1\"\n2");
    }

    #[test]
    fn test_add_assignment_operator_str_int() {
        assert_eq!(
            deobfuscate("$a=\"1\";$a+=1;$a"),
            "$a = \"1\"\n$a += 1\n\"11\""
        );
    }

    #[test]
    fn test_add_assignment_operator_str_str() {
        assert_eq!(
            deobfuscate("$a=\"1\";$a+=\"1\";$a"),
            "$a = \"1\"\n$a += \"1\"\n\"11\""
        );
    }

    #[test]
    fn test_sub_assignment_operator_int_int() {
        assert_eq!(deobfuscate("$a=1;$a-=1;$a"), "$a = 1\n$a-=1\n0");
    }

    #[test]
    fn test_sub_assignment_operator_int_str() {
        assert_eq!(deobfuscate("$a=1;$a-=\"1\";$a"), "$a = 1\n$a-=\"1\"\n0");
    }

    #[test]
    fn test_sub_assignment_operator_str_int() {
        assert_eq!(deobfuscate("$a=\"1\";$a-=1;$a"), "$a = \"1\"\n$a-=1\n0");
    }

    #[test]
    fn test_sub_assignment_operator_str_str() {
        assert_eq!(
            deobfuscate("$a=\"1\";$a-=\"1\";$a"),
            "$a = \"1\"\n$a-=\"1\"\n0"
        );
    }

    #[test]
    fn test_mul_assignment_operator_int_int() {
        assert_eq!(deobfuscate("$a=2;$a*=3;$a"), "$a = 2\n$a *= 3\n6");
    }

    #[test]
    fn test_mul_assignment_operator_int_str() {
        assert_eq!(deobfuscate("$a=2;$a*=\"3\";$a"), "$a = 2\n$a *= \"3\"\n6");
    }

    #[test]
    fn test_mul_assignment_operator_str_int() {
        assert_eq!(
            deobfuscate("$a=\"12\";$a*=3;$a"),
            "$a = \"12\"\n$a *= 3\n\"121212\""
        );
    }

    #[test]
    fn test_mul_assignment_operator_str_str() {
        assert_eq!(
            deobfuscate("$a=\"12\";$a*=\"3\";$a"),
            "$a = \"12\"\n$a *= \"3\"\n\"121212\""
        );
    }

    #[test]
    fn test_div_assignment_operator_int_int() {
        assert_eq!(deobfuscate("$a=10;$a/=2;$a"), "$a = 10\n$a /= 2\n5");
    }

    #[test]
    fn test_div_assignment_operator_int_str() {
        assert_eq!(deobfuscate("$a=10;$a/=\"2\";$a"), "$a = 10\n$a /= \"2\"\n5");
    }

    #[test]
    fn test_div_assignment_operator_str_int() {
        assert_eq!(deobfuscate("$a=\"10\";$a/=2;$a"), "$a = \"10\"\n$a /= 2\n5");
    }

    #[test]
    fn test_div_assignment_operator_str_str() {
        assert_eq!(
            deobfuscate("$a=\"10\";$a/=\"2\";$a"),
            "$a = \"10\"\n$a /= \"2\"\n5"
        );
    }

    #[test]
    fn test_mod_assignment_operator_int_int() {
        assert_eq!(deobfuscate("$a=9;$a%=2;$a"), "$a = 9\n$a %= 2\n1");
    }

    #[test]
    fn test_mod_assignment_operator_int_str() {
        assert_eq!(deobfuscate("$a=9;$a%=\"2\";$a"), "$a = 9\n$a %= \"2\"\n1");
    }

    #[test]
    fn test_mod_assignment_operator_str_int() {
        assert_eq!(deobfuscate("$a=\"9\";$a%=2;$a"), "$a = \"9\"\n$a %= 2\n1");
    }

    #[test]
    fn test_mod_assignment_operator_str_str() {
        assert_eq!(
            deobfuscate("$a=\"9\";$a%=\"2\";$a"),
            "$a = \"9\"\n$a %= \"2\"\n1"
        );
    }

    #[test]
    fn test_infer_local_var_type() {
        assert_eq!(
            deobfuscate("try{$foo = 1;$foo + 2}catch{}"),
            "try {\n $foo = 1\n 3\n}\ncatch{\n}"
        );
    }

    #[test]
    fn test_array_concatenation() {
        // The Linter only renders Raw/Bytes values back to surface syntax;
        // an inferred Powershell::Array has no textual form it can substitute
        // into `Write-Host $foo`, so the result is only observable on the
        // internal data of the command argument, same as test_new_object_array
        // in array_tests.rs.
        let mut tree = build_powershell_tree(
            r#"
            $foo = 1, 2
            $bar = 3, 4
            $foo += $bar
            Write-Host $foo
            "#,
        )
        .expect("A valid powershell tree");

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                AddInt::default(),
                ParseString::default(),
                Forward::default(),
                ParseArrayLiteral::default(),
                Var::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap() // statement_list
                .child(3)
                .unwrap() // Write-Host pipeline
                .child(0)
                .unwrap() // Write-Host pipeline_chain
                .child(0)
                .unwrap() // Write-host command
                .named_child("command_elements")
                .unwrap() // cmd elements
                .child(1)
                .unwrap() // args
                .data()
                .expect("Expecting inferred type"),
            Array(vec![Num(1), Num(2), Num(3), Num(4)])
        );
    }
}
