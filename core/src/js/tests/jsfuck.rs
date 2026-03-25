#[cfg(test)]
pub mod jsfuck_tests {
    use crate::js::array::*;
    use crate::js::b64::*;
    use crate::js::bool::ParseBool;
    use crate::js::bool::*;
    use crate::js::build_javascript_tree;
    use crate::js::comparator::*;
    use crate::js::forward::*;
    use crate::js::functions::fncall::*;
    use crate::js::functions::function::*;
    use crate::js::integer::*;
    use crate::js::linter::Linter;
    use crate::js::objects::object::*;
    use crate::js::specials::*;
    use crate::js::string::*;
    use crate::js::var::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseBool::default(),
            ParseString::default(),
            ParseFunction::default(),
            ParseArray::default(),
            ParseSpecials::default(),
            ParseObject::default(),
            NegInt::default(),
            SubAddInt::default(),
            MultInt::default(),
            PowInt::default(),
            ShiftInt::default(),
            BitwiseInt::default(),
            ObjectField::default(),
            NotBool::default(),
            BoolAlgebra::default(),
            AddBool::default(),
            CombineArrays::default(),
            CharAt::default(),
            Forward::default(),
            StringPlusMinus::default(),
            ArrayPlusMinus::default(),
            BoolPlusMinus::default(),
            Concat::default(),
            ConcatFunction::default(),
            Split::default(),
            GetArrayElement::default(),
            AddSubSpecials::default(),
            ToString::default(),
            B64::default(),
            Var::default(),
            FnCall::default(),
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
}
