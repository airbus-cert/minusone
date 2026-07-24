use crate::engine::CleanEngine;
use crate::error::MinusOneResult;
use crate::ps::backend::PowershellBackend;
use crate::ps::linter::Linter;
use crate::ps::strategy::PowershellStrategy;
use crate::ps::{Powershell, PowershellRuleSet};
use crate::rule::{LeaveStepOutcome, RuleMut, RuleSetBuilderType};
use crate::step::{LeaveOutcome, Walker};
use crate::tree::{HashMapStorage, Node, NodeMut, Strategy};
use self_cell::{MutBorrow, self_cell};
use std::cell::RefCell;
use std::collections::VecDeque;
use tree_sitter::Node as TreeNode;

struct PsOwner {
    source: String,
    parsed: tree_sitter::Tree,
    storage: MutBorrow<HashMapStorage<Powershell>>,
}

struct PsMainEngine<'a> {
    node_mut: NodeMut<'a, Powershell>,
    root: TreeNode<'a>,
    rules: RefCell<PowershellRuleSet<'a>>,
    walker: Walker<'a>,
}

self_cell!(
    struct PsMainCell {
        owner: PsOwner,

        #[not_covariant]
        dependent: PsMainEngine,
    }
);

fn parse_ps(source: &str) -> tree_sitter::Tree {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_powershell::LANGUAGE.into())
        .expect("Error loading powershell grammar");
    parser.parse(source, None).expect("failed to parse")
}

pub struct PsStepper {
    pre: VecDeque<crate::trace::Step>,
    main: MainState,
    post: Option<VecDeque<crate::trace::Step>>,
    keep_dead_code: bool,
    record_all: bool,
}

enum MainState {
    NotStarted(String),
    Running(PsMainCell),
    Done,
}

impl PsStepper {
    pub fn new(src: &str, keep_dead_code: bool, record_all: bool) -> MinusOneResult<Self> {
        let (cleaned, pre) = PowershellBackend::remove_extra_traced(src, record_all)?;
        Ok(PsStepper {
            pre: pre.into(),
            main: MainState::NotStarted(cleaned),
            post: None,
            keep_dead_code,
            record_all,
        })
    }

    pub fn next(&mut self) -> Option<crate::trace::Step> {
        loop {
            if let Some(step) = self.pre.pop_front() {
                return Some(step);
            }

            match &mut self.main {
                MainState::NotStarted(source) => {
                    let source = std::mem::take(source);
                    let parsed = parse_ps(&source);
                    let owner = PsOwner {
                        source,
                        parsed,
                        storage: MutBorrow::new(HashMapStorage::default()),
                    };
                    let cell = PsMainCell::new(owner, |owner| {
                        let root = owner.parsed.root_node();
                        let storage = owner.storage.borrow_mut();
                        let node_mut = NodeMut::new(root, owner.source.as_bytes(), storage);
                        PsMainEngine {
                            node_mut,
                            root,
                            rules: RefCell::new(PowershellRuleSet::new(
                                RuleSetBuilderType::WithoutRules(vec![]),
                            )),
                            walker: Walker::new(root),
                        }
                    });
                    self.main = MainState::Running(cell);
                }
                MainState::Running(cell) => {
                    let outcome = cell.with_dependent_mut(|_owner, engine| {
                        engine.walker.step(
                            &mut engine.node_mut,
                            |n| PowershellStrategy.control(n),
                            |n, flow| engine.rules.borrow_mut().enter(n, flow),
                            |n, flow, start_at| match engine.rules.borrow_mut().leave_traced_step(
                                n,
                                flow,
                                start_at,
                                |v: &Node<Powershell>| {
                                    let mut linter = Linter::default();
                                    v.apply(&mut linter)?;
                                    Ok(linter.output)
                                },
                            )? {
                                LeaveStepOutcome::Changed {
                                    rule_name,
                                    before,
                                    after,
                                    resume_at,
                                } => {
                                    let root_view = crate::trace::find_root(n.view());
                                    let mut linter = Linter::default();
                                    root_view.apply(&mut linter)?;
                                    let step = crate::trace::Step {
                                        phase: "main",
                                        rule: rule_name.to_string(),
                                        kind: n.view().kind(),
                                        start: n.view().start_abs(),
                                        end: n.view().end_abs(),
                                        source: linter.output,
                                        old: before,
                                        new: after,
                                        has_node_diff: true,
                                    };
                                    Ok(LeaveOutcome::Changed {
                                        result: step,
                                        resume_at,
                                    })
                                }
                                LeaveStepOutcome::Finished => Ok(LeaveOutcome::Finished),
                            },
                        )
                    });

                    match outcome {
                        Ok(Some(step)) => return Some(step),
                        Ok(None) => {
                            let final_source = cell.with_dependent_mut(|_owner, engine| {
                                engine.node_mut.inner = engine.root;
                                let mut linter = Linter::default().set_tab("    ");
                                engine.node_mut.view().apply(&mut linter)?;
                                MinusOneResult::Ok(linter.output)
                            });
                            self.main = MainState::Done;

                            match final_source {
                                Ok(final_source) => {
                                    let mut steps = Vec::new();
                                    crate::trace::push_text_step(
                                        &mut steps,
                                        "post",
                                        "Linter",
                                        &final_source,
                                        self.record_all,
                                    );
                                    if let Ok(cleaned) =
                                        CleanEngine::<PowershellBackend>::from_source(&final_source)
                                            .and_then(|mut e| e.clean(self.keep_dead_code))
                                    {
                                        crate::trace::push_text_step(
                                            &mut steps,
                                            "post",
                                            "RemoveUnusedVar",
                                            &cleaned,
                                            self.record_all,
                                        );
                                    }
                                    self.post = Some(steps.into());
                                }
                                Err(_) => {
                                    self.post = Some(VecDeque::new());
                                }
                            }
                        }
                        Err(_) => {
                            self.main = MainState::Done;
                            self.post = Some(VecDeque::new());
                        }
                    }
                }
                MainState::Done => {
                    let post = self.post.get_or_insert_with(VecDeque::new);
                    return post.pop_front();
                }
            }
        }
    }
}
