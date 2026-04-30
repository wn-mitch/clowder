//! Interoceptive perception — the cat's structured view of its own body.
//!
//! Symmetric counterpart to `src/systems/sensing.rs`. External perception
//! authors `HasThreatNearby` / `HasSocialTarget` / `PreyNearby` / `…` from
//! the world; interoceptive perception authors `LowHealth` / `SevereInjury`
//! / `BodyDistressed` from the cat's own `Health` and `Needs` components.
//!
//! Same plumbing: per-tick ZST-marker authoring registered in Chain 2a
//! before the GOAP/scoring pipeline, consumed by DSEs via the
//! `EligibilityFilter::require/forbid(KEY)` and `ctx_scalars()` paths.
//!
//! Ticket 087. Closes the architectural asymmetry where DSEs (and the
//! `ScoringContext` populator at `disposition.rs:730–895`) reach into raw
//! `Needs` / `Health` fields directly rather than going through a
//! perception surface the way external signals do.
//!
//! ## Markers authored
//!
//! - [`LowHealth`](crate::components::markers::LowHealth) — HP ratio at or
//!   below `DispositionConstants::critical_health_threshold`. Fires *at*
//!   the same threshold the disposition-layer critical-health interrupt
//!   triggers on, so DSE scoring can elect Flee or Rest before the
//!   interrupt's panic-fallback fires (ticket 047 treadmill root cause).
//! - [`SevereInjury`](crate::components::markers::SevereInjury) — at least
//!   one unhealed `InjuryKind::Severe`. Cheaper signal than computing
//!   `pain_level` for DSE eligibility gates.
//! - [`BodyDistressed`](crate::components::markers::BodyDistressed) —
//!   composite gate; fires when *any* of {hunger_urgency, energy_deficit,
//!   thermal_deficit, health_deficit} exceeds
//!   `DispositionConstants::body_distress_threshold`. The unified
//!   "I am unwell" signal — analog of how external perception's
//!   `HasThreatNearby` is a unified "I am in danger" signal across many
//!   possible threats.
//!
//! ## Scalars published
//!
//! `pain_level` and `body_distress_composite` are exposed via
//! `crate::ai::scoring::ctx_scalars()`; this module owns their derivation.
//! `health_deficit` continues to be exposed there but is now sourced from
//! the same `Health` read this perception layer uses, rather than the raw
//! component-read in `disposition.rs`'s `ScoringContext` populator.

use bevy::prelude::*;

use crate::components::markers::{BodyDistressed, LowHealth, SevereInjury};
use crate::components::physical::{Dead, Health, Injury, InjuryKind, Needs};
use crate::resources::sim_constants::SimConstants;

/// Per-`InjuryKind` severity score used to derive `pain_level`. Minor
/// wounds register a small ache; Moderate is meaningfully painful; Severe
/// dominates. The numbers are calibrated against `pain_normalization_max`
/// (default 2.0) so three Severe wounds saturate the scalar at 1.0.
pub const INJURY_KIND_MINOR_SEVERITY: f32 = 0.1;
pub const INJURY_KIND_MODERATE_SEVERITY: f32 = 0.3;
pub const INJURY_KIND_SEVERE_SEVERITY: f32 = 0.7;

/// Map an `InjuryKind` to its scalar severity. Unhealed injuries
/// contribute their severity to `pain_level`; healed injuries contribute
/// nothing.
pub fn severity_score(kind: InjuryKind) -> f32 {
    match kind {
        InjuryKind::Minor => INJURY_KIND_MINOR_SEVERITY,
        InjuryKind::Moderate => INJURY_KIND_MODERATE_SEVERITY,
        InjuryKind::Severe => INJURY_KIND_SEVERE_SEVERITY,
    }
}

/// Sum unhealed-injury severity scores, normalized into `[0, 1]` by
/// `pain_normalization_max`. Healed injuries do not contribute.
pub fn pain_level(injuries: &[Injury], normalization_max: f32) -> f32 {
    if normalization_max <= 0.0 {
        return 0.0;
    }
    let total: f32 = injuries
        .iter()
        .filter(|i| !i.healed)
        .map(|i| severity_score(i.kind))
        .sum();
    (total / normalization_max).clamp(0.0, 1.0)
}

/// HP-ratio-derived health deficit in `[0, 1]`. `health.max == 0` is
/// treated as full deficit (degenerate cat with no max HP — should not
/// occur in production but the perception module must not panic).
pub fn health_deficit(health: &Health) -> f32 {
    if health.max <= 0.0 {
        return 1.0;
    }
    (1.0 - health.current / health.max).clamp(0.0, 1.0)
}

/// Composite body-distress signal. `max(hunger_urgency, energy_deficit,
/// thermal_deficit, health_deficit)` — the single highest body-state
/// urgency. Used both for the `BodyDistressed` marker gate and as the
/// `body_distress_composite` scalar consumed by the (future) §L2.10
/// distress-promotion Modifier (ticket 088).
///
/// Maximum (rather than weighted sum) preserves the gate semantic: any
/// one body axis going critical is enough to mark the cat as distressed,
/// regardless of how comfortable the others are. A starving cat that is
/// otherwise warm and rested is still in distress.
pub fn body_distress_composite(needs: &Needs, health: &Health) -> f32 {
    let hunger_urgency = (1.0 - needs.hunger).clamp(0.0, 1.0);
    let energy_deficit = (1.0 - needs.energy).clamp(0.0, 1.0);
    let thermal_deficit = (1.0 - needs.temperature).clamp(0.0, 1.0);
    let h = health_deficit(health);
    hunger_urgency
        .max(energy_deficit)
        .max(thermal_deficit)
        .max(h)
}

/// Per-tick author system for interoceptive ZST markers.
///
/// Reads each living cat's `Health` and `Needs`; computes the three
/// gating predicates; inserts/removes markers transitionally (no
/// idempotent re-insertion). Mirrors the shape of
/// `crate::systems::needs::update_injury_marker` and runs alongside it
/// in Chain 2a.
///
/// **Ordering** — Chain 2a, after `update_incapacitation` and
/// `update_injury_marker`. Independent of those (different markers); the
/// adjacency is documentation, not a data dependency.
///
/// **Lifecycle** — transition-only insert/remove; idempotent in steady
/// state. Dead cats filtered via `Without<Dead>`.
#[allow(clippy::type_complexity)]
pub fn author_self_markers(
    mut commands: Commands,
    constants: Res<SimConstants>,
    cats: Query<
        (
            Entity,
            &Health,
            &Needs,
            Has<LowHealth>,
            Has<SevereInjury>,
            Has<BodyDistressed>,
        ),
        Without<Dead>,
    >,
) {
    let critical_health_threshold = constants.disposition.critical_health_threshold;
    let body_distress_threshold = constants.disposition.body_distress_threshold;

    for (entity, health, needs, has_low_health, has_severe_injury, has_body_distressed) in
        cats.iter()
    {
        let health_ratio = if health.max > 0.0 {
            health.current / health.max
        } else {
            0.0
        };
        let want_low_health = health_ratio <= critical_health_threshold;
        let want_severe_injury = health
            .injuries
            .iter()
            .any(|i| !i.healed && i.kind == InjuryKind::Severe);
        let want_body_distressed =
            body_distress_composite(needs, health) >= body_distress_threshold;

        match (want_low_health, has_low_health) {
            (true, false) => {
                commands.entity(entity).insert(LowHealth);
            }
            (false, true) => {
                commands.entity(entity).remove::<LowHealth>();
            }
            _ => {}
        }
        match (want_severe_injury, has_severe_injury) {
            (true, false) => {
                commands.entity(entity).insert(SevereInjury);
            }
            (false, true) => {
                commands.entity(entity).remove::<SevereInjury>();
            }
            _ => {}
        }
        match (want_body_distressed, has_body_distressed) {
            (true, false) => {
                commands.entity(entity).insert(BodyDistressed);
            }
            (false, true) => {
                commands.entity(entity).remove::<BodyDistressed>();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::physical::{Health, Injury, InjuryKind, InjurySource, Needs};
    use bevy::ecs::schedule::Schedule;

    fn full_health() -> Health {
        Health {
            current: 1.0,
            max: 1.0,
            injuries: Vec::new(),
        }
    }

    fn injury(kind: InjuryKind, healed: bool) -> Injury {
        Injury {
            kind,
            tick_received: 0,
            healed,
            source: InjurySource::Unknown,
        }
    }

    fn comfortable_needs() -> Needs {
        Needs {
            hunger: 0.9,
            energy: 0.9,
            temperature: 0.9,
            safety: 0.9,
            social: 0.9,
            acceptance: 0.9,
            mating: 0.9,
            respect: 0.9,
            mastery: 0.9,
            purpose: 0.9,
        }
    }

    #[test]
    fn severity_score_orders_kinds() {
        assert!(severity_score(InjuryKind::Minor) < severity_score(InjuryKind::Moderate));
        assert!(severity_score(InjuryKind::Moderate) < severity_score(InjuryKind::Severe));
    }

    #[test]
    fn pain_level_zero_with_no_injuries() {
        let health = full_health();
        assert_eq!(pain_level(&health.injuries, 2.0), 0.0);
    }

    #[test]
    fn pain_level_ignores_healed_injuries() {
        let mut health = full_health();
        health.injuries.push(injury(InjuryKind::Severe, true));
        health.injuries.push(injury(InjuryKind::Severe, true));
        assert_eq!(pain_level(&health.injuries, 2.0), 0.0);
    }

    #[test]
    fn pain_level_normalizes_to_unit_range() {
        let mut health = full_health();
        // Three severe wounds at 0.7 each = 2.1 raw, > 2.0 normalization
        // max → saturates at 1.0.
        for _ in 0..3 {
            health.injuries.push(injury(InjuryKind::Severe, false));
        }
        assert_eq!(pain_level(&health.injuries, 2.0), 1.0);
    }

    #[test]
    fn pain_level_scales_intermediate_load() {
        let mut health = full_health();
        // One Severe (0.7) + one Moderate (0.3) = 1.0 raw, /2.0 = 0.5.
        health.injuries.push(injury(InjuryKind::Severe, false));
        health.injuries.push(injury(InjuryKind::Moderate, false));
        assert!((pain_level(&health.injuries, 2.0) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn pain_level_zero_normalizer_is_safe() {
        let mut health = full_health();
        health.injuries.push(injury(InjuryKind::Severe, false));
        assert_eq!(pain_level(&health.injuries, 0.0), 0.0);
        assert_eq!(pain_level(&health.injuries, -1.0), 0.0);
    }

    #[test]
    fn health_deficit_full_health_zero() {
        assert_eq!(health_deficit(&full_health()), 0.0);
    }

    #[test]
    fn health_deficit_half_health() {
        let h = Health {
            current: 0.5,
            max: 1.0,
            injuries: Vec::new(),
        };
        assert!((health_deficit(&h) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn health_deficit_zero_max_is_safe() {
        let h = Health {
            current: 0.0,
            max: 0.0,
            injuries: Vec::new(),
        };
        assert_eq!(health_deficit(&h), 1.0);
    }

    #[test]
    fn body_distress_composite_takes_max() {
        // Comfortable needs across the board → composite is 0.1
        // (1 - 0.9), driven by all four axes equally.
        let needs = comfortable_needs();
        let health = full_health();
        assert!((body_distress_composite(&needs, &health) - 0.1).abs() < 1e-4);
    }

    #[test]
    fn body_distress_composite_picks_worst_axis() {
        // Hunger-starved cat with full energy/thermal/health: composite
        // tracks the hunger axis only.
        let mut needs = comfortable_needs();
        needs.hunger = 0.05;
        let health = full_health();
        let composite = body_distress_composite(&needs, &health);
        assert!((composite - 0.95).abs() < 1e-4);
    }

    #[test]
    fn body_distress_composite_picks_health_when_worst() {
        let needs = comfortable_needs();
        let health = Health {
            current: 0.2,
            max: 1.0,
            injuries: Vec::new(),
        };
        let composite = body_distress_composite(&needs, &health);
        assert!((composite - 0.8).abs() < 1e-4);
    }

    fn setup_world() -> (World, Schedule) {
        let mut world = World::new();
        world.insert_resource(SimConstants::default());
        let mut schedule = Schedule::default();
        schedule.add_systems(author_self_markers);
        (world, schedule)
    }

    #[test]
    fn low_health_marker_inserted_below_threshold() {
        let (mut world, mut schedule) = setup_world();
        let cat = world
            .spawn((
                Health {
                    current: 0.3,
                    max: 1.0,
                    injuries: Vec::new(),
                },
                comfortable_needs(),
            ))
            .id();
        schedule.run(&mut world);
        assert!(world.get::<LowHealth>(cat).is_some());
    }

    #[test]
    fn low_health_marker_clears_when_healed() {
        let (mut world, mut schedule) = setup_world();
        let cat = world
            .spawn((
                Health {
                    current: 0.3,
                    max: 1.0,
                    injuries: Vec::new(),
                },
                comfortable_needs(),
            ))
            .id();
        schedule.run(&mut world);
        assert!(world.get::<LowHealth>(cat).is_some());
        // Heal up.
        world.entity_mut(cat).insert(Health {
            current: 1.0,
            max: 1.0,
            injuries: Vec::new(),
        });
        schedule.run(&mut world);
        assert!(world.get::<LowHealth>(cat).is_none());
    }

    #[test]
    fn severe_injury_marker_only_for_unhealed_severe() {
        let (mut world, mut schedule) = setup_world();
        let mut h = Health {
            current: 0.6,
            max: 1.0,
            injuries: Vec::new(),
        };
        h.injuries.push(injury(InjuryKind::Moderate, false));
        h.injuries.push(injury(InjuryKind::Severe, true));
        let cat = world.spawn((h, comfortable_needs())).id();
        schedule.run(&mut world);
        // Moderate unhealed + Severe healed → no SevereInjury marker.
        assert!(world.get::<SevereInjury>(cat).is_none());

        // Add an unhealed Severe.
        let mut updated = world.get::<Health>(cat).unwrap().clone();
        updated.injuries.push(injury(InjuryKind::Severe, false));
        world.entity_mut(cat).insert(updated);
        schedule.run(&mut world);
        assert!(world.get::<SevereInjury>(cat).is_some());
    }

    #[test]
    fn body_distressed_marker_responds_to_any_axis() {
        let (mut world, mut schedule) = setup_world();
        let mut needs = comfortable_needs();
        needs.energy = 0.2; // deficit 0.8 > default 0.6 threshold
        let cat = world.spawn((full_health(), needs)).id();
        schedule.run(&mut world);
        assert!(world.get::<BodyDistressed>(cat).is_some());
    }

    #[test]
    fn dead_cats_skipped() {
        let (mut world, mut schedule) = setup_world();
        let cat = world
            .spawn((
                Health {
                    current: 0.0,
                    max: 1.0,
                    injuries: Vec::new(),
                },
                comfortable_needs(),
                Dead {
                    tick: 0,
                    cause: crate::components::physical::DeathCause::Injury,
                },
            ))
            .id();
        schedule.run(&mut world);
        assert!(world.get::<LowHealth>(cat).is_none());
    }
}
