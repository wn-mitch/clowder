//! Ticket 400 scenario — diligent low-compassion father picks Hunt
//! (provision_bias) over Caretake.
//!
//! Spawns a mother (presence-heavy: high compassion + warmth) and a father
//! (provision-heavy: high diligence + loyalty, low compassion) with one
//! hungry kitten. Both parents have `ParentingActivity` pre-populated with
//! `parental_engagement` saturated at their personality-derived asymptote
//! (skipping the ~1000-tick EMA build phase that would dominate a 30-tick
//! scenario).
//!
//! Expected outcome: in the focal-trace L2 record, the father's
//! `parenting_activity` modifier contributes more to Hunt
//! (`provision_bias_sum`) than to Caretake (`caretake_bias_sum`),
//! demonstrating that 400's personality-conditional dispersion routes the
//! low-compassion provider away from the Caretake competition — the
//! structural fix for 398's `HandoffItem` cascade.

use bevy_ecs::world::World;

use crate::components::parenting_activity::{
    ParentalKind, ParentingActivity, RelationshipTo,
};
use crate::components::physical::Position;
use crate::systems::parenting_activity::parental_engagement_asymptote;

use super::env::{init_scenario_world, spawn_cat, spawn_kitten};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "parenting_father_provisions",
    default_focal: "Brick",
    // Enough ticks for L2 scoring + modifier-pipeline plumbing to settle.
    // ParentingActivity is pre-populated at spawn so we don't wait on
    // engagement build.
    default_ticks: 10,
    setup,
    expected_features: &[],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    let kitten_pos = Position::new(20, 20);
    let current_tick = world.resource::<crate::resources::TimeState>().tick;

    // Presence-heavy mother — high compassion + warmth. Asymptote ≈ 0.7.
    let magnolia = spawn_cat(
        world,
        CatPreset::adult("Magnolia", Position::new(21, 20))
            .with_personality(|p| {
                p.compassion = 0.9;
                p.warmth = 0.9;
                p.diligence = 0.4;
                p.loyalty = 0.5;
                p.boldness = 0.4;
                p.temper = 0.3;
            })
            .with_marker(MarkerKind::Parent)
            .with_marker(MarkerKind::Adult),
    );

    // Provision-heavy father — high diligence + loyalty, low compassion.
    // Asymptote ≈ 0.4 (Presence weight pulls it down despite high
    // Provision); per-DSE biases skew sharply toward Hunt over Caretake.
    let brick = spawn_cat(
        world,
        CatPreset::adult("Brick", Position::new(19, 20))
            .with_personality(|p| {
                p.compassion = 0.2;
                p.warmth = 0.2;
                p.diligence = 0.9;
                p.loyalty = 0.9;
                p.boldness = 0.5;
                p.temper = 0.4;
            })
            .with_marker(MarkerKind::Parent)
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanHunt),
    );

    // Hungry kitten — mother is Magnolia, father is Brick.
    let crumb = spawn_kitten(
        world,
        CatPreset::kitten("Crumb", kitten_pos, current_tick).with_needs(|n| {
            n.hunger = 0.2;
        }),
        magnolia,
        brick,
    );

    // Pre-populate ParentingActivity on both parents at saturation so the
    // modifier-pipeline trace is informative on tick 1.
    preload_parenting(world, magnolia, brick, crumb, current_tick);
    preload_parenting(world, brick, magnolia, crumb, current_tick);
}

/// Insert ParentingActivity on `owner` with one Biological RelationshipTo
/// toward `target` (the kitten); `partner` is the co-parent. Engagement is
/// set to the personality-derived asymptote so the modifier pipeline
/// reflects steady-state behavior from tick 1.
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
