#[cfg(test)]
mod tests_ps_cast {
    use crate::ps::array::ParseArrayLiteral;
    use crate::ps::build_powershell_tree;
    use crate::ps::cast::Cast;
    use crate::ps::foreach::{ForEach, PSItemInferrator};
    use crate::ps::forward::Forward;
    use crate::ps::integer::{AddInt, ParseInt};
    use crate::ps::join::JoinOperator;
    use crate::ps::linter::Linter;
    use crate::ps::string::{ConcatString, ParseString};
    use crate::ps::typing::ParseType;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_powershell_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            AddInt::default(),
            ConcatString::default(),
            Forward::default(),
            Cast::default(),
            ForEach::default(),
            ParseArrayLiteral::default(),
            PSItemInferrator::default(),
            ParseType::default(),
            JoinOperator::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_cast_int_to_char() {
        assert_eq!(deobfuscate("[char]0x61"), "\"a\"");
    }

    #[test]
    fn test_cast_char_to_int() {
        assert_eq!(deobfuscate("[int]'61'"), "61");
    }

    #[test]
    fn test_cast_int_additive_to_char() {
        assert_eq!(deobfuscate("[char](0x61 + 3)"), "\"d\"");
    }

    #[test]
    fn test_cast_int_concat_char() {
        assert_eq!(
            deobfuscate("[char]0x74 + [char]0x6f + [char]0x74 + [char]0x6f"),
            "\"toto\""
        );
    }

    #[test]
    fn test_cast_foreach_char() {
        assert_eq!(
            deobfuscate("-join ((0x74, 0x6f, 0x74, 0x6f) | % {[char]$_})"),
            "\"toto\""
        );
    }
}
