#[cfg(test)]
pub mod tests_forward {
    use crate::js::build_javascript_tree;
    use crate::js::forward::Forward;
    use crate::js::integer::{AddInt, ParseInt};
    use crate::js::linter::Linter;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_javascript_tree(input).unwrap();
        tree.apply_mut(&mut (ParseInt::default(), AddInt::default(), Forward::default()))
            .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_forward() {
        assert_eq!(deobfuscate("var x = ((1 + (((2)))))"), "var x = 3",);
    }
}
