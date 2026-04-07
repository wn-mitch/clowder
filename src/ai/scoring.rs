use std::collections::HashMap;

use rand::Rng;

use crate::ai::Action;
use crate::components::mental::{Memory, MemoryType};
use crate::components::personality::Personality;
use crate::components::physical::{Needs, Position};

// ---------------------------------------------------------------------------
// Jitter
// ---------------------------------------------------------------------------

/// Small random noise added to every score to break ties and add variety.
fn jitter(rng: &mut impl Rng) -> f32 {
    rng.random_range(-0.05f32..0.05f32)
}

// ---------------------------------------------------------------------------
// ScoringContext
// ---------------------------------------------------------------------------

/// Everything the scoring function needs to evaluate available actions.
pub struct ScoringContext<'a> {
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
    /// Number of injured cats in the colony.
    pub colony_injury_count: usize,
    /// Whether colony ward coverage is low (no wards or average strength < 0.3).
    pub ward_strength_low: bool,
    /// Whether the cat is standing on a corrupted tile.
    pub on_corrupted_tile: bool,
    /// Corruption level of the cat's current tile.
    pub tile_corruption: f32,
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
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

/// Score all available actions for a cat given its current state.
///
/// Returns a `Vec` of `(Action, score)` pairs. Higher score = more preferred.
/// The caller should pass the result to [`select_best_action`].
pub fn score_actions(
    ctx: &ScoringContext,
    rng: &mut impl Rng,
) -> Vec<(Action, f32)> {
    let mut scores = Vec::with_capacity(12);

    // Incapacitated cats can only Eat, Sleep, or Idle.
    if ctx.is_incapacitated {
        if ctx.food_available {
            let urgency = (1.0 - ctx.needs.hunger) * 2.0 + 0.3;
            scores.push((Action::Eat, urgency + jitter(rng)));
        }
        let urgency = (1.0 - ctx.needs.energy) * 2.0 + 0.3;
        scores.push((Action::Sleep, urgency + jitter(rng)));
        scores.push((Action::Idle, 0.2 + jitter(rng)));
        return scores;
    }

    // --- Eat (only when food stores are available) ---
    if ctx.food_available {
        let urgency = (1.0 - ctx.needs.hunger) * 2.0 * ctx.needs.level_suppression(1);
        scores.push((Action::Eat, urgency + jitter(rng)));
    }

    // --- Sleep ---
    {
        let urgency = (1.0 - ctx.needs.energy) * 2.0 * ctx.needs.level_suppression(1);
        scores.push((Action::Sleep, urgency + jitter(rng)));
    }

    // --- Hunt (boldness-driven; requires reachable forest/grass and nearby prey) ---
    if ctx.can_hunt && ctx.prey_nearby {
        let food_scarcity = (1.0 - ctx.food_fraction) * 0.5;
        let urgency = ((1.0 - ctx.needs.hunger) + food_scarcity)
            * ctx.personality.boldness * 1.5
            * ctx.needs.level_suppression(1);
        scores.push((Action::Hunt, urgency + jitter(rng)));
    }

    // --- Forage (diligence-driven; requires terrain with yield) ---
    if ctx.can_forage {
        let food_scarcity = (1.0 - ctx.food_fraction) * 0.5;
        let urgency = ((1.0 - ctx.needs.hunger) + food_scarcity)
            * ctx.personality.diligence * 1.2
            * ctx.needs.level_suppression(1);
        scores.push((Action::Forage, urgency + jitter(rng)));
    }

    // --- Socialize (sociability-driven; requires a visible cat) ---
    if ctx.has_social_target {
        let score = (1.0 - ctx.needs.social) * ctx.personality.sociability * 1.5
            * ctx.needs.level_suppression(3);
        scores.push((Action::Socialize, score + jitter(rng)));
    }

    // --- Groom (self or other; always available for self) ---
    {
        let self_groom = (1.0 - ctx.needs.warmth) * 0.8 * ctx.needs.level_suppression(1);
        let other_groom = if ctx.has_social_target {
            ctx.personality.warmth * (1.0 - ctx.needs.social) * ctx.needs.level_suppression(3)
        } else {
            0.0
        };
        scores.push((Action::Groom, self_groom.max(other_groom) + jitter(rng)));
    }

    // --- Explore (curiosity-driven; suppressed when unsafe or hungry) ---
    {
        let score = ctx.personality.curiosity * 0.6 * ctx.needs.level_suppression(3);
        scores.push((Action::Explore, score + jitter(rng)));
    }

    // --- Wander (light movement; suppressed only by unmet physiological needs) ---
    {
        let score = ctx.personality.curiosity * 0.4 * ctx.needs.level_suppression(2) + 0.08;
        scores.push((Action::Wander, score + jitter(rng)));
    }

    // --- Flee (fear-driven; scored when threat detected or safety is low) ---
    if ctx.has_threat_nearby || ctx.needs.safety < 0.5 {
        let score = (1.0 - ctx.needs.safety) * 3.0 * (1.0 - ctx.personality.boldness)
            * ctx.needs.level_suppression(2);
        scores.push((Action::Flee, score + jitter(rng)));
    }

    // --- Fight (boldness + combat; only with 3+ cats engaging the same threat) ---
    if ctx.has_threat_nearby && ctx.allies_fighting_threat >= 2 {
        let group_bonus = ctx.allies_fighting_threat as f32 * 0.15;
        let score = ctx.personality.boldness * 1.5 * ctx.combat_effective
            * ctx.needs.level_suppression(2)
            + group_bonus;
        scores.push((Action::Fight, score + jitter(rng)));
    }

    // --- Patrol (proactive safety-seeking; available when safety < 0.8) ---
    if ctx.needs.safety < 0.8 {
        let score = ctx.personality.boldness * 0.5 * (1.0 - ctx.needs.safety)
            * ctx.needs.level_suppression(2);
        scores.push((Action::Patrol, score + jitter(rng)));
    }

    // --- Build (diligence-driven; scored when construction/repair is needed) ---
    if ctx.has_construction_site || ctx.has_damaged_building {
        let base = ctx.personality.diligence * 0.8 * ctx.needs.level_suppression(4);
        let site_bonus = if ctx.has_construction_site { 0.2 } else { 0.0 };
        let repair_bonus = if ctx.has_damaged_building { 0.15 } else { 0.0 };
        scores.push((Action::Build, base + site_bonus + repair_bonus + jitter(rng)));
    }

    // --- Farm (diligence-driven; scored when garden exists and food is low) ---
    if ctx.has_garden {
        let urgency = (1.0 - ctx.food_fraction) * ctx.personality.diligence * 0.6
            * ctx.needs.level_suppression(3);
        scores.push((Action::Farm, urgency + jitter(rng)));
    }

    // --- Herbcraft (spirituality + herbcraft skill; three sub-modes) ---
    {
        let gather = if ctx.has_herbs_nearby {
            ctx.personality.spirituality * 0.5 * (0.1 + ctx.herbcraft_skill)
                * ctx.needs.level_suppression(3)
        } else {
            0.0
        };
        let prepare = if ctx.has_remedy_herbs && ctx.colony_injury_count > 0 {
            ctx.personality.compassion * (0.1 + ctx.herbcraft_skill)
                * (ctx.colony_injury_count as f32 * 0.3).min(1.5)
                * ctx.needs.level_suppression(3)
        } else {
            0.0
        };
        let ward = if ctx.has_ward_herbs && ctx.ward_strength_low {
            ctx.personality.spirituality * (0.1 + ctx.herbcraft_skill) * 0.6
                * ctx.needs.level_suppression(4)
        } else {
            0.0
        };
        let best = gather.max(prepare).max(ward);
        if best > 0.0 {
            scores.push((Action::Herbcraft, best + jitter(rng)));
        }
    }

    // --- PracticeMagic (requires affinity > 0.3 AND magic skill > 0.2) ---
    if ctx.magic_affinity > 0.3 && ctx.magic_skill > 0.2 {
        let scry = ctx.personality.curiosity * ctx.personality.spirituality
            * ctx.magic_skill * ctx.needs.level_suppression(5);
        let durable_ward = if ctx.ward_strength_low && ctx.magic_skill > 0.5 {
            ctx.personality.spirituality * ctx.magic_skill * 0.8
                * ctx.needs.level_suppression(4)
        } else {
            0.0
        };
        let cleanse = if ctx.on_corrupted_tile && ctx.tile_corruption > 0.1 {
            ctx.personality.spirituality * ctx.magic_skill * ctx.tile_corruption
                * ctx.needs.level_suppression(4)
        } else {
            0.0
        };
        let commune = if ctx.on_special_terrain {
            ctx.personality.spirituality * ctx.magic_skill * 0.7
                * ctx.needs.level_suppression(5)
        } else {
            0.0
        };
        let best = scry.max(durable_ward).max(cleanse).max(commune);
        if best > 0.0 {
            scores.push((Action::PracticeMagic, best + jitter(rng)));
        }
    }

    // --- Coordinate (coordinator with pending directives only) ---
    if ctx.is_coordinator_with_directives {
        let score = ctx.personality.diligence * 0.8
            * (ctx.pending_directive_count as f32 * 0.3)
            * ctx.needs.level_suppression(4);
        scores.push((Action::Coordinate, score + jitter(rng)));
    }

    // --- Mentor (warmth + diligence; requires valid mentoring target) ---
    if ctx.has_mentoring_target {
        let score = ctx.personality.warmth * ctx.personality.diligence * 0.5
            * ctx.needs.level_suppression(4);
        scores.push((Action::Mentor, score + jitter(rng)));
    }

    // --- Idle (always-available fallback; incurious cats idle more) ---
    scores.push((Action::Idle, 0.05 + (1.0 - ctx.personality.curiosity) * 0.08 + jitter(rng)));

    scores
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
) {
    const NEARBY_RADIUS: f32 = 15.0;

    for entry in &memory.events {
        let Some(loc) = &entry.location else { continue };
        let dist = pos.distance_to(loc);
        if dist > NEARBY_RADIUS {
            continue;
        }

        let proximity = 1.0 - (dist / NEARBY_RADIUS);
        let bonus = proximity * entry.strength;

        match entry.event_type {
            MemoryType::ResourceFound => {
                for (action, score) in scores.iter_mut() {
                    if matches!(action, Action::Hunt | Action::Forage) {
                        *score += bonus * 0.2;
                    }
                }
            }
            MemoryType::Death => {
                for (action, score) in scores.iter_mut() {
                    if matches!(action, Action::Wander | Action::Idle) {
                        *score -= bonus * 0.1;
                    }
                }
            }
            MemoryType::ThreatSeen => {
                // Suppress exploration and hunting near known threat locations.
                for (action, score) in scores.iter_mut() {
                    if matches!(action, Action::Wander | Action::Explore | Action::Hunt) {
                        *score -= bonus * 0.15;
                    }
                }
            }
            _ => {}
        }
    }
}

/// Boost action scores based on what nearby cats are doing.
///
/// For each action, adds `+0.15 * count` where `count` is the number of cats
/// within 5 tiles performing that action. Creates emergent group behaviors.
pub fn apply_cascading_bonuses(
    scores: &mut [(Action, f32)],
    nearby_actions: &HashMap<Action, usize>,
) {
    for (action, score) in scores.iter_mut() {
        if let Some(&count) = nearby_actions.get(action) {
            *score += count as f32 * 0.15;
        }
    }
}

/// Apply a coordinator's directive bonus to the target action's score.
///
/// Called after base scoring and cascading bonuses. The bonus is pre-computed
/// from the directive priority, coordinator social weight, and the target cat's
/// personality (diligence, independence, stubbornness).
pub fn apply_directive_bonus(
    scores: &mut [(Action, f32)],
    target_action: Action,
    bonus: f32,
) {
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
) {
    const KNOWLEDGE_RADIUS: f32 = 20.0;

    for entry in &knowledge.entries {
        let Some(loc) = &entry.location else { continue };
        let dist = pos.distance_to(loc);
        if dist > KNOWLEDGE_RADIUS {
            continue;
        }

        let proximity = 1.0 - (dist / KNOWLEDGE_RADIUS);
        let bonus = proximity * entry.strength * 0.15;

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
) {
    let Some(kind) = priority else { return };
    use crate::resources::colony_priority::PriorityKind;
    let bonus = 0.15;
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
) {
    const BONUS: f32 = 0.2;
    for asp in &aspirations.active {
        let matching = asp.domain.matching_actions();
        for (action, score) in scores.iter_mut() {
            if matching.contains(action) {
                *score += BONUS;
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
) {
    for (action, score) in scores.iter_mut() {
        match preferences.get(*action) {
            Some(crate::components::aspirations::Preference::Like) => *score += 0.08,
            Some(crate::components::aspirations::Preference::Dislike) => *score -= 0.08,
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
) {
    if fated_love_visible {
        for (action, score) in scores.iter_mut() {
            if matches!(action, Action::Socialize | Action::Groom) {
                *score += 0.15;
            }
        }
    }
    if fated_rival_nearby {
        for (action, score) in scores.iter_mut() {
            if matches!(action, Action::Hunt | Action::Patrol | Action::Fight | Action::Explore) {
                *score += 0.1;
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
pub fn enforce_survival_floor(scores: &mut [(Action, f32)], needs: &Needs) {
    let phys = needs.physiological_satisfaction();
    if phys >= 0.5 {
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

    let factor = phys / 0.5; // 1.0 at phys=0.5, 0.0 at phys=0.0
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha8Rng;
    use rand_chacha::rand_core::SeedableRng;

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

    fn ctx<'a>(needs: &'a Needs, personality: &'a Personality) -> ScoringContext<'a> {
        ScoringContext {
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
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
        }
    }

    /// Starving cat (hunger=0.1, energy=0.8) with food available should score Eat highest.
    #[test]
    fn starving_cat_scores_eat_highest() {
        let mut needs = Needs::default();
        needs.hunger = 0.1;
        needs.energy = 0.8;

        let personality = default_personality();
        let mut rng = seeded_rng(1);

        let scores = score_actions(&ctx(&needs, &personality), &mut rng);
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
        let mut needs = Needs::default();
        needs.energy = 0.1;
        needs.hunger = 0.8;

        let personality = default_personality();
        let mut rng = seeded_rng(2);

        let scores = score_actions(&ctx(&needs, &personality), &mut rng);
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
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
        };
        let scores = score_actions(&c, &mut rng);
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
        let mut needs = Needs::default();
        needs.hunger = 0.2;

        let mut personality = default_personality();
        personality.boldness = 0.9;
        personality.diligence = 0.3;

        let mut rng = seeded_rng(10);

        let scores = score_actions(&ctx(&needs, &personality), &mut rng);
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
        let mut needs = Needs::default();
        needs.hunger = 0.2;

        let mut personality = default_personality();
        personality.boldness = 0.2;
        personality.diligence = 0.9;

        let mut rng = seeded_rng(11);

        let scores = score_actions(&ctx(&needs, &personality), &mut rng);
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
        let mut needs = Needs::default();
        needs.social = 0.1; // very lonely
        needs.hunger = 0.9;
        needs.energy = 0.9;
        needs.warmth = 0.9;

        let mut personality = default_personality();
        personality.sociability = 0.9;

        let mut rng = seeded_rng(20);

        let c = ScoringContext {
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
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
        };
        let scores = score_actions(&c, &mut rng);
        let socialize_score = scores.iter().find(|(a, _)| *a == Action::Socialize).unwrap().1;
        let idle_score = scores.iter().find(|(a, _)| *a == Action::Idle).unwrap().1;

        assert!(
            socialize_score > idle_score,
            "lonely social cat should score Socialize ({socialize_score}) above Idle ({idle_score})"
        );
    }

    /// Cold cat should score Groom highly (self-groom for warmth).
    #[test]
    fn cold_cat_scores_groom_high() {
        let mut needs = Needs::default();
        needs.warmth = 0.1;
        needs.hunger = 0.9;
        needs.energy = 0.9;

        let personality = default_personality();
        let mut rng = seeded_rng(21);

        let scores = score_actions(&ctx(&needs, &personality), &mut rng);
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
        let mut scores = vec![
            (Action::Hunt, 1.0),
            (Action::Forage, 1.0),
            (Action::Idle, 0.5),
        ];
        let mut memory = Memory::default();
        memory.remember(make_memory(MemoryType::ResourceFound, Position::new(5, 5), 1.0));

        // Cat at (5, 5) — same tile as remembered resource.
        apply_memory_bonuses(&mut scores, &memory, &Position::new(5, 5));

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
        let mut scores = vec![
            (Action::Wander, 1.0),
            (Action::Hunt, 1.0),
        ];
        let mut memory = Memory::default();
        memory.remember(make_memory(MemoryType::Death, Position::new(5, 5), 1.0));

        apply_memory_bonuses(&mut scores, &memory, &Position::new(5, 5));

        let wander = scores.iter().find(|(a, _)| *a == Action::Wander).unwrap().1;
        let hunt = scores.iter().find(|(a, _)| *a == Action::Hunt).unwrap().1;
        assert!(
            wander < 1.0,
            "Wander should be suppressed near death site; got {wander}"
        );
        assert_eq!(hunt, 1.0, "Hunt should be unaffected by death memory; got {hunt}");
    }

    #[test]
    fn distant_memories_have_less_effect() {
        let mut scores_near = vec![(Action::Hunt, 1.0)];
        let mut scores_far = vec![(Action::Hunt, 1.0)];
        let mut memory = Memory::default();
        memory.remember(make_memory(MemoryType::ResourceFound, Position::new(5, 5), 1.0));

        apply_memory_bonuses(&mut scores_near, &memory, &Position::new(5, 5));
        apply_memory_bonuses(&mut scores_far, &memory, &Position::new(15, 5));

        let near = scores_near[0].1;
        let far = scores_far[0].1;
        assert!(
            near > far,
            "nearby memory should give bigger boost; near={near}, far={far}"
        );
    }

    #[test]
    fn decayed_memories_have_less_effect() {
        let mut scores_strong = vec![(Action::Hunt, 1.0)];
        let mut scores_weak = vec![(Action::Hunt, 1.0)];
        let mut memory_strong = Memory::default();
        memory_strong.remember(make_memory(MemoryType::ResourceFound, Position::new(5, 5), 1.0));
        let mut memory_weak = Memory::default();
        memory_weak.remember(make_memory(MemoryType::ResourceFound, Position::new(5, 5), 0.2));

        apply_memory_bonuses(&mut scores_strong, &memory_strong, &Position::new(5, 5));
        apply_memory_bonuses(&mut scores_weak, &memory_weak, &Position::new(5, 5));

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
        let mut scores = vec![
            (Action::Hunt, 1.0),
            (Action::Idle, 0.5),
        ];
        let nearby = HashMap::from([(Action::Hunt, 3)]);

        apply_cascading_bonuses(&mut scores, &nearby);

        let hunt = scores.iter().find(|(a, _)| *a == Action::Hunt).unwrap().1;
        assert!(
            (hunt - 1.45).abs() < 1e-5,
            "3 nearby hunters should add 0.45; got {hunt}"
        );
    }

    #[test]
    fn cascading_does_not_boost_unrelated_actions() {
        let mut scores = vec![
            (Action::Hunt, 1.0),
            (Action::Sleep, 0.5),
        ];
        let nearby = HashMap::from([(Action::Hunt, 2)]);

        apply_cascading_bonuses(&mut scores, &nearby);

        let sleep = scores.iter().find(|(a, _)| *a == Action::Sleep).unwrap().1;
        assert_eq!(sleep, 0.5, "Sleep should be unaffected; got {sleep}");
    }

    // --- Flee / Fight / Patrol scoring tests ---

    #[test]
    fn cautious_cat_flees_when_threatened() {
        let mut needs = Needs::default();
        needs.safety = 0.2;

        let mut personality = default_personality();
        personality.boldness = 0.2; // cautious

        let mut rng = seeded_rng(30);

        let c = ScoringContext {
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
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
        };
        let scores = score_actions(&c, &mut rng);
        let best = select_best_action(&scores);

        assert_eq!(
            best,
            Action::Flee,
            "cautious cat with low safety should flee; scores: {scores:?}"
        );
    }

    #[test]
    fn bold_cat_fights_when_allies_present() {
        let mut needs = Needs::default();
        needs.safety = 0.3;

        let mut personality = default_personality();
        personality.boldness = 0.9;

        let mut rng = seeded_rng(31);

        let c = ScoringContext {
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
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
        };
        let scores = score_actions(&c, &mut rng);
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
        let needs = Needs::default();
        let personality = default_personality();
        let mut rng = seeded_rng(40);

        let c = ScoringContext {
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
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
        };
        let scores = score_actions(&c, &mut rng);
        let actions: Vec<Action> = scores.iter().map(|(a, _)| *a).collect();

        assert!(actions.contains(&Action::Eat), "incapacitated cat should be able to Eat");
        assert!(actions.contains(&Action::Sleep), "incapacitated cat should be able to Sleep");
        assert!(actions.contains(&Action::Idle), "incapacitated cat should be able to Idle");
        assert!(!actions.contains(&Action::Hunt), "incapacitated cat should not Hunt");
        assert!(!actions.contains(&Action::Fight), "incapacitated cat should not Fight");
        assert!(!actions.contains(&Action::Flee), "incapacitated cat should not Flee");
    }

    #[test]
    fn threat_memory_suppresses_wander_near_threat() {
        let mut scores = vec![
            (Action::Wander, 1.0),
            (Action::Explore, 1.0),
            (Action::Hunt, 1.0),
            (Action::Idle, 0.5),
        ];
        let mut memory = Memory::default();
        memory.remember(make_memory(MemoryType::ThreatSeen, Position::new(5, 5), 1.0));

        apply_memory_bonuses(&mut scores, &memory, &Position::new(5, 5));

        let wander = scores.iter().find(|(a, _)| *a == Action::Wander).unwrap().1;
        let explore = scores.iter().find(|(a, _)| *a == Action::Explore).unwrap().1;
        let hunt = scores.iter().find(|(a, _)| *a == Action::Hunt).unwrap().1;
        let idle = scores.iter().find(|(a, _)| *a == Action::Idle).unwrap().1;

        assert!(wander < 1.0, "wander should be suppressed near threat; got {wander}");
        assert!(explore < 1.0, "explore should be suppressed near threat; got {explore}");
        assert!(hunt < 1.0, "hunt should be suppressed near threat; got {hunt}");
        assert_eq!(idle, 0.5, "idle should be unaffected; got {idle}");
    }

    // --- Herbcraft / PracticeMagic scoring tests ---

    #[test]
    fn spiritual_cat_with_herbs_nearby_scores_herbcraft() {
        let mut needs = Needs::default();
        needs.hunger = 0.8;
        needs.energy = 0.8;

        let mut personality = default_personality();
        personality.spirituality = 0.9;

        let mut rng = seeded_rng(50);

        let mut c = ctx(&needs, &personality);
        c.has_herbs_nearby = true;
        c.herbcraft_skill = 0.3;

        let scores = score_actions(&c, &mut rng);
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
        let needs = Needs::default();
        let personality = default_personality();
        let mut rng = seeded_rng(51);

        let c = ctx(&needs, &personality);
        // no herbs nearby, no inventory herbs

        let scores = score_actions(&c, &mut rng);
        let herbcraft = scores.iter().find(|(a, _)| *a == Action::Herbcraft);
        assert!(
            herbcraft.is_none(),
            "no herbs → no Herbcraft; scores: {scores:?}"
        );
    }

    #[test]
    fn practice_magic_requires_prereqs() {
        let needs = Needs::default();
        let personality = default_personality();
        let mut rng = seeded_rng(52);

        // Below prereqs: affinity 0.2 < 0.3 threshold
        let mut c = ctx(&needs, &personality);
        c.magic_affinity = 0.2;
        c.magic_skill = 0.3;

        let scores = score_actions(&c, &mut rng);
        let magic = scores.iter().find(|(a, _)| *a == Action::PracticeMagic);
        assert!(magic.is_none(), "below affinity threshold → no PracticeMagic");

        // Below prereqs: skill 0.1 < 0.2 threshold
        let mut c2 = ctx(&needs, &personality);
        c2.magic_affinity = 0.5;
        c2.magic_skill = 0.1;

        let scores2 = score_actions(&c2, &mut rng);
        let magic2 = scores2.iter().find(|(a, _)| *a == Action::PracticeMagic);
        assert!(magic2.is_none(), "below skill threshold → no PracticeMagic");
    }

    #[test]
    fn magical_cat_on_corrupted_tile_scores_practice_magic() {
        let mut needs = Needs::default();
        needs.hunger = 0.8;
        needs.energy = 0.8;

        let mut personality = default_personality();
        personality.spirituality = 0.8;

        let mut rng = seeded_rng(53);

        let mut c = ctx(&needs, &personality);
        c.magic_affinity = 0.6;
        c.magic_skill = 0.4;
        c.on_corrupted_tile = true;
        c.tile_corruption = 0.5;

        let scores = score_actions(&c, &mut rng);
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
        let mut needs = Needs::default();
        needs.hunger = 0.8;
        needs.energy = 0.8;

        let mut personality = default_personality();
        personality.compassion = 0.9;

        let mut rng = seeded_rng(54);

        let mut c = ctx(&needs, &personality);
        c.has_remedy_herbs = true;
        c.has_herbs_in_inventory = true;
        c.herbcraft_skill = 0.4;
        c.colony_injury_count = 2;

        let scores = score_actions(&c, &mut rng);
        let herbcraft = scores.iter().find(|(a, _)| *a == Action::Herbcraft);

        assert!(
            herbcraft.is_some() && herbcraft.unwrap().1 > 0.15,
            "compassionate cat with remedy herbs and injured allies should score Herbcraft; got {herbcraft:?}"
        );
    }

    /// Average cat with met needs should pick Wander over Idle.
    #[test]
    fn wander_beats_idle_for_average_cat() {
        let needs = Needs::default();
        let personality = default_personality();
        let mut rng = seeded_rng(42);

        let c = ScoringContext {
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
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
        };
        let scores = score_actions(&c, &mut rng);
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
        let mut needs = Needs::default();
        needs.hunger = 0.9; // not personally hungry
        needs.energy = 0.9;

        let mut personality = default_personality();
        personality.boldness = 0.6;

        let mut rng_full = seeded_rng(50);
        let mut rng_low = seeded_rng(50);

        let base = ScoringContext {
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
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
        };

        let scores_full = score_actions(&base, &mut rng_full);
        let hunt_full = scores_full.iter().find(|(a, _)| *a == Action::Hunt).unwrap().1;

        let low = ScoringContext {
            food_fraction: 0.2,
            ..base
        };
        let scores_low = score_actions(&low, &mut rng_low);
        let hunt_low = scores_low.iter().find(|(a, _)| *a == Action::Hunt).unwrap().1;

        assert!(
            hunt_low > hunt_full,
            "Hunt with low stores ({hunt_low:.3}) should exceed Hunt with full stores ({hunt_full:.3})"
        );
    }

    // --- Survival floor tests ---

    #[test]
    fn survival_floor_caps_build_when_starving() {
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

        enforce_survival_floor(&mut scores, &needs);

        let eat = scores.iter().find(|(a, _)| *a == Action::Eat).unwrap().1;
        let build = scores.iter().find(|(a, _)| *a == Action::Build).unwrap().1;
        assert!(
            eat >= build,
            "starving cat: Eat ({eat:.3}) should >= Build ({build:.3})"
        );
    }

    #[test]
    fn survival_floor_inactive_when_needs_met() {
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

        enforce_survival_floor(&mut scores, &needs);

        let build = scores.iter().find(|(a, _)| *a == Action::Build).unwrap().1;
        assert_eq!(
            build, build_before,
            "well-fed cat: Build should be untouched"
        );
    }

    #[test]
    fn survival_floor_gradual() {
        let mut needs = Needs::default();
        needs.hunger = 0.3; // moderately hungry
        needs.energy = 0.9;
        needs.warmth = 0.9;

        let mut scores = vec![
            (Action::Eat, 1.2),
            (Action::Build, 2.0),
            (Action::Idle, 0.1),
        ];

        enforce_survival_floor(&mut scores, &needs);

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
        let mut needs = Needs::default();
        needs.hunger = 0.05; // nearly starving
        needs.energy = 0.9;
        needs.warmth = 0.9;

        let mut scores = vec![
            (Action::Eat, 1.8),
            (Action::Flee, 3.0),
            (Action::Build, 2.0),
        ];

        enforce_survival_floor(&mut scores, &needs);

        let flee = scores.iter().find(|(a, _)| *a == Action::Flee).unwrap().1;
        assert_eq!(flee, 3.0, "Flee should be exempt from survival floor");
    }
}
