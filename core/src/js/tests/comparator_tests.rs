#[cfg(test)]
mod tests_js_comparator {
    use crate::js::bool::ParseBool;
    use crate::js::build_javascript_tree;
    use crate::js::comparator::*;
    use crate::js::integer::ParseInt;
    use crate::js::linter::Linter;
    use crate::js::string::ParseString;

    fn deobfuscate_comparator(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            ParseBool::default(),
            StrictEq::default(),
            LooseEq::default(),
            CmpOrd::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_strict_eq_num() {
        assert_eq!(deobfuscate_comparator("var x = 1 === 1;"), "var x = true;");
        assert_eq!(deobfuscate_comparator("var x = 1 === 2;"), "var x = false;");
    }

    #[test]
    fn test_strict_eq_cross_type() {
        assert_eq!(
            deobfuscate_comparator("var x = 1 === \"1\";"),
            "var x = false;",
        );
    }

    #[test]
    fn test_strict_neq_num() {
        assert_eq!(deobfuscate_comparator("var x = 1 !== 2;"), "var x = true;",);
    }

    #[test]
    fn test_loose_eq_num() {
        assert_eq!(deobfuscate_comparator("var x = 42 == 42;"), "var x = true;",);
    }

    #[test]
    fn test_loose_eq_str_num() {
        assert_eq!(deobfuscate_comparator("\"42\" == 42;"), "true;",);
        assert_eq!(deobfuscate_comparator("\"abc\" == 0;"), "false;",);
    }

    #[test]
    fn test_loose_eq_bool_num() {
        assert_eq!(deobfuscate_comparator("true == 1;"), "true;",);
        assert_eq!(deobfuscate_comparator("false == 0;"), "true;",);
        assert_eq!(deobfuscate_comparator("true == 2;"), "false;",);
    }

    #[test]
    fn test_loose_eq_empty_str_num() {
        assert_eq!(deobfuscate_comparator("\"\" == 0;"), "true;",);
    }

    #[test]
    fn test_loose_neq() {
        assert_eq!(deobfuscate_comparator("1 != 2;"), "true;",);
    }

    #[test]
    fn test_cmp_num() {
        assert_eq!(deobfuscate_comparator("3 < 5;"), "true;");
        assert_eq!(deobfuscate_comparator("5 > 3;"), "true;");
        assert_eq!(deobfuscate_comparator("3 <= 3;"), "true;");
        assert_eq!(deobfuscate_comparator("4 >= 5;"), "false;");
    }

    #[test]
    fn test_cmp_str_lex() {
        assert_eq!(deobfuscate_comparator("\"abc\" < \"abd\";"), "true;",);
    }

    #[test]
    fn test_cmp_bool_num() {
        assert_eq!(deobfuscate_comparator("true > 0;"), "true;",);
    }

    #[test]
    fn test_cmp_str_num() {
        assert_eq!(deobfuscate_comparator("\"10\" > 9;"), "true;",);
        assert_eq!(deobfuscate_comparator("\"abc\" < 5;"), "false;",);
    }
}
