use bevy_ecs::prelude::*;

use crate::components::building::Structure;
use crate::components::identity::{Gender, Name, Orientation};
use crate::components::physical::{Dead, Position};
use crate::resources::narrative::{NarrativeLog, NarrativeTier};
use crate::resources::relationships::{BondType, Relationships};
use crate::resources::time::TimeState;

// ---------------------------------------------------------------------------
// passive_familiarity system
// ---------------------------------------------------------------------------

/// Each tick, cats within Manhattan distance <= 2 of each other gain a small
/// amount of familiarity. Proximity naturally builds recognition over time.
#[allow(clippy::type_complexity)]
pub fn passive_familiarity(
    query: Query<(Entity, &Position), (Without<Dead>, Without<Structure>)>,
    mut relationships: ResMut<Relationships>,
) {
    let cats: Vec<(Entity, Position)> = query.iter().map(|(e, p)| (e, *p)).collect();
    for i in 0..cats.len() {
        for j in (i + 1)..cats.len() {
            if cats[i].1.manhattan_distance(&cats[j].1) <= 2 {
                relationships.modify_familiarity(cats[i].0, cats[j].0, 0.0001);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// check_bonds system
// ---------------------------------------------------------------------------

/// Periodically check all relationships and upgrade bonds when thresholds are
/// met. Emits Tier::Significant narrative on bond formation.
pub fn check_bonds(
    time: Res<TimeState>,
    mut relationships: ResMut<Relationships>,
    mut log: ResMut<NarrativeLog>,
    names: Query<&Name>,
) {
    // Only check every 50 ticks.
    if !time.tick.is_multiple_of(50) {
        return;
    }

    for ((a, b), rel) in relationships.pairs_iter_mut() {
        let old_bond = rel.bond;

        let new_bond = if rel.romantic > 0.7 && rel.fondness > 0.7 && rel.familiarity > 0.6 {
            Some(BondType::Mates)
        } else if rel.romantic > 0.5 && rel.fondness > 0.6 && rel.familiarity > 0.5 {
            Some(BondType::Partners)
        } else if rel.fondness > 0.3 && rel.familiarity > 0.4 {
            Some(BondType::Friends)
        } else {
            None
        };

        // Only upgrade bonds, never downgrade.
        if new_bond > old_bond {
            rel.bond = new_bond;
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
        Orientation::Straight => a_gender != b_gender || b_gender == Gender::Nonbinary || a_gender == Gender::Nonbinary,
        Orientation::Gay => a_gender == b_gender || b_gender == Gender::Nonbinary || a_gender == Gender::Nonbinary,
        Orientation::Bisexual => true,
        Orientation::Asexual => false,
    };
    let b_attracted = match b_orient {
        Orientation::Straight => b_gender != a_gender || a_gender == Gender::Nonbinary || b_gender == Gender::Nonbinary,
        Orientation::Gay => b_gender == a_gender || a_gender == Gender::Nonbinary || b_gender == Gender::Nonbinary,
        Orientation::Bisexual => true,
        Orientation::Asexual => false,
    };

    a_attracted && b_attracted
}

// ---------------------------------------------------------------------------
// Value compatibility
// ---------------------------------------------------------------------------

/// Compute fondness delta from comparing two cats' value axes during interaction.
/// Same-side values: +0.0002 per axis. Divergent values: -0.0001 per axis.
#[allow(clippy::too_many_arguments)]
pub fn value_compatibility_delta(
    a_loyalty: f32, a_tradition: f32, a_compassion: f32, a_pride: f32, a_independence: f32,
    b_loyalty: f32, b_tradition: f32, b_compassion: f32, b_pride: f32, b_independence: f32,
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
        let same_side = (va > 0.5 && vb > 0.5) || (va < 0.5 && vb < 0.5);
        let divergent = (va > 0.7 && vb < 0.3) || (va < 0.3 && vb > 0.7);
        if same_side {
            delta += 0.0002;
        }
        if divergent {
            delta -= 0.0001;
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
    use crate::resources::relationships::Relationships;
    use crate::resources::time::TimeState;
    use crate::resources::narrative::NarrativeLog;

    fn setup_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(Relationships::default());
        world.insert_resource(TimeState::default());
        world.insert_resource(NarrativeLog::default());
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
        world.resource_mut::<Relationships>().get_or_insert(a, b).familiarity = 0.0;

        schedule.run(&mut world);

        let fam = world.resource::<Relationships>().get(a, b).unwrap().familiarity;
        assert!(
            (fam - 0.0001).abs() < 1e-6,
            "familiarity should be ~0.0001; got {fam}"
        );
    }

    #[test]
    fn passive_familiarity_unchanged_for_distant_cats() {
        let (mut world, mut schedule) = setup_world();

        let a = world.spawn(Position::new(0, 0)).id();
        let b = world.spawn(Position::new(10, 10)).id();

        world.resource_mut::<Relationships>().get_or_insert(a, b).familiarity = 0.0;

        schedule.run(&mut world);

        let fam = world.resource::<Relationships>().get(a, b).unwrap().familiarity;
        assert_eq!(fam, 0.0, "distant cats should not gain familiarity");
    }

    #[test]
    fn value_compatibility_positive_for_aligned_values() {
        // Both cats have all values > 0.5 (same side).
        let delta = value_compatibility_delta(
            0.8, 0.7, 0.9, 0.6, 0.8,
            0.7, 0.8, 0.6, 0.9, 0.7,
        );
        assert!(delta > 0.0, "aligned values should produce positive delta; got {delta}");
        assert!(
            (delta - 0.001).abs() < 1e-6,
            "5 same-side axes should give +0.001; got {delta}"
        );
    }

    #[test]
    fn value_compatibility_negative_for_divergent_values() {
        // Cat A has high values, Cat B has low values (all divergent).
        let delta = value_compatibility_delta(
            0.8, 0.8, 0.8, 0.8, 0.8,
            0.2, 0.2, 0.2, 0.2, 0.2,
        );
        // Each axis: same_side is true (both effectively "above or below") — wait, 0.8 > 0.5 and 0.2 < 0.5, so NOT same side.
        // Each axis: divergent is true (0.8 > 0.7, 0.2 < 0.3).
        // So delta = 5 * (-0.0001) = -0.0005
        assert!(delta < 0.0, "divergent values should produce negative delta; got {delta}");
        assert!(
            (delta - (-0.0005)).abs() < 1e-6,
            "5 divergent axes should give -0.0005; got {delta}"
        );
    }

    #[test]
    fn romantic_stays_zero_for_asexual_cats() {
        assert!(
            !are_orientation_compatible(Gender::Queen, Orientation::Asexual, Gender::Tom, Orientation::Straight),
            "asexual cat should not be romantically compatible"
        );
        assert!(
            !are_orientation_compatible(Gender::Tom, Orientation::Straight, Gender::Queen, Orientation::Asexual),
            "cat paired with asexual should not be compatible"
        );
    }

    #[test]
    fn orientation_compatibility_matrix() {
        // Straight Tom + Queen → compatible
        assert!(are_orientation_compatible(Gender::Tom, Orientation::Straight, Gender::Queen, Orientation::Straight));
        // Straight Tom + Tom → NOT compatible
        assert!(!are_orientation_compatible(Gender::Tom, Orientation::Straight, Gender::Tom, Orientation::Straight));
        // Gay Tom + Tom → compatible
        assert!(are_orientation_compatible(Gender::Tom, Orientation::Gay, Gender::Tom, Orientation::Gay));
        // Gay Tom + Queen → NOT compatible
        assert!(!are_orientation_compatible(Gender::Tom, Orientation::Gay, Gender::Queen, Orientation::Gay));
        // Bisexual + any non-asexual → compatible
        assert!(are_orientation_compatible(Gender::Tom, Orientation::Bisexual, Gender::Tom, Orientation::Bisexual));
        assert!(are_orientation_compatible(Gender::Tom, Orientation::Bisexual, Gender::Queen, Orientation::Straight));
        // Nonbinary + Straight → compatible
        assert!(are_orientation_compatible(Gender::Nonbinary, Orientation::Straight, Gender::Tom, Orientation::Straight));
        // Nonbinary + Gay → compatible
        assert!(are_orientation_compatible(Gender::Nonbinary, Orientation::Gay, Gender::Tom, Orientation::Gay));
    }

    #[test]
    fn bond_forms_at_threshold() {
        let mut world = World::new();
        let mut time = TimeState::default();
        time.tick = 50; // divisible by 50
        world.insert_resource(time);
        world.insert_resource(NarrativeLog::default());
        let mut rels = Relationships::default();

        let a = world.spawn(Name("Fern".to_string())).id();
        let b = world.spawn(Name("Reed".to_string())).id();

        // Set fondness and familiarity above Friends threshold.
        let rel = rels.get_or_insert(a, b);
        rel.fondness = 0.4;
        rel.familiarity = 0.5;
        world.insert_resource(rels);

        let mut schedule = Schedule::default();
        schedule.add_systems(check_bonds);
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
}
