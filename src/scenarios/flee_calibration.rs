//! Ticket 255 microexperiment — `ThreatProximityAdrenalineFlee`
//! Flee-axis calibration probe (post-252 regime).
//!
//! Sibling to [`flee_commitment`](super::flee_commitment) (ticket 230,
//! which validates *commitment shape* — the cat sticks with Fleeing
//! once elected). This scenario validates *calibration magnitude* —
//! does `flee_lift = 0.6` produce the right Flee election rate across
//! the four canonical threat-by-terrain corners, and does `sleep_lift
//! = 0.5` preserve the 047 doctrine ("Sleep is the in-pool partner;
//! Flee is rare") in the post-252 regime?
//!
//! Pre-252, `Action::Flee` was filtered from the L3 softmax pool
//! (`scoring.rs:2411`); 108's Flee-axis lift was architecturally
//! orphaned and only the Sleep-axis partner mattered. 252 lifted the
//! filter, so `flee_lift` is now reachable. The audit ticket (255)
//! poses two questions:
//!
//! 1. **Q1 (calibration magnitude):** Does `flee_lift = 0.6` produce
//!    the right Flee election rate for the new "Flee can win" regime
//!    — rare under low threat, dominant under rising threat with open
//!    escape, suppressed when cornered?
//! 2. **Q2 (sleep_lift redundancy):** Is 108's `sleep_lift = 0.5`
//!    redundant with 251's Sleep-DSE `health_deficit` Logistic axis?
//!    Verdict from code-reading: NO — `sleep_lift` keys on
//!    `threat_proximity_derivative` (an axis Sleep DSE has no
//!    consideration for); 251 only absorbed `health_deficit`-driven
//!    Sleep urgency. The variant `flee_calibration_sleep_partner`
//!    exercises both signals to demonstrate the orthogonal coverage.
//!
//! All four variants intentionally probe **L2/L3 election only** —
//! none depend on `PickFleeTarget` executing end-to-end (that path is
//! gated by ticket 254's witness-contract fix). `expected_features:
//! &[]` opts out of Feature gating because the probe deliberately
//! stops at the softmax winner; any mid-plan failure downstream
//! (picker witness, hold timeout) is out of scope.
//!
//! ## Variants
//!
//! | Variant                         | derivative | viability | Expected L3 winner    |
//! |---------------------------------|-----------:|----------:|-----------------------|
//! | `flee_calibration_low_threat`   |       ~0.0 |      ~0.7 | NOT Flee              |
//! | `flee_calibration_open_terrain` |       ~0.7 |      ~0.7 | Flee                  |
//! | `flee_calibration_cornered`     |       ~0.7 |     <0.4  | Fight (102 suppresses)|
//! | `flee_calibration_sleep_partner`|       ~0.7 |      ~0.7 | Sleep ≥ Flee (047)    |
//!
//! `escape_viability` defaults: `terrain_weight = 0.7`, `sprint_radius
//! = 3` ⇒ open-terrain viability ≈ 0.7, cornered (3×3 grass patch in
//! walls) ≈ 0.13. The 108 Flee-branch gate trips at viability ≥ 0.4.
//!
//! `threat_proximity_derivative = max(0, safety_deficit_now - prev)`,
//! and freshly-spawned cats start with `PrevSafetyDeficit(0.0)`; so
//! `n.safety = 0.3` at preset time gives a tick-1 derivative of ~0.7
//! (deficit = 0.7, prev = 0.0). The 108 ramp threshold is 0.4 — well
//! below that.

use bevy_ecs::world::World;

use crate::components::physical::{Health, Position};
use crate::components::wildlife::WildAnimal;
use crate::resources::map::{Terrain, TileMap};

use super::env::{init_scenario_world, spawn_cat};
use super::preset::{CatPreset, MarkerKind};
use super::Scenario;

/// Focal-cat name shared across all four variants. The runner sets
/// `FocalTraceTarget` on this name and resolves it on tick 1.
pub const FOCAL_NAME: &str = "Probe";
/// Focal start position — center of the 40×40 default map. Variants
/// that wall off the corner override this.
pub const FOCAL_START: Position = Position { x: 20, y: 20 };
/// Wildlife threat position — adjacent to the focal so detection is
/// guaranteed without scent buildup. Distance 2 keeps the cat outside
/// the wildlife's combat range while inside the threat-proximity
/// detection radius.
pub const THREAT_POS: Position = Position { x: 22, y: 20 };
/// Cornered-variant focal start — a (1, 1) tile inside a walled box
/// whose only walkable footprint is the (0..=2, 0..=2) 3×3 grass
/// patch. Mirrors the `escape_viability_low_in_corner` unit test in
/// `interoception.rs:1246`. Sprint-radius 3 → 7×7 = 49-tile sample;
/// 9 walkable / 49 ≈ 0.184; viability = 0.7 × 0.184 ≈ 0.13. Below
/// the 0.4 Flee gate; 102's Fight branch fires instead.
pub const CORNERED_FOCAL: Position = Position { x: 1, y: 1 };
/// Cornered-variant threat — *inside* the 3×3 grass patch so it
/// remains perceivable. Outside-the-patch positions become wall tiles
/// and the wildlife query stops returning them as `nearest_threat`,
/// causing `escape_viability` to short-circuit to 1.0 (the no-threat
/// safety contract) — which would silently pass 108's viability gate.
/// Placing the threat at (2, 1) keeps the fox on a walkable tile,
/// distance 1 from the focal at (1, 1), so perception holds and the
/// openness term still reads the cornered 9-walkable / 49-area sample.
pub const CORNERED_THREAT: Position = Position { x: 2, y: 1 };

// ---------------------------------------------------------------------------
// Variant 1 — low-threat baseline. Flee should NOT win.
// ---------------------------------------------------------------------------

pub static SCENARIO_LOW_THREAT: Scenario = Scenario {
    name: "flee_calibration_low_threat",
    default_focal: FOCAL_NAME,
    default_ticks: 4,
    setup: setup_low_threat,
    // L3-election triage; the substrate variants of 230/252 don't
    // expose Feature emission for "Flee did NOT elect", and we only
    // care about the softmax winner here.
    expected_features: &[],
};

fn setup_low_threat(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // High safety (deficit ~0.05) ⇒ tick-1 derivative ~0.05, well
    // below the 108 ramp threshold of 0.4. Modifier returns
    // `score` unchanged — Flee gets no lift.
    let _cat = spawn_cat(
        world,
        CatPreset::adult(FOCAL_NAME, FOCAL_START)
            .with_personality(|p| {
                p.boldness = 0.5;
                p.diligence = 0.5;
                p.patience = 0.5;
            })
            .with_needs(|n| {
                n.safety = 0.95;
                n.hunger = 0.6;
                n.energy = 0.7;
            })
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanHunt),
    );

    // Spawn a fox so `escape_viability` doesn't short-circuit to 1.0,
    // but the threat-proximity derivative is what the 108 modifier
    // actually gates on — and that's driven by the safety-need delta,
    // not by the fox's presence per se.
    world.spawn((
        THREAT_POS,
        WildAnimal::new(crate::components::wildlife::WildSpecies::Fox),
    ));
}

// ---------------------------------------------------------------------------
// Variant 2 — open-terrain rising-threat. Flee SHOULD win.
// ---------------------------------------------------------------------------

pub static SCENARIO_OPEN_TERRAIN: Scenario = Scenario {
    name: "flee_calibration_open_terrain",
    default_focal: FOCAL_NAME,
    default_ticks: 4,
    setup: setup_open_terrain,
    expected_features: &[],
};

fn setup_open_terrain(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Low safety (deficit 0.7) ⇒ tick-1 derivative 0.7, above the
    // 0.4 ramp threshold. 40×40 grass map ⇒ openness 1.0,
    // viability = 0.7. Both 108 gates pass; Flee gets `flee_lift`,
    // Sleep gets `sleep_lift`.
    let _cat = spawn_cat(
        world,
        CatPreset::adult(FOCAL_NAME, FOCAL_START)
            .with_personality(|p| {
                p.boldness = 0.5;
                p.diligence = 0.5;
                p.patience = 0.5;
            })
            .with_needs(|n| {
                n.safety = 0.3;
                n.hunger = 0.6;
                n.energy = 0.7;
            })
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanHunt),
    );

    world.spawn((
        THREAT_POS,
        WildAnimal::new(crate::components::wildlife::WildSpecies::Fox),
    ));
}

// ---------------------------------------------------------------------------
// Variant 3 — cornered. Fight should win; Flee suppressed.
// ---------------------------------------------------------------------------

pub static SCENARIO_CORNERED: Scenario = Scenario {
    name: "flee_calibration_cornered",
    default_focal: FOCAL_NAME,
    default_ticks: 4,
    setup: setup_cornered,
    expected_features: &[],
};

fn setup_cornered(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Wall off the map outside the (0..=2, 0..=2) 3×3 grass patch.
    // The default `init_scenario_world` builds a 40×40 grass map; we
    // overwrite everything *outside* the corner patch with
    // `Terrain::Wall` (impassable per `Terrain::movement_cost = u32::MAX`).
    {
        let mut map = world.resource_mut::<TileMap>();
        for y in 0..40 {
            for x in 0..40 {
                let inside_corner = x <= 2 && y <= 2;
                if !inside_corner {
                    map.set(x, y, Terrain::Wall);
                }
            }
        }
    }

    // Wounded so 102's Fight gate (`health_deficit > 0.4`) trips:
    // health 0.5 ⇒ deficit 0.5. Combined with low viability (cornered
    // → ~0.13), 102 suppresses Flee by `fight_lift` and lifts Fight.
    let cat = spawn_cat(
        world,
        CatPreset::adult(FOCAL_NAME, CORNERED_FOCAL)
            .with_personality(|p| {
                p.boldness = 0.5;
                p.diligence = 0.5;
                p.patience = 0.5;
            })
            .with_needs(|n| {
                n.safety = 0.3;
                n.hunger = 0.6;
                n.energy = 0.7;
            })
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanHunt),
    );

    // Wound the cat to drive `health_deficit = 0.5` so 102's gate
    // (`health_deficit > 0.4`) passes alongside the cornered viability.
    world.entity_mut(cat).insert(Health {
        current: 0.5,
        max: 1.0,
        injuries: Vec::new(),
        total_starvation_damage: 0.0,
    });

    world.spawn((
        CORNERED_THREAT,
        WildAnimal::new(crate::components::wildlife::WildSpecies::Fox),
    ));
}

// ---------------------------------------------------------------------------
// Variant 4 — Sleep-vs-Flee in-pool partner doctrine probe.
// ---------------------------------------------------------------------------

pub static SCENARIO_SLEEP_PARTNER: Scenario = Scenario {
    name: "flee_calibration_sleep_partner",
    default_focal: FOCAL_NAME,
    default_ticks: 4,
    setup: setup_sleep_partner,
    expected_features: &[],
};

fn setup_sleep_partner(world: &mut World, seed: u64) {
    init_scenario_world(world, seed);

    // Same threat regime as `open_terrain` (derivative ~0.7,
    // viability 0.7) so 108 fires. Add wound + energy-deficit so
    // Sleep DSE's substrate axes (251's `health_deficit` Logistic +
    // Sleep's `energy_deficit` Logistic + `pain_level` Linear)
    // contribute on top. The doctrinal claim from 047: in this
    // composed regime, Sleep ≥ Flee.
    let cat = spawn_cat(
        world,
        CatPreset::adult(FOCAL_NAME, FOCAL_START)
            .with_personality(|p| {
                p.boldness = 0.5;
                p.diligence = 0.5;
                p.patience = 0.5;
            })
            .with_needs(|n| {
                n.safety = 0.3;
                n.hunger = 0.6;
                // Energy deficit ~0.6 so 107 (ExhaustionPressure)
                // also lifts Sleep, layering on top of 108's
                // `sleep_lift`. This is the worst-case for Q1
                // ("Flee crowds Sleep out") — if Sleep wins here,
                // calibration is balanced; if Flee wins, the 047
                // doctrine has flipped post-252.
                n.energy = 0.4;
            })
            .with_marker(MarkerKind::Adult)
            .with_marker(MarkerKind::CanHunt),
    );

    // Wound to deficit 0.5 (post-251 Logistic axis: midpoint 0.4,
    // saturates above ~0.5).
    world.entity_mut(cat).insert(Health {
        current: 0.5,
        max: 1.0,
        injuries: Vec::new(),
        total_starvation_damage: 0.0,
    });

    world.spawn((
        THREAT_POS,
        WildAnimal::new(crate::components::wildlife::WildSpecies::Fox),
    ));
}
