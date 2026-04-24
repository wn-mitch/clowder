//! §7.W Fulfillment register — per-tick systems for decay and passive
//! restoration of fulfillment axes.
//!
//! Two systems:
//! - `decay_fulfillment` — base drain + isolation-accelerated decay
//! - `bond_proximity_social_warmth` — passive gain near bonded companions
//!
//! Active inflow (from grooming, socializing) lives in the step resolvers,
//! not here.

use bevy_ecs::prelude::*;

use crate::components::fulfillment::Fulfillment;
use crate::components::physical::{Dead, Position};
use crate::resources::relationships::Relationships;
use crate::resources::sim_constants::SimConstants;

// ---------------------------------------------------------------------------
// decay_fulfillment
// ---------------------------------------------------------------------------

/// Per-tick drain on fulfillment axes. Social warmth decays continuously,
/// faster when no bonded companion is nearby (isolation multiplier).
pub fn decay_fulfillment(
    mut query: Query<(Entity, &Position, &mut Fulfillment), Without<Dead>>,
    relationships: Res<Relationships>,
    constants: Res<SimConstants>,
) {
    let fc = &constants.fulfillment;

    // Read pass: snapshot positions for the isolation proximity check.
    let snapshot: Vec<(Entity, Position)> = query.iter().map(|(e, p, _)| (e, *p)).collect();

    // Write pass: decay each cat's fulfillment.
    for (entity, pos, mut fulfillment) in &mut query {
        // Isolation check: is any bonded companion within range?
        let has_nearby_bond = snapshot.iter().any(|&(other, other_pos)| {
            if other == entity {
                return false;
            }
            let dist = pos.manhattan_distance(&other_pos);
            if dist == 0 || dist > fc.social_warmth_isolation_range {
                return false;
            }
            relationships
                .get(entity, other)
                .is_some_and(|r| r.bond.is_some())
        });

        let decay = if has_nearby_bond {
            fc.social_warmth_base_decay
        } else {
            fc.social_warmth_base_decay * fc.social_warmth_isolation_multiplier
        };

        fulfillment.social_warmth = (fulfillment.social_warmth - decay).max(0.0);
    }
}

// ---------------------------------------------------------------------------
// bond_proximity_social_warmth
// ---------------------------------------------------------------------------

/// Cats within range of a bonded companion get a small per-tick
/// social_warmth restoration — being around friends satisfies the
/// affective axis even without active grooming or socializing.
///
/// Mirrors `needs::bond_proximity_social` but targets the Fulfillment
/// register instead of `Needs.social`.
pub fn bond_proximity_social_warmth(
    mut query: Query<(Entity, &Position, &mut Fulfillment), Without<Dead>>,
    relationships: Res<Relationships>,
    constants: Res<SimConstants>,
) {
    let fc = &constants.fulfillment;

    // Read pass: snapshot positions.
    let snapshot: Vec<(Entity, Position)> = query.iter().map(|(e, p, _)| (e, *p)).collect();

    // Write pass: boost social_warmth for cats near bonded companions.
    for (entity, pos, mut fulfillment) in &mut query {
        let has_nearby_bond = snapshot.iter().any(|&(other, other_pos)| {
            if other == entity {
                return false;
            }
            let dist = pos.manhattan_distance(&other_pos);
            if dist == 0 || dist > fc.social_warmth_bond_proximity_range {
                return false;
            }
            relationships
                .get(entity, other)
                .is_some_and(|r| r.bond.is_some())
        });

        if has_nearby_bond {
            fulfillment.social_warmth =
                (fulfillment.social_warmth + fc.social_warmth_bond_proximity_rate).min(1.0);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::*;

    fn test_constants() -> SimConstants {
        SimConstants::default()
    }

    fn spawn_cat(world: &mut World, x: i32, y: i32) -> Entity {
        world
            .spawn((
                Position::new(x, y),
                Fulfillment::default(),
            ))
            .id()
    }

    #[test]
    fn decay_reduces_social_warmth() {
        let mut world = World::default();
        world.insert_resource(test_constants());
        world.insert_resource(Relationships::default());

        let cat = spawn_cat(&mut world, 5, 5);
        let initial = world.get::<Fulfillment>(cat).unwrap().social_warmth;

        // Run decay 100 times.
        let mut schedule = Schedule::default();
        schedule.add_systems(decay_fulfillment);
        for _ in 0..100 {
            schedule.run(&mut world);
        }

        let after = world.get::<Fulfillment>(cat).unwrap().social_warmth;
        assert!(
            after < initial,
            "social_warmth should decay: initial={initial}, after={after}"
        );
    }

    #[test]
    fn isolation_accelerates_decay() {
        let constants = test_constants();
        let fc = &constants.fulfillment;

        // Isolated cat: no bonds, decays at isolation rate.
        let mut world_isolated = World::default();
        world_isolated.insert_resource(constants.clone());
        world_isolated.insert_resource(Relationships::default());
        let isolated_cat = spawn_cat(&mut world_isolated, 5, 5);

        // Bonded cat: bond partner nearby, decays at base rate.
        let mut world_bonded = World::default();
        world_bonded.insert_resource(constants.clone());
        let mut rels = Relationships::default();
        let bonded_cat = spawn_cat(&mut world_bonded, 5, 5);
        let companion = spawn_cat(&mut world_bonded, 6, 5); // distance 1
        let rel = rels.get_or_insert(bonded_cat, companion);
        rel.bond = Some(crate::resources::relationships::BondType::Friends);
        world_bonded.insert_resource(rels);

        // Run decay once on each (separate schedules — Bevy binds to first world).
        let mut sched_isolated = Schedule::default();
        sched_isolated.add_systems(decay_fulfillment);
        sched_isolated.run(&mut world_isolated);

        let mut sched_bonded = Schedule::default();
        sched_bonded.add_systems(decay_fulfillment);
        sched_bonded.run(&mut world_bonded);

        let isolated_after = world_isolated
            .get::<Fulfillment>(isolated_cat)
            .unwrap()
            .social_warmth;
        let bonded_after = world_bonded
            .get::<Fulfillment>(bonded_cat)
            .unwrap()
            .social_warmth;

        // Isolated cat should have decayed MORE than bonded cat.
        let isolated_loss = 0.6 - isolated_after;
        let bonded_loss = 0.6 - bonded_after;
        assert!(
            isolated_loss > bonded_loss,
            "isolated decay ({isolated_loss}) should exceed bonded decay ({bonded_loss})"
        );
        // Verify the multiplier is approximately correct.
        let ratio = isolated_loss / bonded_loss;
        assert!(
            (ratio - fc.social_warmth_isolation_multiplier).abs() < 0.01,
            "decay ratio ({ratio}) should match isolation multiplier ({})",
            fc.social_warmth_isolation_multiplier
        );
    }

    #[test]
    fn bond_proximity_restores_social_warmth() {
        let constants = test_constants();
        let mut world = World::default();
        world.insert_resource(constants);
        let mut rels = Relationships::default();

        let cat = spawn_cat(&mut world, 5, 5);
        let companion = spawn_cat(&mut world, 6, 5);
        let rel = rels.get_or_insert(cat, companion);
        rel.bond = Some(crate::resources::relationships::BondType::Friends);
        world.insert_resource(rels);

        // Lower social_warmth first.
        world.get_mut::<Fulfillment>(cat).unwrap().social_warmth = 0.3;

        let mut schedule = Schedule::default();
        schedule.add_systems(bond_proximity_social_warmth);
        schedule.run(&mut world);

        let after = world.get::<Fulfillment>(cat).unwrap().social_warmth;
        assert!(
            after > 0.3,
            "social_warmth should rise from bond proximity: after={after}"
        );
    }

    #[test]
    fn social_warmth_floors_at_zero() {
        let mut world = World::default();
        world.insert_resource(test_constants());
        world.insert_resource(Relationships::default());

        let cat = spawn_cat(&mut world, 5, 5);
        world.get_mut::<Fulfillment>(cat).unwrap().social_warmth = 0.0001;

        let mut schedule = Schedule::default();
        schedule.add_systems(decay_fulfillment);
        for _ in 0..100 {
            schedule.run(&mut world);
        }

        let after = world.get::<Fulfillment>(cat).unwrap().social_warmth;
        assert!(
            after >= 0.0,
            "social_warmth should not go below zero: after={after}"
        );
    }
}
