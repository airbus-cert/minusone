#[cfg(test)]
mod tests_ps_string {
    use crate::ps::array::{ComputeArrayExpr, ParseArrayLiteral};
    use crate::ps::build_powershell_tree;
    use crate::ps::cast::Cast;
    use crate::ps::forward::Forward;
    use crate::ps::integer::ParseInt;
    use crate::ps::join::JoinComparison;
    use crate::ps::linter::Linter;
    use crate::ps::strategy::PowershellStrategy;
    use crate::ps::string::*;
    use crate::ps::typing::ParseType;
    use crate::ps::var::Var;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_powershell_tree(input).unwrap();
        tree.apply_mut_with_strategy(
            &mut (
                ParseString::default(),
                ParseInt::default(),
                Forward::default(),
                ConcatString::default(),
                StringReplaceOp::default(),
                ParseArrayLiteral::default(),
                ComputeArrayExpr::default(),
                StringBuiltins::default(),
                JoinComparison::default(),
                ParseType::default(),
                Cast::default(),
                NewStringMethod::default(),
                FormatString::default(),
                Var::default(),
            ),
            PowershellStrategy::default(),
        )
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_concat_two_elements() {
        assert_eq!(deobfuscate("'a' + 'b'"), "\"ab\"");
    }

    #[test]
    fn test_backtick_escape_sequences() {
        assert_eq!(
            deobfuscate("\"a`nb`tc`0d`ae`bf`ff`vg`\"h`` i\""),
            "\"a`nb`tc`0d`ae`bf`ff`vg`\"h`` i\""
        );
    }

    #[test]
    fn test_backtick_unknown_escape_drops_backtick() {
        assert_eq!(deobfuscate("\"a`zb\""), "\"azb\"");
    }

    #[test]
    fn test_backtick_escaped_dollar_not_expanded() {
        assert_eq!(
            deobfuscate("$x = \"world\"\n\"hello `$x = $x\""),
            "$x = \"world\"\n\"hello `$x = world\""
        );
    }

    #[test]
    fn test_no_collision_when_substituted_value_looks_like_another_reference() {
        assert_eq!(
            deobfuscate("$y = \"resolved\"\n$x = \"literal `$y text\"\n\"a=$x b=$y\""),
            "$y = \"resolved\"\n$x = \"literal `$y text\"\n\"a=literal `$y text b=resolved\""
        );
    }

    #[test]
    fn test_verbatim_string_ignores_backtick() {
        assert_eq!(deobfuscate("'a`nb'"), "\"a``nb\"");
    }

    #[test]
    fn test_infer_subexpression_elements() {
        assert_eq!(deobfuscate("\"foo$(\"b\"+\"a\"+\"r\")\""), "\"foobar\"");
    }

    #[test]
    fn test_replace_operator() {
        assert_eq!(
            deobfuscate("\"hello world\" -replace \"world\", \"toto\""),
            "\"hello toto\""
        );
    }

    #[test]
    fn test_replace_operator_regex() {
        assert_eq!(
            deobfuscate("'ACLAX1300ServerNonUnicode' -replace '([A-Z])(\\d)', '$1 $2'"),
            "\"ACLAX 1300ServerNonUnicode\""
        );
    }

    #[test]
    fn test_creplace_operator_case_sensitive() {
        assert_eq!(deobfuscate("'abcABC' -creplace 'abc', 'x'"), "\"xABC\"");
    }

    #[test]
    fn test_new_string_from_char_codes() {
        assert_eq!(
            deobfuscate("[System.String]::new(@(72, 101, 108, 108, 111))"),
            "\"Hello\""
        );
    }

    #[test]
    fn test_new_string_short_alias() {
        assert_eq!(deobfuscate("[string]::new(@(72, 105))"), "\"Hi\"");
    }

    #[test]
    fn test_new_string_repeat_char() {
        assert_eq!(
            deobfuscate("[System.String]::new([char]65, 5)"),
            "\"AAAAA\""
        );
    }

    #[test]
    fn test_new_string_slice() {
        assert_eq!(
            deobfuscate("[System.String]::new(@(72, 101, 108, 108, 111), 1, 3)"),
            "\"ell\""
        );
    }

    #[test]
    fn test_new_string_from_plain_string() {
        // Real PowerShell coerces a plain string argument into a char[] to match
        // the string(char[]) overload, so [System.String]::new("test") => "test"
        assert_eq!(
            deobfuscate("[System.String]::new(\"test string\")"),
            "\"test string\""
        );
    }

    #[test]
    fn test_to_lower() {
        assert_eq!(deobfuscate("'HeLLo'.ToLower()"), "\"hello\"");
    }

    #[test]
    fn test_to_upper() {
        assert_eq!(deobfuscate("'HeLLo'.ToUpper()"), "\"HELLO\"");
    }

    #[test]
    fn test_to_upper_dynamic_member_name() {
        assert_eq!(deobfuscate("'hi'.'ToUpper'()"), "\"HI\"");
    }

    #[test]
    fn test_replace_method() {
        assert_eq!(deobfuscate("'foo'.Replace('oo', 'aa')"), "\"faa\"");
    }

    #[test]
    fn test_replace_method_non_string_replacement() {
        assert_eq!(deobfuscate("'a1b1c'.Replace('1', 2)"), "\"a2b2c\"");
    }

    #[test]
    fn test_touppertolower_as_command_argument() {
        assert_eq!(
            deobfuscate("Write-Host \"aBcDe\".ToLower()"),
            "Write-Host \"abcde\""
        );
    }

    #[test]
    fn test_contains() {
        assert_eq!(deobfuscate("'hello'.Contains('ell')"), "$true");
        assert_eq!(deobfuscate("'hello'.Contains('xyz')"), "$false");
        assert_eq!(deobfuscate("'hello'.Contains('')"), "$true");
    }

    #[test]
    fn test_index_of() {
        assert_eq!(deobfuscate("'hello'.IndexOf('l')"), "2");
        assert_eq!(deobfuscate("'hello'.IndexOf('z')"), "-1");
        assert_eq!(deobfuscate("'hello'.IndexOf('')"), "0");
        assert_eq!(deobfuscate("'hello'.IndexOf('l', 3)"), "3");
        assert_eq!(deobfuscate("'hello'.IndexOf('l', 0, 2)"), "-1");
        assert_eq!(deobfuscate("'hello'.IndexOf('l', 2, 1)"), "2");
        assert_eq!(deobfuscate("'hello'.IndexOf('l', 3, 0)"), "-1");
        assert_eq!(
            deobfuscate("'hello'.IndexOf('l', 10)"),
            "\"hello\".IndexOf(\"l\", 10)"
        );
    }

    #[test]
    fn test_substring() {
        assert_eq!(deobfuscate("'hello'.Substring(1)"), "\"ello\"");
        assert_eq!(deobfuscate("'hello'.Substring(1, 3)"), "\"ell\"");
        assert_eq!(deobfuscate("'hello'.Substring(5)"), "\"\"");
        assert_eq!(deobfuscate("'hello'.Substring(5, 0)"), "\"\"");
        assert_eq!(
            deobfuscate("'hello'.Substring(10)"),
            "\"hello\".Substring(10)"
        );
        assert_eq!(
            deobfuscate("'hello'.Substring(1, 100)"),
            "\"hello\".Substring(1, 100)"
        );
    }

    #[test]
    fn test_trim() {
        assert_eq!(deobfuscate("'  hello  '.Trim()"), "\"hello\"");
        assert_eq!(deobfuscate("'xxhelloxx'.Trim('x')"), "\"hello\"");
        assert_eq!(deobfuscate("'aaa'.Trim('a')"), "\"\"");
        assert_eq!(deobfuscate("''.Trim()"), "\"\"");
    }

    #[test]
    fn test_trim_char_array() {
        assert_eq!(deobfuscate("'xyhelloyx'.Trim(@('x', 'y'))"), "\"hello\"");
    }

    #[test]
    fn test_to_char_array() {
        assert_eq!(
            deobfuscate("'hello'.ToCharArray() -join ','"),
            "\"h,e,l,l,o\""
        );
        assert_eq!(
            deobfuscate("'hello'.ToCharArray(1, 2) -join ','"),
            "\"e,l\""
        );
    }

    #[test]
    fn test_ends_with() {
        assert_eq!(deobfuscate("'hello'.EndsWith('lo')"), "$true");
        assert_eq!(deobfuscate("'hello'.EndsWith('x')"), "$false");
        assert_eq!(deobfuscate("'hello'.EndsWith('')"), "$true");
    }

    #[test]
    fn test_starts_with() {
        assert_eq!(deobfuscate("'hello'.StartsWith('he')"), "$true");
        assert_eq!(deobfuscate("'hello'.StartsWith('')"), "$true");
    }

    #[test]
    fn test_equals() {
        assert_eq!(deobfuscate("'hello'.Equals('hello')"), "$true");
        assert_eq!(deobfuscate("'hello'.Equals('Hello')"), "$false");
        assert_eq!(deobfuscate("'hello'.Equals(5)"), "$false");
    }

    #[test]
    fn test_insert() {
        assert_eq!(deobfuscate("'hello'.Insert(0, 'X')"), "\"Xhello\"");
        assert_eq!(deobfuscate("'hello'.Insert(5, 'X')"), "\"helloX\"");
        assert_eq!(deobfuscate("'hello'.Insert(2, 'XY')"), "\"heXYllo\"");
        assert_eq!(
            deobfuscate("'hello'.Insert(6, 'X')"),
            "\"hello\".Insert(6, \"X\")"
        );
    }

    #[test]
    fn test_last_index_of() {
        assert_eq!(deobfuscate("'hello'.LastIndexOf('l')"), "3");
        assert_eq!(deobfuscate("'hello'.LastIndexOf('z')"), "-1");
        assert_eq!(deobfuscate("'hello'.LastIndexOf('l', 1)"), "-1");
        assert_eq!(deobfuscate("'hello'.LastIndexOf('l', 0)"), "-1");
        assert_eq!(deobfuscate("'hello'.LastIndexOf('l', 5)"), "3");
        assert_eq!(deobfuscate("'hello'.LastIndexOf('l', 4)"), "3");
        assert_eq!(deobfuscate("'hello'.LastIndexOf('ll', 3, 3)"), "2");
        assert_eq!(deobfuscate("'hello'.LastIndexOf('ll', 3, 2)"), "2");
        assert_eq!(
            deobfuscate("'hello'.LastIndexOf('l', 3, 10)"),
            "\"hello\".LastIndexOf(\"l\", 3, 10)"
        );
    }

    #[test]
    fn test_index_of_any() {
        assert_eq!(deobfuscate("'hello'.IndexOfAny(@('l', 'o'))"), "2");
        assert_eq!(deobfuscate("'hello'.IndexOfAny(@('z'))"), "-1");
        assert_eq!(deobfuscate("'hello'.IndexOfAny(@('l'), 3)"), "3");
        assert_eq!(deobfuscate("'hello'.IndexOfAny(@('l'), 3, 1)"), "3");
        assert_eq!(
            deobfuscate("'hello'.IndexOfAny(@('l'), 10)"),
            "\"hello\".IndexOfAny(@( \"l\"), 10)"
        );
    }

    #[test]
    fn test_last_index_of_any() {
        assert_eq!(deobfuscate("'hello'.LastIndexOfAny(@('l', 'o'))"), "4");
        assert_eq!(deobfuscate("'hello'.LastIndexOfAny(@('z'))"), "-1");
        assert_eq!(deobfuscate("'hello'.LastIndexOfAny(@('l'), 1)"), "-1");
        assert_eq!(deobfuscate("'hello'.LastIndexOfAny(@('l'), 2, 3)"), "2");
        assert_eq!(
            deobfuscate("'hello'.LastIndexOfAny(@('l'), 3, 10)"),
            "\"hello\".LastIndexOfAny(@( \"l\"), 3, 10)"
        );
    }

    #[test]
    fn test_replace_line_endings() {
        assert_eq!(
            deobfuscate("\"a`r`nb`rc`nd\".ReplaceLineEndings('X')"),
            "\"aXbXcXd\""
        );
        assert_eq!(deobfuscate("\"a`r`nb\".ReplaceLineEndings()"), "\"a`r`nb\"");
        assert_eq!(deobfuscate("''.ReplaceLineEndings()"), "\"\"");
    }

    #[test]
    fn test_pad_left_and_right() {
        assert_eq!(deobfuscate("'hi'.PadLeft(5)"), "\"   hi\"");
        assert_eq!(deobfuscate("'hi'.PadLeft(5, '*')"), "\"***hi\"");
        assert_eq!(deobfuscate("'hi'.PadLeft(1)"), "\"hi\"");
        assert_eq!(deobfuscate("'hi'.PadRight(5, '*')"), "\"hi***\"");
    }

    #[test]
    fn test_remove() {
        assert_eq!(deobfuscate("'hello'.Remove(2)"), "\"he\"");
        assert_eq!(deobfuscate("'hello'.Remove(2, 2)"), "\"heo\"");
        assert_eq!(deobfuscate("'hello'.Remove(5)"), "\"hello\"");
        assert_eq!(
            deobfuscate("'hello'.Remove(2, 10)"),
            "\"hello\".Remove(2, 10)"
        );
    }

    #[test]
    fn test_trim_start_and_end() {
        assert_eq!(deobfuscate("'  hi  '.TrimStart()"), "\"hi  \"");
        assert_eq!(deobfuscate("'  hi  '.TrimEnd()"), "\"  hi\"");
        assert_eq!(deobfuscate("'xxhixx'.TrimStart('x')"), "\"hixx\"");
        assert_eq!(deobfuscate("'xxhixx'.TrimEnd('x')"), "\"xxhi\"");
        assert_eq!(deobfuscate("'xyhixy'.TrimStart(@('x', 'y'))"), "\"hixy\"");
    }

    #[test]
    fn test_format_operator() {
        assert_eq!(
            deobfuscate("\"{1} {0}\" -f \"world\", \"hello\""),
            "\"hello world\""
        );
    }
}
