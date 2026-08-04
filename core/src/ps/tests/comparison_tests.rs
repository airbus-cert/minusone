#[cfg(test)]
mod tests_ps_comparison {
    use crate::ps::bool::{Comparison, ParseBool};
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
            Comparison::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    // string / string
    #[test]
    fn test_str_str_eq() {
        assert_eq!(deobfuscate("'abc' -eq 'abc'"), "$true");
    }

    #[test]
    fn test_str_str_ne() {
        assert_eq!(deobfuscate("'abc' -ne 'abd'"), "$true");
    }

    #[test]
    fn test_str_str_ge() {
        assert_eq!(deobfuscate("'b' -ge 'a'"), "$true");
    }

    #[test]
    fn test_str_str_gt() {
        assert_eq!(deobfuscate("'a' -gt 'b'"), "$false");
    }

    #[test]
    fn test_str_str_le() {
        assert_eq!(deobfuscate("'a' -le 'b'"), "$true");
    }

    #[test]
    fn test_str_str_lt() {
        assert_eq!(deobfuscate("'b' -lt 'a'"), "$false");
    }

    // int / int
    #[test]
    fn test_int_int_eq() {
        assert_eq!(deobfuscate("5 -eq 5"), "$true");
    }

    #[test]
    fn test_int_int_ne() {
        assert_eq!(deobfuscate("5 -ne 6"), "$true");
    }

    #[test]
    fn test_int_int_ge() {
        assert_eq!(deobfuscate("5 -ge 5"), "$true");
    }

    #[test]
    fn test_int_int_gt() {
        assert_eq!(deobfuscate("5 -gt 6"), "$false");
    }

    #[test]
    fn test_int_int_lt() {
        assert_eq!(deobfuscate("5 -lt 6"), "$true");
    }

    // bool / bool
    #[test]
    fn test_bool_bool_eq() {
        assert_eq!(deobfuscate("$true -eq $true"), "$true");
    }

    #[test]
    fn test_bool_bool_ne() {
        assert_eq!(deobfuscate("$true -ne $false"), "$true");
    }

    #[test]
    fn test_bool_bool_ge() {
        assert_eq!(deobfuscate("$true -ge $false"), "$true");
    }

    #[test]
    fn test_bool_bool_gt() {
        assert_eq!(deobfuscate("$false -gt $true"), "$false");
    }

    #[test]
    fn test_bool_bool_le() {
        assert_eq!(deobfuscate("$false -le $true"), "$true");
    }

    #[test]
    fn test_bool_bool_lt() {
        assert_eq!(deobfuscate("$false -lt $true"), "$true");
    }

    // bool / string mixed, beyond the -eq/-ne special cases in bool_tests.rs
    #[test]
    fn test_bool_ne_str_empty() {
        assert_eq!(deobfuscate("$true -ne ''"), "$true");
    }

    #[test]
    fn test_true_gt_empty_str() {
        assert_eq!(deobfuscate("$true -gt ''"), "$true");
    }

    #[test]
    fn test_true_gt_nonempty_str() {
        assert_eq!(deobfuscate("$true -gt 'x'"), "$false");
    }

    #[test]
    fn test_true_ge_str() {
        assert_eq!(deobfuscate("$true -ge 'x'"), "$true");
    }

    #[test]
    fn test_false_gt_anything() {
        assert_eq!(deobfuscate("$false -gt 'x'"), "$false");
    }

    #[test]
    fn test_false_ge_empty_str() {
        assert_eq!(deobfuscate("$false -ge ''"), "$true");
    }

    #[test]
    fn test_false_ge_nonempty_str() {
        assert_eq!(deobfuscate("$false -ge 'x'"), "$false");
    }

    // string / number and number / string mixed comparisons
    #[test]
    fn test_str_eq_num() {
        assert_eq!(deobfuscate("'5' -eq 5"), "$true");
    }

    #[test]
    fn test_num_eq_str() {
        assert_eq!(deobfuscate("5 -eq '5'"), "$true");
    }

    #[test]
    fn test_str_ne_num() {
        assert_eq!(deobfuscate("'5' -ne 6"), "$true");
    }

    #[test]
    fn test_str_gt_num_lexicographic() {
        assert_eq!(deobfuscate("'9' -gt 10"), "$true");
    }

    #[test]
    fn test_num_lt_str_is_numeric() {
        assert_eq!(deobfuscate("10 -lt '9'"), "$false");
    }

    #[test]
    fn test_num_lt_str_numeric_crossing_digit_count() {
        assert_eq!(deobfuscate("2 -lt '10'"), "$true");
    }
}
