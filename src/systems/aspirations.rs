use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::Action;
use crate::components::aspirations::{
    ActiveAspiration, AspirationDomain, Aspirations, AspirationsInitialized, Preference,
    Preferences,
};
use crate::components::identity::{Age, LifeStage, Name, Species};
use crate::components::markers;
use crate::components::mental::{Memory, MemoryType, MoodModifier, MoodSource};
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Needs, Position};
use crate::components::skills::{Skills, Training};
use crate::components::zodiac::ZodiacSign;
use crate::resources::aspiration_registry::AspirationRegistry;
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::{AspirationConstants, SimConstants};
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{SimConfig, TimeScale, TimeState};
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
    time_scale: Res<TimeScale>,
    config: Res<SimConfig>,
    mut log: ResMut<NarrativeLog>,
    mut rng: ResMut<SimRng>,
) {
    let Some(registry) = registry else { return };
    let Some(zodiac_data) = zodiac_data else {
        return;
    };
    let c = &constants.aspirations;

    // Rate-limit: once per in-game day.
    if !c
        .second_slot_check_interval
        .fires_at(time.tick, &time_scale)
    {
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
            Entity,
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

    for (cat_entity, name, current, skills, memory, mut aspirations, mut mood, mut needs) in
        &mut query
    {
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
                crate::components::aspirations::MilestoneCondition::FormBond { bond_type } => {
                    use crate::resources::relationships::BondType;
                    let target_bond = match bond_type.as_str() {
                        "Friends" => Some(BondType::Friends),
                        "Partners" => Some(BondType::Partners),
                        "Mates" => Some(BondType::Mates),
                        _ => None,
                    };
                    let has_bond = target_bond.is_some_and(|target| {
                        relationships
                            .all_for(cat_entity)
                            .iter()
                            .any(|(_, rel)| rel.bond.is_some_and(|b| b >= target))
                    });
                    if has_bond {
                        asp.last_progress_tick = time.tick;
                    }
                    has_bond
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
                mood.modifiers.push_back(
                    MoodModifier::new(0.2, 100, format!("achieved {}", milestone.name))
                        .with_kind(MoodSource::Pride),
                );

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

            mood.modifiers.push_back(
                MoodModifier::new(0.4, 200, format!("fulfilled aspiration: {}", asp.chain_name))
                    .with_kind(MoodSource::Pride),
            );
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
// §4 marker authoring — Mentoring batch
// ---------------------------------------------------------------------------

/// Insert/remove a ZST marker only when state actually changes,
/// avoiding unnecessary archetype moves. Mirrors the `toggle` helper
/// in `capabilities.rs`.
fn toggle<M: Component + Copy>(
    commands: &mut Commands,
    entity: Entity,
    want: bool,
    has: bool,
    marker: M,
) {
    match (want, has) {
        (true, false) => {
            commands.entity(entity).insert(marker);
        }
        (false, true) => {
            commands.entity(entity).remove::<M>();
        }
        _ => {}
    }
}

/// Author the `Mentor` and `Apprentice` ZSTs from each cat's `Training`
/// component. A cat is `Mentor` iff `training.apprentice.is_some()`;
/// `Apprentice` iff `training.mentor.is_some()`. A cat may be both
/// simultaneously (mentoring one apprentice while still studying under
/// a senior cat).
///
/// **Predicate** — bit-for-bit mirror of `Training.apprentice` /
/// `Training.mentor` reads. Cats without a `Training` component are
/// treated as having neither role; the second query handles cleanup
/// when a cat loses or never had `Training`.
///
/// **Ordering** — Chain 2a, sibling of `update_directive_markers`. The
/// `Training` component is mutated by `relationships`/skill-progression
/// systems in Chain 2b (after marker authoring), so the marker reflects
/// the prior tick's state for the same-tick scoring read. This matches
/// the `IsCoordinatorWithDirectives` pattern.
#[allow(clippy::type_complexity)]
pub fn update_training_markers(
    mut commands: Commands,
    with_training: Query<
        (
            Entity,
            &Training,
            Has<markers::Mentor>,
            Has<markers::Apprentice>,
        ),
        Without<Dead>,
    >,
    without_training: Query<
        (Entity, Has<markers::Mentor>, Has<markers::Apprentice>),
        (Without<Training>, Without<Dead>),
    >,
) {
    for (entity, training, has_mentor, has_apprentice) in with_training.iter() {
        toggle(
            &mut commands,
            entity,
            training.apprentice.is_some(),
            has_mentor,
            markers::Mentor,
        );
        toggle(
            &mut commands,
            entity,
            training.mentor.is_some(),
            has_apprentice,
            markers::Apprentice,
        );
    }
    // Clean up stale markers on cats that lost their Training component
    // (or never had one). Without<Training> guards entry, but a cat that
    // had a marker before `Training` was removed needs explicit cleanup.
    for (entity, has_mentor, has_apprentice) in without_training.iter() {
        if has_mentor {
            commands.entity(entity).remove::<markers::Mentor>();
        }
        if has_apprentice {
            commands.entity(entity).remove::<markers::Apprentice>();
        }
    }
}

/// Author the `HasMentoringTarget` ZST per the §4.3 per-cat predicate:
/// the cat has at least one skill above `mentor_skill_threshold_high`
/// (default 0.6), AND can sense another living cat within
/// `mentoring_detection_range` whose corresponding skill is below
/// `mentor_skill_threshold_low` (default 0.3) on the same axis.
///
/// **Predicate** — bit-for-bit mirror of the inline `has_mentoring_target_fn`
/// closures previously living in `disposition.rs::evaluate_dispositions`
/// and `goap.rs::evaluate_and_plan`. The mirror retires the
/// silent-divergence between those two scoring loops by routing both
/// through this single author.
///
/// **Ordering** — Chain 2a, after life-stage / injury / inventory so
/// any future combination (e.g. mentoring requires Adult) reads
/// freshly-authored upstream markers. Currently no upstream marker
/// gates apply; the predicate is a pure function of `Position` +
/// `Skills` + sensory range.
#[allow(clippy::type_complexity)]
pub fn update_mentoring_target_markers(
    mut commands: Commands,
    cats: Query<
        (
            Entity,
            &Position,
            &Skills,
            Has<markers::HasMentoringTarget>,
        ),
        (With<Species>, Without<Dead>),
    >,
    constants: Res<SimConstants>,
) {
    let d = &constants.disposition;
    let cat_profile = &constants.sensory.cat;
    let detection_range = d.mentoring_detection_range as f32;
    let high = d.mentor_skill_threshold_high;
    let low = d.mentor_skill_threshold_low;

    let snapshot: Vec<(Entity, Position, [f32; 6])> = cats
        .iter()
        .map(|(e, p, s, _)| {
            (
                e,
                *p,
                [
                    s.hunting,
                    s.foraging,
                    s.herbcraft,
                    s.building,
                    s.combat,
                    s.magic,
                ],
            )
        })
        .collect();

    for (entity, pos, skills, has_marker) in cats.iter() {
        let mentor_arr = [
            skills.hunting,
            skills.foraging,
            skills.herbcraft,
            skills.building,
            skills.combat,
            skills.magic,
        ];
        let qualifies_as_mentor = mentor_arr.iter().any(|&s| s > high);
        let want = qualifies_as_mentor
            && snapshot.iter().any(|(other, other_pos, other_arr)| {
                *other != entity
                    && crate::systems::sensing::observer_sees_at(
                        crate::components::SensorySpecies::Cat,
                        *pos,
                        cat_profile,
                        *other_pos,
                        crate::components::SensorySignature::CAT,
                        detection_range,
                    )
                    && mentor_arr
                        .iter()
                        .zip(other_arr.iter())
                        .any(|(&m, &a)| m > high && a < low)
            });
        toggle(
            &mut commands,
            entity,
            want,
            has_marker,
            markers::HasMentoringTarget,
        );
    }
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

    // -----------------------------------------------------------------------
    // §4 Mentoring batch — author tests
    // -----------------------------------------------------------------------

    use crate::components::physical::{DeathCause, Position};
    use crate::components::skills::{Skills, Training};
    use bevy_ecs::schedule::Schedule;

    fn setup_training() -> (World, Schedule) {
        let world = World::new();
        let mut schedule = Schedule::default();
        schedule.add_systems(update_training_markers);
        (world, schedule)
    }

    fn spawn_cat_with_training(world: &mut World, training: Training) -> Entity {
        world
            .spawn((Species, Position::new(0, 0), Skills::default(), training))
            .id()
    }

    fn spawn_cat_no_training(world: &mut World) -> Entity {
        world
            .spawn((Species, Position::new(0, 0), Skills::default()))
            .id()
    }

    #[test]
    fn cat_without_training_has_neither_mentor_nor_apprentice() {
        let (mut world, mut schedule) = setup_training();
        let cat = spawn_cat_no_training(&mut world);
        schedule.run(&mut world);
        assert!(!world.entity(cat).contains::<markers::Mentor>());
        assert!(!world.entity(cat).contains::<markers::Apprentice>());
    }

    #[test]
    fn cat_with_apprentice_gets_mentor_marker() {
        let (mut world, mut schedule) = setup_training();
        let apprentice = spawn_cat_no_training(&mut world);
        let cat = spawn_cat_with_training(
            &mut world,
            Training {
                apprentice: Some(apprentice),
                mentor: None,
            },
        );
        schedule.run(&mut world);
        assert!(world.entity(cat).contains::<markers::Mentor>());
        assert!(!world.entity(cat).contains::<markers::Apprentice>());
    }

    #[test]
    fn cat_with_mentor_gets_apprentice_marker() {
        let (mut world, mut schedule) = setup_training();
        let mentor = spawn_cat_no_training(&mut world);
        let cat = spawn_cat_with_training(
            &mut world,
            Training {
                apprentice: None,
                mentor: Some(mentor),
            },
        );
        schedule.run(&mut world);
        assert!(world.entity(cat).contains::<markers::Apprentice>());
        assert!(!world.entity(cat).contains::<markers::Mentor>());
    }

    #[test]
    fn cat_with_both_roles_gets_both_markers() {
        let (mut world, mut schedule) = setup_training();
        let other_a = spawn_cat_no_training(&mut world);
        let other_b = spawn_cat_no_training(&mut world);
        let cat = spawn_cat_with_training(
            &mut world,
            Training {
                apprentice: Some(other_a),
                mentor: Some(other_b),
            },
        );
        schedule.run(&mut world);
        assert!(world.entity(cat).contains::<markers::Mentor>());
        assert!(world.entity(cat).contains::<markers::Apprentice>());
    }

    #[test]
    fn losing_apprentice_removes_mentor_marker() {
        let (mut world, mut schedule) = setup_training();
        let apprentice = spawn_cat_no_training(&mut world);
        let cat = spawn_cat_with_training(
            &mut world,
            Training {
                apprentice: Some(apprentice),
                mentor: None,
            },
        );
        schedule.run(&mut world);
        assert!(world.entity(cat).contains::<markers::Mentor>());
        // Clear apprentice slot.
        world.entity_mut(cat).get_mut::<Training>().unwrap().apprentice = None;
        schedule.run(&mut world);
        assert!(!world.entity(cat).contains::<markers::Mentor>());
    }

    #[test]
    fn removing_training_component_cleans_up_markers() {
        let (mut world, mut schedule) = setup_training();
        let apprentice = spawn_cat_no_training(&mut world);
        let cat = spawn_cat_with_training(
            &mut world,
            Training {
                apprentice: Some(apprentice),
                mentor: None,
            },
        );
        schedule.run(&mut world);
        assert!(world.entity(cat).contains::<markers::Mentor>());
        // Drop the Training component entirely.
        world.entity_mut(cat).remove::<Training>();
        schedule.run(&mut world);
        assert!(!world.entity(cat).contains::<markers::Mentor>());
    }

    #[test]
    fn dead_cat_excluded_from_authoring() {
        let (mut world, mut schedule) = setup_training();
        let apprentice = spawn_cat_no_training(&mut world);
        let cat = world
            .spawn((
                Species,
                Position::new(0, 0),
                Skills::default(),
                Training {
                    apprentice: Some(apprentice),
                    mentor: None,
                },
                Dead {
                    tick: 0,
                    cause: DeathCause::Starvation,
                },
            ))
            .id();
        schedule.run(&mut world);
        assert!(!world.entity(cat).contains::<markers::Mentor>());
    }

    #[test]
    fn training_markers_idempotent() {
        let (mut world, mut schedule) = setup_training();
        let apprentice = spawn_cat_no_training(&mut world);
        let cat = spawn_cat_with_training(
            &mut world,
            Training {
                apprentice: Some(apprentice),
                mentor: None,
            },
        );
        schedule.run(&mut world);
        assert!(world.entity(cat).contains::<markers::Mentor>());
        // Second run with same state: no panic, marker still present.
        schedule.run(&mut world);
        assert!(world.entity(cat).contains::<markers::Mentor>());
    }

    // -----------------------------------------------------------------------
    // update_mentoring_target_markers
    // -----------------------------------------------------------------------

    fn setup_mentoring_target() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(SimConstants::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(update_mentoring_target_markers);
        (world, schedule)
    }

    fn spawn_cat_with_skills(world: &mut World, x: i32, y: i32, skills: Skills) -> Entity {
        world.spawn((Species, Position::new(x, y), skills)).id()
    }

    fn high_hunting_skills() -> Skills {
        Skills {
            hunting: 0.7, // > 0.6 high threshold
            ..Skills::default()
        }
    }

    fn low_hunting_skills() -> Skills {
        Skills {
            hunting: 0.1, // < 0.3 low threshold
            ..Skills::default()
        }
    }

    #[test]
    fn solo_cat_no_mentoring_target() {
        let (mut world, mut schedule) = setup_mentoring_target();
        let cat = spawn_cat_with_skills(&mut world, 0, 0, high_hunting_skills());
        schedule.run(&mut world);
        assert!(!world.entity(cat).contains::<markers::HasMentoringTarget>());
    }

    #[test]
    fn high_skill_with_low_skill_peer_in_range_gets_marker() {
        let (mut world, mut schedule) = setup_mentoring_target();
        let mentor = spawn_cat_with_skills(&mut world, 0, 0, high_hunting_skills());
        let _peer = spawn_cat_with_skills(&mut world, 3, 0, low_hunting_skills());
        schedule.run(&mut world);
        assert!(world.entity(mentor).contains::<markers::HasMentoringTarget>());
    }

    #[test]
    fn no_high_skill_no_marker() {
        let (mut world, mut schedule) = setup_mentoring_target();
        let cat = spawn_cat_with_skills(&mut world, 0, 0, Skills::default());
        let _peer = spawn_cat_with_skills(&mut world, 3, 0, low_hunting_skills());
        schedule.run(&mut world);
        assert!(!world.entity(cat).contains::<markers::HasMentoringTarget>());
    }

    #[test]
    fn peer_too_far_no_marker() {
        let (mut world, mut schedule) = setup_mentoring_target();
        let mentor = spawn_cat_with_skills(&mut world, 0, 0, high_hunting_skills());
        // Beyond mentoring_detection_range=10 + cat sight max — well outside.
        let _peer = spawn_cat_with_skills(&mut world, 50, 0, low_hunting_skills());
        schedule.run(&mut world);
        assert!(!world.entity(mentor).contains::<markers::HasMentoringTarget>());
    }

    #[test]
    fn peer_with_high_skill_no_marker() {
        let (mut world, mut schedule) = setup_mentoring_target();
        let mentor = spawn_cat_with_skills(&mut world, 0, 0, high_hunting_skills());
        // Peer also has high hunting — no skill gap on any axis.
        let _peer = spawn_cat_with_skills(&mut world, 3, 0, high_hunting_skills());
        schedule.run(&mut world);
        assert!(!world.entity(mentor).contains::<markers::HasMentoringTarget>());
    }

    #[test]
    fn dead_peer_excluded() {
        let (mut world, mut schedule) = setup_mentoring_target();
        let mentor = spawn_cat_with_skills(&mut world, 0, 0, high_hunting_skills());
        // Spawn a dead cat with the right skill profile.
        world.spawn((
            Species,
            Position::new(3, 0),
            low_hunting_skills(),
            Dead {
                tick: 0,
                cause: DeathCause::Starvation,
            },
        ));
        schedule.run(&mut world);
        // Dead cats are filtered out (Without<Dead>), so the only living peer
        // is the mentor itself — no qualifying gap.
        assert!(!world.entity(mentor).contains::<markers::HasMentoringTarget>());
    }

    #[test]
    fn skill_gap_disappears_when_peer_levels_up() {
        let (mut world, mut schedule) = setup_mentoring_target();
        let mentor = spawn_cat_with_skills(&mut world, 0, 0, high_hunting_skills());
        let peer = spawn_cat_with_skills(&mut world, 3, 0, low_hunting_skills());
        schedule.run(&mut world);
        assert!(world.entity(mentor).contains::<markers::HasMentoringTarget>());
        // Peer learns. Now they're both above 0.3 — no gap > threshold.
        world.entity_mut(peer).get_mut::<Skills>().unwrap().hunting = 0.5;
        schedule.run(&mut world);
        assert!(!world.entity(mentor).contains::<markers::HasMentoringTarget>());
    }

    #[test]
    fn mentoring_target_idempotent() {
        let (mut world, mut schedule) = setup_mentoring_target();
        let mentor = spawn_cat_with_skills(&mut world, 0, 0, high_hunting_skills());
        let _peer = spawn_cat_with_skills(&mut world, 3, 0, low_hunting_skills());
        schedule.run(&mut world);
        assert!(world.entity(mentor).contains::<markers::HasMentoringTarget>());
        schedule.run(&mut world);
        assert!(world.entity(mentor).contains::<markers::HasMentoringTarget>());
    }

    #[test]
    fn cross_axis_gap_qualifies() {
        let (mut world, mut schedule) = setup_mentoring_target();
        // Mentor specializes in herbcraft.
        let mentor = spawn_cat_with_skills(
            &mut world,
            0,
            0,
            Skills {
                herbcraft: 0.7,
                ..Skills::default()
            },
        );
        // Peer is a herbcraft-novice (default herbcraft is 0.05 < 0.3).
        let _peer = spawn_cat_with_skills(&mut world, 3, 0, Skills::default());
        schedule.run(&mut world);
        assert!(world.entity(mentor).contains::<markers::HasMentoringTarget>());
    }
}
