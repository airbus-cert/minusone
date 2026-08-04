#[cfg(test)]
mod tests_ps_compression {
    use crate::ps::build_powershell_tree;
    use crate::ps::compression::{StreamReadToEnd, StreamType};
    use crate::ps::forward::Forward;
    use crate::ps::integer::ParseInt;
    use crate::ps::linter::Linter;
    use crate::ps::method::DecodeBase64;
    use crate::ps::string::ParseString;
    use crate::ps::typing::ParseType;
    use crate::ps::var::Var;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_powershell_tree(input).unwrap();
        tree.apply_mut(&mut (
            Forward::default(),
            ParseType::default(),
            ParseString::default(),
            ParseInt::default(),
            DecodeBase64::default(),
            Var::default(),
            StreamType::default(),
            StreamReadToEnd::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_gzip_decompress() {
        let src = r#"$s=New-Object IO.MemoryStream(,[Convert]::FromBase64String("H4sIAAAAAAAC//NIzcnJ11HIzcwrLc7PS1UEAOUrYEIQAAAA"));IEX ((New-Object IO.StreamReader((New-Object IO.Compression.GzipStream($s,[IO.Compression.CompressionMode]::Decompress)))).ReadToEnd())"#;
        assert!(deobfuscate(src).contains("Hello, minusone!"));
    }

    #[test]
    fn test_deflate_decompress() {
        let src = r#"$s=New-Object IO.MemoryStream(,[Convert]::FromBase64String("80jNycnXUcjNzCstzs9LVQQA"));IEX ((New-Object IO.StreamReader((New-Object IO.Compression.DeflateStream($s,[IO.Compression.CompressionMode]::Decompress)))).ReadToEnd())"#;
        assert!(deobfuscate(src).contains("Hello, minusone!"));
    }

    #[test]
    fn test_zlib_decompress() {
        let src = r#"$s=New-Object IO.MemoryStream(,[Convert]::FromBase64String("eJzzSM3JyddRyM3MKy3Oz0tVBAAxRAXQ"));IEX ((New-Object IO.StreamReader((New-Object IO.Compression.ZLibStream($s,[IO.Compression.CompressionMode]::Decompress)))).ReadToEnd())"#;
        assert!(deobfuscate(src).contains("Hello, minusone!"));
    }

    #[test]
    fn test_compression_mode_type_resolution() {
        assert_eq!(
            deobfuscate("[IO.Compression.CompressionMode]::Decompress"),
            "[Io.compression.compressionmode]::Decompress"
        );
    }

    #[test]
    fn test_compress_mode_is_not_decompressed() {
        let src = r#"$s=New-Object IO.MemoryStream(,[Convert]::FromBase64String("H4sIAAAAAAAC//NIzcnJ11HIzcwrLc7PS1UEAOUrYEIQAAAA"));IEX ((New-Object IO.StreamReader((New-Object IO.Compression.GzipStream($s,[IO.Compression.CompressionMode]::Compress)))).ReadToEnd())"#;
        assert!(!deobfuscate(src).contains("Hello, minusone!"));
    }
}
