use crate::error::MinusOneResult;
use crate::ps::{Powershell, Value};
use crate::rule::RuleMut;
use crate::tree::{ControlFlow, NodeMut};
use log::{trace, warn};

/// This rule will stop on the special var $_
/// And check if it's used into a foreach command
/// And then will infer the result of the previous command
/// in the pipe into a PItem value
///
/// # Example
/// ```
/// # use minusone::ps::build_powershell_tree;
/// # use minusone::ps::forward::Forward;
/// # use minusone::ps::integer::ParseInt;
/// # use minusone::ps::linter::Linter;
/// # use minusone::ps::switch::Switch;
///
/// let mut tree = build_powershell_tree("switch (1) {\n1 {2}\n}").unwrap();
/// tree.apply_mut(&mut (
///     ParseInt::default(),
///     Forward::default(),
///     Switch::default(),
///     )
/// ).unwrap();
///
/// let mut ps_litter_view = Linter::default();
/// tree.apply(&mut ps_litter_view).unwrap();
///
/// assert_eq!(ps_litter_view.output, "2");
/// ```
#[derive(Default, Clone)]
pub struct Switch {
    ctx: Vec<SwitchCtx>, // Holds last infered (or not) switch condition
}

#[derive(Clone)]
struct SwitchCtx {
    condition: Option<Powershell>,
    predictable: bool,
    matching: Option<Powershell>,
    default: Option<Powershell>,
}

impl<'a> RuleMut<'a> for Switch {
    type Language = Powershell;

    fn enter(
        &mut self,
        _node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        Ok(())
    }

    fn leave(
        &mut self,
        node: &mut NodeMut<'a, Self::Language>,
        _flow: ControlFlow,
    ) -> MinusOneResult<()> {
        let view = node.view();
        // find usage of magic variable
        match view.kind() {
            "switch_statement" => {
                let ctx = self.ctx.pop().unwrap();
                if let Some(data) = ctx.matching.or(ctx.default.filter(|_| ctx.predictable)) {
                    trace!(
                        "SwitchCtx (L): Setting predictable switch statement {} as {:?}",
                        node.id(),
                        data
                    );
                    node.set(data)
                } else if ctx.predictable {
                    trace!(
                        "SwitchCtx (L): Setting predictable switch statement {} as DeadCode",
                        node.id(),
                    );
                    node.set(Powershell::DeadCode)
                }
            }

            "switch_condition" => {
                let condition = view
                    .child(1)
                    .filter(|n| n.kind() == "pipeline")
                    .and_then(|n| n.data().cloned());
                _ = self.ctx.push(SwitchCtx {
                    predictable: condition.is_some(),
                    condition,
                    matching: None,
                    default: None,
                });
            }

            "switch_clause" => {
                if let Some(ctx) = self.ctx.last_mut() {
                    if let Some(switch_clause_condition) = view
                        .child(0)
                        .filter(|n| n.kind() == "switch_clause_condition")
                    {
                        let statement_pipeline_data = view
                            .child(1)
                            .filter(|n| n.kind() == "statement_block")
                            .and_then(|n| n.named_child("statement_list"))
                            .and_then(|n| n.child(0))
                            .filter(|n| n.kind() == "pipeline")
                            .and_then(|n| n.data().cloned());

                        if switch_clause_condition.child_count() == 0 {
                            // When switch_clause_condition has no child, it should be interpreted as a string
                            let cond = switch_clause_condition.text().ok();
                            if cond == Some("default") {
                                trace!(
                                    "SwitchCtx (L): Default switch clause found at {} and inferred as {:?}",
                                    node.id(),
                                    statement_pipeline_data,
                                );
                                ctx.default = statement_pipeline_data;
                            } else if let Some(Powershell::Raw(Value::Str(str))) =
                                ctx.condition.clone()
                                && Some(str) == cond.map(|s| s.to_string())
                            {
                                trace!(
                                    "SwitchCtx (L): Matching switch clause found at {} and inferred as {:?}",
                                    node.id(),
                                    statement_pipeline_data,
                                );
                                ctx.matching = statement_pipeline_data;
                            } else {
                                trace!(
                                    "SwitchCtx (L): Setting predictable switch clause {} as DeadCode",
                                    node.id(),
                                );
                                node.set(Powershell::DeadCode);
                            }
                        } else if let Some(data) = switch_clause_condition
                            .child(0)
                            .and_then(|n| n.data().cloned())
                        {
                            if ctx.condition == Some(data) {
                                trace!(
                                    "SwitchCtx (L): Matching switch clause found at {} and inferred as {:?}",
                                    node.id(),
                                    statement_pipeline_data,
                                );
                                ctx.matching = statement_pipeline_data;
                            } else {
                                trace!(
                                    "SwitchCtx (L): Setting predictable switch clause {} as DeadCode",
                                    node.id(),
                                );
                                node.set(Powershell::DeadCode);
                            }
                        } else {
                            trace!("SwitchCtx (L): switch cause {} is unpredictable", node.id());
                            ctx.predictable = false;
                        }
                    } else {
                        warn!(
                            "SwitchCtx (L): failed to parse unpredictable switch cause {}",
                            node.id()
                        );
                        ctx.predictable = false;
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
    use crate::ps::forward::Forward;
    use crate::ps::integer::{AddInt, ParseInt};
    use crate::ps::switch::Switch;
    use crate::ps::{
        Powershell::{DeadCode, Raw},
        Value::Num,
        build_powershell_tree,
    };

    #[test]
    fn test_predictible_switch() {
        let mut tree = build_powershell_tree("switch (1) {\n1 {1}\n2 {2}\ndefault {3}\n}").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), Forward::default(), Switch::default()))
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
            Raw(Num(1))
        );
    }

    #[test]
    fn test_predictible_switch_with_unpredictable_clause() {
        let mut tree =
            build_powershell_tree("switch (2) {\n$a {1}\n2 {2}\ndefault {3}\n}").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), Forward::default(), Switch::default()))
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
            Raw(Num(2))
        );
    }

    #[test]
    fn test_default_switch() {
        let mut tree = build_powershell_tree("switch (4) {\n1 {1}\n2 {2}\ndefault {3}\n}").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), Forward::default(), Switch::default()))
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
            Raw(Num(3))
        );
    }

    #[test]
    fn test_unpredictible_condition_switch() {
        let mut tree =
            build_powershell_tree("switch ($a) {\n1 {1}\n2 {2}\ndefault {3}\n}").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), Forward::default(), Switch::default()))
            .unwrap();

        assert!(
            tree.root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .is_none()
        );
    }

    #[test]
    fn test_unpredictible_clause_switch() {
        let mut tree =
            build_powershell_tree("switch (1) {\n${$a + 1} {1}\ndefault {3}\n}").unwrap();
        tree.apply_mut(&mut (ParseInt::default(), Forward::default(), Switch::default()))
            .unwrap();

        assert!(
            tree.root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .is_none()
        );
    }

    #[test]
    fn test_predictible_complex_clause_switch() {
        let mut tree = build_powershell_tree("switch (1) {\n(1+1) {2}\ndefault {3}\n}").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            AddInt::default(),
            Forward::default(),
            Switch::default(),
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
            Raw(Num(3))
        );
    }

    #[test]
    fn test_unpredictible_clause_switch_simplify() {
        let mut tree =
            build_powershell_tree("switch (1) {\n$a {2}\n4 {666}\ndefault {3}\n}").unwrap();
        tree.apply_mut(&mut (
            ParseInt::default(),
            AddInt::default(),
            Forward::default(),
            Switch::default(),
        ))
        .unwrap();

        assert!(
            tree.root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .data()
                .is_none()
        );

        assert_eq!(
            *tree
                .root()
                .unwrap()
                .child(0)
                .unwrap()
                .child(0)
                .unwrap()
                .child(2)
                .unwrap()
                .child(1)
                .unwrap()
                .child(1)
                .unwrap()
                .data()
                .expect("Inferred type"),
            DeadCode
        );
    }
}
