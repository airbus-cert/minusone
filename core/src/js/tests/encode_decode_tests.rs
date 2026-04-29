#[cfg(test)]
mod test_encode_decode {
    use crate::js::build_javascript_tree;
    use crate::js::encode_decode::EncodeDecodeBuiltins;
    use crate::js::linter::Linter;
    use crate::js::specials::ParseSpecials;
    use crate::js::string::ParseString;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            ParseSpecials::default(),
            EncodeDecodeBuiltins::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_escape() {
        assert_eq!(deobfuscate("escape('abc123');"), "'abc123';");
        assert_eq!(deobfuscate("escape('äöü');"), "'%E4%F6%FC';");
        assert_eq!(deobfuscate("escape('ć');"), "'%u0107';");
        assert_eq!(deobfuscate("escape('@*_+-./');"), "'@*_+-./';");
        assert_eq!(deobfuscate("escape();"), "'undefined';");
        assert_eq!(deobfuscate("escape(null);"), "'null';");
    }

    #[test]
    fn test_unescape() {
        assert_eq!(deobfuscate("unescape('%E4%F6%FC');"), "'äöü';");
        assert_eq!(deobfuscate("unescape('%u0107');"), "'ć';");
        assert_eq!(deobfuscate("unescape('@*_+-./');"), "'@*_+-./';");
        assert_eq!(deobfuscate("unescape();"), "'undefined';");
        assert_eq!(deobfuscate("unescape(null);"), "'null';");
    }

    #[test]
    fn test_encode_uri() {
        assert_eq!(deobfuscate("encodeURI('abc123');"), "'abc123';");
        assert_eq!(deobfuscate("encodeURI('&');"), "'&';");
        assert_eq!(deobfuscate("encodeURI('ć');"), "'%C4%87';");
        assert_eq!(
            deobfuscate("encodeURI('https://mozilla.org/?x=шеллы');"),
            "'https://mozilla.org/?x=%D1%88%D0%B5%D0%BB%D0%BB%D1%8B';"
        );
        assert_eq!(deobfuscate("encodeURI(';,/?:@&=+$#');"), "';,/?:@&=+$#';");
        assert_eq!(deobfuscate("encodeURI('-_.!~*\\'()');"), "'-_.!~*\\'()';");
        assert_eq!(
            deobfuscate("encodeURI('ABC abc 123');"),
            "'ABC%20abc%20123';"
        );
        assert_eq!(deobfuscate("encodeURI();"), "'undefined';");
        assert_eq!(deobfuscate("encodeURI(null);"), "'null';");
    }

    #[test]
    fn test_decode_uri() {
        assert_eq!(deobfuscate("decodeURI('%C4%87');"), "'ć';");
        assert_eq!(deobfuscate("decodeURI('%26');"), "'%26';");
        assert_eq!(
            deobfuscate("decodeURI('https://mozilla.org/?x=%D1%88%D0%B5%D0%BB%D0%BB%D1%8B');"),
            "'https://mozilla.org/?x=шеллы';"
        );
        assert_eq!(deobfuscate("decodeURI();"), "'undefined';");
        assert_eq!(deobfuscate("decodeURI(null);"), "'null';");
    }

    #[test]
    fn test_encode_uri_component() {
        assert_eq!(deobfuscate("encodeURIComponent('&');"), "'%26';");
        assert_eq!(
            deobfuscate("encodeURIComponent('https://mozilla.org/?x=шеллы');"),
            "'https%3A%2F%2Fmozilla.org%2F%3Fx%3D%D1%88%D0%B5%D0%BB%D0%BB%D1%8B';"
        );
    }

    #[test]
    fn test_decode_uri_component() {
        assert_eq!(deobfuscate("decodeURIComponent('%26');"), "'&';");
        assert_eq!(
            deobfuscate(
                "decodeURIComponent('https%3A%2F%2Fmozilla.org%2F%3Fx%3D%D1%88%D0%B5%D0%BB%D0%BB%D1%8B');"
            ),
            "'https://mozilla.org/?x=шеллы';"
        );
    }
}
