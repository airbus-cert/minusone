use crate::js::strategy::JavaScriptStrategy;
use crate::js::{JavaScript, JavaScriptRuleSet, build_javascript_tree};
use crate::rule::RuleSetBuilderType;
use crate::tree::{HashMapStorage, Tree};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::thread::LocalKey;

pub struct DepthCounter {
    key: &'static LocalKey<Cell<usize>>,
    max: usize,
    label: &'static str,
}

impl DepthCounter {
    pub const fn new(key: &'static LocalKey<Cell<usize>>, max: usize, label: &'static str) -> Self {
        Self { key, max, label }
    }

    // returns `None` once the cap is reached
    pub fn enter(&self) -> Option<DepthGuard> {
        let depth = self.key.with(|c| c.get());
        if depth >= self.max {
            log::trace!(
                "{}: depth {} reached the cap, refusing to recurse",
                self.label,
                depth
            );
            return None;
        }
        self.key.with(|c| c.set(depth + 1));
        Some(DepthGuard { key: self.key })
    }

    pub fn depth(&self) -> usize {
        self.key.with(|c| c.get())
    }

    pub fn max(&self) -> usize {
        self.max
    }
}

pub struct DepthGuard {
    key: &'static LocalKey<Cell<usize>>,
}

impl Drop for DepthGuard {
    fn drop(&mut self) {
        self.key.with(|c| c.set(c.get().saturating_sub(1)));
    }
}

/// Nested sub-pipelines allowed for a single `FnCall` resolution.
pub const MAX_FNCALL_DEPTH: usize = 2;
/// Nested `.map()` / `.filter()` callbacks allowed.
pub const MAX_MAP_FILTER_DEPTH: usize = 4;
/// Nested `for` / `for..in` simulations allowed.
pub const MAX_FOR_DEPTH: usize = 3;

thread_local! {
    static FNCALL_DEPTH: Cell<usize> = const { Cell::new(0) };
    static MAP_FILTER_DEPTH: Cell<usize> = const { Cell::new(0) };
    static FOR_DEPTH: Cell<usize> = const { Cell::new(0) };
}

fn fncall_counter() -> DepthCounter {
    DepthCounter::new(&FNCALL_DEPTH, MAX_FNCALL_DEPTH, "FnCall")
}

fn map_filter_counter() -> DepthCounter {
    DepthCounter::new(&MAP_FILTER_DEPTH, MAX_MAP_FILTER_DEPTH, "ArrayMapFilter")
}

fn for_counter() -> DepthCounter {
    DepthCounter::new(&FOR_DEPTH, MAX_FOR_DEPTH, "ForLoop")
}

pub fn enter_fncall() -> Option<DepthGuard> {
    fncall_counter().enter()
}

pub fn enter_map_filter() -> Option<DepthGuard> {
    map_filter_counter().enter()
}

pub fn enter_for_loop() -> Option<DepthGuard> {
    for_counter().enter()
}

pub fn build_and_reduce(src: &str) -> Option<Tree<'_, HashMapStorage<JavaScript>>> {
    let mut tree = build_javascript_tree(src).ok()?;
    tree.apply_mut_with_strategy(
        &mut JavaScriptRuleSet::new(RuleSetBuilderType::WithoutRules(vec![])),
        JavaScriptStrategy,
    )
    .ok()?;
    Some(tree)
}

thread_local! {
    static SEED: RefCell<Option<HashMap<String, JavaScript>>> = const { RefCell::new(None) };
    static RESULT: RefCell<Option<HashMap<String, JavaScript>>> = const { RefCell::new(None) };
}

/// Runs `f` with `seed` visible to `Var` at the top of every sub-program it parses.
pub fn with_seed<R>(seed: HashMap<String, JavaScript>, f: impl FnOnce() -> Option<R>) -> Option<R> {
    let previous_seed = SEED.with(|c| c.replace(Some(seed)));
    let previous_result = RESULT.with(|c| c.replace(None));
    let out = f();
    SEED.with(|c| *c.borrow_mut() = previous_seed);
    RESULT.with(|c| *c.borrow_mut() = previous_result);
    out
}

/// Values captured by the innermost finished sub-program
pub fn take_seed_result() -> Option<HashMap<String, JavaScript>> {
    RESULT.with(|c| c.borrow_mut().take())
}

pub fn is_seed_active() -> bool {
    SEED.with(|c| c.borrow().is_some())
}

/// Hands every seeded name to `assign`. Called by `Var` when it enters a `program` node.
pub fn inject_seed<F: FnMut(&str, &JavaScript)>(mut assign: F) {
    SEED.with(|c| {
        if let Some(seed) = c.borrow().as_ref() {
            for (name, value) in seed {
                assign(name, value);
            }
        }
    });
}

/// Reads every seeded name back out of the scope. Called by `Var` when it leaves a `program` node.
pub fn capture_seed_result<F: Fn(&str) -> Option<JavaScript>>(read: F) {
    SEED.with(|seed| {
        let seed = seed.borrow();
        let Some(seed) = seed.as_ref() else {
            return;
        };
        let captured = seed
            .keys()
            .filter_map(|name| read(name).map(|v| (name.clone(), v)))
            .collect();
        RESULT.with(|r| *r.borrow_mut() = Some(captured));
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::js::Value::Num;

    thread_local! {
        static TEST_DEPTH: Cell<usize> = const { Cell::new(0) };
    }

    fn test_counter() -> DepthCounter {
        DepthCounter::new(&TEST_DEPTH, 2, "test")
    }

    #[test]
    fn test_depth_guard_brackets_and_caps() {
        let counter = test_counter();
        assert_eq!(counter.depth(), 0);

        let g1 = counter.enter().unwrap();
        let g2 = counter.enter().unwrap();
        assert_eq!(counter.depth(), 2);
        assert!(counter.enter().is_none());

        drop(g2);
        assert!(counter.enter().is_some());
        drop(g1);
        assert_eq!(counter.depth(), 0);
    }

    #[test]
    fn test_seed_is_restored_on_nested_runs() {
        assert!(!is_seed_active());

        let mut outer = HashMap::new();
        outer.insert("a".to_string(), JavaScript::Raw(Num(1.0)));

        let seen: Option<bool> = with_seed(outer, || {
            let mut names = Vec::new();
            inject_seed(|name, _| names.push(name.to_string()));
            assert_eq!(names, vec!["a".to_string()]);

            let mut inner = HashMap::new();
            inner.insert("b".to_string(), JavaScript::Raw(Num(2.0)));
            with_seed(inner, || {
                let mut inner_names = Vec::new();
                inject_seed(|name, _| inner_names.push(name.to_string()));
                assert_eq!(inner_names, vec!["b".to_string()]);
                Some(())
            })?;

            // the inner run must not have eaten the outer seed
            let mut after = Vec::new();
            inject_seed(|name, _| after.push(name.to_string()));
            assert_eq!(after, vec!["a".to_string()]);
            Some(true)
        });

        assert_eq!(seen, Some(true));
        assert!(!is_seed_active());
    }

    #[test]
    fn test_capture_seed_result_keeps_only_seeded_names() {
        let mut seed = HashMap::new();
        seed.insert("x".to_string(), JavaScript::Raw(Num(0.0)));

        let captured = with_seed(seed, || {
            capture_seed_result(|name| match name {
                "x" => Some(JavaScript::Raw(Num(42.0))),
                _ => Some(JavaScript::Raw(Num(-1.0))),
            });
            take_seed_result()
        })
        .unwrap();

        assert_eq!(captured.len(), 1);
        assert_eq!(captured.get("x"), Some(&JavaScript::Raw(Num(42.0))));
    }
}
