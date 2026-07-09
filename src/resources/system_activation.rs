use std::collections::BTreeMap;

use bevy_ecs::prelude::*;

use crate::components::joint_intention::PracticeKind;

// ---------------------------------------------------------------------------
// FeatureCategory — valence of a tracked feature
// ---------------------------------------------------------------------------

/// Whether a feature firing represents a good, bad, or neutral event.
///
/// `Positive` features contribute to the colony's activation score and the
/// "is the colony thriving?" diagnostic. `Negative` features are tallied as a
/// raw event count — how many bad things happened — and do *not* inflate the
/// activation score. `Neutral` features are system-churn signals used for
/// per-feature breakdowns but not rolled up into any score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum FeatureCategory {
    Positive,
    Negative,
    Neutral,
}

// ---------------------------------------------------------------------------
// Feature — trackable simulation features
// ---------------------------------------------------------------------------

/// Enumeration of simulation features whose activation we track.
///
/// Each variant represents a meaningful event in the simulation — not a Bevy
/// system running, but actual *work* being done (corruption spreading to a
/// new tile, a bond forming, a ShadowFox spawning, etc.).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum Feature {
    CorruptionSpread,
    CorruptionTileEffect,
    /// Ticket 101 — emitted by `emit_env_quality_features` when at
    /// least one living cat's `EnvironmentalQualityModifier` combined
    /// value clears `+feature_emit_threshold` on a given tick. Positive
    /// valence: the seed-42 canonical colony has a hearth and gardens,
    /// so a cat near them should produce a positive combined modifier.
    /// `expected_to_fire_per_soak() => true` — never-fired means the
    /// influence-map sweep, the modifier wiring, or the personality
    /// scaling is broken; treat as a failed verification.
    EnvironmentalComfortPositive,
    /// Ticket 101 — emitted by `emit_env_quality_features` when at
    /// least one living cat's combined env-quality value falls below
    /// `-feature_emit_threshold`. Negative valence: requires unburied
    /// corpses, dirty buildings, or sustained mud/rock exposure to
    /// fire. `expected_to_fire_per_soak() => false` until a scenario
    /// reliably produces a negative-dominant environment in seed 42.
    EnvironmentalComfortNegative,
    ShadowFoxSpawn,
    WardDecay,
    HerbSeasonalCheck,
    RemedyApplied,
    /// Ticket 365 — Positive. Emitted when `resolve_prepare_remedy`
    /// Advances (a real `ItemKind::Remedy*` lands in the cat's
    /// inventory). Upstream of `RemedyApplied`. Classified as
    /// `expected_to_fire_per_soak() => false` to match
    /// `RemedyApplied`'s opt-out — herbcraft DSEs require both an
    /// injured patient and a cat with healing affinity, which not
    /// every seed-42 soak surfaces.
    RemedyPrepared,
    PersonalCorruptionEffect,
    CombatResolved,
    InjuryHealed,
    /// Ticket 095 Phase 1 — anatomical body-part damage applied to a cat.
    /// Emitted by `damage_to_body_part` (combat.rs) alongside the legacy
    /// `Injury` push during Stage A. Positive: cats take damage in every
    /// canonical seed-42 soak (wildlife encounters, standoffs).
    BodyPartInjury,
    FateAssigned,
    FateAwakened,
    AspirationSelected,
    AspirationCompleted,
    AspirationAbandoned,
    /// Ticket 055 / §7.7.d — drift-threshold abandonment. Distinct
    /// from `AspirationAbandoned` (stagnation + low personality
    /// alignment) so canaries / trace can attribute drops to mood
    /// drift vs. personality drift independently.
    AspirationDriftAbandoned,
    BondFormed,
    CoordinatorElected,
    DirectiveIssued,
    /// Ticket 382 — a Build directive's `compute_building_placement`
    /// returned `None` for `placement_stuck_narrate_threshold_ticks`
    /// consecutive ticks; the coordinator narrated the situation and
    /// the system reset the counter. Regression canary —
    /// `expected_to_fire_per_soak() => false`. Firing in a healthy
    /// seed-42 soak means the influence-map composition or
    /// `ColonyDistrictMap` weights are mis-calibrated; treat as a
    /// failed verification.
    DirectiveStuckOnPlacement,
    /// Ticket 382 — a Build directive's `compute_building_placement`
    /// returned `Some` and `spawn_construction_sites` spawned a new
    /// ConstructionSite entity. Positive observability signal paired
    /// with `DirectiveStuckOnPlacement`. The seed-42 deep-soak issues
    /// ~6 Build directives over 15 min, so this is expected to fire
    /// at least once in any healthy canonical soak.
    ConstructionSiteSpawned,
    BuildingConstructed,
    BuildingTidied,
    GateProcessed,
    MoodContagion,
    PersonalityFriction,
    AnxietyInterrupt,
    /// Ticket 118 — substrate-driven plan preemption fired. The
    /// `check_modifier_preemption` system found at least one acute-class
    /// modifier whose `preempts_in_flight()` predicate returned true
    /// for an in-flight cat, dropped the cat's plan via
    /// `plan_substrate::try_preempt`, and the cat will re-elect on the
    /// next tick. Distinct from `AnxietyInterrupt` (the legacy Maslow
    /// override that 118+119 are retiring): this fires whenever the
    /// substrate is asking for behavioral expression, not only on the
    /// hardcoded `health < critical_threshold` predicate. Negative
    /// valence — interrupts are colony stress signals.
    ModifierPreemption,
    PreyBred,
    PreyDenAbandoned,
    PreyDenFounded,
    DenRaided,
    WildlifeSpawned,
    DeathStarvation,
    DeathOldAge,
    DeathInjury,
    KnowledgePromoted,
    KnowledgeForgotten,
    SpiritCommunion,
    StorageUpgraded,
    DepositRejected,
    DepositFailedNoStore,
    ItemRetrieved,
    /// Ticket 084: a cat deposited ≥1 herb of any `HerbKind` into a
    /// Stores building's `StoredHerbs` via the `DepositHerbs` GOAP
    /// action. Positive — the gather→stash half of the herb economy
    /// fired. Classified `=> true` in `expected_to_fire_per_soak`
    /// because Commit 2 wires every `HerbcraftGather` plan to
    /// terminate at Stores, and Gather fires on healthy seed-42.
    HerbsDeposited,
    /// Ticket 084: a cat retrieved ≥1 herb of a specific `HerbKind`
    /// from a Stores building's `StoredHerbs` via the
    /// `RetrieveHerbs(kind)` GOAP action. Positive — the
    /// retrieve→weave half of the herb economy fired. Classified
    /// `=> true` in `expected_to_fire_per_soak` because Commit 2
    /// wires every `HerbcraftSetWard` plan to either carry-direct
    /// OR retrieve-first, and the colony enters the retrieve-path
    /// regime whenever wild thornbriar isn't immediately at hand.
    HerbsRetrieved,
    /// A cat finished cooking a raw food item at a Kitchen, flipping its
    /// `cooked` flag. Eating the item later grants a hunger multiplier.
    FoodCooked,
    /// 367: cat loaded a raw fish or raw organ onto a Drying Rack.
    /// Positive intermediate — the eventual `FoodDried` Feature fires
    /// when the per-tick preservation system completes the craft.
    FoodLoadedOnDryingRack,
    /// 367: cat loaded raw meat + fuel onto a Smoking Rack. Positive
    /// intermediate — the eventual `MeatSmoked` Feature fires when a
    /// tend cycle closes out the craft.
    MeatLoadedOnSmokingRack,
    /// 367: cat performed one tend cycle on a loaded Smoking Rack
    /// (advanced progress by 1/N). Positive but intermediate — distinct
    /// from `MeatSmoked` so the tend cadence shows up in traces
    /// independent of completion rate.
    SmokingRackTended,
    /// 367: per-tick preservation system completed a drying craft and
    /// spawned a `DriedFish` or `PreservedOrgan` `Item` entity at the
    /// rack tile. Positive — the colony's winter buffer just grew by
    /// one preserved item.
    FoodDried,
    /// 367: tend cycle completed a smoking craft and spawned a
    /// `SmokedMeat` `Item` entity at the rack tile. Positive — same
    /// shape as `FoodDried` but for the meat pipeline.
    MeatSmoked,
    /// 367: per-tick preservation system completed a Preserved Organ
    /// craft. Positive sibling to `FoodDried` — separated so the trace
    /// distinguishes fish-pipeline from organ-pipeline activity (the
    /// organ recipe has a different input shape and rarer source).
    OrganPreserved,
    KittenBorn,
    GestationAdvanced,
    KittenMatured,
    MatingOccurred,
    /// An adult completed a FeedKitten step, transferring food from
    /// inventory to a dependent kitten. Positive-feature signal for
    /// the Caretake system's activity (Phase 4c.3).
    KittenFed,
    /// A cat advanced a Garden's `CropState.growth` via a TendCrops
    /// step. Positive-feature signal for the Farming system's
    /// activity (Phase 4c.4 — previously absent, which kept the
    /// Activation canary from catching the silent-dead farming
    /// pipeline for months).
    CropTended,
    /// A cat harvested a Garden at full growth, spawning food (or
    /// Thornbriar) into Stores. Paired with `CropTended` — splits the
    /// two distinct "farming is alive" signals so a partial failure
    /// (tending fires but harvest never does, or vice versa) is
    /// visible in the activation footer.
    CropHarvested,
    // --- Fox ecology ---
    FoxHuntedPrey,
    FoxStoreRaided,
    FoxStandoff,
    FoxStandoffEscalated,
    FoxRetreated,
    FoxDenEstablished,
    FoxBred,
    FoxCubMatured,
    FoxDied,
    FoxScentMarked,
    FoxAvoidedCat,
    FoxDenDefense,
    FoxAvoidedWard,
    FoxAvoidedPresence,
    ShadowFoxAvoidedWard,
    ShadowFoxAvoidedCatScent,
    /// Ticket 023 Phase A — a shadow-fox dissolved when sustained
    /// clean-ground exposure drove `coherence` to zero. Mythic-register
    /// event paired with `ShadowFoxBanished`; rare-legend category so
    /// it returns `false` from `expected_to_fire_per_soak()`.
    ShadowFoxDissolved,
    /// Ticket 023 Phase B — a shadow-fox entered the Reconstituting
    /// state to recover coherence on a high-corruption tile.
    ShadowFoxReconstitutingEntered,
    /// Ticket 023 Phase B — a shadow-fox entered the Tending state
    /// to reinforce corruption near a ward perimeter.
    ShadowFoxTendingEntered,
    /// Ticket 023 Phase B — a shadow-fox entered the Haunting state
    /// to apply psychological pressure on a target cat. Phase C
    /// wires the per-tick safety/mood drain.
    ShadowFoxHauntingEntered,
    /// Ticket 023 Phase B — a shadow-fox entered the Seeding state
    /// to extend the corruption frontier.
    ShadowFoxSeedingEntered,
    /// Ticket 023 Phase C — `shadowfox_haunting_drain` applied a
    /// per-tick mood/safety drain to a cat in haunting range. Records
    /// at `shadow_fox_haunting_feature_cadence` (not every tick) to
    /// keep the activation footer readable. Positive valence — the
    /// substrate's psychological-predation drive is alive.
    ShadowFoxHaunting,
    /// Ticket 023 Phase C — a Haunting shadow-fox crossed the
    /// `shadow_fox_haunting_escalation_ticks` threshold and the
    /// motivation tick promoted the haunt to Stalking (the existing
    /// pre-023 combat-approach path).
    ShadowFoxHauntingEscalated,
    /// Ticket 310 S1 — the hunger drive (fifth motivation-softmax
    /// input, `(1 − satiation)² × shadow_fox_hunger_drive_weight`)
    /// won the election and the shadow-fox entered Stalking toward
    /// the nearest scanned cat — a goal-directed hunt, distinct from
    /// the legacy 5%/tick stalk roll in `predator_stalk_cats`.
    ShadowFoxHungerHuntEntered,
    /// Ticket 310 S2 — a shadow-fox that landed an ambush entered
    /// Retreating toward its den (SingleMinded until arrival) instead
    /// of the legacy resume-patrol.
    ShadowFoxRetreatEntered,
    DirectiveDelivered,
    // --- Hawk ecology (ticket 025 Phase 2) ---
    /// A hawk spotted prey within detection range. Witness: yes/no per
    /// `resolve_spot_prey` call. Positive — substrate-driven hunting is
    /// finding targets.
    HawkSpottedPrey,
    /// A hawk completed a dive (the dive *attempt* — kill-attribution is
    /// `predator_hunt_prey`'s job). Positive — the predation pipeline
    /// is firing.
    HawkDiveLanded,
    /// A hawk finished perching on a Perch zone. Positive but bursty —
    /// depends on Resting disposition winning the softmax.
    HawkPerched,
    /// A hawk reached the map edge while Fleeing. Positive but state-
    /// dependent (only fires when health drops or cats threaten).
    HawkFled,
    /// A hawk died of starvation / old age / combat. Bursty.
    HawkDied,
    // --- Snake ecology ---
    /// A snake completed a strike attempt against prey (kill-attribution
    /// stays in `predator_hunt_prey`). Positive.
    SnakeStruckPrey,
    /// A snake completed a Bask cycle on warm terrain. Positive but
    /// thermoregulation-dependent.
    SnakeBasked,
    /// A snake settled into ambush posture after `SetAmbush`. Positive
    /// — the default predation mode is firing.
    SnakeAmbushed,
    /// A snake reached cover / map edge while Retreating. State-dependent.
    SnakeRetreated,
    /// A snake died of starvation / old age / combat. Bursty.
    SnakeDied,
    // --- Corruption & carcass systems ---
    CarcassSpawned,
    WardSiegeStarted,
    CarcassCleansed,
    CarcassHarvested,
    CorruptionPushback,
    HerbSuppressed,
    CorruptionHealthDrain,
    GatherHerbCompleted,
    WardPlaced,
    WardDespawned,
    ScryCompleted,
    CleanseCompleted,
    /// A shadow-fox was banished by a posse of cats — colony-defining Legend event.
    ShadowFoxBanished,
    /// A candidate cat was skipped over for a Fight directive because its
    /// hunger fell below the critical-interrupt floor. Emitted from
    /// `dispatch_urgent_directives` so the starvation-respects-posse guard
    /// is observable in `events.jsonl`.
    PosseCandidateExcludedStarving,

    // --- §Phase 5a: silent-advance audit (see StepOutcome<W>) ---
    // Each of these was a "dead subsystem" candidate — the step resolver
    // could Advance without doing its real-world work, and no Feature
    // fired, so the Activation canary went blind to silent failures.
    //
    /// A cat consumed a food item at a Stores building, restoring
    /// hunger. Gated on actual food consumption (store non-empty AND
    /// a food item was found), not just time-out at the Stores zone.
    FoodEaten,
    /// Two cats completed a socialize-target interaction (non-groom,
    /// non-mentor): relationships mutated, social need boosted. Gated
    /// on a real target partner — a socialize step with no target no
    /// longer fires this.
    Socialized,
    /// An adult groomed another cat: relationship + restoration
    /// effect applied. Distinct from `self_groom` (cleanliness need).
    GroomedOther,
    /// A mentor-apprentice interaction occurred: skill/knowledge
    /// transfer + relationship shift.
    MentoredCat,
    /// 035: a cat completed a Bury chain — corpse received `Buried`
    /// marker, was despawned, and a `Grave` entity was spawned at
    /// the same tile. Tally: drives the `burial` continuity canary
    /// via the paired `EventKind::BurialFired` event.
    BurialPerformed,
    /// A cat engaged wildlife in combat-posture: safety-need swing +
    /// combat-skill growth. Paired with the existing
    /// `CombatResolved` (emitted from `src/systems/combat.rs` when a
    /// fight resolves) — ThreatEngaged is "the step ran and found a
    /// target", CombatResolved is "combat terminated with a winner".
    ThreatEngaged,
    /// A cat successfully delivered one unit of build material from
    /// inventory to a ConstructionSite (incrementing the site's
    /// delivered ledger by one). Ticket 038 promoted this from a
    /// dead-wired emission to a real per-unit witness: each haul of
    /// the founding wagon-dismantling pile produces one event.
    MaterialsDelivered,
    /// A cat picked up a build-material `Item` from the ground
    /// (location flipped from `OnGround` to `Carried(cat)`, slot
    /// added to inventory). Paired with `MaterialsDelivered` —
    /// every delivery is preceded by exactly one pickup. Ticket 038.
    MaterialPickedUp,
    /// A cat completed a building-repair pass (condition ≥ 1.0).
    /// Not a silent-advance fix per se (repair already returns Fail
    /// on missing target), but previously unsignalled — repairs were
    /// invisible to the Activation canary.
    BuildingRepaired,
    /// A mating attempt completed without producing a pregnancy
    /// (Tom×Tom, or Queen who is already pregnant, etc). The social
    /// / belonging-tier interaction still occurred. Paired with
    /// `MatingOccurred` which fires only when a `Pregnancy` was
    /// inserted.
    CourtshipInteraction,
    /// Phase 6a §7.2 — drop-trigger gate fired for a cat's held
    /// `GoapPlan`: the `CommitmentStrategy` dispatch said to drop
    /// (achievement believed, planner hard-fail under SingleMinded,
    /// or satiation under OpenMinded) and the plan was removed.
    /// Distinct from `AnxietyInterrupt` (which is the Maslow
    /// event-driven preemption that bypasses §7.2 entirely).
    /// Neutral — the gate is a reconsideration signal, not a
    /// healthy-colony win or adverse event by itself.
    ///
    /// Aggregate counter — retained for back-compat with any
    /// dashboard reading it. Branch-specific counters below replace
    /// it for canary purposes.
    CommitmentDropTriggered,
    /// §7.2 gate dropped a `Blind` plan (Resting, Guarding). The
    /// Blind strategy drops only on `achievement_believed`, so this
    /// fires when a rest cycle or patrol completes. Expected to fire
    /// at least once per 15-min soak when Resting is reached.
    CommitmentDropBlind,
    /// §7.2 gate dropped a `SingleMinded` plan on `achievement_believed`
    /// (Hunt/Build/Forage/etc. goal met). Distinct from `…ReplanCap`
    /// below, which covers the `achievable_believed == false` hard-
    /// fail branch of the same strategy.
    CommitmentDropSingleMinded,
    /// §7.2 gate dropped an `OpenMinded` plan on `still_goal == false`
    /// (satiation / desire drift). Fires for Socializing satiation
    /// and future Exploring curiosity-drift.
    CommitmentDropOpenMinded,
    /// §7.2 `achievable_believed == false` hard-fail channel: the
    /// planner exhausted `max_replans` retries on a goal-shaped plan
    /// and the gate let it go. Fires alongside the executor's own
    /// "abandoned" narrative event at `goap.rs:~3144`; tracking it
    /// as a distinct Feature lets canaries catch planner collapse
    /// separately from legitimate completion.
    CommitmentDropReplanCap,
    /// Ticket 288 — `morale_break` consumed by the GOAP step-fail
    /// dispatcher. The cat's EngageThreat step refused to continue
    /// (HP below `fight_bail_health_threshold`) and the dispatcher
    /// released the disposition commitment so L3 can re-elect on the
    /// next tick — rather than the legacy in-disposition replan path
    /// that left wounded Guarding cats marching toward ambushes.
    /// Neutral; rare-event class (depends on a wounded cat reaching
    /// EngageThreat) so `expected_to_fire_per_soak() => false` until
    /// post-fix baseline shows reliable firing.
    CommitmentDropMoraleBreak,
    // ---------- Ticket 127 — JointIntention (subsumes §7.M PairingActivity) ----------
    /// `crate::ai::joint_intention::author_joint_intentions` inserted a
    /// [`crate::components::JointIntention`] on an eligible cat for the
    /// named practice. For Courtship this is the 1:1 successor to
    /// `PairingIntentionEmitted` — same emission semantics, same canary
    /// classification. `expected_to_fire_per_soak() => true` for
    /// Courtship.
    JointIntentionEmitted {
        practice: PracticeKind,
    },
    /// The author's drop gate fired on a held JointIntention. Neutral
    /// — drops are normal state transitions, not an adverse signal
    /// (mirrors `PairingDropped`). Stays
    /// `expected_to_fire_per_soak() => false` (bursty).
    JointIntentionDropped {
        practice: PracticeKind,
    },
    /// A practice-biased resolver was invoked with
    /// `target == JointIntention.partner` AND
    /// `joint.practice == bias-reader's practice filter`, applying the
    /// practice's `bias_multiplier` to that tick's fondness +
    /// familiarity deltas. Per-amplification fire site (single-seed
    /// observable on any healthy chain). Successor to
    /// `PairingBiasApplied`. `expected_to_fire_per_soak() => true` for
    /// Courtship.
    JointBiasApplied {
        practice: PracticeKind,
    },
    /// The author advanced the cat's `PracticeStage` to the next
    /// observable stage in the practice's transition table (Courtship:
    /// Approach → Courting → Mating → Bonded). Positive — stage
    /// progression is the structural signal that the practice is
    /// progressing rather than stalling. `expected_to_fire_per_soak()
    /// => true` for Courtship (any healthy soak crosses
    /// Approach → Courting in the seed-42 window).
    JointStageAdvanced {
        practice: PracticeKind,
    },
    /// A paired cat's `PracticeStage` differed from its partner's at
    /// the lower-Entity-index side of the pair this tick — the
    /// substrate hook for "codified irony" (one cat believes they're
    /// courting while the other is just being friendly). Counted once
    /// per pair per tick. Neutral — mismatch windows are healthy
    /// narrative texture, not a regression signal.
    /// `expected_to_fire_per_soak() => false` — a perfectly-synced
    /// healthy colony can have zero mismatch ticks.
    JointStageMismatchTickAccrued {
        practice: PracticeKind,
    },

    /// Ticket 080 — `evaluate_target_taking` gated a candidate to 0.0
    /// because its `Reserved.owner` named a cat other than the scoring
    /// cat. Neutral — observability of the resource-reservation
    /// substrate; firing means the gate is doing real work.
    /// `expected_to_fire_per_soak() => false` until the producer side
    /// (`record_target_picked` writes) ships in a follow-on; without
    /// producers, no `Reserved` is ever written, so the gate cannot
    /// activate.
    ReservationContended,
    /// Ticket 073 — the `target_recent_failure` Consideration scored
    /// below 1.0 for a candidate (i.e., the cooldown penalty was
    /// load-bearing for at least one (cat, action, target) tuple this
    /// tick). Neutral — recently-failed candidates being penalized is
    /// normal substrate behavior; the count is a soft soak-delta
    /// sentinel for "is the cooldown actually getting applied?".
    /// Stays `expected_to_fire_per_soak() => false` until soak data
    /// confirms a healthy seed-42 run sees ≥1 cooldown penalty fire.
    TargetCooldownApplied,
    /// Ticket 104 — `HideDse` selected and a freeze cycle concluded
    /// (resolved as `Advance` in `resolve_hide`). Positive — colony
    /// is exercising the third predator-avoidance valence ("remain
    /// still and hope") alongside Flee and Fight.
    /// `expected_to_fire_per_soak() => false` initially: Phase 1 ships
    /// dormant (the `HideEligible` marker is never authored), and
    /// even after activation the freeze valence is rare-event class
    /// — it requires the cornered-and-overmatched gate from modifier
    /// 105 to trip, which most healthy soaks won't see.
    HideFreezeFired,
    /// Ticket 149 — a discrete hunt attempt resolved (kill / lost /
    /// abandoned). Paired 1:1 with `EventKind::HuntAttempt` events;
    /// every successful kill, every approach loss, every abandon
    /// fires this. Lets the never-fired canary catch a silently-dead
    /// hunting pipeline, and gives the per-discrete-attempt rate a
    /// canonical activation footer surface alongside the events
    /// stream. Defaults to `expected_to_fire_per_soak() => true`
    /// because any healthy colony attempts prey ≥1 per 15-min soak.
    HuntAttempted,

    // --- 176: inventory-disposal Features ---
    /// 176: a cat completed a `Discarding` plan — dropped a carried
    /// item on the ground at their current position. Items remain
    /// real entities (`ItemLocation::OnGround`) so the colony hasn't
    /// lost the resource — another cat can plan a `PickingUp` to
    /// retrieve it. Neutral category: a Discarding completion is
    /// neither a colony win nor a loss, just a state transition.
    /// Defaults to `expected_to_fire_per_soak() => false` — the
    /// disposal DSEs ship dormant (default-zero scoring); once
    /// balance-tuning lifts the saturation surfaces this should fire
    /// in healthy colonies but the canary stays opt-in until then.
    ItemDropped,
    /// 176: a cat completed a `Trashing` plan — carried an item to
    /// the nearest Midden building and deposited it. Midden capacity
    /// is unlimited so the deposit never fails on capacity. The
    /// item entity remains real (stored at the midden); future scope:
    /// midden items decay faster via the existing rot ecology.
    /// Neutral / `expected => false` until balance-tuning activates.
    ItemTrashed,
    /// 176: a cat completed a `Handing` plan — transferred a carried
    /// item from their inventory to a target cat's inventory.
    /// Neutral / `expected => false` until balance-tuning activates.
    ItemHandedOff,
    /// 176: an `engage_prey` or `forage_item` resolver couldn't fit
    /// a fresh catch / forage into the cat's full inventory and
    /// dropped it on the ground at the action's position instead.
    /// Items remain real entities (no silent destruction); the
    /// negative classification is the anomaly signal — chronic
    /// `OverflowToGround` correlates with full-inventory cats whose
    /// disposal DSEs aren't clearing the surplus.
    /// `expected_to_fire_per_soak() => false`: in a healthy Maslow-
    /// ascending colony this should be near-zero; non-zero counts
    /// inform the post-soak `verdict` whether disposal is keeping up.
    OverflowToGround,
    /// Ticket 228 — emitted when cat-side path resolution falls back
    /// from per-cat `RouteCostField` gradient walking to A\* with
    /// overlay slices. Fires whenever the field is missing
    /// (pre-flood; despawned and respawned cats), stale (older than
    /// the replan window), or doesn't reach the destination
    /// (cost beyond `MAX_COST_BUDGET`). `expected_to_fire_per_soak()
    /// => false` — a healthy soak should rarely hit this code path
    /// since every replan rebuilds the field; chronic counts indicate
    /// field-staleness or build-correctness bugs.
    RouteCostFieldFallback,
    /// Ticket 230 — `PickFleeTarget` resolved a substrate-aware flee
    /// destination against the cat's per-replan `RouteCostField`. Fires
    /// once per Fleeing-disposition adoption (the first step in the
    /// chain). `expected_to_fire_per_soak() => false`: cascade from
    /// `ThreatProximityAdrenalineFlee` lifting Flee, which itself is
    /// rare on a healthy colony. (Pre-251 also `AcuteHealthAdrenalineFlee`
    /// lifted Flee on injury; 251 retired that lift — but Flee adoption
    /// was already statistical-zero in seed-42 healthy soaks per the
    /// 252 audit, so the retirement does not change Flee's adoption
    /// rate.) Will promote to `true` after the post-230 baseline shows
    /// it firing reliably across seeds.
    FleeTargetPicked,
    /// Ticket 230 — `HoldUntilSafe` completed: the cat held a low-
    /// `RouteCostField` tile with safety need above
    /// `flee_safety_need_threshold` for `flee_hold_ticks`. Closes the
    /// Fleeing trip, which is the canonical signal that the modifier-
    /// driven thrash spiral is broken. `expected_to_fire_per_soak()
    /// => false` initially (cascade from `FleeTargetPicked`); promote
    /// after the soak baseline stabilizes.
    FleeRecovered,

    // --- Ticket 126 BDI intention lifecycle ---
    /// Ticket 126 — L2 evaluator inserted a `HeldIntention` Component
    /// alongside the cat's `GoapPlan` after the softmax winner was
    /// selected. Positive — every adoption is one cat committing to a
    /// goal-shaped intention with a margin-derived strength.
    /// `expected_to_fire_per_soak() => true`: any seed-42 run with at
    /// least one cat picking a non-Idle disposition produces ≥1.
    IntentionAdopted,
    /// Ticket 126 — held intention reached `achievement_believed`
    /// (the §7.2 `DropBranch::Achieved` path) and was cleared.
    /// Positive — every fulfilment is one closed goal cycle. The
    /// `IntentionFulfilled / IntentionAdopted` ratio is the substrate
    /// completion rate. `expected_to_fire_per_soak() => true`.
    IntentionFulfilled,
    /// Ticket 126 — held intention dropped on a non-fulfilment
    /// trigger (preempted, became impossible, target invalid,
    /// expired, or desire drift). Per-cause detail recorded against
    /// the focal-cat trace's `L3Commitment.abandon_reason`; the
    /// activation counter is unparameterised because drops are bursty
    /// (matches `PairingDropped`'s precedent). `expected_to_fire_per_soak()
    /// => false`.
    IntentionAbandoned,

    // --- Ticket 320 — HTN method-stack lifecycle ---
    /// Ticket 320 — the L2 evaluator pushed a `GoalFrame` onto a
    /// cat's `HeldGoalStack` after a winning DSE's `Intention::Goal`
    /// matched a Live method in `MethodRegistry`. Positive — every
    /// adoption is a multi-step arc that registry-walking surfaces
    /// (trace / inspect / footer). `expected_to_fire_per_soak() =>
    /// false` at 320's land (registry is empty of Live methods; no
    /// path to fire). **Promote to `true` in ticket 323** when
    /// `courtship_method` registers as the first Live Tier-1 method.
    MethodAdopted,
    /// Ticket 320 — the active leaf sub-goal fulfilled and the L2
    /// evaluator advanced the cursor to the next sub-goal (or popped
    /// the frame on completion). Positive — the substrate-driven
    /// version of "multi-step plan made forward progress."
    /// `expected_to_fire_per_soak() => false` at 320's land; **promote
    /// to `true` in 323** alongside `MethodAdopted`.
    SubGoalAdvanced,
    /// Ticket 320 — a leaf abandoned with `BecameImpossible` or
    /// `TargetInvalid` and the top frame's `MethodFailure::Backtrack`
    /// strategy fired, walking the registry for the next applicable
    /// method. Neutral — backtracks are alternative-pursuit signals,
    /// not adverse events. `expected_to_fire_per_soak() => false`
    /// (depends on substrate-driven plan failure cadence; promote
    /// after the post-323 baseline stabilizes if reliable).
    MethodBacktracked,
    /// Ticket 320 — the stack hit `MAX_GOAL_STACK_DEPTH` and the L2
    /// evaluator fell back to the no-method adoption path. Neutral —
    /// authoring-loop canary; chronic counts indicate a method-
    /// registry cycle or a method whose sub-goals nest beyond the
    /// cap. `expected_to_fire_per_soak() => false` (rare-event class).
    MethodDepthExceeded,

    // --- Ticket 332 — `mourn_at_grave` HTN method primitives ---
    /// Ticket 332 — a cat performed a vigil tick at a colony-mate's
    /// grave (real-world effect: mourning-cycle counter advanced).
    /// Positive — substrate-driven grief processing is alive.
    /// `expected_to_fire_per_soak() => false` until the dispatch
    /// follow-on (named in #332's landing Log) wires DSE /
    /// GoapActionKind / plan template / resolver call site so this
    /// resolver actually fires.
    VigilHeld,
    /// Ticket 332 — a cat performed a grief-sit tick (in-den grief
    /// processing). Positive — substrate-driven grief processing is
    /// alive. `expected_to_fire_per_soak() => false` until dispatch
    /// follow-on lands.
    GriefProcessed,
    /// Ticket 332 — a cat retired their `Mourning` Component, ending
    /// the mourning arc. Positive — colony grief-cycle completion
    /// is observable. `expected_to_fire_per_soak() => false` until
    /// dispatch follow-on lands.
    GriefReleased,

    // --- Ticket 333 — `rear_kitten` HTN method primitives ---
    /// Ticket 333 — a queen advanced a dependent kitten past the
    /// wean threshold. Positive — substrate-driven kitten-rearing
    /// arc is alive. `expected_to_fire_per_soak() => false` until
    /// dispatch follow-on (named in #333's landing Log) wires DSE /
    /// GoapActionKind / plan template / resolver call site.
    KittenWeaned,
    /// Ticket 333 — a queen demonstrated a skill to a dependent
    /// kitten. Positive — substrate-driven mentoring of next
    /// generation is alive. `expected_to_fire_per_soak() => false`
    /// until dispatch follow-on lands.
    SkillTaught,
    /// Ticket 333 — a queen released a now-independent kitten,
    /// retiring the kitten's `KittenDependency` Component. Positive
    /// — generational continuity is observable beyond the existing
    /// `KittenMatured` Feature (which fires on the kitten side from
    /// the maturity tick; this one fires on the mother side from
    /// the rearing-arc completion). `expected_to_fire_per_soak()
    /// => false` until dispatch follow-on lands.
    KittenReleased,

    /// Ticket 450 — a Stage 1 or Stage 2 kitten completed one beg
    /// cycle via `resolve_beg_for_food`. Positive; ships dormant
    /// (`expected_to_fire_per_soak() => false`) because kittens are
    /// filtered out of `evaluate_and_plan` / `evaluate_dispositions`
    /// via `Without<KittenDependency>` (§Phase 5b — preserves the
    /// FeedKitten +0.5 hunger restoration path). Until a kitten-side
    /// scoring pipeline lifts that filter, kittens never elect
    /// `BegForFood` at L3 and this canary cannot fire. The DSE /
    /// resolver / dispatch arm land as ready-to-wire substrate in
    /// 450; the follow-on ticket that unblocks kitten scoring
    /// promotes this Feature back to `=> true`.
    KittenBegged,
    /// Ticket 375 — `resolve_engage_prey` spawned a non-meat byproduct
    /// item (Bone, Sinew, Whisker, Hide, FishScale, Tallow, RawOrgan,
    /// or Feather) alongside the carcass. Positive — input substrate
    /// for the 016 crafting epic is alive; every successful kill
    /// produces at least one byproduct via `prey_byproducts` table.
    /// `expected_to_fire_per_soak() => true`: a healthy seed-42 soak
    /// produces dozens of hunts across all five prey species, each
    /// firing this canary at least twice. Zero count means the
    /// `prey_byproducts` table reads stale, the loop is gated off,
    /// or `resolve_engage_prey` itself isn't firing — the 367-class
    /// silent-producer dormancy this canary exists to catch.
    ByproductSpawned,

    /// Ticket 368 — `resolve_groom_other` ran with the actor's
    /// `Inventory` containing a `GroomingBrush`. Positive — the
    /// 016 Phase 2 behavioral-tool substrate is exercising the
    /// grooming canary. Ships dormant (`expected_to_fire_per_soak()
    /// => false`) until the Workshop-craft pipeline follow-on
    /// lands and brushes actually circulate in seed-42; until then
    /// no cat carries a brush in a healthy soak and this canary
    /// cannot fire. The resolver branch + emission ARE wired so
    /// the test scenarios that hand-place a brush exercise it.
    GroomingBrushUsed,
    /// Ticket 368 — `resolve_socialize` ran with either participant's
    /// `Inventory` containing a `PlayBundle`. Positive — paired
    /// with `GroomingBrushUsed`. Ships dormant for the same reason.
    PlayBundleEngaged,
    /// Ticket 368 — `resolve_mate_with` ran with the courting cat's
    /// `Inventory` containing a `CourtshipGift`. Positive — paired
    /// with `GroomingBrushUsed`. Ships dormant for the same reason.
    CourtshipGiftOffered,
    /// Ticket 457 — `resolve_craft_at_workshop` consumed one Workshop
    /// recipe's full input set and produced an output `Item`. Positive,
    /// `expected_to_fire_per_soak() => true` — the elect-side first-
    /// light gate for the 368 Phase 2 behavioral-tool substrate. If
    /// seed-42 produces zero `ItemCrafted` events, the Workshop-craft
    /// pipeline is structurally broken (DSE not scoring, plan not
    /// forming, marker not authored, or resolver not dispatched) and
    /// the wiring needs fixing before ship.
    ItemCrafted,
    /// Ticket 477 — a `DurabilityTier::Fragile` weapon (one of the three
    /// bone weapons) snapped on a failed hunt-strike and was removed from
    /// the cat's inventory. Neutral valence — wear is a mechanical texture
    /// event, neither a colony win nor a welfare loss (it surfaces the
    /// "items have bite" durability substrate). Classified
    /// `expected_to_fire_per_soak() == false`: it needs a fragile bone
    /// weapon wielded plus a missed strike plus the snap roll landing,
    /// which doesn't reliably reproduce in a 15-min seed-42 soak.
    /// Exercised by the `equipment_bone_snap` scenario.
    BoneWeaponSnapped,
    /// Ticket 334 — `resolve_wear_item` donned a carried wearable from the
    /// cat's pouch into its anatomical `WearableSlots` (or swapped the
    /// occupant). Positive — surfaces the deliberate don/swap path that the
    /// `acquire_stealth_via_self_craft` HTN method's `WearItem` leaf drives.
    /// Classified `expected_to_fire_per_soak() => false`: in the self-craft
    /// happy path the freshly-crafted cloak is auto-equipped on craft (017),
    /// so the don leaf is an idempotent no-op-success and `ItemWorn` only
    /// fires when the Cape slot was occupied at craft time (a swap) — which
    /// doesn't reliably reproduce in a 15-min seed-42 soak.
    ItemWorn,

    // --- Ticket 429: items-are-real Source/Sink gate Features ---
    //
    // Every `ItemSource` impl under `src/components/item_gate/sources/`
    // emits its `const FEATURE: Feature` via the trait's default
    // push-or-overflow body; the new Sink resolver
    // `resolve_eat_from_own_inventory` emits `EatFromOwnInventory`.
    // Promoting the seven inline `inventory.pouch.push(...)` sites at
    // `disposition.rs:3234/3757/4196` + `goap.rs:8837/9439/9476/9964`
    // (and the `eat_from_inventory` Sink at `needs.rs:325`) to the gate
    // contract means these Features fire 1:1 with the underlying
    // items-creating / items-consuming events.
    //
    // `HuntByproductSource` reuses the existing `ByproductSpawned`
    // Positive canary (375) rather than introducing a parallel variant
    // — the two would be 1:1 by construction (every byproduct
    // push/overflow fires both exactly once).
    /// Ticket 429 — den-raid produced a carcass (item entered Inventory
    /// or spawned on the ground at the den). Positive valence: the
    /// raid's perspective for the *colony* is a food win, distinct from
    /// the existing Negative `DenRaided` Feature which tracks the
    /// prey-population loss. `expected_to_fire_per_soak() => true` —
    /// seed-42 colonies raid dens routinely.
    ItemSourcedFromDenRaid,
    /// Ticket 429 — a hunt catch was sourced (carcass entered Inventory
    /// or spawned on the ground at the prey's tile). Positive valence,
    /// `expected_to_fire_per_soak() => true` — paired with the existing
    /// `HuntAttempted` Feature which tracks every discrete attempt;
    /// this fires only on success.
    ItemSourcedFromHuntCatch,
    /// Ticket 429 — a foraged item was sourced (entered Inventory or
    /// spawned on the ground at the forage tile). Positive valence,
    /// `expected_to_fire_per_soak() => true` — seed-42 colonies forage
    /// routinely.
    ItemSourcedFromForageCatch,
    /// Ticket 429 — a cat consumed a food item from its own Inventory
    /// via the `resolve_eat_from_own_inventory` Sink (Sink resolver
    /// extracted from the pre-substrate-era `eat_from_inventory`
    /// autonomic system at `needs.rs::325`). Positive valence,
    /// `expected_to_fire_per_soak() => true` — kittens autoconsume
    /// pocket food via the dispatcher every seed-42 soak (the existing
    /// behavior; only the code path changed in 429).
    EatFromOwnInventory,
    /// Ticket 482 — a drying-rack or smoking-rack completed its cycle
    /// and spawned its output on the ground at the rack's tile.
    /// Always-ground Source (the loader doesn't pick the output up;
    /// it sits on the rack for a later cat to fetch). Positive valence,
    /// `expected_to_fire_per_soak() => true` — preservation completions
    /// fire reliably in seed-42 colonies.
    ItemSourcedFromPreservation,
    /// Ticket 482 — a cat harvested a corruption-laden carcass and
    /// the ShadowBone yield was sourced (into Inventory if room, else
    /// the ground via the trait's default push-or-overflow body —
    /// retiring the pre-482 silent-drop on inventory-full). Positive
    /// valence, `expected_to_fire_per_soak() => true`. Distinct from
    /// `CarcassHarvested` which is the gameplay-event witness — this
    /// is the items-are-real gate witness, same separation as
    /// `FoodDried` vs `ItemSourcedFromPreservation`.
    ItemSourcedFromHarvestCarcass,
    /// Ticket 482 — a forager's tile rolled an ingredient drop (Twig /
    /// Fiber / Flower) and the ingredient spawned on the ground at the
    /// forager's position. Always-ground Source (the cat doesn't pick
    /// the ingredient up; it sits there for a later herbcrafter).
    /// Positive valence, `expected_to_fire_per_soak() => true` —
    /// ingredient drops fire reliably in seed-42 colonies.
    ItemSourcedFromForageIngredient,
}

impl Feature {
    pub const ALL: &[Feature] = &[
        Feature::CorruptionSpread,
        Feature::CorruptionTileEffect,
        // 101: env-quality influence-map canaries.
        Feature::EnvironmentalComfortPositive,
        Feature::EnvironmentalComfortNegative,
        Feature::ShadowFoxSpawn,
        Feature::WardDecay,
        Feature::HerbSeasonalCheck,
        Feature::RemedyApplied,
        Feature::RemedyPrepared,
        Feature::PersonalCorruptionEffect,
        Feature::CombatResolved,
        Feature::InjuryHealed,
        Feature::BodyPartInjury,
        Feature::FateAssigned,
        Feature::FateAwakened,
        Feature::AspirationSelected,
        Feature::AspirationCompleted,
        Feature::AspirationAbandoned,
        Feature::AspirationDriftAbandoned,
        Feature::BondFormed,
        Feature::CoordinatorElected,
        Feature::DirectiveIssued,
        Feature::BuildingConstructed,
        Feature::BuildingTidied,
        Feature::GateProcessed,
        Feature::MoodContagion,
        Feature::PersonalityFriction,
        Feature::AnxietyInterrupt,
        Feature::ModifierPreemption,
        Feature::PreyBred,
        Feature::PreyDenAbandoned,
        Feature::PreyDenFounded,
        Feature::DenRaided,
        Feature::WildlifeSpawned,
        Feature::DeathStarvation,
        Feature::DeathOldAge,
        Feature::DeathInjury,
        Feature::KnowledgePromoted,
        Feature::KnowledgeForgotten,
        Feature::SpiritCommunion,
        Feature::StorageUpgraded,
        Feature::DepositRejected,
        Feature::DepositFailedNoStore,
        Feature::ItemRetrieved,
        Feature::HerbsDeposited,
        Feature::HerbsRetrieved,
        Feature::FoodCooked,
        // 367: preservation Features. Five expected-to-fire on
        // seed-42 (FoodLoadedOnDryingRack / MeatLoadedOnSmokingRack
        // / SmokingRackTended / FoodDried / MeatSmoked);
        // OrganPreserved is chain-rare (30% organ drop × herb
        // availability × ~2-day cure) and stays expected=false.
        // Without this enrollment the SystemActivation snapshot
        // and never-fired canary don't iterate the new variants —
        // a classic substrate-stub-class failure: writer present,
        // category/display-name match arms updated, but the
        // iteration source overlooked.
        Feature::FoodLoadedOnDryingRack,
        Feature::MeatLoadedOnSmokingRack,
        Feature::SmokingRackTended,
        Feature::FoodDried,
        Feature::MeatSmoked,
        Feature::OrganPreserved,
        Feature::KittenBorn,
        Feature::GestationAdvanced,
        Feature::KittenMatured,
        Feature::MatingOccurred,
        Feature::KittenFed,
        Feature::CropTended,
        Feature::CropHarvested,
        // Fox ecology
        Feature::FoxHuntedPrey,
        Feature::FoxStoreRaided,
        Feature::FoxStandoff,
        Feature::FoxStandoffEscalated,
        Feature::FoxRetreated,
        Feature::FoxDenEstablished,
        Feature::FoxBred,
        Feature::FoxCubMatured,
        Feature::FoxDied,
        Feature::FoxScentMarked,
        Feature::FoxAvoidedCat,
        Feature::FoxDenDefense,
        Feature::FoxAvoidedWard,
        Feature::FoxAvoidedPresence,
        Feature::ShadowFoxAvoidedWard,
        Feature::ShadowFoxAvoidedCatScent,
        Feature::ShadowFoxDissolved,
        Feature::ShadowFoxReconstitutingEntered,
        Feature::ShadowFoxTendingEntered,
        Feature::ShadowFoxHauntingEntered,
        Feature::ShadowFoxSeedingEntered,
        Feature::ShadowFoxHaunting,
        Feature::ShadowFoxHauntingEscalated,
        Feature::ShadowFoxHungerHuntEntered,
        Feature::ShadowFoxRetreatEntered,
        Feature::DirectiveDelivered,
        // Hawk ecology (ticket 025 Phase 2). All four "trunk" positives
        // ship dormant via `expected_to_fire_per_soak() => false` in
        // this commit and are promoted to `true` in commit 6 once the
        // seed-42 cutover soak confirms they fire.
        Feature::HawkSpottedPrey,
        Feature::HawkDiveLanded,
        Feature::HawkPerched,
        Feature::HawkFled,
        Feature::HawkDied,
        // Snake ecology
        Feature::SnakeStruckPrey,
        Feature::SnakeBasked,
        Feature::SnakeAmbushed,
        Feature::SnakeRetreated,
        Feature::SnakeDied,
        // Corruption & carcass systems
        Feature::CarcassSpawned,
        Feature::WardSiegeStarted,
        Feature::CarcassCleansed,
        Feature::CarcassHarvested,
        Feature::CorruptionPushback,
        Feature::HerbSuppressed,
        Feature::CorruptionHealthDrain,
        Feature::GatherHerbCompleted,
        Feature::WardPlaced,
        Feature::WardDespawned,
        Feature::ScryCompleted,
        Feature::CleanseCompleted,
        Feature::ShadowFoxBanished,
        Feature::PosseCandidateExcludedStarving,
        // §Phase 5a silent-advance audit
        Feature::FoodEaten,
        Feature::Socialized,
        Feature::GroomedOther,
        Feature::MentoredCat,
        // 035: burial canary.
        Feature::BurialPerformed,
        Feature::ThreatEngaged,
        Feature::MaterialsDelivered,
        Feature::MaterialPickedUp,
        Feature::BuildingRepaired,
        Feature::CourtshipInteraction,
        // §Phase 6a §7.2 drop-trigger gate
        Feature::CommitmentDropTriggered,
        Feature::CommitmentDropBlind,
        Feature::CommitmentDropSingleMinded,
        Feature::CommitmentDropOpenMinded,
        Feature::CommitmentDropReplanCap,
        // Ticket 288 — morale_break rebind to commitment release.
        Feature::CommitmentDropMoraleBreak,
        // Ticket 127 — JointIntention (subsumes §7.M L2 PairingActivity
        // from ticket 027b). Each parameterized variant is
        // enumerated per-practice; for 127 only Courtship exists.
        Feature::JointIntentionEmitted {
            practice: PracticeKind::Courtship,
        },
        Feature::JointIntentionDropped {
            practice: PracticeKind::Courtship,
        },
        Feature::JointBiasApplied {
            practice: PracticeKind::Courtship,
        },
        Feature::JointStageAdvanced {
            practice: PracticeKind::Courtship,
        },
        Feature::JointStageMismatchTickAccrued {
            practice: PracticeKind::Courtship,
        },
        // §sub-epic 071 — planning-substrate hardening
        Feature::ReservationContended,
        Feature::TargetCooldownApplied,
        // Ticket 104 — Hide/Freeze DSE infrastructure (dormant in Phase 1).
        Feature::HideFreezeFired,
        Feature::HuntAttempted,
        // 176: inventory-disposal Features. Stage 2 ships them
        // dormant via default-zero scoring on the disposal DSEs;
        // balance-tuning lifts the saturation surfaces in a follow-on.
        Feature::ItemDropped,
        Feature::ItemTrashed,
        Feature::ItemHandedOff,
        Feature::OverflowToGround,
        Feature::RouteCostFieldFallback,
        // 230: Fleeing chain completion signals.
        Feature::FleeTargetPicked,
        Feature::FleeRecovered,
        // 126: BDI intention lifecycle.
        Feature::IntentionAdopted,
        Feature::IntentionFulfilled,
        Feature::IntentionAbandoned,
        // 320: HTN method-stack lifecycle. Ship dormant (expected:
        // false) until 323's courtship_method registers Live; flip
        // MethodAdopted + SubGoalAdvanced to expected: true in 323.
        Feature::MethodAdopted,
        Feature::SubGoalAdvanced,
        Feature::MethodBacktracked,
        Feature::MethodDepthExceeded,
        // 332: `mourn_at_grave` HTN method primitives. Ship dormant
        // (expected: false) until the dispatch follow-on lands.
        Feature::VigilHeld,
        Feature::GriefProcessed,
        Feature::GriefReleased,
        // 333: `rear_kitten` HTN method primitives. Ship dormant
        // (expected: false) until the dispatch follow-on lands.
        Feature::KittenWeaned,
        Feature::SkillTaught,
        Feature::KittenReleased,
        // 375: producer-side canary for prey-byproduct decomposition.
        // expected_to_fire_per_soak() => true (every kill emits ≥1).
        Feature::ByproductSpawned,
        // 450: kitten begs for food canary.
        Feature::KittenBegged,
        // 368: Phase 2 behavioral-tool canaries. Ship dormant (see
        // each variant's doc-comment) until the Workshop-craft
        // pipeline follow-on lands and tools circulate in seed-42.
        Feature::GroomingBrushUsed,
        Feature::PlayBundleEngaged,
        Feature::CourtshipGiftOffered,
        // 457: Workshop-craft first-light canary.
        Feature::ItemCrafted,
        // 477: bone-weapon durability snap.
        Feature::BoneWeaponSnapped,
        // 334: deliberate don/swap of a worn item.
        Feature::ItemWorn,
        // 429: items-are-real Source/Sink gate Features (HuntByproduct
        // reuses the existing ByproductSpawned Positive canary).
        Feature::ItemSourcedFromDenRaid,
        Feature::ItemSourcedFromHuntCatch,
        Feature::ItemSourcedFromForageCatch,
        Feature::EatFromOwnInventory,
        // 482: items-are-real Source gate for the three Source-shaped
        // sites 429 deferred (rack output, ShadowBone yield, herbcraft
        // ingredient drop).
        Feature::ItemSourcedFromPreservation,
        Feature::ItemSourcedFromHarvestCarcass,
        Feature::ItemSourcedFromForageIngredient,
    ];

    /// The valence of this feature.
    ///
    /// Exhaustive match — adding a new `Feature` variant without classifying
    /// it here is a compile error, which is intentional: the activation
    /// diagnostics depend on every feature having a known valence.
    pub const fn category(self) -> FeatureCategory {
        use FeatureCategory::*;
        match self {
            // --- Positive: healthy-colony wins ---
            Feature::RemedyApplied => Positive,
            Feature::RemedyPrepared => Positive,
            Feature::InjuryHealed => Positive,
            Feature::BodyPartInjury => Positive,
            Feature::FateAssigned => Positive,
            Feature::FateAwakened => Positive,
            Feature::AspirationSelected => Positive,
            Feature::AspirationCompleted => Positive,
            Feature::BondFormed => Positive,
            Feature::CoordinatorElected => Positive,
            Feature::DirectiveIssued => Positive,
            // 382: regression canary — firing means placement is broken.
            Feature::DirectiveStuckOnPlacement => Negative,
            // 382: positive observability — a new ConstructionSite spawned.
            Feature::ConstructionSiteSpawned => Positive,
            Feature::DirectiveDelivered => Positive,
            Feature::BuildingConstructed => Positive,
            Feature::BuildingTidied => Positive,
            Feature::GateProcessed => Positive,
            Feature::PreyDenFounded => Positive,
            Feature::KnowledgePromoted => Positive,
            Feature::SpiritCommunion => Positive,
            Feature::StorageUpgraded => Positive,
            Feature::ItemRetrieved => Positive,
            Feature::HerbsDeposited => Positive,
            Feature::HerbsRetrieved => Positive,
            Feature::FoodCooked => Positive,
            // 367: preservation pipeline. Loads + tends are positive
            // intermediates (cat actually engaged a station); the three
            // completion features are positive on the colony's winter
            // buffer growing by one preserved item.
            Feature::FoodLoadedOnDryingRack => Positive,
            Feature::MeatLoadedOnSmokingRack => Positive,
            Feature::SmokingRackTended => Positive,
            Feature::FoodDried => Positive,
            Feature::MeatSmoked => Positive,
            Feature::OrganPreserved => Positive,
            Feature::KittenBorn => Positive,
            Feature::GestationAdvanced => Positive,
            Feature::KittenMatured => Positive,
            Feature::MatingOccurred => Positive,
            Feature::KittenFed => Positive,
            Feature::CropTended => Positive,
            Feature::CropHarvested => Positive,
            Feature::CarcassCleansed => Positive,
            Feature::CarcassHarvested => Positive,
            Feature::GatherHerbCompleted => Positive,
            Feature::WardPlaced => Positive,
            Feature::ScryCompleted => Positive,
            Feature::CleanseCompleted => Positive,
            Feature::ShadowFoxBanished => Positive,
            // Old-age death matches the existing `deaths_old_age_bonus`
            // convention in `achievement_points` — a life well lived.
            Feature::DeathOldAge => Positive,
            // Defensive wins against corruption / shadowfoxes.
            Feature::CorruptionPushback => Positive,
            Feature::ShadowFoxAvoidedWard => Positive,
            Feature::ShadowFoxAvoidedCatScent => Positive,
            // Ticket 023 Phase A — dissolution via sustained cleansing
            // is the slow-environmental defeat path. Counts as a
            // colony-defensive win, same valence class as `ShadowFoxBanished`.
            Feature::ShadowFoxDissolved => Positive,
            // §Phase 5a silent-advance audit — healthy-subsystem activity
            Feature::FoodEaten => Positive,
            Feature::Socialized => Positive,
            Feature::GroomedOther => Positive,
            Feature::MentoredCat => Positive,
            Feature::ThreatEngaged => Positive,
            Feature::MaterialsDelivered => Positive,
            Feature::MaterialPickedUp => Positive,
            Feature::BuildingRepaired => Positive,
            Feature::CourtshipInteraction => Positive,
            // Ticket 127 — JointIntention positive valences (subsumes
            // §7.M L2 PairingActivity from ticket 027b).
            Feature::JointIntentionEmitted { .. } => Positive,
            Feature::JointBiasApplied { .. } => Positive,
            Feature::JointStageAdvanced { .. } => Positive,
            // Ticket 104 — Hide/Freeze valence
            Feature::HideFreezeFired => Positive,
            // Ticket 025 Phase 2 — hawk/snake GOAP positives. All ten
            // ship as Positive so the never-fired canary can promote
            // the four trunks (`HawkSpottedPrey`, `HawkDiveLanded`,
            // `SnakeStruckPrey`, `SnakeAmbushed`) once the cutover soak
            // observes them firing (see `expected_to_fire_per_soak`).
            Feature::HawkSpottedPrey => Positive,
            Feature::HawkDiveLanded => Positive,
            Feature::HawkPerched => Positive,
            Feature::HawkFled => Positive,
            Feature::HawkDied => Positive,
            Feature::SnakeStruckPrey => Positive,
            Feature::SnakeBasked => Positive,
            Feature::SnakeAmbushed => Positive,
            Feature::SnakeRetreated => Positive,
            Feature::SnakeDied => Positive,
            // Ticket 149 — discrete hunt attempts (kill or lost) are
            // healthy-colony activity; the never-fired canary catches a
            // dead hunting pipeline.
            Feature::HuntAttempted => Positive,

            // 230: Fleeing chain completions. Both Positive — they
            // signal the substrate-aware Fleeing path is firing
            // (and, in the case of FleeRecovered, completing) instead
            // of the legacy thrash loop.
            Feature::FleeTargetPicked => Positive,
            Feature::FleeRecovered => Positive,

            // 126: BDI intention lifecycle. Adoption + fulfilment
            // are colony-positive (cats are committing and closing
            // goal cycles); abandon is Neutral (drops are state
            // transitions, the per-cause classification lives on the
            // trace not the activation counter).
            Feature::IntentionAdopted => Positive,
            Feature::IntentionFulfilled => Positive,
            Feature::IntentionAbandoned => Neutral,

            // 320: HTN method-stack lifecycle. Adoption + sub-goal
            // advance are Positive (substrate-driven multi-step
            // progress); backtrack + depth-exceeded are Neutral
            // (state transitions / authoring-loop canary).
            Feature::MethodAdopted => Positive,
            Feature::SubGoalAdvanced => Positive,
            Feature::MethodBacktracked => Neutral,
            Feature::MethodDepthExceeded => Neutral,

            // 332/333: HTN method primitive completions. All Positive
            // — substrate-driven grief-processing and rearing-arc
            // progress are colony wins. Each ships dormant via
            // `expected_to_fire_per_soak() => false` until the
            // dispatch follow-on lands.
            Feature::VigilHeld => Positive,
            Feature::GriefProcessed => Positive,
            Feature::GriefReleased => Positive,
            Feature::KittenWeaned => Positive,
            Feature::SkillTaught => Positive,
            Feature::KittenReleased => Positive,
            // 375: prey-byproduct producer canary.
            Feature::ByproductSpawned => Positive,
            // 368: Phase 2 behavioral-tool canaries.
            Feature::GroomingBrushUsed => Positive,
            Feature::PlayBundleEngaged => Positive,
            Feature::CourtshipGiftOffered => Positive,
            // 457: Workshop-craft elect-side first-light canary.
            Feature::ItemCrafted => Positive,
            // 450: kitten begs for food.
            Feature::KittenBegged => Positive,

            // 176: inventory-disposal completions are state-transition
            // signals — neither a colony win nor a loss, just "the cat
            // moved an item." Neutral keeps them out of the
            // positive/negative balance scoring while still surfacing
            // them in the activation footer for the never-fired canary.
            Feature::ItemDropped => Neutral,
            Feature::ItemTrashed => Neutral,
            Feature::ItemHandedOff => Neutral,
            // 250: burial demoted from Positive to Neutral. Caring for
            // the dead is conditional on death occurring; post-247 /
            // 248 the substrate keeps colonies healthy enough that
            // burial is genuinely rare. Treating zero burials as a
            // never-fired-canary defect produced false `verdict: fail`
            // results across post-246 / 247 / 248 baselines. Neutral
            // valence + `expected_to_fire_per_soak() => false` (below)
            // demotes the canary while preserving the footer tally.
            Feature::BurialPerformed => Neutral,
            // 477: bone-weapon snap is mechanical wear texture — surfaces
            // the durability substrate, neither a colony win nor loss.
            Feature::BoneWeaponSnapped => Neutral,
            // 334: deliberate don/swap of a worn item — surfaces the
            // WearItem don path; a colony-equipping behavior, not adverse.
            Feature::ItemWorn => Positive,
            // 429: items-are-real Source/Sink gates. All Positive — the
            // colony's perspective on items entering the world or
            // satisfying hunger is a win, distinct from the existing
            // adverse-event Features (Negative `DenRaided` for the
            // prey-population loss; Negative `OverflowToGround` for
            // chronic inventory-full anomaly).
            Feature::ItemSourcedFromDenRaid => Positive,
            Feature::ItemSourcedFromHuntCatch => Positive,
            Feature::ItemSourcedFromForageCatch => Positive,
            Feature::EatFromOwnInventory => Positive,
            // 482: three more Source gates — all Positive (item-into-world
            // wins; the `OverflowToGround` Negative anomaly canary fires
            // separately when the inventory-first arm overflows).
            Feature::ItemSourcedFromPreservation => Positive,
            Feature::ItemSourcedFromHarvestCarcass => Positive,
            Feature::ItemSourcedFromForageIngredient => Positive,

            // --- Negative: adverse events, colony loss signals ---
            Feature::DeathStarvation => Negative,
            Feature::DeathInjury => Negative,
            Feature::CorruptionSpread => Negative,
            Feature::CorruptionTileEffect => Negative,
            Feature::EnvironmentalComfortPositive => Positive,
            Feature::EnvironmentalComfortNegative => Negative,
            Feature::CorruptionHealthDrain => Negative,
            Feature::PersonalCorruptionEffect => Negative,
            Feature::ShadowFoxSpawn => Negative,
            Feature::WardDecay => Negative,
            Feature::WardDespawned => Negative,
            Feature::WardSiegeStarted => Negative,
            Feature::HerbSuppressed => Negative,
            Feature::AnxietyInterrupt => Negative,
            Feature::ModifierPreemption => Negative,
            Feature::AspirationAbandoned => Negative,
            Feature::AspirationDriftAbandoned => Negative,
            Feature::DenRaided => Negative,
            Feature::PreyDenAbandoned => Negative,
            Feature::DepositRejected => Negative,
            // 176: catches that overflow inventory and end up on the
            // ground. Anomaly signal — chronic counts mean disposal
            // DSEs aren't keeping up with intake. Item entities still
            // exist (items-are-real); only the colony's inventory
            // efficiency suffers.
            Feature::OverflowToGround => Negative,
            Feature::RouteCostFieldFallback => Negative,
            Feature::DepositFailedNoStore => Negative,
            Feature::PosseCandidateExcludedStarving => Negative,
            Feature::KnowledgeForgotten => Negative,
            Feature::FoxStoreRaided => Negative,

            // --- Neutral: system activity, no inherent valence ---
            Feature::HerbSeasonalCheck => Neutral,
            Feature::CombatResolved => Neutral,
            Feature::MoodContagion => Neutral,
            Feature::PersonalityFriction => Neutral,
            Feature::PreyBred => Neutral,
            Feature::WildlifeSpawned => Neutral,
            Feature::CarcassSpawned => Neutral,
            Feature::FoxHuntedPrey => Neutral,
            Feature::FoxStandoff => Neutral,
            Feature::FoxStandoffEscalated => Neutral,
            Feature::FoxRetreated => Neutral,
            Feature::FoxDenEstablished => Neutral,
            Feature::FoxBred => Neutral,
            Feature::FoxCubMatured => Neutral,
            Feature::FoxDied => Neutral,
            Feature::FoxScentMarked => Neutral,
            Feature::FoxAvoidedCat => Neutral,
            Feature::FoxDenDefense => Neutral,
            Feature::FoxAvoidedWard => Neutral,
            Feature::FoxAvoidedPresence => Neutral,
            // Ticket 023 Phase B — shadow-fox motivation state entries
            // are observability signals: state transitions, not harm
            // events. The harm (corruption deposit / mood drain) is
            // tallied separately by its own per-tick emitters in Phase C.
            Feature::ShadowFoxReconstitutingEntered => Neutral,
            Feature::ShadowFoxTendingEntered => Neutral,
            Feature::ShadowFoxHauntingEntered => Neutral,
            Feature::ShadowFoxSeedingEntered => Neutral,
            // Ticket 023 Phase C — haunting-drain cadenced emission and
            // escalation-to-Stalking. Neutral: the actual harm is the
            // cat's Mood/Safety drain (visible in welfare metrics);
            // these Features are activation-footer breadcrumbs so
            // `negative_events_total` doesn't get ballooned by ambient
            // psychological pressure.
            Feature::ShadowFoxHaunting => Neutral,
            Feature::ShadowFoxHauntingEscalated => Neutral,
            // 310 S1 — hunger-elected Stalking is a state transition;
            // the harm, if any, is the downstream Ambush event.
            Feature::ShadowFoxHungerHuntEntered => Neutral,
            // 310 S2 — retreat entry is a state transition.
            Feature::ShadowFoxRetreatEntered => Neutral,
            Feature::CommitmentDropTriggered => Neutral,
            Feature::CommitmentDropBlind => Neutral,
            Feature::CommitmentDropSingleMinded => Neutral,
            Feature::CommitmentDropOpenMinded => Neutral,
            Feature::CommitmentDropReplanCap => Neutral,
            Feature::CommitmentDropMoraleBreak => Neutral,
            // §7.M L2 PairingActivity drop is a state transition,
            // not an adverse event.
            // Ticket 127 — JointIntention neutral valences. Drops are
            // bursty state transitions (mirrors the prior
            // PairingDropped which 127 subsumes). Stage mismatch is
            // healthy narrative texture (codified irony), not a
            // regression signal.
            Feature::JointIntentionDropped { .. } => Neutral,
            Feature::JointStageMismatchTickAccrued { .. } => Neutral,
            // Ticket 080 — reservation contention is observability
            // signal, not adverse.
            Feature::ReservationContended => Neutral,
            // Ticket 073 — recently-failed-target cooldown penalty fired
            // for at least one candidate. State-tracking signal, not
            // adverse.
            Feature::TargetCooldownApplied => Neutral,
        }
    }

    /// Whether a healthy canonical soak (seed 42, 900s release) is
    /// *expected* to fire this feature at least once.
    ///
    /// Used by the "never-fired-but-expected" canary introduced in
    /// §Phase 5a to catch silently-dead subsystems: a Positive
    /// feature returning `true` here must appear in the footer with
    /// `count >= 1`, otherwise the canary fails.
    ///
    /// Features marked `false` are legitimately rare — colony-
    /// defining events (a banishment, a shadow-fox ward save, a
    /// fated awakening) that may not occur in every soak. They're
    /// excluded from the canary rather than treated as dead.
    pub const fn expected_to_fire_per_soak(self) -> bool {
        match self {
            // --- Rare legend / colony-defining events — exempt ---
            Feature::ShadowFoxBanished => false,
            // Ticket 023 Phase A — dissolution depends on sustained
            // colony cleansing pressure, which a healthy 15-min soak
            // may not produce. Pair with `ShadowFoxBanished` in the
            // rare-legend exemption set.
            Feature::ShadowFoxDissolved => false,
            // Ticket 023 Phase B — motivation-state entries depend on
            // the shadow-fox population and corruption substrate; exempt
            // by category (Neutral features don't gate the canary), but
            // listed here for documentation and easy promotion later.
            Feature::ShadowFoxReconstitutingEntered => false,
            Feature::ShadowFoxTendingEntered => false,
            Feature::ShadowFoxHauntingEntered => false,
            Feature::ShadowFoxSeedingEntered => false,
            // Ticket 023 Phase C — haunting drain + escalation. Cadenced
            // emission means firing depends on a Haunting shadow-fox
            // being persistently in drain-radius of a cat; a healthy
            // soak may not produce this if the colony's defenses keep
            // shadow-foxes off vulnerable targets.
            Feature::ShadowFoxHaunting => false,
            Feature::ShadowFoxHauntingEscalated => false,
            // 310 S1 — new-Feature default per the 1:1 sibling rule:
            // false until a seed-42 soak observes ≥1 firing (hunger
            // wins only when satiation has decayed with a cat in scan
            // radius and no stronger drive pressured — plausibly rare
            // per soak at first-light weight 0.10).
            Feature::ShadowFoxHungerHuntEntered => false,
            // 310 S2 — fires only when an ambush lands (rare per soak
            // at the satiation-gated cadence); the
            // shadowfox_hunger_hunt_cycle scenario hosts the assertion.
            Feature::ShadowFoxRetreatEntered => false,
            Feature::FateAwakened => false,
            Feature::SpiritCommunion => false,
            Feature::ShadowFoxAvoidedWard => false,
            // 260: shadow-fox cat-scent avoidance — depends on
            // colony scent gradient overlap with ShadowFox patrol
            // route. Same exemption logic as `ShadowFoxAvoidedWard`
            // and `FoxAvoidedPresence`: structural verification lives
            // in the `fox_cat_scent_avoidance` scenario, not the
            // seed-42 canary.
            Feature::ShadowFoxAvoidedCatScent => false,
            Feature::ShadowFoxSpawn => false,
            Feature::WardSiegeStarted => false,
            Feature::DeathOldAge => false,
            Feature::AspirationCompleted => false,
            Feature::AspirationAbandoned => false,
            Feature::AspirationDriftAbandoned => false,
            // Fox-ecology ambient signals that depend on world state
            // (a fox may or may not spawn / be in range in 15 min).
            Feature::FoxStoreRaided => false,
            Feature::FoxStandoffEscalated => false,
            Feature::FoxRetreated => false,
            Feature::FoxDenEstablished => false,
            Feature::FoxBred => false,
            Feature::FoxCubMatured => false,
            Feature::FoxDied => false,
            Feature::FoxAvoidedWard => false,
            Feature::FoxAvoidedPresence => false,
            Feature::FoxDenDefense => false,
            // Corruption-specific: may not fire in a clean run.
            Feature::CarcassSpawned => false,
            Feature::CarcassCleansed => false,
            Feature::CarcassHarvested => false,
            Feature::CorruptionPushback => false,
            Feature::HerbSuppressed => false,
            Feature::CorruptionHealthDrain => false,
            Feature::PersonalCorruptionEffect => false,
            Feature::RemedyApplied => false,
            Feature::RemedyPrepared => false,
            Feature::InjuryHealed => false,
            // Mirrors `InjuryHealed` / `CombatResolved`: the seed-42
            // baseline currently has zero cat-vs-wildlife combat firings
            // (wards + flee logic keep cats out of damage range), so
            // BodyPartInjury isn't gated either. The substrate is wired
            // and verifiable via direct event-stream inspection
            // (`just q events ... BodyPartInjury`) when combat happens —
            // the never-fired canary doesn't add diagnostic value here.
            Feature::BodyPartInjury => false,
            // Ticket 382: regression canary. Healthy colonies should
            // never see `compute_building_placement` fail repeatedly;
            // firing means the influence-map composition is wrong.
            Feature::DirectiveStuckOnPlacement => false,
            // Misc rare events.
            Feature::PosseCandidateExcludedStarving => false,
            Feature::DepositFailedNoStore => false,
            Feature::GateProcessed => false,
            // Magic that requires specific actor priors.
            Feature::ScryCompleted => false,
            Feature::CleanseCompleted => false,
            Feature::WardPlaced => false,
            Feature::WardDespawned => false,
            // Building/craft that depends on plan cadence.
            Feature::BuildingRepaired => false,
            Feature::BuildingTidied => false,
            Feature::StorageUpgraded => false,
            // Ticket 027 Bug 1 wired this to `social::check_bonds`'s
            // courtship-drift gate, so it now fires whenever any
            // compatible adult pair drifts. Promoted out of the
            // rare-legend list to gate the never-fired canary.
            // §Phase 5a empirical calibration (seed-42 soak): these
            // are exempt because they depend on conditions the sim
            // can't guarantee in 15 min of wall-clock:
            //
            // - PreyDenFounded: multi-day event, rare in 15 min.
            // - KittenMatured: maturation takes sim-days; depends on
            //   kitten survival + enough sim-time to cross the threshold.
            //   Also a cascade dependency on `KittenBorn` firing first.
            // - ThreatEngaged: requires wildlife in range + surviving
            //   to fight_duration without morale break.
            // - BuildingRepaired: routing gap — the GOAP path
            //   internalises repair without emitting the Feature, so
            //   the legacy disposition-chain path is the only emitter.
            //   Tracked separately (no current ticket).
            //
            // - MaterialsDelivered / MaterialPickedUp: ticket 038 wired
            //   the full Pickup → Carry → Deliver pipeline (planner +
            //   step resolvers + executor dispatch) and gated the
            //   founding wagon-dismantling spawn behind the
            //   CLOWDER_FOUNDING_HAUL env var. The founding-spawn
            //   activation work (originally ticket 041) is retired as
            //   obviated — the early-game starvation regression that
            //   blocked it no longer reproduces, and no other producer
            //   of build-material piles exists in the current world-gen,
            //   so both Features stay demoted by default. Open a fresh
            //   ticket if/when a build-material producer ships.
            //
            // The three "trunk" Features `MatingOccurred`,
            // `GroomedOther`, `MentoredCat` deliberately stay in the
            // expected set even though they fire at zero in current
            // soaks — the canary flagging them RED is accurate and
            // tracks load-bearing tickets:
            // - GroomedOther → ticket 037 (silent-advance via GroomingFired)
            // - MentoredCat  → known mastery-decay dynamic
            // - MatingOccurred → ticket 027 (mating cadence cascade)
            //
            // `FoodCooked` was a fourth trunk Feature (tickets 036 + 039)
            // and is now firing; canary green.
            //
            // Cascade-exempt: each entry below is silent strictly
            // because its trunk Feature is silent. Listing them as
            // `expected_to_fire_per_soak()` would multiply a single
            // root-cause failure into N canary entries; demoting them
            // to `false` keeps the canary signal one-per-trunk. When
            // the trunk's ticket lands, these will start firing and
            // can be promoted back to `true` if you want to track
            // them as independent canaries.
            // - GestationAdvanced / KittenBorn / KittenFed: cascade
            //   from MatingOccurred (ticket 027).
            // - ItemRetrieved: cascade from FoodCooked. FoodCooked
            //   (036 + 039) is now green, so this cascade-exempt
            //   demotion may be promotable on the next sweep — left
            //   `false` here pending a fresh soak that confirms
            //   ItemRetrieved fires reliably.
            Feature::PreyDenFounded => false,
            Feature::KittenMatured => false,
            Feature::ThreatEngaged => false,
            // Ticket 038 — parked behind CLOWDER_FOUNDING_HAUL. See
            // block comment above.
            Feature::MaterialsDelivered => false,
            Feature::MaterialPickedUp => false,
            // Cascade-exempt (see block comment above).
            Feature::GestationAdvanced => false,
            Feature::KittenBorn => false,
            Feature::KittenFed => false,
            Feature::ItemRetrieved => false,
            // Ticket 418 fix (2026-05-19, post-soak verification):
            // CanWardFromSupply marker-snapshot population gap closed
            // → HerbcraftSetWard eligibility now passes → retrieve-path
            // elects naturally. Verification soak (logs/tuned-42 vs
            // pre-418-fix baseline): HerbsRetrieved 0 → 18,
            // WardPlaced 2 → 21 (vs pre-084 baseline 5). Promoted.
            Feature::HerbsDeposited => true,
            Feature::HerbsRetrieved => true,
            // Ticket 083 demoted CropTended / CropHarvested because
            // Farm DSE correctly gated dormant under abundant food.
            // Ticket 084 Commit 3 tied Farm to the
            // `ColonyThornbriarChronicallyLow` chronicity marker; the
            // 418-fix verification soak (2026-05-19, logs/tuned-42)
            // observed CropTended 4183, CropHarvested 44 — the
            // herb-pressure axis correctly lifts Farm under chronic
            // ward-stockpile scarcity, even with food stockpiles full.
            // Re-promoted both: the silent-dead farming pipeline is
            // genuinely behind us.
            Feature::CropTended => true,
            Feature::CropHarvested => true,
            // Ticket 080 — `ReservationContended` is exempt until the
            // producer side (`record_target_picked` writes) ships.
            Feature::ReservationContended => false,
            // Ticket 073 — Neutral feature; promotion to canary waits
            // for empirical baseline data.
            Feature::TargetCooldownApplied => false,
            // Ticket 104 — Hide/Freeze valence is rare-event class
            // and Phase-1 dormant (HideEligible never authored).
            // Promote when the activation system + lift land and
            // canonical seed-42 produces ≥1 freeze per soak.
            Feature::HideFreezeFired => false,
            // 176: inventory-disposal Features ship dormant via
            // default-zero scoring on the disposal DSEs. Promote to
            // `true` only after balance-tuning lifts the saturation
            // surfaces so seed-42 produces ≥1 of each per soak.
            // OverflowToGround stays opt-in — chronic counts are an
            // anomaly signal, but in a balanced colony it should be
            // near-zero so the canary would false-fail.
            Feature::ItemDropped => false,
            Feature::ItemTrashed => false,
            Feature::ItemHandedOff => false,
            Feature::OverflowToGround => false,
            // 228: should rarely fire in a healthy soak (every replan
            // rebuilds the field). Chronic counts indicate field
            // staleness or build-correctness bugs; the canary is the
            // tripwire that surfaces them.
            Feature::RouteCostFieldFallback => false,
            // 230: Fleeing chain Features. Cascade-from-rare-event:
            // `ThreatProximityAdrenalineFlee` should rarely lift Flee
            // to win selection on a healthy colony, so the trip
            // signals stay opt-in. (Pre-251 also
            // `AcuteHealthAdrenalineFlee` lifted Flee on injury — 251
            // retired that lift but Flee adoption was already
            // statistical-zero in seed-42 healthy soaks.) Promote to
            // `true` after the post-230 multi-seed baseline shows
            // reliable firing.
            Feature::FleeTargetPicked => false,
            Feature::FleeRecovered => false,
            // 126: BDI abandon is bursty (per `PairingDropped`'s
            // precedent). Adoption + fulfilment fall through to the
            // `_ => true` default and are canary-validated.
            Feature::IntentionAbandoned => false,
            // 320 / 321: HTN method-stack Features. 320 introduced
            // them as `false` (registry held no Live methods); 321's
            // combine-and-test slice registers `hunt_method` Live +
            // adds an Emit row on Hunting "First Blood" that points
            // at it. Every cat that adopts the Hunting chain at
            // milestone 0 emits `Intention::Goal { hunt_prey }` each
            // tick → L2 author wraps as Goal → 320's HTN frame-push
            // gate catches → `MethodAdopted` fires. Promote it to
            // `true` so the canary catches a regression where the
            // wiring breaks.
            //
            // `SubGoalAdvanced` stays `false`: 320's existing
            // `resolve_goap_plans` lifecycle clears the entire stack
            // on intention end rather than walking `sub_goal_index`
            // forward. The picker also currently emits no method
            // with > 1 primitive sub-goal at 321 land, so even with
            // per-frame advance wired the Feature would never trip.
            // 323's `courtship_method` (4 sub-goals) is the first
            // ticket that actually exercises advance; promote there.
            //
            // `MethodBacktracked` and `MethodDepthExceeded` stay
            // `false` — backtrack requires sibling methods sharing a
            // goal_label (none at 321 land) and depth-exceeded is
            // an authoring-loop canary (rare-event class).
            Feature::MethodAdopted => true,
            Feature::SubGoalAdvanced => false,
            Feature::MethodBacktracked => false,
            Feature::MethodDepthExceeded => false,
            // 332/333: HTN method primitive completions. All ship
            // dormant — the methods register Live in #332/#333 but
            // `chosen_action` dispatch isn't wired yet (the HTN
            // substrate doesn't override the L2 evaluator's softmax
            // winner). The dispatch follow-on (named in each ticket's
            // landing Log) flips these to `true` once the resolvers
            // can actually fire under realistic Mourning insertion +
            // KittenDependency presence.
            Feature::VigilHeld => false,
            Feature::GriefProcessed => false,
            Feature::GriefReleased => false,
            Feature::KittenWeaned => false,
            Feature::SkillTaught => false,
            Feature::KittenReleased => false,
            // 375: every successful hunt emits ≥1 byproduct; a healthy
            // seed-42 soak runs dozens of hunts. Zero count = silent
            // producer dormancy class (the canary's purpose).
            Feature::ByproductSpawned => true,
            // 368: behavioral-tool canaries ship dormant. The
            // Workshop-craft pipeline follow-on flips these to
            // `true` once cats craft + circulate the tools in
            // seed-42. Until then, no cat carries a brush / bundle
            // / gift in a healthy soak, so the canary's never-fired
            // tripwire would fail spuriously. The resolver branches
            // and emission sites ARE wired so scenario tests that
            // hand-place a tool exercise the canary.
            Feature::GroomingBrushUsed => false,
            Feature::PlayBundleEngaged => false,
            Feature::CourtshipGiftOffered => false,
            // 457: Workshop-craft first-light canary. expected = true
            // — the elect-side pipeline must fire ≥1 craft in seed-42
            // to satisfy the 368 substrate first-light gate. Zero
            // crafts = wiring broken (DSE not scoring, marker not
            // authored, plan not forming, or resolver not dispatched).
            Feature::ItemCrafted => true,
            // 450 + 451: kitten begging canary. Substrate lifted in 451
            // (kittens enter L2 scoring via the life-stage gate, the
            // BegForFood DSE has Newborn/EyesOpen/Incapacitated siblings,
            // the resolver fires this Feature on cycle completion). But
            // the seed-42 verification soak revealed BegForFood doesn't
            // win L3 in practice: adults preempt kitten hunger via the
            // direct-kitten-targeting Caretake path (`FeedKitten` fires
            // 4355× in 85k ticks) and keep hunger above the begging
            // threshold (hangry curve midpoint 0.5). Promoting the
            // canary requires balance tuning (raise threshold, or steepen
            // the hangry curve so kittens elect earlier) — parked for a
            // follow-on ticket.
            Feature::KittenBegged => false,
            // Ticket 127 — JointIntention: drops are bursty (mirrors
            // `PairingDropped`); stage mismatch is healthy-sometimes-
            // zero. Both stay opt-out. Emitted / BiasApplied /
            // StageAdvanced fall through to `_ => true`.
            Feature::JointIntentionDropped { .. } => false,
            Feature::JointStageMismatchTickAccrued { .. } => false,
            // 250: burial is conditional on death. Post-247 / 248 the
            // substrate keeps colonies healthy enough that deaths
            // (and therefore burials) are genuinely rare; treating
            // zero burials as a never-fired-canary defect produced
            // false `verdict: fail` results. Demoted alongside the
            // valence change above. Footer tally still emits when
            // burials happen.
            Feature::BurialPerformed => false,
            // 288: morale_break-driven commitment release. Rare-event
            // class (requires a wounded cat to reach EngageThreat with
            // HP below `fight_bail_health_threshold`). Promote to
            // `true` after the post-288 baseline shows reliable firing.
            Feature::CommitmentDropMoraleBreak => false,
            // Ticket 025 Phase 2 — hawk/snake GOAP positives ship
            // dormant. Commit 3 (this commit) registers the variants
            // but the resolvers don't fire yet because no entity carries
            // `HawkState` / `SnakeState`. Commit 6's cutover lands the
            // spawn-side attach + runs a soak; the four trunks
            // (`HawkSpottedPrey`, `HawkDiveLanded`, `SnakeStruckPrey`,
            // `SnakeAmbushed`) promote to `true` after the seed-42 soak
            // confirms each fires at least once. The other six stay
            // `false` permanently — they're state-dependent (perch /
            // flee / die / bask / retreat) and a healthy soak may not
            // observe them.
            Feature::HawkSpottedPrey => false,
            Feature::HawkDiveLanded => false,
            Feature::HawkPerched => false,
            Feature::HawkFled => false,
            Feature::HawkDied => false,
            Feature::SnakeStruckPrey => false,
            Feature::SnakeBasked => false,
            Feature::SnakeAmbushed => false,
            Feature::SnakeRetreated => false,
            Feature::SnakeDied => false,
            // 367: chain-rare completion. The organ pipeline is gated
            // by: hunt success → 30% organ drop → cat picks up → loads
            // drying rack → ~2 sim-day Clear-weather window. A 900s
            // soak may not produce all four links, so the canary stays
            // opt-out until balance verification shows reliable
            // firing. `FoodDried` (fish pipeline, no organ-drop gate)
            // falls through to `_ => true` — that's the hypothesis-
            // load-bearing completion that does fire in healthy soaks.
            Feature::OrganPreserved => false,
            // 444 / 446: the smoking-side triple retires from the per-
            // soak canary. 446's layer-walk verified the smoking chain
            // is structurally complete (DSE, dispatcher, registry, plan
            // template, resolvers, marker writers, item drops, canary
            // classification all symmetric to drying), but behaviorally
            // the meat-AND-fuel conjunction inside
            // `HasSmokeableAccessible` never resolves under healthy
            // seed-42: `SmokeMeat` never appears in any `last_scores`
            // table across the run. Drying-side `HasDryableAccessible`
            // is a fish-OR-organ disjunction which does resolve. Re-
            // enroll when a future substrate arc splits the smoking
            // conjunction into sequential retrievals (or adds a fuel-
            // acquisition DSE). Regression coverage for the smoking
            // pipeline migrates to a scenario harness preset under
            // ticket 447 (`just scenario smoking_chain_complete`).
            Feature::MeatLoadedOnSmokingRack => false,
            Feature::SmokingRackTended => false,
            Feature::MeatSmoked => false,
            // 477: bone-weapon snap needs a fragile bone weapon wielded
            // + a missed strike + the snap roll landing — too rare to
            // gate the seed-42 canary. The `equipment_bone_snap` scenario
            // exercises the branch deterministically.
            Feature::BoneWeaponSnapped => false,
            // 334: the self-craft happy path auto-equips the freshly-crafted
            // cloak on craft (017), so the WearItem don leaf is an idempotent
            // no-op-success; `ItemWorn` only fires on a swap (Cape slot
            // occupied at craft time), too rare to gate the seed-42 canary.
            Feature::ItemWorn => false,

            // -------------------------------------------------------------
            // Healthy-trunk Features: a canonical seed-42 soak fires each
            // at least once. Ticket 368 retired the prior `_ => true`
            // catch-all so new Feature variants must be classified
            // explicitly — the default-true was the silent-canary surface
            // a never-fired regression hid behind (the precedent the
            // `## Conventions` "Silent-canary surfaces are forbidden"
            // entry references).
            // -------------------------------------------------------------
            Feature::CorruptionSpread => true,
            Feature::CorruptionTileEffect => true,
            Feature::EnvironmentalComfortPositive => true,
            Feature::EnvironmentalComfortNegative => false,
            Feature::WardDecay => true,
            Feature::HerbSeasonalCheck => true,
            Feature::CombatResolved => true,
            Feature::FateAssigned => true,
            Feature::AspirationSelected => true,
            Feature::BondFormed => true,
            Feature::CoordinatorElected => true,
            Feature::DirectiveIssued => true,
            Feature::DirectiveDelivered => true,
            Feature::ConstructionSiteSpawned => true,
            Feature::BuildingConstructed => true,
            Feature::MoodContagion => true,
            Feature::PersonalityFriction => true,
            Feature::AnxietyInterrupt => true,
            Feature::ModifierPreemption => true,
            Feature::PreyBred => true,
            Feature::PreyDenAbandoned => true,
            Feature::DenRaided => true,
            Feature::WildlifeSpawned => true,
            Feature::DeathStarvation => true,
            Feature::DeathInjury => true,
            // 291's promotion is chain-rare: a 3-cat same-bucket belief
            // quorum fires ~once per 100–160k ticks — the same order as
            // a 900s soak window. Across the five 900s trajectory
            // families ending at 310 S1 it fired in two and zeroed in
            // three, so a per-soak never-fired gate is a coin flip that
            // costs a doubled-window re-soak every tails (step-21
            // gate 3, step-22, and 310 S1 all paid it). Demoted
            // 2026-07-09. Mechanism-break protection lives in the
            // `colony_knowledge_false_belief` scenario
            // (`expected_features: ["KnowledgePromoted"]`, forced
            // deterministically every `cargo test`) + 17 unit tests;
            // ecological-starvation protection is the release-plan
            // step-24 cadence watch-item checked at baseline
            // re-promotes.
            Feature::KnowledgePromoted => false,
            Feature::KnowledgeForgotten => true,
            Feature::DepositRejected => true,
            Feature::FoodCooked => true,
            Feature::FoodLoadedOnDryingRack => true,
            Feature::FoodDried => true,
            Feature::MatingOccurred => true,
            Feature::FoxHuntedPrey => true,
            Feature::FoxStandoff => true,
            Feature::FoxScentMarked => true,
            Feature::FoxAvoidedCat => true,
            Feature::GatherHerbCompleted => true,
            Feature::FoodEaten => true,
            Feature::Socialized => true,
            Feature::GroomedOther => true,
            Feature::MentoredCat => true,
            Feature::CourtshipInteraction => true,
            Feature::CommitmentDropTriggered => true,
            Feature::CommitmentDropBlind => true,
            Feature::CommitmentDropSingleMinded => true,
            Feature::CommitmentDropOpenMinded => true,
            Feature::CommitmentDropReplanCap => true,
            Feature::HuntAttempted => true,
            Feature::IntentionAdopted => true,
            Feature::IntentionFulfilled => true,
            Feature::JointIntentionEmitted { .. } => true,
            Feature::JointBiasApplied { .. } => true,
            Feature::JointStageAdvanced { .. } => true,
            // 429: items-are-real Source gates. Verified firing in the
            // 429 landing soak (seed-42, 900s) — DenRaid 39×,
            // HuntCatch 368×, ForageCatch 308×. All three are
            // load-bearing tripwires for the underlying Source
            // substrate; zero count = the trait dispatch broke.
            Feature::ItemSourcedFromDenRaid => true,
            Feature::ItemSourcedFromHuntCatch => true,
            Feature::ItemSourcedFromForageCatch => true,
            // 429: the eat-from-pocket Sink. The autonomic dispatcher
            // at `needs.rs::eat_from_inventory` fires only when a cat
            // has `hunger < eat_from_inventory_threshold` (0.4) AND
            // food in pouch — in a healthy seed-42 colony this rarely
            // happens (cats eat at Stores via `EatAtStores` before
            // hunger falls that low; kittens autoconsume after
            // they're old enough to carry food but the marker
            // pipeline doesn't reach that branch in the 900s window).
            // The 429 landing soak observed zero firings; the
            // `items_eat_from_own_inventory` scenario is the
            // structural witness instead. Re-enroll when a follow-on
            // lifts the Sink into L2/L3 election (so adults plan
            // through it deliberately) and the seed-42 soak observes
            // ≥1 firing per healthy run.
            Feature::EatFromOwnInventory => false,
            // 482: three more Source gates promoted from inline pushes.
            // Preservation completions + ForageIngredient drops fire
            // reliably in seed-42 (drying racks complete cycles every
            // few hundred ticks; forage tiles roll ingredient drops on
            // every Twig/Fiber/Flower-eligible patch). HarvestCarcass
            // is rare-by-design — it requires a magic-capable cat to
            // harvest a corruption-touched carcass, and the existing
            // `CarcassHarvested` gameplay-event sibling is enrolled
            // `false` for the same reason. The two are 1:1 by
            // construction, so they share classification: re-enroll
            // when CarcassHarvested gets promoted to `true`.
            Feature::ItemSourcedFromPreservation => true,
            Feature::ItemSourcedFromHarvestCarcass => false,
            Feature::ItemSourcedFromForageIngredient => true,
        }
    }
}

/// Stable lower-snake-ish names for `Feature::*` variants used in
/// diagnostic output (JSON-friendly, match the `serde` default of
/// variant-name-as-string). Used by the never-fired canary so the
/// offender list in the footer is human-readable rather than a
/// Debug dump.
pub fn feature_name(f: Feature) -> &'static str {
    // Mirror serde's default (PascalCase variant name). Exhaustive
    // match, so a new variant added without updating this table is a
    // compile error.
    match f {
        Feature::CorruptionSpread => "CorruptionSpread",
        Feature::CorruptionTileEffect => "CorruptionTileEffect",
        Feature::EnvironmentalComfortPositive => "EnvironmentalComfortPositive",
        Feature::EnvironmentalComfortNegative => "EnvironmentalComfortNegative",
        Feature::ShadowFoxSpawn => "ShadowFoxSpawn",
        Feature::WardDecay => "WardDecay",
        Feature::HerbSeasonalCheck => "HerbSeasonalCheck",
        Feature::RemedyApplied => "RemedyApplied",
        Feature::RemedyPrepared => "RemedyPrepared",
        Feature::PersonalCorruptionEffect => "PersonalCorruptionEffect",
        Feature::CombatResolved => "CombatResolved",
        Feature::InjuryHealed => "InjuryHealed",
        Feature::BodyPartInjury => "BodyPartInjury",
        Feature::FateAssigned => "FateAssigned",
        Feature::FateAwakened => "FateAwakened",
        Feature::AspirationSelected => "AspirationSelected",
        Feature::AspirationCompleted => "AspirationCompleted",
        Feature::AspirationAbandoned => "AspirationAbandoned",
        Feature::AspirationDriftAbandoned => "AspirationDriftAbandoned",
        Feature::BondFormed => "BondFormed",
        Feature::CoordinatorElected => "CoordinatorElected",
        Feature::DirectiveIssued => "DirectiveIssued",
        Feature::DirectiveStuckOnPlacement => "DirectiveStuckOnPlacement",
        Feature::ConstructionSiteSpawned => "ConstructionSiteSpawned",
        Feature::BuildingConstructed => "BuildingConstructed",
        Feature::BuildingTidied => "BuildingTidied",
        Feature::GateProcessed => "GateProcessed",
        Feature::MoodContagion => "MoodContagion",
        Feature::PersonalityFriction => "PersonalityFriction",
        Feature::AnxietyInterrupt => "AnxietyInterrupt",
        Feature::ModifierPreemption => "ModifierPreemption",
        Feature::PreyBred => "PreyBred",
        Feature::PreyDenAbandoned => "PreyDenAbandoned",
        Feature::PreyDenFounded => "PreyDenFounded",
        Feature::DenRaided => "DenRaided",
        Feature::WildlifeSpawned => "WildlifeSpawned",
        Feature::DeathStarvation => "DeathStarvation",
        Feature::DeathOldAge => "DeathOldAge",
        Feature::DeathInjury => "DeathInjury",
        Feature::KnowledgePromoted => "KnowledgePromoted",
        Feature::KnowledgeForgotten => "KnowledgeForgotten",
        Feature::SpiritCommunion => "SpiritCommunion",
        Feature::StorageUpgraded => "StorageUpgraded",
        Feature::DepositRejected => "DepositRejected",
        Feature::DepositFailedNoStore => "DepositFailedNoStore",
        Feature::ItemRetrieved => "ItemRetrieved",
        Feature::HerbsDeposited => "HerbsDeposited",
        Feature::HerbsRetrieved => "HerbsRetrieved",
        Feature::FoodCooked => "FoodCooked",
        // 367 preservation pipeline.
        Feature::FoodLoadedOnDryingRack => "FoodLoadedOnDryingRack",
        Feature::MeatLoadedOnSmokingRack => "MeatLoadedOnSmokingRack",
        Feature::SmokingRackTended => "SmokingRackTended",
        Feature::FoodDried => "FoodDried",
        Feature::MeatSmoked => "MeatSmoked",
        Feature::OrganPreserved => "OrganPreserved",
        Feature::KittenBorn => "KittenBorn",
        Feature::GestationAdvanced => "GestationAdvanced",
        Feature::KittenMatured => "KittenMatured",
        Feature::MatingOccurred => "MatingOccurred",
        Feature::KittenFed => "KittenFed",
        Feature::CropTended => "CropTended",
        Feature::CropHarvested => "CropHarvested",
        Feature::FoxHuntedPrey => "FoxHuntedPrey",
        Feature::FoxStoreRaided => "FoxStoreRaided",
        Feature::FoxStandoff => "FoxStandoff",
        Feature::FoxStandoffEscalated => "FoxStandoffEscalated",
        Feature::FoxRetreated => "FoxRetreated",
        Feature::FoxDenEstablished => "FoxDenEstablished",
        Feature::FoxBred => "FoxBred",
        Feature::FoxCubMatured => "FoxCubMatured",
        Feature::FoxDied => "FoxDied",
        Feature::FoxScentMarked => "FoxScentMarked",
        Feature::FoxAvoidedCat => "FoxAvoidedCat",
        Feature::FoxDenDefense => "FoxDenDefense",
        Feature::FoxAvoidedWard => "FoxAvoidedWard",
        Feature::FoxAvoidedPresence => "FoxAvoidedPresence",
        Feature::ShadowFoxAvoidedWard => "ShadowFoxAvoidedWard",
        Feature::ShadowFoxAvoidedCatScent => "ShadowFoxAvoidedCatScent",
        Feature::ShadowFoxDissolved => "ShadowFoxDissolved",
        Feature::ShadowFoxReconstitutingEntered => "ShadowFoxReconstitutingEntered",
        Feature::ShadowFoxTendingEntered => "ShadowFoxTendingEntered",
        Feature::ShadowFoxHauntingEntered => "ShadowFoxHauntingEntered",
        Feature::ShadowFoxSeedingEntered => "ShadowFoxSeedingEntered",
        Feature::ShadowFoxHaunting => "ShadowFoxHaunting",
        Feature::ShadowFoxHungerHuntEntered => "ShadowFoxHungerHuntEntered",
        Feature::ShadowFoxRetreatEntered => "ShadowFoxRetreatEntered",
        Feature::ShadowFoxHauntingEscalated => "ShadowFoxHauntingEscalated",
        Feature::DirectiveDelivered => "DirectiveDelivered",
        // Ticket 025 Phase 2 — hawk/snake GOAP.
        Feature::HawkSpottedPrey => "HawkSpottedPrey",
        Feature::HawkDiveLanded => "HawkDiveLanded",
        Feature::HawkPerched => "HawkPerched",
        Feature::HawkFled => "HawkFled",
        Feature::HawkDied => "HawkDied",
        Feature::SnakeStruckPrey => "SnakeStruckPrey",
        Feature::SnakeBasked => "SnakeBasked",
        Feature::SnakeAmbushed => "SnakeAmbushed",
        Feature::SnakeRetreated => "SnakeRetreated",
        Feature::SnakeDied => "SnakeDied",
        Feature::CarcassSpawned => "CarcassSpawned",
        Feature::WardSiegeStarted => "WardSiegeStarted",
        Feature::CarcassCleansed => "CarcassCleansed",
        Feature::CarcassHarvested => "CarcassHarvested",
        Feature::CorruptionPushback => "CorruptionPushback",
        Feature::HerbSuppressed => "HerbSuppressed",
        Feature::CorruptionHealthDrain => "CorruptionHealthDrain",
        Feature::GatherHerbCompleted => "GatherHerbCompleted",
        Feature::WardPlaced => "WardPlaced",
        Feature::WardDespawned => "WardDespawned",
        Feature::ScryCompleted => "ScryCompleted",
        Feature::CleanseCompleted => "CleanseCompleted",
        Feature::ShadowFoxBanished => "ShadowFoxBanished",
        Feature::PosseCandidateExcludedStarving => "PosseCandidateExcludedStarving",
        Feature::FoodEaten => "FoodEaten",
        Feature::Socialized => "Socialized",
        Feature::GroomedOther => "GroomedOther",
        Feature::MentoredCat => "MentoredCat",
        Feature::BurialPerformed => "BurialPerformed",
        Feature::ThreatEngaged => "ThreatEngaged",
        Feature::MaterialsDelivered => "MaterialsDelivered",
        Feature::MaterialPickedUp => "MaterialPickedUp",
        Feature::BuildingRepaired => "BuildingRepaired",
        Feature::CourtshipInteraction => "CourtshipInteraction",
        Feature::CommitmentDropTriggered => "CommitmentDropTriggered",
        Feature::CommitmentDropBlind => "CommitmentDropBlind",
        Feature::CommitmentDropSingleMinded => "CommitmentDropSingleMinded",
        Feature::CommitmentDropOpenMinded => "CommitmentDropOpenMinded",
        Feature::CommitmentDropReplanCap => "CommitmentDropReplanCap",
        Feature::CommitmentDropMoraleBreak => "CommitmentDropMoraleBreak",
        // Ticket 127 — JointIntention. Display names embed the practice
        // slug so the footer / canary output distinguishes per-practice
        // counters when future practices land.
        Feature::JointIntentionEmitted {
            practice: PracticeKind::Courtship,
        } => "JointIntentionEmitted_Courtship",
        Feature::JointIntentionDropped {
            practice: PracticeKind::Courtship,
        } => "JointIntentionDropped_Courtship",
        Feature::JointBiasApplied {
            practice: PracticeKind::Courtship,
        } => "JointBiasApplied_Courtship",
        Feature::JointStageAdvanced {
            practice: PracticeKind::Courtship,
        } => "JointStageAdvanced_Courtship",
        Feature::JointStageMismatchTickAccrued {
            practice: PracticeKind::Courtship,
        } => "JointStageMismatchTickAccrued_Courtship",
        // Ticket 276 — PlayBout practice display names. The footer
        // emits separate per-practice counters so PlayBout's emission /
        // bias / stage-advance / drop / mismatch counts are
        // independently observable from Courtship's.
        Feature::JointIntentionEmitted {
            practice: PracticeKind::PlayBout,
        } => "JointIntentionEmitted_PlayBout",
        Feature::JointIntentionDropped {
            practice: PracticeKind::PlayBout,
        } => "JointIntentionDropped_PlayBout",
        Feature::JointBiasApplied {
            practice: PracticeKind::PlayBout,
        } => "JointBiasApplied_PlayBout",
        Feature::JointStageAdvanced {
            practice: PracticeKind::PlayBout,
        } => "JointStageAdvanced_PlayBout",
        Feature::JointStageMismatchTickAccrued {
            practice: PracticeKind::PlayBout,
        } => "JointStageMismatchTickAccrued_PlayBout",
        Feature::ReservationContended => "ReservationContended",
        Feature::TargetCooldownApplied => "TargetCooldownApplied",
        Feature::HideFreezeFired => "HideFreezeFired",
        Feature::HuntAttempted => "HuntAttempted",
        Feature::ItemDropped => "ItemDropped",
        Feature::ItemTrashed => "ItemTrashed",
        Feature::ItemHandedOff => "ItemHandedOff",
        Feature::OverflowToGround => "OverflowToGround",
        Feature::RouteCostFieldFallback => "RouteCostFieldFallback",
        Feature::FleeTargetPicked => "FleeTargetPicked",
        Feature::FleeRecovered => "FleeRecovered",
        Feature::IntentionAdopted => "IntentionAdopted",
        Feature::IntentionFulfilled => "IntentionFulfilled",
        Feature::IntentionAbandoned => "IntentionAbandoned",
        // 320: HTN method-stack lifecycle.
        Feature::MethodAdopted => "MethodAdopted",
        Feature::SubGoalAdvanced => "SubGoalAdvanced",
        Feature::MethodBacktracked => "MethodBacktracked",
        Feature::MethodDepthExceeded => "MethodDepthExceeded",
        // 332/333: HTN method primitive completions.
        Feature::VigilHeld => "VigilHeld",
        Feature::GriefProcessed => "GriefProcessed",
        Feature::GriefReleased => "GriefReleased",
        Feature::KittenWeaned => "KittenWeaned",
        Feature::SkillTaught => "SkillTaught",
        Feature::KittenReleased => "KittenReleased",
        // 375: prey-byproduct producer canary.
        Feature::ByproductSpawned => "ByproductSpawned",
        // 368: Phase 2 behavioral-tool canaries.
        Feature::GroomingBrushUsed => "GroomingBrushUsed",
        Feature::PlayBundleEngaged => "PlayBundleEngaged",
        Feature::CourtshipGiftOffered => "CourtshipGiftOffered",
        Feature::ItemCrafted => "ItemCrafted",
        // 450: kitten begs for food canary.
        Feature::KittenBegged => "KittenBegged",
        // 477: bone-weapon durability snap.
        Feature::BoneWeaponSnapped => "BoneWeaponSnapped",
        // 334: deliberate don/swap of a worn item.
        Feature::ItemWorn => "ItemWorn",
        // 429: items-are-real Source/Sink gate Features.
        Feature::ItemSourcedFromDenRaid => "ItemSourcedFromDenRaid",
        Feature::ItemSourcedFromHuntCatch => "ItemSourcedFromHuntCatch",
        Feature::ItemSourcedFromForageCatch => "ItemSourcedFromForageCatch",
        Feature::EatFromOwnInventory => "EatFromOwnInventory",
        // 482
        Feature::ItemSourcedFromPreservation => "ItemSourcedFromPreservation",
        Feature::ItemSourcedFromHarvestCarcass => "ItemSourcedFromHarvestCarcass",
        Feature::ItemSourcedFromForageIngredient => "ItemSourcedFromForageIngredient",
    }
}

// ---------------------------------------------------------------------------
// SystemActivation
// ---------------------------------------------------------------------------

/// Tracks how many times each simulation feature meaningfully fires.
#[derive(Resource, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SystemActivation {
    /// Per-feature firing counts. Stored as a `BTreeMap` (not `HashMap`) so
    /// (1) `activation_score_in`'s f64 sum is associative across processes —
    /// `HashMap` iteration order varies with `RandomState`'s per-process seed,
    /// and float addition is non-associative, so a `HashMap` here produced
    /// 1-ULP drift in `positive_activation_score` on otherwise-identical seed-42
    /// runs; (2) the JSON serialization order in events.jsonl is stable, which
    /// `just verdict` and the determinism test assume.
    pub counts: BTreeMap<Feature, u64>,
}

impl SystemActivation {
    /// Record one firing of a feature.
    pub fn record(&mut self, feature: Feature) {
        *self.counts.entry(feature).or_insert(0) += 1;
    }

    /// Features that have never fired.
    pub fn dead_features(&self) -> Vec<Feature> {
        Feature::ALL
            .iter()
            .filter(|f| self.counts.get(f).copied().unwrap_or(0) == 0)
            .copied()
            .collect()
    }

    /// §Phase 5a never-fired canary: Positive features that a
    /// canonical soak is expected to fire at least once, but
    /// didn't. Returns the names as strings for direct JSON-
    /// friendly emission in the footer.
    ///
    /// A non-empty list means a silently-dead subsystem — the
    /// exact failure mode that kept the farming bug (CropTended
    /// never firing) invisible for months before §Phase 4c.4.
    pub fn never_fired_expected_positives(&self) -> Vec<&'static str> {
        Feature::ALL
            .iter()
            .filter(|f| {
                f.category() == FeatureCategory::Positive
                    && f.expected_to_fire_per_soak()
                    && self.counts.get(f).copied().unwrap_or(0) == 0
            })
            .map(|f| feature_name(*f))
            .collect()
    }

    /// Number of distinct features that have fired at least once.
    pub fn features_active(&self) -> u32 {
        self.counts.values().filter(|&&c| c > 0).count() as u32
    }

    /// Activation score restricted to a single feature category.
    ///
    /// For `Positive`, this is the main colony-thriving signal. For other
    /// categories the score is still computable but less meaningful — prefer
    /// `negative_event_count` for negative-valence features.
    pub fn activation_score_in(
        &self,
        category: FeatureCategory,
        breadth_bonus: f64,
        depth_bonus: f64,
    ) -> f64 {
        self.counts
            .iter()
            .filter(|(feature, &count)| count > 0 && feature.category() == category)
            .map(|(_, &count)| breadth_bonus + depth_bonus * (1.0 + count as f64).ln())
            .sum()
    }

    /// Positive-only activation score. This is the one that should feed
    /// `ColonyScore::aggregate`; mixing in negative/neutral features made the
    /// aggregate reward colony distress.
    pub fn positive_activation_score(&self, breadth_bonus: f64, depth_bonus: f64) -> f64 {
        self.activation_score_in(FeatureCategory::Positive, breadth_bonus, depth_bonus)
    }

    /// Raw count of all negative-valence feature firings.
    ///
    /// "How many bad things happened" is the right question for negative
    /// events — the log-scaled breadth+depth score is designed to reward
    /// diverse activity, which is the opposite of what we want for failures.
    pub fn negative_event_count(&self) -> u64 {
        self.counts
            .iter()
            .filter(|(feature, _)| feature.category() == FeatureCategory::Negative)
            .map(|(_, &count)| count)
            .sum()
    }

    /// Distinct features in a given category that have fired at least once.
    pub fn features_active_in(&self, category: FeatureCategory) -> u32 {
        self.counts
            .iter()
            .filter(|(feature, &count)| count > 0 && feature.category() == category)
            .count() as u32
    }

    /// Total number of features in a given category across `Feature::ALL`.
    pub fn features_total_in(category: FeatureCategory) -> u32 {
        Feature::ALL
            .iter()
            .filter(|f| f.category() == category)
            .count() as u32
    }

    /// Features in a given category that have never fired.
    pub fn dead_features_in(&self, category: FeatureCategory) -> Vec<Feature> {
        Feature::ALL
            .iter()
            .filter(|f| f.category() == category && self.counts.get(f).copied().unwrap_or(0) == 0)
            .copied()
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_increments() {
        let mut sa = SystemActivation::default();
        sa.record(Feature::BondFormed);
        sa.record(Feature::BondFormed);
        sa.record(Feature::CombatResolved);
        assert_eq!(sa.counts[&Feature::BondFormed], 2);
        assert_eq!(sa.counts[&Feature::CombatResolved], 1);
    }

    #[test]
    fn dead_features_returns_unfired() {
        let mut sa = SystemActivation::default();
        sa.record(Feature::BondFormed);
        let dead = sa.dead_features();
        assert!(dead.contains(&Feature::CorruptionSpread));
        assert!(!dead.contains(&Feature::BondFormed));
        assert_eq!(dead.len(), Feature::ALL.len() - 1);
    }

    #[test]
    fn features_active_count() {
        let mut sa = SystemActivation::default();
        assert_eq!(sa.features_active(), 0);
        sa.record(Feature::BondFormed);
        sa.record(Feature::CombatResolved);
        assert_eq!(sa.features_active(), 2);
    }

    #[test]
    fn positive_activation_score_empty() {
        let sa = SystemActivation::default();
        assert_eq!(sa.positive_activation_score(20.0, 5.0), 0.0);
    }

    #[test]
    fn positive_activation_score_one_feature() {
        let mut sa = SystemActivation::default();
        sa.record(Feature::BondFormed);
        let score = sa.positive_activation_score(20.0, 5.0);
        let expected = 20.0 + 5.0 * 2.0_f64.ln();
        assert!((score - expected).abs() < 1e-10);
    }

    #[test]
    fn positive_activation_score_scales_with_breadth() {
        let mut sa = SystemActivation::default();
        for feature in Feature::ALL {
            sa.record(*feature);
        }
        let score = sa.positive_activation_score(20.0, 0.0);
        let expected = 20.0 * SystemActivation::features_total_in(FeatureCategory::Positive) as f64;
        assert!((score - expected).abs() < 1e-10);
    }

    #[test]
    fn every_feature_has_a_category() {
        for feature in Feature::ALL {
            let _ = feature.category();
        }
    }

    /// Ticket 368 — silent-canary surfaces are forbidden (CLAUDE.md
    /// `## Conventions`). The 367 pre-Commit-4 amend bug was a Feature
    /// variant added to the enum but omitted from `Feature::ALL`; the
    /// never-fired canary then silently skipped the new variant. The
    /// exhaustive matches in `feature_name` / `category` /
    /// `expected_to_fire_per_soak` already break compile when a variant
    /// is added without classification. This test closes the last
    /// surface: a hand-maintained sentinel count + duplicate check so
    /// the developer who adds a variant must also add it to ALL and
    /// bump `EXPECTED_VARIANT_COUNT` below.
    #[test]
    fn feature_all_is_exhaustive_and_unique() {
        use std::collections::HashSet;
        const EXPECTED_VARIANT_COUNT: usize = 170;
        let distinct: HashSet<_> = Feature::ALL.iter().map(std::mem::discriminant).collect();
        assert_eq!(
            distinct.len(),
            Feature::ALL.len(),
            "Feature::ALL contains duplicate variants — drop the duplicate.",
        );
        assert_eq!(
            Feature::ALL.len(),
            EXPECTED_VARIANT_COUNT,
            "Feature::ALL count drift. Adding/removing a Feature variant requires \
             updating Feature::ALL and bumping EXPECTED_VARIANT_COUNT in this test. \
             See CLAUDE.md `## Conventions` — 'Silent-canary surfaces are forbidden'.",
        );
    }

    #[test]
    fn category_counts_match_plan() {
        let mut positive = 0;
        let mut negative = 0;
        let mut neutral = 0;
        for feature in Feature::ALL {
            match feature.category() {
                FeatureCategory::Positive => positive += 1,
                FeatureCategory::Negative => negative += 1,
                FeatureCategory::Neutral => neutral += 1,
            }
        }
        assert_eq!(positive + negative + neutral, Feature::ALL.len());
        // 36 pre-existing Positive + 8 added in §Phase 5a (FoodEaten,
        // Socialized, GroomedOther, MentoredCat, ThreatEngaged,
        // MaterialsDelivered, BuildingRepaired, CourtshipInteraction).
        // Phase 6a added 1 Neutral (CommitmentDropTriggered) +
        // 4 branch-specific Neutrals (Blind / SingleMinded /
        // OpenMinded / ReplanCap) for the §7.2 commitment-gate
        // tracing split. Ticket 038 added 1 Positive
        // (MaterialPickedUp, paired with the resurrected
        // MaterialsDelivered). Ticket 027b added 2 Positive
        // (PairingIntentionEmitted, PairingBiasApplied) + 1 Neutral
        // (PairingDropped) for the §7.M L2 PairingActivity layer.
        // Ticket 080 added 1 Neutral (ReservationContended) for the
        // resource-reservation substrate. Ticket 073 added 1 Neutral
        // (TargetCooldownApplied) for the target-predictability cooldown (292).
        // Ticket 104 added 1 Positive (HideFreezeFired) for the
        // Hide/Freeze valence — Phase-1 dormant via the HideEligible
        // gate, but already classified.
        // Ticket 149 added 1 Positive (HuntAttempted) for the
        // discrete-attempt instrumentation that disambiguates per-Hunt-
        // action rate from per-discrete-attempt rate.
        // Ticket 176 added 1 Negative (OverflowToGround — anomaly
        // signal when engage_prey/forage_item overflow inventory) +
        // 3 Neutral (ItemDropped, ItemTrashed, ItemHandedOff) for the
        // disposal-action surface (Drop / Trash / Handoff).
        // Ticket 118 added 1 Negative (ModifierPreemption — substrate-
        // driven plan preemption from acute-class lurch modifiers).
        // Ticket 228 added 1 Negative (RouteCostFieldFallback — A*
        // fallback observability for the per-cat route-cost field).
        // Ticket 230 added 2 Positive (FleeTargetPicked,
        // FleeRecovered — Fleeing chain end-to-end signals).
        // Ticket 035 added 1 Positive (BurialPerformed — burial
        // continuity-canary signal). Ticket 250 demoted it to
        // Neutral after post-247 / 248 substrate stability made
        // deaths (and therefore burials) genuinely rare in healthy
        // colonies — net 0 to positive count, +1 to neutral.
        // Ticket 126 added 2 Positive (IntentionAdopted,
        // IntentionFulfilled) + 1 Neutral (IntentionAbandoned) for
        // the BDI intention substrate lifecycle.
        // Ticket 127 Commit A added 3 Positive (JointIntentionEmitted,
        // JointBiasApplied, JointStageAdvanced) + 2 Neutral
        // (JointIntentionDropped, JointStageMismatchTickAccrued), all
        // parameterized on PracticeKind (Courtship only in 127).
        // Commit C deleted the 3 PA Features (2 Positive
        // PairingIntentionEmitted/PairingBiasApplied + 1 Neutral
        // PairingDropped), net -2 Positive / -1 Neutral.
        // Ticket 288 added 1 Neutral (CommitmentDropMoraleBreak — the
        // morale_break-driven commitment release counter).
        // Ticket 260 added 1 Positive (ShadowFoxAvoidedCatScent — the
        // substrate-visible scent-channel sibling of the existing
        // ShadowFoxAvoidedWard magic-channel feature; both are
        // exempt from the seed-42 canary because firing depends on
        // ShadowFox/ward/colony spatial overlap).
        // Ticket 023 Phase A added 1 Positive (ShadowFoxDissolved —
        // slow-environmental defeat paired with ShadowFoxBanished;
        // exempt from the seed-42 canary as it depends on sustained
        // cleansing pressure that a healthy 15-min soak may not produce).
        // Ticket 023 Phase B added 4 Neutral (state-entry signals for
        // the four motivation states — Reconstituting/Tending/Haunting/
        // Seeding; observability not harm).
        // Ticket 023 Phase C added 2 Neutral (cadenced haunting-drain
        // + escalation; harm signal lives in cat Mood/Safety, not the
        // activation count).
        // Ticket 320 added 2 Positive (MethodAdopted, SubGoalAdvanced)
        // and 2 Neutral (MethodBacktracked, MethodDepthExceeded) for
        // the HTN method-stack lifecycle.
        // Ticket 025 Phase 2 added 10 Positive (5 hawk + 5 snake GOAP
        // features). All ship dormant; the four trunks are promoted
        // in commit 6 after the cutover soak.
        // Ticket 332/333 added 6 Positive (VigilHeld, GriefProcessed,
        // GriefReleased, KittenWeaned, SkillTaught, KittenReleased)
        // for the `mourn_at_grave` and `rear_kitten` HTN method
        // primitive completions. All ship dormant; promotion is the
        // dispatch follow-on named in each ticket's landing Log.
        // Ticket 095 Phase 1 added 1 Positive (BodyPartInjury) for the
        // anatomical injury substrate. Enrolled in the seed-42 canary
        // (cats take damage every soak).
        // Ticket 365 added 1 Positive (RemedyPrepared) for the
        // herbcraft items-are-real migration. expected_to_fire_per_soak
        // is false (mirrors RemedyApplied's opt-out — herbcraft DSEs
        // need both an injured patient and a herbalism-capable cat).
        // Ticket 084 Commit 1: +2 Positive (HerbsDeposited, HerbsRetrieved)
        // for the herb-stash economy. Both ship `expected_to_fire_per_soak()
        // => false` in Commit 1; Commit 2's plan-template wiring promotes
        // them to `=> true`.
        // Ticket 367 Commit 4: +6 Positive (FoodLoadedOnDryingRack,
        // MeatLoadedOnSmokingRack, SmokingRackTended, FoodDried,
        // MeatSmoked, OrganPreserved) for the preservation pipeline.
        // Five expected=true; OrganPreserved expected=false (chain-rare).
        // Ticket 375: +1 Positive (ByproductSpawned) for the prey-byproduct
        // producer canary. expected_to_fire_per_soak() => true — every
        // successful hunt emits ≥1 byproduct via the prey_byproducts table.
        // Ticket 450: +1 Positive (KittenBegged) for the kitten-begs-
        // for-food canary signal. The Begging Activity Intention runs
        // each tick a Stage 1 or Stage 2 kitten holds the Begging
        // disposition; the resolver records the canary on every cycle
        // completion (`expected_to_fire_per_soak() => true`).
        // Ticket 368: +3 Positive (GroomingBrushUsed, PlayBundleEngaged,
        // CourtshipGiftOffered) for the Phase 2 behavioral-tool
        // canaries. All ship dormant (`=> false`) until the Workshop-
        // craft pipeline follow-on circulates the tools in seed-42.
        // Ticket 457: +1 Positive (ItemCrafted) for the Workshop-craft
        // first-light canary. Ships expected=true — the elect-side
        // pipeline must fire ≥1 craft in seed-42 to satisfy the 368
        // substrate first-light gate.
        // Ticket 055: +1 Negative (AspirationDriftAbandoned) for the
        // §7.7.d mood drift-threshold detection layer. Ships
        // `expected_to_fire_per_soak == false` (mirror of
        // `AspirationAbandoned`) until first-light soak cadence
        // confirms the canary expectation.
        // Ticket 101: +1 Positive (EnvironmentalComfortPositive) and
        // +1 Negative (EnvironmentalComfortNegative) for the env-quality
        // canary pair (`emit_env_quality_features`).
        // Ticket 477: +1 Neutral (BoneWeaponSnapped) for the bone-weapon
        // durability snap canary (mechanical wear texture).
        // Ticket 334: +1 Positive (ItemWorn) for the deliberate don/swap
        // path of the WearItem resolver. expected_to_fire_per_soak => false
        // (auto-equip-on-craft makes the don leaf an idempotent no-op).
        // Ticket 429: +4 Positive (ItemSourcedFromDenRaid,
        // ItemSourcedFromHuntCatch, ItemSourcedFromForageCatch,
        // EatFromOwnInventory) for the items-are-real Source/Sink gate
        // contract. HuntByproductSource reuses the existing
        // ByproductSpawned Positive canary (1:1 by construction).
        // All four ship `expected_to_fire_per_soak() => true`.
        // Ticket 482: +3 Positive (ItemSourcedFromPreservation,
        // ItemSourcedFromHarvestCarcass, ItemSourcedFromForageIngredient)
        // promoting the three Source-shaped sites 429 deferred. All ship
        // `expected_to_fire_per_soak() => true`.
        // Ticket 310 S1: +1 Neutral (ShadowFoxHungerHuntEntered) for the
        // hunger-drive Stalking election (state transition, not harm).
        // Ships `expected_to_fire_per_soak() => false` per the
        // new-Feature default; the shadowfox_hunger_hunt_cycle scenario
        // hosts its firing assertion.
        // Ticket 310 S2: +1 Neutral (ShadowFoxRetreatEntered) for the
        // post-ambush retreat-to-den transition; same scenario hosts it.
        assert_eq!(positive, 99);
        assert_eq!(negative, 25);
        assert_eq!(neutral, 46);
    }

    #[test]
    fn representative_classifications() {
        assert_eq!(Feature::BondFormed.category(), FeatureCategory::Positive);
        assert_eq!(Feature::DeathOldAge.category(), FeatureCategory::Positive);
        assert_eq!(
            Feature::ShadowFoxBanished.category(),
            FeatureCategory::Positive
        );
        assert_eq!(
            Feature::DeathStarvation.category(),
            FeatureCategory::Negative
        );
        assert_eq!(
            Feature::CorruptionSpread.category(),
            FeatureCategory::Negative
        );
        assert_eq!(
            Feature::FoxStoreRaided.category(),
            FeatureCategory::Negative
        );
        assert_eq!(Feature::MoodContagion.category(), FeatureCategory::Neutral);
        assert_eq!(Feature::FoxHuntedPrey.category(), FeatureCategory::Neutral);
        assert_eq!(Feature::CombatResolved.category(), FeatureCategory::Neutral);
    }

    #[test]
    fn positive_activation_score_ignores_negatives() {
        let mut sa = SystemActivation::default();
        sa.record(Feature::BondFormed); // positive
        sa.record(Feature::DeathStarvation); // negative — should not contribute
        sa.record(Feature::MoodContagion); // neutral — should not contribute
        let score = sa.positive_activation_score(20.0, 5.0);
        let expected = 20.0 + 5.0 * 2.0_f64.ln(); // one positive feature firing once
        assert!((score - expected).abs() < 1e-10);
    }

    #[test]
    fn negative_event_count_sums_only_negatives() {
        let mut sa = SystemActivation::default();
        sa.record(Feature::DeathStarvation);
        sa.record(Feature::DeathStarvation);
        sa.record(Feature::CorruptionSpread);
        sa.record(Feature::BondFormed); // positive — should not contribute
        sa.record(Feature::MoodContagion); // neutral — should not contribute
        assert_eq!(sa.negative_event_count(), 3);
    }

    #[test]
    fn features_active_in_partitions_by_category() {
        let mut sa = SystemActivation::default();
        sa.record(Feature::BondFormed);
        sa.record(Feature::KittenBorn);
        sa.record(Feature::DeathStarvation);
        sa.record(Feature::MoodContagion);
        assert_eq!(sa.features_active_in(FeatureCategory::Positive), 2);
        assert_eq!(sa.features_active_in(FeatureCategory::Negative), 1);
        assert_eq!(sa.features_active_in(FeatureCategory::Neutral), 1);
    }

    #[test]
    fn features_total_in_matches_category_counts() {
        // Ticket 250: BurialPerformed demoted from Positive to Neutral
        // (post-247 / 248 substrate makes deaths and burials genuinely
        // rare in healthy colonies), shifting -1 positive / +1 neutral.
        // Ticket 127 Commit A: +3 Positive / +0 Negative / +2 Neutral
        // for the JointIntention substrate (5 parameterized features,
        // Courtship-only in 127). Commit C: -2 Positive / -1 Neutral
        // for the PA Feature deletions (PairingIntentionEmitted /
        // PairingBiasApplied / PairingDropped).
        // Ticket 288: +1 Neutral (CommitmentDropMoraleBreak).
        // Ticket 260: +1 Positive (ShadowFoxAvoidedCatScent — the
        // scent-channel sibling of ShadowFoxAvoidedWard).
        // Ticket 023 Phase A: +1 Positive (ShadowFoxDissolved — slow-
        // environmental defeat paired with ShadowFoxBanished).
        // Ticket 023 Phase B: +4 Neutral (motivation-state entries).
        // Ticket 023 Phase C: +2 Neutral (haunting-drain + escalation).
        // Ticket 320: +2 Positive (MethodAdopted, SubGoalAdvanced) /
        // +2 Neutral (MethodBacktracked, MethodDepthExceeded) for the
        // HTN method-stack lifecycle.
        // Ticket 025 Phase 2: +10 Positive (5 hawk + 5 snake GOAP
        // features; ship dormant, four trunks promoted in commit 6).
        // Ticket 332/333: +6 Positive (VigilHeld, GriefProcessed,
        // GriefReleased, KittenWeaned, SkillTaught, KittenReleased)
        // for the `mourn_at_grave` and `rear_kitten` HTN method
        // primitive completions. All ship dormant.
        // Ticket 095 Phase 1: +1 Positive (BodyPartInjury) for the
        // anatomical injury substrate.
        // Ticket 365: +1 Positive (RemedyPrepared) for the herbcraft
        // items-are-real migration.
        // Ticket 084 Commit 1: +2 Positive (HerbsDeposited, HerbsRetrieved)
        // for the herb-stash economy.
        // Ticket 367 Commit 4: +6 Positive preservation Features.
        // Ticket 375: +1 Positive (ByproductSpawned) for prey-byproduct
        // producer canary.
        // Ticket 368: +3 Positive (GroomingBrushUsed, PlayBundleEngaged,
        // CourtshipGiftOffered) for Phase 2 behavioral-tool canaries.
        // Ticket 457: +1 Positive (ItemCrafted) for Workshop-craft
        // first-light canary.
        // Ticket 055: +1 Negative (AspirationDriftAbandoned) for §7.7.d
        // mood drift-threshold detection.
        // Ticket 101: +1 Positive (EnvironmentalComfortPositive) and
        // +1 Negative (EnvironmentalComfortNegative).
        // Ticket 334: +1 Positive (ItemWorn) for the WearItem don/swap path.
        // Ticket 429: +4 Positive (ItemSourcedFromDenRaid /
        // ItemSourcedFromHuntCatch / ItemSourcedFromForageCatch /
        // EatFromOwnInventory) for the items-are-real Source/Sink gates.
        // Ticket 482: +3 Positive (ItemSourcedFromPreservation /
        // ItemSourcedFromHarvestCarcass / ItemSourcedFromForageIngredient)
        // promoting the three Source-shaped sites 429 deferred.
        assert_eq!(
            SystemActivation::features_total_in(FeatureCategory::Positive),
            99
        );
        assert_eq!(
            SystemActivation::features_total_in(FeatureCategory::Negative),
            25
        );
        assert_eq!(
            SystemActivation::features_total_in(FeatureCategory::Neutral),
            46
        );
    }

    #[test]
    fn dead_features_in_filters_by_category() {
        let mut sa = SystemActivation::default();
        sa.record(Feature::BondFormed);
        let dead_pos = sa.dead_features_in(FeatureCategory::Positive);
        assert!(!dead_pos.contains(&Feature::BondFormed));
        assert!(dead_pos.contains(&Feature::KittenBorn));
        // All negative features are dead (none fired).
        let dead_neg = sa.dead_features_in(FeatureCategory::Negative);
        assert_eq!(
            dead_neg.len(),
            SystemActivation::features_total_in(FeatureCategory::Negative) as usize
        );
    }

    #[test]
    fn serde_round_trip() {
        let mut sa = SystemActivation::default();
        sa.record(Feature::BondFormed);
        sa.record(Feature::CorruptionSpread);
        let json = serde_json::to_string(&sa).unwrap();
        let sa2: SystemActivation = serde_json::from_str(&json).unwrap();
        assert_eq!(sa.counts, sa2.counts);
    }

    #[test]
    fn never_fired_expected_positives_reports_silently_dead() {
        // Empty SA: every expected-Positive is missing from counts,
        // so the full expected set is reported.
        let sa = SystemActivation::default();
        let missing = sa.never_fired_expected_positives();
        // Representative check: trunk Features (each one independent)
        // show up when the tracker is empty.
        assert!(missing.contains(&"MatingOccurred"));
        assert!(missing.contains(&"FoodEaten"));
        assert!(missing.contains(&"Socialized"));
        assert!(missing.contains(&"MentoredCat"));
        // Rare-legend features are excluded.
        assert!(!missing.contains(&"ShadowFoxBanished"));
        assert!(!missing.contains(&"FateAwakened"));
        // Ticket 084 + 418 fix — Crop features re-promoted after the
        // herb-pressure chronicity axis lifts Farm under chronic
        // ward-stockpile scarcity. Verification soak observed
        // CropTended 4183 / CropHarvested 44 on seed-42.
        assert!(missing.contains(&"CropTended"));
        assert!(missing.contains(&"CropHarvested"));
        // Cascade-exempt features are excluded — they cascade from
        // their trunk and don't add independent canary signal.
        assert!(!missing.contains(&"KittenFed"));
        assert!(!missing.contains(&"GestationAdvanced"));
        assert!(!missing.contains(&"KittenBorn"));
        assert!(!missing.contains(&"ItemRetrieved"));
    }

    #[test]
    fn never_fired_expected_positives_shrinks_as_features_fire() {
        let mut sa = SystemActivation::default();
        let before = sa.never_fired_expected_positives();
        sa.record(Feature::FoodEaten);
        sa.record(Feature::Socialized);
        let after = sa.never_fired_expected_positives();
        assert_eq!(after.len(), before.len() - 2);
        assert!(!after.contains(&"FoodEaten"));
        assert!(!after.contains(&"Socialized"));
    }

    #[test]
    fn expected_to_fire_per_soak_classification() {
        // Core-subsystem trunks must be expected.
        assert!(Feature::FoodEaten.expected_to_fire_per_soak());
        assert!(Feature::Socialized.expected_to_fire_per_soak());
        assert!(Feature::MentoredCat.expected_to_fire_per_soak());
        // Trunks of chains-with-open-tickets stay expected so the
        // canary keeps flagging them RED until their tickets land.
        assert!(Feature::MatingOccurred.expected_to_fire_per_soak());
        assert!(Feature::FoodCooked.expected_to_fire_per_soak());
        assert!(Feature::GroomedOther.expected_to_fire_per_soak());
        // Promoted by ticket 027 Bug 1 (courtship-drift emits per-tick).
        assert!(Feature::CourtshipInteraction.expected_to_fire_per_soak());
        // Ticket 127 — JointIntention canary classifications: the
        // three Positive variants carry the per-practice fire-rate
        // expectation (PracticeKind::Courtship in 127); the two
        // Neutral variants (drop + mismatch) are bursty / healthy-
        // sometimes-zero respectively.
        assert!(Feature::JointIntentionEmitted {
            practice: PracticeKind::Courtship,
        }
        .expected_to_fire_per_soak());
        assert!(Feature::JointBiasApplied {
            practice: PracticeKind::Courtship,
        }
        .expected_to_fire_per_soak());
        assert!(Feature::JointStageAdvanced {
            practice: PracticeKind::Courtship,
        }
        .expected_to_fire_per_soak());
        assert!(!Feature::JointIntentionDropped {
            practice: PracticeKind::Courtship,
        }
        .expected_to_fire_per_soak());
        assert!(!Feature::JointStageMismatchTickAccrued {
            practice: PracticeKind::Courtship,
        }
        .expected_to_fire_per_soak());
        // Ticket 276 Commit B — PlayBout inherits the same blanket
        // classification: Emitted / BiasApplied / StageAdvanced enrolled
        // as expected-to-fire (positive canary); Dropped + Mismatch
        // tick remain exempt (bursty / healthy-sometimes-zero). The
        // explicit asserts lock the contract so a future per-practice
        // override can't silently demote PlayBout.
        assert!(Feature::JointIntentionEmitted {
            practice: PracticeKind::PlayBout,
        }
        .expected_to_fire_per_soak());
        assert!(Feature::JointBiasApplied {
            practice: PracticeKind::PlayBout,
        }
        .expected_to_fire_per_soak());
        assert!(Feature::JointStageAdvanced {
            practice: PracticeKind::PlayBout,
        }
        .expected_to_fire_per_soak());
        assert!(!Feature::JointIntentionDropped {
            practice: PracticeKind::PlayBout,
        }
        .expected_to_fire_per_soak());
        assert!(!Feature::JointStageMismatchTickAccrued {
            practice: PracticeKind::PlayBout,
        }
        .expected_to_fire_per_soak());
        // Rare-legend events must be exempted.
        assert!(!Feature::ShadowFoxBanished.expected_to_fire_per_soak());
        assert!(!Feature::ShadowFoxDissolved.expected_to_fire_per_soak());
        assert!(
            Feature::ShadowFoxDissolved.category() == FeatureCategory::Positive,
            "ShadowFoxDissolved pairs with ShadowFoxBanished as a defensive win",
        );
        assert!(!Feature::FateAwakened.expected_to_fire_per_soak());
        assert!(!Feature::ScryCompleted.expected_to_fire_per_soak());
        // 250: burial is conditional on death; post-247 / 248
        // substrate keeps colonies healthy enough that deaths are
        // genuinely rare. Demoted to neutral + exempted from the
        // never-fired canary.
        assert!(!Feature::BurialPerformed.expected_to_fire_per_soak());
        // Ticket 025 Phase 2 — hawk/snake GOAP positives ship dormant.
        // Commit 3 (this commit) registers them; commit 6 promotes the
        // four trunks (`HawkSpottedPrey`, `HawkDiveLanded`,
        // `SnakeStruckPrey`, `SnakeAmbushed`) to `true` after the
        // cutover soak observes them firing. The other six stay false.
        assert!(!Feature::HawkSpottedPrey.expected_to_fire_per_soak());
        assert!(!Feature::HawkDiveLanded.expected_to_fire_per_soak());
        assert!(!Feature::HawkPerched.expected_to_fire_per_soak());
        assert!(!Feature::HawkFled.expected_to_fire_per_soak());
        assert!(!Feature::HawkDied.expected_to_fire_per_soak());
        assert!(!Feature::SnakeStruckPrey.expected_to_fire_per_soak());
        assert!(!Feature::SnakeBasked.expected_to_fire_per_soak());
        assert!(!Feature::SnakeAmbushed.expected_to_fire_per_soak());
        assert!(!Feature::SnakeRetreated.expected_to_fire_per_soak());
        assert!(!Feature::SnakeDied.expected_to_fire_per_soak());
        // Category assertions — all ten are Positive.
        assert_eq!(
            Feature::HawkSpottedPrey.category(),
            FeatureCategory::Positive
        );
        assert_eq!(
            Feature::SnakeStruckPrey.category(),
            FeatureCategory::Positive
        );
        assert_eq!(
            Feature::BurialPerformed.category(),
            FeatureCategory::Neutral
        );
        // Cascade-exempt: silent strictly because their trunk is
        // silent. Promoting them to expected would multiply one
        // root-cause failure into N canary entries.
        assert!(!Feature::GestationAdvanced.expected_to_fire_per_soak());
        assert!(!Feature::KittenBorn.expected_to_fire_per_soak());
        assert!(!Feature::KittenFed.expected_to_fire_per_soak());
        assert!(!Feature::ItemRetrieved.expected_to_fire_per_soak());
        // Ticket 084 + 418 fix — verified post-soak (2026-05-19,
        // logs/tuned-42): both gather→deposit and retrieve→weave
        // pipelines fire reliably on seed-42 (HerbsDeposited 115,
        // HerbsRetrieved 18, WardPlaced 21).
        assert!(Feature::HerbsDeposited.expected_to_fire_per_soak());
        assert!(Feature::HerbsRetrieved.expected_to_fire_per_soak());
        assert_eq!(
            Feature::HerbsDeposited.category(),
            FeatureCategory::Positive
        );
        assert_eq!(
            Feature::HerbsRetrieved.category(),
            FeatureCategory::Positive
        );
        // Ticket 083 demoted CropTended / CropHarvested under abundant
        // food. Ticket 084 Commit 3 + 418 fix re-promoted them: the
        // herb-pressure chronicity axis lifts Farm under chronic
        // ward-stockpile scarcity. Verification soak observed 4183 +
        // 44 events on seed-42.
        assert!(Feature::CropTended.expected_to_fire_per_soak());
        assert!(Feature::CropHarvested.expected_to_fire_per_soak());
    }
}
