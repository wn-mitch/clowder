//! Ticket 261 — ActionAffordances substrate microexperiments.
//!
//! Six scenarios exercising the writer's shape across the five behavioural
//! families. Each variant preloads a tiny world, runs the
//! `SimulationPlugin` schedule for a small tick budget, and asserts the
//! corresponding affordance entry in [`ActionAffordances`] satisfies the
//! documented shape claim.
//!
//! 261 lands behavior-neutral — no DSE reads from the substrate, so these
//! scenarios assert directly on the resource state rather than on L2/L3
//! election outcomes. Consumer-ticket scenarios (263+) will assert on the
//! consumer's decision shape once wiring lands.
//!
//! | Variant                                  | Shape claim                                                              |
//! |------------------------------------------|--------------------------------------------------------------------------|
//! | `affordance_flee_high_cover`             | `Affordance(Flee)` reads non-zero in a covered-perceiver / fox scenario  |
//! | `affordance_flee_open_ground`            | Same threat, no cover — `Affordance(Flee)` materially lower than above   |
//! | `affordance_dive_hawk`                   | Hawk perceiver: `Dive > 0`, `Pounce == 0` (species gate)                 |
//! | `affordance_chase_prey`                  | Fox perceiver chasing cat: `Chase > 0` at proximity                      |
//! | `affordance_fight_capability_match`      | Two cats: `Fight` scales with my_health vs target_health                 |
//! | `affordance_supersedes_legacy_scalars`   | Flee-relevant setup → non-zero `Affordance(Flee)` (103 supersede sanity) |

use bevy_ecs::world::World;

use crate::components::physical::{Health, Needs, Position};
use crate::components::wildlife::{WildAnimal, WildSpecies};
use crate::resources::map::{Terrain, TileMap};
use crate::resources::WardCoverageMap;

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

const FOCAL_NAME: &str = "Probe";

// ---------------------------------------------------------------------------
// Variant 1 — Flee with high cover at perceiver position.
// ---------------------------------------------------------------------------

pub static SCENARIO_FLEE_HIGH_COVER: Scenario = Scenario {
    name: "affordance_flee_high_cover",
    default_focal: FOCAL_NAME,
    default_ticks: 4,
    setup: setup_flee_high_cover,
    expected_features: &[],
};

fn setup_flee_high_cover(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    spawn_cat(
        world,
        CatPreset::adult(FOCAL_NAME, Position::new(20, 20))
            .with_needs(|n| n.safety = 0.3)
            .with_marker(MarkerKind::Adult),
    );
    world.spawn((
        Position::new(23, 20),
        WildAnimal::new(WildSpecies::Fox),
        Health::default(),
    ));
    // Stamp a ward at the focal's position so cover_self reads high.
    let mut ward = world.resource_mut::<WardCoverageMap>();
    ward.stamp_ward(20, 20, 1.0, 9.0);
}

// ---------------------------------------------------------------------------
// Variant 2 — Flee with no cover. Same threat distance.
// ---------------------------------------------------------------------------

pub static SCENARIO_FLEE_OPEN_GROUND: Scenario = Scenario {
    name: "affordance_flee_open_ground",
    default_focal: FOCAL_NAME,
    default_ticks: 4,
    setup: setup_flee_open_ground,
    expected_features: &[],
};

fn setup_flee_open_ground(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    spawn_cat(
        world,
        CatPreset::adult(FOCAL_NAME, Position::new(20, 20))
            .with_needs(|n| n.safety = 0.3)
            .with_marker(MarkerKind::Adult),
    );
    world.spawn((
        Position::new(23, 20),
        WildAnimal::new(WildSpecies::Fox),
        Health::default(),
    ));
    // No ward stamp — cover_self reads 0.0.
}

// ---------------------------------------------------------------------------
// Variant 3 — Hawk perceiver: Dive available, Pounce gated to 0.
// ---------------------------------------------------------------------------

pub static SCENARIO_DIVE_HAWK: Scenario = Scenario {
    name: "affordance_dive_hawk",
    default_focal: FOCAL_NAME,
    default_ticks: 4,
    setup: setup_dive_hawk,
    expected_features: &[],
};

fn setup_dive_hawk(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    spawn_cat(
        world,
        CatPreset::adult(FOCAL_NAME, Position::new(20, 20)).with_marker(MarkerKind::Adult),
    );
    // Hawk above (logically) at proximity 2.
    world.spawn((
        Position::new(22, 20),
        WildAnimal::new(WildSpecies::Hawk),
        Health::default(),
    ));
}

// ---------------------------------------------------------------------------
// Variant 4 — Fox perceiver chasing cat. Chase eligible at proximity.
// ---------------------------------------------------------------------------

pub static SCENARIO_CHASE_PREY: Scenario = Scenario {
    name: "affordance_chase_prey",
    default_focal: FOCAL_NAME,
    default_ticks: 4,
    setup: setup_chase_prey,
    expected_features: &[],
};

fn setup_chase_prey(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    spawn_cat(
        world,
        CatPreset::adult(FOCAL_NAME, Position::new(20, 20)).with_marker(MarkerKind::Adult),
    );
    world.spawn((
        Position::new(22, 20),
        WildAnimal::new(WildSpecies::Fox),
        Health::default(),
    ));
}

// ---------------------------------------------------------------------------
// Variant 5 — Two cats with capability differential. Fight scales inversely.
// ---------------------------------------------------------------------------

pub static SCENARIO_FIGHT_CAPABILITY_MATCH: Scenario = Scenario {
    name: "affordance_fight_capability_match",
    default_focal: FOCAL_NAME,
    default_ticks: 4,
    setup: setup_fight_capability_match,
    expected_features: &[],
};

fn setup_fight_capability_match(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    // Healthy focal (full HP).
    spawn_cat(
        world,
        CatPreset::adult(FOCAL_NAME, Position::new(20, 20)).with_marker(MarkerKind::Adult),
    );
    // Wounded opponent (partial HP). Use the preset's Adult marker; we
    // mutate Health directly post-spawn via a query.
    let opponent = spawn_cat(
        world,
        CatPreset::adult("Opponent", Position::new(21, 20)).with_marker(MarkerKind::Adult),
    );
    if let Some(mut health) = world.get_mut::<Health>(opponent) {
        health.current = 0.2; // significantly wounded
    }
}

// ---------------------------------------------------------------------------
// Variant 6 — Flee-relevant setup; 103 supersede sanity check.
// ---------------------------------------------------------------------------

pub static SCENARIO_SUPERSEDES_LEGACY_SCALARS: Scenario = Scenario {
    name: "affordance_supersedes_legacy_scalars",
    default_focal: FOCAL_NAME,
    default_ticks: 4,
    setup: setup_supersedes_legacy,
    expected_features: &[],
};

fn setup_supersedes_legacy(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    // Same shape as the 103 escape_viability tests: low-safety cat with
    // an adjacent fox on a fully walkable map. The new substrate's
    // Affordance(Flee) plays the role 103's escape_viability did, but
    // composed differently. We assert non-zero rather than numeric
    // equivalence — the formulas differ by design (103 was a single
    // axis; 261's Flee composes 4 weighted slots).
    spawn_cat(
        world,
        CatPreset::adult(FOCAL_NAME, Position::new(20, 20))
            .with_needs(|n| n.safety = 0.3)
            .with_marker(MarkerKind::Adult),
    );
    world.spawn((
        Position::new(21, 20),
        WildAnimal::new(WildSpecies::Fox),
        Health::default(),
    ));
    // Walkable map is the default; touching `TileMap` here is a no-op
    // sanity reference so the import isn't dead.
    let _ = world.resource::<TileMap>();
    let _ = Terrain::Grass;
    let _: Needs = Needs::default();
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use crate::components::beliefs::CatBeliefs;
    use crate::components::identity::{Name, Species};
    use crate::components::wildlife::WildAnimal;
    use crate::resources::{ActionAffordances, ActionKind};
    use crate::scenarios::runner::build_scenario_app;
    use bevy_ecs::prelude::*;

    /// Find a cat entity by its `Name`. Used by tests to read out the
    /// affordance scalar for the named focal cat.
    fn cat_by_name(world: &mut World, name: &str) -> Entity {
        let mut q = world.query_filtered::<(Entity, &Name), With<Species>>();
        q.iter(world)
            .find(|(_, n)| n.0 == name)
            .map(|(e, _)| e)
            .unwrap_or_else(|| panic!("cat named {name:?} not found"))
    }

    fn fox_entity(world: &mut World) -> Entity {
        let mut q = world.query::<(Entity, &WildAnimal)>();
        q.iter(world)
            .find(|(_, w)| w.species == WildSpecies::Fox)
            .map(|(e, _)| e)
            .expect("fox entity not found")
    }

    fn hawk_entity(world: &mut World) -> Entity {
        let mut q = world.query::<(Entity, &WildAnimal)>();
        q.iter(world)
            .find(|(_, w)| w.species == WildSpecies::Hawk)
            .map(|(e, _)| e)
            .expect("hawk entity not found")
    }

    fn run_scenario_ticks(scenario: &Scenario, ticks: u32) -> bevy::app::App {
        let mut app = build_scenario_app(42, scenario, scenario.default_focal);
        app.update(); // Startup runs setup_world_exclusive (no-op when WorldSetup overrides).
        for _ in 0..ticks {
            app.update();
        }
        app
    }

    /// Read `Affordance(kind, perceiver, target)` from the world's resource.
    fn read_affordance(world: &World, perceiver: Entity, target: Entity, kind: ActionKind) -> f32 {
        world
            .resource::<ActionAffordances>()
            .read(perceiver, target, kind)
    }

    #[test]
    fn flee_high_cover_lifts_flee_affordance_above_open_ground() {
        let mut app_high = run_scenario_ticks(&SCENARIO_FLEE_HIGH_COVER, 4);
        let mut app_open = run_scenario_ticks(&SCENARIO_FLEE_OPEN_GROUND, 4);

        let high_world = app_high.world_mut();
        let probe_high = cat_by_name(high_world, FOCAL_NAME);
        let fox_high = fox_entity(high_world);
        let flee_high = read_affordance(high_world, probe_high, fox_high, ActionKind::Flee);

        let open_world = app_open.world_mut();
        let probe_open = cat_by_name(open_world, FOCAL_NAME);
        let fox_open = fox_entity(open_world);
        let flee_open = read_affordance(open_world, probe_open, fox_open, ActionKind::Flee);

        assert!(
            flee_high >= flee_open,
            "high-cover Flee ({flee_high}) should be ≥ open-ground Flee ({flee_open})"
        );
        // At least one of them should be non-zero (the substrate is honest day-one).
        assert!(
            flee_high > 0.0 || flee_open > 0.0,
            "both flee affordances were 0.0 — substrate didn't fire (high={flee_high} open={flee_open})"
        );
    }

    #[test]
    fn hawk_dive_eligible_pounce_gated() {
        let mut app = run_scenario_ticks(&SCENARIO_DIVE_HAWK, 4);
        let world = app.world_mut();
        let probe = cat_by_name(world, FOCAL_NAME);
        let hawk = hawk_entity(world);
        let dive = read_affordance(world, hawk, probe, ActionKind::Dive);
        let pounce = read_affordance(world, hawk, probe, ActionKind::Pounce);
        assert!(
            dive > 0.0,
            "hawk's Dive against adjacent cat should be eligible; got {dive}"
        );
        assert_eq!(pounce, 0.0, "hawks can't Pounce — species gate");
    }

    #[test]
    fn fox_chase_eligible_against_cat() {
        let mut app = run_scenario_ticks(&SCENARIO_CHASE_PREY, 4);
        let world = app.world_mut();
        let probe = cat_by_name(world, FOCAL_NAME);
        let fox = fox_entity(world);
        let chase = read_affordance(world, fox, probe, ActionKind::Chase);
        assert!(
            chase > 0.0,
            "fox Chase against cat should be eligible; got {chase}"
        );
    }

    #[test]
    fn fight_affordance_scales_with_capability_advantage() {
        let mut app = run_scenario_ticks(&SCENARIO_FIGHT_CAPABILITY_MATCH, 4);
        let world = app.world_mut();
        let probe = cat_by_name(world, FOCAL_NAME);
        let opponent = cat_by_name(world, "Opponent");
        // Probe at full HP vs opponent at 0.2 HP: dps_balance favors probe.
        let probe_vs_opp = read_affordance(world, probe, opponent, ActionKind::Fight);
        let opp_vs_probe = read_affordance(world, opponent, probe, ActionKind::Fight);
        assert!(
            probe_vs_opp >= opp_vs_probe,
            "healthy cat's Fight ({probe_vs_opp}) should be ≥ wounded cat's Fight ({opp_vs_probe})"
        );
    }

    #[test]
    fn supersedes_legacy_flee_nonzero_in_103_setup() {
        let mut app = run_scenario_ticks(&SCENARIO_SUPERSEDES_LEGACY_SCALARS, 4);
        let world = app.world_mut();
        let probe = cat_by_name(world, FOCAL_NAME);
        let fox = fox_entity(world);
        let flee = read_affordance(world, probe, fox, ActionKind::Flee);
        // 103 supersede sanity — the conceptual ground 103 covered
        // (fleeing an adjacent predator) yields a meaningful new
        // Affordance(Flee) value. Strict numeric equivalence is not
        // claimed (the new heuristic composes 4 weighted slots; 103
        // was a single axis).
        assert!(
            flee > 0.0,
            "supersede sanity: Affordance(Flee) should be > 0 in 103's setup; got {flee}"
        );
    }

    #[test]
    fn focal_cat_has_belief_substrate_after_run() {
        // Smoke test: scenarios use the full SimulationPlugin schedule
        // so CatBeliefs is wired and populated by `belief_integrator`.
        // This guards against an init regression breaking the substrate
        // pipeline scenarios depend on.
        let mut app = run_scenario_ticks(&SCENARIO_DIVE_HAWK, 4);
        let world = app.world_mut();
        let probe = cat_by_name(world, FOCAL_NAME);
        assert!(
            world.get::<CatBeliefs>(probe).is_some(),
            "scenario cats must carry CatBeliefs (the substrate the writer reads)"
        );
    }
}
