#[cfg(test)]
mod tests_ps_integer {
    use crate::ps::build_powershell_tree;
    use crate::ps::forward::Forward;
    use crate::ps::integer::{AddInt, MultInt, ParseInt};
    use crate::ps::linter::Linter;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_powershell_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            AddInt::default(),
            MultInt::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_add_two_elements() {
        assert_eq!(deobfuscate("4 + 5"), "9");
    }

    #[test]
    fn test_add_three_elements() {
        assert_eq!(deobfuscate("4 + 5 + 9"), "18");
    }

    #[test]
    fn test_minus_two_elements() {
        assert_eq!(deobfuscate("4 - 5"), "-1");
    }

    #[test]
    fn test_minus_two_elements_with_unary_operators() {
        assert_eq!(deobfuscate("4 + -5"), "-1");
    }

    #[test]
    fn test_minus_two_elements_with_two_unary_operators() {
        assert_eq!(deobfuscate("-4 - 5"), "-9");
    }

    #[test]
    fn test_mul_two_elements() {
        assert_eq!(deobfuscate("4 * 5"), "20");
    }

    #[test]
    fn test_mul_three_elements() {
        assert_eq!(deobfuscate("4 * 5 * 10"), "200");
    }
}
