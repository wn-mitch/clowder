//! Ticket 400 scenario — JointIntention-aware Caretake suppression.
//!
//! Two high-presence parents (both compassion + warmth ≈ 0.9) and one
//! hungry kitten. Without suppression, both would race to Caretake and
//! trigger the 398 `HandoffItem` cascade (41× plan-failure spike). With
//! suppression: when one parent holds a Caretake intention targeting the
//! shared kitten, the other parent's `caretake_bias_sum` is multiplied by
//! `joint_suppression_factor` (≈ 0.3), yielding without fully dropping
//! out (so they snap back if the first parent lapses).
//!
//! The scenario does not pre-stage a `HeldIntention` — the suppression
//! kicks in naturally once one parent's softmax election lands on
//! Caretake; subsequent ticks should show the other parent's
//! `caretake_suppression_factor` reading 0.3 in their L2 trace, with
//! their picks drifting to other DSEs.

use bevy_ecs::world::World;

use crate::components::parenting_activity::{ParentalKind, ParentingActivity, RelationshipTo};
use crate::components::physical::Position;
use crate::systems::parenting_activity::parental_engagement_asymptote;

use super::env::{init_scenario_world, spawn_cat, spawn_kitten};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "parenting_joint_suppression",
    default_focal: "Sage",
    default_ticks: 15,
    setup,
    expected_features: &[],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    let kitten_pos = Position::new(20, 20);
    let current_tick = world.resource::<crate::resources::TimeState>().tick;

    // Both parents are high-presence — the corner case 398 left
    // unresolved before 400's suppression mechanism.
    let sage = spawn_cat(
        world,
        CatPreset::adult("Sage", Position::new(21, 20))
            .with_personality(|p| {
                p.compassion = 0.9;
                p.warmth = 0.9;
                p.diligence = 0.5;
                p.loyalty = 0.6;
            })
            .with_marker(MarkerKind::Parent)
            .with_marker(MarkerKind::Adult),
    );

    let cedar = spawn_cat(
        world,
        CatPreset::adult("Cedar", Position::new(19, 20))
            .with_personality(|p| {
                p.compassion = 0.9;
                p.warmth = 0.9;
                p.diligence = 0.5;
                p.loyalty = 0.6;
            })
            .with_marker(MarkerKind::Parent)
            .with_marker(MarkerKind::Adult),
    );

    let sprig = spawn_kitten(
        world,
        CatPreset::kitten("Sprig", kitten_pos, current_tick).with_needs(|n| {
            n.hunger = 0.2;
        }),
        sage,
        cedar,
    );

    preload_parenting(world, sage, cedar, sprig, current_tick);
    preload_parenting(world, cedar, sage, sprig, current_tick);
}

fn preload_parenting(
    world: &mut World,
    owner: bevy_ecs::entity::Entity,
    partner: bevy_ecs::entity::Entity,
    target: bevy_ecs::entity::Entity,
    tick: u64,
) {
    let asymptote = {
        let personality = world
            .get::<crate::components::personality::Personality>(owner)
            .expect("owner has Personality")
            .clone();
        let constants = world.resource::<crate::resources::SimConstants>();
        parental_engagement_asymptote(&personality, 0.0, &constants.parenting)
    };
    let mut rel = RelationshipTo::new(target, ParentalKind::Biological, Some(partner), tick);
    rel.parental_engagement = asymptote;
    world.entity_mut(owner).insert(ParentingActivity {
        relationships: vec![rel],
    });
}
