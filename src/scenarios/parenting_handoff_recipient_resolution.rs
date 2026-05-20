//! L3-resolver scenario — `HandoffItem` must find a recipient on the
//! goap-path, not only when the disposition-chain seeded `target_entity`.
//!
//! # Why this scenario exists
//!
//! Surfaced during the 2026-05-20 audit of the `afk-overnight-2026-05-19`
//! 6h soak: 177,190 `HandoffItem: no recipient on disposition (no
//! dependent cat in colony)` plan-failures (rate 0.27/tick) across 5 sim
//! years, paired with 22 starvations (20 of them kittens). Sibling
//! finding to ticket 273 (Caretake election crowded out by perception
//! ratchet) — that ticket is parked behind upstream perception fixes.
//! This scenario isolates a *separate* defect 273 didn't audit: the
//! goap-path resolver's empty-snapshot fallback.
//!
//! # The defect
//!
//! `HandoffItem`'s goap-path resolver at `src/systems/goap.rs:7322-7344`
//! attempts to re-resolve a recipient when `target_entity.is_none()`:
//!
//! ```text
//! if plan.step_state[step_idx].target_entity.is_none() {
//!     plan.step_state[step_idx].target_entity = snaps
//!         .kitten_snapshot
//!         .iter()
//!         .min_by(...)
//!         .map(|k| k.entity);
//! }
//! let Some(recipient) = plan.step_state[step_idx].target_entity else {
//!     return Fail("handoff: no recipient on disposition ...");
//! };
//! ```
//!
//! But `snaps.kitten_snapshot` is statically `Vec::new()` at
//! `goap.rs:3825`. The only writer is the initializer; only readers are
//! `goap.rs:6310` (FeedKitten — no-ops gracefully on empty) and 7323
//! (HandoffItem — hard-fails). The "intentionally empty" comment at
//! 3819-3825 confesses the borrow-checker reason: the cats query owns
//! `&mut Needs` over every non-dead cat including kittens, so a parallel
//! immutable kitten query would conflict.
//!
//! `target_entity` can be `None` at HandoffItem resolve time via the
//! eight `step.target_entity = None` clear sites in `disposition.rs`
//! (lines 1765, 3593, 3609, 3754, 3821, 3853, 3858, 3870) — mid-plan
//! clears when targets drift out of range, search-for-another paths, etc.
//! The disposition-chain path re-builds with a fresh roster on those
//! clears, but goap-path re-entry doesn't.
//!
//! # Why softmax-election isn't part of this test
//!
//! An earlier draft tried to drive Caretake naturally via L2/L3
//! election. The cat picked Caretake at L2 (`final_score = 1.82`,
//! softmax `p = 60%`) but the GoapPlan committed to Wander on tick 3 and
//! the plan-execution mechanism kept it there for the rest of the run
//! (`current_step` advances through the Wander plan; L2 doesn't
//! re-elect mid-plan). That's the same crowding ticket 273 describes at
//! the colony scale — not the resolver bug. This rewrite **pre-injects
//! a `GoapPlan` with a `HandoffItem` step and `target_entity = None`**,
//! bypassing election entirely. The resolver runs on the first
//! `FixedUpdate` tick. Post-fix: the resolver reads from a populated
//! recipient roster and the kitten is fed. Pre-fix: the resolver reads
//! the static `Vec::new()` and emits the canary string.
//!
//! # Why this is a substrate defect, not a tuning question
//!
//! The substrate-over-hacks design pillar says: if a cat has elected
//! Caretake and committed to feeding a kitten, the resolver should not
//! silently fail to find a recipient when one exists in range. The
//! current empty-snapshot path violates the pillar — the cat is
//! committed, the kitten exists, and the resolver fails for a reason
//! that has nothing to do with reachability. This is the same class as
//! tickets 209 / 084 (substrate-stub silent-fails): writer authored,
//! reader present, but the snapshot the consumer reads is never
//! populated.

use std::collections::HashSet;

use bevy_ecs::prelude::*;
use bevy_ecs::world::World;

use crate::ai::planner::{GoapActionKind, PlannedStep};
use crate::ai::Action;
use crate::components::disposition::DispositionKind;
use crate::components::goap_plan::{GoapPlan, StepExecutionState};
use crate::components::identity::Name;
use crate::components::items::ItemKind;
use crate::components::magic::Inventory;
use crate::components::parenting_activity::{ParentalKind, ParentingActivity, RelationshipTo};
use crate::components::physical::Position;
use crate::systems::parenting_activity::parental_engagement_asymptote;

use super::env::{init_scenario_world, spawn_cat, spawn_kitten};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "parenting_handoff_recipient_resolution",
    default_focal: "Magnolia",
    // Minimal — the test only needs the goap-path dispatcher to run
    // HandoffItem once. Anything past that is replan/abandon territory
    // we explicitly don't want to assert against.
    default_ticks: 3,
    setup,
    expected_features: &[],
};

const PARENT_NAME: &str = "Magnolia";
const KITTEN_NAME: &str = "Crumb";
const PARENT_POS: Position = Position { x: 20, y: 20 };
const KITTEN_POS: Position = Position { x: 20, y: 20 };

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    let current_tick = world.resource::<crate::resources::TimeState>().tick;

    let parent = spawn_cat(
        world,
        CatPreset::adult(PARENT_NAME, PARENT_POS)
            .with_personality(|p| {
                p.compassion = 0.9;
                p.warmth = 0.9;
                p.diligence = 0.6;
                p.loyalty = 0.7;
            })
            .with_marker(MarkerKind::Parent)
            .with_marker(MarkerKind::Adult),
    );

    // Hungry kitten in the same tile as the parent — co-located so the
    // resolver's nearest-hungry-kitten pick (when fixed) finds this one
    // unambiguously and no path-walk is needed before the handoff can
    // complete.
    let _kitten = spawn_kitten(
        world,
        CatPreset::kitten(KITTEN_NAME, KITTEN_POS, current_tick).with_needs(|n| {
            n.hunger = 0.05;
        }),
        parent,
        parent,
    );

    fill_parent_inventory(world, PARENT_NAME, ItemKind::RawMouse, 1);
    preload_parenting(world, parent, current_tick);
    inject_handoff_plan(world, parent, current_tick);
}

fn fill_parent_inventory(world: &mut World, name: &str, kind: ItemKind, count: usize) {
    let mut q = world.query::<(Entity, &Name)>();
    let entity = q
        .iter(world)
        .find(|(_, n)| n.0 == name)
        .map(|(e, _)| e)
        .expect("parent cat must exist before fill_parent_inventory");
    let mut em = world.entity_mut(entity);
    let mut inv = em.get_mut::<Inventory>().expect("parent has Inventory");
    for _ in 0..count {
        inv.add_item(kind);
    }
}

fn preload_parenting(world: &mut World, owner: Entity, tick: u64) {
    let asymptote = {
        let personality = world
            .get::<crate::components::personality::Personality>(owner)
            .expect("owner has Personality")
            .clone();
        let constants = world.resource::<crate::resources::SimConstants>();
        parental_engagement_asymptote(&personality, 0.0, &constants.parenting)
    };
    let placeholder = world.spawn(()).id();
    let mut rel = RelationshipTo::new(placeholder, ParentalKind::Biological, None, tick);
    rel.parental_engagement = asymptote;
    world.entity_mut(owner).insert(ParentingActivity {
        relationships: vec![rel],
    });
}

/// Inject a `GoapPlan` whose only step is `HandoffItem` with a `None`
/// target_entity — the exact precondition the goap-path resolver
/// fallback at `src/systems/goap.rs:7322-7344` is meant to recover from.
/// Pre-fix the resolver iterates the static `Vec::new()` snapshot and
/// emits `Fail("handoff: no recipient on disposition ...")`. Post-fix
/// the resolver finds the live kitten and the handoff completes.
///
/// The plan is marked as already-adopted (one tick ago) so the
/// disposition dispatcher treats it as in-flight rather than fresh
/// election.
fn inject_handoff_plan(world: &mut World, parent: Entity, current_tick: u64) {
    let plan = GoapPlan {
        steps: vec![PlannedStep {
            action: GoapActionKind::HandoffItem,
            cost: 1,
        }],
        current_step: 0,
        kind: DispositionKind::Caretaking,
        adopted_tick: current_tick.saturating_sub(1),
        trips_done: 0,
        target_trips: 1,
        replan_count: 0,
        max_replans: 3,
        chosen_action: Action::Handoff,
        step_state: vec![StepExecutionState::default()],
        ward_placement_pos: None,
        failed_actions: HashSet::new(),
    };
    world.entity_mut(parent).insert(plan);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::physical::Needs;
    use crate::scenarios::runner::build_scenario_app;

    /// The substrate-correctness assertion. The parent has an in-flight
    /// Caretaking plan with `HandoffItem` as its only step, no
    /// `target_entity`, food in inventory. The live kitten is
    /// co-located in the same tile, hunger = 0.05.
    ///
    /// Post-fix the goap-path resolver finds the kitten and the handoff
    /// completes — Crumb's hunger rises and Magnolia's inventory drops
    /// from 1 to 0. Pre-fix the resolver iterates the static `Vec::new()`
    /// snapshot, returns `StepResult::Fail("handoff: no recipient on
    /// disposition (no dependent cat in colony)")`, and nothing moves.
    // Ignored until ticket 428's R2b lands — this test is the failing
    // gate that the fix flips GREEN. Remove `#[ignore]` as part of the
    // landing PR. Per CLAUDE.md "Tests gate commits", landing a known-
    // failing test would regress CI; the `#[ignore]` keeps the harness
    // committable while preserving the assertion shape for the fix PR.
    #[test]
    #[ignore = "ticket 428 — fails until goap-path resolver is rebound to a populated kitten roster; un-ignore as part of fix PR"]
    fn goap_path_resolver_finds_live_kitten_with_none_target() {
        let mut app = build_scenario_app(42, &SCENARIO, PARENT_NAME);
        // Drain Startup (scenario setup runs here, including
        // `inject_handoff_plan`); then a single FixedUpdate tick. The
        // resolver runs in that one tick and either completes the
        // handoff (post-fix) or emits the canary fail (pre-fix).
        app.update();
        app.update();

        let world = app.world_mut();
        let kitten_hunger = read_kitten_hunger(world);
        let parent_inventory_slots = read_parent_inventory_slots(world);

        // Primary assertion: kitten fed (hunger rose). `feed_kitten` is
        // the resolver downstream of a successful `HandoffItem` —
        // post-fix this is non-zero, pre-fix the chain never reaches it.
        assert!(
            kitten_hunger > 0.05,
            "goap-path `HandoffItem` resolver must find the co-located \
             hungry kitten when `target_entity = None` — the substrate \
             pillar says a cat committed to Caretake should find the \
             recipient when one exists. Got kitten hunger={kitten_hunger} \
             (started at 0.05); resolver fell through to the canary \
             `handoff: no recipient on disposition (no dependent cat in \
             colony)` because `snaps.kitten_snapshot` is statically \
             `Vec::new()` at goap.rs:3825."
        );

        // Corroborating assertion: parent's inventory drained. Bracket
        // assertion in case the handoff transfer mechanics change but
        // the resolver still finds the kitten — splits "resolver
        // succeeded" from "the whole feed chain succeeded".
        assert_eq!(
            parent_inventory_slots, 0,
            "parent's RawMouse should have transferred to the kitten; \
             got {parent_inventory_slots} slots still occupied. If this \
             fails but the hunger assertion passed, the resolver found \
             the kitten but the transfer mechanism didn't complete \
             (different defect)."
        );
    }

    fn read_kitten_hunger(world: &mut World) -> f32 {
        let mut q = world.query::<(&Name, &Needs)>();
        q.iter(world)
            .find(|(n, _)| n.0 == KITTEN_NAME)
            .map(|(_, needs)| needs.hunger)
            .expect("kitten Crumb must be alive at end-of-scenario")
    }

    fn read_parent_inventory_slots(world: &mut World) -> usize {
        let mut q = world.query::<(&Name, &Inventory)>();
        q.iter(world)
            .find(|(n, _)| n.0 == PARENT_NAME)
            .map(|(_, inv)| inv.slots.len())
            .expect("parent Magnolia must be alive at end-of-scenario")
    }
}
