#[cfg(test)]
mod test_function {
    use crate::js::build_javascript_tree;
    use crate::js::functions::function::ParseFunction;
    use crate::js::linter::Linter;

    #[test]
    fn test_parse_arrow_function_literal() {
        let mut tree = build_javascript_tree("const f = () => 1;").unwrap();
        tree.apply_mut(&mut ParseFunction::default()).unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();

        assert_eq!(linter.output, "const f = () => 1;");
    }
}
