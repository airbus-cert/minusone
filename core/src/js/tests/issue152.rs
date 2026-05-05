//! Fixture-driven tests for issue #152 (recursion system).
//!
//! Each fixture pair `samples/issue152_*.obf.js` / `samples/issue152_*.src.js`
//! is loaded from disk; the obfuscated source is run through the full
//! JavaScript ruleset and the result is compared verbatim against the
//! expected source. The expected outputs were cross-checked against
//! `node -e` (Node.js v25) before being committed:
//!
//!   * `issue152_nested_calls`: `a(2)` -> 4, `b(2)` -> a(2)+3 -> 7
//!   * `issue152_hoisting`: forward call `square(5)` -> 25 (the function
//!     declaration appears AFTER the call site in source order, so this
//!     only resolves once `function_declaration` hoisting is in place)
//!   * `issue152_mutual_recursion_with_conditional`: bodies contain
//!     `if_statement` so shape extraction returns `None` and the calls
//!     are left intact - the contract here is "no panic, no infinite
//!     loop, function sources preserved".
#[cfg(test)]
pub mod issue152_tests {
    use crate::js::array::*;
    use crate::js::b64::*;
    use crate::js::bool::ParseBool;
    use crate::js::bool::*;
    use crate::js::comparator::*;
    use crate::js::forward::*;
    use crate::js::functions::fncall::*;
    use crate::js::functions::function::*;
    use crate::js::integer::*;
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
    fn test_issue152_samples() {
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
                    .starts_with("issue152_")
            {
                files.push(path);
            }
        }

        assert!(
            !files.is_empty(),
            "no issue152_* samples found in {samples_dir:?}",
        );

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

        let mut tested = 0usize;
        for (stem, (obf, src)) in pairs {
            let (Some(obf_path), Some(src_path)) = (obf, src) else {
                panic!("incomplete fixture pair for stem {stem}");
            };

            let obf_content = std::fs::read_to_string(&obf_path)
                .unwrap_or_else(|_| panic!("Failed to read {obf_path:?}"));
            let src_content = std::fs::read_to_string(&src_path)
                .unwrap_or_else(|_| panic!("Failed to read {src_path:?}"));

            let result = deobfuscate(&obf_content);
            assert_eq!(
                src_content.trim(),
                result.trim(),
                "Deobfuscation mismatch for fixture: {stem}\n\
                 ----- expected -----\n{src_content}\n\
                 ----- actual -----\n{result}\n",
            );
            tested += 1;
        }

        // Sanity: we are running real fixtures, not silently skipping them.
        assert!(tested >= 3, "expected at least 3 fixtures, ran {tested}");
    }
}
