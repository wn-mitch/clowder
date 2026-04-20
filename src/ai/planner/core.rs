//! Generic GOAP (Goal-Oriented Action Planning) A* planner.
//!
//! Species-specific planners implement the [`GoapDomain`] trait, which bundles
//! the state, action, predicate, and effect types. The A* search in
//! [`make_plan`] is fully generic over any domain.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};
use std::fmt::Debug;
use std::hash::Hash;

// ---------------------------------------------------------------------------
// GoapDomain trait — implemented per species
// ---------------------------------------------------------------------------

/// Defines the type family for a species' GOAP planner.
///
/// Each species (cats, foxes, eagles, …) implements this trait once. The
/// associated types are all concrete enums specific to that species.
pub trait GoapDomain {
    /// Compact, hashable snapshot of the planner-visible world state.
    type State: Hash + Eq + Clone;
    /// Identity tag for each action (e.g. `SearchPrey`, `DepositScent`).
    type ActionKind: Clone + Copy + Debug;
    /// A boolean condition over `State`.
    type Predicate;
    /// A mutation to apply to `State` when an action executes.
    type Effect;

    /// Evaluate whether `pred` holds in `state`.
    fn evaluate(pred: &Self::Predicate, state: &Self::State) -> bool;
    /// Apply `effect` to `state` in place.
    fn apply(effect: &Self::Effect, state: &mut Self::State);
}

// ---------------------------------------------------------------------------
// Generic planner types
// ---------------------------------------------------------------------------

/// A GOAP action definition with data-driven preconditions and effects.
#[derive(Debug, Clone)]
pub struct ActionDef<D: GoapDomain> {
    pub kind: D::ActionKind,
    pub cost: u32,
    pub preconditions: Vec<D::Predicate>,
    pub effects: Vec<D::Effect>,
}

impl<D: GoapDomain> ActionDef<D> {
    pub fn is_applicable(&self, state: &D::State) -> bool {
        self.preconditions.iter().all(|p| D::evaluate(p, state))
    }

    pub fn apply(&self, state: &D::State) -> D::State {
        let mut next = state.clone();
        for effect in &self.effects {
            D::apply(effect, &mut next);
        }
        next
    }
}

/// A goal is a set of predicates that must all be satisfied.
#[derive(Debug, Clone)]
pub struct Goal<D: GoapDomain> {
    pub predicates: Vec<D::Predicate>,
}

impl<D: GoapDomain> Goal<D> {
    pub fn is_satisfied(&self, state: &D::State) -> bool {
        self.predicates.iter().all(|p| D::evaluate(p, state))
    }

    /// Admissible heuristic: count of unsatisfied goal predicates.
    pub fn heuristic(&self, state: &D::State) -> u32 {
        self.predicates
            .iter()
            .filter(|p| !D::evaluate(p, state))
            .count() as u32
    }
}

/// A single step in a plan produced by the A* search.
#[derive(Debug, Clone)]
pub struct PlannedStep<D: GoapDomain> {
    pub action: D::ActionKind,
    pub cost: u32,
}

// ---------------------------------------------------------------------------
// A* planner
// ---------------------------------------------------------------------------

/// Search node in the A* arena.
struct SearchNode<D: GoapDomain> {
    state: D::State,
    g_cost: u32,
    parent: Option<usize>,
    action: Option<D::ActionKind>,
    action_cost: u32,
    depth: usize,
}

/// Run A* search to find a plan that satisfies `goal` from `start`.
///
/// Returns `None` if no plan is found within the search bounds.
pub fn make_plan<D: GoapDomain>(
    start: D::State,
    actions: &[ActionDef<D>],
    goal: &Goal<D>,
    max_depth: usize,
    max_nodes: usize,
) -> Option<Vec<PlannedStep<D>>> {
    // Early exit: already at goal.
    if goal.is_satisfied(&start) {
        return Some(Vec::new());
    }

    // Arena of search nodes.
    let mut arena: Vec<SearchNode<D>> = Vec::with_capacity(256);

    // Open set: min-heap by (f_cost, insertion order for tiebreak).
    let mut open: BinaryHeap<Reverse<(u32, usize)>> = BinaryHeap::new();

    // Best known g_cost per state.
    let mut best_g: HashMap<D::State, u32> = HashMap::new();

    // Seed with start state.
    let h = goal.heuristic(&start);
    arena.push(SearchNode {
        state: start.clone(),
        g_cost: 0,
        parent: None,
        action: None,
        action_cost: 0,
        depth: 0,
    });
    open.push(Reverse((h, 0)));
    best_g.insert(start, 0);

    let mut expanded = 0usize;

    while let Some(Reverse((_, node_idx))) = open.pop() {
        expanded += 1;
        if expanded > max_nodes {
            return None;
        }

        let g = arena[node_idx].g_cost;
        let depth = arena[node_idx].depth;

        // Skip if we've already found a cheaper path to this state.
        if g > *best_g.get(&arena[node_idx].state).unwrap_or(&u32::MAX) {
            continue;
        }

        // Goal check at dequeue — this node has the lowest f-cost among
        // unvisited nodes, so if it satisfies the goal it's optimal.
        if goal.is_satisfied(&arena[node_idx].state) {
            return Some(reconstruct_path(&arena, node_idx));
        }

        if depth >= max_depth {
            continue;
        }

        for action in actions {
            if !action.is_applicable(&arena[node_idx].state) {
                continue;
            }

            let next_state = action.apply(&arena[node_idx].state);
            let tentative_g = g.saturating_add(action.cost);

            // Skip if we've already found a cheaper or equal path to this state.
            if tentative_g >= *best_g.get(&next_state).unwrap_or(&u32::MAX) {
                continue;
            }
            best_g.insert(next_state.clone(), tentative_g);

            let h = goal.heuristic(&next_state);
            let f = tentative_g.saturating_add(h);

            arena.push(SearchNode {
                state: next_state,
                g_cost: tentative_g,
                parent: Some(node_idx),
                action: Some(action.kind),
                action_cost: action.cost,
                depth: depth + 1,
            });
            open.push(Reverse((f, arena.len() - 1)));
        }
    }

    None
}

/// Walk parent pointers back to the start to reconstruct the step sequence.
fn reconstruct_path<D: GoapDomain>(
    arena: &[SearchNode<D>],
    goal_idx: usize,
) -> Vec<PlannedStep<D>> {
    let mut steps = Vec::new();
    let mut idx = goal_idx;
    while let Some(parent) = arena[idx].parent {
        if let Some(action) = arena[idx].action {
            steps.push(PlannedStep {
                action,
                cost: arena[idx].action_cost,
            });
        }
        idx = parent;
    }
    steps.reverse();
    steps
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // A minimal test domain to verify the generic planner works independently
    // of any species-specific types.

    #[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
    enum TestAction {
        GoToA,
        GoToB,
        PickUp,
        Deliver,
    }

    #[derive(Debug, Clone, Hash, Eq, PartialEq)]
    struct TestState {
        at_a: bool,
        at_b: bool,
        carrying: bool,
        delivered: bool,
    }

    #[derive(Debug, Clone)]
    enum TestPred {
        AtA(bool),
        AtB(bool),
        Carrying(bool),
        Delivered(bool),
    }

    #[derive(Debug, Clone)]
    enum TestEffect {
        SetAtA(bool),
        SetAtB(bool),
        SetCarrying(bool),
        SetDelivered(bool),
    }

    struct TestDomain;

    impl GoapDomain for TestDomain {
        type State = TestState;
        type ActionKind = TestAction;
        type Predicate = TestPred;
        type Effect = TestEffect;

        fn evaluate(pred: &TestPred, state: &TestState) -> bool {
            match pred {
                TestPred::AtA(v) => state.at_a == *v,
                TestPred::AtB(v) => state.at_b == *v,
                TestPred::Carrying(v) => state.carrying == *v,
                TestPred::Delivered(v) => state.delivered == *v,
            }
        }

        fn apply(effect: &TestEffect, state: &mut TestState) {
            match effect {
                TestEffect::SetAtA(v) => {
                    state.at_a = *v;
                    if *v {
                        state.at_b = false;
                    }
                }
                TestEffect::SetAtB(v) => {
                    state.at_b = *v;
                    if *v {
                        state.at_a = false;
                    }
                }
                TestEffect::SetCarrying(v) => state.carrying = *v,
                TestEffect::SetDelivered(v) => state.delivered = *v,
            }
        }
    }

    fn test_actions() -> Vec<ActionDef<TestDomain>> {
        vec![
            ActionDef {
                kind: TestAction::GoToA,
                cost: 2,
                preconditions: vec![TestPred::AtA(false)],
                effects: vec![TestEffect::SetAtA(true)],
            },
            ActionDef {
                kind: TestAction::GoToB,
                cost: 2,
                preconditions: vec![TestPred::AtB(false)],
                effects: vec![TestEffect::SetAtB(true)],
            },
            ActionDef {
                kind: TestAction::PickUp,
                cost: 1,
                preconditions: vec![TestPred::AtA(true), TestPred::Carrying(false)],
                effects: vec![TestEffect::SetCarrying(true)],
            },
            ActionDef {
                kind: TestAction::Deliver,
                cost: 1,
                preconditions: vec![TestPred::AtB(true), TestPred::Carrying(true)],
                effects: vec![
                    TestEffect::SetCarrying(false),
                    TestEffect::SetDelivered(true),
                ],
            },
        ]
    }

    #[test]
    fn generic_planner_finds_delivery_plan() {
        let start = TestState {
            at_a: false,
            at_b: false,
            carrying: false,
            delivered: false,
        };
        let goal = Goal {
            predicates: vec![TestPred::Delivered(true)],
        };

        let plan =
            make_plan::<TestDomain>(start, &test_actions(), &goal, 10, 1000).expect("should plan");

        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert_eq!(
            kinds,
            vec![
                TestAction::GoToA,
                TestAction::PickUp,
                TestAction::GoToB,
                TestAction::Deliver,
            ]
        );
    }

    #[test]
    fn generic_planner_empty_when_satisfied() {
        let start = TestState {
            at_a: false,
            at_b: true,
            carrying: false,
            delivered: true,
        };
        let goal = Goal {
            predicates: vec![TestPred::Delivered(true)],
        };

        let plan =
            make_plan::<TestDomain>(start, &test_actions(), &goal, 10, 1000).expect("should plan");
        assert!(plan.is_empty());
    }

    #[test]
    fn generic_planner_none_when_impossible() {
        let start = TestState {
            at_a: false,
            at_b: false,
            carrying: false,
            delivered: false,
        };
        // No action can set AtA(false) once you're at A.
        let goal = Goal {
            predicates: vec![TestPred::Delivered(true)],
        };
        // Only give GoToA — no PickUp, Deliver, or GoToB.
        let actions = vec![ActionDef {
            kind: TestAction::GoToA,
            cost: 2,
            preconditions: vec![TestPred::AtA(false)],
            effects: vec![TestEffect::SetAtA(true)],
        }];

        let plan = make_plan::<TestDomain>(start, &actions, &goal, 10, 1000);
        assert!(plan.is_none());
    }
}
