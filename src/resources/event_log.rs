use std::collections::BTreeMap;
use std::collections::VecDeque;

use bevy_ecs::prelude::Resource;

use crate::ai::Action;
use crate::ai::planner::PlanningFailureReason;
use crate::components::personality::Personality;
use crate::components::physical::Needs;
use crate::components::skills::Skills;

// ---------------------------------------------------------------------------
// Snapshot sub-types
// ---------------------------------------------------------------------------

/// A relationship entry for the CatSnapshot event.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RelationshipEntry {
    pub cat: String,
    pub fondness: f32,
    pub familiarity: f32,
    pub romantic: f32,
    pub bond: Option<String>,
}

/// One wild-predator position in a spatial snapshot.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WildlifePosRow {
    pub species: String,
    pub x: i32,
    pub y: i32,
}

/// One prey position in a spatial snapshot.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreyPosRow {
    pub species: String,
    pub x: i32,
    pub y: i32,
}

/// One prey den entry in a den snapshot.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PreyDenRow {
    pub species: String,
    pub x: i32,
    pub y: i32,
    pub spawns_remaining: u32,
    pub capacity: u32,
    pub predation_pressure: f32,
}

/// One fox den entry in a den snapshot.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FoxDenRow {
    pub x: i32,
    pub y: i32,
    pub cubs_present: u32,
    pub territory_radius: i32,
    pub scent_strength: f32,
}

/// Outcome of a single discrete hunt attempt — one APPROACH→STALK→CHASE→POUNCE
/// cycle on a single target. Maps 1:1 onto the failure-reason strings emitted
/// by `resolve_engage_prey` so per-discrete-attempt success rate is recoverable
/// from `events.jsonl` without conflating with within-attempt retargeting.
///
/// All three `Killed*` variants count toward the success numerator: a kill
/// happened, regardless of what the cat did next (deposit / replan for
/// multi-kill / consume on-spot). The three `Lost*` variants distinguish
/// which sub-phase the attempt failed in. `Abandoned` covers external
/// invalidation (target despawned / plan replaced).
///
/// Cross-reference ticket 037: this event is emitted at outcome resolution,
/// not on `StepResult::Advance` — witness-gated by construction.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HuntOutcome {
    /// Successful pounce; inventory had space; advance to deposit step.
    Killed,
    /// Successful pounce; multi-kill loop replanned for another target.
    KilledAndReplanned,
    /// Successful pounce; cat ate the catch on-spot.
    KilledAndConsumed,
    /// Prey escaped during approach (fled distance exceeded give-up threshold).
    LostDuringApproach,
    /// Cat got stuck while stalking, or anxiety spooked the prey before pounce.
    LostDuringStalk,
    /// Chase exceeded duration limit, or cat got stuck while chasing.
    LostDuringChase,
    /// Target invalidated externally (despawned, removed, or otherwise gone).
    Abandoned,
}

impl HuntOutcome {
    /// True iff this outcome counts as a successful kill for the audit.
    pub const fn is_kill(self) -> bool {
        matches!(
            self,
            HuntOutcome::Killed | HuntOutcome::KilledAndReplanned | HuntOutcome::KilledAndConsumed
        )
    }
}

// ---------------------------------------------------------------------------
// EventKind
// ---------------------------------------------------------------------------

/// Structured event types for mechanical debugging.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum EventKind {
    ActionChosen {
        cat: String,
        action: Action,
        score: f32,
        runner_up: Action,
        runner_up_score: f32,
        third: Action,
        third_score: f32,
    },
    CatSnapshot {
        cat: String,
        position: (i32, i32),
        personality: Personality,
        needs: Needs,
        skills: Skills,
        mood_valence: f32,
        mood_modifier_count: usize,
        health: f32,
        corruption: f32,
        magic_affinity: f32,
        current_action: Action,
        relationships: Vec<RelationshipEntry>,
        /// All gate-open scored actions from the last decision, sorted
        /// descending (post-bonus, post-suppression). Cap is the size of the
        /// Action enum.
        last_scores: Vec<(Action, f32)>,
        /// LifeStage at snapshot time ("Kitten", "Adolescent", "Adult", "Elder").
        life_stage: String,
        /// Gender at snapshot time ("Tom", "Queen", "Nonbinary").
        sex: String,
        /// Orientation at snapshot time.
        orientation: String,
        is_pregnant: bool,
        /// Season at snapshot time — lets `jq` slice mating-need trajectories.
        season: String,
        /// §7.W social_warmth fulfillment axis (0.0–1.0).
        social_warmth: f32,
    },
    FoodLevel {
        current: f32,
        capacity: f32,
        fraction: f32,
    },
    PopulationSnapshot {
        mice: usize,
        rats: usize,
        rabbits: usize,
        fish: usize,
        birds: usize,
    },
    /// Wild predator census, emitted on the same cadence as FoodLevel /
    /// PopulationSnapshot. Counts live WildAnimal entities grouped by species.
    WildlifePopulation {
        foxes: u32,
        hawks: u32,
        snakes: u32,
        shadow_foxes: u32,
    },
    /// Sampled wild predator positions for the dashboard map overlay. One
    /// row per live WildAnimal entity.
    WildlifePositions {
        positions: Vec<WildlifePosRow>,
    },
    /// Sampled prey positions for the dashboard map overlay. Emitted at a
    /// coarser cadence than wildlife because there are many more prey.
    PreyPositions {
        positions: Vec<PreyPosRow>,
    },
    /// Prey dens and fox dens — near-static, emitted infrequently so the
    /// dashboard can draw them without re-querying the whole log.
    DenSnapshot {
        prey_dens: Vec<PreyDenRow>,
        fox_dens: Vec<FoxDenRow>,
    },
    /// Downsampled colony hunting belief grid (aggregate over all cats).
    /// Values are row-major with length = width * height. The `cat` field
    /// is reserved for per-cat snapshots in a future extension — colony
    /// aggregate emits `None`.
    HuntingBeliefSnapshot {
        cat: Option<String>,
        width: u32,
        height: u32,
        values: Vec<f32>,
    },
    PositionTrace {
        cat: String,
        position: (i32, i32),
        action: Action,
    },
    CoordinatorElected {
        cat: String,
        social_weight: f32,
    },
    DirectiveIssued {
        coordinator: String,
        kind: String,
        priority: f32,
    },
    Death {
        cat: String,
        cause: String,
        /// For Injury deaths: the source of the most recent unhealed injury.
        #[serde(skip_serializing_if = "Option::is_none")]
        injury_source: Option<String>,
        /// Tile where the cat died.
        location: (i32, i32),
    },
    /// A predator struck a cat. Emitted per successful ambush hit.
    Ambush {
        cat: String,
        predator_species: String,
        location: (i32, i32),
        damage: f32,
    },
    /// A cat successfully placed a ward.
    WardPlaced {
        cat: String,
        ward_kind: String,
        location: (i32, i32),
        strength: f32,
    },
    /// A ward expired (decayed to zero). Separate from WardPlaced so you can
    /// answer "is siege-heavy decay concentrated near the colony edge?"
    WardDespawned {
        ward_kind: String,
        location: (i32, i32),
        sieged: bool,
    },
    /// A cat killed a prey animal.
    PreyKilled {
        cat: String,
        species: String,
        location: (i32, i32),
    },
    /// A discrete hunt attempt resolved (kill / lost / abandoned).
    /// Sibling to `PreyKilled`: every `Killed*` variant of this event has a
    /// matching `PreyKilled` at the same tick, but `HuntAttempt` also fires on
    /// failed attempts so per-discrete-attempt success rate is recoverable.
    /// Surface: `just q hunt-success <run-dir>` aggregates these.
    HuntAttempt {
        cat: String,
        prey_species: String,
        location: (i32, i32),
        outcome: HuntOutcome,
        start_tick: u64,
        end_tick: u64,
        /// Manhattan distance between cat and prey at attempt start (cached
        /// from the engage-prey entry tick). Useful for binning success by
        /// approach difficulty.
        start_distance: i32,
        /// Verbatim `StepResult::Fail` reason string when the outcome is a
        /// `Lost*` or `Abandoned`; `None` for the three `Killed*` outcomes.
        /// Lets future audits cross-reference the existing
        /// `plan_failures_by_reason` footer without losing the discrete-attempt
        /// boundary.
        failure_reason: Option<String>,
    },
    /// A kitten was born.
    KittenBorn {
        mother: String,
        kitten: String,
        location: (i32, i32),
    },
    /// Two cats mated.
    MatingOccurred {
        partner_a: String,
        partner_b: String,
        location: (i32, i32),
    },
    /// A building reached completion.
    BuildingConstructed {
        kind: String,
        location: (i32, i32),
    },
    /// A shadow-fox materialized from corrupted ground.
    ShadowFoxSpawn {
        location: (i32, i32),
        corruption: f32,
    },
    /// A posse of cats banished a shadow-fox — a colony-defining moment.
    /// The `posse` field captures every cat who contributed to the kill,
    /// so downstream tooling can reconstruct the heroes of the story.
    ShadowFoxBanished {
        posse: Vec<String>,
        location: (i32, i32),
    },

    // -------------------------------------------------------------------
    // Continuity-canary events (§11.3 — "Emit events for: grooming fires,
    // play fires, mentoring fires, burial fires, courtship fires,
    // mythic-texture events"). Canary-class tallies live in
    // `EventLog::continuity_tallies` and surface in the headless footer;
    // `just check-continuity` fails when any canary is at zero.
    //
    // Some classes piggyback on existing events rather than needing a
    // new variant — MatingOccurred tallies as `courtship`,
    // ShadowFoxBanished tallies as `mythic-texture`. Play and Burial
    // have no emitting system today and stay at zero until the
    // corresponding features land.
    // -------------------------------------------------------------------
    /// A cat completed a grooming action. `target` is `None` for self-
    /// groom, `Some(name)` for grooming another cat.
    GroomingFired {
        cat: String,
        target: Option<String>,
    },
    /// A cat completed a mentoring action with a specific apprentice.
    MentoringFired {
        mentor: String,
        apprentice: String,
    },
    /// A cat buried a deceased companion. Reserved; no emitting system
    /// exists today — a burial action lands with the "broaden sideways"
    /// epic (`docs/systems/project-vision.md` §5).
    BurialFired {
        cat: String,
        deceased: String,
    },
    /// A cat engaged in play. Reserved; no emitting system today.
    PlayFired {
        cat: String,
        partner: Option<String>,
    },
    /// A pair of cats accumulated romantic attraction under the
    /// courtship-drift gate in `social::check_bonds`. Lighter-weight
    /// than `MatingOccurred` — registers an observable courtship tick,
    /// not a consummation. Tallies as `continuity_tallies.courtship`
    /// alongside `MatingOccurred`, decoupling the canary from the
    /// MateWith path that ticket 027 Bug 2 / Bug 3 are still unblocking.
    CourtshipDrifted {
        cat_a: String,
        cat_b: String,
    },
    /// A mythic-texture event — Calling fired, named-object crafted,
    /// cat-on-cat banishment, or visitor arrival. ShadowFoxBanished
    /// tallies as mythic-texture too, without a separate variant.
    MythicTexture {
        /// `"calling"` | `"named-object"` | `"banishment"` | `"visitor-arrival"`.
        subclass: String,
        subject: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
    ColonyScore {
        // Welfare snapshot [0.0, 1.0]
        shelter: f32,
        nourishment: f32,
        health: f32,
        happiness: f32,
        fulfillment: f32,
        welfare: f32,

        // Cumulative ledger
        seasons_survived: u64,
        bonds_formed: u64,
        peak_population: u64,
        deaths_starvation: u64,
        deaths_old_age: u64,
        deaths_injury: u64,
        aspirations_completed: u64,
        structures_built: u64,
        kittens_born: u64,
        prey_dens_discovered: u64,

        // Point-in-time bond tier snapshot. Each unordered pair (A, B) appears
        // once since `Relationships` canonicalizes keys. Diagnoses whether
        // courtship drift ever reaches Partners/Mates.
        friends_count: u64,
        partners_count: u64,
        mates_count: u64,

        // Aggregate
        aggregate: f64,

        // Activation — split by feature valence so the "is the colony thriving?"
        // signal (positive_activation_score) is not polluted by deaths and other
        // adverse events. See src/resources/system_activation.rs FeatureCategory.
        positive_activation_score: f64,
        positive_features_active: u32,
        positive_features_total: u32,
        negative_events_total: u64,
        neutral_features_active: u32,
        neutral_features_total: u32,

        // Context
        living_cats: u64,
    },
    SystemActivation {
        // BTreeMap (not HashMap) so JSON key order in events.jsonl is stable
        // across processes — the determinism contract for replay regression
        // tests (`just verdict` consumes these as strings).
        positive: BTreeMap<String, u64>,
        negative: BTreeMap<String, u64>,
        neutral: BTreeMap<String, u64>,
    },

    // ----- Plan lifecycle events -----
    /// A new GOAP plan was created for a cat.
    PlanCreated {
        cat: String,
        disposition: String,
        steps: Vec<String>,
        hunger: f32,
        energy: f32,
        temperature: f32,
        food_available: bool,
    },

    /// A new GOAP plan was created for a fox. Fox-specific because FoxNeeds
    /// uses a 3-level Maslow hierarchy (not the cat 7-level), and because fox
    /// dispositions (Hunting / Patrolling / Raiding / Resting / ...) are
    /// disjoint from cat dispositions. See src/ai/fox_scoring.rs.
    ///
    /// `fox_id` is the ECS entity id bits — stable within a run, not across runs.
    FoxPlanCreated {
        fox_id: u64,
        disposition: String,
        steps: Vec<String>,
        hunger: f32,
        territory_scent: f32,
        cub_satiation: f32,
        position: (i32, i32),
        day_phase: String,
    },

    /// An interrupt forced a plan to be abandoned.
    PlanInterrupted {
        cat: String,
        disposition: String,
        reason: String,
        current_step: String,
        hunger: f32,
        energy: f32,
        temperature: f32,
    },

    /// A plan step could not execute — this is a bug canary, not normal
    /// gameplay. If a planned step fails, the planner made a promise the
    /// executor can't keep.
    PlanStepFailed {
        cat: String,
        disposition: String,
        step: String,
        step_index: usize,
        reason: String,
        hunger: f32,
        energy: f32,
        temperature: f32,
    },

    /// A plan was regenerated after step failure (replan attempt).
    PlanReplanned {
        cat: String,
        disposition: String,
        replan_count: u32,
        new_steps: Vec<String>,
        hunger: f32,
        energy: f32,
        temperature: f32,
    },

    /// Ticket 091: the planner returned `Err(_)` for a chosen disposition —
    /// the cat had no executable plan and silently idled. Pre-091 this
    /// path emitted nothing, hiding producer-side starvation cascades
    /// from every canary.
    ///
    /// Ticket 172: `reason` was promoted from a stringly-typed
    /// `"no_plan_found"` constant to the typed
    /// [`PlanningFailureReason`] enum so the headless-footer aggregator
    /// (`planning_failures_by_reason`) can attribute the post-155
    /// residual plan-failure surface to a specific cause —
    /// substrate eligibility (`NoApplicableActions`), action effects
    /// (`GoalUnreachable`), or search budget (`NodeBudgetExhausted`).
    /// The events.jsonl payload now carries the variant name as a
    /// string (`"NoApplicableActions"` / `"GoalUnreachable"` /
    /// `"NodeBudgetExhausted"`) instead of the prior `"no_plan_found"`.
    PlanningFailed {
        cat: String,
        disposition: String,
        reason: PlanningFailureReason,
        hunger: f32,
        energy: f32,
        temperature: f32,
        food_available: bool,
        has_stored_food: bool,
    },
}

// ---------------------------------------------------------------------------
// EventEntry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct EventEntry {
    pub tick: u64,
    #[serde(flatten)]
    pub kind: EventKind,
}

// ---------------------------------------------------------------------------
// EventLog resource
// ---------------------------------------------------------------------------

/// Ring-buffer of structured simulation events for debugging.
///
/// Also carries cumulative diagnostic tallies (deaths by cause, plan failures
/// by reason, interrupts by reason) used by the headless runner's end-of-sim
/// footer. Tallies are cumulative across the whole run — they don't decay
/// when ring-buffer entries are evicted.
#[derive(Resource, Debug)]
pub struct EventLog {
    pub entries: VecDeque<EventEntry>,
    pub capacity: usize,
    pub total_pushed: u64,
    // BTreeMap (not HashMap) so the JSON serialization order in the
    // headless footer is stable across processes — see the determinism
    // contract notes on `EventKind::SystemActivation::positive`.
    pub deaths_by_cause: BTreeMap<String, u64>,
    pub plan_failures_by_reason: BTreeMap<String, u64>,
    /// Ticket 091: per-disposition tally of `make_plan → Err` outcomes.
    /// Distinguishes "the planner can't find a plan for X" from runtime
    /// step failures (`plan_failures_by_reason`). A high entry here means
    /// the IAUS layer is electing X but the GOAP layer can't satisfy it —
    /// the producer-side starvation pattern 091 was opened to fix.
    pub planning_failures_by_disposition: BTreeMap<String, u64>,
    /// Ticket 172: per-`(disposition, reason)` tally of `make_plan → Err`
    /// outcomes. Keys are formatted as `"<Disposition>:<Reason>"`
    /// (e.g., `"Cooking:NoApplicableActions"`). Distinguishes
    /// substrate-eligibility failures (`NoApplicableActions` —
    /// nothing applicable from start), action-effect failures
    /// (`GoalUnreachable` — search drained without satisfying goal),
    /// and search-budget failures (`NodeBudgetExhausted` — `max_nodes`
    /// hit). Read by the post-155 plan-failure triage to attribute
    /// the residual Cooking + Herbalism surface to a specific cause.
    pub planning_failures_by_reason: BTreeMap<String, u64>,
    pub interrupts_by_reason: BTreeMap<String, u64>,
    /// Continuity-canary class counters. Six fixed keys: `grooming`,
    /// `play`, `mentoring`, `burial`, `courtship`, `mythic-texture`.
    /// Populated by `push()` from the corresponding canary event
    /// variants and from existing events that map to a canary class
    /// (MatingOccurred → courtship, ShadowFoxBanished → mythic-texture).
    /// Serialized into the headless footer for `just check-continuity`.
    pub continuity_tallies: BTreeMap<String, u64>,
}

impl Default for EventLog {
    fn default() -> Self {
        let mut continuity_tallies = BTreeMap::new();
        // Initialize all six keys to zero so the footer always reports
        // the canary set; a missing key is indistinguishable from zero
        // in JSON but the explicit zero is clearer to readers.
        for key in [
            "grooming",
            "play",
            "mentoring",
            "burial",
            "courtship",
            "mythic-texture",
        ] {
            continuity_tallies.insert(key.to_string(), 0);
        }
        Self {
            entries: VecDeque::new(),
            capacity: 500,
            total_pushed: 0,
            deaths_by_cause: BTreeMap::new(),
            plan_failures_by_reason: BTreeMap::new(),
            planning_failures_by_disposition: BTreeMap::new(),
            planning_failures_by_reason: BTreeMap::new(),
            interrupts_by_reason: BTreeMap::new(),
            continuity_tallies,
        }
    }
}

impl EventLog {
    pub fn push(&mut self, tick: u64, kind: EventKind) {
        match &kind {
            EventKind::Death {
                cause,
                injury_source,
                ..
            } => {
                // For injury deaths, key by injury_source (ShadowFoxAmbush, Fox, ...)
                // so the dominant killer surfaces immediately. Fall back to
                // the cause enum name for non-injury deaths (Starvation, OldAge).
                let key = if cause == "Injury" {
                    injury_source.clone().unwrap_or_else(|| "Injury".into())
                } else {
                    cause.clone()
                };
                *self.deaths_by_cause.entry(key).or_insert(0) += 1;
            }
            EventKind::Ambush { .. }
            | EventKind::WardPlaced { .. }
            | EventKind::WardDespawned { .. }
            | EventKind::PreyKilled { .. }
            | EventKind::HuntAttempt { .. }
            | EventKind::KittenBorn { .. }
            | EventKind::BuildingConstructed { .. }
            | EventKind::ShadowFoxSpawn { .. } => {
                // Tallied only as raw events; no aggregate HashMap needed.
            }
            EventKind::MatingOccurred { .. } => {
                *self
                    .continuity_tallies
                    .entry("courtship".into())
                    .or_insert(0) += 1;
            }
            EventKind::CourtshipDrifted { .. } => {
                *self
                    .continuity_tallies
                    .entry("courtship".into())
                    .or_insert(0) += 1;
            }
            EventKind::ShadowFoxBanished { .. } => {
                *self
                    .continuity_tallies
                    .entry("mythic-texture".into())
                    .or_insert(0) += 1;
            }
            EventKind::GroomingFired { .. } => {
                *self
                    .continuity_tallies
                    .entry("grooming".into())
                    .or_insert(0) += 1;
            }
            EventKind::MentoringFired { .. } => {
                *self
                    .continuity_tallies
                    .entry("mentoring".into())
                    .or_insert(0) += 1;
            }
            EventKind::BurialFired { .. } => {
                *self.continuity_tallies.entry("burial".into()).or_insert(0) += 1;
            }
            EventKind::PlayFired { .. } => {
                *self.continuity_tallies.entry("play".into()).or_insert(0) += 1;
            }
            EventKind::MythicTexture { .. } => {
                *self
                    .continuity_tallies
                    .entry("mythic-texture".into())
                    .or_insert(0) += 1;
            }
            EventKind::PlanStepFailed { step, reason, .. } => {
                let key = format!("{step}: {reason}");
                *self.plan_failures_by_reason.entry(key).or_insert(0) += 1;
            }
            EventKind::PlanInterrupted { reason, .. } => {
                *self.interrupts_by_reason.entry(reason.clone()).or_insert(0) += 1;
            }
            EventKind::PlanningFailed {
                disposition, reason, ..
            } => {
                *self
                    .planning_failures_by_disposition
                    .entry(disposition.clone())
                    .or_insert(0) += 1;
                // 172: also tally by `(disposition, reason)` so triage
                // can attribute the residual post-155 plan-failure
                // surface (Cooking 2126 / Herbalism 1712) to a
                // specific cause without re-running the soak with a
                // focal trace.
                let composite_key = format!("{}:{}", disposition, reason.as_str());
                *self
                    .planning_failures_by_reason
                    .entry(composite_key)
                    .or_insert(0) += 1;
            }
            _ => {}
        }
        self.entries.push_back(EventEntry { tick, kind });
        self.total_pushed += 1;
        while self.entries.len() > self.capacity {
            self.entries.pop_front();
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_adds_entry() {
        let mut log = EventLog::default();
        log.push(
            1,
            EventKind::FoodLevel {
                current: 10.0,
                capacity: 50.0,
                fraction: 0.2,
            },
        );
        assert_eq!(log.entries.len(), 1);
        assert_eq!(log.total_pushed, 1);
    }

    #[test]
    fn push_trims_to_capacity() {
        let mut log = EventLog::default();
        log.capacity = 3;
        for i in 0..5u64 {
            log.push(
                i,
                EventKind::FoodLevel {
                    current: i as f32,
                    capacity: 50.0,
                    fraction: i as f32 / 50.0,
                },
            );
        }
        assert_eq!(log.entries.len(), 3);
        assert_eq!(log.entries[0].tick, 2);
        assert_eq!(log.total_pushed, 5);
    }

    #[test]
    fn courtship_drifted_increments_courtship_tally() {
        // Ticket 027 Bug 1: passive courtship-drift events must bump
        // `continuity_tallies.courtship` so the canary tracks drift even
        // before the MateWith path is unblocked by Bugs 2/3.
        let mut log = EventLog::default();
        assert_eq!(log.continuity_tallies.get("courtship").copied(), Some(0));
        log.push(
            42,
            EventKind::CourtshipDrifted {
                cat_a: "Fern".into(),
                cat_b: "Reed".into(),
            },
        );
        assert_eq!(log.continuity_tallies.get("courtship").copied(), Some(1));
        log.push(
            43,
            EventKind::CourtshipDrifted {
                cat_a: "Fern".into(),
                cat_b: "Reed".into(),
            },
        );
        assert_eq!(log.continuity_tallies.get("courtship").copied(), Some(2));
    }

    #[test]
    fn planning_failed_increments_per_disposition_tally() {
        // Ticket 091: silent `make_plan → Err` path now witnessed via
        // `EventKind::PlanningFailed`. The footer reads
        // `planning_failures_by_disposition`, keyed on the disposition that
        // failed to plan, so an investigator can answer "which DSE is
        // winning the IAUS contest but losing the planner contest" without
        // a focal trace.
        let mut log = EventLog::default();
        log.push(
            100,
            EventKind::PlanningFailed {
                cat: "Nettle".into(),
                disposition: "Foraging".into(),
                reason: PlanningFailureReason::NoApplicableActions,
                hunger: 0.9,
                energy: 0.4,
                temperature: 0.3,
                food_available: false,
                has_stored_food: false,
            },
        );
        log.push(
            101,
            EventKind::PlanningFailed {
                cat: "Nettle".into(),
                disposition: "Foraging".into(),
                reason: PlanningFailureReason::NoApplicableActions,
                hunger: 0.9,
                energy: 0.4,
                temperature: 0.3,
                food_available: false,
                has_stored_food: false,
            },
        );
        log.push(
            102,
            EventKind::PlanningFailed {
                cat: "Mocha".into(),
                disposition: "Hunting".into(),
                reason: PlanningFailureReason::GoalUnreachable,
                hunger: 0.95,
                energy: 0.5,
                temperature: 0.3,
                food_available: false,
                has_stored_food: false,
            },
        );
        assert_eq!(
            log.planning_failures_by_disposition
                .get("Foraging")
                .copied(),
            Some(2)
        );
        assert_eq!(
            log.planning_failures_by_disposition.get("Hunting").copied(),
            Some(1)
        );
    }

    #[test]
    fn planning_failed_increments_per_reason_tally_172() {
        // Ticket 172: `(disposition, reason)` composite key surfaces the
        // failure-cause histogram. Two Cooking failures with different
        // reasons split into distinct buckets; same-reason failures
        // accumulate.
        let mut log = EventLog::default();
        let push = |log: &mut EventLog, disposition: &str, reason: PlanningFailureReason| {
            log.push(
                0,
                EventKind::PlanningFailed {
                    cat: "Bramble".into(),
                    disposition: disposition.into(),
                    reason,
                    hunger: 0.5,
                    energy: 0.5,
                    temperature: 0.5,
                    food_available: false,
                    has_stored_food: false,
                },
            );
        };
        push(&mut log, "Cooking", PlanningFailureReason::NoApplicableActions);
        push(&mut log, "Cooking", PlanningFailureReason::NoApplicableActions);
        push(&mut log, "Cooking", PlanningFailureReason::GoalUnreachable);
        push(&mut log, "Herbalism", PlanningFailureReason::NodeBudgetExhausted);
        assert_eq!(
            log.planning_failures_by_reason
                .get("Cooking:NoApplicableActions")
                .copied(),
            Some(2)
        );
        assert_eq!(
            log.planning_failures_by_reason
                .get("Cooking:GoalUnreachable")
                .copied(),
            Some(1)
        );
        assert_eq!(
            log.planning_failures_by_reason
                .get("Herbalism:NodeBudgetExhausted")
                .copied(),
            Some(1)
        );
        // Per-disposition tally still aggregates across reasons.
        assert_eq!(
            log.planning_failures_by_disposition
                .get("Cooking")
                .copied(),
            Some(3)
        );
    }

    #[test]
    fn mating_and_courtship_drift_share_the_courtship_bucket() {
        // Ticket 027 Bug 1 piggybacks `CourtshipDrifted` onto the same
        // bucket as `MatingOccurred` per the §11.3 design pattern. A run
        // with both kinds of events should sum into a single tally.
        let mut log = EventLog::default();
        log.push(
            10,
            EventKind::MatingOccurred {
                partner_a: "Fern".into(),
                partner_b: "Reed".into(),
                location: (0, 0),
            },
        );
        log.push(
            11,
            EventKind::CourtshipDrifted {
                cat_a: "Fern".into(),
                cat_b: "Reed".into(),
            },
        );
        assert_eq!(log.continuity_tallies.get("courtship").copied(), Some(2));
    }
}
