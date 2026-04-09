use bevy_ecs::prelude::*;

use crate::components::coordination::ActiveDirective;
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Position};
use crate::resources::relationships::Relationships;

// ---------------------------------------------------------------------------
// personality_friction system
// ---------------------------------------------------------------------------

/// Each tick, cats with incompatible extreme personality values who are near
/// each other suffer automatic fondness decay. This creates persistent
/// interpersonal tension without any explicit action.
///
/// Incompatibility rules (from personality.md):
///
/// | Cat A              | Cat B              | Condition          | Fondness/tick        |
/// |--------------------|--------------------|--------------------|----------------------|
/// | Tradition > 0.8    | Independence > 0.8 | Within 3 tiles     | -0.002 (symmetric)   |
/// | Diligence > 0.8    | Playfulness > 0.8  | Within 3 tiles     | -0.001 (symmetric)   |
/// | Loyalty > 0.8      | Independence > 0.8 | During directive    | -0.002 (one-dir)     |
/// | Ambition > 0.8     | Ambition > 0.8     | Both present        | -0.003 (symmetric)   |
pub fn personality_friction(
    cats: Query<(Entity, &Position, &Personality), Without<Dead>>,
    directives: Query<&ActiveDirective>,
    mut relationships: ResMut<Relationships>,
) {
    let snapshot: Vec<(Entity, Position, &Personality)> =
        cats.iter().map(|(e, p, pers)| (e, *p, pers)).collect();

    for i in 0..snapshot.len() {
        for j in (i + 1)..snapshot.len() {
            let (ea, pa, a) = &snapshot[i];
            let (eb, pb, b) = &snapshot[j];

            let dist = pa.manhattan_distance(pb);
            if dist > 3 {
                continue;
            }

            // Tradition vs independence (symmetric).
            if (a.tradition > 0.8 && b.independence > 0.8)
                || (b.tradition > 0.8 && a.independence > 0.8)
            {
                relationships.modify_fondness(*ea, *eb, -0.002);
            }

            // Diligence vs playfulness (symmetric).
            if (a.diligence > 0.8 && b.playfulness > 0.8)
                || (b.diligence > 0.8 && a.playfulness > 0.8)
            {
                relationships.modify_fondness(*ea, *eb, -0.001);
            }

            // Dual ambition (symmetric) — no distance filter, just both present
            // in the snapshot (i.e., both alive).
            if a.ambition > 0.8 && b.ambition > 0.8 {
                relationships.modify_fondness(*ea, *eb, -0.003);
            }

            // Loyalty vs independence during active directive (one-directional).
            // The loyal cat resents the independent cat, not vice versa.
            let either_has_directive =
                directives.get(*ea).is_ok() || directives.get(*eb).is_ok();
            if either_has_directive {
                // A is loyal, B is independent → A resents B.
                if a.loyalty > 0.8 && b.independence > 0.8 {
                    relationships.modify_fondness(*ea, *eb, -0.002);
                }
                // B is loyal, A is independent → B resents A.
                if b.loyalty > 0.8 && a.independence > 0.8 {
                    relationships.modify_fondness(*ea, *eb, -0.002);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::schedule::Schedule;
    use crate::resources::relationships::Relationships;

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

    fn setup_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(Relationships::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(personality_friction);
        (world, schedule)
    }

    #[test]
    fn tradition_vs_independence_within_3_tiles() {
        let (mut world, mut schedule) = setup_world();

        let traditional = Personality { tradition: 0.9, ..default_personality() };
        let independent = Personality { independence: 0.9, ..default_personality() };

        let a = world.spawn((Position::new(5, 5), traditional)).id();
        let b = world.spawn((Position::new(7, 5), independent)).id();

        schedule.run(&mut world);

        let fondness = world.resource::<Relationships>().get(a, b).unwrap().fondness;
        assert!(
            (fondness - (-0.002)).abs() < 1e-5,
            "tradition vs independence should cause -0.002/tick; got {fondness}"
        );
    }

    #[test]
    fn no_friction_beyond_3_tiles() {
        let (mut world, mut schedule) = setup_world();

        let traditional = Personality { tradition: 0.9, ..default_personality() };
        let independent = Personality { independence: 0.9, ..default_personality() };

        let a = world.spawn((Position::new(0, 0), traditional)).id();
        let b = world.spawn((Position::new(4, 0), independent)).id();

        schedule.run(&mut world);

        let rel = world.resource::<Relationships>().get(a, b);
        assert!(
            rel.is_none() || rel.unwrap().fondness == 0.0,
            "no friction beyond 3 tiles"
        );
    }

    #[test]
    fn diligence_vs_playfulness() {
        let (mut world, mut schedule) = setup_world();

        let diligent = Personality { diligence: 0.9, ..default_personality() };
        let playful = Personality { playfulness: 0.9, ..default_personality() };

        let a = world.spawn((Position::new(5, 5), diligent)).id();
        let b = world.spawn((Position::new(6, 5), playful)).id();

        schedule.run(&mut world);

        let fondness = world.resource::<Relationships>().get(a, b).unwrap().fondness;
        assert!(
            (fondness - (-0.001)).abs() < 1e-5,
            "diligence vs playfulness should cause -0.001/tick; got {fondness}"
        );
    }

    #[test]
    fn dual_ambition_friction() {
        let (mut world, mut schedule) = setup_world();

        let ambitious_a = Personality { ambition: 0.9, ..default_personality() };
        let ambitious_b = Personality { ambition: 0.85, ..default_personality() };

        let a = world.spawn((Position::new(5, 5), ambitious_a)).id();
        let b = world.spawn((Position::new(6, 5), ambitious_b)).id();

        schedule.run(&mut world);

        let fondness = world.resource::<Relationships>().get(a, b).unwrap().fondness;
        assert!(
            (fondness - (-0.003)).abs() < 1e-5,
            "dual ambition should cause -0.003/tick; got {fondness}"
        );
    }

    #[test]
    fn loyalty_vs_independence_during_directive() {
        let (mut world, mut schedule) = setup_world();

        let loyal = Personality { loyalty: 0.9, ..default_personality() };
        let independent = Personality { independence: 0.9, ..default_personality() };

        let coordinator = world.spawn_empty().id();
        let a = world.spawn((Position::new(5, 5), loyal)).id();
        let b = world.spawn((
            Position::new(6, 5),
            independent,
            ActiveDirective {
                coordinator,
                kind: crate::components::coordination::DirectiveKind::Hunt,
                priority: 0.5,
                coordinator_social_weight: 0.5,
                delivered_tick: 0,
            },
        )).id();

        schedule.run(&mut world);

        let fondness = world.resource::<Relationships>().get(a, b).unwrap().fondness;
        // tradition/independence check doesn't fire (both below 0.8 threshold)
        // loyalty/independence fires: -0.002
        assert!(
            (fondness - (-0.002)).abs() < 1e-5,
            "loyal cat should resent independent cat during directive; got {fondness}"
        );
    }

    #[test]
    fn no_friction_for_moderate_traits() {
        let (mut world, mut schedule) = setup_world();

        let a_pers = default_personality(); // all 0.5
        let b_pers = default_personality();

        let a = world.spawn((Position::new(5, 5), a_pers)).id();
        let b = world.spawn((Position::new(6, 5), b_pers)).id();

        schedule.run(&mut world);

        let rel = world.resource::<Relationships>().get(a, b);
        assert!(
            rel.is_none() || rel.unwrap().fondness == 0.0,
            "moderate traits should cause no friction"
        );
    }
}
