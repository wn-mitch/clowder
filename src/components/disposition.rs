use bevy_ecs::prelude::*;

use crate::ai::Action;
use crate::components::personality::Personality;

// ---------------------------------------------------------------------------
// DispositionKind — the sustained behavioral orientations
// ---------------------------------------------------------------------------
//
// 155: `CraftingHint` retired. The 8-variant sub-mode enum that papered
// over `DispositionKind::Crafting`'s three-drives-bundled defect (ticket
// 152's audit) is gone. Each former hint variant is now either (a) its
// own L3 Action (HerbcraftGather/Remedy/SetWard, MagicScry/DurableWard/
// Cleanse/ColonyCleanse/Harvest/Commune, Cook), with `from_action`
// routing to the new `Herbalism` / `Witchcraft` / `Cooking`
// disposition; or (b) a directive-routing arm at `to_action` on
// `DirectiveKind` (Cleanse / HarvestCarcass / Cook).

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
    /// Practical herb-medicine and ward-craft.
    ///
    /// 155: split out of the retired `Crafting` umbrella. Owns the
    /// three Herbcraft sub-actions (`HerbcraftGather`,
    /// `HerbcraftRemedy`, `HerbcraftSetWard`). The L3 softmax now picks
    /// the sub-action directly rather than collapsing through a
    /// post-hoc hint. See `docs/open-work/landed/155-crafting-split.md`.
    Herbalism,
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
    /// Spiritual / metaphysical work — scrying, spirit communion,
    /// corruption cleanse, carcass harvest, durable ward placement.
    ///
    /// 155: split out of the retired `Crafting` umbrella. Owns the
    /// six Magic sub-actions (`MagicScry`, `MagicDurableWard`,
    /// `MagicCleanse`, `MagicColonyCleanse`, `MagicHarvest`,
    /// `MagicCommune`). Maslow tier 4 (esteem / craft).
    Witchcraft,
    /// Prepare cooked food at a Kitchen — physiological-adjacent
    /// colony-feeding behavior.
    ///
    /// 155: split out of the retired `Crafting` umbrella. Owns
    /// `Action::Cook` exclusively. Maslow tier 1 (mirrors Hunting /
    /// Foraging shape — Cooking is colony-feeding work, not
    /// esteem-tier craft. Tier 4 reproduced the suppression that left
    /// `FoodCooked` on `never_fired_expected_positives` pre-155).
    Cooking,
    /// 176: drop one carried item on the ground at the cat's current
    /// position. Single-step plan template `[DropItem]`, no travel.
    /// Other cats can forage the dropped item later. Tier 1 — clearing
    /// inventory of unusable surplus is foundation hardening.
    Discarding,
    /// 176: carry one item to the nearest Midden and deposit it there.
    /// Plan template `[TravelTo(Midden), TrashItemAtMidden]`. The
    /// Midden has unlimited capacity, so the deposit cannot fail on
    /// capacity grounds. Tier 1 — keeps the colony commons clean.
    Trashing,
    /// 176: hand one carried item to a nearby cat whose inventory has
    /// room. Tier 1 — direct prosocial transfer.
    Handing,
    /// 176: walk to a desired item on the ground and add it to
    /// inventory. Inverse of `Discarding`. Load-bearing for the
    /// kill→carcass-on-ground→pick-up flow: `engage_prey` spawns a
    /// real carcass entity at the kill site; the cat must plan
    /// `PickingUp` to retrieve it. Tier 1.
    PickingUp,
    /// 230: substrate-aware retreat from a perceived threat.
    ///
    /// Plan template `[PickFleeTarget, Flee, HoldUntilSafe]`:
    /// `PickFleeTarget` reads the cat's per-replan `RouteCostField`
    /// (boldness-scaled fox-scent + corruption overlays) and writes
    /// the lowest-cost passable tile within `flee_distance` to
    /// `target_position`; the `Flee` umbrella delegates to the same
    /// `cat_path_plan!` machinery `TravelTo` uses; `HoldUntilSafe`
    /// then hysteresis-checks `route_cost_at_pos` and
    /// `threat_proximity_derivative` for `flee_hold_ticks` before
    /// `IncrementTrips`. Single-trip completion proxy.
    ///
    /// Closes the §4.7 substrate-over-override migration thread:
    /// 106/107/108/119 retired the Starvation / Exhaustion /
    /// CriticalHealth / CriticalSafety arms of the legacy
    /// `check_anxiety_interrupts`; this carves the last remaining
    /// `ThreatDetected` arm into a real Disposition with goal,
    /// template, completion proxy, and substrate-aware target picker.
    /// The naive vector-projection picker
    /// (`disposition.rs::check_anxiety_interrupts:280-291` pre-230)
    /// preempted plans 39,536× across the 100k-tick post-228 soak;
    /// the modifier preempt guard now composes with the disposition's
    /// commitment instead of firing every tick.
    Fleeing,
    /// 035: bury a deceased colony-mate. Single-action plan template
    /// `[Bury]` over `ZoneIs(CorpseTarget)` (Pattern B, mirrors
    /// Mentoring / Mating / Grooming). Maslow tier 3 (Belonging) —
    /// caring for the dead is a community-belonging act. The
    /// `Burying` disposition is gated by the `HasUnburiedCorpse`
    /// substrate marker (sensing-pre-pass author per §4.7) so its
    /// score is zero whenever no Dead-and-not-Buried entity is in
    /// range. Witness despawns the corpse and spawns a `Grave`
    /// entity at the same position; emits `EventKind::BurialFired`
    /// (continuity canary).
    Burying,
    /// 367: load raw fish or raw organ onto a Drying Rack. Plan
    /// template `[DryFood]` with `ZoneIs(DryingRack)` precondition
    /// (mirrors `Cooking`'s `ZoneIs(Kitchen)` shape). Tier 2 — same
    /// food-buffer infrastructure as Cooking. After loading, drying
    /// progresses without the cat (per-tick under Clear weather), so
    /// the cat completes the action in one tick.
    DryingFood,
    /// 367: load raw meat + fuel onto a Smoking Rack. Plan template
    /// `[SmokeMeat]` with `ZoneIs(SmokingRack)`. Tier 2. Distinct from
    /// `TendingSmokingRack` — loading creates the smoking session;
    /// tending advances it.
    SmokingMeat,
    /// 367: perform one tend cycle on a loaded Smoking Rack. Plan
    /// template `[TendSmokingRack]`. Tier 2. Per-rack cooldown enforced
    /// at the marker layer (`HasLoadedSmokingRackOffCooldown`) — the
    /// DSE doesn't fire again on the same rack until ~2 sim-hours
    /// have elapsed, producing the design-doc "tend, walk away, come
    /// back" rhythm without a multi-tick commitment Component.
    TendingSmokingRack,
    /// 457: cat crafts a 368 Phase 2 behavioral tool at a Workshop.
    /// Plan template `[DropItem?, RetrieveCraftInput*, CraftAtWorkshop]`
    /// with `ZoneIs(Workshop)` precondition on the terminal step.
    /// Generalised over all six Phase 2 recipes (polish + brush +
    /// bundle + 3 gifts) — the DSE elects "any craftable accessible"
    /// via `HasCraftableAccessible`; `resolve_craft_at_workshop`
    /// picks the specific RecipeId from the satisfied set at execute
    /// time (lexicographic order). Maslow tier 4 — esteem / craft,
    /// alongside Herbalism / Witchcraft / Building / Farming. Distinct
    /// from the 367 preservation dispositions (tier 2 — colony-feeding
    /// buffer-building) because behavioral tools enhance social-tier
    /// outcomes, not physiological-tier survival.
    Crafting,
    /// 450: kitten begs for food. Single-action plan template
    /// `[BegForFood]` with no zone precondition (the kitten begs in
    /// place) and no state effect — distinct from Eating because the
    /// DSE emits an `Intention::Activity(Begging, UntilInterrupt)` per
    /// §L2.10.5 rather than a goal-state achievement. The kitten's
    /// hunger doesn't drop *because of* begging; it drops because a
    /// parent witnesses the cry-map signal and feeds them via their
    /// own Caretake plan. Single-trip completion (Pattern B); on each
    /// re-election Begging wins again so long as the kitten remains
    /// `NewbornKitten | EyesOpenKitten` AND lacks `HasFoodInInventory`.
    /// Tier 1 — physiological adjacency: hunger drives the begging
    /// activity, even though the activity itself doesn't satisfy
    /// hunger (the parent does).
    Begging,
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
            // 155: `Crafting` retired into Herbalism / Witchcraft / Cooking;
            // each inherits the single-trip target-completion shape.
            Self::Building
            | Self::Farming
            | Self::Herbalism
            | Self::Witchcraft
            | Self::Cooking
            | Self::Coordinating
            // 367: preservation dispositions each complete after a
            // single action — drying loads-and-walks-away, smoking
            // load creates the session, tend advances one cycle.
            | Self::DryingFood
            | Self::SmokingMeat
            | Self::TendingSmokingRack
            // 457: Crafting completes after one Workshop session
            // (single-trip Pattern B); the resolver consumes one
            // recipe's inputs in one tick.
            | Self::Crafting => 1,
            // 450: Begging is Pattern B (single-tick). Each tick the
            // kitten begs once → `resolve_beg_for_food` stamps the
            // cry-map → plan completes → re-election runs. If the
            // kitten is still `(NewbornKitten | EyesOpenKitten) ∧
            // ¬HasFoodInInventory ∧ hungry`, Begging wins again. The
            // re-election rhythm IS the begging cadence; no commitment
            // Component required.
            Self::Begging => return 1,
            // 176: inventory-disposal dispositions complete after one
            // act. Trashing/Handing/PickingUp involve travel + a single
            // transfer; Discarding is just the in-place drop.
            Self::Discarding | Self::Trashing | Self::Handing | Self::PickingUp => return 1,
            // 230: Fleeing is single-trip — pick a flee target, travel
            // there, hold until the threat scalar lapses. The hysteresis
            // (`flee_hold_ticks`) lives inside `resolve_hold_until_safe`,
            // not in the trip count.
            Self::Fleeing => return 1,
            // 035: Burying is Pattern B (single-interaction). One bury
            // session per commitment; completion proxy is
            // `InteractionDone(true)` at the planner layer.
            Self::Burying => return 1,
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
    /// `Idle` and `Hide` remain in the anxiety-interrupt class with
    /// no parent disposition; `Flee` was promoted to its own
    /// `Fleeing` disposition by ticket 230.
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
    /// 230: `Action::Flee` was extracted from the `Idle | Flee | Hide`
    /// anxiety-interrupt arm and now routes to `DispositionKind::Fleeing`,
    /// closing the §4.7 substrate-over-override migration thread.
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
            // 155: Herbcraft / PracticeMagic / Cook split into 9 + 1 +
            // 0 sub-actions across three new dispositions. Each L3
            // sub-action routes to its parent Disposition directly.
            Action::HerbcraftGather | Action::HerbcraftRemedy | Action::HerbcraftSetWard => {
                Some(Self::Herbalism)
            }
            Action::MagicScry
            | Action::MagicDurableWard
            | Action::MagicCleanse
            | Action::MagicColonyCleanse
            | Action::MagicHarvest
            | Action::MagicCommune => Some(Self::Witchcraft),
            Action::Cook => Some(Self::Cooking),
            // 367: preservation actions route directly to their
            // single-action dispositions. Three peer dispositions
            // because each has a distinct station + state machine
            // (drying = sun-powered passive, smoking-load = consume-
            // inputs, smoking-tend = advance-progress).
            Action::DryFood => Some(Self::DryingFood),
            Action::SmokeMeat => Some(Self::SmokingMeat),
            Action::TendSmokingRack => Some(Self::TendingSmokingRack),
            // 457: Craft (previously dormant for #334) promotes to live
            // here as the Workshop-craft pipeline's L3 sub-action.
            // Generalised over all `StationRequirement::Workshop`
            // recipes; the resolver picks the specific RecipeId at
            // execute time.
            Action::Craft => Some(Self::Crafting),
            // 450: BegForFood routes to its single-action Begging
            // disposition. Begging is Activity-shaped (§L2.10.5) — the
            // Disposition + plan template + resolver still wrap it so
            // the L3 softmax / executor pipeline stays uniform, but the
            // emitted Intention is `Activity(Begging, UntilInterrupt)`,
            // not a `Goal`. See `DispositionKind::Begging` doc.
            Action::BegForFood => Some(Self::Begging),
            Action::Coordinate => Some(Self::Coordinating),
            Action::Explore | Action::Wander => Some(Self::Exploring),
            Action::Mate => Some(Self::Mating),
            Action::Caretake => Some(Self::Caretaking),
            // 176: inventory-disposal Action variants route directly
            // to their parent disposition.
            Action::Drop => Some(Self::Discarding),
            Action::Trash => Some(Self::Trashing),
            Action::Handoff => Some(Self::Handing),
            Action::PickUp => Some(Self::PickingUp),
            // 230: `Action::Flee` was promoted out of the anxiety-
            // interrupt class into its own `Fleeing` disposition. The
            // retired `check_anxiety_interrupts::ThreatDetected` arm
            // (the last surviving anxiety-interrupt path; previously
            // 280-291 in `src/systems/disposition.rs`) is replaced by
            // a substrate-aware target picker + commitment proxy.
            Action::Flee => Some(Self::Fleeing),
            // 035: Bury routes directly to its single-action Burying
            // disposition (Pattern B — mirrors Mentoring / Grooming).
            Action::Bury => Some(Self::Burying),
            // Anxiety-interrupt class — `Hide` (104) is the
            // "remain still and hope" sibling valence; `Idle` is
            // the no-op fallback. Both stay headless until evidence
            // justifies promoting them.
            //
            // 322: HTN-method sub-goal Primitives (WearItem /
            // PetitionCoordinator / Vigil / GriefSit / Wean / Teach /
            // Release) also return None. They're never L3-selectable —
            // a Live HTN method emits them as method sub-goal leaves,
            // not as winning actions from the per-tick DSE softmax.
            // Their wiring tickets (#332/#333/#334) flip the
            // corresponding PendingSubstrate method to Live; the
            // Actions stay outside DispositionKind selection regardless.
            //
            // 457: `Action::Craft` exits this list — promoted to a
            // live L3 selectable via the Workshop-craft pipeline. The
            // dormant comment for #334 (StealthCloak) on the Action
            // variant now points to "first live user, more recipes to
            // come" rather than "deferred until #334."
            Action::Idle
            | Action::Hide
            | Action::WearItem
            | Action::PetitionCoordinator
            | Action::Vigil
            | Action::GriefSit
            | Action::ReleaseGrief
            | Action::Wean
            | Action::Teach
            | Action::Release => None,
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
            // 155: Herbalism / Witchcraft / Cooking each own a disjoint
            // sub-action set; the L3 softmax picks one of these
            // sub-actions directly.
            Self::Herbalism => &[
                Action::HerbcraftGather,
                Action::HerbcraftRemedy,
                Action::HerbcraftSetWard,
            ],
            Self::Witchcraft => &[
                Action::MagicScry,
                Action::MagicDurableWard,
                Action::MagicCleanse,
                Action::MagicColonyCleanse,
                Action::MagicHarvest,
                Action::MagicCommune,
            ],
            Self::Cooking => &[Action::Cook],
            // 367: preservation dispositions each own a single action.
            Self::DryingFood => &[Action::DryFood],
            Self::SmokingMeat => &[Action::SmokeMeat],
            Self::TendingSmokingRack => &[Action::TendSmokingRack],
            // 457: Crafting owns the single Action::Craft constituent.
            // Generalised over the six Phase 2 recipes — recipe choice
            // happens at execute time inside `resolve_craft_at_workshop`,
            // not at the L3 softmax layer.
            Self::Crafting => &[Action::Craft],
            // 450: Begging owns the single Action::BegForFood constituent.
            Self::Begging => &[Action::BegForFood],
            Self::Coordinating => &[Action::Coordinate],
            Self::Exploring => &[Action::Explore, Action::Wander],
            Self::Mating => &[Action::Mate],
            Self::Caretaking => &[Action::Caretake],
            Self::Mentoring => &[Action::Mentor],
            Self::Grooming => &[Action::GroomOther],
            // 176: inventory-disposal dispositions own one Action each.
            Self::Discarding => &[Action::Drop],
            Self::Trashing => &[Action::Trash],
            Self::Handing => &[Action::Handoff],
            Self::PickingUp => &[Action::PickUp],
            // 230: Fleeing owns the single `Action::Flee` umbrella that
            // L2 modifiers (047 / 108) lift on health deficit and
            // threat proximity. The plan template's `PickFleeTarget`
            // and `HoldUntilSafe` steps are NOT first-class L3 actions
            // (they don't appear in the disposition's softmax pool);
            // they're internal chain steps emitted by
            // `fleeing_actions()` and dispatched by the GOAP executor.
            Self::Fleeing => &[Action::Flee],
            // 035: Burying owns the single `Action::Bury` constituent.
            Self::Burying => &[Action::Bury],
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
    /// 155: `Crafting` ordinal 8 is replaced in-place by `Herbalism`
    /// (the herbcraft-DSE-set inherits the slot); `Witchcraft` and
    /// `Cooking` append at ordinals 16 / 17 to keep upstream ordinals
    /// stable for saved soaks and ordinal-equality tests.
    /// 176: inventory-disposal dispositions append at ordinals 17-20
    /// to keep upstream ordinals stable for saved soaks and ordinal-
    /// equality tests.
    /// 230: `Fleeing` appends at ordinal 21 for the same reason —
    /// promoting Flee out of the anxiety-interrupt class doesn't
    /// renumber upstream ordinals.
    pub const ALL: &[Self] = &[
        Self::Resting,
        Self::Hunting,
        Self::Foraging,
        Self::Guarding,
        Self::Socializing,
        Self::Building,
        Self::Farming,
        Self::Herbalism,
        Self::Coordinating,
        Self::Exploring,
        Self::Mating,
        Self::Caretaking,
        Self::Eating,
        Self::Mentoring,
        Self::Grooming,
        Self::Witchcraft,
        Self::Cooking,
        Self::Discarding,
        Self::Trashing,
        Self::Handing,
        Self::PickingUp,
        Self::Fleeing,
        // 035: Burying appended at ordinal 23 (zero-indexed: 22).
        Self::Burying,
        // 367: preservation dispositions appended at ordinals 24-26.
        Self::DryingFood,
        Self::SmokingMeat,
        Self::TendingSmokingRack,
        // 450: Begging appended at ordinal-27 (zero-indexed: 26). Tail
        // append preserves the upstream ordinal-stability invariant for
        // saved soaks and `scoring::active_disposition_ordinal`
        // ordinal-equality tests.
        Self::Begging,
        // 457: Crafting appended at ordinal-28 (zero-indexed: 27). Same
        // ordinal-stability invariant — saved soaks pre-457 don't carry
        // Crafting and the ordinal-equality scorer skips it.
        Self::Crafting,
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
            Self::Herbalism => "Herbalism",
            Self::Coordinating => "Coordinating",
            Self::Exploring => "Exploring",
            Self::Mating => "Mating",
            Self::Caretaking => "Caretaking",
            Self::Mentoring => "Mentoring",
            Self::Grooming => "Grooming",
            Self::Witchcraft => "Witchcraft",
            Self::Cooking => "Cooking",
            Self::Discarding => "Discarding",
            Self::Trashing => "Trashing",
            Self::Handing => "Handing",
            Self::PickingUp => "PickingUp",
            Self::Fleeing => "Fleeing",
            Self::Burying => "Burying",
            Self::DryingFood => "DryingFood",
            Self::SmokingMeat => "SmokingMeat",
            Self::TendingSmokingRack => "TendingSmokingRack",
            Self::Begging => "Begging",
            Self::Crafting => "Crafting",
        }
    }

    /// Maslow hierarchy tier. Lower = more fundamental = higher priority.
    /// An urgency can only preempt a plan whose maslow_tier is numerically
    /// higher (less fundamental).
    pub fn maslow_tier(&self) -> u8 {
        match self {
            // 150 R5a: Eating shares Resting's tier 1 — both physiological.
            // 155: Cooking sits at tier 1 too — colony-feeding behavior
            // mirrors the Hunting / Foraging shape (Retrieve → produce →
            // Deposit). Tier 4 reproduces the suppression that left
            // `FoodCooked` on `never_fired_expected_positives` pre-155.
            Self::Resting
            | Self::Eating
            | Self::Hunting
            | Self::Foraging
            | Self::Cooking
            // 176: inventory-disposal is foundation hardening — clearing
            // surplus and retrieving real ground items is part of the
            // physiological-tier loop.
            | Self::Discarding
            | Self::Trashing
            | Self::Handing
            | Self::PickingUp
            // 450: Begging is physiological-adjacent — driven by the
            // kitten's hunger. The activity itself doesn't satiate
            // hunger (a parent's Caretake does), but the *want* is
            // hunger-driven; tier-1 lets Begging preempt social/
            // exploratory dispositions in the same way Eating does
            // for adults.
            | Self::Begging
            // 230: Fleeing is acute survival — preempting higher tiers
            // is the whole point of `ThreatProximityAdrenalineFlee`
            // lifting Flee. (Pre-251 the now-retired
            // `AcuteHealthAdrenalineFlee` also lifted Flee on injury;
            // 251 retired that lift in favor of Sleep's substrate-side
            // urgency axis.)
            | Self::Fleeing => 1,
            // 158: Grooming sits at tier 2 — above thermal self-care
            // (now `Action::GroomSelf` riding `Resting` at tier 1) and
            // below the affiliative-coordination tier the Socializing
            // peer group anchors. Matches `groom_other_dse.maslow_tier()`.
            // 367: preservation dispositions share tier 2 with
            // Guarding/Grooming — forward-looking buffer-building, a
            // step above Cooking's tier-1 acute colony-feeding floor.
            Self::Guarding
            | Self::Grooming
            | Self::DryingFood
            | Self::SmokingMeat
            | Self::TendingSmokingRack => 2,
            Self::Socializing | Self::Caretaking | Self::Mating | Self::Mentoring => 3,
            // 035: Burying is Belonging (caring for the dead is a
            // community-belonging act). Same tier as the affiliative
            // peer dispositions.
            Self::Burying => 3,
            // 155: Herbalism / Witchcraft inherit Crafting's tier 4
            // (esteem / craft).
            // 457: Crafting (new, generalised over Workshop recipes)
            // sits at tier 4 alongside Herbalism / Witchcraft / Building
            // / Farming — esteem/craft work, not survival.
            Self::Herbalism
            | Self::Witchcraft
            | Self::Coordinating
            | Self::Building
            | Self::Farming
            | Self::Crafting => 4,
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
            Self::Herbalism => "prepare herbs",
            Self::Coordinating => "coordinate",
            Self::Exploring => "explore",
            Self::Mating => "find a mate",
            Self::Caretaking => "tend the young",
            Self::Mentoring => "mentor",
            Self::Grooming => "groom a friend",
            Self::Witchcraft => "work magic",
            Self::Cooking => "cook",
            Self::Discarding => "drop a surplus item",
            Self::Trashing => "carry a surplus item to the midden",
            Self::Handing => "hand a surplus item to a colony-mate",
            Self::PickingUp => "pick up a ground item",
            Self::Fleeing => "flee to safer ground",
            Self::Burying => "bury the fallen",
            // 367: preservation infinitive verbs — used by the
            // narrative writer ("sets out to ..."). Single-cat acts of
            // food-craft infrastructure work.
            Self::DryingFood => "lay food on the drying rack",
            Self::SmokingMeat => "load the smoking rack",
            Self::TendingSmokingRack => "tend the smoking rack",
            Self::Begging => "beg for food",
            Self::Crafting => "craft at the workshop",
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
    /// The exact L3 sub-action the softmax picked. Threaded from
    /// scoring to chain builder so the cascade doesn't re-derive.
    ///
    /// 155: replaces the retired `crafting_hint: Option<CraftingHint>`
    /// field. Every disposition records its chosen Action — for
    /// single-constituent dispositions (Hunting → Hunt, Cooking →
    /// Cook, etc.) it's trivial; for the new Herbalism / Witchcraft
    /// dispositions it carries the sub-mode the L3 picked.
    pub chosen_action: Action,
}

impl Disposition {
    pub fn new(
        kind: DispositionKind,
        chosen_action: Action,
        tick: u64,
        personality: &Personality,
    ) -> Self {
        Self {
            kind,
            adopted_tick: tick,
            disposition_started_tick: 0,
            completions: 0,
            target_completions: kind.target_completions(personality),
            chosen_action,
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
        // 230: `Action::Flee` was promoted out of the
        // anxiety-interrupt class into its own `Fleeing` disposition.
        assert_eq!(
            DispositionKind::from_action(Action::Flee),
            Some(DispositionKind::Fleeing)
        );
        // Idle / Hide remain headless (anxiety-interrupt class).
        assert_eq!(DispositionKind::from_action(Action::Idle), None);
        assert_eq!(DispositionKind::from_action(Action::Hide), None);
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
        assert_eq!(DispositionKind::Eating.target_completions(&p), u32::MAX);
    }

    #[test]
    fn eating_shares_resting_maslow_tier() {
        // 150 R5a: both physiological — Maslow tier 1.
        assert_eq!(DispositionKind::Eating.maslow_tier(), 1);
        assert_eq!(DispositionKind::Resting.maslow_tier(), 1);
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
        // 155: `Crafting` ordinal 8 is replaced in-place by `Herbalism`
        // (the herbcraft-DSE-set inherits the slot); `Witchcraft` and
        // `Cooking` append at ordinals 16 / 17 to preserve the upstream
        // ordinal stability invariant.
        // 176: inventory-disposal dispositions append at ordinals
        // 18-21 (Discarding, Trashing, Handing, PickingUp).
        // 230: `Fleeing` appends at ordinal 22 (closing the
        // anxiety-interrupt-class migration).
        // 035: `Burying` appends at ordinal 23. Fleeing slides from
        // tail to ordinal-22; Burying becomes the new tail.
        // 367: preservation dispositions append at ordinals 24-26
        // (DryingFood, SmokingMeat, TendingSmokingRack).
        // 450: Begging appends at ordinal-27. TendingSmokingRack slides
        // off the tail; Begging becomes the new tail. The 367 trio
        // keeps its ordinal slots (24-26) for ordinal-stability.
        // 457: Crafting appends at ordinal-28. Begging slides from
        // tail to ordinal-27; Crafting becomes the new tail.
        assert_eq!(DispositionKind::ALL.len(), 28);
        assert_eq!(
            DispositionKind::ALL.last(),
            Some(&DispositionKind::Crafting),
            "Crafting must remain at ordinal-28 (tail) position"
        );
        assert_eq!(
            DispositionKind::ALL[25],
            DispositionKind::TendingSmokingRack,
            "TendingSmokingRack must remain at ordinal-26 position"
        );
        assert_eq!(
            DispositionKind::ALL[24],
            DispositionKind::SmokingMeat,
            "SmokingMeat must remain at ordinal-25 position"
        );
        assert_eq!(
            DispositionKind::ALL[23],
            DispositionKind::DryingFood,
            "DryingFood must remain at ordinal-24 position"
        );
        assert_eq!(
            DispositionKind::ALL[22],
            DispositionKind::Burying,
            "Burying must remain at ordinal-23 position"
        );
        assert_eq!(
            DispositionKind::ALL[21],
            DispositionKind::Fleeing,
            "Fleeing must remain at ordinal-22 position"
        );
        assert_eq!(
            DispositionKind::ALL[20],
            DispositionKind::PickingUp,
            "PickingUp must remain at ordinal-21 position"
        );
        assert_eq!(
            DispositionKind::ALL[19],
            DispositionKind::Handing,
            "Handing must remain at ordinal-20 position"
        );
        assert_eq!(
            DispositionKind::ALL[18],
            DispositionKind::Trashing,
            "Trashing must remain at ordinal-19 position"
        );
        assert_eq!(
            DispositionKind::ALL[17],
            DispositionKind::Discarding,
            "Discarding must remain at ordinal-18 position"
        );
        assert_eq!(
            DispositionKind::ALL[16],
            DispositionKind::Cooking,
            "Cooking must remain at ordinal-17 position"
        );
        assert_eq!(
            DispositionKind::ALL[15],
            DispositionKind::Witchcraft,
            "Witchcraft must remain at ordinal-16 position"
        );
        assert_eq!(
            DispositionKind::ALL[14],
            DispositionKind::Grooming,
            "Grooming must remain at ordinal-15 position"
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
            DispositionKind::ALL[7],
            DispositionKind::Herbalism,
            "Herbalism must inherit Crafting's ordinal-8 position"
        );
        assert_eq!(
            DispositionKind::ALL.first(),
            Some(&DispositionKind::Resting),
            "Resting must remain at ordinal-1 position"
        );
    }

    #[test]
    fn crafting_split_routes_sub_actions_to_three_dispositions() {
        // 155 regression-pin: each former `Action::Herbcraft` /
        // `Action::PracticeMagic` / `Action::Cook` sub-action must
        // route to the correct new disposition via `from_action`,
        // bypassing the retired `CraftingHint` post-softmax tournament.
        for a in [
            Action::HerbcraftGather,
            Action::HerbcraftRemedy,
            Action::HerbcraftSetWard,
        ] {
            assert_eq!(
                DispositionKind::from_action(a),
                Some(DispositionKind::Herbalism),
                "{a:?} should route to Herbalism"
            );
        }
        for a in [
            Action::MagicScry,
            Action::MagicDurableWard,
            Action::MagicCleanse,
            Action::MagicColonyCleanse,
            Action::MagicHarvest,
            Action::MagicCommune,
        ] {
            assert_eq!(
                DispositionKind::from_action(a),
                Some(DispositionKind::Witchcraft),
                "{a:?} should route to Witchcraft"
            );
        }
        assert_eq!(
            DispositionKind::from_action(Action::Cook),
            Some(DispositionKind::Cooking)
        );
    }

    #[test]
    fn cooking_is_maslow_tier_one_like_hunting_foraging() {
        // 155 R3 decision: Cooking is colony-feeding work, not
        // esteem-tier craft. Tier 4 reproduces the suppression that
        // left `FoodCooked` on `never_fired_expected_positives`
        // pre-155.
        assert_eq!(DispositionKind::Cooking.maslow_tier(), 1);
        assert_eq!(DispositionKind::Hunting.maslow_tier(), 1);
        assert_eq!(DispositionKind::Foraging.maslow_tier(), 1);
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
        assert_eq!(DispositionKind::Grooming.maslow_tier(), 2);
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
        assert_eq!(DispositionKind::Mentoring.maslow_tier(), 3);
        assert_eq!(DispositionKind::Socializing.maslow_tier(), 3);
    }

    #[test]
    fn action_bury_maps_to_burying() {
        // 035 regression-pin: picking `Action::Bury` at the L3 softmax
        // must commit to the new `Burying` disposition (Pattern B,
        // tier 3 Belonging).
        assert_eq!(
            DispositionKind::from_action(Action::Bury),
            Some(DispositionKind::Burying)
        );
    }

    #[test]
    fn burying_is_pattern_b_single_interaction() {
        // 035: Burying is single-interaction (Pattern B). Mirrors
        // Mentoring / Grooming / Mating. One bury session per
        // commitment.
        let p = test_personality();
        assert_eq!(DispositionKind::Burying.target_completions(&p), 1);
    }

    #[test]
    fn burying_maslow_tier_is_three() {
        // 035: Belonging (caring for the dead is a community-belonging
        // act). Same tier as Socializing / Caretaking / Mating /
        // Mentoring.
        assert_eq!(DispositionKind::Burying.maslow_tier(), 3);
    }

    #[test]
    fn burying_constituent_action_is_just_bury() {
        assert_eq!(
            DispositionKind::Burying.constituent_actions(),
            &[Action::Bury]
        );
    }

    #[test]
    fn disposition_count_completion() {
        let p = Personality {
            diligence: 0.5,
            ..test_personality()
        };
        let mut d = Disposition::new(DispositionKind::Hunting, Action::Hunt, 0, &p);
        assert!(!d.is_count_complete());
        d.completions = d.target_completions;
        assert!(d.is_count_complete());
    }
}
