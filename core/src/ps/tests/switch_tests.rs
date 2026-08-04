#[cfg(test)]
mod tests_ps_switch {
    use crate::ps::build_powershell_tree;
    use crate::ps::forward::Forward;
    use crate::ps::integer::{AddInt, ParseInt};
    use crate::ps::linter::Linter;
    use crate::ps::switch::Switch;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_powershell_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            AddInt::default(),
            Forward::default(),
            Switch::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_predictible_switch() {
        assert_eq!(
            deobfuscate("switch (1) {\n1 {1}\n2 {2}\ndefault {3}\n}"),
            "1"
        );
    }

    #[test]
    fn test_predictible_switch_with_unpredictable_clause() {
        assert_eq!(
            deobfuscate("switch (2) {\n$a {1}\n2 {2}\ndefault {3}\n}"),
            "2"
        );
    }

    #[test]
    fn test_default_switch() {
        assert_eq!(
            deobfuscate("switch (4) {\n1 {1}\n2 {2}\ndefault {3}\n}"),
            "3"
        );
    }

    #[test]
    fn test_unpredictible_condition_switch() {
        assert_eq!(
            deobfuscate("switch ($a) {\n1 {1}\n2 {2}\ndefault {3}\n}"),
            "switch ($a) {\n default {\n  3\n }\n}"
        );
    }

    #[test]
    fn test_unpredictible_clause_switch() {
        assert_eq!(
            deobfuscate("switch (1) {\n${$a + 1} {1}\ndefault {3}\n}"),
            "switch (1) {\n ${$a + 1} {\n  1\n }\n default {\n  3\n }\n}"
        );
    }

    #[test]
    fn test_predictible_complex_clause_switch() {
        assert_eq!(deobfuscate("switch (1) {\n(1+1) {2}\ndefault {3}\n}"), "3");
    }

    #[test]
    fn test_unpredictible_clause_switch_simplify() {
        assert_eq!(
            deobfuscate("switch (1) {\n$a {2}\n4 {666}\ndefault {3}\n}"),
            "switch (1) {\n $a {\n  2\n }\n default {\n  3\n }\n}"
        );
    }
}
