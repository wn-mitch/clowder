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

use crate::components::aspirations::Aspirations;
use crate::components::markers::{
    BodyDistressed, EsteemDistressed, LackingPurpose, LowHealth, LowMastery, SevereInjury,
};
use crate::components::mental::{Memory, MemoryType};
use crate::components::physical::{Dead, Health, Injury, InjuryKind, Needs, Position};
use crate::components::skills::Skills;
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

/// Mean of all six `Skills` field values, normalized into `[0, 1]`.
/// High skill → high felt-competence. Uses `Skills::total() / 6.0`,
/// clamped because `Skills` fields have no hard upper bound (in
/// practice diminishing-returns growth keeps them near `[0, 1]`).
///
/// Default cats (total ≈ 0.4) → ~0.067. A cat with all six fields
/// at 0.6 → 0.6. Saturated cats clamp to 1.0. Ticket 090.
pub fn mastery_confidence(skills: &Skills) -> f32 {
    (skills.total() / 6.0).clamp(0.0, 1.0)
}

/// `1.0` if the cat has at least one active aspiration, `0.0` if
/// none or if the `Aspirations` component is absent. Binary signal
/// — presence of directed striving, not degree of progress.
/// Gradient progress within an aspiration lives in
/// `ActiveAspiration::progress`, not in this scalar. Ticket 090.
pub fn purpose_clarity(aspirations: Option<&Aspirations>) -> f32 {
    if aspirations.is_some_and(|a| !a.active.is_empty()) {
        1.0
    } else {
        0.0
    }
}

/// Higher of the two L4 (esteem) need deficits: `max(1 - respect,
/// 1 - mastery)`. Parallels `body_distress_composite`'s
/// max-of-deficits semantics — any one L4 axis going critical is
/// enough to signal esteem distress, regardless of the other.
/// Range `[0, 1]`. Ticket 090.
pub fn esteem_distress(needs: &Needs) -> f32 {
    (1.0 - needs.respect)
        .max(1.0 - needs.mastery)
        .clamp(0.0, 1.0)
}

/// Body-state-appropriate safe-rest tile. Scans the cat's `Memory`
/// for `MemoryType::Sleep` entries; returns the strongest entry's
/// position whose location is not within `suppression_radius`
/// Manhattan tiles of any `MemoryType::ThreatSeen` or
/// `MemoryType::Death` memory. Returns `None` when no qualifying
/// memory exists.
///
/// Suppression: a Sleep memory at L is rejected if any
/// ThreatSeen/Death memory exists at L' with
/// `L.manhattan_distance(L') <= suppression_radius`. The "I
/// remember resting here, but I also remember a hawk here last
/// week" gate. Ticket 089.
pub fn own_safe_rest_spot(memory: &Memory, suppression_radius: i32) -> Option<Position> {
    let suppressors: Vec<Position> = memory
        .events
        .iter()
        .filter(|e| matches!(e.event_type, MemoryType::ThreatSeen | MemoryType::Death))
        .filter_map(|e| e.location)
        .collect();

    memory
        .events
        .iter()
        .filter(|e| e.event_type == MemoryType::Sleep)
        .filter_map(|e| e.location.map(|loc| (loc, e.strength)))
        .filter(|(loc, _)| {
            !suppressors
                .iter()
                .any(|s| loc.manhattan_distance(s) <= suppression_radius)
        })
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(loc, _)| loc)
}

/// Most-recent unhealed-injury site. Scans `health.injuries` for
/// `!healed` entries and returns the one with the highest
/// `tick_received`'s `at`. `None` when no unhealed injuries.
/// Future TendInjury DSE consumes this via
/// `LandmarkAnchor::OwnInjurySite`. Ticket 089.
pub fn own_injury_site(health: &Health) -> Option<Position> {
    health
        .injuries
        .iter()
        .filter(|i| !i.healed)
        .max_by_key(|i| i.tick_received)
        .map(|i| i.at)
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
            &Skills,
            Option<&Aspirations>,
            Has<LowHealth>,
            Has<SevereInjury>,
            Has<BodyDistressed>,
            Has<LowMastery>,
            Has<LackingPurpose>,
            Has<EsteemDistressed>,
        ),
        Without<Dead>,
    >,
) {
    let critical_health_threshold = constants.disposition.critical_health_threshold;
    let body_distress_threshold = constants.disposition.body_distress_threshold;
    let low_mastery_threshold = constants.disposition.low_mastery_threshold;
    let lacking_purpose_threshold = constants.disposition.lacking_purpose_threshold;
    let esteem_distressed_threshold = constants.disposition.esteem_distressed_threshold;

    for (
        entity,
        health,
        needs,
        skills,
        aspirations,
        has_low_health,
        has_severe_injury,
        has_body_distressed,
        has_low_mastery,
        has_lacking_purpose,
        has_esteem_distressed,
    ) in cats.iter()
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
        let want_low_mastery = mastery_confidence(skills) < low_mastery_threshold;
        let want_lacking_purpose = purpose_clarity(aspirations) < lacking_purpose_threshold;
        let want_esteem_distressed = esteem_distress(needs) > esteem_distressed_threshold;

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
        match (want_low_mastery, has_low_mastery) {
            (true, false) => {
                commands.entity(entity).insert(LowMastery);
            }
            (false, true) => {
                commands.entity(entity).remove::<LowMastery>();
            }
            _ => {}
        }
        match (want_lacking_purpose, has_lacking_purpose) {
            (true, false) => {
                commands.entity(entity).insert(LackingPurpose);
            }
            (false, true) => {
                commands.entity(entity).remove::<LackingPurpose>();
            }
            _ => {}
        }
        match (want_esteem_distressed, has_esteem_distressed) {
            (true, false) => {
                commands.entity(entity).insert(EsteemDistressed);
            }
            (false, true) => {
                commands.entity(entity).remove::<EsteemDistressed>();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::aspirations::{ActiveAspiration, AspirationDomain, Aspirations};
    use crate::components::physical::{Health, Injury, InjuryKind, InjurySource, Needs};
    use crate::components::skills::Skills;
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
            at: crate::components::physical::Position::new(0, 0),
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
                Skills::default(),
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
                Skills::default(),
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
        let cat = world.spawn((h, comfortable_needs(), Skills::default())).id();
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
        let cat = world.spawn((full_health(), needs, Skills::default())).id();
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
                Skills::default(),
                Dead {
                    tick: 0,
                    cause: crate::components::physical::DeathCause::Injury,
                },
            ))
            .id();
        schedule.run(&mut world);
        assert!(world.get::<LowHealth>(cat).is_none());
        assert!(world.get::<LowMastery>(cat).is_none());
        assert!(world.get::<LackingPurpose>(cat).is_none());
        assert!(world.get::<EsteemDistressed>(cat).is_none());
    }

    fn no_aspirations() -> Aspirations {
        Aspirations::default()
    }

    fn one_aspiration() -> Aspirations {
        Aspirations {
            active: vec![ActiveAspiration {
                chain_name: "TestChain".to_string(),
                domain: AspirationDomain::Hunting,
                current_milestone: 0,
                progress: 0,
                adopted_tick: 0,
                last_progress_tick: 0,
            }],
            completed: Vec::new(),
        }
    }

    fn skills_all(value: f32) -> Skills {
        Skills {
            hunting: value,
            foraging: value,
            herbcraft: value,
            building: value,
            combat: value,
            magic: value,
        }
    }

    // ---- Ticket 090 pure-function tests ----

    #[test]
    fn mastery_confidence_zero_skills() {
        assert_eq!(mastery_confidence(&skills_all(0.0)), 0.0);
    }

    #[test]
    fn mastery_confidence_full_skills() {
        assert!((mastery_confidence(&skills_all(1.0)) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn mastery_confidence_default_skills() {
        // Skills::default() total = 0.4; mean = 0.4 / 6 ≈ 0.0667.
        let mc = mastery_confidence(&Skills::default());
        assert!((mc - (0.4 / 6.0)).abs() < 1e-4, "got {mc}");
    }

    #[test]
    fn mastery_confidence_clamped_above_one() {
        assert_eq!(mastery_confidence(&skills_all(2.0)), 1.0);
    }

    #[test]
    fn mastery_confidence_partial_mean() {
        let mut s = skills_all(0.0);
        s.hunting = 0.6;
        // Total 0.6, mean = 0.1.
        assert!((mastery_confidence(&s) - 0.1).abs() < 1e-6);
    }

    #[test]
    fn purpose_clarity_none() {
        assert_eq!(purpose_clarity(None), 0.0);
    }

    #[test]
    fn purpose_clarity_empty_active() {
        let asp = no_aspirations();
        assert_eq!(purpose_clarity(Some(&asp)), 0.0);
    }

    #[test]
    fn purpose_clarity_nonempty_active() {
        let asp = one_aspiration();
        assert_eq!(purpose_clarity(Some(&asp)), 1.0);
    }

    #[test]
    fn esteem_distress_full_needs() {
        let mut needs = comfortable_needs();
        needs.respect = 1.0;
        needs.mastery = 1.0;
        assert_eq!(esteem_distress(&needs), 0.0);
    }

    #[test]
    fn esteem_distress_both_zero() {
        let mut needs = comfortable_needs();
        needs.respect = 0.0;
        needs.mastery = 0.0;
        assert_eq!(esteem_distress(&needs), 1.0);
    }

    #[test]
    fn esteem_distress_takes_max() {
        let mut needs = comfortable_needs();
        needs.respect = 0.3;
        needs.mastery = 0.8;
        // max(1 - 0.3, 1 - 0.8) = max(0.7, 0.2) = 0.7
        assert!((esteem_distress(&needs) - 0.7).abs() < 1e-6);
    }

    #[test]
    fn esteem_distress_takes_max_other_way() {
        let mut needs = comfortable_needs();
        needs.respect = 0.9;
        needs.mastery = 0.2;
        // max(1 - 0.9, 1 - 0.2) = max(0.1, 0.8) = 0.8
        assert!((esteem_distress(&needs) - 0.8).abs() < 1e-6);
    }

    #[test]
    fn esteem_distress_clamps() {
        let mut needs = comfortable_needs();
        needs.respect = -0.1;
        needs.mastery = 1.1;
        // 1 - (-0.1) = 1.1, 1 - 1.1 = -0.1; max = 1.1; clamped to 1.0.
        assert_eq!(esteem_distress(&needs), 1.0);
    }

    // ---- Ticket 090 marker-lifecycle tests ----

    #[test]
    fn low_mastery_fires_for_default_skills() {
        let (mut world, mut schedule) = setup_world();
        let cat = world
            .spawn((full_health(), comfortable_needs(), Skills::default()))
            .id();
        schedule.run(&mut world);
        assert!(world.get::<LowMastery>(cat).is_some());
        // Idempotent transition: re-running steady state must not fail.
        schedule.run(&mut world);
        assert!(world.get::<LowMastery>(cat).is_some());
    }

    #[test]
    fn low_mastery_clears_when_skilled() {
        let (mut world, mut schedule) = setup_world();
        let cat = world
            .spawn((full_health(), comfortable_needs(), Skills::default()))
            .id();
        schedule.run(&mut world);
        assert!(world.get::<LowMastery>(cat).is_some());
        // Practise hard.
        world.entity_mut(cat).insert(skills_all(0.8));
        schedule.run(&mut world);
        assert!(world.get::<LowMastery>(cat).is_none());
    }

    #[test]
    fn low_mastery_boundary_at_threshold() {
        // mean exactly equals threshold (0.35) — strict `<` means marker
        // does NOT fire.
        let (mut world, mut schedule) = setup_world();
        let cat = world
            .spawn((full_health(), comfortable_needs(), skills_all(0.35)))
            .id();
        schedule.run(&mut world);
        assert!(
            world.get::<LowMastery>(cat).is_none(),
            "LowMastery must not fire at exactly the threshold (strict <)"
        );
    }

    #[test]
    fn lacking_purpose_fires_without_aspirations_component() {
        let (mut world, mut schedule) = setup_world();
        let cat = world
            .spawn((full_health(), comfortable_needs(), Skills::default()))
            .id();
        schedule.run(&mut world);
        assert!(world.get::<LackingPurpose>(cat).is_some());
    }

    #[test]
    fn lacking_purpose_fires_with_empty_active() {
        let (mut world, mut schedule) = setup_world();
        let cat = world
            .spawn((
                full_health(),
                comfortable_needs(),
                Skills::default(),
                no_aspirations(),
            ))
            .id();
        schedule.run(&mut world);
        assert!(world.get::<LackingPurpose>(cat).is_some());
    }

    #[test]
    fn lacking_purpose_clears_when_aspiration_adopted() {
        let (mut world, mut schedule) = setup_world();
        let cat = world
            .spawn((full_health(), comfortable_needs(), Skills::default()))
            .id();
        schedule.run(&mut world);
        assert!(world.get::<LackingPurpose>(cat).is_some());
        world.entity_mut(cat).insert(one_aspiration());
        schedule.run(&mut world);
        assert!(world.get::<LackingPurpose>(cat).is_none());
    }

    #[test]
    fn esteem_distressed_fires_when_respect_low() {
        let (mut world, mut schedule) = setup_world();
        let mut needs = comfortable_needs();
        needs.respect = 0.3; // distress = 0.7 > 0.55
        needs.mastery = 0.9;
        let cat = world
            .spawn((full_health(), needs, Skills::default()))
            .id();
        schedule.run(&mut world);
        assert!(world.get::<EsteemDistressed>(cat).is_some());
    }

    #[test]
    fn esteem_distressed_fires_when_mastery_low() {
        let (mut world, mut schedule) = setup_world();
        let mut needs = comfortable_needs();
        needs.respect = 0.9;
        needs.mastery = 0.2; // distress = 0.8 > 0.55
        let cat = world
            .spawn((full_health(), needs, Skills::default()))
            .id();
        schedule.run(&mut world);
        assert!(world.get::<EsteemDistressed>(cat).is_some());
    }

    #[test]
    fn esteem_distressed_absent_when_needs_satisfied() {
        let (mut world, mut schedule) = setup_world();
        let mut needs = comfortable_needs();
        needs.respect = 0.6; // distress = 0.4
        needs.mastery = 0.6;
        let cat = world
            .spawn((full_health(), needs, Skills::default()))
            .id();
        schedule.run(&mut world);
        assert!(world.get::<EsteemDistressed>(cat).is_none());
    }

    #[test]
    fn esteem_distressed_clears_on_recovery() {
        let (mut world, mut schedule) = setup_world();
        let mut needs = comfortable_needs();
        needs.respect = 0.3;
        needs.mastery = 0.9;
        let cat = world
            .spawn((full_health(), needs.clone(), Skills::default()))
            .id();
        schedule.run(&mut world);
        assert!(world.get::<EsteemDistressed>(cat).is_some());
        // Recover.
        needs.respect = 0.7;
        world.entity_mut(cat).insert(needs);
        schedule.run(&mut world);
        assert!(world.get::<EsteemDistressed>(cat).is_none());
    }

    // ---- Ticket 089 — `own_safe_rest_spot` / `own_injury_site` ----

    use crate::components::mental::{Memory, MemoryEntry, MemoryType};

    fn sleep_memory(loc: Position, strength: f32, tick: u64) -> MemoryEntry {
        MemoryEntry {
            event_type: MemoryType::Sleep,
            location: Some(loc),
            involved: Vec::new(),
            tick,
            strength,
            firsthand: true,
        }
    }

    fn threat_memory(loc: Position, tick: u64) -> MemoryEntry {
        MemoryEntry {
            event_type: MemoryType::ThreatSeen,
            location: Some(loc),
            involved: Vec::new(),
            tick,
            strength: 0.7,
            firsthand: true,
        }
    }

    #[test]
    fn own_safe_rest_spot_none_with_empty_memory() {
        let memory = Memory::default();
        assert_eq!(own_safe_rest_spot(&memory, 5), None);
    }

    #[test]
    fn own_safe_rest_spot_picks_strongest_sleep_memory() {
        let mut memory = Memory::default();
        memory.remember(sleep_memory(Position::new(1, 1), 0.3, 10));
        memory.remember(sleep_memory(Position::new(2, 2), 0.5, 20));
        memory.remember(sleep_memory(Position::new(3, 3), 0.4, 30));
        assert_eq!(own_safe_rest_spot(&memory, 5), Some(Position::new(2, 2)));
    }

    #[test]
    fn own_safe_rest_spot_suppressed_by_nearby_threat() {
        let mut memory = Memory::default();
        memory.remember(sleep_memory(Position::new(10, 10), 0.6, 5));
        memory.remember(threat_memory(Position::new(12, 10), 6));
        // ThreatSeen at Manhattan distance 2 — within radius 5 → Sleep
        // memory suppressed; no other Sleep memories → None.
        assert_eq!(own_safe_rest_spot(&memory, 5), None);
    }

    #[test]
    fn own_safe_rest_spot_unsuppressed_by_distant_threat() {
        let mut memory = Memory::default();
        memory.remember(sleep_memory(Position::new(10, 10), 0.6, 5));
        memory.remember(threat_memory(Position::new(50, 50), 6));
        assert_eq!(own_safe_rest_spot(&memory, 5), Some(Position::new(10, 10)));
    }

    #[test]
    fn own_safe_rest_spot_stable_across_ticks() {
        let mut memory = Memory::default();
        memory.remember(sleep_memory(Position::new(1, 1), 0.5, 10));
        memory.remember(sleep_memory(Position::new(2, 2), 0.5, 20));
        let first = own_safe_rest_spot(&memory, 5);
        let second = own_safe_rest_spot(&memory, 5);
        assert_eq!(first, second, "resolver must be deterministic");
    }

    #[test]
    fn own_injury_site_none_with_no_injuries() {
        let h = full_health();
        assert_eq!(own_injury_site(&h), None);
    }

    #[test]
    fn own_injury_site_picks_most_recent_unhealed() {
        let mut h = full_health();
        h.injuries.push(Injury {
            kind: InjuryKind::Minor,
            tick_received: 100,
            healed: false,
            source: InjurySource::Unknown,
            at: Position::new(1, 1),
        });
        h.injuries.push(Injury {
            kind: InjuryKind::Moderate,
            tick_received: 200,
            healed: false,
            source: InjurySource::Unknown,
            at: Position::new(2, 2),
        });
        h.injuries.push(Injury {
            kind: InjuryKind::Severe,
            tick_received: 150,
            healed: false,
            source: InjurySource::Unknown,
            at: Position::new(3, 3),
        });
        assert_eq!(own_injury_site(&h), Some(Position::new(2, 2)));
    }

    #[test]
    fn own_injury_site_ignores_healed() {
        let mut h = full_health();
        h.injuries.push(Injury {
            kind: InjuryKind::Minor,
            tick_received: 100,
            healed: false,
            source: InjurySource::Unknown,
            at: Position::new(1, 1),
        });
        h.injuries.push(Injury {
            kind: InjuryKind::Moderate,
            tick_received: 200,
            healed: true,
            source: InjurySource::Unknown,
            at: Position::new(2, 2),
        });
        // Healed injury is most recent but ignored; falls back to
        // the older unhealed one.
        assert_eq!(own_injury_site(&h), Some(Position::new(1, 1)));
    }
}
