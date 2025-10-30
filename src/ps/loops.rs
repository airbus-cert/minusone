use crate::{
    ps::{
        comparison::infer_comparison,
        LoopStatus,
        Powershell::{self, Loop},
    },
    rule::RuleMut,
};

struct IteratorVariable {
    name: String,
    value: Powershell,
    pub references: Vec<usize>,
}

impl IteratorVariable {
    fn new(name: String, value: Powershell) -> Self {
        Self {
            name,
            value,
            references: vec![],
        }
    }
}

/// This rule will infer for condition at initialisation to detect dead loops
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
/// use minusone::ps::loops::ForStatementCondition;
///
/// let mut tree = build_powershell_tree("for ($i = 132 + 324 - 3; $i -lt 200 - 190; $i++) {echo $i}").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(),
///     AddInt::default(),
///     Forward::default(),
///     ForStatementCondition::default(),
/// )).unwrap();
///
/// let mut ps_litter_view = Linter::default();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "");
/// ```
#[derive(Default)]
pub struct ForStatementCondition;

impl<'a> RuleMut<'a> for ForStatementCondition {
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
        if view.kind() == "for_statement" {
            if let (Some(initialisation), Some(comparison)) = (
                view.named_child("for_condition")
                    .map(|n| n.smallest_child()),
                view.named_child("for_initializer")
                    .map(|n| n.smallest_child()),
            ) {
                if comparison.kind() == "comparison_expression"
                    && initialisation.kind() == "assignment_expression"
                {
                    if let (
                        Some(var_name),
                        Some(value),
                        Some(comp_left),
                        Some(operator),
                        Some(comp_right),
                    ) = (
                        initialisation
                            .child(0)
                            .and_then(|n| Some(n.text().ok()?.to_lowercase())),
                        initialisation.child(2),
                        comparison.child(0),
                        comparison.child(1),
                        comparison.child(2),
                    ) {
                        if let Some(false) = infer_comparison(&comp_left, &operator, &comp_right)
                            .or(
                                (comp_left.text().unwrap_or_default().to_lowercase() == var_name)
                                    .then_some(1)
                                    .and(infer_comparison(&value, &operator, &comp_right)),
                            )
                            .or((comp_right.text().unwrap_or_default() == var_name)
                                .then_some(1)
                                .and(infer_comparison(&comp_left, &operator, &value)))
                        {
                            node.reduce(Loop(LoopStatus::Dead));
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

/// TODO
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
/// use minusone::ps::loops::{ForStatementCondition, ForStatementFlowControl};
///
/// let mut tree = build_powershell_tree("for ($i = 42; $i -lt 200; $i++) {echo $i; break; echo $i + 1}").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(),
///     AddInt::default(),
///     Forward::default(),
///     ForStatementCondition::default(),
///     FlowControlForStatement::default(),
/// )).unwrap();
///
/// let mut ps_litter_view = Linter::default();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "42");
/// ```
#[derive(Default)]
pub struct ForStatementFlowControl {
    statment_count: u32,
    iterators: Vec<IteratorVariable>,
}

impl<'a> RuleMut<'a> for ForStatementFlowControl {
    type Language = Powershell;

    fn enter(
        &mut self,
        node: &mut crate::tree::NodeMut<'a, Self::Language>,
        _flow: crate::tree::ControlFlow,
    ) -> crate::error::MinusOneResult<()> {
        let view = node.view();
        match view.kind() {
            // Track control flow statments
            // TODO: Review control flow count if we are in imbricated loops
            "flow_control_statement" => {
                println!("statment++");
                self.statment_count += 1;

                // Infer dead code afer flow control in a loop
                let following_children_ids: Vec<usize> = view
                    .parent()
                    .unwrap()
                    .iter()
                    .skip_while(|n| n.id() == view.id())
                    .skip(1)
                    .map(|n| n.id().clone())
                    .collect();
                for id in following_children_ids {
                    node.set_by_node_id(id, Powershell::Null);
                }
            }
            "variable" => {
                if view.get_parent_of_types(vec!["for_initializer"]).is_some()
                    && view
                        .get_parent_of_types(vec!["left_assignment_expression"])
                        .is_some()
                {
                    if let Some(assignmenent) =
                        view.get_parent_of_types(vec!["assignment_expression"])
                    {
                        if let Some(right) = assignmenent.named_child("right_assignment") {
                            if let Some(data) = right.data() {
                                {
                                    self.iterators.push(IteratorVariable::new(
                                        view.text().unwrap().to_string(),
                                        data.clone(),
                                    ));
                                }
                            }
                        }
                    }
                } else if let Some(iterator_variable) = self
                    .iterators
                    .iter_mut()
                    .find(|n| n.name == view.text().unwrap().to_string())
                {
                    iterator_variable.references.push(node.id());
                }
            }

            _ => (),
        }

        Ok(())
    }

    fn leave(
        &mut self,
        node: &mut crate::tree::NodeMut<'a, Self::Language>,
        _flow: crate::tree::ControlFlow,
    ) -> crate::error::MinusOneResult<()> {
        let view = node.view();

        match view.kind() {
            "for_statement" => {
                if self.statment_count == 1 {
                    if let Some(statement_list) = view
                        .child(8)
                        .filter(|n| n.kind() == "statement_block")
                        .and_then(|n| n.named_child("statement_list"))
                    {
                        let mut iter = statement_list
                            .iter()
                            .skip_while(|n| n.kind() != "flow_control_statement");

                        match iter.next().map(|n| n.smallest_child().kind()) {
                            Some("break" | "return" | "exit" | "throw") => {
                                node.set(Loop(LoopStatus::OneTurn));

                                // TODO: What if we set the node but it was after the break and was Some(Null)
                                // ex: for ($i = 0; $true;) {$i; break; $i} should give "0" but gives "0\n0" currently
                                self.iterators.iter().for_each(|it| {
                                    it.references
                                        .iter()
                                        .for_each(|&id| node.set_by_node_id(id, it.value.clone()))
                                });
                            }
                            Some("continue") => node.set(Loop(LoopStatus::Inifite)),
                            _ => {}
                        }
                    }
                }
            }
            _ => (),
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::ps::build_powershell_tree;
    use crate::ps::forward::Forward;
    use crate::ps::integer::ParseInt;
    use crate::ps::loops::ForStatementCondition;
    use crate::ps::Powershell::Raw;
    use crate::ps::Value::Bool;

    #[test]
    fn test_dead_for_statement() {
        let mut tree = build_powershell_tree("for ($i = 0; $i -gt 1; $i++) {}").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ForStatementCondition::default(),
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
        tree.apply_mut(&mut (ForStatementCondition::default()))
            .unwrap();

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
