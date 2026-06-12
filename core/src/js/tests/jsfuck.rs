#[cfg(test)]
pub mod jsfuck_tests {
    use crate::js::array::*;
    use crate::js::b64::*;
    use crate::js::bool::ParseBool;
    use crate::js::bool::*;
    use crate::js::comparator::*;
    use crate::js::forward::*;
    use crate::js::functions::fncall::*;
    use crate::js::functions::function::*;
    use crate::js::integer::*;
    use crate::js::jsfuck::*;
    use crate::js::linter::Linter;
    use crate::js::objects::object::*;
    use crate::js::post_process::BracketCallToMember;
    use crate::js::specials::*;
    use crate::js::string::*;
    use crate::js::var::*;
    use crate::js::{build_javascript_tree, build_javascript_tree_for_storage};
    use crate::tree::EmptyStorage;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn deobfuscate(input: &str) -> String {
        let tree = build_javascript_tree_for_storage::<EmptyStorage>(input).unwrap();
        let mut bracket_to_member = BracketCallToMember::default();
        tree.apply(&mut bracket_to_member).unwrap();
        let input = bracket_to_member.clear().unwrap();

        let mut tree = build_javascript_tree(&input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseBool::default(),
            ParseString::default(),
            ParseFunction::default(),
            ParseArray::default(),
            ParseSpecials::default(),
            ParseObject::default(),
            PosNeg::default(),
            AddInt::default(),
            MultDivMod::default(),
            PowInt::default(),
            ShiftInt::default(),
            BitwiseInt::default(),
            ObjectField::default(),
            NotBool::default(),
            BoolAlgebra::default(),
            AddBool::default(),
            CombineArrays::default(),
            ArrayConcat::default(),
            StringBuiltins::default(),
            BracketCharAt::default(),
            Forward::default(),
            ArrayPlusMinus::default(),
            Concat::default(),
            ConcatFunction::default(),
            GetArrayElement::default(),
            AddSubSpecials::default(),
            ToString::default(),
            B64::default(),
            Var::default(),
            FnCall::default(),
            JsFuckLevelNine::default(),
            StrictEq::default(),
            LooseEq::default(),
            CmpOrd,
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_lower_alphabet() {
        assert_eq!("'a'", deobfuscate("(false+[])[1]"));
        assert_eq!("'b'", deobfuscate("([]['entries']()+'')[2]"));
        assert_eq!("'c'", deobfuscate("([]['flat']+[])[3]"));
        assert_eq!("'d'", deobfuscate("(undefined+[])[2]"));
        assert_eq!("'e'", deobfuscate("(true+[])[3]"));
        assert_eq!("'f'", deobfuscate("(false+[])[0]"));
        assert_eq!("'g'", deobfuscate("(false+[0]+String)[20]"));
        assert_eq!("'h'", deobfuscate("(+(101))['to'+String['name']](21)[1]"));
        assert_eq!("'i'", deobfuscate("([false]+undefined)[10]"));
        assert_eq!("'j'", deobfuscate("([]['entries']()+'')[3]"));
        assert_eq!("'k'", deobfuscate("(+(20))['to'+String['name']](21)"));
        assert_eq!("'l'", deobfuscate("(false+[])[2]"));
        assert_eq!("'m'", deobfuscate("(Number+[])[11]"));
        assert_eq!("'n'", deobfuscate("(undefined+[])[1]"));
        assert_eq!("'o'", deobfuscate("(true+[]['at'])[10]"));
        assert_eq!("'p'", deobfuscate("(+(211))['to'+String['name']](31)[1]"));
        assert_eq!("'q'", deobfuscate("(+(2+[1]+[2]))['toString'](3+[1])[1]"));
        assert_eq!("'r'", deobfuscate("(true+[])[1]"));
        assert_eq!("'s'", deobfuscate("(false+[])[3]"));
        assert_eq!("'t'", deobfuscate("(true+[])[0]"));
        assert_eq!("'u'", deobfuscate("(undefined+[])[0]"));
        assert_eq!("'v'", deobfuscate("(+(31))['to'+String['name']](32)"));
        assert_eq!("'w'", deobfuscate("(+(32))['to'+String['name']](33)"));
        assert_eq!("'x'", deobfuscate("(+(101))['to'+String['name']](34)[1]"));
        assert_eq!("'y'", deobfuscate("(NaN+[Infinity])[10]"));
        assert_eq!("'z'", deobfuscate("(+(35))['to'+String['name']](36)"));
    }

    /// JSFuck "level 9" universal builder: `Function("return '\uXXXX'")()`.
    /// The body is a returned string literal, so it is decoded statically
    /// (no code execution) — covering the characters that cannot be assembled
    /// from primitive coercions alone.
    #[test]
    fn test_jsfuck_level_nine() {
        // global Function constructor
        assert_eq!("'A'", deobfuscate(r#"Function("return 'A'")()"#));
        assert_eq!("'AB'", deobfuscate(r#"Function("return 'AB'")()"#));
        // body carrying a literal backslash escape (`return 'A'`), as produced
        // by JSFuck — the `\uXXXX` is decoded by the rule itself, not the parser
        assert_eq!("'A'", deobfuscate(r#"Function("return '\\u0041'")()"#));
        // the JSFuck way of reaching the Function constructor
        assert_eq!(
            "'\u{20ac}'",
            deobfuscate(r#"[]["flat"]["constructor"]("return '€'")()"#)
        );
        // non-ASCII via the BMP `\uXXXX` escape (literal backslash body)
        assert_eq!(
            "'\u{20ac}'",
            deobfuscate(r#"Function("return '\\u20ac'")()"#)
        ); // €
        assert_eq!(
            "'\u{00e9}'",
            deobfuscate(r#"Function("return '\\u00e9'")()"#)
        ); // é
        assert_eq!(
            "'\u{4e2d}'",
            deobfuscate(r#"Function("return '\\u4e2d'")()"#)
        ); // 中
        // several escapes in one body
        assert_eq!(
            "'\u{20ac}$'",
            deobfuscate(r#"Function("return '\\u20ac\\u0024'")()"#)
        );
        // code-point and hex escapes, including astral / surrogate pairs
        assert_eq!(
            "'\u{1f600}'",
            deobfuscate(r#"Function("return '\u{1f600}'")()"#)
        );
        assert_eq!("'AB'", deobfuscate(r#"Function("return '\x41\x42'")()"#));
        // not a string-literal body: left untouched (no eval executed)
        assert_eq!(
            "[]['flat'].constructor('return 1+1')()",
            deobfuscate(r#"[]["flat"]["constructor"]("return 1+1")()"#)
        );
    }

    #[test]
    fn test_specials() {
        assert_eq!("false", deobfuscate("![]"));
        assert_eq!("true", deobfuscate("!![]"));
        assert_eq!("undefined", deobfuscate("[][[]]"));
        assert_eq!("NaN", deobfuscate("+[![]]"));
        assert_eq!("''", deobfuscate("[]+[]"));
        assert_eq!(
            "Infinity",
            deobfuscate("+(+!+[]+(!+[]+[])[!+[]+!+[]+!+[]]+[+!+[]]+[+[]]+[+[]]+[+[]])")
        );
    }

    #[test]
    fn test_constructors() {
        assert_eq!("'Array'", deobfuscate("[]['constructor']['name']"));
        assert_eq!("'Number'", deobfuscate("(+[])['constructor']['name']"));
        assert_eq!("'String'", deobfuscate("([]+[])['constructor']['name']"));
        assert_eq!("'Boolean'", deobfuscate("(![])['constructor']['name']"));
        assert_eq!("'Function'", deobfuscate("[]['at']['constructor']['name']"));
        //assert_eq!("'RegExp'", deobfuscate("Function('return/'+false+'/')()"));
    }

    #[test]
    fn test_digits() {
        for i in 0..10 {
            let jsfuck = format!("{}+[]", i);
            assert_eq!(format!("'{i}'"), deobfuscate(&jsfuck));
        }
    }

    #[test]
    fn test_numbers() {
        for i in 0..10 {
            let jsfuck = if i == 0 {
                "+[]".to_string()
            } else {
                "+!+[]".repeat(i)
            };
            assert_eq!(format!("{i}"), deobfuscate(&jsfuck));
        }
    }

    #[test]
    fn test_samples() {
        let samples_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("samples");

        let mut files: Vec<PathBuf> = vec![];
        for entry in std::fs::read_dir(&samples_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file()
                && path
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("jsfuck")
            {
                files.push(path);
            }
        }

        let mut pairs: HashMap<String, (Option<PathBuf>, Option<PathBuf>)> = HashMap::new();

        for path in &files {
            let name = path.file_name().unwrap().to_str().unwrap();
            if name.ends_with(".obf.js") {
                let stem = name.trim_end_matches(".obf.js").to_string();
                pairs.entry(stem).or_default().0 = Some(path.clone());
            } else if name.ends_with(".src.js") {
                let stem = name.trim_end_matches(".src.js").to_string();
                pairs.entry(stem).or_default().1 = Some(path.clone());
            }
        }

        for (stem, (obf, src)) in pairs {
            let (Some(obf_path), Some(src_path)) = (obf, src) else {
                continue;
            };

            let obf_content = std::fs::read_to_string(&obf_path)
                .unwrap_or_else(|_| panic!("Failed to read {obf_path:?}"));
            let src_content = std::fs::read_to_string(&src_path)
                .unwrap_or_else(|_| panic!("Failed to read {src_path:?}"));

            let result = deobfuscate(&obf_content);
            assert_eq!(
                src_content.trim(),
                result.trim(),
                "Deobfuscation mismatch for sample: {stem}"
            );
        }
    }

    /// Digits recovered from radix toString and exponential number strings.
    #[test]
    fn test_jsfuck_digits_constructions() {
        assert_eq!("'0'", deobfuscate("(+(\"11e100\")+[])[6]"));
        assert_eq!("'1'", deobfuscate("(+(\"11e100\")+[])[0]"));
        assert_eq!("'2'", deobfuscate("(+(2))[\"toString\"](3)"));
        assert_eq!("'3'", deobfuscate("(+(3))[\"toString\"](4)"));
        assert_eq!("'4'", deobfuscate("(+(4))[\"toString\"](5)"));
        assert_eq!("'5'", deobfuscate("(+(5))[\"toString\"](6)"));
        assert_eq!("'6'", deobfuscate("(+(6))[\"toString\"](7)"));
        assert_eq!("'7'", deobfuscate("(+(\".0000001\")+[])[3]"));
        assert_eq!("'8'", deobfuscate("(+(8))[\"toString\"](9)"));
        assert_eq!("'9'", deobfuscate("(+(9))[\"toString\"](10)"));
    }
    /// Every lowercase letter via JSFuck's filter/iterator/constructor/radix atoms.
    #[test]
    fn test_jsfuck_full_lower_alphabet() {
        assert_eq!("'a'", deobfuscate("(false+[])[1]"));
        assert_eq!("'b'", deobfuscate("([][\"entries\"]()+[])[2]"));
        assert_eq!("'c'", deobfuscate("([][\"filter\"]+[])[3]"));
        assert_eq!("'d'", deobfuscate("(undefined+[])[2]"));
        assert_eq!("'e'", deobfuscate("(false+[])[4]"));
        assert_eq!("'f'", deobfuscate("(false+[])[0]"));
        assert_eq!("'g'", deobfuscate("(([]+[])[\"constructor\"]+[])[14]"));
        assert_eq!("'h'", deobfuscate("(+(17))[\"toString\"](18)"));
        assert_eq!("'i'", deobfuscate("(undefined+[])[5]"));
        assert_eq!("'j'", deobfuscate("([][\"entries\"]()+[])[3]"));
        assert_eq!("'k'", deobfuscate("(+(20))[\"toString\"](21)"));
        assert_eq!("'l'", deobfuscate("(false+[])[2]"));
        assert_eq!("'m'", deobfuscate("((+[])[\"constructor\"]+[])[11]"));
        assert_eq!("'n'", deobfuscate("(undefined+[])[1]"));
        assert_eq!("'o'", deobfuscate("([][\"filter\"]+[])[6]"));
        assert_eq!("'p'", deobfuscate("(+(25))[\"toString\"](26)"));
        assert_eq!("'q'", deobfuscate("(+(26))[\"toString\"](27)"));
        assert_eq!("'r'", deobfuscate("(true+[])[1]"));
        assert_eq!("'s'", deobfuscate("(false+[])[3]"));
        assert_eq!("'t'", deobfuscate("(true+[])[0]"));
        assert_eq!("'u'", deobfuscate("(true+[])[2]"));
        assert_eq!("'v'", deobfuscate("([][\"filter\"]+[])[25]"));
        assert_eq!("'w'", deobfuscate("(+(32))[\"toString\"](33)"));
        assert_eq!("'x'", deobfuscate("(+(33))[\"toString\"](34)"));
        assert_eq!("'y'", deobfuscate("([][\"entries\"]()+[])[12]"));
        assert_eq!("'z'", deobfuscate("(+(35))[\"toString\"](36)"));
    }
    /// Uppercase letters reachable without eval (constructor names and the array-iterator string).
    #[test]
    fn test_jsfuck_upper_alphabet() {
        assert_eq!("'A'", deobfuscate("([][\"entries\"]()+[])[8]"));
        assert_eq!("'B'", deobfuscate("((![])[\"constructor\"]+[])[9]"));
        assert_eq!(
            "'F'",
            deobfuscate("([][\"filter\"][\"constructor\"]+[])[9]")
        );
        assert_eq!("'I'", deobfuscate("([][\"entries\"]()+[])[14]"));
        assert_eq!("'N'", deobfuscate("(NaN+[])[0]"));
        assert_eq!("'S'", deobfuscate("(([]+[])[\"constructor\"]+[])[9]"));
    }
    /// Punctuation reachable without eval (function/HTML-method strings, Array.concat, number strings).
    #[test]
    fn test_jsfuck_symbols() {
        assert_eq!("' '", deobfuscate("([][\"filter\"]+[])[8]"));
        assert_eq!("'\"'", deobfuscate("(\"\"[\"fontcolor\"]())[12]"));
        assert_eq!("'('", deobfuscate("([][\"filter\"]+[])[15]"));
        assert_eq!("')'", deobfuscate("([][\"filter\"]+[])[16]"));
        assert_eq!("'+'", deobfuscate("(+(\"11e100\")+[])[4]"));
        assert_eq!("','", deobfuscate("([[]][\"concat\"]([[]])+[])[0]"));
        assert_eq!("'-'", deobfuscate("(+(\".0000001\")+[])[2]"));
        assert_eq!("'.'", deobfuscate("(+(\"11e100\")+[])[1]"));
        assert_eq!("'/'", deobfuscate("(\"\"[\"italics\"]())[4]"));
        assert_eq!("'<'", deobfuscate("(\"\"[\"italics\"]())[0]"));
        assert_eq!("'='", deobfuscate("(\"\"[\"fontcolor\"]())[11]"));
        assert_eq!("'>'", deobfuscate("(\"\"[\"italics\"]())[2]"));
        assert_eq!("'['", deobfuscate("([][\"filter\"]+[])[20]"));
        assert_eq!("']'", deobfuscate("([][\"filter\"]+[])[32]"));
        assert_eq!("'{'", deobfuscate("([][\"filter\"]+[])[18]"));
        assert_eq!("'}'", deobfuscate("([][\"filter\"]+[])[34]"));
    }
}
