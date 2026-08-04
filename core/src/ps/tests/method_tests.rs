#[cfg(test)]
mod tests_ps_method {
    use crate::ps::array::{ComputeArrayExpr, ParseArrayLiteral};
    use crate::ps::build_powershell_tree;
    use crate::ps::forward::Forward;
    use crate::ps::integer::ParseInt;
    use crate::ps::linter::Linter;
    use crate::ps::method::{DecodeBase64, Length};
    use crate::ps::string::ParseString;
    use crate::ps::typing::ParseType;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_powershell_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
            ParseArrayLiteral::default(),
            Length::default(),
            DecodeBase64::default(),
            ParseType::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_array_length() {
        assert_eq!(deobfuscate("@(1,2,3).length"), "3");
    }

    #[test]
    fn test_str_length() {
        assert_eq!(deobfuscate("'foo'.length"), "3");
    }

    #[test]
    fn test_decode_base64() {
        assert_eq!(
            deobfuscate("[System.Convert]::FromBase64String('Zm9v')"),
            "@(102, 111, 111)"
        );
    }

    #[test]
    fn test_error_decode_base64() {
        assert_eq!(
            deobfuscate("[System.Convert]::FromBase64String('AAAAAAAAAA')"),
            "[System.convert]::FromBase64String(\"AAAAAAAAAA\")"
        );
    }

    #[test]
    fn test_error_decode_base64_with_invoke() {
        assert_eq!(
            deobfuscate("[System.Convert]::'FromBase64String'.invoke('AAAAAAAAAA')"),
            "[System.convert]::Frombase64string.invoke(\"AAAAAAAAAA\")"
        );
    }
}
