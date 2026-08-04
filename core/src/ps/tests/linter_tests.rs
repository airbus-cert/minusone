#[cfg(test)]
mod tests_ps_linter {
    use crate::ps::build_powershell_tree;
    use crate::ps::forward::Forward;
    use crate::ps::linter::Linter;
    use crate::ps::string::ParseString;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_powershell_tree(input).unwrap();
        tree.apply_mut(&mut (ParseString::default(), Forward::default()))
            .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_escape_newline_on_output() {
        assert_eq!(deobfuscate("\"a`nb\""), "\"a`nb\"");
    }

    #[test]
    fn test_escape_dollar_on_output() {
        assert_eq!(deobfuscate("\"a`$b\""), "\"a`$b\"");
    }

    #[test]
    fn test_escape_backtick_on_output() {
        assert_eq!(deobfuscate("\"a``b\""), "\"a``b\"");
    }

    #[test]
    fn test_escape_control_chars_on_output() {
        assert_eq!(deobfuscate("\"a`tb`0c\""), "\"a`tb`0c\"");
    }
}
