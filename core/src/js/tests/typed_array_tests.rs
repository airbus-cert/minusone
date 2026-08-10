#[cfg(test)]
mod test_typed_array {
    use crate::js::array::ParseArray;
    use crate::js::b64::B64;
    use crate::js::build_javascript_tree;
    use crate::js::forward::Forward;
    use crate::js::integer::{BitwiseInt, ParseInt, PosNeg};
    use crate::js::linter::Linter;
    use crate::js::r#loop::ArrayMapFilter;
    use crate::js::node::buffer::{BufferIndex, BufferToString};
    use crate::js::string::{CharCodeAt, ParseString, StringBuiltins};
    use crate::js::typed_array::{ParseUint8Array, TextDecoderDecode};
    use crate::js::var::Var;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            ParseArray::default(),
            PosNeg::default(),
            B64::default(),
            BitwiseInt::default(),
            StringBuiltins::default(),
            CharCodeAt::default(),
            ArrayMapFilter::default(),
            ParseUint8Array::default(),
            Forward::default(),
            Var::default(),
            BufferIndex::default(),
            BufferToString::default(),
            TextDecoderDecode::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_uint8array_from_array_literal() {
        assert_eq!(
            deobfuscate("console.log(new Uint8Array([65, 66, 67]).toString());"),
            "console.log('ABC');"
        );
    }

    #[test]
    fn test_uint8array_truncates_and_masks_like_node_buffer() {
        // values are truncated then wrapped modulo 256, matching to_uint8's fuffer semantics
        assert_eq!(
            deobfuscate("console.log(new Uint8Array([257, 257.5, -1]).toString('hex'));"),
            "console.log('0101ff');"
        );
    }

    #[test]
    fn test_uint8array_no_args_is_empty() {
        assert_eq!(
            deobfuscate("console.log(new Uint8Array().toString());"),
            "console.log('');"
        );
    }

    #[test]
    fn test_uint8array_from_length_is_zero_filled() {
        assert_eq!(
            deobfuscate("console.log(new Uint8Array(3).toString('hex'));"),
            "console.log('000000');"
        );
    }

    #[test]
    fn test_uint8array_index_read() {
        assert_eq!(
            deobfuscate("const u = new Uint8Array([65, 66, 67]); console.log(u[0], u[2]);"),
            "const u = Buffer.from('414243', 'hex'); console.log(65, 67);"
        );
    }

    #[test]
    fn test_textdecoder_decodes_uint8array() {
        assert_eq!(
            deobfuscate(
                "console.log(new TextDecoder().decode(new Uint8Array([72, 101, 108, 108, 111])));"
            ),
            "console.log('Hello');"
        );
    }

    #[test]
    fn test_textdecoder_decodes_array_directly() {
        assert_eq!(
            deobfuscate("console.log(new TextDecoder().decode([72, 105]));"),
            "console.log('Hi');"
        );
    }

    #[test]
    fn test_textdecoder_no_args_is_empty_string() {
        assert_eq!(
            deobfuscate("console.log(new TextDecoder().decode());"),
            "console.log('');"
        );
    }

    #[test]
    fn test_textdecoder_utf16le_label() {
        assert_eq!(
            deobfuscate(
                "console.log(new TextDecoder('utf-16le').decode(new Uint8Array([72, 0, 105, 0])));"
            ),
            "console.log('Hi');"
        );
    }

    #[test]
    fn test_textdecoder_replaces_invalid_utf8_with_replacement_char() {
        assert_eq!(
            deobfuscate("console.log(new TextDecoder().decode(new Uint8Array([0xFF])));"),
            "console.log('\u{FFFD}');"
        );
    }

    #[test]
    fn test_full_xor_decode_pipeline() {
        // atob(...).split("").map(x => x.charCodeAt(0) ^ key) -> Uint8Array -> TextDecoder,
        // this come from a sample
        assert_eq!(
            deobfuscate(
                r#"
                const czuft = atob("zsLD3sLByIPBwsqFnIQ=")
                    .split("")
                    .map(x => x.charCodeAt(0) ^ 173);
                const dpef = new TextDecoder().decode(new Uint8Array(czuft));
                console.log(dpef);
                "#
            ),
            "const czuft = [99, 111, 110, 115, 111, 108, 101, 46, 108, 111, 103, 40, 49, 41];\n                const dpef = 'console.log(1)';\n                console.log('console.log(1)');\n                "
        );
    }
}
