#[cfg(test)]
mod tests_js_regex {
    use crate::js::build_javascript_tree;
    use crate::js::integer::ParseInt;
    use crate::js::linter::Linter;
    use crate::js::objects::object::ObjectField;
    use crate::js::regex::*;
    use crate::js::string::{Concat, ParseString};

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            ParseRegex::default(),
            ParseInt::default(),
            Concat::default(),
            RegexConcat::default(),
            ObjectField::default(),
            RegexExec::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_parse_regex_literal() {
        assert_eq!(deobfuscate("var r = /ab+/gi;"), "var r = /ab+/gi;");
    }

    #[test]
    fn test_parse_regex_constructor() {
        assert_eq!(
            deobfuscate("var r = RegExp('ab+', 'i');"),
            "var r = /ab+/i;"
        );
        assert_eq!(
            deobfuscate("var r = new RegExp('a+', 'm');"),
            "var r = /a+/m;"
        );
    }

    #[test]
    fn test_regex_test_and_exec() {
        assert_eq!(
            deobfuscate("var a = /ab+/.test('zabbbz');"),
            "var a = true;"
        );
        assert_eq!(deobfuscate("var a = /ab+/.test('zzz');"), "var a = false;");
        assert_eq!(
            deobfuscate("var m = /a(b+)/.exec('zabbbz');"),
            "var m = ['abbb', 'bbb'];"
        );
        assert_eq!(deobfuscate("var m = /a+/.exec('zzz');"), "var m = null;");
    }

    #[test]
    fn test_regexp_concat() {
        assert_eq!(
            deobfuscate("var m = RegExp + '';"),
            "var m = 'function RegExp() { [native code] }';"
        );
    }

    #[test]
    fn test_regex_concat() {
        assert_eq!(deobfuscate("var m = /a/ + 'a';"), "var m = '/a/a';");
        assert_eq!(deobfuscate("var m = /a/ + 1;"), "var m = '/a/1';");
        assert_eq!(deobfuscate("var m = /a/g + /a/i;"), "var m = '/a/g/a/i';");
    }
}
