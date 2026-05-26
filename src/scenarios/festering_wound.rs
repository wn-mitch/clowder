//! Ticket 472 — festering-wound substrate microexperiment.
//!
//! Verifies the four substrate seams added by 472:
//!
//! 1. **Authoring** — `CatBodyModel::apply_damage_with_kind` lands a
//!    `WoundKind::Festering` wound on a body part.
//! 2. **Persistence** — `BodyZoneHealing::festering_heal_rate_multiplier`
//!    (0.05) makes the wound recover ~20× more slowly, so without
//!    intervention the festering kind is still on the part after the
//!    scenario tick budget.
//! 3. **Emission** — `emit_festering_observations` broadcasts
//!    `WitnessableEvent::CarriesFesteringWound` at the throttled cadence
//!    (`festering_observation_interval_ticks`).
//! 4. **Belief lift** — `belief_integrator` arm consumes the cue and
//!    raises `perceived_injury_level` on the witness's `MentalModel<actor>`.
//!
//! Two cats: `Ashitaka` (focal, preloaded festering on FrontRightPaw) and
//! `Mononoke` (bonded peer in sensing range). 250-tick budget — enough
//! for the throttled emit interval (200 ticks) to fire at least once
//! and for the belief integrator to consume + EMA the lift.
//!
//! Pre-473 the `SeekHealing` HTN method is dormant (`PendingSubstrate`),
//! so this scenario verifies the *passive* substrate path (persistence
//! + observation + belief lift) — the active-healing path lives in 473.

use bevy_ecs::world::World;

use crate::components::body_zones::{BodyPart, CatBodyModel, WoundKind};
use crate::components::physical::Position;
use crate::resources::sim_constants::SimConstants;

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

const FOCAL_NAME: &str = "Ashitaka";
const PARTNER_NAME: &str = "Mononoke";

pub static SCENARIO_FESTERING_WOUND: Scenario = Scenario {
    name: "festering_wound",
    default_focal: FOCAL_NAME,
    // 200-tick observation interval + ~30 ticks for belief EMA + a
    // little headroom. Avoids over-running and accruing noise from
    // unrelated drift.
    default_ticks: 250,
    setup: setup_festering_wound,
    // Belief layer is L1-only; no `Feature::*` emit expected.
    expected_features: &[],
};

fn setup_festering_wound(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Spawn focal first and apply a festering wound to FrontRightPaw
    // (Ashitaka's arm). High warmth on the partner so 473 follow-ons
    // that gate kin-care eligibility on warmth see them as a high-care
    // observer — for 472 the warmth doesn't matter (belief lift is
    // unconditional in 258's apply_observation), but it makes the
    // fixture more useful for 473's reuse.
    let focal = spawn_cat(
        world,
        CatPreset::adult(FOCAL_NAME, Position::new(20, 20))
            .with_personality(|p| p.warmth = 0.7)
            .with_marker(MarkerKind::Adult),
    );

    // Stamp the festering wound directly on the body model — the
    // misfire-authoring path is exercised by the unit tests in
    // `systems::magic::tests`; this scenario tests the downstream
    // observation + belief substrate.
    let constants = world.resource::<SimConstants>().clone();
    let mut body_model = world
        .get_mut::<CatBodyModel>(focal)
        .expect("focal cat must carry CatBodyModel");
    body_model.apply_damage_with_kind(
        BodyPart::FrontRightPaw,
        constants.magic.festering_seed_damage,
        WoundKind::Festering,
        &constants.combat.body_zone_condition_thresholds,
        &constants.combat.body_zone_permanent_at_destroyed,
    );

    spawn_cat(
        world,
        CatPreset::adult(PARTNER_NAME, Position::new(21, 20))
            .with_personality(|p| p.warmth = 0.9)
            .with_marker(MarkerKind::Adult),
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    use crate::components::beliefs::CatBeliefs;
    use crate::components::identity::{Name, Species};
    use crate::scenarios::runner::build_scenario_app;
    use bevy_ecs::prelude::*;

    fn cat_by_name(world: &mut World, name: &str) -> Entity {
        let mut q = world.query_filtered::<(Entity, &Name), With<Species>>();
        q.iter(world)
            .find(|(_, n)| n.0 == name)
            .map(|(e, _)| e)
            .unwrap_or_else(|| panic!("cat named {name:?} not found"))
    }

    fn injury_level(world: &World, witness: Entity, actor: Entity) -> f32 {
        world
            .get::<CatBeliefs>(witness)
            .expect("witness has CatBeliefs")
            .models
            .get(&actor)
            .map(|m| m.perceived_injury_level.value)
            .unwrap_or(0.0)
    }

    fn run_scenario_ticks(scenario: &Scenario, ticks: u32) -> bevy::app::App {
        let mut app = build_scenario_app(42, scenario, scenario.default_focal);
        app.update();
        for _ in 0..ticks {
            app.update();
        }
        app
    }

    #[test]
    fn festering_wound_persists_under_passive_heal() {
        // 472 — `festering_heal_rate_multiplier = 0.05` ⇒ a wound on a
        // structural part (paw, default heal 0.05 days for Bruised)
        // takes ~20× as long. Over 250 ticks the kind must still be
        // Festering and the part must remain non-healthy.
        let mut app = run_scenario_ticks(&SCENARIO_FESTERING_WOUND, 250);
        let world = app.world_mut();
        let ashitaka = cat_by_name(world, FOCAL_NAME);
        let body = world
            .get::<CatBodyModel>(ashitaka)
            .expect("focal has CatBodyModel");
        let paw = body.part(BodyPart::FrontRightPaw);
        assert_eq!(
            paw.kind,
            WoundKind::Festering,
            "festering kind must persist without active intervention"
        );
        assert!(
            paw.tissue_damage > 0.0,
            "tissue_damage should not have decayed to zero under \
             festering heal rate; got {}",
            paw.tissue_damage
        );
    }

    #[test]
    fn nearby_peer_perceives_festering_wound() {
        // 472 — co-located bonded peer should accrue
        // perceived_injury_level on the festering cat after at least
        // one `CarriesFesteringWound` emit + belief-integrator pass.
        let mut app = run_scenario_ticks(&SCENARIO_FESTERING_WOUND, 250);
        let world = app.world_mut();
        let ashitaka = cat_by_name(world, FOCAL_NAME);
        let mononoke = cat_by_name(world, PARTNER_NAME);

        let mononoke_on_ashitaka = injury_level(world, mononoke, ashitaka);
        assert!(
            mononoke_on_ashitaka > 0.0,
            "Mononoke should perceive Ashitaka's festering wound; \
             perceived_injury_level was {mononoke_on_ashitaka}"
        );

        // Self-witness skipped per 258 invariant — Ashitaka should NOT
        // accrue perceived_injury_level on themselves via the
        // CarriesFesteringWound arm (self-festering is handled by the
        // 089 OwnInjurySite anchor, not by the social belief layer).
        let ashitaka_on_self = injury_level(world, ashitaka, ashitaka);
        assert_eq!(
            ashitaka_on_self, 0.0,
            "self-witness must be skipped on CarriesFesteringWound; \
             Ashitaka's self-model perceived_injury_level was \
             {ashitaka_on_self}"
        );
    }
}
