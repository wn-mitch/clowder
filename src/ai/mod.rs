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
pub mod mating;
pub mod modifier;
pub mod pairing;
pub mod pathfinding;
pub mod planner;
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
