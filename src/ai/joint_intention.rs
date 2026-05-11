//! Ticket 127 — JointIntention author / mirror system. Commit A.
//!
//! # Commit A — mirror-only
//!
//! This commit ships a **minimal lockstep mirror** of
//! `crate::ai::pairing::author_pairing_intentions`: every cat with a
//! `PairingActivity` gets a `JointIntention { practice: Courtship,
//! partner: PA.partner, ... }` inserted by this system; every cat with a
//! `JointIntention` but no `PairingActivity` has its JI removed. The
//! mirror is a structural shadow of PA's lifecycle — no separate
//! matchmaker, no separate drop logic. Behavior parity is mechanical:
//! the upstream author owns the lifecycle decisions, this system just
//! propagates them to the JointIntention substrate.
//!
//! The mirror runs IMMEDIATELY AFTER `author_pairing_intentions` on the
//! same schedule chain (Bevy `.chain()` inserts `apply_deferred` between
//! the two so the mirror sees PA's post-flush state). Both authors
//! coexist; both fire their respective `Feature` counters per cat per
//! tick.
//!
//! # Commit B (not in this commit)
//!
//! Replaces the mirror with a real `author_joint_intentions` system
//! that runs its own matchmaker + drop predicate + stage progression +
//! partner-cascade detection + mismatch-tick accrual. At that point the
//! old `author_pairing_intentions` is decommissioned (Commit C deletes
//! it).
//!
//! # Why mirror-only first
//!
//! Per the "substrate over hacks" discipline: substrate axes land
//! first, the corresponding hack retires second — never the reverse.
//! Commit A makes JointIntention live as substrate without retiring
//! PairingActivity. Migration parity is mechanical because the mirror
//! is structurally a no-op on behavior: readers in Commit A still read
//! PA. The mirror's only job is to make `Has<JointIntention>` queries
//! work correctly so Commit B's reader switch is a 1:1 swap.

use bevy_ecs::prelude::*;

use crate::components::joint_intention::{JointIntention, PracticeKind};
use crate::components::pairing::PairingActivity;
use crate::components::physical::Dead;
use crate::resources::system_activation::{Feature, SystemActivation};

/// Per-tick lockstep mirror — see module docs. Idempotent: only inserts
/// JointIntention on a cat that has `PairingActivity` and lacks JI;
/// only removes JointIntention on a cat that has JI and lacks PA.
/// Re-running against a consistent state is a no-op.
///
/// Records `Feature::JointIntentionEmitted { Courtship }` on every
/// insert (paralleling the upstream
/// `Feature::PairingIntentionEmitted` emission); records
/// `Feature::JointIntentionDropped { Courtship }` on every removal
/// (paralleling `Feature::PairingDropped`).
#[allow(clippy::type_complexity)]
pub fn mirror_joint_intentions(
    mut commands: Commands,
    mut activation: ResMut<SystemActivation>,
    with_pairing: Query<(Entity, &PairingActivity, Option<&JointIntention>), Without<Dead>>,
    without_pairing_with_joint: Query<
        Entity,
        (With<JointIntention>, Without<PairingActivity>, Without<Dead>),
    >,
) {
    // Pass 1: cat has PairingActivity → ensure JointIntention exists
    // and points at the same partner. If JI is missing, insert it. If
    // JI exists but points at a different partner (e.g., the upstream
    // author dropped the old PA and adopted a new one in a single
    // tick), realign — remove the stale JI; the next mirror tick will
    // re-insert. This realignment is defensive: today's author doesn't
    // produce same-tick partner swaps, but the invariant is "JI
    // partner == PA partner" and we enforce it.
    for (entity, pairing, joint) in with_pairing.iter() {
        match joint {
            None => {
                commands.entity(entity).insert(JointIntention::new(
                    PracticeKind::Courtship,
                    pairing.partner,
                    pairing.adopted_tick,
                ));
                activation.record(Feature::JointIntentionEmitted {
                    practice: PracticeKind::Courtship,
                });
            }
            Some(j) if j.partner != pairing.partner => {
                // Partner mismatch — drop, then re-insert next tick.
                commands.entity(entity).remove::<JointIntention>();
                activation.record(Feature::JointIntentionDropped {
                    practice: PracticeKind::Courtship,
                });
            }
            Some(_) => {
                // In sync. No action.
            }
        }
    }

    // Pass 2: cat has JointIntention but PA was removed by the
    // upstream author. Mirror removes JI to match.
    for entity in without_pairing_with_joint.iter() {
        commands.entity(entity).remove::<JointIntention>();
        activation.record(Feature::JointIntentionDropped {
            practice: PracticeKind::Courtship,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::components::fertility::{Fertility, FertilityPhase};
    use crate::components::identity::{Age, Gender, Name, Orientation};
    use crate::components::mental::Mood;
    use crate::components::physical::{Health, Needs, Position};
    use crate::resources::relationships::{BondType, Relationships};
    use crate::resources::time::{SimConfig, TimeState};
    use crate::resources::SimConstants;
    use bevy_ecs::schedule::Schedule;

    /// Spawn an Adult cat with all per-cat fertility / sated-and-happy
    /// gates open. Adapted from `mating::tests::spawn_eligible_adult`.
    fn spawn_eligible_adult(
        world: &mut World,
        name: &str,
        gender: Gender,
        orientation: Orientation,
        position: Position,
    ) -> Entity {
        let mut needs = Needs::default();
        needs.hunger = 0.9;
        needs.energy = 0.9;
        needs.mating = 0.3;
        let mut mood = Mood::default();
        mood.valence = 0.5;
        let fertility = if matches!(gender, Gender::Tom) {
            None
        } else {
            Some(Fertility {
                phase: FertilityPhase::Estrus,
                cycle_offset: 0,
                post_partum_remaining_ticks: 0,
            })
        };
        let mut entity = world.spawn((
            Name(name.to_string()),
            Age { born_tick: 0 },
            gender,
            orientation,
            mood,
            needs,
            position,
            Health::default(),
        ));
        if let Some(f) = fertility {
            entity.insert(f);
        }
        entity.id()
    }

    fn mirror_world() -> World {
        let mut world = World::new();
        let mut time = TimeState::default();
        // Tick > 12 seasons of default tps → Adult life-stage.
        time.tick = 20_000 * 13;
        world.insert_resource(time);
        world.insert_resource(SimConfig::default());
        world.insert_resource(SimConstants::default());
        world.insert_resource(Relationships::default());
        world.insert_resource(SystemActivation::default());
        world
    }

    fn run_pairing_then_mirror(world: &mut World) {
        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                crate::ai::pairing::author_pairing_intentions,
                mirror_joint_intentions,
            )
                .chain(),
        );
        schedule.run(world);
    }

    #[test]
    fn mirror_inserts_joint_intention_for_each_pairing_activity() {
        let mut world = mirror_world();
        let a = spawn_eligible_adult(
            &mut world,
            "Fern",
            Gender::Queen,
            Orientation::Straight,
            Position::new(0, 0),
        );
        let b = spawn_eligible_adult(
            &mut world,
            "Reed",
            Gender::Tom,
            Orientation::Straight,
            Position::new(1, 0),
        );
        let mut rels = world.resource_mut::<Relationships>();
        let rel = rels.get_or_insert(a, b);
        rel.bond = Some(BondType::Friends);
        rel.fondness = 0.5;

        run_pairing_then_mirror(&mut world);

        // Both cats should hold PA AND JI with matching partner fields.
        for (entity, expected_partner) in [(a, b), (b, a)] {
            let pa = world
                .get::<PairingActivity>(entity)
                .expect("PA inserted by upstream author");
            assert_eq!(pa.partner, expected_partner);
            let ji = world
                .get::<JointIntention>(entity)
                .expect("JI mirrored by Commit A mirror");
            assert_eq!(ji.partner, expected_partner);
            assert_eq!(ji.practice, PracticeKind::Courtship);
        }

        let activation = world.resource::<SystemActivation>();
        assert_eq!(
            activation
                .counts
                .get(&Feature::JointIntentionEmitted {
                    practice: PracticeKind::Courtship,
                })
                .copied(),
            Some(2),
            "symmetric pair → both cats emit one JI each"
        );
    }

    #[test]
    fn mirror_drops_joint_intention_when_pairing_dropped() {
        let mut world = mirror_world();
        let a = spawn_eligible_adult(
            &mut world,
            "Fern",
            Gender::Queen,
            Orientation::Straight,
            Position::new(0, 0),
        );
        let b = spawn_eligible_adult(
            &mut world,
            "Reed",
            Gender::Tom,
            Orientation::Straight,
            Position::new(1, 0),
        );
        let mut rels = world.resource_mut::<Relationships>();
        let rel = rels.get_or_insert(a, b);
        rel.bond = Some(BondType::Friends);
        rel.fondness = 0.5;

        run_pairing_then_mirror(&mut world);
        assert!(world.get::<JointIntention>(a).is_some());

        // Kill partner → upstream author drops PA next tick → mirror
        // drops JI in the same tick (post-PA flush).
        world.entity_mut(b).insert(Dead {
            tick: 0,
            cause: crate::components::physical::DeathCause::OldAge,
        });
        run_pairing_then_mirror(&mut world);

        assert!(
            world.get::<JointIntention>(a).is_none(),
            "JI dropped in lockstep with PA when partner becomes Dead"
        );
        let activation = world.resource::<SystemActivation>();
        assert!(
            activation
                .counts
                .get(&Feature::JointIntentionDropped {
                    practice: PracticeKind::Courtship,
                })
                .copied()
                .unwrap_or(0)
                >= 1,
            "JointIntentionDropped fires on the drop transition"
        );
    }

    #[test]
    fn mirror_is_idempotent_when_state_already_consistent() {
        let mut world = mirror_world();
        let a = spawn_eligible_adult(
            &mut world,
            "Fern",
            Gender::Queen,
            Orientation::Straight,
            Position::new(0, 0),
        );
        let b = spawn_eligible_adult(
            &mut world,
            "Reed",
            Gender::Tom,
            Orientation::Straight,
            Position::new(1, 0),
        );
        let mut rels = world.resource_mut::<Relationships>();
        let rel = rels.get_or_insert(a, b);
        rel.bond = Some(BondType::Friends);
        rel.fondness = 0.5;

        // Tick 1: PA + JI emitted.
        run_pairing_then_mirror(&mut world);
        // Ticks 2-4: state is consistent; mirror should be a no-op.
        run_pairing_then_mirror(&mut world);
        run_pairing_then_mirror(&mut world);
        run_pairing_then_mirror(&mut world);

        let count = world
            .resource::<SystemActivation>()
            .counts
            .get(&Feature::JointIntentionEmitted {
                practice: PracticeKind::Courtship,
            })
            .copied()
            .unwrap_or(0);
        assert_eq!(
            count, 2,
            "tick 1 emits two JIs (symmetric pair); ticks 2-4 are no-ops"
        );
    }
}
