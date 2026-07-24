use crate::error::MinusOneResult;
use crate::js::backend::{JavaScriptBackend, clean_impl};
use crate::js::linter::Linter;
use crate::js::strategy::JavaScriptStrategy;
use crate::js::{JavaScript, JavaScriptRuleSet};
use crate::rule::{LeaveStepOutcome, RuleMut, RuleSetBuilderType};
use crate::step::{LeaveOutcome, Walker};
use crate::tree::{HashMapStorage, Node, NodeMut, Strategy};
use self_cell::{MutBorrow, self_cell};
use std::cell::RefCell;
use std::collections::VecDeque;
use tree_sitter::Node as TreeNode;

struct JsOwner {
    source: String,
    parsed: tree_sitter::Tree,
    storage: MutBorrow<HashMapStorage<JavaScript>>,
}

struct JsMainEngine<'a> {
    node_mut: NodeMut<'a, JavaScript>,
    root: TreeNode<'a>,
    rules: RefCell<JavaScriptRuleSet<'a>>,
    walker: Walker<'a>,
}

self_cell!(
    struct JsMainCell {
        owner: JsOwner,

        #[not_covariant]
        dependent: JsMainEngine,
    }
);

fn parse_js(source: &str) -> tree_sitter::Tree {
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_javascript::LANGUAGE.into())
        .expect("Error loading javascript grammar");
    parser.parse(source, None).expect("failed to parse")
}

pub struct JsStepper {
    pre: VecDeque<crate::trace::Step>,
    main: MainState,
    post: Option<VecDeque<crate::trace::Step>>,
    keep_dead_code: bool,
    record_all: bool,
}

enum MainState {
    NotStarted(String),
    Running(JsMainCell),
    Done,
}

impl JsStepper {
    pub fn new(src: &str, keep_dead_code: bool, record_all: bool) -> MinusOneResult<Self> {
        let (cleaned, pre) =
            JavaScriptBackend::remove_extra_traced(src, keep_dead_code, record_all)?;
        Ok(JsStepper {
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
                    let parsed = parse_js(&source);
                    let owner = JsOwner {
                        source,
                        parsed,
                        storage: MutBorrow::new(HashMapStorage::default()),
                    };
                    let cell = JsMainCell::new(owner, |owner| {
                        let root = owner.parsed.root_node();
                        let storage = owner.storage.borrow_mut();
                        let node_mut = NodeMut::new(root, owner.source.as_bytes(), storage);
                        JsMainEngine {
                            node_mut,
                            root,
                            rules: RefCell::new(JavaScriptRuleSet::new(
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
                            |n| JavaScriptStrategy.control(n),
                            |n, flow| engine.rules.borrow_mut().enter(n, flow),
                            |n, flow, start_at| match engine.rules.borrow_mut().leave_traced_step(
                                n,
                                flow,
                                start_at,
                                |v: &Node<JavaScript>| {
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
                                let mut linter = Linter::default();
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
                                    let cleaned = clean_impl(
                                        final_source,
                                        self.keep_dead_code,
                                        &mut |rule, current| {
                                            crate::trace::push_text_step(
                                                &mut steps,
                                                "post",
                                                rule,
                                                current,
                                                self.record_all,
                                            );
                                        },
                                    );
                                    if let Ok(cleaned) = cleaned {
                                        let _ = cleaned;
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
