//! Per-cat subjective belief substrate (ticket 258 — C3 worked design).
//!
//! Each cat carries four [`Component`]s, one per **mental-model family**:
//! [`CatBeliefs`], [`LocationBeliefs`], [`PredatorBeliefs`], [`ContextBeliefs`].
//! Each maps a perceiver-specific key (Entity, bucketed position, species
//! identity, or context kind) to a [`MentalModel`] carrying six **belief
//! facets** updated by `belief_integrator` via the [`EvidenceKind`] typology.
//!
//! The four families are distinct Component types (rather than one Component
//! with four maps) because their keying disciplines differ — entity-keyed
//! maps need `#[serde(skip)]` and a liveness sweep on entity despawn; the
//! location and context maps are fully serializable. Splitting them at the
//! type level makes the differences explicit.
//!
//! Substrate-only as of 258. Consumer tickets (263–270) read facets;
//! ColonyKnowledge restructure (follow-on) replaces carrier-count promotion
//! with mental-model agreement.

use bevy_ecs::prelude::*;
use std::collections::HashMap;

use crate::components::disposition::DispositionKind;
use crate::components::magic::ResourceKind;

// ---------------------------------------------------------------------------
// Keying types
// ---------------------------------------------------------------------------

/// Bucketed location key for [`LocationBeliefs`]. The integrator buckets raw
/// `Position` into 5-tile cells before reading or writing — matches the
/// `ColonyKnowledge` bucketing precedent in
/// [`crate::resources::colony_knowledge`].
pub type LocationKey = (i32, i32);

/// Identifier for an environmental-context mental model. v1 covers the
/// "here-now" ambient context and per-disposition-execution self-beliefs;
/// other variants can be added without enum-revision pressure on consumers
/// (every reader matches exhaustively, which is the intent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EnvironmentalContextKey {
    /// Ambient context around the cat right now. Source for
    /// `recency_of_threat_cue` updates from `AmbientShock` and
    /// `ConspecificStartle` events.
    HereNow,
    /// Self-belief about executing a given disposition. Hosts the
    /// `predictability` facet that replaces `RecentDispositionFailures`
    /// (ticket 258 retirement step).
    DispositionExecution(DispositionKind),
}

// ---------------------------------------------------------------------------
// Facet
// ---------------------------------------------------------------------------

/// A single belief facet's state. Carries the current EMA value, the
/// species/world prior it decays toward, the confidence/recency strength,
/// and metadata about the most recent update.
///
/// Range conventions:
/// - `value`: facet-specific. `affiliation_history` is `[-1.0, 1.0]`; all
///   others are `[0.0, 1.0]`. Callers clamp; the substrate does not enforce.
/// - `prior`: same range as `value`. Default 0.0; species priors override
///   on `<Predator>` family construction.
/// - `strength`: `[0.0, 1.0]`. 0 means "no evidence" (entry is forgettable);
///   1 means "just observed". Pass B decays toward 0; pass A lifts toward 1.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Facet {
    pub value: f32,
    pub prior: f32,
    pub strength: f32,
    pub last_source: EvidenceKind,
    pub last_updated_tick: u64,
}

impl Facet {
    /// Construct a facet seeded with `prior` as both initial value and
    /// passive-decay target. Strength starts at 1.0 because species-level
    /// priors are instinctive confidence — the model would otherwise be
    /// immediately culled by `belief_integrator`'s strength-based
    /// `Forgetting` sweep on the same Pass B that implants it. Decay
    /// thereafter drifts strength toward 0 normally as evidence ages.
    pub fn from_prior(prior: f32) -> Self {
        Self {
            value: prior,
            prior,
            strength: 1.0,
            last_source: EvidenceKind::Implant,
            last_updated_tick: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// EvidenceKind
// ---------------------------------------------------------------------------

/// How a facet update was sourced. Ticket 258 v1 wires `Observation`,
/// `Implant`, and `Forgetting`; the remaining four variants exist so
/// consumer tickets can wire emit paths without re-shaping the enum.
/// Mirrors the partial-wiring shape of `AbandonReason` in ticket 126.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum EvidenceKind {
    /// Direct witness via [`WitnessableEvent`](crate::messages::witnessable_event::WitnessableEvent).
    /// The primary v1 update path. Includes the conspecific-as-sensor
    /// subcase (relay's reaction as evidence about a third subject).
    #[default]
    Observation,
    /// Species priors at spawn or first-encounter. v1 wired for
    /// `<Predator>` violence-capability initialization.
    Implant,
    /// Passive decay reached zero strength — entry is pending removal in
    /// pass B. v1 wired.
    Forgetting,
    /// Feature-similarity copying (e.g. one fox reminds cat of another →
    /// fear copies). Variant ships; emit path follows in a consumer ticket.
    Transference,
    /// Probabilistic invention weighted by colony distribution. Variant
    /// ships; emit path follows in a consumer ticket.
    Confabulation,
    /// Acting on belief reinforces it (panic self-reinforces). Variant
    /// ships; emit path follows in a consumer ticket.
    Declaration,
    /// Probabilistic drift per tick; modulated by `memory` personality
    /// attribute. Variant ships; emit path follows in a consumer ticket.
    Mutation,
}

// ---------------------------------------------------------------------------
// MentalModel
// ---------------------------------------------------------------------------

/// A perceiver's belief state about a single known subject (another cat, a
/// location, a predator, or an environmental context). Six facets per the
/// ticket's v1 table.
///
/// `evidence_count` increments on every pass-A update — it's the input to
/// the future `predictability` derivation (currently a separate facet that
/// the integrator updates directly).
///
/// `candidates` carries alternative belief values that contradict the
/// current one with insufficient strength to swap (per ticket 258's
/// candidate-belief tracking rules from 007). Empty in v1 since the
/// candidate-revision system is deferred to a follow-on; the field is here
/// so consumer tickets don't have to revise the struct.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MentalModel {
    pub perceived_injury_level: Facet,
    pub perceived_intent_clarity: Facet,
    pub recency_of_threat_cue: Facet,
    pub perceived_violence_capability: Facet,
    pub affiliation_history: Facet,
    pub predictability: Facet,
    /// How hostile does the subject appear toward the perceiver? Range
    /// `[0.0, 1.0]`. Authored by ticket 261's integrator path on
    /// `WitnessableEvent::Attack` — direct observation of aggression
    /// against me (or against any cat in my model) is the v1 source.
    /// Consumed by the `Fawn`, `Threaten`, `Posture`, `Hiss`, and social
    /// `Socialize` / `GroomOther` affordance estimators.
    ///
    /// Distinct from `affiliation_history`: hostility is a fast,
    /// state-flavored read on *aggressive intent right now*; affiliation
    /// is a slow reputational summary of historical bond. A friendly
    /// cat that suddenly attacks reads high hostility AND positive
    /// affiliation — both signals coexist.
    pub perceived_hostility: Facet,
    /// How receptive does the subject appear to affiliative overtures?
    /// Range `[0.0, 1.0]`. Authored on `Groom`, `Mate`, and `Care` events
    /// (witnessed affiliative practice is the signal). Consumed by the
    /// `Mate`, `Mentor`, `Socialize`, `GroomOther`, and `FeedKitten`
    /// affordance estimators.
    ///
    /// Distinct from `affiliation_history`: receptivity is "is this cat
    /// open *right now* to courtship / grooming / mentoring"; affiliation
    /// is the long-run bond. A cat with strong affiliation but low
    /// receptivity (e.g., busy hunting, or recently widowed) reads
    /// correctly: high bond, low affordance.
    pub perceived_receptivity: Facet,
    /// 293: per-cat per-location belief about prey availability. Range
    /// `[0.0, 1.0]` — high means "this location has prey", low means
    /// "this location is empty". Authored on `WitnessableEvent::Hunt`
    /// (success lifts), `HuntScentDetected` (weak lift), and
    /// `HuntSearchYieldedNoPrey` (drops, weighted by tiles searched).
    /// Replaces the legacy per-cat `HuntingPriors` dense grid; the colony
    /// view is derived via `aggregate_location_belief_snapshot` rather
    /// than maintained as a separate Resource. Slow-timescale tunables
    /// (`BeliefAxisTunables::slow()`) — spatial yield is stable, not a
    /// fast reactive signal.
    pub prey_yield: Facet,
    pub last_updated_tick: u64,
    pub evidence_count: u32,
    pub candidates: Vec<CandidateFacet>,
}

/// A tracked-but-not-adopted alternative facet value. Created when
/// contradicting evidence weaker than the current facet arrives; promoted
/// to primary when its strength surpasses the current facet's. Revision
/// rules ship in a consumer ticket; v1 leaves this empty.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CandidateFacet {
    pub which: FacetSlot,
    pub value: f32,
    pub strength: f32,
    pub last_tick: u64,
}

/// Selector for a slot within [`MentalModel`]. Used by [`CandidateFacet`]
/// to identify which facet a candidate alternative competes against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FacetSlot {
    PerceivedInjuryLevel,
    PerceivedIntentClarity,
    RecencyOfThreatCue,
    PerceivedViolenceCapability,
    AffiliationHistory,
    Predictability,
    PerceivedHostility,
    PerceivedReceptivity,
    /// 293: per-location prey-yield belief. Only meaningful on
    /// [`LocationBeliefs`] (the cat-keyed mental models don't author it).
    PreyYield,
}

// ---------------------------------------------------------------------------
// Components
// ---------------------------------------------------------------------------

/// Per-cat mental models of other cats. Entity-keyed; entries for despawned
/// cats are cleaned by `belief_integrator`'s liveness sweep.
///
/// `#[serde(skip)]` per the `pairing.rs` / `held_intention.rs` precedent —
/// raw `Entity` ids don't round-trip across saves, so the substrate
/// rebuilds state from fresh observations on load.
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CatBeliefs {
    #[serde(skip)]
    pub models: HashMap<Entity, MentalModel>,
}

/// Per-cat mental models of bucketed locations. Fully serializable — keys
/// are stable across saves.
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct LocationBeliefs {
    pub models: HashMap<LocationKey, MentalModel>,
}

/// Per-cat mental models of predator entities. Entity-keyed, same
/// serialization shape as [`CatBeliefs`]. Species priors for the four v1
/// predator kinds (Fox, Hawk, Snake, ShadowFox) seed the
/// `perceived_violence_capability` facet at first-encounter time via the
/// `Implant` evidence kind.
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PredatorBeliefs {
    #[serde(skip)]
    pub models: HashMap<Entity, MentalModel>,
}

/// Per-cat mental models of environmental contexts. Keyed on
/// [`EnvironmentalContextKey`]; fully serializable.
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ContextBeliefs {
    pub models: HashMap<EnvironmentalContextKey, MentalModel>,
}

// ---------------------------------------------------------------------------
// ColonyReservesBelief (ticket 308)
// ---------------------------------------------------------------------------

/// A per-cat subjective estimate of how many of a given reserve resource the
/// colony currently holds. Distinct shape from [`MentalModel`] because the
/// underlying observable is a count, not a `[0.0, 1.0]` EMA opinion — rounding
/// a `f32` EMA back to "low / mid / high" buckets buries information that the
/// downstream Herbcraft consideration (ticket 309) needs.
///
/// Updated by `belief_integrator::apply_observation` from three new
/// `WitnessableEvent` variants: `ReserveDeposited` increments, `ReserveConsumed`
/// decrements, and `InventoryObserved` (broadcast by
/// `gossip_inventory_observations` on each cat's stagger tick) refreshes the
/// count from the observed actor's inventory snapshot.
///
/// `strength` rises on observation (clamped to 1.0) and decays toward 0 in
/// Pass B's forgetting sweep. Zero-strength entries are dropped — the cat has
/// forgotten the reserve state for that resource.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ReserveBelief {
    pub estimated_count: u32,
    pub strength: f32,
    pub last_source: EvidenceKind,
    pub last_updated_tick: u64,
}

/// Per-cat mental models of colony-wide reserve stockpiles. Keyed on
/// [`ResourceKind`]; fully serializable. Authored by `belief_integrator`
/// (ticket 308); the consumer is the Herbcraft DSE consideration in ticket 309.
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ColonyReservesBelief {
    pub reserves: HashMap<ResourceKind, ReserveBelief>,
}

// ---------------------------------------------------------------------------
// ShelterBeliefs (ticket 374)
// ---------------------------------------------------------------------------

/// Per-cat housing-security belief, ticket 374. Four orthogonal sub-axes
/// describing the cat's relationship with its claimed `home_den`:
///
/// - `belonging` — do I have a home_den claimed at all? Lifts on
///   `WitnessableEvent::DenClaimed`; drops on `DenLost`.
/// - `quality` — belief about the home_den's structural condition.
///   Lifts on `DenRepaired`; drops on `DenDamaged`.
/// - `continuity` — how long has it been mine? Updated passively each
///   stagger period: accrues when the cat is within range of its
///   home_den, decays when away. Not event-driven.
/// - `threat` — belief about active siege or contestation. Lifts on
///   `DenSieged`; drops on `DenSiegeBroken`.
///
/// All sub-axes are `[0.0, 1.0]`. Callers (welfare rollup, pressure
/// accumulator) compose them — the substrate does not enforce ordering
/// or relations between them. Pillar 3 (orthogonal axes, not louder
/// single alarms) — each sub-axis encodes a distinct situation; the
/// cat's downstream score combines them via documented composition,
/// not by collapsing them at the perception layer.
///
/// Departs from [`Facet`]'s value/prior/strength shape because the
/// four sub-axes describe one subject (the home_den) rather than four
/// independent beliefs about four independent subjects. A single
/// `last_updated_tick` covers the whole struct — sub-axis read-cadence
/// concerns are downstream.
#[derive(Debug, Clone, Copy, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ShelterFacet {
    pub belonging: f32,
    pub quality: f32,
    pub continuity: f32,
    pub threat: f32,
    pub last_updated_tick: u64,
}

/// Per-cat shelter belief + claimed home_den entity, ticket 374.
///
/// `home_den` carries `#[serde(skip)]` matching [`CatBeliefs::models`]
/// — raw `Entity` ids don't round-trip across saves. On load,
/// re-established by the spawn-time claim system reading nearest
/// functional Den; in-sim, set by `DenClaimed` integrator wiring.
///
/// Replaces the per-tick spatial proximity rollup at
/// `colony_score::compute_shelter` and the `unsheltered_sleepers`
/// counting at `coordination::assess_colony_needs`. The legacy
/// signals were structurally brittle (collapsed to zero under metric
/// changes, gated on rare `Sleep`+distance conjunctions); the belief
/// substrate makes housing security a continuous psychological state
/// the cat carries rather than a transient spatial query.
#[derive(Component, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ShelterBeliefs {
    #[serde(skip)]
    pub home_den: Option<Entity>,
    pub facet: ShelterFacet,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Bucket a position into a [`LocationKey`] using the canonical 5-tile
/// bucket size (matches `ColonyKnowledge::bucket_position`).
pub fn bucket_position(x: i32, y: i32) -> LocationKey {
    const BUCKET: i32 = 5;
    (x.div_euclid(BUCKET), y.div_euclid(BUCKET))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facet_from_prior_seeds_value_and_target() {
        let f = Facet::from_prior(0.7);
        assert_eq!(f.value, 0.7);
        assert_eq!(f.prior, 0.7);
        assert!((f.strength - 1.0).abs() < f32::EPSILON);
        assert_eq!(f.last_source, EvidenceKind::Implant);
    }

    #[test]
    fn bucket_position_uses_5_tile_cells() {
        assert_eq!(bucket_position(0, 0), (0, 0));
        assert_eq!(bucket_position(4, 4), (0, 0));
        assert_eq!(bucket_position(5, 5), (1, 1));
        assert_eq!(bucket_position(-1, -1), (-1, -1));
        assert_eq!(bucket_position(-5, -5), (-1, -1));
        assert_eq!(bucket_position(-6, -6), (-2, -2));
    }

    #[test]
    fn empty_models_are_default() {
        let m = MentalModel::default();
        assert_eq!(m.evidence_count, 0);
        assert!(m.candidates.is_empty());
        assert_eq!(m.perceived_violence_capability.value, 0.0);
    }
}
