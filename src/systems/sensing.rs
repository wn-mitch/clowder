//! Unified sensory model.
//!
//! Phase 1 scaffolding: data types, per-species profile table, and the
//! shared `detect(observer, target, env)` function that call sites across
//! the sim will migrate to in later phases. All environmental multipliers
//! are wired but inert (return 1.0) in this phase — structural refactor
//! only, no semantic change. See `docs/systems/sensory.md`.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::{Mutex, OnceLock};

use crate::components::physical::Position;
use crate::components::sensing::{SensoryModifier, SensorySignature, SensorySpecies};
use crate::resources::map::Terrain;
use crate::resources::time::DayPhase;
use crate::resources::weather::Weather;

// ---------------------------------------------------------------------------
// Falloff — how a channel's confidence decays with distance
// ---------------------------------------------------------------------------

/// Shape of the distance → confidence curve within a channel's effective
/// range. In Phase 1 this is structural only; all channels behave as
/// `Cliff` (full confidence inside range, zero outside) to preserve
/// existing binary-detection semantics. Later phases activate `Linear`
/// and `InverseSquare` once call sites are migrated and the balance pass
/// begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Falloff {
    Linear,
    InverseSquare,
    Cliff,
}

// ---------------------------------------------------------------------------
// Channel — a single sensory modality (sight, hearing, scent, or tremor)
// ---------------------------------------------------------------------------

/// One sensory modality's parameters for a given species.
///
/// `base_range = 0.0` disables this channel for the species — snakes
/// essentially don't see, hawks don't smell, cats don't tremor-sense.
/// `acuity` is the signal-strength threshold a target must clear to
/// register (lower = sharper). `falloff` is the distance decay shape.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Channel {
    pub base_range: f32,
    pub acuity: f32,
    pub falloff: Falloff,
}

impl Channel {
    pub const DISABLED: Self = Self {
        base_range: 0.0,
        acuity: 1.0,
        falloff: Falloff::Cliff,
    };

    pub const fn new(base_range: f32, acuity: f32, falloff: Falloff) -> Self {
        Self {
            base_range,
            acuity,
            falloff,
        }
    }

    /// Is this channel active (non-zero range)?
    pub fn is_active(&self) -> bool {
        self.base_range > 0.0
    }
}

// ---------------------------------------------------------------------------
// SensoryProfile — per-species sensory loadout
// ---------------------------------------------------------------------------

/// The four sensory channels that describe a species' perceptual capacity.
///
/// Stored in `SimConstants` keyed by `SensorySpecies`. Serialized into the
/// constants-hash header so two headless runs are comparable only if
/// their profiles match.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SensoryProfile {
    pub sight: Channel,
    pub hearing: Channel,
    pub scent: Channel,
    pub tremor: Channel,
    /// Whether scent detection requires the observer to be downwind of
    /// the target. True for most mammals; false for insects / magical
    /// creatures. Phase 1: read but not yet applied (wind modulation
    /// lights up in Phase 3).
    pub scent_directional: bool,
}

// ---------------------------------------------------------------------------
// ChannelKind — identifier for query / narrative use
// ---------------------------------------------------------------------------

/// Narrative-oriented identifier for which channel produced a detection.
/// Used by `SensoryResult::strongest_channel()` so cats can say "smelled
/// but didn't see" in log output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ChannelKind {
    Sight,
    Hearing,
    Scent,
    Tremor,
}

// ---------------------------------------------------------------------------
// SensoryResult — the output of a detect() call
// ---------------------------------------------------------------------------

/// Per-channel detection confidence in [0.0, 1.0].
///
/// Call sites wanting a simple boolean can use `.detected()`. Utility-AI
/// scoring multiplies `.best()` into a weight. Narrative sites inspect
/// `.strongest_channel()` to describe *how* a target was detected.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SensoryResult {
    pub sight: f32,
    pub hearing: f32,
    pub scent: f32,
    pub tremor: f32,
}

impl SensoryResult {
    /// Was anything detected on any channel?
    pub fn detected(&self) -> bool {
        self.sight > 0.0 || self.hearing > 0.0 || self.scent > 0.0 || self.tremor > 0.0
    }

    /// Strongest channel confidence across all four.
    pub fn best(&self) -> f32 {
        self.sight
            .max(self.hearing)
            .max(self.scent)
            .max(self.tremor)
    }

    /// Which channel carried the strongest signal (for narrative output).
    /// Returns `None` when nothing was detected.
    pub fn strongest_channel(&self) -> Option<ChannelKind> {
        if !self.detected() {
            return None;
        }
        let best = self.best();
        if self.sight == best {
            Some(ChannelKind::Sight)
        } else if self.hearing == best {
            Some(ChannelKind::Hearing)
        } else if self.scent == best {
            Some(ChannelKind::Scent)
        } else {
            Some(ChannelKind::Tremor)
        }
    }
}

// ---------------------------------------------------------------------------
// Context structs — observer / target / environment
// ---------------------------------------------------------------------------

/// Observer-side context for a detection check.
#[derive(Debug, Clone, Copy)]
pub struct ObserverCtx<'a> {
    pub position: Position,
    pub species: SensorySpecies,
    pub profile: &'a SensoryProfile,
    pub modifier: Option<&'a SensoryModifier>,
}

/// Target-side context for a detection check. `current_action_tremor_mul`
/// is the action-based multiplier on the target's tremor signature
/// (stalking ~0.2, walking 1.0, running 1.8, fighting 1.5). Callers
/// supply it from the target's current action state; a motionless target
/// passes 0.0.
#[derive(Debug, Clone, Copy)]
pub struct TargetCtx {
    pub position: Position,
    pub signature: SensorySignature,
    pub current_action_tremor_mul: f32,
}

/// Environmental modulation. In Phase 1 all multipliers are 1.0; they
/// plug into weather/phase/terrain sources in later phases. The optional
/// `max_range_override` lets migrating call sites preserve their
/// existing per-site range constant during the structural refactor.
#[derive(Debug, Clone, Copy)]
pub struct EnvCtx {
    pub sight_mul: f32,
    pub hearing_mul: f32,
    pub scent_mul: f32,
    pub tremor_mul: f32,
    pub max_range_override: Option<f32>,
}

impl Default for EnvCtx {
    fn default() -> Self {
        Self::identity()
    }
}

impl EnvCtx {
    /// Identity environment — all multipliers at 1.0. Phase 1's default;
    /// later phases construct this from weather/phase/terrain sources.
    pub const fn identity() -> Self {
        Self {
            sight_mul: 1.0,
            hearing_mul: 1.0,
            scent_mul: 1.0,
            tremor_mul: 1.0,
            max_range_override: None,
        }
    }

    /// Compose per-channel multipliers from the observer's current
    /// environment. Phase 1: all sources return 1.0 so this reduces to
    /// `identity()`, but the call pattern is stable — migrating sites
    /// should use this rather than hard-coding identity.
    pub fn from_environment(weather: Weather, phase: DayPhase, terrain: Terrain) -> Self {
        Self {
            sight_mul: weather.sight_multiplier() * phase.sight_multiplier(),
            hearing_mul: weather.hearing_multiplier() * phase.hearing_multiplier(),
            scent_mul: weather.scent_multiplier() * phase.scent_multiplier(),
            tremor_mul: weather.tremor_multiplier()
                * phase.tremor_multiplier()
                * terrain.tremor_transmission(),
            max_range_override: None,
        }
    }

    pub fn with_max_range(mut self, r: f32) -> Self {
        self.max_range_override = Some(r);
        self
    }
}

// ---------------------------------------------------------------------------
// detect() — the unified sensing entry point
// ---------------------------------------------------------------------------

/// Compute a per-channel detection result.
///
/// Phase 1 semantics: each active channel is a binary `dist <= range`
/// check (confidence 1.0 inside effective range, 0.0 outside) so that
/// migration preserves existing behavior exactly under identity
/// multipliers. Later phases activate `Falloff::Linear` /
/// `InverseSquare`, wind-modulated scent, LoS occlusion, and the
/// action-based tremor amplifier — each gated behind a verisimilitude
/// hypothesis per the Balance Methodology rule in `CLAUDE.md`.
pub fn detect(observer: ObserverCtx<'_>, target: TargetCtx, env: EnvCtx) -> SensoryResult {
    let dist = observer.position.manhattan_distance(&target.position) as f32;

    let sight_range = effective_range(
        observer.profile.sight,
        observer
            .modifier
            .map(|m| m.sight_range_bonus)
            .unwrap_or(0.0),
        env.sight_mul,
        env.max_range_override,
    );
    let hearing_range = effective_range(
        observer.profile.hearing,
        observer
            .modifier
            .map(|m| m.hearing_range_bonus)
            .unwrap_or(0.0),
        env.hearing_mul,
        env.max_range_override,
    );
    let scent_range = effective_range(
        observer.profile.scent,
        observer
            .modifier
            .map(|m| m.scent_range_bonus)
            .unwrap_or(0.0),
        env.scent_mul,
        env.max_range_override,
    );
    let tremor_range = effective_range(
        observer.profile.tremor,
        observer
            .modifier
            .map(|m| m.tremor_range_bonus)
            .unwrap_or(0.0),
        env.tremor_mul,
        env.max_range_override,
    );

    SensoryResult {
        sight: channel_confidence(
            dist,
            sight_range,
            target.signature.visual,
            observer.profile.sight.falloff,
        ),
        hearing: channel_confidence(
            dist,
            hearing_range,
            target.signature.auditory,
            observer.profile.hearing.falloff,
        ),
        scent: channel_confidence(
            dist,
            scent_range,
            target.signature.olfactory,
            observer.profile.scent.falloff,
        ),
        tremor: channel_confidence(
            dist,
            tremor_range,
            target.signature.tremor_baseline * target.current_action_tremor_mul,
            observer.profile.tremor.falloff,
        ),
    }
}

fn effective_range(channel: Channel, bonus: f32, env_mul: f32, override_: Option<f32>) -> f32 {
    if !channel.is_active() && override_.is_none() {
        return 0.0;
    }
    let base = override_.unwrap_or(channel.base_range);
    (base + bonus) * env_mul
}

fn channel_confidence(dist: f32, range: f32, signature: f32, falloff: Falloff) -> f32 {
    if range <= 0.0 || signature <= 0.0 {
        return 0.0;
    }
    match falloff {
        Falloff::Cliff => {
            if dist <= range {
                1.0
            } else {
                0.0
            }
        }
        // Linear falloff: confidence = 1 - dist/range, clamped to [0, 1].
        // Used by probabilistic channels (prey-detects-cat) where the
        // wrapper multiplies confidence by alertness/vigilance factors
        // and rolls against the product. At dist = 0 returns 1.0; at
        // dist = range returns 0.0; at dist > range returns 0.0.
        Falloff::Linear => {
            if dist < range {
                1.0 - dist / range
            } else {
                0.0
            }
        }
        // Inverse-square-ish: 1 / (1 + (dist/range)^2). Returns 1.0 at
        // dist = 0, 0.5 at dist = range, asymptotically 0. Reserved for
        // hearing / scent curves where a soft tail is realistic.
        Falloff::InverseSquare => {
            if range <= 0.0 {
                0.0
            } else {
                let ratio = dist / range;
                1.0 / (1.0 + ratio * ratio)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Migration helpers — thin wrappers for common call-site patterns
// ---------------------------------------------------------------------------

/// Does the observer's sight channel detect a target at this position?
///
/// Phase 3 migration helper for the cluster of legacy visual-range
/// constants (`herb_detection_range`, `prey_detection_range`,
/// `search_visual_detection_range`). Each call site passes its own
/// legacy range as `max_range` so behavior is byte-identical to the
/// pre-migration `dist <= range` check.
///
/// `target_signature` controls whether the target emits a visual
/// signal. Pass a signature with `visual > 0` for entities that can be
/// seen (herbs, prey, carcasses). Under identity multipliers with
/// `max_range = R` the result is exactly `dist <= R`.
/// Bresenham-walk line-of-sight check.
///
/// Returns true when no tile strictly between `from` and `to` returns
/// `Terrain::occludes_sight()` = true. The endpoints themselves are
/// excluded from the occlusion check so an observer or target on a
/// DenseForest tile isn't blocked from its own position.
///
/// Phase 5a introduction: used only by wildlife-as-observer sight
/// checks for now. Cat-side sight paths still run without LoS — a
/// follow-up pass can extend coverage once the wildlife-side
/// behavior shift is quantified.
pub fn line_of_sight_clear(
    from: Position,
    to: Position,
    map: &crate::resources::map::TileMap,
) -> bool {
    if from == to {
        return true;
    }
    // Standard Bresenham over integer grid.
    let mut x0 = from.x;
    let mut y0 = from.y;
    let x1 = to.x;
    let y1 = to.y;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        // Step first so we don't test the observer's own tile.
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
        // Stop when we've reached the target (don't test its tile).
        if x0 == x1 && y0 == y1 {
            return true;
        }
        if map.in_bounds(x0, y0) && map.get(x0, y0).terrain.occludes_sight() {
            return false;
        }
    }
}

/// Line-of-sight-aware sight check. Returns true iff the observer has
/// both range AND a clear Bresenham path to the target (no occluding
/// terrain strictly between them). Phase 5a wildlife call sites use
/// this; Phase 2-4 cat sites continue to use the non-LoS variant.
#[allow(clippy::too_many_arguments)]
pub fn observer_sees_at_with_los(
    observer_species: SensorySpecies,
    observer_pos: Position,
    observer_profile: &SensoryProfile,
    target_pos: Position,
    target_signature: SensorySignature,
    max_range: f32,
    map: &crate::resources::map::TileMap,
) -> bool {
    if !observer_sees_at(
        observer_species,
        observer_pos,
        observer_profile,
        target_pos,
        target_signature,
        max_range,
    ) {
        return false;
    }
    line_of_sight_clear(observer_pos, target_pos, map)
}

pub fn observer_sees_at(
    observer_species: SensorySpecies,
    observer_pos: Position,
    observer_profile: &SensoryProfile,
    target_pos: Position,
    target_signature: SensorySignature,
    max_range: f32,
) -> bool {
    detect(
        ObserverCtx {
            position: observer_pos,
            species: observer_species,
            profile: observer_profile,
            modifier: None,
        },
        TargetCtx {
            position: target_pos,
            signature: target_signature,
            current_action_tremor_mul: 1.0,
        },
        EnvCtx::identity().with_max_range(max_range),
    )
    .sight
        > 0.0
}

/// Does the observer's scent channel detect a target? Binary variant —
/// no wind or terrain modulation, just `dist <= max_range`. Used for
/// carcass smell and similar flat-range olfactory checks.
pub fn observer_smells_at(
    observer_species: SensorySpecies,
    observer_pos: Position,
    observer_profile: &SensoryProfile,
    target_pos: Position,
    target_signature: SensorySignature,
    max_range: f32,
) -> bool {
    detect(
        ObserverCtx {
            position: observer_pos,
            species: observer_species,
            profile: observer_profile,
            modifier: None,
        },
        TargetCtx {
            position: target_pos,
            signature: target_signature,
            current_action_tremor_mul: 1.0,
        },
        EnvCtx::identity().with_max_range(max_range),
    )
    .scent
        > 0.0
}

/// Wind-modulated prey-scent detection. Consolidates the two
/// byte-identical copies of `can_smell_prey` that used to live in
/// `disposition.rs` and `goap.rs`. Close-range (dist ≤ `min_range`)
/// bypasses wind entirely; beyond that, the cat must be downwind and
/// terrain at the prey's tile modulates effective range.
///
/// The formula matches the legacy code exactly so Phase 3 preserves
/// behavior. Phase 5b activation of weather multipliers will compose
/// on top of this (via `env.scent_mul`).
#[allow(clippy::too_many_arguments)]
pub fn cat_smells_prey_windaware(
    cat_pos: Position,
    prey_pos: Position,
    wind: &crate::resources::wind::WindState,
    map: &crate::resources::map::TileMap,
    min_range: f32,
    base_range: f32,
    downwind_dot_threshold: f32,
    dense_forest_modifier: f32,
    light_forest_modifier: f32,
) -> bool {
    let dist = cat_pos.manhattan_distance(&prey_pos) as f32;
    if dist == 0.0 {
        return true;
    }
    // Close-range olfaction: always detectable.
    if dist <= min_range {
        return true;
    }
    let dx = (prey_pos.x - cat_pos.x) as f32;
    let dy = (prey_pos.y - cat_pos.y) as f32;
    let (nx, ny) = (dx / dist, dy / dist);
    let (wx, wy) = wind.direction();
    let dot = wx * nx + wy * ny;
    if dot < downwind_dot_threshold {
        return false;
    }
    let terrain_mod = if map.in_bounds(prey_pos.x, prey_pos.y) {
        match map.get(prey_pos.x, prey_pos.y).terrain {
            crate::resources::map::Terrain::DenseForest => dense_forest_modifier,
            crate::resources::map::Terrain::LightForest => light_forest_modifier,
            _ => 1.0,
        }
    } else {
        1.0
    };
    let scent_range = base_range * wind.strength * terrain_mod;
    dist <= scent_range
}

/// Probabilistic prey-cat detection proximity factor.
///
/// Phase 4 migration helper for `src/systems/prey.rs::try_detect_cat`.
/// Returns a proximity confidence in [0, 1] routed through `detect()`
/// with the prey's sight channel (Linear falloff). Callers multiply the
/// returned value by alertness and vigilance factors and roll against
/// the product — the probabilistic Bernoulli gate stays outside the
/// sensory model.
///
/// **Algebraic equivalence:** under identity multipliers, returns
/// exactly `1 - dist/(alert_radius+1)` for `dist ∈ [1, alert_radius]`,
/// matching the pre-migration `proximity` formula. Returns 0 for
/// `dist == 0` or `dist > alert_radius` so the caller's Bernoulli roll
/// consumes no RNG on unreachable cats (preserving upstream RNG order).
pub fn prey_cat_proximity(
    prey_pos: Position,
    prey_kind: crate::components::prey::PreyKind,
    prey_profile: &SensoryProfile,
    cat_pos: Position,
    alert_radius: i32,
) -> f32 {
    let dist = prey_pos.manhattan_distance(&cat_pos);
    if dist > alert_radius || dist == 0 {
        return 0.0;
    }
    detect(
        ObserverCtx {
            position: prey_pos,
            species: SensorySpecies::Prey(prey_kind),
            profile: prey_profile,
            modifier: None,
        },
        TargetCtx {
            position: cat_pos,
            signature: SensorySignature::CAT,
            current_action_tremor_mul: 1.0,
        },
        EnvCtx::identity().with_max_range(alert_radius as f32 + 1.0),
    )
    .sight
}

/// Does this cat *see* a wildlife threat at `threat_pos`?
///
/// Migration helper for the shadowfox-threat-awareness path replacing
/// the old `dist <= d.threat_awareness_range` check at
/// `disposition.rs:212` and `goap.rs:427`. The original check was
/// explicitly visual — a cat reacting to a fox in line of sight — so
/// this helper reads only the sight channel, not the multi-channel
/// `detected()`. Scent-based threat awareness is a future Phase 5b
/// verisimilitude claim (a cat smelling a fox behind a wall), distinct
/// from this visual path.
///
/// Phase 1 profile default has `cat.sight.base_range = 10.0`, matching
/// the pre-migration `threat_awareness_range: 10` so behavior is
/// byte-identical under identity multipliers.
pub fn cat_sees_threat_at(
    cat_pos: Position,
    cat_profile: &SensoryProfile,
    threat_pos: Position,
) -> bool {
    detect(
        ObserverCtx {
            position: cat_pos,
            species: SensorySpecies::Cat,
            profile: cat_profile,
            modifier: None,
        },
        TargetCtx {
            position: threat_pos,
            signature: SensorySignature::WILDLIFE,
            current_action_tremor_mul: 1.0,
        },
        EnvCtx::identity(),
    )
    .sight
        > 0.0
}

// ---------------------------------------------------------------------------
// SENSING_TRACE — per-call logging for migration equivalence proofs
// ---------------------------------------------------------------------------
//
// Enabled when the `SENSING_TRACE` env var is set to a non-empty value
// other than "0". The file path defaults to `logs/sensing-trace.jsonl`
// and can be overridden with `SENSING_TRACE_PATH`.
//
// The trace is Phase 2+ migration infrastructure: a pre-migration run
// and a post-migration run with the same seed must produce
// byte-identical traces. Call sites opt in by invoking
// `trace_detect()` after their `detect()` call. Zero runtime cost when
// disabled (OnceLock keeps the inner Option at None).

fn trace_sink() -> &'static Mutex<Option<BufWriter<File>>> {
    static SINK: OnceLock<Mutex<Option<BufWriter<File>>>> = OnceLock::new();
    SINK.get_or_init(|| {
        let enabled = std::env::var("SENSING_TRACE")
            .ok()
            .is_some_and(|s| !s.is_empty() && s != "0");
        let writer = if enabled {
            let path = std::env::var("SENSING_TRACE_PATH")
                .unwrap_or_else(|_| "logs/sensing-trace.jsonl".to_string());
            File::create(&path).ok().map(BufWriter::new)
        } else {
            None
        };
        Mutex::new(writer)
    })
}

/// Emit a structured trace record for a `detect()` call. No-op when
/// SENSING_TRACE is unset. Call-site identity uses positions (not Entity
/// IDs, which aren't stable across runs).
pub fn trace_detect(
    tick: u64,
    observer_pos: Position,
    observer_species: SensorySpecies,
    target_pos: Position,
    result: &SensoryResult,
) {
    let mutex = trace_sink();
    let Ok(mut guard) = mutex.lock() else {
        return;
    };
    let Some(w) = guard.as_mut() else {
        return;
    };
    // Manual JSON formatting: avoids allocating per call and produces a
    // stable key order for deterministic diffs.
    let _ = writeln!(
        w,
        r#"{{"tick":{},"o":[{},{}],"os":"{}","t":[{},{}],"r":[{},{},{},{}]}}"#,
        tick,
        observer_pos.x,
        observer_pos.y,
        species_tag(observer_species),
        target_pos.x,
        target_pos.y,
        result.sight,
        result.hearing,
        result.scent,
        result.tremor,
    );
}

fn species_tag(s: SensorySpecies) -> &'static str {
    use crate::components::prey::PreyKind;
    use crate::components::wildlife::WildSpecies;
    match s {
        SensorySpecies::Cat => "cat",
        SensorySpecies::Wild(WildSpecies::Fox) => "fox",
        SensorySpecies::Wild(WildSpecies::Hawk) => "hawk",
        SensorySpecies::Wild(WildSpecies::Snake) => "snake",
        SensorySpecies::Wild(WildSpecies::ShadowFox) => "shadowfox",
        SensorySpecies::Prey(PreyKind::Mouse) => "mouse",
        SensorySpecies::Prey(PreyKind::Rat) => "rat",
        SensorySpecies::Prey(PreyKind::Rabbit) => "rabbit",
        SensorySpecies::Prey(PreyKind::Fish) => "fish",
        SensorySpecies::Prey(PreyKind::Bird) => "bird",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::prey::PreyKind;
    use crate::components::wildlife::WildSpecies;

    fn cat_profile() -> SensoryProfile {
        SensoryProfile {
            sight: Channel::new(10.0, 0.5, Falloff::Cliff),
            hearing: Channel::new(8.0, 0.5, Falloff::Cliff),
            scent: Channel::new(15.0, 0.5, Falloff::Cliff),
            tremor: Channel::DISABLED,
            scent_directional: true,
        }
    }

    fn rabbit_profile() -> SensoryProfile {
        SensoryProfile {
            sight: Channel::new(6.0, 0.5, Falloff::Cliff),
            hearing: Channel::new(10.0, 0.5, Falloff::Cliff),
            scent: Channel::new(4.0, 0.5, Falloff::Cliff),
            tremor: Channel::new(12.0, 0.5, Falloff::Cliff),
            scent_directional: false,
        }
    }

    #[test]
    fn detect_inside_sight_range_returns_confidence() {
        let profile = cat_profile();
        let observer = ObserverCtx {
            position: Position::new(0, 0),
            species: SensorySpecies::Cat,
            profile: &profile,
            modifier: None,
        };
        let target = TargetCtx {
            position: Position::new(5, 0),
            signature: SensorySignature::WILDLIFE,
            current_action_tremor_mul: 1.0,
        };
        let result = detect(observer, target, EnvCtx::identity());
        assert!(result.detected());
        assert_eq!(result.sight, 1.0);
    }

    #[test]
    fn detect_at_exact_boundary_still_detects() {
        // Critical equivalence: old `dist <= range` must still return true
        // at the boundary. The refactor must preserve this.
        let profile = cat_profile();
        let observer = ObserverCtx {
            position: Position::new(0, 0),
            species: SensorySpecies::Cat,
            profile: &profile,
            modifier: None,
        };
        let target = TargetCtx {
            position: Position::new(10, 0), // exactly at sight.base_range
            signature: SensorySignature::WILDLIFE,
            current_action_tremor_mul: 1.0,
        };
        let result = detect(observer, target, EnvCtx::identity());
        assert!(result.detected(), "boundary (dist == range) must detect");
    }

    #[test]
    fn detect_beyond_range_yields_zero() {
        let profile = cat_profile();
        let observer = ObserverCtx {
            position: Position::new(0, 0),
            species: SensorySpecies::Cat,
            profile: &profile,
            modifier: None,
        };
        let target = TargetCtx {
            position: Position::new(20, 0),
            signature: SensorySignature::WILDLIFE,
            current_action_tremor_mul: 1.0,
        };
        let result = detect(observer, target, EnvCtx::identity());
        assert!(!result.detected());
        assert_eq!(result.sight, 0.0);
        assert_eq!(result.scent, 0.0);
    }

    #[test]
    fn disabled_channel_never_detects() {
        let profile = cat_profile(); // tremor disabled for cats
        let observer = ObserverCtx {
            position: Position::new(0, 0),
            species: SensorySpecies::Cat,
            profile: &profile,
            modifier: None,
        };
        let target = TargetCtx {
            position: Position::new(1, 0),
            signature: SensorySignature::CAT,
            current_action_tremor_mul: 1.8, // running cat, full emission
        };
        let result = detect(observer, target, EnvCtx::identity());
        assert_eq!(result.tremor, 0.0, "cats have no tremor channel");
    }

    #[test]
    fn rabbit_feels_tremor_from_running_cat() {
        // Verisimilitude check: rabbit's tremor channel picks up a running
        // cat through the ground even if the rabbit's sight range is
        // shorter than the distance.
        let profile = rabbit_profile();
        let observer = ObserverCtx {
            position: Position::new(0, 0),
            species: SensorySpecies::Prey(PreyKind::Rabbit),
            profile: &profile,
            modifier: None,
        };
        let target = TargetCtx {
            // beyond sight (6) and hearing (10) but within tremor (12)
            position: Position::new(11, 0),
            signature: SensorySignature::CAT,
            current_action_tremor_mul: 1.8,
        };
        let result = detect(observer, target, EnvCtx::identity());
        assert_eq!(result.sight, 0.0, "out of sight range");
        assert_eq!(result.hearing, 0.0, "out of hearing range");
        assert!(result.tremor > 0.0, "tremor channel should catch it");
        assert_eq!(result.strongest_channel(), Some(ChannelKind::Tremor));
    }

    #[test]
    fn stalking_cat_hides_tremor_from_rabbit() {
        // The stalking mechanic gains teeth: a stalking cat emits much
        // less tremor than a walking one and slips under the threshold.
        let profile = rabbit_profile();
        let observer = ObserverCtx {
            position: Position::new(0, 0),
            species: SensorySpecies::Prey(PreyKind::Rabbit),
            profile: &profile,
            modifier: None,
        };
        // Signature computed as baseline * action_multiplier. Set baseline
        // and multiplier such that the effective signature is zero (perfect
        // stalk) and confirm tremor drops out.
        let target = TargetCtx {
            position: Position::new(10, 0),
            signature: SensorySignature::CAT,
            current_action_tremor_mul: 0.0, // motionless
        };
        let result = detect(observer, target, EnvCtx::identity());
        assert_eq!(result.tremor, 0.0);
    }

    #[test]
    fn max_range_override_preserves_call_site_behavior() {
        // During migration, each call site passes its existing constant
        // as max_range_override so behavior is byte-identical.
        let profile = cat_profile(); // sight.base_range = 10
        let observer = ObserverCtx {
            position: Position::new(0, 0),
            species: SensorySpecies::Cat,
            profile: &profile,
            modifier: None,
        };
        let target = TargetCtx {
            position: Position::new(12, 0),
            signature: SensorySignature::WILDLIFE,
            current_action_tremor_mul: 1.0,
        };
        // Profile range is 10, but call site overrides to 15.
        let result = detect(observer, target, EnvCtx::identity().with_max_range(15.0));
        assert_eq!(result.sight, 1.0, "override extends effective range");
    }

    #[test]
    fn sensory_result_defaults_to_undetected() {
        let r = SensoryResult::default();
        assert!(!r.detected());
        assert_eq!(r.best(), 0.0);
        assert_eq!(r.strongest_channel(), None);
    }

    #[test]
    fn sensory_species_covers_all_variants() {
        // Ensures the enum stays exhaustive; if a new WildSpecies or
        // PreyKind is added, this test compiles the reminder that a
        // sensory profile must be defined for it.
        let _cat = SensorySpecies::Cat;
        let _fox = SensorySpecies::Wild(WildSpecies::Fox);
        let _hawk = SensorySpecies::Wild(WildSpecies::Hawk);
        let _snake = SensorySpecies::Wild(WildSpecies::Snake);
        let _sfox = SensorySpecies::Wild(WildSpecies::ShadowFox);
        let _mouse = SensorySpecies::Prey(PreyKind::Mouse);
        let _rat = SensorySpecies::Prey(PreyKind::Rat);
        let _rabbit = SensorySpecies::Prey(PreyKind::Rabbit);
        let _fish = SensorySpecies::Prey(PreyKind::Fish);
        let _bird = SensorySpecies::Prey(PreyKind::Bird);
    }

    #[test]
    fn env_multipliers_match_activation_schedule() {
        // Per-activation canary. For every (weather × phase × terrain) combo,
        // multiplier values equal the product of the channel contributions.
        // Activations ship one at a time with a verisimilitude hypothesis per
        // the Balance Methodology rule in CLAUDE.md:
        //
        //   Activation 1 (2026-04-18): Weather::Fog sight = 0.4
        //
        // Everything else is still identity 1.0. When a new activation
        // lands, add its (condition → channel → value) line and update the
        // expected product below — never silently delete this test.
        use crate::resources::map::Terrain;
        use crate::resources::time::DayPhase;
        use crate::resources::weather::Weather;

        fn expected_sight(_w: Weather) -> f32 {
            1.0
        }

        let weathers = [
            Weather::Clear,
            Weather::Overcast,
            Weather::LightRain,
            Weather::HeavyRain,
            Weather::Snow,
            Weather::Fog,
            Weather::Wind,
            Weather::Storm,
        ];
        let phases = [
            DayPhase::Dawn,
            DayPhase::Day,
            DayPhase::Dusk,
            DayPhase::Night,
        ];
        let terrains = [
            Terrain::Grass,
            Terrain::DenseForest,
            Terrain::Rock,
            Terrain::Water,
        ];

        for weather in weathers {
            for phase in phases {
                for terrain in terrains {
                    let env = EnvCtx::from_environment(weather, phase, terrain);
                    let ctx = format!("{weather:?} / {phase:?} / {terrain:?}");
                    assert_eq!(env.sight_mul, expected_sight(weather), "sight @ {ctx}");
                    assert_eq!(env.hearing_mul, 1.0, "hearing @ {ctx}");
                    assert_eq!(env.scent_mul, 1.0, "scent @ {ctx}");
                    assert_eq!(env.tremor_mul, 1.0, "tremor @ {ctx}");
                }
            }
        }
    }

    #[test]
    fn line_of_sight_clear_on_empty_grass() {
        use crate::resources::map::{Terrain, TileMap};
        let map = TileMap::new(20, 20, Terrain::Grass);
        assert!(line_of_sight_clear(
            Position::new(0, 0),
            Position::new(10, 7),
            &map
        ));
    }

    #[test]
    fn dense_forest_blocks_line_of_sight() {
        use crate::resources::map::{Terrain, TileMap};
        let mut map = TileMap::new(20, 20, Terrain::Grass);
        // Put a DenseForest tile directly on the line between (0,0) and (10,0).
        map.set(5, 0, Terrain::DenseForest);
        assert!(!line_of_sight_clear(
            Position::new(0, 0),
            Position::new(10, 0),
            &map
        ));
    }

    #[test]
    fn los_ignores_observer_and_target_own_tiles() {
        // An observer ON a DenseForest tile isn't blocked from itself.
        use crate::resources::map::{Terrain, TileMap};
        let mut map = TileMap::new(20, 20, Terrain::Grass);
        map.set(0, 0, Terrain::DenseForest);
        map.set(10, 0, Terrain::DenseForest);
        // Intermediate tiles are all Grass → LoS must be clear.
        assert!(line_of_sight_clear(
            Position::new(0, 0),
            Position::new(10, 0),
            &map
        ));
    }

    #[test]
    fn wall_blocks_line_of_sight() {
        use crate::resources::map::{Terrain, TileMap};
        let mut map = TileMap::new(20, 20, Terrain::Grass);
        map.set(4, 4, Terrain::Wall);
        assert!(!line_of_sight_clear(
            Position::new(0, 0),
            Position::new(8, 8),
            &map
        ));
    }

    #[test]
    fn observer_sees_at_with_los_blocks_behind_forest() {
        use crate::components::prey::PreyKind;
        use crate::resources::map::{Terrain, TileMap};
        use crate::resources::sim_constants::SensoryConstants;
        let sensory = SensoryConstants::default();
        let fox_profile = &sensory.fox;
        let _ = PreyKind::Mouse; // touches the import path
        let mut map = TileMap::new(20, 20, Terrain::Grass);
        map.set(3, 0, Terrain::DenseForest);
        // Within range but behind forest occluder.
        let sees = observer_sees_at_with_los(
            SensorySpecies::Wild(crate::components::wildlife::WildSpecies::Fox),
            Position::new(0, 0),
            fox_profile,
            Position::new(6, 0),
            SensorySignature::CAT,
            8.0,
            &map,
        );
        assert!(!sees, "fox should lose LoS on cat behind DenseForest");
    }

    #[test]
    fn observer_sees_at_matches_old_manhattan_check() {
        // Phase 3 sight-channel migrations (herb, prey, search-visual)
        // all use `dist <= range` in the pre-migration code. Under
        // identity multipliers the helper must return the same boolean.
        // Test every (legacy_range, dx, dy) combination up to 2× range.
        use crate::resources::sim_constants::SensoryConstants;
        let sensory = SensoryConstants::default();
        let cat_profile = &sensory.cat;
        let center = Position::new(0, 0);
        for range in [10, 15] {
            // prey_detection_range, herb/search_visual_detection_range
            for dx in -(range * 2)..=(range * 2) {
                for dy in -(range * 2)..=(range * 2) {
                    let target = Position::new(dx, dy);
                    let dist = center.manhattan_distance(&target);
                    let old_detected = dist <= range;
                    let new_detected = observer_sees_at(
                        SensorySpecies::Cat,
                        center,
                        cat_profile,
                        target,
                        SensorySignature::PREY,
                        range as f32,
                    );
                    assert_eq!(
                        old_detected, new_detected,
                        "range={range} dx={dx} dy={dy} dist={dist}"
                    );
                }
            }
        }
    }

    #[test]
    fn observer_smells_at_matches_old_manhattan_check() {
        // Phase 3 scent-binary migration (carcass smell) uses the same
        // binary `dist <= range` in pre-migration code.
        use crate::resources::sim_constants::SensoryConstants;
        let sensory = SensoryConstants::default();
        let cat_profile = &sensory.cat;
        let center = Position::new(0, 0);
        let range: i32 = 15; // carcass_detection_range
        for dx in -(range * 2)..=(range * 2) {
            for dy in -(range * 2)..=(range * 2) {
                let target = Position::new(dx, dy);
                let dist = center.manhattan_distance(&target);
                let old_detected = dist <= range;
                let new_detected = observer_smells_at(
                    SensorySpecies::Cat,
                    center,
                    cat_profile,
                    target,
                    SensorySignature::CARCASS,
                    range as f32,
                );
                assert_eq!(
                    old_detected, new_detected,
                    "dx={dx} dy={dy} dist={dist}"
                );
            }
        }
    }

    #[test]
    fn prey_cat_proximity_matches_old_formula_pointwise() {
        // Phase 4 probabilistic migration: `1 - dist/(alert_radius+1)`
        // for dist ∈ [1, alert_radius], zero elsewhere. Must match
        // pointwise at every (dist, alert_radius) lattice point so the
        // Bernoulli roll produces the same probability distribution.
        use crate::components::prey::PreyKind;
        use crate::resources::sim_constants::SensoryConstants;
        let sensory = SensoryConstants::default();
        // Use each prey profile at least once so the Linear-falloff
        // path is exercised with real data.
        let prey_cases = [
            (PreyKind::Mouse, &sensory.mouse),
            (PreyKind::Rat, &sensory.rat),
            (PreyKind::Rabbit, &sensory.rabbit),
            (PreyKind::Fish, &sensory.fish),
            (PreyKind::Bird, &sensory.bird),
        ];
        let prey_pos = Position::new(0, 0);
        // Test a range of alert_radius values covering all species.
        for alert_radius in 1..=10 {
            for (kind, profile) in &prey_cases {
                for dx in -(alert_radius * 2)..=(alert_radius * 2) {
                    for dy in -(alert_radius * 2)..=(alert_radius * 2) {
                        let cat_pos = Position::new(dx, dy);
                        let dist = prey_pos.manhattan_distance(&cat_pos);
                        let old_proximity = if dist > alert_radius || dist == 0 {
                            0.0
                        } else {
                            1.0 - (dist as f32 / (alert_radius as f32 + 1.0))
                        };
                        let new_proximity = prey_cat_proximity(
                            prey_pos,
                            *kind,
                            profile,
                            cat_pos,
                            alert_radius,
                        );
                        let diff = (old_proximity - new_proximity).abs();
                        assert!(
                            diff < 1e-6,
                            "mismatch kind={kind:?} r={alert_radius} dx={dx} dy={dy} \
                             dist={dist} old={old_proximity} new={new_proximity}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn cat_sees_threat_matches_old_manhattan_check() {
        // Exhaustive equivalence proof for the Phase 2 migration:
        // old code computed `pos.manhattan_distance(wp) <= d.threat_awareness_range`
        // with threat_awareness_range = 10. The new path uses the cat's
        // sight channel only (not multi-channel detect()) because the
        // original call was explicitly visual. Every tile in a 2×-range
        // neighborhood is tested so every boundary case is covered.
        use crate::resources::sim_constants::SensoryConstants;
        let sensory = SensoryConstants::default();
        let cat_profile = &sensory.cat;
        let old_range: i32 = 10; // legacy threat_awareness_range default
        let center = Position::new(0, 0);
        for dx in -(old_range * 2)..=(old_range * 2) {
            for dy in -(old_range * 2)..=(old_range * 2) {
                let wp = Position::new(dx, dy);
                let dist = center.manhattan_distance(&wp);
                let old_detected = dist <= old_range;
                let new_detected = cat_sees_threat_at(center, cat_profile, wp);
                assert_eq!(
                    old_detected, new_detected,
                    "mismatch at (dx={dx}, dy={dy}, dist={dist}): \
                     old_detected={old_detected}, new_detected={new_detected}"
                );
            }
        }
    }

    #[test]
    fn trace_detect_is_safe_when_disabled() {
        // SENSING_TRACE unset in the default test environment → sink is
        // None → call must be a no-op without panicking.
        let result = SensoryResult {
            sight: 1.0,
            hearing: 0.0,
            scent: 0.5,
            tremor: 0.0,
        };
        trace_detect(
            42,
            Position::new(1, 2),
            SensorySpecies::Cat,
            Position::new(3, 4),
            &result,
        );
        // If we got here, it didn't panic.
    }

    #[test]
    fn modifier_combine_is_additive() {
        let a = SensoryModifier {
            sight_range_bonus: 2.0,
            hearing_acuity_bonus: 0.1,
            ..Default::default()
        };
        let b = SensoryModifier {
            sight_range_bonus: 1.0,
            scent_range_bonus: 3.0,
            ..Default::default()
        };
        let c = a.combine(b);
        assert_eq!(c.sight_range_bonus, 3.0);
        assert_eq!(c.hearing_acuity_bonus, 0.1);
        assert_eq!(c.scent_range_bonus, 3.0);
    }
}
