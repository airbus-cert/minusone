#[cfg(test)]
mod tests_js_array {
    use crate::js::array::*;
    use crate::js::build_javascript_tree;
    use crate::js::forward::Forward;
    use crate::js::integer::{ParseInt, Substract};
    use crate::js::linter::Linter;
    use crate::js::specials::AddSubSpecials;
    use crate::js::string::BracketCharAt;
    use crate::js::string::ParseString;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            ParseArray::default(),
            CombineArrays::default(),
            Forward::default(),
            Substract::default(),
            GetArrayElement::default(),
            ArrayPlusMinus::default(),
            AddSubSpecials::default(),
            BracketCharAt::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_array_parsing() {
        assert_eq!(
            deobfuscate("var x = [1, 2, [3, '4']]"),
            "var x = [1, 2, [3, '4']]"
        );
    }

    #[test]
    fn test_combine_arrays() {
        assert_eq!(
            deobfuscate("var x = [0, 1,7] + [3, [7, '2', [88]]]"),
            "var x = '0,1,73,7,2,88'"
        );
    }

    #[test]
    fn test_get_array_element() {
        assert_eq!(
            deobfuscate("var x = ([1, [2, '3'], 4][1])[0];"),
            "var x = 2;"
        );
    }

    #[test]
    fn test_array_plus_minus() {
        assert_eq!(deobfuscate("var x = +[['455']];"), "var x = 455;");

        assert_eq!(deobfuscate("var x = +['a'];"), "var x = NaN;");

        assert_eq!(deobfuscate("var x = [8] - 1;"), "var x = 7;");
    }

    #[test]
    fn test_jsfuck_from_array_access() {
        assert_eq!(deobfuscate("var x = ([][[]]+[])[1];"), "var x = 'n';");
    }

    #[test]
    fn test_dont_reduce_array_lookup_when_used_as_callee() {
        assert_eq!(deobfuscate("var x = [][[]]();"), "var x = [][[]]();");
    }
}
