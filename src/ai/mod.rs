pub mod aspirations;
pub mod capabilities;
pub mod caretake_targeting;
pub mod commitment;
pub mod composition;
pub mod considerations;
pub mod curves;
pub mod dse;
pub mod dses;
pub mod eval;
pub mod faction;
pub mod fox_planner;
pub mod fox_scoring;
pub mod hawk_planner;
pub mod hawk_scoring;
pub mod joint_intention;
pub mod mating;
pub mod methods;
pub mod modifier;
pub mod pathfinding;
pub mod planner;
pub mod route_cost;
pub mod scoring;
pub mod snake_planner;
pub mod snake_scoring;
pub mod target_dse;

use bevy_ecs::prelude::*;

use crate::components::physical::Position;

// ---------------------------------------------------------------------------
// Action
// ---------------------------------------------------------------------------

/// The discrete actions available to a cat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Action {
    Eat,
    Sleep,
    Hunt,
    Forage,
    Wander,
    Idle,
    Socialize,
    /// 158: split from `Groom`. Self-grooming (thermal-comfort) — rides
    /// `DispositionKind::Resting` alongside `Sleep`. The L3 softmax now
    /// picks `GroomSelf` vs `GroomOther` directly; the side-channel
    /// `self_groom_won` resolver retired.
    GroomSelf,
    /// 158: split from `Groom`. Allogrooming (bond-building) — rides
    /// the new `DispositionKind::Grooming` (single-step plan template
    /// `[GroomOther]` mirroring 154's Mentoring extraction). Replaces
    /// the equivalent-effect sibling under Socializing that A* was
    /// pre-pruning at `planner/mod.rs:437`.
    GroomOther,
    Explore,
    Flee,
    Fight,
    Patrol,
    Build,
    Farm,
    /// 155: split from `Herbcraft`. Single-action sub-mode — gather a
    /// herb at a HerbPatch zone. Rides `DispositionKind::Herbalism`.
    HerbcraftGather,
    /// 155: split from `Herbcraft`. Multi-step chain (gather + prepare +
    /// travel + apply) terminating at `ApplyRemedy`. Rides
    /// `DispositionKind::Herbalism`.
    HerbcraftRemedy,
    /// 155: split from `Herbcraft`. Multi-step chain (gather thornbriar +
    /// place ward) terminating at `SetWard`. Rides
    /// `DispositionKind::Herbalism`.
    HerbcraftSetWard,
    /// 155: split from `PracticeMagic`. Scrying for resource-found
    /// memories. Rides `DispositionKind::Witchcraft`.
    MagicScry,
    /// 155: split from `PracticeMagic`. Magic-specialist durable ward
    /// placement. Rides `DispositionKind::Witchcraft`.
    MagicDurableWard,
    /// 155: split from `PracticeMagic`. Tile-targeted corruption
    /// cleanse — fires when the cat stands on a corrupted tile. Rides
    /// `DispositionKind::Witchcraft`.
    MagicCleanse,
    /// 155: split from `PracticeMagic`. Colony-wide corruption-hotspot
    /// cleanse — directive-routed or self-driven. Rides
    /// `DispositionKind::Witchcraft`.
    MagicColonyCleanse,
    /// 155: split from `PracticeMagic`. Carcass harvest for shadowbone
    /// items. Rides `DispositionKind::Witchcraft`.
    MagicHarvest,
    /// 155: split from `PracticeMagic`. Spirit communion at a special
    /// terrain tile. Rides `DispositionKind::Witchcraft`.
    MagicCommune,
    Coordinate,
    Mentor,
    Mate,
    Caretake,
    /// Prepare raw food at a Kitchen structure, transforming it into a cooked
    /// item that restores more hunger when eaten. Fulfillment-tier.
    /// 155: rides `DispositionKind::Cooking` (split from Crafting).
    Cook,
    /// Ticket 104 — Hide/Freeze response. The third predator-avoidance
    /// valence ("remain still and hope") alongside Flee and Fight. The
    /// cat flattens against the ground at its current position, no
    /// movement, ticking a freeze counter. Anxiety-interrupt class
    /// alongside `Flee` and `Idle` — has no parent disposition.
    /// Phase 1 ships dormant: the `HideEligible` marker that gates
    /// `HideDse` is never authored until lift activation.
    Hide,
    /// 176: drop one carried item on the ground at the cat's current
    /// position. Single-step plan template `[DropItem]`. Rides
    /// `DispositionKind::Discarding`. The dropped item becomes a real
    /// `Item` entity with `ItemLocation::OnGround` — other cats can
    /// forage it later.
    Drop,
    /// 176: carry one item to the nearest Midden and deposit it there.
    /// Plan template `[TravelTo(Midden), TrashItemAtMidden]`. Rides
    /// `DispositionKind::Trashing`. Midden has unlimited capacity so
    /// the deposit cannot fail on capacity grounds.
    Trash,
    /// 176: hand one carried item to a nearby cat whose inventory has
    /// room and who could use it. Plan template `[TravelTo(target_cat),
    /// HandoffItem]`. Rides `DispositionKind::Handing`.
    Handoff,
    /// 176: walk to a desired item with `ItemLocation::OnGround` and
    /// add it to inventory. Plan template `[TravelTo(item_pos),
    /// PickUpItemFromGround]`. Rides `DispositionKind::PickingUp`.
    /// Load-bearing for the kill→carcass-on-ground→pick-up flow:
    /// `engage_prey` always spawns a real carcass entity, and the cat
    /// must elect `PickingUp` to retrieve it.
    PickUp,
    /// 035: bury a deceased colony-mate. Single-action plan template
    /// `[Bury]` with `ZoneIs(CorpseTarget)` precondition; rides
    /// `DispositionKind::Burying` at Maslow tier 3 (Belonging). On
    /// completion the corpse entity is despawned and a `Grave` entity
    /// is spawned at the same position. Witness fires
    /// `Feature::BurialPerformed` and `EventKind::BurialFired`, which
    /// tallies the `burial` continuity canary.
    Bury,
    /// 322 / 334 — dormant stub for `acquire_stealth_via_*` methods.
    /// `Action::WearItem` will be wired in #334 alongside the slot-
    /// inventory substrate and the StealthCloak recipe. Until then no
    /// Live HTN method emits it; the placeholder resolver returns
    /// `StepResult::Fail` so accidental dispatch is observable.
    WearItem,
    /// 322 / 334 — dormant stub for `acquire_stealth_via_self_craft`.
    /// `Action::Craft` will be wired in #334 alongside the StealthCloak
    /// recipe + crafting substrate. Until then no Live HTN method emits
    /// it; the placeholder resolver returns `StepResult::Fail`.
    Craft,
    /// 322 / 334 — dormant stub for `acquire_stealth_via_commission`.
    /// `Action::PetitionCoordinator` will be wired in #334 (the
    /// commission flow needs both this action and the coordinator-side
    /// fulfillment substrate). Until then no Live HTN method emits it.
    PetitionCoordinator,
    /// 322 / 332 — dormant stub for `mourn_at_grave`. `Action::Vigil`
    /// will be wired in #332 (grief-vigil action vocabulary) alongside
    /// the Grave-target picker. Until then no Live HTN method emits it.
    Vigil,
    /// 322 / 332 — dormant stub for `mourn_at_grave`. `Action::GriefSit`
    /// will be wired in #332 (grief-vigil action vocabulary). Until
    /// then no Live HTN method emits it.
    GriefSit,
    /// 332 — terminal sub-goal of the `mourn_at_grave` HTN method.
    /// Distinct from `Action::Release` (which retires a `rear_kitten`
    /// arc — different real-world effect, different witness shape).
    /// The resolver retires the cat's `Mourning` Component when the
    /// arc concludes; HTN-driven action dispatch (DSE, GoapActionKind,
    /// plan template, dispatch arm) is a follow-on per #332's landing
    /// Log.
    ReleaseGrief,
    /// 322 / 333 — dormant stub for `rear_kitten`. `Action::Wean` will
    /// be wired in #333 (kitten-rearing action vocabulary) keyed to
    /// `KittenDependency`. Until then no Live HTN method emits it.
    Wean,
    /// 322 / 333 — dormant stub for `rear_kitten`. `Action::Teach` will
    /// be wired in #333 (kitten-rearing action vocabulary). Until then
    /// no Live HTN method emits it.
    Teach,
    /// 322 / 333 — dormant stub for `rear_kitten`. `Action::Release`
    /// will be wired in #333 (kitten-rearing action vocabulary —
    /// terminal sub-goal that retires the rearing arc). Until then no
    /// Live HTN method emits it.
    Release,
}

// ---------------------------------------------------------------------------
// CurrentAction component
// ---------------------------------------------------------------------------

/// Tracks what a cat is currently doing and how long it will continue.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CurrentAction {
    pub action: Action,
    /// How many simulation ticks remain for this action.
    pub ticks_remaining: u64,
    /// Optional spatial target (e.g. food source, sleeping spot).
    pub target_position: Option<Position>,
    /// Optional entity target (e.g. cat to socialize/groom with).
    #[serde(skip)]
    pub target_entity: Option<Entity>,
    /// All gate-open action scores from the last decision, sorted descending
    /// (post-bonus, post-suppression). Used by the log_panel UI and by
    /// offline scoring-competition analysis.
    #[serde(skip)]
    pub last_scores: Vec<(Action, f32)>,
}

impl Default for CurrentAction {
    fn default() -> Self {
        Self {
            action: Action::Idle,
            ticks_remaining: 0,
            target_position: None,
            target_entity: None,
            last_scores: Vec::new(),
        }
    }
}
