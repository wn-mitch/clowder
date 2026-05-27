use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::aspirations::can_adopt;
use crate::ai::Action;
use crate::components::aspirations::{
    ActiveAspiration, AspirationDomain, Aspirations, AspirationsInitialized, Preference,
    Preferences,
};
use crate::components::identity::{Age, LifeStage, Name, Species};
use crate::components::markers;
use crate::components::mental::{Memory, MemoryType, Mood, MoodModifier, MoodSource};
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
        // Ticket 398 — Kinship aligns with compassion (the values-axis
        // most tightly coupled to the spec's "compassionate → Caretake-
        // biased" cleavage in §7.M.2).
        AspirationDomain::Kinship => p.compassion,
        // 366 — Phase 5 mastery arcs. Practical crafts (fiber / bone /
        // hide) align with `diligence` — patient repetition. Pigment
        // and Cairn straddle into ritual/ceremony (per crafting.md
        // §Phase 5 — shrine-cairns cross-register with monuments.md;
        // pigment-deepened textiles read as colony-age) so they align
        // with `spirituality`.
        AspirationDomain::Weaving => p.diligence,
        AspirationDomain::BoneShaping => p.diligence,
        AspirationDomain::Hidework => p.diligence,
        AspirationDomain::Pigment => p.spirituality,
        AspirationDomain::Cairn => p.spirituality,
        // 463 — CraftItemAspiration. First-light personality alignment
        // is 0.0 — Crafting is not yet a passive-scoring adoption
        // candidate. The arc lives in `ALL_CHAINS` so the picker
        // (commit 6) can score it on the per-tick *emit* loop (where
        // the threat-cue + skill-affinity + anti-monotony axes drive
        // recipe selection), but it doesn't compete with Hunting /
        // Combat / etc. for top-of-list adoption via personality bias.
        // Memory `feedback_dormant_substrate_activation_soak_first` —
        // verify the L2 emit path fires before tuning the adoption
        // surface. Commit 7+ may re-introduce `p.diligence` if the
        // post-first-light verdict shows Crafting under-firing for
        // diligent cats.
        AspirationDomain::Crafting => 0.0,
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
        // Ticket 398 — Kinship has no specific memory-type proxy yet.
        // Adoption is personality-driven (compassion axis) at this
        // phase; the §L2.10.6 land + post-partum trigger (398 follow-on)
        // will add a `BecomesParent`/`KittenBorn` event-driven adoption
        // path separate from this passive-scoring loop.
        AspirationDomain::Kinship => 0.0,
        // 366 — Phase 5 mastery arcs. No memory-type proxy today;
        // adoption is personality-driven (diligence / spirituality
        // axis). 372 may surface a CraftEvent memory type once the
        // discipline actions land.
        AspirationDomain::Weaving
        | AspirationDomain::BoneShaping
        | AspirationDomain::Hidework
        | AspirationDomain::Pigment
        | AspirationDomain::Cairn => 0.0,
        // 463 — CraftItemAspiration. Adoption pressure is the cat's
        // accumulated craft history (a cat who has crafted is more
        // likely to want to craft again — the practiced hand pattern);
        // 463 follow-on may add a CraftedItem MemoryType if Memory
        // currently lacks one. Tertiary-priority emission means the
        // arc's adoption can be passive (no specific memory-driven
        // bump at first-light).
        AspirationDomain::Crafting => 0.0,
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
            // Ticket 398 — Kinship aspirations are spec-event-driven
            // (post-partum / first-litter triggers per §7.M.2), not
            // passive-scored. Skip them in the passive adoption picker.
            // Skipping BEFORE the `rng.random_range` call inside
            // `score_chain` preserves seed-42 determinism: adding the
            // chain to `ALL_CHAINS` must not consume RNG state in this
            // path (per `learning_bevy_schedule_edge_perturbation`).
            // Event-driven adoption wiring lands as 398 Phase 1c+
            // follow-on once `BecomesParent` plumbing is in place.
            //
            // 366 — Phase 5 mastery arcs are dormant in 366: the
            // substrate exists but adoption is deferred to 372
            // alongside the craft-action substrate (event-driven
            // adoption on first relevant craft, analogous to Kinship's
            // post-partum trigger). Skipping here preserves seed-42
            // determinism, identical pattern to Kinship.
            if matches!(
                chain.domain,
                AspirationDomain::Kinship
                    | AspirationDomain::Weaving
                    | AspirationDomain::BoneShaping
                    | AspirationDomain::Hidework
                    | AspirationDomain::Pigment
                    | AspirationDomain::Cairn
                    // 463 — CraftItemAspiration is a daily-driver
                    // L2-emission chain, not a passive-adoption
                    // candidate. The picker (commit 6) scores it on
                    // the per-tick emit loop; skip before rng-
                    // consumption to preserve seed determinism (memory
                    // `learning_bevy_schedule_edge_perturbation`).
                    | AspirationDomain::Crafting
            ) {
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
                best = Some((chain.name, chain.domain, s));
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
                    misaligned_since_tick: None,
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

        // Score chains, excluding active domains, already-completed
        // chains, and chains that hard-conflict with an existing
        // aspiration (§7.7.1 adoption gate).
        let mut best: Option<(&str, AspirationDomain, f32)> = None;
        for chain in registry.all_chains() {
            if existing_domains.contains(&chain.domain) {
                continue;
            }
            if aspirations.completed.iter().any(|c| c == chain.name) {
                continue;
            }
            if can_adopt(&aspirations.active, chain, &registry).is_some() {
                continue;
            }
            // Ticket 398 — Kinship is event-driven (see select_aspirations
            // sibling site). Skip before rng-consumption to preserve
            // seed-42 determinism.
            //
            // 366 — Mastery arcs likewise. Adoption substrate lands
            // with 372 alongside craft actions; until then they remain
            // dormant in the registry. Same Kinship rationale.
            if matches!(
                chain.domain,
                AspirationDomain::Kinship
                    | AspirationDomain::Weaving
                    | AspirationDomain::BoneShaping
                    | AspirationDomain::Hidework
                    | AspirationDomain::Pigment
                    | AspirationDomain::Cairn
                    // 463 — CraftItemAspiration is a daily-driver
                    // L2-emission chain, not a passive-adoption
                    // candidate. The picker (commit 6) scores it on
                    // the per-tick emit loop; skip before rng-
                    // consumption to preserve seed determinism (memory
                    // `learning_bevy_schedule_edge_perturbation`).
                    | AspirationDomain::Crafting
            ) {
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
                best = Some((chain.name, chain.domain, s));
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
                misaligned_since_tick: None,
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

/// Abandons aspirations via two orthogonal triggers:
///
/// - **Stagnation** — no progress for 2000 ticks AND personality
///   alignment for the domain has drifted below 0.3 (trait-vs-arc
///   signal).
/// - **Mood drift** (§7.7.d, ticket 055) — `Mood::valence` has sat
///   outside the arc's `expected_valence_target` hysteresis band for
///   `drift_sustain_duration` (mood-vs-arc signal).
///
/// Both triggers drop via `aspirations.active.remove(i)`; subsequent
/// re-adoption happens organically through `select_aspirations`
/// (pillar #4 — one commitment mechanism).
///
/// The two checks live in one system (rather than two systems racing
/// on the same Vec) for two reasons: (1) pillar discipline — one walk
/// per aspiration; (2) Bevy schedule-edge determinism — a second
/// sibling system would perturb seed-42 parallelism (precedent ticket
/// 061 / memory `learning_bevy_schedule_edge_perturbation`).
pub fn check_aspiration_abandonment(
    mut query: Query<(&Name, &Personality, &Mood, &mut Aspirations), Without<Dead>>,
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    registry: Res<AspirationRegistry>,
    time_scale: Res<TimeScale>,
    mut log: ResMut<NarrativeLog>,
    mut activation: ResMut<SystemActivation>,
) {
    const STAGNATION_TICKS: u64 = 2000;
    const MIN_ALIGNMENT: f32 = 0.3;
    let drift = &constants.aspirations;
    let drift_sustain_ticks = drift.drift_sustain_duration.ticks(&time_scale);

    for (name, personality, mood, mut aspirations) in &mut query {
        let mut to_remove = Vec::new();
        for (i, asp) in aspirations.active.iter_mut().enumerate() {
            // Ticket 398 — Kinship aspirations are event-driven (adopt
            // on first kitten, drop when no more dependents per
            // `adopt_kinship_aspiration`). Both stagnation and drift
            // paths are exempt: ActionCount(9999) is unreachable by
            // design, and low-compassion mothers would otherwise cycle
            // adopt → abandon for the duration of their kittens'
            // dependency.
            if asp.domain == AspirationDomain::Kinship {
                continue;
            }

            // Stagnation check.
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
                continue;
            }

            // §7.7.d mood drift-threshold check. Two-band hysteresis on
            // valence vs the arc's `expected_valence_target`; drops
            // when the misaligned interval reaches `drift_sustain_duration`.
            let Some(target) =
                crate::ai::aspirations::expected_valence_for(&asp.chain_name, &registry)
            else {
                continue;
            };
            let below_enter = mood.valence < target - drift.drift_enter_margin;
            let above_exit = mood.valence > target - drift.drift_exit_margin;
            match asp.misaligned_since_tick {
                None if below_enter => {
                    asp.misaligned_since_tick = Some(time.tick);
                }
                Some(_) if above_exit => {
                    asp.misaligned_since_tick = None;
                }
                Some(since) if time.tick.saturating_sub(since) >= drift_sustain_ticks => {
                    to_remove.push(i);
                    activation.record(Feature::AspirationDriftAbandoned);
                    log.push(
                        time.tick,
                        format!(
                            "The fire dims. {}'s heart no longer follows the path of {:?}.",
                            name.0, asp.domain,
                        ),
                        NarrativeTier::Action,
                    );
                }
                _ => {}
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

            let met = match &milestone.progress_tracker {
                crate::components::aspirations::ProgressTracker::ActionCount { actions, count } => {
                    // Increment progress when any matching action completes.
                    if current.ticks_remaining == 1 && actions.contains(&current.action) {
                        asp.progress += 1;
                        asp.last_progress_tick = time.tick;
                    }
                    asp.progress >= *count
                }
                crate::components::aspirations::ProgressTracker::SkillLevel { skill, level } => {
                    let current_level = skill.value(skills);
                    if current_level >= *level {
                        asp.last_progress_tick = time.tick;
                    }
                    current_level >= *level
                }
                crate::components::aspirations::ProgressTracker::FormBond { bond_type } => {
                    let target = *bond_type;
                    // Ticket 427 Step 4 — `.any()` short-circuits; no
                    // need to materialize the Vec.
                    let has_bond = relationships
                        .iter_for(cat_entity)
                        .any(|(_, rel)| rel.bond.is_some_and(|b| b >= target));
                    if has_bond {
                        asp.last_progress_tick = time.tick;
                    }
                    has_bond
                }
                crate::components::aspirations::ProgressTracker::WitnessEvent {
                    event_type,
                    count,
                } => {
                    let mt = *event_type;
                    let witnessed = memory
                        .events
                        .iter()
                        .filter(|e| e.event_type == mt && e.tick >= asp.adopted_tick)
                        .count();
                    if witnessed > 0 {
                        asp.last_progress_tick = time.tick;
                    }
                    witnessed as u32 >= *count
                }
                crate::components::aspirations::ProgressTracker::Mentor { count } => {
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
                MoodModifier::new(
                    0.4,
                    200,
                    format!("fulfilled aspiration: {}", asp.chain_name),
                )
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
        (Entity, &Position, &Skills, Has<markers::HasMentoringTarget>),
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
// adopt_kinship_aspiration (Ticket 398)
// ---------------------------------------------------------------------------

/// Ticket 398 — event-driven adoption + drop of `RAISE_OFFSPRING_ASPIRATION`.
///
/// §7.M.2 frames RaiseOffspringAspiration as a post-partum aspiration
/// (a parent adopts on first kitten; persists across the dependency
/// window; drops when no more dependent kittens). Unlike the other
/// 14 chains which adopt via the passive personality-scored picker,
/// Kinship is **event-driven**: presence-in-`KittenDependency.{mother,father}`
/// is the trigger.
///
/// **Both parents adopt (ticket 400 widening).** 398's original landing
/// scoped adoption to mothers only — the `is_mother` filter was a
/// stopgap to prevent low-compassion fathers from over-attempting
/// Caretake via the L1 `AspirationLift` (the `HandoffItem` cascade).
/// 400 pulls that boundary forward by replacing the uniform Caretake
/// lift with the personality-conditional `ParentingActivityModifier`
/// (see `src/ai/modifier.rs`), so fathers now adopt the aspiration but
/// only over-attempt Caretake when their `scale_presence` (compassion +
/// warmth) is actually high. Low-compassion diligent fathers receive a
/// provision_bias instead (Hunt-DSE lift), per the 399 design.
///
/// **Drops when no more dependent kittens.** When a cat is no longer
/// a parent of any living kitten (kittens matured or died), the
/// aspiration is removed from `Aspirations.active` — it doesn't move
/// to `completed` (no narrative-worthy completion event; the cat may
/// have another litter later). This makes the chain effectively
/// re-adoptable per-litter.
///
/// **Bypasses progress-stagnation abandonment.** The chain's
/// ActionCount(9999) progress tracker is deliberately unreachable
/// (the spec's natural completion is §7.7.a Elder transition).
/// `check_aspiration_abandonment` would otherwise drop the aspiration
/// on low-compassion parents after 2000 ticks of stagnation; the
/// abandonment system has a sibling guard to skip Kinship.
///
/// Sibling of `update_parent_markers` in Chain 2a — runs after the
/// markers are authored so adoption sees the freshly-set state.
#[allow(clippy::type_complexity)]
pub fn adopt_kinship_aspiration(
    mut query: Query<
        (Entity, &Name, &mut Aspirations),
        (With<AspirationsInitialized>, Without<Dead>),
    >,
    kittens: Query<&crate::components::KittenDependency, Without<Dead>>,
    time: Res<TimeState>,
    mut log: ResMut<NarrativeLog>,
) {
    const KINSHIP_CHAIN_NAME: &str = "Raise Offspring";

    use std::collections::HashSet;
    // Build the set of parent Entities (mother OR father) with at least
    // one living dependent kitten. Ticket 400 widens 398's mother-only
    // gate now that ParentingActivityModifier handles per-parent
    // dispersion personality-conditionally.
    let mut parents: HashSet<Entity> = HashSet::new();
    for dep in kittens.iter() {
        if let Some(m) = dep.mother {
            parents.insert(m);
        }
        if let Some(f) = dep.father {
            parents.insert(f);
        }
    }

    for (entity, name, mut aspirations) in &mut query {
        let is_parent = parents.contains(&entity);
        let kinship_active_idx = aspirations
            .active
            .iter()
            .position(|a| a.chain_name == KINSHIP_CHAIN_NAME);

        match (is_parent, kinship_active_idx) {
            (true, None) => {
                // Newly parent (mother or father) — adopt.
                aspirations.active.push(ActiveAspiration {
                    chain_name: KINSHIP_CHAIN_NAME.to_string(),
                    domain: AspirationDomain::Kinship,
                    current_milestone: 0,
                    progress: 0,
                    adopted_tick: time.tick,
                    last_progress_tick: time.tick,
                    misaligned_since_tick: None,
                });
                log.push(
                    time.tick,
                    format!(
                        "{} commits to raising their young. The litter becomes the work.",
                        name.0,
                    ),
                    NarrativeTier::Action,
                );
            }
            (false, Some(idx)) => {
                // No more living dependents — drop the aspiration.
                aspirations.active.remove(idx);
                log.push(
                    time.tick,
                    format!(
                        "{}'s litter has grown beyond their reach. The work is done.",
                        name.0,
                    ),
                    NarrativeTier::Action,
                );
            }
            _ => {}
        }
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
        world
            .entity_mut(cat)
            .get_mut::<Training>()
            .unwrap()
            .apprentice = None;
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
        assert!(world
            .entity(mentor)
            .contains::<markers::HasMentoringTarget>());
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
        assert!(!world
            .entity(mentor)
            .contains::<markers::HasMentoringTarget>());
    }

    #[test]
    fn peer_with_high_skill_no_marker() {
        let (mut world, mut schedule) = setup_mentoring_target();
        let mentor = spawn_cat_with_skills(&mut world, 0, 0, high_hunting_skills());
        // Peer also has high hunting — no skill gap on any axis.
        let _peer = spawn_cat_with_skills(&mut world, 3, 0, high_hunting_skills());
        schedule.run(&mut world);
        assert!(!world
            .entity(mentor)
            .contains::<markers::HasMentoringTarget>());
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
        assert!(!world
            .entity(mentor)
            .contains::<markers::HasMentoringTarget>());
    }

    #[test]
    fn skill_gap_disappears_when_peer_levels_up() {
        let (mut world, mut schedule) = setup_mentoring_target();
        let mentor = spawn_cat_with_skills(&mut world, 0, 0, high_hunting_skills());
        let peer = spawn_cat_with_skills(&mut world, 3, 0, low_hunting_skills());
        schedule.run(&mut world);
        assert!(world
            .entity(mentor)
            .contains::<markers::HasMentoringTarget>());
        // Peer learns. Now they're both above 0.3 — no gap > threshold.
        world.entity_mut(peer).get_mut::<Skills>().unwrap().hunting = 0.5;
        schedule.run(&mut world);
        assert!(!world
            .entity(mentor)
            .contains::<markers::HasMentoringTarget>());
    }

    #[test]
    fn mentoring_target_idempotent() {
        let (mut world, mut schedule) = setup_mentoring_target();
        let mentor = spawn_cat_with_skills(&mut world, 0, 0, high_hunting_skills());
        let _peer = spawn_cat_with_skills(&mut world, 3, 0, low_hunting_skills());
        schedule.run(&mut world);
        assert!(world
            .entity(mentor)
            .contains::<markers::HasMentoringTarget>());
        schedule.run(&mut world);
        assert!(world
            .entity(mentor)
            .contains::<markers::HasMentoringTarget>());
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
        assert!(world
            .entity(mentor)
            .contains::<markers::HasMentoringTarget>());
    }

    // -----------------------------------------------------------------------
    // §7.7.d mood drift-threshold detection (ticket 055)
    // -----------------------------------------------------------------------

    fn setup_drift_world() -> (World, Schedule) {
        use crate::resources::SimConstants;
        let mut world = World::new();
        world.insert_resource(SimConstants::default());
        let cfg = SimConfig::default();
        world.insert_resource(TimeScale::from_config(&cfg, 16.6667));
        world.insert_resource(cfg);
        world.insert_resource(TimeState::default());
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(SystemActivation::default());
        world.insert_resource(AspirationRegistry::build_static());
        let mut schedule = Schedule::default();
        schedule.add_systems(check_aspiration_abandonment);
        (world, schedule)
    }

    /// Default-mid personality used for the drift tests so the
    /// stagnation arm of `check_aspiration_abandonment` never fires
    /// (every axis at 0.5 keeps `domain_personality_axis >= 0.3`).
    fn drift_personality() -> Personality {
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

    fn spawn_drift_cat(
        world: &mut World,
        chain_name: &str,
        domain: AspirationDomain,
        valence: f32,
    ) -> Entity {
        // Use a non-zero `last_progress_tick` matching the test's
        // initial `time.tick` so the sibling stagnation check is
        // satisfied — its `time.tick - last_progress_tick >= 2000`
        // gate is what would otherwise trigger spuriously when the
        // drift test advances `tick`.
        world
            .spawn((
                Name("Test".to_string()),
                drift_personality(),
                Mood {
                    valence,
                    modifiers: std::collections::VecDeque::new(),
                },
                Aspirations {
                    active: vec![ActiveAspiration {
                        chain_name: chain_name.to_string(),
                        domain,
                        current_milestone: 0,
                        progress: 0,
                        adopted_tick: 0,
                        last_progress_tick: 0,
                        misaligned_since_tick: None,
                    }],
                    completed: Vec::new(),
                },
            ))
            .id()
    }

    /// Hysteresis: a brief dip below the entry band that recovers above
    /// the exit band must clear `misaligned_since_tick` without firing.
    #[test]
    fn mood_drift_short_dip_clears_state() {
        let (mut world, mut schedule) = setup_drift_world();
        // MASTER_OF_THE_HUNT target 0.30; enter < 0.30 - 0.25 = 0.05.
        let cat = spawn_drift_cat(
            &mut world,
            "Master of the Hunt",
            AspirationDomain::Hunting,
            -0.10, // well below enter band
        );

        schedule.run(&mut world);
        assert_eq!(
            world.get::<Aspirations>(cat).unwrap().active[0].misaligned_since_tick,
            Some(0),
            "below-enter valence should mark misaligned_since_tick"
        );

        // Recover above the exit band (target - exit_margin = 0.20).
        world.get_mut::<Mood>(cat).unwrap().valence = 0.30;
        schedule.run(&mut world);
        assert_eq!(
            world.get::<Aspirations>(cat).unwrap().active[0].misaligned_since_tick,
            None,
            "above-exit valence should clear misaligned_since_tick"
        );
        assert_eq!(
            world.get::<Aspirations>(cat).unwrap().active.len(),
            1,
            "no abandonment on a transient dip"
        );
        assert_eq!(
            world
                .resource::<SystemActivation>()
                .counts
                .get(&Feature::AspirationDriftAbandoned)
                .copied()
                .unwrap_or(0),
            0
        );
    }

    /// Sustain: misalignment held for `drift_sustain_duration` drops the
    /// arc, records the Feature, and pushes a narrative log entry.
    #[test]
    fn mood_drift_sustained_dip_triggers_abandonment() {
        let (mut world, mut schedule) = setup_drift_world();
        let sustain = world
            .resource::<crate::resources::SimConstants>()
            .aspirations
            .drift_sustain_duration
            .ticks(world.resource::<TimeScale>());
        let cat = spawn_drift_cat(
            &mut world,
            "Master of the Hunt",
            AspirationDomain::Hunting,
            -0.50, // far below enter band
        );

        // First tick: enter misaligned state.
        schedule.run(&mut world);
        assert!(world.get::<Aspirations>(cat).unwrap().active[0]
            .misaligned_since_tick
            .is_some());

        // Advance `tick` past sustain and run one more time.
        world.resource_mut::<TimeState>().tick = sustain + 1;
        let initial_log_len = world.resource::<NarrativeLog>().entries.len();
        schedule.run(&mut world);

        assert!(
            world.get::<Aspirations>(cat).unwrap().active.is_empty(),
            "arc should drop after sustained drift"
        );
        assert_eq!(
            world
                .resource::<SystemActivation>()
                .counts
                .get(&Feature::AspirationDriftAbandoned)
                .copied()
                .unwrap_or(0),
            1
        );
        assert!(
            world.resource::<NarrativeLog>().entries.len() > initial_log_len,
            "narrative log should record the drift abandonment"
        );
    }

    /// Arc-relative gating: low absolute valence that still sits within
    /// the arc's negative-target band must NOT trigger. Pure mood-vs-arc
    /// signal, not absolute-low-valence signal.
    #[test]
    fn mood_drift_aligned_with_negative_arc_does_not_trigger() {
        let (mut world, mut schedule) = setup_drift_world();
        let sustain = world
            .resource::<crate::resources::SimConstants>()
            .aspirations
            .drift_sustain_duration
            .ticks(world.resource::<TimeScale>());
        // WARRIORS_PATH target -0.10; enter band < -0.10 - 0.25 = -0.35.
        // Valence -0.05 is BELOW Mood::default() 0.2 (looks "low" naively)
        // but ABOVE the entry threshold for this arc.
        let cat = spawn_drift_cat(
            &mut world,
            "Warrior's Path",
            AspirationDomain::Combat,
            -0.05,
        );

        // Advance well past sustain duration.
        for tick in 0..=(sustain + 1) {
            world.resource_mut::<TimeState>().tick = tick;
            schedule.run(&mut world);
        }

        assert_eq!(
            world.get::<Aspirations>(cat).unwrap().active.len(),
            1,
            "aligned arc should not drop on absolute-low valence"
        );
        assert_eq!(
            world.get::<Aspirations>(cat).unwrap().active[0].misaligned_since_tick,
            None
        );
        assert_eq!(
            world
                .resource::<SystemActivation>()
                .counts
                .get(&Feature::AspirationDriftAbandoned)
                .copied()
                .unwrap_or(0),
            0
        );
    }

    /// Kinship carve-out: even with sustained low valence, the kinship
    /// arc is exempt (parallels the 398 carve-out in
    /// `check_aspiration_abandonment`).
    #[test]
    fn mood_drift_kinship_arc_is_exempt() {
        let (mut world, mut schedule) = setup_drift_world();
        let sustain = world
            .resource::<crate::resources::SimConstants>()
            .aspirations
            .drift_sustain_duration
            .ticks(world.resource::<TimeScale>());
        // RAISE_OFFSPRING_ASPIRATION target 0.40; even with valence at
        // the floor, the Kinship carve-out must skip the check.
        let cat = spawn_drift_cat(
            &mut world,
            "Raise Offspring",
            AspirationDomain::Kinship,
            -1.0,
        );

        for tick in 0..=(sustain + 1) {
            world.resource_mut::<TimeState>().tick = tick;
            schedule.run(&mut world);
        }

        assert_eq!(
            world.get::<Aspirations>(cat).unwrap().active.len(),
            1,
            "kinship arc must not drop via mood drift"
        );
        assert_eq!(
            world
                .resource::<SystemActivation>()
                .counts
                .get(&Feature::AspirationDriftAbandoned)
                .copied()
                .unwrap_or(0),
            0
        );
    }
}
