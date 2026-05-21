//! Ticket 400 scenario — ParentingActivity persists past kitten death
//! (grief-substrate foundation, §7.7.b).
//!
//! Spawns an engaged mother whose `ParentingActivity` carries a Biological
//! `RelationshipTo` toward a kitten that's already despawned (or never
//! spawned — the relationship target is a stale Entity). The
//! `tick_parental_engagement` system reads `target_alive == false`, drops
//! the asymptote to `matured_residual_factor × asymptote` (~0.15× of
//! full), and decays the gradient toward that residual. The Component is
//! NOT removed; the entry stays with its (now-dangling) target. This is
//! the substrate foundation for the §7.7.b grief cascade — the
//! desire-target gap is the mechanic.
//!
//! Verification: after the scenario's tick budget, the mother still
//! carries `ParentingActivity` and the entry's `parental_engagement` is
//! lower than the pre-loaded value but still > 0.

use bevy_ecs::world::World;

use crate::components::parenting_activity::{ParentalKind, ParentingActivity, RelationshipTo};
use crate::components::physical::Position;
use crate::systems::parenting_activity::parental_engagement_asymptote;

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

pub static SCENARIO: Scenario = Scenario {
    name: "parenting_grief_kitten_death",
    default_focal: "Briar",
    // 30 ticks of decay-EMA — at decay rate 0.0001 toward residual
    // ~0.10, engagement falls roughly 3% per tick × 30 ≈ visible but
    // still well above zero.
    default_ticks: 30,
    setup,
    expected_features: &[],
};

fn setup(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    let current_tick = world.resource::<crate::resources::TimeState>().tick;

    // Engaged mother — high compassion gives a high asymptote so we
    // start with a clear engagement-vs-residual delta to observe.
    let briar = spawn_cat(
        world,
        CatPreset::adult("Briar", Position::new(20, 20))
            .with_personality(|p| {
                p.compassion = 0.9;
                p.warmth = 0.8;
                p.diligence = 0.6;
                p.loyalty = 0.7;
            })
            // Note: no Parent marker — the kitten is gone, so by the
            // marker semantic ("has at least one living dependent
            // kitten") Briar is no longer an active parent. But her
            // ParentingActivity entry persists per the design contract.
            .with_marker(MarkerKind::Adult),
    );

    // Spawn a "ghost target" — an entity with no Components beyond
    // existence; the parenting entry's `target` points at it. The
    // tick_parental_engagement system reads target_alive=false (no
    // KittenDependency) and applies the matured/dead asymptote path.
    let ghost_target = world.spawn(()).id();

    // Pre-populate ParentingActivity at full asymptote so the decay
    // trajectory is clearly observable in trace.
    let asymptote = {
        let personality = world
            .get::<crate::components::personality::Personality>(briar)
            .expect("Briar has Personality")
            .clone();
        let constants = world.resource::<crate::resources::SimConstants>();
        parental_engagement_asymptote(&personality, 0.0, &constants.parenting)
    };
    let mut rel = RelationshipTo::new(ghost_target, ParentalKind::Biological, None, current_tick);
    rel.parental_engagement = asymptote;
    world.entity_mut(briar).insert(ParentingActivity {
        relationships: vec![rel],
    });
}
