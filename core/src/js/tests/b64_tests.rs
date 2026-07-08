#[cfg(test)]
mod tests_js_b64 {
    use crate::js::b64::B64;
    use crate::js::b64::js_bytes_to_string;
    use crate::js::build_javascript_tree;
    use crate::js::linter::Linter;
    use crate::js::string::ParseString;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (ParseString::default(), B64::default()))
            .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_parse_b64() {
        assert_eq!(
            deobfuscate("var x = atob('bWludXNvbmU=');"),
            "var x = 'minusone';",
        );
    }

    #[test]
    fn test_parse_b64_encode() {
        assert_eq!(
            deobfuscate("var x = btoa('minusone');"),
            "var x = 'bWludXNvbmU=';",
        );
    }

    #[test]
    fn test_bytes_to_string() {
        let mut bytes = Vec::new();
        for i in 0..=255 {
            bytes.push(i);
        }
        let decoded_string = js_bytes_to_string(&bytes);
        println!("{}", decoded_string);
        assert_eq!(
            decoded_string,
            "\\x00\\x01\\x02\\x03\\x04\\x05\\x06\\x07\\b\\t\\n\\v\\f\\r\\x0E\\x0F\\x10\\x11\\x12\\x13\\x14\\x15\\x16\\x17\\x18\\x19\\x1A\\x1B\\x1C\\x1D\\x1E\\x1F !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~\\x7F\\x80\\x81\\x82\\x83\\x84\\x85\\x86\\x87\\x88\\x89\\x8A\\x8B\\x8C\\x8D\\x8E\\x8F\\x90\\x91\\x92\\x93\\x94\\x95\\x96\\x97\\x98\\x99\\x9A\\x9B\\x9C\\x9D\\x9E\\x9F ¡¢£¤¥¦§¨©ª«¬­®¯°±²³´µ¶·¸¹º»¼½¾¿ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕÖ×ØÙÚÛÜÝÞßàáâãäåæçèéêëìíîïðñòóôõö÷øùúûüýþÿ"
        );
    }
}
