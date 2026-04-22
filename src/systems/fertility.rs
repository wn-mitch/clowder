//! Fertility phase-transition system — §7.M.7.2 + §7.M.7.7 of
//! `docs/systems/ai-substrate-refactor.md`.
//!
//! Two systems:
//!
//! 1. [`update_fertility_phase`] — runs at
//!    `FertilityConstants::update_interval_ticks` cadence (100 ticks
//!    default). Iterates all cats, inserts `Fertility` on Queens +
//!    Nonbinaries entering `Adult`, removes it on `Elder` / `Pregnant`
//!    / `Kitten|Young` regression, and recomputes `phase` via the
//!    pure-function [`phase_from`] for cats that carry it.
//!
//! 2. [`handle_post_partum_reinsert`] — runs every tick, listens for
//!    `RemovedComponents<Pregnant>` (the birth moment in
//!    `pregnancy.rs:tick_pregnancy`) and re-inserts `Fertility` on
//!    the birthing mother with `phase = Postpartum` + full
//!    `post_partum_remaining_ticks`.
//!
//! **Scope boundary.** This system authors *phase state*. The spec
//! (§7.M.7.4) calls for a second fix to `resolve_mate_with` so a Tom
//! initiator's Queen partner gets `Pregnant` rather than the Tom
//! himself. That's an independent bug predating Phase 3 (no Fertility
//! component required to reproduce) and is deferred to Phase 4 per
//! the Phase 3 acceptance-gate reading — gate item 7 asks for
//! "phase transitions consistent with §7.M.7.2," which is this file.

use bevy_ecs::prelude::*;

use crate::components::fertility::{Fertility, FertilityPhase};
use crate::components::identity::{Age, Gender, LifeStage};
use crate::components::physical::Dead;
use crate::components::pregnancy::Pregnant;
use crate::resources::sim_constants::{FertilityConstants, SimConstants};
use crate::resources::time::{SimConfig, Season, TimeState};

// ---------------------------------------------------------------------------
// Phase transition function (§7.M.7.2) — pure, deterministic.
// ---------------------------------------------------------------------------

/// Evaluate the §7.M.7.2 phase function. Inputs are all tick-derived,
/// season-derived, spawn-immutable, or event-stamped — two soaks with
/// matching seed and constants produce byte-identical traces.
///
/// Rule order (first match wins):
/// 1. `season == Winter` → `Anestrus`.
/// 2. `post_partum > 0` → `Postpartum` (countdown handled by caller).
/// 3. `cycle_tick < proestrus_end` → `Proestrus`.
/// 4. `cycle_tick < estrus_end` → `Estrus`.
/// 5. otherwise → `Diestrus`.
pub fn phase_from(
    cycle_tick: u32,
    season: Season,
    post_partum_remaining: u32,
    constants: &FertilityConstants,
) -> FertilityPhase {
    if season == Season::Winter {
        return FertilityPhase::Anestrus;
    }
    if post_partum_remaining > 0 {
        return FertilityPhase::Postpartum;
    }
    let cycle_len = constants.cycle_length_ticks as f32;
    let proestrus_end = (cycle_len * constants.proestrus_fraction) as u32;
    let estrus_end = proestrus_end + (cycle_len * constants.estrus_fraction) as u32;
    if cycle_tick < proestrus_end {
        FertilityPhase::Proestrus
    } else if cycle_tick < estrus_end {
        FertilityPhase::Estrus
    } else {
        FertilityPhase::Diestrus
    }
}

/// Derive `cycle_tick` for a cat: `(current_tick + cycle_offset) %
/// cycle_length_ticks`. Factored out so tests can reason about phase
/// without threading a full Fertility struct.
pub fn cycle_tick_for(current_tick: u64, cycle_offset: u64, cycle_length: u32) -> u32 {
    ((current_tick.wrapping_add(cycle_offset)) % cycle_length as u64) as u32
}

// ---------------------------------------------------------------------------
// update_fertility_phase — 100-tick cadence transition driver
// ---------------------------------------------------------------------------

/// Insert / remove / update `Fertility` on every eligible cat once
/// per `update_interval_ticks`. Gender-gated per §7.M.7.4: Toms skip
/// this path entirely (they use the §7.M.7.5 Tom-fallback for
/// scoring, with no marker).
#[allow(clippy::type_complexity)]
pub fn update_fertility_phase(
    time: Res<TimeState>,
    config: Res<SimConfig>,
    constants: Res<SimConstants>,
    mut query: Query<
        (
            Entity,
            &Age,
            &Gender,
            Option<&mut Fertility>,
            Option<&Pregnant>,
        ),
        Without<Dead>,
    >,
    mut commands: Commands,
) {
    let fertility = &constants.fertility;
    if !time.tick.is_multiple_of(fertility.update_interval_ticks as u64) {
        return;
    }

    let season = time.season(&config);
    let tps = config.ticks_per_season;

    for (entity, age, gender, maybe_fert, maybe_preg) in &mut query {
        let stage = age.stage(time.tick, tps);
        let gestation_capable = !matches!(*gender, Gender::Tom);

        // Pregnancy takes priority — Fertility is mutually exclusive
        // with Pregnant (§7.M.7.1). If both coexist (shouldn't happen
        // past `handle_conception_remove`, but defensive), remove.
        if maybe_preg.is_some() {
            if maybe_fert.is_some() {
                commands.entity(entity).remove::<Fertility>();
            }
            continue;
        }

        // Toms never cycle — no marker, no work.
        if !gestation_capable {
            continue;
        }

        // Pre-adult: no marker.
        if matches!(stage, LifeStage::Kitten | LifeStage::Young) {
            if maybe_fert.is_some() {
                commands.entity(entity).remove::<Fertility>();
            }
            continue;
        }

        // Elder exit: remove per §7.M.7.1 — Adult→Elder terminates
        // the reproductive window.
        if matches!(stage, LifeStage::Elder) {
            if maybe_fert.is_some() {
                commands.entity(entity).remove::<Fertility>();
            }
            continue;
        }

        // Adult Queen/NB without Fertility → insert with computed
        // phase (post_partum=0 since not post-birth; re-insert handler
        // owns that path).
        let Some(mut fert) = maybe_fert else {
            let offset_mix = 0x9E37_79B9_7F4A_7C15_u64;
            let cycle_offset = entity.to_bits().wrapping_mul(offset_mix);
            let cycle_tick = cycle_tick_for(time.tick, cycle_offset, fertility.cycle_length_ticks);
            let phase = phase_from(cycle_tick, season, 0, fertility);
            commands.entity(entity).insert(Fertility {
                phase,
                cycle_offset,
                post_partum_remaining_ticks: 0,
            });
            continue;
        };

        // Adult Queen/NB with Fertility: recompute phase, decrement
        // post_partum counter.
        let cycle_tick =
            cycle_tick_for(time.tick, fert.cycle_offset, fertility.cycle_length_ticks);
        let phase = phase_from(cycle_tick, season, fert.post_partum_remaining_ticks, fertility);
        fert.phase = phase;
        if fert.post_partum_remaining_ticks > 0 {
            fert.post_partum_remaining_ticks = fert
                .post_partum_remaining_ticks
                .saturating_sub(fertility.update_interval_ticks);
        }
    }
}

// ---------------------------------------------------------------------------
// handle_post_partum_reinsert — reactive to `Pregnant` removal
// ---------------------------------------------------------------------------

/// Re-insert `Fertility` with `phase = Postpartum` on a cat who just
/// lost `Pregnant` (birth event from `pregnancy.rs:tick_pregnancy`).
///
/// Uses `RemovedComponents<Pregnant>` to fire once per removal;
/// entity-existence + gender + life-stage gates match the
/// `update_fertility_phase` insert logic so re-inserts respect
/// §7.M.7.4 (Toms never carry) and §7.M.7.1 (post-birth Elder cats
/// don't resume cycling).
#[allow(clippy::type_complexity)]
pub fn handle_post_partum_reinsert(
    time: Res<TimeState>,
    config: Res<SimConfig>,
    constants: Res<SimConstants>,
    mut removed: RemovedComponents<Pregnant>,
    query: Query<(&Age, &Gender, Option<&Fertility>), Without<Dead>>,
    mut commands: Commands,
) {
    let fertility = &constants.fertility;
    let tps = config.ticks_per_season;

    for entity in removed.read() {
        // Entity may have died during birth — skip if no longer queryable.
        let Ok((age, gender, maybe_fert)) = query.get(entity) else {
            continue;
        };

        if matches!(*gender, Gender::Tom) {
            continue;
        }

        let stage = age.stage(time.tick, tps);
        if !matches!(stage, LifeStage::Adult) {
            // Young / Kitten / Elder: don't re-insert.
            continue;
        }

        // Preserve cycle_offset if we already had a stale Fertility
        // (shouldn't, since pregnancy removed it — but defensive).
        let cycle_offset = maybe_fert
            .map(|f| f.cycle_offset)
            .unwrap_or_else(|| entity.to_bits().wrapping_mul(0x9E37_79B9_7F4A_7C15));

        commands.entity(entity).insert(Fertility::on_post_partum(
            cycle_offset,
            fertility.post_partum_recovery_ticks,
        ));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::time::{SimConfig, TimeState};

    fn test_constants() -> FertilityConstants {
        FertilityConstants::default()
    }

    /// Drive `update_fertility_phase` once against a freshly-built
    /// World containing a single Adult Queen. Verifies that the system
    /// inserts `Fertility` as designed.
    #[test]
    fn update_fertility_phase_inserts_on_adult_queen() {
        let mut world = World::new();
        world.insert_resource(SimConstants::default());
        world.insert_resource(TimeState::default());
        world.insert_resource(SimConfig::default());

        // Spawn an Adult Queen — born far enough back that Age::stage
        // resolves to Adult (12+ seasons at ticks_per_season=20000).
        let born_tick = 0u64;
        let current_tick = 12 * 20_000u64;
        let mut time = world.resource_mut::<TimeState>();
        time.tick = current_tick;
        let queen = world
            .spawn((
                Age::new(born_tick),
                Gender::Queen,
            ))
            .id();

        // Run the system.
        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(update_fertility_phase);
        schedule.run(&mut world);

        // After one frame + command flush, the Queen must carry Fertility.
        assert!(
            world.get::<Fertility>(queen).is_some(),
            "Adult Queen should have Fertility inserted after update_fertility_phase"
        );
    }

    #[test]
    fn update_fertility_phase_mutates_existing_phase_to_anestrus_in_winter() {
        let mut world = World::new();
        world.insert_resource(SimConstants::default());
        world.insert_resource(SimConfig::default());
        world.insert_resource(TimeState::default());
        // Tick 300 000 = 15 seasons in → cat born at tick 0 is Adult
        // (LifeStage::Adult spans seasons 12–47). Modulo 80 000-tick
        // year puts us 60 000 ticks into the year = Winter onset.
        world.resource_mut::<TimeState>().tick = 300_000;

        let queen = world
            .spawn((
                Age::new(0),
                Gender::Queen,
                Fertility {
                    phase: FertilityPhase::Estrus,
                    cycle_offset: 12345,
                    post_partum_remaining_ticks: 0,
                },
            ))
            .id();

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(update_fertility_phase);
        schedule.run(&mut world);

        // Mutation sanity: if the query iterator doesn't actually
        // mutate through `Option<&mut Fertility>`, the Queen stays in
        // Estrus and this test fails — guarding the subtle
        // `for x in query` vs `for x in &mut query` distinction.
        let phase = world
            .get::<Fertility>(queen)
            .expect("Queen must retain Fertility in winter (removed only at Elder)")
            .phase;
        assert_eq!(
            phase,
            FertilityPhase::Anestrus,
            "update_fertility_phase must mutate Estrus → Anestrus at Winter onset"
        );
    }

    #[test]
    fn update_fertility_phase_does_not_insert_on_tom() {
        let mut world = World::new();
        world.insert_resource(SimConstants::default());
        world.insert_resource(SimConfig::default());
        world.insert_resource(TimeState::default());
        world.resource_mut::<TimeState>().tick = 12 * 20_000;

        let tom = world.spawn((Age::new(0), Gender::Tom)).id();

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(update_fertility_phase);
        schedule.run(&mut world);

        assert!(
            world.get::<Fertility>(tom).is_none(),
            "Tom should never carry Fertility (§7.M.7.4)"
        );
    }

    #[test]
    fn phase_winter_is_anestrus_regardless_of_cycle() {
        let c = test_constants();
        // Peak cycle tick in winter still returns Anestrus per rule 1.
        let mid_estrus =
            (c.cycle_length_ticks as f32 * (c.proestrus_fraction + c.estrus_fraction * 0.5)) as u32;
        assert_eq!(
            phase_from(mid_estrus, Season::Winter, 0, &c),
            FertilityPhase::Anestrus
        );
    }

    #[test]
    fn phase_postpartum_overrides_cycle_in_non_winter() {
        let c = test_constants();
        // With post_partum > 0 and any non-winter season, Postpartum wins.
        assert_eq!(
            phase_from(0, Season::Spring, 100, &c),
            FertilityPhase::Postpartum
        );
        assert_eq!(
            phase_from(0, Season::Summer, 100, &c),
            FertilityPhase::Postpartum
        );
    }

    #[test]
    fn phase_cycles_proestrus_estrus_diestrus_in_non_winter() {
        let c = test_constants();
        let proestrus_end = (c.cycle_length_ticks as f32 * c.proestrus_fraction) as u32;
        let estrus_end =
            proestrus_end + (c.cycle_length_ticks as f32 * c.estrus_fraction) as u32;
        assert_eq!(
            phase_from(0, Season::Spring, 0, &c),
            FertilityPhase::Proestrus
        );
        assert_eq!(
            phase_from(proestrus_end - 1, Season::Spring, 0, &c),
            FertilityPhase::Proestrus
        );
        assert_eq!(
            phase_from(proestrus_end, Season::Spring, 0, &c),
            FertilityPhase::Estrus
        );
        assert_eq!(
            phase_from(estrus_end - 1, Season::Spring, 0, &c),
            FertilityPhase::Estrus
        );
        assert_eq!(
            phase_from(estrus_end, Season::Spring, 0, &c),
            FertilityPhase::Diestrus
        );
        assert_eq!(
            phase_from(c.cycle_length_ticks - 1, Season::Spring, 0, &c),
            FertilityPhase::Diestrus
        );
    }

    #[test]
    fn cycle_tick_wraps_at_cycle_length() {
        let c = test_constants();
        // current_tick = cycle_length → cycle_tick = cycle_offset.
        let offset = 42u64;
        let ct = cycle_tick_for(c.cycle_length_ticks as u64, offset, c.cycle_length_ticks);
        assert_eq!(ct, offset as u32);
    }

    #[test]
    fn two_cats_with_different_offsets_desynchronize() {
        let c = test_constants();
        let tick = 1000u64;
        let cat_a = cycle_tick_for(tick, 0, c.cycle_length_ticks);
        let cat_b = cycle_tick_for(
            tick,
            (c.cycle_length_ticks as u64) / 2,
            c.cycle_length_ticks,
        );
        // Half-cycle offset → about 5000 ticks apart.
        assert!(cat_a.abs_diff(cat_b) > c.cycle_length_ticks / 3);
    }

    #[test]
    fn diestrus_fraction_sums_to_one() {
        let c = test_constants();
        let total = c.proestrus_fraction + c.estrus_fraction + c.diestrus_fraction();
        assert!((total - 1.0).abs() < 1e-5, "sum was {total}");
    }
}
