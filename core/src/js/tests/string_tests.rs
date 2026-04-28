#[cfg(test)]
mod tests_js_string {
    use crate::js::array::{GetArrayElement, ParseArray};
    use crate::js::forward::Forward;
    use crate::js::integer::{AddInt, ParseInt, PosNeg};
    use crate::js::linter::Linter;
    use crate::js::post_process::BracketCallToMember;
    use crate::js::regex::ParseRegex;
    use crate::js::specials::AddSubSpecials;
    use crate::js::string::*;
    use crate::js::var::Var;
    use crate::js::{build_javascript_tree, build_javascript_tree_for_storage};
    use crate::tree::EmptyStorage;

    fn deobfuscate(input: &str) -> String {
        let tree = build_javascript_tree_for_storage::<EmptyStorage>(input).unwrap();
        let mut bracket_to_member = BracketCallToMember::default();
        tree.apply(&mut bracket_to_member).unwrap();
        let input = bracket_to_member.clear().unwrap();

        let mut tree = build_javascript_tree(&input).unwrap();
        tree.apply_mut(&mut (
            ParseString::default(),
            ParseInt::default(),
            ParseArray::default(),
            ParseRegex::default(),
            StringBuiltins::default(),
            Forward::default(),
            PosNeg::default(),
            BracketCharAt::default(),
            CharCodeAt::default(),
            FromCharCode::default(),
            StringConstructor::default(),
            Concat::default(),
            AddInt::default(),
            StringRaw::default(),
            TemplateString::default(),
            GetArrayElement::default(),
            ToString::default(),
            AddSubSpecials::default(),
            Var::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_unescaped_js_string() {
        assert_eq!(unescaped_js_string(r#"'Hello\nWorld'"#), "Hello\nWorld");
        assert_eq!(unescaped_js_string(r#"'Tab\tSeparated'"#), "Tab\tSeparated");
        assert_eq!(unescaped_js_string(r#"'Quote: \"'"#), "Quote: \"");
        assert_eq!(unescaped_js_string(r#"'Backslash: \\'"#), "Backslash: \\");
        assert_eq!(unescaped_js_string(r#"'Unicode: \u0041'"#), "Unicode: A");
        assert_eq!(
            unescaped_js_string(
                r#"'Unicode: \u0030 \u{00030} \u{000030} \u{0000000000000030} \u{30}'"#
            ),
            "Unicode: 0 0 0 0 0"
        );
        assert_eq!(unescaped_js_string(r#"'Hex: \x41'"#), "Hex: A");
    }

    #[test]
    fn test_escape_js_string() {
        assert_eq!(escape_js_string("Hello\nWorld"), r#"'Hello\nWorld'"#);
        assert_eq!(escape_js_string("Tab\tSeparated"), r#"'Tab\tSeparated'"#);
        assert_eq!(escape_js_string("Quote: \""), r#"'Quote: "'"#);
        assert_eq!(escape_js_string("Backslash: \\"), r#"'Backslash: \\'"#);
    }

    #[test]
    fn test_concat() {
        assert_eq!(
            deobfuscate("var x = 'Hello, ' + 'world!' + 1;"),
            "var x = 'Hello, world!1';"
        );
    }

    #[test]
    fn test_charat() {
        assert_eq!(deobfuscate("var x = 'abc'.charAt();"), "var x = 'a';");
        assert_eq!(deobfuscate("var x = 'abc'.charAt(1);"), "var x = 'b';");
        assert_eq!(deobfuscate("var x = 'abc'['charAt'](2);"), "var x = 'c';");
        assert_eq!(deobfuscate("var x = 'abc'.charAt(3);"), "var x = '';");
        assert_eq!(deobfuscate("var x = 'abc'.charAt(-3);"), "var x = '';");
        assert_eq!(deobfuscate("var x = 'abc'.charAt('1');"), "var x = 'b';");
        assert_eq!(deobfuscate("var x = 'test'[1];"), "var x = 'e';");
        assert_eq!(deobfuscate("var x = 'test'[10];"), "var x = undefined;");
        assert_eq!(deobfuscate("var x = 'abc'[0];"), "var x = 'a';");
        assert_eq!(deobfuscate("var x = 'abc'[-1];"), "var x = undefined;");
        assert_eq!(deobfuscate("var x = 'abc'[3];"), "var x = undefined;");
    }

    #[test]
    fn test_at() {
        assert_eq!(deobfuscate("var x = 'abc'.at();"), "var x = 'a';");
        assert_eq!(deobfuscate("var x = 'abc'.at(1);"), "var x = 'b';");
        assert_eq!(deobfuscate("var x = 'abc'.at(-1);"), "var x = 'c';");
        assert_eq!(deobfuscate("var x = 'abc'.at('2');"), "var x = 'c';");
        assert_eq!(deobfuscate("var x = 'abc'['at']('-2');"), "var x = 'b';");
        assert_eq!(deobfuscate("var x = 'abc'.at(10);"), "var x = undefined;");
    }

    #[test]
    fn test_charat_concat() {
        assert_eq!(
            deobfuscate(
                "var x = 'minusone'[0] + 'minusone'[1] + 'minusone'[2] + 'minusone'[3] + 'minusone'[4] + 'minusone'[5] + 'minusone'[6] + 'minusone'[7];"
            ),
            "var x = 'minusone';"
        );
    }

    #[test]
    fn test_charcodeat() {
        assert_eq!(deobfuscate("var x = 'ABC'.charCodeAt(0);"), "var x = 65;");
        assert_eq!(deobfuscate("var x = 'ABC'.charCodeAt(14);"), "var x = NaN;");
    }

    #[test]
    fn test_from_char_code() {
        assert_eq!(
            deobfuscate(
                "var x = String.fromCharCode(0x6D, 0x69, 0x6E, 0x75, 0x73, 0x6F, 0x6E, 0x65);"
            ),
            "var x = 'minusone';"
        );
        assert_eq!(
            deobfuscate("var x = String['fromCharCode'](65, 66, 67);"),
            "var x = 'ABC';"
        );
    }

    #[test]
    fn test_code_point_at() {
        assert_eq!(deobfuscate("var x = 'abc'.codePointAt(1);"), "var x = 98;");
        assert_eq!(deobfuscate("var x = 'abc'.codePointAt();"), "var x = 97;");
        assert_eq!(
            deobfuscate("var x = '☃★♲'.codePointAt(1);"),
            "var x = 9733;"
        );
        assert_eq!(
            deobfuscate("var x = 'abc'.codePointAt(-1);"),
            "var x = undefined;"
        );
        assert_eq!(
            deobfuscate("var x = 'abc'.codePointAt(3);"),
            "var x = undefined;"
        );
    }

    #[test]
    fn test_string_constructor() {
        assert_eq!(deobfuscate("var x = String(1);"), "var x = '1';");
        assert_eq!(deobfuscate("var x = String();"), "var x = '';");
    }

    #[test]
    fn test_string_plus_minus() {
        assert_eq!(
            deobfuscate("var x = +'42'; var y = -'42';"),
            "var x = 42; var y = -42;"
        );
        assert_eq!(deobfuscate("var x = +'0xff';"), "var x = 255;");
        assert_eq!(deobfuscate("var x = +'-0x56';"), "var x = NaN;");
        assert_eq!(deobfuscate("var x = +'-56';"), "var x = -56;");
        assert_eq!(
            deobfuscate("var x = 'b' + 'a' + +'a' + 'a'"),
            "var x = 'baNaNa'"
        );
    }

    #[test]
    fn test_start_with() {
        assert_eq!(
            deobfuscate("var x = '123'.startsWith('1');"),
            "var x = true;"
        );
        assert_eq!(
            deobfuscate("var x = '123'.startsWith('2');"),
            "var x = false;"
        );
        assert_eq!(
            deobfuscate("var x = '123'.startsWith([1]);"),
            "var x = true;"
        );
        assert_eq!(
            deobfuscate("var x = '123'.startsWith('');"),
            "var x = true;"
        );
        assert_eq!(
            deobfuscate("var x = '123'.startsWith([]);"),
            "var x = true;"
        );
    }

    #[test]
    fn test_end_with() {
        assert_eq!(deobfuscate("var x = '123'.endsWith('3');"), "var x = true;");
        assert_eq!(
            deobfuscate("var x = '123'.endsWith('2');"),
            "var x = false;"
        );
        assert_eq!(deobfuscate("var x = '123'.endsWith([3]);"), "var x = true;");
        assert_eq!(deobfuscate("var x = '123'.endsWith('');"), "var x = true;");
        assert_eq!(deobfuscate("var x = '123'.endsWith([]);"), "var x = true;");
    }

    #[test]
    fn test_includes() {
        assert_eq!(deobfuscate("var x = '123'.includes('3');"), "var x = true;");
        assert_eq!(deobfuscate("var x = '123'.includes('2');"), "var x = true;");
        assert_eq!(
            deobfuscate("var x = '123'.includes('4');"),
            "var x = false;"
        );
        assert_eq!(deobfuscate("var x = '123'.includes([1]);"), "var x = true;");
        assert_eq!(deobfuscate("var x = '123'.includes('');"), "var x = true;");
        assert_eq!(deobfuscate("var x = '123'.includes([]);"), "var x = true;");
    }

    #[test]
    fn test_index_of() {
        assert_eq!(deobfuscate("var x = '123'.indexOf('3');"), "var x = 2;");
        assert_eq!(deobfuscate("var x = '123'.indexOf('2');"), "var x = 1;");
        assert_eq!(deobfuscate("var x = '123'.indexOf('4');"), "var x = -1;");
        assert_eq!(deobfuscate("var x = '123'.indexOf([1]);"), "var x = 0;");
        assert_eq!(deobfuscate("var x = '123'.indexOf('');"), "var x = 0;");
        assert_eq!(deobfuscate("var x = '123'.indexOf();"), "var x = -1;");
        assert_eq!(deobfuscate("var x = '123'.indexOf([]);"), "var x = 0;");
    }

    #[test]
    fn test_last_index_of() {
        assert_eq!(
            deobfuscate("var x = '123123'.lastIndexOf('3');"),
            "var x = 5;"
        );
        assert_eq!(
            deobfuscate("var x = '123123'.lastIndexOf('2');"),
            "var x = 4;"
        );
        assert_eq!(
            deobfuscate("var x = '123123'.lastIndexOf('4');"),
            "var x = -1;"
        );
        assert_eq!(
            deobfuscate("var x = '123123'.lastIndexOf([1]);"),
            "var x = 3;"
        );
        assert_eq!(
            deobfuscate("var x = '123123'.lastIndexOf('');"),
            "var x = 6;"
        );
        assert_eq!(
            deobfuscate("var x = '123123'.lastIndexOf();"),
            "var x = -1;"
        );
        assert_eq!(
            deobfuscate("var x = '123123'.lastIndexOf([]);"),
            "var x = 6;"
        );
    }

    #[test]
    fn to_upper_or_lower_case() {
        assert_eq!(
            deobfuscate("var x = 'abc'.toUpperCase();"),
            "var x = 'ABC';"
        );
        assert_eq!(
            deobfuscate("var x = 'ABC'.toLowerCase();"),
            "var x = 'abc';"
        );
    }

    #[test]
    fn test_trim() {
        assert_eq!(deobfuscate("var x = '  abc  '.trim();"), "var x = 'abc';");
        assert_eq!(
            deobfuscate("var x = '\\t\\nabc\\n\\t'.trim();"),
            "var x = 'abc';"
        );
        assert_eq!(
            deobfuscate("var x = '  abc  '.trimStart();"),
            "var x = 'abc  ';"
        );
        assert_eq!(
            deobfuscate("var x = '  abc  '.trimEnd();"),
            "var x = '  abc';"
        );
    }

    #[test]
    fn test_pad() {
        assert_eq!(
            deobfuscate("var x = '123'.padStart(5, '0');"),
            "var x = '00123';"
        );
        assert_eq!(
            deobfuscate("var x = '123'.padEnd(5, '0');"),
            "var x = '12300';"
        );
        assert_eq!(
            deobfuscate("var x = '123'.padStart(5);"),
            "var x = '  123';"
        );
        assert_eq!(deobfuscate("var x = '123'.padEnd(5);"), "var x = '123  ';");
    }

    #[test]
    fn test_repeat() {
        assert_eq!(
            deobfuscate("var x = 'abc'.repeat(3);"),
            "var x = 'abcabcabc';"
        );
        assert_eq!(deobfuscate("var x = 'abc'.repeat(0);"), "var x = '';");
        assert_eq!(deobfuscate("var x = 'abc'.repeat(1.5);"), "var x = 'abc';");
    }

    #[test]
    fn test_slice() {
        assert_eq!(
            deobfuscate("var x = 'abcdef'.slice(1, 4);"),
            "var x = 'bcd';"
        );
        assert_eq!(deobfuscate("var x = 'abcdef'.slice(2);"), "var x = 'cdef';");
        assert_eq!(deobfuscate("var x = 'abcdef'.slice(-3);"), "var x = 'def';");
        assert_eq!(
            deobfuscate("var x = 'abcdef'.slice(-4, -1);"),
            "var x = 'cde';"
        );
        assert_eq!(deobfuscate("var x = 'abcdef'.slice(2, 1);"), "var x = '';");
        assert_eq!(deobfuscate("var x = 'abcdef'.slice(10);"), "var x = '';");
        assert_eq!(
            deobfuscate("var x = 'abcdef'.slice();"),
            "var x = 'abcdef';"
        );
    }

    #[test]
    fn test_substring() {
        assert_eq!(
            deobfuscate("var x = 'abcdef'.substring(1, 4);"),
            "var x = 'bcd';"
        );
        assert_eq!(
            deobfuscate("var x = 'abcdef'.substring(4, 1);"),
            "var x = 'bcd';"
        );
        assert_eq!(
            deobfuscate("var x = 'abcdef'.substring(2);"),
            "var x = 'cdef';"
        );
        assert_eq!(
            deobfuscate("var x = 'abcdef'.substring(-3);"),
            "var x = 'abcdef';"
        );
        assert_eq!(
            deobfuscate("var x = 'abcdef'.substring(-4, -1);"),
            "var x = '';"
        );
        assert_eq!(
            deobfuscate("var x = 'abcdef'.substring(2, 1);"),
            "var x = 'b';"
        );
        assert_eq!(
            deobfuscate("var x = 'abcdef'.substring(10);"),
            "var x = '';"
        );
        assert_eq!(
            deobfuscate("var x = 'abcdef'.substring();"),
            "var x = 'abcdef';"
        );
    }

    #[test]
    fn test_to_string_dot_and_subscript() {
        assert_eq!(deobfuscate("var x = (1)['toString']();"), "var x = '1';");
        assert_eq!(deobfuscate("var x = (1).toString();"), "var x = '1';");
    }

    #[test]
    fn test_split_with_params() {
        assert_eq!(
            deobfuscate("var x = 'alert164t50t471t47t51'['split']('t')[0];"),
            "var x = 'aler';"
        );
        assert_eq!(
            deobfuscate("var x = 'a,b,c'.split(',', 2)[1];"),
            "var x = 'b';"
        );
    }

    #[test]
    fn test_replace() {
        // string
        assert_eq!(
            deobfuscate("var x = 'a,b,c'.replace(',', '');"),
            "var x = 'ab,c';"
        );
        assert_eq!(
            deobfuscate("var x = 'a,b,c'.replaceAll(',', '');"),
            "var x = 'abc';"
        );

        // num
        assert_eq!(
            deobfuscate("var x = '124'.replaceAll(4, 3);"),
            "var x = '123';"
        );

        // regex
        assert_eq!(
            deobfuscate("var x = 'a,b,c'.replaceAll(/,/g, '');"),
            "var x = 'abc';"
        );
        assert_eq!(
            deobfuscate("var x = 'a,b,c'.replaceAll(/,/, '');"),
            "var x = 'a,b,c'.replaceAll(/,/, '');"
        );
        assert_eq!(
            deobfuscate("var x = 'a,b,c'.replace(/,/g, '');"),
            "var x = 'abc';"
        );
        assert_eq!(
            deobfuscate("var x = 'a,b,c'.replace(/,/, '');"),
            "var x = 'ab,c';"
        );
    }

    #[test]
    fn test_dynamic_tags() {
        assert_eq!(
            deobfuscate("var x = 'minusone'.link('https://minusone.skyblue.team/');"),
            "var x = '<a href=\"https://minusone.skyblue.team/\">minusone</a>';"
        );
        assert_eq!(
            deobfuscate("var x = 'minusone'.anchor('minusone');"),
            "var x = '<a name=\"minusone\">minusone</a>';"
        );
        assert_eq!(
            deobfuscate("var x = 'minusone'.link();"),
            "var x = '<a href=\"undefined\">minusone</a>';"
        );
        assert_eq!(
            deobfuscate("var x = 'minusone'.anchor();"),
            "var x = '<a name=\"undefined\">minusone</a>';"
        );
        assert_eq!(
            deobfuscate("var x = 'minusone'['link']('https://minusone.skyblue.team/');"),
            "var x = '<a href=\"https://minusone.skyblue.team/\">minusone</a>';"
        );
        assert_eq!(
            deobfuscate("var x = 'minusone'.link('\"');"),
            "var x = '<a href=\"&quot;\">minusone</a>';"
        );
    }

    #[test]
    fn test_template_string() {
        assert_eq!(
            deobfuscate("var x = `hello ${1} ${'world'}`;"),
            "var x = 'hello 1 world';"
        );

        assert_eq!(
            deobfuscate("var x = `hello ${1} ${1+1}`;"),
            "var x = 'hello 1 2';"
        );

        assert_eq!(
            deobfuscate("console.log(`hello ${a} ${1+1}`)"),
            "console.log(`hello ${a} 2`)"
        );
    }

    #[test]
    fn test_string_raw_tagged_template() {
        assert_eq!(
            deobfuscate("console.log(String.raw`minusone`)"),
            "console.log('minusone')"
        );

        assert_eq!(
            deobfuscate("let a = 'a'; console.log(String.raw`${a}`);"),
            "let a = 'a'; console.log('a');"
        );

        assert_eq!(
            deobfuscate("let a = 1; console.log(String.raw`${a + 1}`);"),
            "let a = 1; console.log('2');"
        );
    }
}
