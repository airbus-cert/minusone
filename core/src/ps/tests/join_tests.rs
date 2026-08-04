#[cfg(test)]
mod tests_ps_join {
    use crate::ps::access::AccessString;
    use crate::ps::array::{ComputeArrayExpr, ParseArrayLiteral};
    use crate::ps::build_powershell_tree;
    use crate::ps::forward::Forward;
    use crate::ps::integer::ParseInt;
    use crate::ps::join::{JoinComparison, JoinOperator, JoinStringMethod};
    use crate::ps::linter::Linter;
    use crate::ps::string::ParseString;
    use crate::ps::typing::ParseType;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_powershell_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            ParseInt::default(),
            Forward::default(),
            ParseArrayLiteral::default(),
            ComputeArrayExpr::default(),
            AccessString::default(),
            ParseType::default(),
            JoinComparison::default(),
            JoinStringMethod::default(),
            JoinOperator::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_join_comparison_with_separator() {
        assert_eq!(deobfuscate("@('a', 'b', 'c') -join '-'"), "\"a-b-c\"");
    }

    #[test]
    fn test_join_comparison_empty_separator() {
        assert_eq!(deobfuscate("@('a', 'b', 'c') -join ''"), "\"abc\"");
    }

    #[test]
    fn test_join_comparison_on_string_indexing() {
        assert_eq!(deobfuscate("(\"foobar\")[0,1,2] -join ''"), "\"foo\"");
    }

    #[test]
    fn test_join_comparison_numeric_array() {
        assert_eq!(deobfuscate("@(1, 2, 3) -join ','"), "\"1,2,3\"");
    }

    #[test]
    fn test_join_string_method() {
        assert_eq!(
            deobfuscate("[string]::join('', (\"a\",\"b\",\"c\"))"),
            "\"abc\""
        );
    }

    #[test]
    fn test_join_string_method_with_separator() {
        assert_eq!(
            deobfuscate("[string]::join('-', (\"a\",\"b\",\"c\"))"),
            "\"a-b-c\""
        );
    }

    #[test]
    fn test_join_operator() {
        assert_eq!(deobfuscate("-join @(\"a\",\"b\", \"c\")"), "\"abc\"");
    }

    #[test]
    fn test_join_operator_numeric_array() {
        assert_eq!(deobfuscate("-join @(1, 2, 3)"), "\"123\"");
    }

    #[test]
    fn test_join_operator_single_element() {
        assert_eq!(deobfuscate("-join @(\"solo\")"), "\"solo\"");
    }
}
