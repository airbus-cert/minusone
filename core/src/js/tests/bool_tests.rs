#[cfg(test)]
mod tests_js_bool {
    use crate::js::bool::*;
    use crate::js::build_javascript_tree;
    use crate::js::integer::{ParseInt, PosNeg, Substract};
    use crate::js::linter::Linter;
    use crate::js::string::ParseString;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseBool::default(),
            ParseInt::default(),
            ParseString::default(),
            Substract::default(),
            NotBool::default(),
            BoolAlgebra::default(),
            PosNeg::default(),
            AddBool::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_parse_bool() {
        assert_eq!(
            deobfuscate("var x = true; var y = false;"),
            "var x = true; var y = false;",
        );
    }

    #[test]
    fn test_not_bool() {
        assert_eq!(deobfuscate("var x = !true;"), "var x = false;",);
    }

    #[test]
    fn test_bool_algebra() {
        assert_eq!(
            deobfuscate("var x = true && false || true;"),
            "var x = true;",
        );
        assert_eq!(deobfuscate("var x = '' && 555;"), "var x = '';",);
        assert_eq!(deobfuscate("var x = '' || 555;"), "var x = 555;",);
    }

    #[test]
    fn test_add_bool() {
        assert_eq!(deobfuscate("var x = true + false - true;"), "var x = 0;",);
    }

    #[test]
    fn test_bool_plus_minus() {
        assert_eq!(
            deobfuscate("var x = +true; var y = -false;"),
            "var x = 1; var y = 0;",
        );
    }
}
