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
    /// Sleep + self-groom until energy and temperature recover.
    ///
    /// 150 R5a: split — Eat is no longer a constituent of Resting.
    /// Picking `Action::Eat` now commits to `Eating`, not Resting; a
    /// hungry-but-not-tired cat reaches the stockpile without first
    /// committing to a multi-need Sleep + SelfGroom chain. See
    /// `docs/open-work/tickets/150-cat-starvation-hunt-deposit-loop-no-eat-path.md`.
    Resting,
    /// Travel to stores and consume one food item until hunger is sated.
    ///
    /// 150 R5a: new — separated from Resting so picking Eat at the L3
    /// softmax doesn't drag in Sleep and SelfGroom. Plan template is
    /// `[TravelTo(Stores), EatAtStores]`; completion gates on hunger
    /// only. Tier 1, Blind strategy, single-trip target.
    Eating,
    /// Hunt prey, carry to stores, deposit. Loop until target trips met.
    Hunting,
    /// Forage items, carry to stores, deposit. Loop until target trips met.
    Foraging,
    /// Patrol perimeter and fight threats until safety is restored.
    Guarding,
    /// Socialize and groom others until social needs are met.
    ///
    /// 154: split — Mentor is no longer a constituent of Socializing.
    /// Picking `Action::Mentor` now commits to `Mentoring`, not
    /// Socializing; the L3 softmax pick survives the
    /// disposition-collapse instead of getting crowded out by the
    /// cheaper sibling steps under a count-based completion goal.
    /// See `docs/open-work/tickets/154-socializing-mentoring-extraction.md`.
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
    /// Mentor a younger or less-skilled cat — single-interaction skill
    /// transfer.
    ///
    /// 154: new — separated from Socializing so picking Mentor at the
    /// L3 softmax doesn't get crowded out by cheaper sibling steps
    /// (SocializeWith / GroomOther) under a `TripsAtLeast(N+1)`
    /// completion goal. Plan template is `[MentorCat]` with
    /// `InteractionDone(true)` completion proxy (Pattern B, mirrors
    /// Mating). Tier 3.
    Mentoring,
    /// Allogrooming a peer — single-interaction bond-building.
    ///
    /// 158: new — separated from Socializing because the post-154
    /// `[SocializeWith (2), GroomOther (2)]` template had two
    /// equivalent-effect actions (`SetInteractionDone(true),
    /// IncrementTrips`), and A* at `planner/mod.rs:437` pre-pruned
    /// the second action via `tentative_g >= best_g` — `GroomOther`
    /// was never even pushed to the open set. Single-action plan
    /// template `[GroomOther]` with `InteractionDone(true)` completion
    /// proxy (Pattern B, mirrors Mentoring / Mating). Tier 2: above
    /// thermal self-care, below socialize-with-peers in the
    /// affiliative ladder.
    Grooming,
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
            // 154: Mentoring is a single-interaction disposition (Pattern B,
            // matches Mating). One mentor session per commitment; the
            // completion proxy is `InteractionDone(true)`, not a trip count.
            Self::Mentoring => return 1,
            // 158: Grooming is a single-interaction disposition (Pattern B,
            // matches Mentoring / Mating). One groom session per commitment;
            // the completion proxy is `InteractionDone(true)`. Single-action
            // template `[GroomOther]` makes equivalent-effect sibling
            // pre-pruning (the bug that hid `GroomedOther` post-154)
            // structurally impossible.
            Self::Grooming => return 1,
            // Chain-driven dispositions complete when their chain finishes.
            Self::Building | Self::Farming | Self::Crafting | Self::Coordinating => 1,
            // 150 R5a: Eating completes on need threshold, not count.
            // Like Resting, target_completions returns MAX so the count-
            // based completion check never fires; the actual
            // completion arm in `goap.rs::resolve_goap_plans` is need-
            // based (`needs.hunger >= resting_complete_hunger`).
            Self::Eating => return u32::MAX,
            // Resting completes on need thresholds, not count.
            Self::Resting => return u32::MAX,
        };
        // Patience adds to all non-Resting/Eating/Mating dispositions: patient cats commit longer.
        base + (personality.patience * 1.0).round() as u32
    }

    /// Maps each action to the disposition that contains it.
    /// Flee has no disposition — it's an anxiety interrupt.
    ///
    /// 150 R5a: `Action::Eat` now maps to `DispositionKind::Eating`
    /// (not `Resting`). `Action::Sleep` still maps to `Resting`.
    /// Picking Eat at the softmax pool no longer drags Sleep + SelfGroom
    /// into the same plan.
    /// 158: `Action::Groom` retired into sibling variants — `GroomSelf`
    /// rides `Resting` (alongside `Sleep`) and `GroomOther` rides the
    /// new `Grooming` disposition (single-action template, mirrors
    /// 154's Mentoring split). The side-channel `self_groom_won`
    /// resolver retires with the split.
    pub fn from_action(action: Action) -> Option<Self> {
        match action {
            Action::Eat => Some(Self::Eating),
            Action::Sleep => Some(Self::Resting),
            Action::Hunt => Some(Self::Hunting),
            Action::Forage => Some(Self::Foraging),
            Action::Patrol | Action::Fight => Some(Self::Guarding),
            // 154: Mentor splits out of Socializing — see DispositionKind
            // doc-comment for the cost-asymmetry rationale.
            Action::Socialize => Some(Self::Socializing),
            Action::Mentor => Some(Self::Mentoring),
            // 158: sibling Action variants route directly to their
            // respective dispositions; no resolver in between.
            Action::GroomSelf => Some(Self::Resting),
            Action::GroomOther => Some(Self::Grooming),
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
    ///
    /// 150 R5a: Resting drops `Action::Eat`; the new `Eating` variant
    /// owns it.
    /// 154: Socializing drops `Action::Mentor`; the new `Mentoring`
    /// variant owns it.
    /// 158: `Action::Groom` retired. `Resting` keeps the self-groom
    /// constituent via `Action::GroomSelf`; `Socializing` drops the
    /// allogrooming constituent (which moves to the new `Grooming`
    /// disposition with its single-action `[GroomOther]` template).
    pub fn constituent_actions(&self) -> &[Action] {
        match self {
            Self::Resting => &[Action::Sleep, Action::GroomSelf],
            Self::Eating => &[Action::Eat],
            Self::Hunting => &[Action::Hunt],
            Self::Foraging => &[Action::Forage],
            Self::Guarding => &[Action::Patrol, Action::Fight],
            Self::Socializing => &[Action::Socialize],
            Self::Building => &[Action::Build],
            Self::Farming => &[Action::Farm],
            Self::Crafting => &[Action::Herbcraft, Action::PracticeMagic, Action::Cook],
            Self::Coordinating => &[Action::Coordinate],
            Self::Exploring => &[Action::Explore, Action::Wander],
            Self::Mating => &[Action::Mate],
            Self::Caretaking => &[Action::Caretake],
            Self::Mentoring => &[Action::Mentor],
            Self::Grooming => &[Action::GroomOther],
        }
    }

    /// All disposition variants, for iteration.
    ///
    /// 150 R5a: `Eating` is appended at the end (rather than inserted
    /// near `Resting`) to keep the positional ordinals in
    /// `scoring::active_disposition_ordinal` and
    /// `modifier::constituent_dses_for_ordinal` stable for the
    /// pre-existing 12 variants. Saved soaks and hand-written
    /// ordinal-equality tests don't need rebasing.
    /// 154: `Mentoring` is appended at ordinal 14 for the same reason.
    /// 158: `Grooming` is appended at ordinal 15 for the same reason.
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
        Self::Eating,
        Self::Mentoring,
        Self::Grooming,
    ];

    /// Human-readable label for the inspect panel.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Resting => "Resting",
            Self::Eating => "Eating",
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
            Self::Mentoring => "Mentoring",
            Self::Grooming => "Grooming",
        }
    }

    /// Maslow hierarchy level. Lower = more fundamental = higher priority.
    /// An urgency can only preempt a plan whose maslow_level is numerically
    /// higher (less fundamental).
    pub fn maslow_level(&self) -> u8 {
        match self {
            // 150 R5a: Eating shares Resting's tier 1 — both physiological.
            Self::Resting | Self::Eating | Self::Hunting | Self::Foraging => 1,
            // 158: Grooming sits at tier 2 — above thermal self-care
            // (now `Action::GroomSelf` riding `Resting` at tier 1) and
            // below the affiliative-coordination tier the Socializing
            // peer group anchors. Matches `groom_other_dse.maslow_tier()`.
            Self::Guarding | Self::Grooming => 2,
            Self::Socializing | Self::Caretaking | Self::Mating | Self::Mentoring => 3,
            Self::Crafting | Self::Coordinating | Self::Building | Self::Farming => 4,
            Self::Exploring => 5,
        }
    }

    /// Infinitive verb form for use after "sets out to".
    pub fn verb_infinitive(&self) -> &'static str {
        match self {
            Self::Resting => "rest",
            Self::Eating => "eat",
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
            Self::Mentoring => "mentor",
            Self::Grooming => "groom a friend",
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
    fn action_eat_maps_to_eating_not_resting() {
        // 150 R5a regression-pin: picking `Action::Eat` at the L3
        // softmax must commit to the new `Eating` disposition, not to
        // `Resting`. The old mapping bundled Eat with Sleep + SelfGroom
        // under Resting, which forced hungry-not-tired cats into a
        // multi-need plan they never finished, leaving them to starve
        // mid-Sleep beat.
        assert_eq!(
            DispositionKind::from_action(Action::Eat),
            Some(DispositionKind::Eating)
        );
        // Sleep stays under Resting — Resting still owns sleep + groom.
        assert_eq!(
            DispositionKind::from_action(Action::Sleep),
            Some(DispositionKind::Resting)
        );
    }

    #[test]
    fn resting_constituents_drop_eat() {
        // 150 R5a: Resting owns Sleep + Groom only; Eating owns Eat.
        // 158: Resting owns Sleep + GroomSelf (allogrooming moved to
        // Grooming).
        assert_eq!(
            DispositionKind::Resting.constituent_actions(),
            &[Action::Sleep, Action::GroomSelf]
        );
        assert_eq!(
            DispositionKind::Eating.constituent_actions(),
            &[Action::Eat]
        );
    }

    #[test]
    fn eating_target_completions_is_max_like_resting() {
        // 150 R5a: Eating completes on hunger threshold (need-based),
        // not on a trip count. `target_completions` returns MAX so the
        // count-based fallback in `should_complete_disposition` /
        // `disposition_complete` never fires for Eating; the need-based
        // arms are authoritative.
        let p = test_personality();
        assert_eq!(
            DispositionKind::Eating.target_completions(&p),
            u32::MAX
        );
    }

    #[test]
    fn eating_shares_resting_maslow_tier() {
        // 150 R5a: both physiological — Maslow tier 1.
        assert_eq!(DispositionKind::Eating.maslow_level(), 1);
        assert_eq!(DispositionKind::Resting.maslow_level(), 1);
    }

    #[test]
    fn all_includes_eating_then_mentoring_then_grooming_appended() {
        // 150 R5a appends Eating at ordinal 13; 154 appends Mentoring
        // at ordinal 14; 158 appends Grooming at ordinal 15. All
        // append (rather than insert near related variants) so
        // positional ordinals in `scoring::active_disposition_ordinal`
        // and `modifier::constituent_dses_for_ordinal` stay stable for
        // the pre-existing variants. Saved soaks and ordinal-equality
        // tests don't need rebasing.
        assert_eq!(DispositionKind::ALL.len(), 15);
        assert_eq!(
            DispositionKind::ALL.last(),
            Some(&DispositionKind::Grooming)
        );
        assert_eq!(
            DispositionKind::ALL[13],
            DispositionKind::Mentoring,
            "Mentoring must remain at ordinal-14 position"
        );
        assert_eq!(
            DispositionKind::ALL[12],
            DispositionKind::Eating,
            "Eating must remain at ordinal-13 position"
        );
        assert_eq!(
            DispositionKind::ALL.first(),
            Some(&DispositionKind::Resting),
            "Resting must remain at ordinal-1 position"
        );
    }

    #[test]
    fn action_mentor_maps_to_mentoring_not_socializing() {
        // 154 regression-pin: picking `Action::Mentor` at the L3
        // softmax must commit to the new `Mentoring` disposition, not
        // to `Socializing`. The old mapping bundled Mentor with
        // Socialize + Groom under Socializing, where MentorCat (cost
        // 3) lost on every plan to the cheaper SocializeWith /
        // GroomOther steps under a count-based completion goal.
        assert_eq!(
            DispositionKind::from_action(Action::Mentor),
            Some(DispositionKind::Mentoring)
        );
        // Socialize stays under Socializing — Socializing still owns
        // the chitchat constituent.
        assert_eq!(
            DispositionKind::from_action(Action::Socialize),
            Some(DispositionKind::Socializing)
        );
    }

    #[test]
    fn socializing_constituents_drop_mentor_and_groom() {
        // 154: Socializing dropped Mentor.
        // 158: Socializing also drops GroomOther — both single-trip
        // peers extracted into their own dispositions to break the
        // equivalent-effect A* pre-pruning at planner/mod.rs:437.
        assert_eq!(
            DispositionKind::Socializing.constituent_actions(),
            &[Action::Socialize]
        );
        assert_eq!(
            DispositionKind::Mentoring.constituent_actions(),
            &[Action::Mentor]
        );
        assert_eq!(
            DispositionKind::Grooming.constituent_actions(),
            &[Action::GroomOther]
        );
    }

    #[test]
    fn action_groom_other_maps_to_grooming_not_socializing() {
        // 158 regression-pin: picking `Action::GroomOther` at the L3
        // softmax must commit to the new `Grooming` disposition, not
        // to `Socializing`. The pre-158 path bundled allogrooming
        // with `SocializeWith` under Socializing's
        // `[SocializeWith (2), GroomOther (2)]` template, where A*
        // pre-pruned the second action because both produced the
        // same `(SetInteractionDone(true), IncrementTrips)` next-state.
        assert_eq!(
            DispositionKind::from_action(Action::GroomOther),
            Some(DispositionKind::Grooming)
        );
        // GroomSelf stays under Resting — Resting still owns the
        // self-care groom constituent.
        assert_eq!(
            DispositionKind::from_action(Action::GroomSelf),
            Some(DispositionKind::Resting)
        );
    }

    #[test]
    fn grooming_target_completions_is_one_like_mentoring() {
        // 158: Grooming is single-interaction (Pattern B). Mirrors
        // Mentoring (also Pattern B, also extracted from Socializing
        // for the equivalent-effect pre-pruning bug class).
        let p = test_personality();
        assert_eq!(DispositionKind::Grooming.target_completions(&p), 1);
        assert_eq!(DispositionKind::Mentoring.target_completions(&p), 1);
    }

    #[test]
    fn grooming_maslow_tier_matches_groom_other_dse() {
        // 158: Grooming sits at tier 2, matching
        // `groom_other_dse.maslow_tier()`. One step up from thermal
        // self-care (GroomSelf rides Resting at tier 1), one step
        // below Socializing (tier 3) — keeps the affiliative ladder
        // monotone in need-priority.
        assert_eq!(DispositionKind::Grooming.maslow_level(), 2);
    }

    #[test]
    fn mentoring_target_completions_is_one_like_mating() {
        // 154: Mentoring is single-interaction (Pattern B). Mirrors
        // Mating's hardcoded `return 1`. The completion proxy at the
        // planner layer is `InteractionDone(true)`, not a trip count;
        // `target_completions` of 1 keeps the count-based fallback in
        // sync (`trips_done >= target_trips` reads as complete after
        // one plan-exhaustion).
        let p = test_personality();
        assert_eq!(DispositionKind::Mentoring.target_completions(&p), 1);
        assert_eq!(DispositionKind::Mating.target_completions(&p), 1);
    }

    #[test]
    fn mentoring_shares_socializing_maslow_tier() {
        // 154: Mentoring is tier 3 (matches Mating / Socializing /
        // Caretaking). Skill transfer is social-coordination work,
        // not physiological.
        assert_eq!(DispositionKind::Mentoring.maslow_level(), 3);
        assert_eq!(DispositionKind::Socializing.maslow_level(), 3);
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
