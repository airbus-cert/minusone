#[cfg(test)]
mod tests_ps_encoding {
    use crate::ps::array::{ComputeArrayExpr, ParseArrayLiteral};
    use crate::ps::build_powershell_tree;
    use crate::ps::encoding::{EncodingGetBytes, EncodingGetString, EncodingType};
    use crate::ps::forward::Forward;
    use crate::ps::integer::ParseInt;
    use crate::ps::linter::Linter;
    use crate::ps::string::ParseString;
    use crate::ps::typing::ParseType;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_powershell_tree(input).unwrap();
        tree.apply_mut(&mut (
            Forward::default(),
            ParseType::default(),
            EncodingType::default(),
            ParseInt::default(),
            ParseString::default(),
            ParseArrayLiteral::default(),
            ComputeArrayExpr::default(),
            EncodingGetString::default(),
            EncodingGetBytes::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_decode_utf8() {
        assert_eq!(
            deobfuscate("[System.Text.Encoding]::utf8.getstring(@(102, 111, 111))"),
            "\"foo\""
        );
    }

    #[test]
    fn test_decode_unicode() {
        assert_eq!(
            deobfuscate("[System.Text.Encoding]::Unicode.getstring(@(102, 0, 111, 0, 111, 0))"),
            "\"foo\""
        );
    }

    #[test]
    fn test_decode_bigendianunicode() {
        assert_eq!(
            deobfuscate(
                "[System.Text.Encoding]::BigEndianUnicode.getstring(@(0, 102, 0, 111, 0, 111))"
            ),
            "\"foo\""
        );
    }

    #[test]
    fn test_decode_utf32() {
        assert_eq!(
            deobfuscate(
                "[System.Text.Encoding]::utf32.getstring(@(102, 0, 0, 0, 111, 0, 0, 0, 111, 0, 0, 0))"
            ),
            "\"foo\""
        );
    }

    #[test]
    fn test_decode_latin1() {
        assert_eq!(
            deobfuscate("[System.Text.Encoding]::Latin1.getstring(@(102, 111, 111))"),
            "\"foo\""
        );
    }

    #[test]
    fn test_decode_ascii_replaces_high_bytes() {
        assert_eq!(
            deobfuscate("[System.Text.Encoding]::ascii.getstring(@(102, 200, 111))"),
            "\"f?o\""
        );
    }

    #[test]
    fn test_decode_with_invoke() {
        assert_eq!(
            deobfuscate(
                "[System.Text.Encoding]::'unicode'.'getstring'.invoke(@(102, 0, 111, 0, 111, 0))"
            ),
            "\"foo\""
        );
    }

    #[test]
    fn test_getencoding_by_name() {
        assert_eq!(
            deobfuscate("[System.Text.Encoding]::GetEncoding('utf-8').getstring(@(102, 111, 111))"),
            "\"foo\""
        );
    }

    #[test]
    fn test_getencoding_by_codepage() {
        assert_eq!(
            deobfuscate("[System.Text.Encoding]::GetEncoding(28591).getstring(@(102, 111, 111))"),
            "\"foo\""
        );
    }

    #[test]
    fn test_new_constructor() {
        assert_eq!(
            deobfuscate("[System.Text.UTF8Encoding]::new().getstring(@(102, 111, 111))"),
            "\"foo\""
        );
    }

    #[test]
    fn test_new_object_constructor() {
        assert_eq!(
            deobfuscate("(New-Object System.Text.UTF8Encoding).getstring(@(102, 111, 111))"),
            "\"foo\""
        );
    }

    #[test]
    fn test_encode_utf8() {
        assert_eq!(
            deobfuscate("[System.Text.Encoding]::UTF8.getbytes('foo')"),
            "@(102, 111, 111)"
        );
    }

    #[test]
    fn test_encode_unicode() {
        assert_eq!(
            deobfuscate("[System.Text.Encoding]::Unicode.getbytes('foo')"),
            "@(102, 0, 111, 0, 111, 0)"
        );
    }

    #[test]
    fn test_encode_ascii_replaces_out_of_range() {
        let source = format!("[System.Text.Encoding]::ASCII.getbytes('{}')", '\u{1234}');
        assert_eq!(deobfuscate(&source), "@(63)");
    }

    #[test]
    fn test_decode_default_pure_ascii() {
        assert_eq!(
            deobfuscate("[System.Text.Encoding]::Default.getstring(@(102, 111, 111))"),
            "\"foo\""
        );
    }

    #[test]
    fn test_decode_default_non_ascii_is_not_resolved() {
        assert_eq!(
            deobfuscate("[System.Text.Encoding]::Default.getstring(@(102, 200, 111))"),
            "[System.text.encoding]::Default.getstring(@( 102, 200, 111;))"
        );
    }
}
