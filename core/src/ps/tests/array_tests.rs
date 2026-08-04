#[cfg(test)]
mod tests_ps_array {
    use crate::ps::Powershell::Array;
    use crate::ps::Value::Num;
    use crate::ps::access::AccessString;
    use crate::ps::array::{
        AddArray, ComputeArrayExpr, NewObjectArray, ParseArrayLiteral, ParseRange,
    };
    use crate::ps::build_powershell_tree;
    use crate::ps::forward::Forward;
    use crate::ps::integer::{AddInt, ParseInt};
    use crate::ps::join::JoinOperator;
    use crate::ps::linter::Linter;
    use crate::ps::string::ParseString;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_powershell_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            ParseString::default(),
            AddInt::default(),
            Forward::default(),
            ComputeArrayExpr::default(),
            ParseArrayLiteral::default(),
            AddArray::default(),
            ParseRange::default(),
            AccessString::default(),
            JoinOperator::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_init_num_array() {
        assert_eq!(deobfuscate("-join @(1,2,3)"), "\"123\"");
    }

    #[test]
    fn test_init_mix_array() {
        assert_eq!(deobfuscate("-join @(1,2,'3')"), "\"123\"");
    }

    #[test]
    fn test_init_str_array() {
        assert_eq!(deobfuscate("-join @('a','b','c')"), "\"abc\"");
    }

    #[test]
    fn test_init_int_array_without_at() {
        assert_eq!(deobfuscate("-join (1,2,3)"), "\"123\"");
    }

    #[test]
    fn test_init_array_with_multi_statement() {
        assert_eq!(deobfuscate("-join @(1,2,3; 4 + 6)"), "\"12310\"");
    }

    #[test]
    fn test_concat_array() {
        assert_eq!(deobfuscate("-join ('foo'[0,1] + 'x')"), "\"fox\"");
    }

    #[test]
    fn test_negative_range() {
        assert_eq!(deobfuscate("-join (-1..-3)"), "\"-1-2-3\"");
    }

    #[test]
    fn test_new_object_array() {
        // NewObjectArray produces an internal Array value with no PowerShell
        // surface syntax to render it back to, so it's asserted on directly.
        let mut tree = build_powershell_tree("New-Object byte[] 16").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), Forward::default()))
            .unwrap();
        tree.apply_mut(&mut NewObjectArray::default()).unwrap();

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred data"),
            Array(vec![Num(0); 16]),
        );
    }
}
