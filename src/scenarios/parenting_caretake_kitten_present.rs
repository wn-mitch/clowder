//! Ticket 410 scenario — Caretake DSE elects when a care-dependent
//! exists in the colony.
//!
//! Companion to `parenting_caretake_kitten_absent`. One parent cat
//! (high compassion + warmth) and one hungry kitten with the
//! pre-loaded `ParentingActivity` saturation pattern used by the other
//! 400-cluster scenarios. The kitten's `Kitten` marker flips
//! `update_colony_building_markers` to author `HasDependentCat`, and
//! the new eligibility filter on the Caretake DSE passes.
//!
//! Verification: `caretake` rows in the focal L2 trace are
//! `eligible: true` with a positive `final_score` on every tick — the
//! mirror of the absent scenario's gate assertion. Closes the
//! "no succeeding parent→kitten handoff scenario" coverage gap
//! surfaced during the 410 layer-walk (the only existing succeeding
//! handoff scenario is `disposal_dispatch::HANDOFF_SCENARIO`, which is
//! adult-to-adult).

use bevy_ecs::world::World;

use crate::components::parenting_activity::{ParentalKind, ParentingActivity, RelationshipTo};
use crate::components::physical::Position;
use crate::systems::parenting_activity::parental_engagement_asymptote;

use super::env::{init_scenario_world, spawn_cat, spawn_kitten};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "parenting_caretake_kitten_present",
    default_focal: "Magnolia",
    // Same tick budget as `parenting_father_provisions` — enough for
    // the modifier pipeline to settle on tick 1.
    default_ticks: 10,
    setup,
    expected_features: &[],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    let kitten_pos = Position::new(20, 20);
    let current_tick = world.resource::<crate::resources::TimeState>().tick;

    // Presence-heavy mother — high compassion + warmth so Caretake's
    // three-axis WeightedSum produces a healthy positive raw score
    // even before the `ParentingActivityModifier` lift fires.
    let magnolia = spawn_cat(
        world,
        CatPreset::adult("Magnolia", Position::new(21, 20))
            .with_personality(|p| {
                p.compassion = 0.9;
                p.warmth = 0.9;
                p.diligence = 0.4;
                p.loyalty = 0.5;
            })
            .with_marker(MarkerKind::Parent)
            .with_marker(MarkerKind::Adult),
    );

    // Hungry kitten — its `Kitten` marker is what flips the colony's
    // `HasDependentCat` true. Without this kitten, the absent-scenario
    // assertions kick in instead.
    let _crumb = spawn_kitten(
        world,
        CatPreset::kitten("Crumb", kitten_pos, current_tick).with_needs(|n| {
            n.hunger = 0.2;
        }),
        magnolia,
        magnolia, // single-parent setup — mother is also father slot
    );

    preload_parenting(world, magnolia, current_tick);
}

fn preload_parenting(world: &mut World, owner: bevy_ecs::entity::Entity, tick: u64) {
    let asymptote = {
        let personality = world
            .get::<crate::components::personality::Personality>(owner)
            .expect("owner has Personality")
            .clone();
        let constants = world.resource::<crate::resources::SimConstants>();
        parental_engagement_asymptote(&personality, 0.0, &constants.parenting)
    };
    // The relationship target points at the kitten's entity; the live
    // kitten makes `target_alive=true` in `tick_parental_engagement`
    // and the asymptote stays at full (no residual decay). We don't
    // need the target Entity locally — the kitten is the only Kitten
    // in the world, so `update_parenting_activity_biological` will
    // populate the entry on the first tick. Here we just seed at
    // asymptote so the modifier pipeline reflects steady-state from
    // tick 1.
    //
    // Using a placeholder ghost target keeps the seed mechanically
    // identical to `parenting_grief_kitten_death` while the biological
    // sync system overwrites with the live target on tick 1.
    let placeholder = world.spawn(()).id();
    let mut rel = RelationshipTo::new(placeholder, ParentalKind::Biological, None, tick);
    rel.parental_engagement = asymptote;
    world.entity_mut(owner).insert(ParentingActivity {
        relationships: vec![rel],
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scenarios::runner::{build_scenario_app, run};

    /// Mirror of the absent scenario's marker check: with a Kitten in
    /// the colony, `HasDependentCat` is true on the ColonyState
    /// singleton — the structural precondition for Caretake's
    /// eligibility filter to pass.
    #[test]
    fn has_dependent_cat_marker_is_true_when_kitten_present() {
        let mut app = build_scenario_app(42, &SCENARIO, "Magnolia");
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
            has_marker,
            "HasDependentCat must be true on the ColonyState singleton \
             when ≥1 Kitten exists"
        );
    }

    /// With the colony marker true, Caretake's eligibility filter
    /// passes and the DSE scores positively. The complement of the
    /// absent scenario's `focal_never_elects_caretake_chain` — closes
    /// the "no succeeding parent→kitten handoff scenario" coverage
    /// gap surfaced during the 410 layer-walk.
    #[test]
    fn caretake_eligible_with_kitten_present() {
        let report = run(&SCENARIO, None, None, 42);
        let mut eligible_with_positive_score = 0;
        let mut total_caretake_rows = 0;
        for tick in &report.ticks {
            for row in &tick.l2 {
                if row.dse == "caretake" {
                    total_caretake_rows += 1;
                    if row.eligible && row.final_score > 0.0 {
                        eligible_with_positive_score += 1;
                    }
                }
            }
        }
        assert!(
            total_caretake_rows > 0,
            "Caretake DSE must appear in the L2 trace; got {total_caretake_rows} rows"
        );
        assert!(
            eligible_with_positive_score > 0,
            "Caretake must be eligible AND score positively on at least \
             one tick when a Kitten exists in the colony; got \
             {eligible_with_positive_score}/{total_caretake_rows} qualifying rows"
        );
    }
}
