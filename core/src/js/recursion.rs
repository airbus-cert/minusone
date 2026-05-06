//! Depth-tracking utility for rules that recurse into function calls or `eval`.

use crate::tree::Node;
use std::cell::Cell;

thread_local! {
    static GLOBAL_DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// Cross-instance recursion bracket. Sub-pipelines spawn fresh `FnCall`
/// instances with their own `RecursionTracker`, so the per-instance counter
/// would reset to zero on every nested deobfuscation. The thread-local
/// `GLOBAL_DEPTH` is shared between all `FnCall` instances on the same
/// thread, so an inner sub-tree can see how deep the outer caller is and
/// refuse to recurse past `DEFAULT_MAX_RECURSION_DEPTH`.
pub fn try_global_bump() -> bool {
    GLOBAL_DEPTH.with(|c| {
        let depth = c.get();
        if depth >= DEFAULT_MAX_RECURSION_DEPTH {
            log::trace!(
                "global recursion depth {} reached, refusing to recurse",
                depth
            );
            return false;
        }
        c.set(depth + 1);
        true
    })
}

pub fn global_unbump() {
    GLOBAL_DEPTH.with(|c| c.set(c.get().saturating_sub(1)));
}

pub fn global_depth() -> usize {
    GLOBAL_DEPTH.with(|c| c.get())
}

pub const DEFAULT_MAX_RECURSION_DEPTH: usize = 16;

#[derive(Clone, Debug)]
pub struct RecursionTracker {
    depth: usize,
    max_depth: usize,
}

impl RecursionTracker {
    pub fn new(max_depth: usize) -> Self {
        Self {
            depth: 0,
            max_depth,
        }
    }

    pub fn enter(&mut self) -> Option<RecursionGuard<'_>> {
        if self.depth >= self.max_depth {
            log::trace!(
                "RecursionTracker: depth limit {} reached, refusing to recurse",
                self.max_depth
            );
            return None;
        }
        self.depth += 1;
        Some(RecursionGuard { tracker: self })
    }

    pub fn depth(&self) -> usize {
        self.depth
    }

    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    pub fn reset(&mut self) {
        self.depth = 0;
    }

    /// Imperative bracket for recursive callees that need `&mut self` access
    /// (the RAII guard would conflict with the borrow checker). Each successful
    /// `bump` must be paired with exactly one `unbump`.
    pub fn bump(&mut self) -> bool {
        if self.depth >= self.max_depth {
            log::trace!(
                "RecursionTracker: depth limit {} reached, refusing to recurse",
                self.max_depth
            );
            return false;
        }
        self.depth += 1;
        true
    }

    pub fn unbump(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }
}

impl Default for RecursionTracker {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_RECURSION_DEPTH)
    }
}

pub struct RecursionGuard<'a> {
    tracker: &'a mut RecursionTracker,
}

impl Drop for RecursionGuard<'_> {
    fn drop(&mut self) {
        self.tracker.depth = self.tracker.depth.saturating_sub(1);
    }
}

pub trait RecursionExt {
    fn within_recursion<F, R>(&self, tracker: &mut RecursionTracker, op: F) -> Option<R>
    where
        F: FnOnce(&Self) -> R;
}

impl<'a, T> RecursionExt for Node<'a, T> {
    fn within_recursion<F, R>(&self, tracker: &mut RecursionTracker, op: F) -> Option<R>
    where
        F: FnOnce(&Self) -> R,
    {
        let _guard = tracker.enter()?;
        Some(op(self))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracker_increments_and_decrements() {
        let mut tracker = RecursionTracker::default();
        assert_eq!(tracker.depth(), 0);

        let guard = tracker.enter().unwrap();
        assert_eq!(guard.tracker.depth, 1);
        drop(guard);

        assert_eq!(tracker.depth(), 0);
    }

    #[test]
    fn test_tracker_caps_depth() {
        let mut tracker = RecursionTracker::new(2);
        let g1 = tracker.enter().unwrap();
        let g2 = g1.tracker.enter().unwrap();
        let g3 = g2.tracker.enter();
        assert!(g3.is_none());
    }

    #[test]
    fn test_default_max_depth_is_sixteen() {
        assert_eq!(DEFAULT_MAX_RECURSION_DEPTH, 16);
        assert_eq!(RecursionTracker::default().max_depth(), 16);
    }

    #[test]
    fn test_bump_unbump_roundtrip() {
        let mut tracker = RecursionTracker::new(3);
        assert!(tracker.bump());
        assert!(tracker.bump());
        assert!(tracker.bump());
        assert!(!tracker.bump());
        tracker.unbump();
        assert!(tracker.bump());
        tracker.unbump();
        tracker.unbump();
        tracker.unbump();
        tracker.unbump();
        assert_eq!(tracker.depth(), 0);
    }

    #[test]
    fn test_reset_clears_counter() {
        let mut tracker = RecursionTracker::new(4);
        let _g1 = tracker.enter().unwrap();
        drop(_g1);
        let _g2 = tracker.enter().unwrap();
        drop(_g2);
        tracker.reset();
        assert_eq!(tracker.depth(), 0);
    }
}
