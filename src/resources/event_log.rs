use std::collections::HashMap;
use std::collections::VecDeque;

use bevy_ecs::prelude::Resource;

use crate::ai::Action;
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
        positive: HashMap<String, u64>,
        negative: HashMap<String, u64>,
        neutral: HashMap<String, u64>,
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
    pub deaths_by_cause: HashMap<String, u64>,
    pub plan_failures_by_reason: HashMap<String, u64>,
    pub interrupts_by_reason: HashMap<String, u64>,
    /// Continuity-canary class counters. Six fixed keys: `grooming`,
    /// `play`, `mentoring`, `burial`, `courtship`, `mythic-texture`.
    /// Populated by `push()` from the corresponding canary event
    /// variants and from existing events that map to a canary class
    /// (MatingOccurred → courtship, ShadowFoxBanished → mythic-texture).
    /// Serialized into the headless footer for `just check-continuity`.
    pub continuity_tallies: HashMap<String, u64>,
}

impl Default for EventLog {
    fn default() -> Self {
        let mut continuity_tallies = HashMap::new();
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
            deaths_by_cause: HashMap::new(),
            plan_failures_by_reason: HashMap::new(),
            interrupts_by_reason: HashMap::new(),
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
}
