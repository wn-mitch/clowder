use bevy_ecs::prelude::*;

use crate::ai::Action;
use crate::components::building::StructureType;
use crate::components::physical::Position;

// ---------------------------------------------------------------------------
// Coordinator marker
// ---------------------------------------------------------------------------

/// Marker component for cats who have emerged as colony coordinators through
/// social weight, diligence, and sociability. Evaluated every ~100 ticks.
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Coordinator;

// ---------------------------------------------------------------------------
// Directives
// ---------------------------------------------------------------------------

/// What kind of colony need a directive addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DirectiveKind {
    Hunt,
    Forage,
    Build,
    Fight,
    Patrol,
    Herbcraft,
    /// Prepare raw food at a Kitchen — issued when a functional Kitchen exists
    /// and there are raw food items in Stores.
    Cook,
    SetWard,
    /// Cleanse a specific corrupted tile — dispatched when corruption breaches
    /// a threshold within sensing range of the colony.
    Cleanse,
    /// Harvest or cleanse a spawned carcass before it corrupts surrounding tiles.
    HarvestCarcass,
}

impl DirectiveKind {
    /// Map a directive kind to the corresponding cat action.
    pub fn to_action(self) -> Action {
        match self {
            DirectiveKind::Hunt => Action::Hunt,
            DirectiveKind::Forage => Action::Forage,
            DirectiveKind::Build => Action::Build,
            DirectiveKind::Fight => Action::Fight,
            DirectiveKind::Patrol => Action::Patrol,
            // 155: directive routing now lands on per-sub-mode Actions
            // directly (no CraftingHint indirection). The Disposition
            // is derived via `from_action` (Herbalism / Witchcraft /
            // Cooking) so the planner sees the correct chain shape.
            DirectiveKind::Herbcraft => Action::HerbcraftGather,
            DirectiveKind::Cook => Action::Cook,
            // Ward-setting under a coordinator directive uses the
            // herbcraft chain (gather thornbriar then place ward).
            DirectiveKind::SetWard => Action::HerbcraftSetWard,
            // Cleanse routes to the colony-cleanse sub-action when
            // dispatched by a coordinator (the directive carries a
            // hotspot position; tile-self-cleanse comes from the
            // self-driven `MagicCleanse` DSE).
            DirectiveKind::Cleanse => Action::MagicColonyCleanse,
            DirectiveKind::HarvestCarcass => Action::MagicHarvest,
        }
    }
}

/// A single directive produced by colony assessment.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Directive {
    pub kind: DirectiveKind,
    /// Priority in [0.0, 1.0] — higher means more urgent.
    pub priority: f32,
    /// Suggested target entity (e.g. the damaged building, the injured cat).
    #[serde(skip)]
    pub target_entity: Option<Entity>,
    /// Suggested target position.
    pub target_position: Option<Position>,
    /// Blueprint for new construction (None = repair existing building).
    pub blueprint: Option<StructureType>,
    /// 382: consecutive ticks `compute_building_placement` has returned
    /// `None` for this directive. Reset on successful placement or on
    /// each emission of `Feature::DirectiveStuckOnPlacement`. Only
    /// meaningful for `Build` directives with a blueprint; ignored for
    /// every other kind. `#[serde(default)]` so pre-382 saves
    /// deserialize cleanly.
    #[serde(default)]
    pub placement_failure_count: u32,
}

/// Queue of pending directives on a coordinator entity.
/// Rebuilt every ~20 ticks by `assess_colony_needs`, consumed one at a time
/// as the coordinator walks to cats and delivers them.
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DirectiveQueue {
    pub directives: Vec<Directive>,
}

/// 487 — Colony-self directive queue. Populated by `assess_colony_needs`
/// when no `Coordinator`-tagged cat exists yet (the day-1 founder phase),
/// so directives like Forage / Build / Cook can fire from the colony
/// itself before an emergent coordinator is elected. Drained by
/// `dispatch_urgent_directives` alongside per-coordinator queues. Cleared
/// at the start of every assess cycle so the queue reflects the latest
/// colony state — the colony-self path is fundamentally ephemeral
/// (no in-flight memory across cycles, unlike a coordinator's queue
/// which threads through `Coordinate` deliveries).
#[derive(Resource, Debug, Clone, Default)]
pub struct ColonySelfDirectiveQueue {
    pub directives: Vec<Directive>,
}

// ---------------------------------------------------------------------------
// Directive delivery
// ---------------------------------------------------------------------------

/// Component placed on a target cat when a directive is delivered.
/// Provides a score bonus to the directed action at next evaluation.
#[derive(Component, Debug, Clone)]
pub struct ActiveDirective {
    /// The action the cat should perform.
    pub kind: DirectiveKind,
    /// Priority of the directive.
    pub priority: f32,
    /// 487 — Issuer of the directive. `Some(coordinator_entity)` for a
    /// normally-delivered directive from an elected coordinator;
    /// `None` for a colony-self directive (day-1 founder phase, no
    /// coordinator exists yet). The directive-bonus formula in
    /// `goap.rs::evaluate_and_plan` substitutes
    /// `CoordinationConstants::colony_self_directive_weight` for
    /// `coordinator_social_weight` and `disposition.fondness_default`
    /// for the fondness factor when this is `None`, so the directive
    /// still applies a (softer) pull on action scoring even without
    /// an issuer cat.
    pub coordinator: Option<Entity>,
    /// Coordinator's social weight at time of delivery. Reads as
    /// `CoordinationConstants::colony_self_directive_weight` for a
    /// colony-self directive (the field is left as the substituted
    /// value at delivery so downstream readers don't need to know
    /// about the colony-self special case).
    pub coordinator_social_weight: f32,
    /// Tick when this directive was delivered. Expires after ~200 ticks.
    pub delivered_tick: u64,
    /// Target position for spatial directives (e.g. ward placement).
    pub target_position: Option<crate::components::physical::Position>,
    /// Target entity (e.g. a shadow-fox for a posse Fight directive).
    pub target_entity: Option<Entity>,
}

/// Directive-in-transit on a coordinator walking to deliver it.
/// Inserted when `Action::Coordinate` is chosen, removed on delivery.
#[derive(Component, Debug, Clone)]
pub struct PendingDelivery(pub Directive);

// ---------------------------------------------------------------------------
// Flag resource
// ---------------------------------------------------------------------------

/// Inserted when a coordinator dies, triggering immediate re-evaluation.
#[derive(Resource, Default)]
pub struct CoordinatorDied;

// ---------------------------------------------------------------------------
// 487 — emergent-coordinator support
// ---------------------------------------------------------------------------

/// 487 — EWMA of how often this cat's `CurrentAction` has been a
/// "colony-aligned" action (Forage / Build / Cook / Hunt / Herbcraft
/// kinds — the same set the `assess_colony_needs` directive vocabulary
/// covers). Updated once per tick by `update_colony_alignment_scores`:
/// multiplicative decay by `CoordinationConstants::alignment_decay_per_tick`
/// every tick, plus an additive `alignment_match_increment` when the
/// cat's current action is colony-aligned. The fixpoint for a cat who
/// spends every tick on aligned work is exactly 1.0 at the default
/// tuning.
///
/// Read by `evaluate_coordinators` as a multiplicative term wrapped in
/// `(1 + score * alignment_skill_weight)` so the cat who *does* the
/// most colony work naturally accumulates election credit. This is the
/// emergent half of the day-1 cuddle-puddle fix (487): we don't pre-
/// elect a coordinator; the colony recognises one from observed
/// behaviour. See `update_colony_alignment_scores` + the score formula
/// in `evaluate_coordinators`.
#[derive(Component, Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct ColonyAlignmentScore {
    /// EWMA of "ticks spent on colony-aligned work" — bounded [0, 1+]
    /// in steady state at the default tuning (a slightly-above-1.0 peak
    /// is possible during transient compounding when the cat is freshly
    /// inserted with no prior decay).
    pub recent_aligned_actions: f32,
}

// ---------------------------------------------------------------------------
// Build pressure
// ---------------------------------------------------------------------------

/// Slowly-accumulating pressure channels that track unmet colony infrastructure
/// needs. Attached to coordinators. Each channel rises when its signal persists
/// and decays when it doesn't. The coordinator's attentiveness (derived from
/// personality) determines accumulation rate and action threshold.
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BuildPressure {
    /// No Stores building exists at all.
    #[serde(default)]
    pub no_store: f32,
    /// Stores at capacity for extended period.
    pub storage: f32,
    /// Cats sleeping outdoors (no Den in range).
    pub shelter: f32,
    /// Low social satisfaction despite Hearth.
    pub gathering: f32,
    /// Skilled crafters with no Workshop available.
    pub workshop: f32,
    /// Raw food stored with no Kitchen to prepare it.
    #[serde(default)]
    pub cooking: f32,
    /// Food scarcity with no Garden.
    pub farming: f32,
    /// Wildlife breaching colony perimeter.
    pub defense: f32,
    /// 367 Commit 8 — colony has raw food but no Drying Rack to
    /// preserve fish/organs. Accumulates when raw food sits in
    /// Stores and decays once a Drying Rack exists. The election
    /// pillar matches the cooking/workshop pattern: signal-driven
    /// build pressure, not directive-driven.
    #[serde(default)]
    pub drying_rack: f32,
    /// 367 Commit 8 — colony has raw meat but no Smoking Rack to
    /// preserve mammals/birds. Independent channel from
    /// `drying_rack` (smoking needs fuel + tend cycles; drying just
    /// needs sun) so the two structures can be elected
    /// independently as the food economy demands.
    #[serde(default)]
    pub smoking_rack: f32,
    /// 369 — colony has hide accumulating in Stores but no Tanning
    /// Frame to convert it into armor. Same shape as `drying_rack` /
    /// `smoking_rack` channels (signal-driven accumulation, decays
    /// when the structure exists). Tunable threshold lives at
    /// `SimConstants.crafting.build_pressure_tanning_min_hides`.
    #[serde(default)]
    pub tanning_frame: f32,
}

impl BuildPressure {
    /// Pressure accumulation base rate per evaluation cycle.
    pub const BASE_RATE: f32 = 0.01;
    /// Decay factor applied when the signal is inactive.
    pub const DECAY: f32 = 0.95;

    /// The structure type each pressure channel corresponds to.
    pub fn highest_actionable(&self, threshold: f32) -> Option<StructureType> {
        let channels = [
            (self.no_store, StructureType::Stores),
            (self.shelter, StructureType::Den),
            (self.storage, StructureType::Stores),
            (self.gathering, StructureType::Hearth),
            (self.workshop, StructureType::Workshop),
            (self.cooking, StructureType::Kitchen),
            (self.farming, StructureType::Garden),
            (self.defense, StructureType::Watchtower),
            // 367 Commit 8 — preservation infrastructure. Independent
            // channels so the food economy can lift Drying Rack first
            // (cheaper, sun-driven) and Smoking Rack second (needs
            // fuel + tend cycles), or both, based on which raw foods
            // are accumulating.
            (self.drying_rack, StructureType::DryingRack),
            (self.smoking_rack, StructureType::SmokingRack),
            // 369 — hide-tanning infrastructure. Independent channel
            // from preservation racks; lifts when hides accumulate in
            // Stores without a TanningFrame to convert them.
            (self.tanning_frame, StructureType::TanningFrame),
        ];
        channels
            .iter()
            .filter(|(pressure, _)| *pressure > threshold)
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(_, kind)| *kind)
    }
}
