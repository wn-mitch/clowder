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
//!
//! `escape_viability` (ticket 103) — pure threat-coupled physics signal:
//! "given an active threat, can this cat escape?" Composed from terrain
//! openness around the cat plus a flat penalty when the cat has a
//! dependent (kitten or pair-bonded mate). Returns `1.0` when no threat
//! is present — the question is undefined-but-safe; downstream consumers
//! (Fight branch ticket 102, Freeze branch ticket 105) gate on threat
//! presence before reading the scalar. The single-axis discipline holds
//! — *ambient* anxiety about closed spaces (claustrophobia) is a
//! separate axis owned by ticket 126's phobia modifier family, not
//! folded into this scalar.

use bevy::prelude::*;

use crate::components::aspirations::Aspirations;
use crate::components::markers::{
    BodyDistressed, EsteemDistressed, LackingPurpose, LowHealth, LowMastery, SevereInjury,
};
use crate::components::mental::{Memory, MemoryType};
use crate::components::physical::{Dead, Health, Injury, InjuryKind, Needs, Position};
use crate::components::skills::Skills;
use crate::resources::map::TileMap;
use crate::resources::sim_constants::{EscapeViabilityConstants, SimConstants};

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

/// Count walkable tiles inside the `(2 * radius + 1) × (2 * radius + 1)`
/// bounding box centered on `center`. Out-of-bounds tiles do not count
/// toward the walkable total but *do* count toward the box area — this
/// makes a cat near a map edge register as more cornered than one in
/// the same shape of walls in the interior, which matches the
/// "fewer escape options" intent.
///
/// `radius == 0` returns `1` if `center` is in-bounds and passable,
/// else `0`. Negative radius is treated as `0`.
fn count_walkable_tiles_in_box(center: Position, radius: i32, map: &TileMap) -> u32 {
    let r = radius.max(0);
    let mut count: u32 = 0;
    for dy in -r..=r {
        for dx in -r..=r {
            let x = center.x + dx;
            let y = center.y + dy;
            if map.in_bounds(x, y) && map.get(x, y).terrain.is_passable() {
                count += 1;
            }
        }
    }
    count
}

/// Box-area helper for `(2 * radius + 1)²`. Negative radius treated
/// as `0` (single tile). Used as the openness denominator.
fn sprint_box_area(radius: i32) -> u32 {
    let r = radius.max(0) as u32;
    let side = 2 * r + 1;
    side * side
}

/// Threat-coupled escape viability in `[0, 1]`. **Single-axis** — pure
/// physics about whether the cat can escape an *active* threat;
/// ambient/personality anxiety lives on separate scalars (ticket 126's
/// phobia family). Ticket 103.
///
/// Returns `1.0` when `nearest_threat` is `None`. Rationale: with no
/// active threat, escape is trivially viable, and the no-threat
/// short-circuit ensures the scalar never reads the map (locking in
/// the contract that downstream Fight/Freeze gates which forget to
/// check threat presence don't accidentally trigger on a low-openness
/// peacetime cat).
///
/// When a threat is present, composes two terms:
///
/// 1. **Terrain openness** — fraction of walkable tiles in the
///    `(2 * sprint_radius + 1)²` bounding box centered on the cat.
///    Closed terrain (walls, water, cliff) drops viability.
/// 2. **Dependent penalty** — flat subtractive when
///    `has_nearby_dependent` is true. Models cost-of-abandonment: a
///    cat next to a kitten or bonded mate registers escape as less
///    viable.
///
/// Composition: `terrain_weight * openness - dependent_weight *
/// dependent_penalty (if has_nearby_dependent)`, clamped to `[0, 1]`.
/// Weights configurable via `EscapeViabilityConstants`.
pub fn escape_viability(
    self_pos: Position,
    nearest_threat: Option<Position>,
    map: &TileMap,
    has_nearby_dependent: bool,
    constants: &EscapeViabilityConstants,
) -> f32 {
    if nearest_threat.is_none() {
        return 1.0;
    }

    let walkable = count_walkable_tiles_in_box(self_pos, constants.sprint_radius, map) as f32;
    let area = sprint_box_area(constants.sprint_radius) as f32;
    let openness = if area > 0.0 { walkable / area } else { 0.0 };

    let dependent_term = if has_nearby_dependent {
        constants.dependent_weight * constants.dependent_penalty
    } else {
        0.0
    };

    (constants.terrain_weight * openness - dependent_term).clamp(0.0, 1.0)
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
            total_starvation_damage: 0.0,
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
            total_starvation_damage: 0.0,
        };
        assert!((health_deficit(&h) - 0.5).abs() < 1e-4);
    }

    #[test]
    fn health_deficit_zero_max_is_safe() {
        let h = Health {
            current: 0.0,
            max: 0.0,
            injuries: Vec::new(),
            total_starvation_damage: 0.0,
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
            total_starvation_damage: 0.0,
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
                    total_starvation_damage: 0.0,
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
                    total_starvation_damage: 0.0,
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
            total_starvation_damage: 0.0,
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
            total_starvation_damage: 0.0,
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
                    total_starvation_damage: 0.0,
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

    // ---- Ticket 103 — `escape_viability` ----

    use crate::resources::map::{Terrain, TileMap};

    fn open_grass_map(width: i32, height: i32) -> TileMap {
        TileMap::new(width, height, Terrain::Grass)
    }

    #[test]
    fn escape_viability_one_with_no_threat() {
        let map = open_grass_map(20, 20);
        let constants = EscapeViabilityConstants::default();
        let v = escape_viability(Position::new(10, 10), None, &map, false, &constants);
        assert_eq!(v, 1.0);
    }

    #[test]
    fn escape_viability_no_threat_short_circuits_terrain() {
        // Same `None` threat in two very different terrains must
        // produce the same 1.0 — locks in the contract that the
        // no-threat branch never reads the map.
        let mut walled = open_grass_map(20, 20);
        for x in 0..20 {
            for y in 0..20 {
                walled.set(x, y, Terrain::Wall);
            }
        }
        let open = open_grass_map(20, 20);
        let constants = EscapeViabilityConstants::default();
        let pos = Position::new(10, 10);
        assert_eq!(escape_viability(pos, None, &walled, false, &constants), 1.0);
        assert_eq!(escape_viability(pos, None, &open, false, &constants), 1.0);
    }

    #[test]
    fn escape_viability_high_in_open_terrain() {
        // Threat present, all-grass map, no dependents → terrain
        // weight × full openness = 0.7 (with default weights).
        let map = open_grass_map(20, 20);
        let constants = EscapeViabilityConstants::default();
        let v = escape_viability(
            Position::new(10, 10),
            Some(Position::new(3, 3)),
            &map,
            false,
            &constants,
        );
        // Default terrain_weight = 0.7, full openness, no dependent.
        assert!((v - 0.7).abs() < 1e-4, "got {v}");
    }

    #[test]
    fn escape_viability_low_in_corner() {
        // Cat at (1, 1) surrounded by walls within the sprint box —
        // openness should drop below half. Build a 9×9 map of walls
        // with only a 3×3 patch of grass around (1, 1).
        let mut map = TileMap::new(9, 9, Terrain::Wall);
        for x in 0..=2 {
            for y in 0..=2 {
                map.set(x, y, Terrain::Grass);
            }
        }
        let constants = EscapeViabilityConstants::default();
        let v = escape_viability(
            Position::new(1, 1),
            Some(Position::new(8, 8)),
            &map,
            false,
            &constants,
        );
        // Sprint radius 3 → 7×7 = 49-tile box. Walkable subset is
        // the 3×3 grass patch = 9 tiles. Openness = 9/49 ≈ 0.184.
        // viability = 0.7 × 0.184 ≈ 0.129. Well below 0.3.
        assert!(v < 0.3, "expected low viability in corner, got {v}");
    }

    #[test]
    fn escape_viability_reduced_with_dependent() {
        let map = open_grass_map(20, 20);
        let constants = EscapeViabilityConstants::default();
        let pos = Position::new(10, 10);
        let threat = Some(Position::new(3, 3));

        let without = escape_viability(pos, threat, &map, false, &constants);
        let with = escape_viability(pos, threat, &map, true, &constants);

        // Penalty = 0.3 × 1.0 = 0.3 → 0.7 - 0.3 = 0.4.
        assert!((without - 0.7).abs() < 1e-4);
        assert!((with - 0.4).abs() < 1e-4);
        assert!(with < without);
    }

    #[test]
    fn escape_viability_clamps_to_unit_range() {
        let map = open_grass_map(20, 20);
        // Pathological weights — dependent_penalty inflated past
        // terrain_weight. Should still clamp at 0.0, not go negative.
        let constants = EscapeViabilityConstants {
            sprint_radius: 3,
            terrain_weight: 0.5,
            dependent_weight: 1.0,
            dependent_penalty: 1.0,
        };
        let v = escape_viability(
            Position::new(10, 10),
            Some(Position::new(3, 3)),
            &map,
            true,
            &constants,
        );
        assert_eq!(v, 0.0);
    }

    #[test]
    fn count_walkable_tiles_in_box_handles_radius_zero() {
        let map = open_grass_map(5, 5);
        // Single passable tile at center.
        assert_eq!(count_walkable_tiles_in_box(Position::new(2, 2), 0, &map), 1);
        // Negative radius treated as 0 — still single tile.
        assert_eq!(
            count_walkable_tiles_in_box(Position::new(2, 2), -3, &map),
            1
        );
    }

    #[test]
    fn count_walkable_tiles_in_box_skips_oob() {
        // Cat near map edge — out-of-bounds tiles do not count.
        let map = open_grass_map(5, 5);
        // Center at (0, 0), radius 1 → 3×3 box from (-1,-1) to (1,1).
        // Only (0,0), (1,0), (0,1), (1,1) are in-bounds → 4 walkable.
        assert_eq!(count_walkable_tiles_in_box(Position::new(0, 0), 1, &map), 4);
    }
}
