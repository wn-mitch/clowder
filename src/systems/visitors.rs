//! §9.2 Visitor / HostileVisitor authoring.
//!
//! These markers are *authoritative-on-arrival*: per `docs/systems/trade.md`,
//! the trade subsystem (Aspirational) will spawn non-colony cats with
//! `Visitor` or `HostileVisitor` already attached, and despawn them when
//! they depart. There is no per-tick author system to derive these markers
//! from other state — the marker IS the state.
//!
//! This module exists today as a `#[cfg(test)]` spawn shim so the §9.3
//! consumer wiring (faction-stance prefilter) and the `MarkerSnapshot`
//! population path can be exercised end-to-end before trade lands. When
//! the trade subsystem ships, a real `arrive_visitor` / `depart_visitor`
//! system will replace this module's responsibilities.

#[cfg(test)]
pub mod test_helpers {
    use bevy_ecs::prelude::*;

    use crate::components::identity::Species;
    use crate::components::markers::{HostileVisitor, Visitor};
    use crate::components::physical::Position;

    /// Spawn a non-colony cat with the appropriate §9.2 visitor marker.
    /// The minimal bundle is `(Species, Position, marker)` — enough for
    /// stance-prefilter and snapshot-population tests to find the entity
    /// and read the marker presence. A real trade-subsystem spawner
    /// will attach the full cat bundle.
    pub fn spawn_visitor_cat(world: &mut World, pos: Position, hostile: bool) -> Entity {
        let mut e = world.spawn((Species, pos));
        if hostile {
            e.insert(HostileVisitor);
        } else {
            e.insert(Visitor);
        }
        e.id()
    }
}

#[cfg(test)]
mod tests {
    use bevy_ecs::prelude::*;

    use super::test_helpers::spawn_visitor_cat;
    use crate::ai::faction::{resolve_stance, FactionStance, StanceOverlays};
    use crate::components::markers::{BefriendedAlly, Banished, HostileVisitor, Visitor};
    use crate::components::physical::Position;

    #[test]
    fn shim_inserts_visitor_marker() {
        let mut world = World::new();
        let e = spawn_visitor_cat(&mut world, Position::new(0, 0), false);
        assert!(world.get::<Visitor>(e).is_some());
        assert!(world.get::<HostileVisitor>(e).is_none());
    }

    #[test]
    fn shim_inserts_hostile_visitor_marker() {
        let mut world = World::new();
        let e = spawn_visitor_cat(&mut world, Position::new(0, 0), true);
        assert!(world.get::<HostileVisitor>(e).is_some());
        assert!(world.get::<Visitor>(e).is_none());
    }

    #[test]
    fn visitor_overlay_demotes_same_to_neutral() {
        // Cat → Visitor cat: base Same → Neutral via §9.2 overlay.
        let resolved = resolve_stance(
            FactionStance::Same,
            true,
            StanceOverlays {
                visitor: true,
                ..Default::default()
            },
        );
        assert_eq!(resolved, FactionStance::Neutral);
    }

    #[test]
    fn hostile_visitor_overlay_demotes_same_to_enemy() {
        let resolved = resolve_stance(
            FactionStance::Same,
            true,
            StanceOverlays {
                hostile_visitor: true,
                ..Default::default()
            },
        );
        assert_eq!(resolved, FactionStance::Enemy);
    }

    #[test]
    fn visitor_and_hostile_coexistence_picks_most_negative() {
        // Both markers set: most-negative wins, so Enemy beats Neutral.
        let resolved = resolve_stance(
            FactionStance::Same,
            true,
            StanceOverlays {
                visitor: true,
                hostile_visitor: true,
                ..Default::default()
            },
        );
        assert_eq!(resolved, FactionStance::Enemy);
    }

    #[test]
    fn no_overlay_returns_base_stance() {
        // Sanity: with all overlay flags false, base stance is preserved.
        for base in [
            FactionStance::Same,
            FactionStance::Ally,
            FactionStance::Neutral,
            FactionStance::Prey,
            FactionStance::Predator,
            FactionStance::Enemy,
        ] {
            let resolved = resolve_stance(base, true, StanceOverlays::default());
            assert_eq!(resolved, base);
        }
    }

    #[test]
    fn overlay_markers_are_independent_zsts() {
        // A single entity carrying all four §9.2 markers compiles and
        // queries cleanly — the marker types do not collide.
        let mut world = World::new();
        let e = world
            .spawn((Visitor, HostileVisitor, Banished, BefriendedAlly))
            .id();
        assert!(world.get::<Visitor>(e).is_some());
        assert!(world.get::<HostileVisitor>(e).is_some());
        assert!(world.get::<Banished>(e).is_some());
        assert!(world.get::<BefriendedAlly>(e).is_some());
    }
}
