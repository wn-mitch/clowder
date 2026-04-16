use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::Action;
use crate::components::aspirations::{
    ActiveAspiration, AspirationDomain, Aspirations, AspirationsInitialized, Preference,
    Preferences,
};
use crate::components::identity::{Age, LifeStage, Name};
use crate::components::mental::{Memory, MemoryType, MoodModifier};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Needs};
use crate::components::zodiac::ZodiacSign;
use crate::resources::aspiration_registry::AspirationRegistry;
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::{AspirationConstants, SimConstants};
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{SimConfig, TimeState};
use crate::resources::zodiac::ZodiacData;

// ---------------------------------------------------------------------------
// Aspiration selection helpers
// ---------------------------------------------------------------------------

/// Map an aspiration domain to the personality axis that most strongly aligns.
fn domain_personality_axis(domain: AspirationDomain, p: &Personality) -> f32 {
    match domain {
        AspirationDomain::Hunting => p.boldness,
        AspirationDomain::Combat => (p.boldness + p.temper) / 2.0,
        AspirationDomain::Social => p.warmth,
        AspirationDomain::Herbcraft => p.spirituality,
        AspirationDomain::Exploration => p.curiosity,
        AspirationDomain::Building => p.diligence,
        AspirationDomain::Leadership => p.ambition,
    }
}

/// Count memories of a given type.
fn memory_count(memory: &Memory, mem_type: MemoryType) -> usize {
    memory
        .events
        .iter()
        .filter(|e| e.event_type == mem_type)
        .count()
}

/// Score a candidate chain for a cat.
fn score_chain(
    domain: AspirationDomain,
    personality: &Personality,
    memory: &Memory,
    zodiac_domains: &[AspirationDomain],
    c: &AspirationConstants,
    rng: &mut impl Rng,
) -> f32 {
    let mut score = 0.0;

    // Zodiac affinity.
    if zodiac_domains.contains(&domain) {
        score += c.zodiac_affinity_bonus;
    }

    // Personality alignment.
    score += c.personality_alignment_weight * domain_personality_axis(domain, personality);

    // Experience modifier: relevant memories boost the score.
    let experience = match domain {
        AspirationDomain::Hunting => {
            memory_count(memory, MemoryType::ResourceFound) as f32 * c.experience_memory_scale
        }
        AspirationDomain::Combat => {
            memory_count(memory, MemoryType::ThreatSeen) as f32 * c.experience_memory_scale
                + memory_count(memory, MemoryType::Injury) as f32 * c.experience_secondary_scale
        }
        AspirationDomain::Social => {
            memory_count(memory, MemoryType::SocialEvent) as f32 * c.experience_memory_scale
        }
        AspirationDomain::Herbcraft => {
            memory_count(memory, MemoryType::MagicEvent) as f32 * c.experience_memory_scale
        }
        AspirationDomain::Exploration => {
            memory_count(memory, MemoryType::ResourceFound) as f32 * c.experience_secondary_scale
        }
        AspirationDomain::Building => 0.0, // no specific memory type
        AspirationDomain::Leadership => {
            memory_count(memory, MemoryType::SocialEvent) as f32 * c.experience_secondary_scale
        }
    };
    score += experience.min(c.experience_cap); // cap experience contribution

    // Jitter.
    score += rng.random_range(-c.scoring_jitter..c.scoring_jitter);

    score
}

// ---------------------------------------------------------------------------
// Preference generation helpers
// ---------------------------------------------------------------------------

/// Generate likes and dislikes from zodiac sign and personality.
fn generate_preferences(
    sign: ZodiacSign,
    personality: &Personality,
    zodiac_data: &ZodiacData,
) -> Preferences {
    let mut prefs: Vec<(Action, Preference)> = Vec::new();
    let sign_domains = zodiac_data.domain_affinities(sign);

    // Likes: actions in zodiac domain affinities.
    for domain in sign_domains {
        for &action in domain.matching_actions() {
            if !prefs.iter().any(|(a, _)| *a == action) {
                prefs.push((action, Preference::Like));
            }
        }
    }

    // Extra likes from strong personality axes (> 0.7).
    let strong_domains: Vec<(AspirationDomain, f32)> = [
        (AspirationDomain::Hunting, personality.boldness),
        (AspirationDomain::Combat, personality.temper),
        (AspirationDomain::Social, personality.warmth),
        (AspirationDomain::Herbcraft, personality.spirituality),
        (AspirationDomain::Exploration, personality.curiosity),
        (AspirationDomain::Building, personality.diligence),
        (AspirationDomain::Leadership, personality.ambition),
    ]
    .into_iter()
    .filter(|(_, v)| *v > 0.7)
    .collect();

    for (domain, _) in &strong_domains {
        for &action in domain.matching_actions() {
            if !prefs.iter().any(|(a, _)| *a == action) {
                prefs.push((action, Preference::Like));
            }
        }
    }

    // Dislikes: actions in zodiac rival domains.
    let rival_domains: Vec<AspirationDomain> = zodiac_data
        .signs
        .get(&sign)
        .map(|sd| {
            sd.rival
                .iter()
                .flat_map(|rs| zodiac_data.domain_affinities(*rs))
                .copied()
                .collect()
        })
        .unwrap_or_default();

    for domain in &rival_domains {
        for &action in domain.matching_actions() {
            // Don't dislike something already liked.
            if !prefs.iter().any(|(a, _)| *a == action) {
                prefs.push((action, Preference::Dislike));
            }
        }
    }

    // Extra dislikes from weak personality axes (< 0.3).
    let weak_domains: Vec<(AspirationDomain, f32)> = [
        (AspirationDomain::Hunting, personality.boldness),
        (AspirationDomain::Combat, personality.temper),
        (AspirationDomain::Social, personality.warmth),
        (AspirationDomain::Herbcraft, personality.spirituality),
        (AspirationDomain::Exploration, personality.curiosity),
        (AspirationDomain::Building, personality.diligence),
        (AspirationDomain::Leadership, personality.ambition),
    ]
    .into_iter()
    .filter(|(_, v)| *v < 0.3)
    .collect();

    for (domain, _) in &weak_domains {
        for &action in domain.matching_actions() {
            if !prefs.iter().any(|(a, _)| *a == action) {
                prefs.push((action, Preference::Dislike));
            }
        }
    }

    Preferences {
        action_preferences: prefs,
    }
}

// ---------------------------------------------------------------------------
// select_aspirations system
// ---------------------------------------------------------------------------

/// Assigns initial aspirations and preferences to cats reaching Young stage.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn select_aspirations(
    query: Query<
        (Entity, &Name, &Age, &Personality, &Memory, &ZodiacSign),
        (Without<AspirationsInitialized>, Without<Dead>),
    >,
    registry: Option<Res<AspirationRegistry>>,
    zodiac_data: Option<Res<ZodiacData>>,
    constants: Res<SimConstants>,
    time: Res<TimeState>,
    config: Res<SimConfig>,
    mut log: ResMut<NarrativeLog>,
    mut rng: ResMut<SimRng>,
    mut commands: Commands,
    mut activation: ResMut<SystemActivation>,
) {
    let Some(registry) = registry else { return };
    let Some(zodiac_data) = zodiac_data else {
        return;
    };
    let c = &constants.aspirations;

    for (entity, name, age, personality, memory, &sign) in &query {
        let stage = age.stage(time.tick, config.ticks_per_season);
        if stage == LifeStage::Kitten {
            continue;
        }

        let zodiac_domains = zodiac_data.domain_affinities(sign);

        // Score all available chains.
        let mut best: Option<(&str, AspirationDomain, f32)> = None;
        for chain in registry.all_chains() {
            let s = score_chain(
                chain.domain,
                personality,
                memory,
                zodiac_domains,
                c,
                &mut rng.rng,
            );
            if best.as_ref().is_none_or(|(_, _, bs)| s > *bs) {
                best = Some((&chain.name, chain.domain, s));
            }
        }

        if let Some((chain_name, domain, _)) = best {
            activation.record(Feature::AspirationSelected);
            let aspirations = Aspirations {
                active: vec![ActiveAspiration {
                    chain_name: chain_name.to_string(),
                    domain,
                    current_milestone: 0,
                    progress: 0,
                    adopted_tick: time.tick,
                    last_progress_tick: time.tick,
                }],
                completed: Vec::new(),
            };

            let preferences = generate_preferences(sign, personality, &zodiac_data);

            commands
                .entity(entity)
                .insert((aspirations, preferences, AspirationsInitialized));

            log.push(
                time.tick,
                format!(
                    "Something settles in {}'s heart -- a quiet certainty. The path of {:?} calls.",
                    name.0, domain,
                ),
                NarrativeTier::Action,
            );
        } else {
            // No chains available — still mark as initialized.
            commands.entity(entity).insert(AspirationsInitialized);
        }
    }
}

// ---------------------------------------------------------------------------
// check_second_aspiration_slot system
// ---------------------------------------------------------------------------

/// Grants a second aspiration slot to Adult cats.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn check_second_aspiration_slot(
    constants: Res<SimConstants>,
    mut query: Query<
        (
            Entity,
            &Name,
            &Age,
            &Personality,
            &Memory,
            &ZodiacSign,
            &mut Aspirations,
        ),
        Without<Dead>,
    >,
    registry: Option<Res<AspirationRegistry>>,
    zodiac_data: Option<Res<ZodiacData>>,
    time: Res<TimeState>,
    config: Res<SimConfig>,
    mut log: ResMut<NarrativeLog>,
    mut rng: ResMut<SimRng>,
) {
    let Some(registry) = registry else { return };
    let Some(zodiac_data) = zodiac_data else {
        return;
    };
    let c = &constants.aspirations;

    // Rate-limit: only check every 100 ticks.
    if !time.tick.is_multiple_of(c.second_slot_check_interval) {
        return;
    }

    for (_entity, name, age, personality, memory, &sign, mut aspirations) in &mut query {
        let stage = age.stage(time.tick, config.ticks_per_season);
        if stage != LifeStage::Adult && stage != LifeStage::Elder {
            continue;
        }
        if aspirations.active.len() >= 2 {
            continue;
        }

        let zodiac_domains = zodiac_data.domain_affinities(sign);
        let existing_domains: Vec<AspirationDomain> =
            aspirations.active.iter().map(|a| a.domain).collect();

        // Score chains, excluding active domains and already-completed chains.
        let mut best: Option<(&str, AspirationDomain, f32)> = None;
        for chain in registry.all_chains() {
            if existing_domains.contains(&chain.domain) {
                continue;
            }
            if aspirations.completed.contains(&chain.name) {
                continue;
            }
            let s = score_chain(
                chain.domain,
                personality,
                memory,
                zodiac_domains,
                c,
                &mut rng.rng,
            );
            if best.as_ref().is_none_or(|(_, _, bs)| s > *bs) {
                best = Some((&chain.name, chain.domain, s));
            }
        }

        if let Some((chain_name, domain, _)) = best {
            aspirations.active.push(ActiveAspiration {
                chain_name: chain_name.to_string(),
                domain,
                current_milestone: 0,
                progress: 0,
                adopted_tick: time.tick,
                last_progress_tick: time.tick,
            });

            log.push(
                time.tick,
                format!(
                    "A new fire kindles in {}. The path of {:?} beckons.",
                    name.0, domain,
                ),
                NarrativeTier::Action,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// check_aspiration_abandonment system
// ---------------------------------------------------------------------------

/// Abandons aspirations when a cat has made no progress for 2000 ticks and
/// their personality alignment for that domain has drifted below 0.3.
pub fn check_aspiration_abandonment(
    mut query: Query<(&Name, &Personality, &mut Aspirations), Without<Dead>>,
    time: Res<TimeState>,
    mut log: ResMut<NarrativeLog>,
    mut activation: ResMut<SystemActivation>,
) {
    const STAGNATION_TICKS: u64 = 2000;
    const MIN_ALIGNMENT: f32 = 0.3;

    for (name, personality, mut aspirations) in &mut query {
        let mut to_remove = Vec::new();
        for (i, asp) in aspirations.active.iter().enumerate() {
            let stagnant = time.tick.saturating_sub(asp.last_progress_tick) >= STAGNATION_TICKS;
            let low_alignment = domain_personality_axis(asp.domain, personality) < MIN_ALIGNMENT;
            if stagnant && low_alignment {
                to_remove.push(i);
                activation.record(Feature::AspirationAbandoned);
                log.push(
                    time.tick,
                    format!(
                        "The dream fades. {} no longer sees the path in {:?}.",
                        name.0, asp.domain,
                    ),
                    NarrativeTier::Action,
                );
            }
        }

        // Remove in reverse order to preserve indices.
        for i in to_remove.into_iter().rev() {
            aspirations.active.remove(i);
        }
    }
}

// ---------------------------------------------------------------------------
// track_milestones system
// ---------------------------------------------------------------------------

/// Checks active aspirations for milestone completion.
///
/// Runs every tick. For `ActionCount` conditions, increments progress when the
/// cat's current action matches and is on its last tick (`ticks_remaining == 1`).
/// Other conditions are checked directly against cat state.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn track_milestones(
    mut query: Query<
        (
            &Name,
            &crate::ai::CurrentAction,
            &crate::components::skills::Skills,
            &Memory,
            &mut Aspirations,
            &mut crate::components::mental::Mood,
            &mut Needs,
        ),
        Without<Dead>,
    >,
    registry: Option<Res<AspirationRegistry>>,
    relationships: Res<crate::resources::relationships::Relationships>,
    time: Res<TimeState>,
    mut log: ResMut<NarrativeLog>,
    mut colony_score: Option<ResMut<crate::resources::colony_score::ColonyScore>>,
    mut activation: ResMut<SystemActivation>,
) {
    let Some(registry) = registry else { return };

    for (name, current, skills, memory, mut aspirations, mut mood, mut needs) in &mut query {
        let mut completions: Vec<usize> = Vec::new(); // indices of fully completed chains

        for (i, asp) in aspirations.active.iter_mut().enumerate() {
            let Some(chain) = registry.chain_by_name(&asp.chain_name) else {
                continue;
            };
            if asp.current_milestone >= chain.milestones.len() {
                // Already completed all milestones — will be moved to completed.
                completions.push(i);
                continue;
            }
            let milestone = &chain.milestones[asp.current_milestone];

            let met = match &milestone.condition {
                crate::components::aspirations::MilestoneCondition::ActionCount {
                    action,
                    count,
                } => {
                    // Increment progress when the matching action completes.
                    if current.ticks_remaining == 1 {
                        let action_name = format!("{:?}", current.action);
                        if action_name == *action {
                            asp.progress += 1;
                            asp.last_progress_tick = time.tick;
                        }
                    }
                    asp.progress >= *count
                }
                crate::components::aspirations::MilestoneCondition::SkillLevel { skill, level } => {
                    let current_level = match skill.as_str() {
                        "hunting" => skills.hunting,
                        "foraging" => skills.foraging,
                        "herbcraft" => skills.herbcraft,
                        "building" => skills.building,
                        "combat" => skills.combat,
                        "magic" => skills.magic,
                        _ => 0.0,
                    };
                    if current_level >= *level {
                        asp.last_progress_tick = time.tick;
                    }
                    current_level >= *level
                }
                crate::components::aspirations::MilestoneCondition::FormBond { bond_type: _ } => {
                    // Check if the cat has any bond (simplified — just check relationships resource).
                    // A proper implementation would filter by bond_type string.
                    false // Will be refined when bond checking is available per-entity.
                }
                crate::components::aspirations::MilestoneCondition::WitnessEvent {
                    event_type,
                    count,
                } => {
                    let mem_type = match event_type.as_str() {
                        "ThreatSeen" => Some(MemoryType::ThreatSeen),
                        "Death" => Some(MemoryType::Death),
                        "ResourceFound" => Some(MemoryType::ResourceFound),
                        "MagicEvent" => Some(MemoryType::MagicEvent),
                        "Injury" => Some(MemoryType::Injury),
                        "SocialEvent" => Some(MemoryType::SocialEvent),
                        _ => None,
                    };
                    if let Some(mt) = mem_type {
                        let witnessed = memory
                            .events
                            .iter()
                            .filter(|e| e.event_type == mt && e.tick >= asp.adopted_tick)
                            .count();
                        if witnessed > 0 {
                            asp.last_progress_tick = time.tick;
                        }
                        witnessed as u32 >= *count
                    } else {
                        false
                    }
                }
                crate::components::aspirations::MilestoneCondition::Mentor { count } => {
                    // Mentor actions tracked same as ActionCount.
                    if current.ticks_remaining == 1 && current.action == Action::Mentor {
                        asp.progress += 1;
                        asp.last_progress_tick = time.tick;
                    }
                    asp.progress >= *count
                }
            };

            if met {
                // Milestone completed!
                log.push(
                    time.tick,
                    milestone
                        .narrative_on_complete
                        .replace("{name}", &name.0)
                        .replace("{possessive}", "their") // simplified
                        .replace("{subject}", "they")
                        .replace("{object}", "them"),
                    NarrativeTier::Action,
                );

                // Mood boost.
                mood.modifiers.push_back(MoodModifier {
                    amount: 0.2,
                    ticks_remaining: 100,
                    source: format!("achieved {}", milestone.name),
                });

                // Need restoration.
                needs.mastery = (needs.mastery + 0.05).min(1.0);
                needs.purpose = (needs.purpose + 0.03).min(1.0);

                // Advance to next milestone and reset progress.
                asp.current_milestone += 1;
                asp.progress = 0;

                // Check if chain is now fully complete.
                if asp.current_milestone >= chain.milestones.len() {
                    completions.push(i);
                }
            }
        }

        // Handle completed chains (in reverse to preserve indices).
        let mut seen = std::collections::HashSet::new();
        for &i in completions.iter().rev() {
            if !seen.insert(i) {
                continue;
            }
            let asp = aspirations.active.remove(i);

            if let Some(chain) = registry.chain_by_name(&asp.chain_name) {
                log.push(
                    time.tick,
                    chain
                        .completion_narrative
                        .replace("{name}", &name.0)
                        .replace("{possessive}", "their")
                        .replace("{subject}", "they")
                        .replace("{Subject}", "They")
                        .replace("{object}", "them"),
                    NarrativeTier::Significant,
                );
            }

            mood.modifiers.push_back(MoodModifier {
                amount: 0.4,
                ticks_remaining: 200,
                source: format!("fulfilled aspiration: {}", asp.chain_name),
            });
            needs.purpose = (needs.purpose + 0.1).min(1.0);

            aspirations.completed.push(asp.chain_name);
            activation.record(Feature::AspirationCompleted);

            if let Some(ref mut score) = colony_score {
                score.aspirations_completed += 1;
            }
        }
    }

    // Suppress unused warning — relationships will be used for FormBond checks.
    let _ = &relationships;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::aspirations::AspirationDomain;
    use rand::SeedableRng;

    #[test]
    fn score_chain_zodiac_affinity_boosts() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let personality = Personality::random(&mut rng);
        let memory = Memory::default();
        let ac = crate::resources::sim_constants::AspirationConstants::default();

        let with_affinity = score_chain(
            AspirationDomain::Hunting,
            &personality,
            &memory,
            &[AspirationDomain::Hunting, AspirationDomain::Combat],
            &ac,
            &mut rng,
        );

        let mut rng2 = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let personality2 = Personality::random(&mut rng2);
        let without_affinity = score_chain(
            AspirationDomain::Hunting,
            &personality2,
            &memory,
            &[AspirationDomain::Social], // not hunting
            &ac,
            &mut rng2,
        );

        // With zodiac affinity should score ~0.4 higher.
        assert!(
            with_affinity > without_affinity,
            "zodiac affinity should boost score"
        );
    }

    #[test]
    fn preferences_include_likes_for_zodiac_domains() {
        let zodiac_data = ZodiacData::load(std::path::Path::new("assets/data/zodiac.ron")).unwrap();
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);
        let personality = Personality::random(&mut rng);

        let prefs = generate_preferences(ZodiacSign::LeapingFlame, &personality, &zodiac_data);

        // LeapingFlame has Hunting and Combat affinities → should like Hunt and Fight.
        assert!(prefs
            .get(Action::Hunt)
            .is_some_and(|p| p == Preference::Like));
    }
}
