pub mod actions;
pub mod core;
pub mod goals;

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap};

use bevy::prelude::Entity;

use crate::ai::scoring::MarkerSnapshot;

/// Read-only context threaded through `make_plan`'s A* loop alongside
/// `PlannerState`. Carries the `MarkerSnapshot` (the IAUS substrate's
/// authored facts) and the entity being planned for, so
/// `StatePredicate::HasMarker(...)` evaluates against the same source of
/// truth that DSE `EligibilityFilter` consults — collapsing the parallel
/// feasibility languages 092 retired.
///
/// Lives outside `PlannerState` so the search node stays `Hash + Eq`;
/// the snapshot is immutable across the search and is passed by reference.
pub struct PlanContext<'a> {
    pub markers: &'a MarkerSnapshot,
    pub entity: Entity,
}

// ---------------------------------------------------------------------------
// PlannerState — compact, hashable state for A* search
// ---------------------------------------------------------------------------

/// Abstract zone categories. Resolved to concrete positions at execution time.
///
/// `PartialOrd, Ord` exist so `(PlannerZone, PlannerZone)` can key a
/// `BTreeMap` in `ZoneDistances` — the planner's action list is built by
/// iterating that map, and we need a stable iteration order for replay
/// determinism (HashMap order seeded the GOAP A* tiebreak and let same-seed
/// runs pick `TravelTo(Kitchen)` vs `TravelTo(ForagingGround)` for the same
/// cat-state).
#[derive(
    Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
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
    /// Ticket 193: on-the-ground pickable food. Resolves to the nearest
    /// `Item` entity whose `kind.is_food()` and whose `location` is
    /// `OnGround`. Today's source is the `engage_prey` overflow path
    /// (`goap.rs::resolve_engage_prey` spawns an `Item` at the kill tile
    /// when inventory is full and the cat isn't self-eating). Forward-
    /// compatible with the future "carcass-as-container" loot-table
    /// surface — child Items spawned at a `Carcass` entity's tile will
    /// appear in the same OnGround food-Item snapshot without any zone
    /// reshape. Replaces the 176 `MaterialPile` stub that
    /// `picking_up_actions` previously routed through.
    CarcassPile,
    /// 035: position of an unburied colony-mate corpse (entity with
    /// `Dead` and without `Buried`). Distinct from `SocialTarget`
    /// because the canonical `cat_positions` snapshot is built
    /// `Without<Dead>` for the social-family DSEs; reusing
    /// `SocialTarget` for burial would silently drop dead-cat
    /// candidates from the zone resolver. The dead-cat positions
    /// snapshot lives in `goap.rs::ScoringSnapshots::dead_cat_positions`
    /// and is consumed by `resolve_zone_position`'s `CorpseTarget`
    /// arm.
    CorpseTarget,
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

impl Carrying {
    /// Project a multi-slot `Inventory` into the planner's coarse
    /// `Carrying` state — "the most important thing the cat is
    /// holding right now."
    ///
    /// Priority cascade: `BuildMaterials > Remedy > Prey >
    /// ForagedFood > Herbs > Nothing`. Note the projection only
    /// ever produces these six variants; `RawFood` / `CookedFood`
    /// are search-state-only — set during A* expansion by chain
    /// effects (`SetCarrying(...)`), never produced from a runtime
    /// inventory snapshot. (Cooked food in inventory projects to
    /// `Prey` if its `kind` is a raw-prey variant with
    /// `modifiers.cooked = true`, else `ForagedFood` — preserved
    /// verbatim from the pre-175 `build_planner_state` behavior.)
    ///
    /// Ticket 365 (016 Phase 1a) added the `Remedy` arm: prepared
    /// remedies live as real `ItemKind::Remedy*` slots in inventory
    /// rather than as a search-state-only virtual carry. Remedy
    /// outranks food because cats craft remedies to apply them — a
    /// cat holding a remedy and prey wants to apply the remedy
    /// next, not eat. Build materials still outrank everything
    /// (delivery contracts).
    ///
    /// Used by both `build_planner_state` (planner-side, ticket
    /// 175) and `scoring::carry_affinity_bonus` (L2 carry-
    /// affinity bias, ticket 175). Keep the two callers in
    /// lockstep — they MUST agree on which carry the cat
    /// projects to, so "the planner sees `Carrying::X`" and "the
    /// scorer biased toward X-consuming DSEs" are always
    /// consistent.
    pub fn from_inventory(inventory: &crate::components::magic::Inventory) -> Self {
        use crate::components::items::ItemKind;

        if inventory
            .slots
            .iter()
            .any(|s| s.kind.material().is_some())
        {
            Carrying::BuildMaterials
        } else if inventory.slots.iter().any(|s| s.kind.is_remedy()) {
            Carrying::Remedy
        } else if inventory.slots.iter().any(|s| s.kind.is_food()) {
            if inventory.slots.iter().any(|s| {
                matches!(
                    s.kind,
                    ItemKind::RawMouse
                        | ItemKind::RawRat
                        | ItemKind::RawBird
                        | ItemKind::RawFish
                        | ItemKind::RawRabbit
                )
            }) {
                Carrying::Prey
            } else {
                Carrying::ForagedFood
            }
        } else if inventory.slots.iter().any(|s| s.kind.is_herb()) {
            Carrying::Herbs
        } else {
            Carrying::Nothing
        }
    }
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
    /// Search-state only (ticket 096): `true` iff a `DeliverMaterials`
    /// step has been simulated earlier in *this* A* expansion. Lets the
    /// planner reason "after I deliver, the site is fundable, so the
    /// next `Construct` step is applicable" inside one search without
    /// re-reading ECS. The world-fact half of the old hybrid
    /// (`materials_available`) lives in the substrate as the
    /// `MaterialsAvailable` marker, consulted via
    /// `StatePredicate::HasMarker`. `Construct` accepts either branch
    /// (two action defs in `building_actions`).
    pub materials_delivered_this_plan: bool,
    /// Search-state only (ticket 230): `true` iff a `PickFleeTarget`
    /// step has been simulated earlier in *this* A* expansion.
    /// `Flee` (umbrella travel) and `HoldUntilSafe` are gated on this
    /// predicate so the chain stays ordered (pick → travel → hold).
    /// Cleared by `HoldUntilSafe`'s `IncrementTrips` so the cat can
    /// re-pick on a subsequent re-plan if a fresh threat appears.
    pub flee_target_picked: bool,
    /// Search-state only (ticket 231): `true` iff a `DropItem` step
    /// has been simulated earlier in *this* A* expansion. Pair with
    /// `HasMarker(HasFreeSlot::KEY)` as alternate preconditions on the
    /// four pickup-class actions — substrate branch covers cats with a
    /// free slot; this branch covers in-plan clutter clearance via
    /// DropItem-as-prefix. Mirrors `materials_delivered_this_plan`'s
    /// pattern (ticket 096).
    pub has_free_slot_this_plan: bool,
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
    // 084: Herb-stash deposit + retrieve. DepositHerbs transfers
    // every herb-kind slot from a cat's `Inventory` into the nearest
    // Stores' `StoredHerbs`. RetrieveHerbs(kind) is the inverse —
    // pulls one count of `kind` from `StoredHerbs` back into
    // `Inventory.slots`. Mirrors `DepositFood`/`RetrieveFoodForKitten`
    // shape but operates on the per-kind count surface rather than
    // the Entity-backed `StoredItems` surface.
    DepositHerbs,
    RetrieveHerbs(crate::components::magic::HerbKind),
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
    // 176: inventory-disposal actions
    /// Drop a single carried item at the cat's current position. The
    /// resolver spawns an `Item` entity with `ItemLocation::OnGround`
    /// and removes one slot from inventory. No travel.
    DropItem,
    /// Carry a single item to the nearest Midden building and add it
    /// to the Midden's `StoredItems`. Midden capacity is unlimited so
    /// the resolver cannot fail on capacity grounds.
    TrashItemAtMidden,
    /// Transfer a single carried item from this cat's inventory to a
    /// target cat's inventory. Resolver fails if the target's
    /// inventory is full or the target moved out of range.
    HandoffItem,
    /// Add a desired ground item to inventory. Inverse of `DropItem`.
    /// The cat must be at the item's position; resolver fails if the
    /// item is gone (despawned, picked up by another cat) or the cat
    /// arrived too late.
    PickUpItemFromGround,
    // 230: Fleeing chain.
    /// Pick the lowest-cost passable tile within Chebyshev
    /// `flee_distance` of the cat using the per-replan `RouteCostField`,
    /// and write it into the GOAP step's `target_position` so the
    /// downstream `Flee` umbrella can `cat_path_plan!` to it.
    PickFleeTarget,
    /// Umbrella `Action::Flee` — gradient-walk to the picked flee target
    /// using the same `cat_path_plan!` machinery `TravelTo` uses
    /// (`field` resolution when reachable; `A*` fallback otherwise).
    /// Completion: cat's position equals the picked target tile.
    Flee,
    /// Wait `flee_hold_ticks` while the cat sits on a tile whose
    /// `RouteCostField` cost is at most `route_cost_safe_threshold` and
    /// `threat_proximity_derivative` is non-positive (threat moving away
    /// or stationary). On counter exhaustion, increment trips. If the
    /// cat is unsafe when the counter expires, `Fail` so the planner
    /// re-picks against the now-refreshed `RouteCostField`.
    HoldUntilSafe,
    /// 035: bury an unburied colony-mate corpse. Pattern B
    /// single-action plan template `[Bury]` with
    /// `ZoneIs(CorpseTarget)` precondition + `SetInteractionDone(true)`
    /// effect. Mirrors `MentorCat` / `GroomOther` shape.
    Bury,
    /// 357: advance a dependent kitten's `KittenDependency.maturity`
    /// past the weaning floor. Sub-goal 0 of the `rear_kitten` HTN
    /// method. Pattern B single-action plan template with
    /// `ZoneIs(SocialTarget)` precondition (kittens are alive cats,
    /// present in the canonical `cat_positions` snapshot) +
    /// `SetInteractionDone(true)` effect. Mirrors `MentorCat` shape.
    Wean,
    /// 357: advance a dependent kitten's `KittenDependency.maturity`
    /// past the teaching floor. Sub-goal 1 of the `rear_kitten` HTN
    /// method. Same plan-template shape as `Wean`.
    Teach,
    /// 357: terminal sub-goal of `rear_kitten` — removes
    /// `KittenDependency` from a fully-matured kitten, signalling
    /// independence. Same plan-template shape as `Wean`.
    Release,
    /// 357: scaffold for the `mourn_at_grave` HTN method's first
    /// sub-goal. Dispatch wiring lands here but the `Mourning`
    /// Component writer is §7.7.b debt (out of scope) — until that
    /// writer ships, `has_active_mourning` is always false, so this
    /// variant is structurally Live but never invoked at runtime.
    /// Pattern B shape; routes through `SocialTarget` as a stub until
    /// a `GraveTarget` zone is authored alongside the §7.7.b writer.
    Vigil,
    /// 357: scaffold for `mourn_at_grave` sub-goal 1. Same caveats as
    /// `Vigil`.
    GriefSit,
    /// 357: scaffold for `mourn_at_grave` terminal sub-goal — removes
    /// the `Mourning` Component, ending the grief arc. Same caveats
    /// as `Vigil`.
    ReleaseGrief,
}

// ---------------------------------------------------------------------------
// Predicates and effects — data-driven, not function pointers
// ---------------------------------------------------------------------------

/// A condition over `PlannerState` (or marker substrate via `PlanContext`)
/// that must hold for an action to be applicable, or that defines a goal.
///
/// `HasMarker(name)` consults the IAUS `MarkerSnapshot` directly via
/// `PlanContext` — same lookup the `EligibilityFilter` uses for DSE gating,
/// so L2 and L3 cannot disagree on whether a marker-authored fact holds
/// (ticket 092). Use the `KEY` const on the marker type
/// (`markers::HasStoredFood::KEY`) rather than a literal string.
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
    TripsAtLeast(u32),
    /// Search-state predicate (ticket 096): true iff a
    /// `DeliverMaterials` step has been simulated earlier in this A*
    /// expansion. Pair with `HasMarker(MaterialsAvailable::KEY)` as
    /// alternate `Construct` preconditions — the substrate branch
    /// covers prefunded sites, this branch covers in-plan delivery.
    MaterialsDeliveredThisPlan(bool),
    HasMarker(&'static str),
    /// Ticket 230: search-state predicate. True iff the planner has
    /// simulated a `PickFleeTarget` step earlier in this A* expansion.
    /// Used by `Flee` and `HoldUntilSafe` to enforce chain ordering.
    FleeTargetPicked(bool),
    /// Ticket 231: search-state predicate. True iff the planner has
    /// simulated a `DropItem` step earlier in this A* expansion.
    /// Pair with `HasMarker(HasFreeSlot::KEY)` as alternate
    /// preconditions on the four pickup-class actions — substrate
    /// branch covers cats with a free slot; this branch covers in-plan
    /// clutter clearance via DropItem-as-prefix.
    HasFreeSlotThisPlan(bool),
}

impl StatePredicate {
    pub fn evaluate(&self, state: &PlannerState, ctx: &PlanContext<'_>) -> bool {
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
            Self::TripsAtLeast(n) => state.trips_done >= *n,
            Self::MaterialsDeliveredThisPlan(v) => state.materials_delivered_this_plan == *v,
            Self::HasMarker(name) => ctx.markers.has(name, ctx.entity),
            Self::FleeTargetPicked(v) => state.flee_target_picked == *v,
            Self::HasFreeSlotThisPlan(v) => state.has_free_slot_this_plan == *v,
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
    SetMaterialsDeliveredThisPlan(bool),
    /// Ticket 230: set the search-state `flee_target_picked` flag so
    /// the chain ordering predicate `FleeTargetPicked(true)` evaluates
    /// in subsequent A* expansions.
    SetFleeTargetPicked(bool),
    /// Ticket 231: set the search-state `has_free_slot_this_plan`
    /// flag, applied by the DropItem-as-prefix action in pickup-class
    /// plans so the substrate-vs-plan-path Construct precedent
    /// composition lets pickup actions fire after a planned DropItem
    /// step.
    SetHasFreeSlotThisPlan(bool),
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
            Self::SetMaterialsDeliveredThisPlan(v) => state.materials_delivered_this_plan = *v,
            Self::SetFleeTargetPicked(v) => state.flee_target_picked = *v,
            Self::SetHasFreeSlotThisPlan(v) => state.has_free_slot_this_plan = *v,
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
    pub fn is_applicable(&self, state: &PlannerState, ctx: &PlanContext<'_>) -> bool {
        self.preconditions.iter().all(|p| p.evaluate(state, ctx))
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
    pub fn is_satisfied(&self, state: &PlannerState, ctx: &PlanContext<'_>) -> bool {
        self.predicates.iter().all(|p| p.evaluate(state, ctx))
    }

    /// Admissible heuristic: count of unsatisfied goal predicates.
    pub fn heuristic(&self, state: &PlannerState, ctx: &PlanContext<'_>) -> u32 {
        self.predicates
            .iter()
            .filter(|p| !p.evaluate(state, ctx))
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
///
/// Stored as a `BTreeMap` (not `HashMap`) so `travel_actions` iterates in a
/// stable, process-independent order. The resulting action list seeds A*'s
/// open-list insertion order, which is the equal-f-cost tiebreak — so a
/// `HashMap` here let same-seed runs of the same binary pick different
/// goal-equivalent plans (e.g. `TravelTo(Kitchen)` vs `TravelTo(ForagingGround)`
/// for prey-deposit on Mallow at tick 1,203,876).
#[derive(Debug, Clone, Default)]
pub struct ZoneDistances {
    pub distances: BTreeMap<(PlannerZone, PlannerZone), u32>,
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
pub struct SearchNode {
    state: PlannerState,
    g_cost: u32,
    parent: Option<usize>,
    action: Option<GoapActionKind>,
    action_cost: u32,
    depth: usize,
}

/// Categorical reason `make_plan` failed to produce a plan. Threaded out
/// to the `EventKind::PlanningFailed` event so the headless-footer
/// aggregator (`planning_failures_by_reason`) can attribute the post-155
/// residual plan-failure surface to a specific cause instead of the
/// pre-172 opaque `"no_plan_found"` blob.
///
/// Distinct from `crate::components::PlanFailureReason` (072), which
/// classifies step-level failures during plan *execution*; this enum
/// classifies plan *creation* failures inside the A* search. The two
/// surfaces are conceptually different and intentionally typed
/// separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize)]
pub enum PlanningFailureReason {
    /// No action in `actions` was applicable from `start`. The search
    /// would have drained the open set on the first expansion. Cheaply
    /// detected by a precheck so this case is distinguishable from
    /// `GoalUnreachable` (where actions exist but their effects don't
    /// reach the goal) — that distinction is the load-bearing one for
    /// ticket 172's triage of Cooking + Herbalism plan failures.
    NoApplicableActions,
    /// A* explored every reachable state and none satisfied the goal.
    /// Means the action effects available from `start` cannot in
    /// principle reach the goal — typically a substrate problem (no
    /// herbs nearby, no remedy patient in range, etc.) rather than a
    /// search-budget problem.
    GoalUnreachable,
    /// A* hit the `max_nodes` budget before finding a plan. Means the
    /// state space is searchable in principle but the budget was too
    /// tight; bumping `max_nodes` (currently 1000 at the three cat
    /// call sites in `goap.rs`) would let it succeed.
    NodeBudgetExhausted,
}

impl PlanningFailureReason {
    /// Stable string key for footer aggregation
    /// (`planning_failures_by_reason`). Mirrors the variant name so
    /// the events.jsonl `reason` field and the footer key share a
    /// vocabulary.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::NoApplicableActions => "NoApplicableActions",
            Self::GoalUnreachable => "GoalUnreachable",
            Self::NodeBudgetExhausted => "NodeBudgetExhausted",
        }
    }
}

/// Run A* search over `PlannerState` to find a plan that satisfies `goal`.
///
/// Returns `Err(PlanningFailureReason)` if no plan is found within the
/// search bounds; the typed reason flows out to
/// `EventKind::PlanningFailed` so the headless footer can attribute
/// failures by cause (172).
/// Ticket 427 Step 3 — A* search scratch arena for the concrete cat
/// planner. Mirrors `core::PlannerScratch<D>` but holds the cat's
/// non-generic `SearchNode` / `PlannerState` types. Threaded as
/// `Local<CatPlannerScratch>` from `evaluate_and_plan`.
pub struct CatPlannerScratch {
    pub arena: Vec<SearchNode>,
    pub open: BinaryHeap<Reverse<(u32, usize)>>,
    pub best_g: HashMap<PlannerState, u32>,
}

impl Default for CatPlannerScratch {
    fn default() -> Self {
        Self {
            arena: Vec::with_capacity(256),
            open: BinaryHeap::new(),
            best_g: HashMap::new(),
        }
    }
}

impl CatPlannerScratch {
    fn reset(&mut self) {
        self.arena.clear();
        self.open.clear();
        self.best_g.clear();
    }
}

pub fn make_plan(
    start: PlannerState,
    actions: &[GoapActionDef],
    goal: &GoalState,
    max_depth: usize,
    max_nodes: usize,
    ctx: &PlanContext<'_>,
    scratch: &mut CatPlannerScratch,
) -> Result<Vec<PlannedStep>, PlanningFailureReason> {
    // Early exit: already at goal.
    if goal.is_satisfied(&start, ctx) {
        return Ok(Vec::new());
    }

    // 172 precheck: if no action is applicable from start, the search
    // would expand the start node, find no successors, and drain the
    // open set on the next pop. Short-circuit with a typed reason so
    // triage can attribute "stuck at start" (substrate gating issue)
    // separately from "explored but no path" (action-effects issue).
    if !actions.iter().any(|a| a.is_applicable(&start, ctx)) {
        return Err(PlanningFailureReason::NoApplicableActions);
    }

    scratch.reset();
    let CatPlannerScratch {
        arena,
        open,
        best_g,
    } = scratch;

    // Seed with start state.
    let h = goal.heuristic(&start, ctx);
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
            return Err(PlanningFailureReason::NodeBudgetExhausted);
        }

        let g = arena[node_idx].g_cost;
        let depth = arena[node_idx].depth;

        // Skip if we've already found a cheaper path to this state.
        if g > *best_g.get(&arena[node_idx].state).unwrap_or(&u32::MAX) {
            continue;
        }

        // Goal check at dequeue — this node has the lowest f-cost among
        // unvisited nodes, so if it satisfies the goal it's optimal.
        if goal.is_satisfied(&arena[node_idx].state, ctx) {
            return Ok(reconstruct_path(arena, node_idx));
        }

        if depth >= max_depth {
            continue;
        }

        for action in actions {
            if !action.is_applicable(&arena[node_idx].state, ctx) {
                continue;
            }

            let next_state = action.apply(&arena[node_idx].state);
            let tentative_g = g.saturating_add(action.cost);

            // Skip if we've already found a cheaper or equal path to this state.
            if tentative_g >= *best_g.get(&next_state).unwrap_or(&u32::MAX) {
                continue;
            }
            best_g.insert(next_state.clone(), tentative_g);

            let h = goal.heuristic(&next_state, ctx);
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

    Err(PlanningFailureReason::GoalUnreachable)
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

// Note: cat planner uses the concrete `make_plan` above (with `PlanContext`)
// rather than the generic `core::make_plan`. Non-cat species (fox / hawk /
// snake) implement `core::GoapDomain` for their state types — auditing
// those for the same parallel-feasibility-language smell is tracked as a
// 092 follow-up.

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
            materials_delivered_this_plan: false,
            flee_target_picked: false,
            has_free_slot_this_plan: false,
        }
    }

    /// Empty `MarkerSnapshot` + a synthetic test entity. Tests that need
    /// to gate on `HasMarker(...)` build their own snapshot and pass a
    /// custom `PlanContext` instead.
    fn test_ctx() -> (MarkerSnapshot, Entity) {
        (
            MarkerSnapshot::new(),
            Entity::from_raw_u32(1).expect("nonzero raw entity id"),
        )
    }

    /// Run `make_plan` with an empty marker snapshot. Use when the test
    /// doesn't depend on any marker-gated predicates.
    macro_rules! plan {
        ($start:expr, $actions:expr, $goal:expr, $depth:expr, $nodes:expr) => {{
            let (markers, entity) = test_ctx();
            let ctx = PlanContext {
                markers: &markers,
                entity,
            };
            let mut scratch = CatPlannerScratch::default();
            make_plan($start, $actions, $goal, $depth, $nodes, &ctx, &mut scratch)
        }};
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

        let plan = plan!(start, &actions, &goal, 12, 1000).expect("should find plan");

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

        let plan = plan!(start, &actions, &goal, 12, 1000).expect("should find plan");

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

        let plan = plan!(start, &actions, &goal, 12, 1000).expect("should find plan");

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

        let plan = plan!(start, &actions, &goal, 12, 1000).expect("should find plan");

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
        // SelfGroom IS applicable (no preconditions), so the precheck
        // passes; the search drains the open set and returns
        // GoalUnreachable.
        let actions = vec![GoapActionDef {
            kind: GoapActionKind::SelfGroom,
            cost: 1,
            preconditions: vec![],
            effects: vec![StateEffect::SetTemperatureOk(true)],
        }];

        let plan = plan!(start, &actions, &goal, 12, 1000);
        assert_eq!(
            plan.expect_err("plan should fail"),
            PlanningFailureReason::GoalUnreachable
        );
    }

    #[test]
    fn no_applicable_actions_at_start_returns_specific_reason() {
        // 172: distinguish "no action applicable from start" from the
        // generic GoalUnreachable. Construct a state where every
        // action's preconditions fail at start, then assert the
        // typed reason surfaces.
        let start = default_state(); // zone = Wilds
        let goal = GoalState {
            predicates: vec![StatePredicate::ZoneIs(PlannerZone::Stores)],
        };
        // The only action requires being IN Stores already — impossible
        // to apply from Wilds.
        let actions = vec![GoapActionDef {
            kind: GoapActionKind::DepositPrey,
            cost: 1,
            preconditions: vec![StatePredicate::ZoneIs(PlannerZone::Stores)],
            effects: vec![StateEffect::IncrementTrips],
        }];
        let plan = plan!(start, &actions, &goal, 12, 1000);
        assert_eq!(
            plan.expect_err("plan should fail"),
            PlanningFailureReason::NoApplicableActions
        );
    }

    #[test]
    fn empty_plan_when_goal_already_satisfied() {
        let start = PlannerState {
            zone: PlannerZone::Stores,
            trips_done: 3,
            ..default_state()
        };
        let goal = GoalState {
            predicates: vec![StatePredicate::TripsAtLeast(3)],
        };

        let (markers, entity) = test_ctx();
        let ctx = PlanContext {
            markers: &markers,
            entity,
        };
        let mut scratch = CatPlannerScratch::default();
        let plan = make_plan(start, &[], &goal, 12, 1000, &ctx, &mut scratch).expect("should find plan");
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
        let plan = plan!(start, &actions, &goal, 200, 10);
        assert_eq!(
            plan.expect_err("plan should fail"),
            PlanningFailureReason::NodeBudgetExhausted
        );
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

        // Max depth = 3, but reaching trips=5 requires 5 steps. Depth
        // pruning leaves the open set drainable without satisfying the
        // goal, so this surfaces as GoalUnreachable rather than
        // NodeBudgetExhausted (depth caps don't trip the node budget).
        let plan = plan!(start, &actions, &goal, 3, 1000);
        assert_eq!(
            plan.expect_err("plan should fail"),
            PlanningFailureReason::GoalUnreachable
        );
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

        let plan = plan!(start, &actions, &goal, 12, 1000).expect("should find plan");
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

        let plan = plan!(start, &actions, &goal, 12, 1000).expect("should find plan");
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

        let plan = plan!(start, &actions, &goal, 12, 1000).expect("should find plan");
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
