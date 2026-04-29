use bevy_ecs::prelude::*;

use crate::components::prey::PreyKind;
use crate::components::sensing::SensorySpecies;
use crate::components::wildlife::WildSpecies;
use crate::resources::time::Season;
use crate::resources::time_units::{DurationDays, DurationSeasons, IntervalPerDay, RatePerDay};
use crate::systems::sensing::{Channel, Falloff, SensoryProfile};

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
    pub species: SpeciesConstants,
    pub scoring: ScoringConstants,
    pub disposition: DispositionConstants,
    pub colony_score: ColonyScoreConstants,
    pub wildlife: WildlifeConstants,
    #[serde(default)]
    pub fox_ecology: FoxEcologyConstants,
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
    #[serde(default)]
    pub pairing: PairingConstants,
    #[serde(default)]
    pub planning_substrate: PlanningSubstrateConstants,
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
    /// and `starvation_mood_penalty` for the full cascade. See ticket
    /// 032 for the open work to soften this cliff into a graded
    /// body-condition axis.
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
    // --- Counts / distances ---
    pub starvation_mood_ticks: u64,
    pub tradition_familiar_distance: i32,
    pub bond_proximity_range: i32,
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
            // Counts / distances
            starvation_mood_ticks: 5,
            tradition_familiar_distance: 5,
            bond_proximity_range: 3,
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
    pub den_effect_radius: i32,
    pub hearth_effect_radius: i32,
    pub dirty_discomfort_radius: i32,
    pub tidy_radius: i32,
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
            den_effect_radius: 5,
            hearth_effect_radius: 6,
            dirty_discomfort_radius: 3,
            tidy_radius: 3,
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
    pub legend_witness_range: i32,
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
    pub banishment_pushback_radius: i32,
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
            legend_witness_range: 12,
            banishment_combat_skill_grow: 0.25,
            banishment_skill_gain_diminish_factor: 0.25,
            banishment_valor_mood: 0.35,
            banishment_witness_mood: 0.20,
            banishment_witness_safety_floor: 0.8,
            banishment_pushback_radius: 20,
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
    pub territory_corruption_inner_radius: i32,
    /// Outer radius (manhattan) of the territory corruption ring query.
    /// Tiles farther than this from colony center are ignored (too distant).
    pub territory_corruption_outer_radius: i32,
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
            territory_corruption_inner_radius: 15,
            territory_corruption_outer_radius: 35,
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
    pub passive_familiarity_range: i32,
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
            passive_familiarity_range: 2,
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

    // --- Counts / radii ---
    pub contagion_range: i32,
    pub bond_proximity_range: i32,
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

            // Counts / radii
            contagion_range: 3,
            bond_proximity_range: 3,
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
    pub grief_detection_range: i32,
    pub grief_memory_strength: f32,
    pub cleanup_grace_period: u64,
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
            grief_detection_range: 5,
            grief_memory_strength: 1.0,
            cleanup_grace_period: 500,
        }
    }
}

// ---------- FounderAgeConstants ----------

/// Distribution used when rolling ages for starting cats.
///
/// Paired invariant with `DeathConstants`: `elder_max_seasons` must stay
/// below `elder_entry_seasons + grace_seasons` so founders always have
/// runway before the old-age mortality ramp activates. The
/// `founder_ages_leave_elder_grace` test enforces this.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FounderAgeConstants {
    pub young_min_seasons: u64,
    pub young_max_seasons: u64,
    pub young_probability: f32,
    pub adult_min_seasons: u64,
    pub adult_max_seasons: u64,
    pub adult_probability: f32,
    pub elder_min_seasons: u64,
    pub elder_max_seasons: u64,
}

impl Default for FounderAgeConstants {
    fn default() -> Self {
        Self {
            young_min_seasons: 4,
            young_max_seasons: 11,
            young_probability: 0.60,
            adult_min_seasons: 12,
            adult_max_seasons: 30,
            adult_probability: 0.30,
            // Phase 4.3 retune: the `LifeStage::Elder` boundary moved
            // from season 48 to 60, so the founder Elder range moves
            // with it. Paired invariant still holds — the cap stays
            // below `DeathConstants::elder_entry_seasons +
            // grace_seasons = 67` so founders get runway before the
            // mortality ramp. Widening this past 67 reintroduces the
            // pre-Activation-1 baseline wipe regression (see
            // docs/balance/activation-1-status.md).
            elder_min_seasons: 60,
            elder_max_seasons: 62,
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
    pub bird_teleport_min_range: i32,
    pub bird_teleport_max_range: i32,
    pub grazing_wander_chance: f32,
    pub grazing_jitter_chance: f32,
    /// Maximum duration of a grazing bout before the prey returns to Idle.
    /// (ticket 033 Phase 4)
    #[serde(alias = "grazing_max_ticks")]
    pub grazing_max_duration: DurationDays,
    pub grazing_max_roam_normal: i32,
    pub grazing_max_roam_pressured: i32,
    pub grazing_pressure_roam_threshold: f32,
    pub flee_stop_distance: i32,
    pub hunger_base_rate: RatePerDay,
    pub overcrowding_threshold: f32,
    pub overcrowding_hunger_extra: f32,
    pub store_raid_chance: f32,
    pub store_raid_range: i32,
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
    pub den_kill_pressure_range: i32,
    pub den_raid_pressure_increment: f32,
    pub den_orphan_adopt_range: i32,
    pub den_orphan_adopt_capacity_threshold: f32,
    pub den_orphan_found_chance: f32,
    pub den_orphan_min_spacing: i32,
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
}

fn default_prey_scent_deposit_per_tick() -> f32 {
    0.1
}

fn default_prey_scent_decay_rate() -> RatePerDay {
    RatePerDay::new(1.0)
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
            bird_teleport_min_range: 5,
            bird_teleport_max_range: 8,
            grazing_wander_chance: 0.05,
            grazing_jitter_chance: 0.1,
            // 200 ticks ÷ 1000 ticks/day = 0.2 days (Phase 4).
            grazing_max_duration: DurationDays::new(0.2),
            grazing_max_roam_normal: 15,
            grazing_max_roam_pressured: 8,
            grazing_pressure_roam_threshold: 0.5,
            flee_stop_distance: 10,
            hunger_base_rate: RatePerDay::new(0.2),
            overcrowding_threshold: 0.8,
            overcrowding_hunger_extra: 0.0001,
            store_raid_chance: 0.05,
            store_raid_range: 2,
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
            den_kill_pressure_range: 15,
            den_raid_pressure_increment: 0.3,
            den_orphan_adopt_range: 15,
            den_orphan_adopt_capacity_threshold: 0.5,
            den_orphan_found_chance: 0.001,
            den_orphan_min_spacing: 25,
            prey_corruption_avoidance: 1.0,
            den_corruption_threshold: 0.4,
            initial_den_count_mouse: 4,
            initial_den_count_rat: 3,
            initial_den_count_rabbit: 3,
            initial_den_count_fish: 2,
            initial_den_count_bird: 2,
            scent_deposit_per_tick: default_prey_scent_deposit_per_tick(),
            scent_decay_rate: default_prey_scent_decay_rate(),
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
    pub alert_radius: i32,
    pub freeze_ticks: u64,
    pub catch_difficulty: f32,
    pub flee_duration: u64,
    pub den_capacity: u32,
    pub den_spawn_rate: f32,
    pub den_raid_drop: u32,
    pub den_spacing: i32,
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
                alert_radius: 3,
                freeze_ticks: 1,
                catch_difficulty: 0.9,
                flee_duration: 50,
                den_capacity: 80,
                den_spawn_rate: 0.01,
                den_raid_drop: 6,
                den_spacing: 10,
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
                alert_radius: 4,
                freeze_ticks: 2,
                catch_difficulty: 1.0,
                flee_duration: 75,
                den_capacity: 60,
                den_spawn_rate: 0.012,
                den_raid_drop: 5,
                den_spacing: 10,
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
                alert_radius: 6,
                freeze_ticks: 10,
                catch_difficulty: 0.85,
                flee_duration: 60,
                den_capacity: 60,
                den_spawn_rate: 0.01,
                den_raid_drop: 4,
                den_spacing: 20,
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
                alert_radius: 2,
                freeze_ticks: 0,
                catch_difficulty: 0.6,
                flee_duration: 0,
                den_capacity: 50,
                den_spawn_rate: 0.006,
                den_raid_drop: 3,
                den_spacing: 20,
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
                alert_radius: 8,
                freeze_ticks: 1,
                catch_difficulty: 0.5,
                flee_duration: 30,
                den_capacity: 40,
                den_spawn_rate: 0.004,
                den_raid_drop: 3,
                den_spacing: 15,
                den_density: 250,
            },
        }
    }
}

// ---------- ScoringConstants ----------

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
    /// Bonus to Sleep score when injured (scaled by injury severity).
    pub injury_rest_bonus: f32,
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
    pub self_groom_temperature_scale: f32,
    pub groom_temper_penalty_scale: f32,
    pub explore_curiosity_scale: f32,
    /// Fox scent threshold above which Hunt/Explore scores are suppressed.
    pub fox_scent_suppression_threshold: f32,
    /// Scale for how much fox scent suppresses risky action scores.
    pub fox_scent_suppression_scale: f32,
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
    pub magic_affinity_threshold: f32,
    pub magic_skill_threshold: f32,
    pub magic_durable_ward_skill_threshold: f32,
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
    pub survival_floor_phys_threshold: f32,
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
    /// Softmax temperature for §L2.10.6 softmax-over-Intentions selection.
    /// Used by `select_intention_softmax` (eval.rs) and the flat-action softmax
    /// path that replaces the legacy `aggregate_to_dispositions →
    /// select_disposition_softmax` pipeline in goap.rs / disposition.rs.
    /// Kept separate from `action_softmax_temperature` /
    /// `disposition_softmax_temperature` so the Intention-layer temperature
    /// can be tuned independently of the legacy per-Action / per-Disposition
    /// softmaxes retained for diagnostics.
    #[serde(default = "default_intention_softmax_temperature")]
    pub intention_softmax_temperature: f32,
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
    pub carcass_detection_range: i32,
    /// Tile radius within which a cat "smells" corruption on nearby tiles.
    /// Corruption beyond this range is out of sensing reach.
    pub corruption_smell_range: i32,
}

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
            injury_rest_bonus: 0.4,
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
            self_groom_temperature_scale: 0.6,
            groom_temper_penalty_scale: 0.3,
            explore_curiosity_scale: 0.7,
            fox_scent_suppression_threshold: 0.3,
            fox_scent_suppression_scale: 0.8,
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
            magic_affinity_threshold: 0.3,
            magic_skill_threshold: 0.2,
            magic_durable_ward_skill_threshold: 0.25,
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
            survival_floor_phys_threshold: 0.5,
            action_softmax_temperature: 0.15,
            disposition_softmax_temperature: 0.15,
            fox_softmax_temperature: default_fox_softmax_temperature(),
            intention_softmax_temperature: default_intention_softmax_temperature(),
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
            breeding_hunger_floor: 0.6,
            breeding_energy_floor: 0.5,
            breeding_mood_floor: 0.2,
            mating_interest_threshold: 0.6,
            mating_fertility_spring: default_mating_fertility_spring(),
            mating_fertility_summer: default_mating_fertility_summer(),
            mating_fertility_autumn: default_mating_fertility_autumn(),
            mating_fertility_winter: default_mating_fertility_winter(),
            magic_harvest_carcass_scale: 0.6,
            magic_cleanse_colony_scale: 0.4,
            herbcraft_ward_siege_bonus: 0.4,
            corruption_social_bonus: 0.15,
            corruption_suppression_threshold: 0.3,
            corruption_suppression_scale: 0.6,
            carcass_detection_range: 15,
            corruption_smell_range: 5,
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
    pub threat_awareness_range: i32,
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
    pub damaged_building_threshold: f32,
    pub ward_strength_low_threshold: f32,
    pub hunt_terrain_search_radius: i32,
    pub forage_terrain_search_radius: i32,
    pub social_target_range: i32,
    pub wildlife_threat_range: i32,
    /// Proximity radius for counting allies fighting the same threat.
    pub allies_fighting_range: i32,
    pub allies_fighting_cap: usize,
    /// Minimum HP ratio for a guarding cat to enter a FightThreat chain.
    pub guard_fight_health_min: f32,
    pub combat_effective_hunting_cross_train: f32,
    pub herb_detection_range: i32,
    pub prey_detection_range: i32,
    pub corrupted_tile_threshold: f32,
    pub mentor_skill_threshold_high: f32,
    pub mentor_skill_threshold_low: f32,
    pub mentoring_detection_range: i32,
    pub directive_bonus_base_weight: f32,
    pub directive_independence_penalty: f32,
    pub directive_stubbornness_penalty: f32,
    pub fondness_default: f32,
    pub fondness_social_weight: f32,
    pub novelty_social_weight: f32,
    pub disposition_independence_penalty: f32,
    pub fated_love_detection_range: i32,
    pub fated_rival_detection_range: i32,
    pub cascading_bonus_range: i32,
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
    pub guard_threat_detection_range: i32,
    pub guard_patrol_radius: f32,
    /// §L2.10.7 perimeter offset (tiles) from colony center used as the
    /// anchor for cat Patrol and HerbcraftWard spatial axes. Single
    /// representative point along the outer ring; the cat orbits
    /// toward it. 12 ≈ inner colony walk; further refinements could
    /// pick a per-cat angle for spread.
    pub patrol_perimeter_offset: i32,
    pub social_chain_target_range: i32,
    pub mentor_temperature_threshold: f32,
    pub groom_temperature_threshold: f32,
    pub building_search_range: i32,
    pub crafting_herb_detection_range: i32,
    pub crafting_herbcraft_skill_threshold: f32,
    pub crafting_magic_affinity_threshold: f32,
    pub crafting_magic_skill_threshold: f32,
    pub coordinating_target_range: i32,
    pub coordinating_distance_penalty: f32,
    pub explore_range: i32,
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
    pub scent_search_radius: i32,
    /// Phase 2B — minimum `PreyScentMap` value at the strongest
    /// nearby bucket for a cat to register "prey is scent-detectable
    /// here." Below this, the hunt-search step returns without
    /// committing to a prey target.
    #[serde(default = "default_scent_detect_threshold")]
    pub scent_detect_threshold: f32,
    pub den_discovery_range: i32,
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
    pub pounce_range_patient: i32,
    pub pounce_range_impatient: i32,
    pub pounce_range_default: i32,
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
    pub stalk_start_buffer: i32,
    pub stalk_start_minimum: i32,
    pub anxiety_spook_threshold: f32,
    pub anxiety_spook_chance: f32,
    pub chase_limit_bold: u64,
    pub chase_limit_default: u64,
    pub chase_stuck_ticks: u64,
    pub chase_speed: i32,
    pub approach_speed: i32,
    pub approach_give_up_distance: i32,
    pub search_belief_radius: i32,
    pub search_wind_direction_threshold: f32,
    pub search_jitter_chance: f32,
    pub search_speed: i32,
    pub search_visual_detection_range: i32,
    pub search_timeout_ticks: u64,
    pub travel_timeout_ticks: u64,
    pub travel_no_path_stuck_ticks: u64,
    pub global_step_timeout_ticks: u64,
    pub forage_jitter_chance: f32,
    pub forage_yield_scale: f32,
    pub forage_skill_growth: f32,
    pub forage_timeout_ticks: u64,
    pub deposit_quality_base: f32,
    pub deposit_quality_skill_scale: f32,
    pub eat_at_stores_duration: u64,
    /// Scales food_value reduction from tile corruption when eating at stores.
    pub corruption_food_penalty: f32,
    pub sleep_energy_per_tick: f32,
    pub sleep_temperature_per_tick: f32,
    pub self_groom_duration: u64,
    pub self_groom_temperature_gain: f32,
    pub socialize_social_per_tick: f32,
    pub socialize_fondness_per_tick: f32,
    pub socialize_familiarity_per_tick: f32,
    pub socialize_colony_absorb_rate: f32,
    pub socialize_personal_learn_rate: f32,
    pub socialize_duration: u64,
    pub groom_other_social_per_tick: f32,
    pub groom_other_fondness_per_tick: f32,
    pub groom_other_familiarity_per_tick: f32,
    pub groom_other_colony_absorb_rate: f32,
    pub groom_other_personal_learn_rate: f32,
    pub groom_other_duration: u64,
    pub groom_other_temperature_gain: f32,
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
    pub respect_witness_radius: i32,
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
    pub survey_explore_radius: i32,
    pub exploration_decay_rate: f32,
    /// Radius around each living cat that gets marked explored every tick,
    /// modelling passive awareness — cats notice their surroundings as they
    /// move through the world.  Smaller than `survey_explore_radius` (active
    /// perception).  Default 2 (5×5 = 25 tiles).
    #[serde(default = "default_passive_explore_radius")]
    pub passive_explore_radius: i32,
    /// Radius used by `unexplored_fraction_nearby` to determine how
    /// familiar a cat's local area feels.  Decoupled from `explore_range`
    /// (action distance) — a cat's sense of "I know this place" should
    /// cover a smaller area than "how far I could walk to explore."
    /// Default 10 (21×21 = 441 tiles).
    #[serde(default = "default_explore_perception_radius")]
    pub explore_perception_radius: i32,
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
    // --- Contextual threat evaluation (zoo vs bush) ---
    /// Threat intensity multiplier when the cat is inside a ward's repel radius.
    #[serde(default = "default_threat_ward_dampening")]
    pub threat_ward_dampening: f32,
    /// Threat intensity multiplier when the cat is near a colony building.
    #[serde(default = "default_threat_colony_building_dampening")]
    pub threat_colony_building_dampening: f32,
    /// Manhattan range within which a building counts as "nearby" for threat dampening.
    #[serde(default = "default_threat_building_safety_range")]
    pub threat_building_safety_range: i32,
    /// Radius from colony center used to normalize colony proximity factor.
    #[serde(default = "default_threat_colony_radius")]
    pub threat_colony_radius: f32,
    /// Minimum threat intensity multiplier when at colony center (lerps to 1.0 at radius edge).
    #[serde(default = "default_threat_colony_center_dampening")]
    pub threat_colony_center_dampening: f32,
    /// Range within which other cats count as allies for threat dampening.
    #[serde(default = "default_threat_ally_range")]
    pub threat_ally_range: i32,
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
    pub kitchen_cook_radius: i32,
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
fn default_threat_building_safety_range() -> i32 {
    5
}
fn default_threat_colony_radius() -> f32 {
    30.0
}
fn default_threat_colony_center_dampening() -> f32 {
    0.4
}
fn default_threat_ally_range() -> i32 {
    8
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

fn default_kitchen_cook_radius() -> i32 {
    1
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

fn default_cook_base_score() -> f32 {
    0.6
}

fn default_cook_diligence_scale() -> f32 {
    0.5
}

fn default_cook_hunger_gate() -> f32 {
    0.5
}

fn default_cook_food_scarcity_scale() -> f32 {
    0.6
}

fn default_build_pressure_cooking_min_raw_food() -> usize {
    3
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

fn default_gate_reckless_health_threshold() -> f32 {
    0.5
}

fn default_fox_softmax_temperature() -> f32 {
    0.15
}

fn default_intention_softmax_temperature() -> f32 {
    0.15
}

fn default_scent_search_radius() -> i32 {
    20
}

fn default_scent_detect_threshold() -> f32 {
    0.05
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
            threat_awareness_range: 10,
            threat_urgency_divisor: 10.0,
            flee_threshold_base: 0.15,
            flee_threshold_boldness_scale: 0.4,
            critical_safety_threshold: 0.2,
            guarding_exit_epsilon: default_guarding_exit_epsilon(),
            flee_distance: 8.0,
            flee_ticks: 5,
            damaged_building_threshold: 0.4,
            ward_strength_low_threshold: 0.3,
            hunt_terrain_search_radius: 15,
            forage_terrain_search_radius: 10,
            social_target_range: 10,
            wildlife_threat_range: 10,
            allies_fighting_range: 8,
            allies_fighting_cap: 5,
            guard_fight_health_min: 0.5,
            combat_effective_hunting_cross_train: 0.3,
            herb_detection_range: 15,
            prey_detection_range: 10,
            corrupted_tile_threshold: 0.1,
            mentor_skill_threshold_high: 0.6,
            mentor_skill_threshold_low: 0.3,
            mentoring_detection_range: 10,
            directive_bonus_base_weight: 0.5,
            directive_independence_penalty: 0.3,
            directive_stubbornness_penalty: 0.4,
            fondness_default: 0.5,
            fondness_social_weight: 0.6,
            novelty_social_weight: 0.4,
            disposition_independence_penalty: 0.2,
            fated_love_detection_range: 15,
            fated_rival_detection_range: 15,
            cascading_bonus_range: 5,
            resting_complete_hunger: 0.5,
            resting_complete_energy: 0.3,
            resting_complete_temperature: 0.3,
            planner_hunger_ok_threshold: 0.5,
            planner_energy_ok_threshold: 0.3,
            planner_temperature_ok_threshold: 0.3,
            resting_max_replans: 12,
            sleep_duration_deficit_multiplier: 175.0,
            sleep_duration_base: 75,
            guard_threat_detection_range: 10,
            guard_patrol_radius: 10.0,
            patrol_perimeter_offset: 12,
            social_chain_target_range: 15,
            mentor_temperature_threshold: 0.5,
            groom_temperature_threshold: 0.7,
            building_search_range: 30,
            crafting_herb_detection_range: 15,
            crafting_herbcraft_skill_threshold: 0.0,
            crafting_magic_affinity_threshold: 0.3,
            crafting_magic_skill_threshold: 0.2,
            coordinating_target_range: 30,
            coordinating_distance_penalty: 0.01,
            explore_range: 20,
            scent_downwind_dot_threshold: 0.0,
            scent_dense_forest_modifier: 0.5,
            scent_light_forest_modifier: 0.75,
            scent_base_range: 80.0,
            scent_min_range: 20.0,
            scent_search_radius: default_scent_search_radius(),
            scent_detect_threshold: default_scent_detect_threshold(),
            den_discovery_range: 3,
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
            pounce_range_patient: 2,
            pounce_range_impatient: 3,
            pounce_range_default: 2,
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
            stalk_start_buffer: 2,
            stalk_start_minimum: 5,
            anxiety_spook_threshold: 0.7,
            anxiety_spook_chance: 0.02,
            chase_limit_bold: 200,
            chase_limit_default: 120,
            chase_stuck_ticks: 10,
            chase_speed: 3,
            approach_speed: 3,
            approach_give_up_distance: 60,
            search_belief_radius: 25,
            search_wind_direction_threshold: 0.3,
            search_jitter_chance: 0.20,
            search_speed: 2,
            search_visual_detection_range: 15,
            search_timeout_ticks: 80,
            travel_timeout_ticks: 200,
            travel_no_path_stuck_ticks: 10,
            global_step_timeout_ticks: 500,
            forage_jitter_chance: 0.10,
            forage_yield_scale: 0.35,
            forage_skill_growth: 0.0008,
            forage_timeout_ticks: 40,
            deposit_quality_base: 0.3,
            deposit_quality_skill_scale: 0.4,
            eat_at_stores_duration: 50,
            corruption_food_penalty: 0.5,
            sleep_energy_per_tick: 0.0035,
            sleep_temperature_per_tick: 0.002,
            self_groom_duration: 8,
            self_groom_temperature_gain: 0.15,
            socialize_social_per_tick: 0.005,
            socialize_fondness_per_tick: 0.0005,
            socialize_familiarity_per_tick: 0.0008,
            socialize_colony_absorb_rate: 0.005,
            socialize_personal_learn_rate: 0.01,
            socialize_duration: 100,
            groom_other_social_per_tick: 0.002,
            groom_other_fondness_per_tick: 0.0008,
            groom_other_familiarity_per_tick: 0.0003,
            groom_other_colony_absorb_rate: 0.008,
            groom_other_personal_learn_rate: 0.012,
            groom_other_duration: 80,
            groom_other_temperature_gain: 0.005,
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
            threat_ward_dampening: 0.3,
            threat_colony_building_dampening: 0.5,
            threat_building_safety_range: 5,
            threat_colony_radius: 30.0,
            threat_colony_center_dampening: 0.4,
            threat_ally_range: 8,
            threat_ally_dampening_per_cat: 0.4,
            cooked_food_multiplier: default_cooked_food_multiplier(),
            cook_duration: default_cook_duration(),
            kitchen_cook_radius: default_kitchen_cook_radius(),
            min_disposition_tenure_ticks: default_min_disposition_tenure_ticks(),
            oscillation_score_lift: default_oscillation_score_lift(),
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
    pub den_shelter_radius: i32,
    pub activation_breadth_bonus: f64,
    pub activation_depth_bonus: f64,
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
            den_shelter_radius: 4,
            activation_breadth_bonus: 20.0,
            activation_depth_bonus: 5.0,
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
    pub base_detection_range: i32,
    pub forest_range_penalty: i32,
    pub threat_safety_drain: f32,
    pub threat_mood_penalty: f32,
    pub threat_mood_ticks: u64,
    pub predator_hunt_chance: f32,
    pub predator_hunt_range_fox: i32,
    pub predator_hunt_range_hawk: i32,
    pub predator_hunt_range_snake: i32,
    pub predator_hunt_range_shadow_fox: i32,
    pub predator_kill_chance: f32,
    pub predator_kill_narrative_chance: f32,
    pub initial_fox_count_min: u32,
    pub initial_fox_count_max: u32,
    pub initial_fox_min_distance: i32,
    pub initial_hawk_count_min: u32,
    pub initial_hawk_count_max: u32,
    pub initial_hawk_min_distance: i32,
    pub initial_snake_count_min: u32,
    pub initial_snake_count_max: u32,
    pub initial_snake_min_distance: i32,
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
    /// Probability a shadow fox encircles a ward instead of reversing.
    pub ward_siege_chance: f32,
    /// Extra decay per tick per encircling shadow fox.
    pub ward_siege_decay_bonus: f32,
    /// Corruption deposit rate per tick while encircling.
    pub ward_siege_corruption_rate: f32,
    /// Tile radius around ward affected by siege corruption.
    pub ward_siege_corruption_radius: i32,
    /// Max ticks a shadow fox will encircle before reverting to patrol.
    pub ward_siege_max_ticks: u64,
    /// If a cat comes within this range, encircling fox switches to stalking.
    pub siege_break_range: i32,
    /// Threat power multiplier from local tile corruption (additive, e.g. 0.5 = +50% at full corruption).
    pub corruption_threat_multiplier: f32,
    /// Ticks a shadow fox must wait after an ambush before it can stalk again.
    pub ambush_cooldown_ticks: u32,
    /// Range (manhattan) within which cats witness an ambush and have safety drained.
    pub ambush_witness_range: i32,
    /// Safety drain applied to cats who witness a nearby ambush.
    pub ambush_witness_safety_drain: f32,
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
            base_detection_range: 8,
            forest_range_penalty: 1,
            threat_safety_drain: 0.15,
            threat_mood_penalty: -0.2,
            threat_mood_ticks: 30,
            predator_hunt_chance: 0.1,
            predator_hunt_range_fox: 3,
            predator_hunt_range_hawk: 5,
            predator_hunt_range_snake: 1,
            predator_hunt_range_shadow_fox: 3,
            predator_kill_chance: 0.3,
            predator_kill_narrative_chance: 0.15,
            initial_fox_count_min: 2,
            initial_fox_count_max: 3,
            initial_fox_min_distance: 10,
            initial_hawk_count_min: 1,
            initial_hawk_count_max: 2,
            initial_hawk_min_distance: 10,
            initial_snake_count_min: 1,
            initial_snake_count_max: 2,
            initial_snake_min_distance: 7,
            carcass_corruption_rate: 0.002,
            carcass_drop_chance: 0.25,
            carcass_max_age: 500,
            carcass_scent_deposit_per_tick: default_carcass_scent_deposit_per_tick(),
            carcass_scent_decay_rate: default_carcass_scent_decay_rate(),
            ward_siege_chance: 0.3,
            ward_siege_decay_bonus: 0.0005,
            ward_siege_corruption_rate: 0.005,
            ward_siege_corruption_radius: 3,
            ward_siege_max_ticks: 200,
            siege_break_range: 3,
            corruption_threat_multiplier: 0.5,
            ambush_cooldown_ticks: 100,
            ambush_witness_range: 12,
            ambush_witness_safety_drain: 0.08,
        }
    }
}

fn default_carcass_scent_deposit_per_tick() -> f32 {
    0.1
}

fn default_carcass_scent_decay_rate() -> RatePerDay {
    RatePerDay::new(0.5)
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
    pub cat_avoidance_range: i32,
    /// Hunger level above which fox considers attacking risky targets.
    pub desperate_hunger_threshold: f32,
    /// Distance from den at which fox attacks ANY intruder (when cubs present).
    pub den_defense_range: i32,
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
    pub territory_radius: i32,
    /// Scent amount deposited per marking event.
    pub scent_deposit: f32,
    /// Global scent decay on `FoxScentMap` (and the symmetric
    /// `CatPresenceMap` at `disposition.rs:cat_presence_tick`),
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
    pub min_den_spacing: i32,

    // --- Store raiding ---
    /// Distance at which fox can detect colony food stores.
    pub raid_smell_range: i32,
    /// Food units stolen per successful raid.
    pub raid_food_stolen: f32,
    /// Cat proximity to stores that deters a raid.
    pub guard_deterrent_range: i32,

    // --- Ward / cat presence ---
    /// Hunger threshold above which a fox pushes through wards anyway.
    pub ward_hunger_override_threshold: f32,
    /// Cat-presence bucket value above which foxes avoid the area.
    pub cat_presence_avoidance_threshold: f32,

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
    pub initial_den_min_distance: i32,
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
            cat_avoidance_range: 6,
            desperate_hunger_threshold: 0.9,
            den_defense_range: 5,
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
            territory_radius: 18,
            scent_deposit: 0.1,
            scent_decay_rate: RatePerDay::new(0.1),
            max_dens: 3,
            min_den_spacing: 25,

            // Store raiding
            raid_smell_range: 12,
            raid_food_stolen: 2.0,
            guard_deterrent_range: 5,

            // Ward / cat presence
            ward_hunger_override_threshold: 0.7,
            cat_presence_avoidance_threshold: 0.3,

            // Cooldowns
            // Reduced from 2000 to 800 (~0.8 sim days) — 2000 was suppressing
            // most fox activity; foxes spent the bulk of each day frozen in
            // Resting. Shorter cooldown keeps downstream features (FoxStandoff,
            // FoxAvoidedCat, etc.) firing regularly.
            post_action_cooldown: DurationDays::new(0.8),

            // Initial spawn
            initial_den_count_min: 1,
            initial_den_count_max: 2,
            initial_den_min_distance: 15,
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
    pub love_awaken_distance: i32,
    pub rival_awaken_distance: i32,
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
            love_awaken_distance: 5,
            rival_awaken_distance: 10,
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
    pub wildlife_breach_range: i32,
    pub build_directive_priority_base: f32,
    pub build_directive_priority_building_scale: f32,
    pub forage_critical_multiplier: f32,
    pub build_repair_priority_base: f32,
    pub build_repair_priority_building_scale: f32,
    /// Range from colony buildings within which wildlife counts as a threat.
    pub threat_proximity_range: i32,
    /// Priority for targeted patrol toward an incursion point.
    pub threat_patrol_targeted_priority: f32,
    /// Range from a building at which wildlife triggers a Fight directive (breach).
    pub colony_breach_range: i32,
    /// Radius (manhattan) to check fox scent near colony center for preemptive patrol.
    pub preemptive_patrol_scent_radius: i32,
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
    pub corruption_search_radius: i32,
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
    pub urgent_dispatch_range: i32,
    /// Tiles around colony center within which a shadow-fox triggers posse
    /// assembly. Large enough to catch foxes before they ambush.
    #[serde(default = "default_posse_alarm_range")]
    pub posse_alarm_range: i32,
    /// How many cats the coordinator summons for a posse. 3-4 is the sweet
    /// spot: enough for ally damage bonuses, not so many the colony is
    /// disarmed defensively.
    #[serde(default = "default_posse_size")]
    pub posse_size: usize,
    /// Priority of posse Fight directives. Higher than ward-set so bold
    /// cats drop ward duty to engage the threat.
    #[serde(default = "default_posse_priority")]
    pub posse_priority: f32,
}

fn default_corruption_search_radius() -> i32 {
    20
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
fn default_urgent_dispatch_range() -> i32 {
    50
}

fn default_posse_alarm_range() -> i32 {
    20
}

fn default_posse_size() -> usize {
    3
}

fn default_posse_priority() -> f32 {
    0.9
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

fn default_survey_explore_radius() -> i32 {
    4
}

fn default_passive_explore_radius() -> i32 {
    2
}

fn default_explore_perception_radius() -> i32 {
    10
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

fn default_respect_witness_radius() -> i32 {
    // Manhattan tiles. Mirrors hearth_effect_radius scale.
    5
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
            cook_directive_priority: default_cook_directive_priority(),
            unmet_demand_amplifier: default_unmet_demand_amplifier(),
            wildlife_breach_range: 10,
            build_directive_priority_base: 0.5,
            build_directive_priority_building_scale: 0.2,
            forage_critical_multiplier: 0.8,
            build_repair_priority_base: 0.6,
            build_repair_priority_building_scale: 0.1,
            threat_proximity_range: 20,
            threat_patrol_targeted_priority: 0.6,
            colony_breach_range: 8,
            preemptive_patrol_scent_radius: 25,
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

// ---------- KnowledgeConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeConstants {
    pub decay_per_tick: f32,
    pub promotion_threshold: u32,
    pub scan_interval: u64,
    pub forgotten_cooldown: u64,
}

impl Default for KnowledgeConstants {
    fn default() -> Self {
        Self {
            decay_per_tick: 0.0001,
            promotion_threshold: 3,
            scan_interval: 500,
            forgotten_cooldown: 1000,
        }
    }
}

// ---------- PersonalityFrictionConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PersonalityFrictionConstants {
    pub friction_range: i32,
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
            friction_range: 3,
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
    pub special_min_spacing: i32,
    /// Minimum manhattan distance from AncientRuin to colony site.
    pub corruption_colony_min_distance: i32,
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
            special_min_spacing: 15,
            corruption_colony_min_distance: 30,
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
    pub social_warmth_isolation_range: i32,
    /// social_warmth gain per groom-other completion (both parties).
    pub social_warmth_groom_other_gain: f32,
    /// Passive per-tick social_warmth gain when a bonded companion is nearby.
    pub social_warmth_bond_proximity_rate: f32,
    /// Manhattan range for bond-proximity social_warmth restoration.
    pub social_warmth_bond_proximity_range: i32,
    /// Per-tick social_warmth gain while actively socializing with a target.
    pub social_warmth_socialize_per_tick: f32,
}

impl Default for FulfillmentConstants {
    fn default() -> Self {
        Self {
            social_warmth_base_decay: 0.00008,
            social_warmth_isolation_multiplier: 2.5,
            social_warmth_isolation_range: 3,
            social_warmth_groom_other_gain: 0.08,
            social_warmth_bond_proximity_rate: 0.0002,
            social_warmth_bond_proximity_range: 3,
            social_warmth_socialize_per_tick: 0.001,
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
    /// Falloff radius for `KittenUrgencyMap` — §5.6.3 row #13. Each
    /// kitten paints a disc weighted by hunger deficit (`1 - hunger`).
    pub kitten_urgency_sense_range: f32,
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
            kitten_urgency_sense_range: 12.0,
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

// ---------- PairingConstants (§7.M L2 PairingActivity) ----------

/// Knobs for the §7.M L2 PairingActivity Intention layer. Read by
/// `crate::ai::pairing::author_pairing_intentions` (Commit A) and by
/// the bias readers in `socialize_target` / `groom_other_target` /
/// `scoring` (Commit B). Calibrated against the seed-42 noise band
/// observed in `logs/baseline-2026-04-25/`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PairingConstants {
    /// Manhattan-distance candidate filter when scanning for an
    /// emission target. Wider than `MATE_TARGET_RANGE = 10` because
    /// L2 is multi-season — a cat may pursue a partner across the
    /// colony, not require adjacency every tick.
    pub candidate_range: i32,
    /// Minimum pairing-quality score for a Friends-bonded peer to
    /// trigger emission. Calibrated so a fresh-Friends peer at
    /// `fondness ≈ 0.5, romantic ≈ 0.0`, `bond_score = 0.5` clears
    /// (`0.40·0.5 + 0.40·0.0 + 0.20·0.5 = 0.30 ≥ 0.25`).
    pub emission_threshold: f32,
    /// Drop-gate `DesireDrift` floor on the romantic axis. Both this
    /// and `fondness_floor` must be breached simultaneously for the
    /// branch to fire — Mates-bonded post-conception cooldown should
    /// not drop on romantic alone.
    pub romantic_floor: f32,
    /// Drop-gate `DesireDrift` floor on the fondness axis (see
    /// `romantic_floor`).
    pub fondness_floor: f32,
    /// Per-axis weights of the pairing-quality scalar. Sum to 1.0.
    pub quality_fondness_weight: f32,
    pub quality_romantic_weight: f32,
    pub quality_bond_weight: f32,
}

impl Default for PairingConstants {
    fn default() -> Self {
        Self {
            candidate_range: 25,
            emission_threshold: 0.25,
            romantic_floor: 0.05,
            fondness_floor: 0.30,
            quality_fondness_weight: 0.40,
            quality_romantic_weight: 0.40,
            quality_bond_weight: 0.20,
        }
    }
}

// ---------- PlanningSubstrateConstants (ticket 071 sub-epic) ----------

/// Knobs for the planning-substrate hardening sub-epic (parent ticket
/// 071). Read by `crate::systems::plan_substrate` and the ECS systems
/// it composes.
///
/// Children that contribute knobs here:
/// - **080** — `reservation_ttl_ticks`: how long a `Reserved` marker
///   on a target lives before `expire_reservations` clears it.
/// - **073** *(parallel)* — `target_failure_cooldown_ticks` (when 073
///   lands; this struct is forward-compatible).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlanningSubstrateConstants {
    /// Ticket 080 — TTL for `Reserved` markers on contended resource
    /// targets (carcass, herb tile, prey, mate). Long enough for the
    /// owner cat to traverse + execute under normal sim cadence; short
    /// enough that abandoned reservations clear without manual release.
    /// Default 600 ticks ≈ 1 in-sim hour at the canonical 1000-ticks-
    /// per-day scale; tuneable post-soak per ticket Out-of-scope.
    pub reservation_ttl_ticks: u64,
}

impl Default for PlanningSubstrateConstants {
    fn default() -> Self {
        Self {
            reservation_ttl_ticks: 600,
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
