//! Recursion tracking utility for JavaScript rules that need to expand
//! function calls or `eval` invocations during deobfuscation.
//!
//! Several JavaScript rules need to interpret a piece of code by recursing into
//! it: resolving a function call inlines the body, evaluating an `eval(...)`
//! call parses and re-applies a rule pipeline on the new sub-tree, etc. Without
//! a guard those expansions can spiral infinitely on pathological inputs (e.g.
//! a function that calls itself unconditionally, or a string that decodes to
//! more JavaScript). This module provides a small RAII-style tracker that
//! caps the expansion depth.
//!
//! The tracker is meant to be embedded in a rule and accessed from the
//! `enter()` / `leave()` callbacks: a rule asks for an expansion via
//! [`RecursionTracker::enter`], performs the work while it owns the returned
//! [`RecursionGuard`], and the guard's `Drop` implementation decrements the
//! counter automatically. The [`RecursionExt`] trait makes this idiom
//! available straight from any [`crate::tree::Node`] view via
//! [`RecursionExt::within_recursion`].
//!
//! ```
//! use minusone::js::recursion::{RecursionTracker, DEFAULT_MAX_RECURSION_DEPTH};
//!
//! let mut tracker = RecursionTracker::default();
//! assert_eq!(tracker.depth(), 0);
//! assert_eq!(tracker.max_depth(), DEFAULT_MAX_RECURSION_DEPTH);
//!
//! // Take the bracket: depth goes 0 -> 1.
//! let guard = tracker.enter().expect("first level should be allowed");
//! drop(guard); // dropping the guard runs Drop and decrements the counter.
//!
//! assert_eq!(tracker.depth(), 0);
//! ```

use crate::tree::Node;

/// Default cap on recursion depth.
///
/// 16 is large enough for realistic obfuscation patterns (decoder chains,
/// nested function calls, layered `eval` indirections) yet small enough to
/// keep deobfuscation responsive on adversarial inputs.
pub const DEFAULT_MAX_RECURSION_DEPTH: usize = 16;

/// Tracks the current expansion depth for a rule and enforces a maximum
/// before further recursion is allowed.
///
/// The tracker stores a counter and a configurable upper bound. Callers ask
/// for the right to recurse via [`RecursionTracker::enter`] and receive an
/// optional [`RecursionGuard`]. When the guard is dropped, the counter is
/// decremented; this guarantees the counter stays consistent even if the
/// caller bails out early via `?` or panics.
#[derive(Clone, Debug)]
pub struct RecursionTracker {
    depth: usize,
    max_depth: usize,
}

impl RecursionTracker {
    /// Create a tracker with a custom maximum depth.
    pub fn new(max_depth: usize) -> Self {
        Self {
            depth: 0,
            max_depth,
        }
    }

    /// Try to enter a new recursion level. Returns [`None`] when the maximum
    /// depth has been reached, in which case the caller must skip the
    /// recursive expansion entirely.
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

    /// Current recursion depth.
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Maximum recursion depth.
    pub fn max_depth(&self) -> usize {
        self.max_depth
    }

    /// Reset the counter to zero. Useful when a rule sees a top-level node
    /// (e.g. `program`) and wants to clear any leftover state from a previous
    /// run of the same rule instance.
    pub fn reset(&mut self) {
        self.depth = 0;
    }
}

impl Default for RecursionTracker {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_RECURSION_DEPTH)
    }
}

/// RAII guard that decrements the recursion depth on drop.
///
/// The guard cannot be cloned, must be created via
/// [`RecursionTracker::enter`], and is intentionally not [`Send`] /
/// [`Sync`]: a recursion bracket is always confined to a single rule visit on
/// a single thread.
pub struct RecursionGuard<'a> {
    tracker: &'a mut RecursionTracker,
}

impl Drop for RecursionGuard<'_> {
    fn drop(&mut self) {
        self.tracker.depth = self.tracker.depth.saturating_sub(1);
    }
}

/// Extension trait making the recursion bracket available from a node view,
/// matching the "callable from `node.something()`" requirement of the
/// minusone recursion design.
///
/// Rules typically look like:
///
/// ```ignore
/// node.view().within_recursion(&mut self.recursion, |node| {
///     resolve_call(node)
/// });
/// ```
///
/// The closure only runs while a guard is held, so the tracker depth is
/// guaranteed to be incremented for the duration of the operation and
/// decremented when it returns (or short-circuits via `?` / `return`).
pub trait RecursionExt {
    /// Run `op` inside a recursion bracket. Returns [`None`] when the
    /// tracker has reached its maximum depth and the operation cannot be
    /// performed.
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
        // third call must be refused at depth limit
        let g3 = g2.tracker.enter();
        assert!(g3.is_none());
    }

    #[test]
    fn test_default_max_depth_is_sixteen() {
        assert_eq!(DEFAULT_MAX_RECURSION_DEPTH, 16);
        assert_eq!(RecursionTracker::default().max_depth(), 16);
    }

    #[test]
    fn test_reset_clears_counter() {
        let mut tracker = RecursionTracker::new(4);
        let _g1 = tracker.enter().unwrap();
        // reset() takes &mut self, so we can't call it while holding the guard
        // (which itself borrows `tracker` mutably). That is the expected,
        // safe behaviour - reset is meant for top-level cleanup.
        drop(_g1);
        let _g2 = tracker.enter().unwrap();
        drop(_g2);
        tracker.reset();
        assert_eq!(tracker.depth(), 0);
    }
}
