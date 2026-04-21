//! Focal-cat trace records — layer-by-layer observational surface for
//! the AI substrate refactor per §11 of `docs/systems/ai-substrate-refactor.md`.
//!
//! Headless-only emission. Systems that emit records gate on
//! `run_if(resource_exists::<FocalTraceTarget>)`. No interactive code path
//! sees the trace emitter. See §11.5.
//!
//! Shapes match §11.3 record sketches; the sidecar file
//! `logs/trace-<focal>.jsonl` is diff-joinable with `events.jsonl` via
//! the shared header (§11.4 joinability invariant).

use std::collections::VecDeque;

use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::Resource;

// ---------------------------------------------------------------------------
// FocalTraceTarget
// ---------------------------------------------------------------------------

/// Marker resource. When present, trace-emitter systems produce
/// layer-by-layer records for the named cat. Inserted only by the
/// headless runner (see `run_headless` in `src/main.rs`); never by
/// `SimulationPlugin`. Per §11.5 scope rule.
///
/// The target is identified by name at the CLI level; `entity` is
/// resolved lazily on the first tick the named cat is queryable.
/// Unresolved targets produce no records — the cat may not exist
/// yet (pre-birth), may have died, or the name may be typo'd.
#[derive(Resource, Debug, Clone)]
pub struct FocalTraceTarget {
    pub name: String,
    pub entity: Option<Entity>,
}

// ---------------------------------------------------------------------------
// Shared sub-types — kept intentionally minimal at Phase 1 entry
// ---------------------------------------------------------------------------

/// Per-channel attenuation breakdown for L1 samples. Phase 2 wires
/// real values from the species × role × injury × environment pipeline
/// (§5.6.6). At Phase 1 the shim emits identity (1.0) for channels
/// the current scent/sensing code doesn't expose.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AttenuationBreakdown {
    pub species_sens: f32,
    pub role_mod: f32,
    pub injury_deficit: f32,
    pub env_mul: f32,
}

impl Default for AttenuationBreakdown {
    fn default() -> Self {
        Self {
            species_sens: 1.0,
            role_mod: 1.0,
            injury_deficit: 0.0,
            env_mul: 1.0,
        }
    }
}

/// One contributor row — "which emitter drove this sample value?" —
/// load-bearing per §11.3 ("without the breakdown, you see 'scent is
/// high' but not *which* fox drove it").
#[derive(Debug, Clone, serde::Serialize)]
pub struct ContributorRow {
    pub emitter: String,
    pub pos: (i32, i32),
    pub distance: i32,
    pub contribution: f32,
}

/// One consideration's contribution to an L2 DSE score. Fields mirror
/// §11.3 L2 record sketch; at Phase 1 entry the trait doesn't exist so
/// `curve` carries a descriptive string rather than a typed enum.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConsiderationContribution {
    pub name: String,
    pub input: f32,
    /// Textual description of the response curve (e.g. `"Logistic(8,0.75)"`,
    /// `"Linear"`). Phase 3 will replace with a typed `Curve` enum.
    pub curve: String,
    pub score: f32,
    pub weight: f32,
    /// Optional spatial reference — set when this consideration reads
    /// an L1 map. Phase 2 enriches with per-consideration top-contributor
    /// join keys.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spatial: Option<SpatialRef>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SpatialRef {
    pub map: String,
    pub best_target: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EligibilitySummary {
    pub markers_required: Vec<String>,
    pub passed: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CompositionSummary {
    /// `"WeightedSum"` / `"CompensatedProduct"` / `"Max"` (§3.1 modes;
    /// Phase 3 adds the enum). Phase 1 shim always emits `"WeightedSum"`
    /// since current scoring is additive.
    pub mode: String,
    pub raw: f32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ModifierApplication {
    pub name: String,
    /// Set on additive modifiers (Pride bonus, Independence solo boost).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta: Option<f32>,
    /// Set on multiplicative modifiers (Fox-territory suppression).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub multiplier: Option<f32>,
}

/// Phase 3a lands a typed `Intention` enum (§L2.10.4); Phase 1 shim
/// captures the subset the current code produces.
#[derive(Debug, Clone, serde::Serialize)]
pub struct IntentionSummary {
    /// `"Goal"` | `"Activity"` — §L2.10.5. Phase 1 shim emits `"Activity"`
    /// for today's DispositionKind-driven actions until the split lands.
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goal_state: Option<String>,
}

/// Schema slot reserved for §7.W.6 top-N losing-axis logging. Populated
/// in Phase 6 when the Fulfillment register lands; empty vector at Phase 1.
/// Narrative emitters bind to "narrow winning axis + active losing
/// counter-axis + valence drop" triples via this field.
#[derive(Debug, Clone, serde::Serialize)]
pub struct LosingAxisSlot {
    pub axis: String,
    pub score: f32,
    pub deficit: f32,
}

/// Schema slot reserved for §8.6 apophenia continuity canary: pairwise
/// behavioral distance across N sampled cats and same-cat autocorrelation
/// across K-day windows. Populated in Phase 6; `None` at Phase 1.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ApopheniaSummary {
    pub pairwise_distance_sample: f32,
    pub self_autocorrelation_k_days: Vec<f32>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SoftmaxSummary {
    pub temperature: f32,
    pub probabilities: Vec<f32>,
}

/// Per-§7 commitment layer. Phase 6 fills this with CommitmentStrategy +
/// persistence bonus; Phase 1 emits a best-effort shape with
/// `commitment_strength` mapping to today's patience bonus where relevant.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MomentumSummary {
    pub active_intention: Option<String>,
    pub commitment_strength: f32,
    pub margin_threshold: f32,
    pub preempted: bool,
}

// ---------------------------------------------------------------------------
// TraceRecord — L1 / L2 / L3 variants per §11.3
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "layer")]
pub enum TraceRecord {
    /// L1 — one record per (focal cat × map × sample). Emitted lazily
    /// as a side-effect of an L2 consideration that reads the map; no
    /// every-tick × every-map emission.
    L1 {
        map: String,
        faction: String,
        channel: String,
        pos: (i32, i32),
        base_sample: f32,
        attenuation: AttenuationBreakdown,
        perceived: f32,
        top_contributors: Vec<ContributorRow>,
    },
    /// L2 — one record per (focal cat × eligible DSE × tick).
    L2 {
        dse: String,
        eligibility: EligibilitySummary,
        considerations: Vec<ConsiderationContribution>,
        composition: CompositionSummary,
        maslow_pregate: f32,
        modifiers: Vec<ModifierApplication>,
        final_score: f32,
        intention: IntentionSummary,
        /// Schema slot for §7.W.6 axis-capture logging — empty at Phase 1.
        top_losing: Vec<LosingAxisSlot>,
    },
    /// L3 — one record per (focal cat × tick). Closes the curvature
    /// loop: what the cat saw → wanted → planned to get.
    L3 {
        ranked: Vec<(String, f32)>,
        softmax: SoftmaxSummary,
        momentum: MomentumSummary,
        chosen: String,
        intention: IntentionSummary,
        goap_plan: Vec<String>,
        /// Schema slot for §8.6 apophenia canary — `None` at Phase 1.
        #[serde(skip_serializing_if = "Option::is_none")]
        apophenia: Option<ApopheniaSummary>,
    },
}

// ---------------------------------------------------------------------------
// TraceEntry + TraceLog
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct TraceEntry {
    pub tick: u64,
    pub cat: String,
    #[serde(flatten)]
    pub record: TraceRecord,
}

/// In-memory buffer drained every tick by the headless runner's
/// `flush_trace_entries`. Follows the same `total_pushed` +
/// ring-buffer convention as `EventLog` so that flush is a single
/// forward-walk from `last_flushed` to `total_pushed`.
///
/// `capacity` is sized for one cat × ~30 DSEs × a handful of L1 samples
/// × L3 record per tick; flush-every-tick keeps live memory bounded.
#[derive(Resource, Debug)]
pub struct TraceLog {
    pub entries: VecDeque<TraceEntry>,
    pub capacity: usize,
    pub total_pushed: u64,
}

impl Default for TraceLog {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            capacity: 5000,
            total_pushed: 0,
        }
    }
}

impl TraceLog {
    pub fn push(&mut self, entry: TraceEntry) {
        self.entries.push_back(entry);
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

    fn make_l3() -> TraceRecord {
        TraceRecord::L3 {
            ranked: vec![("Hunt".into(), 0.72), ("Eat".into(), 0.68)],
            softmax: SoftmaxSummary {
                temperature: 0.15,
                probabilities: vec![0.58, 0.42],
            },
            momentum: MomentumSummary {
                active_intention: Some("Hunt".into()),
                commitment_strength: 0.6,
                margin_threshold: 0.09,
                preempted: false,
            },
            chosen: "Hunt".into(),
            intention: IntentionSummary {
                kind: "Goal".into(),
                target: Some("Mouse#42".into()),
                goal_state: Some("prey_caught".into()),
            },
            goap_plan: vec!["MoveToTile(15,10)".into(), "PouncePrey(Mouse#42)".into()],
            apophenia: None,
        }
    }

    #[test]
    fn push_counts_entries() {
        let mut log = TraceLog::default();
        log.push(TraceEntry {
            tick: 1,
            cat: "Simba".into(),
            record: make_l3(),
        });
        log.push(TraceEntry {
            tick: 2,
            cat: "Simba".into(),
            record: make_l3(),
        });
        assert_eq!(log.total_pushed, 2);
        assert_eq!(log.entries.len(), 2);
    }

    #[test]
    fn ring_buffer_evicts_old() {
        let mut log = TraceLog::default();
        log.capacity = 3;
        for i in 0..5u64 {
            log.push(TraceEntry {
                tick: i,
                cat: "Simba".into(),
                record: make_l3(),
            });
        }
        assert_eq!(log.total_pushed, 5);
        assert_eq!(log.entries.len(), 3);
        assert_eq!(log.entries[0].tick, 2);
    }

    #[test]
    fn l3_record_serializes_with_layer_tag() {
        let entry = TraceEntry {
            tick: 100,
            cat: "Simba".into(),
            record: make_l3(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"layer\":\"L3\""));
        assert!(json.contains("\"tick\":100"));
        assert!(json.contains("\"cat\":\"Simba\""));
        assert!(json.contains("\"chosen\":\"Hunt\""));
        // apophenia is None → field omitted
        assert!(!json.contains("apophenia"));
    }

    #[test]
    fn l1_record_serializes_with_attenuation() {
        let entry = TraceEntry {
            tick: 100,
            cat: "Simba".into(),
            record: TraceRecord::L1 {
                map: "fox_scent".into(),
                faction: "fox".into(),
                channel: "scent".into(),
                pos: (14, 9),
                base_sample: 0.42,
                attenuation: AttenuationBreakdown::default(),
                perceived: 0.42,
                top_contributors: vec![],
            },
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"layer\":\"L1\""));
        assert!(json.contains("\"species_sens\":1.0"));
    }
}
