#[cfg(test)]
mod tests_ps_crypto {
    use crate::ps::build_powershell_tree;
    use crate::ps::crypto::{AesState, AesTransformFinalBlock, AesType};
    use crate::ps::encoding::{EncodingGetBytes, EncodingType};
    use crate::ps::forward::Forward;
    use crate::ps::integer::ParseInt;
    use crate::ps::linter::Linter;
    use crate::ps::method::DecodeBase64;
    use crate::ps::string::ParseString;
    use crate::ps::typing::ParseType;
    use crate::ps::var::Var;
    use base64::{Engine as _, engine::general_purpose};

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_powershell_tree(input).unwrap();
        tree.apply_mut(&mut (
            Forward::default(),
            ParseType::default(),
            ParseString::default(),
            ParseInt::default(),
            EncodingType::default(),
            EncodingGetBytes::default(),
            DecodeBase64::default(),
            AesType::default(),
            AesTransformFinalBlock::default(),
            Var::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    // Parses the `@(72, 101, ...)` rendering produced by the Linter for a `Bytes` value.
    fn parse_bytes_literal(output: &str) -> Vec<u8> {
        output
            .trim()
            .trim_start_matches("@(")
            .trim_end_matches(')')
            .split(',')
            .map(|s| s.trim().parse().unwrap())
            .collect()
    }

    #[test]
    fn test_aes_decrypt_new_object() {
        // ASCII codes of "Hello, minusone!"
        assert_eq!(
            deobfuscate(
                r#"(New-Object System.Security.Cryptography.AesManaged).CreateDecryptor([System.text.encoding]::UTF8.GetBytes("0123456789abcdef"), [System.text.encoding]::UTF8.GetBytes("abcdef0123456789")).TransformFinalBlock([Convert]::FromBase64String("H6eyhfvPHCP9VlON0Wfk9cx+9TT3kIu2PZ4DdCcvAaY="), 0, 32)"#
            ),
            "@(72, 101, 108, 108, 111, 44, 32, 109, 105, 110, 117, 115, 111, 110, 101, 33)"
        );
    }

    #[test]
    fn test_aes_decrypt_new_syntax() {
        assert_eq!(
            deobfuscate(
                r#"[System.Security.Cryptography.AesManaged]::new().CreateDecryptor([System.text.encoding]::UTF8.GetBytes("0123456789abcdef"), [System.text.encoding]::UTF8.GetBytes("abcdef0123456789")).TransformFinalBlock([Convert]::FromBase64String("H6eyhfvPHCP9VlON0Wfk9cx+9TT3kIu2PZ4DdCcvAaY="), 0, 32)"#
            ),
            "@(72, 101, 108, 108, 111, 44, 32, 109, 105, 110, 117, 115, 111, 110, 101, 33)"
        );
    }

    #[test]
    fn test_aes_encrypt_round_trip_via_create() {
        let output = deobfuscate(
            r#"[System.Security.Cryptography.Aes]::Create().CreateEncryptor([System.text.encoding]::UTF8.GetBytes("0123456789abcdef"), [System.text.encoding]::UTF8.GetBytes("abcdef0123456789")).TransformFinalBlock([System.text.encoding]::UTF8.GetBytes("secretmsg"), 0, 9)"#,
        );

        let ciphertext = parse_bytes_literal(&output);

        let plaintext = crate::ps::crypto::aes_transform(
            &AesState {
                key: Some(b"0123456789abcdef".to_vec()),
                iv: Some(b"abcdef0123456789".to_vec()),
                mode: None,
                padding: None,
                is_decrypt: Some(true),
            },
            true,
            &ciphertext,
        )
        .unwrap();
        assert_eq!(plaintext, b"secretmsg");
    }

    #[test]
    fn test_aes_decrypt_via_property_assignment_cfb() {
        let key = b"0123456789abcdef";
        let iv = b"abcdef0123456789";
        let plaintext = b"synthetic-msg!!!"; // 16 bytes, block-aligned so CFB8 needs no padding talk
        let ciphertext = crate::ps::crypto::aes_cfb8_encrypt(key, iv, plaintext).unwrap();
        let ciphertext_b64 = general_purpose::STANDARD.encode(&ciphertext);

        let source = format!(
            r#"
$aes = New-Object System.Security.Cryptography.AesCryptoServiceProvider
$aes.Mode = [System.Security.Cryptography.CipherMode]::CFB
$aes.Padding = [System.Security.Cryptography.PaddingMode]::None
$aes.Key = [System.text.encoding]::UTF8.GetBytes("0123456789abcdef")
$aes.IV = [System.text.encoding]::UTF8.GetBytes("abcdef0123456789")
$decryptor = $aes.CreateDecryptor()
$decryptor.TransformFinalBlock([Convert]::FromBase64String("{}"), 0, {})
"#,
            ciphertext_b64,
            ciphertext.len()
        );

        let output = deobfuscate(&source);
        let last_line = output.lines().next_back().unwrap();

        assert_eq!(parse_bytes_literal(last_line), plaintext.to_vec());
    }
}
