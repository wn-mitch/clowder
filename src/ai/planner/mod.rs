pub mod actions;
pub mod core;
pub mod goals;

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

// ---------------------------------------------------------------------------
// PlannerState — compact, hashable state for A* search
// ---------------------------------------------------------------------------

/// Abstract zone categories. Resolved to concrete positions at execution time.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PlannerZone {
    Stores,
    HuntingGround,
    ForagingGround,
    Farm,
    ConstructionSite,
    HerbPatch,
    Kitchen,
    RestingSpot,
    SocialTarget,
    Wilds,
    PatrolZone,
    /// On-the-ground material pile. Resolves to the nearest `Item` entity
    /// whose kind maps to a build `Material` and whose location is
    /// `OnGround`. The wagon-dismantling founding flow spawns these next
    /// to founding sites; cats haul them in via the
    /// `PickupMaterial` → `DeliverMaterials` plan leg.
    MaterialPile,
}

/// What the cat is carrying.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Carrying {
    Nothing,
    Prey,
    ForagedFood,
    RawFood,
    CookedFood,
    BuildMaterials,
    Herbs,
    Remedy,
}

/// Compact world state from the planner's perspective.
/// Constructed from ECS queries on demand — never stored persistently.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct PlannerState {
    pub zone: PlannerZone,
    pub carrying: Carrying,
    pub trips_done: u32,
    pub hunger_ok: bool,
    pub energy_ok: bool,
    pub temperature_ok: bool,
    pub interaction_done: bool,
    pub construction_done: bool,
    pub prey_found: bool,
    pub farm_tended: bool,
    pub thornbriar_available: bool,
    /// True iff at least one reachable `ConstructionSite` has
    /// `materials_complete() == true`. Construct gates on this; founding
    /// builds only flip true after enough Pickup→Deliver cycles fill the
    /// site's materials ledger. Coarse for the founding-only scope (one
    /// site at a time); per-site tracking is the open follow-on.
    pub materials_available: bool,
}

// ---------------------------------------------------------------------------
// GoapActionKind — identity of each planner-visible action
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GoapActionKind {
    TravelTo(PlannerZone),
    // Hunting
    SearchPrey,
    EngagePrey,
    DepositPrey,
    // Foraging
    ForageItem,
    DepositFood,
    // Resting
    EatAtStores,
    Sleep,
    SelfGroom,
    // Guarding
    PatrolArea,
    EngageThreat,
    Survey,
    // Socializing
    SocializeWith,
    GroomOther,
    MentorCat,
    // Building
    GatherMaterials,
    DeliverMaterials,
    Construct,
    // Farming
    TendCrops,
    HarvestCrops,
    // Crafting / Magic
    GatherHerb,
    PrepareRemedy,
    ApplyRemedy,
    SetWard,
    Scry,
    SpiritCommunion,
    // Cooking
    RetrieveRawFood,
    Cook,
    DepositCookedFood,
    // Cleansing
    CleanseCorruption,
    HarvestCarcass,
    // Other dispositions
    MateWith,
    FeedKitten,
    /// Retrieve any food (raw OR cooked) from Stores into the adult's
    /// inventory so the subsequent FeedKitten step has something to
    /// transfer. Phase 4c.4 — Phase 4c.3 wired the retrieve step in
    /// the disposition-chain path (`StepKind::RetrieveAnyFoodFromStores`)
    /// but forgot to add the GOAP mirror, so the scheduled planner built
    /// `[TravelTo(Stores), FeedKitten]` with empty inventory and no-op'd.
    RetrieveFoodForKitten,
    DeliverDirective,
    ExploreSurvey,
}

// ---------------------------------------------------------------------------
// Predicates and effects — data-driven, not function pointers
// ---------------------------------------------------------------------------

/// A condition over `PlannerState` that must hold for an action to be applicable,
/// or that defines a goal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatePredicate {
    ZoneIs(PlannerZone),
    ZoneIsNot(PlannerZone),
    CarryingIs(Carrying),
    CarryingIsNot(Carrying),
    PreyFound(bool),
    HungerOk(bool),
    EnergyOk(bool),
    TemperatureOk(bool),
    InteractionDone(bool),
    ConstructionDone(bool),
    FarmTended(bool),
    ThornbriarAvailable(bool),
    TripsAtLeast(u32),
    MaterialsAvailable(bool),
}

impl StatePredicate {
    pub fn evaluate(&self, state: &PlannerState) -> bool {
        match self {
            Self::ZoneIs(z) => state.zone == *z,
            Self::ZoneIsNot(z) => state.zone != *z,
            Self::CarryingIs(c) => state.carrying == *c,
            Self::CarryingIsNot(c) => state.carrying != *c,
            Self::PreyFound(v) => state.prey_found == *v,
            Self::HungerOk(v) => state.hunger_ok == *v,
            Self::EnergyOk(v) => state.energy_ok == *v,
            Self::TemperatureOk(v) => state.temperature_ok == *v,
            Self::InteractionDone(v) => state.interaction_done == *v,
            Self::ConstructionDone(v) => state.construction_done == *v,
            Self::FarmTended(v) => state.farm_tended == *v,
            Self::ThornbriarAvailable(v) => state.thornbriar_available == *v,
            Self::TripsAtLeast(n) => state.trips_done >= *n,
            Self::MaterialsAvailable(v) => state.materials_available == *v,
        }
    }
}

/// A mutation to apply to `PlannerState` when an action executes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateEffect {
    SetZone(PlannerZone),
    SetCarrying(Carrying),
    SetPreyFound(bool),
    SetHungerOk(bool),
    SetEnergyOk(bool),
    SetTemperatureOk(bool),
    SetInteractionDone(bool),
    SetConstructionDone(bool),
    SetFarmTended(bool),
    IncrementTrips,
    SetMaterialsAvailable(bool),
}

impl StateEffect {
    pub fn apply(&self, state: &mut PlannerState) {
        match self {
            Self::SetZone(z) => state.zone = *z,
            Self::SetCarrying(c) => state.carrying = *c,
            Self::SetPreyFound(v) => state.prey_found = *v,
            Self::SetHungerOk(v) => state.hunger_ok = *v,
            Self::SetEnergyOk(v) => state.energy_ok = *v,
            Self::SetTemperatureOk(v) => state.temperature_ok = *v,
            Self::SetInteractionDone(v) => state.interaction_done = *v,
            Self::SetConstructionDone(v) => state.construction_done = *v,
            Self::SetFarmTended(v) => state.farm_tended = *v,
            Self::IncrementTrips => state.trips_done += 1,
            Self::SetMaterialsAvailable(v) => state.materials_available = *v,
        }
    }
}

// ---------------------------------------------------------------------------
// GoapActionDef — declarative action table entry
// ---------------------------------------------------------------------------

/// A GOAP action definition with data-driven preconditions and effects.
#[derive(Debug, Clone)]
pub struct GoapActionDef {
    pub kind: GoapActionKind,
    pub cost: u32,
    pub preconditions: Vec<StatePredicate>,
    pub effects: Vec<StateEffect>,
}

impl GoapActionDef {
    pub fn is_applicable(&self, state: &PlannerState) -> bool {
        self.preconditions.iter().all(|p| p.evaluate(state))
    }

    pub fn apply(&self, state: &PlannerState) -> PlannerState {
        let mut next = state.clone();
        for effect in &self.effects {
            effect.apply(&mut next);
        }
        next
    }
}

// ---------------------------------------------------------------------------
// GoalState
// ---------------------------------------------------------------------------

/// A goal is a set of predicates that must all be satisfied.
#[derive(Debug, Clone)]
pub struct GoalState {
    pub predicates: Vec<StatePredicate>,
}

impl GoalState {
    pub fn is_satisfied(&self, state: &PlannerState) -> bool {
        self.predicates.iter().all(|p| p.evaluate(state))
    }

    /// Admissible heuristic: count of unsatisfied goal predicates.
    pub fn heuristic(&self, state: &PlannerState) -> u32 {
        self.predicates
            .iter()
            .filter(|p| !p.evaluate(state))
            .count() as u32
    }
}

// ---------------------------------------------------------------------------
// PlannedStep — output of the planner
// ---------------------------------------------------------------------------

/// A single step in a plan produced by the A* search.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlannedStep {
    pub action: GoapActionKind,
    pub cost: u32,
}

// ---------------------------------------------------------------------------
// ZoneDistances — travel cost matrix for parameterized TravelTo actions
// ---------------------------------------------------------------------------

/// Pre-computed distances between abstract zones, built from ECS spatial queries.
#[derive(Debug, Clone, Default)]
pub struct ZoneDistances {
    pub distances: HashMap<(PlannerZone, PlannerZone), u32>,
}

impl ZoneDistances {
    /// Register a bidirectional travel cost between two zones.
    pub fn set(&mut self, from: PlannerZone, to: PlannerZone, cost: u32) {
        self.distances.insert((from, to), cost);
    }

    pub fn get(&self, from: PlannerZone, to: PlannerZone) -> Option<u32> {
        self.distances.get(&(from, to)).copied()
    }
}

// ---------------------------------------------------------------------------
// A* planner
// ---------------------------------------------------------------------------

/// Search node in the A* arena.
struct SearchNode {
    state: PlannerState,
    g_cost: u32,
    parent: Option<usize>,
    action: Option<GoapActionKind>,
    action_cost: u32,
    depth: usize,
}

/// Run A* search over `PlannerState` to find a plan that satisfies `goal`.
///
/// Returns `None` if no plan is found within the search bounds.
pub fn make_plan(
    start: PlannerState,
    actions: &[GoapActionDef],
    goal: &GoalState,
    max_depth: usize,
    max_nodes: usize,
) -> Option<Vec<PlannedStep>> {
    // Early exit: already at goal.
    if goal.is_satisfied(&start) {
        return Some(Vec::new());
    }

    // Arena of search nodes.
    let mut arena: Vec<SearchNode> = Vec::with_capacity(256);

    // Open set: min-heap by (f_cost, insertion order for tiebreak).
    let mut open: BinaryHeap<Reverse<(u32, usize)>> = BinaryHeap::new();

    // Best known g_cost per state.
    let mut best_g: HashMap<PlannerState, u32> = HashMap::new();

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
fn reconstruct_path(arena: &[SearchNode], goal_idx: usize) -> Vec<PlannedStep> {
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
// CatDomain — implements GoapDomain for the cat planner
// ---------------------------------------------------------------------------

/// Marker type binding the cat-specific planner types to the generic core.
pub struct CatDomain;

impl core::GoapDomain for CatDomain {
    type State = PlannerState;
    type ActionKind = GoapActionKind;
    type Predicate = StatePredicate;
    type Effect = StateEffect;

    fn evaluate(pred: &StatePredicate, state: &PlannerState) -> bool {
        pred.evaluate(state)
    }

    fn apply(effect: &StateEffect, state: &mut PlannerState) {
        effect.apply(state);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn default_state() -> PlannerState {
        PlannerState {
            zone: PlannerZone::Wilds,
            carrying: Carrying::Nothing,
            trips_done: 0,
            hunger_ok: true,
            energy_ok: true,
            temperature_ok: true,
            interaction_done: false,
            construction_done: false,
            prey_found: false,
            farm_tended: false,
            thornbriar_available: false,
            materials_available: false,
        }
    }

    fn hunting_actions_with_travel() -> Vec<GoapActionDef> {
        vec![
            // TravelTo(HuntingGround) from any zone != HuntingGround
            GoapActionDef {
                kind: GoapActionKind::TravelTo(PlannerZone::HuntingGround),
                cost: 3,
                preconditions: vec![StatePredicate::ZoneIsNot(PlannerZone::HuntingGround)],
                effects: vec![StateEffect::SetZone(PlannerZone::HuntingGround)],
            },
            // TravelTo(Stores) from any zone != Stores
            GoapActionDef {
                kind: GoapActionKind::TravelTo(PlannerZone::Stores),
                cost: 3,
                preconditions: vec![StatePredicate::ZoneIsNot(PlannerZone::Stores)],
                effects: vec![StateEffect::SetZone(PlannerZone::Stores)],
            },
            // SearchPrey
            GoapActionDef {
                kind: GoapActionKind::SearchPrey,
                cost: 3,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::HuntingGround),
                    StatePredicate::CarryingIs(Carrying::Nothing),
                ],
                effects: vec![StateEffect::SetPreyFound(true)],
            },
            // EngagePrey
            GoapActionDef {
                kind: GoapActionKind::EngagePrey,
                cost: 2,
                preconditions: vec![StatePredicate::PreyFound(true)],
                effects: vec![
                    StateEffect::SetCarrying(Carrying::Prey),
                    StateEffect::SetPreyFound(false),
                ],
            },
            // DepositPrey
            GoapActionDef {
                kind: GoapActionKind::DepositPrey,
                cost: 1,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::Stores),
                    StatePredicate::CarryingIs(Carrying::Prey),
                ],
                effects: vec![
                    StateEffect::SetCarrying(Carrying::Nothing),
                    StateEffect::IncrementTrips,
                ],
            },
        ]
    }

    #[test]
    fn hunting_plan_from_wilds() {
        let start = default_state();
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let actions = hunting_actions_with_travel();

        let plan = make_plan(start, &actions, &goal, 12, 1000).expect("should find plan");

        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert_eq!(
            kinds,
            vec![
                GoapActionKind::TravelTo(PlannerZone::HuntingGround),
                GoapActionKind::SearchPrey,
                GoapActionKind::EngagePrey,
                GoapActionKind::TravelTo(PlannerZone::Stores),
                GoapActionKind::DepositPrey,
            ]
        );
    }

    #[test]
    fn hunting_plan_already_at_hunting_ground() {
        let start = PlannerState {
            zone: PlannerZone::HuntingGround,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let actions = hunting_actions_with_travel();

        let plan = make_plan(start, &actions, &goal, 12, 1000).expect("should find plan");

        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert_eq!(
            kinds,
            vec![
                GoapActionKind::SearchPrey,
                GoapActionKind::EngagePrey,
                GoapActionKind::TravelTo(PlannerZone::Stores),
                GoapActionKind::DepositPrey,
            ]
        );
    }

    #[test]
    fn resting_plan_hungry_only() {
        let start = PlannerState {
            zone: PlannerZone::Wilds,
            hunger_ok: false,
            energy_ok: true,
            temperature_ok: true,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![
                StatePredicate::HungerOk(true),
                StatePredicate::EnergyOk(true),
                StatePredicate::TemperatureOk(true),
            ],
        };
        let actions = vec![
            GoapActionDef {
                kind: GoapActionKind::TravelTo(PlannerZone::Stores),
                cost: 2,
                preconditions: vec![StatePredicate::ZoneIsNot(PlannerZone::Stores)],
                effects: vec![StateEffect::SetZone(PlannerZone::Stores)],
            },
            GoapActionDef {
                kind: GoapActionKind::EatAtStores,
                cost: 2,
                preconditions: vec![StatePredicate::ZoneIs(PlannerZone::Stores)],
                effects: vec![StateEffect::SetHungerOk(true)],
            },
            GoapActionDef {
                kind: GoapActionKind::TravelTo(PlannerZone::RestingSpot),
                cost: 2,
                preconditions: vec![StatePredicate::ZoneIsNot(PlannerZone::RestingSpot)],
                effects: vec![StateEffect::SetZone(PlannerZone::RestingSpot)],
            },
            GoapActionDef {
                kind: GoapActionKind::Sleep,
                cost: 2,
                preconditions: vec![StatePredicate::ZoneIs(PlannerZone::RestingSpot)],
                effects: vec![StateEffect::SetEnergyOk(true)],
            },
            GoapActionDef {
                kind: GoapActionKind::SelfGroom,
                cost: 1,
                preconditions: vec![],
                effects: vec![StateEffect::SetTemperatureOk(true)],
            },
        ];

        let plan = make_plan(start, &actions, &goal, 12, 1000).expect("should find plan");

        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        // Should go to stores, eat, then done (energy and warmth already ok).
        assert_eq!(
            kinds,
            vec![
                GoapActionKind::TravelTo(PlannerZone::Stores),
                GoapActionKind::EatAtStores,
            ]
        );
    }

    #[test]
    fn resting_plan_all_needs_unmet() {
        let start = PlannerState {
            zone: PlannerZone::Wilds,
            hunger_ok: false,
            energy_ok: false,
            temperature_ok: false,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![
                StatePredicate::HungerOk(true),
                StatePredicate::EnergyOk(true),
                StatePredicate::TemperatureOk(true),
            ],
        };
        let actions = vec![
            GoapActionDef {
                kind: GoapActionKind::TravelTo(PlannerZone::Stores),
                cost: 2,
                preconditions: vec![StatePredicate::ZoneIsNot(PlannerZone::Stores)],
                effects: vec![StateEffect::SetZone(PlannerZone::Stores)],
            },
            GoapActionDef {
                kind: GoapActionKind::TravelTo(PlannerZone::RestingSpot),
                cost: 2,
                preconditions: vec![StatePredicate::ZoneIsNot(PlannerZone::RestingSpot)],
                effects: vec![StateEffect::SetZone(PlannerZone::RestingSpot)],
            },
            GoapActionDef {
                kind: GoapActionKind::EatAtStores,
                cost: 2,
                preconditions: vec![StatePredicate::ZoneIs(PlannerZone::Stores)],
                effects: vec![StateEffect::SetHungerOk(true)],
            },
            GoapActionDef {
                kind: GoapActionKind::Sleep,
                cost: 2,
                preconditions: vec![StatePredicate::ZoneIs(PlannerZone::RestingSpot)],
                effects: vec![StateEffect::SetEnergyOk(true)],
            },
            GoapActionDef {
                kind: GoapActionKind::SelfGroom,
                cost: 1,
                preconditions: vec![],
                effects: vec![StateEffect::SetTemperatureOk(true)],
            },
        ];

        let plan = make_plan(start, &actions, &goal, 12, 1000).expect("should find plan");

        // All three needs addressed. SelfGroom is cheapest and has no preconditions,
        // so the planner may weave it in wherever it's cheapest.
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert!(kinds.contains(&GoapActionKind::EatAtStores));
        assert!(kinds.contains(&GoapActionKind::Sleep));
        assert!(kinds.contains(&GoapActionKind::SelfGroom));
        // Total cost should be optimal.
        let total_cost: u32 = plan.iter().map(|s| s.cost).sum();
        // Optimal: SelfGroom(1) + TravelTo(Stores,2) + Eat(2) + TravelTo(Rest,2) + Sleep(2) = 9
        assert_eq!(total_cost, 9);
    }

    #[test]
    fn no_plan_when_impossible() {
        let start = default_state();
        let goal = GoalState {
            predicates: vec![StatePredicate::HungerOk(false)],
        };
        // Hunger is already ok=true, and no action sets it to false.
        let actions = vec![GoapActionDef {
            kind: GoapActionKind::SelfGroom,
            cost: 1,
            preconditions: vec![],
            effects: vec![StateEffect::SetTemperatureOk(true)],
        }];

        let plan = make_plan(start, &actions, &goal, 12, 1000);
        assert!(plan.is_none());
    }

    #[test]
    fn empty_plan_when_goal_already_satisfied() {
        let start = PlannerState {
            zone: PlannerZone::Stores,
            carrying: Carrying::Nothing,
            trips_done: 3,
            hunger_ok: true,
            energy_ok: true,
            temperature_ok: true,
            interaction_done: false,
            construction_done: false,
            prey_found: false,
            farm_tended: false,
            thornbriar_available: false,
            materials_available: false,
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(3)],
        };

        let plan = make_plan(start, &[], &goal, 12, 1000).expect("should find plan");
        assert!(plan.is_empty());
    }

    #[test]
    fn max_nodes_cap_prevents_runaway() {
        // Create a situation with many states but no solution in sight.
        let start = default_state();
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(100)],
        };
        let actions = vec![GoapActionDef {
            kind: GoapActionKind::Survey,
            cost: 1,
            preconditions: vec![],
            effects: vec![StateEffect::IncrementTrips],
        }];

        // Max nodes = 10, but reaching trips=100 requires 100 expansions.
        let plan = make_plan(start, &actions, &goal, 200, 10);
        assert!(plan.is_none());
    }

    #[test]
    fn max_depth_limits_plan_length() {
        let start = default_state();
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(5)],
        };
        let actions = vec![GoapActionDef {
            kind: GoapActionKind::Survey,
            cost: 1,
            preconditions: vec![],
            effects: vec![StateEffect::IncrementTrips],
        }];

        // Max depth = 3, but reaching trips=5 requires 5 steps.
        let plan = make_plan(start, &actions, &goal, 3, 1000);
        assert!(plan.is_none());
    }

    #[test]
    fn planner_prefers_cheaper_path() {
        let start = PlannerState {
            zone: PlannerZone::Wilds,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::InteractionDone(true)],
        };

        // Two routes to SocialTarget: expensive direct vs cheap via Stores.
        let actions = vec![
            // Expensive direct route.
            GoapActionDef {
                kind: GoapActionKind::TravelTo(PlannerZone::SocialTarget),
                cost: 10,
                preconditions: vec![StatePredicate::ZoneIs(PlannerZone::Wilds)],
                effects: vec![StateEffect::SetZone(PlannerZone::SocialTarget)],
            },
            // Cheap hop via Stores.
            GoapActionDef {
                kind: GoapActionKind::TravelTo(PlannerZone::Stores),
                cost: 1,
                preconditions: vec![StatePredicate::ZoneIs(PlannerZone::Wilds)],
                effects: vec![StateEffect::SetZone(PlannerZone::Stores)],
            },
            GoapActionDef {
                kind: GoapActionKind::TravelTo(PlannerZone::SocialTarget),
                cost: 1,
                preconditions: vec![StatePredicate::ZoneIs(PlannerZone::Stores)],
                effects: vec![StateEffect::SetZone(PlannerZone::SocialTarget)],
            },
            GoapActionDef {
                kind: GoapActionKind::SocializeWith,
                cost: 2,
                preconditions: vec![StatePredicate::ZoneIs(PlannerZone::SocialTarget)],
                effects: vec![StateEffect::SetInteractionDone(true)],
            },
        ];

        let plan = make_plan(start, &actions, &goal, 12, 1000).expect("should find plan");
        let total_cost: u32 = plan.iter().map(|s| s.cost).sum();
        // Cheap route: Stores(1) + SocialTarget(1) + Socialize(2) = 4
        // Expensive route: SocialTarget(10) + Socialize(2) = 12
        assert_eq!(total_cost, 4);
    }

    #[test]
    fn foraging_plan() {
        let start = PlannerState {
            zone: PlannerZone::Wilds,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(1)],
        };
        let actions = vec![
            GoapActionDef {
                kind: GoapActionKind::TravelTo(PlannerZone::ForagingGround),
                cost: 2,
                preconditions: vec![StatePredicate::ZoneIsNot(PlannerZone::ForagingGround)],
                effects: vec![StateEffect::SetZone(PlannerZone::ForagingGround)],
            },
            GoapActionDef {
                kind: GoapActionKind::TravelTo(PlannerZone::Stores),
                cost: 3,
                preconditions: vec![StatePredicate::ZoneIsNot(PlannerZone::Stores)],
                effects: vec![StateEffect::SetZone(PlannerZone::Stores)],
            },
            GoapActionDef {
                kind: GoapActionKind::ForageItem,
                cost: 3,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::ForagingGround),
                    StatePredicate::CarryingIs(Carrying::Nothing),
                ],
                effects: vec![StateEffect::SetCarrying(Carrying::ForagedFood)],
            },
            GoapActionDef {
                kind: GoapActionKind::DepositFood,
                cost: 1,
                preconditions: vec![
                    StatePredicate::ZoneIs(PlannerZone::Stores),
                    StatePredicate::CarryingIs(Carrying::ForagedFood),
                ],
                effects: vec![
                    StateEffect::SetCarrying(Carrying::Nothing),
                    StateEffect::IncrementTrips,
                ],
            },
        ];

        let plan = make_plan(start, &actions, &goal, 12, 1000).expect("should find plan");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert_eq!(
            kinds,
            vec![
                GoapActionKind::TravelTo(PlannerZone::ForagingGround),
                GoapActionKind::ForageItem,
                GoapActionKind::TravelTo(PlannerZone::Stores),
                GoapActionKind::DepositFood,
            ]
        );
    }

    #[test]
    fn building_plan() {
        let start = PlannerState {
            zone: PlannerZone::Wilds,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::ConstructionDone(true)],
        };
        let actions = vec![
            GoapActionDef {
                kind: GoapActionKind::TravelTo(PlannerZone::ConstructionSite),
                cost: 3,
                preconditions: vec![StatePredicate::ZoneIsNot(PlannerZone::ConstructionSite)],
                effects: vec![StateEffect::SetZone(PlannerZone::ConstructionSite)],
            },
            GoapActionDef {
                kind: GoapActionKind::Construct,
                cost: 6,
                preconditions: vec![StatePredicate::ZoneIs(PlannerZone::ConstructionSite)],
                effects: vec![StateEffect::SetConstructionDone(true)],
            },
        ];

        let plan = make_plan(start, &actions, &goal, 12, 1000).expect("should find plan");
        let kinds: Vec<_> = plan.iter().map(|s| s.action).collect();
        assert_eq!(
            kinds,
            vec![
                GoapActionKind::TravelTo(PlannerZone::ConstructionSite),
                GoapActionKind::Construct,
            ]
        );
    }
}
