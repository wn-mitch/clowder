use bevy_ecs::prelude::*;

// ---------- SimConstants (top-level resource) ----------

#[derive(Resource, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SimConstants {
    pub needs: NeedsConstants,
    pub buildings: BuildingConstants,
    pub combat: CombatConstants,
    pub magic: MagicConstants,
    pub social: SocialConstants,
    pub mood: MoodConstants,
    pub death: DeathConstants,
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
}

impl Default for SimConstants {
    fn default() -> Self {
        Self {
            needs: NeedsConstants::default(),
            buildings: BuildingConstants::default(),
            combat: CombatConstants::default(),
            magic: MagicConstants::default(),
            social: SocialConstants::default(),
            mood: MoodConstants::default(),
            death: DeathConstants::default(),
            prey: PreyConstants::default(),
            species: SpeciesConstants::default(),
            scoring: ScoringConstants::default(),
            disposition: DispositionConstants::default(),
            colony_score: ColonyScoreConstants::default(),
            wildlife: WildlifeConstants::default(),
            fox_ecology: FoxEcologyConstants::default(),
            fate: FateConstants::default(),
            coordination: CoordinationConstants::default(),
            aspirations: AspirationConstants::default(),
            knowledge: KnowledgeConstants::default(),
            personality_friction: PersonalityFrictionConstants::default(),
            world_gen: WorldGenConstants::default(),
        }
    }
}

// ---------- NeedsConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NeedsConstants {
    pub hunger_decay: f32,
    pub energy_decay: f32,
    pub base_warmth_drain: f32,
    pub weather_warmth_snow: f32,
    pub weather_warmth_storm: f32,
    pub weather_warmth_wind: f32,
    pub weather_warmth_heavy_rain: f32,
    pub weather_warmth_light_rain: f32,
    pub season_warmth_winter: f32,
    pub season_warmth_autumn: f32,
    pub starvation_health_drain: f32,
    pub starvation_safety_drain: f32,
    pub starvation_mood_penalty: f32,
    pub starvation_mood_ticks: u64,
    pub starvation_social_multiplier: f32,
    pub safety_recovery_rate: f32,
    pub social_base_drain: f32,
    pub social_sociability_scale: f32,
    pub acceptance_base_drain: f32,
    pub acceptance_warmth_scale: f32,
    pub respect_base_drain: f32,
    pub respect_ambition_scale: f32,
    pub respect_low_threshold: f32,
    pub pride_amplifier_scale: f32,
    pub mastery_base_drain: f32,
    pub mastery_diligence_scale: f32,
    pub purpose_base_drain: f32,
    pub purpose_curiosity_scale: f32,
    pub purpose_patience_scale: f32,
    pub purpose_independence_scale: f32,
    pub tradition_familiar_distance: i32,
    pub tradition_safety_boost: f32,
    pub tradition_safety_drain: f32,
    pub eat_from_inventory_threshold: f32,
    /// Scales food_value reduction from tile corruption (e.g. 0.5 = half nourishment at full corruption).
    pub corruption_food_penalty: f32,
    // --- Grooming ---
    pub grooming_decay: f32,
    pub grooming_pride_penalty_scale: f32,
    // --- Mating ---
    pub mating_base_decay: f32,
    pub mating_warmth_scale: f32,
    // --- Bond proximity ---
    pub bond_proximity_social_rate: f32,
    pub bond_proximity_range: i32,
}

impl Default for NeedsConstants {
    fn default() -> Self {
        Self {
            hunger_decay: 0.0001,
            energy_decay: 0.0001,
            base_warmth_drain: 0.0001,
            weather_warmth_snow: 0.0004,
            weather_warmth_storm: 0.0003,
            weather_warmth_wind: 0.0002,
            weather_warmth_heavy_rain: 0.0002,
            weather_warmth_light_rain: 0.0001,
            season_warmth_winter: 0.0003,
            season_warmth_autumn: 0.0001,
            starvation_health_drain: 0.0005,
            starvation_safety_drain: 0.0005,
            starvation_mood_penalty: -0.3,
            starvation_mood_ticks: 5,
            starvation_social_multiplier: 2.0,
            safety_recovery_rate: 0.0002,
            social_base_drain: 0.0001,
            social_sociability_scale: 0.5,
            acceptance_base_drain: 0.00005,
            acceptance_warmth_scale: 0.5,
            respect_base_drain: 0.00003,
            respect_ambition_scale: 0.5,
            respect_low_threshold: 0.4,
            pride_amplifier_scale: 0.8,
            mastery_base_drain: 0.00002,
            mastery_diligence_scale: 0.5,
            purpose_base_drain: 0.00001,
            purpose_curiosity_scale: 0.5,
            purpose_patience_scale: 0.3,
            purpose_independence_scale: 0.4,
            tradition_familiar_distance: 5,
            tradition_safety_boost: 0.0002,
            tradition_safety_drain: 0.0001,
            eat_from_inventory_threshold: 0.05,
            corruption_food_penalty: 0.5,
            grooming_decay: 0.00003,
            grooming_pride_penalty_scale: 0.00005,
            mating_base_decay: 0.00008,
            mating_warmth_scale: 0.5,
            bond_proximity_social_rate: 0.0003,
            bond_proximity_range: 3,
        }
    }
}

// ---------- BuildingConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BuildingConstants {
    pub den_effect_radius: i32,
    pub den_warmth_bonus: f32,
    pub den_safety_bonus: f32,
    pub hearth_effect_radius: i32,
    pub hearth_social_bonus: f32,
    pub hearth_warmth_bonus_cold: f32,
    pub stores_spoilage_multiplier: f32,
    pub dirty_threshold: f32,
    pub dirty_discomfort_radius: i32,
    pub dirty_warmth_drain: f32,
    pub structural_decay_storm: f32,
    pub structural_decay_snow: f32,
    pub structural_decay_heavy_rain: f32,
    pub cleanliness_decay_storm: f32,
    pub cleanliness_decay_snow: f32,
    pub cleanliness_decay_fog: f32,
    pub cleanliness_decay_clear: f32,
    pub tidy_radius: i32,
    pub tidy_cleanliness_rate: f32,
    pub gate_tired_energy_threshold: f32,
    pub gate_tired_diligence_scale: f32,
    pub gate_close_diligence_threshold: f32,
}

impl Default for BuildingConstants {
    fn default() -> Self {
        Self {
            den_effect_radius: 4,
            den_warmth_bonus: 0.001,
            den_safety_bonus: 0.0005,
            hearth_effect_radius: 5,
            hearth_social_bonus: 0.001,
            hearth_warmth_bonus_cold: 0.001,
            stores_spoilage_multiplier: 0.5,
            dirty_threshold: 0.3,
            dirty_discomfort_radius: 3,
            dirty_warmth_drain: 0.0003,
            structural_decay_storm: 0.00003,
            structural_decay_snow: 0.00002,
            structural_decay_heavy_rain: 0.00001,
            cleanliness_decay_storm: 0.0002,
            cleanliness_decay_snow: 0.00015,
            cleanliness_decay_fog: 0.0001,
            cleanliness_decay_clear: 0.00008,
            tidy_radius: 3,
            tidy_cleanliness_rate: 0.0005,
            gate_tired_energy_threshold: 0.3,
            gate_tired_diligence_scale: 0.6,
            gate_close_diligence_threshold: 0.5,
        }
    }
}

// ---------- CombatConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CombatConstants {
    pub jitter_range: f32,
    pub combat_effective_hunting_weight: f32,
    pub ally_damage_bonus_per_ally: f32,
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
    pub flee_mood_ticks: u64,
    pub victory_respect_gain: f32,
    pub victory_safety_gain: f32,
    pub victory_mood_bonus: f32,
    pub victory_mood_ticks: u64,
    pub flee_action_ticks: u64,
    pub heal_duration_minor: u64,
    pub heal_duration_moderate: u64,
    pub heal_duration_severe: u64,
}

impl Default for CombatConstants {
    fn default() -> Self {
        Self {
            jitter_range: 0.02,
            combat_effective_hunting_weight: 0.3,
            ally_damage_bonus_per_ally: 0.2,
            temper_damage_bonus: 0.15,
            narrative_attack_chance: 0.15,
            wildlife_flee_health_threshold: 0.3,
            wildlife_flee_outnumbered_count: 3,
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
            flee_mood_ticks: 40,
            victory_respect_gain: 0.1,
            victory_safety_gain: 0.2,
            victory_mood_bonus: 0.3,
            victory_mood_ticks: 50,
            flee_action_ticks: 15,
            heal_duration_minor: 50,
            heal_duration_moderate: 200,
            heal_duration_severe: 500,
        }
    }
}

// ---------- MagicConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MagicConstants {
    pub corruption_spread_interval: u64,
    pub corruption_spread_threshold: f32,
    pub corruption_spread_rate: f32,
    pub corruption_new_tile_threshold: f32,
    pub ward_post_decay_multiplier: f32,
    pub healing_poultice_rate: f32,
    pub energy_tonic_rate: f32,
    pub mood_tonic_bonus: f32,
    pub mood_tonic_ticks: u64,
    pub personal_corruption_mood_threshold: f32,
    pub personal_corruption_mood_chance: f32,
    pub personal_corruption_mood_penalty: f32,
    pub personal_corruption_mood_ticks: u64,
    pub personal_corruption_erratic_threshold: f32,
    pub personal_corruption_erratic_chance: f32,
    pub corruption_tile_mood_threshold: f32,
    pub corruption_tile_mood_ticks: u64,
    pub corruption_twisted_herb_threshold: f32,
    pub shadow_fox_corruption_threshold: f32,
    pub shadow_fox_spawn_chance: f32,
    pub shadow_fox_population_cap: usize,
    pub shadow_fox_spawn_interval: u64,
    pub gather_herb_ticks: u64,
    pub herbcraft_gather_skill_growth: f32,
    pub prepare_remedy_ticks_workshop: u64,
    pub prepare_remedy_ticks_default: u64,
    pub herbcraft_prepare_skill_growth: f32,
    pub gratitude_fondness_gain: f32,
    pub herbcraft_apply_skill_growth: f32,
    pub set_ward_ticks: u64,
    pub herbcraft_ward_skill_growth: f32,
    pub magic_ward_skill_growth: f32,
    pub scry_ticks: u64,
    pub scry_memory_strength: f32,
    pub scry_magic_skill_growth: f32,
    pub cleanse_corruption_rate: f32,
    pub cleanse_personal_corruption_rate: f32,
    pub cleanse_magic_skill_growth: f32,
    pub cleanse_done_threshold: f32,
    pub cleanse_max_ticks: u64,
    pub spirit_communion_ticks: u64,
    pub spirit_communion_mood_bonus: f32,
    pub spirit_communion_mood_ticks: u64,
    pub spirit_communion_skill_growth: f32,
    pub misfire_skill_safe_ratio: f32,
    pub misfire_chance_scale: f32,
    pub misfire_fizzle_threshold: f32,
    pub misfire_corruption_backsplash_threshold: f32,
    pub misfire_inverted_ward_threshold: f32,
    pub misfire_wound_transfer_threshold: f32,
    pub misfire_fizzle_mood_penalty: f32,
    pub misfire_fizzle_mood_ticks: u64,
    pub misfire_corruption_backsplash_amount: f32,
    /// Multiplier on ward repel radius for shadow foxes (corrupted creatures).
    pub shadow_fox_ward_repel_multiplier: f32,
    /// Ticks between each growth stage advance for herbs and flavor plants.
    pub herb_growth_interval: u64,
    /// Ticks to harvest a carcass for shadow bone.
    pub harvest_carcass_ticks: u64,
    /// Personal corruption gained when harvesting a carcass.
    pub harvest_corruption_gain: f32,
    /// Corruption above this threshold suppresses herb harvestability.
    pub herb_suppression_threshold: f32,
    /// Health drain per tick on tiles with corruption > 0.8.
    pub corruption_health_drain: f32,
    /// Corruption threshold above which health drain applies.
    pub corruption_health_drain_threshold: f32,
    /// Rest quality multiplier on corrupted tiles (lower = worse rest).
    pub corruption_rest_penalty: f32,
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
            corruption_spread_interval: 10,
            corruption_spread_threshold: 0.3,
            corruption_spread_rate: 0.0001,
            corruption_new_tile_threshold: 0.05,
            ward_post_decay_multiplier: 0.3,
            healing_poultice_rate: 0.008,
            energy_tonic_rate: 0.003,
            mood_tonic_bonus: 0.2,
            mood_tonic_ticks: 500,
            personal_corruption_mood_threshold: 0.3,
            personal_corruption_mood_chance: 0.05,
            personal_corruption_mood_penalty: -0.15,
            personal_corruption_mood_ticks: 10,
            personal_corruption_erratic_threshold: 0.7,
            personal_corruption_erratic_chance: 0.02,
            corruption_tile_mood_threshold: 0.1,
            corruption_tile_mood_ticks: 5,
            corruption_twisted_herb_threshold: 0.3,
            shadow_fox_corruption_threshold: 0.7,
            shadow_fox_spawn_chance: 0.001,
            shadow_fox_population_cap: 2,
            shadow_fox_spawn_interval: 10,
            gather_herb_ticks: 5,
            herbcraft_gather_skill_growth: 0.01,
            prepare_remedy_ticks_workshop: 10,
            prepare_remedy_ticks_default: 15,
            herbcraft_prepare_skill_growth: 0.01,
            gratitude_fondness_gain: 0.1,
            herbcraft_apply_skill_growth: 0.005,
            set_ward_ticks: 8,
            herbcraft_ward_skill_growth: 0.01,
            magic_ward_skill_growth: 0.01,
            scry_ticks: 10,
            scry_memory_strength: 0.6,
            scry_magic_skill_growth: 0.01,
            cleanse_corruption_rate: 0.001,
            cleanse_personal_corruption_rate: 0.0005,
            cleanse_magic_skill_growth: 0.005,
            cleanse_done_threshold: 0.05,
            cleanse_max_ticks: 100,
            spirit_communion_ticks: 15,
            spirit_communion_mood_bonus: 0.3,
            spirit_communion_mood_ticks: 100,
            spirit_communion_skill_growth: 0.01,
            misfire_skill_safe_ratio: 0.8,
            misfire_chance_scale: 0.5,
            misfire_fizzle_threshold: 0.3,
            misfire_corruption_backsplash_threshold: 0.5,
            misfire_inverted_ward_threshold: 0.7,
            misfire_wound_transfer_threshold: 0.9,
            misfire_fizzle_mood_penalty: -0.1,
            misfire_fizzle_mood_ticks: 20,
            misfire_corruption_backsplash_amount: 0.1,
            shadow_fox_ward_repel_multiplier: 2.0,
            herb_growth_interval: 200,
            harvest_carcass_ticks: 15,
            harvest_corruption_gain: 0.05,
            herb_suppression_threshold: 0.5,
            corruption_health_drain: 0.0005,
            corruption_health_drain_threshold: 0.8,
            corruption_rest_penalty: 0.5,
            territory_corruption_inner_radius: 15,
            territory_corruption_outer_radius: 35,
        }
    }
}

// ---------- SocialConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SocialConstants {
    pub passive_familiarity_range: i32,
    pub passive_familiarity_rate: f32,
    pub bond_check_interval: u64,
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
}

impl Default for SocialConstants {
    fn default() -> Self {
        Self {
            passive_familiarity_range: 2,
            passive_familiarity_rate: 0.0003,
            bond_check_interval: 50,
            mates_romantic_threshold: 0.7,
            mates_fondness_threshold: 0.7,
            mates_familiarity_threshold: 0.6,
            partners_romantic_threshold: 0.5,
            partners_fondness_threshold: 0.6,
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
        }
    }
}

// ---------- MoodConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MoodConstants {
    pub baseline_optimism_weight: f32,
    pub baseline_offset: f32,
    pub anxiety_amplification: f32,
    pub temper_amplification_scale: f32,
    pub wounded_pride_respect_threshold: f32,
    pub wounded_pride_scale: f32,
    pub patience_extension_scale: f32,
    pub contagion_range: i32,
    pub contagion_base_influence: f32,
    pub contagion_stubbornness_resistance: f32,
    pub contagion_modifier_ticks: u64,
    pub contentment_phys_threshold: f32,
    pub contentment_mood_bonus: f32,
    pub contentment_mood_ticks: u64,
    pub bond_proximity_mood: f32,
    pub bond_proximity_mood_ticks: u64,
    pub bond_proximity_range: i32,
}

impl Default for MoodConstants {
    fn default() -> Self {
        Self {
            baseline_optimism_weight: 0.4,
            baseline_offset: -0.05,
            anxiety_amplification: 0.5,
            temper_amplification_scale: 0.3,
            wounded_pride_respect_threshold: 0.3,
            wounded_pride_scale: 0.15,
            patience_extension_scale: 0.3,
            contagion_range: 3,
            contagion_base_influence: 0.002,
            contagion_stubbornness_resistance: 0.2,
            contagion_modifier_ticks: 5,
            contentment_phys_threshold: 0.85,
            contentment_mood_bonus: 0.05,
            contentment_mood_ticks: 10,
            bond_proximity_mood: 0.03,
            bond_proximity_mood_ticks: 5,
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
            elder_entry_seasons: 48,
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
    pub grazing_max_ticks: u64,
    pub grazing_max_roam_normal: i32,
    pub grazing_max_roam_pressured: i32,
    pub grazing_pressure_roam_threshold: f32,
    pub flee_stop_distance: i32,
    pub hunger_base_rate: f32,
    pub overcrowding_threshold: f32,
    pub overcrowding_hunger_extra: f32,
    pub store_raid_chance: f32,
    pub store_raid_range: i32,
    pub store_raid_hunger_relief: f32,
    pub store_raid_cleanliness_drain: f32,
    pub store_raid_narrative_chance: f32,
    pub passive_hunger_relief: f32,
    pub starvation_health_drain: f32,
    pub starvation_threshold: f32,
    pub starvation_narrative_chance: f32,
    pub background_breed_rate_multiplier: f32,
    pub den_refill_base_chance: f32,
    pub den_fear_breeding_suppression: f32,
    pub den_predation_pressure_decay: f32,
    pub den_stress_high_threshold: f32,
    pub den_stress_low_threshold: f32,
    pub den_abandon_stress_ticks: u64,
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
            grazing_max_ticks: 200,
            grazing_max_roam_normal: 15,
            grazing_max_roam_pressured: 8,
            grazing_pressure_roam_threshold: 0.5,
            flee_stop_distance: 10,
            hunger_base_rate: 0.0002,
            overcrowding_threshold: 0.8,
            overcrowding_hunger_extra: 0.0001,
            store_raid_chance: 0.05,
            store_raid_range: 2,
            store_raid_hunger_relief: 0.015,
            store_raid_cleanliness_drain: 0.001,
            store_raid_narrative_chance: 0.02,
            passive_hunger_relief: 0.0003,
            starvation_health_drain: 0.001,
            starvation_threshold: 0.9,
            starvation_narrative_chance: 0.1,
            background_breed_rate_multiplier: 0.5,
            den_refill_base_chance: 0.005,
            den_fear_breeding_suppression: 0.8,
            den_predation_pressure_decay: 0.9995,
            den_stress_high_threshold: 0.7,
            den_stress_low_threshold: 0.5,
            den_abandon_stress_ticks: 3000,
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
    pub incapacitated_eat_urgency_scale: f32,
    pub incapacitated_eat_urgency_offset: f32,
    pub incapacitated_sleep_urgency_scale: f32,
    pub incapacitated_sleep_urgency_offset: f32,
    pub incapacitated_idle_score: f32,
    pub eat_urgency_scale: f32,
    pub sleep_urgency_scale: f32,
    /// Bonus to Sleep score when injured (scaled by injury severity).
    pub injury_rest_bonus: f32,
    pub hunt_food_scarcity_scale: f32,
    pub hunt_prey_bonus: f32,
    pub hunt_boldness_scale: f32,
    pub forage_food_scarcity_scale: f32,
    pub forage_diligence_scale: f32,
    pub socialize_sociability_scale: f32,
    pub socialize_temper_penalty_scale: f32,
    pub socialize_playfulness_bonus: f32,
    pub self_groom_warmth_scale: f32,
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
    pub build_diligence_scale: f32,
    pub build_site_bonus: f32,
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
    pub mentor_warmth_diligence_scale: f32,
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
    pub mate_warmth_scale: f32,
    pub caretake_compassion_scale: f32,
    pub caretake_parent_bonus: f32,
    // --- Corruption/carcass/siege scoring ---
    pub magic_harvest_carcass_scale: f32,
    pub magic_cleanse_colony_scale: f32,
    pub herbcraft_ward_siege_bonus: f32,
    /// Flat score bonus for ward-setting when territory corruption is detected
    /// and ward coverage is low. Makes wards competitive with Hunt/Eat.
    pub ward_corruption_emergency_bonus: f32,
    /// Flat score bonus for cleansing when territory corruption is detected.
    pub cleanse_corruption_emergency_bonus: f32,
    pub corruption_social_bonus: f32,
    pub corruption_suppression_threshold: f32,
    pub corruption_suppression_scale: f32,
    pub carcass_detection_range: i32,
}

impl Default for ScoringConstants {
    fn default() -> Self {
        Self {
            jitter_range: 0.05,
            incapacitated_eat_urgency_scale: 2.0,
            incapacitated_eat_urgency_offset: 0.3,
            incapacitated_sleep_urgency_scale: 2.0,
            incapacitated_sleep_urgency_offset: 0.3,
            incapacitated_idle_score: 0.2,
            eat_urgency_scale: 2.0,
            sleep_urgency_scale: 1.2,
            injury_rest_bonus: 0.4,
            hunt_food_scarcity_scale: 0.6,
            hunt_prey_bonus: 0.2,
            hunt_boldness_scale: 2.2,
            forage_food_scarcity_scale: 0.5,
            forage_diligence_scale: 2.0,
            socialize_sociability_scale: 2.0,
            socialize_temper_penalty_scale: 0.3,
            socialize_playfulness_bonus: 0.3,
            self_groom_warmth_scale: 0.8,
            groom_temper_penalty_scale: 0.3,
            explore_curiosity_scale: 0.7,
            fox_scent_suppression_threshold: 0.3,
            fox_scent_suppression_scale: 0.8,
            wander_curiosity_scale: 0.4,
            wander_base: 0.08,
            wander_playfulness_bonus: 0.2,
            flee_safety_threshold: 0.5,
            flee_safety_scale: 3.0,
            fight_min_allies: 1,
            fight_ally_bonus_per_cat: 0.15,
            fight_boldness_scale: 1.5,
            fight_health_suppression_threshold: 0.5,
            fight_safety_suppression_threshold: 0.3,
            patrol_safety_threshold: 0.8,
            patrol_boldness_scale: 1.5,
            build_diligence_scale: 0.8,
            build_site_bonus: 0.2,
            build_repair_bonus: 0.15,
            farm_diligence_scale: 0.6,
            herbcraft_gather_spirituality_scale: 0.5,
            herbcraft_gather_skill_offset: 0.1,
            herbcraft_prepare_skill_offset: 0.1,
            herbcraft_prepare_injury_scale: 0.3,
            herbcraft_prepare_injury_cap: 1.5,
            herbcraft_ward_skill_offset: 0.1,
            herbcraft_ward_scale: 0.6,
            magic_affinity_threshold: 0.3,
            magic_skill_threshold: 0.2,
            magic_durable_ward_skill_threshold: 0.5,
            magic_durable_ward_scale: 0.8,
            magic_cleanse_corruption_threshold: 0.1,
            magic_commune_scale: 0.7,
            coordinate_diligence_scale: 0.8,
            coordinate_directive_scale: 0.3,
            coordinate_ambition_bonus: 0.2,
            mentor_warmth_diligence_scale: 0.5,
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
            gate_timid_fight_threshold: 0.1,
            gate_shy_socialize_threshold: 0.15,
            gate_reckless_flee_threshold: 0.9,
            gate_compulsive_helper_threshold: 0.6,
            gate_compulsive_explorer_threshold: 0.9,
            gate_compulsive_explorer_chance: 0.20,
            gate_reckless_health_threshold: 0.5,
            mate_warmth_scale: 1.5,
            caretake_compassion_scale: 1.8,
            caretake_parent_bonus: 0.5,
            magic_harvest_carcass_scale: 0.6,
            magic_cleanse_colony_scale: 0.4,
            herbcraft_ward_siege_bonus: 0.4,
            ward_corruption_emergency_bonus: 2.0,
            cleanse_corruption_emergency_bonus: 0.8,
            corruption_social_bonus: 0.15,
            corruption_suppression_threshold: 0.3,
            corruption_suppression_scale: 0.6,
            carcass_detection_range: 15,
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
    pub resting_complete_warmth: f32,
    pub resting_max_replans: u32,
    pub sleep_duration_deficit_multiplier: f32,
    pub sleep_duration_base: u64,
    pub guard_threat_detection_range: i32,
    pub guard_patrol_radius: f32,
    pub social_chain_target_range: i32,
    pub mentor_warmth_threshold: f32,
    pub groom_warmth_threshold: f32,
    pub building_search_range: i32,
    pub crafting_herb_detection_range: i32,
    pub crafting_herbcraft_skill_threshold: f32,
    pub crafting_ward_placement_radius: f32,
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
    pub sleep_warmth_per_tick: f32,
    pub self_groom_duration: u64,
    pub self_groom_warmth_gain: f32,
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
    pub groom_other_warmth_gain: f32,
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
    pub survey_duration: u64,
    pub survey_purpose_gain: f32,
    pub survey_colony_discovery_scale: f32,
    pub survey_personal_discovery_scale: f32,
    pub exploration_decay_rate: f32,
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
}

fn default_true() -> bool {
    true
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

impl Default for DispositionConstants {
    fn default() -> Self {
        Self {
            starvation_interrupt_threshold: 0.15,
            exhaustion_interrupt_threshold: 0.10,
            critical_hunger_interrupt_threshold: 0.05,
            threat_awareness_range: 2,
            threat_urgency_divisor: 3.0,
            flee_threshold_base: 0.4,
            flee_threshold_boldness_scale: 0.4,
            critical_safety_threshold: 0.2,
            flee_distance: 8.0,
            flee_ticks: 5,
            damaged_building_threshold: 0.4,
            ward_strength_low_threshold: 0.3,
            hunt_terrain_search_radius: 15,
            forage_terrain_search_radius: 10,
            social_target_range: 10,
            wildlife_threat_range: 5,
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
            resting_complete_hunger: 0.3,
            resting_complete_energy: 0.3,
            resting_complete_warmth: 0.3,
            resting_max_replans: 12,
            sleep_duration_deficit_multiplier: 175.0,
            sleep_duration_base: 75,
            guard_threat_detection_range: 10,
            guard_patrol_radius: 10.0,
            social_chain_target_range: 15,
            mentor_warmth_threshold: 0.5,
            groom_warmth_threshold: 0.7,
            building_search_range: 30,
            crafting_herb_detection_range: 15,
            crafting_herbcraft_skill_threshold: 0.0,
            crafting_ward_placement_radius: 10.0,
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
            forage_yield_scale: 0.25,
            forage_skill_growth: 0.0008,
            forage_timeout_ticks: 40,
            deposit_quality_base: 0.3,
            deposit_quality_skill_scale: 0.4,
            eat_at_stores_duration: 50,
            corruption_food_penalty: 0.5,
            sleep_energy_per_tick: 0.002,
            sleep_warmth_per_tick: 0.001,
            self_groom_duration: 8,
            self_groom_warmth_gain: 0.15,
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
            groom_other_warmth_gain: 0.005,
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
            survey_duration: 50,
            survey_purpose_gain: 0.008,
            survey_colony_discovery_scale: 0.02,
            survey_personal_discovery_scale: 0.005,
            exploration_decay_rate: 0.00005,
            explore_den_discovery_chance: 0.08,
            deliver_directive_duration: 50,
            deliver_directive_respect_gain: 0.005,
            deliver_directive_social_gain: 0.005,
            idle_fallback_duration: 5,
            anti_stack_jitter: true,
            critical_health_threshold: 0.4,
            fight_bail_health_threshold: 0.35,
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
            ward_siege_chance: 0.3,
            ward_siege_decay_bonus: 0.003,
            ward_siege_corruption_rate: 0.005,
            ward_siege_corruption_radius: 3,
            ward_siege_max_ticks: 200,
            siege_break_range: 3,
            corruption_threat_multiplier: 0.5,
            ambush_cooldown_ticks: 100,
        }
    }
}

// ---------- FoxEcologyConstants ----------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FoxEcologyConstants {
    // --- Needs ---
    /// Per-tick hunger increase when not satiated (~1 season to starve from full).
    pub hunger_decay_per_tick: f32,
    /// Ticks of satiation after killing small prey (~3 days).
    pub satiation_after_prey_kill: u64,
    /// Ticks of satiation after raiding colony stores.
    pub satiation_after_store_raid: u64,
    /// Ticks of satiation after scavenging carrion/scraps.
    pub satiation_after_scavenge: u64,

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
    /// Maximum ticks a standoff lasts before auto-resolving.
    pub standoff_max_ticks: u64,
    /// Per-tick chance a standoff escalates to physical contact.
    pub standoff_escalation_chance: f32,
    /// Chance fox retreats when standoff ends without escalation.
    pub standoff_fox_retreat_chance: f32,
    /// Damage dealt to both parties when standoff escalates (minor scratch).
    pub standoff_damage_on_escalation: f32,
    /// Escalation chance for den defense confrontations (higher than normal).
    pub den_defense_escalation_chance: f32,

    // --- Lifecycle ---
    /// Ticks a fox stays in Cub stage (~1 season).
    pub cub_duration_ticks: u64,
    /// Ticks a fox stays in Juvenile stage (~2 seasons).
    pub juvenile_duration_ticks: u64,
    /// Maximum age in ticks before fox dies of old age (~4 years / 16 seasons).
    pub max_age_ticks: u64,
    /// Minimum litter size during breeding.
    pub litter_size_min: u32,
    /// Maximum litter size during breeding.
    pub litter_size_max: u32,
    /// Per-tick mortality chance for dispersing juveniles.
    pub juvenile_mortality_per_tick: f32,
    /// Per-tick mortality chance for elder foxes.
    pub elder_mortality_per_tick: f32,
    /// Ticks of sustained hunger=1.0 before starvation death.
    pub starvation_death_ticks: u64,

    // --- Territory ---
    /// Default territory radius from den in tiles.
    pub territory_radius: i32,
    /// Scent amount deposited per marking event.
    pub scent_deposit: f32,
    /// Per-tick global scent decay.
    pub scent_decay_per_tick: f32,
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
    /// Ticks of cooldown after any confrontation/raid/hunt action.
    pub post_action_cooldown: u64,

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
            // Needs — matched to cat hunger_decay (0.0001/tick)
            hunger_decay_per_tick: 0.0001,
            satiation_after_prey_kill: 1000,
            satiation_after_store_raid: 800,
            satiation_after_scavenge: 500,

            // Risk assessment
            cat_avoidance_range: 6,
            desperate_hunger_threshold: 0.9,
            den_defense_range: 5,
            flee_health_threshold: 0.4,
            outnumbered_flee_count: 2,

            // Confrontation
            standoff_max_ticks: 15,
            standoff_escalation_chance: 0.05,
            standoff_fox_retreat_chance: 0.7,
            standoff_damage_on_escalation: 0.05,
            den_defense_escalation_chance: 0.15,

            // Lifecycle
            cub_duration_ticks: 20_000,
            juvenile_duration_ticks: 40_000,
            max_age_ticks: 320_000,
            litter_size_min: 3,
            litter_size_max: 5,
            juvenile_mortality_per_tick: 0.000002,
            elder_mortality_per_tick: 0.000005,
            starvation_death_ticks: 2000,

            // Territory
            territory_radius: 18,
            scent_deposit: 0.1,
            scent_decay_per_tick: 0.0001,
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
            post_action_cooldown: 2000,

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
    pub assign_cooldown: u64,
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
            assign_cooldown: 50,
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
    pub evaluate_interval: u64,
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
}

impl Default for CoordinationConstants {
    fn default() -> Self {
        Self {
            social_weight_familiarity_scale: 0.5,
            social_weight_event_scale: 0.1,
            evaluate_interval: 100,
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
            wildlife_breach_range: 10,
            build_directive_priority_base: 0.5,
            build_directive_priority_building_scale: 0.2,
            forage_critical_multiplier: 0.8,
            build_repair_priority_base: 0.6,
            build_repair_priority_building_scale: 0.1,
            threat_proximity_range: 20,
            threat_patrol_targeted_priority: 0.6,
            colony_breach_range: 8,
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
    pub second_slot_check_interval: u64,
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
            second_slot_check_interval: 100,
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
            original.combat.flee_mood_ticks,
            deserialized.combat.flee_mood_ticks
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
}
