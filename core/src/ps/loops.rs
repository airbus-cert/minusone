use crate::{
    ps::{
        LoopStatus::{Dead, Inifite, OneTurn},
        Powershell::{self, Loop, Raw},
        Value::{self, Bool},
    },
    rule::RuleMut,
};
use log::trace;

struct IteratorVariable {
    name: String,
    value: Value,
    pub references: Vec<usize>,
}

impl IteratorVariable {
    fn new(name: String, value: Value) -> Self {
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
/// use minusone::ps::bool::Comparison;
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::integer::AddInt;
/// use minusone::ps::var::Var;
/// use minusone::ps::loops::ForStatementCondition;
///
/// let mut tree = build_powershell_tree("for ($i = 132 + 324 - 3; $i -lt 200 - 190; $i++) {echo $i}").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(),
///     AddInt::default(),
///     Comparison::default(),
///     Forward::default(),
///     Var::default(),
///     ForStatementCondition::default(),
/// )).unwrap();
///
/// let mut ps_litter_view = Linter::default();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "");
/// ```
#[derive(Default)]
pub struct ForStatementCondition {
    loop_id: Option<usize>,
}

impl<'a> RuleMut<'a> for ForStatementCondition {
    type Language = Powershell;

    fn enter(
        &mut self,
        node: &mut crate::tree::NodeMut<'a, Self::Language>,
        _flow: crate::tree::ControlFlow,
    ) -> crate::error::MinusOneResult<()> {
        if matches!(node.view().kind(), "while_statement" | "for_statement")
            && node.start_transaction().is_ok()
        {
            // Save the loop id to close the transaction
            self.loop_id = Some(node.id());
        }

        Ok(())
    }

    fn leave(
        &mut self,
        node: &mut crate::tree::NodeMut<'a, Self::Language>,
        _flow: crate::tree::ControlFlow,
    ) -> crate::error::MinusOneResult<()> {
        let view = node.view();
        if matches!(view.kind(), "while_condition" | "for_condition")
            && self.loop_id.is_some()
            && self.loop_id == view.parent().map(|n| n.id())
        {
            if let Some(&Raw(Bool(false))) = view.data() {
                trace!(
                    "ForStatementCondition (L): Setting loop with id {} as dead",
                    self.loop_id.unwrap()
                );
                node.set_by_node_id(self.loop_id.unwrap(), Loop(Dead));
                node.apply_transaction();
            } else {
                trace!(
                    "ForStatementCondition (L): Abort transaction of loop with id {}",
                    self.loop_id.unwrap()
                );
                node.abort_transaction();
            }
            self.loop_id = None;
        };

        Ok(())
    }
}

/// TODO
///
///
/// # Example
/// ```
/// use minusone::tree::{HashMapStorage, Tree};
/// use minusone::engine::CleanEngine;
/// use minusone::ps::build_powershell_tree;
/// use minusone::ps::linter::Linter;
/// use minusone::ps::forward::Forward;
/// use minusone::ps::integer::ParseInt;
/// use minusone::ps::integer::AddInt;
/// use minusone::ps::loops::{ForStatementCondition, ForStatementFlowControl};
///
/// let mut tree = build_powershell_tree("for ($i = 42; $i -lt 200; $i++) {$i; break; $i}").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(),
///     AddInt::default(),
///     Forward::default(),
///     ForStatementCondition::default(),
///     ForStatementFlowControl::default(),
/// )).unwrap();
///
/// let mut ps_litter_view = Linter::default();
/// tree.apply(&mut ps_litter_view).unwrap();
/// let clean = CleanEngine::from_powershell(&ps_litter_view.output).unwrap().clean().unwrap();
///
/// assert_eq!(clean.trim(), "42");
/// ```
#[derive(Default)]
pub struct ForStatementFlowControl {
    iterators: Vec<IteratorVariable>,
    loop_id: Option<usize>,
    statment_count: u32,
}

impl<'a> RuleMut<'a> for ForStatementFlowControl {
    type Language = Powershell;

    fn enter(
        &mut self,
        node: &mut crate::tree::NodeMut<'a, Self::Language>,
        _flow: crate::tree::ControlFlow,
    ) -> crate::error::MinusOneResult<()> {
        if matches!(node.view().kind(), "while_statement" | "for_statement") {
            if self.loop_id.is_none() {
                self.loop_id = Some(node.id());
            } else {
                // We don't support nested loops
                self.loop_id = None;
                self.iterators.clear();
                self.statment_count = 0;
            }
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
            "for_statement" | "while_statement" if self.loop_id == Some(view.id()) => {
                self.loop_id = None;
                self.iterators.clear();
                self.statment_count = 0;
            }
            "flow_control_statement" => {
                // Update the statement count only if in followed loop
                // Skip in nested loops
                if self.loop_id.is_some() {
                    self.statment_count += 1;
                }

                // Infer dead code afer flow control in a loop, even in nested ones
                let following_children_ids: Vec<usize> = view
                    .parent()
                    .unwrap()
                    .iter()
                    .skip_while(|n| n.id() != view.id())
                    .map(|n| n.id())
                    .collect();
                for id in following_children_ids {
                    trace!(
                        "ForStatementFlowControl (L): Setting node with id {} as dead",
                        id
                    );
                    node.set_by_node_id(id, Powershell::DeadCode);
                }
            }
            "assignment_expression"
                if view.get_parent_of_types(vec!["for_initializer"]).is_some()
                    && self.loop_id.is_some()
                    && view
                        .get_parent_of_types(vec!["for_statement"])
                        .map(|n| n.id())
                        == self.loop_id =>
            {
                if let (Some(left), Some(right)) = (view.child(0), view.child(2)) {
                    if let Some(Powershell::Raw(value)) = right.data() {
                        self.iterators.push(IteratorVariable::new(
                            left.text().unwrap().to_string(),
                            value.clone(),
                        ));
                    }
                }
            }
            "variable" => {
                if let Some(iterator_variable) = self
                    .iterators
                    .iter_mut()
                    .find(|n| n.name == view.text().unwrap())
                {
                    iterator_variable.references.push(node.id());
                }
            }
            "statement_block" => {
                let parent = view.parent().unwrap();
                if matches!(parent.kind(), "while_statement" | "for_statement")
                    && self.loop_id == Some(parent.id())
                    && parent.data().is_none()
                    && self.statment_count == 1
                {
                    if let Some(statement_list) = view.named_child("statement_list") {
                        let mut iter = statement_list
                            .iter()
                            .skip_while(|n| n.kind() != "flow_control_statement");

                        match iter.next().map(|n| n.smallest_child().kind()) {
                            Some("break" | "return" | "exit" | "throw") => {
                                trace!(
                                    "ForStatementFlowControl (L): Setting loop with id {} as one turn",
                                    parent.id()
                                );
                                node.set_by_node_id(parent.id(), Loop(OneTurn));

                                self.iterators.iter().for_each(|it| {
                                    it.references.iter().for_each(|&id| {
                                        trace!("ForStatementFlowControl (L): Setting node with id {} as raw with value {}", id, it.value);
                                        node.set_by_node_id(id, Raw(it.value.clone()))
                                    })
                                });
                            }
                            Some("continue") => {
                                trace!(
                                    "ForStatementFlowControl (L): Setting loop with id {} as infinite",
                                    parent.id()
                                );
                                node.set(Loop(Inifite))
                            }
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
    use crate::ps::LoopStatus::{Dead, OneTurn};
    use crate::ps::Powershell::{Loop, Raw};
    use crate::ps::Value::Num;
    use crate::ps::bool::Comparison;
    use crate::ps::build_powershell_tree;
    use crate::ps::forward::Forward;
    use crate::ps::integer::ParseInt;
    use crate::ps::loops::{ForStatementCondition, ForStatementFlowControl};
    use crate::ps::var::Var;

    #[test]
    fn test_dead_for_statement() {
        let mut tree = build_powershell_tree("for ($i = 0; $i -gt 1; $i++) {}").unwrap();
        tree.apply_mut(&mut (
            Forward::default(),
            ParseInt::default(),
            Comparison::default(),
            Var::default(),
            ForStatementCondition::default(),
        ))
        .unwrap();

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred type"),
            Loop(Dead)
        );
    }

    #[test]
    fn test_one_turn_for_statement() {
        let mut tree =
            build_powershell_tree("for ($i = 0; $i -lt 1000; $i++) {$i; break; $i = $i - 1}")
                .unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            Forward::default(),
            ForStatementFlowControl::default(),
        ))
        .unwrap();

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .expect("Inferred type"),
            Loop(OneTurn)
        );

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .child(8)
                .unwrap()
                .child(1)
                .unwrap()
                .child(0)
                .unwrap()
                .smallest_child()
                .data()
                .expect("Inferred type"),
            Raw(Num(0))
        );
    }

    #[test]
    fn test_unpredictable_for_statement() {
        let mut tree = build_powershell_tree("for ($i = 0; $i -lt 10; $i++) {$i}").unwrap();
        tree.apply_mut(&mut (
            Forward::default(),
            ParseInt::default(),
            Var::default(),
            ForStatementCondition::default(),
        ))
        .unwrap();

        // The loop should not be infered
        assert!(
            tree.root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .child(3)
                .unwrap()
                .data()
                .is_none()
        );

        // The statement should not be infered
        assert!(
            tree.root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .child(8)
                .unwrap()
                .child(1)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .is_none()
        );
    }
}
