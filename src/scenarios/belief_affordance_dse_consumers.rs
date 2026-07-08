//! Ticket 263 — Flee/Patrol/Hunt belief + affordance consumer
//! microexperiments. Sister to `affordance_substrate.rs` (261's
//! writer-side gate) and `colony_reserves_belief.rs` (258 sibling
//! consumer pattern).
//!
//! Four scenarios verifying the read paths that 263 wires into the
//! Flee, Patrol, and Hunt DSEs. All four 263 consumer axes ship
//! **dormant** (`flee_affordance_weight = 0`, `patrol_threat_recency_weight
//! = 0`, `hunt_best_predation_weight = 0`, `hunt_stalk_chase_affordance_bias
//! = 0`), so these scenarios assert on the substrate-side reads — the
//! `ActionAffordances` resource entry, the `LocationBeliefs` facet
//! value, and the `ScoringContext.patrol_threat_recency` precomputed
//! scalar — rather than on L3 election outcomes. Behavioural
//! assertions land in the activation follow-on alongside the
//! four-artifact methodology.
//!
//! | Variant                                       | Read path under test                                              |
//! |-----------------------------------------------|-------------------------------------------------------------------|
//! | `flee_belief_high_violence_capability`        | `Affordance(Flee, cat, fox)` populates; substrate sees the belief |
//! | `patrol_avoids_high_threat_sector`            | `LocationBeliefs.recency_of_threat_cue` round-trips into ScoringContext  |
//! | `hunt_picks_stalk_for_oblivious_prey`         | Cat-vs-prey rows populate; Stalk > Chase for oblivious prey       |
//! | `hunt_picks_chase_for_alerted_prey`           | Cat-vs-prey rows populate; Chase > Stalk for alerted/fleeing prey |
//!
//! **Writer gap (cat-vs-prey) — CLOSED by ticket 314.** Substrate
//! 261's writer originally covered `cat-vs-cat` and `cat-vs-wildlife`
//! but not `cat-vs-prey`; the two Hunt scenarios asserted the honest
//! `0.0` gate signal. 314 extended the writer (`write_cat_vs_prey`,
//! Stalk / Chase / Pounce composed from proximity, cover, health, and
//! the prey's own `alertness` scalar), and the assertions graduated to
//! the behavioural shape claims below. The Hunt per-target axis
//! (`hunt_best_predation_weight`) and the resolver phase-bias
//! (`hunt_stalk_chase_affordance_bias`) remain dormant — activation is
//! ticket 315's four-artifact job.

use bevy_ecs::world::World;

use crate::components::beliefs::{
    bucket_position, EvidenceKind, Facet, LocationBeliefs, MentalModel, PredatorBeliefs,
};
use crate::components::physical::{Health, Position};
use crate::components::prey::PreyKind;
use crate::components::wildlife::{WildAnimal, WildSpecies};

use super::env::{init_scenario_world, spawn_cat, spawn_prey_at};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

const FOCAL_NAME: &str = "Probe";

// ---------------------------------------------------------------------------
// Variant 1 — Flee belief read at high violence capability
// ---------------------------------------------------------------------------

pub static SCENARIO_FLEE_BELIEF_HIGH_VIOLENCE: Scenario = Scenario {
    name: "flee_belief_high_violence_capability",
    default_focal: FOCAL_NAME,
    default_ticks: 4,
    setup: setup_flee_belief_high_violence,
    expected_features: &[],
};

fn setup_flee_belief_high_violence(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    let cat = spawn_cat(
        world,
        CatPreset::adult(FOCAL_NAME, Position::new(20, 20))
            .with_needs(|n| n.safety = 0.3)
            .with_marker(MarkerKind::Adult),
    );
    let fox = world
        .spawn((
            Position::new(22, 20),
            WildAnimal::new(WildSpecies::Fox),
            Health::default(),
        ))
        .id();
    // Pre-stamp the cat's PredatorBeliefs against the fox with a
    // high `perceived_violence_capability`. The affordance writer
    // composes this into `Affordance(Flee)`; the read path 263 wires
    // surfaces the entry under `ctx_scalars["flee_affordance"]`.
    if let Some(mut pb) = world.get_mut::<PredatorBeliefs>(cat) {
        let model = MentalModel {
            perceived_violence_capability: Facet {
                value: 0.9,
                prior: 0.5,
                strength: 1.0,
                last_source: EvidenceKind::Observation,
                last_updated_tick: 0,
            },
            ..MentalModel::default()
        };
        pb.models.insert(fox, model);
    }
}

// ---------------------------------------------------------------------------
// Variant 2 — Patrol belief read at high threat-recency sector
// ---------------------------------------------------------------------------

pub static SCENARIO_PATROL_AVOIDS_HIGH_THREAT_SECTOR: Scenario = Scenario {
    name: "patrol_avoids_high_threat_sector",
    default_focal: FOCAL_NAME,
    default_ticks: 4,
    setup: setup_patrol_avoids_high_threat_sector,
    expected_features: &[],
};

fn setup_patrol_avoids_high_threat_sector(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    let cat = spawn_cat(
        world,
        CatPreset::adult(FOCAL_NAME, Position::new(20, 20)).with_marker(MarkerKind::Adult),
    );
    // Pre-stamp the cat's LocationBeliefs at the territory perimeter
    // anchor's bucket with a saturated `recency_of_threat_cue`. The
    // 263 `patrol_threat_recency` scalar reads this bucket; the Patrol
    // DSE's dormant 6th axis would consume it if activated.
    // We don't know the exact anchor bucket without running the
    // disposition pipeline first, so we stamp the bucket the colony
    // center resolves to as a fallback (the route the production code
    // takes when WardCoverageMap has no coverage).
    if let Some(mut lb) = world.get_mut::<LocationBeliefs>(cat) {
        let model = MentalModel {
            recency_of_threat_cue: Facet {
                value: 0.85,
                prior: 0.0,
                strength: 1.0,
                last_source: EvidenceKind::Observation,
                last_updated_tick: 0,
            },
            ..MentalModel::default()
        };
        // Stamp adjacent buckets so whichever sector_centroid the
        // ward-coverage resolution lands on, at least one bucket has
        // the high recency reading the cat's LocationBeliefs map.
        for offset in [(0, 0), (1, 0), (0, 1), (-1, 0), (0, -1)] {
            let key = bucket_position(20 + offset.0, 20 + offset.1);
            lb.models.insert(key, model.clone());
        }
    }
}

// ---------------------------------------------------------------------------
// Variant 3 — Hunt picks Stalk for oblivious prey
// ---------------------------------------------------------------------------

pub static SCENARIO_HUNT_PICKS_STALK_FOR_OBLIVIOUS_PREY: Scenario = Scenario {
    name: "hunt_picks_stalk_for_oblivious_prey",
    default_focal: FOCAL_NAME,
    default_ticks: 4,
    setup: setup_hunt_picks_stalk_for_oblivious_prey,
    expected_features: &[],
};

fn setup_hunt_picks_stalk_for_oblivious_prey(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    spawn_cat(
        world,
        CatPreset::adult(FOCAL_NAME, Position::new(20, 20))
            .with_needs(|n| n.hunger = 0.3)
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanHunt),
    );
    // Prey at intermediate distance: within stalk_start band but
    // outside pounce_range. Default alertness from `spawn_prey_at` is
    // low; the affordance writer composes `(1 - intent_clarity)` into
    // Stalk, so the substrate-level read should favor Stalk over
    // Chase for an oblivious target.
    spawn_prey_at(world, Position::new(24, 20), PreyKind::Mouse);
}

// ---------------------------------------------------------------------------
// Variant 4 — Hunt picks Chase for alerted prey
// ---------------------------------------------------------------------------

pub static SCENARIO_HUNT_PICKS_CHASE_FOR_ALERTED_PREY: Scenario = Scenario {
    name: "hunt_picks_chase_for_alerted_prey",
    default_focal: FOCAL_NAME,
    default_ticks: 4,
    setup: setup_hunt_picks_chase_for_alerted_prey,
    expected_features: &[],
};

fn setup_hunt_picks_chase_for_alerted_prey(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    let cat = spawn_cat(
        world,
        CatPreset::adult(FOCAL_NAME, Position::new(20, 20))
            .with_needs(|n| n.hunger = 0.3)
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanHunt),
    );
    // Same prey at same distance, but bump its alertness to the
    // `Alert` band post-spawn so the affordance writer reads the
    // belief facet near-saturated. Chase becomes the higher-afforded
    // predation action because the substrate's Stalk heuristic
    // suppresses for aware prey.
    let prey = spawn_prey_at(world, Position::new(24, 20), PreyKind::Mouse);
    if let Some(mut state) = world.get_mut::<crate::components::prey::PreyState>(prey) {
        state.alertness = 0.95;
        state.ai_state = crate::components::prey::PreyAiState::Fleeing {
            from: cat,
            toward: Some((28, 20)),
            ticks: 0,
        };
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use crate::components::identity::{Name, Species};
    use crate::resources::{ActionAffordances, ActionKind};
    use crate::scenarios::runner::build_scenario_app;
    use bevy_ecs::prelude::*;

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

    fn prey_entity(world: &mut World) -> Entity {
        let mut q = world.query_filtered::<Entity, With<crate::components::prey::PreyAnimal>>();
        q.iter(world).next().expect("prey entity not found")
    }

    fn run_scenario_ticks(scenario: &Scenario, ticks: u32) -> bevy::app::App {
        let mut app = build_scenario_app(42, scenario, scenario.default_focal);
        app.update();
        for _ in 0..ticks {
            app.update();
        }
        app
    }

    fn read_affordance(world: &World, perceiver: Entity, target: Entity, kind: ActionKind) -> f32 {
        world
            .resource::<ActionAffordances>()
            .read(perceiver, target, kind)
    }

    #[test]
    fn flee_belief_high_violence_capability_populates_affordance() {
        let mut app = run_scenario_ticks(&SCENARIO_FLEE_BELIEF_HIGH_VIOLENCE, 4);
        let world = app.world_mut();
        let probe = cat_by_name(world, FOCAL_NAME);
        let fox = fox_entity(world);
        let flee = read_affordance(world, probe, fox, ActionKind::Flee);
        // The substrate's writer composes proximity + cover_self +
        // my_health + violence_cap; at proximity 2 with pre-stamped
        // high violence_cap, the entry must be non-zero. The dormant
        // DSE-side axis would consume this if activated.
        assert!(
            flee > 0.0,
            "Affordance(Flee) must populate when fox in range; got {flee}"
        );
    }

    #[test]
    fn patrol_avoids_high_threat_sector_belief_round_trip() {
        let mut app = run_scenario_ticks(&SCENARIO_PATROL_AVOIDS_HIGH_THREAT_SECTOR, 4);
        let world = app.world_mut();
        let probe = cat_by_name(world, FOCAL_NAME);
        // Verify the pre-stamped belief survived a few ticks of
        // integrator-side decay. The integrator's strength-based
        // Forgetting sweep would zero entries below the threshold,
        // but a 4-tick window with strength=1.0 is well above floor.
        let lb = world
            .get::<LocationBeliefs>(probe)
            .expect("focal cat must have LocationBeliefs (spawned by setup.rs)");
        let any_high = lb
            .models
            .values()
            .any(|m| m.recency_of_threat_cue.value > 0.5);
        assert!(
            any_high,
            "pre-stamped recency_of_threat_cue must survive into the LocationBeliefs map"
        );
    }

    #[test]
    fn hunt_picks_stalk_for_oblivious_prey_behavioural() {
        // Graduated from the 263 "writer gap" plumbing smoke by ticket
        // 314: `write_cat_vs_prey` now populates the predation trio.
        // An oblivious (low-alertness) mouse suppresses nothing in the
        // Stalk composition (`1 - alertness` slot near 1.0) while the
        // Chase composition's alertness slot sits near 0.0 — Stalk
        // must out-afford Chase.
        let mut app = run_scenario_ticks(&SCENARIO_HUNT_PICKS_STALK_FOR_OBLIVIOUS_PREY, 4);
        let world = app.world_mut();
        let probe = cat_by_name(world, FOCAL_NAME);
        let prey = prey_entity(world);
        let stalk = read_affordance(world, probe, prey, ActionKind::Stalk);
        let chase = read_affordance(world, probe, prey, ActionKind::Chase);
        let pounce = read_affordance(world, probe, prey, ActionKind::Pounce);
        assert!(
            stalk > 0.0,
            "cat-vs-prey Stalk must populate for an in-range mouse; got {stalk}"
        );
        assert!(
            pounce > 0.0,
            "cat-vs-prey Pounce must populate for an in-range mouse; got {pounce}"
        );
        assert!(
            stalk > chase,
            "oblivious prey: Stalk ({stalk}) should out-afford Chase ({chase})"
        );
    }

    #[test]
    fn hunt_picks_chase_for_alerted_prey_behavioural() {
        // Graduated mirror: the near-saturated alertness of a fleeing
        // mouse feeds Chase (flushed prey is run down — interception
        // geometry, not stealth) and starves Stalk's `1 - alertness`
        // slot. Chase must out-afford Stalk.
        let mut app = run_scenario_ticks(&SCENARIO_HUNT_PICKS_CHASE_FOR_ALERTED_PREY, 4);
        let world = app.world_mut();
        let probe = cat_by_name(world, FOCAL_NAME);
        let prey = prey_entity(world);
        let stalk = read_affordance(world, probe, prey, ActionKind::Stalk);
        let chase = read_affordance(world, probe, prey, ActionKind::Chase);
        assert!(
            chase > 0.0,
            "cat-vs-prey Chase must populate for an in-range mouse; got {chase}"
        );
        assert!(
            chase > stalk,
            "alerted prey: Chase ({chase}) should out-afford Stalk ({stalk})"
        );
    }
}
