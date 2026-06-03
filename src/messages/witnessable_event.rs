//! `WitnessableEvent` — perceivable side-effects broadcast by action resolvers
//! and consumed by `belief_integrator` (ticket 258 C3 substrate).
//!
//! Each variant describes an event that happened in the world. The message is
//! broadcast; the integrator system determines which cats are witnesses by
//! checking sensing range against `position`, and updates per-witness mental
//! models accordingly. This matches the `PreyKilled { kind, position }`
//! precedent — `witness` is not a field of the event, it's a property of
//! consumption.

use bevy_ecs::prelude::*;

use crate::components::body_zones::BodyPart;
use crate::components::disposition::DispositionKind;
use crate::components::magic::ResourceKind;
use crate::components::physical::{InjurySource, Position};
use crate::components::prey::PreyKind;

/// Observable simulation events that update per-cat mental models.
///
/// Ticket 258 v1 wires `Observation` evidence updates from these messages.
/// Other evidence kinds (`Transference`, `Confabulation`, `Declaration`,
/// `Mutation`) are scaffolded in [`EvidenceKind`](crate::components::beliefs::EvidenceKind)
/// but not yet emitted; consumer tickets wire their emit paths.
#[derive(bevy_ecs::prelude::Message, Debug, Clone)]
pub enum WitnessableEvent {
    /// A cat attacked another cat. Updates witnesses' beliefs about both
    /// participants — actor's violence capability lifts, target's perceived
    /// injury level lifts, location's recency-of-threat-cue maxes.
    Attack {
        actor: Entity,
        target: Entity,
        position: Position,
        /// Severity of the blow in `[0.0, 1.0]`. Drives EMA observed-value.
        severity: f32,
        tick: u64,
    },
    /// A cat groomed another cat. Witnesses' affiliation-history facet for
    /// the actor lifts (positive social signal).
    Groom {
        actor: Entity,
        target: Entity,
        position: Position,
        tick: u64,
    },
    /// A cat completed a mating interaction. Strongest positive affiliation
    /// signal in the substrate.
    Mate {
        actor: Entity,
        target: Entity,
        position: Position,
        tick: u64,
    },
    /// A cat fed or cared for a kitten. Affiliation-history positive.
    Care {
        caregiver: Entity,
        kitten: Entity,
        position: Position,
        tick: u64,
    },
    /// A cat fled from a threat. Witnesses learn the fleer's predictability
    /// and the threat's perceived-violence-capability; the location's
    /// recency-of-threat-cue lifts.
    FleeFrom {
        fleer: Entity,
        threat: Entity,
        position: Position,
        tick: u64,
    },
    /// A cat completed a hunt attempt. `success = true` lifts the hunter's
    /// perceived-violence-capability and predictability; failure dampens both.
    Hunt {
        hunter: Entity,
        prey_kind: PreyKind,
        position: Position,
        success: bool,
        tick: u64,
    },
    /// A cat showed an observable startle response. Consumed by the
    /// conspecific-as-sensor evidence subtype (decision 16 in ticket 258):
    /// the relay's reaction is evidence about a *different* subject (the
    /// environmental context), with credibility weighted by the relay's
    /// state at the cue.
    ///
    /// **Not yet emitted in v1** — depends on body-cue substrate (ticket
    /// 242). Variant exists so consumer wiring (the door-slam scenario in
    /// 242's exit criteria) can land without re-shaping the message enum.
    ConspecificStartle {
        startled: Entity,
        position: Position,
        relay_state: RelayState,
        tick: u64,
    },
    /// An ambient environmental shock (door slam, thunderclap, etc.) heard
    /// within `range` of `position`. Lifts witnesses' here-now
    /// recency-of-threat-cue.
    ///
    /// **Not yet emitted in v1** — needs a weather/world-event hook. Variant
    /// exists so consumer wiring can land in a later ticket.
    AmbientShock {
        position: Position,
        intensity: f32,
        range: f32,
        tick: u64,
    },
    /// A cat's own plan failed to materialize for a given disposition
    /// (`evaluate_and_plan` couldn't produce a chain). Self-observation:
    /// the witness IS the cat. Lowers the cat's own
    /// `ContextBeliefs[DispositionExecution(kind)].predictability` —
    /// the EMA-based successor to `RecentDispositionFailures` (ticket
    /// 258 proxy retirement).
    SelfPlanFailed {
        cat: Entity,
        disposition: DispositionKind,
        position: Position,
        tick: u64,
    },
    /// A cat dropped a reserve-kind herb (thornbriar / remedy-herb) into a
    /// Stores building. Updates witnesses' `ColonyReservesBelief` for the
    /// matching `ResourceKind` — the depositor's contribution is authoritative
    /// (self-witness); nearby cats integrate it as additive evidence.
    ///
    /// Ticket 308.
    ReserveDeposited {
        actor: Entity,
        kind: ResourceKind,
        position: Position,
        tick: u64,
    },
    /// A cat consumed a reserve-kind herb during a magic resolver
    /// (`resolve_set_ward` Thornward branch, `resolve_prepare_remedy`).
    /// Updates witnesses' `ColonyReservesBelief` — symmetric decrement to
    /// `ReserveDeposited`.
    ///
    /// Ticket 308.
    ReserveConsumed {
        actor: Entity,
        kind: ResourceKind,
        position: Position,
        tick: u64,
    },
    /// A per-cat "what I'm currently carrying" snapshot, broadcast on each
    /// cat's stagger tick by `gossip_inventory_observations`. Narrative
    /// framing: cats communicate about what they hold. Implementation:
    /// god-eye sensor — every witness within range adopts the actor's
    /// declared inventory as additive lower-bound evidence about the
    /// colony pool. The actor's self-update is authoritative.
    ///
    /// Ticket 308.
    InventoryObserved {
        actor: Entity,
        position: Position,
        inventory: Vec<(ResourceKind, u32)>,
        tick: u64,
    },
    /// A cat performed an observable play-bow solicitation posture. Witnesses
    /// lift `perceived_intent_clarity` on the actor (strongest play-engagement
    /// signal) and `perceived_receptivity` at lower strength. Ticket 279.
    PlayBow {
        actor: Entity,
        position: Position,
        tick: u64,
    },
    /// `actor` moved into engagement range of `target` while a recent play-bow
    /// or reciprocal-advance from `target` toward `actor` was within
    /// `reciprocal_window_ticks`. Mutual-engagement signal: when `target` is
    /// the witness, this is "they advanced toward me" (full-strength lift on
    /// `perceived_intent_clarity`); third-party witnesses lift at half
    /// strength. Ticket 279.
    ReciprocalAdvance {
        actor: Entity,
        target: Entity,
        position: Position,
        tick: u64,
    },
    /// `actor` and `target` remained within sensing range of each other for
    /// `ticks_held` consecutive ticks. Substrate-honest reframe of the
    /// "sustained orientation" cue named in the JointIntention rustdoc — no
    /// `Heading` substrate exists today, so this records continuous
    /// co-presence rather than directional facing. Generic engagement signal
    /// (used by Courtship and PlayBout in downstream consumers). Lift on
    /// `perceived_intent_clarity` scales by `ticks_held`. Ticket 279.
    SustainedCoPresence {
        actor: Entity,
        target: Entity,
        ticks_held: u32,
        position: Position,
        tick: u64,
    },
    /// 294 — A predator successfully ambushed a cat. Lifts witnesses'
    /// `LocationBeliefs[bucket(position)].recency_of_threat_cue` to
    /// `OBSERVED_MAX` — the per-cat substrate replacement for the
    /// retired colony-wide `RecentAmbushMap`. Witness gating is the
    /// integrator's standard `WITNESS_RANGE = 10` Manhattan check; only
    /// nearby cats learn first-hand, with colony-level aggregation
    /// derived via `belief_aggregation::aggregated_location_belief` for
    /// readers that need a colony view (e.g. ward placement).
    PredatorAmbush {
        predator: Entity,
        victim: Entity,
        position: Position,
        tick: u64,
    },
    /// 472 — `actor` carries a `WoundKind::Festering` wound on the named
    /// `body_part`. Emitted at a throttled cadence per festering cat by
    /// `emit_festering_observations` rather than at the moment the wound
    /// is authored (festering is a *persistent state*, not a one-shot
    /// event — witnesses build belief over many observations of the
    /// same wound, not one). Updates witnesses' `perceived_injury_level`
    /// on the actor; lift scales by severity (the current
    /// `tissue_damage` on the festering part). `source_kind` carries
    /// the curse's origin (today: `InjurySource::MagicMisfire`; future
    /// kin-care tickets may emit other sources). Self-witness skipped
    /// per the 258 invariant — self-festering is handled by the
    /// `OwnInjurySite` interoceptive anchor from ticket 089.
    CarriesFesteringWound {
        actor: Entity,
        body_part: BodyPart,
        source_kind: InjurySource,
        severity: f32,
        position: Position,
        tick: u64,
    },
}

/// Behavioral state of a relay cat at the moment of a startle cue. Drives
/// the credibility weight when the integrator treats a relay's reaction as
/// evidence about a separate subject (conspecific-as-sensor, ticket 258
/// decision 16). A scaredy-cat who startles while sleeping is a
/// low-credibility relay; a stoic cat who startles while alert is a
/// high-credibility relay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RelayState {
    Sleeping,
    Resting,
    Alert,
    Engaged,
}
