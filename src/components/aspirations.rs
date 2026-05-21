use bevy_ecs::prelude::*;

use crate::ai::Action;

// ---------------------------------------------------------------------------
// Re-exports — `&'static`-shaped aspiration substrate
// ---------------------------------------------------------------------------
//
// Ticket 321 retired the RON-deserialized `Milestone` / `MilestoneCondition`
// / `AspirationChain` shapes in favor of code-defined const data in
// `crate::ai::aspirations`. The two type names that remain in heavy
// use across the codebase (`AspirationChain`, `Milestone`) are
// re-exported here so existing `use crate::components::aspirations::…`
// import paths keep compiling. `MilestoneCondition` is retired
// entirely; `ProgressTracker` is the replacement.

pub use crate::ai::aspirations::{
    AspirationChain, Emit, Milestone, Priority, ProgressTracker, SkillKind,
};

// ---------------------------------------------------------------------------
// Aspiration Domain
// ---------------------------------------------------------------------------

/// The broad domains that aspirations can belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AspirationDomain {
    Hunting,
    Combat,
    Social,
    Herbcraft,
    Exploration,
    Building,
    Leadership,
    /// Ticket 398 — §7.M.2 RaiseOffspringAspiration domain. Lifetime
    /// kitten-rearing arc; matching action is `Action::Caretake`.
    /// Distinct from Social (relational-but-not-kin) because §7.M.2's
    /// spec-named aspiration tracks a separate commitment axis
    /// (provisioning + protection of dependents) that the personality
    /// alignment picks up via `compassion`, not `warmth`.
    Kinship,
    /// 366 — Phase 5 mastery domains. Each maps 1:1 to a crafting
    /// discipline in `docs/systems/crafting.md` §Phase 5. Progress
    /// is read off the matching `Skills` axis (Skills::weaving /
    /// bone_shaping / hidework / pigment / cairn). `matching_actions`
    /// returns `&[]` here until 372 lands the discipline-specific
    /// `Action` variants; the L1 picker still works because mastery
    /// milestones use `SkillLevel` trackers, not action-count.
    Weaving,
    BoneShaping,
    Hidework,
    Pigment,
    Cairn,
}

impl AspirationDomain {
    /// Actions that fall under this domain, used for desire bonuses.
    pub fn matching_actions(self) -> &'static [Action] {
        match self {
            Self::Hunting => &[Action::Hunt, Action::Forage],
            Self::Combat => &[Action::Fight, Action::Patrol],
            Self::Social => &[Action::Socialize, Action::GroomOther],
            // 155: Herbcraft fanned into 3 sub-actions; the Herbcraft
            // aspiration counts all three as contributors.
            Self::Herbcraft => &[
                Action::HerbcraftGather,
                Action::HerbcraftRemedy,
                Action::HerbcraftSetWard,
            ],
            Self::Exploration => &[Action::Explore, Action::Wander],
            Self::Building => &[Action::Build],
            Self::Leadership => &[Action::Coordinate],
            Self::Kinship => &[Action::Caretake],
            // 366 — Phase 5 mastery domains. 372 fans in the
            // discipline-specific Action variants (Weave, Knap,
            // TanHide, MixPigment, LayCairn) and updates these arms.
            Self::Weaving | Self::BoneShaping | Self::Hidework | Self::Pigment | Self::Cairn => &[],
        }
    }
}

// ---------------------------------------------------------------------------
// Active aspiration (per-cat runtime state)
// ---------------------------------------------------------------------------

/// An aspiration a cat is actively pursuing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActiveAspiration {
    pub chain_name: String,
    pub domain: AspirationDomain,
    pub current_milestone: usize,
    /// Accumulated progress toward the current milestone's condition.
    pub progress: u32,
    /// Tick when this aspiration was adopted.
    pub adopted_tick: u64,
    /// Tick when progress last advanced (for abandonment check).
    pub last_progress_tick: u64,
}

// ---------------------------------------------------------------------------
// Aspirations component
// ---------------------------------------------------------------------------

/// Tracks a cat's active and completed aspirations.
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Aspirations {
    pub active: Vec<ActiveAspiration>,
    pub completed: Vec<String>,
}

// ---------------------------------------------------------------------------
// Preferences (likes / dislikes)
// ---------------------------------------------------------------------------

/// Whether a cat likes or dislikes an activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Preference {
    Like,
    Dislike,
}

/// A cat's personal likes and dislikes for specific actions.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Preferences {
    pub action_preferences: Vec<(Action, Preference)>,
}

impl Preferences {
    /// Look up the preference for a given action, if any.
    pub fn get(&self, action: Action) -> Option<Preference> {
        self.action_preferences
            .iter()
            .find(|(a, _)| *a == action)
            .map(|(_, p)| *p)
    }
}

// ---------------------------------------------------------------------------
// Marker component
// ---------------------------------------------------------------------------

/// Inserted after a cat's aspirations and preferences have been initialised.
/// Systems use `Without<AspirationsInitialized>` to detect cats needing setup.
#[derive(Component, Debug, Clone, Copy)]
pub struct AspirationsInitialized;
