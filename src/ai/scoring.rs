use std::collections::HashMap;

use rand::Rng;

use crate::ai::Action;
use crate::components::disposition::CraftingHint;
use crate::components::mental::{Memory, MemoryType};
use crate::components::personality::Personality;
use crate::components::physical::{Needs, Position};
use crate::resources::sim_constants::ScoringConstants;
use crate::resources::time::DayPhase;

// ---------------------------------------------------------------------------
// Jitter
// ---------------------------------------------------------------------------

/// Small random noise added to every score to break ties and add variety.
fn jitter(rng: &mut impl Rng, range: f32) -> f32 {
    rng.random_range(-range..range)
}

// ---------------------------------------------------------------------------
// ScoringContext
// ---------------------------------------------------------------------------

/// Everything the scoring function needs to evaluate available actions.
pub struct ScoringContext<'a> {
    pub scoring: &'a ScoringConstants,
    pub needs: &'a Needs,
    pub personality: &'a Personality,
    pub food_available: bool,
    pub can_hunt: bool,
    pub can_forage: bool,
    /// Whether there is at least one visible cat to interact with.
    pub has_social_target: bool,
    /// Whether a wildlife threat is within detection range.
    pub has_threat_nearby: bool,
    /// Number of ally cats already fighting the same threat.
    pub allies_fighting_threat: usize,
    /// Combat skill + hunting cross-training.
    pub combat_effective: f32,
    /// Cat's current health (0.0–1.0).
    pub health: f32,
    /// Whether the cat is incapacitated by a severe injury.
    pub is_incapacitated: bool,
    /// Whether a construction site exists that needs work.
    pub has_construction_site: bool,
    /// Whether a building has structural condition < 0.4 (needs repair).
    pub has_damaged_building: bool,
    /// Whether a garden exists for farming.
    pub has_garden: bool,
    /// Fraction of food capacity filled (0.0–1.0).
    pub food_fraction: f32,
    // --- Magic/herbcraft context ---
    /// Cat's innate magical aptitude.
    pub magic_affinity: f32,
    /// Cat's trained magic skill level.
    pub magic_skill: f32,
    /// Cat's herbcraft skill level.
    pub herbcraft_skill: f32,
    /// Whether harvestable herbs are within gathering range.
    pub has_herbs_nearby: bool,
    /// Whether the cat has herbs in inventory.
    pub has_herbs_in_inventory: bool,
    /// Whether the cat has remedy herbs (HealingMoss/Moonpetal/Calmroot).
    pub has_remedy_herbs: bool,
    /// Whether the cat has Thornbriar for ward-setting.
    pub has_ward_herbs: bool,
    /// Whether harvestable Thornbriar exists anywhere in the world.
    pub thornbriar_available: bool,
    /// Number of injured cats in the colony.
    pub colony_injury_count: usize,
    /// Whether colony ward coverage is low (no wards or average strength < 0.3).
    pub ward_strength_low: bool,
    /// Whether the cat is standing on a corrupted tile.
    pub on_corrupted_tile: bool,
    /// Corruption level of the cat's current tile.
    pub tile_corruption: f32,
    /// Max corruption level on any tile within `corruption_smell_range` of the
    /// cat's current position. Represents the cat "smelling" rot nearby — drives
    /// proactive Cleanse/SetWard response even when not standing on corruption.
    pub nearby_corruption_level: f32,
    /// Whether the cat is on a fairy ring or standing stone.
    pub on_special_terrain: bool,
    // --- Coordination context ---
    /// Whether this cat is a coordinator with pending directives to deliver.
    pub is_coordinator_with_directives: bool,
    /// Number of pending directives (0 if not a coordinator).
    pub pending_directive_count: usize,
    // --- Mentoring context ---
    /// Whether a valid mentoring target exists (cat with skill < 0.3 where
    /// this cat has the same skill > 0.6).
    pub has_mentoring_target: bool,
    /// Whether at least one prey animal is within hunting range.
    pub prey_nearby: bool,
    // --- Personality integration fields ---
    /// Physiological satisfaction (0.0–1.0) for temper modifiers.
    pub phys_satisfaction: f32,
    /// Cat's current respect need level (0.0–1.0) for pride modifiers.
    pub respect: f32,
    /// Whether this cat currently has an active disposition.
    pub has_active_disposition: bool,
    /// The current disposition kind, if any (for patience commitment bonus).
    pub active_disposition: Option<crate::components::disposition::DispositionKind>,
    /// Pre-computed tradition location bonus for the cat's current tile.
    /// Set to `tradition * 0.1` by the caller if the cat's current action
    /// matches a previously successful action at this tile, else 0.0.
    pub tradition_location_bonus: f32,
    // --- Reproduction context ---
    /// Whether an orientation-compatible partner with Partners+ bond exists.
    pub has_eligible_mate: bool,
    /// Urgency of nearby hungry kittens (0.0 if none).
    pub hungry_kitten_urgency: f32,
    /// Whether this cat is a parent of the hungriest nearby kitten.
    pub is_parent_of_hungry_kitten: bool,
    // --- Exploration context ---
    /// Fraction of tiles within explore_range that are unexplored (0.0–1.0).
    /// Gates the explore action score: when nearby area is fully explored,
    /// explore becomes uninteresting.
    pub unexplored_nearby: f32,
    // --- Territorial context ---
    /// Fox scent intensity at the cat's current position (0.0–1.0).
    /// High values indicate deep fox territory.
    pub fox_scent_level: f32,
    // --- Corruption/carcass/siege context ---
    /// Whether uncleansed/unharvested carcasses are within detection range.
    pub carcass_nearby: bool,
    /// Count of nearby actionable carcasses (uncleansed or unharvested).
    pub nearby_carcass_count: usize,
    /// Max corruption of any tile in the territory ring around colony center (0.0–1.0).
    pub territory_max_corruption: f32,
    /// Whether any ward is currently being encircled by a shadow fox.
    pub wards_under_siege: bool,
    // --- Temporal context ---
    /// Current phase of the day. Drives species-specific activity bias —
    /// Sleep scoring alone in Phase 1, extends to hunting/foraging/denning as
    /// the initiative progresses. See `docs/systems/sleep-that-makes-sense.md`.
    pub day_phase: DayPhase,
    // --- Cooking context ---
    /// Whether at least one functional Kitchen exists in the colony.
    pub has_functional_kitchen: bool,
    /// Whether Stores currently holds at least one raw (uncooked) food item.
    pub has_raw_food_in_stores: bool,
}

// ---------------------------------------------------------------------------
// ScoringResult
// ---------------------------------------------------------------------------

/// Bundles action scores with metadata the chain builder needs.
/// `herbcraft_hint` carries which sub-mode won during herbcraft scoring,
/// so the chain builder doesn't re-derive it via its own priority cascade.
/// `magic_hint` is the equivalent for PracticeMagic — without it, the GOAP
/// planner falls back to A*'s cheapest action (Scry) and never picks
/// DurableWard even when its sub-score is highest.
pub struct ScoringResult {
    pub scores: Vec<(Action, f32)>,
    pub herbcraft_hint: Option<CraftingHint>,
    pub magic_hint: Option<CraftingHint>,
    /// True when the cat would have scored Cook competitively (had raw
    /// food, non-critical hunger, diligent personality) but no functional
    /// Kitchen exists. The caller turns this into a colony-wide
    /// `UnmetDemand::kitchen` tick so the coordinator's BuildPressure can
    /// respond to the latent desire.
    pub wants_cook_but_no_kitchen: bool,
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

/// Score all available actions for a cat given its current state.
///
/// Returns a [`ScoringResult`] containing `(Action, score)` pairs and an
/// optional herbcraft sub-mode hint. Higher score = more preferred.
/// The caller should pass the scores to [`select_best_action`].
pub fn score_actions(ctx: &ScoringContext, rng: &mut impl Rng) -> ScoringResult {
    let s = ctx.scoring;
    let mut scores = Vec::with_capacity(12);

    // Incapacitated cats can only Eat, Sleep, or Idle.
    if ctx.is_incapacitated {
        if ctx.food_available {
            let urgency = (1.0 - ctx.needs.hunger) * s.incapacitated_eat_urgency_scale
                + s.incapacitated_eat_urgency_offset;
            scores.push((Action::Eat, urgency + jitter(rng, s.jitter_range)));
        }
        let urgency = (1.0 - ctx.needs.energy) * s.incapacitated_sleep_urgency_scale
            + s.incapacitated_sleep_urgency_offset;
        scores.push((Action::Sleep, urgency + jitter(rng, s.jitter_range)));
        scores.push((
            Action::Idle,
            s.incapacitated_idle_score + jitter(rng, s.jitter_range),
        ));
        return ScoringResult {
            scores,
            herbcraft_hint: None,
            magic_hint: None,
            wants_cook_but_no_kitchen: false,
        };
    }

    // --- Eat (only when food stores are available) ---
    if ctx.food_available {
        let urgency =
            (1.0 - ctx.needs.hunger) * s.eat_urgency_scale * ctx.needs.level_suppression(1);
        scores.push((Action::Eat, urgency + jitter(rng, s.jitter_range)));
    }

    // --- Sleep ---
    {
        // Day-phase offset encodes the cat's Night-heavy, Dawn/Dusk-feeding
        // rhythm. Additive (not multiplicative) so Sleep remains available as
        // a pressure-release valve at low energy even during feeding peaks.
        let day_phase_offset = match ctx.day_phase {
            DayPhase::Dawn => s.sleep_dawn_bonus,
            DayPhase::Day => s.sleep_day_bonus,
            DayPhase::Dusk => s.sleep_dusk_bonus,
            DayPhase::Night => s.sleep_night_bonus,
        };
        let urgency = ((1.0 - ctx.needs.energy) * s.sleep_urgency_scale + day_phase_offset)
            * ctx.needs.level_suppression(1);
        // Injured cats are more inclined to rest — recovery requires downtime.
        let injury_bonus = if ctx.health < 1.0 {
            (1.0 - ctx.health) * s.injury_rest_bonus
        } else {
            0.0
        };
        scores.push((
            Action::Sleep,
            urgency + injury_bonus + jitter(rng, s.jitter_range),
        ));
    }

    // --- Hunt (boldness-driven; requires reachable forest/grass) ---
    if ctx.can_hunt {
        let food_scarcity = (1.0 - ctx.food_fraction) * s.hunt_food_scarcity_scale;
        let prey_bonus = if ctx.prey_nearby {
            s.hunt_prey_bonus
        } else {
            0.0
        };
        let urgency = ((1.0 - ctx.needs.hunger) + food_scarcity)
            * ctx.personality.boldness
            * s.hunt_boldness_scale
            * ctx.needs.level_suppression(1)
            + prey_bonus;
        scores.push((Action::Hunt, urgency + jitter(rng, s.jitter_range)));
    }

    // --- Forage (diligence-driven; requires terrain with yield) ---
    if ctx.can_forage {
        let food_scarcity = (1.0 - ctx.food_fraction) * s.forage_food_scarcity_scale;
        let urgency = ((1.0 - ctx.needs.hunger) + food_scarcity)
            * ctx.personality.diligence
            * s.forage_diligence_scale
            * ctx.needs.level_suppression(1);
        scores.push((Action::Forage, urgency + jitter(rng, s.jitter_range)));
    }

    // --- Socialize (sociability-driven; requires a visible cat) ---
    if ctx.has_social_target {
        let temper_penalty = ctx.personality.temper
            * s.socialize_temper_penalty_scale
            * (1.0 - ctx.phys_satisfaction);
        let mut score = (1.0 - ctx.needs.social)
            * ctx.personality.sociability
            * s.socialize_sociability_scale
            * ctx.needs.level_suppression(2)
            - temper_penalty
            + ctx.personality.playfulness * s.socialize_playfulness_bonus;
        // Socializing on corrupted ground pushes back corruption.
        if ctx.tile_corruption > 0.1 {
            score +=
                s.corruption_social_bonus * ctx.tile_corruption * ctx.needs.level_suppression(3);
        }
        scores.push((
            Action::Socialize,
            score.max(0.0) + jitter(rng, s.jitter_range),
        ));
    }

    // --- Groom (self or other; always available for self) ---
    {
        let self_groom =
            (1.0 - ctx.needs.warmth) * s.self_groom_warmth_scale * ctx.needs.level_suppression(1);
        let temper_penalty =
            ctx.personality.temper * s.groom_temper_penalty_scale * (1.0 - ctx.phys_satisfaction);
        let other_groom = if ctx.has_social_target {
            (ctx.personality.warmth * (1.0 - ctx.needs.social) * ctx.needs.level_suppression(2)
                - temper_penalty)
                .max(0.0)
        } else {
            0.0
        };
        scores.push((
            Action::Groom,
            self_groom.max(other_groom) + jitter(rng, s.jitter_range),
        ));
    }

    // --- Explore (curiosity-driven; gated by unexplored area and physiological needs) ---
    {
        let score = ctx.personality.curiosity
            * s.explore_curiosity_scale
            * ctx.needs.level_suppression(2)
            * ctx.unexplored_nearby;
        scores.push((Action::Explore, score + jitter(rng, s.jitter_range)));
    }

    // --- Wander (light movement; suppressed only by unmet physiological needs) ---
    {
        let score =
            ctx.personality.curiosity * s.wander_curiosity_scale * ctx.needs.level_suppression(2)
                + s.wander_base
                + ctx.personality.playfulness * s.wander_playfulness_bonus;
        scores.push((Action::Wander, score + jitter(rng, s.jitter_range)));
    }

    // --- Flee (fear-driven; scored when threat detected or safety is low) ---
    if ctx.has_threat_nearby || ctx.needs.safety < s.flee_safety_threshold {
        let score = (1.0 - ctx.needs.safety)
            * s.flee_safety_scale
            * (1.0 - ctx.personality.boldness)
            * ctx.needs.level_suppression(2);
        scores.push((Action::Flee, score + jitter(rng, s.jitter_range)));
    }

    // --- Fight (boldness + combat; only with allies engaging the same threat) ---
    // Suppressed when health is low — injured cats should avoid combat.
    // Also suppressed when safety is critically low — a cat already in danger
    // shouldn't double down by charging a threat.
    if ctx.has_threat_nearby && ctx.allies_fighting_threat >= s.fight_min_allies {
        let health_factor = if ctx.health < s.fight_health_suppression_threshold {
            ctx.health / s.fight_health_suppression_threshold
        } else {
            1.0
        };
        let safety_factor = if ctx.needs.safety < s.fight_safety_suppression_threshold {
            ctx.needs.safety / s.fight_safety_suppression_threshold
        } else {
            1.0
        };
        let group_bonus = ctx.allies_fighting_threat as f32 * s.fight_ally_bonus_per_cat;
        let score = ctx.personality.boldness
            * s.fight_boldness_scale
            * ctx.combat_effective
            * ctx.needs.level_suppression(2)
            * health_factor
            * safety_factor
            + group_bonus;
        scores.push((Action::Fight, score + jitter(rng, s.jitter_range)));
    }

    // --- Patrol (proactive safety-seeking; available when safety < threshold) ---
    if ctx.needs.safety < s.patrol_safety_threshold {
        let score = ctx.personality.boldness
            * s.patrol_boldness_scale
            * (1.0 - ctx.needs.safety)
            * ctx.needs.level_suppression(2);
        scores.push((Action::Patrol, score + jitter(rng, s.jitter_range)));
    }

    // --- Build (diligence-driven; scored when construction/repair is needed) ---
    // Level 2 suppression (phys only) — Build erects safety/defense
    // infrastructure (walls, wards, Hearth, Kitchen), so gating it on
    // pre-existing safety satisfaction creates a chicken-and-egg: the cats
    // who most need a wall never build one because they never feel safe.
    // A hungry/exhausted cat still shouldn't build (level 2), but a fed cat
    // under predator pressure should be able to.
    if ctx.has_construction_site || ctx.has_damaged_building {
        let base =
            ctx.personality.diligence * s.build_diligence_scale * ctx.needs.level_suppression(2);
        let site_bonus = if ctx.has_construction_site {
            s.build_site_bonus
        } else {
            0.0
        };
        let repair_bonus = if ctx.has_damaged_building {
            s.build_repair_bonus
        } else {
            0.0
        };
        scores.push((
            Action::Build,
            base + site_bonus + repair_bonus + jitter(rng, s.jitter_range),
        ));
    }

    // --- Farm (diligence-driven; scored when garden exists and food is low) ---
    // Level 2 suppression (phys only) — Farm is the colony's response to food
    // scarcity, same class as Hunt/Forage. Gating on safety would leave the
    // garden unattended when the colony most needs it (predator pressure +
    // empty stores is exactly when farming matters).
    if ctx.has_garden {
        let urgency = (1.0 - ctx.food_fraction)
            * ctx.personality.diligence
            * s.farm_diligence_scale
            * ctx.needs.level_suppression(2);
        scores.push((Action::Farm, urgency + jitter(rng, s.jitter_range)));
    }

    // --- Herbcraft (spirituality + herbcraft skill; three sub-modes) ---
    let herbcraft_hint;
    {
        // Gathering herbs for warding is the first step in the defense pipeline.
        // When corruption is detected and wards are low, the gather step also
        // gets the emergency bonus — otherwise cats never acquire the Thornbriar
        // they need to place wards (base gather score ~0.05 can't beat Hunt ~1.5).
        let gather_emergency = if ctx.has_herbs_nearby
            && ctx.ward_strength_low
            && !ctx.has_ward_herbs
            && ctx.thornbriar_available
            && ctx.territory_max_corruption > 0.0
        {
            s.ward_corruption_emergency_bonus * ctx.needs.level_suppression(2)
        } else {
            0.0
        };
        let gather = if ctx.has_herbs_nearby {
            ctx.personality.spirituality
                * s.herbcraft_gather_spirituality_scale
                * (s.herbcraft_gather_skill_offset + ctx.herbcraft_skill)
                * ctx.needs.level_suppression(2)
                + gather_emergency
        } else {
            0.0
        };
        let prepare = if ctx.has_remedy_herbs && ctx.colony_injury_count > 0 {
            ctx.personality.compassion
                * (s.herbcraft_prepare_skill_offset + ctx.herbcraft_skill)
                * (ctx.colony_injury_count as f32 * s.herbcraft_prepare_injury_scale)
                    .min(s.herbcraft_prepare_injury_cap)
                * ctx.needs.level_suppression(2)
        } else {
            0.0
        };
        // Ward-setting is a Safety (Level 2) action — it builds defenses against
        // shadow foxes. When corruption is detected in colony territory and ward
        // coverage is low, an emergency bonus makes warding competitive with
        // Hunt/Eat so cats actually place wards before the colony is overrun.
        let corruption_emergency = if ctx.ward_strength_low && ctx.territory_max_corruption > 0.0 {
            s.ward_corruption_emergency_bonus * ctx.needs.level_suppression(2)
        } else {
            0.0
        };
        // When corruption is present, allow ward to score even without ward herbs
        // in inventory. The GOAP plan for SetWard includes a GatherHerb step that
        // will acquire Thornbriar. This ensures CraftingHint::SetWard wins over
        // GatherHerbs, producing a plan that filters for the right herb type.
        let ward_eligible = ctx.ward_strength_low
            && (ctx.has_ward_herbs || (corruption_emergency > 0.0 && ctx.thornbriar_available));
        let mut ward = if ward_eligible {
            ctx.personality.spirituality
                * (s.herbcraft_ward_skill_offset + ctx.herbcraft_skill)
                * s.herbcraft_ward_scale
                * ctx.needs.level_suppression(2)
                + corruption_emergency
        } else {
            0.0
        };
        if ctx.wards_under_siege && ctx.has_ward_herbs {
            ward += s.herbcraft_ward_siege_bonus * ctx.needs.level_suppression(2);
        }
        let best = gather.max(prepare).max(ward);
        // Determine winning sub-mode deterministically (no jitter).
        herbcraft_hint = if best <= 0.0 {
            None
        } else if prepare >= gather && prepare >= ward {
            Some(CraftingHint::PrepareRemedy)
        } else if ward >= gather {
            Some(CraftingHint::SetWard)
        } else {
            Some(CraftingHint::GatherHerbs)
        };
        if best > 0.0 {
            scores.push((Action::Herbcraft, best + jitter(rng, s.jitter_range)));
        }
    }

    // --- PracticeMagic (requires affinity > threshold AND magic skill > threshold) ---
    let mut magic_hint: Option<CraftingHint> = None;
    if ctx.magic_affinity > s.magic_affinity_threshold && ctx.magic_skill > s.magic_skill_threshold
    {
        let scry = ctx.personality.curiosity
            * ctx.personality.spirituality
            * ctx.magic_skill
            * ctx.needs.level_suppression(5);
        // Durable wards and cleansing are Safety actions — they defend against
        // corruption, an existential threat. Same emergency bonus pattern as
        // herbcraft wards.
        let ward_emergency = if ctx.ward_strength_low && ctx.territory_max_corruption > 0.0 {
            s.ward_corruption_emergency_bonus * ctx.needs.level_suppression(2)
        } else {
            0.0
        };
        let cleanse_emergency = if ctx.territory_max_corruption > 0.0 {
            s.cleanse_corruption_emergency_bonus * ctx.needs.level_suppression(2)
        } else {
            0.0
        };
        // "Smells like rot" — proactive response when corruption is sensed nearby.
        // Boosts both Cleanse and SetWard scoring even if the cat isn't
        // personally standing on a corrupted tile.
        let sensed_rot_bonus = if ctx.nearby_corruption_level > 0.1 {
            s.corruption_sensed_response_bonus
                * ctx.nearby_corruption_level
                * ctx.needs.level_suppression(2)
        } else {
            0.0
        };
        let durable_ward =
            if ctx.ward_strength_low && ctx.magic_skill > s.magic_durable_ward_skill_threshold {
                ctx.personality.spirituality
                    * ctx.magic_skill
                    * s.magic_durable_ward_scale
                    * ctx.needs.level_suppression(2)
                    + ward_emergency
                    + sensed_rot_bonus
            } else {
                0.0
            };
        let cleanse = if ctx.on_corrupted_tile
            && ctx.tile_corruption > s.magic_cleanse_corruption_threshold
        {
            ctx.personality.spirituality
                * ctx.magic_skill
                * ctx.tile_corruption
                * ctx.needs.level_suppression(2)
                + cleanse_emergency
        } else {
            0.0
        };
        // Territory corruption motivates proactive cleansing even off corrupted tiles.
        let colony_cleanse = ctx.personality.spirituality
            * ctx.magic_skill
            * ctx.territory_max_corruption
            * s.magic_cleanse_colony_scale
            * ctx.needs.level_suppression(2)
            + cleanse_emergency
            + sensed_rot_bonus;
        // Carcass harvesting — curiosity-driven, risk/reward.
        let harvest = if ctx.carcass_nearby {
            ctx.personality.curiosity
                * (ctx.herbcraft_skill + 0.1)
                * (ctx.nearby_carcass_count.min(3) as f32)
                * s.magic_harvest_carcass_scale
                * ctx.needs.level_suppression(3)
        } else {
            0.0
        };
        let commune = if ctx.on_special_terrain {
            ctx.personality.spirituality
                * ctx.magic_skill
                * s.magic_commune_scale
                * ctx.needs.level_suppression(5)
        } else {
            0.0
        };
        let best = scry
            .max(durable_ward)
            .max(cleanse)
            .max(colony_cleanse)
            .max(harvest)
            .max(commune);
        // Pick the winning sub-action as a hint so the GOAP planner uses a
        // directed action list instead of cost-picking the cheapest option.
        magic_hint = if best <= 0.0 {
            None
        } else if durable_ward >= best {
            Some(CraftingHint::DurableWard)
        } else if colony_cleanse >= best || cleanse >= best {
            Some(CraftingHint::Cleanse)
        } else if harvest >= best {
            Some(CraftingHint::HarvestCarcass)
        } else {
            // Scry and Commune fall through to the generic Magic action list.
            Some(CraftingHint::Magic)
        };
        if best > 0.0 {
            scores.push((Action::PracticeMagic, best + jitter(rng, s.jitter_range)));
        }
    }

    // --- Coordinate (coordinator with pending directives only) ---
    if ctx.is_coordinator_with_directives {
        let score = ctx.personality.diligence
            * s.coordinate_diligence_scale
            * (ctx.pending_directive_count as f32 * s.coordinate_directive_scale)
            * ctx.needs.level_suppression(4)
            + ctx.personality.ambition
                * s.coordinate_ambition_bonus
                * ctx.needs.level_suppression(4);
        scores.push((Action::Coordinate, score + jitter(rng, s.jitter_range)));
    }

    // --- Mentor (warmth + diligence; requires valid mentoring target) ---
    if ctx.has_mentoring_target {
        let score = ctx.personality.warmth
            * ctx.personality.diligence
            * s.mentor_warmth_diligence_scale
            * ctx.needs.level_suppression(4)
            + ctx.personality.ambition * s.mentor_ambition_bonus;
        scores.push((Action::Mentor, score + jitter(rng, s.jitter_range)));
    }

    // --- Mate (warmth-driven; gated by mating need, partners, season) ---
    if ctx.has_eligible_mate {
        let urgency = (1.0 - ctx.needs.mating)
            * ctx.personality.warmth
            * s.mate_warmth_scale
            * ctx.needs.level_suppression(3);
        if urgency > 0.0 {
            scores.push((Action::Mate, urgency + jitter(rng, s.jitter_range)));
        }
    }

    // --- Cook (food-production tier; requires a Kitchen, raw food, and the
    //     cat not to be on the verge of starvation). Diligent cats cook more,
    //     and urgency scales with food scarcity — cooking is the colony's
    //     food-buffer multiplier, analogous to Farm. Level 2 suppression
    //     (phys only) matches Hunt/Forage: a fed cat will cook; an exhausted
    //     cat will still sleep first, but safety doesn't gate the action.
    //     Receives a directive bonus if a `DirectiveKind::Cook` is active.
    let cook_base_conditions =
        ctx.has_raw_food_in_stores && ctx.needs.hunger > s.cook_hunger_gate;
    let mut wants_cook_but_no_kitchen = false;
    if cook_base_conditions && ctx.has_functional_kitchen {
        let food_scarcity = (1.0 - ctx.food_fraction) * s.cook_food_scarcity_scale;
        let base = s.cook_base_score + ctx.personality.diligence * s.cook_diligence_scale;
        let score =
            (base + food_scarcity) * ctx.needs.level_suppression(2) + jitter(rng, s.jitter_range);
        scores.push((Action::Cook, score));
    } else if cook_base_conditions && !ctx.has_functional_kitchen {
        // Latent desire — the cat would cook if a Kitchen existed. Signal
        // this up to the caller so colony-wide BuildPressure on Kitchen
        // reflects the demand.
        wants_cook_but_no_kitchen = true;
    }

    // --- Caretake (compassion-driven; requires nearby hungry kitten) ---
    if ctx.hungry_kitten_urgency > 0.0 {
        let parent_bonus = if ctx.is_parent_of_hungry_kitten {
            s.caretake_parent_bonus
        } else {
            0.0
        };
        let score = ctx.hungry_kitten_urgency
            * ctx.personality.compassion
            * s.caretake_compassion_scale
            * ctx.needs.level_suppression(3)
            + parent_bonus;
        scores.push((Action::Caretake, score + jitter(rng, s.jitter_range)));
    }

    // --- Idle (always-available fallback; incurious cats idle more) ---
    let idle_score = s.idle_base + (1.0 - ctx.personality.curiosity) * s.idle_incuriosity_scale
        - ctx.personality.playfulness * s.idle_playfulness_penalty;
    scores.push((
        Action::Idle,
        idle_score.max(s.idle_minimum_floor) + jitter(rng, s.jitter_range),
    ));

    // --- Post-scoring personality modifiers ---

    // Pride: boost status-granting actions when respect is low.
    if ctx.respect < s.pride_respect_threshold {
        let pride_bonus = ctx.personality.pride * s.pride_bonus;
        for (action, score) in scores.iter_mut() {
            if matches!(
                action,
                Action::Hunt | Action::Fight | Action::Patrol | Action::Build | Action::Coordinate
            ) {
                *score += pride_bonus;
            }
        }
    }

    // Independence: boost solo actions, penalize group actions.
    {
        let ind = ctx.personality.independence;
        for (action, score) in scores.iter_mut() {
            match action {
                Action::Explore | Action::Wander | Action::Hunt => {
                    *score += ind * s.independence_solo_bonus
                }
                Action::Socialize | Action::Coordinate | Action::Mentor => {
                    *score = (*score - ind * s.independence_group_penalty).max(0.0);
                }
                _ => {}
            }
        }
    }

    // Patience: commitment bonus to actions within the active disposition.
    if let Some(active_disp) = ctx.active_disposition {
        let patience_bonus = ctx.personality.patience * s.patience_commitment_bonus;
        let constituent = active_disp.constituent_actions();
        for (action, score) in scores.iter_mut() {
            if constituent.contains(action) {
                *score += patience_bonus;
            }
        }
    }

    // Tradition: location preference bonus (pre-computed by caller).
    if ctx.tradition_location_bonus > 0.0 {
        // Apply to whichever action the caller pre-computed the bonus for.
        // The caller sets this based on the best-matching action at this tile.
        // For simplicity, boost all actions — the caller already scaled by tradition.
        for (_, score) in scores.iter_mut() {
            *score += ctx.tradition_location_bonus;
        }
    }

    // Fox territory suppression: cats in fox-scented areas are less inclined
    // to hunt, explore, forage, or patrol there. The suppression grows with
    // scent intensity above the threshold — deep fox territory is dangerous.
    if ctx.fox_scent_level > s.fox_scent_suppression_threshold {
        let suppression = (ctx.fox_scent_level - s.fox_scent_suppression_threshold)
            / (1.0 - s.fox_scent_suppression_threshold)
            * s.fox_scent_suppression_scale;
        for (action, score) in scores.iter_mut() {
            if matches!(
                action,
                Action::Hunt | Action::Explore | Action::Forage | Action::Patrol | Action::Wander
            ) {
                *score *= (1.0 - suppression).max(0.0);
            }
        }
        // Boost Flee score proportionally — cat wants to leave.
        for (action, score) in scores.iter_mut() {
            if matches!(action, Action::Flee) {
                *score += suppression * 0.5;
            }
        }
    }

    // Corruption territory suppression: corrupted ground discourages aimless
    // activity. Same pattern as fox scent — threshold, normalize, scale.
    if ctx.tile_corruption > s.corruption_suppression_threshold {
        let suppression = (ctx.tile_corruption - s.corruption_suppression_threshold)
            / (1.0 - s.corruption_suppression_threshold)
            * s.corruption_suppression_scale;
        for (action, score) in scores.iter_mut() {
            if matches!(action, Action::Explore | Action::Wander | Action::Idle) {
                *score *= (1.0 - suppression).max(0.0);
            }
        }
    }

    ScoringResult {
        scores,
        herbcraft_hint,
        magic_hint,
        wants_cook_but_no_kitchen,
    }
}

// ---------------------------------------------------------------------------
// Context bonuses (applied after base scoring)
// ---------------------------------------------------------------------------

/// Boost action scores based on remembered events near the cat's position.
///
/// - `ResourceFound` memories boost Hunt and Forage.
/// - `Death` memories suppress Wander and Idle (safety instinct).
///
/// Both scale with memory strength and proximity to the remembered location.
pub fn apply_memory_bonuses(
    scores: &mut [(Action, f32)],
    memory: &Memory,
    pos: &Position,
    sc: &ScoringConstants,
) {
    for entry in &memory.events {
        let Some(loc) = &entry.location else { continue };
        let dist = pos.distance_to(loc);
        if dist > sc.memory_nearby_radius {
            continue;
        }

        let proximity = 1.0 - (dist / sc.memory_nearby_radius);
        let bonus = proximity * entry.strength;

        match entry.event_type {
            MemoryType::ResourceFound => {
                for (action, score) in scores.iter_mut() {
                    if matches!(action, Action::Hunt | Action::Forage) {
                        *score += bonus * sc.memory_resource_bonus;
                    }
                }
            }
            MemoryType::Death => {
                for (action, score) in scores.iter_mut() {
                    if matches!(action, Action::Wander | Action::Idle) {
                        *score -= bonus * sc.memory_death_penalty;
                    }
                }
            }
            MemoryType::ThreatSeen => {
                // Suppress exploration and hunting near known threat locations.
                for (action, score) in scores.iter_mut() {
                    if matches!(action, Action::Wander | Action::Explore | Action::Hunt) {
                        *score -= bonus * sc.memory_threat_penalty;
                    }
                }
            }
            _ => {}
        }
    }
}

/// Boost action scores based on what nearby cats are doing.
///
/// For each action (except Fight, which has its own dedicated ally bonus),
/// adds `cascading_bonus_per_cat * count` where `count` is the number of cats
/// within range performing that action. Creates emergent group behaviors.
pub fn apply_cascading_bonuses(
    scores: &mut [(Action, f32)],
    nearby_actions: &HashMap<Action, usize>,
    sc: &ScoringConstants,
) {
    for (action, score) in scores.iter_mut() {
        // Fight already has fight_ally_bonus_per_cat in its base scoring;
        // applying the generic cascade on top creates a positive feedback
        // loop that snowballs into colony-wide fight charges.
        if *action == Action::Fight {
            continue;
        }
        if let Some(&count) = nearby_actions.get(action) {
            *score += count as f32 * sc.cascading_bonus_per_cat;
        }
    }
}

/// Apply a coordinator's directive bonus to the target action's score.
///
/// Called after base scoring and cascading bonuses. The bonus is pre-computed
/// from the directive priority, coordinator social weight, and the target cat's
/// personality (diligence, independence, stubbornness).
pub fn apply_directive_bonus(scores: &mut [(Action, f32)], target_action: Action, bonus: f32) {
    for (action, score) in scores.iter_mut() {
        if *action == target_action {
            *score += bonus;
        }
    }
}

/// Boost action scores based on colony-wide knowledge of the environment.
///
/// ThreatSeen/Death entries near the cat boost Patrol scores.
/// ResourceFound entries near the cat boost Hunt/Forage scores.
pub fn apply_colony_knowledge_bonuses(
    scores: &mut [(Action, f32)],
    knowledge: &crate::resources::colony_knowledge::ColonyKnowledge,
    pos: &Position,
    sc: &ScoringConstants,
) {
    for entry in &knowledge.entries {
        let Some(loc) = &entry.location else { continue };
        let dist = pos.distance_to(loc);
        if dist > sc.colony_knowledge_radius {
            continue;
        }

        let proximity = 1.0 - (dist / sc.colony_knowledge_radius);
        let bonus = proximity * entry.strength * sc.colony_knowledge_bonus_scale;

        match entry.event_type {
            MemoryType::ThreatSeen | MemoryType::Death => {
                for (action, score) in scores.iter_mut() {
                    if matches!(action, Action::Patrol) {
                        *score += bonus;
                    }
                }
            }
            MemoryType::ResourceFound => {
                for (action, score) in scores.iter_mut() {
                    if matches!(action, Action::Forage | Action::Hunt) {
                        *score += bonus;
                    }
                }
            }
            _ => {}
        }
    }
}

/// Boost action scores based on an active player-set colony priority.
pub fn apply_priority_bonus(
    scores: &mut [(Action, f32)],
    priority: Option<crate::resources::colony_priority::PriorityKind>,
    sc: &ScoringConstants,
) {
    let Some(kind) = priority else { return };
    use crate::resources::colony_priority::PriorityKind;
    let bonus = sc.priority_bonus;
    let matching: &[Action] = match kind {
        PriorityKind::Food => &[Action::Hunt, Action::Forage, Action::Farm],
        PriorityKind::Defense => &[Action::Patrol, Action::Fight],
        PriorityKind::Building => &[Action::Build],
        PriorityKind::Exploration => &[Action::Explore],
    };
    for (action, score) in scores.iter_mut() {
        if matching.contains(action) {
            *score += bonus;
        }
    }
}

// ---------------------------------------------------------------------------
// Aspiration, preference, and fate bonuses
// ---------------------------------------------------------------------------

/// Boost action scores based on active aspirations.
///
/// For each active aspiration, adds a flat desire bonus to actions in the
/// aspiration's domain. This makes cats *want* to do things related to
/// their goals, without changing their skill at doing them.
pub fn apply_aspiration_bonuses(
    scores: &mut [(Action, f32)],
    aspirations: &crate::components::aspirations::Aspirations,
    sc: &ScoringConstants,
) {
    for asp in &aspirations.active {
        let matching = asp.domain.matching_actions();
        for (action, score) in scores.iter_mut() {
            if matching.contains(action) {
                *score += sc.aspiration_bonus;
            }
        }
    }
}

/// Adjust action scores based on personal likes and dislikes.
///
/// Like: +0.08 desire bonus. Dislike: -0.08 desire penalty.
/// Smaller than aspiration bonuses — preferences are background flavor.
pub fn apply_preference_bonuses(
    scores: &mut [(Action, f32)],
    preferences: &crate::components::aspirations::Preferences,
    sc: &ScoringConstants,
) {
    for (action, score) in scores.iter_mut() {
        match preferences.get(*action) {
            Some(crate::components::aspirations::Preference::Like) => {
                *score += sc.preference_like_bonus
            }
            Some(crate::components::aspirations::Preference::Dislike) => {
                *score -= sc.preference_dislike_penalty
            }
            None => {}
        }
    }
}

/// Boost action scores based on awakened fated connections.
///
/// - Fated love (awakened, partner visible): +0.15 to Socialize/Groom.
/// - Fated rival (awakened, rival nearby): +0.1 to Hunt/Patrol/Fight/Explore.
pub fn apply_fated_bonuses(
    scores: &mut [(Action, f32)],
    fated_love_visible: bool,
    fated_rival_nearby: bool,
    sc: &ScoringConstants,
) {
    if fated_love_visible {
        for (action, score) in scores.iter_mut() {
            if matches!(action, Action::Socialize | Action::Groom | Action::Mate) {
                *score += sc.fated_love_social_bonus;
            }
        }
    }
    if fated_rival_nearby {
        for (action, score) in scores.iter_mut() {
            if matches!(
                action,
                Action::Hunt | Action::Patrol | Action::Fight | Action::Explore
            ) {
                *score += sc.fated_rival_competition_bonus;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Survival floor
// ---------------------------------------------------------------------------

/// Ensure survival actions (Eat, Sleep) aren't outcompeted by bonus-inflated
/// higher-level actions when basic needs are critical.
///
/// When physiological satisfaction drops below 0.5, non-survival action scores
/// are compressed toward the highest survival score. At full starvation (phys=0),
/// no non-survival action can outscore Eat/Sleep. Flee is exempt — running from
/// a predator while starving is rational.
pub fn enforce_survival_floor(scores: &mut [(Action, f32)], needs: &Needs, sc: &ScoringConstants) {
    let phys = needs.physiological_satisfaction();
    if phys >= sc.survival_floor_phys_threshold {
        return;
    }

    let survival_ceiling = scores
        .iter()
        .filter(|(a, _)| matches!(a, Action::Eat | Action::Sleep))
        .map(|(_, s)| *s)
        .fold(0.0f32, f32::max);

    if survival_ceiling <= 0.0 {
        return;
    }

    let factor = phys / sc.survival_floor_phys_threshold;
    for (action, score) in scores.iter_mut() {
        if matches!(action, Action::Eat | Action::Sleep | Action::Flee) {
            continue;
        }
        if *score > survival_ceiling {
            *score = survival_ceiling + (*score - survival_ceiling) * factor;
        }
    }
}

// ---------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------

/// Pick the action with the highest score. Falls back to [`Action::Idle`] if
/// the slice is empty or all scores are non-finite.
pub fn select_best_action(scores: &[(Action, f32)]) -> Action {
    scores
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(action, _)| *action)
        .unwrap_or(Action::Idle)
}

/// Select an action using softmax (Boltzmann) sampling.
///
/// Instead of always picking the highest score, treats scores as weights
/// and samples probabilistically. Temperature controls variety:
/// - T → 0: converges to argmax (deterministic)
/// - T = 0.10: personality-primary ~45-60%, realistic secondary behaviors
/// - T → ∞: uniform random (personality irrelevant)
pub fn select_action_softmax(
    scores: &[(Action, f32)],
    rng: &mut impl Rng,
    sc: &ScoringConstants,
) -> Action {
    let temperature = sc.action_softmax_temperature;

    if scores.is_empty() {
        return Action::Idle;
    }

    let max_score = scores
        .iter()
        .map(|(_, s)| *s)
        .fold(f32::NEG_INFINITY, f32::max);
    let weights: Vec<f32> = scores
        .iter()
        .map(|(_, s)| ((s - max_score) / temperature).exp())
        .collect();
    let total: f32 = weights.iter().sum();

    let mut roll: f32 = rng.random::<f32>() * total;
    for (i, w) in weights.iter().enumerate() {
        roll -= w;
        if roll <= 0.0 {
            return scores[i].0;
        }
    }

    scores.last().map(|(a, _)| *a).unwrap_or(Action::Idle)
}

// ---------------------------------------------------------------------------
// Behavior gates
// ---------------------------------------------------------------------------

/// Apply behavior gate overrides for extreme personality values.
///
/// Checked after scoring, before action execution. Returns `Some(overridden)`
/// if a gate fires, `None` if the chosen action stands.
///
/// Gates (from personality.md):
/// - Boldness < 0.1: Fight → Flee (too timid)
/// - Sociability < 0.15: Socialize → Idle (too shy)
/// - Curiosity > 0.9: 20% chance override → Explore (compulsive explorer)
/// - Boldness > 0.9: Flee → Fight (reckless bravery)
/// - Compassion > 0.9 + injured nearby: override → Herbcraft (compulsive helper)
///
/// Stubbornness > 0.85 directive rejection is handled separately via the
/// `DirectiveRefused` event in the personality events system.
pub fn behavior_gate_check(
    chosen: Action,
    personality: &Personality,
    has_injured_nearby: bool,
    health_ratio: f32,
    rng: &mut impl Rng,
    sc: &ScoringConstants,
) -> Option<Action> {
    // Too timid to fight — always flee instead.
    if chosen == Action::Fight && personality.boldness < sc.gate_timid_fight_threshold {
        return Some(Action::Flee);
    }
    // Too shy to socialize — skip to idle.
    if chosen == Action::Socialize && personality.sociability < sc.gate_shy_socialize_threshold {
        return Some(Action::Idle);
    }
    // Reckless bravery — cannot flee, must fight. But only when healthy enough.
    if chosen == Action::Flee
        && personality.boldness > sc.gate_reckless_flee_threshold
        && health_ratio > sc.gate_reckless_health_threshold
    {
        return Some(Action::Fight);
    }
    // Compulsive helper — drop everything to aid an injured cat.
    if personality.compassion > sc.gate_compulsive_helper_threshold && has_injured_nearby {
        return Some(Action::Herbcraft);
    }
    // Compulsive explorer — chance per tick to ignore current action.
    if personality.curiosity > sc.gate_compulsive_explorer_threshold
        && !matches!(
            chosen,
            Action::Eat | Action::Sleep | Action::Flee | Action::Explore
        )
        && rng.random::<f32>() < sc.gate_compulsive_explorer_chance
    {
        return Some(Action::Explore);
    }
    None
}

// ---------------------------------------------------------------------------
// Disposition scoring (aggregate from action scores)
// ---------------------------------------------------------------------------

use crate::components::disposition::DispositionKind;

/// Aggregate per-action scores into per-disposition scores.
///
/// For each disposition, takes the MAX score among its constituent actions.
/// Actions not present in the score list contribute nothing (the disposition
/// may still appear with score 0.0 if no constituent scored).
///
/// Groom is special: it appears in both Resting (self-groom) and Socializing
/// (groom-other). The caller should set `self_groom_won` based on whether the
/// self-groom sub-score beat the other-groom sub-score during action scoring.
pub fn aggregate_to_dispositions(
    action_scores: &[(Action, f32)],
    self_groom_won: bool,
) -> Vec<(DispositionKind, f32)> {
    let mut disposition_scores: Vec<(DispositionKind, f32)> = DispositionKind::ALL
        .iter()
        .map(|kind| (*kind, 0.0f32))
        .collect();

    for &(action, score) in action_scores {
        // Skip Flee and Idle — not dispositions.
        if matches!(action, Action::Flee | Action::Idle) {
            continue;
        }

        // Groom routes to Resting or Socializing depending on which sub-score won.
        if action == Action::Groom {
            let target_kind = if self_groom_won {
                DispositionKind::Resting
            } else {
                DispositionKind::Socializing
            };
            if let Some((_, existing)) = disposition_scores
                .iter_mut()
                .find(|(k, _)| *k == target_kind)
            {
                *existing = existing.max(score);
            }
            continue;
        }

        if let Some(kind) = DispositionKind::from_action(action) {
            if let Some((_, existing)) = disposition_scores.iter_mut().find(|(k, _)| *k == kind) {
                *existing = existing.max(score);
            }
        }
    }

    // Remove dispositions with zero or negative scores (no constituent action was available).
    disposition_scores.retain(|(_, score)| *score > 0.0);
    disposition_scores
}

/// Select a disposition using softmax (Boltzmann) sampling.
///
/// Same algorithm as `select_action_softmax` but over disposition scores.
/// Falls back to `Resting` if the slice is empty.
pub fn select_disposition_softmax(
    scores: &[(DispositionKind, f32)],
    rng: &mut impl Rng,
    sc: &ScoringConstants,
) -> DispositionKind {
    let temperature = sc.disposition_softmax_temperature;

    if scores.is_empty() {
        return DispositionKind::Resting;
    }

    let max_score = scores
        .iter()
        .map(|(_, s)| *s)
        .fold(f32::NEG_INFINITY, f32::max);
    let weights: Vec<f32> = scores
        .iter()
        .map(|(_, s)| ((s - max_score) / temperature).exp())
        .collect();
    let total: f32 = weights.iter().sum();

    let mut roll: f32 = rng.random::<f32>() * total;
    for (i, w) in weights.iter().enumerate() {
        roll -= w;
        if roll <= 0.0 {
            return scores[i].0;
        }
    }

    scores
        .last()
        .map(|(k, _)| *k)
        .unwrap_or(DispositionKind::Resting)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::rand_core::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn seeded_rng(seed: u64) -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(seed)
    }

    fn default_personality() -> Personality {
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

    fn default_scoring() -> ScoringConstants {
        ScoringConstants::default()
    }

    fn ctx<'a>(
        needs: &'a Needs,
        personality: &'a Personality,
        scoring: &'a ScoringConstants,
    ) -> ScoringContext<'a> {
        ScoringContext {
            scoring,
            needs,
            personality,
            food_available: true,
            can_hunt: true,
            can_forage: true,
            has_social_target: false,
            has_threat_nearby: false,
            allies_fighting_threat: 0,
            combat_effective: 0.05,
            is_incapacitated: false,
            has_construction_site: false,
            has_damaged_building: false,
            has_garden: false,
            food_fraction: 0.4,
            magic_affinity: 0.0,
            magic_skill: 0.0,
            herbcraft_skill: 0.0,
            has_herbs_nearby: false,
            has_herbs_in_inventory: false,
            has_remedy_herbs: false,
            has_ward_herbs: false,
            thornbriar_available: false,
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            tradition_location_bonus: 0.0,
            has_eligible_mate: false,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            unexplored_nearby: 1.0,
            health: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
        }
    }

    /// Starving cat (hunger=0.1, energy=0.8) with food available should score Eat highest.
    #[test]
    fn starving_cat_scores_eat_highest() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.1;
        needs.energy = 0.8;

        let personality = default_personality();
        let mut rng = seeded_rng(1);

        let scores = score_actions(&ctx(&needs, &personality, &sc), &mut rng).scores;
        let best = select_best_action(&scores);

        assert_eq!(
            best,
            Action::Eat,
            "starving cat should choose Eat; scores: {scores:?}"
        );

        // Confirm Eat is also strictly above Sleep
        let eat_score = scores.iter().find(|(a, _)| *a == Action::Eat).unwrap().1;
        let sleep_score = scores.iter().find(|(a, _)| *a == Action::Sleep).unwrap().1;
        assert!(
            eat_score > sleep_score,
            "Eat ({eat_score}) should beat Sleep ({sleep_score}) for a starving cat"
        );
    }

    /// Exhausted cat (energy=0.1, hunger=0.8) should score Sleep highest.
    #[test]
    fn exhausted_cat_scores_sleep_highest() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.energy = 0.1;
        needs.hunger = 0.8;

        let personality = default_personality();
        let mut rng = seeded_rng(2);

        let scores = score_actions(&ctx(&needs, &personality, &sc), &mut rng).scores;
        let best = select_best_action(&scores);

        assert_eq!(
            best,
            Action::Sleep,
            "exhausted cat should choose Sleep; scores: {scores:?}"
        );

        let sleep_score = scores.iter().find(|(a, _)| *a == Action::Sleep).unwrap().1;
        let eat_score = scores.iter().find(|(a, _)| *a == Action::Eat).unwrap().1;
        assert!(
            sleep_score > eat_score,
            "Sleep ({sleep_score}) should beat Eat ({eat_score}) for an exhausted cat"
        );
    }

    /// Satisfied curious cat (all needs high, high curiosity) with no food available should
    /// not pick Eat or Sleep — Wander, Explore, or Idle should win.
    #[test]
    fn satisfied_curious_cat_does_not_eat_or_sleep() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        // All needs well-met
        needs.hunger = 0.95;
        needs.energy = 0.95;
        needs.warmth = 0.95;
        needs.safety = 0.95;
        needs.social = 0.95;
        needs.acceptance = 0.95;
        needs.respect = 0.95;
        needs.mastery = 0.95;
        needs.purpose = 0.95;

        let mut personality = default_personality();
        personality.curiosity = 0.9; // highly curious

        let mut rng = seeded_rng(3);

        // No food, no hunt/forage targets — only Wander/Idle/Sleep/Groom/Explore available
        let c = ScoringContext {
            scoring: &sc,
            needs: &needs,
            personality: &personality,
            food_available: false,
            can_hunt: false,
            can_forage: false,
            has_social_target: false,
            has_threat_nearby: false,
            allies_fighting_threat: 0,
            combat_effective: 0.05,
            is_incapacitated: false,
            has_construction_site: false,
            has_damaged_building: false,
            has_garden: false,
            food_fraction: 0.4,
            magic_affinity: 0.0,
            magic_skill: 0.0,
            herbcraft_skill: 0.0,
            has_herbs_nearby: false,
            has_herbs_in_inventory: false,
            has_remedy_herbs: false,
            has_ward_herbs: false,
            thornbriar_available: false,
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            tradition_location_bonus: 0.0,
            has_eligible_mate: false,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            unexplored_nearby: 1.0,
            health: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
        };
        let scores = score_actions(&c, &mut rng).scores;
        let best = select_best_action(&scores);

        assert!(
            best == Action::Wander || best == Action::Idle || best == Action::Explore,
            "satisfied cat should wander, explore, or idle, got {best:?}; scores: {scores:?}"
        );
        assert_ne!(best, Action::Eat, "no food available, Eat should not win");
        assert_ne!(
            best,
            Action::Sleep,
            "well-rested cat should not sleep; scores: {scores:?}"
        );
    }

    /// A bold hungry cat with hunt available should prefer Hunt over Forage.
    #[test]
    fn bold_cat_prefers_hunt_over_forage() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.2;

        let mut personality = default_personality();
        personality.boldness = 0.9;
        personality.diligence = 0.3;

        let mut rng = seeded_rng(10);

        let scores = score_actions(&ctx(&needs, &personality, &sc), &mut rng).scores;
        let hunt_score = scores.iter().find(|(a, _)| *a == Action::Hunt).unwrap().1;
        let forage_score = scores.iter().find(|(a, _)| *a == Action::Forage).unwrap().1;

        assert!(
            hunt_score > forage_score,
            "bold cat should prefer Hunt ({hunt_score}) over Forage ({forage_score})"
        );
    }

    /// A diligent non-bold cat should prefer Forage over Hunt.
    #[test]
    fn diligent_cat_prefers_forage_over_hunt() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.2;

        let mut personality = default_personality();
        personality.boldness = 0.2;
        personality.diligence = 0.9;

        let mut rng = seeded_rng(11);

        let scores = score_actions(&ctx(&needs, &personality, &sc), &mut rng).scores;
        let hunt_score = scores.iter().find(|(a, _)| *a == Action::Hunt).unwrap().1;
        let forage_score = scores.iter().find(|(a, _)| *a == Action::Forage).unwrap().1;

        assert!(
            forage_score > hunt_score,
            "diligent cat should prefer Forage ({forage_score}) over Hunt ({hunt_score})"
        );
    }

    /// A lonely social cat with a visible target should score Socialize highly.
    #[test]
    fn lonely_social_cat_scores_socialize_high() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.social = 0.1; // very lonely
        needs.hunger = 0.9;
        needs.energy = 0.9;
        needs.warmth = 0.9;

        let mut personality = default_personality();
        personality.sociability = 0.9;

        let mut rng = seeded_rng(20);

        let c = ScoringContext {
            scoring: &sc,
            needs: &needs,
            personality: &personality,
            food_available: true,
            can_hunt: true,
            can_forage: true,
            has_social_target: true,
            has_threat_nearby: false,
            allies_fighting_threat: 0,
            combat_effective: 0.05,
            is_incapacitated: false,
            has_construction_site: false,
            has_damaged_building: false,
            has_garden: false,
            food_fraction: 0.4,
            magic_affinity: 0.0,
            magic_skill: 0.0,
            herbcraft_skill: 0.0,
            has_herbs_nearby: false,
            has_herbs_in_inventory: false,
            has_remedy_herbs: false,
            has_ward_herbs: false,
            thornbriar_available: false,
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            tradition_location_bonus: 0.0,
            has_eligible_mate: false,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            unexplored_nearby: 1.0,
            health: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
        };
        let scores = score_actions(&c, &mut rng).scores;
        let socialize_score = scores
            .iter()
            .find(|(a, _)| *a == Action::Socialize)
            .unwrap()
            .1;
        let idle_score = scores.iter().find(|(a, _)| *a == Action::Idle).unwrap().1;

        assert!(
            socialize_score > idle_score,
            "lonely social cat should score Socialize ({socialize_score}) above Idle ({idle_score})"
        );
    }

    /// Cold cat should score Groom highly (self-groom for warmth).
    #[test]
    fn cold_cat_scores_groom_high() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.warmth = 0.1;
        needs.hunger = 0.9;
        needs.energy = 0.9;

        let personality = default_personality();
        let mut rng = seeded_rng(21);

        let scores = score_actions(&ctx(&needs, &personality, &sc), &mut rng).scores;
        let groom_score = scores.iter().find(|(a, _)| *a == Action::Groom).unwrap().1;
        let idle_score = scores.iter().find(|(a, _)| *a == Action::Idle).unwrap().1;

        assert!(
            groom_score > idle_score,
            "cold cat should score Groom ({groom_score}) above Idle ({idle_score})"
        );
    }

    // --- Memory bonus tests ---

    use crate::components::mental::{Memory, MemoryEntry, MemoryType};
    use crate::components::physical::Position;
    use std::collections::HashMap;

    fn make_memory(event_type: MemoryType, location: Position, strength: f32) -> MemoryEntry {
        MemoryEntry {
            event_type,
            location: Some(location),
            involved: vec![],
            tick: 0,
            strength,
            firsthand: true,
        }
    }

    #[test]
    fn resource_memory_boosts_hunt_score() {
        let sc = default_scoring();
        let mut scores = vec![
            (Action::Hunt, 1.0),
            (Action::Forage, 1.0),
            (Action::Idle, 0.5),
        ];
        let mut memory = Memory::default();
        memory.remember(make_memory(
            MemoryType::ResourceFound,
            Position::new(5, 5),
            1.0,
        ));

        // Cat at (5, 5) — same tile as remembered resource.
        apply_memory_bonuses(&mut scores, &memory, &Position::new(5, 5), &sc);

        let hunt = scores.iter().find(|(a, _)| *a == Action::Hunt).unwrap().1;
        let idle = scores.iter().find(|(a, _)| *a == Action::Idle).unwrap().1;
        assert!(
            hunt > 1.0,
            "Hunt should be boosted above base 1.0; got {hunt}"
        );
        assert_eq!(idle, 0.5, "Idle should be unaffected; got {idle}");
    }

    #[test]
    fn death_memory_suppresses_wander() {
        let sc = default_scoring();
        let mut scores = vec![(Action::Wander, 1.0), (Action::Hunt, 1.0)];
        let mut memory = Memory::default();
        memory.remember(make_memory(MemoryType::Death, Position::new(5, 5), 1.0));

        apply_memory_bonuses(&mut scores, &memory, &Position::new(5, 5), &sc);

        let wander = scores.iter().find(|(a, _)| *a == Action::Wander).unwrap().1;
        let hunt = scores.iter().find(|(a, _)| *a == Action::Hunt).unwrap().1;
        assert!(
            wander < 1.0,
            "Wander should be suppressed near death site; got {wander}"
        );
        assert_eq!(
            hunt, 1.0,
            "Hunt should be unaffected by death memory; got {hunt}"
        );
    }

    #[test]
    fn distant_memories_have_less_effect() {
        let sc = default_scoring();
        let mut scores_near = vec![(Action::Hunt, 1.0)];
        let mut scores_far = vec![(Action::Hunt, 1.0)];
        let mut memory = Memory::default();
        memory.remember(make_memory(
            MemoryType::ResourceFound,
            Position::new(5, 5),
            1.0,
        ));

        apply_memory_bonuses(&mut scores_near, &memory, &Position::new(5, 5), &sc);
        apply_memory_bonuses(&mut scores_far, &memory, &Position::new(15, 5), &sc);

        let near = scores_near[0].1;
        let far = scores_far[0].1;
        assert!(
            near > far,
            "nearby memory should give bigger boost; near={near}, far={far}"
        );
    }

    #[test]
    fn decayed_memories_have_less_effect() {
        let sc = default_scoring();
        let mut scores_strong = vec![(Action::Hunt, 1.0)];
        let mut scores_weak = vec![(Action::Hunt, 1.0)];
        let mut memory_strong = Memory::default();
        memory_strong.remember(make_memory(
            MemoryType::ResourceFound,
            Position::new(5, 5),
            1.0,
        ));
        let mut memory_weak = Memory::default();
        memory_weak.remember(make_memory(
            MemoryType::ResourceFound,
            Position::new(5, 5),
            0.2,
        ));

        apply_memory_bonuses(
            &mut scores_strong,
            &memory_strong,
            &Position::new(5, 5),
            &sc,
        );
        apply_memory_bonuses(&mut scores_weak, &memory_weak, &Position::new(5, 5), &sc);

        let strong = scores_strong[0].1;
        let weak = scores_weak[0].1;
        assert!(
            strong > weak,
            "strong memory should give bigger boost; strong={strong}, weak={weak}"
        );
    }

    // --- Activity cascading tests ---

    #[test]
    fn cascading_boosts_matching_action() {
        let sc = default_scoring();
        let mut scores = vec![(Action::Hunt, 1.0), (Action::Idle, 0.5)];
        let nearby = HashMap::from([(Action::Hunt, 3)]);

        apply_cascading_bonuses(&mut scores, &nearby, &sc);

        let hunt = scores.iter().find(|(a, _)| *a == Action::Hunt).unwrap().1;
        assert!(
            (hunt - 1.24).abs() < 1e-5,
            "3 nearby hunters should add 0.24; got {hunt}"
        );
    }

    #[test]
    fn cascading_does_not_boost_unrelated_actions() {
        let sc = default_scoring();
        let mut scores = vec![(Action::Hunt, 1.0), (Action::Sleep, 0.5)];
        let nearby = HashMap::from([(Action::Hunt, 2)]);

        apply_cascading_bonuses(&mut scores, &nearby, &sc);

        let sleep = scores.iter().find(|(a, _)| *a == Action::Sleep).unwrap().1;
        assert_eq!(sleep, 0.5, "Sleep should be unaffected; got {sleep}");
    }

    // --- Flee / Fight / Patrol scoring tests ---

    #[test]
    fn cautious_cat_flees_when_threatened() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.safety = 0.2;

        let mut personality = default_personality();
        personality.boldness = 0.2; // cautious

        let mut rng = seeded_rng(30);

        let c = ScoringContext {
            scoring: &sc,
            needs: &needs,
            personality: &personality,
            food_available: true,
            can_hunt: true,
            can_forage: true,
            has_social_target: false,
            has_threat_nearby: true,
            allies_fighting_threat: 0,
            combat_effective: 0.15,
            is_incapacitated: false,
            has_construction_site: false,
            has_damaged_building: false,
            has_garden: false,
            food_fraction: 0.4,
            magic_affinity: 0.0,
            magic_skill: 0.0,
            herbcraft_skill: 0.0,
            has_herbs_nearby: false,
            has_herbs_in_inventory: false,
            has_remedy_herbs: false,
            has_ward_herbs: false,
            thornbriar_available: false,
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            tradition_location_bonus: 0.0,
            has_eligible_mate: false,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            unexplored_nearby: 1.0,
            health: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
        };
        let scores = score_actions(&c, &mut rng).scores;
        let best = select_best_action(&scores);

        assert_eq!(
            best,
            Action::Flee,
            "cautious cat with low safety should flee; scores: {scores:?}"
        );
    }

    #[test]
    fn bold_cat_fights_when_allies_present() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.safety = 0.3;

        let mut personality = default_personality();
        personality.boldness = 0.9;

        let mut rng = seeded_rng(31);

        let c = ScoringContext {
            scoring: &sc,
            needs: &needs,
            personality: &personality,
            food_available: true,
            can_hunt: true,
            can_forage: true,
            has_social_target: false,
            has_threat_nearby: true,
            allies_fighting_threat: 2,
            combat_effective: 0.35, // experienced hunter
            is_incapacitated: false,
            has_construction_site: false,
            has_damaged_building: false,
            has_garden: false,
            food_fraction: 0.4,
            magic_affinity: 0.0,
            magic_skill: 0.0,
            herbcraft_skill: 0.0,
            has_herbs_nearby: false,
            has_herbs_in_inventory: false,
            has_remedy_herbs: false,
            has_ward_herbs: false,
            thornbriar_available: false,
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            tradition_location_bonus: 0.0,
            has_eligible_mate: false,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            unexplored_nearby: 1.0,
            health: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
        };
        let scores = score_actions(&c, &mut rng).scores;
        let fight_score = scores.iter().find(|(a, _)| *a == Action::Fight).unwrap().1;
        let flee_score = scores.iter().find(|(a, _)| *a == Action::Flee);

        assert!(
            fight_score > 0.3,
            "bold cat with allies should have meaningful fight score; got {fight_score}"
        );
        // Bold cat shouldn't flee.
        if let Some((_, fs)) = flee_score {
            assert!(
                fight_score > *fs,
                "bold cat should prefer fight ({fight_score}) over flee ({fs})"
            );
        }
    }

    #[test]
    fn incapacitated_cat_only_scores_basic_actions() {
        let sc = default_scoring();
        let needs = Needs::default();
        let personality = default_personality();
        let mut rng = seeded_rng(40);

        let c = ScoringContext {
            scoring: &sc,
            needs: &needs,
            personality: &personality,
            food_available: true,
            can_hunt: true,
            can_forage: true,
            has_social_target: true,
            has_threat_nearby: true,
            allies_fighting_threat: 0,
            combat_effective: 0.1,
            is_incapacitated: true,
            has_construction_site: false,
            has_damaged_building: false,
            has_garden: false,
            food_fraction: 0.4,
            magic_affinity: 0.0,
            magic_skill: 0.0,
            herbcraft_skill: 0.0,
            has_herbs_nearby: false,
            has_herbs_in_inventory: false,
            has_remedy_herbs: false,
            has_ward_herbs: false,
            thornbriar_available: false,
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            tradition_location_bonus: 0.0,
            has_eligible_mate: false,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            unexplored_nearby: 1.0,
            health: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
        };
        let scores = score_actions(&c, &mut rng).scores;
        let actions: Vec<Action> = scores
            .iter()
            .filter(|(_, s)| *s > 0.0)
            .map(|(a, _)| *a)
            .collect();

        assert!(
            actions.contains(&Action::Eat),
            "incapacitated cat should be able to Eat"
        );
        assert!(
            actions.contains(&Action::Sleep),
            "incapacitated cat should be able to Sleep"
        );
        assert!(
            actions.contains(&Action::Idle),
            "incapacitated cat should be able to Idle"
        );
        assert!(
            !actions.contains(&Action::Hunt),
            "incapacitated cat should not Hunt"
        );
        assert!(
            !actions.contains(&Action::Fight),
            "incapacitated cat should not Fight"
        );
        assert!(
            !actions.contains(&Action::Flee),
            "incapacitated cat should not Flee"
        );
    }

    #[test]
    fn threat_memory_suppresses_wander_near_threat() {
        let sc = default_scoring();
        let mut scores = vec![
            (Action::Wander, 1.0),
            (Action::Explore, 1.0),
            (Action::Hunt, 1.0),
            (Action::Idle, 0.5),
        ];
        let mut memory = Memory::default();
        memory.remember(make_memory(
            MemoryType::ThreatSeen,
            Position::new(5, 5),
            1.0,
        ));

        apply_memory_bonuses(&mut scores, &memory, &Position::new(5, 5), &sc);

        let wander = scores.iter().find(|(a, _)| *a == Action::Wander).unwrap().1;
        let explore = scores
            .iter()
            .find(|(a, _)| *a == Action::Explore)
            .unwrap()
            .1;
        let hunt = scores.iter().find(|(a, _)| *a == Action::Hunt).unwrap().1;
        let idle = scores.iter().find(|(a, _)| *a == Action::Idle).unwrap().1;

        assert!(
            wander < 1.0,
            "wander should be suppressed near threat; got {wander}"
        );
        assert!(
            explore < 1.0,
            "explore should be suppressed near threat; got {explore}"
        );
        assert!(
            hunt < 1.0,
            "hunt should be suppressed near threat; got {hunt}"
        );
        assert_eq!(idle, 0.5, "idle should be unaffected; got {idle}");
    }

    // --- Herbcraft / PracticeMagic scoring tests ---

    #[test]
    fn spiritual_cat_with_herbs_nearby_scores_herbcraft() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.8;
        needs.energy = 0.8;

        let mut personality = default_personality();
        personality.spirituality = 0.9;

        let mut rng = seeded_rng(50);

        let mut c = ctx(&needs, &personality, &sc);
        c.has_herbs_nearby = true;
        c.herbcraft_skill = 0.3;

        let scores = score_actions(&c, &mut rng).scores;
        let herbcraft = scores.iter().find(|(a, _)| *a == Action::Herbcraft);

        assert!(
            herbcraft.is_some(),
            "spiritual cat with herbs nearby should score Herbcraft"
        );
        assert!(
            herbcraft.unwrap().1 > 0.0,
            "Herbcraft score should be positive"
        );
    }

    #[test]
    fn herbcraft_not_scored_without_herbs_or_inventory() {
        let sc = default_scoring();
        let needs = Needs::default();
        let personality = default_personality();
        let mut rng = seeded_rng(51);

        let c = ctx(&needs, &personality, &sc);
        // no herbs nearby, no inventory herbs

        let scores = score_actions(&c, &mut rng).scores;
        let herbcraft = scores.iter().find(|(a, _)| *a == Action::Herbcraft);
        assert!(
            herbcraft.is_none(),
            "no herbs → no Herbcraft; scores: {scores:?}"
        );
    }

    #[test]
    fn practice_magic_requires_prereqs() {
        let sc = default_scoring();
        let needs = Needs::default();
        let personality = default_personality();
        let mut rng = seeded_rng(52);

        // Below prereqs: affinity 0.2 < 0.3 threshold
        let mut c = ctx(&needs, &personality, &sc);
        c.magic_affinity = 0.2;
        c.magic_skill = 0.3;

        let scores = score_actions(&c, &mut rng).scores;
        let magic = scores.iter().find(|(a, _)| *a == Action::PracticeMagic);
        assert!(
            magic.is_none(),
            "below affinity threshold → no PracticeMagic"
        );

        // Below prereqs: skill 0.1 < 0.2 threshold
        let mut c2 = ctx(&needs, &personality, &sc);
        c2.magic_affinity = 0.5;
        c2.magic_skill = 0.1;

        let scores2 = score_actions(&c2, &mut rng).scores;
        let magic2 = scores2.iter().find(|(a, _)| *a == Action::PracticeMagic);
        assert!(magic2.is_none(), "below skill threshold → no PracticeMagic");
    }

    #[test]
    fn magical_cat_on_corrupted_tile_scores_practice_magic() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.8;
        needs.energy = 0.8;

        let mut personality = default_personality();
        personality.spirituality = 0.8;

        let mut rng = seeded_rng(53);

        let mut c = ctx(&needs, &personality, &sc);
        c.magic_affinity = 0.6;
        c.magic_skill = 0.4;
        c.on_corrupted_tile = true;
        c.tile_corruption = 0.5;

        let scores = score_actions(&c, &mut rng).scores;
        let magic = scores.iter().find(|(a, _)| *a == Action::PracticeMagic);

        assert!(
            magic.is_some(),
            "magical cat on corrupted tile should score PracticeMagic"
        );
        assert!(
            magic.unwrap().1 > 0.0,
            "PracticeMagic score should be positive"
        );
    }

    #[test]
    fn compassionate_cat_with_remedy_herbs_and_injured_ally_scores_herbcraft() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.8;
        needs.energy = 0.8;

        let mut personality = default_personality();
        personality.compassion = 0.9;

        let mut rng = seeded_rng(54);

        let mut c = ctx(&needs, &personality, &sc);
        c.has_remedy_herbs = true;
        c.has_herbs_in_inventory = true;
        c.herbcraft_skill = 0.4;
        c.colony_injury_count = 2;

        let scores = score_actions(&c, &mut rng).scores;
        let herbcraft = scores.iter().find(|(a, _)| *a == Action::Herbcraft);

        assert!(
            herbcraft.is_some() && herbcraft.unwrap().1 > 0.15,
            "compassionate cat with remedy herbs and injured allies should score Herbcraft; got {herbcraft:?}"
        );
    }

    /// Average cat with met needs should pick Wander over Idle.
    #[test]
    fn wander_beats_idle_for_average_cat() {
        let sc = default_scoring();
        let needs = Needs::default();
        let personality = default_personality();
        let mut rng = seeded_rng(42);

        let c = ScoringContext {
            scoring: &sc,
            needs: &needs,
            personality: &personality,
            food_available: false,
            can_hunt: false,
            can_forage: false,
            has_social_target: false,
            has_threat_nearby: false,
            allies_fighting_threat: 0,
            combat_effective: 0.05,
            is_incapacitated: false,
            has_construction_site: false,
            has_damaged_building: false,
            has_garden: false,
            food_fraction: 0.5,
            magic_affinity: 0.0,
            magic_skill: 0.0,
            herbcraft_skill: 0.0,
            has_herbs_nearby: false,
            has_herbs_in_inventory: false,
            has_remedy_herbs: false,
            has_ward_herbs: false,
            thornbriar_available: false,
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            tradition_location_bonus: 0.0,
            has_eligible_mate: false,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            unexplored_nearby: 1.0,
            health: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
        };
        let scores = score_actions(&c, &mut rng).scores;
        let wander = scores.iter().find(|(a, _)| *a == Action::Wander).unwrap().1;
        let idle = scores.iter().find(|(a, _)| *a == Action::Idle).unwrap().1;
        assert!(
            wander > idle,
            "Wander ({wander:.3}) should beat Idle ({idle:.3}) for an average cat"
        );
    }

    /// Low food stores should boost Hunt score even when the cat isn't personally hungry.
    #[test]
    fn low_food_stores_boost_hunt_score() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.9; // not personally hungry
        needs.energy = 0.9;

        let mut personality = default_personality();
        personality.boldness = 0.6;

        let mut rng_full = seeded_rng(50);
        let mut rng_low = seeded_rng(50);

        let base = ScoringContext {
            scoring: &sc,
            needs: &needs,
            personality: &personality,
            food_available: true,
            can_hunt: true,
            can_forage: false,
            has_social_target: false,
            has_threat_nearby: false,
            allies_fighting_threat: 0,
            combat_effective: 0.05,
            is_incapacitated: false,
            has_construction_site: false,
            has_damaged_building: false,
            has_garden: false,
            food_fraction: 0.9,
            magic_affinity: 0.0,
            magic_skill: 0.0,
            herbcraft_skill: 0.0,
            has_herbs_nearby: false,
            has_herbs_in_inventory: false,
            has_remedy_herbs: false,
            has_ward_herbs: false,
            thornbriar_available: false,
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            tradition_location_bonus: 0.0,
            has_eligible_mate: false,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            unexplored_nearby: 1.0,
            health: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
        };

        let scores_full = score_actions(&base, &mut rng_full).scores;
        let hunt_full = scores_full
            .iter()
            .find(|(a, _)| *a == Action::Hunt)
            .unwrap()
            .1;

        let low = ScoringContext {
            food_fraction: 0.2,
            ..base
        };
        let scores_low = score_actions(&low, &mut rng_low).scores;
        let hunt_low = scores_low
            .iter()
            .find(|(a, _)| *a == Action::Hunt)
            .unwrap()
            .1;

        assert!(
            hunt_low > hunt_full,
            "Hunt with low stores ({hunt_low:.3}) should exceed Hunt with full stores ({hunt_full:.3})"
        );
    }

    // --- Survival floor tests ---

    #[test]
    fn survival_floor_caps_build_when_starving() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.1; // critically hungry
        needs.energy = 0.9;
        needs.warmth = 0.9;

        // Simulate a bonus-inflated Build score beating Eat.
        let mut scores = vec![
            (Action::Eat, 1.6),   // hungry cat, high Eat score
            (Action::Build, 2.5), // bonus-inflated Build
            (Action::Idle, 0.1),
        ];

        enforce_survival_floor(&mut scores, &needs, &sc);

        let eat = scores.iter().find(|(a, _)| *a == Action::Eat).unwrap().1;
        let build = scores.iter().find(|(a, _)| *a == Action::Build).unwrap().1;
        assert!(
            eat >= build,
            "starving cat: Eat ({eat:.3}) should >= Build ({build:.3})"
        );
    }

    #[test]
    fn survival_floor_inactive_when_needs_met() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.8;
        needs.energy = 0.8;
        needs.warmth = 0.8;

        let mut scores = vec![
            (Action::Eat, 0.5),
            (Action::Build, 1.5),
            (Action::Idle, 0.1),
        ];
        let build_before = 1.5f32;

        enforce_survival_floor(&mut scores, &needs, &sc);

        let build = scores.iter().find(|(a, _)| *a == Action::Build).unwrap().1;
        assert_eq!(
            build, build_before,
            "well-fed cat: Build should be untouched"
        );
    }

    #[test]
    fn survival_floor_gradual() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.3; // moderately hungry
        needs.energy = 0.9;
        needs.warmth = 0.9;

        let mut scores = vec![
            (Action::Eat, 1.2),
            (Action::Build, 2.0),
            (Action::Idle, 0.1),
        ];

        enforce_survival_floor(&mut scores, &needs, &sc);

        let eat = scores.iter().find(|(a, _)| *a == Action::Eat).unwrap().1;
        let build = scores.iter().find(|(a, _)| *a == Action::Build).unwrap().1;
        // Build should be compressed but not fully capped.
        assert!(
            build < 2.0,
            "moderately hungry: Build ({build:.3}) should be compressed below 2.0"
        );
        assert!(
            build > eat,
            "moderately hungry: Build ({build:.3}) may still beat Eat ({eat:.3}) — partial suppression"
        );
    }

    #[test]
    fn survival_floor_exempts_flee() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.05; // nearly starving
        needs.energy = 0.9;
        needs.warmth = 0.9;

        let mut scores = vec![
            (Action::Eat, 1.8),
            (Action::Flee, 3.0),
            (Action::Build, 2.0),
        ];

        enforce_survival_floor(&mut scores, &needs, &sc);

        let flee = scores.iter().find(|(a, _)| *a == Action::Flee).unwrap().1;
        assert_eq!(flee, 3.0, "Flee should be exempt from survival floor");
    }

    // --- Disposition aggregation tests ---

    #[test]
    fn aggregate_maps_hunt_to_hunting() {
        let scores = vec![
            (Action::Hunt, 1.5),
            (Action::Eat, 1.0),
            (Action::Sleep, 0.8),
        ];
        let disp = aggregate_to_dispositions(&scores, true);
        let hunting = disp.iter().find(|(k, _)| *k == DispositionKind::Hunting);
        assert_eq!(hunting.unwrap().1, 1.5);
    }

    #[test]
    fn aggregate_takes_max_of_constituent_actions() {
        let scores = vec![(Action::Patrol, 0.5), (Action::Fight, 1.2)];
        let disp = aggregate_to_dispositions(&scores, true);
        let guarding = disp.iter().find(|(k, _)| *k == DispositionKind::Guarding);
        assert_eq!(guarding.unwrap().1, 1.2);
    }

    #[test]
    fn aggregate_routes_self_groom_to_resting() {
        let scores = vec![(Action::Groom, 0.9), (Action::Eat, 0.5)];
        let disp = aggregate_to_dispositions(&scores, true);
        let resting = disp.iter().find(|(k, _)| *k == DispositionKind::Resting);
        assert_eq!(resting.unwrap().1, 0.9);
        // Should NOT appear under Socializing
        let socializing = disp
            .iter()
            .find(|(k, _)| *k == DispositionKind::Socializing);
        assert!(socializing.is_none());
    }

    #[test]
    fn aggregate_routes_other_groom_to_socializing() {
        let scores = vec![(Action::Groom, 0.9), (Action::Socialize, 0.5)];
        let disp = aggregate_to_dispositions(&scores, false);
        let socializing = disp
            .iter()
            .find(|(k, _)| *k == DispositionKind::Socializing);
        assert_eq!(socializing.unwrap().1, 0.9);
    }

    #[test]
    fn aggregate_excludes_flee_and_idle() {
        let scores = vec![
            (Action::Flee, 3.0),
            (Action::Idle, 0.1),
            (Action::Hunt, 1.0),
        ];
        let disp = aggregate_to_dispositions(&scores, true);
        assert!(disp.iter().all(|(k, _)| *k != DispositionKind::Resting
            || disp
                .iter()
                .find(|(dk, _)| *dk == DispositionKind::Resting)
                .is_none()));
        // No disposition should have the Flee score
        assert!(disp.iter().all(|(_, s)| *s <= 1.0));
    }

    #[test]
    fn aggregate_omits_zero_score_dispositions() {
        let scores = vec![(Action::Hunt, 1.0)];
        let disp = aggregate_to_dispositions(&scores, true);
        // Only Hunting should appear
        assert_eq!(disp.len(), 1);
        assert_eq!(disp[0].0, DispositionKind::Hunting);
    }

    #[test]
    fn disposition_softmax_returns_resting_for_empty() {
        let sc = default_scoring();
        let mut rng = seeded_rng(42);
        assert_eq!(
            select_disposition_softmax(&[], &mut rng, &sc),
            DispositionKind::Resting,
        );
    }

    // --- Behavior gate tests ---

    #[test]
    fn gate_timid_fight_becomes_flee() {
        let sc = default_scoring();
        let mut p = default_personality();
        p.boldness = 0.05;
        let mut rng = seeded_rng(1);
        assert_eq!(
            behavior_gate_check(Action::Fight, &p, false, 1.0, &mut rng, &sc),
            Some(Action::Flee),
        );
    }

    #[test]
    fn gate_shy_socialize_becomes_idle() {
        let sc = default_scoring();
        let mut p = default_personality();
        p.sociability = 0.1;
        let mut rng = seeded_rng(1);
        assert_eq!(
            behavior_gate_check(Action::Socialize, &p, false, 1.0, &mut rng, &sc),
            Some(Action::Idle),
        );
    }

    #[test]
    fn gate_reckless_flee_becomes_fight() {
        let sc = default_scoring();
        let mut p = default_personality();
        p.boldness = 0.95;
        let mut rng = seeded_rng(1);
        assert_eq!(
            behavior_gate_check(Action::Flee, &p, false, 1.0, &mut rng, &sc),
            Some(Action::Fight),
        );
    }

    #[test]
    fn gate_reckless_flee_not_when_injured() {
        let sc = default_scoring();
        let mut p = default_personality();
        p.boldness = 0.95;
        let mut rng = seeded_rng(1);
        // Below health threshold — reckless gate should NOT fire.
        assert_eq!(
            behavior_gate_check(Action::Flee, &p, false, 0.3, &mut rng, &sc),
            None,
        );
    }

    #[test]
    fn gate_compulsive_helper() {
        let sc = default_scoring();
        let mut p = default_personality();
        p.compassion = 0.95;
        let mut rng = seeded_rng(1);
        assert_eq!(
            behavior_gate_check(Action::Wander, &p, true, 1.0, &mut rng, &sc),
            Some(Action::Herbcraft),
        );
        // No injured nearby → no override.
        assert_eq!(
            behavior_gate_check(Action::Wander, &p, false, 1.0, &mut rng, &sc),
            None,
        );
    }

    #[test]
    fn gate_no_override_for_normal_personality() {
        let sc = default_scoring();
        let p = default_personality();
        let mut rng = seeded_rng(1);
        assert_eq!(
            behavior_gate_check(Action::Hunt, &p, false, 1.0, &mut rng, &sc),
            None,
        );
    }

    #[test]
    fn gate_compulsive_explorer_fires_probabilistically() {
        let sc = default_scoring();
        let mut p = default_personality();
        p.curiosity = 0.95;
        // Run 200 trials — expect roughly 20% to fire.
        let mut fires = 0;
        let mut rng = seeded_rng(42);
        for _ in 0..200 {
            if behavior_gate_check(Action::Patrol, &p, false, 1.0, &mut rng, &sc)
                == Some(Action::Explore)
            {
                fires += 1;
            }
        }
        assert!(
            fires > 20 && fires < 60,
            "compulsive explorer should fire ~20% of the time; fired {fires}/200"
        );
    }

    #[test]
    fn gate_compulsive_explorer_does_not_override_survival() {
        let sc = default_scoring();
        let mut p = default_personality();
        p.curiosity = 0.95;
        let mut rng = seeded_rng(1);
        // Eat, Sleep, Flee should never be overridden by compulsive explorer.
        for action in [Action::Eat, Action::Sleep, Action::Flee] {
            assert_eq!(
                behavior_gate_check(action, &p, false, 1.0, &mut rng, &sc),
                None,
                "compulsive explorer should not override {action:?}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Herbcraft hint tests
    // -----------------------------------------------------------------------

    #[test]
    fn herbcraft_hint_prepare() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.8;
        needs.energy = 0.8;

        let mut personality = default_personality();
        personality.compassion = 0.9;

        let mut rng = seeded_rng(70);

        let mut c = ctx(&needs, &personality, &sc);
        c.has_remedy_herbs = true;
        c.has_herbs_in_inventory = true;
        c.herbcraft_skill = 0.4;
        c.colony_injury_count = 2;
        // Also set herbs nearby so gather *could* fire — hint should still be PrepareRemedy.
        c.has_herbs_nearby = true;

        let result = score_actions(&c, &mut rng);
        assert_eq!(
            result.herbcraft_hint,
            Some(CraftingHint::PrepareRemedy),
            "with remedy herbs + injuries, hint should be PrepareRemedy; got {:?}",
            result.herbcraft_hint
        );
    }

    #[test]
    fn herbcraft_hint_gather() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.8;
        needs.energy = 0.8;

        let mut personality = default_personality();
        personality.spirituality = 0.9;

        let mut rng = seeded_rng(71);

        let mut c = ctx(&needs, &personality, &sc);
        c.has_herbs_nearby = true;
        c.herbcraft_skill = 0.3;
        // No remedy herbs, no ward herbs — only gather is viable.

        let result = score_actions(&c, &mut rng);
        assert_eq!(
            result.herbcraft_hint,
            Some(CraftingHint::GatherHerbs),
            "with only herbs nearby, hint should be GatherHerbs; got {:?}",
            result.herbcraft_hint
        );
    }

    #[test]
    fn herbcraft_hint_ward() {
        let sc = default_scoring();
        // Ward is Level 2 (Safety) — only physiological needs matter.
        let mut needs = Needs::default();
        needs.hunger = 0.8;
        needs.energy = 0.8;

        let mut personality = default_personality();
        personality.spirituality = 0.9;

        let mut rng = seeded_rng(72);

        let mut c = ctx(&needs, &personality, &sc);
        c.has_ward_herbs = true;
        c.has_herbs_in_inventory = true;
        c.ward_strength_low = true;
        c.herbcraft_skill = 0.3;
        // No herbs nearby, no remedy herbs — only ward is viable.

        let result = score_actions(&c, &mut rng);
        assert_eq!(
            result.herbcraft_hint,
            Some(CraftingHint::SetWard),
            "with ward herbs + low ward strength, hint should be SetWard; got {:?}",
            result.herbcraft_hint
        );
    }

    #[test]
    fn ward_corruption_emergency_boosts_score() {
        let sc = default_scoring();
        let needs = Needs::default(); // all needs satisfied
        let mut personality = default_personality();
        personality.spirituality = 0.6;

        let mut rng = seeded_rng(100);

        let mut c = ctx(&needs, &personality, &sc);
        c.has_ward_herbs = true;
        c.ward_strength_low = true;
        c.herbcraft_skill = 0.1;
        c.territory_max_corruption = 0.5; // corruption detected in territory ring

        let result = score_actions(&c, &mut rng);
        let ward_score = result
            .scores
            .iter()
            .find(|(a, _)| matches!(a, Action::Herbcraft))
            .map(|(_, s)| *s)
            .unwrap_or(0.0);

        // With emergency bonus (1.2) the ward score should be well above 1.0.
        assert!(
            ward_score > 1.0,
            "ward with corruption emergency should score > 1.0; got {ward_score:.3}"
        );
    }

    #[test]
    fn ward_no_corruption_no_emergency() {
        let sc = default_scoring();
        let needs = Needs::default();
        let mut personality = default_personality();
        personality.spirituality = 0.6;

        let mut rng = seeded_rng(101);

        let mut c = ctx(&needs, &personality, &sc);
        c.has_ward_herbs = true;
        c.ward_strength_low = true;
        c.herbcraft_skill = 0.1;
        c.territory_max_corruption = 0.0; // no corruption

        let result = score_actions(&c, &mut rng);
        let ward_score = result
            .scores
            .iter()
            .find(|(a, _)| matches!(a, Action::Herbcraft))
            .map(|(_, s)| *s)
            .unwrap_or(0.0);

        // Without corruption, base ward score should be small (no emergency bonus).
        assert!(
            ward_score < 0.5,
            "ward without corruption should score < 0.5; got {ward_score:.3}"
        );
    }
}
