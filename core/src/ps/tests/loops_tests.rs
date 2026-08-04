#[cfg(test)]
mod tests_ps_loops {
    use crate::ps::bool::Comparison;
    use crate::ps::build_powershell_tree;
    use crate::ps::forward::Forward;
    use crate::ps::integer::{AddInt, ParseInt};
    use crate::ps::linter::Linter;
    use crate::ps::loops::{ForStatementCondition, ForStatementFlowControl};
    use crate::ps::var::Var;

    fn deobfuscate(input: &str) -> String {
        let mut tree = build_powershell_tree(input).unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            AddInt::default(),
            Comparison::default(),
            Forward::default(),
            Var::default(),
            ForStatementCondition::default(),
            ForStatementFlowControl::default(),
        ))
        .unwrap();

        let mut linter = Linter::default();
        tree.apply(&mut linter).unwrap();
        linter.output
    }

    #[test]
    fn test_dead_for_statement() {
        assert_eq!(deobfuscate("for ($i = 0; $i -gt 1; $i++) {}"), "");
    }

    #[test]
    fn test_one_turn_for_statement() {
        assert_eq!(
            deobfuscate("for ($i = 0; $i -lt 1000; $i++) {$i; break; $i = $i - 1}"),
            "$i = 0\n0"
        );
    }

    #[test]
    fn test_unpredictable_for_statement() {
        assert_eq!(
            deobfuscate("for ($i = 0; $i -lt 10; $i++) {$i}"),
            "for ($i = 0;$i -lt 10;$i++){\n $i\n}"
        );
    }
}
