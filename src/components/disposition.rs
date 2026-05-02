use bevy_ecs::prelude::*;

use crate::ai::Action;
use crate::components::personality::Personality;

// ---------------------------------------------------------------------------
// CraftingHint — sub-mode selected by the scorer for crafting dispositions
// ---------------------------------------------------------------------------

/// Indicates which crafting sub-mode won during scoring, so the chain builder
/// doesn't re-derive the decision and accidentally shadow PrepareRemedy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CraftingHint {
    GatherHerbs,
    PrepareRemedy,
    SetWard,
    Magic,
    /// Directed cleanse — planner should only pick CleanseCorruption.
    Cleanse,
    /// Directed carcass harvest — planner should only pick HarvestCarcass.
    HarvestCarcass,
    /// Magic-specialist ward — planner uses SetWard, resolver picks
    /// WardKind::DurableWard. Selected when a cat's durable_ward sub-score
    /// wins the PracticeMagic contest.
    DurableWard,
    /// Cook a raw food item at a Kitchen — emits a Stores → Kitchen → Stores
    /// round-trip chain.
    Cook,
}

// ---------------------------------------------------------------------------
// DispositionKind — the sustained behavioral orientations
// ---------------------------------------------------------------------------

/// A disposition is a sustained behavioral orientation. Instead of re-evaluating
/// actions every tick, a cat commits to a disposition and mechanically sequences
/// sub-actions (via TaskChain) until a goal is met or anxiety interrupts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DispositionKind {
    /// Eat, sleep, self-groom until physiological needs are satisfied.
    Resting,
    /// Hunt prey, carry to stores, deposit. Loop until target trips met.
    Hunting,
    /// Forage items, carry to stores, deposit. Loop until target trips met.
    Foraging,
    /// Patrol perimeter and fight threats until safety is restored.
    Guarding,
    /// Socialize, groom others, mentor until social needs are met.
    Socializing,
    /// Build or repair structures (existing TaskChain-driven).
    Building,
    /// Tend and harvest crops (existing TaskChain-driven).
    Farming,
    /// Herbcraft or practice magic (existing TaskChain-driven).
    Crafting,
    /// Deliver coordinator directives until queue is empty.
    Coordinating,
    /// Explore distant tiles, survey surroundings.
    Exploring,
    /// Court and mate with a partner.
    Mating,
    /// Feed and groom dependent kittens.
    Caretaking,
}

impl DispositionKind {
    /// How many sub-outcomes (deposit trips, social interactions, tiles surveyed)
    /// this disposition targets before clearing. Personality-scaled.
    pub fn target_completions(&self, personality: &Personality) -> u32 {
        let base = match self {
            Self::Hunting => 1 + (personality.diligence * 2.0).round() as u32,
            Self::Foraging => 1 + (personality.diligence * 2.0).round() as u32,
            Self::Exploring => 2 + (personality.curiosity * 3.0).round() as u32,
            Self::Guarding => 1 + (personality.boldness * 2.0).round() as u32,
            Self::Socializing => {
                1 + (personality.sociability * 2.0 + personality.playfulness * 1.0).round() as u32
            }
            Self::Caretaking => 1 + (personality.compassion * 2.0).round() as u32,
            // Single-event dispositions.
            Self::Mating => return 1,
            // Chain-driven dispositions complete when their chain finishes.
            Self::Building | Self::Farming | Self::Crafting | Self::Coordinating => 1,
            // Resting completes on need thresholds, not count.
            Self::Resting => return u32::MAX,
        };
        // Patience adds to all non-Resting/Mating dispositions: patient cats commit longer.
        base + (personality.patience * 1.0).round() as u32
    }

    /// Maps each action to the disposition that contains it.
    /// Flee has no disposition — it's an anxiety interrupt.
    pub fn from_action(action: Action) -> Option<Self> {
        match action {
            Action::Eat | Action::Sleep => Some(Self::Resting),
            Action::Hunt => Some(Self::Hunting),
            Action::Forage => Some(Self::Foraging),
            Action::Patrol | Action::Fight => Some(Self::Guarding),
            Action::Socialize | Action::Mentor => Some(Self::Socializing),
            Action::Groom => None, // Depends on self vs other — caller decides
            Action::Build => Some(Self::Building),
            Action::Farm => Some(Self::Farming),
            Action::Herbcraft | Action::PracticeMagic | Action::Cook => Some(Self::Crafting),
            Action::Coordinate => Some(Self::Coordinating),
            Action::Explore | Action::Wander => Some(Self::Exploring),
            Action::Mate => Some(Self::Mating),
            Action::Caretake => Some(Self::Caretaking),
            // Anxiety-interrupt class — actions without a parent
            // disposition. `Flee` (047) drives retreat; `Hide` (104) is
            // the "remain still and hope" sibling valence; `Idle` is
            // the no-op fallback.
            Action::Idle | Action::Flee | Action::Hide => None,
        }
    }

    /// The actions that contribute to this disposition's score.
    pub fn constituent_actions(&self) -> &[Action] {
        match self {
            Self::Resting => &[Action::Eat, Action::Sleep, Action::Groom],
            Self::Hunting => &[Action::Hunt],
            Self::Foraging => &[Action::Forage],
            Self::Guarding => &[Action::Patrol, Action::Fight],
            Self::Socializing => &[Action::Socialize, Action::Groom, Action::Mentor],
            Self::Building => &[Action::Build],
            Self::Farming => &[Action::Farm],
            Self::Crafting => &[Action::Herbcraft, Action::PracticeMagic, Action::Cook],
            Self::Coordinating => &[Action::Coordinate],
            Self::Exploring => &[Action::Explore, Action::Wander],
            Self::Mating => &[Action::Mate],
            Self::Caretaking => &[Action::Caretake],
        }
    }

    /// All disposition variants, for iteration.
    pub const ALL: &[Self] = &[
        Self::Resting,
        Self::Hunting,
        Self::Foraging,
        Self::Guarding,
        Self::Socializing,
        Self::Building,
        Self::Farming,
        Self::Crafting,
        Self::Coordinating,
        Self::Exploring,
        Self::Mating,
        Self::Caretaking,
    ];

    /// Human-readable label for the inspect panel.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Resting => "Resting",
            Self::Hunting => "Hunting",
            Self::Foraging => "Foraging",
            Self::Guarding => "Guarding",
            Self::Socializing => "Socializing",
            Self::Building => "Building",
            Self::Farming => "Farming",
            Self::Crafting => "Crafting",
            Self::Coordinating => "Coordinating",
            Self::Exploring => "Exploring",
            Self::Mating => "Mating",
            Self::Caretaking => "Caretaking",
        }
    }

    /// Maslow hierarchy level. Lower = more fundamental = higher priority.
    /// An urgency can only preempt a plan whose maslow_level is numerically
    /// higher (less fundamental).
    pub fn maslow_level(&self) -> u8 {
        match self {
            Self::Resting | Self::Hunting | Self::Foraging => 1,
            Self::Guarding => 2,
            Self::Socializing | Self::Caretaking | Self::Mating => 3,
            Self::Crafting | Self::Coordinating | Self::Building | Self::Farming => 4,
            Self::Exploring => 5,
        }
    }

    /// Infinitive verb form for use after "sets out to".
    pub fn verb_infinitive(&self) -> &'static str {
        match self {
            Self::Resting => "rest",
            Self::Hunting => "hunt",
            Self::Foraging => "forage",
            Self::Guarding => "guard",
            Self::Socializing => "socialize",
            Self::Building => "build",
            Self::Farming => "farm",
            Self::Crafting => "craft",
            Self::Coordinating => "coordinate",
            Self::Exploring => "explore",
            Self::Mating => "find a mate",
            Self::Caretaking => "tend the young",
        }
    }
}

// ---------------------------------------------------------------------------
// Disposition component
// ---------------------------------------------------------------------------

/// Tracks a cat's current sustained behavioral orientation.
///
/// When present, the cat's behavior is driven by this disposition rather than
/// per-tick action re-evaluation. Sub-actions are sequenced via TaskChain.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Disposition {
    pub kind: DispositionKind,
    /// Tick when this disposition was adopted.
    pub adopted_tick: u64,
    /// Tick when this disposition was last switched into.
    /// Written by `plan_substrate::record_disposition_switch` (ticket
    /// 072). Initialized to 0 by `Disposition::new`; the existing
    /// switch site in `evaluate_dispositions` calls
    /// `record_disposition_switch` to set the current tick. Consumed
    /// by ticket 075 (`CommitmentTenure` Modifier) — 072 only writes.
    #[serde(default)]
    pub disposition_started_tick: u64,
    /// Completed sub-outcomes (e.g., deposit trips for Hunting).
    pub completions: u32,
    /// Disposition clears when completions >= target.
    pub target_completions: u32,
    /// For Crafting dispositions: which sub-mode the scorer selected.
    /// Threaded from scoring to chain builder so the cascade doesn't re-derive.
    pub crafting_hint: Option<CraftingHint>,
}

impl Disposition {
    pub fn new(kind: DispositionKind, tick: u64, personality: &Personality) -> Self {
        Self {
            kind,
            adopted_tick: tick,
            disposition_started_tick: 0,
            completions: 0,
            target_completions: kind.target_completions(personality),
            crafting_hint: None,
        }
    }

    /// Whether the count-based completion condition is met.
    /// Resting uses need thresholds instead — checked elsewhere.
    pub fn is_count_complete(&self) -> bool {
        self.completions >= self.target_completions
    }
}

// ---------------------------------------------------------------------------
// ActionHistory — per-cat log for the inspect panel
// ---------------------------------------------------------------------------

/// Outcome of a completed action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ActionOutcome {
    Success,
    Failure,
    Interrupted,
}

/// A single entry in a cat's action history.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ActionRecord {
    pub action: Action,
    pub disposition: Option<DispositionKind>,
    pub tick: u64,
    pub outcome: ActionOutcome,
}

/// Per-cat action history for the inspect panel. Capped at 20 entries.
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ActionHistory {
    pub entries: Vec<ActionRecord>,
    /// Tracks the last disposition narrated for this cat to suppress repeated
    /// "heads out to hunt" messages when the same disposition is chosen again.
    #[serde(skip, default)]
    pub last_narrated_disposition: Option<DispositionKind>,
    /// Tick of the last narrated Completed event per disposition. Used to
    /// throttle "feels rested" and similar repeated completion messages.
    #[serde(skip, default)]
    pub last_completed_tick: Option<(DispositionKind, u64)>,
    /// Number of Replanned events narrated for the current plan lifecycle.
    /// Reset when a new plan is Adopted.
    #[serde(skip, default)]
    pub replans_narrated: u32,
}

impl ActionHistory {
    const MAX_ENTRIES: usize = 20;

    pub fn record(&mut self, record: ActionRecord) {
        self.entries.push(record);
        if self.entries.len() > Self::MAX_ENTRIES {
            self.entries.remove(0);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_personality() -> Personality {
        Personality {
            boldness: 0.5,
            sociability: 0.5,
            curiosity: 0.5,
            diligence: 0.5,
            warmth: 0.5,
            spirituality: 0.5,
            ambition: 0.5,
            patience: 0.5,
            anxiety: 0.5,
            optimism: 0.5,
            temper: 0.5,
            stubbornness: 0.5,
            playfulness: 0.5,
            loyalty: 0.5,
            tradition: 0.5,
            compassion: 0.5,
            pride: 0.5,
            independence: 0.5,
        }
    }

    #[test]
    fn target_completions_scales_with_personality() {
        // patience=0.5 adds (0.5*1.0).round()=1 to all non-Resting dispositions.
        let lazy = Personality {
            diligence: 0.0,
            ..test_personality()
        };
        let diligent = Personality {
            diligence: 1.0,
            ..test_personality()
        };
        assert_eq!(DispositionKind::Hunting.target_completions(&lazy), 2); // 1 + patience(1)
        assert_eq!(DispositionKind::Hunting.target_completions(&diligent), 4); // 3 + patience(1)
    }

    #[test]
    fn patience_increases_target_completions() {
        let impatient = Personality {
            patience: 0.0,
            ..test_personality()
        };
        let patient = Personality {
            patience: 1.0,
            ..test_personality()
        };
        let impatient_target = DispositionKind::Hunting.target_completions(&impatient);
        let patient_target = DispositionKind::Hunting.target_completions(&patient);
        assert!(
            patient_target > impatient_target,
            "patient cat should have more target completions; patient={patient_target}, impatient={impatient_target}"
        );
    }

    #[test]
    fn playfulness_increases_socializing_target() {
        let boring = Personality {
            playfulness: 0.0,
            ..test_personality()
        };
        let playful = Personality {
            playfulness: 1.0,
            ..test_personality()
        };
        let boring_target = DispositionKind::Socializing.target_completions(&boring);
        let playful_target = DispositionKind::Socializing.target_completions(&playful);
        assert!(
            playful_target > boring_target,
            "playful cat should socialize longer; playful={playful_target}, boring={boring_target}"
        );
    }

    #[test]
    fn resting_target_completions_is_max() {
        let p = test_personality();
        assert_eq!(DispositionKind::Resting.target_completions(&p), u32::MAX,);
    }

    #[test]
    fn action_history_caps_at_20() {
        let mut history = ActionHistory::default();
        for i in 0..25 {
            history.record(ActionRecord {
                action: Action::Idle,
                disposition: None,
                tick: i,
                outcome: ActionOutcome::Success,
            });
        }
        assert_eq!(history.entries.len(), 20);
        assert_eq!(history.entries[0].tick, 5); // oldest 5 dropped
    }

    #[test]
    fn from_action_mapping() {
        assert_eq!(
            DispositionKind::from_action(Action::Hunt),
            Some(DispositionKind::Hunting)
        );
        assert_eq!(DispositionKind::from_action(Action::Flee), None);
        assert_eq!(DispositionKind::from_action(Action::Idle), None);
        assert_eq!(
            DispositionKind::from_action(Action::Build),
            Some(DispositionKind::Building)
        );
    }

    #[test]
    fn disposition_count_completion() {
        let p = Personality {
            diligence: 0.5,
            ..test_personality()
        };
        let mut d = Disposition::new(DispositionKind::Hunting, 0, &p);
        assert!(!d.is_count_complete());
        d.completions = d.target_completions;
        assert!(d.is_count_complete());
    }
}
