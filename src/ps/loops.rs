use crate::{
    ps::{
        comparison::infer_comparison,
        Powershell::{self, Raw},
        Value::Bool,
    },
    rule::RuleMut,
};

/// This rule will infer for condition if it is false at initialization
///
///
/// # Example
/// ```
/// use minusone::tree::{HashMapStorage, Tree};
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::integer::AddInt;
/// use minusone::ps::loops::ForStatement;
///
/// let mut tree = build_powershell_tree("for ($i = 132 + 324 - 3; $i -lt 200 - 190; $i++) {echo $i}").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(),
///     AddInt::default(),
///     Forward::default(),
///     ForStatement::default(),
/// )).unwrap();
///
/// let mut ps_litter_view = Linter::new();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "");
/// ```
#[derive(Default)]
pub struct ForStatement;

impl<'a> RuleMut<'a> for ForStatement {
    type Language = Powershell;

    fn enter(
        &mut self,
        _node: &mut crate::tree::NodeMut<'a, Self::Language>,
        _flow: crate::tree::ControlFlow,
    ) -> crate::error::MinusOneResult<()> {
        Ok(())
    }

    fn leave(
        &mut self,
        node: &mut crate::tree::NodeMut<'a, Self::Language>,
        _flow: crate::tree::ControlFlow,
    ) -> crate::error::MinusOneResult<()> {
        let view = node.view();
        if view.kind() == "for_condition" {
            let parent = view.parent().unwrap();
            let comparison = view.smallest_child();
            if comparison.kind() == "comparison_expression" && parent.kind() == "for_statement" {
                if let Some(assignment) = parent
                    .named_child("for_initializer")
                    .map(|n| n.smallest_child())
                {
                    if assignment.kind() == "assignment_expression" {
                        if let (
                            Some(var_name),
                            Some(value),
                            Some(comp_left),
                            Some(operator),
                            Some(comp_right),
                        ) = (
                            assignment
                                .child(0)
                                .and_then(|n| Some(n.text().ok()?.to_lowercase())),
                            assignment.child(2),
                            comparison.child(0),
                            comparison.child(1),
                            comparison.child(2),
                        ) {
                            if (comp_left.text().unwrap_or_default().to_lowercase() == var_name
                                && !infer_comparison(&value, &operator, &comp_right)
                                    .unwrap_or(true))
                                || (comp_right.text().unwrap_or_default() == var_name
                                    && !infer_comparison(&comp_left, &operator, &value)
                                        .unwrap_or(true))
                            {
                                node.reduce(Raw(Bool(false)));
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::ps::build_powershell_tree;
    use crate::ps::forward::Forward;
    use crate::ps::integer::ParseInt;
    use crate::ps::loops::ForStatement;
    use crate::ps::Powershell::Raw;
    use crate::ps::Value::Bool;

    #[test]
    fn test_dead_for_statement() {
        let mut tree = build_powershell_tree("for ($i = 0; $i -gt 1; $i++) {}").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ForStatement::default(),
        ))
        .unwrap();

        println!(
            "{:?}",
            tree.root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .child(4)
                .unwrap()
                .data()
        );

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .child(4)
                .unwrap()
                .data()
                .expect("Inferred type"),
            Raw(Bool(false))
        );
    }

    #[test]
    fn test_unpredictable_for_statement() {
        let mut tree = build_powershell_tree("for ($i = 0; $i -lt 10; $i++) {}").unwrap();
        tree.apply_mut(&mut (ForStatement::default())).unwrap();

        assert!(tree
            .root()
            .unwrap()
            .child(0)
            .unwrap()
            .child(0)
            .unwrap()
            .child(3)
            .unwrap()
            .data()
            .is_none());
    }
}
