//! Ticket 265 (activation, plan step 21) — wildlife belief +
//! affordance consumer microexperiments. Sister to
//! `belief_affordance_dse_consumers.rs` (263/314's cat-side gate);
//! these cover the wildlife-side consumers activated by step 21.
//!
//! | Variant                                    | Claim under test                                                   |
//! |--------------------------------------------|--------------------------------------------------------------------|
//! | `fox_belief_high_violence_capability_cat`  | Fox that believes a nearby cat is violent elects Fleeing; a twin fox with a harmless belief does not |
//! | `hawk_dive_affordance_aerial_cover`        | Hawk Dive affordance high on open ground, suppressed under cover   |
//! | `wildlife_species_clash`                   | Cat and fox mutually perceive via substrate; the frightened side backs off with no director |
//!
//! The fox variant lifts `fox_flee_cat_violence_belief_weight` above
//! its tuned first-light default inside `setup` — the scenario pins
//! the *wiring* (CatBeliefs → `build_scoring_context` →
//! `perceived_cat_threat` ctx scalar → FoxFleeing axis → L3 election),
//! not the tuned magnitude, which is the soak gate's job. Both foxes
//! share mirrored geometry (equal edge distance, one cat at Manhattan
//! 5, sated needs, fixed personality), so the *only* live difference
//! is the stamped `perceived_violence_capability` facet.

use bevy_ecs::world::World;

use crate::components::beliefs::{CatBeliefs, EvidenceKind, Facet, MentalModel};
use crate::components::fox_personality::{FoxNeeds, FoxPersonality};
use crate::components::physical::{Health, Position};
use crate::components::wildlife::{FoxAiPhase, FoxSex, FoxState, WildAnimal, WildSpecies};

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

/// Fox A ("believer") — stamped with a saturated violence belief
/// about its adjacent cat.
const BELIEVER_POS: Position = Position::new(10, 10);
const BELIEVER_CAT_POS: Position = Position::new(15, 10);
/// Fox B ("skeptic") — stamped with a floor violence belief about its
/// own nearby cat. Mirrored edge distance (10 tiles) on the default
/// 40×40 scenario map. Cats sit at Manhattan 5 — inside the ≤6
/// cats-nearby / belief radius, but far enough out that FoxAvoiding's
/// sharp `(d/8)²`-invert cluster axis doesn't bury the Fleeing
/// comparison (Avoiding is the pre-threat back-off; this scenario
/// pins the *belief-driven* one).
const SKEPTIC_POS: Position = Position::new(29, 29);
const SKEPTIC_CAT_POS: Position = Position::new(24, 29);

pub static SCENARIO_FOX_BELIEF_HIGH_VIOLENCE: Scenario = Scenario {
    name: "fox_belief_high_violence_capability_cat",
    default_focal: "Menace",
    default_ticks: 6,
    setup: setup_fox_belief_high_violence,
    expected_features: &[],
};

fn setup_fox_belief_high_violence(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Pin the belief-axis weight above the tuned first-light default
    // and squeeze the disposition softmax to near-argmax, so the
    // election assertion tests wiring, not tuning or roll noise (the
    // same constants-override idiom as `fox_cat_scent_avoidance`).
    {
        let mut constants = world.resource_mut::<crate::resources::sim_constants::SimConstants>();
        constants.scoring.fox_flee_cat_violence_belief_weight = 0.7;
        constants.scoring.fox_softmax_temperature = 0.02;
    }

    let menace = spawn_cat(
        world,
        CatPreset::adult("Menace", BELIEVER_CAT_POS).with_marker(MarkerKind::Adult),
    );
    let bystander = spawn_cat(
        world,
        CatPreset::adult("Bystander", SKEPTIC_CAT_POS).with_marker(MarkerKind::Adult),
    );

    let believer = spawn_scenario_fox(world, BELIEVER_POS);
    let skeptic = spawn_scenario_fox(world, SKEPTIC_POS);

    // Stamp the beliefs directly (the observation channel that would
    // produce these is exercised by the belief_integrator tests): the
    // believer watched Menace win fights; the skeptic knows Bystander
    // is harmless.
    stamp_cat_violence_belief(world, believer, menace, 0.95);
    stamp_cat_violence_belief(world, skeptic, bystander, 0.05);
}

/// Full fox spawn bundle (mirrors `wildlife.rs` adult spawns) with
/// deterministic personality, a home den with saturated scent
/// (suppresses the Patrolling urge — `sync_fox_needs` reads
/// `territory_scent` from the den, so a denless fox reads 0.0 and
/// patrols forever), so the Fleeing election difference can only come
/// from the stamped belief.
fn spawn_scenario_fox(world: &mut World, pos: Position) -> bevy_ecs::entity::Entity {
    let den = world
        .spawn((
            {
                let mut den = crate::components::wildlife::FoxDen::new(12.0, 0);
                den.scent_strength = 1.0;
                den
            },
            pos,
        ))
        .id();
    world
        .spawn((
            WildAnimal::new(WildSpecies::Fox),
            pos,
            Health::default(),
            crate::components::wildlife::WildlifeAiState::Patrolling { dx: 1, dy: 0 },
            FoxState::new_adult(FoxSex::Male, Some(den)),
            FoxAiPhase::PatrolTerritory { dx: 1, dy: 0 },
            FoxNeeds {
                hunger: 1.0,
                territory_scent: 1.0,
                den_security: 1.0,
                ..Default::default()
            },
            // Bold fox: FoxAvoiding's damped-invert (slope 0.8) reads
            // 1 − 0.64 = 0.36 while FoxFleeing's (slope 0.5) reads
            // 0.6 — boldness suppresses the pre-threat back-off much
            // harder than the belief-driven one, which is exactly the
            // discrimination this scenario needs.
            FoxPersonality {
                boldness: 0.8,
                cunning: 0.5,
                protectiveness: 0.5,
                territoriality: 0.5,
            },
            crate::components::fox_spatial::FoxHuntingBeliefs::default_map(),
            crate::components::fox_spatial::FoxThreatMemory::default_map(),
            crate::components::fox_spatial::FoxExplorationMap::default_map(),
            crate::components::SensorySpecies::Wild(WildSpecies::Fox),
            crate::components::SensorySignature::WILDLIFE,
        ))
        .id()
}

// ---------------------------------------------------------------------------
// Variant 2 — hawk Dive affordance vs aerial cover
// ---------------------------------------------------------------------------

/// Hawk between two mice at symmetric distance 6 (inside the 10-tile
/// writer sensing range). The west mouse shelters under a live
/// Thornward (repel radius 6 — a real `Ward` entity, not a setup-time
/// `stamp_ward`, because `update_ward_coverage_map` clears and
/// rebuilds from live wards every tick); the east mouse sits on open
/// ground. `write_wildlife_vs_prey`'s Dive slot reads
/// `1 − cover_at_target`, so open ground must out-afford cover.
const HAWK_POS: Position = Position::new(20, 20);
const COVERED_MOUSE_POS: Position = Position::new(14, 20);
const OPEN_MOUSE_POS: Position = Position::new(26, 20);

pub static SCENARIO_HAWK_DIVE_AERIAL_COVER: Scenario = Scenario {
    name: "hawk_dive_affordance_aerial_cover",
    default_focal: "Watcher",
    default_ticks: 4,
    setup: setup_hawk_dive_aerial_cover,
    expected_features: &[],
};

fn setup_hawk_dive_aerial_cover(world: &mut World, seed: u64) {
    use crate::components::prey::PreyKind;

    init_scenario_world(world, seed);

    // A named cat far from the action — the runner needs a focal to
    // resolve, but it must not perturb the hawk/mice geometry.
    spawn_cat(
        world,
        CatPreset::adult("Watcher", Position::new(5, 35)).with_marker(MarkerKind::Adult),
    );

    world.spawn((
        HAWK_POS,
        WildAnimal::new(WildSpecies::Hawk),
        Health::default(),
    ));

    super::env::spawn_prey_at(world, COVERED_MOUSE_POS, PreyKind::Mouse);
    super::env::spawn_prey_at(world, OPEN_MOUSE_POS, PreyKind::Mouse);

    // Live ward sheltering the west mouse (thornward radius 6.0 at
    // full strength; decay over a 4-tick run is negligible).
    world.spawn((
        crate::components::magic::Ward::thornward(),
        COVERED_MOUSE_POS,
    ));
}

fn stamp_cat_violence_belief(
    world: &mut World,
    fox: bevy_ecs::entity::Entity,
    cat: bevy_ecs::entity::Entity,
    value: f32,
) {
    let mut beliefs = world
        .get_mut::<CatBeliefs>(fox)
        .expect("WildAnimal requires CatBeliefs (265 contract)");
    beliefs.models.insert(
        cat,
        MentalModel {
            perceived_violence_capability: Facet {
                value,
                prior: 0.5,
                strength: 1.0,
                last_source: EvidenceKind::Observation,
                last_updated_tick: 0,
            },
            ..MentalModel::default()
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::ai::fox_planner::FoxDispositionKind;
    use crate::components::fox_goap_plan::FoxGoapPlan;
    use crate::scenarios::runner::build_scenario_app;
    use bevy_ecs::prelude::{Entity, With};

    fn run_scenario_ticks(scenario: &Scenario, ticks: u32) -> bevy::app::App {
        let mut app = build_scenario_app(42, scenario, scenario.default_focal);
        app.update();
        for _ in 0..ticks {
            app.update();
        }
        app
    }

    /// The two foxes start 19 tiles apart and can move at most one
    /// tile per tick over the 6-tick run, so the x=20 half-plane
    /// still uniquely identifies them at assertion time.
    fn fox_plan_kind_in_half(world: &mut World, west: bool) -> FoxDispositionKind {
        let mut q = world.query::<(&WildAnimal, &Position, Option<&FoxGoapPlan>)>();
        let roster: Vec<String> = q
            .iter(world)
            .filter(|(w, _, _)| w.species == WildSpecies::Fox)
            .map(|(_, p, plan)| {
                format!(
                    "fox@({},{}) plan={:?}",
                    p.x(),
                    p.y(),
                    plan.map(|pl| pl.kind)
                )
            })
            .collect();
        q.iter(world)
            .find(|(w, p, plan)| {
                w.species == WildSpecies::Fox && ((p.x() < 20) == west) && plan.is_some()
            })
            .and_then(|(_, _, plan)| plan.map(|pl| pl.kind))
            .unwrap_or_else(|| {
                let tick = world.resource::<crate::resources::TimeState>().tick;
                panic!("fox with a FoxGoapPlan not found in {} half-plane at tick {tick}; roster: {roster:?}",
                    if west { "west" } else { "east" })
            })
    }

    #[test]
    fn hawk_dive_prefers_open_ground_mouse_over_covered() {
        use crate::resources::{ActionAffordances, ActionKind};

        let mut app = run_scenario_ticks(&SCENARIO_HAWK_DIVE_AERIAL_COVER, 4);
        let world = app.world_mut();

        let hawk = {
            let mut q = world.query::<(Entity, &WildAnimal)>();
            q.iter(world)
                .find(|(_, w)| w.species == WildSpecies::Hawk)
                .map(|(e, _)| e)
                .expect("hawk not found")
        };
        // Mice may shuffle a tile or two over 4 ticks; the x=20
        // half-plane still splits them.
        let mouse_in_half = |world: &mut World, west: bool| -> Entity {
            let mut q = world
                .query_filtered::<(Entity, &Position), With<crate::components::prey::PreyAnimal>>();
            q.iter(world)
                .find(|(_, p)| (p.x() < 20) == west)
                .map(|(e, _)| e)
                .expect("mouse not found in expected half-plane")
        };
        let covered = mouse_in_half(world, true);
        let open = mouse_in_half(world, false);

        let read = |world: &World, target: Entity| -> f32 {
            world
                .resource::<ActionAffordances>()
                .read(hawk, target, ActionKind::Dive)
        };
        let dive_open = read(world, open);
        let dive_covered = read(world, covered);
        assert!(
            dive_open > 0.0,
            "Dive against the open-ground mouse must populate; got {dive_open}"
        );
        assert!(
            dive_open > dive_covered,
            "open ground must out-afford ward cover: open={dive_open} covered={dive_covered}"
        );
    }

    #[test]
    fn fox_flees_cat_it_believes_violent_twin_does_not() {
        let mut app = run_scenario_ticks(&SCENARIO_FOX_BELIEF_HIGH_VIOLENCE, 6);
        let world = app.world_mut();
        let believer_kind = fox_plan_kind_in_half(world, true);
        let skeptic_kind = fox_plan_kind_in_half(world, false);
        assert_eq!(
            believer_kind,
            FoxDispositionKind::Fleeing,
            "fox with a saturated violence belief about the adjacent cat \
             must elect Fleeing; got {believer_kind:?}"
        );
        assert_ne!(
            skeptic_kind,
            FoxDispositionKind::Fleeing,
            "twin fox with a floor violence belief at mirrored geometry \
             must NOT elect Fleeing"
        );
    }
}
