#[cfg(test)]
mod test_buffer {
    use crate::js::array::ParseArray;
    use crate::js::build_javascript_tree;
    use crate::js::forward::Forward;
    use crate::js::integer::{ParseInt, PosNeg};
    use crate::js::linter::Linter;
    use crate::js::node::buffer::{BufferAlloc, BufferFrom, BufferIndex, BufferToString};
    use crate::js::string::ParseString;
    use crate::js::var::Var;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            ParseArray::default(),
            BufferFrom::default(),
            BufferAlloc::default(),
            PosNeg::default(),
            Forward::default(),
            Var::default(),
            BufferIndex::default(),
            BufferToString::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_buffer_from_array() {
        assert_eq!(
            deobfuscate("const buf4 = Buffer.from([1, 2, 3]);"),
            "const buf4 = Buffer.from('010203', 'hex');"
        );
    }

    #[test]
    fn test_buffer_from_array_truncates_with_and_255() {
        assert_eq!(
            deobfuscate("const buf5 = Buffer.from([257, 257.5, -255, '1']);"),
            "const buf5 = Buffer.from('01010101', 'hex');"
        );
    }

    #[test]
    fn test_buffer_from_utf8() {
        assert_eq!(
            deobfuscate("const buf6 = Buffer.from('tést');"),
            "const buf6 = Buffer.from('74c3a97374', 'hex');"
        );
    }

    #[test]
    fn test_buffer_from_latin1_alias_binary() {
        assert_eq!(
            deobfuscate("const buf7 = Buffer.from('tést', 'latin1');"),
            "const buf7 = Buffer.from('74e97374', 'hex');"
        );
        assert_eq!(
            deobfuscate("const buf8 = Buffer.from('tést', 'binary');"),
            "const buf8 = Buffer.from('74e97374', 'hex');"
        );
    }

    #[test]
    fn test_buffer_from_encodings() {
        assert_eq!(
            deobfuscate("const b = Buffer.from('QQ==', 'base64');"),
            "const b = Buffer.from('41', 'hex');"
        );
        assert_eq!(
            deobfuscate("const b = Buffer.from('QQ', 'base64url');"),
            "const b = Buffer.from('41', 'hex');"
        );
        assert_eq!(
            deobfuscate("const b = Buffer.from('A', 'hex');"),
            "const b = Buffer.from('', 'hex');"
        );
        assert_eq!(
            deobfuscate("const b = Buffer.from('4142ZZ', 'hex');"),
            "const b = Buffer.from('4142', 'hex');"
        );
        assert_eq!(
            deobfuscate("const b = Buffer.from('A', 'ucs2');"),
            "const b = Buffer.from('4100', 'hex');"
        );
        assert_eq!(
            deobfuscate("const b = Buffer.from('A', 'utf16le');"),
            "const b = Buffer.from('4100', 'hex');"
        );
    }

    #[test]
    fn test_buffer_to_string_utf8_and_range() {
        assert_eq!(
            deobfuscate(
                "const buf1 = Buffer.from('abcdefghijklmnopqrstuvwxyz'); console.log(buf1.toString('utf8'));"
            ),
            "const buf1 = Buffer.from('6162636465666768696a6b6c6d6e6f707172737475767778797a', 'hex'); console.log('abcdefghijklmnopqrstuvwxyz');"
        );

        assert_eq!(
            deobfuscate(
                "const buf1 = Buffer.from('abcdefghijklmnopqrstuvwxyz'); console.log(buf1.toString('utf8', 0, 5));"
            ),
            "const buf1 = Buffer.from('6162636465666768696a6b6c6d6e6f707172737475767778797a', 'hex'); console.log('abcde');"
        );
    }

    #[test]
    fn test_buffer_to_string_hex_and_undefined_encoding() {
        assert_eq!(
            deobfuscate("const buf2 = Buffer.from('tést'); console.log(buf2.toString('hex'));"),
            "const buf2 = Buffer.from('74c3a97374', 'hex'); console.log('74c3a97374');"
        );

        assert_eq!(
            deobfuscate(
                "const buf2 = Buffer.from('tést'); console.log(buf2.toString('utf8', 0, 3));"
            ),
            "const buf2 = Buffer.from('74c3a97374', 'hex'); console.log('té');"
        );

        assert_eq!(
            deobfuscate(
                "const buf2 = Buffer.from('tést'); console.log(buf2.toString(undefined, 0, 3));"
            ),
            "const buf2 = Buffer.from('74c3a97374', 'hex'); console.log('té');"
        );
    }

    #[test]
    fn test_buffer_alloc() {
        assert_eq!(
            deobfuscate("console.log(Buffer.alloc(10).toString('hex'));"),
            "console.log('00000000000000000000');"
        );
        assert_eq!(
            deobfuscate("console.log(Buffer.alloc(10, 1).toString('hex'));"),
            "console.log('01010101010101010101');"
        );
        assert_eq!(
            deobfuscate("console.log(Buffer.alloc(10, 'ABC').toString('hex'));"),
            "console.log('41424341424341424341');"
        );
        assert_eq!(
            deobfuscate("console.log(Buffer.alloc(10, 'ABC').toString());"),
            "console.log('ABCABCABCA');"
        );
    }

    #[test]
    fn test_buffer_index_read_and_write() {
        assert_eq!(
            deobfuscate("let s = Buffer.from('ABCDEFGHIJ', 'utf8'); console.log(s[0], s[9]);"),
            "let s = Buffer.from('4142434445464748494a', 'hex'); console.log(65, 74);"
        );

        assert_eq!(
            deobfuscate(
                "let s = Buffer.alloc(3); s[0] = 65; s[1] = 66; s[2] = 67; console.log(s.toString());"
            ),
            "let s = Buffer.from('000000', 'hex'); s[0] = 65; s[1] = 66; s[2] = 67; console.log('ABC');"
        );
    }
}
