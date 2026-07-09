use bevy_ecs::prelude::*;

use crate::components::prey::PreyKind;
use crate::components::sensing::SensorySpecies;
use crate::components::wildlife::WildSpecies;
use crate::resources::time::Season;
use crate::resources::time_units::{DurationDays, DurationSeasons, IntervalPerDay, RatePerDay};
use crate::systems::sensing::{Channel, Falloff, SensoryProfile};

// ---------- MovementConstants (0.4.0 fluid locomotion — ticket 140) ----------

/// Fluid free-range movement knobs (0.4.0 "Free Range", ticket 140 /
/// plan step 5). Speeds are world units (tiles) per tick, Euclidean —
/// with arbitrary headings an L-infinity cap would make ground speed
/// direction-dependent (+41% at 45 degrees). Accelerations are tiles
/// per tick squared and feed `steering::steer`, the single source of
/// momentum. INERT until the step-6 integrator lands; only
/// `MovementBudget::for_species` reads the per-species speeds today
/// (values identical to the retired `WildSpecies::default_movement_budget`
/// match — footer-identical by construction).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MovementConstants {
    /// Cat steady-state max speed. The grid era moved 1 tile/tick in
    /// any of 8 directions; 1.0 Euclidean keeps cardinal parity and
    /// makes diagonals sqrt(2) slower — the deliberate,
    /// hypothesis-carried re-baseline plan.md names.
    pub cat_max_speed: f32,
    /// Fox steady-state max speed (parity with cats — chases are
    /// decided by interception geometry and stamina, not raw speed).
    pub fox_max_speed: f32,
    /// Hawk cruise speed (flight is terrain-exempt; the dive burst is
    /// its own ability, not this knob).
    pub hawk_max_speed: f32,
    /// ShadowFox steady-state max speed.
    pub shadowfox_max_speed: f32,
    /// Snake slither speed. 0.5 was previously expressed as
    /// tick-skipping (`MovementBudget` accumulator); under the
    /// integrator it becomes genuinely continuous half-speed motion —
    /// same average, no stutter.
    pub snake_max_speed: f32,
    /// Ground prey (mouse/rat/rabbit + grounded birds) max speed.
    pub prey_ground_max_speed: f32,
    /// Bird escape-burst flight speed (replaces the radial teleport —
    /// plan step 10). ~3x cat speed: a 2-3 tick head start a cat
    /// cannot close, matching the teleport's survival profile.
    pub bird_burst_speed: f32,
    /// Default max acceleration (tiles/tick^2) for ground movers —
    /// reaching full speed takes ~4 ticks; reversals curve instead of
    /// pivoting.
    pub max_accel: f32,
    /// Hawk max acceleration — banking raptors turn harder than
    /// ground movers accelerate.
    pub hawk_max_accel: f32,
    /// Hunt stalk-phase speed multiplier (slow sinuous approach).
    pub stalk_speed_mult: f32,
    /// Chase / flee / strike sprint multiplier over the species base
    /// speed. 140 step-12 gate tune: 1.4 → 2.4 — the ceiling must beat
    /// the rabbit's flee cap (`prey_ground_max_speed × flee_speed` =
    /// 2.0) or fleeing rabbits are UNCATCHABLE (soak tuned-42-2af8c34d:
    /// hunt success 22.3% → 15.1%, `lost prey during approach` 2183).
    /// Set to 3.0 = pre-140 parity: the legacy hunt kinematics were
    /// `chase_speed = approach_speed = 3` tiles/tick, and the whole
    /// detection/alertness/catch economy was tuned against them
    /// (2.4 under-shot: tuned-42-32e46f09 hunt success 9.5%). Real-cat
    /// sanity: sprint ≈ 9× walking pace IRL, so 3× is conservative.
    /// Endurance is bounded by `chase_limit_*`, not a stamina model.
    pub sprint_speed_mult: f32,
    /// Personal-space radius (tiles) for `steering::separation` —
    /// replaces the jitter-teleport anti-stacking hacks (plan step 7).
    pub separation_radius: f32,
    /// A smoothed-path waypoint counts as reached inside this radius;
    /// the mover then seeks the next one. MUST be > max_speed / 2:
    /// the pop check samples position once per tick, so a mover
    /// stepping `max_speed` per tick can jump clean over any window
    /// with diameter < max_speed and then orbit the missed waypoint
    /// forever (the step-6 verification soak produced ~1,750
    /// `TravelTo(*): travel timeout` failures at the original 0.35
    /// before this constraint was understood).
    pub waypoint_arrival_radius: f32,
    /// Minimum ticks between A* recomputes for a moving traveler
    /// (recompute throttling, plan step 13).
    pub path_recompute_min_ticks: u32,
    /// Recompute immediately when the target has drifted more than
    /// this many tiles from the position the current path was planned
    /// against.
    pub path_recompute_target_drift_tiles: f32,
    /// 140 step 12 — terrain ground-speed multipliers, sampled by the
    /// integrator from the mover's CURRENT containing tile
    /// (`Terrain::movement_cost()` buckets). Terrain finally costs
    /// SPEED, not just route preference: dense forest slows a sprint,
    /// rock slows a slither. `Flying` movers are exempt (bounds-clamp
    /// branch). cost 1 (grass/sand/buildings) → full speed.
    pub terrain_speed_mult_cost2: f32,
    /// cost 2 (light forest / mud / garden / special tiles).
    pub terrain_speed_mult_cost3: f32,
    /// cost 3 (dense forest).
    pub terrain_speed_mult_cost4: f32,
}

impl Default for MovementConstants {
    fn default() -> Self {
        Self {
            cat_max_speed: 1.0,
            fox_max_speed: 1.0,
            hawk_max_speed: 1.0,
            shadowfox_max_speed: 1.0,
            snake_max_speed: 0.5,
            prey_ground_max_speed: 1.0,
            bird_burst_speed: 3.0,
            max_accel: 0.25,
            hawk_max_accel: 0.5,
            stalk_speed_mult: 0.4,
            sprint_speed_mult: 3.0,
            separation_radius: 0.6,
            waypoint_arrival_radius: 0.6,
            path_recompute_min_ticks: 8,
            path_recompute_target_drift_tiles: 3.0,
            terrain_speed_mult_cost2: 0.8,
            terrain_speed_mult_cost3: 0.6,
            terrain_speed_mult_cost4: 0.5,
        }
    }
}

impl MovementConstants {
    /// Per-species steady-state max speed (the `MovementBudget.per_tick`
    /// source — ticket 140 retired the hardcoded
    /// `WildSpecies::default_movement_budget` match into this lookup).
    pub fn max_speed(&self, species: crate::components::wildlife::WildSpecies) -> f32 {
        use crate::components::wildlife::WildSpecies;
        match species {
            WildSpecies::Fox => self.fox_max_speed,
            WildSpecies::Hawk => self.hawk_max_speed,
            WildSpecies::ShadowFox => self.shadowfox_max_speed,
            WildSpecies::Snake => self.snake_max_speed,
        }
    }

    /// 140 step 12 — ground-speed multiplier for a terrain movement
    /// cost. Buckets rather than a formula so each knob is
    /// independently tunable (and visible to `just explain`).
    /// Impassable costs never reach the integrator's speed path (the
    /// passability check rejects them first) — return the slowest
    /// bucket defensively.
    pub fn terrain_speed_mult(&self, movement_cost: u32) -> f32 {
        match movement_cost {
            0 | 1 => 1.0,
            2 => self.terrain_speed_mult_cost2,
            3 => self.terrain_speed_mult_cost3,
            _ => self.terrain_speed_mult_cost4,
        }
    }
}

// ---------- SimConstants (top-level resource) ----------

#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct SimConstants {
    pub needs: NeedsConstants,
    pub buildings: BuildingConstants,
    pub combat: CombatConstants,
    pub magic: MagicConstants,
    pub social: SocialConstants,
    pub mood: MoodConstants,
    pub death: DeathConstants,
    #[serde(default)]
    pub founder_age: FounderAgeConstants,
    pub prey: PreyConstants,
    /// 0.4.0 "Free Range" fluid locomotion (ticket 140 / plan step 5).
    /// `#[serde(default)]` so pre-140 `events.jsonl` headers still
    /// deserialize cleanly.
    #[serde(default)]
    pub movement: MovementConstants,
    pub species: SpeciesConstants,
    pub scoring: ScoringConstants,
    pub disposition: DispositionConstants,
    pub colony_score: ColonyScoreConstants,
    pub wildlife: WildlifeConstants,
    #[serde(default)]
    pub fox_ecology: FoxEcologyConstants,
    /// Ticket 025 Phase 2 — hawk GOAP tuning. `#[serde(default)]` so
    /// pre-025 `events.jsonl` headers still deserialize cleanly; new
    /// fields populate from [`HawkEcologyConstants::default`].
    #[serde(default)]
    pub hawk_ecology: HawkEcologyConstants,
    /// Ticket 025 Phase 2 — snake GOAP tuning with thermoregulation.
    #[serde(default)]
    pub snake_ecology: SnakeEcologyConstants,
    pub fate: FateConstants,
    pub coordination: CoordinationConstants,
    pub aspirations: AspirationConstants,
    pub knowledge: KnowledgeConstants,
    pub personality_friction: PersonalityFrictionConstants,
    #[serde(default)]
    pub world_gen: WorldGenConstants,
    #[serde(default)]
    pub sensory: SensoryConstants,
    #[serde(default)]
    pub fertility: FertilityConstants,
    #[serde(default)]
    pub fulfillment: FulfillmentConstants,
    #[serde(default)]
    pub influence_maps: InfluenceMapConstants,
    /// Ticket 127 — per-practice JointIntention knobs. For Courtship
    /// these subsume the prior `PairingConstants` (deleted in Commit C)
    /// plus a `stage_stall_ticks` field gating the novel `StageStalled`
    /// drop branch. `#[serde(default)]` so old archive headers
    /// (pre-127) deserialize without the new field; the unknown
    /// `pairing` field in old headers is silently dropped by serde's
    /// default leniency.
    #[serde(default)]
    pub practices: PracticeConstants,
    #[serde(default)]
    pub planning_substrate: PlanningSubstrateConstants,
    /// Ticket 103 — `escape_viability` perception scalar tunables.
    #[serde(default)]
    pub escape_viability: EscapeViabilityConstants,
    /// Ticket 258 — C3 subjective belief substrate tunables. Per-facet
    /// EMA rates and decay-to-prior rates, species-violence priors for
    /// `<Predator>` initialization, and the passive-decay stagger period.
    #[serde(default)]
    pub beliefs: BeliefsConstants,
    /// Ticket 294 — colony-level aggregation of per-cat `LocationBeliefs`
    /// for substrate readers (e.g. ward placement) that need a colony
    /// view of the C3 facets. Ships with permissive defaults so the
    /// `RecentAmbushMap` retirement preserves pre-294 semantics; tuning
    /// belongs to 291's full ColonyKnowledge restructure.
    #[serde(default)]
    pub belief_aggregation: BeliefAggregationConstants,
    /// Ticket 374 — per-cat shelter belief tunables. Four-axis EMA rates,
    /// continuity accrual/decay, and the Phase C welfare/pressure
    /// composition weights. `#[serde(default)]` so pre-374 events.jsonl
    /// headers deserialize cleanly.
    #[serde(default)]
    pub shelter_beliefs: ShelterBeliefConstants,
    /// Ticket 261 — ActionAffordances substrate. Per-action heuristic
    /// weights + min-eligibility threshold + global sensing range. Five
    /// family-grouped sub-structs; each kind carries an `AffordanceWeights`
    /// quad. Lands as a behavior-neutral substrate (no DSE consumers at
    /// land) — defaults are plausible v1 placeholders; tuning belongs to
    /// the consumer tickets (263+) per the four-artifact methodology.
    #[serde(default)]
    pub affordances: AffordancesConstants,
    /// Ticket 364 — `rear_kitten` HTN method thresholds. Maturity bands
    /// for Wean / Teach / Release sub-goals + teach-phase curriculum size
    /// (substrate-only at 364 land; richer skill attribution lives downstream).
    #[serde(default)]
    pub kitten_rearing: KittenRearingConstants,
    /// Ticket 400 — L2 ParentingActivity tunables. Five-scale composition
    /// asymptote weights (presence/provision/protection/cultural/autonomy),
    /// engagement EMA rates, proximity gating range, matured-residual factor,
    /// and JointIntention coordination-suppression factor. Per the 399
    /// design plan, these are starting-point values; tuning belongs to
    /// follow-on 408 under the four-artifact methodology.
    #[serde(default)]
    pub parenting: ParentingActivityConstants,
    /// Ticket 367 — Phase 1b preservation tunables. Drying/smoking
    /// durations, smoking tend cadence, organ-drop rate from hunts,
    /// and the per-eat mood bump for organ-derived food. `#[serde(default)]`
    /// so pre-367 events.jsonl headers deserialize cleanly.
    #[serde(default)]
    pub crafting: CraftingConstants,
    /// Ticket 100 — tremor influence-map tunables. Action-keyed
    /// emission multipliers + deposit / decay / detection-threshold
    /// scalars. `#[serde(default)]` so pre-100 archive headers
    /// deserialize cleanly.
    #[serde(default)]
    pub tremor: TremorConstants,
    /// Ticket 375 — per-species guaranteed prey-byproduct tables. Each
    /// successful kill in `resolve_engage_prey` spawns the species' meat
    /// plus this list (Bone, Sinew, Hide, Feather, FishScale, Tallow,
    /// RawOrgan, Whisker). Independent of `crafting.organ_drop_chance`,
    /// which is 367's probabilistic mammal+bird organ roll and continues
    /// to fire on top of the guaranteed list. `#[serde(default)]` so
    /// pre-375 archives deserialize cleanly.
    #[serde(default)]
    pub prey_byproducts: PreyByproductConstants,
    /// Ticket 101 — five-axis environmental quality influence maps
    /// (comfort / cleanliness / beauty / mystery / corruption). Per-source
    /// radii + peak values, personality scaling factors, combination
    /// weight, and clamp bounds. `#[serde(default)]` so pre-101 archive
    /// headers deserialize cleanly.
    #[serde(default)]
    pub environmental_quality: EnvironmentalQualityConstants,
    /// Ticket 279 — play-engagement cue emission tunables (PlayBow,
    /// ReciprocalAdvance, SustainedCoPresence). Eligibility thresholds,
    /// candidate ranges, probabilistic emit chance, and cooldowns. The
    /// *belief lift* magnitude lives on `BeliefsConstants` (which
    /// `belief_integrator` already reads); only the *emit cadence*
    /// knobs live here. `#[serde(default)]` so pre-279 archives
    /// deserialize cleanly.
    #[serde(default)]
    pub play_cue_emission: PlayCueEmissionConstants,
    /// Founder relationship init — fondness / familiarity floors authored
    /// by `Relationships::init_pair` at world setup. Lifted off cold-strangers
    /// random `[-0.2, 0.3)` × `[0.1, 0.3)` to encode the design intent that
    /// founders share history. `#[serde(default)]` so pre-existing event-log
    /// headers deserialize cleanly.
    #[serde(default)]
    pub relationships: RelationshipsConstants,
}

// ---------- NeedsConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NeedsConstants {
    // --- RatePerDay rates (ticket 033 Phase 2) ---
    pub hunger_decay: RatePerDay,
    pub energy_decay: RatePerDay,
    pub base_temperature_drain: RatePerDay,
    pub weather_temperature_snow: RatePerDay,
    pub weather_temperature_storm: RatePerDay,
    pub weather_temperature_wind: RatePerDay,
    pub weather_temperature_heavy_rain: RatePerDay,
    pub weather_temperature_light_rain: RatePerDay,
    pub season_temperature_winter: RatePerDay,
    pub season_temperature_autumn: RatePerDay,
    /// Health drained per in-game day while a cat is at `hunger == 0.0`
    /// (the starvation cliff in `src/systems/needs.rs:95`). At
    /// `RatePerDay::new(0.5)` (0.0005/tick at the default 1000
    /// ticks/day scale), a continuously-starving cat dies from this
    /// drain in ~2 in-sim days. Pair with `starvation_safety_drain`
    /// and `starvation_mood_penalty` for the full cascade. Under
    /// graded mode (ticket 032) this rate is multiplied by
    /// `cliff_factor` and the matching `starvation_drain_multiplier_*`
    /// life-stage knob.
    pub starvation_health_drain: RatePerDay,
    pub starvation_safety_drain: RatePerDay,
    pub safety_recovery_rate: RatePerDay,
    pub social_base_drain: RatePerDay,
    pub acceptance_base_drain: RatePerDay,
    pub respect_base_drain: RatePerDay,
    pub mastery_base_drain: RatePerDay,
    pub purpose_base_drain: RatePerDay,
    pub tradition_safety_boost: RatePerDay,
    pub tradition_safety_drain: RatePerDay,
    // --- Grooming ---
    pub grooming_decay: RatePerDay,
    pub grooming_pride_penalty_scale: RatePerDay,
    // --- Mating ---
    pub mating_base_decay: RatePerDay,
    // --- Bond proximity ---
    pub bond_proximity_social_rate: RatePerDay,
    // --- Scalar tuning (non-temporal) ---
    pub starvation_mood_penalty: f32,
    pub starvation_social_multiplier: f32,
    pub social_sociability_scale: f32,
    pub acceptance_temperature_scale: f32,
    pub respect_ambition_scale: f32,
    pub respect_low_threshold: f32,
    pub pride_amplifier_scale: f32,
    pub mastery_diligence_scale: f32,
    pub purpose_curiosity_scale: f32,
    pub purpose_patience_scale: f32,
    pub purpose_independence_scale: f32,
    pub eat_from_inventory_threshold: f32,
    /// Scales food_value reduction from tile corruption (e.g. 0.5 = half nourishment at full corruption).
    pub corruption_food_penalty: f32,
    pub mating_temperature_scale: f32,
    // --- Ticket 032: graded starvation cliff (ship-inert) ---
    /// Ticket 032 — selects the starvation cascade form. **Default `true`**
    /// (ship inert) reproduces the legacy all-or-nothing `hunger == 0.0`
    /// cliff exactly: `cliff_factor = 1.0` iff `hunger == 0.0`, else `0.0`.
    /// Treatment override `false` enables graded mode where the cliff
    /// factor ramps from 0 (at `hunger = starvation_cliff_threshold`) to
    /// 1 (at `hunger = 0`) via `((threshold − hunger) / threshold)^k`.
    /// Drain is **zero** above the threshold — the normal feeding range
    /// does not damage health.
    pub starvation_cliff_use_legacy: bool,
    /// Ticket 032 — exponent on the graded `(deficit / threshold)^k`
    /// cliff curve (only consulted when `starvation_cliff_use_legacy =
    /// false`). `1.0` ⇒ linear ramp from 0 (at `hunger = threshold`) to
    /// 1 (at `hunger = 0`); `2.0` ⇒ quadratic — gentler near the
    /// threshold edge, sharper near zero. Default `2.0`.
    pub starvation_cliff_exponent: f32,
    /// Ticket 032 — hunger value above which the graded starvation
    /// cascade does **not** fire. Only consulted when
    /// `starvation_cliff_use_legacy = false`. The cliff factor is zero
    /// for `hunger >= starvation_cliff_threshold` and ramps up below.
    ///
    /// **Default `0.15` — mirrors `critical_hunger_interrupt_threshold`**
    /// (the Maslow tier where the planner already interrupts the cat's
    /// current plan to send her to eat). The composition is intentional:
    /// at hunger=0.6 the cat feels urgency (`hunger_urgency_threshold`);
    /// at hunger=0.5 the planner stops calling hunger "OK"
    /// (`planner_hunger_ok_threshold`); at hunger=0.15 the planner
    /// interrupts whatever she's doing to eat *now*. Health damage
    /// engaging at the same boundary models the real-cat ladder: brief
    /// mid-hunger excursions are normal (Maslow tier 1 driving
    /// behavior), only sustained sub-critical hunger (the cat has
    /// **failed** to recover despite the planner trying) constitutes
    /// starvation.
    ///
    /// **Earlier scaffolding used `(1 − hunger)^k` directly**, which has
    /// no zero except at hunger=1.0 — over a 1.34M-tick soak the
    /// integrated drain killed even well-fed cats (focal-cat trace on
    /// Mocha: hunger min=0.50, max=0.99, final=0.84, yet "starved").
    /// See `docs/balance/starvation-rebalance.md` Iter 4.
    pub starvation_cliff_threshold: f32,
    /// Ticket 032 — under graded mode, the persistent starvation mood
    /// modifier only fires when `cliff_factor > starvation_mood_threshold`
    /// (so brief mid-hunger dips don't spam mood-modifier creation).
    /// Default `0.0` ⇒ mood fires whenever the cliff is non-zero, matching
    /// legacy semantics where any starving tick lands the modifier.
    /// Treatment value `0.5` reserves the mood penalty for the deeper
    /// half of the cliff curve.
    pub starvation_mood_threshold: f32,
    /// Ticket 032 — per-life-stage multipliers on starvation health and
    /// safety drains. **All default to `1.0` (ship inert).** Treatment
    /// override per ticket §Scope item 2: kittens 2.0×, young 1.3×,
    /// adults 1.0×, elders 1.5× — kittens and elders are far more
    /// vulnerable to acute hunger than prime adults in real cat biology.
    /// Compounds with `cliff_factor` (Step 2) when graded mode is on.
    pub starvation_drain_multiplier_kitten: f32,
    pub starvation_drain_multiplier_young: f32,
    pub starvation_drain_multiplier_adult: f32,
    pub starvation_drain_multiplier_elder: f32,
    /// Ticket 032 — under graded-cliff mode (`starvation_cliff_use_legacy
    /// = false`), the death-cause discriminator attributes a death to
    /// `DeathCause::Starvation` when `Health.total_starvation_damage`
    /// exceeds this threshold. **Default `0.1`** — a cat that has lost
    /// more than 10% of max health to the starvation cascade counts as
    /// starved (the graded curve smears damage across many ticks at
    /// non-zero hunger, so a discrete threshold replaces the legacy
    /// `hunger == 0.0` discriminator). Ignored under legacy mode.
    pub starvation_attribution_threshold: f32,
    // --- Counts / distances ---
    pub starvation_mood_ticks: u64,
    pub tradition_familiar_distance: f32,
    pub bond_proximity_range: f32,
}

impl Default for NeedsConstants {
    fn default() -> Self {
        Self {
            // RatePerDay rates (per_day = per_tick * 1000)
            hunger_decay: RatePerDay::new(0.1),
            energy_decay: RatePerDay::new(0.1),
            base_temperature_drain: RatePerDay::new(0.1),
            weather_temperature_snow: RatePerDay::new(0.4),
            weather_temperature_storm: RatePerDay::new(0.3),
            weather_temperature_wind: RatePerDay::new(0.2),
            weather_temperature_heavy_rain: RatePerDay::new(0.2),
            weather_temperature_light_rain: RatePerDay::new(0.1),
            season_temperature_winter: RatePerDay::new(0.3),
            season_temperature_autumn: RatePerDay::new(0.1),
            starvation_health_drain: RatePerDay::new(0.5),
            starvation_safety_drain: RatePerDay::new(0.5),
            safety_recovery_rate: RatePerDay::new(0.2),
            social_base_drain: RatePerDay::new(0.1),
            acceptance_base_drain: RatePerDay::new(0.05),
            respect_base_drain: RatePerDay::new(0.03),
            mastery_base_drain: RatePerDay::new(0.02),
            purpose_base_drain: RatePerDay::new(0.01),
            tradition_safety_boost: RatePerDay::new(0.2),
            tradition_safety_drain: RatePerDay::new(0.1),
            grooming_decay: RatePerDay::new(0.03),
            grooming_pride_penalty_scale: RatePerDay::new(0.05),
            mating_base_decay: RatePerDay::new(0.08),
            bond_proximity_social_rate: RatePerDay::new(0.3),
            // Scalar tuning
            starvation_mood_penalty: -0.3,
            starvation_social_multiplier: 2.0,
            social_sociability_scale: 0.5,
            acceptance_temperature_scale: 0.5,
            respect_ambition_scale: 0.5,
            respect_low_threshold: 0.4,
            pride_amplifier_scale: 0.8,
            mastery_diligence_scale: 0.5,
            purpose_curiosity_scale: 0.5,
            purpose_patience_scale: 0.3,
            purpose_independence_scale: 0.4,
            eat_from_inventory_threshold: 0.4,
            corruption_food_penalty: 0.5,
            mating_temperature_scale: 0.5,
            // Ticket 032: graded starvation cliff (ship-inert defaults)
            starvation_cliff_use_legacy: true,
            starvation_cliff_exponent: 2.0,
            starvation_cliff_threshold: 0.15,
            starvation_mood_threshold: 0.0,
            starvation_drain_multiplier_kitten: 1.0,
            starvation_drain_multiplier_young: 1.0,
            starvation_drain_multiplier_adult: 1.0,
            starvation_drain_multiplier_elder: 1.0,
            starvation_attribution_threshold: 0.1,
            // Counts / distances
            starvation_mood_ticks: 5,
            tradition_familiar_distance: 5.0,
            bond_proximity_range: 3.0,
        }
    }
}

// ---------- BuildingConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BuildingConstants {
    // --- RatePerDay rates (ticket 033 Phase 2) ---
    pub den_temperature_bonus: RatePerDay,
    pub den_safety_bonus: RatePerDay,
    pub hearth_social_bonus: RatePerDay,
    pub hearth_temperature_bonus_cold: RatePerDay,
    pub dirty_temperature_drain: RatePerDay,
    pub structural_decay_storm: RatePerDay,
    pub structural_decay_snow: RatePerDay,
    pub structural_decay_heavy_rain: RatePerDay,
    pub cleanliness_decay_storm: RatePerDay,
    pub cleanliness_decay_snow: RatePerDay,
    pub cleanliness_decay_fog: RatePerDay,
    pub cleanliness_decay_clear: RatePerDay,
    pub tidy_cleanliness_rate: RatePerDay,
    // --- Scalars ---
    pub stores_spoilage_multiplier: f32,
    pub dirty_threshold: f32,
    pub gate_tired_energy_threshold: f32,
    pub gate_tired_diligence_scale: f32,
    pub gate_close_diligence_threshold: f32,
    // --- Radii ---
    pub den_effect_radius: f32,
    pub hearth_effect_radius: f32,
    pub dirty_discomfort_radius: f32,
    pub tidy_radius: f32,
}

impl Default for BuildingConstants {
    fn default() -> Self {
        Self {
            // RatePerDay rates (per_day = per_tick * 1000)
            den_temperature_bonus: RatePerDay::new(3.0),
            den_safety_bonus: RatePerDay::new(0.5),
            hearth_social_bonus: RatePerDay::new(1.0),
            hearth_temperature_bonus_cold: RatePerDay::new(3.0),
            dirty_temperature_drain: RatePerDay::new(0.3),
            structural_decay_storm: RatePerDay::new(0.03),
            structural_decay_snow: RatePerDay::new(0.02),
            structural_decay_heavy_rain: RatePerDay::new(0.01),
            cleanliness_decay_storm: RatePerDay::new(0.2),
            cleanliness_decay_snow: RatePerDay::new(0.15),
            cleanliness_decay_fog: RatePerDay::new(0.1),
            cleanliness_decay_clear: RatePerDay::new(0.08),
            tidy_cleanliness_rate: RatePerDay::new(0.5),
            // Scalars
            stores_spoilage_multiplier: 0.5,
            dirty_threshold: 0.3,
            gate_tired_energy_threshold: 0.3,
            gate_tired_diligence_scale: 0.6,
            gate_close_diligence_threshold: 0.5,
            // Radii
            den_effect_radius: 5.0,
            hearth_effect_radius: 6.0,
            dirty_discomfort_radius: 3.0,
            tidy_radius: 3.0,
        }
    }
}

// ---------- CombatConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CombatConstants {
    pub jitter_range: f32,
    pub combat_effective_hunting_weight: f32,
    pub ally_damage_bonus_per_ally: f32,
    /// Extra damage bonus per ally stacked on top of `ally_damage_bonus_per_ally`
    /// when 2+ cats coordinate an attack on the same target (a "posse").
    /// Rewards the colony for collective offense, not just individual ganking.
    pub combat_posse_bonus_per_ally: f32,
    /// Minimum ally count (including the attacking cat) for the posse bonus
    /// to activate. A lone ganker doesn't get the posse multiplier.
    pub combat_posse_min_allies: usize,
    /// Posse attacks at or below this HP fraction trigger banishment instead
    /// of a normal kill: shadow-fox dissolves into mist, posse earns a
    /// Legend-tier event and stat boons. See src/steps/combat/banishment.rs.
    pub shadow_fox_banish_threshold: f32,
    /// Tiles within which cats can "witness" a banishment and receive the
    /// secondhand memory + mood boost.
    pub legend_witness_range: f32,
    /// Combat skill delta applied to each posse participant at banishment.
    pub banishment_combat_skill_grow: f32,
    /// Diminishing-returns factor on repeat banishments. Effective gain is
    /// `banishment_combat_skill_grow / (1 + prior_triumphs * factor)`, so a
    /// cat with N prior banishments earns progressively less from each
    /// subsequent one. Prevents one cat (see: Mocha) from accumulating
    /// runaway combat skill across a long game while keeping the first
    /// banishment meaningful. Set to 0.0 to restore linear gain.
    pub banishment_skill_gain_diminish_factor: f32,
    /// Valor mood modifier amount for posse participants (duration = seasons × 2).
    pub banishment_valor_mood: f32,
    /// Mood modifier amount for witnesses of a banishment.
    pub banishment_witness_mood: f32,
    /// Safety floor for witnesses — they saw the darkness defeated.
    pub banishment_witness_safety_floor: f32,
    /// Corruption pushback radius from banishment site.
    pub banishment_pushback_radius: f32,
    /// Corruption pushback amount.
    pub banishment_pushback_amount: f32,
    pub temper_damage_bonus: f32,
    pub narrative_attack_chance: f32,
    pub wildlife_flee_health_threshold: f32,
    pub wildlife_flee_outnumbered_count: usize,
    pub injury_negligible_threshold: f32,
    pub injury_moderate_threshold: f32,
    pub injury_severe_threshold: f32,
    pub injury_minor_health_penalty: f32,
    pub injury_moderate_health_penalty: f32,
    pub injury_severe_health_penalty: f32,
    pub memory_strength_minor: f32,
    pub memory_strength_moderate: f32,
    pub memory_strength_severe: f32,
    pub combat_skill_growth: f32,
    pub morale_hp_weight: f32,
    pub morale_boldness_weight: f32,
    pub morale_temper_weight: f32,
    pub morale_ally_weight: f32,
    pub morale_loyalty_weight: f32,
    pub morale_flee_threshold: f32,
    pub flee_mood_penalty: f32,
    pub victory_respect_gain: f32,
    pub victory_safety_gain: f32,
    pub victory_mood_bonus: f32,
    // --- DurationDays durations (ticket 033 Phase 4) ---
    #[serde(alias = "flee_mood_ticks")]
    pub flee_mood_duration: DurationDays,
    #[serde(alias = "victory_mood_ticks")]
    pub victory_mood_duration: DurationDays,
    #[serde(alias = "heal_duration_minor")]
    pub heal_minor_duration: DurationDays,
    #[serde(alias = "heal_duration_moderate")]
    pub heal_moderate_duration: DurationDays,
    #[serde(alias = "heal_duration_severe")]
    pub heal_severe_duration: DurationDays,

    // --- Body-zone substrate (ticket 095 Phase 1) ---
    /// Cat is incapacitated (Idle/Sleep/treatment only) when
    /// `total_pain / max_possible_pain >= pain_incapacitation_threshold`.
    /// Spec §Cat Pain System default 0.9 (normalized). Replaces the legacy
    /// "any `InjuryKind::Severe && !healed`" predicate in Stage B.
    #[serde(default = "default_pain_incapacitation_threshold")]
    pub pain_incapacitation_threshold: f32,
    /// Per-part pain weight indexed by `BodyPart::index()`. Sum is the
    /// effective `max_possible_pain` used by `health_derived`. Spec §Cat
    /// Pain System.
    #[serde(default = "default_body_zone_pain_weights")]
    pub body_zone_pain_weights: [f32; 13],
    /// Lower bounds for `PartCondition` promotion, in order:
    /// [Bruised, Wounded, Mangled, Destroyed]. Spec §Cat Condition Thresholds.
    #[serde(default = "default_body_zone_condition_thresholds")]
    pub body_zone_condition_thresholds: [f32; 4],
    /// Whether a part stays Destroyed forever once it first reaches that
    /// tier. Indexed by `BodyPart::index()`. Spec §Cat Healing Rates
    /// (Permanent column).
    #[serde(default = "default_body_zone_permanent_at_destroyed")]
    pub body_zone_permanent_at_destroyed: [bool; 13],
    /// Per-(category × condition-transition) healing durations. Tick
    /// conversion happens once at system construction; per-tick
    /// `1.0 / duration_ticks` is the per-part decrement. Spec §Cat
    /// Healing Rates.
    #[serde(default)]
    pub body_zone_healing: BodyZoneHealing,

    // --- Equipment-effect substrate (ticket 477) ---
    /// Quality multiplier floor for equipment effects. A freshly-failed
    /// craft (quality≈0) still contributes `floor × base_magnitude`; a
    /// perfect craft (quality=1) contributes the full magnitude. Linear
    /// curve: `effective = base * (floor + (1 - floor) * quality)`.
    /// Single curve shared across every equipment classifier so the
    /// per-magnitude tuning constants below stay legible in isolation.
    #[serde(default = "default_equipment_quality_floor")]
    pub equipment_quality_floor: f32,
    /// Fraction of incoming blunt damage absorbed by HideBracers at
    /// quality `1.0`. Read by `damage_to_body_part` via the equipment
    /// aggregation layer.
    #[serde(default = "default_armor_blunt_absorb_magnitude")]
    pub armor_blunt_absorb_magnitude: f32,
    /// Blunt-damage contribution of HidePlatedWrap at quality `1.0`
    /// (composes additively with HideBracers; floor-clamped at
    /// `armor_reduction_floor_blunt`).
    #[serde(default = "default_armor_pierce_partial_blunt_magnitude")]
    pub armor_pierce_partial_blunt_magnitude: f32,
    /// Pierce-damage contribution of HidePlatedWrap at quality `1.0`.
    #[serde(default = "default_armor_pierce_partial_pierce_magnitude")]
    pub armor_pierce_partial_pierce_magnitude: f32,
    /// Additive ceiling on blunt armor reduction (prevents stacking
    /// past invulnerability).
    #[serde(default = "default_armor_reduction_floor_blunt")]
    pub armor_reduction_floor_blunt: f32,
    /// Additive ceiling on pierce armor reduction.
    #[serde(default = "default_armor_reduction_floor_pierce")]
    pub armor_reduction_floor_pierce: f32,
    /// Visual-detection mask from a WovenReedCloak at quality `1.0`.
    /// Multiplies the *sight* component of prey detection (before its
    /// `max` with tremor). Read by `try_detect_cat`.
    #[serde(default = "default_cloak_visual_mask_magnitude")]
    pub cloak_visual_mask_magnitude: f32,
    /// Strike-success bonus applied to the catch threshold when the cat
    /// wields a `WeaponClass::Pierce` weapon at quality `1.0`. Read by
    /// `resolve_engage_prey` at pounce-success eval.
    #[serde(default = "default_hunt_strike_pierce_bonus")]
    pub hunt_strike_pierce_bonus: f32,
    /// Strike-success bonus for `WeaponClass::Slash` at quality `1.0`.
    #[serde(default = "default_hunt_strike_slash_bonus")]
    pub hunt_strike_slash_bonus: f32,
    /// Strike-success bonus for `WeaponClass::Blunt` at quality `1.0`.
    #[serde(default = "default_hunt_strike_blunt_bonus")]
    pub hunt_strike_blunt_bonus: f32,
    /// Per-strike probability that a `DurabilityTier::Fragile` weapon
    /// (the three bone weapons) snaps on a *failed* strike. On snap, the
    /// weapon is removed from the cat's inventory and
    /// `Feature::BoneWeaponSnapped` fires.
    #[serde(default = "default_bone_weapon_snap_chance_on_miss")]
    pub bone_weapon_snap_chance_on_miss: f32,
    /// Tremor floor lifted by carrying `NoiseClass::Loud` kit. Replaces
    /// the action-derived tremor multiplier inside `try_detect_cat` so a
    /// patient stalker carrying loud metal can still be heard. Phase 2b
    /// items are all Silent; 370's metal items flip this read live.
    #[serde(default = "default_noise_class_loud_tremor_floor")]
    pub noise_class_loud_tremor_floor: f32,
}

fn default_pain_incapacitation_threshold() -> f32 {
    0.9
}

// Equipment-effect defaults (ticket 477). Conservative initial magnitudes —
// balance iteration follows in `docs/balance/equipment-effects.md` gated on
// the post-477 soak verdict.
fn default_equipment_quality_floor() -> f32 {
    0.25
}
fn default_armor_blunt_absorb_magnitude() -> f32 {
    0.20
}
fn default_armor_pierce_partial_blunt_magnitude() -> f32 {
    0.30
}
fn default_armor_pierce_partial_pierce_magnitude() -> f32 {
    0.10
}
fn default_armor_reduction_floor_blunt() -> f32 {
    0.50
}
fn default_armor_reduction_floor_pierce() -> f32 {
    0.40
}
fn default_cloak_visual_mask_magnitude() -> f32 {
    0.35
}
fn default_hunt_strike_pierce_bonus() -> f32 {
    0.12
}
fn default_hunt_strike_slash_bonus() -> f32 {
    0.10
}
fn default_hunt_strike_blunt_bonus() -> f32 {
    0.08
}
fn default_bone_weapon_snap_chance_on_miss() -> f32 {
    0.04
}
fn default_noise_class_loud_tremor_floor() -> f32 {
    1.2
}

fn default_body_zone_pain_weights() -> [f32; 13] {
    [
        0.5, // Whiskers
        0.5, // Ears
        1.5, // Mouth/Jaw
        0.8, // Scruff
        3.0, // Throat
        1.5, // Flanks
        0.8, // Belly
        1.0, // Front-left paw
        1.0, // Front-right paw
        1.0, // Rear-left paw
        1.0, // Rear-right paw
        2.0, // Haunches
        0.5, // Tail
    ]
}

fn default_body_zone_condition_thresholds() -> [f32; 4] {
    [0.01, 0.26, 0.61, 0.91]
}

fn default_body_zone_permanent_at_destroyed() -> [bool; 13] {
    [
        false, // Whiskers — regrow
        true,  // Ears — torn tips persist as identity scar
        true,  // Mouth/Jaw — permanent if Destroyed
        false, // Scruff
        false, // Throat — fatal before Destroyed
        false, // Flanks
        false, // Belly
        false, // Front-left paw
        false, // Front-right paw
        false, // Rear-left paw
        false, // Rear-right paw
        true,  // Haunches — permanent limp
        true,  // Tail — permanent crook
    ]
}

/// Healing durations per (category × condition-transition). At default
/// 1000-ticks/day scale, the day fractions below recover the spec's raw-tick
/// numerics (30 ticks = 0.03 days, etc).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BodyZoneHealing {
    pub soft_bruised_to_healthy: DurationDays,
    pub soft_wounded_to_bruised: DurationDays,
    pub soft_mangled_to_wounded: DurationDays,
    pub structural_bruised_to_healthy: DurationDays,
    pub structural_wounded_to_bruised: DurationDays,
    pub structural_mangled_to_wounded: DurationDays,
    pub sensory_bruised_to_healthy: DurationDays,
    pub sensory_wounded_to_bruised: DurationDays,
    pub sensory_mangled_to_wounded: DurationDays,
    pub throat_bruised_to_healthy: DurationDays,
    pub throat_wounded_to_bruised: DurationDays,
    pub tail_bruised_to_healthy: DurationDays,
    pub tail_wounded_to_bruised: DurationDays,
    pub tail_mangled_to_wounded: DurationDays,
    /// 472 — heal-rate multiplier for parts in `WoundKind::Festering`.
    /// Applied multiplicatively to the per-tick `tissue_damage`
    /// decrement: `decrement *= festering_heal_rate_multiplier`. At
    /// 0.05 a festering wound recovers ~20× more slowly than a normal
    /// wound of the same category × condition. Pulls the Ashitaka
    /// anchor onto a concrete substrate knob: cure only advances
    /// meaningfully via intervention (the kin-care surface in
    /// 473/474 wires the active healing path). Future flavors (Frozen,
    /// Poisoned) add one new f32 each — no per-(category, condition,
    /// kind) field explosion.
    #[serde(default = "default_festering_heal_rate_multiplier")]
    pub festering_heal_rate_multiplier: f32,
}

fn default_festering_heal_rate_multiplier() -> f32 {
    0.05
}

fn default_misfire_festering_chance() -> f32 {
    0.5
}

fn default_festering_seed_damage() -> f32 {
    0.10
}

fn default_festering_observation_interval_ticks() -> u64 {
    200
}

fn default_siege_fear_ramp_ticks() -> u64 {
    60
}

fn default_siege_fear_radius() -> f32 {
    6.0
}

fn default_ward_siege_fear_weight() -> f32 {
    0.0
}

impl Default for BodyZoneHealing {
    fn default() -> Self {
        Self {
            soft_bruised_to_healthy: DurationDays::new(0.03),
            soft_wounded_to_bruised: DurationDays::new(0.08),
            soft_mangled_to_wounded: DurationDays::new(0.20),
            structural_bruised_to_healthy: DurationDays::new(0.05),
            structural_wounded_to_bruised: DurationDays::new(0.15),
            structural_mangled_to_wounded: DurationDays::new(0.40),
            sensory_bruised_to_healthy: DurationDays::new(0.04),
            sensory_wounded_to_bruised: DurationDays::new(0.12),
            sensory_mangled_to_wounded: DurationDays::new(0.30),
            // Throat: Mangled→Wounded is "N/A — fatal before Mangled without
            // treatment" per spec. Phase 1 leaves the field absent.
            throat_bruised_to_healthy: DurationDays::new(0.04),
            throat_wounded_to_bruised: DurationDays::new(0.10),
            tail_bruised_to_healthy: DurationDays::new(0.03),
            tail_wounded_to_bruised: DurationDays::new(0.08),
            tail_mangled_to_wounded: DurationDays::new(0.20),
            festering_heal_rate_multiplier: default_festering_heal_rate_multiplier(),
        }
    }
}

impl Default for CombatConstants {
    fn default() -> Self {
        Self {
            jitter_range: 0.02,
            combat_effective_hunting_weight: 0.3,
            ally_damage_bonus_per_ally: 0.2,
            combat_posse_bonus_per_ally: 0.4,
            combat_posse_min_allies: 2,
            // Banish at 80% HP: shadow-foxes are spectral, not bodies — the
            // first real blow from a cat breaks the ambush aura and begins
            // the dissolution. Keeps above `wildlife_flee_health_threshold`
            // (0.3) so the fox doesn't run before the cat can finish it.
            shadow_fox_banish_threshold: 0.8,
            legend_witness_range: 12.0,
            banishment_combat_skill_grow: 0.25,
            banishment_skill_gain_diminish_factor: 0.25,
            banishment_valor_mood: 0.35,
            banishment_witness_mood: 0.20,
            banishment_witness_safety_floor: 0.8,
            banishment_pushback_radius: 20.0,
            banishment_pushback_amount: 0.5,
            temper_damage_bonus: 0.15,
            narrative_attack_chance: 0.15,
            wildlife_flee_health_threshold: 0.3,
            // A 2-cat posse already qualifies as "outnumbered" for a shadow-fox.
            // Combined with the posse pressure banishment trigger, this means
            // a duo is usually enough to force the fox into dissolution.
            wildlife_flee_outnumbered_count: 2,
            injury_negligible_threshold: 0.03,
            injury_moderate_threshold: 0.1,
            injury_severe_threshold: 0.25,
            injury_minor_health_penalty: 0.03,
            injury_moderate_health_penalty: 0.08,
            injury_severe_health_penalty: 0.15,
            memory_strength_minor: 0.5,
            memory_strength_moderate: 0.8,
            memory_strength_severe: 1.0,
            combat_skill_growth: 0.02,
            morale_hp_weight: 0.4,
            morale_boldness_weight: 0.2,
            morale_temper_weight: 0.1,
            morale_ally_weight: 0.1,
            morale_loyalty_weight: 0.2,
            morale_flee_threshold: 0.4,
            flee_mood_penalty: -0.3,
            victory_respect_gain: 0.1,
            victory_safety_gain: 0.2,
            victory_mood_bonus: 0.3,
            // DurationDays durations (Phase 4) — preserve raw-tick numerics at
            // the default 1000-ticks/day scale: N ticks → N / 1000 days.
            flee_mood_duration: DurationDays::new(0.04),
            victory_mood_duration: DurationDays::new(0.05),
            heal_minor_duration: DurationDays::new(0.05),
            heal_moderate_duration: DurationDays::new(0.2),
            heal_severe_duration: DurationDays::new(0.5),
            pain_incapacitation_threshold: default_pain_incapacitation_threshold(),
            body_zone_pain_weights: default_body_zone_pain_weights(),
            body_zone_condition_thresholds: default_body_zone_condition_thresholds(),
            body_zone_permanent_at_destroyed: default_body_zone_permanent_at_destroyed(),
            body_zone_healing: BodyZoneHealing::default(),
            equipment_quality_floor: default_equipment_quality_floor(),
            armor_blunt_absorb_magnitude: default_armor_blunt_absorb_magnitude(),
            armor_pierce_partial_blunt_magnitude: default_armor_pierce_partial_blunt_magnitude(),
            armor_pierce_partial_pierce_magnitude: default_armor_pierce_partial_pierce_magnitude(),
            armor_reduction_floor_blunt: default_armor_reduction_floor_blunt(),
            armor_reduction_floor_pierce: default_armor_reduction_floor_pierce(),
            cloak_visual_mask_magnitude: default_cloak_visual_mask_magnitude(),
            hunt_strike_pierce_bonus: default_hunt_strike_pierce_bonus(),
            hunt_strike_slash_bonus: default_hunt_strike_slash_bonus(),
            hunt_strike_blunt_bonus: default_hunt_strike_blunt_bonus(),
            bone_weapon_snap_chance_on_miss: default_bone_weapon_snap_chance_on_miss(),
            noise_class_loud_tremor_floor: default_noise_class_loud_tremor_floor(),
        }
    }
}

// ---------- MagicConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MagicConstants {
    // --- RatePerDay rates (ticket 033 Phase 2) ---
    pub corruption_spread_rate: RatePerDay,
    pub healing_poultice_rate: RatePerDay,
    pub energy_tonic_rate: RatePerDay,
    /// Per-day strength loss for a thornward at the unsieged baseline
    /// (`Ward::strength` starts at 1.0; decay is linear in
    /// `src/systems/magic.rs:158`). `RatePerDay::new(1.0)` = 1-day
    /// lifetime, `RatePerDay::new(0.5)` = 2-day, etc. Under siege,
    /// `effective_decay += siege_pressure × ward_siege_decay_bonus` so
    /// a sieged ward burns faster than the baseline lifetime suggests.
    /// The wrapper converts to per-tick at ward-spawn time
    /// (`src/steps/magic/set_ward.rs`); the resulting per-tick rate
    /// is stored on `Ward::decay_rate`.
    pub thornward_decay_rate: RatePerDay,
    pub cleanse_corruption_rate: RatePerDay,
    pub cleanse_personal_corruption_rate: RatePerDay,
    /// Health drain per in-game day on tiles with corruption > 0.8.
    /// Per-day = per-tick × 1000 at the default scale.
    pub corruption_health_drain: RatePerDay,

    // --- DurationDays durations (ticket 033 Phase 3) ---
    /// Duration of the mood-tonic positive modifier applied on remedy use.
    /// Pre-ticket-033 value was `500` raw ticks (= 0.5 days at default scale).
    #[serde(alias = "mood_tonic_ticks")]
    pub mood_tonic_duration: DurationDays,
    /// Duration of the mood penalty applied each time personal corruption
    /// triggers a mood roll. Pre-ticket-033 value was `10` raw ticks.
    #[serde(alias = "personal_corruption_mood_ticks")]
    pub personal_corruption_mood_duration: DurationDays,
    /// Duration of the mood penalty applied per tick a cat stands on a
    /// corrupted tile. Pre-ticket-033 value was `5` raw ticks.
    #[serde(alias = "corruption_tile_mood_ticks")]
    pub corruption_tile_mood_duration: DurationDays,
    /// Time required to gather one herb. Pre-ticket-033 value was `5` raw ticks.
    #[serde(alias = "gather_herb_ticks")]
    pub gather_herb_duration: DurationDays,
    /// Time required to prepare a remedy at a workshop. Pre-ticket-033 value
    /// was `10` raw ticks.
    #[serde(alias = "prepare_remedy_ticks_workshop")]
    pub prepare_remedy_duration_workshop: DurationDays,
    /// Time required to prepare a remedy without a workshop. Pre-ticket-033
    /// value was `15` raw ticks.
    #[serde(alias = "prepare_remedy_ticks_default")]
    pub prepare_remedy_duration_default: DurationDays,
    /// Time required to set a ward. Pre-ticket-033 value was `8` raw ticks.
    #[serde(alias = "set_ward_ticks")]
    pub set_ward_duration: DurationDays,
    /// Time required to complete a scrying. Pre-ticket-033 value was `10`
    /// raw ticks.
    #[serde(alias = "scry_ticks")]
    pub scry_duration: DurationDays,
    /// Maximum time spent on a single CleanseCorruption step before the
    /// step Advances even if the tile isn't fully cleansed. Pre-ticket-033
    /// value was `100` raw ticks.
    #[serde(alias = "cleanse_max_ticks")]
    pub cleanse_max_duration: DurationDays,
    /// Time required to complete a SpiritCommunion. Pre-ticket-033 value
    /// was `15` raw ticks.
    #[serde(alias = "spirit_communion_ticks")]
    pub spirit_communion_duration: DurationDays,
    /// Duration of the mood bonus applied on a successful SpiritCommunion.
    /// Pre-ticket-033 value was `100` raw ticks.
    #[serde(alias = "spirit_communion_mood_ticks")]
    pub spirit_communion_mood_duration: DurationDays,
    /// Duration of the mood penalty applied on a misfire fizzle.
    /// Pre-ticket-033 value was `20` raw ticks.
    #[serde(alias = "misfire_fizzle_mood_ticks")]
    pub misfire_fizzle_mood_duration: DurationDays,
    /// Time required to harvest a carcass for shadow bone.
    /// Pre-ticket-033 value was `15` raw ticks.
    #[serde(alias = "harvest_carcass_ticks")]
    pub harvest_carcass_duration: DurationDays,

    // --- IntervalPerDay cadences (ticket 033 Phase 3) ---
    /// Cadence at which `corruption_spread` runs. Pre-ticket-033 value was
    /// `10` (raw ticks; 10×/day at the new 1000-ticks/day scale, behaviour
    /// is preserved by `IntervalPerDay::new(100.0)`). Flagged for follow-on
    /// rebalancing — see ticket 033 spec for context.
    #[serde(alias = "corruption_spread_interval")]
    pub corruption_spread_cadence: IntervalPerDay,
    /// Cadence at which `spawn_shadow_fox_from_corruption` rolls. Pre-ticket-033
    /// value was `10` raw ticks (= 100/day). Flagged for follow-on rebalancing.
    #[serde(alias = "shadow_fox_spawn_interval")]
    pub shadow_fox_spawn_cadence: IntervalPerDay,
    /// Cadence at which herb / flavor-plant growth advances by one stage.
    /// Pre-ticket-033 value was `200` raw ticks (= 5/day).
    #[serde(alias = "herb_growth_interval")]
    pub herb_growth_cadence: IntervalPerDay,
    /// Cadence at which herb regrowth is attempted. Pre-ticket-033 value was
    /// `500` raw ticks (= 2/day).
    #[serde(alias = "herb_regrowth_interval")]
    pub herb_regrowth_cadence: IntervalPerDay,

    // --- Scalar tuning (non-temporal) ---
    pub corruption_spread_threshold: f32,
    pub corruption_new_tile_threshold: f32,
    pub ward_post_decay_multiplier: f32,
    pub mood_tonic_bonus: f32,
    pub personal_corruption_mood_threshold: f32,
    pub personal_corruption_mood_chance: f32,
    pub personal_corruption_mood_penalty: f32,
    pub personal_corruption_erratic_threshold: f32,
    pub personal_corruption_erratic_chance: f32,
    pub corruption_tile_mood_threshold: f32,
    pub corruption_twisted_herb_threshold: f32,
    pub shadow_fox_corruption_threshold: f32,
    pub shadow_fox_spawn_chance: f32,
    pub herbcraft_gather_skill_growth: f32,
    pub herbcraft_prepare_skill_growth: f32,
    pub gratitude_fondness_gain: f32,
    pub herbcraft_apply_skill_growth: f32,
    pub herbcraft_ward_skill_growth: f32,
    pub magic_ward_skill_growth: f32,
    pub scry_memory_strength: f32,
    pub scry_magic_skill_growth: f32,
    pub cleanse_magic_skill_growth: f32,
    pub cleanse_done_threshold: f32,
    pub spirit_communion_mood_bonus: f32,
    pub spirit_communion_skill_growth: f32,
    pub misfire_skill_safe_ratio: f32,
    pub misfire_chance_scale: f32,
    pub misfire_fizzle_threshold: f32,
    pub misfire_corruption_backsplash_threshold: f32,
    pub misfire_inverted_ward_threshold: f32,
    pub misfire_wound_transfer_threshold: f32,
    pub misfire_fizzle_mood_penalty: f32,
    pub misfire_corruption_backsplash_amount: f32,
    /// 472 — conditional chance, given that a `WoundTransfer` misfire
    /// has already rolled, that the synthetic wound is authored as
    /// `WoundKind::Festering` on a randomly-selected body part
    /// (rather than only draining `Health.current`). At default 0.5
    /// this fires on roughly half of WoundTransfer misfires, and
    /// because WoundTransfer is one of five misfire kinds (~20% of
    /// all misfires) the steady-state festering-author rate per
    /// misfire-having cat is ~10%. This is the substrate anchor for
    /// the Ashitaka-from-Princess-Mononoke arc: visible, source-
    /// attributed, progressive wound that drives kin-care behavior.
    #[serde(default = "default_misfire_festering_chance")]
    pub misfire_festering_chance: f32,
    /// 472 — initial `tissue_damage` increment applied to the
    /// randomly-selected body part when a `WoundTransfer` misfire
    /// authors a festering wound. Tuned to land in `Bruised` so the
    /// part is observable but not immediately incapacitating; the
    /// festering kind axis (and its slow heal rate) is what drives
    /// the long-tail behavior, not the initial severity.
    #[serde(default = "default_festering_seed_damage")]
    pub festering_seed_damage: f32,
    /// 472 — cooldown between `CarriesFesteringWound` emits per cat.
    /// Festering is a *persistent state* — witnesses build belief
    /// over many observations of the same wound rather than one
    /// emit-per-tick. Default 200 matches the SustainedCoPresence
    /// cadence order of magnitude (279's affiliative cue rate).
    #[serde(default = "default_festering_observation_interval_ticks")]
    pub festering_observation_interval_ticks: u64,
    /// 470 — number of `WildlifeAiState::EncirclingWard.ticks` over
    /// which the `WardSiegeFearMap` intensity ramps from 0.0 to 1.0
    /// for a single besieged ward. At 60 ticks (~1 in-sim minute at
    /// the canonical 1000-ticks/day scale) a fresh siege reads as
    /// low-intensity; a sustained siege reads as full intensity.
    /// Substrate-active at land but byte-identical because the
    /// consumer DSE weight is 0.0 (the 301 conditional-axis pattern).
    #[serde(default = "default_siege_fear_ramp_ticks")]
    pub siege_fear_ramp_ticks: u64,
    /// 470 — falloff radius in world tiles for siege-fear stamps.
    /// `stamp_siege_at(intensity, radius)` paints a linear falloff so
    /// tiles inside the radius read non-zero. Default 6 — wider than
    /// `ward_repel_radius` (3) by design: cats perceive the siege at
    /// some standoff distance, not only on the besieged tile itself.
    #[serde(default = "default_siege_fear_radius")]
    pub siege_fear_radius: f32,
    /// 470 — dormant DSE-consumer weight for the
    /// `WardSiegeFearMap` consideration. At 0.0 the conditional-axis
    /// pattern (`slope=w, intercept=1-w`) collapses to identity, so
    /// every consumer DSE scores byte-identically pre-470. A follow-
    /// on tuning ticket lifts this above 0.0 to activate the fear
    /// signal in Flee / Wander / Explore / HerbcraftWard / `cover_at`.
    #[serde(default = "default_ward_siege_fear_weight")]
    pub ward_siege_fear_weight: f32,
    /// Multiplier on ward repel radius for shadow foxes (corrupted creatures).
    pub shadow_fox_ward_repel_multiplier: f32,
    /// Chance per attempt that a regrowth herb actually spawns.
    pub herb_regrowth_chance: f32,
    /// Growth rate multiplier for thornbriar in gardens (slower than food crops).
    pub thornbriar_farm_growth_modifier: f32,
    /// Personal corruption gained when harvesting a carcass.
    pub harvest_corruption_gain: f32,
    /// Corruption above this threshold suppresses herb harvestability.
    pub herb_suppression_threshold: f32,
    /// Corruption threshold above which health drain applies.
    pub corruption_health_drain_threshold: f32,
    /// Rest quality multiplier on corrupted tiles (lower = worse rest).
    pub corruption_rest_penalty: f32,

    // --- Counts / radii ---
    pub shadow_fox_population_cap: usize,
    /// Max concurrent Thornbriar herbs allowed (prevents unbounded growth).
    pub thornbriar_regrowth_cap: u32,
    /// Inner radius (manhattan) of the territory corruption ring query.
    /// Tiles closer than this to colony center are ignored (safe core).
    pub territory_corruption_inner_radius: f32,
    /// Outer radius (manhattan) of the territory corruption ring query.
    /// Tiles farther than this from colony center are ignored (too distant).
    pub territory_corruption_outer_radius: f32,
}

impl Default for MagicConstants {
    fn default() -> Self {
        Self {
            // RatePerDay rates (Phase 2)
            corruption_spread_rate: RatePerDay::new(0.1),
            healing_poultice_rate: RatePerDay::new(8.0),
            energy_tonic_rate: RatePerDay::new(3.0),
            thornward_decay_rate: RatePerDay::new(1.0),
            cleanse_corruption_rate: RatePerDay::new(1.0),
            cleanse_personal_corruption_rate: RatePerDay::new(0.5),
            corruption_health_drain: RatePerDay::new(0.5),

            // DurationDays durations (Phase 3) — preserve raw-tick numerics at
            // default 1000 ticks/day.
            mood_tonic_duration: DurationDays::new(0.5),
            personal_corruption_mood_duration: DurationDays::new(0.01),
            corruption_tile_mood_duration: DurationDays::new(0.005),
            gather_herb_duration: DurationDays::new(0.005),
            prepare_remedy_duration_workshop: DurationDays::new(0.01),
            prepare_remedy_duration_default: DurationDays::new(0.015),
            set_ward_duration: DurationDays::new(0.008),
            scry_duration: DurationDays::new(0.01),
            cleanse_max_duration: DurationDays::new(0.1),
            spirit_communion_duration: DurationDays::new(0.015),
            spirit_communion_mood_duration: DurationDays::new(0.1),
            misfire_fizzle_mood_duration: DurationDays::new(0.02),
            harvest_carcass_duration: DurationDays::new(0.015),

            // IntervalPerDay cadences (Phase 3) — preserve raw-tick numerics at
            // default 1000 ticks/day. Values 100/day (= every 10 ticks) for
            // corruption-spread + shadow-fox-spawn are flagged for follow-on
            // rebalancing per ticket 033 spec; migrating preserves behavior.
            corruption_spread_cadence: IntervalPerDay::new(100.0),
            shadow_fox_spawn_cadence: IntervalPerDay::new(100.0),
            herb_growth_cadence: IntervalPerDay::new(5.0),
            herb_regrowth_cadence: IntervalPerDay::new(2.0),

            // Scalar tuning
            corruption_spread_threshold: 0.3,
            corruption_new_tile_threshold: 0.05,
            ward_post_decay_multiplier: 0.3,
            mood_tonic_bonus: 0.2,
            personal_corruption_mood_threshold: 0.3,
            personal_corruption_mood_chance: 0.05,
            personal_corruption_mood_penalty: -0.15,
            personal_corruption_erratic_threshold: 0.7,
            personal_corruption_erratic_chance: 0.02,
            corruption_tile_mood_threshold: 0.1,
            corruption_twisted_herb_threshold: 0.3,
            shadow_fox_corruption_threshold: 0.85,
            shadow_fox_spawn_chance: 0.001,
            herbcraft_gather_skill_growth: 0.01,
            herbcraft_prepare_skill_growth: 0.01,
            gratitude_fondness_gain: 0.1,
            herbcraft_apply_skill_growth: 0.005,
            herbcraft_ward_skill_growth: 0.01,
            magic_ward_skill_growth: 0.01,
            scry_memory_strength: 0.6,
            scry_magic_skill_growth: 0.01,
            cleanse_magic_skill_growth: 0.005,
            cleanse_done_threshold: 0.05,
            spirit_communion_mood_bonus: 0.3,
            spirit_communion_skill_growth: 0.01,
            misfire_skill_safe_ratio: 0.8,
            misfire_chance_scale: 0.5,
            misfire_fizzle_threshold: 0.3,
            misfire_corruption_backsplash_threshold: 0.5,
            misfire_inverted_ward_threshold: 0.7,
            misfire_wound_transfer_threshold: 0.9,
            misfire_fizzle_mood_penalty: -0.1,
            misfire_corruption_backsplash_amount: 0.1,
            misfire_festering_chance: default_misfire_festering_chance(),
            festering_seed_damage: default_festering_seed_damage(),
            festering_observation_interval_ticks: default_festering_observation_interval_ticks(),
            siege_fear_ramp_ticks: default_siege_fear_ramp_ticks(),
            siege_fear_radius: default_siege_fear_radius(),
            ward_siege_fear_weight: default_ward_siege_fear_weight(),
            // Bumped from 2.0 to 3.0: the 15-min sim showed wards deflecting
            // shadow foxes but still allowing kills because cat activity zones
            // were outside the effective radius. 3.0 makes a ward cover a cat
            // cluster rather than just the ward itself.
            shadow_fox_ward_repel_multiplier: 3.0,
            herb_regrowth_chance: 0.3,
            thornbriar_farm_growth_modifier: 0.5,
            harvest_corruption_gain: 0.05,
            herb_suppression_threshold: 0.5,
            corruption_health_drain_threshold: 0.8,
            corruption_rest_penalty: 0.5,

            // Counts / radii
            // Restored to 2 (from 0) for the post-substrate-refactor
            // baseline-dataset capture. The v0.2.0 disable was provisional
            // — the food/building/survival loops have held green on seed 42
            // through Phase 4 (target-taking ports, marker authoring, §7.2
            // commitment gate, respect-iter-2). Re-engaging shadowfoxes is
            // a precondition for the deferred corruption-defense balance
            // work that the upcoming baseline dataset is meant to anchor.
            shadow_fox_population_cap: 2,
            thornbriar_regrowth_cap: 30,
            territory_corruption_inner_radius: 15.0,
            territory_corruption_outer_radius: 35.0,
        }
    }
}

// ---------- SocialConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SocialConstants {
    // --- RatePerDay rates (ticket 033 Phase 2) ---
    pub passive_familiarity_rate: RatePerDay,
    /// Per-day-equivalent of the romantic-attraction step applied at every
    /// `bond_check_interval` firing. The wrapper preserves the literal
    /// per-tick numeric so consumer math is unchanged: at the default 1000
    /// ticks/day scale, `RatePerDay::new(3.5).per_tick(&ts) = 0.0035` —
    /// the legacy per-check value. Combined with `bond_check_interval = 50`
    /// (20 checks/day), the actual per-day accumulation is 0.07/day; the
    /// `RatePerDay` value here does NOT represent that. Treat the wrapper
    /// as a typing-only retype: the *displayed* per-day number is the
    /// per-tick × 1000 mechanical scale, not the per-day-accumulated total.
    pub courtship_romantic_rate: RatePerDay,
    // --- Counts / radii ---
    pub passive_familiarity_range: f32,
    pub bond_check_interval: u64,
    // --- Bond thresholds ---
    pub mates_romantic_threshold: f32,
    pub mates_fondness_threshold: f32,
    pub mates_familiarity_threshold: f32,
    pub partners_romantic_threshold: f32,
    pub partners_fondness_threshold: f32,
    pub partners_familiarity_threshold: f32,
    pub friends_fondness_threshold: f32,
    pub friends_familiarity_threshold: f32,
    pub value_compat_same_threshold: f32,
    pub value_compat_divergent_high: f32,
    pub value_compat_divergent_low: f32,
    pub value_compat_same_delta: f32,
    pub value_compat_divergent_delta: f32,
    // --- Grooming modulation ---
    pub fondness_grooming_floor: f32,
    pub fondness_grooming_scale: f32,
    pub romantic_grooming_floor: f32,
    pub romantic_grooming_scale: f32,
    // --- Courtship: gated romantic drift for orientation-compatible pairs ---
    // Romantic only accumulates via the MateWith step otherwise, which creates
    // a chicken-and-egg: Partners bond requires romantic>0.5, but mating
    // requires Partners bond. Courtship drift breaks the cycle: compatible
    // close-friend pairs develop romantic attraction passively over time.
    pub courtship_fondness_gate: f32,
    pub courtship_familiarity_gate: f32,
    // --- §9.2 BefriendedAlly authoring (ticket 049) ---
    /// Cat ↔ wildlife familiarity at or above this threshold flips
    /// `BefriendedAlly` on both entities. Today no system writes
    /// cat ↔ wildlife familiarity in production, so the marker fires
    /// only via test fixtures or a future "non-hostile contact"
    /// signal source.
    pub befriend_familiarity_threshold: f32,
    /// Hysteresis band — the marker is removed when familiarity
    /// drops below `(threshold - hysteresis)`, preventing flicker
    /// at the boundary.
    pub befriend_familiarity_hysteresis: f32,
}

impl Default for SocialConstants {
    fn default() -> Self {
        Self {
            // RatePerDay rates (per_day = per_tick × 1000 — see field doc on
            // `courtship_romantic_rate` re: per-check vs per-day semantics).
            passive_familiarity_rate: RatePerDay::new(0.3),
            // Per bond_check_interval=50: 0.0035 per check × 20 checks/day =
            // 0.07/day actual accumulation. Reaches Partners threshold (0.5)
            // in ~7.1 in-game days; Mates (0.7) in ~10. Compatible
            // close-friend pairs become Partners within their first fertile
            // Spring, Mates by their second. Bumped 0.0025 → 0.0035 (ticket
            // 027 Bug 3 partial, 1.4× — inside the ±30% noise band): the
            // prior rate left late-spawning pairs short of the Partners
            // threshold by the end of the 900s soak window even when their
            // fondness / familiarity were already gate-passing.
            courtship_romantic_rate: RatePerDay::new(3.5),
            // Counts / radii
            passive_familiarity_range: 2.0,
            bond_check_interval: 50,
            // Bond thresholds
            mates_romantic_threshold: 0.7,
            mates_fondness_threshold: 0.7,
            mates_familiarity_threshold: 0.6,
            partners_romantic_threshold: 0.5,
            // Lowered 0.6 → 0.55 (ticket 027 Bug 3 partial). Mocha+Birch
            // in `logs/tuned-42-027bug3-trace` reached romantic=1.0 yet
            // their bond stayed Friends because fondness plateaued
            // below 0.6 — the courtship-drift loop only accumulates
            // `romantic`, not `fondness`. Pairing with the bond-bias on
            // socialize_target's target picker, this lower gate lets
            // sustained partner-directed socialization actually carry a
            // Friends bond to Partners. `mates_fondness_threshold = 0.7`
            // remains untouched as the deeper-affection ceiling.
            partners_fondness_threshold: 0.55,
            partners_familiarity_threshold: 0.5,
            friends_fondness_threshold: 0.3,
            friends_familiarity_threshold: 0.4,
            value_compat_same_threshold: 0.5,
            value_compat_divergent_high: 0.7,
            value_compat_divergent_low: 0.3,
            value_compat_same_delta: 0.0002,
            value_compat_divergent_delta: -0.0001,
            fondness_grooming_floor: 0.7,
            fondness_grooming_scale: 0.3,
            romantic_grooming_floor: 0.5,
            romantic_grooming_scale: 0.5,
            // The fondness gate sits at the Friends threshold (0.3) so drift
            // engages the moment a Friends bond forms — no dead zone between
            // tiers.
            courtship_fondness_gate: 0.3,
            courtship_familiarity_gate: 0.4,
            // §9.2 BefriendedAlly threshold mirrors the trade.md
            // recruitment fondness gate (0.6) — a cat that has built
            // enough familiarity with a wildlife creature to recruit
            // it is the same gate as befriending it.
            befriend_familiarity_threshold: 0.6,
            befriend_familiarity_hysteresis: 0.1,
        }
    }
}

// ---------- RelationshipsConstants ----------

/// Founder-pair init distribution for `Relationships::init_pair`. Authored
/// at world setup; not consulted after. Floors are chosen so that familiarity
/// straddles `SocialConstants::friends_familiarity_threshold` (0.4) — the
/// natural bond_check graduation gate — so some founder pairs land Friends on
/// the first 50-tick check while others remain unbonded, encoding founder
/// heterogeneity without setup-time bond magic.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RelationshipsConstants {
    pub founder_fondness_min: f32,
    pub founder_fondness_max: f32,
    pub founder_familiarity_min: f32,
    pub founder_familiarity_max: f32,
}

impl Default for RelationshipsConstants {
    fn default() -> Self {
        Self {
            // Lift fondness off the random `[-0.2, 0.3)` distribution — the
            // negative tail (~30% of pairs spawning mildly hostile) encodes
            // a "stranger gathering" world that the design rejects.
            founder_fondness_min: 0.1,
            founder_fondness_max: 0.4,
            // Ticket 490 (R1) — a TRUE straddle of the Friends
            // graduation gate (0.4). The previous band [0.4, 0.6) sat
            // entirely at/above the gate, so with fondness [0.1, 0.4)
            // every sufficiently-fond founder pair graduated to
            // BondType::Friends on the first bond check — and bonded
            // pairs accrue passive `bond_proximity_social_warmth`,
            // turning the whole founder set into one spatial attractor
            // (dispersion collapsed 24 → 4.7 tiles: the bond-driven
            // cuddle puddle). [0.3, 0.5) restores the struct-level
            // comment's stated intent: ~half of pairs land Friends,
            // half stay unbonded — founder heterogeneity. The novelty
            // axis (1 − familiarity) reads [0.5, 0.7], still safely
            // below the old over-socializing [0.7, 0.9) band that the
            // original lift off [0.1, 0.3) was guarding against.
            founder_familiarity_min: 0.3,
            founder_familiarity_max: 0.5,
        }
    }
}

// ---------- MoodConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MoodConstants {
    // --- DurationDays durations (ticket 033 Phase 3) ---
    /// Duration of a contagion mood modifier pushed onto a nearby cat.
    /// Pre-ticket-033 value was `5` raw ticks (= 0.005 days at default scale).
    #[serde(alias = "contagion_modifier_ticks")]
    pub contagion_modifier_duration: DurationDays,
    /// Duration of the contentment mood bonus applied to well-fed cats.
    /// Pre-ticket-033 value was `10` raw ticks.
    #[serde(alias = "contentment_mood_ticks")]
    pub contentment_mood_duration: DurationDays,
    /// Duration of the social-warmth mood bonus from being near a bonded
    /// companion. Pre-ticket-033 value was `5` raw ticks.
    #[serde(alias = "bond_proximity_mood_ticks")]
    pub bond_proximity_mood_duration: DurationDays,

    // --- Scalar tuning (non-temporal) ---
    pub baseline_optimism_weight: f32,
    pub baseline_offset: f32,
    pub anxiety_amplification: f32,
    pub temper_amplification_scale: f32,
    pub wounded_pride_respect_threshold: f32,
    pub wounded_pride_scale: f32,
    pub patience_extension_scale: f32,
    pub contagion_base_influence: f32,
    pub contagion_stubbornness_resistance: f32,
    pub contentment_phys_threshold: f32,
    pub contentment_mood_bonus: f32,
    pub bond_proximity_mood: f32,

    // --- Per-kind decay / amplification (ticket 114) ---
    /// Ticks subtracted per game-tick from Fear modifiers (default 2 = fades twice as fast).
    pub fear_decay_rate: u64,
    /// Anxiety amplification weight for Fear modifiers (default 1.5 — fear hits harder).
    pub fear_anxiety_amp_weight: f32,
    /// Anxiety amplification weight for Grief modifiers (default 0.3 — grief is inward).
    pub grief_anxiety_amp_weight: f32,

    // --- Counts / radii ---
    pub contagion_range: f32,
    pub bond_proximity_range: f32,
}

impl Default for MoodConstants {
    fn default() -> Self {
        Self {
            // DurationDays durations (Phase 3) — preserve raw-tick numerics at
            // default 1000 ticks/day.
            contagion_modifier_duration: DurationDays::new(0.005),
            contentment_mood_duration: DurationDays::new(0.01),
            bond_proximity_mood_duration: DurationDays::new(0.005),

            // Scalar tuning
            baseline_optimism_weight: 0.4,
            baseline_offset: -0.05,
            anxiety_amplification: 0.5,
            temper_amplification_scale: 0.3,
            wounded_pride_respect_threshold: 0.3,
            wounded_pride_scale: 0.15,
            patience_extension_scale: 0.3,
            contagion_base_influence: 0.002,
            contagion_stubbornness_resistance: 0.2,
            contentment_phys_threshold: 0.85,
            contentment_mood_bonus: 0.05,
            bond_proximity_mood: 0.03,

            // Per-kind decay / amplification
            fear_decay_rate: 2,
            fear_anxiety_amp_weight: 1.5,
            grief_anxiety_amp_weight: 0.3,

            // Counts / radii
            contagion_range: 3.0,
            bond_proximity_range: 3.0,
        }
    }
}

// ---------- DeathConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DeathConstants {
    pub elder_entry_seasons: u64,
    pub grace_seasons: u64,
    pub chance_per_excess_season: f64,
    pub grief_mood_penalty: f32,
    pub grief_mood_ticks: u64,
    pub grief_detection_range: f32,
    pub grief_memory_strength: f32,
    pub cleanup_grace_period: u64,

    // --- Bond-grief (ticket 116) ---
    /// Mood-penalty magnitude for losing a bonded partner. Applied as
    /// `-(intensity * fondness)` so a fondness-0.0 bond produces no grief.
    pub bereavement_mates_intensity: f32,
    pub bereavement_partners_intensity: f32,
    pub bereavement_friends_intensity: f32,
    /// How long bond grief lasts in raw ticks. Mates grief outlasts Partners outlasts Friends.
    /// Default at 1000 ticks/day: 3 days / 1.5 days / 0.5 day.
    pub bereavement_mates_ticks: u64,
    pub bereavement_partners_ticks: u64,
    pub bereavement_friends_ticks: u64,

    // --- Grave aura (035) ---
    /// Per-grave anti-corruption aura strength. Stamped into
    /// `GraveAuraMap` each tick alongside a linear falloff over
    /// `grave_anti_corruption_radius`. Foundation default; balance-
    /// tuning lives in follow-on ticket #5.
    #[serde(default = "default_grave_anti_corruption_strength")]
    pub grave_anti_corruption_strength: f32,
    /// Per-grave aura radius in tiles.
    #[serde(default = "default_grave_anti_corruption_radius")]
    pub grave_anti_corruption_radius: f32,
}

impl Default for DeathConstants {
    fn default() -> Self {
        Self {
            // Paired with `LifeStage::Elder` boundary in
            // `components/identity.rs::Age::stage` (Phase 4.3 retune:
            // Adult extends through season 59, Elder begins at 60).
            // Keeping these in lockstep is load-bearing — the old-age
            // mortality check at `src/systems/death.rs:50` only fires
            // for `stage == LifeStage::Elder`, so a mismatch between
            // this value and the stage boundary silently disables the
            // mortality ramp for a band of ages.
            elder_entry_seasons: 60,
            grace_seasons: 7,
            chance_per_excess_season: 0.0002,
            grief_mood_penalty: -0.3,
            grief_mood_ticks: 50,
            grief_detection_range: 5.0,
            grief_memory_strength: 1.0,
            cleanup_grace_period: 500,

            // Bond grief (ticket 116) — ships active at meaningful intensities.
            bereavement_mates_intensity: 0.7,
            bereavement_partners_intensity: 0.5,
            bereavement_friends_intensity: 0.3,
            bereavement_mates_ticks: 3000,
            bereavement_partners_ticks: 1500,
            bereavement_friends_ticks: 500,

            // 035: grave-aura constants. Foundation defaults; tuning
            // lives in follow-on ticket #5.
            grave_anti_corruption_strength: default_grave_anti_corruption_strength(),
            grave_anti_corruption_radius: default_grave_anti_corruption_radius(),
        }
    }
}

// ---------- FounderAgeConstants ----------

/// Stage quota and per-stage age bands used when rolling starting cats.
///
/// Ticket 148: replaced the prior 60/30/10 Young/Adult/Elder probability
/// distribution with a hard quota — at most `max_young_founders` Young
/// founders, with all remaining slots forced Adult. This guarantees a
/// large orientation-eligible Adult pool every seed, fixing the
/// `continuity_tallies.courtship` collapse caused by orientation-roll
/// wipeouts when the founder Adult count was small. Elders are no longer
/// spawned as founders (the colony grows its first Elders organically as
/// Adult founders age past `LifeStage::Elder`'s entry season).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FounderAgeConstants {
    pub young_min_seasons: u64,
    pub young_max_seasons: u64,
    pub adult_min_seasons: u64,
    pub adult_max_seasons: u64,
    /// Hard ceiling on Young founders per spawn (ticket 148). Slots beyond
    /// this are forced Adult. With the default of 2, an 8-founder colony
    /// has 6 Adults — enough that orientation-incompat-wipeout failures
    /// (whole-seed courtship collapse) become rare.
    pub max_young_founders: usize,
}

impl Default for FounderAgeConstants {
    fn default() -> Self {
        Self {
            young_min_seasons: 4,
            young_max_seasons: 11,
            adult_min_seasons: 12,
            adult_max_seasons: 30,
            max_young_founders: 2,
        }
    }
}

// ---------- PreyConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreyConstants {
    pub detection_base_chance: f32,
    pub alertness_base: f32,
    pub alertness_range: f32,
    pub alertness_recovery: f32,
    pub vigilance_center: f32,
    pub vigilance_steepness: f32,
    pub vigilance_baseline: f32,
    pub vigilance_amplitude: f32,
    pub bird_teleport_min_range: f32,
    pub bird_teleport_max_range: f32,
    pub grazing_wander_chance: f32,
    pub grazing_jitter_chance: f32,
    /// Maximum duration of a grazing bout before the prey returns to Idle.
    /// (ticket 033 Phase 4)
    #[serde(alias = "grazing_max_ticks")]
    pub grazing_max_duration: DurationDays,
    pub grazing_max_roam_normal: f32,
    pub grazing_max_roam_pressured: f32,
    pub grazing_pressure_roam_threshold: f32,
    pub flee_stop_distance: f32,
    pub hunger_base_rate: RatePerDay,
    pub overcrowding_threshold: f32,
    pub overcrowding_hunger_extra: f32,
    pub store_raid_chance: f32,
    pub store_raid_range: f32,
    pub store_raid_hunger_relief: f32,
    pub store_raid_cleanliness_drain: RatePerDay,
    pub store_raid_narrative_chance: f32,
    pub passive_hunger_relief: f32,
    /// Per-day health drain for a *prey animal* (rabbit, etc.) at full
    /// hunger — the prey-side analogue of `NeedsConstants::starvation_health_drain`.
    /// Drives prey mortality in lean seasons; combined with
    /// `den_refill_base_chance` this is the prey-population governor.
    /// Per-day = per-tick × 1000 at the default scale.
    pub starvation_health_drain: RatePerDay,
    pub starvation_threshold: f32,
    pub starvation_narrative_chance: f32,
    pub background_breed_rate_multiplier: f32,
    pub den_refill_base_chance: f32,
    pub den_fear_breeding_suppression: f32,
    // stays raw f32 — exponential decay, not additive (would need a different wrapper type)
    pub den_predation_pressure_decay: f32,
    pub den_stress_high_threshold: f32,
    pub den_stress_low_threshold: f32,
    /// Sustained stress duration after which a prey den is abandoned and
    /// despawns (ticket 033 Phase 4).
    #[serde(alias = "den_abandon_stress_ticks")]
    pub den_abandon_stress_duration: DurationDays,
    pub den_kill_pressure_increment: f32,
    pub den_kill_pressure_range: f32,
    pub den_raid_pressure_increment: f32,
    pub den_orphan_adopt_range: f32,
    pub den_orphan_adopt_capacity_threshold: f32,
    pub den_orphan_found_chance: f32,
    pub den_orphan_min_spacing: f32,
    /// Prey reject movement tiles with corruption above this threshold.
    pub prey_corruption_avoidance: f32,
    /// Den breeding suppressed when tile corruption exceeds this.
    pub den_corruption_threshold: f32,
    pub initial_den_count_mouse: usize,
    pub initial_den_count_rat: usize,
    pub initial_den_count_rabbit: usize,
    pub initial_den_count_fish: usize,
    pub initial_den_count_bird: usize,

    // --- Scent (Phase 2B) ---
    /// Per-tick scent magnitude each live prey deposits at its tile.
    /// Mirrors `FoxEcologyConstants::scent_deposit` for symmetry
    /// between predator- and prey-scent grids. Phase 2B baseline;
    /// tune per §5.6.5 decay-rationale.
    #[serde(default = "default_prey_scent_deposit_per_tick")]
    pub scent_deposit_per_tick: f32,
    /// Global decay on `PreyScentMap`, expressed per in-game day.
    ///
    /// Prey scent is an *activity trail* — not a territorial mark
    /// (those are `FoxEcologyConstants::scent_decay_rate` and the cat
    /// presence map). At deposit `0.1` and detect threshold `0.05`,
    /// a peak (1.0) bucket needs to persist a usable fraction of an
    /// in-game day to make the `goap.rs:4159` scent-led hunt path do
    /// real work. `RatePerDay::new(1.0)` lands a peak deposit at the
    /// detection threshold (~0.05) after roughly one in-game day —
    /// "yesterday's trail" semantics, no supernatural multi-day
    /// memory.
    ///
    /// Pre-ticket-033 value was `0.02/tick = 20/day`, which faded a
    /// fresh deposit below threshold in ~3 ticks (functionally
    /// inert). See ticket 033 / `docs/balance/time-anchor-iteration-1.md`.
    #[serde(
        rename = "scent_decay_rate",
        alias = "scent_decay_per_tick",
        default = "default_prey_scent_decay_rate"
    )]
    pub scent_decay_rate: RatePerDay,

    /// Denominator for per-species scent emission scaling on
    /// `PreyScentMaps::deposit_for_kind`. Each tick a live prey deposits
    /// `scent_deposit_per_tick × (profile.scent.base_range / normalizer)`,
    /// clamped to `[0.0, 1.0]`.
    ///
    /// Set to the maximum prey scent `base_range` (Rat = 6.0) so Rat
    /// deposits at 1.0× and Bird at ~0.33×, matching the ecological
    /// profile already encoded in `SensoryConstants` defaults. Changing
    /// this value rescales all five emission strengths proportionally
    /// without touching per-species sensory constants — useful for a
    /// uniform "less prey scent everywhere" tuning sweep without
    /// invalidating per-species ecology.
    #[serde(default = "default_prey_scent_deposit_normalizer")]
    pub scent_deposit_normalizer: f32,
}

fn default_prey_scent_deposit_per_tick() -> f32 {
    0.1
}

fn default_prey_scent_decay_rate() -> RatePerDay {
    RatePerDay::new(1.0)
}

fn default_prey_scent_deposit_normalizer() -> f32 {
    6.0
}

impl Default for PreyConstants {
    fn default() -> Self {
        Self {
            detection_base_chance: 0.10,
            alertness_base: 0.5,
            alertness_range: 0.5,
            alertness_recovery: 0.005,
            vigilance_center: 0.45,
            vigilance_steepness: 3.5,
            vigilance_baseline: 0.4,
            vigilance_amplitude: 1.2,
            bird_teleport_min_range: 5.0,
            bird_teleport_max_range: 8.0,
            grazing_wander_chance: 0.05,
            grazing_jitter_chance: 0.1,
            // 200 ticks ÷ 1000 ticks/day = 0.2 days (Phase 4).
            grazing_max_duration: DurationDays::new(0.2),
            grazing_max_roam_normal: 15.0,
            grazing_max_roam_pressured: 8.0,
            grazing_pressure_roam_threshold: 0.5,
            flee_stop_distance: 10.0,
            hunger_base_rate: RatePerDay::new(0.2),
            overcrowding_threshold: 0.8,
            overcrowding_hunger_extra: 0.0001,
            store_raid_chance: 0.05,
            store_raid_range: 2.0,
            store_raid_hunger_relief: 0.015,
            store_raid_cleanliness_drain: RatePerDay::new(1.0),
            store_raid_narrative_chance: 0.02,
            passive_hunger_relief: 0.0003,
            starvation_health_drain: RatePerDay::new(1.0),
            starvation_threshold: 0.9,
            starvation_narrative_chance: 0.1,
            background_breed_rate_multiplier: 0.5,
            den_refill_base_chance: 0.005,
            den_fear_breeding_suppression: 0.8,
            den_predation_pressure_decay: 0.9995,
            den_stress_high_threshold: 0.7,
            den_stress_low_threshold: 0.5,
            // 3000 ticks ÷ 1000 ticks/day = 3 days (Phase 4).
            den_abandon_stress_duration: DurationDays::new(3.0),
            den_kill_pressure_increment: 0.1,
            den_kill_pressure_range: 15.0,
            den_raid_pressure_increment: 0.3,
            den_orphan_adopt_range: 15.0,
            den_orphan_adopt_capacity_threshold: 0.5,
            den_orphan_found_chance: 0.001,
            den_orphan_min_spacing: 25.0,
            prey_corruption_avoidance: 1.0,
            den_corruption_threshold: 0.4,
            initial_den_count_mouse: 4,
            initial_den_count_rat: 3,
            initial_den_count_rabbit: 3,
            initial_den_count_fish: 2,
            initial_den_count_bird: 2,
            scent_deposit_per_tick: default_prey_scent_deposit_per_tick(),
            scent_decay_rate: default_prey_scent_decay_rate(),
            scent_deposit_normalizer: default_prey_scent_deposit_normalizer(),
        }
    }
}

// ---------- SpeciesConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpeciesProfile {
    pub breed_rate: f32,
    pub population_cap: usize,
    pub seasonal_breed_spring: f32,
    pub seasonal_breed_summer: f32,
    pub seasonal_breed_autumn: f32,
    pub seasonal_breed_winter: f32,
    pub flee_speed: u32,
    pub graze_cadence: u64,
    pub alert_radius: f32,
    pub freeze_ticks: u64,
    pub catch_difficulty: f32,
    pub flee_duration: u64,
    pub den_capacity: u32,
    pub den_spawn_rate: f32,
    pub den_raid_drop: u32,
    pub den_spacing: f32,
    pub den_density: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpeciesConstants {
    pub mouse: SpeciesProfile,
    pub rat: SpeciesProfile,
    pub rabbit: SpeciesProfile,
    pub fish: SpeciesProfile,
    pub bird: SpeciesProfile,
}

impl Default for SpeciesConstants {
    fn default() -> Self {
        Self {
            mouse: SpeciesProfile {
                breed_rate: 0.0003,
                population_cap: 80,
                seasonal_breed_spring: 1.5,
                seasonal_breed_summer: 1.0,
                seasonal_breed_autumn: 0.5,
                seasonal_breed_winter: 0.1,
                flee_speed: 1,
                graze_cadence: 40,
                alert_radius: 3.0,
                freeze_ticks: 1,
                catch_difficulty: 0.9,
                flee_duration: 50,
                den_capacity: 80,
                den_spawn_rate: 0.01,
                den_raid_drop: 6,
                den_spacing: 10.0,
                den_density: 100,
            },
            rat: SpeciesProfile {
                breed_rate: 0.0005,
                population_cap: 55,
                seasonal_breed_spring: 1.5,
                seasonal_breed_summer: 1.0,
                seasonal_breed_autumn: 0.5,
                seasonal_breed_winter: 0.2,
                flee_speed: 1,
                graze_cadence: 25,
                alert_radius: 4.0,
                freeze_ticks: 2,
                catch_difficulty: 1.0,
                flee_duration: 75,
                den_capacity: 60,
                den_spawn_rate: 0.012,
                den_raid_drop: 5,
                den_spacing: 10.0,
                den_density: 100,
            },
            rabbit: SpeciesProfile {
                breed_rate: 0.0004,
                population_cap: 45,
                seasonal_breed_spring: 2.0,
                seasonal_breed_summer: 1.0,
                seasonal_breed_autumn: 0.0,
                seasonal_breed_winter: 0.0,
                flee_speed: 1,
                graze_cadence: 20,
                alert_radius: 6.0,
                freeze_ticks: 10,
                catch_difficulty: 0.85,
                flee_duration: 60,
                den_capacity: 60,
                den_spawn_rate: 0.01,
                den_raid_drop: 4,
                den_spacing: 20.0,
                den_density: 250,
            },
            fish: SpeciesProfile {
                breed_rate: 0.0002,
                population_cap: 35,
                seasonal_breed_spring: 2.0,
                seasonal_breed_summer: 0.5,
                seasonal_breed_autumn: 0.3,
                seasonal_breed_winter: 0.1,
                flee_speed: 0,
                graze_cadence: 50,
                alert_radius: 2.0,
                freeze_ticks: 0,
                catch_difficulty: 0.6,
                flee_duration: 0,
                den_capacity: 50,
                den_spawn_rate: 0.006,
                den_raid_drop: 3,
                den_spacing: 20.0,
                den_density: 250,
            },
            bird: SpeciesProfile {
                breed_rate: 0.0001,
                population_cap: 30,
                seasonal_breed_spring: 1.5,
                seasonal_breed_summer: 1.0,
                seasonal_breed_autumn: 0.0,
                seasonal_breed_winter: 0.0,
                flee_speed: 3,
                graze_cadence: 35,
                alert_radius: 8.0,
                freeze_ticks: 1,
                catch_difficulty: 0.5,
                flee_duration: 30,
                den_capacity: 40,
                den_spawn_rate: 0.004,
                den_raid_drop: 3,
                den_spacing: 15.0,
                den_density: 250,
            },
        }
    }
}

// ---------- ScoringConstants ----------

/// 301: selection rule used by the coordinator's ward-placement
/// scorer in `compute_ward_placement()`. The first enum-typed field
/// in `ScoringConstants` — serde-serializes the variant as a JSON
/// string into the events.jsonl header. Variant names are stable
/// identifiers; new options must be added as new variants rather
/// than renamed in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum WardPlacementSemantics {
    /// Pre-301 behavior. The scorer picks the single highest-scored
    /// candidate tile per coordinator wake. Default.
    #[default]
    SingleShotArgmax,
    /// 301 SPLIT option. Run K=`ward_placement_residual_rounds`
    /// rounds of submodular greedy inside the scorer: round 0 picks
    /// the argmax winner, stamps virtual coverage around it, then
    /// round 1 re-scores all candidates against the partially-eaten
    /// threat surface, etc. Returns the round-(K-1) pick.
    DescendingResidual,
}

/// 382: selection rule used by `compute_building_placement()` for the
/// coordinator's autonomous Build directive site-spawning. The
/// pre-382 `Spiral` arm preserves the radius-16 spiral search from
/// `find_building_placement` as an emergency revert / regression-bisect
/// fixture; the `InfluenceMap` default replaces it with an argmax over
/// `ColonyDistrictMap` + per-kind weight tables. Variant names are
/// stable identifiers; add new variants rather than renaming in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum BuildingPlacementSemantics {
    /// Pre-382 behavior. Spiral search outward from `colony_center` to
    /// Manhattan radius 16; returns the first `footprint_valid` tile.
    /// Saturates after ~3-5 buildings near center and silently returns
    /// `None`. Retained as a regression-bisect fixture only.
    Spiral,
    /// 382 default. Argmax over `ColonyDistrictMap` (the composite
    /// `frontier − crowding − threat`) plus per-kind affinity lifts and
    /// `same_kind_proximity` clustering / dispersion. Whole-map
    /// candidate generation (`building_placement_candidate_step`),
    /// `footprint_valid` gate, jitter tiebreak.
    #[default]
    InfluenceMap,
}

/// 313: composition rule for the `CatScentMap` lift to ward-placement
/// scoring. `Additive` (default) preserves the pre-313 formula
/// `+ w_cat_value * cat_value` — proximity to cat-density peaks
/// adds a reward. `Gate` (option (c) from ticket 313) replaces the
/// additive reward with a multiplicative saturating-ramp gate
/// `gate(cat_value) = (cat_value / gate_floor).clamp(0, 1)` on the
/// threat-merit term, so a dead tile (cat_value ~ 0) is suppressed
/// near zero while warm tiles (cat_value ≥ gate_floor) get full
/// merit with no extra reward for density peaks.
///
/// Serde-serializes the variant as a JSON string into the
/// events.jsonl header alongside `WardPlacementSemantics`. Variant
/// names are stable identifiers; add new variants rather than
/// renaming in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum WardPlacementCatValueComposition {
    /// Pre-313 behavior. `+ ward_placement_cat_value_weight *
    /// cat_value` is added to the score. Rewards proximity to
    /// cat-density peaks. Default.
    #[default]
    Additive,
    /// 313 SPLIT option (c). The additive `+ w_cat_value * cat_value`
    /// term is dropped. The threat-merit term is multiplied by a
    /// saturating-ramp gate `(cat_value / gate_floor).clamp(0, 1)`,
    /// suppressing dead tiles without rewarding density peaks.
    /// Tunable via `ward_placement_cat_value_gate_floor`.
    Gate,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScoringConstants {
    pub jitter_range: f32,
    pub eat_urgency_scale: f32,
    pub sleep_urgency_scale: f32,
    /// Day-phase additive offsets to Sleep urgency. Encodes the cat's
    /// crepuscular-with-night-heavy-rest design (see
    /// `docs/systems/sleep-that-makes-sense.md`). Dawn/Dusk are feeding peaks
    /// (0.0), Day is a tie-break so exhausted cats can still nap (0.1), Night
    /// dominates fulfillment-tier scores so Sleep wins by default (1.2).
    #[serde(default = "default_sleep_dawn_bonus")]
    pub sleep_dawn_bonus: f32,
    #[serde(default = "default_sleep_day_bonus")]
    pub sleep_day_bonus: f32,
    #[serde(default = "default_sleep_dusk_bonus")]
    pub sleep_dusk_bonus: f32,
    #[serde(default = "default_sleep_night_bonus")]
    pub sleep_night_bonus: f32,
    /// Ticket 251 — Sleep DSE `health_deficit` axis Logistic midpoint.
    /// HP-deficit at the sigmoid inflection — below this, the axis is
    /// near-zero (healthy-cat composition near-unchanged); above this,
    /// the axis ramps sharply toward 1.0 (acute-injury crisis lurch).
    /// 0.4 mirrors the retired `acute_health_adrenaline_threshold` so
    /// the substrate-side urgency fires in the same regime the post-
    /// scoring modifier did. Replaces the pre-251 `injury_rest_bonus`
    /// Linear-slope semantic — under the Logistic curve, the
    /// `health_deficit` axis encodes acute-injury urgency directly,
    /// retiring `AcuteHealthAdrenalineFlee`'s post-scoring lift.
    #[serde(default = "default_sleep_health_deficit_midpoint")]
    pub sleep_health_deficit_midpoint: f32,
    /// Ticket 251 — Sleep DSE `health_deficit` axis Logistic steepness.
    /// 10.0 matches `sleep_dep`'s involuntary-micro-sleep curve — the
    /// catalog's steepest sigmoid aside from flee-or-fight. At this
    /// steepness, the curve is ≈0 at midpoint−0.1 and ≈1 at
    /// midpoint+0.1, preserving the retired modifier's smoothstep
    /// transition-width (~0.1 in normalized HP-deficit units).
    #[serde(default = "default_sleep_health_deficit_steepness")]
    pub sleep_health_deficit_steepness: f32,
    // Fox disposition day-phase bonuses (crepuscular/nocturnal vulpine rhythm).
    // Applied in src/ai/fox_scoring.rs::score_fox_dispositions. Hunting peaks
    // Dusk→Night, Resting peaks Day, Patrolling mild-positive Dusk→Dawn.
    #[serde(default = "default_fox_hunt_dawn_bonus")]
    pub fox_hunt_dawn_bonus: f32,
    #[serde(default = "default_fox_hunt_day_bonus")]
    pub fox_hunt_day_bonus: f32,
    #[serde(default = "default_fox_hunt_dusk_bonus")]
    pub fox_hunt_dusk_bonus: f32,
    #[serde(default = "default_fox_hunt_night_bonus")]
    pub fox_hunt_night_bonus: f32,
    #[serde(default = "default_fox_patrol_dawn_bonus")]
    pub fox_patrol_dawn_bonus: f32,
    #[serde(default = "default_fox_patrol_day_bonus")]
    pub fox_patrol_day_bonus: f32,
    #[serde(default = "default_fox_patrol_dusk_bonus")]
    pub fox_patrol_dusk_bonus: f32,
    #[serde(default = "default_fox_patrol_night_bonus")]
    pub fox_patrol_night_bonus: f32,
    #[serde(default = "default_fox_rest_dawn_bonus")]
    pub fox_rest_dawn_bonus: f32,
    #[serde(default = "default_fox_rest_day_bonus")]
    pub fox_rest_day_bonus: f32,
    #[serde(default = "default_fox_rest_dusk_bonus")]
    pub fox_rest_dusk_bonus: f32,
    #[serde(default = "default_fox_rest_night_bonus")]
    pub fox_rest_night_bonus: f32,
    /// Base score for the Cook action when a Kitchen and raw food are both
    /// available. Fulfillment-tier action — fires when physiological needs
    /// are mostly met.
    #[serde(default = "default_cook_base_score")]
    pub cook_base_score: f32,
    /// Diligence-trait scalar added to Cook score.
    #[serde(default = "default_cook_diligence_scale")]
    pub cook_diligence_scale: f32,
    /// Minimum hunger (0.0=starving, 1.0=full) above which a cat is willing
    /// to cook. Prevents starving cats from wandering off to the Kitchen.
    #[serde(default = "default_cook_hunger_gate")]
    pub cook_hunger_gate: f32,
    /// Cook urgency scales with food-store scarcity — matches Hunt/Forage.
    /// Low `food_fraction` raises Cook score so the buffer is restocked
    /// before the stores empty.
    #[serde(default = "default_cook_food_scarcity_scale")]
    pub cook_food_scarcity_scale: f32,
    pub hunt_food_scarcity_scale: f32,
    pub hunt_prey_bonus: f32,
    pub hunt_boldness_scale: f32,
    pub forage_food_scarcity_scale: f32,
    pub forage_diligence_scale: f32,
    pub socialize_sociability_scale: f32,
    pub socialize_temper_penalty_scale: f32,
    pub socialize_playfulness_bonus: f32,
    // 158: `self_groom_temperature_scale` retired with the
    // `self_groom_won` resolver. The field weighted a side-channel
    // computation (raw thermal-deficit × scale × tier_suppression(1))
    // used only to derive the routing boolean — never the actual L2
    // score the L3 softmax saw. Splitting `Action::Groom` made the L3
    // pick directly determinative, so the side-channel went away.
    pub groom_temper_penalty_scale: f32,
    pub explore_curiosity_scale: f32,
    /// Fox scent threshold above which Hunt/Explore scores are suppressed.
    pub fox_scent_suppression_threshold: f32,
    /// Scale for how much fox scent suppresses risky action scores.
    pub fox_scent_suppression_scale: f32,
    /// `food_fraction` floor below which `StockpileSatiation` does not
    /// suppress food-acquisition DSEs. Above this, the modifier
    /// multiplicatively damps Hunt and Forage so the IAUS contest tilts
    /// toward Eat at the existing stockpile. The desperation-hunting
    /// case (`food_fraction = 0`) is preserved by construction. See
    /// `src/ai/modifier.rs::StockpileSatiation` and ticket 094.
    #[serde(default = "default_stockpile_satiation_threshold")]
    pub stockpile_satiation_threshold: f32,
    /// Maximum suppression `StockpileSatiation` applies to Hunt/Forage
    /// when `food_fraction = 1.0`. The score multiplier is
    /// `(1 - suppression).max(0.0)` where
    /// `suppression = ((food_fraction - threshold) / (1 - threshold)) *
    /// scale`. With scale = 0.85 and threshold = 0.5, full stores
    /// reduce Hunt/Forage scores to ~15% of their pre-modifier value
    /// — the IAUS contest then yields to Eat at the stockpile.
    #[serde(default = "default_stockpile_satiation_scale")]
    pub stockpile_satiation_scale: f32,
    /// Ticket 490 (R3) — `WorkPressureAffiliativeYield` threshold.
    /// Physical-need pressure (`1 − phys_satisfaction`) floor below
    /// which the modifier is a no-op. Above it, the affiliative DSE
    /// class (Socialize, GroomOther) is multiplicatively damped so
    /// bond-warmth-seeking *yields to productive pull* — a hungry/tired
    /// founder leaves the clump to eat/forage/hunt instead of being
    /// walked back by the social-deficit axes. Acts on the warmth-
    /// seeking DRIVER score (visible in the L2 modifier trace), not on
    /// eligibility — the 487 gate-only fix shifted bandwidth to Patrol;
    /// this prices it into the need that's actually pulling.
    #[serde(default = "default_work_pressure_affiliative_yield_threshold")]
    pub work_pressure_affiliative_yield_threshold: f32,
    /// Ticket 490 (R3) — maximum multiplicative damp
    /// `WorkPressureAffiliativeYield` applies to Socialize/GroomOther at
    /// physical-need pressure 1.0. The multiplier is
    /// `(1 − suppression).max(0)` where `suppression =
    /// ((pressure − threshold) / (1 − threshold)) × scale`. Asymmetric:
    /// a fed, rested cat's affiliative scoring is untouched — warm
    /// friends still converge off-hours.
    #[serde(default = "default_work_pressure_affiliative_yield_scale")]
    pub work_pressure_affiliative_yield_scale: f32,
    /// Ticket 146 — saturating-composition cap on the cumulative positive
    /// lift any one DSE can receive across the §3.5.1 modifier pipeline.
    /// `0.0` disables the cap (raw additive sum). With `0.30` and two
    /// independent lifts of `+0.20` each (e.g. 107 ExhaustionPressure
    /// Sleep + 110 ThermalDistress Sleep on a cold tired night), the
    /// effective combined lift is
    /// `0.30 * (1 - (1 - 0.20/0.30)^2) ≈ 0.267` — diminishing returns
    /// instead of unbounded sum. Single-axis behavior is unchanged
    /// whenever a single modifier's lift is `< cap`. Tuned to bound the
    /// seed-42 Sleep double-stack that caused colony extinction in
    /// ticket 146's verification soak.
    #[serde(default = "default_max_additive_lift_per_dse")]
    pub max_additive_lift_per_dse: f32,
    /// Ticket 088. `body_distress_composite` floor below which
    /// `BodyDistressPromotion` does not lift self-care DSEs. Above this,
    /// the modifier additively lifts every self-care DSE (Flee, Sleep,
    /// Eat, Hunt, Forage, GroomSelf) so the IAUS contest tilts toward the
    /// body-recovery class as a unit. Set deliberately higher than 087's
    /// `body_distress_threshold` (the marker-insertion gate, default 0.6)
    /// so the marker fires first as a perception event and the modifier
    /// engages later as a stronger lift. Substrate prerequisite for
    /// retiring 047's CriticalHealth interrupt; the 087 perception
    /// substrate publishes the input scalar via
    /// `interoception::body_distress_composite`.
    #[serde(default = "default_body_distress_promotion_threshold")]
    pub body_distress_promotion_threshold: f32,
    /// Ticket 088. Maximum additive lift `BodyDistressPromotion` applies
    /// to each self-care DSE when `body_distress_composite = 1.0`. The
    /// per-tick lift is
    /// `((distress - threshold) / (1 - threshold)) * lift`. With lift =
    /// 0.20 and threshold = 0.7, a fully-distressed cat sees +0.20 added
    /// to each self-care DSE — enough to flip a 0.55 non-self-care
    /// competitor below a 0.50 self-care option. Tune empirically before
    /// 047 retires its CriticalHealth interrupt branch.
    #[serde(default = "default_body_distress_promotion_lift")]
    pub body_distress_promotion_lift: f32,
    /// Tickets 102 / 105 — `AcuteHealthAdrenaline{Fight, Freeze}` Modifier
    /// threshold. The `health_deficit` floor below which the lurch is a
    /// no-op. Set to align with `disposition.critical_health_threshold`
    /// (0.4) so the substrate engages whenever the legacy `CriticalHealth`
    /// interrupt would have. Distinct from `body_distress_promotion_threshold`
    /// (0.7, applied to the `max()`-composite scalar): this threshold reads
    /// `health_deficit` directly so the Fight / Freeze valences fire on
    /// injury alone. The Flee valence (`AcuteHealthAdrenalineFlee`, ticket
    /// 047) was retired by ticket 251 — its load moved into the Sleep DSE's
    /// `health_deficit` axis (Logistic curve at this same midpoint), so
    /// retiring the constant would break the still-active Fight + Freeze
    /// valences but the threshold itself is preserved.
    #[serde(default = "default_acute_health_adrenaline_threshold")]
    pub acute_health_adrenaline_threshold: f32,
    /// Ticket 102 — `AcuteHealthAdrenalineFight` lift on the Fight DSE
    /// when `health_deficit` is high AND `escape_viability` is below
    /// `acute_health_adrenaline_fight_viability_threshold` (cornered cat,
    /// maternal defense, terrain-locked but combat is winnable). The
    /// same `health_deficit` ramp drives the lift as the Flee branch;
    /// the additional viability gate is what splits this branch off.
    /// Default 0.0 (inert at ship) — proposed magnitude 0.50, enabled
    /// via `CLOWDER_OVERRIDES` for the four-artifact hypothesize sweep.
    /// The same magnitude is *subtracted* from Flee on the same tick so
    /// the cornered cat doesn't see Flee promoted by 047's Flee branch
    /// while Fight is also lifting — the suppression keeps the two
    /// branches mutually exclusive in a single contest.
    #[serde(default = "default_acute_health_adrenaline_fight_lift")]
    pub acute_health_adrenaline_fight_lift: f32,
    /// Ticket 102 — `escape_viability` threshold below which the Fight
    /// branch fires. Above this value the cat's `Flee` valence (047)
    /// drives the response; below it, Fight takes over (with Flee
    /// suppressed by the same magnitude). Default 0.4 — picked so the
    /// gate fires in walled corners / dependent-burdened scenarios but
    /// stays quiet in open terrain.
    #[serde(default = "default_acute_health_adrenaline_fight_viability_threshold")]
    pub acute_health_adrenaline_fight_viability_threshold: f32,
    /// Ticket 105 — `AcuteHealthAdrenalineFreeze` lift on the Hide
    /// DSE (ticket 104). Fires under combined high `health_deficit`
    /// AND low `escape_viability` — the cornered + overmatched
    /// scenario where neither Flee nor Fight is viable. **Default
    /// 0.0** (ships inert); proposed magnitude 0.70 (largest of the
    /// three valences — freeze is the last-resort response per the
    /// 047 N-valence framework). Promotion gated on the
    /// `HideEligible` authoring system landing alongside, so the
    /// double-inert contract holds: this commit is bit-identical to
    /// baseline.
    #[serde(default = "default_acute_health_adrenaline_freeze_lift")]
    pub acute_health_adrenaline_freeze_lift: f32,
    /// Ticket 105 — `escape_viability` gate threshold for the Freeze
    /// branch. Below this value, the Freeze branch fires (in concert
    /// with 102's Fight gate when applicable; the choice between
    /// Fight and Freeze is owned by the magnitudes
    /// `acute_health_adrenaline_fight_lift` vs
    /// `acute_health_adrenaline_freeze_lift`). Default 0.4 mirrors
    /// 102's gate so the two cornered-scenario valences share onset
    /// semantics.
    #[serde(default = "default_acute_health_adrenaline_freeze_viability_threshold")]
    pub acute_health_adrenaline_freeze_viability_threshold: f32,
    /// Ticket 106 — `HungerUrgency` Modifier threshold. The
    /// `hunger_urgency` floor below which the lift is a no-op. Default
    /// 0.6 (cat at hunger 0.4 or below). Mirrors `1 -
    /// starvation_interrupt_threshold` (1 - 0.15 = 0.85) lifted earlier
    /// so the substrate engages well *before* the legacy interrupt
    /// would have, giving the IAUS contest time to re-rank Eat / Hunt
    /// / Forage above non-food dispositions.
    #[serde(default = "default_hunger_urgency_threshold")]
    pub hunger_urgency_threshold: f32,
    /// Ticket 032 (cross-cutting with 106) — exponent on the
    /// `HungerUrgency` ramp shape. **Default `1.0` = linear** (current
    /// shipped behavior — gentle uniform rise from `hunger_urgency =
    /// threshold` to `urgency = 1.0`). Values `< 1.0` give a *leading*
    /// nerve-impulse shape: the lift saturates fast in the early band
    /// and plateaus near max well before hunger reaches starvation
    /// territory — the cat is "already as motivated as she can be" by
    /// the time the cliff fires. Treatment override `0.4` paired with
    /// 032's damage curve is the matched nerve-impulse / consequence
    /// pair: input leads, damage trails. Per `docs/balance/starvation-rebalance.md`
    /// Iter 5 (TBD): the linear default lets the cat enter the damage
    /// band at ~62% of full motivation lift; sub-linear shape lifts that
    /// to ~92% so cats fight harder for food before they bleed.
    #[serde(default = "default_hunger_urgency_curve_exponent")]
    pub hunger_urgency_curve_exponent: f32,
    /// Ticket 106 — `HungerUrgency` lift on Eat. Largest of the three —
    /// Eat is the direct solution; Hunt / Forage are upstream.
    /// **Default 0.0** (ships inert); proposed magnitude 0.40 enabled
    /// via `CLOWDER_OVERRIDES` for the Phase 3 hypothesize sweep.
    #[serde(default = "default_hunger_urgency_eat_lift")]
    pub hunger_urgency_eat_lift: f32,
    /// Ticket 106 — `HungerUrgency` lift on Hunt. Smaller than Eat —
    /// Hunt is upstream of Eat in the food chain. Default 0.0;
    /// proposed magnitude 0.20.
    #[serde(default = "default_hunger_urgency_hunt_lift")]
    pub hunger_urgency_hunt_lift: f32,
    /// Ticket 106 — `HungerUrgency` lift on Forage. Symmetric to Hunt
    /// — both are food-acquisition fallbacks. Default 0.0; proposed
    /// 0.20.
    #[serde(default = "default_hunger_urgency_forage_lift")]
    pub hunger_urgency_forage_lift: f32,
    /// Ticket 156 — `KittenEatBoost` Modifier threshold. The
    /// `hunger_urgency` floor above which the kitten-only Eat lift
    /// engages. Calibrated below the colony-wide `HungerUrgency`
    /// threshold (0.6) so kittens lift earlier than adults — kittens
    /// have smaller stomachs and shorter starvation runways. Default
    /// 0.4 (kitten at hunger 0.6 or below).
    #[serde(default = "default_kitten_eat_boost_threshold")]
    pub kitten_eat_boost_threshold: f32,
    /// Ticket 156 — `KittenEatBoost` Modifier multiplier. The maximum
    /// multiplicative lift on a kitten's Eat score at
    /// `hunger_urgency = 1.0`. Default 4.0: at the empirical
    /// Robinkit-33 / Maplekit-98 frozen-breakdown urgency (~0.81)
    /// the lift is enough to push the kitten's Eat score past the
    /// social/grooming DSEs that previously dominated the breakdown.
    /// Multiplicative because the underlying Eat score is already
    /// shaped by curve + spatial axes; this just shifts the cohort.
    #[serde(default = "default_kitten_eat_boost_multiplier")]
    pub kitten_eat_boost_multiplier: f32,
    /// Ticket 156 — `KittenCryCaretakeLift` Modifier threshold. The
    /// `kitten_cry_perceived` floor above which the additive lift on
    /// non-kitten cats' Caretake score engages. Below threshold the
    /// modifier is a no-op (no quiet-cry over-response). Default
    /// 0.05 — any non-trivial perception fires the lift.
    #[serde(default = "default_kitten_cry_caretake_lift_threshold")]
    pub kitten_cry_caretake_lift_threshold: f32,
    /// Ticket 156 — `KittenCryCaretakeLift` Modifier additive lift on
    /// non-kitten cats' Caretake DSE at `kitten_cry_perceived = 1.0`.
    /// Linear ramp from threshold → 1.0. Default 0.5 — when the
    /// adult is hearing the loudest possible cry, Caretake's score
    /// gains +0.5, enough to dominate the legacy weighted-sum
    /// baseline (which empirically tops out around 0.4–0.6 for
    /// non-parent compassionate cats).
    #[serde(default = "default_kitten_cry_caretake_lift")]
    pub kitten_cry_caretake_lift: f32,
    /// Ticket 107 — `ExhaustionPressure` Modifier threshold. The
    /// `energy_deficit` floor below which the lift is a no-op. Default
    /// 0.7 (cat at energy 0.3 or below). Engages before the legacy
    /// `Exhaustion` interrupt (`energy < 0.10` ⇒ deficit > 0.90).
    #[serde(default = "default_exhaustion_pressure_threshold")]
    pub exhaustion_pressure_threshold: f32,
    /// Ticket 107 — `ExhaustionPressure` lift on Sleep. Largest lift —
    /// Sleep is the direct rest. Default 0.0 (inert); proposed 0.40.
    #[serde(default = "default_exhaustion_pressure_sleep_lift")]
    pub exhaustion_pressure_sleep_lift: f32,
    /// Ticket 107 — `ExhaustionPressure` lift on GroomSelf. Smaller
    /// lift — exhausted cats sometimes groom-then-sleep as a settling
    /// ritual per ticket §Scope. Default 0.0; proposed 0.10.
    #[serde(default = "default_exhaustion_pressure_groom_lift")]
    pub exhaustion_pressure_groom_lift: f32,
    /// Ticket 110 — `ThermalDistress` Modifier threshold. The
    /// `thermal_deficit` floor below which the lift is a no-op. Default
    /// 0.7 — a cat well outside its thermal comfort band. No legacy
    /// interrupt to retire on this axis; pure perception-richness lever.
    #[serde(default = "default_thermal_distress_threshold")]
    pub thermal_distress_threshold: f32,
    /// Ticket 110 — `ThermalDistress` lift on Sleep (find a den /
    /// hearth — routes the cat to a warm tile). Default 0.0 (inert);
    /// proposed 0.30. Build-shelter lift deferred per ticket §Out-of-
    /// scope.
    #[serde(default = "default_thermal_distress_sleep_lift")]
    pub thermal_distress_sleep_lift: f32,
    /// Ticket 108 — `ThreatProximityAdrenalineFlee` Modifier threshold.
    /// `threat_proximity_derivative` floor below which the lurch is a
    /// no-op. Default 0.4 — mirrors 047's adrenaline threshold so the
    /// two adrenaline branches have sibling onset semantics.
    #[serde(default = "default_threat_proximity_adrenaline_threshold")]
    pub threat_proximity_adrenaline_threshold: f32,
    /// Ticket 108 — `ThreatProximityAdrenalineFlee` lift on Flee.
    /// Default 0.60 (active) mirrors 047's Flee lift so the two
    /// adrenaline branches (health-deficit lurch and threat-proximity
    /// lurch) compose at sibling magnitudes when both fire.
    #[serde(default = "default_threat_proximity_adrenaline_flee_lift")]
    pub threat_proximity_adrenaline_flee_lift: f32,
    /// Ticket 108 — `ThreatProximityAdrenalineFlee` lift on Sleep
    /// (in-pool partner since Flee is filtered from disposition
    /// softmax; Sleep routes the cat to a den). Default 0.50 (active)
    /// mirrors 047's Sleep lift.
    #[serde(default = "default_threat_proximity_adrenaline_sleep_lift")]
    pub threat_proximity_adrenaline_sleep_lift: f32,
    /// Ticket 108 — `escape_viability` gate threshold for the Flee
    /// branch. Above this value, the Flee branch owns the response;
    /// below, the future Fight valence (108b) takes over. Default 0.4
    /// mirrors 102's gate — the two adrenaline frameworks (047/102 on
    /// health, 108/108b on threat-proximity) share the same viability
    /// pivot.
    #[serde(default = "default_threat_proximity_adrenaline_viability_threshold")]
    pub threat_proximity_adrenaline_viability_threshold: f32,
    /// Ticket 109 (Phase A) — `IntraspeciesConflictResponseFlight`
    /// Modifier threshold. The `social_status_distress` floor below
    /// which the lift is a no-op. Default 0.6 — mirrors the pressure-
    /// modifier family (106 on hunger). Pressure shape, not lurch:
    /// social-status pressure is gradual.
    #[serde(default = "default_intraspecies_conflict_flight_threshold")]
    pub intraspecies_conflict_flight_threshold: f32,
    /// Ticket 109 (Phase A) — `IntraspeciesConflictResponseFlight`
    /// lift on Flee (subordinate-retreat valence — the cat withdraws
    /// from the dominant). Default 0.30 (active) — pressure-shape
    /// magnitude mirroring 106's hunger-urgency lift.
    #[serde(default = "default_intraspecies_conflict_flight_lift")]
    pub intraspecies_conflict_flight_lift: f32,
    /// Ticket 142 (109 Phase B) — `IntraspeciesConflictResponseFreeze`
    /// Modifier threshold. The `social_status_distress` floor below
    /// which the Freeze lift is a no-op. Default mirrors Flight's
    /// threshold so the two valences share onset; the substrate-side
    /// `HideEligible` gate is what discriminates Hide-vs-Flee.
    #[serde(default = "default_intraspecies_conflict_freeze_threshold")]
    pub intraspecies_conflict_freeze_threshold: f32,
    /// Ticket 142 (109 Phase B) — `IntraspeciesConflictResponseFreeze`
    /// lift on Hide (subordinate-hold-position valence). **Default 0.0
    /// (inert)** — ships behind a balance follow-on that tunes the lift
    /// from 0.0 once the Hide-activation substrate (170 + 268)
    /// stabilizes. Tuning out of inert before that stabilization is a
    /// premature substrate-vs-modifier coupling.
    #[serde(default = "default_intraspecies_conflict_freeze_hide_lift")]
    pub intraspecies_conflict_freeze_hide_lift: f32,
    /// Ticket 109 (Phase A) — perception radius (Manhattan tiles) for
    /// "nearest other cat" resolution feeding `social_status_distress`.
    /// Cats further than this don't contribute to the social-status
    /// pressure axis. Default 8 — close enough that the dominant is
    /// in everyday-interaction range, far enough that random
    /// territorial overlap doesn't trip the pressure lift.
    #[serde(default = "default_social_perception_radius")]
    pub social_perception_radius: f32,
    /// Ticket 109 (Phase A) — weight on the `respect_diff` arm of the
    /// composite `social_status_distress` scalar:
    /// `clamp((nearest_other.respect - focal.respect), 0, 1)`. Default
    /// 0.333 (equal-weighted with age_diff and bond_asymmetry); raise
    /// this in balance ticks if status hierarchy should dominate the
    /// signal.
    #[serde(default = "default_social_status_distress_respect_weight")]
    pub social_status_distress_respect_weight: f32,
    /// Ticket 109 (Phase A) — weight on the `age_diff` arm of the
    /// composite. Older cats are social superiors;
    /// `age_diff = clamp((nearest_other.age - focal.age) /
    /// age_normalization_ticks, 0, 1)`. Default 0.333.
    #[serde(default = "default_social_status_distress_age_weight")]
    pub social_status_distress_age_weight: f32,
    /// Ticket 109 (Phase A) — weight on the `bond_asymmetry` arm.
    /// Distress lifts when the focal cat has a weaker bond to
    /// `nearest_other` than the colony average:
    /// `clamp(colony_avg_bond_to_other - focal.bond_to_other, 0, 1)`.
    /// Default 0.333.
    #[serde(default = "default_social_status_distress_bond_weight")]
    pub social_status_distress_bond_weight: f32,
    /// Ticket 109 (Phase A) — normalization horizon for `age_diff`.
    /// `age_diff = (other_age_ticks - focal_age_ticks) /
    /// age_normalization_ticks`. Default `ticks_per_season * 12 = 1
    /// sim-year` so a one-year age gap saturates the arm.
    #[serde(default = "default_social_status_distress_age_normalization_ticks")]
    pub social_status_distress_age_normalization_ticks: u64,
    pub wander_curiosity_scale: f32,
    pub wander_base: f32,
    pub wander_playfulness_bonus: f32,
    pub flee_safety_threshold: f32,
    pub flee_safety_scale: f32,
    pub fight_min_allies: usize,
    pub fight_ally_bonus_per_cat: f32,
    pub fight_boldness_scale: f32,
    /// HP threshold below which Fight score is suppressed.
    pub fight_health_suppression_threshold: f32,
    /// Safety threshold below which Fight score is linearly suppressed.
    pub fight_safety_suppression_threshold: f32,
    pub patrol_safety_threshold: f32,
    pub patrol_boldness_scale: f32,
    /// Upper-bound safety band above which the Patrol DSE's third
    /// consideration gates the score toward zero. Paired with the
    /// Guarding commitment-gate `guarding_exit_epsilon` in
    /// `DispositionConstants`: the commitment gate drops an active
    /// Guarding plan when safety climbs past the exit band
    /// (`critical_safety_threshold + guarding_exit_epsilon` ≈ 0.35);
    /// this threshold (0.5) is the further point at which Patrol DSE
    /// stops being picked at all. Together they give a graded exit —
    /// active plans drop at 0.35, re-selection suppresses at 0.5.
    /// See `docs/balance/guarding-exit-recipe.md` iter 2.
    #[serde(default = "default_patrol_exit_threshold")]
    pub patrol_exit_threshold: f32,
    pub build_diligence_scale: f32,
    pub build_repair_bonus: f32,
    pub farm_diligence_scale: f32,
    pub herbcraft_gather_spirituality_scale: f32,
    pub herbcraft_gather_skill_offset: f32,
    pub herbcraft_prepare_skill_offset: f32,
    pub herbcraft_prepare_injury_scale: f32,
    pub herbcraft_prepare_injury_cap: f32,
    pub herbcraft_ward_skill_offset: f32,
    pub herbcraft_ward_scale: f32,
    pub magic_durable_ward_scale: f32,
    pub magic_cleanse_corruption_threshold: f32,
    pub magic_commune_scale: f32,
    pub coordinate_diligence_scale: f32,
    pub coordinate_directive_scale: f32,
    pub coordinate_ambition_bonus: f32,
    pub mentor_temperature_diligence_scale: f32,
    pub mentor_ambition_bonus: f32,
    pub idle_base: f32,
    pub idle_incuriosity_scale: f32,
    pub idle_playfulness_penalty: f32,
    pub idle_minimum_floor: f32,
    pub pride_respect_threshold: f32,
    pub pride_bonus: f32,
    pub independence_solo_bonus: f32,
    pub independence_group_penalty: f32,
    pub patience_commitment_bonus: f32,
    pub memory_nearby_radius: f32,
    pub memory_resource_bonus: f32,
    pub memory_death_penalty: f32,
    pub memory_threat_penalty: f32,
    pub cascading_bonus_per_cat: f32,
    pub colony_knowledge_radius: f32,
    pub colony_knowledge_bonus_scale: f32,
    pub priority_bonus: f32,
    pub aspiration_bonus: f32,
    pub preference_like_bonus: f32,
    pub preference_dislike_penalty: f32,
    pub fated_love_social_bonus: f32,
    pub fated_rival_competition_bonus: f32,
    pub action_softmax_temperature: f32,
    pub disposition_softmax_temperature: f32,
    /// Softmax temperature for fox disposition selection. Matches
    /// `action_softmax_temperature` / `disposition_softmax_temperature` at
    /// 0.15 by default. Unused until the substrate refactor's Phase 3c
    /// retires `fox_scoring.rs`'s per-score jitter and wires fox
    /// disposition selection through the shared softmax path
    /// (§8.5 in `docs/systems/ai-substrate-refactor.md`).
    #[serde(default = "default_fox_softmax_temperature")]
    pub fox_softmax_temperature: f32,
    /// Ticket 232 — body-state-coupled L3 softmax temperature floor.
    /// `softmax_temperature_floor` and `softmax_temperature_ceiling`
    /// bracket the per-tick L3 softmax temperature used by §L2.10.6
    /// softmax-over-Intentions. The floor is hit at full body distress
    /// or rising-threat saturation: a 5% L2 score margin then picks
    /// the winner deterministically, so a wounded cat next to a fox
    /// does not coin-flip between work and survival. Curve:
    /// `T = ceiling - (ceiling - floor) × max(body_distress_composite,
    /// threat_proximity_derivative)`. Replaces the pre-232 fixed
    /// `intention_softmax_temperature = 0.15`.
    #[serde(default = "default_softmax_temperature_floor")]
    pub softmax_temperature_floor: f32,
    /// Ticket 232 — body-state-coupled L3 softmax temperature ceiling.
    /// Hit when body distress and threat-proximity derivative are both
    /// zero (calm, secure cat). Slightly broader than the pre-232
    /// fixed 0.15 so healthy-cat decisions stay narratively diverse.
    #[serde(default = "default_softmax_temperature_ceiling")]
    pub softmax_temperature_ceiling: f32,
    /// Ticket 175 — L2 carry-affinity bias. Multiplicative bonus
    /// applied to a DSE's pre-softmax score when the cat's current
    /// `Carrying` projection (computed by
    /// `Carrying::from_inventory`) maps to that DSE's terminal-
    /// product chain. Encodes "use what you're holding" as a soft
    /// preference at the L2 election layer rather than a hard
    /// veto at the planner layer (planner-side vetoes were
    /// removed in 175 in mirror of ticket 091's
    /// hunting/foraging fix). Calibrated so a Prey-carrying cat
    /// strongly prefers Hunting over Cooking when both are
    /// roughly equally needed, but does NOT override acute
    /// survival pressure: a starving cat at Stores still picks
    /// Eating regardless of carry. Setting this to `1.0`
    /// disables the bias entirely.
    #[serde(default = "default_carry_affinity_bonus")]
    pub carry_affinity_bonus: f32,
    /// 176: trailing-window length (in sim ticks) used to detect
    /// chronic Stores-overflow pressure. The
    /// `ColonyStoresChronicallyFull` marker is authored when the count
    /// of `Feature::DepositRejected` events in this window divided by
    /// colony cat-count exceeds `chronicity_threshold`. Default 1000
    /// ticks ≈ 16 sim seconds at 60 ticks/sec — long enough to filter
    /// transient overflow during deposit-then-cook cycles, short
    /// enough that genuine chronic pressure registers within a few
    /// tens of seconds.
    #[serde(default = "default_chronicity_window_ticks")]
    pub chronicity_window_ticks: u64,
    /// 176: rejected-deposits-per-cat-per-window threshold above which
    /// `ColonyStoresChronicallyFull` is authored. Default 0.10 — one
    /// deposit-rejection per cat per 10 windows = ~10k ticks. Below
    /// this rate the rejections look like noise and Build doesn't lift
    /// toward "more Stores."
    #[serde(default = "default_chronicity_threshold")]
    pub chronicity_threshold: f32,
    /// 084: per-kind capacity for `StoredHerbs` on each `Stores`
    /// building. Default 20 — large enough to buffer multiple
    /// ward-weaving cycles between gather trips, small enough to keep
    /// `ColonyThornbriarChronicallyLow` responsive when wild
    /// thornbriar availability drops. Per-kind so the four `HerbKind`s
    /// each have their own ceiling and one species can't crowd out
    /// the others.
    #[serde(default = "default_stores_herb_capacity_per_kind")]
    pub stores_herb_capacity_per_kind: u32,
    /// 084 Commit 3: colony-wide thornbriar stash total below which
    /// `ColonyThornbriarChronicallyLow` latches. Default 3 — calibrated
    /// for plausibility (a single SetWard consumes one thornbriar; ≥3
    /// means at least one buffer beyond the current ward demand
    /// signal). Sampled at `chronicity_window_ticks` boundaries by
    /// `update_colony_building_markers`. Below threshold across the
    /// window = chronic low; otherwise the marker clears.
    #[serde(default = "default_thornbriar_stash_low_threshold")]
    pub thornbriar_stash_low_threshold: u32,
    /// 084 Commit 3: FarmDse `farm_herb_pressure` MarkerConsideration
    /// `present_score` for `ColonyThornbriarChronicallyLow`. Default
    /// 1.0 preserves the prior `Curve::Linear(1.0, 0.0)` magnitude on
    /// the now-retired scalar — i.e. firing the marker contributes the
    /// same effective input to Farm's `CompensatedProduct` as the
    /// pre-Commit-3 0/1 scalar did. Tune downward to dampen the herb-
    /// pressure axis without dropping it entirely.
    #[serde(default = "default_farm_herb_pressure_weight")]
    pub farm_herb_pressure_weight: f32,
    /// 179: present-score on the `colony_stores_chronically_full`
    /// MarkerConsideration in BuildDse. Lifted from 178's dormant 0.0
    /// once the wave-closeout consumer (179) wired the marker into
    /// BuildDse — see `src/ai/dses/build.rs`. Plausibility default;
    /// balance-tuning is the follow-on ticket.
    #[serde(default = "default_build_chronic_full_weight")]
    pub build_chronic_full_weight: f32,
    /// 176: weight on the `colony_food_security` saturation axis in
    /// Hunt's DSE composition. Ships dormant at 0.0 — 181 closed
    /// with a documented predator-exposure cascade that survives
    /// recalibration; ticket 209 designs the paired-axis alternative.
    /// See `docs/balance/181-hunt-forage-saturation-tune.md`.
    #[serde(default = "default_hunt_food_security_weight")]
    pub hunt_food_security_weight: f32,
    /// 176: weight on the `colony_food_security` saturation axis in
    /// Forage's DSE composition. Ships dormant at 0.0 — sibling to
    /// `hunt_food_security_weight`; same 181 disposition, same
    /// follow-on (ticket 209).
    #[serde(default = "default_forage_food_security_weight")]
    pub forage_food_security_weight: f32,
    /// 209: weight on the positive `colony_food_security` axis in
    /// Mentor's WS composition. Tuned to 0.10 in ticket 210 — see
    /// `docs/balance/210-mentor-food-security.md` for hypothesis +
    /// observation (Mentor share flat, but cohesion canaries lifted
    /// and food-economy collapsed via unbonded mating; structural
    /// follow-on parked for later substrate work). Mirrors
    /// Hunt/Forage's saturation pattern but without `Invert` so high
    /// food security adds positive lift to the higher-tier DSE
    /// rather than suppressing it. `(1-w)` rebalance on the existing
    /// three axes preserves RtEO sum=1.0.
    #[serde(default = "default_mentor_food_security_weight")]
    pub mentor_food_security_weight: f32,
    /// 209: weight on the positive `colony_food_security` axis in
    /// Coordinate's WS composition. Sibling to
    /// `mentor_food_security_weight`. Ships dormant at 0.0.
    #[serde(default = "default_coordinate_food_security_weight")]
    pub coordinate_food_security_weight: f32,
    /// 209: weight on the positive `colony_food_security` axis in
    /// Caretake's WS composition. Sibling to
    /// `mentor_food_security_weight`. Ships dormant at 0.0.
    #[serde(default = "default_caretake_food_security_weight")]
    pub caretake_food_security_weight: f32,
    /// 397 Layer 2 — durable-commitment L2 lift on Caretake when the
    /// cat carries `HasJuvenileDependent` (i.e., is structurally
    /// inside a rear_kitten arc window). Approximates §L2.10.6
    /// softmax-over-Intentions for the rear_kitten ↔ Caretake pair:
    /// the durable commitment manifests as an additive score bias on
    /// the corresponding DSE rather than a hard frame-pin override.
    /// Magnitude derived from the observed Cook−Caretake L2 score gap
    /// in `logs/tuned-42-395-with-yield` (Cook avg 0.355, Caretake
    /// post-Layer-1 avg 0.101) — +0.25 brings Caretake's floor up to
    /// ~0.35 (parity) and acute-hunger ticks climb naturally above.
    /// Read at `score_actions` (`src/ai/scoring.rs:~1990`); applied
    /// post-DSE-eval and clamped to [0, 1]. Balance-doc iteration
    /// after first soak surfaces a tuned value if survival cadence is
    /// marginal.
    #[serde(default = "default_rear_kitten_caretake_lift")]
    pub rear_kitten_caretake_lift: f32,
    /// 209: weight on the `FoodSecurityGroomLift` modifier targeting
    /// GroomOther. Multiplicative shape `(1 + w · colony_food_security)`
    /// preserves CompensatedProduct gates in the underlying DSE. Ships
    /// dormant at 0.0; tuning is a follow-on ticket.
    #[serde(default = "default_groom_food_security_weight")]
    pub groom_food_security_weight: f32,
    /// 209: weight on the `fox_scent_at_position` cost axis in
    /// Patrol's CompensatedProduct composition. Reads
    /// `FoxScentMap::base_sample(cat_position)` (sensing-mediated, not
    /// omniscient). Curve `Composite{Logistic(6.0, 0.4), Invert}` —
    /// high scent → low axis → CP gate suppresses Patrol. Ships
    /// dormant at 0.0; tuning is a follow-on ticket. Addresses the 181
    /// cascade by pricing predator-exposure into Patrol's L2 score.
    #[serde(default = "default_patrol_fox_scent_weight")]
    #[deprecated(
        note = "228: replaced by `patrol_route_cost_weight` (destination-aware Field axis). Kept dormant for one cycle for header schema compat; remove in the tuning follow-on."
    )]
    pub patrol_fox_scent_weight: f32,
    /// 228: weight on Wander's `Consideration::Field` route-cost
    /// axis. Reads `OwnRouteCost` at `WanderTargetAnchor` — a
    /// deterministic seeded offset pre-picked at score time so
    /// Wander has a destination to price. Curve
    /// `Composite{Logistic(8.0, 0.5), Invert}`. Ships dormant at
    /// 0.0; tuning is a follow-on.
    #[serde(default = "default_wander_route_cost_weight")]
    pub wander_route_cost_weight: f32,
    /// 228: weight on Hunt's `Consideration::Field` route-cost axis.
    /// Reads `OwnRouteCost` at `NearestPreyAnchor` — the cat's
    /// flooded path-cost to the nearest prey-scent peak. Curve
    /// `Composite{Quadratic(2), Invert}` — sharper falloff than
    /// Forage's Logistic shape because Hunt is the urgency-tier
    /// action and a high-cost route should suppress it more
    /// aggressively. Ships dormant at 0.0; tuning is a follow-on.
    #[serde(default = "default_hunt_route_cost_weight")]
    pub hunt_route_cost_weight: f32,
    /// 228: weight on Explore's `Consideration::Field` route-cost
    /// axis. Reads `OwnRouteCost` at `UnexploredFrontierCentroid`
    /// — the cat's flooded path-cost to the unexplored frontier.
    /// Curve `Linear(slope=-1, intercept=1)` mirrors the existing
    /// `explore_frontier_distance` Spatial axis. Ships dormant at
    /// 0.0; tuning is a follow-on ticket.
    #[serde(default = "default_explore_route_cost_weight")]
    pub explore_route_cost_weight: f32,
    /// 228: weight on Forage's `Consideration::Field` route-cost
    /// axis. Reads `OwnRouteCost` at `NearestForageableCluster`
    /// — the cat's flooded path-cost to forageable terrain.
    /// Curve `Composite{Logistic(8.0, 0.5), Invert}` mirrors the
    /// existing `forage_cluster_distance` Spatial shape so dormant
    /// behavior matches baseline. Ships dormant at 0.0; tuning is
    /// a follow-on ticket.
    #[serde(default = "default_forage_route_cost_weight")]
    pub forage_route_cost_weight: f32,
    /// 228: weight on Patrol's `Consideration::Field` route-cost axis.
    /// Reads `OwnRouteCost` at `TerritoryPerimeterAnchor` — the cat's
    /// flooded path-cost to the patrol perimeter, including
    /// terrain + boldness-weighted fox-scent + corruption. Curve
    /// `Composite{Logistic(6.0, 0.4), Invert}` mirrors the dormant
    /// 209 `patrol_fox_scent_weight` shape; high route cost → low
    /// axis → CP gate suppresses Patrol when the *path to the
    /// perimeter* is risky, not just when the cat's cell is risky.
    /// 256 R4: activated (default 0.6) — was 0.0 in 228 pending the
    /// L3 patrol-cascade root-cause fix. Pairs with
    /// `patrol_path_fox_scent_weight` / `patrol_path_corruption_weight`
    /// which over-weight the overlays for Guarding-disposed cats.
    #[serde(default = "default_patrol_route_cost_weight")]
    pub patrol_route_cost_weight: f32,
    /// 256 R4: weight on the FoxScent overlay when constructing a
    /// Guarding-disposed cat's `RouteCostField`. Replaces the
    /// boldness-derived weight (which caps at 1.0 for the most timid
    /// cat). Default `1.5` — guardian cats route around fox-scent
    /// corridors more aggressively than the average pathing cat,
    /// pricing the predator-exposure cost of patrol exposure into
    /// the path geometry itself. Tune alongside `patrol_route_cost_weight`.
    #[serde(default = "default_patrol_path_fox_scent_weight")]
    pub patrol_path_fox_scent_weight: f32,
    /// 256 R4: weight on the Corruption overlay when constructing a
    /// Guarding-disposed cat's `RouteCostField`. Symmetric shape to
    /// `patrol_path_fox_scent_weight`. Default `1.5` — guardian cats
    /// avoid corruption corridors more aggressively than the average
    /// pathing cat, since the patrol role is precisely about not
    /// walking the colony into rot.
    #[serde(default = "default_patrol_path_corruption_weight")]
    pub patrol_path_corruption_weight: f32,
    /// 263: weight on Flee's 5th conditional axis `flee_affordance` —
    /// reads `ActionAffordances::read(cat, nearest_threat, ActionKind::Flee)`
    /// (substrate 261). The affordance heuristic composes proximity +
    /// cover-self + my-health + perceived-violence-capability into one
    /// `[0,1]` scalar. Pushed onto Flee's `CompensatedProduct` only
    /// when the weight is non-zero (CP semantics: `c · 0 = 0` would
    /// zero the whole product). Ships dormant at 0.0; activation in a
    /// follow-on with the four-artifact methodology since the new
    /// axis may shift the ShadowFoxAmbush canary.
    #[serde(default = "default_flee_affordance_weight")]
    pub flee_affordance_weight: f32,
    /// 268: weight on Hide's conditional Affordance(Freeze, self,
    /// NearestThreat) axis. Reads `ActionAffordances::read(cat,
    /// nearest_threat, ActionKind::Freeze)` (substrate 261). The
    /// affordance heuristic composes proximity + cover proximity +
    /// perceived-violence-capability into one `[0,1]` scalar. Pushed
    /// onto Hide's `CompensatedProduct` only when the weight is
    /// non-zero (CP semantics: `c · 0 = 0` would zero the whole
    /// product). Ships dormant at 0.0; activation in the balance
    /// follow-on tracked alongside the Hide activation substrate
    /// (170 + 142 + 268).
    #[serde(default = "default_hide_affordance_freeze_weight")]
    pub hide_affordance_freeze_weight: f32,
    /// 268: weight on Hide's conditional recency-of-threat-cue axis.
    /// Reads `max(PredatorBeliefs[nearest_threat].recency_of_threat_cue,
    /// ContextBeliefs[HereNow].recency_of_threat_cue)` so either a
    /// creature-specific belief OR an ambient-shock-lifted HereNow
    /// belief can drive Hide. Pushed onto CP only at non-zero weight.
    /// Ships dormant at 0.0.
    #[serde(default = "default_hide_recency_of_threat_cue_weight")]
    pub hide_recency_of_threat_cue_weight: f32,
    /// 268: weight on Hide's conditional perceived-intent-clarity
    /// axis. Reads
    /// `PredatorBeliefs[nearest_threat].perceived_intent_clarity`.
    /// Semantics: Hide wins under *unclear* intent (predator's
    /// commitment ambiguous); Flee wins under clear-hostile intent.
    /// The activation follow-on chooses the inversion direction via
    /// the per-axis curve; the substrate scalar surfaced here is raw
    /// clarity. Pushed onto CP only at non-zero weight. Ships
    /// dormant at 0.0.
    #[serde(default = "default_hide_perceived_intent_clarity_weight")]
    pub hide_perceived_intent_clarity_weight: f32,
    /// 263: weight on Patrol's 6th conditional axis
    /// `patrol_threat_recency` — reads `LocationBeliefs.recency_of_threat_cue`
    /// at the cat's patrol perimeter anchor bucket (substrate 258).
    /// Curve `Composite{Linear, Invert}` — high recency → low patrol
    /// attractiveness. Pushed onto Patrol's `CompensatedProduct` only
    /// when the weight is non-zero. Ships dormant at 0.0; activation
    /// in a follow-on with the four-artifact methodology since
    /// per-cat-subjective threat memory will shift Patrol's L3 share
    /// and is the substrate-side fix for the L3 patrol-absorption
    /// cascade (memory `project_l3_patrol_absorption_cascade`).
    #[serde(default = "default_patrol_threat_recency_weight")]
    pub patrol_threat_recency_weight: f32,
    /// 263: weight on Hunt's per-target 5th axis
    /// `hunt_best_predation_affordance` — reads
    /// `max(Affordance(Stalk|Chase|Pounce))` for `(self, prey)` from
    /// substrate 261. Encodes "is this prey actually catchable in any
    /// predation form?" as an orthogonal axis to yield + alertness +
    /// cooldown. WeightedSum composition: the other four axes are
    /// scaled by `(1 - this weight)` to keep the sum at 1.0 when
    /// activated. Ships dormant at 0.0.
    #[serde(default = "default_hunt_best_predation_weight")]
    pub hunt_best_predation_weight: f32,
    /// 100 — orthogonal `prey_alertness_tolerance` axis weight on the
    /// HuntTarget DSE. Input is `boldness × alertness`. The signal
    /// peaks when a bold cat eyes nervous prey — exactly the "I'm
    /// bold *and* you're alert" case the existing single-axis
    /// `prey_calm` penalty can't represent. WeightedSum composition:
    /// when non-zero, the other axes (and any 263 affordance axis)
    /// renormalize so the sum stays at 1.0. Ships live at 0.15 — a
    /// meaningful bias toward bold-cat acceptance of nervous prey
    /// without dominating the yield + pursuit-cost signal.
    #[serde(default = "default_hunt_alertness_tolerance_weight")]
    pub hunt_alertness_tolerance_weight: f32,
    /// 264: weight on SocializeTarget's conditional `target_affiliation`
    /// axis — reads the actor's own `CatBeliefs[target].affiliation_history`
    /// facet (substrate 258), mapped `[-1, 1] → [0, 1]` with a 0.5
    /// neutral default for unmodeled / zero-strength targets. The
    /// asymmetric per-perceiver belief that supersedes the symmetric
    /// `Relationships.fondness` read (substrate first, legacy axis
    /// retires second — pillar 2; the fondness axis stays until the
    /// belief axis has soak history). WeightedSum: the seven base
    /// axes scale by `(1 − Σ 264 extras)` when non-zero. Activated
    /// 2026-07-08 at first-light 0.10 (plan step 20, four-artifact).
    #[serde(default = "default_socialize_affiliation_weight")]
    pub socialize_affiliation_weight: f32,
    /// 264: weight on SocializeTarget's conditional
    /// `affordance_socialize` axis — reads `Affordance(Socialize,
    /// self, target)` from substrate 261 (estimator: proximity +
    /// affiliation + low hostility + receptivity). Activated
    /// 2026-07-08 at first-light 0.10 (plan step 20).
    #[serde(default = "default_socialize_affordance_weight")]
    pub socialize_affordance_weight: f32,
    /// 264: weight on GroomOtherTarget's conditional
    /// `target_affiliation` axis — same read + mapping as
    /// `socialize_affiliation_weight`. Activated 2026-07-08 at
    /// first-light 0.10 (plan step 20).
    #[serde(default = "default_groom_other_affiliation_weight")]
    pub groom_other_affiliation_weight: f32,
    /// 264: weight on GroomOtherTarget's conditional
    /// `target_perceived_hostility` axis — reads
    /// `CatBeliefs[target].perceived_hostility` (fast aggressive-intent
    /// read, distinct from slow affiliation) through an inverted
    /// Linear curve so high perceived hostility deprioritizes the
    /// grooming candidate ("don't groom the cat that just hissed at
    /// you"). Unmodeled targets read 0.0 hostility → no penalty
    /// (fail-open). Activated 2026-07-08 at first-light 0.10.
    #[serde(default = "default_groom_other_hostility_weight")]
    pub groom_other_hostility_weight: f32,
    /// 264: weight on GroomOtherTarget's conditional
    /// `affordance_groom_other` axis — reads `Affordance(GroomOther,
    /// self, target)` from substrate 261. Activated 2026-07-08 at
    /// first-light 0.10.
    #[serde(default = "default_groom_other_affordance_weight")]
    pub groom_other_affordance_weight: f32,
    /// 264: weight on MateTarget's conditional
    /// `target_perceived_receptivity` axis — reads
    /// `CatBeliefs[target].perceived_receptivity` (is the partner open
    /// to courtship *right now*, distinct from long-run bond). The
    /// downstream lever on the 126/027 Mate supply-chain problem:
    /// low-receptivity partners stop winning the pick and oscillating.
    /// Unmodeled targets read a 0.5 neutral prior. Activated
    /// 2026-07-08 at first-light 0.12; the gate verified the 027
    /// Mate-cadence canary.
    #[serde(default = "default_mate_receptivity_weight")]
    pub mate_receptivity_weight: f32,
    /// 264: weight on MateTarget's conditional `affordance_mate` axis —
    /// reads `Affordance(Mate, self, target)` from substrate 261
    /// (estimator: fertility proxy + bond + receptivity + proximity).
    /// Activated 2026-07-08 at first-light 0.10.
    #[serde(default = "default_mate_affordance_weight")]
    pub mate_affordance_weight: f32,
    /// 264: weight on MentorTarget's conditional `affordance_mentor`
    /// axis — reads `Affordance(Mentor, self, target)` from substrate
    /// 261 (estimator: bond + receptivity + my condition + proximity).
    /// Mentor gets no direct belief axis — receptivity lives at the
    /// affordance layer per the Hunt architectural rule (263).
    /// Activated 2026-07-08 at first-light 0.10.
    #[serde(default = "default_mentor_affordance_weight")]
    pub mentor_affordance_weight: f32,
    /// 264: weight on CaretakeTarget's conditional
    /// `affordance_feed_kitten` axis — reads `Affordance(FeedKitten,
    /// self, target)` from substrate 261 (estimator: kitten hunger +
    /// my food proxy + bond + proximity). Caretake is the FeedKitten
    /// consumer (the hungry-kitten picker); the rearing-arc
    /// `dependent_kitten_target` has no ActionKind analog and stays
    /// unwired. Activated 2026-07-08 at first-light 0.10.
    #[serde(default = "default_caretake_affordance_weight")]
    pub caretake_affordance_weight: f32,
    /// 264: weight on ApplyRemedyTarget's conditional
    /// `target_perceived_injury` axis — reads the actor's own
    /// `CatBeliefs[patient].perceived_injury_level` facet through the
    /// same convex Quadratic(2) as the raw `target_injury` axis.
    /// ApplyRemedy is the `Care` consumer (261 estimator table:
    /// `perceived_injury_level + bond`) — it owns the only raw-HP
    /// target read (`1 − health.current/health.max`), which this
    /// belief axis SUPERSEDED at the 2026-07-08 activation (raw axis
    /// retired from the default composition, pillar 2; the belief
    /// axis holds the full 8/14 triage slot). Unmodeled patients read
    /// 0.0 — no belief of injury → no triage lift.
    #[serde(default = "default_apply_remedy_injury_belief_weight")]
    pub apply_remedy_injury_belief_weight: f32,
    /// 264: weight on ApplyRemedyTarget's conditional `affordance_care`
    /// axis — reads `Affordance(Care, self, target)` from substrate
    /// 261. Activated 2026-07-08 at first-light 0.10.
    #[serde(default = "default_apply_remedy_affordance_weight")]
    pub apply_remedy_affordance_weight: f32,
    /// 265: weight on FoxHunting's conditional
    /// `best_prey_predation_affordance` axis — max over prey in
    /// detection range of `Affordance(Stalk|Chase, fox, prey)` from
    /// substrate 261, fed by the 314 wildlife-vs-prey writer rows.
    /// Activated 2026-07-08 at first-light 0.10.
    #[serde(default = "default_fox_hunting_prey_affordance_weight")]
    pub fox_hunting_prey_affordance_weight: f32,
    /// 265: weight on HawkHunting's conditional
    /// `best_prey_predation_affordance` axis — max over prey in
    /// detection range of `Affordance(Dive|Chase, hawk, prey)`.
    /// Activated 2026-07-08 at first-light 0.10.
    #[serde(default = "default_hawk_hunting_prey_affordance_weight")]
    pub hawk_hunting_prey_affordance_weight: f32,
    /// 265: weight on SnakeAmbushing's conditional
    /// `best_prey_strike_affordance` axis — max over prey in detection
    /// range of `Affordance(Strike, snake, prey)`. Strike is
    /// adjacency-gated in the writer, so this axis rewards holding an
    /// ambush spot prey actually pass. Activated 2026-07-08 at
    /// first-light 0.10.
    #[serde(default = "default_snake_ambush_strike_affordance_weight")]
    pub snake_ambush_strike_affordance_weight: f32,
    /// 265: weight on SnakeForaging's conditional
    /// `best_prey_stalk_affordance` axis — max over prey in detection
    /// range of `Affordance(Stalk, snake, prey)`. Activated 2026-07-08
    /// at first-light 0.10.
    #[serde(default = "default_snake_forage_stalk_affordance_weight")]
    pub snake_forage_stalk_affordance_weight: f32,
    /// 265: weight on FoxFleeing's conditional `perceived_cat_threat`
    /// axis — max over cats in avoidance range of the fox's own
    /// `CatBeliefs[cat].perceived_violence_capability` facet (implanted
    /// from `cat_perceived_by_fox` on first encounter, updated by
    /// witnessed Attack/Hunt evidence). The wildlife-symmetric peer of
    /// the cat-side PredatorBeliefs read. Activated 2026-07-08 at
    /// first-light 0.10.
    #[serde(default = "default_fox_flee_cat_violence_belief_weight")]
    pub fox_flee_cat_violence_belief_weight: f32,
    /// 265 activation: `perceived_cat_threat` level at which FoxFleeing
    /// becomes *eligible* for a healthy, un-outnumbered fox (the legacy
    /// outer gate is `health < 0.5 || cats_nearby >= 2`; this adds the
    /// belief clause). Sits well above the 0.5 `cat_perceived_by_fox`
    /// implant prior so instinct alone never trips it — only witnessed
    /// Attack/Hunt evidence does. Inert while
    /// `fox_flee_cat_violence_belief_weight` is 0.0.
    #[serde(default = "default_fox_flee_belief_eligibility_threshold")]
    pub fox_flee_belief_eligibility_threshold: f32,
    /// 265: weight on HawkFleeing's conditional `perceived_cat_threat`
    /// axis. Activated 2026-07-08 at first-light 0.10.
    #[serde(default = "default_hawk_flee_cat_violence_belief_weight")]
    pub hawk_flee_cat_violence_belief_weight: f32,
    /// 265 activation: hawk-side peer of
    /// `fox_flee_belief_eligibility_threshold` — belief level opening
    /// the HawkFleeing outer gate for a healthy, un-outnumbered hawk.
    /// Inert while `hawk_flee_cat_violence_belief_weight` is 0.0.
    #[serde(default = "default_hawk_flee_belief_eligibility_threshold")]
    pub hawk_flee_belief_eligibility_threshold: f32,
    /// 265: weight on SnakeFleeing's conditional `perceived_cat_threat`
    /// axis. Activated 2026-07-08 at first-light 0.10 (no eligibility
    /// clause — the snake gate already admits one nearby cat).
    #[serde(default = "default_snake_flee_cat_violence_belief_weight")]
    pub snake_flee_cat_violence_belief_weight: f32,
    /// 263: bias magnitude on the Hunt resolver's `stalk_start` band
    /// threshold inside `resolve_engage_prey`. At bias `0.0` (default,
    /// dormant) the threshold is unchanged from the distance-keyed
    /// formula; at bias `b`, `stalk_start` is multiplied by
    /// `(1 + b * (a_stalk - a_chase))` clamped to `±b`, where
    /// `a_stalk` and `a_chase` are the affordances for the current
    /// `(self, prey)` state. High stalk-affordance widens the stalk
    /// band (cat starts stalking from farther out); high chase-
    /// affordance narrows it (cat transitions to chase sooner).
    /// `pounce_range` is NOT biased — the leap is a physics
    /// invariant the catch math relies on.
    #[serde(default = "default_hunt_stalk_chase_affordance_bias")]
    pub hunt_stalk_chase_affordance_bias: f32,
    /// 220: weight on the `RecentAmbushMap` lift to ward-placement's
    /// threat term in `compute_ward_placement()`. Curve
    /// `Logistic(steepness=8.0, midpoint=0.5)` applied to the per-tile
    /// ambush memory sample; the result adds into the placement scorer's
    /// `max(fox_scent, corruption)` threat axis, then re-clamps to 1.0.
    /// Forcing 0.0 returns the formula to byte-identical pre-220
    /// behavior (regression-guarded by
    /// `ward_placement_dormant_when_weights_forced_to_zero`). 284
    /// activated the substrate at 0.5 as a first-light landing —
    /// biases new wards toward tiles where ShadowFox ambushes have
    /// actually struck, closing the 210-soak gap where fox-scent
    /// perimeters drew wards away from the empirical hot zones.
    #[serde(default = "default_ward_ambush_anchor_weight")]
    pub ward_ambush_anchor_weight: f32,
    /// 220: weight on the `CarcassScentMap` lift to ward-placement's
    /// threat term. Symmetric shape to `ward_ambush_anchor_weight` —
    /// `Logistic(8.0, 0.5)` over the per-tile carcass-scent sample,
    /// additively combined into the threat axis. Restores the
    /// recency-anchor consumer originally scoped in 209 §Scope (line
    /// 74) but trimmed from the actual landing; the `CarcassScentMap`
    /// substrate itself is already in place from Phase 2C. 284
    /// activated at 0.3 as a first-light landing — carcass scent
    /// persists longer than the event-decay ambush map, so the
    /// smaller weight avoids chronic-corpse drag toward old kill-sites.
    #[serde(default = "default_ward_recency_anchor_weight")]
    pub ward_recency_anchor_weight: f32,
    /// 296: steepness `k` of the Logistic curve `L(x) = 1/(1+exp(-k(x-m)))`
    /// applied to each per-tile threat-axis lift in
    /// `compute_ward_placement()` (ambush, carcass, fox-intercept). Promoted
    /// from the previously-hardcoded `8.0` after 285's three-seed
    /// triangulation showed that at `k=8.0, m=0.5` the curve saturates
    /// near 1.0 on any hot tile at anchor weights ≥ ~0.3, making weight
    /// magnitude architecturally inert in that regime
    /// (`docs/balance/284-ward-anchor-tuning.md` iter-2). Softer
    /// steepness restores per-tile gradient that the anchor weights can
    /// then bias. Default `8.0` preserves pre-296 behavior; 296's
    /// hypothesize sweep tunes from there.
    #[serde(default = "default_ward_placement_logistic_steepness")]
    pub ward_placement_logistic_steepness: f32,
    /// 296: midpoint `m` of the Logistic curve `L(x) = 1/(1+exp(-k(x-m)))`
    /// applied to each per-tile threat-axis lift in
    /// `compute_ward_placement()`. Sibling of
    /// `ward_placement_logistic_steepness` — shifts the inflection point
    /// along the [0, 1] threat axis. Default `0.5` preserves pre-296
    /// behavior.
    #[serde(default = "default_ward_placement_logistic_midpoint")]
    pub ward_placement_logistic_midpoint: f32,
    /// 300: stride (in tiles) of the coarse-grid candidate generation in
    /// `compute_ward_placement()`. The scorer enumerates candidates at
    /// `(x, y)` where `x % step == 0` and `y % step == 0`, so the chosen
    /// argmax can only ever land on a multiple-of-`step` tile. Promoted
    /// from the previously-hardcoded `5` because every `WardPlaced`
    /// position across 285+296+297 sat at multiples of 5 by construction
    /// — finer-grained sampling tests whether the placement-grid was the
    /// binding constraint. Within a bucket, threat-axis terms are flat
    /// (per-bucket influence maps with no interpolation); the only
    /// sub-bucket signal is the per-tile distance-to-anchor penalty,
    /// which pulls the optimum toward the anchor. Default `5` preserves
    /// pre-300 behavior.
    #[serde(default = "default_ward_placement_candidate_step")]
    pub ward_placement_candidate_step: i32,
    /// 297: weight on the `FoxSpawnVicinityMap` lift to ward-placement's
    /// threat term. Curve `Logistic(steepness, midpoint)` (shared with
    /// the ambush + carcass anchors, 296) applied to the per-tile
    /// vicinity sample; the result adds into the placement scorer's
    /// `max(fox_scent, corruption)` threat axis, then re-clamps to 1.0.
    /// Forcing 0.0 returns the formula to byte-identical pre-297
    /// behavior (regression-guarded by
    /// `ward_placement_dormant_when_weights_forced_to_zero`). Ships
    /// dormant; first-light activation per
    /// `feedback_dormant_substrate_activation_soak_first` lifts to
    /// match `ward_ambush_anchor_weight` (default 0.5). Closes 285's
    /// architectural gap: existing inputs encode where cats got hurt;
    /// this axis encodes where fox patrols enter the colony (halo
    /// around corruption-tile spawn sources).
    #[serde(default = "default_ward_fox_intercept_anchor_weight")]
    pub ward_fox_intercept_anchor_weight: f32,
    /// 297: kernel radius in world tiles for the inline fox-spawn-vicinity
    /// computation in `compute_ward_placement`. For each candidate tile,
    /// the scorer scans tiles within this Manhattan radius for ShadowFox
    /// spawn-eligible corruption (≥ `shadow_fox_corruption_threshold`).
    /// Default `20` — large enough to cover the approach corridor a fox
    /// walks from a corruption tile before reaching cat territory, small
    /// enough that the per-candidate scan cost (~800 tile lookups) stays
    /// cheap. Computed inline (not via a populated Resource) to avoid
    /// schedule-edge perturbation of seed-42 (ticket 061 precedent).
    #[serde(default = "default_fox_intercept_kernel_radius_tiles")]
    pub fox_intercept_kernel_radius_tiles: u32,
    /// 298: weight on the `CatScentMap` lift to ward-placement's
    /// argmax tiebreak among threat-saturated tiles. The `0.3` default
    /// preserves ticket 045's first-light reasoning ("modest weight
    /// keeps placement biased toward where cats actually live") and
    /// the formula at `src/systems/coordination.rs:1557`:
    ///
    /// ```text
    /// score = unaddressed_threat + W * cat_value - distance_cost + jitter
    /// ```
    ///
    /// Tuning this weight is the first non-threat-axis lever after
    /// 285/296/297 established that threat-axis inputs are
    /// rank-preserving for the argmax once any threat-side input
    /// saturates a sufficient number of tiles
    /// (`docs/balance/297-fox-patrol-topology-axis.md` iter-2). See
    /// `docs/balance/298-ward-placement-cat-value-coefficient.md`.
    ///
    /// 313: only used when
    /// `ward_placement_cat_value_composition == Additive` (the
    /// default). Under `Gate` the formula drops the additive
    /// `+ W * cat_value` term entirely and uses
    /// `ward_placement_cat_value_gate_floor` instead.
    #[serde(default = "default_ward_placement_cat_value_weight")]
    pub ward_placement_cat_value_weight: f32,
    /// 301: selection rule for the coordinator's ward-placement scorer
    /// in `compute_ward_placement()`. After 285+296+297+298+300 ruled
    /// out five threat- and tiebreak-side levers as movers of the
    /// argmax across seeds 42/99/7, ticket 301 targets the composition
    /// rule itself. `SingleShotArgmax` (default) reproduces the
    /// pre-301 single-best-tile selection byte-for-byte.
    /// `DescendingResidual` runs K=`ward_placement_residual_rounds`
    /// rounds of submodular greedy inside the scorer: each round
    /// stamps virtual coverage around its winner so the next round
    /// re-scores against a partially-eaten threat surface, returning
    /// the K-th pick. Spreads successive placements across the threat
    /// field instead of co-locating in the same saturated cluster.
    ///
    /// **First enum-typed field in `ScoringConstants`.** Serde
    /// serializes the variant as a JSON string into the events.jsonl
    /// header (`headless_io.rs:263`), so the comparability invariant
    /// is preserved. Variant names are stable identifiers — never
    /// rename in place; add new variants and deprecate old ones.
    #[serde(default = "default_ward_placement_semantics")]
    pub ward_placement_semantics: WardPlacementSemantics,
    /// 301: K (number of greedy rounds) used by
    /// `WardPlacementSemantics::DescendingResidual`. K=1 is identical
    /// to `SingleShotArgmax` (round 0 = argmax). Default `2` lifts
    /// once when the descending-residual flag is on. Ignored when
    /// `ward_placement_semantics == SingleShotArgmax`. Clamped `>= 1`
    /// defensively at use-site.
    #[serde(default = "default_ward_placement_residual_rounds")]
    pub ward_placement_residual_rounds: i32,
    /// 301: weight on the `WardIntentMap` lift to the Path-B
    /// `HerbcraftWardDse` score (`src/ai/dses/herbcraft_ward.rs`).
    /// Default `0.0` — substrate is wired but dormant at land per the
    /// 220 / 297 first-light pattern, so the DSE score is byte-identical
    /// pre-301 at the default. Coordinator-stamped intent (populated
    /// by `compute_ward_placement` when `ward_placement_semantics ==
    /// DescendingResidual`) biases cats toward planting on tiles the
    /// colony's spread logic has marked. The DSE doesn't pick a target
    /// position — it boosts the score when the cat is *standing on* an
    /// intent tile, so wandering cats whose path crosses an intent
    /// tile are more likely to commit there.
    #[serde(default = "default_ward_intent_dse_weight")]
    pub ward_intent_dse_weight: f32,
    /// 301: multiplicative per-wake decay factor applied to
    /// `WardIntentMap` at the start of each coordinator wake under
    /// `WardPlacementSemantics::DescendingResidual`. Applied to every
    /// bucket before the round picks stamp fresh intent. `0.5` →
    /// ~one-wake half-life on a stamp that wasn't refreshed (~20 ticks
    /// at default `coordination.assess_interval`). Dormant when
    /// semantics is `SingleShotArgmax` (the populator short-circuits
    /// so decay never runs either) — at default `SimConstants` this
    /// field is allocated but never read.
    #[serde(default = "default_ward_intent_decay_per_wake")]
    pub ward_intent_decay_per_wake: f32,
    /// 312: weight on the `FoxApproachCorridorMap` lift in
    /// `compute_ward_placement()`. Composes **multiplicatively
    /// outside** the saturating threat sum:
    ///
    /// ```text
    /// score = unaddressed_threat * (1.0 + w_corridor * L(corridor))
    ///       + w_cat_value * cat_value - distance_cost + jitter
    /// ```
    ///
    /// At `0.0` (the default) the `(1.0 + 0.0 * L(x)) = 1.0` factor
    /// preserves byte-identical pre-312 behavior. Above 0.0, tiles
    /// with high observed fox traffic lift the threat term **past
    /// the [0, 1] saturation ceiling** — escaping the
    /// rank-preservation pathology that
    /// `docs/balance/297-fox-patrol-topology-axis.md` iter-2 ruled
    /// in (additive lifts inside `.min(1.0)` are rank-preserving for
    /// argmax once any threat input saturates).
    ///
    /// FO-1 scenario (`chokepoint_defense_isthmus`) activates at
    /// `0.3` to assert the isthmus-corked outcome; three-seed
    /// `just hypothesize` validates the same weight against
    /// `shadow_foxes_avoided_ward_total` direction match.
    /// First-light global activation is FO-3 (separate ticket).
    #[serde(default = "default_ward_fox_approach_corridor_weight")]
    pub ward_fox_approach_corridor_weight: f32,
    /// 313: composition rule for the `CatScentMap` lift to
    /// ward-placement scoring. See
    /// [`WardPlacementCatValueComposition`]. Default `Additive`
    /// preserves the pre-313 score formula bit-for-bit. `Gate`
    /// (option (c) from 301 FO-3) replaces the additive reward
    /// with a saturating-ramp multiplicative gate on the
    /// threat-merit term.
    ///
    /// FO-1 scenario (`chokepoint_defense_isthmus`) activates
    /// `Gate` to assert the isthmus-corked outcome alongside
    /// `ward_fox_approach_corridor_weight = 0.3`; three-seed
    /// `just hypothesize` validates the same composition.
    /// First-light global activation is deferred to a follow-on
    /// iter once the four-artifact concordance is on paper.
    #[serde(default = "default_ward_placement_cat_value_composition")]
    pub ward_placement_cat_value_composition: WardPlacementCatValueComposition,
    /// 313: knee point for the saturating-ramp gate when
    /// `ward_placement_cat_value_composition == Gate`. The gate
    /// is `(cat_value / floor).clamp(0, 1)`, so `cat_value = 0`
    /// yields gate 0 (dead tile fully suppressed) and any
    /// `cat_value >= floor` yields gate 1 (full merit, no
    /// density-peak reward). Default `0.2` per ticket 313's
    /// pseudocode. Ignored when composition is `Additive`.
    #[serde(default = "default_ward_placement_cat_value_gate_floor")]
    pub ward_placement_cat_value_gate_floor: f32,
    /// 256 R5: deposit per tick when a cat's current action is
    /// `Action::Patrol`. Lays a deterrent gradient that foxes read as
    /// routing cost via `CatPatrolDeterrentOverlay`. Default `0.05`
    /// — a peak (1.0) bucket reaches saturation after ~20 ticks of
    /// continuous patrol, balancing "patrols actually deter foxes"
    /// against "single transient patrol step doesn't lock foxes out
    /// of an ambush corridor for hours."
    #[serde(default = "default_cat_patrol_deterrent_deposit_per_tick")]
    pub cat_patrol_deterrent_deposit_per_tick: f32,
    /// 256 R5: global decay rate for `CatPatrolDeterrentMap`,
    /// expressed per in-game day. Default `0.5/day` — a peak bucket
    /// fades to zero over ~2 in-game days, matching Patrol's
    /// short-term coverage semantics (foxes test fresh patrols, not
    /// stale ones from days prior). Faster decay than fox-scent's
    /// 0.1/day because cat patrol is a *deterrent presence*, not a
    /// territorial mark.
    #[serde(default = "default_cat_patrol_deterrent_decay_rate")]
    pub cat_patrol_deterrent_decay_rate: RatePerDay,
    /// 256 R5: maximum additive cost contribution from cat patrol
    /// deterrent on a single tile during fox A*. Default `6` — less
    /// than `fox_scent_path_cost_max=8` so foxes detour around
    /// patrols rather than refuse to move toward prey at all. Sits
    /// above DenseForest's `movement_cost = 3` so foxes prefer
    /// patrolled forest only when no patrol-free path exists.
    #[serde(default = "default_cat_patrol_deterrent_path_cost_max")]
    pub cat_patrol_deterrent_path_cost_max: u32,
    /// 256 R5: scalar weight applied to the deterrent overlay in fox
    /// pathfinding. Default `1.0` — full effect. Below 1.0 makes
    /// foxes less averse to crossing patrols (e.g., desperate
    /// hunger); above 1.0 makes them more averse. Pairs with the
    /// `path_cost_max` constant: `effective_cost = round(deterrent *
    /// path_cost_max * overlay_weight)`.
    #[serde(default = "default_cat_patrol_deterrent_overlay_weight")]
    pub cat_patrol_deterrent_overlay_weight: f32,
    /// 223: Maximum additive cost contribution from fox-scent on a single
    /// tile during cat A* pathfinding. Cat path-cost overlay scales
    /// `FoxScentMap::get(x, y).clamp(0.0, 1.0) * fox_scent_path_cost_max`
    /// (rounded). Default `8` — chosen so a cat avoids a single
    /// max-scent tile in favor of a 4-grass-tile detour (cost 4 < 8),
    /// while a long path through a corridor of fox-scent tiles is
    /// still walkable when no detour exists. The buffer sits above
    /// DenseForest's `movement_cost = 3` so cats prefer scented forest
    /// only when no scent-free path exists.
    #[serde(default = "default_fox_scent_path_cost_max")]
    pub fox_scent_path_cost_max: u32,
    /// 508 — max A* edge-cost contribution of the routing cat's own
    /// `LocationBeliefs.recency_of_threat_cue` at a tile's belief
    /// bucket (`ThreatBeliefOverlay`). Above fox-scent's 8: scent
    /// marks territory, this marks witnessed-ambush ground. At 12, a
    /// max-cue bucket is worth a 12-tile detour on grass.
    #[serde(default = "default_threat_belief_path_cost_max")]
    pub threat_belief_path_cost_max: u32,
    /// 223: Maximum additive cost contribution from corruption on a
    /// single tile during cat A* pathfinding. Symmetric shape to
    /// `fox_scent_path_cost_max`. Default `10` — slightly higher because
    /// corruption is rarer in healthy colony states and ecologically
    /// more corrosive than fox scent (corruption persists; scent
    /// decays). Tune alongside `fox_scent_path_cost_max` if balance
    /// surfaces an asymmetry that doesn't reflect the fiction.
    #[serde(default = "default_corruption_path_cost_max")]
    pub corruption_path_cost_max: u32,
    /// 228: Cap on the per-cat `flood_dijkstra` flood radius (cost
    /// units, not tiles). Tiles whose tentative cost exceeds this
    /// stay unreached at `MAX_COST_BUDGET`. Default `600` — terrain
    /// max=4 + per-tile weighted-overlay max ≈ 18 ⇒ avg edge ~10;
    /// 600 covers a ~30-tile flood radius for typical overlays. Lower
    /// values shrink flood cost at the price of fewer destinations
    /// being scored as reachable. Bounded above by
    /// `route_cost_field::MAX_COST_BUDGET`.
    #[serde(default = "default_route_cost_flood_budget")]
    pub route_cost_flood_budget: u32,
    /// 228: Tick window over which `WanderTargetAnchor` rotates its
    /// seeded offset. Default `30` — at 1 Hz sim cadence ≈ 30 s wall
    /// time. Plans typically commit before the candidate moves, so
    /// L3 doesn't thrash on a destination that walks out from under
    /// it; over a longer horizon the wander destination drifts so
    /// cats don't pin to a single seed forever.
    #[serde(default = "default_wander_recandidate_ticks")]
    pub wander_recandidate_ticks: u64,
    /// 228: Staleness window for `CatPathPlan::should_fall_back_at`.
    /// A cached `RouteCostField` whose `origin_tick` is older than
    /// `current_tick - route_cost_replan_window_ticks` forces an A*
    /// fallback (and emits `Feature::RouteCostFieldFallback`). Default
    /// `120` — ~2 sim-minutes; tight enough to catch a stuck cat
    /// drifting on an old field, loose enough that mid-plan walks
    /// rarely hit the cliff. Independent of the per-replan field
    /// rebuild cadence in `evaluate_and_plan` (which is governed by
    /// `Without<GoapPlan>`-driven replanning, not a tick window).
    #[serde(default = "default_route_cost_replan_window_ticks")]
    pub route_cost_replan_window_ticks: u64,
    /// 209: weight on the `TensionDefusionGroomLift` modifier
    /// targeting GroomOther. Multiplicative lift when
    /// `colony_tension_recent` is elevated AND `HasGroomingCandidate`
    /// is set. Captures real-cat allogrooming's tension-defusion role
    /// (van den Bos 1998). Ships dormant at 0.0.
    #[serde(default = "default_tension_defusion_groom_weight")]
    pub tension_defusion_groom_weight: f32,
    /// 178: Logistic slope on the `inventory_excess` axis used by the
    /// Discarding and Trashing DSEs. Higher slope → sharper lift past
    /// the midpoint (cats with full inventory react harder once they
    /// cross the threshold). Default 8.0 mirrors the post-044 hangry
    /// curve — a conservative starting shape known to behave well in
    /// L3 softmax under unit-weight composition.
    #[serde(default = "default_disposal_inventory_excess_slope")]
    pub disposal_inventory_excess_slope: f32,
    /// 178: Logistic midpoint on the `inventory_excess` axis. The
    /// scalar reaches 0.5 at this fraction of `Inventory::MAX_SLOTS`.
    /// Default 0.5 — half the inventory's food capacity. Below this
    /// the disposition score sits near 0; above it the score climbs
    /// rapidly and beats Hunt/Forage at L3.
    #[serde(default = "default_disposal_inventory_excess_midpoint")]
    pub disposal_inventory_excess_midpoint: f32,
    pub gate_timid_fight_threshold: f32,
    pub gate_shy_socialize_threshold: f32,
    pub gate_reckless_flee_threshold: f32,
    pub gate_compulsive_helper_threshold: f32,
    pub gate_compulsive_explorer_threshold: f32,
    pub gate_compulsive_explorer_chance: f32,
    /// Bold cats only override Flee→Fight when HP ratio is above this threshold.
    #[serde(default = "default_gate_reckless_health_threshold")]
    pub gate_reckless_health_threshold: f32,
    // --- Reproduction scoring ---
    pub mate_temperature_scale: f32,
    pub caretake_compassion_scale: f32,
    pub caretake_parent_bonus: f32,
    /// Phase 4c.4 alloparenting Reframe A: per-unit-fondness boost to
    /// the compassion axis used by `CaretakeDse` when the adult is not
    /// a parent of the target kitten. With default 1.0 and fondness
    /// clamped to [0, 1] on the positive side, bond-scale maxes out at
    /// 2.0 — compassion axis is doubled for a cat that adores mama.
    /// Negative fondness doesn't suppress (scale floors at 1.0) because
    /// hostility toward mama shouldn't reduce baseline compassion for
    /// the kitten itself below colony norm.
    #[serde(default = "default_caretake_bond_compassion_boost_max")]
    pub caretake_bond_compassion_boost_max: f32,
    /// Minimum hunger a cat (and its prospective partner) must have to be
    /// eligible to mate. Hungry cats breed hungry kittens.
    ///
    /// Ticket 032 — colony-wide reproduction collapse traced partly to this
    /// floor: at `0.6`, the AND-gate of (hunger > 0.6 ∧ energy > 0.5 ∧
    /// mood > 0.2 ∧ partners-bond ∧ photoperiod) was rarely satisfied because
    /// the colony lives in survival mode. Landed `0.4` in ticket 272 — 032's
    /// drafted treatment (`docs/balance/hypotheses/032-3-breeding-floor.yaml`)
    /// validated against the post-257 substrate so the `MatingOccurred`
    /// canary can close. See `docs/balance/social-target-range.report.md`
    /// finding 2 — bond progression was the deeper bottleneck (fixed in 257),
    /// the hunger gate was the residual amplifier.
    pub breeding_hunger_floor: f32,
    /// Minimum energy a cat (and its prospective partner) must have to be
    /// eligible to mate. Exhausted cats don't court.
    pub breeding_energy_floor: f32,
    /// Minimum mood valence required to be eligible to mate. Miserable cats
    /// don't feel romantic.
    pub breeding_mood_floor: f32,
    /// Mating need must drop below this before a cat is interested enough to
    /// score the Mate action. Creates a seasonal ramp-up window.
    pub mating_interest_threshold: f32,
    /// Per-season fertility multiplier on mating-need decay and the
    /// has_eligible_mate gate. Models the photoperiodic breeding cycle of
    /// domestic cats — seasonally polyestrous with a Spring peak and Winter
    /// anestrous window. A value of 0 fully suppresses breeding in that
    /// season.
    #[serde(default = "default_mating_fertility_spring")]
    pub mating_fertility_spring: f32,
    #[serde(default = "default_mating_fertility_summer")]
    pub mating_fertility_summer: f32,
    #[serde(default = "default_mating_fertility_autumn")]
    pub mating_fertility_autumn: f32,
    #[serde(default = "default_mating_fertility_winter")]
    pub mating_fertility_winter: f32,
    // --- Corruption/carcass/siege scoring ---
    pub magic_harvest_carcass_scale: f32,
    pub magic_cleanse_colony_scale: f32,
    pub herbcraft_ward_siege_bonus: f32,
    pub corruption_social_bonus: f32,
    pub corruption_suppression_threshold: f32,
    pub corruption_suppression_scale: f32,
    pub carcass_detection_range: f32,
    /// Tile radius within which a cat "smells" corruption on nearby tiles.
    /// Corruption beyond this range is out of sensing reach.
    pub corruption_smell_range: f32,
    // --- 382 — Building placement (autonomous coordinator path) ---
    /// 382: selection rule for `compute_building_placement()`. Default
    /// `InfluenceMap` replaces the radius-16 spiral search from
    /// `find_building_placement` with an argmax over `ColonyDistrictMap`.
    /// `Spiral` is retained as an emergency revert / regression-bisect
    /// fixture.
    #[serde(default = "default_building_placement_semantics")]
    pub building_placement_semantics: BuildingPlacementSemantics,
    /// 382: minimum composite score required to place. Below this, the
    /// directive defers and the stuck-counter increments. Default `0.0`
    /// — any positive score wins.
    #[serde(default = "default_building_placement_score_floor")]
    pub building_placement_score_floor: f32,
    /// 382: deterministic per-candidate jitter range. Matches ward
    /// placement's `[0, 0.05)` convention.
    #[serde(default = "default_building_placement_jitter_range")]
    pub building_placement_jitter_range: f32,
    /// 382: per-Manhattan-tile soft penalty pulling placement toward
    /// the colony anchor. Matches ward placement's `0.005` coefficient
    /// (`compute_ward_placement` `DIST_PENALTY_PER_TILE`).
    #[serde(default = "default_building_placement_distance_cost_per_tile")]
    pub building_placement_distance_cost_per_tile: f32,
    /// 382: candidate-tile grid step in world tiles. Default `5` to
    /// align with the 5-tile influence-map bucket size.
    #[serde(default = "default_building_placement_candidate_step")]
    pub building_placement_candidate_step: i32,
    /// 382: weight of `ColonyDistrictMap::frontier_at` in the placement
    /// composite.
    #[serde(default = "default_building_placement_frontier_weight")]
    pub building_placement_frontier_weight: f32,
    /// 382: weight of `ColonyDistrictMap::crowding_at` (subtracted).
    #[serde(default = "default_building_placement_crowding_weight")]
    pub building_placement_crowding_weight: f32,
    /// 382: weight of `ColonyDistrictMap::threat_at` (subtracted for
    /// non-defensive kinds).
    #[serde(default = "default_building_placement_threat_weight")]
    pub building_placement_threat_weight: f32,
    /// 382: per-kind affinity lift for `Stores` / `Kitchen` / `Workshop`
    /// against `FoodLocationMap` (cooked-food infrastructure adjacency).
    #[serde(default = "default_building_placement_food_proximity_weight")]
    pub building_placement_food_proximity_weight: f32,
    /// 382: per-kind affinity lift for `Garden` against
    /// `GardenLocationMap` plus fertile-terrain class.
    #[serde(default = "default_building_placement_garden_terrain_weight")]
    pub building_placement_garden_terrain_weight: f32,
    /// 382: per-kind affinity lift for `Watchtower` / `WardPost` against
    /// `FoxApproachCorridorMap`. Combined with a sign flip on
    /// `building_placement_threat_weight` so defensive structures want
    /// the predator corridor.
    #[serde(default = "default_building_placement_defensive_corridor_weight")]
    pub building_placement_defensive_corridor_weight: f32,
    /// 382: per-kind affinity for `Midden`. Inverts frontier (refuse
    /// pile wants the periphery).
    #[serde(default = "default_building_placement_midden_periphery_weight")]
    pub building_placement_midden_periphery_weight: f32,
    /// 382: weight on the nearest-same-kind Manhattan-proximity term.
    /// Sign per kind: positive for `Stores` / `Kitchen` / `Workshop`
    /// (warehouse-district clustering); negative for `Den` (dispersion);
    /// zero for everything else.
    #[serde(default = "default_building_placement_same_kind_proximity_weight")]
    pub building_placement_same_kind_proximity_weight: f32,
    /// 382: Manhattan range over which `same_kind_proximity` lifts.
    /// Beyond this distance the term is exactly zero.
    #[serde(default = "default_building_placement_same_kind_proximity_range")]
    pub building_placement_same_kind_proximity_range: f32,
    /// 382: structure-halo radius used by `update_colony_district_map`
    /// when stamping the frontier axis around existing buildings.
    #[serde(default = "default_colony_district_structure_halo_radius")]
    pub colony_district_structure_halo_radius: f32,
    /// 382: building-crowding radius. Each existing structure stamps a
    /// disc of this radius into the crowding axis.
    #[serde(default = "default_colony_district_crowding_radius")]
    pub colony_district_crowding_radius: f32,
    /// 382: per-tick deposit ceiling on `frontier` per CatScent bucket
    /// — scales the cat-scent contribution so dense colonies don't peg
    /// the frontier axis to 1.0 everywhere.
    #[serde(default = "default_colony_district_cat_scent_scale")]
    pub colony_district_cat_scent_scale: f32,
    /// 382: ticks between threshold-triggered emissions of
    /// `Feature::DirectiveStuckOnPlacement` and the "looks for a spot
    /// for the new …" narration. Default `60` ≈ one in-game minute.
    #[serde(default = "default_placement_stuck_narrate_threshold_ticks")]
    pub placement_stuck_narrate_threshold_ticks: u32,
    /// 382: cadence in ticks between `update_colony_center`
    /// recomputations. Snap-to-centroid; cat populations move slowly
    /// enough that jitter at this cadence is negligible. Default
    /// `1000` ≈ one in-game season segment.
    #[serde(default = "default_colony_center_update_cadence_ticks")]
    pub colony_center_update_cadence_ticks: u64,
}

#[allow(deprecated)] // 228: patrol_fox_scent_weight retained for one-cycle header compat.
impl Default for ScoringConstants {
    fn default() -> Self {
        Self {
            jitter_range: 0.05,
            eat_urgency_scale: 2.0,
            sleep_urgency_scale: 1.2,
            sleep_dawn_bonus: default_sleep_dawn_bonus(),
            sleep_day_bonus: default_sleep_day_bonus(),
            sleep_dusk_bonus: default_sleep_dusk_bonus(),
            sleep_night_bonus: default_sleep_night_bonus(),
            sleep_health_deficit_midpoint: default_sleep_health_deficit_midpoint(),
            sleep_health_deficit_steepness: default_sleep_health_deficit_steepness(),
            fox_hunt_dawn_bonus: default_fox_hunt_dawn_bonus(),
            fox_hunt_day_bonus: default_fox_hunt_day_bonus(),
            fox_hunt_dusk_bonus: default_fox_hunt_dusk_bonus(),
            fox_hunt_night_bonus: default_fox_hunt_night_bonus(),
            fox_patrol_dawn_bonus: default_fox_patrol_dawn_bonus(),
            fox_patrol_day_bonus: default_fox_patrol_day_bonus(),
            fox_patrol_dusk_bonus: default_fox_patrol_dusk_bonus(),
            fox_patrol_night_bonus: default_fox_patrol_night_bonus(),
            fox_rest_dawn_bonus: default_fox_rest_dawn_bonus(),
            fox_rest_day_bonus: default_fox_rest_day_bonus(),
            fox_rest_dusk_bonus: default_fox_rest_dusk_bonus(),
            fox_rest_night_bonus: default_fox_rest_night_bonus(),
            cook_base_score: default_cook_base_score(),
            cook_diligence_scale: default_cook_diligence_scale(),
            cook_hunger_gate: default_cook_hunger_gate(),
            cook_food_scarcity_scale: default_cook_food_scarcity_scale(),
            hunt_food_scarcity_scale: 0.6,
            hunt_prey_bonus: 0.2,
            hunt_boldness_scale: 2.2,
            forage_food_scarcity_scale: 0.5,
            forage_diligence_scale: 2.0,
            socialize_sociability_scale: 2.0,
            socialize_temper_penalty_scale: 0.3,
            socialize_playfulness_bonus: 0.3,
            groom_temper_penalty_scale: 0.3,
            explore_curiosity_scale: 0.7,
            fox_scent_suppression_threshold: 0.3,
            fox_scent_suppression_scale: 0.8,
            stockpile_satiation_threshold: default_stockpile_satiation_threshold(),
            stockpile_satiation_scale: default_stockpile_satiation_scale(),
            work_pressure_affiliative_yield_threshold:
                default_work_pressure_affiliative_yield_threshold(),
            work_pressure_affiliative_yield_scale: default_work_pressure_affiliative_yield_scale(),
            max_additive_lift_per_dse: default_max_additive_lift_per_dse(),
            body_distress_promotion_threshold: default_body_distress_promotion_threshold(),
            body_distress_promotion_lift: default_body_distress_promotion_lift(),
            acute_health_adrenaline_threshold: default_acute_health_adrenaline_threshold(),
            acute_health_adrenaline_fight_lift: default_acute_health_adrenaline_fight_lift(),
            acute_health_adrenaline_fight_viability_threshold:
                default_acute_health_adrenaline_fight_viability_threshold(),
            acute_health_adrenaline_freeze_lift: default_acute_health_adrenaline_freeze_lift(),
            acute_health_adrenaline_freeze_viability_threshold:
                default_acute_health_adrenaline_freeze_viability_threshold(),
            hunger_urgency_threshold: default_hunger_urgency_threshold(),
            hunger_urgency_curve_exponent: default_hunger_urgency_curve_exponent(),
            hunger_urgency_eat_lift: default_hunger_urgency_eat_lift(),
            hunger_urgency_hunt_lift: default_hunger_urgency_hunt_lift(),
            hunger_urgency_forage_lift: default_hunger_urgency_forage_lift(),
            kitten_eat_boost_threshold: default_kitten_eat_boost_threshold(),
            kitten_eat_boost_multiplier: default_kitten_eat_boost_multiplier(),
            kitten_cry_caretake_lift_threshold: default_kitten_cry_caretake_lift_threshold(),
            kitten_cry_caretake_lift: default_kitten_cry_caretake_lift(),
            exhaustion_pressure_threshold: default_exhaustion_pressure_threshold(),
            exhaustion_pressure_sleep_lift: default_exhaustion_pressure_sleep_lift(),
            exhaustion_pressure_groom_lift: default_exhaustion_pressure_groom_lift(),
            thermal_distress_threshold: default_thermal_distress_threshold(),
            thermal_distress_sleep_lift: default_thermal_distress_sleep_lift(),
            threat_proximity_adrenaline_threshold: default_threat_proximity_adrenaline_threshold(),
            threat_proximity_adrenaline_flee_lift: default_threat_proximity_adrenaline_flee_lift(),
            threat_proximity_adrenaline_sleep_lift: default_threat_proximity_adrenaline_sleep_lift(
            ),
            threat_proximity_adrenaline_viability_threshold:
                default_threat_proximity_adrenaline_viability_threshold(),
            intraspecies_conflict_flight_threshold: default_intraspecies_conflict_flight_threshold(
            ),
            intraspecies_conflict_flight_lift: default_intraspecies_conflict_flight_lift(),
            intraspecies_conflict_freeze_threshold: default_intraspecies_conflict_freeze_threshold(
            ),
            intraspecies_conflict_freeze_hide_lift: default_intraspecies_conflict_freeze_hide_lift(
            ),
            social_perception_radius: default_social_perception_radius(),
            social_status_distress_respect_weight: default_social_status_distress_respect_weight(),
            social_status_distress_age_weight: default_social_status_distress_age_weight(),
            social_status_distress_bond_weight: default_social_status_distress_bond_weight(),
            social_status_distress_age_normalization_ticks:
                default_social_status_distress_age_normalization_ticks(),
            wander_curiosity_scale: 0.4,
            wander_base: 0.08,
            wander_playfulness_bonus: 0.2,
            flee_safety_threshold: 0.5,
            flee_safety_scale: 3.0,
            fight_min_allies: 0,
            fight_ally_bonus_per_cat: 0.15,
            fight_boldness_scale: 1.5,
            fight_health_suppression_threshold: 0.5,
            fight_safety_suppression_threshold: 0.3,
            patrol_safety_threshold: 0.8,
            patrol_exit_threshold: default_patrol_exit_threshold(),
            patrol_boldness_scale: 1.5,
            build_diligence_scale: 1.5,
            build_repair_bonus: 0.35,
            farm_diligence_scale: 1.2,
            herbcraft_gather_spirituality_scale: 0.5,
            herbcraft_gather_skill_offset: 0.1,
            herbcraft_prepare_skill_offset: 0.1,
            herbcraft_prepare_injury_scale: 0.3,
            herbcraft_prepare_injury_cap: 1.5,
            herbcraft_ward_skill_offset: 0.1,
            herbcraft_ward_scale: 0.6,
            magic_durable_ward_scale: 0.8,
            magic_cleanse_corruption_threshold: 0.1,
            magic_commune_scale: 0.7,
            coordinate_diligence_scale: 0.8,
            coordinate_directive_scale: 0.3,
            coordinate_ambition_bonus: 0.2,
            mentor_temperature_diligence_scale: 0.5,
            mentor_ambition_bonus: 0.1,
            idle_base: 0.05,
            idle_incuriosity_scale: 0.08,
            idle_playfulness_penalty: 0.05,
            idle_minimum_floor: 0.01,
            pride_respect_threshold: 0.5,
            pride_bonus: 0.1,
            independence_solo_bonus: 0.1,
            independence_group_penalty: 0.1,
            patience_commitment_bonus: 0.15,
            memory_nearby_radius: 15.0,
            memory_resource_bonus: 0.2,
            memory_death_penalty: 0.1,
            memory_threat_penalty: 0.15,
            cascading_bonus_per_cat: 0.08,
            colony_knowledge_radius: 20.0,
            colony_knowledge_bonus_scale: 0.15,
            priority_bonus: 0.15,
            aspiration_bonus: 0.2,
            preference_like_bonus: 0.08,
            preference_dislike_penalty: 0.08,
            fated_love_social_bonus: 0.15,
            fated_rival_competition_bonus: 0.1,
            action_softmax_temperature: 0.15,
            disposition_softmax_temperature: 0.15,
            fox_softmax_temperature: default_fox_softmax_temperature(),
            softmax_temperature_floor: default_softmax_temperature_floor(),
            softmax_temperature_ceiling: default_softmax_temperature_ceiling(),
            carry_affinity_bonus: default_carry_affinity_bonus(),
            chronicity_window_ticks: default_chronicity_window_ticks(),
            chronicity_threshold: default_chronicity_threshold(),
            stores_herb_capacity_per_kind: default_stores_herb_capacity_per_kind(),
            thornbriar_stash_low_threshold: default_thornbriar_stash_low_threshold(),
            farm_herb_pressure_weight: default_farm_herb_pressure_weight(),
            build_chronic_full_weight: default_build_chronic_full_weight(),
            hunt_food_security_weight: default_hunt_food_security_weight(),
            forage_food_security_weight: default_forage_food_security_weight(),
            mentor_food_security_weight: default_mentor_food_security_weight(),
            coordinate_food_security_weight: default_coordinate_food_security_weight(),
            caretake_food_security_weight: default_caretake_food_security_weight(),
            rear_kitten_caretake_lift: default_rear_kitten_caretake_lift(),
            groom_food_security_weight: default_groom_food_security_weight(),
            patrol_fox_scent_weight: default_patrol_fox_scent_weight(),
            patrol_route_cost_weight: default_patrol_route_cost_weight(),
            patrol_path_fox_scent_weight: default_patrol_path_fox_scent_weight(),
            patrol_path_corruption_weight: default_patrol_path_corruption_weight(),
            flee_affordance_weight: default_flee_affordance_weight(),
            hide_affordance_freeze_weight: default_hide_affordance_freeze_weight(),
            hide_recency_of_threat_cue_weight: default_hide_recency_of_threat_cue_weight(),
            hide_perceived_intent_clarity_weight: default_hide_perceived_intent_clarity_weight(),
            patrol_threat_recency_weight: default_patrol_threat_recency_weight(),
            hunt_best_predation_weight: default_hunt_best_predation_weight(),
            hunt_alertness_tolerance_weight: default_hunt_alertness_tolerance_weight(),
            socialize_affiliation_weight: default_socialize_affiliation_weight(),
            socialize_affordance_weight: default_socialize_affordance_weight(),
            groom_other_affiliation_weight: default_groom_other_affiliation_weight(),
            groom_other_hostility_weight: default_groom_other_hostility_weight(),
            groom_other_affordance_weight: default_groom_other_affordance_weight(),
            mate_receptivity_weight: default_mate_receptivity_weight(),
            mate_affordance_weight: default_mate_affordance_weight(),
            mentor_affordance_weight: default_mentor_affordance_weight(),
            caretake_affordance_weight: default_caretake_affordance_weight(),
            apply_remedy_injury_belief_weight: default_apply_remedy_injury_belief_weight(),
            apply_remedy_affordance_weight: default_apply_remedy_affordance_weight(),
            fox_hunting_prey_affordance_weight: default_fox_hunting_prey_affordance_weight(),
            hawk_hunting_prey_affordance_weight: default_hawk_hunting_prey_affordance_weight(),
            snake_ambush_strike_affordance_weight: default_snake_ambush_strike_affordance_weight(),
            snake_forage_stalk_affordance_weight: default_snake_forage_stalk_affordance_weight(),
            fox_flee_cat_violence_belief_weight: default_fox_flee_cat_violence_belief_weight(),
            fox_flee_belief_eligibility_threshold: default_fox_flee_belief_eligibility_threshold(),
            hawk_flee_cat_violence_belief_weight: default_hawk_flee_cat_violence_belief_weight(),
            hawk_flee_belief_eligibility_threshold: default_hawk_flee_belief_eligibility_threshold(
            ),
            snake_flee_cat_violence_belief_weight: default_snake_flee_cat_violence_belief_weight(),
            hunt_stalk_chase_affordance_bias: default_hunt_stalk_chase_affordance_bias(),
            ward_ambush_anchor_weight: default_ward_ambush_anchor_weight(),
            ward_recency_anchor_weight: default_ward_recency_anchor_weight(),
            ward_placement_logistic_steepness: default_ward_placement_logistic_steepness(),
            ward_placement_logistic_midpoint: default_ward_placement_logistic_midpoint(),
            ward_placement_candidate_step: default_ward_placement_candidate_step(),
            ward_fox_intercept_anchor_weight: default_ward_fox_intercept_anchor_weight(),
            fox_intercept_kernel_radius_tiles: default_fox_intercept_kernel_radius_tiles(),
            ward_placement_cat_value_weight: default_ward_placement_cat_value_weight(),
            ward_placement_semantics: default_ward_placement_semantics(),
            ward_placement_residual_rounds: default_ward_placement_residual_rounds(),
            ward_intent_dse_weight: default_ward_intent_dse_weight(),
            ward_intent_decay_per_wake: default_ward_intent_decay_per_wake(),
            ward_fox_approach_corridor_weight: default_ward_fox_approach_corridor_weight(),
            ward_placement_cat_value_composition: default_ward_placement_cat_value_composition(),
            ward_placement_cat_value_gate_floor: default_ward_placement_cat_value_gate_floor(),
            cat_patrol_deterrent_deposit_per_tick: default_cat_patrol_deterrent_deposit_per_tick(),
            cat_patrol_deterrent_decay_rate: default_cat_patrol_deterrent_decay_rate(),
            cat_patrol_deterrent_path_cost_max: default_cat_patrol_deterrent_path_cost_max(),
            cat_patrol_deterrent_overlay_weight: default_cat_patrol_deterrent_overlay_weight(),
            forage_route_cost_weight: default_forage_route_cost_weight(),
            hunt_route_cost_weight: default_hunt_route_cost_weight(),
            wander_route_cost_weight: default_wander_route_cost_weight(),
            explore_route_cost_weight: default_explore_route_cost_weight(),
            fox_scent_path_cost_max: default_fox_scent_path_cost_max(),
            threat_belief_path_cost_max: default_threat_belief_path_cost_max(),
            corruption_path_cost_max: default_corruption_path_cost_max(),
            route_cost_flood_budget: default_route_cost_flood_budget(),
            wander_recandidate_ticks: default_wander_recandidate_ticks(),
            route_cost_replan_window_ticks: default_route_cost_replan_window_ticks(),
            tension_defusion_groom_weight: default_tension_defusion_groom_weight(),
            disposal_inventory_excess_slope: default_disposal_inventory_excess_slope(),
            disposal_inventory_excess_midpoint: default_disposal_inventory_excess_midpoint(),
            gate_timid_fight_threshold: 0.1,
            gate_shy_socialize_threshold: 0.15,
            gate_reckless_flee_threshold: 0.9,
            gate_compulsive_helper_threshold: 0.6,
            gate_compulsive_explorer_threshold: 0.9,
            gate_compulsive_explorer_chance: 0.20,
            gate_reckless_health_threshold: 0.5,
            mate_temperature_scale: 5.0,
            caretake_compassion_scale: 1.8,
            caretake_parent_bonus: 0.5,
            caretake_bond_compassion_boost_max: default_caretake_bond_compassion_boost_max(),
            breeding_hunger_floor: 0.4,
            breeding_energy_floor: 0.5,
            breeding_mood_floor: 0.2,
            mating_interest_threshold: 0.6,
            mating_fertility_spring: default_mating_fertility_spring(),
            mating_fertility_summer: default_mating_fertility_summer(),
            mating_fertility_autumn: default_mating_fertility_autumn(),
            mating_fertility_winter: default_mating_fertility_winter(),
            building_placement_semantics: default_building_placement_semantics(),
            building_placement_score_floor: default_building_placement_score_floor(),
            building_placement_jitter_range: default_building_placement_jitter_range(),
            building_placement_distance_cost_per_tile:
                default_building_placement_distance_cost_per_tile(),
            building_placement_candidate_step: default_building_placement_candidate_step(),
            building_placement_frontier_weight: default_building_placement_frontier_weight(),
            building_placement_crowding_weight: default_building_placement_crowding_weight(),
            building_placement_threat_weight: default_building_placement_threat_weight(),
            building_placement_food_proximity_weight:
                default_building_placement_food_proximity_weight(),
            building_placement_garden_terrain_weight:
                default_building_placement_garden_terrain_weight(),
            building_placement_defensive_corridor_weight:
                default_building_placement_defensive_corridor_weight(),
            building_placement_midden_periphery_weight:
                default_building_placement_midden_periphery_weight(),
            building_placement_same_kind_proximity_weight:
                default_building_placement_same_kind_proximity_weight(),
            building_placement_same_kind_proximity_range:
                default_building_placement_same_kind_proximity_range(),
            colony_district_structure_halo_radius: default_colony_district_structure_halo_radius(),
            colony_district_crowding_radius: default_colony_district_crowding_radius(),
            colony_district_cat_scent_scale: default_colony_district_cat_scent_scale(),
            placement_stuck_narrate_threshold_ticks:
                default_placement_stuck_narrate_threshold_ticks(),
            colony_center_update_cadence_ticks: default_colony_center_update_cadence_ticks(),
            magic_harvest_carcass_scale: 0.6,
            magic_cleanse_colony_scale: 0.4,
            herbcraft_ward_siege_bonus: 0.4,
            corruption_social_bonus: 0.15,
            corruption_suppression_threshold: 0.3,
            corruption_suppression_scale: 0.6,
            carcass_detection_range: 15.0,
            corruption_smell_range: 5.0,
        }
    }
}

// ---------- DispositionConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DispositionConstants {
    pub starvation_interrupt_threshold: f32,
    pub exhaustion_interrupt_threshold: f32,
    /// Critical hunger threshold that interrupts even Hunting/Foraging.
    /// Lower than `starvation_interrupt_threshold` — only fires when the cat
    /// is on the verge of starvation death, not merely hungry.
    pub critical_hunger_interrupt_threshold: f32,
    pub threat_awareness_range: f32,
    pub threat_urgency_divisor: f32,
    pub flee_threshold_base: f32,
    pub flee_threshold_boldness_scale: f32,
    pub critical_safety_threshold: f32,
    /// Safety-recovery band above `critical_safety_threshold` that marks a
    /// Guarding plan's `achievement_believed` as true. When a cat enters
    /// Guarding (triggered by `safety < critical_safety_threshold`) and its
    /// safety has climbed past `critical_safety_threshold + guarding_exit_epsilon`
    /// after at least one patrol trip, the §7.2 commitment gate drops the
    /// plan so the cat re-evaluates. Breaks the Patrol-loop pattern where
    /// same-tier preempts can't abandon Guarding; see
    /// `docs/balance/guarding-exit-recipe.md` (to be written) and the
    /// Thistle seed-69 soak diagnosis.
    #[serde(default = "default_guarding_exit_epsilon")]
    pub guarding_exit_epsilon: f32,
    pub flee_distance: f32,
    pub flee_ticks: u64,
    /// Ticket 230. How long the `HoldUntilSafe` step waits on a low-
    /// route-cost tile (with non-elevated threat-derivative) before
    /// committing the trip increment that closes the Fleeing
    /// disposition. Set well above the post-228 thrash cadence
    /// (1.21 ticks/plan) so the plan accumulates real commitment;
    /// well below typical healthy plan-replan cadence so it doesn't
    /// strand cats in safe corners. Read by
    /// `src/steps/disposition/hold_until_safe.rs::resolve_hold_until_safe`.
    pub flee_hold_ticks: u64,
    /// Ticket 230. The `RouteCostField::cost_at(pos)` value below which
    /// a tile is considered "safe enough" for `HoldUntilSafe` to count
    /// hold ticks. Roughly 1/6 of `MAX_COST_BUDGET` (600); a flooded
    /// tile at this cost is reachable via a low-overlay corridor (no
    /// strong fox-scent or corruption pressure on the way). Read by
    /// `resolve_hold_until_safe`.
    pub route_cost_safe_threshold: u32,
    /// Ticket 230. The `Needs.safety` value at or above which
    /// `HoldUntilSafe` counts ticks. Composes with
    /// `route_cost_safe_threshold`: a cat is "safe enough to hold"
    /// only when both spatial cost is low AND the perception-side
    /// safety scalar has caught up. Read by `resolve_hold_until_safe`.
    pub flee_safety_need_threshold: f32,
    pub damaged_building_threshold: f32,
    pub ward_strength_low_threshold: f32,
    pub hunt_terrain_search_radius: f32,
    pub forage_terrain_search_radius: f32,
    pub social_target_range: f32,
    pub wildlife_threat_range: f32,
    /// Proximity radius for counting allies fighting the same threat.
    pub allies_fighting_range: f32,
    pub allies_fighting_cap: usize,
    /// Minimum HP ratio for a guarding cat to enter a FightThreat chain.
    pub guard_fight_health_min: f32,
    pub combat_effective_hunting_cross_train: f32,
    pub herb_detection_range: f32,
    pub prey_detection_range: f32,
    pub corrupted_tile_threshold: f32,
    pub mentor_skill_threshold_high: f32,
    pub mentor_skill_threshold_low: f32,
    pub mentoring_detection_range: f32,
    pub directive_bonus_base_weight: f32,
    pub directive_independence_penalty: f32,
    pub directive_stubbornness_penalty: f32,
    pub fondness_default: f32,
    pub fondness_social_weight: f32,
    pub novelty_social_weight: f32,
    pub disposition_independence_penalty: f32,
    pub fated_love_detection_range: f32,
    pub fated_rival_detection_range: f32,
    pub cascading_bonus_range: f32,
    pub resting_complete_hunger: f32,
    pub resting_complete_energy: f32,
    pub resting_complete_temperature: f32,
    /// Planner gate thresholds — needs below these are considered unsatisfied
    /// and trigger the corresponding recovery action (EatAtStores, Sleep,
    /// SelfGroom) in the Resting plan.
    pub planner_hunger_ok_threshold: f32,
    pub planner_energy_ok_threshold: f32,
    pub planner_temperature_ok_threshold: f32,
    pub resting_max_replans: u32,
    pub sleep_duration_deficit_multiplier: f32,
    pub sleep_duration_base: u64,
    pub guard_threat_detection_range: f32,
    pub guard_patrol_radius: f32,
    /// §L2.10.7 perimeter offset (tiles) from colony center used as the
    /// fallback anchor for cat Patrol and HerbcraftWard spatial axes
    /// when `WardCoverageMap` has no coverage (early-game, pre-ward).
    /// 12 ≈ inner colony walk. Post-256 the live patrol anchor is the
    /// per-replan ward-sector centroid; this offset only fires on the
    /// fallback path.
    pub patrol_perimeter_offset: i32,
    /// 256 R3: width of the sector grid overlaid on the
    /// `WardCoverageMap` bucket grid for Patrol's per-replan anchor
    /// rotation. With the standard 120×90 map and 5-tile buckets
    /// (24×18 buckets), `4 × 3` sectors give 12 sectors of 30×30
    /// tiles each. Larger sector grids → smaller sectors → tighter
    /// patrol arcs but more rotation churn.
    #[serde(default = "default_patrol_sector_grid_w")]
    pub patrol_sector_grid_w: usize,
    /// 256 R3: height of the sector grid overlaid on
    /// `WardCoverageMap`. See `patrol_sector_grid_w`.
    #[serde(default = "default_patrol_sector_grid_h")]
    pub patrol_sector_grid_h: usize,
    /// 256 R3: how many ticks each cat spends on a single patrol
    /// sector before rotating to the next. The sector index advances
    /// as `(tick / rotation_ticks + per_cat_offset) % num_sectors`.
    /// Default `1000` ≈ ~1 sim hour per sector at the canonical
    /// 1000-tick/hour cadence. The per-cat offset prevents synchronous
    /// beat clustering across the colony.
    #[serde(default = "default_patrol_sector_rotation_ticks")]
    pub patrol_sector_rotation_ticks: u64,
    pub social_chain_target_range: f32,
    pub mentor_temperature_threshold: f32,
    pub groom_temperature_threshold: f32,
    pub building_search_range: f32,
    pub crafting_herb_detection_range: f32,
    pub crafting_herbcraft_skill_threshold: f32,
    pub coordinating_target_range: f32,
    pub coordinating_distance_penalty: f32,
    pub explore_range: f32,
    pub scent_downwind_dot_threshold: f32,
    pub scent_dense_forest_modifier: f32,
    pub scent_light_forest_modifier: f32,
    pub scent_base_range: f32,
    pub scent_min_range: f32,
    /// Phase 2B — manhattan radius around a cat's position that the
    /// `PreyScentMap.highest_nearby` search covers when detecting
    /// prey scent. Replaces the point-to-point `scent_base_range` /
    /// downwind-dot / forest-modifier formula; those constants stay
    /// for any residual reader but are not consulted by the new
    /// grid-sampled detection path.
    #[serde(default = "default_scent_search_radius")]
    pub scent_search_radius: f32,
    /// Phase 2B — minimum `PreyScentMap` value at the strongest
    /// nearby bucket for a cat to register "prey is scent-detectable
    /// here." Below this, the hunt-search step returns without
    /// committing to a prey target.
    #[serde(default = "default_scent_detect_threshold")]
    pub scent_detect_threshold: f32,
    pub den_discovery_range: f32,
    pub den_discovery_base_chance: f32,
    pub den_discovery_skill_scale: f32,
    pub den_raid_kill_fraction: f32,
    pub den_dropped_item_quality: f32,
    pub respect_gain_hunting: f32,
    pub respect_gain_foraging: f32,
    pub respect_gain_guarding: f32,
    pub respect_gain_building: f32,
    pub respect_gain_coordinating: f32,
    pub respect_gain_socializing: f32,
    pub pounce_range_patient: f32,
    pub pounce_range_impatient: f32,
    pub pounce_range_default: f32,
    pub pounce_awareness_idle: f32,
    pub pounce_awareness_alert: f32,
    pub pounce_awareness_fleeing: f32,
    pub pounce_distance_close_mod: f32,
    pub pounce_distance_mid_mod: f32,
    pub pounce_distance_far_mod: f32,
    pub pounce_density_threshold: f32,
    pub pounce_skill_base: f32,
    pub pounce_skill_scale: f32,
    pub hunt_catch_skill_growth: f32,
    pub stalk_start_buffer: f32,
    pub stalk_start_minimum: f32,
    /// Ticket 100 — additive lift applied to `effective_stalk_distance`
    /// per unit of `PreyState.alertness`. A nervous rabbit
    /// (`alertness ≈ 1.0`) pushes a typical patient stalker out by
    /// `alertness_push` extra tiles before the stalk transition fires.
    /// Default `1.5` (ticket 464 halved the ticket-100 default of `3.0`:
    /// the original lift saturated the `[5, 9]` clamp ceiling for
    /// high-tremor prey, dropping colony hunt success 19.7% → 13.3%).
    #[serde(default = "default_alertness_push")]
    pub alertness_push: f32,
    /// Ticket 100 — additive lift applied to `effective_stalk_distance`
    /// per unit of normalized prey tremor sensitivity
    /// (`prey_tremor_sensitivity` returns `base_range / 12.0`; Rabbit at
    /// max emits 1.0, Bird at 2/12 ≈ 0.17). Default `1.0` (ticket 464
    /// halved the ticket-100 default of `2.0`: high-tremor prey were
    /// stalked from too far out, quadrupling `lost_during_stalk`).
    #[serde(default = "default_species_push")]
    pub species_push: f32,
    /// Ticket 100 — multiplier on the patient cat's reading of the
    /// `TremorMap` at the prey's tile. Effective contribution is
    /// `patience × tremor_push × tremor_map.get(prey_pos)`. Bold cats
    /// (`patience ≈ 0`) ignore the ambient read by construction.
    /// Default `4.0` — a 0.5 reading at full patience adds 2 tiles of
    /// caution; a 1.0 reading at full patience adds 4.
    #[serde(default = "default_tremor_push")]
    pub tremor_push: f32,
    /// Ticket 100 — multiplier on the patient cat's reading of the
    /// per-species prey scent at the prey's tile (settled prey accrue
    /// scent at their loiter spot). Effective contribution is
    /// `patience × scent_settle_push × scent.get(prey_pos)` and is
    /// *subtracted* from `effective_stalk_distance` — settled prey
    /// invite the patient cat to close. Default `3.0`.
    #[serde(default = "default_scent_settle_push")]
    pub scent_settle_push: f32,
    pub anxiety_spook_threshold: f32,
    pub anxiety_spook_chance: f32,
    pub chase_limit_bold: u64,
    pub chase_limit_default: u64,
    pub chase_stuck_ticks: u64,
    pub chase_speed: i32,
    pub approach_speed: i32,
    pub approach_give_up_distance: f32,
    pub search_belief_radius: f32,
    pub search_wind_direction_threshold: f32,
    pub search_jitter_chance: f32,
    pub search_speed: i32,
    pub search_visual_detection_range: f32,
    pub search_timeout_ticks: u64,
    pub travel_timeout_ticks: u64,
    pub travel_no_path_stuck_ticks: u64,
    pub global_step_timeout_ticks: u64,
    pub forage_jitter_chance: f32,
    pub forage_yield_scale: f32,
    pub forage_skill_growth: f32,
    pub forage_timeout_ticks: u64,
    /// Ticket 368 — chance of also dropping a Phase 2 crafting input
    /// (Twig on forest tiles; Fiber or Flower on Grass) alongside the
    /// foraged food, as a separate `OnGround` `Item` entity. Preserves
    /// food throughput on successful forages while adding ingredient
    /// supply for the Grooming Brush / Play Bundle / Courtship Gift
    /// recipes. `0.10` is the first-light value (tuned down from an
    /// initial 0.30 after seed-42 wall-clock verification showed the
    /// MatingOccurred canary never-fired due to ~28% fewer ticks per
    /// second from the extra entity churn). Tune via sweep once the
    /// Workshop-craft pipeline (ticket 457) lands and recipes actually
    /// consume the ingredients.
    pub forage_ingredient_drop_chance: f32,
    pub deposit_quality_base: f32,
    pub deposit_quality_skill_scale: f32,
    pub eat_at_stores_duration: u64,
    /// Scales food_value reduction from tile corruption when eating at stores.
    pub corruption_food_penalty: f32,
    /// Ticket 150 R1 — hunger threshold below which a hunting/foraging
    /// cat consumes the catch in-place rather than depositing it. Uses
    /// the canonical `Needs.hunger` semantic (`1.0` = full belly, `0.0`
    /// = starving), so `hunger < production_self_eat_threshold` means
    /// "cat is at least this hungry." Default 0.5 matches the hangry
    /// curve midpoint: a cat below half-full eats the catch where they
    /// stand instead of walking it home, then re-plans (the original
    /// production plan dies on the consume-on-spot fail). A cat above
    /// 0.5 deposits as today.
    #[serde(default = "default_production_self_eat_threshold")]
    pub production_self_eat_threshold: f32,
    pub sleep_energy_per_tick: f32,
    pub sleep_temperature_per_tick: f32,
    pub self_groom_duration: u64,
    pub self_groom_temperature_gain: f32,
    pub socialize_social_per_tick: f32,
    pub socialize_fondness_per_tick: f32,
    pub socialize_familiarity_per_tick: f32,
    pub socialize_duration: u64,
    pub groom_other_social_per_tick: f32,
    pub groom_other_fondness_per_tick: f32,
    pub groom_other_familiarity_per_tick: f32,
    pub groom_other_duration: u64,
    pub groom_other_temperature_gain: f32,
    /// 035: bury action duration in ticks. Brief — burial is a
    /// witnessed completion, not a sustained interaction.
    #[serde(default = "default_bury_ticks")]
    pub bury_ticks: u64,
    /// 035: Manhattan radius within which a cat senses an unburied
    /// colony-mate corpse and authors `HasUnburiedCorpse`. Smaller
    /// than the social-family ranges (10) so cats must encounter the
    /// corpse to react.
    #[serde(default = "default_burial_sense_range")]
    pub burial_sense_range: f32,
    /// 035: Belonging-tier fulfillment lift on burial completion.
    /// Mirrors the small social_warmth gain from grooming.
    #[serde(default = "default_bury_belonging_gain")]
    pub bury_belonging_gain: f32,
    /// Recipient-side acceptance bump when a cat is groomed to completion.
    /// Fires once per `groom_other_duration` session, on the same witness
    /// that applies the grooming restoration. Models the felt sense of
    /// being welcomed by the colony.
    pub acceptance_per_groomed: f32,
    /// Kitten-side acceptance bump when a kitten is successfully fed
    /// (witnessed `FeedKitten` — adult inventory took a food item).
    pub acceptance_per_kitten_fed: f32,
    /// Iteration 2 of `docs/balance/acceptance-restoration.md` — per-tick
    /// recipient bump on the GroomOther recipient. Mirror of the existing
    /// `groom_other_social_per_tick` which restores the *groomer's* social.
    /// Iter-1 mechanism correction: completion-witness was dormant because
    /// 80-tick groom sessions get preempted; per-tick fires on actual
    /// engagement, not just completion.
    #[serde(default = "default_acceptance_per_groom_other_per_tick")]
    pub acceptance_per_groom_other_per_tick: f32,
    /// Iteration 2 — per-tick recipient bump on the FedKitten recipient
    /// (the kitten). Same iter-1 mechanism-correction rationale.
    #[serde(default = "default_acceptance_per_feed_kitten_per_tick")]
    pub acceptance_per_feed_kitten_per_tick: f32,
    /// Iteration 2 — per-tick apprentice bump on Mentor sessions.
    /// Receiver-side acceptance pathway for the apprentice — paired with
    /// the existing `mentor_mastery_per_tick` (mentor-side felt-competence).
    #[serde(default = "default_acceptance_per_mentor_per_tick")]
    pub acceptance_per_mentor_per_tick: f32,
    /// Iteration 2 — per-tick recipient bump on Cleanse sessions
    /// (target cat being cleansed of corruption).
    #[serde(default = "default_acceptance_per_cleanse_per_tick")]
    pub acceptance_per_cleanse_per_tick: f32,
    /// New balance thread `docs/balance/respect-restoration.md` — per-witness
    /// respect multiplier applied at chain completion on top of the
    /// existing `respect_for_disposition` baseline. Models social visibility
    /// of accomplishment.
    #[serde(default = "default_respect_per_witness")]
    pub respect_per_witness: f32,
    /// Manhattan radius for counting witnesses to a chain completion.
    #[serde(default = "default_respect_witness_radius")]
    pub respect_witness_radius: f32,
    /// Diminishing-returns cap on witness count.
    #[serde(default = "default_respect_witness_cap")]
    pub respect_witness_cap: u32,
    /// New balance thread `docs/balance/purpose-restoration.md` — generic
    /// per-action colony-positive bump baseline. Used for actions whose
    /// colony-contribution pulse doesn't have a dedicated knob below.
    #[serde(default = "default_purpose_per_colony_action")]
    pub purpose_per_colony_action: f32,
    /// Per-event purpose bump on a successful deposit-to-stores
    /// (tangible asset added to colony pool).
    #[serde(default = "default_purpose_per_deposit")]
    pub purpose_per_deposit: f32,
    /// Per-event purpose bump on a successful ward placement
    /// (significant defensive contribution).
    #[serde(default = "default_purpose_per_ward_set")]
    pub purpose_per_ward_set: f32,
    /// Per-event purpose bump on completing a coordinator directive.
    #[serde(default = "default_purpose_per_directive_completed")]
    pub purpose_per_directive_completed: f32,
    /// Per-tick purpose bump while building (high-cadence; small).
    #[serde(default = "default_purpose_per_build_tick")]
    pub purpose_per_build_tick: f32,
    /// Per-event mastery bump on successful magic outcomes (set ward,
    /// cleanse corruption, scry, harvest carcass). STUB — placeholder
    /// for ticket 016 Phase 5's per-skill crafting/experience table
    /// which will replace this with per-action × per-quality resolution.
    #[serde(default = "default_mastery_per_magic_success")]
    pub mastery_per_magic_success: f32,
    /// Per-event mastery bump on successful TendCrops completion.
    /// Same STUB caveat as `mastery_per_magic_success`.
    #[serde(default = "default_mastery_per_successful_tend")]
    pub mastery_per_successful_tend: f32,
    /// Per-tick mastery bump while constructing/building.
    /// Same STUB caveat.
    #[serde(default = "default_mastery_per_build_tick")]
    pub mastery_per_build_tick: f32,
    /// Per-event mastery bump on successful Cook (raw → cooked flip).
    /// Same STUB caveat.
    #[serde(default = "default_mastery_per_successful_cook")]
    pub mastery_per_successful_cook: f32,
    /// Per-event mastery bump on successful Hunt kill (HuntPrey
    /// dispatch arm — inline in goap.rs, not a standalone resolver).
    /// Same STUB caveat.
    #[serde(default = "default_mastery_per_successful_hunt")]
    pub mastery_per_successful_hunt: f32,
    pub mentor_mastery_per_tick: f32,
    pub mentor_social_per_tick: f32,
    pub mentor_respect_per_tick: f32,
    pub mentor_fondness_per_tick: f32,
    pub mentor_familiarity_per_tick: f32,
    pub mentor_duration: u64,
    pub apprentice_skill_growth_multiplier: f32,
    pub patrol_arrival_safety_gain: f32,
    pub patrol_per_tile_safety_gain: f32,
    pub patrol_stuck_timeout: u64,
    pub fight_duration: u64,
    pub fight_combat_skill_growth: f32,
    pub fight_safety_gain: f32,
    /// Actor mastery bump on completed fight engagement (ticks ≥
    /// fight_duration, morale not broken). Models felt-competence from
    /// holding one's ground — parallels the `acceptance_per_groomed`
    /// pathway for needs that would otherwise be one-way drains.
    pub fight_mastery_gain: f32,
    pub survey_duration: u64,
    pub survey_purpose_gain: f32,
    /// Actor mastery bump on completed survey step. Fires once per
    /// `survey_duration` completion, independent of the discovery value
    /// (the skill is "I went and looked", not "I found something").
    pub survey_mastery_gain: f32,
    pub survey_colony_discovery_scale: f32,
    pub survey_personal_discovery_scale: f32,
    /// Radius around the surveying cat that gets marked explored per
    /// survey step. Cats can see around themselves — a single-tile stamp
    /// doesn't model that. Default 4 (9×9 = 81 tiles).
    #[serde(default = "default_survey_explore_radius")]
    pub survey_explore_radius: f32,
    pub exploration_decay_rate: f32,
    /// Radius around each living cat that gets marked explored every tick,
    /// modelling passive awareness — cats notice their surroundings as they
    /// move through the world.  Smaller than `survey_explore_radius` (active
    /// perception).  Default 2 (5×5 = 25 tiles).
    #[serde(default = "default_passive_explore_radius")]
    pub passive_explore_radius: f32,
    /// Radius used by `unexplored_fraction_nearby` to determine how
    /// familiar a cat's local area feels.  Decoupled from `explore_range`
    /// (action distance) — a cat's sense of "I know this place" should
    /// cover a smaller area than "how far I could walk to explore."
    /// Default 10 (21×21 = 441 tiles).
    #[serde(default = "default_explore_perception_radius")]
    pub explore_perception_radius: f32,
    /// `still_goal` threshold for the §7.2 commitment gate on Exploring
    /// plans.  When `unexplored_fraction_nearby` drops below this value,
    /// the cat's desire to explore fades and an OpenMinded plan may be
    /// dropped.  Set below the Logistic saturation midpoint (0.3) so
    /// Explore plans survive in moderately-explored territory but drop
    /// once the area is thoroughly familiar.  Default 0.15.
    #[serde(default = "default_explore_satiation_threshold")]
    pub explore_satiation_threshold: f32,
    /// `still_goal` threshold for the §7.2 commitment gate on Socializing
    /// plans.  When `needs.social` climbs above this value the cat considers
    /// itself socially sated and an OpenMinded plan may be dropped.
    ///
    /// Phase 6a wired this as `resting_complete_temperature` (0.3) to avoid
    /// a new knob mid-refactor.  Seed-42 soaks show social need never
    /// drops below 0.54 (passive proximity restoration), so the 0.3
    /// threshold prevented any Socializing plan from persisting past a
    /// single commitment check — every plan was immediately dropped as
    /// "goal drifted."  Raising to 0.85 lets plans hold until the cat is
    /// genuinely sated, allowing TravelTo + SocializeWith to complete.
    #[serde(default = "default_social_satiation_threshold")]
    pub social_satiation_threshold: f32,
    pub explore_den_discovery_chance: f32,
    pub deliver_directive_duration: u64,
    pub deliver_directive_respect_gain: f32,
    pub deliver_directive_social_gain: f32,
    pub idle_fallback_duration: u64,
    #[serde(default = "default_true")]
    pub anti_stack_jitter: bool,
    /// Below this HP ratio, any cat breaks out of its disposition to re-evaluate.
    #[serde(default = "default_critical_health_threshold")]
    pub critical_health_threshold: f32,
    /// Below this HP ratio, FightThreat step fails the chain (morale break).
    #[serde(default = "default_fight_bail_health_threshold")]
    pub fight_bail_health_threshold: f32,
    /// Ticket 087 — interoceptive perception. Composite gate for the
    /// `BodyDistressed` ZST marker: fires when any of the cat's body-state
    /// urgencies (hunger / energy / thermal / health deficit) exceed this
    /// value. The unified "I am unwell" signal that DSEs and (later) the
    /// 088 distress-promotion Modifier consume. Default 0.6 — onset of
    /// real discomfort, well before any single axis hits a panic threshold.
    #[serde(default = "default_body_distress_threshold")]
    pub body_distress_threshold: f32,
    /// Ticket 087 — divisor used by interoceptive perception to normalize
    /// the `pain_level` scalar. `pain_level` is the sum of severity scores
    /// (Minor=0.1 / Moderate=0.3 / Severe=0.7) across unhealed injuries,
    /// divided by this max. Default 2.0 ≈ three Severe wounds saturate.
    #[serde(default = "default_pain_normalization_max")]
    pub pain_normalization_max: f32,
    /// Ticket 090 — `LowMastery` ZST marker gate. Fires when
    /// `mastery_confidence` (mean of all six `Skills` fields) is strictly
    /// below this threshold. Default 0.35: a cat averaging 35% skill across
    /// all six axes has meaningfully low felt-competence; above 35% the cat
    /// is coping. Freshly spawned cats start near 0.07 (default skills),
    /// well below this — `LowMastery` fires for all novice cats and clears
    /// as they practise.
    #[serde(default = "default_low_mastery_threshold")]
    pub low_mastery_threshold: f32,
    /// Ticket 090 — `LackingPurpose` ZST marker gate. Fires when
    /// `purpose_clarity` is strictly below this threshold. Default 0.5:
    /// since `purpose_clarity` is binary {0.0, 1.0}, the 0.5 midpoint is
    /// the conventional binary-signal threshold — fires exactly when the
    /// cat has no active aspiration.
    #[serde(default = "default_lacking_purpose_threshold")]
    pub lacking_purpose_threshold: f32,
    /// Ticket 090 — `EsteemDistressed` ZST marker gate. Fires when
    /// `esteem_distress` (max of L4 deficits) strictly exceeds this
    /// threshold. Default 0.55: intentionally lower than
    /// `body_distress_threshold` (0.6) because L4 distress is chronic /
    /// slow-onset, not acute. A cat whose respect or mastery need is below
    /// 45% satisfied is meaningfully esteem-distressed.
    #[serde(default = "default_esteem_distressed_threshold")]
    pub esteem_distressed_threshold: f32,
    /// Ticket 089 — initial strength assigned to a `MemoryType::Sleep`
    /// entry written by `resolve_sleep` on chain advance. Default 0.6,
    /// chosen below the per-injury memory strengths (Severe = 0.8) so
    /// rest memories fade faster than wound memories under the
    /// `decay_memories` per-tick decay (0.001 firsthand). At 0.6 a Sleep
    /// memory persists ≈ 600 ticks (~one sim day) before fading below
    /// 0.0 — matches "I rested here a few sim-hours ago."
    #[serde(default = "default_safe_rest_memory_strength_initial")]
    pub safe_rest_memory_strength_initial: f32,
    /// Ticket 089 — Manhattan radius (tiles) within which a
    /// `MemoryType::ThreatSeen` or `MemoryType::Death` memory suppresses
    /// a `MemoryType::Sleep` memory's eligibility for
    /// `LandmarkAnchor::OwnSafeRestSpot`. Default 5: a one-room ward
    /// scale, calibrated against `threat_awareness_range`. The "I
    /// remember resting here, but I also remember a hawk near here last
    /// week" gate.
    #[serde(default = "default_safe_rest_threat_suppression_radius")]
    pub safe_rest_threat_suppression_radius: f32,
    // --- Contextual threat evaluation (zoo vs bush) ---
    /// Threat intensity multiplier when the cat is inside a ward's repel radius.
    #[serde(default = "default_threat_ward_dampening")]
    pub threat_ward_dampening: f32,
    /// Threat intensity multiplier when the cat is near a colony building.
    #[serde(default = "default_threat_colony_building_dampening")]
    pub threat_colony_building_dampening: f32,
    /// Manhattan range within which a building counts as "nearby" for threat dampening.
    #[serde(default = "default_threat_building_safety_range")]
    pub threat_building_safety_range: f32,
    /// Radius from colony center used to normalize colony proximity factor.
    #[serde(default = "default_threat_colony_radius")]
    pub threat_colony_radius: f32,
    /// Minimum threat intensity multiplier when at colony center (lerps to 1.0 at radius edge).
    #[serde(default = "default_threat_colony_center_dampening")]
    pub threat_colony_center_dampening: f32,
    /// Range within which other cats count as allies for threat dampening.
    #[serde(default = "default_threat_ally_range")]
    pub threat_ally_range: f32,
    /// Per-ally dampening factor: effective urgency = 1 / (1 + n * this).
    #[serde(default = "default_threat_ally_dampening_per_cat")]
    pub threat_ally_dampening_per_cat: f32,
    // --- Cooking (Kitchen) ---
    /// Hunger-restoration multiplier applied when eating a cooked item.
    /// Applied in `resolve_eat_at_stores` after corruption freshness.
    #[serde(default = "default_cooked_food_multiplier")]
    pub cooked_food_multiplier: f32,
    /// Duration a cat spends at a Kitchen to transform a raw food item into
    /// cooked (ticket 033 Phase 4 — was `cook_ticks: u64`).
    #[serde(default = "default_cook_duration", alias = "cook_ticks")]
    pub cook_duration: DurationDays,
    /// Manhattan range within which a cat counts as "at" the Kitchen to cook.
    #[serde(default = "default_kitchen_cook_radius")]
    pub kitchen_cook_radius: f32,
    /// Ticket 075 — `CommitmentTenure` Modifier tenure window. Once a
    /// cat adopts a disposition, the modifier lifts that disposition's
    /// constituent DSE scores by `oscillation_score_lift` for this many
    /// ticks. Default ~200 (≈ 30 sim-minutes) is a conservative anti-
    /// oscillation pad: enough to break the score-tied-every-tick churn
    /// pattern, short enough not to lock cats into a stale disposition
    /// when needs genuinely shift. Tune via post-landing sensitivity
    /// sweep per the §071 sub-epic doctrine.
    #[serde(default = "default_min_disposition_tenure_ticks")]
    pub min_disposition_tenure_ticks: u64,
    /// Ticket 075 — additive lift `CommitmentTenure` applies to each
    /// constituent DSE of the cat's currently-adopted disposition while
    /// `tick - disposition_started_tick < min_disposition_tenure_ticks`.
    /// Default 0.10 — matches `befriend_familiarity_hysteresis` (only
    /// other persisted hysteresis knob in the codebase) and sits below
    /// `patience_commitment_bonus` (0.15) so the two additive bonuses
    /// stack rather than dominate.
    #[serde(default = "default_oscillation_score_lift")]
    pub oscillation_score_lift: f32,
    /// Ticket 126 — `IntentionMomentum` Modifier per-intention lift
    /// scale. Magnitude on the held intention's underlying DSE is
    /// `commitment_strength × intention_momentum_lift × decay_factor`.
    /// Default 0.10 — matches `oscillation_score_lift` so the two
    /// stack additively (anti-oscillation pad on the active
    /// disposition + per-intention bonus on the held DSE). Combined
    /// ceiling stays under `softmax_temperature × 4`, the threshold
    /// above which softmax-over-Intentions becomes effectively argmax.
    #[serde(default = "default_intention_momentum_lift")]
    pub intention_momentum_lift: f32,
    /// Ticket 126 — momentum decay window for `Goal`-shaped held
    /// intentions. Lift ramps linearly from full at `adopted_tick` to
    /// zero at `adopted_tick + intention_momentum_decay_ticks`.
    /// `Activity` intentions use their own `Termination::Ticks(n)`
    /// expiry instead. Default 600 ticks — three times the
    /// `min_disposition_tenure_ticks` window so a high-strength
    /// intention can outlast multiple oscillation cycles before its
    /// bonus erodes.
    #[serde(default = "default_intention_momentum_decay_ticks")]
    pub intention_momentum_decay_ticks: u64,
    /// Ticket 126 — preempt-margin floor for reconsideration trigger
    /// (3): a non-held DSE must exceed
    /// `held_score + intention_preempt_margin` to trigger a re-
    /// evaluation. `held_score` already reflects the
    /// `IntentionMomentum` modifier's lift after ticket 248's L3
    /// adoption-site write (`goap.rs::evaluate_and_plan` lifts
    /// `last_scores[chosen_action]` in-place at adoption time, mirroring
    /// `IntentionMomentum::apply` semantics). The "single-minded but
    /// not stupid" knob — small enough that a genuine emergency
    /// clears it, large enough that score-jitter can't. Default 0.05
    /// (half of `oscillation_score_lift`).
    #[serde(default = "default_intention_preempt_margin")]
    pub intention_preempt_margin: f32,
    /// Ticket 247 / 248 — strength regime boundary for reconsideration
    /// trigger (3); held intentions with `commitment_strength` strictly
    /// below this value skip trigger-3 entirely. Default 0.5.
    ///
    /// **Why the gate is still load-bearing after 248's substrate
    /// fix.** 247 introduced this gate after 246's soak collapsed
    /// when the function-local `PREEMPT_STRENGTH_FLOOR = 0.5` was
    /// retired. The original framing was "the gate compensates for
    /// the timing defect where `last_scores[held]` was captured
    /// before `HeldIntention` existed, so the modifier's lift never
    /// landed in the recorded `held_score`." 248 fixed that timing
    /// defect by surgically applying the lift to
    /// `last_scores[chosen_action]` at the L3 adoption site (see
    /// `goap.rs::evaluate_and_plan`). The verification run with this
    /// boundary at 0.0 STILL collapsed (5,000-tick PickUp lock,
    /// preserved at `logs/tuned-42-post-248-boundary-zero-collapsed/`)
    /// — proving the gate's true function is NOT compensating for
    /// the missing lift, but protecting against a different
    /// dynamic: **softmax-low-margin oscillations.** Low
    /// `commitment_strength` reflects a near-tie softmax pick,
    /// meaning the runner-up's actual score in `last_scores` is
    /// barely below `held_score`. Without the gate, trigger-3 fires
    /// the next tick (`top_non_held > held_score + margin`), §7.2
    /// dual-removal clears the held intention, the re-election
    /// picks (probably) the same chosen_action with similar low
    /// margin, and the cycle repeats — locking colony-scale
    /// behavior. The gate at 0.5 says: "if the softmax was a close
    /// call, don't preempt — let the natural §7.2 path handle it."
    ///
    /// The middle compensation term is still retired (248), because
    /// the lift is now honestly in `last_scores`. The gate stays at
    /// 0.5 because it addresses a separate failure mode that
    /// substrate honesty alone cannot resolve. A future ticket that
    /// addresses softmax-low-margin oscillation directly (e.g., by
    /// raising `commitment_strength_from_margin` floors, or by
    /// elevating the L3 softmax temperature when margins thin)
    /// could revisit retirement.
    ///
    /// Setting this below 0.5 risks reproducing the lock cliff;
    /// only do so under a fresh diagnosis that names the new failure
    /// mode being guarded against.
    #[serde(default = "default_intention_preempt_strength_regime_boundary")]
    pub intention_preempt_strength_regime_boundary: f32,
    /// 235: Manhattan radius within which a Stores building counts as
    /// "reachable" for the deposit-prefix branch of pickup-class plan
    /// templates. Read by `goap.rs::herb_stash_accessible_for` to
    /// author the per-cat `HasHerbStashAccessible` marker. When the
    /// marker is true, A\* may compose `[TravelTo(Stores),
    /// DepositHerbs(prefix), <goal-action>]` instead of `[DropItem,
    /// <goal-action>]`; when false, the cat falls back to DropItem.
    ///
    /// Tuning rationale: the deposit branch's effective cost includes
    /// `TravelTo(Stores)`, so cats far from the stash naturally fall
    /// back to DropItem on cost arithmetic alone. The radius gate is
    /// a behavioral guard against the degenerate "cat traverses half
    /// the map to deposit one herb before hunting" case — it caps
    /// detour eligibility at a roughly-third-of-map distance even
    /// when cost arithmetic would otherwise permit it.
    #[serde(default = "default_herb_stash_reachable_radius")]
    pub herb_stash_reachable_radius: f32,
}

fn default_true() -> bool {
    true
}

/// §respect-restoration iter 1 companion. Safety band above the
/// `critical_safety_threshold` that marks a Guarding plan achieved.
/// Default 0.15 → exit band 0.35 when `critical_safety_threshold = 0.2`.
/// Tune downstream per `docs/balance/guarding-exit-recipe.md`.
fn default_guarding_exit_epsilon() -> f32 {
    0.15
}

/// Patrol DSE safety upper-bound — score gates toward zero when
/// safety climbs past this threshold. Pairs with
/// `guarding_exit_epsilon`: commitment drops active Guarding plans at
/// ~0.35, Patrol DSE scoring gates off at 0.5, so a cat whose safety
/// has recovered above 0.5 stops picking Patrol/Guarding at the
/// scoring layer — not just at the commitment layer. Breaks the
/// seed-18301685438630318625 Thistle Patrol loop.
fn default_patrol_exit_threshold() -> f32 {
    0.5
}

fn default_threat_ward_dampening() -> f32 {
    0.3
}
fn default_threat_colony_building_dampening() -> f32 {
    0.5
}
fn default_threat_building_safety_range() -> f32 {
    5.0
}
fn default_threat_colony_radius() -> f32 {
    30.0
}
fn default_threat_colony_center_dampening() -> f32 {
    0.4
}
fn default_threat_ally_range() -> f32 {
    8.0
}
fn default_threat_ally_dampening_per_cat() -> f32 {
    0.4
}

fn default_cooked_food_multiplier() -> f32 {
    1.3
}

fn default_cook_duration() -> DurationDays {
    // 40 ticks ÷ 1000 ticks/day = 0.04 days at default scale.
    DurationDays::new(0.04)
}

fn default_kitchen_cook_radius() -> f32 {
    1.0
}

/// Ticket 075 — `CommitmentTenure` Modifier tenure window. ~30
/// sim-minutes at the default 7-tick-per-second cadence.
fn default_min_disposition_tenure_ticks() -> u64 {
    200
}

/// Ticket 075 — additive lift on the cat's incumbent disposition's
/// constituent DSEs during the tenure window.
fn default_oscillation_score_lift() -> f32 {
    0.10
}

/// Ticket 126 — per-intention lift scale for `IntentionMomentum`.
fn default_intention_momentum_lift() -> f32 {
    0.10
}

/// Ticket 126 — Goal-intention momentum decay window in ticks.
fn default_intention_momentum_decay_ticks() -> u64 {
    600
}

/// Ticket 126 — preempt-margin floor for reconsideration trigger (3).
fn default_intention_preempt_margin() -> f32 {
    0.05
}

/// Ticket 247 / 248 — strength regime boundary below which trigger-3
/// is skipped. 248 made the substrate honest (lift now lives in
/// `last_scores`) and retired the redundant compensation term in
/// the trigger-3 formula, but the gate is still load-bearing —
/// addresses softmax-low-margin oscillation, a separate failure
/// mode from the lift-timing defect. See field doc-comment for the
/// full rationale and the preserved `boundary=0.0` collapse run at
/// `logs/tuned-42-post-248-boundary-zero-collapsed/`.
fn default_intention_preempt_strength_regime_boundary() -> f32 {
    0.5
}

/// 235 default — 60 Manhattan tiles ≈ ⅓ of a typical 200-tile map
/// diagonal. Wide enough that a cat working mid-map can still route
/// through the stash, narrow enough that cross-map detours are
/// suppressed.
fn default_herb_stash_reachable_radius() -> f32 {
    60.0
}

fn default_cook_base_score() -> f32 {
    0.6
}

fn default_cook_diligence_scale() -> f32 {
    0.5
}

fn default_cook_hunger_gate() -> f32 {
    0.5
}

/// Ticket 150 R1 — see `DispositionConstants::production_self_eat_threshold`.
/// 0.5 anchors at the hangry-curve midpoint: cats below half-full eat the
/// catch in-place; cats above it deposit. The threshold becomes a tunable
/// knob for §3.7 plan-duration-symmetry follow-on work without further code
/// changes.
fn default_production_self_eat_threshold() -> f32 {
    0.5
}

fn default_cook_food_scarcity_scale() -> f32 {
    0.6
}

/// Ticket 094 — `StockpileSatiation` Modifier threshold. Mirrors the
/// shape of `fox_scent_suppression_threshold`: below this `food_fraction`
/// no suppression is applied. 0.5 = colony stockpile half-full.
fn default_stockpile_satiation_threshold() -> f32 {
    0.5
}

/// Ticket 094 — `StockpileSatiation` Modifier scale. Maximum
/// multiplicative suppression on Hunt/Forage scores at full stores
/// (`food_fraction = 1.0`). 0.85 means full stores reduce Hunt/Forage
/// to ~15% of their pre-modifier value.
fn default_stockpile_satiation_scale() -> f32 {
    0.85
}

/// Ticket 490 (R3) — `WorkPressureAffiliativeYield` threshold. Physical-
/// need pressure (`1 − phys_satisfaction`) floor below which affiliative
/// scoring is untouched. 0.5 = the damp engages once the cat's
/// physiological needs are less than half-satisfied — i.e. there is
/// real work (eat / forage / hunt / sleep) being deferred.
fn default_work_pressure_affiliative_yield_threshold() -> f32 {
    0.5
}

/// Ticket 490 (R3) — `WorkPressureAffiliativeYield` scale. Maximum
/// multiplicative damp on Socialize/GroomOther at pressure 1.0.
///
/// **Ships dormant (0.0 = identity transform).** The 2026-06-09
/// first-light A/B (seed 42, 120 s, scale 0.5) hit the known
/// Patrol-absorption cascade: freed bandwidth flowed to Patrol
/// (13% → 22% of snapshots) and Flee (23% → 33%), Cook FELL
/// (19% → 11%), and founder dispersion dropped below the ticket-490
/// canary floor. The damp must price where the freed bandwidth lands
/// before activation — see the 490 follow-on activation ticket.
fn default_work_pressure_affiliative_yield_scale() -> f32 {
    0.0
}

/// Ticket 088 — `BodyDistressPromotion` Modifier threshold.
/// `body_distress_composite` floor below which the modifier is a no-op.
/// Set above 087's `body_distress_threshold` (0.6) so the marker fires
/// first as a perception event and the modifier lifts later as a
/// stronger response.
fn default_body_distress_promotion_threshold() -> f32 {
    0.7
}

/// Ticket 146 — saturating-composition cap on cumulative positive lift
/// per DSE per pipeline pass. **Default 0.0 (disabled)**: ships inert
/// matching the removal-bare verification regime, since personality
/// modifier compositions (Pride+Independence+Patience+etc) sum to small
/// positive deltas that any non-zero cap clips from happiness. The cap
/// is scaffolding for future balance work — when distress modifier
/// values surface non-zero in a follow-on tuning ticket, set this to
/// `0.60` (matches 047's single-modifier Flee design value) to bound
/// the 107+110 Sleep double-stack without touching 047's lurches.
fn default_max_additive_lift_per_dse() -> f32 {
    0.0
}

/// Ticket 088 — `BodyDistressPromotion` Modifier lift. Maximum additive
/// lift applied to each self-care DSE at `body_distress_composite = 1.0`.
/// Lift ramps linearly from 0 at threshold to this value at full
/// distress.
fn default_body_distress_promotion_lift() -> f32 {
    0.20
}

/// Ticket 251 — Sleep DSE `health_deficit` axis Logistic midpoint.
/// 0.4 mirrors the retired `acute_health_adrenaline_threshold` so the
/// substrate-side axis fires in the same regime the post-scoring
/// modifier did. Replaces the Linear `injury_rest_bonus` semantic from
/// pre-251.
fn default_sleep_health_deficit_midpoint() -> f32 {
    0.4
}

/// Ticket 251 — Sleep DSE `health_deficit` axis Logistic steepness.
/// 10.0 matches `sleep_dep`'s involuntary-micro-sleep curve.
fn default_sleep_health_deficit_steepness() -> f32 {
    10.0
}

/// Tickets 102 / 105 — `AcuteHealthAdrenaline{Fight, Freeze}` threshold.
/// Mirrors `disposition.critical_health_threshold = 0.4`. Originally
/// shared by the Flee valence (ticket 047, retired by ticket 251).
fn default_acute_health_adrenaline_threshold() -> f32 {
    0.4
}

/// Ticket 102 — `AcuteHealthAdrenalineFight` Fight-DSE lurch magnitude.
/// **Defaults to 0.0** so the modifier ships inert; the proposed
/// magnitude (0.50) is enabled via `CLOWDER_OVERRIDES` for the Fight-branch
/// hypothesize sweep. The same magnitude is subtracted from Flee on the
/// same tick (mutual exclusion with the 047 Flee valence).
fn default_acute_health_adrenaline_fight_lift() -> f32 {
    0.0
}

/// Ticket 102 — `escape_viability` gate threshold for the Fight branch.
/// Below this value, the cat is "cornered" (insufficient open terrain or
/// burdened by dependents) and the modifier elects Fight over Flee. The
/// 0.4 default lines up with the dependent-penalty regime in 103: an
/// unburdened cat in moderately closed terrain (~0.6 viability) stays in
/// the Flee valence; a parent cat in a corner drops below 0.4 and
/// triggers the Fight gate.
fn default_acute_health_adrenaline_fight_viability_threshold() -> f32 {
    0.4
}

/// Ticket 105 — `AcuteHealthAdrenalineFreeze` Hide-DSE lurch
/// magnitude. **Defaults to 0.0** (ships inert); proposed magnitude
/// 0.70 (largest of the three valences — freeze is the last-resort
/// response). Promotion gated on the Phase-2/3 commit that lands
/// the `HideEligible` authoring system; until then the
/// `HideEligible`-gated DSE keeps Hide at score 0 and the
/// gated-boost contract makes this lift a no-op anyway
/// (double-inert).
fn default_acute_health_adrenaline_freeze_lift() -> f32 {
    0.0
}

/// Ticket 105 — `escape_viability` gate threshold for the Freeze
/// branch. Mirrors 102's Fight gate (0.4) — the two cornered-scenario
/// valences share onset semantics; the choice between them is owned
/// by their relative lift magnitudes once activated.
fn default_acute_health_adrenaline_freeze_viability_threshold() -> f32 {
    0.4
}

/// Ticket 106 — `HungerUrgency` Modifier threshold. Mirrors `1 -
/// starvation_interrupt_threshold` lifted earlier (0.6 vs 0.85) so the
/// substrate engages well before the legacy interrupt would have. The
/// threshold is the `hunger_urgency` value at which the linear ramp
/// begins; below, the modifier is a no-op.
fn default_hunger_urgency_threshold() -> f32 {
    0.6
}

/// Ticket 032 (cross-cutting with 106) — exponent on the HungerUrgency
/// ramp. **Default 1.0 (linear)** keeps current shipped behavior; sub-1
/// values reshape the curve to lead damage onset (nerve-impulse shape).
fn default_hunger_urgency_curve_exponent() -> f32 {
    1.0
}

/// Ticket 106 — `HungerUrgency` lift on Eat. **Defaults to 0.0** so the
/// modifier ships inert; the proposed magnitude (0.40) is enabled via
/// `CLOWDER_OVERRIDES` for the Phase 3 hypothesize sweep before
/// promoting to the shipped default in Phase 4 alongside the legacy
/// Starvation interrupt's removal.
fn default_hunger_urgency_eat_lift() -> f32 {
    0.0
}

/// Ticket 106 — `HungerUrgency` lift on Hunt. Defaults to 0.0 (inert);
/// proposed magnitude 0.20 — smaller than Eat because Hunt is upstream
/// in the food chain.
fn default_hunger_urgency_hunt_lift() -> f32 {
    0.0
}

/// Ticket 106 — `HungerUrgency` lift on Forage. Defaults to 0.0
/// (inert); proposed magnitude 0.20 — symmetric to Hunt.
fn default_hunger_urgency_forage_lift() -> f32 {
    0.0
}

/// Ticket 156 — `KittenEatBoost` Modifier threshold. Default `0.4`
/// — engages once a kitten's hunger urgency exceeds 0.4 (i.e., the
/// kitten is at hunger 0.6 or below), well before the colony-wide
/// `HungerUrgency` threshold (0.6). Earlier engagement reflects
/// kittens' shorter starvation runway.
fn default_kitten_eat_boost_threshold() -> f32 {
    0.4
}

/// Ticket 156 — `KittenEatBoost` Modifier multiplier ceiling.
/// Default `4.0`. At the empirical post-154 frozen-breakdown urgency
/// (~0.81) the resulting multiplier (~3.05) lifts the kitten's Eat
/// score past the social / grooming DSEs that previously dominated
/// the breakdown, restoring breakdown honesty about physiological
/// priorities for the kitten cohort. Behavior-neutral for non-kitten
/// cats (the Kitten marker gate).
fn default_kitten_eat_boost_multiplier() -> f32 {
    4.0
}

/// Ticket 156 — `KittenCryCaretakeLift` threshold. Default `0.05` —
/// any meaningful cry perception fires the additive lift; very weak
/// signals (e.g., one quiet kitten 25 tiles away) stay below.
fn default_kitten_cry_caretake_lift_threshold() -> f32 {
    0.05
}

/// Ticket 156 — `KittenCryCaretakeLift` max additive lift. Default
/// `0.5`. Sits between the typical Caretake legacy-axes baseline
/// (0.3–0.5) and the dominant social DSEs' typical scores (0.6–1.0),
/// so a perceived cry meaningfully promotes Caretake without
/// drowning out competing actions when the cry is partial.
fn default_kitten_cry_caretake_lift() -> f32 {
    0.5
}

/// Ticket 107 — `ExhaustionPressure` Modifier threshold. The
/// `energy_deficit` floor below which the lift is a no-op. Default 0.7
/// (cat at energy 0.3 or below) — engages before the legacy
/// `Exhaustion` interrupt (`energy < 0.10` ⇒ deficit > 0.90).
fn default_exhaustion_pressure_threshold() -> f32 {
    0.7
}

/// Ticket 107 — `ExhaustionPressure` lift on Sleep. **Defaults to 0.0**
/// (inert); proposed magnitude 0.40 enabled via `CLOWDER_OVERRIDES` for
/// the Phase 3 hypothesize sweep.
fn default_exhaustion_pressure_sleep_lift() -> f32 {
    0.0
}

/// Ticket 107 — `ExhaustionPressure` lift on GroomSelf. Defaults to 0.0
/// (inert); proposed magnitude 0.10 — exhausted cats sometimes
/// groom-then-sleep as a settling ritual.
fn default_exhaustion_pressure_groom_lift() -> f32 {
    0.0
}

/// Ticket 110 — `ThermalDistress` Modifier threshold. The
/// `thermal_deficit` floor below which the lift is a no-op. Default 0.7
/// — a cat well outside its thermal comfort band.
fn default_thermal_distress_threshold() -> f32 {
    0.7
}

/// Ticket 110 — `ThermalDistress` lift on Sleep (find a den / hearth).
/// **Defaults to 0.0** (inert); proposed magnitude 0.30. Build-shelter
/// lift deferred per ticket §Out-of-scope.
fn default_thermal_distress_sleep_lift() -> f32 {
    0.0
}

/// Ticket 108 — `ThreatProximityAdrenalineFlee` Modifier threshold.
/// Mirrors 047's adrenaline threshold (0.4) so the two adrenaline
/// branches have sibling onset semantics. The threshold is the
/// `threat_proximity_derivative` value at which the smoothstep ramp
/// begins; below, the modifier is a no-op.
fn default_threat_proximity_adrenaline_threshold() -> f32 {
    0.4
}

/// Ticket 108 — `ThreatProximityAdrenalineFlee` Flee-DSE lurch
/// magnitude. Promoted from 0.0 → 0.60 in the Phase-2/3/4 activation
/// commit alongside the `PrevSafetyDeficit` Component + derivative-
/// update system. Mirrors 047's Flee lift so the two adrenaline
/// branches (health-deficit lurch and threat-proximity lurch) compose
/// at sibling magnitudes when both fire.
fn default_threat_proximity_adrenaline_flee_lift() -> f32 {
    0.60
}

/// Ticket 108 — `ThreatProximityAdrenalineFlee` Sleep-DSE lurch
/// magnitude (in-pool partner — Flee is filtered from disposition
/// softmax; Sleep routes the cat to a den). Promoted from 0.0 → 0.50
/// at activation, mirroring 047's Sleep lift.
fn default_threat_proximity_adrenaline_sleep_lift() -> f32 {
    0.50
}

/// Ticket 108 — `escape_viability` gate threshold for the Flee branch.
/// Above this value, the Flee branch owns the response; below, the
/// future Fight valence (108b) takes over. Default 0.4 mirrors 102.
fn default_threat_proximity_adrenaline_viability_threshold() -> f32 {
    0.4
}

/// Ticket 109 (Phase A) — `IntraspeciesConflictResponseFlight`
/// threshold. Default 0.6 mirrors 106's hunger threshold — pressure
/// modifiers share onset semantics (gradual physiological/social
/// build).
fn default_intraspecies_conflict_flight_threshold() -> f32 {
    0.6
}

/// Ticket 109 (Phase A) — `IntraspeciesConflictResponseFlight` lift on
/// Flee. Default 0.30 (active) — pressure-shape magnitude mirroring
/// 106's hunger-urgency lift. Promoted alongside the
/// `social_status_distress` composition + nearest-cat resolution in
/// 109's Phase A activation commit.
fn default_intraspecies_conflict_flight_lift() -> f32 {
    0.30
}

/// Ticket 142 (109 Phase B) — `IntraspeciesConflictResponseFreeze`
/// threshold. Mirrors Flight's threshold so Hide and Flee share onset
/// on the social-distress axis; the substrate-side `HideEligible`
/// eligibility gate is what discriminates which valence is reachable.
fn default_intraspecies_conflict_freeze_threshold() -> f32 {
    0.6
}

/// Ticket 142 (109 Phase B) — `IntraspeciesConflictResponseFreeze`
/// lift on Hide. **Default 0.0 (inert).** Tuning out of inert is the
/// balance follow-on tracked alongside the Hide-activation substrate
/// (170 + 268).
fn default_intraspecies_conflict_freeze_hide_lift() -> f32 {
    0.0
}

/// Ticket 109 (Phase A) — perception radius (Manhattan tiles) for
/// nearest-other-cat resolution. Mirrors a typical "in the same
/// camp / next room" distance — close enough to be socially salient,
/// far enough that random territorial overlap doesn't trigger.
fn default_social_perception_radius() -> f32 {
    8.0
}

/// Ticket 109 (Phase A) — equal weight on `respect_diff` arm of the
/// composite scalar (1/3 each across respect / age / bond).
fn default_social_status_distress_respect_weight() -> f32 {
    1.0 / 3.0
}

/// Ticket 109 (Phase A) — equal weight on `age_diff` arm.
fn default_social_status_distress_age_weight() -> f32 {
    1.0 / 3.0
}

/// Ticket 109 (Phase A) — equal weight on `bond_asymmetry` arm.
fn default_social_status_distress_bond_weight() -> f32 {
    1.0 / 3.0
}

/// Ticket 109 (Phase A) — age normalization horizon. 1 sim-year
/// (default `ticks_per_season * 12 = 1_200_000`) so a one-year age
/// gap saturates the age_diff arm. Read at constant-init time as a
/// literal because `ticks_per_season` lives in `TimeConstants` and
/// the constant tables don't cross-reference at build time.
fn default_social_status_distress_age_normalization_ticks() -> u64 {
    1_200_000
}

fn default_build_pressure_cooking_min_raw_food() -> usize {
    3
}

/// 367 Commit 8 — minimum raw food items in Stores before
/// preservation-pressure (Drying Rack / Smoking Rack) starts
/// accumulating. Lower than cooking (3) because preservation is
/// most valuable when surplus exists — wait until the colony has
/// genuine excess before building rack infrastructure. Tune up if
/// preservation racks are over-elected at the expense of
/// foundational infrastructure (Stores / Kitchen / Den).
fn default_build_pressure_preservation_min_raw_food() -> usize {
    5
}

/// 367 Commit 8 — multiplier on preservation pressure rate. Default
/// 1.0 matches the Workshop/Defense pattern (no boost, no nerf);
/// a future iteration can lift this if `just verdict` shows
/// preservation racks are getting out-competed by other infrastructure
/// channels.
fn default_preservation_pressure_multiplier() -> f32 {
    1.0
}

/// 369 — minimum count of `ItemKind::Hide` items in Stores before
/// the Tanning Frame BuildPressure channel begins accumulating.
/// Mirrors `build_pressure_preservation_min_raw_food`. Default 2 —
/// two prey-rabbit kills' worth of hide (rabbits each drop one
/// `ItemKind::Hide` per the `PreyByproductConstants` default table).
/// 461 dropped from 5 → 2 after 369's first-light soak showed the
/// 5-threshold was never met in 900 ticks; tune up if Tanning Frames
/// are over-elected at the expense of foundational infrastructure.
fn default_build_pressure_tanning_min_hides() -> usize {
    2
}

/// 369 — multiplier on tanning pressure accumulation rate. 461
/// lifted from 1.0 → 2.0 after the first-light soak showed the
/// channel never winning `highest_actionable` against the 11
/// competing channels in 900-sec verification windows even with
/// the hide-anywhere signal: tanning frames are a non-foundational
/// secondary structure, and at 1.0 they accumulate too slowly to
/// fire before foundational + preservation channels saturate the
/// one-build-at-a-time slot. 2.0 is roughly cooking-tier urgency
/// (cooking sits at 1.5) — high enough to win the contest after
/// foundational infrastructure stands up, low enough that hide
/// accumulation still has to be real (the threshold gate, not
/// just a constant pressure floor, gates election). Tune down if
/// tanning frames over-elect at the expense of foundational
/// infrastructure (Stores / Kitchen / Den).
fn default_tanning_pressure_multiplier() -> f32 {
    2.0
}

fn default_cook_directive_priority() -> f32 {
    0.4
}

fn default_unmet_demand_amplifier() -> f32 {
    4.0
}

fn default_critical_health_threshold() -> f32 {
    0.4
}

fn default_fight_bail_health_threshold() -> f32 {
    0.35
}

fn default_body_distress_threshold() -> f32 {
    0.6
}

fn default_pain_normalization_max() -> f32 {
    2.0
}

fn default_low_mastery_threshold() -> f32 {
    0.35
}

fn default_lacking_purpose_threshold() -> f32 {
    0.5
}

fn default_esteem_distressed_threshold() -> f32 {
    0.55
}

fn default_safe_rest_memory_strength_initial() -> f32 {
    0.6
}

fn default_safe_rest_threat_suppression_radius() -> f32 {
    5.0
}

fn default_gate_reckless_health_threshold() -> f32 {
    0.5
}

fn default_fox_softmax_temperature() -> f32 {
    0.15
}

fn default_softmax_temperature_floor() -> f32 {
    0.05
}

fn default_softmax_temperature_ceiling() -> f32 {
    0.20
}

/// Ticket 175 — see `ScoringConstants::carry_affinity_bonus`.
///
/// Default `1.0` (multiplier disabled — the L2 carry-affinity
/// scaffolding is wired but balance-inactive). The 175 soak with
/// `1.5` regressed nourishment (-34.5%), happiness (-50.8%), and
/// seasons-survived (-57.1%) on the canonical seed-42 deep-soak.
/// The bias was strong enough to override Eating when cats were
/// holding food-adjacent items, costing the colony more
/// nourishment than the routing wins. Calibration is a separate
/// balance task — open as a follow-on ticket once the substrate-
/// refactor stabilizes (CLAUDE.md substrate-refactor doctrine
/// defers balance-tuning on refactor-affected metrics until then).
///
/// Setting this to a value > 1.0 enables the bias; the mapping
/// itself (`scoring::apply_carry_affinity`) is exhaustive and
/// tested, so flipping the knob produces the documented
/// behavioral shift without code changes.
fn default_carry_affinity_bonus() -> f32 {
    1.0
}

/// 176: trailing-window length for chronic-overflow detection. See
/// `ScoringConstants::chronicity_window_ticks`.
fn default_chronicity_window_ticks() -> u64 {
    1000
}

/// 176: rejected-deposits-per-cat-per-window threshold. See
/// `ScoringConstants::chronicity_threshold`.
fn default_chronicity_threshold() -> f32 {
    0.10
}

/// 084: per-kind herb-stash capacity on Stores. See
/// `ScoringConstants::stores_herb_capacity_per_kind`.
fn default_stores_herb_capacity_per_kind() -> u32 {
    20
}

/// 084 Commit 3: chronic-low threshold for the colony thornbriar stash.
/// See `ScoringConstants::thornbriar_stash_low_threshold`.
fn default_thornbriar_stash_low_threshold() -> u32 {
    3
}

/// 084 Commit 3: FarmDse `farm_herb_pressure` axis present_score. See
/// `ScoringConstants::farm_herb_pressure_weight`.
fn default_farm_herb_pressure_weight() -> f32 {
    1.0
}

/// 179: Build DSE present-score on the
/// `colony_stores_chronically_full` MarkerConsideration axis. Lifted
/// from 178's dormant 0.0 once the wave-closeout consumer wired the
/// marker into BuildDse. Plausibility value — 179 lands the structure
/// at this default; balance-tuning is the follow-on ticket.
fn default_build_chronic_full_weight() -> f32 {
    0.5
}

/// 176: Hunt DSE saturation weight on the `colony_food_security`
/// axis. Ships dormant at 0.0 — see
/// `ScoringConstants::hunt_food_security_weight`.
fn default_hunt_food_security_weight() -> f32 {
    0.0
}

/// 176: Forage DSE saturation weight on the `colony_food_security`
/// axis. Ships dormant at 0.0 — see
/// `ScoringConstants::forage_food_security_weight`.
fn default_forage_food_security_weight() -> f32 {
    0.0
}

/// 209: Mentor DSE positive-lift weight on the `colony_food_security`
/// axis. Tuned to 0.10 in ticket 210 — see
/// `docs/balance/210-mentor-food-security.md`.
fn default_mentor_food_security_weight() -> f32 {
    0.10
}

/// 209: Coordinate DSE positive-lift weight on the
/// `colony_food_security` axis. Tuned to 0.10 in ticket 211 — see
/// `docs/balance/211-coordinate-food-security.md`.
fn default_coordinate_food_security_weight() -> f32 {
    0.10
}

/// 209: Caretake DSE positive-lift weight on the
/// `colony_food_security` axis. Ships dormant at 0.0.
fn default_caretake_food_security_weight() -> f32 {
    0.0
}

/// 397 Layer 2 — additive L2 score lift on Caretake when the cat
/// carries `HasJuvenileDependent`. Derived from the Cook−Caretake
/// score gap in Pebblekit-67's window (~0.25). See struct field doc.
fn default_rear_kitten_caretake_lift() -> f32 {
    0.25
}

/// 209: GroomOther `FoodSecurityGroomLift` modifier weight. Ships
/// dormant at 0.0.
fn default_groom_food_security_weight() -> f32 {
    0.0
}

/// 209: Patrol `fox_scent_at_position` cost-axis weight. Ships
/// dormant at 0.0.
fn default_patrol_fox_scent_weight() -> f32 {
    0.0
}

/// 220: weight on the `RecentAmbushMap` lift in
/// `compute_ward_placement()`. 284 lifted from the 220-dormant 0.0
/// to 0.5 as a first-light activation — strong enough to dominate
/// `fox_scent.max(corruption)` when an ambush cluster is present,
/// but not so strong it overrides cat_scent / distance_cost.
/// Tighter magnitude tuning is deferred to a follow-on.
fn default_ward_ambush_anchor_weight() -> f32 {
    0.5
}

/// 220: weight on the `CarcassScentMap` lift in
/// `compute_ward_placement()`. Restores the consumer originally
/// scoped in 209 §Scope line 74. 284 lifted from 0.0 to 0.3 as a
/// first-light activation — carcass scent persists longer than
/// the event-decay ambush map, so the smaller weight avoids
/// chronic-corpse drag toward old kill-sites. Tighter magnitude
/// tuning is deferred to a follow-on.
fn default_ward_recency_anchor_weight() -> f32 {
    0.3
}

/// 296: steepness `k` of the Logistic curve applied to per-tile
/// threat-axis lifts in `compute_ward_placement()`. Default `8.0`
/// preserves pre-296 behavior (hardcoded value before promotion).
fn default_ward_placement_logistic_steepness() -> f32 {
    8.0
}

/// 296: midpoint `m` of the Logistic curve applied to per-tile
/// threat-axis lifts in `compute_ward_placement()`. Default `0.5`
/// preserves pre-296 behavior (hardcoded value before promotion).
fn default_ward_placement_logistic_midpoint() -> f32 {
    0.5
}

/// 300: stride (in tiles) of the coarse-grid candidate generation in
/// `compute_ward_placement()`. Default `5` preserves pre-300 behavior
/// (hardcoded value before promotion); the bucket-size alignment with
/// the influence maps was the original justification but the maps
/// return per-bucket values without interpolation, so finer strides
/// only resolve sub-bucket variation from the distance-to-anchor
/// penalty.
fn default_ward_placement_candidate_step() -> i32 {
    5
}

/// 297: weight on the inline fox-spawn-vicinity lift in
/// `compute_ward_placement()`. First-light activation per ticket 297
/// lifts to 0.5 (matching the ambush anchor's first-light magnitude
/// from 284). The Logistic-lift short-circuits when this weight is
/// 0.0, so the inline `compute_fox_spawn_vicinity` scan is skipped
/// entirely at dormancy — placement output is byte-identical to
/// pre-297 in that regime.
fn default_ward_fox_intercept_anchor_weight() -> f32 {
    0.5
}

/// 297: kernel radius in world tiles for the inline fox-spawn-vicinity
/// computation in `compute_ward_placement`. Default `20` tiles — large
/// enough for a fox-approach corridor, small enough to keep the per-call
/// scan cost (~800 tile lookups per candidate) cheap.
fn default_fox_intercept_kernel_radius_tiles() -> u32 {
    20
}

/// 298: weight on the `CatScentMap` lift to ward-placement's
/// argmax tiebreak. Default `0.3` preserves ticket 045's first-light
/// value (hardcoded as a literal until 298's promotion).
fn default_ward_placement_cat_value_weight() -> f32 {
    0.3
}

/// 301: default placement semantics. `SingleShotArgmax` reproduces
/// the pre-301 single-best-tile selection byte-for-byte.
fn default_ward_placement_semantics() -> WardPlacementSemantics {
    WardPlacementSemantics::SingleShotArgmax
}

/// 301: K (number of greedy rounds) under
/// `WardPlacementSemantics::DescendingResidual`. K=1 is identical to
/// `SingleShotArgmax`; default `2` lifts once when the descending-
/// residual flag is on. Ignored when semantics is `SingleShotArgmax`.
fn default_ward_placement_residual_rounds() -> i32 {
    2
}

/// 301: weight on the `WardIntentMap` sample at the cat's current
/// position in `HerbcraftWardDse`. Default `0.0` — substrate is
/// wired but dormant at land per the 220 / 297 first-light pattern,
/// so the DSE score is byte-identical pre-301 at the default.
fn default_ward_intent_dse_weight() -> f32 {
    0.0
}

/// 301: per-wake decay applied to `WardIntentMap` under
/// `DescendingResidual`. Default `0.5` — fades a fresh stamp by half
/// each wake when not refreshed, so stale intent doesn't linger past
/// a few coordinator cycles. Dormant under `SingleShotArgmax`.
fn default_ward_intent_decay_per_wake() -> f32 {
    0.5
}

/// 312: weight on the fox-approach-corridor lift in
/// `compute_ward_placement()`. Ships dormant at `0.0` per the 220 /
/// 297 / 301 first-light pattern: the substrate lands wired but
/// inert so the global-default soak stays byte-identical to
/// pre-312. The FO-1 isthmus scenario activates at fixture-level
/// `0.3`; three-seed `just hypothesize` validates the same weight.
/// First-light *global* activation is FO-3 (separate ticket).
fn default_ward_fox_approach_corridor_weight() -> f32 {
    0.0
}

/// 313: default composition rule for the `CatScentMap` lift to
/// ward-placement scoring. `Additive` reproduces the pre-313
/// formula byte-for-byte. The new `Gate` composition is opt-in
/// via fixtures and hypothesize specs until concordance promotes
/// it globally in a follow-on iter.
fn default_ward_placement_cat_value_composition() -> WardPlacementCatValueComposition {
    WardPlacementCatValueComposition::Additive
}

/// 313: default knee point for the saturating-ramp cat_value
/// gate under `WardPlacementCatValueComposition::Gate`. `0.2`
/// matches ticket 313's pseudocode: dead tiles
/// (cat_value = 0) score zero merit, warm tiles
/// (cat_value >= 0.2) score full merit, no extra reward for
/// peak density. Ignored under `Additive`.
fn default_ward_placement_cat_value_gate_floor() -> f32 {
    0.2
}

/// 228: Patrol `Consideration::Field` route-cost axis weight.
/// 256 R4: bumped from 0.0 to 0.6 to activate the dormant gate so
/// Patrol's L2 score is suppressed when the path to the perimeter
/// is risky.
fn default_patrol_route_cost_weight() -> f32 {
    0.6
}

/// 256 R3: default width of the patrol-sector grid overlaid on
/// `WardCoverageMap`.
fn default_patrol_sector_grid_w() -> usize {
    4
}

/// 256 R3: default height of the patrol-sector grid overlaid on
/// `WardCoverageMap`.
fn default_patrol_sector_grid_h() -> usize {
    3
}

/// 256 R3: default ticks per patrol sector before rotation.
fn default_patrol_sector_rotation_ticks() -> u64 {
    1000
}

/// 256 R4: FoxScent overlay weight for Guarding-disposed cats'
/// `RouteCostField` construction.
fn default_patrol_path_fox_scent_weight() -> f32 {
    1.5
}

/// 256 R4: Corruption overlay weight for Guarding-disposed cats'
/// `RouteCostField` construction.
fn default_patrol_path_corruption_weight() -> f32 {
    1.5
}

/// 263: Flee `flee_affordance` axis weight. Ships dormant at 0.0;
/// activation in a follow-on after concordance verification.
fn default_flee_affordance_weight() -> f32 {
    0.0
}

/// 268: Hide `affordance_freeze` axis weight. Ships dormant at 0.0;
/// activation in the balance follow-on for the Hide-activation
/// substrate (170 + 142 + 268).
fn default_hide_affordance_freeze_weight() -> f32 {
    0.0
}

/// 268: Hide `hide_recency_of_threat_cue` axis weight. Ships dormant
/// at 0.0; activation in the balance follow-on.
fn default_hide_recency_of_threat_cue_weight() -> f32 {
    0.0
}

/// 268: Hide `hide_perceived_intent_clarity` axis weight. Ships
/// dormant at 0.0; activation in the balance follow-on.
fn default_hide_perceived_intent_clarity_weight() -> f32 {
    0.0
}

/// 263: Patrol `patrol_threat_recency` axis weight. Ships dormant
/// at 0.0; activation in a follow-on after concordance verification.
fn default_patrol_threat_recency_weight() -> f32 {
    0.0
}

/// 263: Hunt per-target `hunt_best_predation_affordance` axis weight.
/// Ships dormant at 0.0; activation in a follow-on (recommended 0.15
/// with the other four axes scaled by 0.85).
fn default_hunt_best_predation_weight() -> f32 {
    0.0
}

/// 100 — HuntTarget DSE 5th-axis `prey_alertness_tolerance` weight.
/// Ships live at 0.15 — modest bias toward bold cats accepting alert
/// prey. Tuning belongs to follow-on per the four-artifact
/// methodology if drift > ±10% on Hunt success.
fn default_hunt_alertness_tolerance_weight() -> f32 {
    0.15
}

/// 263: Hunt resolver `stalk_start` band-threshold bias magnitude.
/// Ships dormant at 0.0; activation in a follow-on (recommended
/// 0.25 → ±25% width swing under maximum affordance asymmetry).
fn default_hunt_stalk_chase_affordance_bias() -> f32 {
    0.0
}

/// 264: SocializeTarget `target_affiliation` belief-axis weight.
/// Activated 2026-07-08 (plan step 20, first-light 0.10): partner
/// choice now reads the actor's own witnessed-practice belief
/// alongside the symmetric Relationships ledger. Balance record:
/// `docs/balance/264-social-activation.md`.
fn default_socialize_affiliation_weight() -> f32 {
    0.10
}

/// 264: SocializeTarget `affordance_socialize` axis weight.
/// Activated 2026-07-08 (plan step 20, first-light 0.10).
fn default_socialize_affordance_weight() -> f32 {
    0.10
}

/// 264: GroomOtherTarget `target_affiliation` belief-axis weight.
/// Activated 2026-07-08 (plan step 20, first-light 0.10).
fn default_groom_other_affiliation_weight() -> f32 {
    0.10
}

/// 264: GroomOtherTarget `target_perceived_hostility` belief-axis
/// weight (inverted curve — high hostility deprioritizes: "don't
/// groom the cat that just hissed at you"). Activated 2026-07-08
/// (plan step 20, first-light 0.10).
fn default_groom_other_hostility_weight() -> f32 {
    0.10
}

/// 264: GroomOtherTarget `affordance_groom_other` axis weight.
/// Activated 2026-07-08 (plan step 20, first-light 0.10).
fn default_groom_other_affordance_weight() -> f32 {
    0.10
}

/// 264: MateTarget `target_perceived_receptivity` belief-axis weight.
/// Activated 2026-07-08 (plan step 20, first-light 0.12 — the
/// receptivity read is the downstream lever on the 126/027 Mate
/// supply-chain problem, weighted slightly above the other first-light
/// axes; gate verified the 027 Mate-cadence canary).
fn default_mate_receptivity_weight() -> f32 {
    0.12
}

/// 264: MateTarget `affordance_mate` axis weight. Activated
/// 2026-07-08 (plan step 20, first-light 0.10).
fn default_mate_affordance_weight() -> f32 {
    0.10
}

/// 264: MentorTarget `affordance_mentor` axis weight. Activated
/// 2026-07-08 (plan step 20, first-light 0.10).
fn default_mentor_affordance_weight() -> f32 {
    0.10
}

/// 264: CaretakeTarget `affordance_feed_kitten` axis weight.
/// Activated 2026-07-08 (plan step 20, first-light 0.10; the scorer
/// pre-check in `evaluate_and_plan` reads the live resource since the
/// Socialize-activation commit, so the urgency argmax equals the
/// dispatch-site pick).
fn default_caretake_affordance_weight() -> f32 {
    0.10
}

/// 264: ApplyRemedyTarget `target_perceived_injury` belief-axis
/// weight (Care consumer). Activated 2026-07-08 (plan step 20) at the
/// raw-HP `target_injury` axis's full 8/14 triage slot — the belief
/// axis SUPERSEDES the raw axis, which retires from the composition
/// whenever this weight is non-zero (pillar 2: substrate first, hack
/// second). Cats now triage from witnessed/believed injury, not the
/// patient's HP bar.
fn default_apply_remedy_injury_belief_weight() -> f32 {
    8.0 / 14.0
}

/// 264: ApplyRemedyTarget `affordance_care` axis weight. Activated
/// 2026-07-08 (plan step 20, first-light 0.10).
fn default_apply_remedy_affordance_weight() -> f32 {
    0.10
}

/// 265: FoxHunting `best_prey_predation_affordance` axis weight.
/// Activated 2026-07-08 at first-light 0.10 (plan step 21).
fn default_fox_hunting_prey_affordance_weight() -> f32 {
    0.10
}

/// 265: HawkHunting `best_prey_predation_affordance` axis weight.
/// Activated 2026-07-08 at first-light 0.10 (plan step 21).
fn default_hawk_hunting_prey_affordance_weight() -> f32 {
    0.10
}

/// 265: SnakeAmbushing `best_prey_strike_affordance` axis weight.
/// Activated 2026-07-08 at first-light 0.10 (plan step 21).
fn default_snake_ambush_strike_affordance_weight() -> f32 {
    0.10
}

/// 265: SnakeForaging `best_prey_stalk_affordance` axis weight.
/// Activated 2026-07-08 at first-light 0.10 (plan step 21).
fn default_snake_forage_stalk_affordance_weight() -> f32 {
    0.10
}

/// 265: FoxFleeing `perceived_cat_threat` axis weight. Activated
/// 2026-07-08 at first-light 0.10 (plan step 21).
fn default_fox_flee_cat_violence_belief_weight() -> f32 {
    0.10
}

/// 265 activation: FoxFleeing belief-eligibility threshold. 0.75 sits
/// above the 0.5 implant prior (instinct never trips it) and below the
/// ~0.9 EMA plateau a fox reaches after witnessing repeated cat
/// violence.
fn default_fox_flee_belief_eligibility_threshold() -> f32 {
    0.75
}

/// 265: HawkFleeing `perceived_cat_threat` axis weight. Activated
/// 2026-07-08 at first-light 0.10 (plan step 21).
fn default_hawk_flee_cat_violence_belief_weight() -> f32 {
    0.10
}

/// 265 activation: HawkFleeing belief-eligibility threshold — the
/// hawk-side peer of `fox_flee_belief_eligibility_threshold`. Above
/// the 0.3 `cat_perceived_by_hawk` implant prior by a wide margin;
/// only witnessed Attack/Hunt evidence trips it.
fn default_hawk_flee_belief_eligibility_threshold() -> f32 {
    0.75
}

/// 265: SnakeFleeing `perceived_cat_threat` axis weight. Activated
/// 2026-07-08 at first-light 0.10 (plan step 21). No eligibility
/// clause needed on the snake side — SnakeFleeing's legacy outer gate
/// is `cats_nearby >= 1`, which already admits the single-cat case;
/// the axis differentiates score, not eligibility.
fn default_snake_flee_cat_violence_belief_weight() -> f32 {
    0.10
}

/// 256 R5: per-tick patrol deterrent deposit when a cat's
/// current_action is Action::Patrol.
fn default_cat_patrol_deterrent_deposit_per_tick() -> f32 {
    0.05
}

/// 256 R5: global decay rate for `CatPatrolDeterrentMap`, per
/// in-game day. 0.5/day → ~2-day half-life.
fn default_cat_patrol_deterrent_decay_rate() -> RatePerDay {
    RatePerDay::new(0.5)
}

/// 256 R5: maximum cost contribution from cat patrol deterrent on
/// a single tile during fox pathfinding.
fn default_cat_patrol_deterrent_path_cost_max() -> u32 {
    6
}

/// 256 R5: scalar weight on the deterrent overlay in fox A*.
fn default_cat_patrol_deterrent_overlay_weight() -> f32 {
    1.0
}

/// 228: Forage `Consideration::Field` route-cost axis weight. Ships
/// dormant at 0.0.
fn default_forage_route_cost_weight() -> f32 {
    0.0
}

/// 228: Hunt `Consideration::Field` route-cost axis weight. Ships
/// dormant at 0.0.
fn default_hunt_route_cost_weight() -> f32 {
    0.0
}

/// 228: Wander `Consideration::Field` route-cost axis weight. Ships
/// dormant at 0.0.
fn default_wander_route_cost_weight() -> f32 {
    0.0
}

/// 228: Explore `Consideration::Field` route-cost axis weight. Ships
/// dormant at 0.0.
fn default_explore_route_cost_weight() -> f32 {
    0.0
}

/// 223: Cat A* path-cost overlay max contribution from fox scent. See
/// `ScoringConstants::fox_scent_path_cost_max` for the rationale on
/// the value `8`.
fn default_threat_belief_path_cost_max() -> u32 {
    12
}

fn default_fox_scent_path_cost_max() -> u32 {
    8
}

/// 223: Cat A* path-cost overlay max contribution from corruption.
/// See `ScoringConstants::corruption_path_cost_max` for the rationale.
fn default_corruption_path_cost_max() -> u32 {
    10
}

/// 228: Per-cat route-cost flood radius cap. See
/// `ScoringConstants::route_cost_flood_budget`.
fn default_route_cost_flood_budget() -> u32 {
    600
}

/// 228: Tick window for `WanderTargetAnchor` seed rotation. See
/// `ScoringConstants::wander_recandidate_ticks`.
fn default_wander_recandidate_ticks() -> u64 {
    30
}

/// 228: Staleness window for `CatPathPlan::should_fall_back_at`. See
/// `ScoringConstants::route_cost_replan_window_ticks`.
fn default_route_cost_replan_window_ticks() -> u64 {
    120
}

/// 209: GroomOther `TensionDefusionGroomLift` modifier weight. Ships
/// dormant at 0.0.
fn default_tension_defusion_groom_weight() -> f32 {
    0.0
}

/// 178: Logistic slope on the `inventory_excess` axis used by
/// Discarding and Trashing DSEs. See
/// `ScoringConstants::disposal_inventory_excess_slope`.
fn default_disposal_inventory_excess_slope() -> f32 {
    8.0
}

/// 178: Logistic midpoint on the `inventory_excess` axis used by
/// Discarding and Trashing DSEs. See
/// `ScoringConstants::disposal_inventory_excess_midpoint`.
fn default_disposal_inventory_excess_midpoint() -> f32 {
    0.5
}

fn default_scent_search_radius() -> f32 {
    20.0
}

fn default_scent_detect_threshold() -> f32 {
    0.05
}

fn default_alertness_push() -> f32 {
    1.5
}

fn default_species_push() -> f32 {
    1.0
}

fn default_tremor_push() -> f32 {
    4.0
}

fn default_scent_settle_push() -> f32 {
    3.0
}

fn default_sleep_dawn_bonus() -> f32 {
    0.0
}

fn default_sleep_day_bonus() -> f32 {
    0.1
}

fn default_sleep_dusk_bonus() -> f32 {
    0.0
}

fn default_sleep_night_bonus() -> f32 {
    1.2
}

// Fox disposition phase bonuses. Values from
// docs/systems/sleep-that-makes-sense.md Phase 2 table, mapped Hunt→Hunting,
// Den→Resting; Patrolling values chosen modest-positive Dusk→Dawn to match
// crepuscular territorial rounds.
fn default_fox_hunt_dawn_bonus() -> f32 {
    0.3
}
fn default_fox_hunt_day_bonus() -> f32 {
    -0.2
}
fn default_fox_hunt_dusk_bonus() -> f32 {
    0.5
}
fn default_fox_hunt_night_bonus() -> f32 {
    0.7
}
fn default_fox_patrol_dawn_bonus() -> f32 {
    0.2
}
fn default_fox_patrol_day_bonus() -> f32 {
    -0.1
}
fn default_fox_patrol_dusk_bonus() -> f32 {
    0.3
}
fn default_fox_patrol_night_bonus() -> f32 {
    0.2
}
fn default_fox_rest_dawn_bonus() -> f32 {
    0.0
}
fn default_fox_rest_day_bonus() -> f32 {
    0.5
}
fn default_fox_rest_dusk_bonus() -> f32 {
    0.0
}
fn default_fox_rest_night_bonus() -> f32 {
    0.0
}

fn default_mating_fertility_spring() -> f32 {
    1.0
}

fn default_mating_fertility_summer() -> f32 {
    0.55
}

fn default_mating_fertility_autumn() -> f32 {
    0.2
}

fn default_mating_fertility_winter() -> f32 {
    0.0
}

fn default_caretake_bond_compassion_boost_max() -> f32 {
    1.0
}

// 382: building-placement default helpers
fn default_building_placement_semantics() -> BuildingPlacementSemantics {
    BuildingPlacementSemantics::InfluenceMap
}
fn default_building_placement_score_floor() -> f32 {
    0.0
}
fn default_building_placement_jitter_range() -> f32 {
    0.05
}
fn default_building_placement_distance_cost_per_tile() -> f32 {
    0.005
}
fn default_building_placement_candidate_step() -> i32 {
    5
}
fn default_building_placement_frontier_weight() -> f32 {
    1.0
}
fn default_building_placement_crowding_weight() -> f32 {
    1.0
}
fn default_building_placement_threat_weight() -> f32 {
    1.0
}
fn default_building_placement_food_proximity_weight() -> f32 {
    0.4
}
fn default_building_placement_garden_terrain_weight() -> f32 {
    0.3
}
fn default_building_placement_defensive_corridor_weight() -> f32 {
    0.5
}
fn default_building_placement_midden_periphery_weight() -> f32 {
    0.4
}
fn default_building_placement_same_kind_proximity_weight() -> f32 {
    0.2
}
fn default_building_placement_same_kind_proximity_range() -> f32 {
    12.0
}
fn default_colony_district_structure_halo_radius() -> f32 {
    8.0
}
fn default_colony_district_crowding_radius() -> f32 {
    5.0
}
fn default_colony_district_cat_scent_scale() -> f32 {
    1.0
}
fn default_placement_stuck_narrate_threshold_ticks() -> u32 {
    60
}
fn default_colony_center_update_cadence_ticks() -> u64 {
    1000
}

impl ScoringConstants {
    /// Fertility multiplier for a given season. Scales mating-need decay and
    /// gates the has_eligible_mate check. Returns 0 means "no breeding this
    /// season" (Winter anestrous by default).
    pub fn season_fertility(&self, season: Season) -> f32 {
        match season {
            Season::Spring => self.mating_fertility_spring,
            Season::Summer => self.mating_fertility_summer,
            Season::Autumn => self.mating_fertility_autumn,
            Season::Winter => self.mating_fertility_winter,
        }
    }
}

impl Default for DispositionConstants {
    fn default() -> Self {
        Self {
            starvation_interrupt_threshold: 0.15,
            exhaustion_interrupt_threshold: 0.10,
            critical_hunger_interrupt_threshold: 0.15,
            threat_awareness_range: 10.0,
            threat_urgency_divisor: 10.0,
            flee_threshold_base: 0.15,
            flee_threshold_boldness_scale: 0.4,
            critical_safety_threshold: 0.2,
            guarding_exit_epsilon: default_guarding_exit_epsilon(),
            flee_distance: 8.0,
            flee_ticks: 5,
            // 230: Fleeing-disposition tunables. 30 ticks of hold ≈ 30
            // sim-seconds at 1 tps; comfortable hysteresis without
            // freezing the cat for an entire activity slice.
            flee_hold_ticks: 30,
            // 100 ≈ 1/6 of MAX_COST_BUDGET (600). A 30-tile flood at
            // avg-edge ~3 (low-overlay terrain) reaches ~100 — so
            // "cost ≤ 100" picks tiles that are walkable without
            // crossing strong fox-scent or corruption pressure.
            route_cost_safe_threshold: 100,
            // 0.6 puts the safety scalar above its perceptual neutral
            // (0.5 in the interoception module) and below the typical
            // post-fight recovery plateau, so the hold step doesn't
            // count ticks while the cat still feels actively pressed.
            flee_safety_need_threshold: 0.6,
            damaged_building_threshold: 0.4,
            ward_strength_low_threshold: 0.3,
            hunt_terrain_search_radius: 15.0,
            forage_terrain_search_radius: 10.0,
            social_target_range: 10.0,
            wildlife_threat_range: 10.0,
            allies_fighting_range: 8.0,
            allies_fighting_cap: 5,
            guard_fight_health_min: 0.5,
            combat_effective_hunting_cross_train: 0.3,
            herb_detection_range: 15.0,
            prey_detection_range: 10.0,
            corrupted_tile_threshold: 0.1,
            mentor_skill_threshold_high: 0.6,
            mentor_skill_threshold_low: 0.3,
            mentoring_detection_range: 10.0,
            directive_bonus_base_weight: 0.5,
            directive_independence_penalty: 0.3,
            directive_stubbornness_penalty: 0.4,
            fondness_default: 0.5,
            fondness_social_weight: 0.6,
            novelty_social_weight: 0.4,
            disposition_independence_penalty: 0.2,
            fated_love_detection_range: 15.0,
            fated_rival_detection_range: 15.0,
            cascading_bonus_range: 5.0,
            resting_complete_hunger: 0.5,
            resting_complete_energy: 0.3,
            resting_complete_temperature: 0.3,
            planner_hunger_ok_threshold: 0.5,
            planner_energy_ok_threshold: 0.3,
            planner_temperature_ok_threshold: 0.3,
            resting_max_replans: 12,
            sleep_duration_deficit_multiplier: 175.0,
            sleep_duration_base: 75,
            guard_threat_detection_range: 10.0,
            guard_patrol_radius: 10.0,
            patrol_perimeter_offset: 12,
            patrol_sector_grid_w: default_patrol_sector_grid_w(),
            patrol_sector_grid_h: default_patrol_sector_grid_h(),
            patrol_sector_rotation_ticks: default_patrol_sector_rotation_ticks(),
            social_chain_target_range: 15.0,
            mentor_temperature_threshold: 0.5,
            groom_temperature_threshold: 0.7,
            building_search_range: 30.0,
            crafting_herb_detection_range: 15.0,
            crafting_herbcraft_skill_threshold: 0.0,
            coordinating_target_range: 30.0,
            coordinating_distance_penalty: 0.01,
            explore_range: 20.0,
            scent_downwind_dot_threshold: 0.0,
            scent_dense_forest_modifier: 0.5,
            scent_light_forest_modifier: 0.75,
            scent_base_range: 80.0,
            scent_min_range: 20.0,
            scent_search_radius: default_scent_search_radius(),
            scent_detect_threshold: default_scent_detect_threshold(),
            den_discovery_range: 3.0,
            den_discovery_base_chance: 0.02,
            den_discovery_skill_scale: 0.01,
            den_raid_kill_fraction: 0.4,
            den_dropped_item_quality: 0.8,
            respect_gain_hunting: 0.03,
            respect_gain_foraging: 0.01,
            respect_gain_guarding: 0.02,
            respect_gain_building: 0.15,
            respect_gain_coordinating: 0.05,
            respect_gain_socializing: 0.02,
            pounce_range_patient: 2.0,
            pounce_range_impatient: 3.0,
            pounce_range_default: 2.0,
            pounce_awareness_idle: 0.95,
            pounce_awareness_alert: 0.65,
            pounce_awareness_fleeing: 0.30,
            pounce_distance_close_mod: 1.0,
            pounce_distance_mid_mod: 0.9,
            pounce_distance_far_mod: 0.75,
            pounce_density_threshold: 0.5,
            pounce_skill_base: 0.5,
            pounce_skill_scale: 0.5,
            hunt_catch_skill_growth: 0.01,
            stalk_start_buffer: 2.0,
            stalk_start_minimum: 5.0,
            // 100: effective_stalk_distance lifts.
            alertness_push: default_alertness_push(),
            species_push: default_species_push(),
            tremor_push: default_tremor_push(),
            scent_settle_push: default_scent_settle_push(),
            anxiety_spook_threshold: 0.7,
            anxiety_spook_chance: 0.02,
            chase_limit_bold: 200,
            chase_limit_default: 120,
            chase_stuck_ticks: 10,
            chase_speed: 3,
            approach_speed: 3,
            approach_give_up_distance: 60.0,
            search_belief_radius: 25.0,
            search_wind_direction_threshold: 0.3,
            search_jitter_chance: 0.20,
            search_speed: 2,
            search_visual_detection_range: 15.0,
            search_timeout_ticks: 80,
            // 140 step 6 — 200 → 300. Fluid movement makes legs
            // legitimately slower: acceleration from rest (~4 ticks
            // per stop) and the Euclidean speed cap (diagonal travel
            // sqrt(2) slower than the grid era — the plan's
            // hypothesis-carried re-baseline). 1.5x headroom keeps
            // the watchdog meaningful without mass-failing patrols.
            travel_timeout_ticks: 300,
            travel_no_path_stuck_ticks: 10,
            global_step_timeout_ticks: 500,
            forage_jitter_chance: 0.10,
            forage_yield_scale: 0.35,
            forage_skill_growth: 0.0008,
            forage_timeout_ticks: 40,
            forage_ingredient_drop_chance: 0.10,
            deposit_quality_base: 0.3,
            deposit_quality_skill_scale: 0.4,
            eat_at_stores_duration: 50,
            corruption_food_penalty: 0.5,
            production_self_eat_threshold: default_production_self_eat_threshold(),
            sleep_energy_per_tick: 0.0035,
            sleep_temperature_per_tick: 0.002,
            self_groom_duration: 8,
            self_groom_temperature_gain: 0.15,
            socialize_social_per_tick: 0.005,
            socialize_fondness_per_tick: 0.0005,
            socialize_familiarity_per_tick: 0.0008,
            socialize_duration: 100,
            groom_other_social_per_tick: 0.002,
            groom_other_fondness_per_tick: 0.0008,
            groom_other_familiarity_per_tick: 0.0003,
            groom_other_duration: 80,
            groom_other_temperature_gain: 0.005,
            // 035: bury action constants.
            bury_ticks: default_bury_ticks(),
            burial_sense_range: default_burial_sense_range(),
            bury_belonging_gain: default_bury_belonging_gain(),
            acceptance_per_groomed: 0.08,
            acceptance_per_kitten_fed: 0.10,
            mentor_mastery_per_tick: 0.02,
            mentor_social_per_tick: 0.01,
            mentor_respect_per_tick: 0.002,
            mentor_fondness_per_tick: 0.005,
            mentor_familiarity_per_tick: 0.003,
            mentor_duration: 12,
            apprentice_skill_growth_multiplier: 0.04,
            patrol_arrival_safety_gain: 0.005,
            patrol_per_tile_safety_gain: 0.0005,
            patrol_stuck_timeout: 300,
            fight_duration: 300,
            fight_combat_skill_growth: 0.0015,
            fight_safety_gain: 0.2,
            fight_mastery_gain: 0.03,
            survey_duration: 50,
            survey_purpose_gain: 0.008,
            // Iteration 2 of `docs/balance/mastery-restoration.md` —
            // dropped from 0.02 to 0.002 per the iter-1 mechanism
            // correction (every cat saturated to 1.0 at 0.02; survey
            // is a more common resolver completion than iter-1
            // assumed).
            survey_mastery_gain: 0.002,
            // Iteration 2 — receiver-side per-tick acceptance.
            acceptance_per_groom_other_per_tick: default_acceptance_per_groom_other_per_tick(),
            acceptance_per_feed_kitten_per_tick: default_acceptance_per_feed_kitten_per_tick(),
            acceptance_per_mentor_per_tick: default_acceptance_per_mentor_per_tick(),
            acceptance_per_cleanse_per_tick: default_acceptance_per_cleanse_per_tick(),
            // New thread — respect witness multiplier.
            respect_per_witness: default_respect_per_witness(),
            respect_witness_radius: default_respect_witness_radius(),
            respect_witness_cap: default_respect_witness_cap(),
            // New thread — purpose colony-action hooks.
            purpose_per_colony_action: default_purpose_per_colony_action(),
            purpose_per_deposit: default_purpose_per_deposit(),
            purpose_per_ward_set: default_purpose_per_ward_set(),
            purpose_per_directive_completed: default_purpose_per_directive_completed(),
            purpose_per_build_tick: default_purpose_per_build_tick(),
            // Iteration 2 — per-action mastery (STUB; ticket 016 Phase 5).
            mastery_per_magic_success: default_mastery_per_magic_success(),
            mastery_per_successful_tend: default_mastery_per_successful_tend(),
            mastery_per_build_tick: default_mastery_per_build_tick(),
            mastery_per_successful_cook: default_mastery_per_successful_cook(),
            mastery_per_successful_hunt: default_mastery_per_successful_hunt(),
            survey_colony_discovery_scale: 0.02,
            survey_personal_discovery_scale: 0.005,
            survey_explore_radius: default_survey_explore_radius(),
            exploration_decay_rate: 0.0000125,
            passive_explore_radius: default_passive_explore_radius(),
            explore_perception_radius: default_explore_perception_radius(),
            explore_satiation_threshold: default_explore_satiation_threshold(),
            social_satiation_threshold: default_social_satiation_threshold(),
            explore_den_discovery_chance: 0.08,
            deliver_directive_duration: 50,
            deliver_directive_respect_gain: 0.005,
            deliver_directive_social_gain: 0.005,
            idle_fallback_duration: 5,
            anti_stack_jitter: true,
            critical_health_threshold: 0.4,
            fight_bail_health_threshold: 0.35,
            body_distress_threshold: default_body_distress_threshold(),
            pain_normalization_max: default_pain_normalization_max(),
            low_mastery_threshold: default_low_mastery_threshold(),
            lacking_purpose_threshold: default_lacking_purpose_threshold(),
            esteem_distressed_threshold: default_esteem_distressed_threshold(),
            safe_rest_memory_strength_initial: default_safe_rest_memory_strength_initial(),
            safe_rest_threat_suppression_radius: default_safe_rest_threat_suppression_radius(),
            threat_ward_dampening: 0.3,
            threat_colony_building_dampening: 0.5,
            threat_building_safety_range: 5.0,
            threat_colony_radius: 30.0,
            threat_colony_center_dampening: 0.4,
            threat_ally_range: 8.0,
            threat_ally_dampening_per_cat: 0.4,
            cooked_food_multiplier: default_cooked_food_multiplier(),
            cook_duration: default_cook_duration(),
            kitchen_cook_radius: default_kitchen_cook_radius(),
            min_disposition_tenure_ticks: default_min_disposition_tenure_ticks(),
            oscillation_score_lift: default_oscillation_score_lift(),
            intention_momentum_lift: default_intention_momentum_lift(),
            intention_momentum_decay_ticks: default_intention_momentum_decay_ticks(),
            intention_preempt_margin: default_intention_preempt_margin(),
            intention_preempt_strength_regime_boundary:
                default_intention_preempt_strength_regime_boundary(),
            herb_stash_reachable_radius: default_herb_stash_reachable_radius(),
        }
    }
}

// ---------- ColonyScoreConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ColonyScoreConstants {
    pub bonds_weight: f64,
    pub aspirations_weight: f64,
    pub structures_weight: f64,
    pub kittens_weight: f64,
    pub prey_dens_weight: f64,
    pub deaths_starvation_penalty: f64,
    pub deaths_injury_penalty: f64,
    pub deaths_old_age_bonus: f64,
    pub den_shelter_radius: f32,
    pub activation_breadth_bonus: f64,
    pub activation_depth_bonus: f64,
    /// Elapsed-tick mark at which `emit_colony_score` freezes a one-shot
    /// `ColonyScoreCheckpoint` (snapshot + achievement ledger). Soaks are
    /// fixed *wall-clock* (900 s), so end-of-run score is confounded with
    /// binary throughput — both the `welfare × seasons` multiplier and the
    /// achievement ledger grow with elapsed sim-time, and the 2026-05 63%
    /// TPS regression deflated end-of-run aggregate ~30% with identical
    /// behavior. The checkpoint is the TPS-invariant comparison surface
    /// `just verdict` prefers when both runs carry it.
    ///
    /// 50_000 = 2.5 seasons (season transitions at 20k/40k/60k elapsed):
    /// sits 10k ticks from the nearest integer-season boundary (avoids
    /// knife-edge multiplier flips) and ~20% below the slowest observed
    /// current run (~63k elapsed in 900 s), so every current binary
    /// reaches it. 0 disables capture.
    pub checkpoint_elapsed_ticks: u64,
}

impl Default for ColonyScoreConstants {
    fn default() -> Self {
        Self {
            bonds_weight: 10.0,
            aspirations_weight: 25.0,
            structures_weight: 15.0,
            kittens_weight: 50.0,
            prey_dens_weight: 20.0,
            deaths_starvation_penalty: 30.0,
            deaths_injury_penalty: 15.0,
            deaths_old_age_bonus: 5.0,
            den_shelter_radius: 4.0,
            activation_breadth_bonus: 20.0,
            activation_depth_bonus: 5.0,
            checkpoint_elapsed_ticks: 50_000,
        }
    }
}

// ---------- WildlifeConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WildlifeConstants {
    pub circling_angle_step: f32,
    pub circling_radius: f32,
    pub shadow_fox_corruption_deposit: f32,
    pub patrol_jitter_chance: f32,
    pub detection_narrative_cooldown: u64,
    pub spawn_narrative_cooldown: u64,
    pub base_detection_range: f32,
    pub forest_range_penalty: f32,
    pub threat_safety_drain: f32,
    pub threat_mood_penalty: f32,
    pub threat_mood_ticks: u64,
    pub predator_hunt_chance: f32,
    pub predator_hunt_range_fox: f32,
    pub predator_hunt_range_hawk: f32,
    pub predator_hunt_range_snake: f32,
    pub predator_hunt_range_shadow_fox: f32,
    pub predator_kill_chance: f32,
    pub predator_kill_narrative_chance: f32,
    pub initial_fox_count_min: u32,
    pub initial_fox_count_max: u32,
    pub initial_fox_min_distance: f32,
    pub initial_hawk_count_min: u32,
    pub initial_hawk_count_max: u32,
    pub initial_hawk_min_distance: f32,
    pub initial_snake_count_min: u32,
    pub initial_snake_count_max: u32,
    pub initial_snake_min_distance: f32,
    /// Corruption emitted per tick by an uncleansed carcass.
    pub carcass_corruption_rate: f32,
    /// Chance a shadow fox kill leaves a rotting carcass (vs consuming fully).
    pub carcass_drop_chance: f32,
    /// Ticks before a carcass crumbles to dust.
    pub carcass_max_age: u64,
    /// Per-tick scent magnitude an actionable carcass deposits onto
    /// `CarcassScentMap`. Phase 2C — mirrors `PreyConstants::
    /// scent_deposit_per_tick`. §5.6.3 row #6.
    #[serde(default = "default_carcass_scent_deposit_per_tick")]
    pub carcass_scent_deposit_per_tick: f32,
    /// Global decay on `CarcassScentMap`, expressed per in-game day.
    /// Per §5.6.5 #6 ("slow fade"), carcass scent persists longer than
    /// active prey-scent activity trails — the kill site lingers as
    /// a draw for scavengers even after the carcass is processed.
    /// Half the prey-scent decay rate gives ~2 in-game days for a peak
    /// deposit to fade below the typical detection threshold.
    #[serde(default = "default_carcass_scent_decay_rate")]
    pub carcass_scent_decay_rate: RatePerDay,
    /// `WardCoverageMap` bucket value at which a shadow fox flips
    /// into ward-avoidance (260: replaces the hardcoded
    /// `Ward.repel_radius() * shadow_fox_ward_repel_multiplier`
    /// snapshot read with a `InfluenceMap`-visible threshold).
    ///
    /// `WardCoverageMap` stamps linear falloff
    /// `strength * (1 - dist/repel_radius)`, so at the default
    /// strength 1.0 and repel_radius ≈ 9 a fox 2/3 of the way out
    /// from a ward reads ≈ 0.33 — `0.15` is roughly the boundary the
    /// pre-260 `multiplier=3.0` check fired at.
    #[serde(default = "default_shadow_fox_ward_avoid_threshold")]
    pub shadow_fox_ward_avoid_threshold: f32,
    /// Probability a shadow fox encircles a ward instead of reversing.
    pub ward_siege_chance: f32,
    /// Extra decay per tick per encircling shadow fox.
    pub ward_siege_decay_bonus: f32,
    /// Corruption deposit rate per tick while encircling.
    pub ward_siege_corruption_rate: f32,
    /// Tile radius around ward affected by siege corruption.
    pub ward_siege_corruption_radius: f32,
    /// Max ticks a shadow fox will encircle before reverting to patrol.
    pub ward_siege_max_ticks: u64,
    /// If a cat comes within this range, encircling fox switches to stalking.
    pub siege_break_range: f32,
    /// Threat power multiplier from local tile corruption (additive, e.g. 0.5 = +50% at full corruption).
    pub corruption_threat_multiplier: f32,
    /// Ticks a shadow fox must wait after an ambush before it can stalk again.
    pub ambush_cooldown_ticks: u32,
    /// Range (manhattan) within which cats witness an ambush and have safety drained.
    pub ambush_witness_range: f32,
    /// Safety drain applied to cats who witness a nearby ambush.
    pub ambush_witness_safety_drain: f32,
    /// 312: deposit per tick when a patrolling ShadowFox advances
    /// through a tile. Lays a corridor-traffic gradient sampled by
    /// `compute_ward_placement` to recognize topological criticality
    /// (the tiles foxes actually traverse to reach cats). Default
    /// `0.05` — a peak (1.0) bucket reaches saturation after ~20
    /// passes, mirroring `cat_patrol_deterrent_deposit_per_tick`.
    /// Only deposits during `FoxAiPhase::PatrolTerritory` (active
    /// patrol movement); skips `Resting` / `ScentMarking` /
    /// `Confronting` so stationary or den-pinned foxes don't paint
    /// the corridor map.
    #[serde(default = "default_fox_approach_corridor_deposit_per_tick")]
    pub fox_approach_corridor_deposit_per_tick: f32,
    /// 312: half-life (in ticks) of `FoxApproachCorridorMap` per-tile
    /// values. Exponential-decay shape with default 20_000 ticks
    /// (~4 in-game days) reflects that corridors are stable
    /// terrain features (fox patrol routes persist across many
    /// ambush events). Slower-than-ambush decay keeps the substrate
    /// visible on routes that see traffic every few days but no
    /// ambush event, which is exactly the topological-criticality
    /// signal the ward placement scorer needs.
    #[serde(default = "default_fox_approach_corridor_half_life_ticks")]
    pub fox_approach_corridor_half_life_ticks: u32,

    // ----- Ticket 023 Phase A: ShadowFox Coherence drive -----
    /// Coherence loss per tick on tiles whose corruption is at or below
    /// `shadow_fox_coherence_decay_threshold`. With the default
    /// `0.0005` and a freshly-manifested shadow-fox (coherence 1.0), a
    /// continuously-clean tile dissolves the entity in ~2000 ticks
    /// (~1/30th of a sim-day). Matches the design doc's intent:
    /// aggressive cleansing is a viable defeat path measured in
    /// sim-minutes, not sim-seconds. (Initial soak with `0.002`
    /// — 4x steeper — produced a +463% shadow_fox_spawn churn and a
    /// continuity mythic-texture canary failure on seed-42; the
    /// gentler rate keeps populations stable while still letting
    /// sustained cleansing dissolve them.)
    #[serde(default = "default_shadow_fox_coherence_decay_clean")]
    pub shadow_fox_coherence_decay_clean: f32,
    /// Coherence gain per tick on tiles whose corruption is above
    /// `shadow_fox_coherence_recovery_threshold`. Net-positive vs
    /// `shadow_fox_coherence_decay_clean` so a shadow-fox sitting on
    /// dark ground reconstitutes faster than it dissolves on clean
    /// ground — the corruption substrate genuinely sustains it.
    #[serde(default = "default_shadow_fox_coherence_recovery_corrupt")]
    pub shadow_fox_coherence_recovery_corrupt: f32,
    /// Tile-corruption ceiling at or below which decay applies. Tiles
    /// with corruption between `decay_threshold` and
    /// `recovery_threshold` produce neither decay nor recovery — the
    /// "flicker band" the design doc calls out. Hysteresis prevents
    /// shadow-foxes from oscillating across a single decay/recovery
    /// boundary as they patrol the corruption gradient.
    #[serde(default = "default_shadow_fox_coherence_decay_threshold")]
    pub shadow_fox_coherence_decay_threshold: f32,
    /// Tile-corruption floor at or above which recovery applies. See
    /// `shadow_fox_coherence_decay_threshold` for the hysteresis band.
    #[serde(default = "default_shadow_fox_coherence_recovery_threshold")]
    pub shadow_fox_coherence_recovery_threshold: f32,
    /// Coherence floor at or below which the shadow-fox dissolves
    /// (despawn + `EventKind::ShadowFoxDissolved`). Default `0.0`;
    /// surfaced as a knob for balance experiments that want a
    /// hysteresis band.
    #[serde(default = "default_shadow_fox_coherence_dissolution_threshold")]
    pub shadow_fox_coherence_dissolution_threshold: f32,

    // ----- Ticket 023 Phase B: Motivation softmax + 4 drives -----
    /// How often the motivation tick re-elects a shadow-fox's state.
    /// Default `16` ticks balances responsiveness with cost: cats see
    /// continuous shadow-fox motion (16 ticks ≈ 1/8th of a sim-second
    /// at default scale), while keeping the O(N²) cat-proximity reads
    /// in the Dread scorer cheap.
    #[serde(default = "default_shadow_fox_motivation_tick_cadence")]
    pub shadow_fox_motivation_tick_cadence: u64,
    /// Softmax temperature for selecting the winning drive. Lower
    /// values produce sharper picks (clear winner dominates), higher
    /// values produce more stochastic exploration. Design doc default
    /// `0.3` keeps coherence dominance reliable when the drive is
    /// pressured while letting Resonance/Dread/Entropy occasionally
    /// fire when scores are close.
    #[serde(default = "default_shadow_fox_motivation_softmax_temp")]
    pub shadow_fox_motivation_softmax_temp: f32,
    /// Additive uniform jitter on each drive's pre-softmax score.
    /// Range is symmetric (`-jitter ..= +jitter`). Prevents two
    /// shadow-foxes with identical state from picking identical
    /// drives every motivation tick (which would produce visible
    /// lockstep behavior).
    #[serde(default = "default_shadow_fox_motivation_jitter")]
    pub shadow_fox_motivation_jitter: f32,
    /// Weight applied to the Resonance drive's raw pressure score.
    /// Resonance pressure scales with the count of corrupt tiles
    /// inside a ward's repel zone; weight calibrates that count
    /// against the other drives' [0, 1] pressure ranges.
    #[serde(default = "default_shadow_fox_resonance_weight")]
    pub shadow_fox_resonance_weight: f32,
    /// Falloff scale for Entropy's distance-to-frontier signal. The
    /// raw score is `1.0 / (1.0 + distance_scale * dist)`, so larger
    /// values push attention toward the nearest frontier tile,
    /// smaller values let the shadow-fox roam farther in search of
    /// more promising frontier gaps.
    #[serde(default = "default_shadow_fox_entropy_distance_scale")]
    pub shadow_fox_entropy_distance_scale: f32,
    /// Manhattan distance the Haunting state tries to maintain from
    /// its target cat. At this distance the cat detects the shadow-
    /// fox at sensory threshold but no combat occurs (combat
    /// requires adjacency). Phase B is detection-only; Phase C wires
    /// the per-tick safety/mood drain.
    #[serde(default = "default_shadow_fox_haunting_edge_distance")]
    pub shadow_fox_haunting_edge_distance: f32,
    /// While Reconstituting (sitting on a high-corruption tile),
    /// coherence recovers at `recovery_corrupt × this` per tick.
    /// Default `3.0` makes recovery decisive — a shadow-fox that
    /// retreats to its origin patch can recover meaningfully fast.
    #[serde(default = "default_shadow_fox_reconstituting_recovery_multiplier")]
    pub shadow_fox_reconstituting_recovery_multiplier: f32,
    /// Radius (Manhattan) within which the motivation tick scans for
    /// cats / wards / frontier tiles when scoring drives. Default
    /// `12` matches the shadow-fox sensory range; lifted here so a
    /// future change can tune perception scope without recompiling
    /// the sensory profile.
    #[serde(default = "default_shadow_fox_motivation_scan_radius")]
    pub shadow_fox_motivation_scan_radius: f32,
    /// Minimum drive-pressure required for the motivation tick to
    /// transition out of the current state. When all four drives'
    /// pressures fall below this, the tick declines to transition —
    /// the existing `wildlife_ai` Patrolling/Stalking pipeline keeps
    /// running unchanged. Without this guard, Phase B's softmax
    /// monotonically pulls shadow-foxes out of Patrolling forever,
    /// suppressing the Stalking → Ambush → Banishment chain that
    /// drives the mythic-texture continuity canary. Default `0.05`
    /// — empirically the noise floor above which a drive is
    /// genuinely pressured rather than jitter-driven.
    #[serde(default = "default_shadow_fox_motivation_min_pressure")]
    pub shadow_fox_motivation_min_pressure: f32,

    // ----- Ticket 023 Phase C: deep Dread + haunting drain -----
    /// Manhattan radius around each cat that counts toward isolation
    /// scoring. Cats with ≥ `shadow_fox_dread_group_threshold` allies
    /// inside this radius receive a Dread-pressure multiplier of
    /// `shadow_fox_dread_group_suppression` (suppression). Default
    /// `8` matches the lower end of the cat-scent gradient — beyond
    /// 8 tiles cats are effectively isolated from each other for
    /// shadow-fox perception purposes.
    #[serde(default = "default_shadow_fox_dread_isolation_radius")]
    pub shadow_fox_dread_isolation_radius: f32,
    /// Minimum number of nearby allies (within
    /// `shadow_fox_dread_isolation_radius`) that triggers Dread
    /// group-suppression. Default `2` — a cat with at least 2 allies
    /// nearby is meaningfully *not* alone; the shadow-fox should
    /// hunt easier prey.
    #[serde(default = "default_shadow_fox_dread_group_threshold")]
    pub shadow_fox_dread_group_threshold: u32,
    /// Dread-pressure multiplier when the target cat has ≥
    /// `group_threshold` allies nearby. Range [0, 1]. Default `0.2`
    /// reduces the haunt-pressure to 20% of solo-cat baseline —
    /// shadow-foxes still consider grouped cats but strongly prefer
    /// isolated ones.
    #[serde(default = "default_shadow_fox_dread_group_suppression")]
    pub shadow_fox_dread_group_suppression: f32,
    /// Manhattan radius around a Haunting shadow-fox within which
    /// `shadowfox_haunting_drain` applies per-tick safety/mood drain
    /// to the target cat. Default `5` — close enough to be
    /// psychologically present, far enough to not trigger the
    /// adjacent-cell combat threshold.
    #[serde(default = "default_shadow_fox_haunting_drain_radius")]
    pub shadow_fox_haunting_drain_radius: f32,
    /// Per-tick negative mood-valence delta applied to a cat being
    /// haunted (additive on `Mood.valence`, clamped at -1). Default
    /// `0.002` — over a 150-tick haunting episode the cat's mood
    /// drops by ~0.3, which is meaningful but recoverable.
    #[serde(default = "default_shadow_fox_haunting_mood_drain")]
    pub shadow_fox_haunting_mood_drain: f32,
    /// Per-tick safety drain applied to a cat being haunted (drops
    /// `Needs.safety`, clamped at 0.0). Default `0.003` — paired
    /// with the mood drain, gives an isolated cat an unmistakable
    /// "something is wrong" signal before any combat occurs.
    #[serde(default = "default_shadow_fox_haunting_safety_drain")]
    pub shadow_fox_haunting_safety_drain: f32,
    /// Cadence (in ticks) at which `Feature::ShadowFoxHaunting`
    /// records a haunting-pressure event. Drain happens every tick,
    /// but Feature emission is rate-limited so the activation footer
    /// doesn't get inundated. Default `30` ticks ≈ 1 emission per
    /// 1/4 sim-second.
    #[serde(default = "default_shadow_fox_haunting_feature_cadence")]
    pub shadow_fox_haunting_feature_cadence: u64,
    /// Ticks of continuous Haunting before the motivation tick will
    /// promote the Haunting target to Stalking (the existing
    /// pre-Phase-A combat path). Default `30` — about 1/4 sim-second
    /// at default scale. Provides a "haunting first, attack second"
    /// rhythm: the cat has time to flee or seek allies before combat
    /// commits.
    #[serde(default = "default_shadow_fox_haunting_escalation_ticks")]
    pub shadow_fox_haunting_escalation_ticks: u64,
    /// Manhattan range within which the Entropy drive's deep scan
    /// looks for corruption-frontier tiles. Default `12` matches
    /// the motivation scan radius; lifted as a knob in case Entropy
    /// should range farther than the other drives.
    #[serde(default = "default_shadow_fox_frontier_detection_range")]
    pub shadow_fox_frontier_detection_range: f32,

    // ----- Ticket 310 S1: satiation drive -----
    /// `ShadowFoxDrives.satiation` at spawn. Corruption births a
    /// shadow-fox part-hungry: below the stalk-suppression threshold
    /// (so it is not born suppressed) but far from starving (so the
    /// hunger drive engages only after some cadence decay). Default
    /// `0.5`.
    #[serde(default = "default_shadow_fox_satiation_at_spawn")]
    pub shadow_fox_satiation_at_spawn: f32,
    /// Satiation gained from a successful cat ambush (clamped to 1.0).
    /// Default `0.8` — one cat is nearly a full meal; the fed
    /// shadow-fox drops out of stalk eligibility for
    /// `(gain + threshold − 1) / decay_per_cadence` cadences.
    #[serde(default = "default_shadow_fox_satiation_gain_ambush")]
    pub shadow_fox_satiation_gain_ambush: f32,
    /// Satiation gained from a prey-animal kill in
    /// `predator_hunt_prey` (clamped to 1.0). Default `0.4` — prey is
    /// half a meal; two kills roughly match one ambush, giving the
    /// prey ecology real weight as an alternative to cat predation.
    #[serde(default = "default_shadow_fox_satiation_gain_prey_kill")]
    pub shadow_fox_satiation_gain_prey_kill: f32,
    /// Satiation decay applied once per motivation cadence
    /// (`shadow_fox_motivation_tick_cadence`, default 16 ticks).
    /// Default `0.001` — full-to-starving in ~16k ticks, so a fed
    /// shadow-fox stays out of the hunt for thousands of ticks
    /// instead of re-rolling stalks the moment its ambush cooldown
    /// expires (the pre-310 "pinball" cadence).
    #[serde(default = "default_shadow_fox_satiation_decay_per_cadence")]
    pub shadow_fox_satiation_decay_per_cadence: f32,
    /// Satiation at or above which `predator_stalk_cats` skips the
    /// legacy 5%/tick stalk roll — a fed predator doesn't hunt.
    /// Default `0.7`: post-ambush satiation (+0.8) sits above it;
    /// spawn satiation (0.5) sits below.
    #[serde(default = "default_shadow_fox_stalk_satiation_threshold")]
    pub shadow_fox_stalk_satiation_threshold: f32,
    /// Weight on the hunger pressure `(1 − satiation)²` entering the
    /// 023 motivation softmax as the fifth drive (winner elects
    /// Stalking toward the nearest scanned cat). `0.0` restores the
    /// four-drive softmax byte-exactly (the fifth score and its
    /// jitter draw are skipped entirely — conditional-axis pattern).
    /// First-light default `0.10` per the release-plan activation
    /// discipline.
    #[serde(default = "default_shadow_fox_hunger_drive_weight")]
    pub shadow_fox_hunger_drive_weight: f32,

    // ----- Ticket 310 S2: den + post-ambush retreat -----
    /// Distance from the den at which a Retreating shadow-fox counts
    /// as arrived and releases to Patrolling. Default `1.5` tiles —
    /// on or adjacent to the den tile.
    #[serde(default = "default_shadow_fox_retreat_arrival_radius")]
    pub shadow_fox_retreat_arrival_radius: f32,
    /// `steering::arrive` slow-radius for the retreat leg — the fox
    /// decelerates inside this distance of the den instead of
    /// overshooting. Default `2.0` tiles.
    #[serde(default = "default_shadow_fox_retreat_arrive_slow_radius")]
    pub shadow_fox_retreat_arrive_slow_radius: f32,
    /// A new manifestation within this distance of an existing
    /// `ShadowFoxDen` adopts that den instead of opening another —
    /// bounds den accumulation across spawn cycles. Default `8.0`.
    #[serde(default = "default_shadow_fox_den_reuse_radius")]
    pub shadow_fox_den_reuse_radius: f32,

    // ----- Ticket 310 S3: kill-site memory -----
    /// How long (ticks) a shadow-fox remembers its last kill site as
    /// fished-out. Default `20_000` — outlasts the post-kill satiation
    /// suppression window (~4.8k ticks from 1.0 back under the stalk
    /// threshold), so when hunger returns the fox hunts *elsewhere*
    /// instead of re-farming the same corner of the colony.
    #[serde(default = "default_shadow_fox_kill_site_memory_ticks")]
    pub shadow_fox_kill_site_memory_ticks: u64,
    /// Cats within this distance of the remembered kill site are
    /// excluded from stalk-target selection (legacy roll + hunger
    /// election) while the memory is fresh. Default `6.0`.
    #[serde(default = "default_shadow_fox_kill_site_avoid_radius")]
    pub shadow_fox_kill_site_avoid_radius: f32,
}

impl Default for WildlifeConstants {
    fn default() -> Self {
        Self {
            circling_angle_step: 0.3,
            circling_radius: 8.0,
            shadow_fox_corruption_deposit: 0.001,
            patrol_jitter_chance: 0.1,
            detection_narrative_cooldown: 100,
            spawn_narrative_cooldown: 50,
            base_detection_range: 8.0,
            forest_range_penalty: 1.0,
            threat_safety_drain: 0.15,
            threat_mood_penalty: -0.2,
            threat_mood_ticks: 30,
            predator_hunt_chance: 0.1,
            predator_hunt_range_fox: 3.0,
            predator_hunt_range_hawk: 5.0,
            predator_hunt_range_snake: 1.0,
            predator_hunt_range_shadow_fox: 3.0,
            predator_kill_chance: 0.3,
            predator_kill_narrative_chance: 0.15,
            initial_fox_count_min: 2,
            initial_fox_count_max: 3,
            initial_fox_min_distance: 10.0,
            initial_hawk_count_min: 1,
            initial_hawk_count_max: 2,
            initial_hawk_min_distance: 10.0,
            initial_snake_count_min: 1,
            initial_snake_count_max: 2,
            initial_snake_min_distance: 7.0,
            carcass_corruption_rate: 0.002,
            carcass_drop_chance: 0.25,
            carcass_max_age: 500,
            carcass_scent_deposit_per_tick: default_carcass_scent_deposit_per_tick(),
            carcass_scent_decay_rate: default_carcass_scent_decay_rate(),
            shadow_fox_ward_avoid_threshold: default_shadow_fox_ward_avoid_threshold(),
            ward_siege_chance: 0.3,
            ward_siege_decay_bonus: 0.0005,
            ward_siege_corruption_rate: 0.005,
            ward_siege_corruption_radius: 3.0,
            ward_siege_max_ticks: 200,
            siege_break_range: 3.0,
            corruption_threat_multiplier: 0.5,
            ambush_cooldown_ticks: 100,
            ambush_witness_range: 12.0,
            ambush_witness_safety_drain: 0.08,
            fox_approach_corridor_deposit_per_tick: default_fox_approach_corridor_deposit_per_tick(
            ),
            fox_approach_corridor_half_life_ticks: default_fox_approach_corridor_half_life_ticks(),
            shadow_fox_coherence_decay_clean: default_shadow_fox_coherence_decay_clean(),
            shadow_fox_coherence_recovery_corrupt: default_shadow_fox_coherence_recovery_corrupt(),
            shadow_fox_coherence_decay_threshold: default_shadow_fox_coherence_decay_threshold(),
            shadow_fox_coherence_recovery_threshold:
                default_shadow_fox_coherence_recovery_threshold(),
            shadow_fox_coherence_dissolution_threshold:
                default_shadow_fox_coherence_dissolution_threshold(),
            shadow_fox_motivation_tick_cadence: default_shadow_fox_motivation_tick_cadence(),
            shadow_fox_motivation_softmax_temp: default_shadow_fox_motivation_softmax_temp(),
            shadow_fox_motivation_jitter: default_shadow_fox_motivation_jitter(),
            shadow_fox_resonance_weight: default_shadow_fox_resonance_weight(),
            shadow_fox_entropy_distance_scale: default_shadow_fox_entropy_distance_scale(),
            shadow_fox_haunting_edge_distance: default_shadow_fox_haunting_edge_distance(),
            shadow_fox_reconstituting_recovery_multiplier:
                default_shadow_fox_reconstituting_recovery_multiplier(),
            shadow_fox_motivation_scan_radius: default_shadow_fox_motivation_scan_radius(),
            shadow_fox_motivation_min_pressure: default_shadow_fox_motivation_min_pressure(),
            shadow_fox_dread_isolation_radius: default_shadow_fox_dread_isolation_radius(),
            shadow_fox_dread_group_threshold: default_shadow_fox_dread_group_threshold(),
            shadow_fox_dread_group_suppression: default_shadow_fox_dread_group_suppression(),
            shadow_fox_haunting_drain_radius: default_shadow_fox_haunting_drain_radius(),
            shadow_fox_haunting_mood_drain: default_shadow_fox_haunting_mood_drain(),
            shadow_fox_haunting_safety_drain: default_shadow_fox_haunting_safety_drain(),
            shadow_fox_haunting_feature_cadence: default_shadow_fox_haunting_feature_cadence(),
            shadow_fox_haunting_escalation_ticks: default_shadow_fox_haunting_escalation_ticks(),
            shadow_fox_frontier_detection_range: default_shadow_fox_frontier_detection_range(),
            shadow_fox_satiation_at_spawn: default_shadow_fox_satiation_at_spawn(),
            shadow_fox_satiation_gain_ambush: default_shadow_fox_satiation_gain_ambush(),
            shadow_fox_satiation_gain_prey_kill: default_shadow_fox_satiation_gain_prey_kill(),
            shadow_fox_satiation_decay_per_cadence: default_shadow_fox_satiation_decay_per_cadence(
            ),
            shadow_fox_stalk_satiation_threshold: default_shadow_fox_stalk_satiation_threshold(),
            shadow_fox_hunger_drive_weight: default_shadow_fox_hunger_drive_weight(),
            shadow_fox_retreat_arrival_radius: default_shadow_fox_retreat_arrival_radius(),
            shadow_fox_retreat_arrive_slow_radius: default_shadow_fox_retreat_arrive_slow_radius(),
            shadow_fox_den_reuse_radius: default_shadow_fox_den_reuse_radius(),
            shadow_fox_kill_site_memory_ticks: default_shadow_fox_kill_site_memory_ticks(),
            shadow_fox_kill_site_avoid_radius: default_shadow_fox_kill_site_avoid_radius(),
        }
    }
}

fn default_shadow_fox_kill_site_memory_ticks() -> u64 {
    20_000
}

fn default_shadow_fox_kill_site_avoid_radius() -> f32 {
    6.0
}

fn default_shadow_fox_retreat_arrival_radius() -> f32 {
    1.5
}

fn default_shadow_fox_retreat_arrive_slow_radius() -> f32 {
    2.0
}

fn default_shadow_fox_den_reuse_radius() -> f32 {
    8.0
}

fn default_shadow_fox_satiation_at_spawn() -> f32 {
    0.5
}

fn default_shadow_fox_satiation_gain_ambush() -> f32 {
    0.8
}

fn default_shadow_fox_satiation_gain_prey_kill() -> f32 {
    0.4
}

fn default_shadow_fox_satiation_decay_per_cadence() -> f32 {
    0.001
}

fn default_shadow_fox_stalk_satiation_threshold() -> f32 {
    0.7
}

fn default_shadow_fox_hunger_drive_weight() -> f32 {
    0.10
}

fn default_shadow_fox_motivation_tick_cadence() -> u64 {
    16
}

fn default_shadow_fox_motivation_softmax_temp() -> f32 {
    0.3
}

fn default_shadow_fox_motivation_jitter() -> f32 {
    0.05
}

fn default_shadow_fox_resonance_weight() -> f32 {
    0.15
}

fn default_shadow_fox_entropy_distance_scale() -> f32 {
    0.1
}

fn default_shadow_fox_haunting_edge_distance() -> f32 {
    4.0
}

fn default_shadow_fox_reconstituting_recovery_multiplier() -> f32 {
    3.0
}

fn default_shadow_fox_motivation_scan_radius() -> f32 {
    12.0
}

fn default_shadow_fox_motivation_min_pressure() -> f32 {
    0.05
}

fn default_shadow_fox_dread_isolation_radius() -> f32 {
    8.0
}

fn default_shadow_fox_dread_group_threshold() -> u32 {
    2
}

fn default_shadow_fox_dread_group_suppression() -> f32 {
    0.2
}

fn default_shadow_fox_haunting_drain_radius() -> f32 {
    5.0
}

fn default_shadow_fox_haunting_mood_drain() -> f32 {
    0.002
}

fn default_shadow_fox_haunting_safety_drain() -> f32 {
    0.003
}

fn default_shadow_fox_haunting_feature_cadence() -> u64 {
    30
}

fn default_shadow_fox_haunting_escalation_ticks() -> u64 {
    30
}

fn default_shadow_fox_frontier_detection_range() -> f32 {
    12.0
}

fn default_shadow_fox_coherence_decay_clean() -> f32 {
    0.0005
}

fn default_shadow_fox_coherence_recovery_corrupt() -> f32 {
    0.002
}

fn default_shadow_fox_coherence_decay_threshold() -> f32 {
    0.2
}

fn default_shadow_fox_coherence_recovery_threshold() -> f32 {
    0.5
}

fn default_shadow_fox_coherence_dissolution_threshold() -> f32 {
    0.0
}

fn default_fox_approach_corridor_deposit_per_tick() -> f32 {
    0.05
}

fn default_fox_approach_corridor_half_life_ticks() -> u32 {
    20_000
}

/// 260: `WardCoverageMap` intensity above which shadow foxes flip
/// into ward-avoidance. Calibrated so the new substrate-visible
/// trigger fires at roughly the same boundary as the pre-260
/// hardcoded `pre-492 manhattan_distance <= repel_radius * 3.0` check
/// (default ward strength 1.0, repel_radius ≈ 9, multiplier 3 →
/// fox flinches inside ~27 tiles; 0.15 coverage corresponds to a
/// similar gradient point on the linear falloff).
fn default_shadow_fox_ward_avoid_threshold() -> f32 {
    0.15
}

fn default_carcass_scent_deposit_per_tick() -> f32 {
    0.1
}

fn default_carcass_scent_decay_rate() -> RatePerDay {
    RatePerDay::new(0.5)
}

/// 260: small base deposit so every adult cat radiates a steady-state
/// scent residue. Calibrated as `~10%` of the pre-260 patrol-only
/// rate, since every cat now emits every tick. Steady-state intensity
/// is `base / per_tick_decay` — at decay_rate `0.1/day` (≈1e-4/tick at
/// default time scale) and `base = 0.01`, a base-only tile plateaus
/// near 1.0 over a long horizon; saturate-and-decay is the dominant
/// behavior, so the gradient still reflects activity.
fn default_cat_scent_base_deposit() -> f32 {
    0.01
}

/// 260: patrol/fight/explore bonus on top of `cat_scent_base_deposit`.
/// `base + bonus = 0.10`, preserving the pre-260 `scent_deposit` peak
/// intensity for active-territorial actions and making the rename a
/// near-no-op for seed-42 behavioral baselines.
fn default_cat_scent_action_bonus() -> f32 {
    0.09
}

// ---------- FoxEcologyConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FoxEcologyConstants {
    // --- Needs ---
    /// Hunger increase per in-game day when not satiated (~1 season to
    /// starve from full at the default 1000 ticks/day scale)
    /// (ticket 033 Phase 4: was `hunger_decay_per_tick: f32`).
    #[serde(alias = "hunger_decay_per_tick")]
    pub hunger_decay_rate: RatePerDay,
    /// Duration of satiation after killing small prey (~1 day at default)
    /// (ticket 033 Phase 4).
    #[serde(alias = "satiation_after_prey_kill")]
    pub satiation_after_prey_kill: DurationDays,
    /// Duration of satiation after raiding colony stores (Phase 4).
    #[serde(alias = "satiation_after_store_raid")]
    pub satiation_after_store_raid: DurationDays,
    /// Duration of satiation after scavenging carrion/scraps (Phase 4).
    #[serde(alias = "satiation_after_scavenge")]
    pub satiation_after_scavenge: DurationDays,

    // --- Risk assessment ---
    /// Distance at which a fox actively avoids a healthy adult cat.
    pub cat_avoidance_range: f32,
    /// Hunger level above which fox considers attacking risky targets.
    pub desperate_hunger_threshold: f32,
    /// Distance from den at which fox attacks ANY intruder (when cubs present).
    pub den_defense_range: f32,
    /// Health fraction below which fox flees.
    pub flee_health_threshold: f32,
    /// Number of nearby cats that triggers fox flee response.
    pub outnumbered_flee_count: usize,

    // --- Confrontation ---
    /// Maximum duration a standoff lasts before auto-resolving (Phase 4).
    #[serde(alias = "standoff_max_ticks")]
    pub standoff_max_duration: DurationDays,
    /// Per-tick chance a standoff escalates to physical contact.
    pub standoff_escalation_chance: f32,
    /// Chance fox retreats when standoff ends without escalation.
    pub standoff_fox_retreat_chance: f32,
    /// Damage dealt to both parties when standoff escalates (minor scratch).
    pub standoff_damage_on_escalation: f32,
    /// Escalation chance for den defense confrontations (higher than normal).
    pub den_defense_escalation_chance: f32,

    // --- Lifecycle ---
    /// Duration a fox stays in Cub stage (~1 season) (Phase 4).
    #[serde(alias = "cub_duration_ticks")]
    pub cub_duration: DurationSeasons,
    /// Duration a fox stays in Juvenile stage (~2 seasons) (Phase 4).
    #[serde(alias = "juvenile_duration_ticks")]
    pub juvenile_duration: DurationSeasons,
    /// Maximum age before a fox dies of old age (~4 years / 16 seasons)
    /// (Phase 4).
    #[serde(alias = "max_age_ticks")]
    pub max_age: DurationSeasons,
    /// Minimum litter size during breeding.
    pub litter_size_min: u32,
    /// Maximum litter size during breeding.
    pub litter_size_max: u32,
    /// Per-tick mortality chance for dispersing juveniles.
    pub juvenile_mortality_per_tick: f32,
    /// Per-tick mortality chance for elder foxes.
    pub elder_mortality_per_tick: f32,
    /// Sustained hunger=1.0 duration before starvation death (Phase 4).
    #[serde(alias = "starvation_death_ticks")]
    pub starvation_death_duration: DurationDays,

    // --- Territory ---
    /// Default territory radius from den in tiles.
    pub territory_radius: f32,
    /// Scent amount deposited per marking event.
    pub scent_deposit: f32,
    /// Global scent decay on `FoxScentMap` (and the symmetric
    /// `CatScentMap` at `disposition.rs:cat_scent_tick`),
    /// expressed per in-game day.
    ///
    /// Fox scent is a *territorial mark* — long persistence (days),
    /// in contrast to `PreyConstants::scent_decay_rate`'s
    /// activity-trail semantics. `RatePerDay::new(0.1)` decays a
    /// peak (1.0) bucket to zero over ~10 in-game days, enough for a
    /// claimed territory to register against passing prey, cats, and
    /// rival foxes. Pre-ticket-033 value was `0.0001/tick = 0.1/day`,
    /// numerically identical at the default scale; the typed wrapper
    /// makes the unit explicit and lets the peg control downstream.
    #[serde(rename = "scent_decay_rate", alias = "scent_decay_per_tick")]
    pub scent_decay_rate: RatePerDay,
    /// Hard cap on fox dens in the world.
    pub max_dens: usize,
    /// Minimum tile distance between fox dens.
    pub min_den_spacing: f32,

    // --- Store raiding ---
    /// Distance at which fox can detect colony food stores.
    pub raid_smell_range: f32,
    /// Food units stolen per successful raid.
    pub raid_food_stolen: f32,
    /// Cat proximity to stores that deters a raid.
    pub guard_deterrent_range: f32,

    // --- Ward / cat scent ---
    /// Hunger threshold above which a fox pushes through wards anyway.
    pub ward_hunger_override_threshold: f32,
    /// `CatScentMap` bucket value above which foxes avoid the area
    /// (ticket 260: was `cat_presence_avoidance_threshold` before the
    /// rename, semantics unchanged).
    pub cat_scent_avoidance_threshold: f32,
    /// Per-tick scent every adult cat deposits at its position
    /// (steady-state component of `CatScentMap`). With the default
    /// decay rate this plateaus at roughly `base / per_tick_decay` —
    /// well below the 1.0 ceiling, but enough to register against
    /// `cat_scent_avoidance_threshold` in dense colony interiors.
    /// Ticket 260: replaces the patrol-only reuse of `scent_deposit`
    /// with a two-rate (base + action bonus) model.
    #[serde(default = "default_cat_scent_base_deposit")]
    pub cat_scent_base_deposit: f32,
    /// Additional per-tick scent a cat in `Patrol`/`Fight`/`Explore`
    /// deposits on top of `cat_scent_base_deposit`. Calibrated so
    /// `base + bonus ≈ scent_deposit (0.1)` for backward compatibility
    /// with the pre-260 patrol-only peak intensity.
    #[serde(default = "default_cat_scent_action_bonus")]
    pub cat_scent_action_bonus: f32,

    // --- Cooldowns ---
    /// Cooldown duration after any confrontation/raid/hunt action
    /// (ticket 033 Phase 4 — was raw `u64` ticks).
    pub post_action_cooldown: DurationDays,

    // --- Initial spawn ---
    /// Minimum fox dens placed during world gen.
    pub initial_den_count_min: u32,
    /// Maximum fox dens placed during world gen.
    pub initial_den_count_max: u32,
    /// Minimum distance from colony center for initial den placement.
    pub initial_den_min_distance: f32,
}

impl Default for FoxEcologyConstants {
    fn default() -> Self {
        Self {
            // Needs — matched to cat hunger_decay (0.0001/tick = 0.1/day)
            hunger_decay_rate: RatePerDay::new(0.1),
            // 1000 ticks → 1 day, 800 → 0.8 day, 500 → 0.5 day at default scale.
            satiation_after_prey_kill: DurationDays::new(1.0),
            satiation_after_store_raid: DurationDays::new(0.8),
            satiation_after_scavenge: DurationDays::new(0.5),

            // Risk assessment
            cat_avoidance_range: 6.0,
            desperate_hunger_threshold: 0.9,
            den_defense_range: 5.0,
            flee_health_threshold: 0.4,
            outnumbered_flee_count: 2,

            // Confrontation
            // 15 ticks ÷ 1000 = 0.015 days at default scale.
            standoff_max_duration: DurationDays::new(0.015),
            standoff_escalation_chance: 0.05,
            standoff_fox_retreat_chance: 0.7,
            standoff_damage_on_escalation: 0.05,
            den_defense_escalation_chance: 0.15,

            // Lifecycle — 20000 ticks/season at default; convert ticks → seasons.
            cub_duration: DurationSeasons::new(1.0),
            juvenile_duration: DurationSeasons::new(2.0),
            max_age: DurationSeasons::new(16.0),
            litter_size_min: 3,
            litter_size_max: 5,
            juvenile_mortality_per_tick: 0.000002,
            elder_mortality_per_tick: 0.000005,
            // 2000 ticks ÷ 1000 = 2 days at default scale.
            starvation_death_duration: DurationDays::new(2.0),

            // Territory
            territory_radius: 18.0,
            scent_deposit: 0.1,
            scent_decay_rate: RatePerDay::new(0.1),
            max_dens: 3,
            min_den_spacing: 25.0,

            // Store raiding
            raid_smell_range: 12.0,
            raid_food_stolen: 2.0,
            guard_deterrent_range: 5.0,

            // Ward / cat scent
            ward_hunger_override_threshold: 0.7,
            cat_scent_avoidance_threshold: 0.3,
            cat_scent_base_deposit: default_cat_scent_base_deposit(),
            cat_scent_action_bonus: default_cat_scent_action_bonus(),

            // Cooldowns
            // Reduced from 2000 to 800 (~0.8 sim days) — 2000 was suppressing
            // most fox activity; foxes spent the bulk of each day frozen in
            // Resting. Shorter cooldown keeps downstream features (FoxStandoff,
            // FoxAvoidedCat, etc.) firing regularly.
            post_action_cooldown: DurationDays::new(0.8),

            // Initial spawn
            initial_den_count_min: 1,
            initial_den_count_max: 2,
            initial_den_min_distance: 15.0,
        }
    }
}

// ---------- HawkEcologyConstants (ticket 025 Phase 2) ----------

/// Tuning knobs for the hawk GOAP loop. Hawks are aerial survival-tier
/// predators with no territory/breeding tier — the struct is simpler
/// than [`FoxEcologyConstants`]. Default values in
/// [`HawkEcologyConstants::default`] mirror ticket 025 §9.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HawkEcologyConstants {
    /// Hunger increase per in-game day when not satiated.
    pub hunger_decay_rate: RatePerDay,
    /// Satiation duration after a successful dive.
    pub satiation_after_dive_kill: DurationDays,
    /// Health fraction below which hawk switches to Fleeing.
    pub flee_health_threshold: f32,
    /// Tile range at which a hawk avoids healthy adult cats.
    pub cat_avoidance_range: f32,
    /// Tile range from which hawk initiates a dive.
    pub dive_range: f32,
    /// Detection range for spotting prey from altitude.
    pub detection_range: f32,
    /// Tiles searched for a Perch zone.
    pub perch_search_radius: f32,
    /// Ticks the hawk perches before advancing the Resting plan.
    pub rest_duration_ticks: u64,
    /// Sustained starvation before death.
    pub starvation_death_duration: DurationDays,
    /// Cooldown after dive / flee.
    pub post_action_cooldown: DurationDays,
    /// Softmax temperature for disposition selection (mirror
    /// `fox_softmax_temperature`).
    pub softmax_temperature: f32,
    /// Cats nearby that trigger flee response.
    pub outnumbered_flee_count: usize,
}

impl Default for HawkEcologyConstants {
    fn default() -> Self {
        Self {
            hunger_decay_rate: RatePerDay::new(0.15),
            satiation_after_dive_kill: DurationDays::new(0.7),
            flee_health_threshold: 0.4,
            cat_avoidance_range: 4.0,
            dive_range: 6.0,
            detection_range: 12.0,
            perch_search_radius: 15.0,
            rest_duration_ticks: 200,
            starvation_death_duration: DurationDays::new(2.0),
            post_action_cooldown: DurationDays::new(0.4),
            softmax_temperature: 0.15,
            outnumbered_flee_count: 2,
        }
    }
}

// ---------- SnakeEcologyConstants (ticket 025 Phase 2) ----------

/// Tuning knobs for the snake GOAP loop. Adds thermoregulation (Maslow
/// tier 2: `warmth`) on top of the survival tier. Default values mirror
/// ticket 025 §9.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SnakeEcologyConstants {
    /// Hunger decay; snakes are slow-metabolism, slower than foxes.
    pub hunger_decay_rate: RatePerDay,
    /// Warmth decay rate when off warm terrain.
    pub warmth_decay_rate: RatePerDay,
    /// Warmth set after a complete Bask.
    pub bask_warmth_restore: f32,
    /// Bask duration before the warmth top-up applies.
    pub bask_duration_ticks: u64,
    /// Ticks the snake spends settling into ambush before the
    /// `SnakeAmbushed` witness fires.
    pub ambush_settle_ticks: u64,
    /// Satiation duration after a successful strike.
    pub satiation_after_strike_kill: DurationDays,
    /// Health fraction below which snake switches to Fleeing.
    pub flee_health_threshold: f32,
    /// Tile range for strike (adjacency by default).
    pub strike_range: f32,
    /// Detection range for sensing prey from cover.
    pub detection_range: f32,
    /// Search radius for Cover zone.
    pub cover_search_radius: f32,
    /// Long; snakes survive lean periods.
    pub starvation_death_duration: DurationDays,
    /// Post-strike / post-bask cooldown.
    pub post_action_cooldown: DurationDays,
    /// Softmax temperature for disposition selection.
    pub softmax_temperature: f32,
    /// Warmth below which snake forces Basking disposition.
    pub cold_threshold: f32,
}

impl Default for SnakeEcologyConstants {
    fn default() -> Self {
        Self {
            hunger_decay_rate: RatePerDay::new(0.05),
            warmth_decay_rate: RatePerDay::new(0.4),
            bask_warmth_restore: 1.0,
            bask_duration_ticks: 180,
            ambush_settle_ticks: 40,
            satiation_after_strike_kill: DurationDays::new(2.0),
            flee_health_threshold: 0.5,
            strike_range: 1.0,
            detection_range: 4.0,
            cover_search_radius: 8.0,
            starvation_death_duration: DurationDays::new(5.0),
            post_action_cooldown: DurationDays::new(0.5),
            softmax_temperature: 0.15,
            cold_threshold: 0.3,
        }
    }
}

// ---------- FateConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FateConstants {
    /// Maximum rate at which `assign_fated_connections` issues new
    /// fated love / rival assignments. Acts as a minimum-gap throttle —
    /// once an assignment fires, no further fate event will land for
    /// `assign_cooldown.ticks(&time_scale)` ticks. Pre-ticket-033
    /// value was `50` raw ticks (= 20×/day at default scale), which
    /// burst all colony fate events in the first ~1 in-game day.
    /// Once-per-day matches the "fate trickles in" narrative intent.
    #[serde(alias = "assign_cooldown")]
    pub assign_cooldown: IntervalPerDay,
    pub love_zodiac_score: f32,
    pub love_personality_weight: f32,
    pub love_jitter: f32,
    pub rival_zodiac_score: f32,
    pub rival_personality_weight: f32,
    pub rival_jitter: f32,
    pub love_awaken_distance: f32,
    pub rival_awaken_distance: f32,
}

impl Default for FateConstants {
    fn default() -> Self {
        Self {
            assign_cooldown: IntervalPerDay::new(1.0),
            love_zodiac_score: 0.5,
            love_personality_weight: 0.3,
            love_jitter: 0.05,
            rival_zodiac_score: 0.5,
            rival_personality_weight: 0.3,
            rival_jitter: 0.05,
            love_awaken_distance: 5.0,
            rival_awaken_distance: 10.0,
        }
    }
}

// ---------- CoordinationConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CoordinationConstants {
    pub social_weight_familiarity_scale: f32,
    pub social_weight_event_scale: f32,
    /// Cadence at which `evaluate_coordinators` re-scores all cats and
    /// promotes/demotes the Coordinator role. Coordinator promotion is
    /// a slow process — once per in-game day matches the timescale.
    /// Pre-ticket-033 value was `100` (raw ticks; legacy from the
    /// 100-ticks/day era), which silently behaved as 10×/day after the
    /// 2026-04-10 overhaul.
    #[serde(alias = "evaluate_interval")]
    pub evaluate_interval: IntervalPerDay,
    pub small_colony_threshold: usize,
    pub promotion_threshold: f32,
    pub ambition_bonus: f32,
    pub assess_interval: u64,
    pub food_threshold_base: f32,
    pub food_threshold_hunting_scale: f32,
    pub food_threshold_foraging_scale: f32,
    pub building_threshold_base: f32,
    pub building_threshold_building_scale: f32,
    pub threat_fight_priority: f32,
    pub threat_patrol_priority: f32,
    pub injury_priority_per_cat: f32,
    pub ward_set_priority: f32,
    pub ward_avg_strength_low_threshold: f32,
    pub directive_expiry_ticks: u64,
    pub attentiveness_diligence_weight: f32,
    pub attentiveness_ambition_weight: f32,
    pub attentiveness_impatience_weight: f32,
    pub build_pressure_attentiveness_threshold_scale: f32,
    pub build_pressure_farming_food_threshold: f32,
    pub build_pressure_workshop_min_cats: usize,
    /// Minimum raw food items in Stores before cooking-pressure starts
    /// accumulating. Below this the colony hasn't built enough surplus to
    /// justify a Kitchen.
    #[serde(default = "default_build_pressure_cooking_min_raw_food")]
    pub build_pressure_cooking_min_raw_food: usize,
    /// 367 Commit 8 — minimum raw food items in Stores before
    /// preservation pressure (Drying Rack / Smoking Rack) starts
    /// accumulating. See `default_build_pressure_preservation_min_raw_food`.
    #[serde(default = "default_build_pressure_preservation_min_raw_food")]
    pub build_pressure_preservation_min_raw_food: usize,
    /// 367 Commit 8 — multiplier on preservation-pressure accumulation
    /// rate. Mirrors `cooking_pressure_multiplier`.
    #[serde(default = "default_preservation_pressure_multiplier")]
    pub preservation_pressure_multiplier: f32,
    /// 369 — minimum hide items in Stores before tanning-pressure
    /// (TanningFrame BuildPressure channel) starts accumulating.
    /// Mirrors `build_pressure_preservation_min_raw_food`. See
    /// `default_build_pressure_tanning_min_hides`.
    #[serde(default = "default_build_pressure_tanning_min_hides")]
    pub build_pressure_tanning_min_hides: usize,
    /// 369 — multiplier on tanning-pressure accumulation rate.
    /// Mirrors `preservation_pressure_multiplier`.
    #[serde(default = "default_tanning_pressure_multiplier")]
    pub tanning_pressure_multiplier: f32,
    /// Priority of a Cook directive when a Kitchen is functional and raw food
    /// is available. Kept below Hunt/Fight (~0.7+) so cooking doesn't crowd
    /// out survival directives.
    #[serde(default = "default_cook_directive_priority")]
    pub cook_directive_priority: f32,
    /// Scales the effect of unmet-demand ledger entries on BuildPressure
    /// accumulation. `pressure += rate * (1 + unmet * amplifier)` — 2.0
    /// means a single frustrated-cat increment doubles the pressure rise
    /// on that cycle. Kept moderate so a few attempts escalate, but the
    /// coordinator still requires the underlying conditions (Hearth,
    /// raw food) to issue a build.
    #[serde(default = "default_unmet_demand_amplifier")]
    pub unmet_demand_amplifier: f32,
    pub wildlife_breach_range: f32,
    pub build_directive_priority_base: f32,
    pub build_directive_priority_building_scale: f32,
    pub forage_critical_multiplier: f32,
    pub build_repair_priority_base: f32,
    pub build_repair_priority_building_scale: f32,
    /// Range from colony buildings within which wildlife counts as a threat.
    pub threat_proximity_range: f32,
    /// Priority for targeted patrol toward an incursion point.
    pub threat_patrol_targeted_priority: f32,
    /// Range from a building at which wildlife triggers a Fight directive (breach).
    pub colony_breach_range: f32,
    /// Radius (manhattan) to check fox scent near colony center for preemptive patrol.
    pub preemptive_patrol_scent_radius: f32,
    /// Scent level threshold above which a preemptive patrol is issued.
    pub preemptive_patrol_scent_threshold: f32,
    /// Priority for preemptive patrol issued from fox scent detection.
    pub preemptive_patrol_priority: f32,
    /// Multiplier on build pressure accumulation rate when no Stores building exists.
    #[serde(default = "default_no_store_pressure_multiplier")]
    pub no_store_pressure_multiplier: f32,
    /// Multiplier on Kitchen build-pressure accumulation rate. Raised above
    /// 1.0 to push Kitchen up the BuildPressure priority queue so the
    /// cooking buffer activates before food supply collapses.
    #[serde(default = "default_cooking_pressure_multiplier")]
    pub cooking_pressure_multiplier: f32,
    /// Foundational "phase unlock" multiplier for Kitchen pressure when
    /// no Kitchen exists yet. Mirrors `no_store_pressure_multiplier` — a
    /// colony without a Kitchen can't enter the Cook loop at all, so the
    /// first Kitchen deserves a disproportionate push. Once one exists,
    /// the `cooking_pressure_multiplier` path takes over for incremental
    /// expansion.
    #[serde(default = "default_no_kitchen_pressure_multiplier")]
    pub no_kitchen_pressure_multiplier: f32,
    /// Priority of the "work on the existing construction site" directive
    /// the coordinator pushes whenever an unfinished site exists. Above
    /// `urgent_directive_priority_threshold` so `dispatch_urgent_directives`
    /// assigns it to cats directly, boosting their Build scoring via
    /// the standard ActiveDirective bonus. Without this, blueprint-carrying
    /// Build directives get consumed by site-spawn and never propagate
    /// to cats — sites languish unbuilt.
    #[serde(default = "default_construct_site_directive_priority")]
    pub construct_site_directive_priority: f32,
    /// Radius (tiles) around colony center that coordinators sweep for
    /// corruption hotspots.
    #[serde(default = "default_corruption_search_radius")]
    pub corruption_search_radius: f32,
    /// Sample-step size for the corruption sweep (every Nth tile).
    #[serde(default = "default_corruption_search_step")]
    pub corruption_search_step: i32,
    /// Tile corruption level above which a Cleanse directive is issued.
    #[serde(default = "default_corruption_alarm_threshold")]
    pub corruption_alarm_threshold: f32,
    /// Cleanse directive priority = corruption * this + magic_skill * magic_scale.
    #[serde(default = "default_corruption_directive_priority_scale")]
    pub corruption_directive_priority_scale: f32,
    /// Magic-skill contribution to cleanse directive priority.
    #[serde(default = "default_corruption_directive_magic_scale")]
    pub corruption_directive_magic_scale: f32,
    /// Base priority for HarvestCarcass directives.
    #[serde(default = "default_carcass_directive_priority_base")]
    pub carcass_directive_priority_base: f32,
    /// Herbcraft-skill contribution to carcass directive priority.
    #[serde(default = "default_carcass_directive_herbcraft_scale")]
    pub carcass_directive_herbcraft_scale: f32,
    /// Priority threshold above which a directive is dispatched directly
    /// to the best-skilled cat (skipping the physical walk-to-cat delivery).
    #[serde(default = "default_urgent_directive_priority_threshold")]
    pub urgent_directive_priority_threshold: f32,
    /// Maximum range in tiles for urgent directive auto-dispatch.
    #[serde(default = "default_urgent_dispatch_range")]
    pub urgent_dispatch_range: f32,
    /// Tiles around colony center within which a shadow-fox triggers posse
    /// assembly. Large enough to catch foxes before they ambush.
    #[serde(default = "default_posse_alarm_range")]
    pub posse_alarm_range: f32,
    /// How many cats the coordinator summons for a posse. 3-4 is the sweet
    /// spot: enough for ally damage bonuses, not so many the colony is
    /// disarmed defensively.
    #[serde(default = "default_posse_size")]
    pub posse_size: usize,
    /// Priority of posse Fight directives. Higher than ward-set so bold
    /// cats drop ward duty to engage the threat.
    #[serde(default = "default_posse_priority")]
    pub posse_priority: f32,
    /// 487 — magnitude applied to a colony-self directive in place of the
    /// elected coordinator's `social_weight` (which would multiply the
    /// per-cat bonus in `goap.rs`'s directive-bonus formula). Tuned softer
    /// than a real coordinator's social weight so colony-self directives
    /// nudge day-1 founders toward Forage / Build / Cook without
    /// overwhelming individual scoring. The same multiplicative shape
    /// preserves how personality (diligence, independence, stubbornness)
    /// continues to modulate compliance.
    #[serde(default = "default_colony_self_directive_weight")]
    pub colony_self_directive_weight: f32,
    /// 487 — multiplicative bonus on `evaluate_coordinators` for cats whose
    /// `ColonyAlignmentScore.recent_aligned_actions` is high. Wraps the
    /// existing score in `(1 + alignment_score * weight)` so it stacks
    /// with personality rather than overwhelming the social-weight × trait
    /// pillars. Set to 0.0 to disable the emergent-leader feedback loop.
    #[serde(default = "default_alignment_skill_weight")]
    pub alignment_skill_weight: f32,
    /// 487 — per-tick EWMA decay applied to `ColonyAlignmentScore`. Tuned
    /// for a half-life of roughly one season so emergent leadership is
    /// responsive to a colony's current era without locking in a first-
    /// mover whose alignment was earned long ago.
    #[serde(default = "default_alignment_decay_per_tick")]
    pub alignment_decay_per_tick: f32,
    /// 487 — increment added to `ColonyAlignmentScore.recent_aligned_actions`
    /// when a cat completes an Action that matches a pressing need in the
    /// current `ColonyAssessment`. Combined with the per-tick decay, this
    /// sets the steady-state score for a consistently-aligned cat.
    #[serde(default = "default_alignment_match_increment")]
    pub alignment_match_increment: f32,
}

fn default_corruption_search_radius() -> f32 {
    20.0
}
fn default_corruption_search_step() -> i32 {
    3
}
fn default_corruption_alarm_threshold() -> f32 {
    0.15
}
fn default_corruption_directive_priority_scale() -> f32 {
    1.0
}
fn default_corruption_directive_magic_scale() -> f32 {
    0.3
}
fn default_carcass_directive_priority_base() -> f32 {
    // Raised from 0.55 → 0.80: carcasses emit corruption to their tile every
    // tick (~0.002) and are the primary source of colony-threatening decay.
    // At 0.55 base + 0.2*herbcraft, no realistic skill level reached the 0.75
    // auto-dispatch threshold, so CarcassHarvested stayed at 0. Emergency
    // removal of corruption sources warrants immediate dispatch.
    0.80
}
fn default_carcass_directive_herbcraft_scale() -> f32 {
    0.2
}
fn default_urgent_directive_priority_threshold() -> f32 {
    // Threshold tuning: 0.5 caused corruption response to dominate everything;
    // cats abandoned hunting/foraging/ward-setting to cleanse. 0.75 reserves
    // auto-dispatch for genuine emergencies (severe corruption, siege) while
    // letting normal directives flow through physical coordinator delivery.
    0.75
}
fn default_urgent_dispatch_range() -> f32 {
    50.0
}

fn default_posse_alarm_range() -> f32 {
    20.0
}

fn default_posse_size() -> usize {
    3
}

fn default_posse_priority() -> f32 {
    0.9
}

fn default_colony_self_directive_weight() -> f32 {
    // Softer than a typical coordinator's social weight (~0.6–1.5 in
    // mid-game). 0.5 nudges day-1 founders toward colony-needed work
    // without the level of pull a charismatic elected coordinator
    // would apply once one emerges.
    0.5
}

fn default_alignment_skill_weight() -> f32 {
    // Multiplicative wrap: `score × (1 + alignment × 0.5)` lets a
    // consistently-aligned cat (steady-state alignment ≈ 1.0) bump
    // their election score by up to 1.5× — meaningful but not
    // overwhelming the social-weight × diligence × sociability
    // pillars.
    0.5
}

fn default_alignment_decay_per_tick() -> f32 {
    // Per-tick multiplicative decay. With a 20k-tick season this
    // gives a half-life of one season: `0.99965^20000 ≈ 0.10`. So a
    // cat who stops aligning loses most of their election credit
    // within a season, while sustained aligned behavior accumulates.
    0.99965
}

fn default_alignment_match_increment() -> f32 {
    // Per-tick boost applied while a cat's `CurrentAction` is a
    // colony-aligned action (Forage / Build / Cook / Hunt /
    // HerbcraftGather / etc). EWMA fixpoint with the matching decay
    // (0.99965) at this increment is 1.0 for a cat who spends every
    // tick on colony work — the saturating reference value that the
    // election bonus scales against. Cats who split time between
    // alignment-positive and alignment-neutral work settle at the
    // proportional steady-state (e.g. 30% aligned → score ≈ 0.3).
    0.00035
}

fn default_cooking_pressure_multiplier() -> f32 {
    1.5
}

fn default_no_store_pressure_multiplier() -> f32 {
    5.0
}

fn default_no_kitchen_pressure_multiplier() -> f32 {
    5.0
}

fn default_construct_site_directive_priority() -> f32 {
    0.85
}

fn default_survey_explore_radius() -> f32 {
    4.0
}

fn default_passive_explore_radius() -> f32 {
    2.0
}

fn default_explore_perception_radius() -> f32 {
    10.0
}

fn default_explore_satiation_threshold() -> f32 {
    0.15
}

fn default_social_satiation_threshold() -> f32 {
    0.85
}

// --- Iteration 2 of `docs/balance/acceptance-restoration.md` —
// receiver-side per-tick acceptance accumulators. Sized so that an
// uninterrupted action produces an acceptance bump comparable to
// the iteration-1 completion-witness magnitudes (groomed=0.08,
// fed=0.10), but cumulates across tick boundaries so that a
// preempted action still confers partial credit.

fn default_acceptance_per_groom_other_per_tick() -> f32 {
    // 0.0008 × 80-tick groom_other_duration ≈ 0.064 if uninterrupted —
    // close to iter-1's 0.08, but partial credit if preempted.
    0.0008
}

fn default_bury_ticks() -> u64 {
    // 035: brief — burial is a witnessed completion, not sustained
    // interaction. Mirrors mentor_duration (~12) more than
    // groom_other_duration (80).
    60
}

fn default_burial_sense_range() -> f32 {
    // 035: smaller than the 10-tile social-family ranges. Cats must
    // encounter the corpse to react; design intent is "burial is a
    // local response to a local death."
    8.0
}

fn default_bury_belonging_gain() -> f32 {
    // 035: small Belonging-tier lift. Mirrors the small social_warmth
    // gain from grooming completion.
    0.05
}

fn default_grave_anti_corruption_strength() -> f32 {
    // 035: foundation-only aura strength. Tuning lives in follow-on
    // ticket #5 (anti-corruption balance pass) once the kitten-rest-at-
    // grave chain (follow-on #4) lands and there's a consumer that
    // exercises this map. 0.05 is small enough that the
    // `WardCoverageMap`-shaped `[0, 1]` clamp won't saturate from a
    // few graves.
    0.05
}

fn default_grave_anti_corruption_radius() -> f32 {
    // 035: foundation-only aura radius. Smaller than ward radii so a
    // cluster of graves doesn't accidentally re-paper a corruption
    // hotspot before the balance-tuning pass.
    4.0
}

fn default_acceptance_per_feed_kitten_per_tick() -> f32 {
    // FeedKitten is short — 0.005/tick × ~10 ticks ≈ 0.05.
    0.005
}

fn default_acceptance_per_mentor_per_tick() -> f32 {
    // Mentor sessions are long; small per-tick keeps the apprentice
    // from saturating in a single session while still rewarding
    // sustained mentorship.
    0.0005
}

fn default_acceptance_per_cleanse_per_tick() -> f32 {
    // Cleanse is rare and high-signal — bigger per-tick.
    0.001
}

// --- New balance thread `docs/balance/respect-restoration.md` —
// witness-multiplier respect at chain completion.

fn default_respect_per_witness() -> f32 {
    // Iter 2 (batch 3): probing the bistability cliff. 0.0015 → 0.996,
    // 0.0003 → 0.982 — barely shifted mean despite 16× cut. The
    // disposition baselines (`respect_gain_*`) plus any nonzero witness
    // contribution stay above drain; .min(1.0) clips. Try 0.0001 to
    // probe further toward the cliff edge.
    // See docs/balance/respect-restoration.md.
    0.0001
}

fn default_respect_witness_radius() -> f32 {
    // Tile-domain radius. Mirrors hearth_effect_radius scale.
    5.0
}

fn default_respect_witness_cap() -> u32 {
    // Diminishing returns above 4 witnesses.
    4
}

// --- New balance thread `docs/balance/purpose-restoration.md` —
// per-action colony-positive purpose bumps.

fn default_purpose_per_colony_action() -> f32 {
    0.005
}

fn default_purpose_per_deposit() -> f32 {
    // Tangible asset added to colony pool — a clear contribution.
    0.02
}

fn default_purpose_per_ward_set() -> f32 {
    // Significant defensive contribution.
    0.03
}

fn default_purpose_per_directive_completed() -> f32 {
    // Explicit colony-coordinated work completed.
    0.04
}

fn default_purpose_per_build_tick() -> f32 {
    // High-cadence per-tick during construction.
    0.0003
}

// --- Iteration 2 of `docs/balance/mastery-restoration.md` —
// per-action mastery STUBS (placeholder for ticket 016 Phase 5's
// per-skill crafting/experience table). Magnitudes target a
// colony-mean mastery in the 0.3-0.5 band against the existing
// drain rate.

fn default_mastery_per_magic_success() -> f32 {
    0.015
}

fn default_mastery_per_successful_tend() -> f32 {
    0.005
}

fn default_mastery_per_build_tick() -> f32 {
    0.001
}

fn default_mastery_per_successful_cook() -> f32 {
    0.01
}

fn default_mastery_per_successful_hunt() -> f32 {
    0.02
}

impl Default for CoordinationConstants {
    fn default() -> Self {
        Self {
            social_weight_familiarity_scale: 0.5,
            social_weight_event_scale: 0.1,
            evaluate_interval: IntervalPerDay::new(1.0),
            small_colony_threshold: 6,
            promotion_threshold: 0.15,
            ambition_bonus: 0.3,
            assess_interval: 20,
            food_threshold_base: 0.5,
            food_threshold_hunting_scale: 0.1,
            food_threshold_foraging_scale: 0.1,
            building_threshold_base: 0.7,
            building_threshold_building_scale: 0.1,
            threat_fight_priority: 0.5,
            threat_patrol_priority: 0.5,
            injury_priority_per_cat: 0.3,
            ward_set_priority: 0.5,
            ward_avg_strength_low_threshold: 0.3,
            directive_expiry_ticks: 200,
            attentiveness_diligence_weight: 0.5,
            attentiveness_ambition_weight: 0.3,
            attentiveness_impatience_weight: 0.2,
            build_pressure_attentiveness_threshold_scale: 0.3,
            build_pressure_farming_food_threshold: 0.3,
            build_pressure_workshop_min_cats: 4,
            build_pressure_cooking_min_raw_food: default_build_pressure_cooking_min_raw_food(),
            build_pressure_preservation_min_raw_food:
                default_build_pressure_preservation_min_raw_food(),
            preservation_pressure_multiplier: default_preservation_pressure_multiplier(),
            build_pressure_tanning_min_hides: default_build_pressure_tanning_min_hides(),
            tanning_pressure_multiplier: default_tanning_pressure_multiplier(),
            cook_directive_priority: default_cook_directive_priority(),
            unmet_demand_amplifier: default_unmet_demand_amplifier(),
            wildlife_breach_range: 10.0,
            build_directive_priority_base: 0.5,
            build_directive_priority_building_scale: 0.2,
            forage_critical_multiplier: 0.8,
            build_repair_priority_base: 0.6,
            build_repair_priority_building_scale: 0.1,
            threat_proximity_range: 20.0,
            threat_patrol_targeted_priority: 0.6,
            colony_breach_range: 8.0,
            preemptive_patrol_scent_radius: 25.0,
            preemptive_patrol_scent_threshold: 0.3,
            preemptive_patrol_priority: 0.4,
            no_store_pressure_multiplier: 5.0,
            cooking_pressure_multiplier: default_cooking_pressure_multiplier(),
            no_kitchen_pressure_multiplier: default_no_kitchen_pressure_multiplier(),
            construct_site_directive_priority: default_construct_site_directive_priority(),
            corruption_search_radius: default_corruption_search_radius(),
            corruption_search_step: default_corruption_search_step(),
            corruption_alarm_threshold: default_corruption_alarm_threshold(),
            corruption_directive_priority_scale: default_corruption_directive_priority_scale(),
            corruption_directive_magic_scale: default_corruption_directive_magic_scale(),
            carcass_directive_priority_base: default_carcass_directive_priority_base(),
            carcass_directive_herbcraft_scale: default_carcass_directive_herbcraft_scale(),
            urgent_directive_priority_threshold: default_urgent_directive_priority_threshold(),
            urgent_dispatch_range: default_urgent_dispatch_range(),
            posse_alarm_range: default_posse_alarm_range(),
            posse_size: default_posse_size(),
            posse_priority: default_posse_priority(),
            colony_self_directive_weight: default_colony_self_directive_weight(),
            alignment_skill_weight: default_alignment_skill_weight(),
            alignment_decay_per_tick: default_alignment_decay_per_tick(),
            alignment_match_increment: default_alignment_match_increment(),
        }
    }
}

// ---------- AspirationConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AspirationConstants {
    pub zodiac_affinity_bonus: f32,
    pub personality_alignment_weight: f32,
    pub experience_memory_scale: f32,
    pub experience_secondary_scale: f32,
    pub experience_cap: f32,
    pub scoring_jitter: f32,
    pub strong_personality_threshold: f32,
    pub weak_personality_threshold: f32,
    /// Cadence at which the second-slot population probe runs in
    /// `tick_aspirations`. Aspiration unlocks are a slow process —
    /// once per in-game day matches the timescale. Pre-ticket-033
    /// value was `100` (raw ticks); silently behaved as 10×/day after
    /// the 2026-04-10 overhaul.
    #[serde(alias = "second_slot_check_interval")]
    pub second_slot_check_interval: IntervalPerDay,
    pub stagnation_ticks: u64,
    pub min_alignment: f32,
    pub milestone_mood_bonus: f32,
    pub milestone_mood_ticks: u64,
    pub milestone_mastery_gain: f32,
    pub milestone_purpose_gain: f32,
    pub chain_complete_mood_bonus: f32,
    pub chain_complete_mood_ticks: u64,
    pub chain_complete_purpose_gain: f32,
    /// §7.7.d (ticket 055) hysteresis ENTER band. Mood drift-threshold
    /// detection enters the misaligned state when
    /// `Mood::valence < AspirationChain::expected_valence_target −
    /// drift_enter_margin`. Author-set valence targets sit in
    /// `[-0.20, 0.30]`; margin `0.25` puts the entry threshold at
    /// `[-0.45, 0.05]`, comfortably below the `Mood::default()`
    /// valence of `0.2`.
    pub drift_enter_margin: f32,
    /// §7.7.d hysteresis EXIT band. The cat must recover to
    /// `Mood::valence > expected_valence_target − drift_exit_margin`
    /// to clear the misaligned state. Must satisfy
    /// `drift_exit_margin < drift_enter_margin` for true hysteresis —
    /// the recovery threshold sits strictly above the entry threshold,
    /// so a single positive contagion tick can't erase accumulated
    /// misalignment.
    pub drift_exit_margin: f32,
    /// §7.7.d sustain duration. Once a cat enters the misaligned
    /// band, the arc is dropped after `drift_sustain_duration` of
    /// continuous misalignment. First-light value is conservative;
    /// tune in a follow-on once soak cadence is known.
    pub drift_sustain_duration: DurationDays,
}

impl Default for AspirationConstants {
    fn default() -> Self {
        Self {
            zodiac_affinity_bonus: 0.4,
            personality_alignment_weight: 0.3,
            experience_memory_scale: 0.2,
            experience_secondary_scale: 0.1,
            experience_cap: 0.6,
            scoring_jitter: 0.05,
            strong_personality_threshold: 0.7,
            weak_personality_threshold: 0.3,
            second_slot_check_interval: IntervalPerDay::new(1.0),
            stagnation_ticks: 2000,
            min_alignment: 0.3,
            milestone_mood_bonus: 0.2,
            milestone_mood_ticks: 100,
            milestone_mastery_gain: 0.05,
            milestone_purpose_gain: 0.03,
            chain_complete_mood_bonus: 0.4,
            chain_complete_mood_ticks: 200,
            chain_complete_purpose_gain: 0.1,
            drift_enter_margin: 0.25,
            drift_exit_margin: 0.10,
            drift_sustain_duration: DurationDays::new(2.0),
        }
    }
}

// ---------- FertilityConstants (§7.M.7.3) ----------

/// Cycle parameters driving the `Fertility` phase-transition function
/// (§7.M.7.2). Defaults committed in §7.M.7.3. Diestrus fraction is
/// implied by `1.0 - proestrus_fraction - estrus_fraction` and is
/// validated rather than stored as a free field.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FertilityConstants {
    /// Length of one full proestrus → estrus → diestrus cycle,
    /// expressed as a fraction of an in-game season.
    /// `DurationSeasons::new(0.5)` = half a season = ~10 in-game days
    /// at default scale, which lets the §7.M.7.3 fractions
    /// (15% / 20% / 65%) read narratively as ~1.5d proestrus, ~2d
    /// estrus, ~6.5d refractory — multiple cycles per season.
    /// Pre-ticket-033 value was `10000` raw ticks (= 0.5 seasons at
    /// default `ticks_per_season = 20000`).
    #[serde(alias = "cycle_length_ticks")]
    pub cycle_length: DurationSeasons,
    pub proestrus_fraction: f32,
    /// Fraction of `cycle_length` spent in fertile estrus (the
    /// breedable window). The remaining cycle is split between
    /// `proestrus_fraction` and the implicit diestrus
    /// (`1.0 - proestrus - estrus`, see `diestrus_fraction()` method).
    /// AND-gated against `breeding_hunger_floor`,
    /// `breeding_energy_floor`, `breeding_mood_floor`, and the partner's
    /// own estrus state — narrow values here compose with the gates to
    /// collapse the mating window. Ticket 032 §3 tracks the
    /// hunger-floor angle of this entanglement.
    pub estrus_fraction: f32,
    pub post_partum_recovery_ticks: u32,
    /// Cadence at which `update_fertility_phase` re-resolves a cat's
    /// `Fertility::phase`. Phase resolution is bounded by once-per-day
    /// phase progression — anything more frequent is wasted work.
    /// Pre-ticket-033 value was `100` (raw ticks); silently behaved as
    /// 10×/day after the 2026-04-10 overhaul.
    #[serde(alias = "update_interval_ticks")]
    pub update_interval: IntervalPerDay,
    /// Soft-gate firing threshold for L3 `MateWithGoal` (§7.M.7.6).
    /// Used by the Phase 4 target-taking pass; declared here so the
    /// tunable is already present in headers before it's consumed.
    pub l3_firing_threshold: f32,
}

impl Default for FertilityConstants {
    fn default() -> Self {
        Self {
            cycle_length: DurationSeasons::new(0.5),
            proestrus_fraction: 0.15,
            estrus_fraction: 0.20,
            post_partum_recovery_ticks: 5_000,
            update_interval: IntervalPerDay::new(1.0),
            l3_firing_threshold: 0.15,
        }
    }
}

impl FertilityConstants {
    /// Diestrus fraction = `1.0 - proestrus - estrus` per §7.M.7.3.
    /// Guards against pathological tunings where the other two
    /// fractions exceed 1.0 by clamping at zero.
    pub fn diestrus_fraction(&self) -> f32 {
        (1.0 - self.proestrus_fraction - self.estrus_fraction).max(0.0)
    }
}

// ---------- KittenRearingConstants (ticket 364) ----------

/// Maturity-band thresholds + curriculum size for the `rear_kitten` HTN
/// method's Wean / Teach / Release sub-goals. The `dependent_kitten_target`
/// picker's per-action eligibility filter uses these to gate which kittens
/// each action sees.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KittenRearingConstants {
    /// Wean is eligible while `maturity < weaned_threshold`. resolve_wean
    /// bumps maturity to this value on success.
    pub weaned_threshold: f32,
    /// Teach is eligible while
    /// `weaned_threshold <= maturity < teach_done_threshold`. resolve_teach
    /// bumps maturity to this value on success.
    pub teach_done_threshold: f32,
    /// Lower bound of the **near-mature emission window** for the
    /// `rear_kitten` HTN arc (ticket 395). The arc emits in two narrow
    /// windows gated by the `HasJuvenileDependent` marker:
    /// (a) early `[0, teach_done_threshold)` for Wean / Teach milestones,
    /// (b) near-mature `[release_threshold, 1.0)` for symbolic Release.
    /// The Release picker uses this value as its eligibility lower bound,
    /// so Release fires "at max age" — near the natural maturation moment.
    /// Natural maturation itself (the moment `KittenDependency` is removed
    /// physiologically) is hardcoded at maturity ≥ 1.0 in
    /// `tick_kitten_growth` and is independent of this constant. The
    /// kitten-side `RearKittenReleased` marker is one-shot so the second
    /// parent's frame can't re-witness within the window.
    /// **Invariant:** `teach_done_threshold < release_threshold < 1.0`.
    pub release_threshold: f32,
    /// Number of skill demonstrations recorded on the kitten's
    /// `KittenDependency.skills_learned` over the Teach phase. Substrate-only
    /// at 364 land — no consumer reads the count yet; the field exists so
    /// downstream memory/personality attribution can read which skills the
    /// kitten was taught without re-authoring substrate.
    pub teach_curriculum_size: u8,
}

impl Default for KittenRearingConstants {
    fn default() -> Self {
        Self {
            weaned_threshold: 0.33,
            teach_done_threshold: 0.66,
            release_threshold: 0.95,
            teach_curriculum_size: 5,
        }
    }
}

// ---------- ParentingActivityConstants (ticket 400) ----------

/// Tunables for the L2 `ParentingActivity` substrate (ticket 400). The
/// `parental_engagement` gradient on each `RelationshipTo` ramps toward an
/// asymptote computed by weighting five orthogonal personality scales —
/// **Presence** (compassion + warmth), **Provision** (diligence + loyalty),
/// **Protection** (boldness + temper), **Cultural** (tradition + ambition),
/// **Autonomy** (curiosity + patience + (1 − overprotection)). The asymptote
/// weights deliberately lean toward Presence (`w_presence = 0.30`) per the
/// 399 design's values stance: a hard-working low-presence parent lands at
/// moderate engagement, visibly different from a high-presence partner.
///
/// The `ParentingActivityModifier` (`src/ai/modifier.rs`) reads the
/// engagement gradient × per-scale bias formulas to lift Caretake / Hunt /
/// Patrol / Mentor DSE scores personality-conditionally; this replaces 398's
/// uniform `AspirationLift(+0.2 Caretake)` for the Kinship `RaiseOffspring`
/// aspiration. The `joint_suppression_factor` resolves the
/// two-high-compassion-parents corner case (yields to a partner already
/// holding Caretake for our dependent) without re-introducing an L3 override.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParentingActivityConstants {
    /// Per-tick EMA build rate toward the personality-derived asymptote
    /// while the cat is in tile-range of the target OR performing a
    /// parental-class action toward the target. `engagement +=
    /// (asymptote - engagement) * engagement_build_rate`.
    pub engagement_build_rate: f32,
    /// Per-tick EMA decay rate while out-of-range and not performing a
    /// parental-class action. `engagement -= engagement * engagement_decay_rate`.
    /// Build is ~10× faster than decay so the gradient stabilizes near the
    /// asymptote under typical proximity patterns; absence (e.g., the
    /// single-working-mother Hunt-while-away case) does not snap the bond.
    pub engagement_decay_rate: f32,
    /// Manhattan/Euclidean tile range within which the target is considered
    /// "present" for engagement-build purposes (parallel to other
    /// proximity-gated systems like `bond_proximity_range`).
    pub engagement_range_tiles: f32,
    /// Multiplier applied to the asymptote once the dependent kitten reaches
    /// `maturity >= 1.0` (the `KittenDependency`-removal threshold). Engagement
    /// decays toward `matured_residual_factor × asymptote` rather than zero —
    /// "still your mother." `0.15` per 399 design.
    pub matured_residual_factor: f32,
    /// Multiplier applied to `caretake_bias` when a partner has a held
    /// Caretake intention targeting one of our dependents. `0.3` per 399 —
    /// yields without fully suppressing (so a high-compassion second parent
    /// can still snap to Caretake if the first lapses).
    pub joint_suppression_factor: f32,
    /// Asymptote weight on the Presence scale. Default `0.30` (highest of
    /// the five) per 399's values stance — presence is load-bearing for
    /// parental engagement.
    pub w_presence: f32,
    /// Asymptote weight on the Provision scale. Default `0.20`.
    pub w_provision: f32,
    /// Asymptote weight on the Protection scale. Default `0.20`.
    pub w_protection: f32,
    /// Asymptote weight on the Cultural scale. Default `0.15`.
    pub w_cultural: f32,
    /// Asymptote weight on the Autonomy scale. Default `0.15`. Sum across
    /// all five weights = 1.00.
    pub w_autonomy: f32,
}

impl Default for ParentingActivityConstants {
    fn default() -> Self {
        Self {
            engagement_build_rate: 0.001,
            engagement_decay_rate: 0.0001,
            engagement_range_tiles: 5.0,
            matured_residual_factor: 0.15,
            joint_suppression_factor: 0.3,
            w_presence: 0.30,
            w_provision: 0.20,
            w_protection: 0.20,
            w_cultural: 0.15,
            w_autonomy: 0.15,
        }
    }
}

// ---------- CraftingConstants (ticket 367 — 016 Phase 1b) ----------

/// Tunables for the food-preservation pipeline. Recipe durations come
/// from `docs/systems/crafting.md` Phase 1 table; tend cadence and
/// organ-drop probability are first-cut values that the 367 hypothesis
/// soak will validate via `just verdict`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CraftingConstants {
    /// Total ticks of Clear weather required for the
    /// `preserve.dried_fish` recipe to complete. Per crafting.md:
    /// "~3 days of Clear weather." At the default `SimConfig`
    /// (ticks_per_day ≈ 5000 wall ticks), 3 days ≈ 15_000 ticks.
    /// The drying system advances per-tick *only when*
    /// `weather.current == Weather::Clear`; ticks under non-clear
    /// weather don't count, so wall-clock completion can run longer.
    pub drying_dried_fish_total_ticks: u64,

    /// Total clear-weather ticks for `preserve.preserved_organ`. Per
    /// crafting.md: "~2 days." Slightly faster than fish since organs
    /// are smaller / cure with herb assistance.
    pub drying_preserved_organ_total_ticks: u64,

    /// Number of discrete tend visits a Smoking Rack needs before it
    /// produces `SmokedMeat`. 3 = the design-doc default (~3-4 visits
    /// over the ~1 day craft window). Stored on
    /// `SmokingRackState::tends_needed` at load time so future recipes
    /// could declare variant cadences.
    pub smoking_tends_needed: u32,

    /// Per-rack cooldown enforced between tend visits. The
    /// `TendSmokingRackDse` eligibility gate's
    /// `HasLoadedSmokingRackOffCooldown` marker fires only after this
    /// many ticks have elapsed since the last tend on a given rack.
    /// Forces interleaving — the cat must do something else for
    /// ~2 sim-hours between tends, producing the "tend, walk away,
    /// come back" rhythm the design doc calls for. At default
    /// SimConfig (ticks_per_hour ≈ 208), ~2 hours ≈ 416 ticks.
    pub smoking_tend_cooldown_ticks: u64,

    /// Probability that a successful hunt drops a `RawOrgan` alongside
    /// the carcass. Fish hunts bypass this roll (only mammals + birds
    /// drop organs).
    pub organ_drop_chance: f32,

    /// Mood bump applied to a cat when it eats organ-derived food
    /// (`RawOrgan` or `PreservedOrgan`). Per the "items are real,
    /// effects live on resolvers keyed to item identity" pillar — this
    /// is an action-side effect, not a numeric field on the item.
    pub organ_mood_bonus: f32,

    // ----- 367-4b: preservation output-quality formula -----
    //
    // Output quality at a preservation rack's completion site (drying
    // system or tend resolver) is:
    //
    //     output = clamp01(
    //         source_quality * preservation_quality_input_weight
    //       + crafter_skill  * preservation_quality_skill_weight
    //     )
    //
    // where `crafter_skill` is the loader's normalised skill:
    //
    //     crafter_skill = clamp01(
    //         skills.herbcraft * 0.5
    //       + skills.foraging  * 0.3
    //       + preservation_skill_baseline
    //     )
    //
    // Defaults (input 0.7 / skill 0.3 / baseline 0.4) give a default
    // colony cat a `crafter_skill ≈ 0.46` and an output quality on a
    // perfect-1.0 input of `~0.84`. Highly skilled cats craft near 1.0.
    // **Decorative until consumption-side wiring lands**: `food_value`
    // currently reads `ItemKind` only, so quality only surfaces in the
    // narrative label / inspect panel. Wiring quality into food value
    // (or mood) is a separate follow-on (cross-references ticket 016 +
    // 429 for the items-are-real / Source-Transfer-Sink home).
    /// Weight applied to the source item's `quality` when computing
    /// preservation output quality. `0.7` means input quality
    /// dominates; reducing this lifts the floor for unskilled crafters
    /// (RimWorld-style skill-matters) and reducing it amplifies the
    /// skill contribution.
    pub preservation_quality_input_weight: f32,

    /// Weight applied to the crafter's normalised skill when computing
    /// preservation output quality. `0.3` means a maxed-skill crafter
    /// adds 0.3 quality on top of the input contribution (capped at
    /// 1.0). Symmetric pair with `preservation_quality_input_weight`.
    pub preservation_quality_skill_weight: f32,

    /// Additive baseline added to the loader's herbcraft+foraging
    /// skills before clamp. `0.4` means a default-skill cat
    /// (herbcraft 0.05, foraging 0.1) crafts at `~0.46` rather than
    /// the catastrophic `~0.05` the raw skill values alone would
    /// give. Tune down if novice cats should craft poorly; tune up
    /// if novice cats should craft acceptably from day one.
    pub preservation_skill_baseline: f32,

    // ----- 368 Phase 2 — behavioral-tool recipe durations -----
    //
    // Tick budgets are nominal at the canonical SimConfig
    // (ticks_per_day ≈ 5000). The Workshop-craft pipeline (DSE +
    // plan template + executor wiring) lands in a follow-on ticket;
    // these fields are registry metadata used by the Recipe.duration
    // field so future tooling can answer "how long does this take?"
    /// Stone → PolishedStone polish duration. Quick surface-shaping
    /// at a Workshop — half a sim-day at canonical SimConfig.
    pub polish_polished_stone_ticks: u64,
    /// (Twig + Bristle) → GroomingBrush. One sim-day.
    pub craft_grooming_brush_ticks: u64,
    /// (Fiber + Feather) → PlayBundle. Half a sim-day; play bundles
    /// are simpler than a full tool.
    pub craft_play_bundle_ticks: u64,
    /// (One of PolishedStone / Feather / Flower) → CourtshipGift.
    /// Quick — the courting cat picks an existing material and
    /// presents it as expressive prop, no major shaping.
    pub craft_courtship_gift_ticks: u64,

    // ----- 369 Phase 2b — warrior's-kit recipe durations -----
    //
    // 8 new recipes. Tick budgets calibrated to roughly mirror the
    // 368 Phase 2 family — simpler items (stiletto, club) take ~half
    // a sim-day, complex items (spear, plated wrap) take a full day.
    // First-light values; tune via sweep once production stabilises.
    /// Bone + Twig + Sinew → BoneTipSpear. Full sim-day — shaft
    /// preparation + bone-tip lashing.
    pub craft_bone_tip_spear_ticks: u64,
    /// Bone → BoneStiletto. Quick — single-piece bone-shaping.
    pub craft_bone_stiletto_ticks: u64,
    /// Stone → FlintBlade. Open-ground knapping — slightly faster
    /// than workshop-bound shaping.
    pub craft_flint_blade_ticks: u64,
    /// Hide + Sinew → HideBracers. Tanning-frame work — a stretched
    /// hide cures while the cat trims and ties.
    pub craft_hide_bracers_ticks: u64,
    /// Hide + Sinew → HidePlatedWrap. Heavier coverage; longest
    /// Phase 2b craft.
    pub craft_hide_plated_wrap_ticks: u64,
    /// Fiber + Hide → Sling. Cradle weaving — half a sim-day.
    pub craft_sling_ticks: u64,
    /// Fiber → WovenReedCloak. Full sim-day; reed-mat weaving.
    pub craft_woven_reed_cloak_ticks: u64,
    /// Twig + Whisker → ToothNotchedClub. Quick — shaft + notch
    /// shaping.
    pub craft_tooth_notched_club_ticks: u64,

    // ----- 368 Phase 2 — behavioral-tool resolver multipliers -----
    //
    // The three new ItemKinds (GroomingBrush / PlayBundle /
    // CourtshipGift) modify their corresponding action resolvers'
    // outcome magnitudes via these multipliers. Read in commit 5
    // (groom_other / socialize / mate_with) when the actor's
    // inventory carries the tool. Per the "items are real" pillar,
    // the multiplier lives on the resolver branch, not as a
    // modifier field on the item type.
    /// Multiplier applied to `groom_other` fondness delta when the
    /// actor carries a `GroomingBrush`. `1.5` = 50% richer mutual
    /// grooming when a brush is used. Tune via sweep once the
    /// Workshop-craft pipeline lands and brushes actually circulate
    /// in seed-42.
    pub groom_brush_fondness_multiplier: f32,
    /// Multiplier applied to social fondness gain when either
    /// participant in a Socializing pairing carries a `PlayBundle`.
    /// Kittens benefit more in narrative (`play_social` already
    /// templates differently for the `life_stage == Kitten` arm);
    /// the multiplier scales the underlying delta uniformly.
    pub play_bundle_social_multiplier: f32,
    /// Multiplier applied to the romantic-delta in `mate_with` when
    /// the courting cat carries a `CourtshipGift`. `2.0` = double
    /// fondness gain on the gift-bearing pairing tick.
    pub courtship_gift_romantic_multiplier: f32,
}

impl Default for CraftingConstants {
    fn default() -> Self {
        // Tick budgets are nominal and derived from the canonical
        // SimConfig (ticks_per_day ≈ 5000). The drying system reads
        // these directly; the resolver layer can convert via TimeScale
        // if a future SimConfig scales them.
        Self {
            drying_dried_fish_total_ticks: 15_000,
            drying_preserved_organ_total_ticks: 10_000,
            smoking_tends_needed: 3,
            smoking_tend_cooldown_ticks: 416,
            // 30% per the user-selected scope (cf. plan §6 / hunt-drop
            // wiring). High enough to surface organs reliably during a
            // 15-min soak; low enough that they remain a "prize part of
            // the kill" rather than the default carcass shape.
            organ_drop_chance: 0.30,
            // Small mood tick — enough to surface in mood traces, not
            // enough to push organ-eating above hunger-driven choices.
            organ_mood_bonus: 0.05,
            // 367-4b — preservation quality formula. See struct
            // doc-comments for the formula shape.
            preservation_quality_input_weight: 0.7,
            preservation_quality_skill_weight: 0.3,
            preservation_skill_baseline: 0.4,
            // 368 Phase 2 — recipe durations (registry metadata; runtime
            // wiring lands with the Workshop-craft DSE follow-on).
            polish_polished_stone_ticks: 2_500,
            craft_grooming_brush_ticks: 5_000,
            craft_play_bundle_ticks: 2_500,
            craft_courtship_gift_ticks: 1_000,
            // 369 Phase 2b — warrior's-kit durations.
            craft_bone_tip_spear_ticks: 5_000,
            craft_bone_stiletto_ticks: 1_500,
            craft_flint_blade_ticks: 2_000,
            craft_hide_bracers_ticks: 4_000,
            craft_hide_plated_wrap_ticks: 7_500,
            craft_sling_ticks: 2_500,
            craft_woven_reed_cloak_ticks: 5_000,
            craft_tooth_notched_club_ticks: 1_500,
            // 368 Phase 2 — behavioral-tool resolver multipliers
            // (first-light values; tune via sweep when the
            // Workshop-craft pipeline lands and the tools circulate
            // in seed-42).
            groom_brush_fondness_multiplier: 1.5,
            play_bundle_social_multiplier: 1.5,
            courtship_gift_romantic_multiplier: 2.0,
        }
    }
}

// ---------- PreyByproductConstants (ticket 375 — 016 input substrate) ----------

/// Per-species guaranteed byproduct lists for `resolve_engage_prey`.
///
/// Each successful kill spawns the species' meat (from `PreyProfile::item_kind()`)
/// plus the matching list from this table. Composition is additive: a Rat kill
/// produces `RawRat` plus `[Bone, Sinew, Whisker]` plus — with `crafting.organ_drop_chance`
/// probability — an optional `RawOrgan`. The 367 probabilistic organ drop continues
/// to fire independently on top of this guaranteed list.
///
/// Downstream sinks (016 phase children):
/// - `Bone` → 369 / 372 · `Sinew` → 369 / 368 · `Whisker` → 370 / 368
/// - `Hide` → 369 / 370 · `FishScale` → 372 / 371 · `Tallow` → 371
/// - `RawOrgan` → 367 (existing drying pipeline) · `Feather` → 368 / 370
/// - `Bristle` → 368 (Grooming Brush; mammal-only — Mouse/Rat/Rabbit)
///
/// Items-are-real: each entry is a spatial entity emitted by the hunt resolver,
/// not a numeric modifier on the prey. Inventory pressure (4 items per rabbit
/// instead of 1) is the load-bearing emergent consequence.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreyByproductConstants {
    pub mouse: Vec<crate::components::items::ItemKind>,
    pub rat: Vec<crate::components::items::ItemKind>,
    pub rabbit: Vec<crate::components::items::ItemKind>,
    pub fish: Vec<crate::components::items::ItemKind>,
    pub bird: Vec<crate::components::items::ItemKind>,
}

impl PreyByproductConstants {
    /// Byproducts dropped by a kill of this species. Returns an empty slice
    /// if the species somehow falls outside the table (defensive — every
    /// `PreyKind` variant is wired below).
    pub fn for_kind(&self, kind: PreyKind) -> &[crate::components::items::ItemKind] {
        match kind {
            PreyKind::Mouse => &self.mouse,
            PreyKind::Rat => &self.rat,
            PreyKind::Rabbit => &self.rabbit,
            PreyKind::Fish => &self.fish,
            PreyKind::Bird => &self.bird,
        }
    }
}

impl Default for PreyByproductConstants {
    fn default() -> Self {
        use crate::components::items::ItemKind;
        Self {
            // 368: Mammals shed Bristle (the prey-shedding ingredient
            // for Grooming Brush). Appended to existing per-species
            // vectors to preserve byproduct iteration order from 375.
            mouse: vec![ItemKind::Bone, ItemKind::Sinew, ItemKind::Bristle],
            rat: vec![
                ItemKind::Bone,
                ItemKind::Sinew,
                ItemKind::Whisker,
                ItemKind::Bristle,
            ],
            rabbit: vec![
                ItemKind::Hide,
                ItemKind::Bone,
                ItemKind::Sinew,
                ItemKind::Bristle,
            ],
            fish: vec![ItemKind::FishScale, ItemKind::Tallow, ItemKind::RawOrgan],
            bird: vec![ItemKind::Feather, ItemKind::Bone],
        }
    }
}

// ---------- KnowledgeConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeConstants {
    /// Ticks between colony-knowledge derivation scans. 291 — the
    /// derivation replaced the legacy per-tick decay + Memory scan;
    /// entry strength now tracks live belief state at each scan.
    pub scan_interval: u64,
    /// Cooldown on re-narrating "the colony has forgotten X" for the
    /// same description, preventing promote/dissolve cycle spam.
    pub forgotten_cooldown: u64,
    /// 291 — how many cats must agree (facet values mutually within
    /// `agreement_epsilon` of the group median) for a belief to
    /// promote to colony knowledge. Successor to the retired
    /// carrier-count `promotion_threshold` (same default, 3).
    pub agreement_quorum: u32,
    /// 291 — max distance from the group median a cat's facet value
    /// may sit while still counting toward the agreement quorum.
    pub agreement_epsilon: f32,
    /// 291 — minimum facet STRENGTH (evidence confidence) for a cat's
    /// belief to count toward the quorum at all. Filters barely-formed
    /// or nearly-forgotten beliefs out of the consensus.
    pub promotion_strength: f32,
    /// 291 — minimum agreed facet VALUE for promotion. A quorum
    /// agreeing "nothing notable here" (decayed values near 0) is
    /// consensus about absence — not knowledge worth an entry, and
    /// without this floor it would churn phantom near-zero entries
    /// through the promote/forget narrative.
    pub min_promotion_value: f32,
}

impl Default for KnowledgeConstants {
    fn default() -> Self {
        Self {
            scan_interval: 500,
            forgotten_cooldown: 1000,
            agreement_quorum: 3,
            agreement_epsilon: 0.2,
            promotion_strength: 0.3,
            min_promotion_value: 0.05,
        }
    }
}

// ---------- PersonalityFrictionConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersonalityFrictionConstants {
    pub friction_range: f32,
    pub tradition_vs_independence_threshold: f32,
    pub tradition_vs_independence_decay: f32,
    pub diligence_vs_playfulness_threshold: f32,
    pub diligence_vs_playfulness_decay: f32,
    pub dual_ambition_threshold: f32,
    pub dual_ambition_decay: f32,
    pub loyalty_vs_independence_threshold: f32,
    pub loyalty_vs_independence_decay: f32,
}

impl Default for PersonalityFrictionConstants {
    fn default() -> Self {
        Self {
            friction_range: 3.0,
            tradition_vs_independence_threshold: 0.8,
            tradition_vs_independence_decay: -0.0002,
            diligence_vs_playfulness_threshold: 0.8,
            diligence_vs_playfulness_decay: -0.0001,
            dual_ambition_threshold: 0.8,
            dual_ambition_decay: -0.0003,
            loyalty_vs_independence_threshold: 0.8,
            loyalty_vs_independence_decay: -0.0002,
        }
    }
}

// ---------- WorldGenConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorldGenConstants {
    /// Target number of AncientRuin sites per map.
    pub ancient_ruin_count: usize,
    /// Target number of FairyRing sites per map.
    pub fairy_ring_count: usize,
    /// Target number of StandingStone sites per map.
    pub standing_stone_count: usize,
    /// Target number of DeepPool sites per map.
    pub deep_pool_count: usize,
    /// Minimum manhattan distance between any two special site anchors.
    pub special_min_spacing: f32,
    /// Minimum manhattan distance from AncientRuin to colony site.
    pub corruption_colony_min_distance: f32,
    /// Minimum distance from map edges for special site placement.
    pub edge_margin: i32,
    /// Maximum candidates to evaluate per type after shuffle.
    pub max_placement_attempts: usize,
}

impl Default for WorldGenConstants {
    fn default() -> Self {
        Self {
            ancient_ruin_count: 3,
            fairy_ring_count: 2,
            standing_stone_count: 3,
            deep_pool_count: 2,
            special_min_spacing: 15.0,
            corruption_colony_min_distance: 30.0,
            edge_margin: 10,
            max_placement_attempts: 500,
        }
    }
}

// ---------- SensoryConstants ----------

/// Per-species sensory profiles.
///
/// Keyed by `SensorySpecies`. Phase 1 defaults are calibrated so that
/// migrating call sites can preserve existing behavior under identity
/// environmental multipliers (see `src/systems/sensing.rs`). Specific
/// ranges like `threat_awareness_range: 10` match the cat sight profile;
/// broader call sites (herb / search / fated-love detection at 15) pass
/// a per-site `max_range_override` during migration rather than bloating
/// the profile table with task-specific fields.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SensoryConstants {
    pub cat: SensoryProfile,
    pub fox: SensoryProfile,
    pub hawk: SensoryProfile,
    pub snake: SensoryProfile,
    pub shadow_fox: SensoryProfile,
    pub mouse: SensoryProfile,
    pub rat: SensoryProfile,
    pub rabbit: SensoryProfile,
    pub fish: SensoryProfile,
    pub bird: SensoryProfile,
}

impl SensoryConstants {
    /// Look up the profile for a species. Panics on no match — the
    /// enum is exhaustive and every variant has a field.
    pub fn profile_for(&self, species: SensorySpecies) -> &SensoryProfile {
        match species {
            SensorySpecies::Cat => &self.cat,
            SensorySpecies::Wild(WildSpecies::Fox) => &self.fox,
            SensorySpecies::Wild(WildSpecies::Hawk) => &self.hawk,
            SensorySpecies::Wild(WildSpecies::Snake) => &self.snake,
            SensorySpecies::Wild(WildSpecies::ShadowFox) => &self.shadow_fox,
            SensorySpecies::Prey(PreyKind::Mouse) => &self.mouse,
            SensorySpecies::Prey(PreyKind::Rat) => &self.rat,
            SensorySpecies::Prey(PreyKind::Rabbit) => &self.rabbit,
            SensorySpecies::Prey(PreyKind::Fish) => &self.fish,
            SensorySpecies::Prey(PreyKind::Bird) => &self.bird,
        }
    }
}

impl Default for SensoryConstants {
    fn default() -> Self {
        // Phase 1 defaults: chosen to match or bracket existing detection
        // ranges. Scent ranges are the *common-case* baseline — migrating
        // call sites with longer task-specific ranges (search, forage,
        // fated-love at 15) pass `max_range_override`. Post-refactor a
        // task-multiplier system can absorb those; for now keep the
        // profile table compact.
        Self {
            // Cats: sight/hearing hunters, no substrate sense.
            cat: SensoryProfile {
                sight: Channel::new(10.0, 0.5, Falloff::Cliff),
                hearing: Channel::new(8.0, 0.5, Falloff::Cliff),
                scent: Channel::new(15.0, 0.5, Falloff::Cliff),
                tremor: Channel::DISABLED,
                scent_directional: true,
            },
            // Fox: ears and nose dominant, modest tremor.
            fox: SensoryProfile {
                sight: Channel::new(8.0, 0.5, Falloff::Cliff),
                hearing: Channel::new(10.0, 0.5, Falloff::Cliff),
                scent: Channel::new(12.0, 0.5, Falloff::Cliff),
                tremor: Channel::new(3.0, 0.5, Falloff::Cliff),
                scent_directional: true,
            },
            // Hawk: raptor vision, essentially pure sight.
            hawk: SensoryProfile {
                sight: Channel::new(15.0, 0.5, Falloff::Cliff),
                hearing: Channel::new(5.0, 0.5, Falloff::Cliff),
                scent: Channel::DISABLED,
                tremor: Channel::DISABLED,
                scent_directional: false,
            },
            // Snake: scent + vibration hunter, barely sees.
            snake: SensoryProfile {
                sight: Channel::new(1.0, 0.5, Falloff::Cliff),
                hearing: Channel::new(3.0, 0.5, Falloff::Cliff),
                scent: Channel::new(8.0, 0.5, Falloff::Cliff),
                tremor: Channel::new(6.0, 0.5, Falloff::Cliff),
                scent_directional: true,
            },
            // Shadow-fox: corrupted; elevated non-visual senses.
            shadow_fox: SensoryProfile {
                sight: Channel::new(8.0, 0.5, Falloff::Cliff),
                hearing: Channel::new(7.0, 0.5, Falloff::Cliff),
                scent: Channel::new(10.0, 0.5, Falloff::Cliff),
                tremor: Channel::new(5.0, 0.5, Falloff::Cliff),
                scent_directional: false, // supernatural — ignores wind
            },
            // Prey: substrate-sensitive by design.
            // `sight` uses Linear falloff so the prey-detects-cat path
            // can produce a probabilistic proximity gradient matching
            // the legacy `1 - dist/(alert_radius+1)` formula. Other
            // channels stay Cliff for Phase 1-4 structural discipline.
            mouse: SensoryProfile {
                sight: Channel::new(3.0, 0.5, Falloff::Linear),
                hearing: Channel::new(6.0, 0.5, Falloff::Cliff),
                scent: Channel::new(5.0, 0.5, Falloff::Cliff),
                tremor: Channel::new(6.0, 0.5, Falloff::Cliff),
                scent_directional: true,
            },
            rat: SensoryProfile {
                sight: Channel::new(5.0, 0.5, Falloff::Linear),
                hearing: Channel::new(7.0, 0.5, Falloff::Cliff),
                scent: Channel::new(6.0, 0.5, Falloff::Cliff),
                tremor: Channel::new(7.0, 0.5, Falloff::Cliff),
                scent_directional: true,
            },
            rabbit: SensoryProfile {
                sight: Channel::new(6.0, 0.5, Falloff::Linear),
                hearing: Channel::new(10.0, 0.5, Falloff::Cliff),
                scent: Channel::new(4.0, 0.5, Falloff::Cliff),
                tremor: Channel::new(12.0, 0.5, Falloff::Cliff),
                scent_directional: true,
            },
            fish: SensoryProfile {
                sight: Channel::new(3.0, 0.5, Falloff::Linear),
                hearing: Channel::new(5.0, 0.5, Falloff::Cliff),
                scent: Channel::new(5.0, 0.5, Falloff::Cliff),
                tremor: Channel::new(6.0, 0.5, Falloff::Cliff), // lateral line
                scent_directional: false,                       // water currents handled separately
            },
            bird: SensoryProfile {
                sight: Channel::new(10.0, 0.5, Falloff::Linear),
                hearing: Channel::new(5.0, 0.5, Falloff::Cliff),
                scent: Channel::new(2.0, 0.5, Falloff::Cliff),
                tremor: Channel::new(2.0, 0.5, Falloff::Cliff),
                scent_directional: true,
            },
        }
    }
}

// ---------- TremorConstants (ticket 100) ----------

/// `TremorMap` tunables — action-keyed emission multipliers + deposit /
/// decay / detection-threshold scalars. The map is the §5.6 tremor
/// channel's influence-map substrate.
///
/// Action multipliers ladder from `idle` (motionless) to `pounce`
/// (explosive spring). The convention preserved by the test
/// `action_tremor_mul_ordered_by_loudness` is
/// `idle ≤ stalk ≤ walk ≤ fight ≤ run ≤ pounce`; a tuning pass that
/// breaks the ordering must update the test or the design.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TremorConstants {
    /// Multiplier on `sensory_signature.tremor_baseline` when the cat
    /// is mid-stalk. ≈0.2 — stalking suppresses vibration by design;
    /// that's the action's whole behavioral function.
    pub action_tremor_stalk: f32,
    /// Multiplier for stationary / quiescent actions (Sleep, Idle,
    /// Hide, GroomSelf, Vigil, GriefSit). 0.0 — no emitter motion.
    pub action_tremor_idle: f32,
    /// Multiplier for normal-pace movement and most mundane actions.
    /// 1.0 — the baseline.
    pub action_tremor_walk: f32,
    /// Multiplier for sustained high-speed motion (Flee, chase-phase
    /// Hunt). ≈1.8 — running cats announce themselves through the
    /// ground.
    pub action_tremor_run: f32,
    /// Multiplier for combat. ≈1.5 — high but below run, because the
    /// thrash is chaotic-localized rather than ground-coupled-stride.
    pub action_tremor_fight: f32,
    /// Multiplier for the strike. ≈2.0 — explosive spring is peak
    /// emission. The pounce range is by construction inside the
    /// terminal grab window, so the spike is "too late" feedback by
    /// design.
    pub action_tremor_pounce: f32,
    /// Per-tick scale applied to every deposit; tunes the absolute
    /// intensity of the map without disturbing the action-ratio
    /// ladder. Default 1.0.
    pub deposit_per_tick: f32,
    /// Per-tick decay subtracted from every bucket. ≈0.4 empties a
    /// full bucket in 1-3 ticks — fast enough that "this tile is hot
    /// right now" stays meaningful and slow enough that a few-tick
    /// burst (e.g. a running cat crossing) leaves a perceivable trail.
    pub decay_per_tick: f32,
    /// Minimum `TremorMap::peak_nearby` reading required for
    /// `try_detect_cat` to transition the prey to `PreyAiState::Alert`
    /// on vibration alone. ≈0.25 — well above noise, below the peak
    /// reading produced by a single walking cat in an adjacent bucket.
    pub detect_threshold: f32,
    /// Manhattan radius around prey for the `peak_nearby` sample.
    /// Larger values make prey more sensitive to distant footfall but
    /// flatten the spatial gradient. Default 6 tiles — roughly two
    /// bucket-radii at the canonical bucket_size=3.
    pub detect_radius: f32,
}

impl Default for TremorConstants {
    fn default() -> Self {
        Self {
            action_tremor_stalk: 0.2,
            action_tremor_idle: 0.0,
            action_tremor_walk: 1.0,
            action_tremor_run: 1.8,
            action_tremor_fight: 1.5,
            action_tremor_pounce: 2.0,
            deposit_per_tick: 1.0,
            decay_per_tick: 0.4,
            detect_threshold: 0.25,
            detect_radius: 6.0,
        }
    }
}

// ---------- EnvironmentalQualityConstants (ticket 101) ----------

/// Ticket 101 — tunables for the five ambient influence maps. Per-source
/// stamping radii + peak values for the terrain / buildings / corpse
/// sweep, personality scaling factors used by
/// `EnvironmentalQualityModifier`, modifier clamp bounds, and the
/// `feature_emit_threshold` that gates
/// `Feature::EnvironmentalComfortPositive`. Carried through the
/// `events.jsonl` header by virtue of `SimConstants`' Serialize impl.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EnvironmentalQualityConstants {
    // --- Comfort: terrain base values (on-tile, no falloff) ---
    pub comfort_terrain_fairy_ring: f32,
    pub comfort_terrain_light_forest: f32,
    pub comfort_terrain_dense_forest: f32,
    pub comfort_terrain_sand: f32,
    pub comfort_terrain_mud: f32,
    pub comfort_terrain_rock: f32,
    // --- Comfort: building bonuses (peak × condition, with radii) ---
    pub comfort_building_den_peak: f32,
    pub comfort_building_den_radius: f32,
    pub comfort_building_hearth_peak: f32,
    pub comfort_building_hearth_radius: f32,
    pub comfort_building_stores_peak: f32,
    pub comfort_building_stores_radius: f32,
    pub comfort_building_workshop_peak: f32,
    pub comfort_building_workshop_radius: f32,
    pub comfort_building_garden_peak: f32,
    pub comfort_building_garden_radius: f32,
    pub comfort_building_ward_post_peak: f32,
    pub comfort_building_ward_post_radius: f32,
    // --- Cleanliness ---
    pub cleanliness_terrain_mud: f32,
    pub cleanliness_corpse_peak: f32,
    pub cleanliness_corpse_radius: f32,
    pub cleanliness_dirty_building_peak: f32,
    pub cleanliness_dirty_building_radius: f32,
    // --- Beauty (terrain sources) ---
    pub beauty_terrain_fairy_ring_peak: f32,
    pub beauty_terrain_fairy_ring_radius: f32,
    pub beauty_terrain_standing_stone_peak: f32,
    pub beauty_terrain_standing_stone_radius: f32,
    pub beauty_terrain_deep_pool_peak: f32,
    pub beauty_terrain_deep_pool_radius: f32,
    pub beauty_terrain_garden_peak: f32,
    pub beauty_terrain_garden_radius: f32,
    pub beauty_terrain_ancient_ruin: f32,
    /// Subtracted from on-tile beauty proportional to `tile.corruption`
    /// during the terrain sweep. High corruption suppresses beauty.
    pub beauty_corruption_suppression: f32,
    // --- Beauty (building aesthetic upkeep) ---
    pub beauty_building_den_peak: f32,
    pub beauty_building_den_radius: f32,
    pub beauty_building_hearth_peak: f32,
    pub beauty_building_hearth_radius: f32,
    pub beauty_building_garden_peak: f32,
    pub beauty_building_garden_radius: f32,
    // --- Mystery ---
    pub mystery_stamp_radius: f32,
    pub mystery_stamp_threshold: f32,
    // --- Corruption (perception map) ---
    pub corruption_stamp_radius: f32,
    pub corruption_stamp_threshold: f32,
    // --- Modifier (personality scaling + combination) ---
    /// Multiplier on `local_comfort` per unit warmth. `0.3` ⇒ a
    /// warmth-1.0 cat reads comfort 30% larger than a warmth-0 cat.
    pub warmth_bonus: f32,
    /// Multiplier on `local_comfort` damped by independence. `0.2` ⇒
    /// an independence-1.0 cat reads comfort 20% smaller.
    pub independence_dampen: f32,
    /// Multiplier on `local_cleanliness` per unit anxiety.
    pub anxiety_bonus: f32,
    /// Multiplier on `local_beauty` per unit spirituality.
    pub spirituality_bonus: f32,
    /// Multiplier on `local_mystery` per unit curiosity.
    pub curiosity_bonus: f32,
    /// Weight applied to the summed contribution before clamping.
    /// Default `0.5` keeps the modifier modest relative to acute
    /// drives.
    pub combination_weight: f32,
    /// Lower clamp on the combined modifier value.
    pub combined_min: f32,
    /// Upper clamp on the combined modifier value.
    pub combined_max: f32,
    /// Threshold on the combined modifier for emitting
    /// `Feature::EnvironmentalComfortPositive`. The feature fires once
    /// per tick if any living cat clears `+feature_emit_threshold`;
    /// the negative pendant uses `-feature_emit_threshold`.
    pub feature_emit_threshold: f32,
}

impl Default for EnvironmentalQualityConstants {
    fn default() -> Self {
        Self {
            // Comfort terrain
            comfort_terrain_fairy_ring: 0.3,
            comfort_terrain_light_forest: 0.1,
            comfort_terrain_dense_forest: 0.05,
            comfort_terrain_sand: -0.05,
            comfort_terrain_mud: -0.15,
            comfort_terrain_rock: -0.1,
            // Comfort buildings
            comfort_building_den_peak: 0.20,
            comfort_building_den_radius: 2.0,
            comfort_building_hearth_peak: 0.25,
            comfort_building_hearth_radius: 3.0,
            comfort_building_stores_peak: 0.05,
            comfort_building_stores_radius: 1.0,
            comfort_building_workshop_peak: 0.10,
            comfort_building_workshop_radius: 1.0,
            comfort_building_garden_peak: 0.15,
            comfort_building_garden_radius: 2.0,
            comfort_building_ward_post_peak: 0.05,
            comfort_building_ward_post_radius: 1.0,
            // Cleanliness
            cleanliness_terrain_mud: -0.15,
            cleanliness_corpse_peak: -0.4,
            cleanliness_corpse_radius: 3.0,
            cleanliness_dirty_building_peak: -0.3,
            cleanliness_dirty_building_radius: 2.0,
            // Beauty terrain
            beauty_terrain_fairy_ring_peak: 0.4,
            beauty_terrain_fairy_ring_radius: 3.0,
            beauty_terrain_standing_stone_peak: 0.25,
            beauty_terrain_standing_stone_radius: 2.0,
            beauty_terrain_deep_pool_peak: 0.15,
            beauty_terrain_deep_pool_radius: 2.0,
            beauty_terrain_garden_peak: 0.20,
            beauty_terrain_garden_radius: 2.0,
            beauty_terrain_ancient_ruin: -0.1,
            beauty_corruption_suppression: 0.2,
            // Beauty buildings
            beauty_building_den_peak: 0.10,
            beauty_building_den_radius: 1.0,
            beauty_building_hearth_peak: 0.10,
            beauty_building_hearth_radius: 1.0,
            beauty_building_garden_peak: 0.20,
            beauty_building_garden_radius: 2.0,
            // Mystery
            mystery_stamp_radius: 2.0,
            mystery_stamp_threshold: 0.0,
            // Corruption perception
            corruption_stamp_radius: 3.0,
            corruption_stamp_threshold: 0.0,
            // Modifier
            warmth_bonus: 0.3,
            independence_dampen: 0.2,
            anxiety_bonus: 0.4,
            spirituality_bonus: 0.4,
            curiosity_bonus: 0.4,
            combination_weight: 0.5,
            combined_min: -0.3,
            combined_max: 0.3,
            feature_emit_threshold: 0.05,
        }
    }
}

// ---------- FulfillmentConstants (§7.W) ----------

/// Constants for the §7.W Fulfillment register. MVP scope: `social_warmth`
/// axis decay and restoration. Sensitization, tolerance, and diversity-decay
/// mechanics are future work that adds fields here.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FulfillmentConstants {
    /// Base per-tick decay rate for social_warmth when no social contact.
    pub social_warmth_base_decay: f32,
    /// Decay multiplier when no bonded cat is within proximity range.
    pub social_warmth_isolation_multiplier: f32,
    /// Manhattan distance to detect nearby bonded companions for isolation check.
    pub social_warmth_isolation_range: f32,
    /// social_warmth gain per groom-other completion (both parties).
    pub social_warmth_groom_other_gain: f32,
    /// Passive per-tick social_warmth gain when a bonded companion is nearby.
    pub social_warmth_bond_proximity_rate: f32,
    /// Manhattan range for bond-proximity social_warmth restoration.
    pub social_warmth_bond_proximity_range: f32,
    /// Per-tick social_warmth gain while actively socializing with a target.
    pub social_warmth_socialize_per_tick: f32,
    // --- Ticket 032: body-condition axis (ship-inert) ---
    /// Ticket 032 — per-tick decay rate on `body_condition` per unit
    /// hunger deficit below `body_condition_pivot`. **Default `0.0`**
    /// (axis ships flat at 1.0). Treatment values exercise the slow-
    /// moving body-condition curve; ~`0.0001` is a reasonable starting
    /// magnitude (loses 10% body condition over 1000 ticks of moderate
    /// hunger deficit).
    pub body_condition_decay_per_unit_hunger_deficit: f32,
    /// Ticket 032 — per-tick recovery rate on `body_condition` per unit
    /// satiation above `body_condition_pivot`. **Default `0.0`**.
    /// Recovery is typically slower than decay (real cats lose
    /// condition fast under fasting and rebuild it slowly under
    /// re-feeding); a treatment value of half the decay rate is a
    /// sensible starting asymmetry.
    pub body_condition_recovery_per_unit_satiation: f32,
    /// Ticket 032 — pivot hunger value above which `body_condition`
    /// recovers and below which it decays. Default `0.5` — neutral when
    /// hunger is at the midpoint of its range. Low-relevance knob when
    /// the decay/recovery rates are 0.
    pub body_condition_pivot: f32,
    /// Ticket 032 — when `true`, the mating gate `is_sated_and_happy`
    /// reads `Fulfillment.body_condition` instead of raw `needs.hunger`
    /// for its hunger leg. **Default `false`** (legacy behavior). Set
    /// to `true` only in concert with non-zero body-condition rates,
    /// otherwise the axis is flat at 1.0 and the gate becomes free.
    pub use_body_condition_for_breeding_gate: bool,
}

impl Default for FulfillmentConstants {
    fn default() -> Self {
        Self {
            social_warmth_base_decay: 0.00008,
            social_warmth_isolation_multiplier: 2.5,
            social_warmth_isolation_range: 3.0,
            social_warmth_groom_other_gain: 0.08,
            social_warmth_bond_proximity_rate: 0.0002,
            social_warmth_bond_proximity_range: 3.0,
            social_warmth_socialize_per_tick: 0.001,
            // Ticket 032 — body_condition axis ships inert.
            body_condition_decay_per_unit_hunger_deficit: 0.0,
            body_condition_recovery_per_unit_satiation: 0.0,
            body_condition_pivot: 0.5,
            use_body_condition_for_breeding_gate: false,
        }
    }
}

// ---------- InfluenceMapConstants (§5.6.3 producer-side knobs) ----------

/// Per-map sense-range knobs for the colony-faction influence-map
/// producers landed in ticket 006. Each map stamps a linear-falloff
/// disc around its source positions; the radius below sets the
/// falloff distance in world tiles.
///
/// **Producer-only at landing.** Consumer cutover (DSE
/// `SpatialConsideration` integration) is owned by ticket 052 — these
/// knobs only shape the on-substrate gradient for now. Numeric values
/// are placeholders chosen to roughly match cat sight range; they
/// become balance-affecting once a `SpatialConsideration` reads them.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InfluenceMapConstants {
    /// Falloff radius (world tiles) for `FoodLocationMap` — §5.6.3 row
    /// #7. Each functional Stores or Kitchen paints a disc of this
    /// radius scaled by `Structure::effectiveness()`.
    pub food_location_sense_range: f32,
    /// Falloff radius for `GardenLocationMap` — §5.6.3 row #10.
    pub garden_location_sense_range: f32,
    /// Falloff radius for `ConstructionSiteMap` — §5.6.3 row #9.
    /// Stamps both `ConstructionSite` (urgency = `1 - progress`) and
    /// damaged `Structure` (urgency = `1 - condition` when condition
    /// falls below `damaged_threshold`).
    pub construction_site_sense_range: f32,
    /// Falloff radius for `KittenCryMap` — §5.6.3 row #13, repurposed
    /// by ticket 156 from a sight-channel "kitten urgency" gradient to
    /// a hearing-channel cry broadcast. Sound travels farther than
    /// sight, so this is bumped above `CARETAKE_TARGET_RANGE = 12`;
    /// a non-parent adult outside caretake range may still hear and
    /// pivot toward the kitten.
    pub kitten_cry_sense_range: f32,
    /// Hunger threshold below which a `KittenDependency` cat starts
    /// crying (and thus painting `KittenCryMap`). A quiet kitten with
    /// `hunger >= threshold` paints nothing; a kitten at `hunger=0`
    /// paints at full strength. Calibrated so the cry precedes the
    /// starvation point with enough lead-time for adults to react.
    pub kitten_cry_hunger_threshold: f32,
    /// Falloff radius for `HerbLocationMap` — §5.6.3 row #8. Each
    /// `Harvestable` herb paints a per-kind disc scaled by growth
    /// stage (`Sprout=0.25` → `Blossom=1.0`). Default mirrors
    /// `disposition.herb_detection_range` (15) so the
    /// `HasHerbsNearby` marker projection (`map.total(pos) > 0`)
    /// agrees with the legacy per-pair `observer_sees_at` predicate
    /// at the in-range threshold.
    pub herb_location_sense_range: f32,
    /// Hunger threshold below which a damaged `Structure` deposits
    /// into `ConstructionSiteMap`. Mirrors §4 `HasDamagedBuilding`'s
    /// `condition < damaged_threshold` predicate so the map view
    /// agrees with the marker view.
    pub damaged_threshold: f32,
}

impl Default for InfluenceMapConstants {
    fn default() -> Self {
        Self {
            food_location_sense_range: 15.0,
            garden_location_sense_range: 15.0,
            construction_site_sense_range: 15.0,
            kitten_cry_sense_range: 30.0,
            kitten_cry_hunger_threshold: 0.5,
            herb_location_sense_range: 15.0,
            damaged_threshold: 0.4,
        }
    }
}

// ---------- env-var override hook ----------

/// Reads the `CLOWDER_OVERRIDES` env var (if set) as a JSON object and
/// deep-merges it into [`SimConstants::default`]. Used by
/// `scripts/hypothesize.py` and ad-hoc balance tuning to drive a
/// treatment run without rebuilding the binary. Format mirrors the
/// `_header.constants` block, e.g.
///
/// ```text
/// CLOWDER_OVERRIDES='{"fulfillment":{"social_warmth_socialize_per_tick":0.002}}'
/// ```
///
/// On parse error the override is dropped with a stderr warning and
/// defaults are used — never silently corrupts a run. The applied
/// patch (or `null` when no override) is exposed via
/// [`SimConstants::applied_overrides_snapshot`] so the events.jsonl
/// header can echo it for downstream reproducibility.
impl SimConstants {
    pub fn from_env() -> Self {
        match std::env::var("CLOWDER_OVERRIDES") {
            Err(_) => Self::default(),
            Ok(s) if s.trim().is_empty() => Self::default(),
            Ok(raw) => match serde_json::from_str::<serde_json::Value>(&raw) {
                Err(e) => {
                    eprintln!(
                        "Warning: CLOWDER_OVERRIDES is not valid JSON ({e}); using defaults."
                    );
                    Self::default()
                }
                Ok(patch) => Self::default_with_overrides(&patch).unwrap_or_else(|e| {
                    eprintln!("Warning: CLOWDER_OVERRIDES failed to apply ({e}); using defaults.");
                    Self::default()
                }),
            },
        }
    }

    /// Returns the parsed `CLOWDER_OVERRIDES` JSON value (or
    /// `serde_json::Value::Null` if unset/empty/malformed) so callers
    /// can record what was applied. This is what
    /// `write_jsonl_headers` echoes into the events log header.
    pub fn applied_overrides_snapshot() -> serde_json::Value {
        match std::env::var("CLOWDER_OVERRIDES") {
            Err(_) => serde_json::Value::Null,
            Ok(s) if s.trim().is_empty() => serde_json::Value::Null,
            Ok(raw) => serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null),
        }
    }

    fn default_with_overrides(patch: &serde_json::Value) -> Result<Self, String> {
        let mut base =
            serde_json::to_value(Self::default()).map_err(|e| format!("serialize default: {e}"))?;
        deep_merge(&mut base, patch);
        serde_json::from_value(base).map_err(|e| format!("deserialize merged: {e}"))
    }
}

/// Recursively merges `patch` into `target`. Object keys in `patch`
/// overwrite or extend `target`; non-object values replace whatever
/// is at the same path.
fn deep_merge(target: &mut serde_json::Value, patch: &serde_json::Value) {
    use serde_json::Value;
    match (target, patch) {
        (Value::Object(t), Value::Object(p)) => {
            for (k, v) in p {
                deep_merge(t.entry(k.clone()).or_insert(Value::Null), v);
            }
        }
        (slot, other) => {
            *slot = other.clone();
        }
    }
}

// ---------- PracticeConstants (ticket 127) ----------

/// Ticket 127 — per-practice JointIntention tuning. One block per
/// practice; for 127 only Courtship exists. Each practice carries its
/// own matchmaker / drop-gate / bias-multiplier shape — co-mentoring,
/// joint cache-stocking, and play-bouts can compose against this
/// taxonomy as follow-on tickets land.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PracticeConstants {
    /// Courtship practice — successor to `PairingConstants`. During the
    /// 127 migration both blocks coexist (Commit A mirrors; Commit C
    /// deletes `pairing`).
    #[serde(default)]
    pub courtship: CourtshipPracticeConstants,
    /// Ticket 276 — PlayBout practice. Hosts the `play` continuity
    /// canary on JointIntention substrate.
    #[serde(default)]
    pub play_bout: PlayBoutPracticeConstants,
}

/// Ticket 127 — Courtship-practice knobs. Mirrors `PairingConstants`
/// 1:1 (so the migration is mechanical) plus `stage_stall_ticks` for
/// the novel `StageStalled` drop branch. Doc-comments mirror
/// `PairingConstants` for the carry-over fields.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CourtshipPracticeConstants {
    /// Manhattan-distance candidate filter when scanning for an
    /// emission target. Wider than `MATE_TARGET_RANGE = 10` because
    /// joint practices are multi-season — a cat may pursue a partner
    /// across the colony, not require adjacency every tick.
    pub candidate_range: f32,
    /// Minimum quality score for a Friends-bonded peer to trigger
    /// emission. See `PairingConstants::emission_threshold` for the
    /// 257-recalibration rationale.
    pub emission_threshold: f32,
    /// Multiplier applied to fondness + familiarity deltas when a
    /// paired actor's resolver target matches its
    /// `JointIntention.partner` AND `JointIntention.practice ==
    /// Courtship`. See `PairingConstants::bias_multiplier` for the
    /// 257-Commit-B calibration rationale.
    pub bias_multiplier: f32,
    /// `DesireDrift` floor on the romantic axis. Both this and
    /// `fondness_floor` must be breached simultaneously for the branch
    /// to fire.
    pub romantic_floor: f32,
    /// `DesireDrift` floor on the fondness axis.
    pub fondness_floor: f32,
    /// Per-axis weights of the courtship-quality scalar. Sum to 1.0.
    pub quality_fondness_weight: f32,
    pub quality_romantic_weight: f32,
    pub quality_bond_weight: f32,
    /// Ticket 127 — max ticks a cat may sit in a single
    /// `PracticeStage` before `StageStalled` fires. Default 10_000 ≈
    /// 10 sim-days at default constants (1000 ticks/day). Generous
    /// enough to absorb temporary blockers (a Tom waiting out winter,
    /// a Queen between estrus cycles) while still catching pairs that
    /// never progress (compatibility predicates pass but the bond
    /// never tips into Partners).
    pub stage_stall_ticks: u64,
}

impl Default for CourtshipPracticeConstants {
    fn default() -> Self {
        // Carry-over defaults from the prior `PairingConstants`
        // (deleted in 127 Commit C). Migration parity is mechanical —
        // these values are the post-272 calibration referenced in
        // `logs/baselines/current.json` and the seed-42 mating-chain
        // verdict gate.
        Self {
            candidate_range: 25.0,
            emission_threshold: 0.20,
            bias_multiplier: 1.5,
            romantic_floor: 0.05,
            fondness_floor: 0.30,
            quality_fondness_weight: 0.40,
            quality_romantic_weight: 0.40,
            quality_bond_weight: 0.20,
            stage_stall_ticks: 10_000,
        }
    }
}

/// Ticket 276 — PlayBout-practice knobs. Hosts the `play` continuity
/// canary on JointIntention substrate. Defaults are first-pass;
/// post-Commit-A soak data will refine the cooldown / playfulness
/// floors against the pre-066 stable range (50–150 play events/soak).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlayBoutPracticeConstants {
    /// Manhattan-distance candidate filter when scanning for a play
    /// partner. Tighter than Courtship (4 vs 25) — play is opportunistic
    /// co-presence rather than colony-wide pursuit.
    pub candidate_range: f32,
    /// Minimum quality score for a peer to trigger PlayBout emission.
    /// Score = playfulness_avg + mood_valence_avg + bond_score; floor of
    /// 0.5 keeps emissions out of the noise-floor for low-mood pairs.
    pub emission_threshold: f32,
    /// Multiplier applied to fondness / familiarity deltas when the
    /// actor's resolver target matches its `JointIntention.partner` AND
    /// `JointIntention.practice == PlayBout`. Lighter lift than
    /// Courtship's 1.5 — PlayBout is supposed to be a low-intensity
    /// background social practice, not a softmax-dominator. (Bias
    /// readers wire in a follow-on ticket — Commit A relies on stage-
    /// tick gates for stage progression rather than bias-reader
    /// `JointInteractionObserved` messages.)
    pub bias_multiplier: f32,
    /// Minimum `Personality.playfulness` for either partner to qualify
    /// as eligible. Mirrors the pre-276 direct-emit gate at
    /// `personality_events.rs:83`; both partners must clear.
    pub playfulness_floor: f32,
    /// Minimum `Mood.valence` for either partner to qualify as
    /// eligible. Mirrors the pre-276 direct-emit gate at
    /// `personality_events.rs:84`; both partners must clear.
    pub mood_valence_floor: f32,
    /// Ticks a cat spends in `PracticeStage::PlayBoutApproach` before
    /// advancing to `PlayBoutBouting`. Stage-tick gate (no bias-reader
    /// dependency) — the matchmaker emitted on co-presence within
    /// `candidate_range`; this gate models the brief approach window
    /// before play kicks off.
    pub approach_duration_ticks: u64,
    /// Ticks a cat spends in `PracticeStage::PlayBoutBouting` before
    /// advancing to `PlayBoutCooldown`. 60 ticks ≈ 6 sim-minutes; long
    /// enough for the Bouting cascade to fire repeatedly, short enough
    /// that paired cats churn through bouts (rather than getting locked
    /// in one practice for a sim-hour).
    pub bouting_duration_ticks: u64,
    /// Ticks a cat spends in `PracticeStage::PlayBoutCooldown` before
    /// `JointDropBranch::Completed` fires and the JI is removed. The
    /// `EventKind::JointPlayBoutCompleted` event emits on this drop,
    /// incrementing `continuity_tallies["play"]`.
    pub cooldown_duration_ticks: u64,
    /// Per-practice `stage_stall_ticks` override for PlayBout. Tighter
    /// than Courtship's 10_000 because PlayBout stages are short —
    /// Approach should turn into Bouting within a sim-day or the pair
    /// is misclassified (e.g., they're not actually in proximity any
    /// more).
    pub stage_stall_ticks: u64,
}

impl Default for PlayBoutPracticeConstants {
    fn default() -> Self {
        Self {
            candidate_range: 4.0,
            emission_threshold: 0.5,
            bias_multiplier: 1.2,
            playfulness_floor: 0.6,
            mood_valence_floor: 0.0,
            approach_duration_ticks: 30,
            bouting_duration_ticks: 60,
            cooldown_duration_ticks: 30,
            stage_stall_ticks: 1_000,
        }
    }
}

// ---------- PlanningSubstrateConstants (sub-epic 071) ----------

/// Knobs for the unified `plan_substrate` API (sub-epic 071). Lifts
/// cross-tick defenses (memory-of-failure / reservation gating /
/// fallback) out of inline call sites and into the IAUS engine.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanningSubstrateConstants {
    /// Ticket 080 — TTL for `Reserved` markers on contended resource
    /// targets (carcass, herb tile, prey, mate). Default 600 ticks
    /// ≈ 1 in-sim hour at the 1000-ticks-per-day scale; tuneable
    /// post-soak per ticket Out-of-scope.
    pub reservation_ttl_ticks: u64,
}

// 292 — `target_failure_cooldown_ticks` (073) retired with the
// `RecentTargetFailures` map; the target-cooldown recovery window is
// governed by `belief_facets.predictability` decay tunables now.

impl Default for PlanningSubstrateConstants {
    fn default() -> Self {
        Self {
            reservation_ttl_ticks: 600,
        }
    }
}

// ---------- EscapeViabilityConstants (ticket 103) ----------

/// Knobs for the `escape_viability` perception scalar
/// (`crate::systems::interoception::escape_viability`). Pure
/// threat-coupled physics; ambient closed-space anxiety
/// (claustrophobia / agoraphobia) lives on a separate axis owned by
/// ticket 126's phobia modifier family.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EscapeViabilityConstants {
    /// Half-extent of the bounding box scanned for walkable tiles
    /// around the cat. The full sample is `(2 * sprint_radius + 1)²`
    /// tiles. Default 3 → 7×7 = 49-tile sample, roughly the cat's
    /// "two-turn flight options" footprint at the current 1
    /// tile/tick movement rate.
    #[serde(default = "default_escape_viability_sprint_radius")]
    pub sprint_radius: f32,
    /// Multiplier on the terrain-openness term
    /// (walkable / box-area). `terrain_weight + mobility_weight +
    /// dependent_weight` should remain ≤ 1.0 so the scalar saturates
    /// at 1.0 in fully open terrain with the maximum mobility
    /// advantage and no dependents. Default 0.6 (ticket 138 — was
    /// 0.7, reduced by 0.1 to make room for the new mobility term).
    #[serde(default = "default_escape_viability_terrain_weight")]
    pub terrain_weight: f32,
    /// Multiplier on the mobility-advantage term (ticket 138). The
    /// raw mobility advantage is
    /// `clamp((own_per_tick − threat_per_tick) / mobility_normalization, −1, +1)`
    /// remapped to `[0, 1]` via `advantage × 0.5 + 0.5`. A cat at
    /// 1.0/tick facing a snake at 0.5/tick saturates at full
    /// advantage → contributes `mobility_weight × 1.0`. Default 0.2.
    #[serde(default = "default_escape_viability_mobility_weight")]
    pub mobility_weight: f32,
    /// Normalizer on the (own − threat) cadence delta. With default
    /// 1.0, a 1.0-vs-0.5 cadence gap saturates the advantage at
    /// +0.5 raw → +1.0 remapped → full mobility-term contribution.
    /// Tune up to soften the term, down to make it bite at smaller
    /// cadence gaps. Default 1.0.
    #[serde(default = "default_escape_viability_mobility_normalization")]
    pub mobility_normalization: f32,
    /// Multiplier on the dependent-presence penalty term. Subtracted
    /// from the terrain + mobility composition when
    /// `has_nearby_dependent` is true, modeling cost-of-abandonment
    /// for parents and pair-bonded cats. Default 0.2 (ticket 138 —
    /// was 0.3, reduced by 0.1 to keep the weight sum ≤ 1.0 after
    /// adding mobility_weight=0.2).
    #[serde(default = "default_escape_viability_dependent_weight")]
    pub dependent_weight: f32,
    /// Bool-style penalty magnitude — when present, the dependent
    /// term is exactly `dependent_weight * dependent_penalty`. Held
    /// at `1.0` by default so all tuning happens via
    /// `dependent_weight`; left as a separate knob to allow phobia /
    /// trait modifiers (ticket 126) to override the per-cat
    /// magnitude without disturbing the global weight. Default 1.0.
    #[serde(default = "default_escape_viability_dependent_penalty")]
    pub dependent_penalty: f32,
    /// Threshold the `CoverAvailabilityMap` cell must exceed for a
    /// cat to be `HideEligible` (ticket 423). v1 ships boolean cells
    /// (0.0 / 1.0), so `0.5` is the midpoint — any "cover present"
    /// cell qualifies. Tuning hook for the 170 balance follow-on if
    /// the map gains a distance-gradient cell representation.
    /// Default 0.5.
    #[serde(default = "default_cover_availability_threshold")]
    pub cover_availability_threshold: f32,
}

fn default_escape_viability_sprint_radius() -> f32 {
    3.0
}
fn default_escape_viability_terrain_weight() -> f32 {
    0.6
}
fn default_escape_viability_mobility_weight() -> f32 {
    0.2
}
fn default_escape_viability_mobility_normalization() -> f32 {
    1.0
}
fn default_escape_viability_dependent_weight() -> f32 {
    0.2
}
fn default_escape_viability_dependent_penalty() -> f32 {
    1.0
}
fn default_cover_availability_threshold() -> f32 {
    0.5
}

impl Default for EscapeViabilityConstants {
    fn default() -> Self {
        Self {
            sprint_radius: default_escape_viability_sprint_radius(),
            terrain_weight: default_escape_viability_terrain_weight(),
            mobility_weight: default_escape_viability_mobility_weight(),
            mobility_normalization: default_escape_viability_mobility_normalization(),
            dependent_weight: default_escape_viability_dependent_weight(),
            dependent_penalty: default_escape_viability_dependent_penalty(),
            cover_availability_threshold: default_cover_availability_threshold(),
        }
    }
}

// ---------- BeliefsConstants (ticket 258) ----------

/// Per-facet EMA + decay tunables for the C3 belief substrate.
///
/// EMA update math (pass A, on evidence): `value ← value + lr × (observed − value)`.
/// Passive decay (pass B, every `BeliefsConstants::decay_stagger_period`
/// ticks): `value ← value + decay_rate_to_prior × (prior − value)`.
/// `strength` rises by `strength_per_observation` on pass A (clamped to 1.0)
/// and decays linearly by `strength_decay_per_tick × period` on pass B.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BeliefAxisTunables {
    pub learning_rate: f32,
    pub decay_rate_to_prior: f32,
    pub strength_per_observation: f32,
    pub strength_decay_per_tick: f32,
}

impl BeliefAxisTunables {
    /// Tunables for "fast-timescale" facets (recency-of-threat-cue,
    /// perceived-injury-level, perceived-intent-clarity). Tuned for
    /// O(seconds) salience and O(minutes) decay-to-prior.
    pub fn fast() -> Self {
        Self {
            learning_rate: 0.3,
            decay_rate_to_prior: 0.001,
            strength_per_observation: 0.3,
            strength_decay_per_tick: 0.0005,
        }
    }
    /// Tunables for "slow-timescale" facets
    /// (perceived-violence-capability, affiliation-history, predictability).
    /// Reputation-flavored — many observations to shift, slow drift back.
    pub fn slow() -> Self {
        Self {
            learning_rate: 0.1,
            decay_rate_to_prior: 0.0001,
            strength_per_observation: 0.1,
            strength_decay_per_tick: 0.00005,
        }
    }
}

/// Cat-perceived violence-capability priors per predator/cat species. Seeds
/// `<Predator>` and `<Cat>` mental models on first encounter via the
/// `Implant` evidence kind. Range `[0.0, 1.0]` — higher means "I instinctually
/// expect this species to be dangerous to me".
///
/// Wildlife-side reciprocal priors live in a future ticket when wildlife
/// AI gains its own mental-model substrate (sibling 265).
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct SpeciesViolencePriors {
    /// Cats don't inherently expect violence from other cats.
    pub cat: f32,
    /// Foxes attack cats opportunistically.
    pub fox: f32,
    /// Raptors are a hard-counter threat to cats outside cover.
    pub hawk: f32,
    /// Snake bites are harmful but snakes typically flee first.
    pub snake: f32,
    /// Apex threat — instilled at world-gen for narrative weight.
    pub shadow_fox: f32,
    /// 265: wildlife-perceiver rows — how dangerous a CAT looks to each
    /// wildlife species, implanted into the wildlife entity's own
    /// `CatBeliefs` model on first encounter (`belief_integrator`
    /// Pass B, symmetric to the cat-side `PredatorBeliefs` implant).
    /// Facets are populated-but-unread until the step-21 activation
    /// lifts the wildlife fleeing DSEs' `perceived_cat_threat` weights.
    ///
    /// A lone cat is a real fight risk to a fox; a mobbed fox loses.
    #[serde(default = "default_cat_perceived_by_fox")]
    pub cat_perceived_by_fox: f32,
    /// Cats rarely threaten a hawk on the wing; ground encounters are
    /// avoidable at will.
    #[serde(default = "default_cat_perceived_by_hawk")]
    pub cat_perceived_by_hawk: f32,
    /// Cats are practiced snake-killers — highest wildlife-perceiver
    /// prior.
    #[serde(default = "default_cat_perceived_by_snake")]
    pub cat_perceived_by_snake: f32,
    /// Apex predator — cats read as prey more than threat (wards, not
    /// cats, repel it). Consumers land with ticket 310.
    #[serde(default = "default_cat_perceived_by_shadow_fox")]
    pub cat_perceived_by_shadow_fox: f32,
    /// 314: prey-perceiver rows — how dangerous each threat species
    /// looks to prey, implanted into the prey entity's
    /// `PredatorBeliefs` on first encounter (`belief_integrator`
    /// Pass B, symmetric to the cat- and wildlife-side implants).
    /// Read by the affordance writer's prey-perceiver `Bolt`
    /// heuristic; downstream DSE consumers arrive with ticket 266
    /// (prey-side AI), so the rows are behaviorally dormant until
    /// then. One row per threat species, not per prey kind — prey
    /// share an instinct table.
    ///
    /// Cats are the archetypal prey-killer.
    #[serde(default = "default_cat_perceived_by_prey")]
    pub cat_perceived_by_prey: f32,
    /// Foxes hunt the same small-mammal menu.
    #[serde(default = "default_fox_perceived_by_prey")]
    pub fox_perceived_by_prey: f32,
    /// Raptor shadow — highest ambient dread for ground prey.
    #[serde(default = "default_hawk_perceived_by_prey")]
    pub hawk_perceived_by_prey: f32,
    /// Ambush-only threat; less feared in the open.
    #[serde(default = "default_snake_perceived_by_prey")]
    pub snake_perceived_by_prey: f32,
    /// Apex threat reads as lethal to everything beneath it.
    #[serde(default = "default_shadow_fox_perceived_by_prey")]
    pub shadow_fox_perceived_by_prey: f32,
}

impl Default for SpeciesViolencePriors {
    fn default() -> Self {
        Self {
            cat: 0.0,
            fox: 0.5,
            hawk: 0.6,
            snake: 0.4,
            shadow_fox: 0.95,
            cat_perceived_by_fox: default_cat_perceived_by_fox(),
            cat_perceived_by_hawk: default_cat_perceived_by_hawk(),
            cat_perceived_by_snake: default_cat_perceived_by_snake(),
            cat_perceived_by_shadow_fox: default_cat_perceived_by_shadow_fox(),
            cat_perceived_by_prey: default_cat_perceived_by_prey(),
            fox_perceived_by_prey: default_fox_perceived_by_prey(),
            hawk_perceived_by_prey: default_hawk_perceived_by_prey(),
            snake_perceived_by_prey: default_snake_perceived_by_prey(),
            shadow_fox_perceived_by_prey: default_shadow_fox_perceived_by_prey(),
        }
    }
}

/// 314: cat violence prior as perceived by prey.
fn default_cat_perceived_by_prey() -> f32 {
    0.9
}

/// 314: fox violence prior as perceived by prey.
fn default_fox_perceived_by_prey() -> f32 {
    0.8
}

/// 314: hawk violence prior as perceived by prey.
fn default_hawk_perceived_by_prey() -> f32 {
    0.85
}

/// 314: snake violence prior as perceived by prey.
fn default_snake_perceived_by_prey() -> f32 {
    0.6
}

/// 314: shadow-fox violence prior as perceived by prey.
fn default_shadow_fox_perceived_by_prey() -> f32 {
    0.9
}

/// 265: cat violence prior as perceived by foxes.
fn default_cat_perceived_by_fox() -> f32 {
    0.5
}

/// 265: cat violence prior as perceived by hawks.
fn default_cat_perceived_by_hawk() -> f32 {
    0.3
}

/// 265: cat violence prior as perceived by snakes.
fn default_cat_perceived_by_snake() -> f32 {
    0.65
}

/// 265: cat violence prior as perceived by shadow foxes.
fn default_cat_perceived_by_shadow_fox() -> f32 {
    0.2
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BeliefsConstants {
    pub perceived_injury_level: BeliefAxisTunables,
    pub perceived_intent_clarity: BeliefAxisTunables,
    pub recency_of_threat_cue: BeliefAxisTunables,
    pub perceived_violence_capability: BeliefAxisTunables,
    pub affiliation_history: BeliefAxisTunables,
    pub predictability: BeliefAxisTunables,
    /// 261: state-flavored read on "is this subject hostile to me right
    /// now". Fast-timescale — observed aggression updates quickly and
    /// decays quickly; bond reputation (slow) lives on
    /// `affiliation_history`.
    pub perceived_hostility: BeliefAxisTunables,
    /// 261: how open this subject is to affiliative practice right now
    /// (grooming, courtship, mentoring). Slow-timescale — receptivity
    /// is a stable disposition modulated by current life-stage, not a
    /// per-tick reactive read.
    pub perceived_receptivity: BeliefAxisTunables,
    /// 293: per-location prey-yield belief on [`LocationBeliefs`]. Slow-
    /// timescale — spatial prey availability is stable knowledge built
    /// across many observations, not a fast reactive read. Replaces the
    /// legacy `HuntingPriors` proxy grid.
    pub prey_yield: BeliefAxisTunables,
    pub species_violence_priors: SpeciesViolencePriors,
    /// Passive-decay pass runs every Nth tick, with per-cat phase staggered
    /// by `entity.index() % period`. Default 20 — amortizes cost; missing
    /// 19 ticks of decay at `decay_rate_to_prior ≈ 0.001` is ~2% error,
    /// well under observation threshold.
    pub decay_stagger_period: u64,
    /// 308: `HasLowWardReserve` marker fires when the cat's belief estimate
    /// of colony thornbriar count is `<= threshold`. Default `2` — gives a
    /// few ticks of anticipatory lead before SetWard would fail outright.
    pub low_ward_reserve_threshold: u32,
    /// 308: `strength` bump applied to a `ReserveBelief` on each observation
    /// (`ReserveDeposited` / `ReserveConsumed` / `InventoryObserved`).
    /// Clamped to 1.0. Mirrors `BeliefAxisTunables::strength_per_observation`
    /// shape; tuned faster than slow-belief axes because reserve state
    /// changes on a per-action cadence.
    pub reserve_strength_per_observation: f32,
    /// 308: per-stagger strength drain on `ReserveBelief` entries. When
    /// strength falls to `<= EPSILON`, the entry is dropped from the
    /// cat's `ColonyReservesBelief.reserves` map. Mirrors
    /// `BeliefAxisTunables::strength_decay_per_tick × period`.
    pub reserve_decay_per_stagger: f32,
    /// 279: tick count at which a `SustainedCoPresence` event's
    /// EMA observed-value saturates at `OBSERVED_MAX`. Shorter windows
    /// produce a proportionally weaker lift; ticks held beyond this point
    /// don't lift further (cooldown re-arms the emit anyway). Lives on
    /// `BeliefsConstants` rather than `PlayCueEmission` because it shapes
    /// the *belief lift* magnitude, not the emit cadence.
    pub sustained_copresence_saturation_ticks: u32,
}

impl Default for BeliefsConstants {
    fn default() -> Self {
        Self {
            perceived_injury_level: BeliefAxisTunables::fast(),
            perceived_intent_clarity: BeliefAxisTunables::fast(),
            recency_of_threat_cue: BeliefAxisTunables::fast(),
            perceived_violence_capability: BeliefAxisTunables::slow(),
            affiliation_history: BeliefAxisTunables::slow(),
            // 290: inline predictability tunables to preserve the legacy
            // RDF-style snap-to-0 on failure (`learning_rate = 1.0`)
            // and ~3000-tick recovery toward `prior = 1.0`
            // (`decay_rate_to_prior = 0.00075`, applied per Pass-B stagger
            // = 20 ticks → ~0.015 fractional gap closure per pass).
            //
            // Iteration history (single-seed-42, see `docs/balance/290-rdf-reader-cutover.md`):
            //   - iter-1 `decay = 0.00075` (kept): aggregate +12.9%, pop
            //     +18%, bonds +52%, kittens +67% (activity-permissive
            //     drift; survival gates clean).
            //   - iter-2 `decay = 0.00035` (rejected): tighter midpoint
            //     match but shelter score -100% / health -37% / welfare
            //     -11% — slower recovery starves shelter-seeking
            //     dispositions of retry opportunity.
            //
            // The exponential EMA can't simultaneously match the legacy
            // linear midpoint AND endpoint; iter-1 prefers the permissive
            // mid-curve (~0.55 at t=1000 vs legacy 0.25), which the
            // verdict surfaces as more colony activity rather than
            // suppressed welfare. The drift is documented as substrate-
            // revealing balance change per the four-artifact write-up.
            //
            // `strength_*` retain `slow()` values (govern eviction, not
            // the cooldown signal's shape; validated null-drift in 258).
            predictability: BeliefAxisTunables {
                learning_rate: 1.0,
                decay_rate_to_prior: 0.00075,
                strength_per_observation: 0.1,
                strength_decay_per_tick: 0.00005,
            },
            perceived_hostility: BeliefAxisTunables::fast(),
            perceived_receptivity: BeliefAxisTunables::slow(),
            prey_yield: BeliefAxisTunables::slow(),
            species_violence_priors: SpeciesViolencePriors::default(),
            decay_stagger_period: 20,
            low_ward_reserve_threshold: 2,
            reserve_strength_per_observation: 0.3,
            reserve_decay_per_stagger: 0.05,
            // 279: 60 ticks ≈ a short interaction window. ticks_held / 60
            // yields a lift in [0,1]; sustained co-presence over many
            // seconds saturates at OBSERVED_MAX.
            sustained_copresence_saturation_ticks: 60,
        }
    }
}

// ---------- BeliefAggregationConstants (ticket 294) ----------

/// Colony-aggregation tunables for the C3 belief substrate. The 294
/// `RecentAmbushMap` retirement reads `LocationBeliefs.recency_of_threat_cue`
/// via `belief_aggregation::aggregated_location_belief`, which gates
/// each cat's contribution by their `Facet::strength` to silence
/// not-yet-decayed-but-no-longer-confident memories.
///
/// Default `min_strength_to_contribute = 0.0` admits every cat that has
/// any entry at the bucket — matches the pre-294 semantics where every
/// witnessed ambush bumped the colony-shared field regardless of how
/// long ago. Raising the floor is the lever for tuning ward placement
/// toward "areas where cats *still actively remember* ambushes" vs.
/// "areas where someone once saw one."
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct BeliefAggregationConstants {
    /// Per-cat `Facet::strength` floor below which the cat's belief
    /// does not contribute to the aggregated colony view. Default 0.0
    /// — every cat with any entry contributes (pre-294 semantics).
    pub min_strength_to_contribute: f32,
}

impl Default for BeliefAggregationConstants {
    fn default() -> Self {
        Self {
            min_strength_to_contribute: 0.0,
        }
    }
}

// ---------- ShelterBeliefConstants (ticket 374) ----------

/// Tunables for the per-cat `ShelterBeliefs` housing-security facet,
/// ticket 374. Four orthogonal sub-axes (belonging, quality, continuity,
/// threat) each carry their own EMA learning-rate toward observed values;
/// continuity is the only axis updated passively (per-tick accrual when
/// the cat is within range of its home_den, decay when away).
///
/// Reader composition weights (`continuity_weight`, `insecurity_threshold`)
/// shape the Phase C consumer rewrite of `compute_shelter` and
/// `pressure.shelter`. Defaults are starting-point values; the Phase D
/// hypothesize cycle tunes them against the post-494 baseline.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ShelterBeliefConstants {
    /// EMA step toward the observed value on `DenClaimed`/`DenLost`
    /// (target 1.0/0.0). High because belonging is event-keyed and
    /// shouldn't lag a single observation.
    pub belonging_learning_rate: f32,
    /// EMA step toward observed value on `DenDamaged`/`DenRepaired`.
    /// Slower than belonging — the cat's belief about quality builds
    /// across several condition-crossing events, not one.
    pub quality_learning_rate: f32,
    /// EMA step toward observed value on `DenSieged`/`DenSiegeBroken`.
    /// Fast — threat is a reactive read; a single siege event should
    /// snap the belief up promptly.
    pub threat_learning_rate: f32,
    /// Per-stagger continuity accrual when within `home_den_radius`
    /// of the claimed `home_den`. Slow enough that "time at home"
    /// builds across many ticks.
    pub continuity_accrual_per_stagger: f32,
    /// Per-stagger continuity decay when outside `home_den_radius`
    /// of the claimed `home_den`, or when `home_den == None`. Slow —
    /// a cat at work hunting should not lose continuity quickly.
    pub continuity_decay_per_stagger: f32,
    /// Tile distance considered "at home" for continuity accrual.
    /// Matches the historical `den_shelter_radius` default (4.0) so
    /// pre-374 spatial-proximity tuning intuition transfers.
    pub home_den_radius: f32,
    /// Tile distance from a cat's home_den at which an active fox is
    /// considered to be sieging the den. Triggers `DenSieged`/
    /// `DenSiegeBroken` emit.
    pub siege_proximity: f32,
    /// `Structure::condition` thresholds at which `DenDamaged` /
    /// `DenRepaired` emit. Crossing a threshold downward triggers
    /// `DenDamaged`; crossing upward triggers `DenRepaired`. Matches
    /// `Structure::effectiveness()` knees at 0.2 and 0.5.
    pub damage_threshold_low: f32,
    pub damage_threshold_high: f32,
    /// Weight applied to the `continuity` sub-axis in the welfare
    /// rollup at `compute_shelter`. The other three axes compose
    /// multiplicatively (belonging × quality × (1 - threat));
    /// continuity scales the result. Default 1.0 = full weight.
    pub continuity_weight: f32,
    /// Per-cat housing-insecurity threshold above which the cat
    /// contributes to `pressure.shelter`. Computed as
    /// `1.0 - belonging * quality * (1 - threat)`. Higher = stricter
    /// (fewer cats counted as insecure). Default 0.5 — a cat with
    /// belonging 0.5 contributes; a cat fully sheltered does not.
    pub insecurity_threshold: f32,
}

impl Default for ShelterBeliefConstants {
    fn default() -> Self {
        Self {
            belonging_learning_rate: 0.8,
            quality_learning_rate: 0.2,
            threat_learning_rate: 0.5,
            continuity_accrual_per_stagger: 0.01,
            continuity_decay_per_stagger: 0.002,
            home_den_radius: 4.0,
            siege_proximity: 6.0,
            damage_threshold_low: 0.2,
            damage_threshold_high: 0.5,
            // 374: 0.3 default — belonging/quality/(1-threat) provides
            // ~70% of a cat's security signal regardless of continuity,
            // and continuity adds up to ~30% extra credit for cats that
            // have spent time at home. Reserved for tuning; the
            // pre-soak default of 1.0 silently zeroed welfare.shelter
            // because cats with home_den but continuity=0 contributed 0.
            continuity_weight: 0.3,
            insecurity_threshold: 0.5,
        }
    }
}

// ---------- AffordancesConstants (ticket 261) ----------

/// Per-action heuristic-input weights + eligibility floor. Each
/// `ActionKind` heuristic in `affordance_writer` interprets the four
/// weight slots in its own (documented) way; this uniform shape keeps the
/// `SimConstants` footprint bounded while still letting balance work
/// retune individual kinds. `min_eligibility` floors the raw heuristic —
/// values below it write `0.0` so consumers see a hard gate, not a faint
/// signal.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct AffordanceWeights {
    /// Slot 1 — typically "proximity / distance closeness" or the primary
    /// spatial input.
    pub w1: f32,
    /// Slot 2 — typically a belief-facet read (intent_clarity, hostility,
    /// affiliation, receptivity, violence_capability, …).
    pub w2: f32,
    /// Slot 3 — typically a capability axis (speed differential, my health,
    /// my inventory, cover at my position).
    pub w3: f32,
    /// Slot 4 — typically a context modifier (allies nearby, fox-scent
    /// territory, ward coverage, age-stage relevance).
    pub w4: f32,
    /// Heuristic outputs below this threshold are written as `0.0`. The
    /// hard gate makes "action not afforded" visible to consumers as a
    /// distinct signal from "afforded but weakly".
    pub min_eligibility: f32,
}

impl AffordanceWeights {
    /// Plausible v1 default — four equal slots and a 0.10 floor.
    /// Consumer tickets retune per-kind during their drift methodology.
    pub fn default_quartet() -> Self {
        Self {
            w1: 0.25,
            w2: 0.25,
            w3: 0.25,
            w4: 0.25,
            min_eligibility: 0.10,
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct PredationAffordanceConstants {
    pub stalk: AffordanceWeights,
    pub chase: AffordanceWeights,
    pub pounce: AffordanceWeights,
    pub dive: AffordanceWeights,
    pub strike: AffordanceWeights,
    pub ambush: AffordanceWeights,
}

impl Default for PredationAffordanceConstants {
    fn default() -> Self {
        Self {
            stalk: AffordanceWeights::default_quartet(),
            chase: AffordanceWeights::default_quartet(),
            pounce: AffordanceWeights::default_quartet(),
            dive: AffordanceWeights::default_quartet(),
            strike: AffordanceWeights::default_quartet(),
            ambush: AffordanceWeights::default_quartet(),
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ThreatResponseAffordanceConstants {
    pub flee: AffordanceWeights,
    pub fight: AffordanceWeights,
    pub freeze: AffordanceWeights,
    pub fawn: AffordanceWeights,
}

impl Default for ThreatResponseAffordanceConstants {
    fn default() -> Self {
        Self {
            flee: AffordanceWeights::default_quartet(),
            fight: AffordanceWeights::default_quartet(),
            freeze: AffordanceWeights::default_quartet(),
            fawn: AffordanceWeights::default_quartet(),
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct ConflictLowAffordanceConstants {
    pub threaten: AffordanceWeights,
    pub posture: AffordanceWeights,
    pub hiss: AffordanceWeights,
}

impl Default for ConflictLowAffordanceConstants {
    fn default() -> Self {
        Self {
            threaten: AffordanceWeights::default_quartet(),
            posture: AffordanceWeights::default_quartet(),
            hiss: AffordanceWeights::default_quartet(),
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct SocialAffordanceConstants {
    pub socialize: AffordanceWeights,
    pub groom_other: AffordanceWeights,
    pub mate: AffordanceWeights,
    pub mentor: AffordanceWeights,
    pub care: AffordanceWeights,
    pub feed_kitten: AffordanceWeights,
}

impl Default for SocialAffordanceConstants {
    fn default() -> Self {
        Self {
            socialize: AffordanceWeights::default_quartet(),
            groom_other: AffordanceWeights::default_quartet(),
            mate: AffordanceWeights::default_quartet(),
            mentor: AffordanceWeights::default_quartet(),
            care: AffordanceWeights::default_quartet(),
            feed_kitten: AffordanceWeights::default_quartet(),
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct PreySideAffordanceConstants {
    pub bolt: AffordanceWeights,
    pub scatter_group: AffordanceWeights,
}

impl Default for PreySideAffordanceConstants {
    fn default() -> Self {
        Self {
            bolt: AffordanceWeights::default_quartet(),
            scatter_group: AffordanceWeights::default_quartet(),
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct AffordancesConstants {
    /// Manhattan-distance sensing radius for affordance writes. Pairs of
    /// `(perceiver, target)` beyond this range are not written; consumers
    /// see the `0.0` default. Mirrors the belief-substrate `WITNESS_RANGE`
    /// of 10 tiles by default.
    pub sensing_range: f32,
    pub predation: PredationAffordanceConstants,
    pub threat_response: ThreatResponseAffordanceConstants,
    pub conflict_low: ConflictLowAffordanceConstants,
    pub social: SocialAffordanceConstants,
    pub prey_side: PreySideAffordanceConstants,
}

impl Default for AffordancesConstants {
    fn default() -> Self {
        Self {
            sensing_range: 10.0,
            predation: PredationAffordanceConstants::default(),
            threat_response: ThreatResponseAffordanceConstants::default(),
            conflict_low: ConflictLowAffordanceConstants::default(),
            social: SocialAffordanceConstants::default(),
            prey_side: PreySideAffordanceConstants::default(),
        }
    }
}

// ---------- PlayCueEmissionConstants (ticket 279) ----------

/// Per-cue emission tunables for the play-engagement perception substrate
/// (`PlayBow`, `ReciprocalAdvance`, `SustainedCoPresence` `WitnessableEvent`
/// variants). Thresholds, ranges, chances, and cooldowns governing when
/// the emitters fire. The *belief lift* magnitude downstream of emission
/// lives on `BeliefsConstants` (per-axis `BeliefAxisTunables`).
///
/// Defaults are first-light placeholders; tuning belongs to balance work
/// once 280's matchmaker rebind reads the resulting facets.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlayCueEmissionConstants {
    /// `PlayBow` emit requires `Personality.playfulness >= this`. Default 0.4
    /// — moderate playfulness suffices; a strict cutoff would silence the
    /// substrate in colonies skewed toward serious personalities.
    pub playbow_min_playfulness: f32,
    /// `PlayBow` emit requires `Mood.valence >= this`. Default 0.0 —
    /// neutral-or-better mood. Negative-mood cats don't solicit play.
    pub playbow_min_mood_valence: f32,
    /// `PlayBow` candidate-range in tiles. A peer must be within this
    /// range (Manhattan) for the actor to consider soliciting. Default 5
    /// — close enough for the bow to be perceived.
    pub playbow_candidate_range_tiles: f32,
    /// Per-tick probability that an eligible cat emits a `PlayBow`.
    /// Default `0.002` — roughly one solicitation per 500 eligible ticks,
    /// modulated by cooldown.
    pub playbow_emit_chance_per_tick: f32,
    /// Minimum ticks between successive `PlayBow` emissions from the same
    /// actor. Default 200 — prevents serial-soliciter spam.
    pub playbow_cooldown_ticks: u64,
    /// Window after a `PlayBow` (or prior `ReciprocalAdvance`) during which
    /// a peer's movement-toward-actor counts as `ReciprocalAdvance`.
    /// Default 80 ticks — long enough for a movement to land, short
    /// enough that stale solicitations don't trigger false reciprocity.
    pub reciprocal_window_ticks: u64,
    /// Manhattan range within which an approaching peer counts as
    /// "advanced into engagement range". Default 3 tiles.
    pub reciprocal_engagement_range_tiles: f32,
    /// Tick count a pair must remain in `NearPairCache` together before
    /// the tracker emits `SustainedCoPresence`. Default 30 — short
    /// adjacencies don't count as engagement signal; ~30 ticks of
    /// continuous proximity does.
    pub sustained_copresence_threshold_ticks: u32,
    /// Per-pair cooldown between `SustainedCoPresence` emissions. Default
    /// 200 — keeps the belief-lift cadence within the same order of
    /// magnitude as other affiliative cues.
    pub sustained_copresence_emit_cooldown_ticks: u64,
}

impl Default for PlayCueEmissionConstants {
    fn default() -> Self {
        Self {
            playbow_min_playfulness: 0.4,
            playbow_min_mood_valence: 0.0,
            playbow_candidate_range_tiles: 5.0,
            playbow_emit_chance_per_tick: 0.002,
            playbow_cooldown_ticks: 200,
            reciprocal_window_ticks: 80,
            reciprocal_engagement_range_tiles: 3.0,
            sustained_copresence_threshold_ticks: 30,
            sustained_copresence_emit_cooldown_ticks: 200,
        }
    }
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_serde_json() {
        let original = SimConstants::default();
        let json = serde_json::to_string_pretty(&original).expect("serialize");
        let deserialized: SimConstants = serde_json::from_str(&json).expect("deserialize");

        // Spot-check a few values across different sub-structs
        assert_eq!(original.needs.hunger_decay, deserialized.needs.hunger_decay);
        assert_eq!(
            original.combat.flee_mood_duration,
            deserialized.combat.flee_mood_duration
        );
        assert_eq!(
            original.species.rabbit.catch_difficulty,
            deserialized.species.rabbit.catch_difficulty
        );
        assert_eq!(
            original.disposition.idle_fallback_duration,
            deserialized.disposition.idle_fallback_duration
        );
        assert_eq!(
            original.colony_score.kittens_weight,
            deserialized.colony_score.kittens_weight
        );
        assert_eq!(
            original.personality_friction.dual_ambition_decay,
            deserialized.personality_friction.dual_ambition_decay
        );
        assert_eq!(
            original.world_gen.ancient_ruin_count,
            deserialized.world_gen.ancient_ruin_count
        );

        // Re-serialize and compare strings to confirm full fidelity
        let json2 = serde_json::to_string_pretty(&deserialized).expect("re-serialize");
        assert_eq!(json, json2);
    }

    #[test]
    fn hawk_snake_ecology_defaults_match_ticket_025_table() {
        let d = SimConstants::default();
        // Hawk — ticket 025 §9 table.
        assert_eq!(d.hawk_ecology.flee_health_threshold, 0.4);
        assert_eq!(d.hawk_ecology.cat_avoidance_range, 4.0);
        assert_eq!(d.hawk_ecology.dive_range, 6.0);
        assert_eq!(d.hawk_ecology.outnumbered_flee_count, 2);
        assert_eq!(d.hawk_ecology.softmax_temperature, 0.15);
        // Snake — ticket 025 §9 table.
        assert_eq!(d.snake_ecology.strike_range, 1.0);
        assert_eq!(d.snake_ecology.flee_health_threshold, 0.5);
        assert_eq!(d.snake_ecology.cold_threshold, 0.3);
        assert_eq!(d.snake_ecology.bask_warmth_restore, 1.0);
        assert_eq!(d.snake_ecology.softmax_temperature, 0.15);
    }

    #[test]
    fn hawk_snake_ecology_serde_default_loads_legacy_header() {
        // Simulate a pre-Phase-2 events.jsonl header that omits the
        // new sections. `#[serde(default)]` on both fields means the
        // legacy JSON should still load; the missing sections populate
        // from `*EcologyConstants::default()`.
        let legacy_json = serde_json::json!({
            "needs": serde_json::to_value(NeedsConstants::default()).unwrap(),
            "buildings": serde_json::to_value(BuildingConstants::default()).unwrap(),
            "combat": serde_json::to_value(CombatConstants::default()).unwrap(),
            "magic": serde_json::to_value(MagicConstants::default()).unwrap(),
            "social": serde_json::to_value(SocialConstants::default()).unwrap(),
            "mood": serde_json::to_value(MoodConstants::default()).unwrap(),
            "death": serde_json::to_value(DeathConstants::default()).unwrap(),
            "prey": serde_json::to_value(PreyConstants::default()).unwrap(),
            "species": serde_json::to_value(SpeciesConstants::default()).unwrap(),
            "scoring": serde_json::to_value(ScoringConstants::default()).unwrap(),
            "disposition": serde_json::to_value(DispositionConstants::default()).unwrap(),
            "colony_score": serde_json::to_value(ColonyScoreConstants::default()).unwrap(),
            "wildlife": serde_json::to_value(WildlifeConstants::default()).unwrap(),
            "fate": serde_json::to_value(FateConstants::default()).unwrap(),
            "coordination": serde_json::to_value(CoordinationConstants::default()).unwrap(),
            "aspirations": serde_json::to_value(AspirationConstants::default()).unwrap(),
            "knowledge": serde_json::to_value(KnowledgeConstants::default()).unwrap(),
            "personality_friction": serde_json::to_value(PersonalityFrictionConstants::default()).unwrap(),
            // Deliberately omit hawk_ecology + snake_ecology to model a
            // pre-Phase-2 header.
        });
        let parsed: SimConstants =
            serde_json::from_value(legacy_json).expect("legacy header should still load");
        assert_eq!(parsed.hawk_ecology.dive_range, 6.0);
        assert_eq!(parsed.snake_ecology.strike_range, 1.0);
    }

    /// 284 (first-light) + 296 (curve-shape extraction): ward-placement
    /// scoring constants ship at their documented first-light / pre-296
    /// values so `compute_ward_placement` reproduces the post-284
    /// behavior. Flipping any of these is a balance-affecting change
    /// and must go through a dedicated tuning ticket. The 296 fields
    /// are a value-extraction refactor — their defaults must preserve
    /// the previously-hardcoded `(8.0, 0.5)` Logistic curve.
    #[test]
    fn ward_placement_scoring_constants_ship_at_documented_defaults() {
        let defaults = SimConstants::default();
        // 284 first-light values.
        assert_eq!(defaults.scoring.ward_ambush_anchor_weight, 0.5);
        assert_eq!(defaults.scoring.ward_recency_anchor_weight, 0.3);
        // 296 curve-extraction defaults preserve pre-296 hardcoded curve.
        assert_eq!(defaults.scoring.ward_placement_logistic_steepness, 8.0);
        assert_eq!(defaults.scoring.ward_placement_logistic_midpoint, 0.5);
        // 297 first-light activation: fox-intercept axis lifted to 0.5.
        // Mirrors the ambush anchor's first-light value from 284.
        assert_eq!(defaults.scoring.ward_fox_intercept_anchor_weight, 0.5);
        assert_eq!(defaults.scoring.fox_intercept_kernel_radius_tiles, 20);
        // 300 promotion: candidate-step default preserves the pre-300
        // hardcoded `5` value. Changing this default is a balance change
        // (modifies `compute_ward_placement`'s search grid resolution).
        assert_eq!(defaults.scoring.ward_placement_candidate_step, 5);
        // 298 promotion: cat_value tiebreak coefficient default preserves
        // the pre-298 hardcoded `0.3` value. First non-threat-axis lever
        // tested after 285/296/297 ruled out the threat-axis ones.
        assert_eq!(defaults.scoring.ward_placement_cat_value_weight, 0.3);
    }

    #[test]
    fn env_override_deep_merges_nested_field() {
        let patch: serde_json::Value =
            serde_json::from_str(r#"{"fulfillment":{"social_warmth_socialize_per_tick":0.0042}}"#)
                .expect("parse patch");
        let merged = SimConstants::default_with_overrides(&patch).expect("merge ok");
        // Patched field changed.
        assert_eq!(merged.fulfillment.social_warmth_socialize_per_tick, 0.0042);
        // Sibling field in same struct unchanged.
        assert_eq!(
            merged.fulfillment.social_warmth_groom_other_gain,
            SimConstants::default()
                .fulfillment
                .social_warmth_groom_other_gain,
        );
        // Unrelated sub-struct unchanged.
        assert_eq!(
            merged.needs.hunger_decay,
            SimConstants::default().needs.hunger_decay
        );
    }

    /// 375: the per-species byproduct table is producer-only (no L2
    /// consumer) and the canary only counts firings, not contents — a
    /// typo in `PreyByproductConstants::default()` would slip past the
    /// soak verdict. Pin each row here so the scenario / soak don't
    /// need to be the sole guard.
    #[test]
    fn prey_byproducts_table_default_matches_spec() {
        use crate::components::items::ItemKind;
        let table = SimConstants::default().prey_byproducts;
        // 368: Bristle appended to mammal byproduct lists (Grooming
        // Brush input). Fish + Bird unchanged.
        assert_eq!(
            table.for_kind(PreyKind::Mouse),
            &[ItemKind::Bone, ItemKind::Sinew, ItemKind::Bristle]
        );
        assert_eq!(
            table.for_kind(PreyKind::Rat),
            &[
                ItemKind::Bone,
                ItemKind::Sinew,
                ItemKind::Whisker,
                ItemKind::Bristle,
            ]
        );
        assert_eq!(
            table.for_kind(PreyKind::Rabbit),
            &[
                ItemKind::Hide,
                ItemKind::Bone,
                ItemKind::Sinew,
                ItemKind::Bristle,
            ]
        );
        assert_eq!(
            table.for_kind(PreyKind::Fish),
            &[ItemKind::FishScale, ItemKind::Tallow, ItemKind::RawOrgan]
        );
        assert_eq!(
            table.for_kind(PreyKind::Bird),
            &[ItemKind::Feather, ItemKind::Bone]
        );
    }

    #[test]
    fn env_override_silently_drops_unknown_fields() {
        // serde_json::from_value is lenient by default — unknown fields
        // are silently dropped rather than rejected. Documented caveat:
        // hypothesize.py validates field paths against the constants
        // dump in the events.jsonl header before treating an override
        // as applied. Don't tighten this with `deny_unknown_fields` —
        // several sub-structs use `#[serde(default)]` for forward-compat
        // and would break under stricter parsing.
        let patch: serde_json::Value =
            serde_json::from_str(r#"{"fulfillment":{"not_a_real_field":0.5}}"#).expect("parse");
        let merged = SimConstants::default_with_overrides(&patch).expect("merge ok");
        // Real field unchanged from default.
        assert_eq!(
            merged.fulfillment.social_warmth_socialize_per_tick,
            SimConstants::default()
                .fulfillment
                .social_warmth_socialize_per_tick,
        );
    }
}
