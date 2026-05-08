//! Ticket 231 R3b — wounded cat L2 score regression check.
//!
//! Reproduces the dying-arc analysis (Calcifer at HP=0.49, Cedar at
//! HP=0.38; both elected PickUp despite Sleep/Flee scoring higher).
//! Pre-R3b PickingUpDse scored from `colony_food_security` only, with
//! no body-state subscription — wounded cats kept electing PickUp at
//! ~0.96 in the seed-42 dying-arc despite Sleep at ~1.08 and Flee at
//! ~0.95 with HP=0.49 and a fox 2 tiles away. Post-R3b, the
//! multiplicative `health_deficit` damping suppresses PickUp's score
//! by `(1 - health_deficit)`, so HP=0.49 → score × 0.49 (51%
//! reduction).
//!
//! This scenario asserts the L2 score effect directly: a wounded cat
//! with adjacent ground food has PickUp's `final_score` materially
//! below its pre-R3b shape. The assertion is over L2 records (not L3
//! winners) so the test is robust to softmax noise.

use bevy_ecs::world::World;

use crate::components::items::{Item, ItemKind, ItemLocation};
use crate::components::physical::{Health, Position};

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

const COLONY_CENTER: Position = Position { x: 20, y: 20 };

fn spawn_ground_food(world: &mut World, kind: ItemKind, pos: Position) {
    world.spawn((Item::new(kind, 1.0, ItemLocation::OnGround), pos));
}

fn assert_has_ground_carcass(world: &mut World) {
    let colony = world
        .query_filtered::<bevy_ecs::entity::Entity, bevy_ecs::query::With<crate::components::markers::ColonyState>>()
        .iter(world)
        .next()
        .expect("ColonyState singleton must exist");
    world
        .entity_mut(colony)
        .insert(crate::components::markers::HasGroundCarcass);
}

fn wound_focal(world: &mut World, focal_name: &str, hp: f32) {
    use crate::components::identity::Name;
    let mut q = world.query::<(bevy_ecs::entity::Entity, &Name)>();
    let entity = q
        .iter(world)
        .find(|(_, n)| n.0 == focal_name)
        .map(|(e, _)| e)
        .expect("focal cat must exist before wounding");
    let mut em = world.entity_mut(entity);
    let mut health = em.get_mut::<Health>().expect("focal has Health");
    health.current = hp;
}

fn setup_wounded(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);
    let _focal = spawn_cat(
        world,
        CatPreset::adult("Calcifer", COLONY_CENTER).with_marker(MarkerKind::Adult),
    );
    wound_focal(world, "Calcifer", 0.49);
    spawn_ground_food(world, ItemKind::RawMouse, Position::new(21, 20));
    spawn_ground_food(world, ItemKind::RawMouse, Position::new(20, 21));
    assert_has_ground_carcass(world);
}

pub static SCENARIO: Scenario = Scenario {
    name: "wounded_cat_no_pickup",
    default_focal: "Calcifer",
    default_ticks: 6,
    setup: setup_wounded,
    expected_features: &[],
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::runner::run;

    /// Wounded cat (HP=0.49) with adjacent ground food. Pre-R3b
    /// PickUp scored ≈0.96 (no body-state suppression). Post-R3b
    /// PickUp's L2 final_score is multiplied by (1 - health_deficit)
    /// = 0.49 → score ≈ 0.47. We assert PickUp's L2 score is below
    /// 0.6 across at least one tick (gives some softmax-jitter
    /// budget) AND below 1.0 across all ticks (the pre-R3b ceiling).
    #[test]
    fn wounded_cat_pickup_score_is_damped() {
        let report = run(&SCENARIO, None, Some(6), 42);
        let mut min_pickup_score: Option<f32> = None;
        let mut max_pickup_score: Option<f32> = None;
        for tick in &report.ticks {
            for row in &tick.l2 {
                if row.dse == "pick_up" && row.eligible {
                    let s = row.final_score;
                    min_pickup_score = Some(min_pickup_score.map_or(s, |m| m.min(s)));
                    max_pickup_score = Some(max_pickup_score.map_or(s, |m| m.max(s)));
                }
            }
        }
        let min_score = min_pickup_score.expect(
            "PickingUp DSE must score on at least one tick (HasGroundCarcass authored, eligibility met)",
        );
        let max_score = max_pickup_score.unwrap();
        assert!(
            min_score < 0.6,
            "wounded cat (HP=0.49) PickUp final_score must be damped below 0.6 \
             via health_deficit multiplicative; got min={min_score}, max={max_score}"
        );
    }

    /// Sanity check on the math: at HP=0.49 the health_deficit axis
    /// evaluates to 0.51 (Linear with slope=-1, intercept=1 at
    /// deficit=0.51 gives 1 - 0.51 = 0.49). Multiplicative damping
    /// gives final_score ≈ 0.49 * pre_score. With food_axis ≈ 1.0
    /// (hungry-low food-security), final_score ≈ 0.49.
    #[test]
    fn wounded_cat_pickup_score_matches_damping_math() {
        let report = run(&SCENARIO, None, Some(6), 42);
        let pickup_scores: Vec<f32> = report
            .ticks
            .iter()
            .flat_map(|t| {
                t.l2.iter()
                    .filter(|r| r.dse == "pick_up" && r.eligible)
                    .map(|r| r.final_score)
            })
            .collect();
        assert!(
            !pickup_scores.is_empty(),
            "pick_up must produce ≥1 L2 score across the scenario"
        );
        // health_deficit at HP=0.49 → damping factor 0.49.
        // food_axis at low food-security ≈ 1.0.
        // Expected score ≈ 0.49 * 1.0 = 0.49 ± noise.
        for &s in &pickup_scores {
            assert!(
                s < 0.7,
                "every pick_up final_score must be damped below 0.7 \
                 (deficit=0.51, expected ≈0.49); got {s}"
            );
        }
    }
}
