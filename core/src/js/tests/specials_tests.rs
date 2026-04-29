#[cfg(test)]
mod tests_js_specials {
    use crate::js::array::*;
    use crate::js::bool::ParseBool;
    use crate::js::build_javascript_tree;
    use crate::js::forward::Forward;
    use crate::js::integer::ParseInt;
    use crate::js::linter::Linter;
    use crate::js::specials::*;
    use crate::js::string::ParseString;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            ParseBool::default(),
            ParseArray::default(),
            ParseSpecials::default(),
            AddSubSpecials::default(),
            CombineArrays::default(),
            GetArrayElement::default(),
            Forward::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_parse_specials() {
        assert_eq!(deobfuscate("var x = undefined;"), "var x = undefined;");
        assert_eq!(deobfuscate("var x = NaN;"), "var x = NaN;");
    }

    #[test]
    fn test_empty_array_plus_undefined() {
        assert_eq!(
            deobfuscate("var x = ([1][2]) + [];"),
            "var x = 'undefined';"
        );
    }

    #[test]
    fn test_empty_array_plus_nan() {
        assert_eq!(deobfuscate("var x = [] + NaN;"), "var x = 'NaN';");
    }

    #[test]
    fn test_undefined_plus_number_gives_nan() {
        assert_eq!(deobfuscate("var x = undefined + 1;"), "var x = NaN;");
    }

    #[test]
    fn test_special_plus_string() {
        assert_eq!(
            deobfuscate("var x = undefined + 'hello';"),
            "var x = 'undefinedhello';"
        );
        assert_eq!(
            deobfuscate("var x = 'cheese' + NaN;"),
            "var x = 'cheeseNaN';"
        );
    }

    #[test]
    fn test_array_plus_special() {
        assert_eq!(
            deobfuscate("var x = [1, 2] + undefined;"),
            "var x = '1,2undefined';"
        );
        assert_eq!(deobfuscate("var x = [1, 2] + NaN;"), "var x = '1,2NaN';");
    }
}
