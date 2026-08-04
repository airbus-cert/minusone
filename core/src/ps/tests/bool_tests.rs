#[cfg(test)]
mod tests_ps_bool {
    use crate::ps::bool::{BoolAlgebra, Comparison, ParseBool};
    use crate::ps::build_powershell_tree;
    use crate::ps::forward::Forward;
    use crate::ps::integer::ParseInt;
    use crate::ps::linter::Linter;
    use crate::ps::string::ParseString;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_powershell_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseBool::default(),
            ParseInt::default(),
            ParseString::default(),
            Forward::default(),
            BoolAlgebra::default(),
            Comparison::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_parse_bool_true() {
        assert_eq!(deobfuscate("$true"), "$true");
    }

    #[test]
    fn test_parse_bool_false() {
        assert_eq!(deobfuscate("$false"), "$false");
    }

    #[test]
    fn test_boolean_algebra_or() {
        assert_eq!(deobfuscate("$true -or $false"), "$true");
    }

    #[test]
    fn test_boolean_algebra_and() {
        assert_eq!(deobfuscate("$true -and $false"), "$false");
    }

    #[test]
    fn test_comparison_int_int() {
        assert_eq!(deobfuscate("4 -le 5"), "$true");
    }

    #[test]
    fn test_comparison_int_str() {
        assert_eq!(deobfuscate("4 -le '5'"), "$true");
    }

    #[test]
    fn test_comparison_special_case_1() {
        assert_eq!(deobfuscate("'True' -eq $true"), "$true");
    }

    #[test]
    fn test_comparison_special_case_2() {
        assert_eq!(deobfuscate("'False' -eq $false"), "$true");
    }

    #[test]
    fn test_comparison_special_case_3() {
        assert_eq!(deobfuscate("'' -eq $true"), "$false");
    }

    #[test]
    fn test_comparison_special_case_4() {
        assert_eq!(deobfuscate("'' -eq $false"), "$false");
    }

    #[test]
    fn test_comparison_special_case_5() {
        assert_eq!(deobfuscate("$false -eq ''"), "$true");
    }
}
