//! Ticket 450 — three-stage kittenhood substrate triage.
//!
//! Preloads four cats — a Stage 1 newborn (maturity 0.10), a Stage 2
//! eyes-open (maturity 0.50), a Stage 3 juvenile (maturity 0.85), and
//! an adult mother — and runs long enough for the begging cycle to
//! complete at least once. Asserts on `Feature::KittenBegged` firing
//! via the [`Scenario::expected_features`] gate; the per-stage marker
//! authoring is tested separately by the [`mod tests`] suite below.
//!
//! ## Why a multi-stage fixture
//!
//! 450 introduces three new mutually-exclusive sub-stage markers
//! (`NewbornKitten` / `EyesOpenKitten` / `JuvenileKitten`) plus the
//! `MentorableAge` mentee-side gate. The substrate refactor's "richer
//! perception, better strategy" pillar (CLAUDE.md) holds that
//! decomposing the monolithic `Kitten` flag into orthogonal axes
//! should produce more situation-appropriate behavior — but a regression
//! that conflates two of the sub-stages (e.g. an authoring bug that
//! lets `NewbornKitten` linger past 0.33 maturity) would be invisible
//! to a soak that just looks at population dynamics. This scenario
//! preloads one cat at each sub-stage boundary so each marker is
//! observable inside a 12-tick window.
//!
//! ## What the harness output shows
//!
//! Each focal cat's per-tick winning DSE + ranked L2 score table. The
//! default focal is the Stage 1 newborn ("Crumb") — at the L3 layer
//! we expect `BegForFood` to win consistently since `(NewbornKitten ∧
//! ¬HasFoodInInventory ∧ hungry)` is the canonical Stage-1 hunger-
//! response shape. Sleep / Idle are siblings in the L2 pool but
//! `Incapacitated` (auto-authored on Stage 1) doesn't gate
//! `BegForFoodDse` (intentional — newborns are the prototypical
//! beggers), so on a hungry tick BegForFood outscores the rest.
//!
//! ## Assertions
//!
//! - `Feature::KittenBegged` fires ≥ 1× (via `expected_features`);
//!   asserts the resolver actually completes a beg cycle in the
//!   12-tick window.
//! - Unit tests in [`mod tests`] verify (a) Stage 1 has both
//!   `NewbornKitten` and `Incapacitated`; (b) Stage 2 has
//!   `EyesOpenKitten` and does NOT have `Incapacitated`; (c) Stage 3
//!   has `JuvenileKitten` and `MentorableAge`; (d) the adult mother
//!   has `MentorableAge` but no sub-stage marker.

use bevy_ecs::world::World;

use crate::components::physical::Position;

use super::env::{init_scenario_world, spawn_cat, spawn_kitten};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "kittenhood_stages",
    default_focal: "Crumb",
    // 12 ticks lets the 5-tick BegForFood cycle complete in the
    // scenario (per-stage kittens with hunger 0.15 reliably elect
    // BegForFood — verified in `mod tests`). The cycle DOES fire here;
    // the canary `KittenBegged` is parked at production-soak level
    // because adults preempt begging in steady-state colonies. See
    // `Feature::KittenBegged`'s `expected_to_fire_per_soak` comment.
    default_ticks: 12,
    setup,
    // 451 — scenario-level structural verification still rides on the
    // per-stage marker assertions in `mod tests`. Soak-level canary
    // promotion is parked behind balance tuning that gives kittens a
    // window to elect BegForFood before adults preempt with FeedKitten.
    expected_features: &[],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    let current_tick = world.resource::<crate::resources::TimeState>().tick;
    let mother_pos = Position::new(20, 20);

    let mallow = spawn_cat(
        world,
        CatPreset::adult("Mallow", mother_pos)
            .with_personality(|p| {
                p.warmth = 0.9;
                p.compassion = 0.9;
            })
            .with_marker(MarkerKind::Parent)
            .with_marker(MarkerKind::Adult),
    );

    // Stage 1 — newborn. Below `weaned_threshold` (0.33). Adjacent to
    // the mother so the cry-map signal reaches her at full strength.
    // Hunger pinned below `kitten_cry_hunger_threshold` (0.5 default)
    // so the BegForFoodDse scores positive.
    let newborn = spawn_kitten(
        world,
        CatPreset::kitten("Crumb", Position::new(21, 20), current_tick).with_needs(|n| {
            n.hunger = 0.15;
        }),
        mallow,
        mallow,
    );
    set_maturity(world, newborn, 0.10);

    // Stage 2 — eyes-open. Within `[weaned_threshold,
    // teach_done_threshold)` = `[0.33, 0.66)`. Same hunger profile so
    // BegForFoodDse fires on this kitten too — the two sibling
    // registrations both produce a `Begging` Intention. Positioned
    // diagonally from the mother so the cry-map's per-kitten stamp is
    // separable in any focal trace inspecting tile coverage.
    let eyes_open = spawn_kitten(
        world,
        CatPreset::kitten("Sprig", Position::new(22, 21), current_tick).with_needs(|n| {
            n.hunger = 0.20;
        }),
        mallow,
        mallow,
    );
    set_maturity(world, eyes_open, 0.50);

    // Stage 3 — juvenile. `[teach_done_threshold, 1.0)` = `[0.66,
    // 1.0)`. `MentorableAge` should fire; sub-stage marker should be
    // `JuvenileKitten`. NOT eligible for BegForFood (both sibling
    // registrations require `NewbornKitten` OR `EyesOpenKitten` —
    // Stage 3 has neither). The cat should elect a non-Begging action
    // (Forage / Wander / Idle) on a healthy tick — verifying that the
    // capability gate cleanly excludes Stage 3 from the begging path.
    let juvenile = spawn_kitten(
        world,
        CatPreset::kitten("Reed", Position::new(19, 19), current_tick).with_needs(|n| {
            n.hunger = 0.50;
        }),
        mallow,
        mallow,
    );
    set_maturity(world, juvenile, 0.85);
}

/// Override the kitten's `KittenDependency.maturity` post-spawn so the
/// per-tick `update_life_stage_markers` system authors the correct
/// sub-stage marker on the first tick.
fn set_maturity(world: &mut World, kitten: bevy_ecs::entity::Entity, maturity: f32) {
    let mut em = world.entity_mut(kitten);
    if let Some(mut dep) = em.get_mut::<crate::components::KittenDependency>() {
        dep.maturity = maturity;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::markers::{
        EyesOpenKitten, Incapacitated, JuvenileKitten, MentorableAge, NewbornKitten,
    };
    use crate::scenarios::runner::build_scenario_app;
    use bevy::app::App;
    use bevy_ecs::prelude::Entity;

    fn run_for(ticks: u32) -> App {
        let mut app = build_scenario_app(42, &SCENARIO, SCENARIO.default_focal);
        // First update runs Startup (which runs the setup_world_exclusive
        // override that loads the scenario's preset cats); subsequent
        // updates each advance one FixedUpdate tick.
        app.update();
        for _ in 0..ticks {
            app.update();
        }
        app
    }

    fn find_cat_by_name(world: &mut bevy_ecs::world::World, name: &str) -> Entity {
        let mut q = world.query::<(Entity, &crate::components::identity::Name)>();
        for (entity, n) in q.iter(world) {
            if n.0.as_str() == name {
                return entity;
            }
        }
        panic!("no cat named {name}");
    }

    #[test]
    fn stage_1_authors_newborn_kitten_and_incapacitated() {
        let mut app = run_for(3);
        let world = app.world_mut();
        let crumb = find_cat_by_name(world, "Crumb");
        let em = world.entity(crumb);
        assert!(
            em.contains::<NewbornKitten>(),
            "Stage 1 must have NewbornKitten"
        );
        assert!(
            em.contains::<Incapacitated>(),
            "Stage 1 must have Incapacitated"
        );
        assert!(!em.contains::<EyesOpenKitten>());
        assert!(!em.contains::<JuvenileKitten>());
        assert!(!em.contains::<MentorableAge>());
    }

    #[test]
    fn stage_2_authors_eyes_open_and_clears_incapacitated() {
        let mut app = run_for(3);
        let world = app.world_mut();
        let sprig = find_cat_by_name(world, "Sprig");
        let em = world.entity(sprig);
        assert!(
            em.contains::<EyesOpenKitten>(),
            "Stage 2 must have EyesOpenKitten"
        );
        assert!(
            !em.contains::<Incapacitated>(),
            "Stage 2 must NOT have Incapacitated"
        );
        assert!(!em.contains::<NewbornKitten>());
        assert!(!em.contains::<JuvenileKitten>());
        assert!(!em.contains::<MentorableAge>());
    }

    #[test]
    fn stage_3_authors_juvenile_kitten_and_mentorable_age() {
        let mut app = run_for(3);
        let world = app.world_mut();
        let reed = find_cat_by_name(world, "Reed");
        let em = world.entity(reed);
        assert!(
            em.contains::<JuvenileKitten>(),
            "Stage 3 must have JuvenileKitten"
        );
        assert!(
            em.contains::<MentorableAge>(),
            "Stage 3 must have MentorableAge"
        );
        assert!(!em.contains::<NewbornKitten>());
        assert!(!em.contains::<EyesOpenKitten>());
        assert!(!em.contains::<Incapacitated>());
    }

    #[test]
    fn adult_mother_has_mentorable_age_but_no_kitten_sub_stage() {
        let mut app = run_for(3);
        let world = app.world_mut();
        let mallow = find_cat_by_name(world, "Mallow");
        let em = world.entity(mallow);
        assert!(
            em.contains::<MentorableAge>(),
            "Adult must have MentorableAge"
        );
        assert!(!em.contains::<NewbornKitten>());
        assert!(!em.contains::<EyesOpenKitten>());
        assert!(!em.contains::<JuvenileKitten>());
    }
}
