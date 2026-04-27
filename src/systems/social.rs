use std::collections::HashMap;

use bevy_ecs::prelude::*;

use crate::components::building::Structure;
use crate::components::identity::{Age, Gender, LifeStage, Name, Orientation};
use crate::components::physical::{Dead, Position};
use crate::resources::event_log::{EventKind, EventLog};
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::relationships::{BondType, Relationships};
use crate::resources::sim_constants::SimConstants;
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{SimConfig, TimeScale, TimeState};

// ---------------------------------------------------------------------------
// passive_familiarity system
// ---------------------------------------------------------------------------

/// Each tick, cats within Manhattan distance <= 2 of each other gain a small
/// amount of familiarity. Proximity naturally builds recognition over time.
#[allow(clippy::type_complexity)]
pub fn passive_familiarity(
    query: Query<(Entity, &Position), (Without<Dead>, Without<Structure>)>,
    mut relationships: ResMut<Relationships>,
    constants: Res<SimConstants>,
    time_scale: Res<TimeScale>,
) {
    let c = &constants.social;
    let passive_familiarity_rate = c.passive_familiarity_rate.per_tick(&time_scale);
    let cats: Vec<(Entity, Position)> = query.iter().map(|(e, p)| (e, *p)).collect();
    for i in 0..cats.len() {
        for j in (i + 1)..cats.len() {
            if cats[i].1.manhattan_distance(&cats[j].1) <= c.passive_familiarity_range {
                relationships.modify_familiarity(cats[i].0, cats[j].0, passive_familiarity_rate);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// befriend_wildlife author (§9.2 BefriendedAlly)
// ---------------------------------------------------------------------------

/// §9.2 / ticket 049 — author the `BefriendedAlly` marker on cats and
/// wildlife once their cross-species relationship familiarity crosses
/// `constants.social.befriend_familiarity_threshold`. Tags both sides
/// of the pair (a befriended fox carries the marker on the fox; the
/// cat that befriended it also carries it).
///
/// Hysteresis: removed when familiarity drops below
/// `(threshold - hysteresis)`. Without the band, repeated socialize
/// vs. avoid would flicker the marker each tick at the boundary.
///
/// **Note**: `Relationships` accepts cat ↔ wildlife pairs at the
/// storage layer, but no production system writes familiarity for
/// such pairs today. The author runs each tick and produces a
/// no-op until a follow-on (or test fixtures) seed familiarity.
///
/// Algorithm: for each (cat, wildlife) pair where familiarity ≥
/// upgrade-threshold, tag both. If either side carries the marker
/// but the highest pairwise familiarity drops below the
/// downgrade-threshold, remove the marker. Per-entity decision —
/// the marker is *per-entity*, not per-pair (consumers like
/// fox_raiding read off the fox itself; the per-pair "befriended-by-
/// whom" model is a follow-on per ticket 049 D5).
#[allow(clippy::type_complexity)]
pub fn befriend_wildlife(
    mut commands: Commands,
    cats: Query<
        (
            Entity,
            bevy::prelude::Has<crate::components::markers::BefriendedAlly>,
        ),
        (
            With<crate::components::identity::Species>,
            Without<Dead>,
            Without<Structure>,
            Without<crate::components::wildlife::WildAnimal>,
        ),
    >,
    wildlife: Query<
        (
            Entity,
            bevy::prelude::Has<crate::components::markers::BefriendedAlly>,
        ),
        (
            With<crate::components::wildlife::WildAnimal>,
            Without<Dead>,
        ),
    >,
    relationships: Res<Relationships>,
    constants: Res<SimConstants>,
) {
    let s = &constants.social;
    let upgrade = s.befriend_familiarity_threshold;
    let downgrade = (upgrade - s.befriend_familiarity_hysteresis).max(0.0);

    let cat_list: Vec<(Entity, bool)> = cats.iter().collect();
    let wildlife_list: Vec<(Entity, bool)> = wildlife.iter().collect();

    // Per-entity max familiarity over all cross-species pairs the
    // entity participates in. The marker is per-entity, so a cat with
    // *any* befriended wildlife counterpart carries it; same for a
    // wildlife creature with any befriending cat.
    let mut cat_max_fam: HashMap<Entity, f32> = HashMap::new();
    let mut wild_max_fam: HashMap<Entity, f32> = HashMap::new();
    for (cat_entity, _) in &cat_list {
        for (wild_entity, _) in &wildlife_list {
            let fam = relationships
                .get(*cat_entity, *wild_entity)
                .map(|r| r.familiarity)
                .unwrap_or(0.0);
            let cmax = cat_max_fam.entry(*cat_entity).or_insert(0.0);
            if fam > *cmax {
                *cmax = fam;
            }
            let wmax = wild_max_fam.entry(*wild_entity).or_insert(0.0);
            if fam > *wmax {
                *wmax = fam;
            }
        }
    }

    for (cat_entity, has_marker) in &cat_list {
        let fam = cat_max_fam.get(cat_entity).copied().unwrap_or(0.0);
        toggle_marker(&mut commands, *cat_entity, *has_marker, fam, upgrade, downgrade);
    }
    for (wild_entity, has_marker) in &wildlife_list {
        let fam = wild_max_fam.get(wild_entity).copied().unwrap_or(0.0);
        toggle_marker(
            &mut commands,
            *wild_entity,
            *has_marker,
            fam,
            upgrade,
            downgrade,
        );
    }
}

fn toggle_marker(
    commands: &mut Commands,
    entity: Entity,
    has: bool,
    familiarity: f32,
    upgrade: f32,
    downgrade: f32,
) {
    let want = if has {
        familiarity >= downgrade
    } else {
        familiarity >= upgrade
    };
    match (want, has) {
        (true, false) => {
            commands
                .entity(entity)
                .insert(crate::components::markers::BefriendedAlly);
        }
        (false, true) => {
            commands
                .entity(entity)
                .remove::<crate::components::markers::BefriendedAlly>();
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// check_bonds system
// ---------------------------------------------------------------------------

/// Per-cat fields relevant to courtship drift and bond-upgrade gating.
///
/// Snapshotted before the main loop so we can look up both sides of each
/// relationship pair without re-querying components per iteration.
#[derive(Clone, Copy)]
struct CourtshipFitness {
    stage: LifeStage,
    gender: Gender,
    orientation: Orientation,
}

/// Periodically check all relationships and upgrade bonds when thresholds are
/// met. Emits Tier::Significant narrative on bond formation.
///
/// Also accumulates romantic attachment for orientation-compatible pairs of
/// adult cats whose fondness and familiarity have crossed the courtship
/// gates. Without this, romantic stays at 0.0 forever — the MateWith step is
/// the only other writer, and it requires a Partners bond to reach.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn check_bonds(
    time: Res<TimeState>,
    config: Res<SimConfig>,
    time_scale: Res<TimeScale>,
    mut relationships: ResMut<Relationships>,
    mut log: ResMut<NarrativeLog>,
    names: Query<&Name>,
    positions: Query<&Position>,
    fitness_query: Query<
        (Entity, &Age, &Gender, &Orientation),
        (Without<Dead>, Without<Structure>),
    >,
    mut colony_score: Option<ResMut<crate::resources::colony_score::ColonyScore>>,
    constants: Res<SimConstants>,
    mut activation: ResMut<SystemActivation>,
    mut pushback: MessageWriter<crate::systems::magic::CorruptionPushback>,
    mut event_log: Option<ResMut<EventLog>>,
) {
    let c = &constants.social;
    // Only check every bond_check_interval ticks.
    if !time.tick.is_multiple_of(c.bond_check_interval) {
        return;
    }
    // Per-check semantics: courtship_romantic_rate is the value added each
    // time the cadence fires. RatePerDay value × ticks_per_day_phase →
    // that legacy per-tick numeric.
    let courtship_romantic_rate = c.courtship_romantic_rate.per_tick(&time_scale);

    let fitness: HashMap<Entity, CourtshipFitness> = fitness_query
        .iter()
        .map(|(e, age, gender, orient)| {
            (
                e,
                CourtshipFitness {
                    stage: age.stage(time.tick, config.ticks_per_season),
                    gender: *gender,
                    orientation: *orient,
                },
            )
        })
        .collect();

    for ((a, b), rel) in relationships.pairs_iter_mut() {
        let old_bond = rel.bond;

        // Orientation + life-stage gate for romantic involvement. Friends bonds
        // remain open to anyone, including kittens and asexual cats; only
        // romantic outcomes require compatibility.
        let romantic_eligible = match (fitness.get(&a), fitness.get(&b)) {
            (Some(fa), Some(fb)) => {
                matches!(fa.stage, LifeStage::Adult | LifeStage::Elder)
                    && matches!(fb.stage, LifeStage::Adult | LifeStage::Elder)
                    && are_orientation_compatible(
                        fa.gender,
                        fa.orientation,
                        fb.gender,
                        fb.orientation,
                    )
            }
            _ => false,
        };

        // Courtship drift: compatible close-friend pairs develop romantic
        // attraction over time, breaking the Partners/Mate chicken-and-egg.
        //
        // Ticket 027 Bug 1: emit `Feature::CourtshipInteraction` and push
        // an `EventKind::CourtshipDrifted` event each time the gate fires.
        // Without this the `continuity_tallies.courtship` canary tracks
        // only `MatingOccurred` (which is currently zero per Bugs 2/3),
        // hiding the fact that passive drift IS accumulating.
        if romantic_eligible
            && rel.fondness > c.courtship_fondness_gate
            && rel.familiarity > c.courtship_familiarity_gate
        {
            rel.romantic = (rel.romantic + courtship_romantic_rate).min(1.0);
            activation.record(Feature::CourtshipInteraction);
            if let Some(elog) = event_log.as_mut() {
                if let (Ok(name_a), Ok(name_b)) = (names.get(a), names.get(b)) {
                    elog.push(
                        time.tick,
                        EventKind::CourtshipDrifted {
                            cat_a: name_a.0.clone(),
                            cat_b: name_b.0.clone(),
                        },
                    );
                }
            }
        }

        let new_bond = if romantic_eligible
            && rel.romantic > c.mates_romantic_threshold
            && rel.fondness > c.mates_fondness_threshold
            && rel.familiarity > c.mates_familiarity_threshold
        {
            Some(BondType::Mates)
        } else if romantic_eligible
            && rel.romantic > c.partners_romantic_threshold
            && rel.fondness > c.partners_fondness_threshold
            && rel.familiarity > c.partners_familiarity_threshold
        {
            Some(BondType::Partners)
        } else if rel.fondness > c.friends_fondness_threshold
            && rel.familiarity > c.friends_familiarity_threshold
        {
            Some(BondType::Friends)
        } else {
            None
        };

        // Only upgrade bonds, never downgrade.
        if new_bond > old_bond {
            rel.bond = new_bond;
            activation.record(Feature::BondFormed);
            if let Some(ref mut score) = colony_score {
                score.bonds_formed += 1;
            }
            if let (Ok(name_a), Ok(name_b)) = (names.get(a), names.get(b)) {
                let text = match new_bond.unwrap() {
                    BondType::Friends => {
                        format!("{} and {} have become close friends.", name_a.0, name_b.0)
                    }
                    BondType::Partners => {
                        format!("{} and {} have become partners.", name_a.0, name_b.0)
                    }
                    BondType::Mates => {
                        format!("{} and {} have become mates.", name_a.0, name_b.0)
                    }
                };
                log.push(time.tick, text, NarrativeTier::Significant);
            }
            // Bond warmth pushes back corruption.
            if let Ok(pos) = positions.get(a) {
                pushback.write(crate::systems::magic::CorruptionPushback {
                    position: *pos,
                    radius: 3,
                    amount: 0.05,
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Orientation compatibility
// ---------------------------------------------------------------------------

/// Check whether two cats can develop romantic feelings for each other based
/// on gender and orientation.
///
/// Nonbinary cats are compatible with all orientations (Straight, Gay, Bisexual).
/// Only Asexual blocks romantic development entirely.
pub fn are_orientation_compatible(
    a_gender: Gender,
    a_orient: Orientation,
    b_gender: Gender,
    b_orient: Orientation,
) -> bool {
    if a_orient == Orientation::Asexual || b_orient == Orientation::Asexual {
        return false;
    }

    let a_attracted = match a_orient {
        Orientation::Straight => {
            a_gender != b_gender || b_gender == Gender::Nonbinary || a_gender == Gender::Nonbinary
        }
        Orientation::Gay => {
            a_gender == b_gender || b_gender == Gender::Nonbinary || a_gender == Gender::Nonbinary
        }
        Orientation::Bisexual => true,
        Orientation::Asexual => false,
    };
    let b_attracted = match b_orient {
        Orientation::Straight => {
            b_gender != a_gender || a_gender == Gender::Nonbinary || b_gender == Gender::Nonbinary
        }
        Orientation::Gay => {
            b_gender == a_gender || a_gender == Gender::Nonbinary || b_gender == Gender::Nonbinary
        }
        Orientation::Bisexual => true,
        Orientation::Asexual => false,
    };

    a_attracted && b_attracted
}

// ---------------------------------------------------------------------------
// Value compatibility
// ---------------------------------------------------------------------------

/// Compute fondness delta from comparing two cats' value axes during interaction.
/// Same-side values: +same_delta per axis. Divergent values: +divergent_delta per axis.
#[allow(clippy::too_many_arguments)]
pub fn value_compatibility_delta(
    a_loyalty: f32,
    a_tradition: f32,
    a_compassion: f32,
    a_pride: f32,
    a_independence: f32,
    b_loyalty: f32,
    b_tradition: f32,
    b_compassion: f32,
    b_pride: f32,
    b_independence: f32,
    constants: &crate::resources::sim_constants::SocialConstants,
) -> f32 {
    let axes = [
        (a_loyalty, b_loyalty),
        (a_tradition, b_tradition),
        (a_compassion, b_compassion),
        (a_pride, b_pride),
        (a_independence, b_independence),
    ];
    let mut delta = 0.0;
    for (va, vb) in axes {
        let same_side = (va > constants.value_compat_same_threshold
            && vb > constants.value_compat_same_threshold)
            || (va < constants.value_compat_same_threshold
                && vb < constants.value_compat_same_threshold);
        let divergent = (va > constants.value_compat_divergent_high
            && vb < constants.value_compat_divergent_low)
            || (va < constants.value_compat_divergent_low
                && vb > constants.value_compat_divergent_high);
        if same_side {
            delta += constants.value_compat_same_delta;
        }
        if divergent {
            delta += constants.value_compat_divergent_delta;
        }
    }
    delta
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::Schedule;

    use crate::components::physical::Position;
    use crate::resources::narrative::NarrativeLog;
    use crate::resources::relationships::Relationships;
    use crate::resources::time::TimeState;

    fn test_time_scale() -> TimeScale {
        TimeScale::from_config(&SimConfig::default(), 16.6667)
    }

    fn setup_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(Relationships::default());
        world.insert_resource(TimeState::default());
        world.insert_resource(crate::resources::time::SimConfig::default());
        world.insert_resource(test_time_scale());
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(SystemActivation::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(passive_familiarity);
        (world, schedule)
    }

    #[test]
    fn passive_familiarity_increases_for_adjacent_cats() {
        let (mut world, mut schedule) = setup_world();

        let a = world.spawn(Position::new(5, 5)).id();
        let b = world.spawn(Position::new(5, 6)).id();

        // Init relationship.
        world
            .resource_mut::<Relationships>()
            .get_or_insert(a, b)
            .familiarity = 0.0;

        schedule.run(&mut world);

        let fam = world
            .resource::<Relationships>()
            .get(a, b)
            .unwrap()
            .familiarity;
        assert!(
            (fam - 0.0003).abs() < 1e-6,
            "familiarity should be ~0.0003; got {fam}"
        );
    }

    #[test]
    fn passive_familiarity_unchanged_for_distant_cats() {
        let (mut world, mut schedule) = setup_world();

        let a = world.spawn(Position::new(0, 0)).id();
        let b = world.spawn(Position::new(10, 10)).id();

        world
            .resource_mut::<Relationships>()
            .get_or_insert(a, b)
            .familiarity = 0.0;

        schedule.run(&mut world);

        let fam = world
            .resource::<Relationships>()
            .get(a, b)
            .unwrap()
            .familiarity;
        assert_eq!(fam, 0.0, "distant cats should not gain familiarity");
    }

    #[test]
    fn value_compatibility_positive_for_aligned_values() {
        let sc = &crate::resources::SimConstants::default().social;
        // Both cats have all values > 0.5 (same side).
        let delta = value_compatibility_delta(0.8, 0.7, 0.9, 0.6, 0.8, 0.7, 0.8, 0.6, 0.9, 0.7, sc);
        assert!(
            delta > 0.0,
            "aligned values should produce positive delta; got {delta}"
        );
        assert!(
            (delta - 0.001).abs() < 1e-6,
            "5 same-side axes should give +0.001; got {delta}"
        );
    }

    #[test]
    fn value_compatibility_negative_for_divergent_values() {
        let sc = &crate::resources::SimConstants::default().social;
        // Cat A has high values, Cat B has low values (all divergent).
        let delta = value_compatibility_delta(0.8, 0.8, 0.8, 0.8, 0.8, 0.2, 0.2, 0.2, 0.2, 0.2, sc);
        // Each axis: same_side is true (both effectively "above or below") — wait, 0.8 > 0.5 and 0.2 < 0.5, so NOT same side.
        // Each axis: divergent is true (0.8 > 0.7, 0.2 < 0.3).
        // So delta = 5 * (-0.0001) = -0.0005
        assert!(
            delta < 0.0,
            "divergent values should produce negative delta; got {delta}"
        );
        assert!(
            (delta - (-0.0005)).abs() < 1e-6,
            "5 divergent axes should give -0.0005; got {delta}"
        );
    }

    #[test]
    fn romantic_stays_zero_for_asexual_cats() {
        assert!(
            !are_orientation_compatible(
                Gender::Queen,
                Orientation::Asexual,
                Gender::Tom,
                Orientation::Straight
            ),
            "asexual cat should not be romantically compatible"
        );
        assert!(
            !are_orientation_compatible(
                Gender::Tom,
                Orientation::Straight,
                Gender::Queen,
                Orientation::Asexual
            ),
            "cat paired with asexual should not be compatible"
        );
    }

    #[test]
    fn orientation_compatibility_matrix() {
        // Straight Tom + Queen → compatible
        assert!(are_orientation_compatible(
            Gender::Tom,
            Orientation::Straight,
            Gender::Queen,
            Orientation::Straight
        ));
        // Straight Tom + Tom → NOT compatible
        assert!(!are_orientation_compatible(
            Gender::Tom,
            Orientation::Straight,
            Gender::Tom,
            Orientation::Straight
        ));
        // Gay Tom + Tom → compatible
        assert!(are_orientation_compatible(
            Gender::Tom,
            Orientation::Gay,
            Gender::Tom,
            Orientation::Gay
        ));
        // Gay Tom + Queen → NOT compatible
        assert!(!are_orientation_compatible(
            Gender::Tom,
            Orientation::Gay,
            Gender::Queen,
            Orientation::Gay
        ));
        // Bisexual + any non-asexual → compatible
        assert!(are_orientation_compatible(
            Gender::Tom,
            Orientation::Bisexual,
            Gender::Tom,
            Orientation::Bisexual
        ));
        assert!(are_orientation_compatible(
            Gender::Tom,
            Orientation::Bisexual,
            Gender::Queen,
            Orientation::Straight
        ));
        // Nonbinary + Straight → compatible
        assert!(are_orientation_compatible(
            Gender::Nonbinary,
            Orientation::Straight,
            Gender::Tom,
            Orientation::Straight
        ));
        // Nonbinary + Gay → compatible
        assert!(are_orientation_compatible(
            Gender::Nonbinary,
            Orientation::Gay,
            Gender::Tom,
            Orientation::Gay
        ));
    }

    /// Helper: build a test world with `check_bonds` ready to run.
    /// Pre-registers every resource and the single message type the system writes.
    fn bond_test_world(tick: u64) -> (World, Schedule) {
        let mut world = World::new();
        let mut time = TimeState::default();
        time.tick = tick;
        world.insert_resource(time);
        world.insert_resource(crate::resources::time::SimConfig::default());
        world.insert_resource(test_time_scale());
        world.insert_resource(NarrativeLog::default());
        world.insert_resource(crate::resources::SimConstants::default());
        world.insert_resource(SystemActivation::default());
        bevy_ecs::message::MessageRegistry::register_message::<
            crate::systems::magic::CorruptionPushback,
        >(&mut world);
        let mut schedule = Schedule::default();
        schedule.add_systems(bevy_ecs::message::message_update_system);
        schedule.add_systems(check_bonds);
        (world, schedule)
    }

    /// Helper: spawn a cat at life stage Adult by using a born_tick old enough
    /// for a 12+ season age under the default ticks_per_season (20_000).
    fn spawn_adult(
        world: &mut World,
        name: &str,
        gender: Gender,
        orientation: Orientation,
    ) -> Entity {
        world
            .spawn((
                Name(name.to_string()),
                Age { born_tick: 0 },
                gender,
                orientation,
            ))
            .id()
    }

    #[test]
    fn bond_forms_at_threshold() {
        // Age cats to Adult: tick 50 + ticks_per_season * 12 is enough.
        let adult_tick = 50 + 20_000 * 12;
        let (mut world, mut schedule) = bond_test_world(adult_tick);
        let a = spawn_adult(&mut world, "Fern", Gender::Queen, Orientation::Straight);
        let b = spawn_adult(&mut world, "Reed", Gender::Tom, Orientation::Straight);

        let mut rels = Relationships::default();
        let rel = rels.get_or_insert(a, b);
        rel.fondness = 0.4;
        rel.familiarity = 0.5;
        world.insert_resource(rels);

        schedule.run(&mut world);

        let rels = world.resource::<Relationships>();
        assert_eq!(
            rels.get(a, b).unwrap().bond,
            Some(BondType::Friends),
            "bond should be Friends at f=0.4, fam=0.5"
        );

        let log = world.resource::<NarrativeLog>();
        assert!(
            log.entries.iter().any(|e| e.text.contains("close friends")),
            "should narrate bond formation"
        );
    }

    #[test]
    fn courtship_drift_grows_romantic_for_compatible_pair() {
        let adult_tick = 50 + 20_000 * 12;
        let (mut world, mut schedule) = bond_test_world(adult_tick);
        let a = spawn_adult(&mut world, "Fern", Gender::Queen, Orientation::Straight);
        let b = spawn_adult(&mut world, "Reed", Gender::Tom, Orientation::Straight);

        let mut rels = Relationships::default();
        let rel = rels.get_or_insert(a, b);
        rel.fondness = 0.5;
        rel.familiarity = 0.5;
        rel.romantic = 0.0;
        world.insert_resource(rels);

        schedule.run(&mut world);

        let rels = world.resource::<Relationships>();
        let ts = test_time_scale();
        let rate = crate::resources::SimConstants::default()
            .social
            .courtship_romantic_rate
            .per_tick(&ts);
        assert!(
            (rels.get(a, b).unwrap().romantic - rate).abs() < 1e-6,
            "one tick of courtship should add exactly courtship_romantic_rate to romantic"
        );
    }

    #[test]
    fn courtship_drift_skips_incompatible_orientation() {
        let adult_tick = 50 + 20_000 * 12;
        let (mut world, mut schedule) = bond_test_world(adult_tick);
        // Two straight Toms — not orientation-compatible.
        let a = spawn_adult(&mut world, "Flint", Gender::Tom, Orientation::Straight);
        let b = spawn_adult(&mut world, "Reed", Gender::Tom, Orientation::Straight);

        let mut rels = Relationships::default();
        let rel = rels.get_or_insert(a, b);
        rel.fondness = 0.5;
        rel.familiarity = 0.5;
        rel.romantic = 0.0;
        world.insert_resource(rels);

        schedule.run(&mut world);

        let rels = world.resource::<Relationships>();
        assert_eq!(
            rels.get(a, b).unwrap().romantic,
            0.0,
            "incompatible orientations should not accumulate romantic"
        );
    }

    #[test]
    fn courtship_drift_skips_kittens() {
        // Cats born at tick 0, checked at tick 50 → Kitten stage.
        let (mut world, mut schedule) = bond_test_world(50);
        let a = spawn_adult(&mut world, "Sprout", Gender::Queen, Orientation::Straight);
        let b = spawn_adult(&mut world, "Brook", Gender::Tom, Orientation::Straight);

        let mut rels = Relationships::default();
        let rel = rels.get_or_insert(a, b);
        rel.fondness = 0.5;
        rel.familiarity = 0.5;
        rel.romantic = 0.0;
        world.insert_resource(rels);

        schedule.run(&mut world);

        let rels = world.resource::<Relationships>();
        assert_eq!(
            rels.get(a, b).unwrap().romantic,
            0.0,
            "kittens cannot accumulate romantic"
        );
    }

    #[test]
    fn courtship_drift_engages_at_friends_tier_fondness() {
        // The courtship_fondness_gate is aligned with friends_fondness_threshold
        // (0.3) so that drift engages the moment a Friends bond can form.
        // Previously this was 0.4, leaving a dead zone where Friends-tier pairs
        // never developed romantic attraction.
        let adult_tick = 50 + 20_000 * 12;
        let (mut world, mut schedule) = bond_test_world(adult_tick);
        let a = spawn_adult(&mut world, "Fern", Gender::Queen, Orientation::Straight);
        let b = spawn_adult(&mut world, "Reed", Gender::Tom, Orientation::Straight);

        let mut rels = Relationships::default();
        let rel = rels.get_or_insert(a, b);
        rel.fondness = 0.35; // above Friends (0.3) and the new gate (0.3)
        rel.familiarity = 0.45; // above Friends (0.4) and the gate (0.4)
        rel.romantic = 0.0;
        world.insert_resource(rels);

        schedule.run(&mut world);

        let rels = world.resource::<Relationships>();
        let ts = test_time_scale();
        let rate = crate::resources::SimConstants::default()
            .social
            .courtship_romantic_rate
            .per_tick(&ts);
        assert!(
            rels.get(a, b).unwrap().romantic > 0.0,
            "drift should engage for Friends-tier pair under new fondness gate"
        );
        assert!(
            (rels.get(a, b).unwrap().romantic - rate).abs() < 1e-6,
            "one tick of drift should add exactly courtship_romantic_rate"
        );
    }

    #[test]
    fn compatible_adults_reach_partners_bond_in_expected_time() {
        // Confirms the math: courtship_romantic_rate = 0.0015 per check means
        // partners_romantic_threshold (0.5) is reached in ~334 checks. We
        // simulate the needed number of checks directly rather than advancing
        // time ticks through a full schedule.
        let c = crate::resources::SimConstants::default().social;
        let ts = test_time_scale();
        let courtship_rate_per_tick = c.courtship_romantic_rate.per_tick(&ts);
        let checks_needed =
            (c.partners_romantic_threshold / courtship_rate_per_tick).ceil() as u64;

        let adult_tick = 50 + 20_000 * 12;
        let (mut world, mut schedule) = bond_test_world(adult_tick);
        let a = spawn_adult(&mut world, "Fern", Gender::Queen, Orientation::Straight);
        let b = spawn_adult(&mut world, "Reed", Gender::Tom, Orientation::Straight);

        let mut rels = Relationships::default();
        let rel = rels.get_or_insert(a, b);
        rel.fondness = 0.7;
        rel.familiarity = 0.6;
        rel.romantic = 0.0;
        world.insert_resource(rels);

        for i in 0..checks_needed + 1 {
            // Advance tick by bond_check_interval each iteration so check_bonds fires.
            world.resource_mut::<TimeState>().tick = adult_tick + (i + 1) * c.bond_check_interval;
            schedule.run(&mut world);
        }

        let rels = world.resource::<Relationships>();
        let bond = rels.get(a, b).unwrap().bond;
        assert_eq!(
            bond,
            Some(BondType::Partners),
            "compatible adults with strong fondness/familiarity should reach Partners in ~{checks_needed} checks; got bond {bond:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Ticket 027 Bug 1: courtship-drift gate emits Feature + EventKind so the
    // continuity_tallies.courtship canary tracks passive drift independently
    // of the deadlocked MateWith path.
    // -----------------------------------------------------------------------

    fn count_courtship_drifted(world: &World) -> usize {
        world
            .resource::<EventLog>()
            .entries
            .iter()
            .filter(|e| matches!(e.kind, EventKind::CourtshipDrifted { .. }))
            .count()
    }

    #[test]
    fn courtship_drift_emits_feature_and_event_when_gate_fires() {
        let adult_tick = 50 + 20_000 * 12;
        let (mut world, mut schedule) = bond_test_world(adult_tick);
        world.insert_resource(EventLog::default());
        let a = spawn_adult(&mut world, "Fern", Gender::Queen, Orientation::Straight);
        let b = spawn_adult(&mut world, "Reed", Gender::Tom, Orientation::Straight);

        let mut rels = Relationships::default();
        let rel = rels.get_or_insert(a, b);
        rel.fondness = 0.5;
        rel.familiarity = 0.5;
        rel.romantic = 0.0;
        world.insert_resource(rels);

        schedule.run(&mut world);

        let activation = world.resource::<SystemActivation>();
        assert_eq!(
            activation
                .counts
                .get(&Feature::CourtshipInteraction)
                .copied()
                .unwrap_or(0),
            1,
            "drift gate should record exactly one CourtshipInteraction this tick"
        );
        assert_eq!(
            count_courtship_drifted(&world),
            1,
            "drift gate should push exactly one CourtshipDrifted event this tick"
        );
        let log = world.resource::<EventLog>();
        assert_eq!(
            log.continuity_tallies
                .get("courtship")
                .copied()
                .unwrap_or(0),
            1,
            "CourtshipDrifted should bump continuity_tallies.courtship"
        );
    }

    #[test]
    fn courtship_drift_emits_nothing_for_incompatible_orientation() {
        let adult_tick = 50 + 20_000 * 12;
        let (mut world, mut schedule) = bond_test_world(adult_tick);
        world.insert_resource(EventLog::default());
        // Two straight Toms — orientation-incompatible.
        let a = spawn_adult(&mut world, "Flint", Gender::Tom, Orientation::Straight);
        let b = spawn_adult(&mut world, "Reed", Gender::Tom, Orientation::Straight);

        let mut rels = Relationships::default();
        let rel = rels.get_or_insert(a, b);
        rel.fondness = 0.5;
        rel.familiarity = 0.5;
        rel.romantic = 0.0;
        world.insert_resource(rels);

        schedule.run(&mut world);

        let activation = world.resource::<SystemActivation>();
        assert_eq!(
            activation
                .counts
                .get(&Feature::CourtshipInteraction)
                .copied()
                .unwrap_or(0),
            0,
            "incompatible orientation should not record CourtshipInteraction"
        );
        assert_eq!(
            count_courtship_drifted(&world),
            0,
            "incompatible orientation should not push CourtshipDrifted"
        );
    }

    #[test]
    fn courtship_drift_emits_nothing_below_gates() {
        let adult_tick = 50 + 20_000 * 12;
        let (mut world, mut schedule) = bond_test_world(adult_tick);
        world.insert_resource(EventLog::default());
        let a = spawn_adult(&mut world, "Fern", Gender::Queen, Orientation::Straight);
        let b = spawn_adult(&mut world, "Reed", Gender::Tom, Orientation::Straight);

        let mut rels = Relationships::default();
        let rel = rels.get_or_insert(a, b);
        // Below courtship_fondness_gate (0.3) and courtship_familiarity_gate (0.4).
        rel.fondness = 0.1;
        rel.familiarity = 0.1;
        rel.romantic = 0.0;
        world.insert_resource(rels);

        schedule.run(&mut world);

        let activation = world.resource::<SystemActivation>();
        assert_eq!(
            activation
                .counts
                .get(&Feature::CourtshipInteraction)
                .copied()
                .unwrap_or(0),
            0,
            "below-gate fondness/familiarity should not record CourtshipInteraction"
        );
        assert_eq!(
            count_courtship_drifted(&world),
            0,
            "below-gate fondness/familiarity should not push CourtshipDrifted"
        );
    }

    // -----------------------------------------------------------------------
    // §9.2 / ticket 049 befriend_wildlife author tests
    // -----------------------------------------------------------------------

    fn befriend_test_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(Relationships::default());
        world.insert_resource(SimConstants::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(befriend_wildlife);
        (world, schedule)
    }

    fn spawn_test_cat(world: &mut World) -> Entity {
        world
            .spawn((crate::components::identity::Species, Position::new(0, 0)))
            .id()
    }

    fn spawn_test_fox(world: &mut World) -> Entity {
        world
            .spawn((
                crate::components::wildlife::WildAnimal::new(
                    crate::components::wildlife::WildSpecies::Fox,
                ),
                Position::new(0, 0),
            ))
            .id()
    }

    #[test]
    fn befriend_inserts_marker_on_cat_and_fox_when_familiarity_crosses_threshold() {
        let (mut world, mut schedule) = befriend_test_world();
        let cat = spawn_test_cat(&mut world);
        let fox = spawn_test_fox(&mut world);
        // Threshold is 0.6; push familiarity to 0.7.
        world
            .resource_mut::<Relationships>()
            .modify_familiarity(cat, fox, 0.7);
        schedule.run(&mut world);
        assert!(
            world
                .get::<crate::components::markers::BefriendedAlly>(cat)
                .is_some(),
            "cat should carry BefriendedAlly after familiarity crosses 0.6"
        );
        assert!(
            world
                .get::<crate::components::markers::BefriendedAlly>(fox)
                .is_some(),
            "fox should carry BefriendedAlly reciprocally"
        );
    }

    #[test]
    fn befriend_omits_marker_below_threshold() {
        let (mut world, mut schedule) = befriend_test_world();
        let cat = spawn_test_cat(&mut world);
        let fox = spawn_test_fox(&mut world);
        // Familiarity 0.4 — below the 0.6 upgrade threshold.
        world
            .resource_mut::<Relationships>()
            .modify_familiarity(cat, fox, 0.4);
        schedule.run(&mut world);
        assert!(world
            .get::<crate::components::markers::BefriendedAlly>(cat)
            .is_none());
        assert!(world
            .get::<crate::components::markers::BefriendedAlly>(fox)
            .is_none());
    }

    #[test]
    fn befriend_hysteresis_keeps_marker_until_below_band() {
        // Threshold 0.6, hysteresis 0.1 → downgrade at 0.5.
        let (mut world, mut schedule) = befriend_test_world();
        let cat = spawn_test_cat(&mut world);
        let fox = spawn_test_fox(&mut world);

        // Cross threshold → marker on.
        world
            .resource_mut::<Relationships>()
            .modify_familiarity(cat, fox, 0.7);
        schedule.run(&mut world);
        assert!(world
            .get::<crate::components::markers::BefriendedAlly>(cat)
            .is_some());

        // Decay to 0.55 — still above downgrade band, marker persists.
        let mut rels = world.resource_mut::<Relationships>();
        rels.get_or_insert(cat, fox).familiarity = 0.55;
        schedule.run(&mut world);
        assert!(
            world
                .get::<crate::components::markers::BefriendedAlly>(cat)
                .is_some(),
            "marker should persist within hysteresis band"
        );

        // Drop below downgrade — marker comes off.
        let mut rels = world.resource_mut::<Relationships>();
        rels.get_or_insert(cat, fox).familiarity = 0.4;
        schedule.run(&mut world);
        assert!(
            world
                .get::<crate::components::markers::BefriendedAlly>(cat)
                .is_none(),
            "marker should clear once familiarity drops below threshold-hysteresis"
        );
    }

    #[test]
    fn befriend_marker_stable_when_no_wildlife_present() {
        let (mut world, mut schedule) = befriend_test_world();
        let cat = spawn_test_cat(&mut world);
        // No wildlife in the world — author runs as a no-op.
        schedule.run(&mut world);
        assert!(world
            .get::<crate::components::markers::BefriendedAlly>(cat)
            .is_none());
    }

    #[test]
    fn befriend_marker_stable_when_no_familiarity_written() {
        let (mut world, mut schedule) = befriend_test_world();
        let _cat = spawn_test_cat(&mut world);
        let _fox = spawn_test_fox(&mut world);
        // Relationships unwritten — fam defaults to 0.0, no markers.
        schedule.run(&mut world);
        let mut q = world.query_filtered::<
            Entity,
            With<crate::components::markers::BefriendedAlly>,
        >();
        assert_eq!(q.iter(&world).count(), 0);
    }

    #[test]
    fn befriend_per_entity_max_familiarity_promotes_a_cat_with_any_partner() {
        // A cat with one wildlife partner above threshold and another
        // below — the cat carries the marker (per-entity max is taken).
        let (mut world, mut schedule) = befriend_test_world();
        let cat = spawn_test_cat(&mut world);
        let fox_a = spawn_test_fox(&mut world);
        let fox_b = spawn_test_fox(&mut world);
        {
            let mut rels = world.resource_mut::<Relationships>();
            rels.modify_familiarity(cat, fox_a, 0.4);
            rels.modify_familiarity(cat, fox_b, 0.7);
        }
        schedule.run(&mut world);
        assert!(world
            .get::<crate::components::markers::BefriendedAlly>(cat)
            .is_some());
        assert!(world
            .get::<crate::components::markers::BefriendedAlly>(fox_b)
            .is_some());
        assert!(
            world
                .get::<crate::components::markers::BefriendedAlly>(fox_a)
                .is_none(),
            "fox_a's max familiarity (0.4) is below threshold; should not carry marker"
        );
    }
}
