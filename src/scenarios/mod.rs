//! Ticket 162 — scenario harness for fast deterministic AI decision triage.
//!
//! A scenario is a tiny preloaded world (1–5 cats with specific
//! needs/personality/markers/positions, optionally seeded influence-map
//! cells) that runs for a small number of ticks and reports which DSE the
//! focal cat picked at each tick. Wall-clock target ~3 seconds, vs. ~15
//! minutes for `just soak`.
//!
//! Scenarios bypass `build_new_world` via the [`crate::plugins::setup::WorldSetup`]
//! resource, so terrain and entity spawn are entirely under scenario
//! control. Helpers in [`env`] do the resource-init heavy lifting.
//!
//! # Bugfix discipline integration
//!
//! Per CLAUDE.md, the scenario harness is the **triage** tool: it answers
//! "given this state, which DSE wins?" cheaply. Reach for `just scenario
//! <name>` before `just soak` whenever a hypothesis names specific cat
//! state. `just soak` remains for whole-colony verification once a fix is
//! drafted.
//!
//! # Determinism
//!
//! `SimulationPlugin` already pins both Startup and FixedUpdate to the
//! single-threaded executor (`src/plugins/simulation.rs:115-132`), so
//! scenario runs are byte-deterministic per seed. The runner asserts this
//! invariant in tests via stdout-diff.

pub mod affordance_substrate;
pub mod belief_affordance_dse_consumers;
pub mod chokepoint_defense_isthmus;
pub mod colony_knowledge_false_belief;
pub mod colony_reserves_belief;
pub mod disposal_dispatch;
pub mod disposal_election;
pub mod district_placement_under_pressure;
pub mod drying_chain_eligibility;
pub mod dying_arc_softmax;
pub mod env;
pub mod equipment_bone_snap;
pub mod equipment_cloak_mask;
pub mod equipment_weapon_strike;
pub mod exploration_ranging;
pub mod farm_herb_demand;
pub mod farming_cycle;
pub mod festering_wound;
pub mod fish_shoreline_pounce;
pub mod flee_calibration;
pub mod flee_commitment;
pub mod fondness_kitten_imprint;
pub mod fox_cat_scent_avoidance;
pub mod fox_ward_only_avoidance;
pub mod grooming_other;
pub mod guarding_morale_break_releases;
pub mod hunt_acquisition;
pub mod hunt_deposit_chain;
pub mod intention_momentum_pickup_lock;
pub mod inventory_full_no_pickup;
pub mod items_eat_from_own_inventory;
pub mod kitten_cry;
pub mod kittenhood_stages;
pub mod lone_burial;
pub mod mate_chain;
pub mod parenting_caretake_kitten_absent;
pub mod parenting_caretake_kitten_present;
pub mod parenting_father_provisions;
pub mod parenting_grief_kitten_death;
pub mod parenting_handoff_recipient_resolution;
pub mod parenting_joint_suppression;
pub mod patrol_recalibration;
pub mod picking_up_scavenging;
pub mod play_engagement_cues;
pub mod preset;
pub mod prey_byproduct_spawn;
pub mod route_cost_decision;
pub mod runner;
pub mod shelter_belief_security;
pub mod smoking_chain_complete;
pub mod smoking_chain_eligibility;
pub mod surrounded_colony;
pub mod ward_placement;
pub mod wildlife_belief_affordance_activation;
pub mod wildlife_fight;
pub mod wounded_cat_no_pickup;

use bevy_ecs::world::World;

/// A scenario describes how to populate the world before tick 0 and how
/// long to run the focal-cat trace. Scenarios are static at-spawn state;
/// multi-step scripting (e.g., "drop hunger to 0.1 at tick 5") is
/// deliberately out of scope (see ticket 162 `## Out of scope`).
#[derive(Clone, Copy)]
pub struct Scenario {
    /// Stable identifier used by the CLI (`just scenario <name>`).
    pub name: &'static str,
    /// Default focal cat. The runner inserts `FocalTraceTarget { name }`;
    /// the trace-emit system at `src/systems/trace_emit.rs:99-116`
    /// resolves the entity by name on the first tick the cat exists.
    pub default_focal: &'static str,
    /// Per-scenario tick budget. Behaviors live on different timescales
    /// (kitten-cry triage settles in ~5 ticks; farming spans ~120). The
    /// CLI flag `--ticks N` overrides this.
    pub default_ticks: u32,
    /// Populate the world: terrain, resources, entities. Replaces
    /// `build_new_world` via the `WorldSetup` resource.
    pub setup: fn(&mut World, u64),
    /// Ticket 198 — substrate-fires landing gate. `Feature::*` variants
    /// (by name) the scenario expects to emit ≥ 1× during the run. The
    /// `cargo test scenario_feature_assertions` integration test runs
    /// every scenario with a non-empty `expected_features` and asserts
    /// each declared Feature actually fired in `SystemActivation.counts`.
    /// Empty `&[]` opts out — legitimate for scenarios whose only
    /// purpose is L2/L3 election triage (no Feature emission expected).
    /// Scenarios exercising rare-tier outcomes (legend events, etc.)
    /// also opt out — the absence is the contract.
    pub expected_features: &'static [&'static str],
}

/// All scenarios known to the binary and the test suite. Adding a new
/// scenario means: write it under `src/scenarios/<name>.rs`, declare its
/// `pub static SCENARIO: Scenario = …`, append it here.
pub const ALL: &[&Scenario] = &[
    &kitten_cry::SCENARIO,
    &wildlife_fight::SCENARIO,
    &fondness_kitten_imprint::SCENARIO,
    &hunt_acquisition::SCENARIO,
    // 467 — shoreline-pounce vantage: offshore fish catchable from the
    // bank; mid-lake fish excluded from the candidate set.
    &fish_shoreline_pounce::SCENARIO,
    // 477 — equipment weapon-strike bonus surfaces in the resolver trace.
    &equipment_weapon_strike::SCENARIO,
    // 477 — cloak visual-mask surfaces in the resolver trace.
    &equipment_cloak_mask::SCENARIO,
    // 477 — fragile bone weapon snaps on a missed strike.
    &equipment_bone_snap::SCENARIO,
    // 184 — kill→travel→DepositPrey pipeline regression triage.
    &hunt_deposit_chain::SCENARIO,
    // 184 — fix lock: injured cats can still elect Hunt.
    &hunt_deposit_chain::SCENARIO_INJURED,
    &exploration_ranging::SCENARIO,
    &ward_placement::SCENARIO,
    &farming_cycle::SCENARIO,
    // 086 — Farm DSE herb-pressure axis (084) integration gate.
    // Deterministic Thornbriar tend→harvest cycle that asserts both
    // `Feature::CropTended` and `Feature::CropHarvested` fire. The
    // soak canary stays demoted; this scenario is the structural-
    // correctness surface. See module rustdoc for the empirical
    // reframe (085 evidence, sociology/economy follow-on parked).
    &farm_herb_demand::SCENARIO,
    // 158 — triage harness for the GroomedOther never-fired structural fix.
    &grooming_other::SCENARIO,
    // 178 — election-side scenarios for the lifted disposal DSEs.
    &disposal_election::SCENARIO_TRASHING,
    &disposal_election::SCENARIO_DISCARDING,
    &disposal_election::SCENARIO_IDLE,
    &disposal_election::SCENARIO_DISCARDING_BLOCKED,
    // 193 — election-side scenario for the rerouted PickingUp plan
    // template (PlannerZone::CarcassPile).
    &picking_up_scavenging::SCENARIO,
    // 191 — full scavenge → travel → deposit chain to Stores
    // (sister to `hunt_deposit_chain::SCENARIO`).
    &picking_up_scavenging::SCENARIO_TO_STORES,
    // 375 — per-species prey-byproduct table verification. Four
    // species variants (Fish excluded — water-habitat requirement
    // not met by the default test world; Fish row is covered by the
    // `prey_byproducts_table_default_matches_spec` unit test and the
    // seed-42 soak).
    &prey_byproduct_spawn::SCENARIO_MOUSE,
    &prey_byproduct_spawn::SCENARIO_RAT,
    &prey_byproduct_spawn::SCENARIO_RABBIT,
    &prey_byproduct_spawn::SCENARIO_BIRD,
    // 228 — bold-vs-timid route-cost suppression microexperiment.
    // Lifts hunt_route_cost_weight to 1.0 locally; canonical soak
    // constants ship at 0.0 (substrate-dormant).
    &route_cost_decision::SCENARIO,
    // 230 — substrate-aware Fleeing chain end-to-end smoke. Wounded
    // cat with adjacent fox + saturated naive-projection corridor.
    // Asserts `FleeTargetPicked` fires; the picker is reachable
    // through the `Action::Flee → DispositionKind::Fleeing` route
    // closing the last anxiety-interrupt arm migration.
    &flee_commitment::SCENARIO,
    // 255 — `ThreatProximityAdrenalineFlee` Flee-axis calibration
    // probe. Four variants across the (threat_proximity_derivative,
    // escape_viability) corners + a Sleep-partner doctrine probe.
    // L3-election-only; opts out of Feature gating.
    &flee_calibration::SCENARIO_LOW_THREAT,
    &flee_calibration::SCENARIO_OPEN_TERRAIN,
    &flee_calibration::SCENARIO_CORNERED,
    &flee_calibration::SCENARIO_SLEEP_PARTNER,
    // 271 — bold + critically wounded + cornered (Mocha profile from
    // the post-254 verification soak). Pre-fix the boldness-invert
    // axis hard-zero collapses CP and Flee falls out of L3. Post-fix
    // Flee reaches top-2 of the softmax pool on this profile.
    &flee_calibration::SCENARIO_CRITICAL_CORNERED,
    // 231 — capacity-aware pickup pipeline. Three sister scenarios:
    // full-of-curios + adjacent food drops the curio first then picks
    // up; full-of-herbs validates the ItemSlot collapse; empty cat
    // takes the substrate path with no DropItem prefix.
    &inventory_full_no_pickup::SCENARIO_FULL_CURIOS,
    &inventory_full_no_pickup::SCENARIO_FULL_HERBS,
    &inventory_full_no_pickup::SCENARIO_EMPTY_PICKUP,
    // 231 R3b — wounded cat L2 score regression. Reproduces the
    // dying-arc analysis: HP=0.49 + adjacent food → PickUp's L2
    // final_score is multiplicatively damped by `health_deficit`
    // (post-R3b) instead of scoring near 1.0 (pre-R3b).
    &wounded_cat_no_pickup::SCENARIO,
    // 232 — body-state-coupled L3 softmax temperature triage harness.
    // Wounded cat (HP=0.49) + adjacent fox saturates body_distress
    // and threat_proximity, driving softmax_temperature to the floor.
    // Fix-lock requires 231 + 232 together; load-bearing assertions
    // live in src/ai/scoring.rs unit tests.
    &dying_arc_softmax::SCENARIO,
    // 035 — lone-burial foundation gate. One adult adjacent to a
    // freshly-tagged `Dead` colony-mate; the chain must run end-to-end
    // and emit `Feature::BurialPerformed` so the burial canary's
    // mechanism is independently verified from the soak-side death-
    // rate dynamics.
    &lone_burial::SCENARIO,
    // 246 — repro the colony-scale PickUp lock pattern at scenario
    // scale. 3 cats clustered + 5 ground items + no Stores. If the
    // wiring/floor-removal causes the lock, focal action distribution
    // will show >70% PickUp.
    &intention_momentum_pickup_lock::SCENARIO,
    // 256 — Patrol DSE substrate recalibration. Warded demesne + a
    // sentinel + a distant fox. Demonstrates R3 (sector anchor),
    // R4 (patrol-tuned overlay weights), and R5 (fox-side deterrent
    // affect) firing in a small preloaded world. Unit tests in the
    // module assert each substrate piece independently.
    &patrol_recalibration::SCENARIO_WARDED_DEMESNE,
    // 257 — courtship → Partners-bond → Mate election → MatingOccurred
    // chain. Two cats pre-loaded at `bond = Friends` with
    // fondness/familiarity just above the Friends gate. Pre-fix the
    // chain stalls at Friends; post-fix (Pairing Commit B + retuned
    // emission_threshold) it advances to MatingOccurred within the
    // tick budget.
    &mate_chain::SCENARIO,
    // 288 — wounded cat in a Guarding plan reaches EngageThreat,
    // resolver returns Fail("morale_break"). Post-fix the GOAP
    // dispatcher releases the commitment so L3 re-elects instead of
    // replanning inside Guarding.
    &guarding_morale_break_releases::SCENARIO,
    // 260 — orthogonal-channel avoidance microexperiments.
    // `fox_cat_scent_avoidance` — colony cats radiate scent; lone
    // ShadowFox flips on `Feature::ShadowFoxAvoidedCatScent`.
    // `fox_ward_only_avoidance` — durable ward, no cats; lone
    // ShadowFox flips on `Feature::ShadowFoxAvoidedWard`. Together
    // they prove the magic + scent channels fire independently.
    &fox_cat_scent_avoidance::SCENARIO,
    &fox_ward_only_avoidance::SCENARIO,
    // 311 (301 FO-1) — chokepoint isthmus fixture. Narrow-isthmus map
    // exercising the ward supply chain end-to-end: pre-loaded inventory
    // → `WardPlaced`, mature Garden → `CropHarvested`, wild patches →
    // `GatherHerbCompleted`. FO-2 adds the location assertion that
    // ward selection corks the isthmus rather than painting the
    // landmass; this fixture lands GREEN under FO-1 defaults.
    &chokepoint_defense_isthmus::SCENARIO,
    // 308 — ColonyReservesBelief substrate first-light. Priestess
    // burns the colony's only thornbriar; witnesses pick up the
    // resulting low-reserve state via stagger-tick InventoryObserved
    // broadcasts; `HasLowWardReserve` marker fires.
    &colony_reserves_belief::SCENARIO,
    // 291 — false consensus promotes; contested bucket does not.
    &colony_knowledge_false_belief::SCENARIO,
    // 374 — ShelterBeliefs substrate first-light. Four-phase
    // lifecycle (claim, damage, siege, siege broken) drives each
    // sub-axis through its full update path; the lifecycle test
    // asserts belonging/quality/threat respond as documented.
    &shelter_belief_security::SCENARIO,
    // 313 (301 FO-3) — surrounded-colony ring-coverage fixture.
    // 5 cats clustered at center, 8 ShadowFoxes static on the
    // periphery. `mod tests` asserts that 4 successive
    // `compute_ward_placement` wakes plant wards in all 4
    // cardinal quadrants — under both the default `Additive`
    // composition and the 313 `Gate` composition. The Gate test
    // is 313's load-bearing compatibility check: does the
    // saturating-ramp gate break the multi-wake ring-formation
    // behavior in surrounded-threat geometry?
    &surrounded_colony::SCENARIO,
    // 261 — ActionAffordances substrate microexperiments. Six variants
    // exercising the writer across all five families. Behavior-neutral
    // assertions read directly from the `ActionAffordances` resource —
    // L2/L3 trace assertions land in consumer tickets (263+).
    &affordance_substrate::SCENARIO_FLEE_HIGH_COVER,
    &affordance_substrate::SCENARIO_FLEE_OPEN_GROUND,
    &affordance_substrate::SCENARIO_DIVE_HAWK,
    &affordance_substrate::SCENARIO_CHASE_PREY,
    &affordance_substrate::SCENARIO_FIGHT_CAPABILITY_MATCH,
    &affordance_substrate::SCENARIO_SUPERSEDES_LEGACY_SCALARS,
    // 263 — Flee/Patrol/Hunt belief + affordance consumer scenarios.
    // All four 263 consumer axes ship dormant; these scenarios assert
    // on the substrate-side reads (`ActionAffordances`,
    // `LocationBeliefs` facets) rather than L3 election outcomes.
    &belief_affordance_dse_consumers::SCENARIO_FLEE_BELIEF_HIGH_VIOLENCE,
    &belief_affordance_dse_consumers::SCENARIO_PATROL_AVOIDS_HIGH_THREAT_SECTOR,
    &belief_affordance_dse_consumers::SCENARIO_HUNT_PICKS_STALK_FOR_OBLIVIOUS_PREY,
    &belief_affordance_dse_consumers::SCENARIO_HUNT_PICKS_CHASE_FOR_ALERTED_PREY,
    // 265 (plan step 21) — wildlife-side consumer activation scenarios.
    // Election-level assertions: the activated wildlife axes must move
    // L3 outcomes, not just populate substrate reads.
    &wildlife_belief_affordance_activation::SCENARIO_FOX_BELIEF_HIGH_VIOLENCE,
    &wildlife_belief_affordance_activation::SCENARIO_HAWK_DIVE_AERIAL_COVER,
    &play_engagement_cues::SCENARIO_PLAY_ENGAGEMENT_CUES,
    // 472 — festering-wound substrate. Preloads Ashitaka with a
    // WoundKind::Festering on FrontRightPaw and asserts (a) the wound
    // persists under the slow heal rate, (b) the bonded peer Mononoke
    // accrues `perceived_injury_level` via the CarriesFesteringWound
    // belief-layer lift.
    &festering_wound::SCENARIO_FESTERING_WOUND,
    // 382 — district placement under colony-crowd pressure. Six
    // founder buildings packed inside the radius-16 spiral disc;
    // pre-loaded `Build` directive for `Stores`. Asserts the
    // influence-map placement finds a spot on the expansion frontier
    // and `Feature::ConstructionSiteSpawned` fires.
    &district_placement_under_pressure::SCENARIO,
    // Ticket 400 — L2 ParentingActivity archetype scenarios. Each
    // pre-populates `ParentingActivity` at the personality-derived
    // engagement asymptote to skip the ~1000-tick EMA build phase.
    &parenting_father_provisions::SCENARIO,
    &parenting_joint_suppression::SCENARIO,
    &parenting_grief_kitten_death::SCENARIO,
    // Ticket 410 — Caretake DSE eligibility-gate scenarios. `absent`
    // proves the gate suppresses Caretake when no dependent cat
    // exists; `present` proves the gate passes when a Kitten exists.
    // Closes the canary regression on the 400 verdict.
    &parenting_caretake_kitten_absent::SCENARIO,
    &parenting_caretake_kitten_present::SCENARIO,
    // [DRAFT — pending review] L3-resolver companion to the 410
    // scenarios. Surfaces the goap-path `HandoffItem` empty-snapshot
    // defect (afk-overnight-2026-05-19 soak: 177k canary fires).
    &parenting_handoff_recipient_resolution::SCENARIO,
    // 436 — drying-chain eligibility microexperiment. Three sister
    // fixtures isolate why `DryFoodDse` is silently filtered in the
    // post-367-Commit-9 verification soak by exercising the
    // `HasDryableAccessible` composite-marker disjuncts one at a time.
    &drying_chain_eligibility::SCENARIO_HOT_INVENTORY,
    &drying_chain_eligibility::SCENARIO_STORES_HAS_DRYABLE,
    &drying_chain_eligibility::SCENARIO_EMPTY_STORES,
    // 439 — resolver-completion fixtures. Eligibility settled at 436/437;
    // these isolate whether the basic resolver chain completes happy-path
    // at unit scale so the post-437 soak's "no reachable zone target"
    // failure can be classified as structural vs state-specific.
    &drying_chain_eligibility::SCENARIO_RESOLVER_COMPLETES,
    &drying_chain_eligibility::SCENARIO_RESOLVER_FAR_RACK,
    // 443 — smoking-chain eligibility fixtures. Mirrors the 436 drying
    // suite for the `HasSmokeableAccessible` composite-marker fix.
    &smoking_chain_eligibility::SCENARIO_HOT_INVENTORY,
    &smoking_chain_eligibility::SCENARIO_STORES_HAS_SMOKEABLE,
    &smoking_chain_eligibility::SCENARIO_EMPTY_STORES,
    // 447 — smoking-chain end-to-end completion. Replaces the
    // per-soak never-fired-positives regression coverage that 444
    // retired for the smoking triple (`MeatLoadedOnSmokingRack` /
    // `SmokingRackTended` / `MeatSmoked`). Cat preloaded with meat +
    // fuel adjacent to a functional idle SmokingRack; the unit test
    // pins a seed where the full chain fires within ~2000 ticks.
    &smoking_chain_complete::SCENARIO,
    // 450 — three-stage kittenhood substrate triage. Preloads one cat
    // at each Stage 1/2/3 sub-stage + an adult mother. Asserts
    // `KittenBegged` fires within the 12-tick window and verifies the
    // per-stage marker authoring via the module's unit tests.
    &kittenhood_stages::SCENARIO,
    // 429 — items-are-real Sink contract. Adult preloaded with food
    // in pouch + hunger below `eat_from_inventory_threshold`. Asserts
    // the slot drains, hunger rises, and `Feature::EatFromOwnInventory`
    // fires — locking in the dispatcher → resolver → Feature chain.
    &items_eat_from_own_inventory::SCENARIO,
];

/// Look up a scenario by its `name` field.
pub fn by_name(name: &str) -> Option<&'static Scenario> {
    ALL.iter().copied().find(|s| s.name == name)
}
