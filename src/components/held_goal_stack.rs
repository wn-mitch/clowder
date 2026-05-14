//! Ticket 320 — `HeldGoalStack` actor-private goal-stack substrate.
//!
//! Sibling Component to 126's [`HeldIntention`](super::held_intention)
//! that carries the cat's HTN method-decomposition cursor. Together
//! they form the cat's full actor-private commitment vector: the top
//! `GoalFrame` names *which method* and *which sub-goal index* the
//! cat is pursuing; `HeldIntention` names *which leaf intention* is
//! actually held.
//!
//! At 320's landing the registry is empty of Live methods (per 319),
//! so this Component is authored on zero cats by design. The wiring
//! is in place; 321 (L1→L2 picker emits `Intention::Goal`) and 323+
//! (first Live Tier-1 method) are the first tickets that exercise
//! frame authorship.
//!
//! # Substrate placement (§4.7.2)
//!
//! `HeldGoalStack` is **substrate**: no `StateEffect::Set*` mutates
//! it during A* expansion (the planner runs over `PlannerState`
//! per sub-goal; the stack is invisible to the planner); external
//! authorship by the L2 evaluator + sibling exclusive system (see
//! `populate_held_goal_stack` in `src/systems/goap.rs`).
//!
//! # Actor-private
//!
//! Like `HeldIntention`, never read across cats. Methods are
//! actor-private decomposition; cross-cat practice state lives in
//! 127's `JointIntention` (mutually-public substrate). Methods that
//! mirror a joint practice (e.g. `courtship_method` in 323) author
//! the method frame on each partner independently — they do not
//! share a `HeldGoalStack`.

use bevy_ecs::prelude::*;

use crate::ai::methods::MethodId;
use crate::components::held_intention::IntentionSource;

/// Maximum frame depth before the stack refuses to push further.
/// Authoring loops (a method recursing into itself, or a cycle in the
/// registry) emit `Feature::MethodDepthExceeded` on cap-hit and fall
/// back to the no-method adoption path. 8 is a Phase-1 guess per the
/// 128 epic design doc; the follow-on instrumentation ticket measures
/// actual depths from a soak and revises if needed.
pub const MAX_GOAL_STACK_DEPTH: usize = 8;

/// One frame on the goal-stack. Names which method is being followed
/// and which sub-goal index inside that method is currently active.
///
/// `Serialize` only (no `Deserialize`): `target: Option<Entity>` has
/// no `Default` and the Component is runtime state. Mirrors
/// `HeldIntention`'s precedent.
#[derive(Debug, Clone, serde::Serialize)]
pub struct GoalFrame {
    /// Stable id of the method this frame is decomposing.
    pub method: MethodId,
    /// The goal-label this method was selected for. Stored so trace
    /// records and backtrack-walks don't have to re-resolve via the
    /// registry every tick.
    pub goal_label: &'static str,
    /// Cursor into `method.sub_goals`. Incremented on
    /// `IntentionFulfilled` of the active leaf.
    pub sub_goal_index: usize,
    /// Total sub-goal count for the method. Captured at push-time so
    /// pop logic can compare without re-walking the registry.
    pub sub_goal_count: usize,
    /// Tick the frame was pushed. Drives `ticks_in_method` for trace.
    pub adopted_tick: u64,
    /// Bound target for target-taking methods (per §6.3). `None` for
    /// methods whose primitives carry their own target resolution.
    /// `Serialize`-skipped per `JointIntention` / `HeldIntention`.
    #[serde(skip)]
    pub target: Option<Entity>,
    /// Where the frame was authored from. `SelfMotivated` for
    /// per-tick DSE-emitted goals; `AspirationEmitted { chain }` when
    /// 321's picker produced the goal from an aspiration's `emits`
    /// table; `CoordinatorDirective` when 057's directive emit path
    /// catches.
    pub source: IntentionSource,
    /// Retry counter for `MethodFailure::Retry { max_attempts }`. Zero
    /// for `Backtrack` and `Abandon` strategies. Incremented on each
    /// retry; the frame falls through to `Backtrack` semantics once
    /// the counter reaches `max_attempts`.
    pub retry_count: u8,
}

impl GoalFrame {
    /// Convenience constructor. `retry_count` starts at zero; the
    /// caller drives the counter for `Retry`-strategy methods.
    pub fn new(
        method: MethodId,
        goal_label: &'static str,
        sub_goal_count: usize,
        adopted_tick: u64,
        target: Option<Entity>,
        source: IntentionSource,
    ) -> Self {
        Self {
            method,
            goal_label,
            sub_goal_index: 0,
            sub_goal_count,
            adopted_tick,
            target,
            source,
            retry_count: 0,
        }
    }

    /// `true` when the cursor has walked past the last sub-goal —
    /// the frame is ready to pop on the next `IntentionFulfilled`.
    pub fn is_complete(&self) -> bool {
        self.sub_goal_index >= self.sub_goal_count
    }
}

/// Per-cat goal-stack. Top of the stack is the *active* frame whose
/// `sub_goal_index` names the leaf currently being pursued; deeper
/// frames are the chain of parent methods waiting for the active leaf
/// to fulfill.
///
/// Capped at [`MAX_GOAL_STACK_DEPTH`] — the L2 evaluator emits
/// `Feature::MethodDepthExceeded` and falls back to the no-method path
/// before pushing the 9th frame.
#[derive(Component, Debug, Clone, Default, serde::Serialize)]
pub struct HeldGoalStack {
    /// Frames in adoption order — `frames[0]` is the root method,
    /// `frames.last()` is the active leaf frame.
    pub frames: Vec<GoalFrame>,
}

impl HeldGoalStack {
    /// Empty stack — the no-method-applies state.
    pub fn empty() -> Self {
        Self { frames: Vec::new() }
    }

    /// Single-frame stack — the common case at L2 author time.
    pub fn from_frame(frame: GoalFrame) -> Self {
        Self { frames: vec![frame] }
    }

    /// Reference to the active (top) frame.
    pub fn top(&self) -> Option<&GoalFrame> {
        self.frames.last()
    }

    /// Mutable reference to the active frame. Used by the advance /
    /// backtrack / retry walkers in `resolve_goap_plans` to bump
    /// `sub_goal_index` or `retry_count`.
    pub fn top_mut(&mut self) -> Option<&mut GoalFrame> {
        self.frames.last_mut()
    }

    /// `true` when no frames are held.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Push a new frame onto the stack. Returns `Err(frame)` when the
    /// push would exceed [`MAX_GOAL_STACK_DEPTH`] — the caller emits
    /// `Feature::MethodDepthExceeded` and falls through to the no-
    /// method adoption path.
    pub fn push(&mut self, frame: GoalFrame) -> Result<(), GoalFrame> {
        if self.frames.len() >= MAX_GOAL_STACK_DEPTH {
            return Err(frame);
        }
        self.frames.push(frame);
        Ok(())
    }

    /// Pop the active frame. Returns `None` when the stack is empty.
    pub fn pop(&mut self) -> Option<GoalFrame> {
        self.frames.pop()
    }

    /// Current depth (zero when empty).
    pub fn depth(&self) -> usize {
        self.frames.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_frame(label: &'static str) -> GoalFrame {
        GoalFrame::new(
            MethodId(label),
            label,
            3,
            100,
            None,
            IntentionSource::SelfMotivated,
        )
    }

    #[test]
    fn empty_stack_has_no_top_and_zero_depth() {
        let s = HeldGoalStack::empty();
        assert!(s.is_empty());
        assert_eq!(s.depth(), 0);
        assert!(s.top().is_none());
    }

    #[test]
    fn push_and_top_round_trip() {
        let mut s = HeldGoalStack::empty();
        s.push(fixture_frame("a")).unwrap();
        s.push(fixture_frame("b")).unwrap();
        assert_eq!(s.depth(), 2);
        assert_eq!(s.top().unwrap().goal_label, "b");
    }

    #[test]
    fn push_at_depth_cap_returns_err() {
        let mut s = HeldGoalStack::empty();
        for i in 0..MAX_GOAL_STACK_DEPTH {
            // Leak a static label per index for the fixture; the test
            // only inspects depth.
            let label: &'static str = Box::leak(format!("m{i}").into_boxed_str());
            s.push(fixture_frame(label)).unwrap();
        }
        assert_eq!(s.depth(), MAX_GOAL_STACK_DEPTH);
        let overflow = fixture_frame("overflow");
        let returned = s.push(overflow).expect_err("push past cap must err");
        assert_eq!(returned.goal_label, "overflow");
        assert_eq!(s.depth(), MAX_GOAL_STACK_DEPTH);
    }

    #[test]
    fn pop_returns_top_and_shrinks() {
        let mut s = HeldGoalStack::empty();
        s.push(fixture_frame("a")).unwrap();
        s.push(fixture_frame("b")).unwrap();
        let popped = s.pop().unwrap();
        assert_eq!(popped.goal_label, "b");
        assert_eq!(s.depth(), 1);
        assert_eq!(s.top().unwrap().goal_label, "a");
    }

    #[test]
    fn top_mut_advances_cursor() {
        let mut s = HeldGoalStack::from_frame(fixture_frame("a"));
        s.top_mut().unwrap().sub_goal_index += 1;
        assert_eq!(s.top().unwrap().sub_goal_index, 1);
        assert!(!s.top().unwrap().is_complete());
        s.top_mut().unwrap().sub_goal_index = 3;
        assert!(s.top().unwrap().is_complete());
    }

    #[test]
    fn from_frame_holds_one() {
        let s = HeldGoalStack::from_frame(fixture_frame("a"));
        assert_eq!(s.depth(), 1);
        assert_eq!(s.top().unwrap().goal_label, "a");
    }
}
