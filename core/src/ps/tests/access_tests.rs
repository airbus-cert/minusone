#[cfg(test)]
mod tests_ps_access {
    use crate::ps::access::AccessString;
    use crate::ps::array::ParseArrayLiteral;
    use crate::ps::build_powershell_tree;
    use crate::ps::forward::Forward;
    use crate::ps::integer::ParseInt;
    use crate::ps::join::JoinOperator;
    use crate::ps::linter::Linter;
    use crate::ps::string::ParseString;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_powershell_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ParseString::default(),
            AccessString::default(),
            ParseArrayLiteral::default(),
            JoinOperator::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_access_string_element_from_int() {
        assert_eq!(deobfuscate("-join 'abc'[0, 1]"), "\"ab\"");
    }

    #[test]
    fn test_access_string_element_from_negative_int() {
        assert_eq!(deobfuscate("-join 'abc'[-2, -1]"), "\"bc\"");
    }

    #[test]
    fn test_access_string_element_from_negative_string() {
        assert_eq!(deobfuscate("'abc'['-2']"), "\"b\"");
    }

    #[test]
    fn test_access_string_element_from_string() {
        assert_eq!(deobfuscate("'abc'['0']"), "\"a\"");
    }

    #[test]
    fn test_access_string_multi_element_from_int() {
        assert_eq!(deobfuscate("-join 'abc'[1, 2]"), "\"bc\"");
    }
}
