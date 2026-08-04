#[cfg(test)]
mod tests_ps_foreach {
    use crate::ps::array::ParseArrayLiteral;
    use crate::ps::build_powershell_tree;
    use crate::ps::cast::Cast;
    use crate::ps::foreach::{ForEach, PSItemInferrator};
    use crate::ps::forward::Forward;
    use crate::ps::integer::ParseInt;
    use crate::ps::join::JoinOperator;
    use crate::ps::linter::Linter;
    use crate::ps::string::ParseString;
    use crate::ps::typing::ParseType;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_powershell_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            Forward::default(),
            ParseArrayLiteral::default(),
            PSItemInferrator::default(),
            ForEach::default(),
            Cast::default(),
            ParseType::default(),
            JoinOperator::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_foreach_transparent() {
        assert_eq!(deobfuscate("-join ((1,2,3) | % {$_})"), "\"123\"");
    }

    #[test]
    fn test_foreach_transparent_with_mixed_array() {
        assert_eq!(deobfuscate("-join ((\"a\",2,3) | % {$_})"), "\"a23\"");
    }

    #[test]
    fn test_foreach_transparent_with_one_element() {
        assert_eq!(deobfuscate("-join ((1) | % {$_})"), "\"1\"");
    }

    #[test]
    fn test_foreach_cast_with_one_element() {
        assert_eq!(deobfuscate("-join ((0x61) | % {[char]$_})"), "\"a\"");
    }

    #[test]
    fn test_foreach_cast_with_array() {
        assert_eq!(deobfuscate("-join ((0x61, 0x62) | % {[char]$_})"), "\"ab\"");
    }

    #[test]
    fn test_foreach_cast_with_array_and_static_result() {
        assert_eq!(
            deobfuscate("-join ((0x61, 0x62) | % {'z'; [char]$_})"),
            "\"zazb\""
        );
    }

    #[test]
    fn test_foreach_case_insensitive_transparent() {
        assert_eq!(
            deobfuscate("-join ((1,2,3) | fOrEacH-ObjECT {$_})"),
            "\"123\""
        );
    }
}
