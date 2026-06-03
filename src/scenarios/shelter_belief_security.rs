//! 374 — `ShelterBeliefs` substrate first-light + four-phase lifecycle.
//!
//! Verifies the per-cat housing-security belief substrate fires
//! end-to-end at scenario scale:
//!
//! 1. **Claim** — a cat spawned adjacent to a functional Den picks it
//!    up as `home_den` on the first stagger pass (`claim_home_dens`),
//!    `DenClaimed` integrator arm lifts `belonging` toward 1.0.
//! 2. **Damage** — mid-test we drop the Den's `Structure::condition`
//!    below `damage_threshold_high` (0.5); the next
//!    `emit_den_condition_events` pass fires `DenDamaged`, the
//!    integrator arm pulls `quality` down toward the new condition.
//! 3. **Siege** — mid-test we spawn a fox at the Den's center;
//!    `detect_den_sieges` fires `DenSieged` on the 0→positive
//!    transition, integrator lifts `threat` toward 1.0.
//! 4. **Siege broken** — mid-test we despawn the fox;
//!    `detect_den_sieges` fires `DenSiegeBroken` on positive→0,
//!    integrator pulls `threat` back toward 0.0.
//!
//! The scenario doesn't reach a DSE-decision consumer — `welfare.shelter`
//! and `pressure.shelter` are the only consumers and they read the
//! population aggregate, not the focal cat's facet. The four phases
//! verify the substrate-firing path tick-by-tick rather than asserting
//! population-scale behavior.

use bevy_ecs::world::World;

use crate::components::building::{Structure, StructureType};
use crate::components::physical::Position;

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "shelter_belief_security",
    default_focal: "Hearthkeeper",
    default_ticks: 20,
    setup,
    // Substrate first-light scenario — no Feature canaries to assert
    // (Phase A/B/C don't emit Features; the lifecycle test verifies
    // sub-axis movement directly).
    expected_features: &[],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Pre-place a functional Den at (15, 20). The Hearthkeeper spawns
    // adjacent at (16, 20) — well inside CLAIM_SEARCH_RADIUS (40 tiles)
    // and inside home_den_radius (default 4 tiles) for continuity
    // accrual on the very first stagger.
    world.spawn((Structure::new(StructureType::Den), Position::new(15, 20)));

    spawn_cat(
        world,
        CatPreset::adult("Hearthkeeper", Position::new(16, 20)).with_marker(MarkerKind::Adult),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    use bevy::app::App;
    use bevy_ecs::entity::Entity;

    use crate::components::beliefs::ShelterBeliefs;
    use crate::components::physical::Health;
    use crate::components::wildlife::{WildAnimal, WildSpecies};
    use crate::scenarios::runner::build_scenario_app;

    fn find_cat_by_name(world: &mut World, name: &str) -> Entity {
        let mut q = world.query::<(Entity, &crate::components::identity::Name)>();
        for (entity, n) in q.iter(world) {
            if n.0.as_str() == name {
                return entity;
            }
        }
        panic!("no cat named {name}");
    }

    /// Step `n` simulation ticks against the running app.
    fn step(app: &mut App, n: usize) {
        for _ in 0..n {
            app.update();
        }
    }

    /// 374 first-light: the four-phase lifecycle exercises every emit
    /// path (claim, damage, siege, siege broken) and asserts each
    /// sub-axis responds as documented in `ShelterFacet`.
    ///
    /// Tick budgets are generous (multiple stagger periods per phase)
    /// to absorb the entity-index-staggered claim cadence — a single
    /// stagger period (default 20 ticks) guarantees the focal cat's
    /// phase is hit, but the belief integrator's EMA needs several
    /// observations to reach the assertion thresholds chosen here.
    #[test]
    fn four_phase_lifecycle() {
        let mut app = build_scenario_app(42, &SCENARIO, "Hearthkeeper");
        // First update runs Startup (scenario setup).
        app.update();
        let focal = find_cat_by_name(app.world_mut(), "Hearthkeeper");

        // ---- Phase 1 — Claim ----------------------------------------
        // 40 ticks ≥ 2 stagger periods → claim_home_dens has fired at
        // least once for the focal cat, and the DenClaimed integrator
        // arm has lifted belonging from 0.
        step(&mut app, 40);
        let shelter = app
            .world()
            .get::<ShelterBeliefs>(focal)
            .expect("focal carries ShelterBeliefs");
        let den = shelter.home_den.expect(
            "Phase 1: focal should have claimed the pre-placed Den \
             within 40 ticks",
        );
        assert!(
            shelter.facet.belonging > 0.5,
            "Phase 1: belonging should lift on DenClaimed; got {}",
            shelter.facet.belonging
        );

        // ---- Phase 2 — Damage ---------------------------------------
        // Drop condition from 1.0 to 0.3 (crosses the 0.5 knee
        // downward). Next emit_den_condition_events stagger fires
        // DenDamaged → quality EMAs toward 0.3.
        {
            let mut s = app
                .world_mut()
                .get_mut::<Structure>(den)
                .expect("den structure exists");
            s.condition = 0.3;
        }
        step(&mut app, 60);
        let shelter = app
            .world()
            .get::<ShelterBeliefs>(focal)
            .expect("focal still carries ShelterBeliefs");
        assert!(
            shelter.facet.quality < 0.7,
            "Phase 2: quality should drop after DenDamaged; got {}",
            shelter.facet.quality
        );

        // ---- Phase 3 — Siege ----------------------------------------
        // Spawn a fox at the den's center; detect_den_sieges fires
        // DenSieged on the 0→1 fox-count transition; threat EMAs up.
        let fox = app
            .world_mut()
            .spawn((
                WildAnimal::new(WildSpecies::Fox),
                Position::new(15, 20),
                Health::default(),
            ))
            .id();
        step(&mut app, 60);
        let shelter = app
            .world()
            .get::<ShelterBeliefs>(focal)
            .expect("focal still carries ShelterBeliefs");
        let threat_at_siege = shelter.facet.threat;
        assert!(
            threat_at_siege > 0.1,
            "Phase 3: threat should lift after DenSieged; got {threat_at_siege}"
        );

        // ---- Phase 4 — Siege broken ---------------------------------
        // Despawn the fox; detect_den_sieges fires DenSiegeBroken on
        // the positive→0 transition; threat decays back.
        app.world_mut().despawn(fox);
        step(&mut app, 60);
        let shelter = app
            .world()
            .get::<ShelterBeliefs>(focal)
            .expect("focal still carries ShelterBeliefs");
        assert!(
            shelter.facet.threat < threat_at_siege,
            "Phase 4: threat should decay after DenSiegeBroken; \
             was {threat_at_siege}, now {}",
            shelter.facet.threat
        );
    }
}
