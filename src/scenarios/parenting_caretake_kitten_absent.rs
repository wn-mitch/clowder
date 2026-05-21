//! Ticket 410 scenario — Caretake DSE is eligibility-gated when no
//! care-dependent cat exists in the colony.
//!
//! Two adult parents (both high-compassion, both with persistent
//! `ParentingActivity` carrying Biological relationships toward kittens
//! who already despawned — the §7.7.b grief substrate). Zero `Kitten`-
//! marked entities. The `update_colony_building_markers` populator
//! therefore leaves `HasDependentCat` false.
//!
//! Pre-410 the three-axis Caretake WeightedSum still produced a
//! positive raw score (compassion ≈ 0.5 × 0.3 = 0.15 plus
//! parental_engagement residual ≈ 0.07 × 0.25 ≈ 0.02), and
//! `ParentingActivityModifier` lifted that further. Cats elected
//! Caretake, the planner couldn't resolve a kitten target, and the
//! `HandoffItem: no recipient on disposition` canary fired.
//!
//! Post-410 the new `.require(HasDependentCat::KEY)` eligibility filter
//! suppresses Caretake outright when no dependent exists — the score
//! is irrelevant, the row is `eligible: false`, and the L3 softmax
//! picks something else. The grief gradient on the ParentingActivity
//! Component still decays toward residual (the cat still *feels* the
//! pull) but the DSE never elects.

use bevy_ecs::world::World;

use crate::components::parenting_activity::{ParentalKind, ParentingActivity, RelationshipTo};
use crate::components::physical::Position;
use crate::systems::parenting_activity::parental_engagement_asymptote;

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "parenting_caretake_kitten_absent",
    default_focal: "Briar",
    // Enough ticks for L2 scoring + modifier pipeline to settle. The
    // eligibility gate fires on every tick, so we don't need many.
    default_ticks: 5,
    setup,
    expected_features: &[],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    let current_tick = world.resource::<crate::resources::TimeState>().tick;

    // Two high-compassion adult parents whose kittens are gone — high
    // pre-410 Caretake-axis lift comes from both compassion (always
    // non-zero) and parental_engagement residual (the §7.7.b grief
    // substrate). Without the gate, both would compete for Caretake
    // and fail the planner.
    let briar = spawn_cat(
        world,
        CatPreset::adult("Briar", Position::new(20, 20))
            .with_personality(|p| {
                p.compassion = 0.9;
                p.warmth = 0.8;
                p.diligence = 0.6;
                p.loyalty = 0.7;
            })
            .with_marker(MarkerKind::Adult),
    );

    let sage = spawn_cat(
        world,
        CatPreset::adult("Sage", Position::new(21, 20))
            .with_personality(|p| {
                p.compassion = 0.9;
                p.warmth = 0.8;
                p.diligence = 0.6;
                p.loyalty = 0.7;
            })
            .with_marker(MarkerKind::Adult),
    );

    // Ghost target — same pattern as `parenting_grief_kitten_death`.
    // Bare entity, no `Kitten` marker, so the colony populator's
    // `kittens.is_empty()` check sees an empty roster and
    // `HasDependentCat` stays false.
    let ghost_target = world.spawn(()).id();

    preload_grieving(world, briar, ghost_target, current_tick);
    preload_grieving(world, sage, ghost_target, current_tick);
}

fn preload_grieving(
    world: &mut World,
    owner: bevy_ecs::entity::Entity,
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
    let mut rel = RelationshipTo::new(target, ParentalKind::Biological, None, tick);
    rel.parental_engagement = asymptote;
    world.entity_mut(owner).insert(ParentingActivity {
        relationships: vec![rel],
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::runner::{build_scenario_app, run};

    /// `HasDependentCat` is the colony-scoped marker the new eligibility
    /// filter reads. With zero `Kitten`-marked entities,
    /// `update_colony_building_markers` populates it false — the
    /// structural precondition for Caretake's eligibility gate to fire.
    #[test]
    fn has_dependent_cat_marker_is_false_when_no_kittens() {
        let mut app = build_scenario_app(42, &SCENARIO, "Briar");
        // Two updates: first drains the Startup schedule (scenario
        // setup); second runs one FixedUpdate including the colony
        // marker populator (`update_colony_building_markers`).
        app.update();
        app.update();

        let world = app.world_mut();
        let has_marker = {
            let mut q = world.query_filtered::<bevy_ecs::entity::Entity, (
                bevy_ecs::query::With<crate::components::markers::ColonyState>,
                bevy_ecs::query::With<crate::components::markers::HasDependentCat>,
            )>();
            q.iter(world).next().is_some()
        };
        assert!(
            !has_marker,
            "HasDependentCat must be false on the ColonyState singleton \
             when no Kitten-marked entities exist (the structural \
             precondition for Caretake's eligibility gate)"
        );
    }

    /// The focal cat's chosen action must never be Caretake-derived
    /// (anything that emits HandoffItem). Together with the marker
    /// check above, this asserts the eligibility-gate-meets-behavior
    /// pipeline: marker is false → Caretake eligibility filter fails
    /// → L3 cannot elect a HandoffItem chain → planner never tries to
    /// resolve a kitten target → canary doesn't fire.
    #[test]
    fn focal_never_elects_caretake_chain() {
        let report = run(&SCENARIO, None, None, 42);
        for tick in &report.ticks {
            if let Some(chosen) = &tick.chosen {
                // Caretake's HTN method emits `Action::Caretake`, which
                // dispatches to `GoapActionKind::HandoffItem`. Either
                // surface name is a giveaway.
                assert!(
                    !chosen.contains("Caretake") && !chosen.contains("Handoff"),
                    "focal must not elect Caretake/Handoff when no dependent \
                     cat exists in the colony; tick {} chose {chosen:?}",
                    tick.tick,
                );
            }
        }
    }
}
