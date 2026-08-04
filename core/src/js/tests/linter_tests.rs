#[cfg(test)]
mod test_linter {
    use crate::js::build_javascript_tree;
    use crate::js::forward::Forward;
    use crate::js::functions::fncall::FnCall;
    use crate::js::functions::function::ParseFunction;
    use crate::js::integer::{AddInt, ParseInt};
    use crate::js::linter::Linter;
    use crate::js::objects::object::{ObjectField, ParseObject};
    use crate::js::strategy::JavaScriptStrategy;
    use crate::js::var::Var;

    #[test]
    fn test_linter_emits_simplified_function_expression_body() {
        let mut tree = build_javascript_tree("let x = function () { return 1 + 2; };").unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                AddInt::default(),
                ParseFunction::default(),
            ),
            JavaScriptStrategy::default(),
        )
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();

        assert_eq!(linter.output, "let x = function () { return 3; };");
    }

    #[test]
    fn test_linter_keeps_identifier_on_function_object_assignment() {
        let mut tree = build_javascript_tree(
            "let a = {}; let x = function (n) { return n + 1; }; a.t = x; console.log(a.t(1));",
        )
        .unwrap();

        tree.apply_mut_with_strategy(
            &mut (
                ParseInt::default(),
                AddInt::default(),
                ParseFunction::default(),
                ParseObject::default(),
                Forward::default(),
                ObjectField::default(),
                Var::default(),
                FnCall::default(),
            ),
            JavaScriptStrategy::default(),
        )
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();

        assert!(linter.output.contains("a.t = x;"));
    }
}
