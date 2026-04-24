use std::collections::HashMap;

use bevy::prelude::Entity;
use rand::Rng;

use crate::ai::dse::EvalCtx;
use crate::ai::eval::{evaluate_single, DseRegistry, ModifierPipeline};
use crate::ai::Action;
use crate::components::disposition::CraftingHint;
use crate::components::mental::{Memory, MemoryType};
use crate::components::personality::Personality;
use crate::components::physical::{Needs, Position};
use crate::resources::sim_constants::ScoringConstants;
use crate::resources::time::DayPhase;

// ---------------------------------------------------------------------------
// EvalInputs — evaluator identity / registry bundle
// ---------------------------------------------------------------------------

/// Per-cat evaluator identity and registry refs. Passed alongside
/// `ScoringContext` so `score_actions` can dispatch ported DSEs
/// through the L2 evaluator per §L2.10.2.
///
/// Phase 3c.0 lands this bundle; Phase 3c.N ports each remaining DSE
/// into the registry and deletes its inline block in `score_actions`.
/// The consideration scalar inputs are pulled via [`ctx_scalars`] —
/// centralizing the semantic inversion (spec's "hunger" = urgency =
/// `1 - needs.hunger`).
pub struct EvalInputs<'a> {
    pub cat: Entity,
    pub position: Position,
    pub tick: u64,
    pub dse_registry: &'a DseRegistry,
    pub modifier_pipeline: &'a ModifierPipeline,
    /// §4 marker lookup. Replaces the stub `|_, _| false` closure
    /// in `score_dse_by_id` so `EligibilityFilter::require(marker)`
    /// rows start resolving. Phase 4b.2 MVP: populated with the
    /// colony-scoped subset at system start (caller iterates
    /// resources/queries, dumps flags into the snapshot). Per-cat
    /// marker support extends the snapshot when per-cat authoring
    /// systems land.
    pub markers: &'a MarkerSnapshot,
    /// §11 focal-cat entity, when active. The caller resolves the
    /// focal name against the live cat roster and populates this once
    /// per tick; `score_dse_by_id` compares it against `cat` to gate
    /// trace capture. `None` on every non-focal scoring call and on
    /// every interactive build.
    pub focal_cat: Option<Entity>,
    /// §11 rich-trace sink. When `Some` and `focal_cat == Some(cat)`,
    /// `score_dse_by_id` routes through `evaluate_single_with_trace` and
    /// pushes the rich `CapturedDse` into this resource's inner mutex.
    /// Kept as a `&'a FocalScoreCapture` (with interior `Mutex`) rather
    /// than `&'a mut FocalScoreCapture` so `EvalInputs` can be passed by
    /// shared reference through the existing 30+ `score_dse_by_id` call
    /// sites without threading mutable borrows.
    pub focal_capture: Option<&'a crate::resources::FocalScoreCapture>,
}

// ---------------------------------------------------------------------------
// MarkerSnapshot — §4 marker lookup surface
// ---------------------------------------------------------------------------

/// Per-tick snapshot of §4 marker presence. Built by the caller
/// (`goap.rs`, `disposition.rs`) at system start from live ECS queries
/// and/or resources, then passed by reference into the evaluator so
/// `EligibilityFilter::require(name)` rows resolve without each DSE
/// carrying its own query bundle.
///
/// Colony-scoped markers are stored by name alone — the snapshot's
/// `has(name, _)` lookup ignores the entity parameter for keys in
/// `colony_markers`, so any cat passes the eligibility check when
/// the colony-scoped flag is set. Per-cat markers (when added) live
/// in a separate `entity_markers: HashSet<(&'static str, Entity)>`.
///
/// MVP note — the canonical spec shape (§4.3) attaches colony-scoped
/// markers as ZST components on a dedicated `ColonyState` singleton
/// entity, queried via `Q<With<ColonyState>, With<MarkerN>>`. This
/// snapshot is a lookup-shim that produces the same answer without
/// requiring the singleton to land first. When the singleton arrives,
/// the population logic in each caller shifts from "read Resource"
/// to "read ColonyState components" — the evaluator-side surface
/// stays identical.
#[derive(Default, Debug)]
pub struct MarkerSnapshot {
    colony_markers: std::collections::HashSet<&'static str>,
    entity_markers: std::collections::HashMap<&'static str, std::collections::HashSet<Entity>>,
}

impl MarkerSnapshot {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a colony-scoped marker. Any cat passes `has(name, _)` when
    /// this is set.
    pub fn set_colony(&mut self, name: &'static str, present: bool) {
        if present {
            self.colony_markers.insert(name);
        } else {
            self.colony_markers.remove(name);
        }
    }

    /// Set a per-cat marker.
    pub fn set_entity(&mut self, name: &'static str, entity: Entity, present: bool) {
        let set = self.entity_markers.entry(name).or_default();
        if present {
            set.insert(entity);
        } else {
            set.remove(&entity);
        }
    }

    /// Eligibility check. True iff the name is in the colony set OR
    /// the (name, entity) pair is in the per-cat set.
    pub fn has(&self, name: &str, entity: Entity) -> bool {
        self.colony_markers.contains(name)
            || self
                .entity_markers
                .get(name)
                .is_some_and(|set| set.contains(&entity))
    }
}

// ---------------------------------------------------------------------------
// Jitter
// ---------------------------------------------------------------------------

/// Small random noise added to every score to break ties and add variety.
fn jitter(rng: &mut impl Rng, range: f32) -> f32 {
    rng.random_range(-range..range)
}

// ---------------------------------------------------------------------------
// ScoringContext
// ---------------------------------------------------------------------------

/// Everything the scoring function needs to evaluate available actions.
pub struct ScoringContext<'a> {
    pub scoring: &'a ScoringConstants,
    pub needs: &'a Needs,
    pub personality: &'a Personality,
    pub food_available: bool,
    pub can_hunt: bool,
    pub can_forage: bool,
    /// Whether there is at least one visible cat to interact with.
    pub has_social_target: bool,
    /// Whether a wildlife threat is within detection range.
    pub has_threat_nearby: bool,
    /// Number of ally cats already fighting the same threat.
    pub allies_fighting_threat: usize,
    /// Combat skill + hunting cross-training.
    pub combat_effective: f32,
    /// Cat's current health (0.0–1.0).
    pub health: f32,
    /// Whether the cat is incapacitated by a severe injury.
    pub is_incapacitated: bool,
    /// Whether a construction site exists that needs work.
    pub has_construction_site: bool,
    /// Whether a building has structural condition < 0.4 (needs repair).
    pub has_damaged_building: bool,
    /// Whether a garden exists for farming.
    pub has_garden: bool,
    /// Fraction of food capacity filled (0.0–1.0).
    pub food_fraction: f32,
    // --- Magic/herbcraft context ---
    /// Cat's innate magical aptitude.
    pub magic_affinity: f32,
    /// Cat's trained magic skill level.
    pub magic_skill: f32,
    /// Cat's herbcraft skill level.
    pub herbcraft_skill: f32,
    /// Whether harvestable herbs are within gathering range.
    pub has_herbs_nearby: bool,
    /// Whether the cat has herbs in inventory.
    pub has_herbs_in_inventory: bool,
    /// Whether the cat has remedy herbs (HealingMoss/Moonpetal/Calmroot).
    pub has_remedy_herbs: bool,
    /// Whether the cat has Thornbriar for ward-setting.
    pub has_ward_herbs: bool,
    /// Whether harvestable Thornbriar exists anywhere in the world.
    pub thornbriar_available: bool,
    /// Number of injured cats in the colony.
    pub colony_injury_count: usize,
    /// Whether colony ward coverage is low (no wards or average strength < 0.3).
    pub ward_strength_low: bool,
    /// Whether the cat is standing on a corrupted tile.
    pub on_corrupted_tile: bool,
    /// Corruption level of the cat's current tile.
    pub tile_corruption: f32,
    /// Max corruption level on any tile within `corruption_smell_range` of the
    /// cat's current position. Represents the cat "smelling" rot nearby — drives
    /// proactive Cleanse/SetWard response even when not standing on corruption.
    pub nearby_corruption_level: f32,
    /// Whether the cat is on a fairy ring or standing stone.
    pub on_special_terrain: bool,
    // --- Coordination context ---
    /// Whether this cat is a coordinator with pending directives to deliver.
    pub is_coordinator_with_directives: bool,
    /// Number of pending directives (0 if not a coordinator).
    pub pending_directive_count: usize,
    // --- Mentoring context ---
    /// Whether a valid mentoring target exists (cat with skill < 0.3 where
    /// this cat has the same skill > 0.6).
    pub has_mentoring_target: bool,
    /// Whether at least one prey animal is within hunting range.
    pub prey_nearby: bool,
    // --- Personality integration fields ---
    /// Physiological satisfaction (0.0–1.0) for temper modifiers.
    pub phys_satisfaction: f32,
    /// Cat's current respect need level (0.0–1.0) for pride modifiers.
    pub respect: f32,
    /// Whether this cat currently has an active disposition.
    pub has_active_disposition: bool,
    /// The current disposition kind, if any (for patience commitment bonus).
    pub active_disposition: Option<crate::components::disposition::DispositionKind>,
    /// Pre-computed tradition location bonus for the cat's current tile.
    /// Set to `tradition * 0.1` by the caller if the cat's current action
    /// matches a previously successful action at this tile, else 0.0.
    pub tradition_location_bonus: f32,
    // --- Reproduction context ---
    /// Whether an orientation-compatible partner with Partners+ bond exists.
    pub has_eligible_mate: bool,
    /// Urgency of nearby hungry kittens (0.0 if none).
    pub hungry_kitten_urgency: f32,
    /// Whether this cat is a parent of the hungriest nearby kitten.
    pub is_parent_of_hungry_kitten: bool,
    /// Phase 4c.4 alloparenting Reframe A: multiplier applied to the
    /// `personality.compassion` input when scoring the Caretake DSE.
    /// 1.0 = no boost (default); 1.25 = 25% boost for bonded friends
    /// of the kitten's mother; 2.0 = doubled at max fondness × max
    /// boost_max. See `caretake_compassion_bond_scale` +
    /// `sim_constants.caretake_bond_compassion_boost_max`. Kept as
    /// its own axis input (not shared with herbcraft_prepare's
    /// `compassion`) so the bond-weighting is caretake-local.
    pub caretake_compassion_bond_scale: f32,
    // --- Exploration context ---
    /// Fraction of tiles within explore_range that are unexplored (0.0–1.0).
    /// Gates the explore action score: when nearby area is fully explored,
    /// explore becomes uninteresting.
    pub unexplored_nearby: f32,
    // --- Territorial context ---
    /// Fox scent intensity at the cat's current position (0.0–1.0).
    /// High values indicate deep fox territory.
    pub fox_scent_level: f32,
    // --- Corruption/carcass/siege context ---
    /// Whether uncleansed/unharvested carcasses are within detection range.
    pub carcass_nearby: bool,
    /// Count of nearby actionable carcasses (uncleansed or unharvested).
    pub nearby_carcass_count: usize,
    /// Max corruption of any tile in the territory ring around colony center (0.0–1.0).
    pub territory_max_corruption: f32,
    /// Whether any ward is currently being encircled by a shadow fox.
    pub wards_under_siege: bool,
    // --- Temporal context ---
    /// Current phase of the day. Drives species-specific activity bias —
    /// Sleep scoring alone in Phase 1, extends to hunting/foraging/denning as
    /// the initiative progresses. See `docs/systems/sleep-that-makes-sense.md`.
    pub day_phase: DayPhase,
    // --- Cooking context ---
    /// Whether at least one functional Kitchen exists in the colony.
    pub has_functional_kitchen: bool,
    /// Whether Stores currently holds at least one raw (uncooked) food item.
    pub has_raw_food_in_stores: bool,
    // --- §7.W Fulfillment context ---
    /// Social-warmth deficit (1.0 - social_warmth). Drives grooming and
    /// socializing urgency from the Fulfillment register, independent of
    /// the Maslow social need.
    pub social_warmth_deficit: f32,
}

// ---------------------------------------------------------------------------
// ScoringResult
// ---------------------------------------------------------------------------

/// Bundles action scores with metadata the chain builder needs.
/// `herbcraft_hint` carries which sub-mode won during herbcraft scoring,
/// so the chain builder doesn't re-derive it via its own priority cascade.
/// `magic_hint` is the equivalent for PracticeMagic — without it, the GOAP
/// planner falls back to A*'s cheapest action (Scry) and never picks
/// DurableWard even when its sub-score is highest.
pub struct ScoringResult {
    pub scores: Vec<(Action, f32)>,
    pub herbcraft_hint: Option<CraftingHint>,
    pub magic_hint: Option<CraftingHint>,
    /// True when the cat would have scored Cook competitively (had raw
    /// food, non-critical hunger, diligent personality) but no functional
    /// Kitchen exists. The caller turns this into a colony-wide
    /// `UnmetDemand::kitchen` tick so the coordinator's BuildPressure can
    /// respond to the latent desire.
    pub wants_cook_but_no_kitchen: bool,
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

/// Build the scalar-input map for L2 consideration dispatch. Each
/// entry maps a consideration's scalar name to the value the curve
/// should see. Critically, **needs are inverted** — the spec's §2.3
/// "hunger" axis is a deficit, so `needs.hunger = 0.1` (stomach 10%
/// full) maps to `"hunger_urgency" = 0.9`. The inversion lives here
/// rather than in each DSE's fetch_scalar so future ports share one
/// source of truth.
fn ctx_scalars(ctx: &ScoringContext) -> HashMap<&'static str, f32> {
    let mut m = HashMap::new();
    // Needs-as-urgency (deficit form).
    m.insert(
        "hunger_urgency",
        (1.0 - ctx.needs.hunger).clamp(0.0, 1.0),
    );
    // Food-stores scarcity (deficit fraction in `[0, 1]`).
    m.insert(
        "food_scarcity",
        (1.0 - ctx.food_fraction).clamp(0.0, 1.0),
    );
    // Safety deficit — Flee axis. Safety is a satisfaction scalar; the
    // DSE wants urgency form so the Logistic midpoint semantics line
    // up with `flee_safety_threshold`.
    m.insert("safety_deficit", (1.0 - ctx.needs.safety).clamp(0.0, 1.0));
    // Raw safety + health for Fight's piecewise-gated axes (shape
    // `fight_gating` already encodes the deficit semantics through
    // its knots).
    m.insert("safety", ctx.needs.safety.clamp(0.0, 1.0));
    m.insert("health", ctx.health.clamp(0.0, 1.0));
    // Energy/health deficits for Rest peer group. Sleep uses
    // `energy_deficit` through `sleep_dep()` Logistic and
    // `health_deficit` through injury-bonus Linear.
    m.insert(
        "energy_deficit",
        (1.0 - ctx.needs.energy).clamp(0.0, 1.0),
    );
    m.insert("health_deficit", (1.0 - ctx.health).clamp(0.0, 1.0));
    // Fight: combat_effective is already a `[0, 1]` composite index
    // upstream; flow through directly.
    m.insert(
        "combat_effective",
        ctx.combat_effective.clamp(0.0, 1.0),
    );
    // Fight: ally_count is raw count — the DSE's saturating-count
    // Composite handles normalization.
    m.insert("ally_count", ctx.allies_fighting_threat as f32);
    // Personality coefficients flow through directly as `[0, 1]`
    // inputs to each DSE's Linear identity curve.
    m.insert("boldness", ctx.personality.boldness.clamp(0.0, 1.0));
    m.insert("diligence", ctx.personality.diligence.clamp(0.0, 1.0));
    m.insert(
        "sociability",
        ctx.personality.sociability.clamp(0.0, 1.0),
    );
    m.insert("temper", ctx.personality.temper.clamp(0.0, 1.0));
    m.insert(
        "playfulness",
        ctx.personality.playfulness.clamp(0.0, 1.0),
    );
    m.insert("warmth", ctx.personality.warmth.clamp(0.0, 1.0));
    m.insert("ambition", ctx.personality.ambition.clamp(0.0, 1.0));
    m.insert(
        "compassion",
        ctx.personality.compassion.clamp(0.0, 1.0),
    );
    // Phase 4c.4 alloparenting Reframe A: bond-weighted compassion for
    // Caretake. Shared-key `compassion` stays stable for herbcraft_prepare;
    // CaretakeDse reads this caretake-local key instead. Scale ≥ 1 so
    // unbonded adults retain baseline compassion, clamp keeps post-boost
    // values in the [0, 1] input range the Linear curve expects.
    m.insert(
        "caretake_compassion",
        (ctx.personality.compassion * ctx.caretake_compassion_bond_scale).clamp(0.0, 1.0),
    );
    // Social-urgency axis inputs. Social deficit = `1 - social`;
    // mating deficit = `1 - mating`; thermal deficit = `1 -
    // temperature`. Phys-satisfaction flows through as raw so the
    // inverted-need-penalty Logistic sees the satisfaction scalar
    // directly (per §2.3 row 1021).
    m.insert(
        "social_deficit",
        (1.0 - ctx.needs.social).clamp(0.0, 1.0),
    );
    m.insert(
        "mating_deficit",
        (1.0 - ctx.needs.mating).clamp(0.0, 1.0),
    );
    m.insert(
        "thermal_deficit",
        (1.0 - ctx.needs.temperature).clamp(0.0, 1.0),
    );
    m.insert(
        "phys_satisfaction",
        ctx.phys_satisfaction.clamp(0.0, 1.0),
    );
    m.insert(
        "tile_corruption",
        ctx.tile_corruption.clamp(0.0, 1.0),
    );
    // Caretake axes.
    m.insert(
        "kitten_urgency",
        ctx.hungry_kitten_urgency.clamp(0.0, 1.0),
    );
    m.insert(
        "is_parent_of_hungry_kitten",
        if ctx.is_parent_of_hungry_kitten {
            1.0
        } else {
            0.0
        },
    );
    // Exploration peer group.
    m.insert(
        "curiosity",
        ctx.personality.curiosity.clamp(0.0, 1.0),
    );
    m.insert(
        "unexplored_nearby",
        ctx.unexplored_nearby.clamp(0.0, 1.0),
    );
    // Work peer group — Build presence axes are 0/1.
    m.insert(
        "has_construction_site",
        if ctx.has_construction_site { 1.0 } else { 0.0 },
    );
    m.insert(
        "has_damaged_building",
        if ctx.has_damaged_building { 1.0 } else { 0.0 },
    );
    m.insert(
        "pending_directive_count",
        ctx.pending_directive_count as f32,
    );
    // Herbcraft + PracticeMagic sibling-DSE axes.
    m.insert(
        "spirituality",
        ctx.personality.spirituality.clamp(0.0, 1.0),
    );
    m.insert(
        "herbcraft_skill",
        ctx.herbcraft_skill.clamp(0.0, 1.0),
    );
    m.insert("magic_skill", ctx.magic_skill.clamp(0.0, 1.0));
    // Ward deficit: 1.0 when wards are low, 0 when fully warded.
    // `ward_strength_low` is the inline gate today; port as a 0/1
    // scalar so the sibling DSE sees it through Linear identity.
    m.insert(
        "ward_deficit",
        if ctx.ward_strength_low { 1.0 } else { 0.0 },
    );
    m.insert(
        "territory_max_corruption",
        ctx.territory_max_corruption.clamp(0.0, 1.0),
    );
    // §2.3 row 6 axis input — consumed by `magic_durable_ward`'s
    // `nearby_corruption_level` Logistic(8, 0.1) consideration,
    // which absorbs the retired `corruption_sensed_response_bonus`
    // modifier contribution by construction.
    m.insert(
        "nearby_corruption_level",
        ctx.nearby_corruption_level.clamp(0.0, 1.0),
    );
    // Saturating-count for Harvest carcass axis — cap at 3 per the
    // old inline `min(3)`.
    m.insert(
        "carcass_count_saturated",
        (ctx.nearby_carcass_count.min(3) as f32) / 3.0,
    );
    m.insert(
        "on_special_terrain",
        if ctx.on_special_terrain { 1.0 } else { 0.0 },
    );
    // Pre-inverted personality scalars for Idle. `incuriosity` =
    // `1 − curiosity`; `playfulness_invert` = `1 − playfulness`. The
    // pre-inversion keeps the consuming curve as a plain Linear
    // rather than requiring `Composite { Linear, Invert }` — cheaper
    // and matches the old additive formula's reading.
    m.insert(
        "incuriosity",
        (1.0 - ctx.personality.curiosity).clamp(0.0, 1.0),
    );
    m.insert(
        "playfulness_invert",
        (1.0 - ctx.personality.playfulness).clamp(0.0, 1.0),
    );
    // Binary presence signals as 0/1 scalars.
    m.insert("prey_nearby", if ctx.prey_nearby { 1.0 } else { 0.0 });
    // Day-phase knot encoding shared with fox_ctx_scalars. Sleep's
    // Piecewise resolves these knot-xs to its cat-specific bonus
    // values; cross-species DSEs (Sleep, fox Hunting, fox Resting)
    // all consume the same encoding.
    m.insert(
        "day_phase",
        day_phase_scalar(ctx.day_phase),
    );
    // §3.5.1 modifier-pipeline inputs for the seven foundational
    // modifiers (Pride, Independence-solo / -group, Patience,
    // Tradition, Fox-suppression, Corruption-suppression). Each modifier
    // reads its trigger and transform inputs through the canonical
    // scalar surface rather than carrying a per-field `ScoringContext`
    // accessor — keeps `ScoreModifier` pure and `EvalCtx` unchanged.
    m.insert("respect", ctx.respect.clamp(0.0, 1.0));
    m.insert("pride", ctx.personality.pride.clamp(0.0, 1.0));
    m.insert(
        "independence",
        ctx.personality.independence.clamp(0.0, 1.0),
    );
    m.insert("patience", ctx.personality.patience.clamp(0.0, 1.0));
    m.insert(
        "tradition_location_bonus",
        ctx.tradition_location_bonus.max(0.0),
    );
    m.insert("fox_scent_level", ctx.fox_scent_level.clamp(0.0, 1.0));
    // Active-disposition ordinal drives the Patience modifier's
    // constituent-DSE membership check. 0.0 encodes `None`; 1.0..=12.0
    // encode each `DispositionKind` variant. The Patience modifier owns
    // the ordinal→kind decode + the `(kind, dse_id)` membership table,
    // preserving the inline `DispositionKind::constituent_actions()`
    // semantics without threading `Option<DispositionKind>` through
    // `EvalCtx`.
    m.insert(
        "active_disposition_ordinal",
        active_disposition_ordinal(ctx.active_disposition),
    );
    // Dummy "one" input for DSEs with base-rate axes (Cook, Idle,
    // Wander). Carried as a scalar so the curve's weight slot is
    // uniform with the other axes.
    m.insert("one", 1.0);
    // §7.W fulfillment deficit — drives grooming/socializing urgency
    // from the Fulfillment register, independent of Maslow social.
    m.insert(
        "social_warmth_deficit",
        ctx.social_warmth_deficit.clamp(0.0, 1.0),
    );
    m
}

/// Encode the active `DispositionKind` as an `f32` ordinal for the
/// Patience modifier's scalar surface. 0.0 = no active disposition;
/// 1.0..=12.0 = each variant in declaration order. The
/// `src/ai/modifier.rs::Patience` modifier decodes and uses the ordinal
/// to look up the set of DSE ids that inherit Patience's additive bonus
/// for the active disposition.
fn active_disposition_ordinal(
    active: Option<crate::components::disposition::DispositionKind>,
) -> f32 {
    use crate::components::disposition::DispositionKind;
    match active {
        None => 0.0,
        Some(DispositionKind::Resting) => 1.0,
        Some(DispositionKind::Hunting) => 2.0,
        Some(DispositionKind::Foraging) => 3.0,
        Some(DispositionKind::Guarding) => 4.0,
        Some(DispositionKind::Socializing) => 5.0,
        Some(DispositionKind::Building) => 6.0,
        Some(DispositionKind::Farming) => 7.0,
        Some(DispositionKind::Crafting) => 8.0,
        Some(DispositionKind::Coordinating) => 9.0,
        Some(DispositionKind::Exploring) => 10.0,
        Some(DispositionKind::Mating) => 11.0,
        Some(DispositionKind::Caretaking) => 12.0,
    }
}

/// Day-phase scalar knots, keyed to the Piecewise curve in Sleep /
/// fox_hunting / fox_resting. Must match
/// `dses::sleep::{DAWN_KNOT, …}` and
/// `dses::fox_hunting::{DAWN_KNOT, …}`.
fn day_phase_scalar(phase: DayPhase) -> f32 {
    use crate::ai::dses::sleep;
    match phase {
        DayPhase::Dawn => sleep::DAWN_KNOT,
        DayPhase::Day => sleep::DAY_KNOT,
        DayPhase::Dusk => sleep::DUSK_KNOT,
        DayPhase::Night => sleep::NIGHT_KNOT,
    }
}

/// Score a registered cat DSE through the L2 evaluator. Returns the
/// DSE's final score (post-Maslow, post-modifier-pipeline) or 0.0 if
/// the DSE is missing or ineligible.
///
/// When `inputs.focal_cat == Some(inputs.cat)` and
/// `inputs.focal_capture` is set, this routes the evaluator call
/// through `evaluate_single_with_trace` and pushes the full §11.3 L2
/// breakdown (per-consideration inputs + curve labels, composition
/// mode/weights, Maslow pre-gate, per-modifier pre/post deltas) into
/// the capture resource. Non-focal calls take the untraced path and
/// incur zero capture cost beyond the two `Option` checks below.
fn score_dse_by_id(
    dse_id: &str,
    ctx: &ScoringContext,
    inputs: &EvalInputs,
) -> f32 {
    let Some(dse) = inputs.dse_registry.cat_dse(dse_id) else {
        return 0.0;
    };
    let scalars = ctx_scalars(ctx);
    let fetch_scalar = |name: &str, _entity: Entity| -> f32 {
        scalars.get(name).copied().unwrap_or(0.0)
    };
    // §4 marker lookup — consumes `EvalInputs::markers` populated by
    // the caller. `entity` is the evaluating cat when eligibility runs
    // against a per-cat marker; colony-scoped markers ignore it.
    let markers = inputs.markers;
    let has_marker = |name: &str, entity: Entity| -> bool {
        markers.has(name, entity)
    };
    let sample_map = |_name: &str, _pos: Position| 0.0_f32;
    let needs_ref = ctx.needs;
    let maslow = |tier: u8| needs_ref.level_suppression(tier);

    let eval_ctx = EvalCtx {
        cat: inputs.cat,
        tick: inputs.tick,
        sample_map: &sample_map,
        has_marker: &has_marker,
        self_position: inputs.position,
        target: None,
        target_position: None,
    };

    let focal_active =
        inputs.focal_capture.is_some() && inputs.focal_cat == Some(inputs.cat);

    if focal_active {
        let filter = dse.eligibility();
        // Capture ineligible DSEs explicitly — without this, a
        // permanently-ineligible DSE (e.g. Hunt for a cat whose
        // `CanHunt` marker is never set) leaves no trace row at all,
        // and "why didn't Hunt even appear?" has no answer in the
        // focal trace. One stripped row per ineligible DSE per tick
        // is bounded (DSE catalog ~20 items).
        if !crate::ai::eval::passes_eligibility(filter, inputs.cat, &eval_ctx) {
            if let Some(capture) = inputs.focal_capture {
                capture.push_dse(
                    crate::resources::trace_log::CapturedDse {
                        dse_id: dse.id(),
                        raw_score: 0.0,
                        gated_score: 0.0,
                        final_score: 0.0,
                        // Placeholder Intention — the `eligible: false`
                        // flag tells downstream tooling this field is
                        // meaningless. `default_strategy()` keeps the
                        // shape well-formed without calling `emit()`,
                        // which would need inputs an ineligible DSE
                        // doesn't have.
                        intention: crate::ai::dse::Intention::Activity {
                            kind: crate::ai::dse::ActivityKind::Idle,
                            termination: crate::ai::dse::Termination::UntilInterrupt,
                            strategy: dse.default_strategy(),
                        },
                        trace: crate::ai::eval::EvalTrace::default(),
                        eligibility_required: filter.required.to_vec(),
                        eligibility_forbidden: filter.forbidden.to_vec(),
                        eligible: false,
                    },
                    inputs.tick,
                );
            }
            return 0.0;
        }

        let mut trace = crate::ai::eval::EvalTrace::default();
        let scored = crate::ai::eval::evaluate_single_with_trace(
            dse,
            inputs.cat,
            &eval_ctx,
            &maslow,
            inputs.modifier_pipeline,
            &fetch_scalar,
            Some(&mut trace),
        );
        if let (Some(scored), Some(capture)) = (scored, inputs.focal_capture) {
            capture.push_dse(
                crate::resources::trace_log::CapturedDse {
                    dse_id: scored.id,
                    raw_score: scored.raw_score,
                    gated_score: scored.gated_score,
                    final_score: scored.final_score,
                    intention: scored.intention.clone(),
                    trace,
                    eligibility_required: filter.required.to_vec(),
                    eligibility_forbidden: filter.forbidden.to_vec(),
                    eligible: true,
                },
                inputs.tick,
            );
            return scored.final_score;
        }
        return 0.0;
    }

    evaluate_single(
        dse,
        inputs.cat,
        &eval_ctx,
        &maslow,
        inputs.modifier_pipeline,
        &fetch_scalar,
    )
    .map(|s| s.final_score)
    .unwrap_or(0.0)
}

/// Score all available actions for a cat given its current state.
///
/// Returns a [`ScoringResult`] containing `(Action, score)` pairs and an
/// optional herbcraft sub-mode hint. Higher score = more preferred.
/// The caller should pass the scores to [`select_best_action`].
pub fn score_actions(
    ctx: &ScoringContext,
    inputs: &EvalInputs,
    rng: &mut impl Rng,
) -> ScoringResult {
    let s = ctx.scoring;
    let mut scores = Vec::with_capacity(12);

    // §13.1 rows 1–3: the inline `if ctx.is_incapacitated` early-return
    // retired. Incapacitation is now enforced as a per-cat §4.3 marker
    // (`Incapacitated`, authored by `systems::incapacitation`) that
    // gates every non-Eat/Sleep/Idle DSE via `.forbid("Incapacitated")`
    // on its `EligibilityFilter`. The surviving Eat/Sleep/Idle DSEs
    // stay eligible because their Logistic-anchored curves spike hard
    // enough on hunger/energy to dominate selection without the
    // bespoke `incapacitated_*_urgency_{scale,offset}` multipliers
    // that rode this branch. The `ScoringContext.is_incapacitated`
    // field is retained for non-scoring consumers.

    // --- Eat (§2.3 hangry anchor: Logistic(8, 0.75)) ---
    // §4 (Phase 4b.2) retired the outer `ctx.food_available` gate. The
    // Eat DSE's `.require("HasStoredFood")` eligibility filter resolves
    // against `EvalInputs::markers` (populated by the caller from
    // `FoodStores`), returning 0 when the colony has no food.
    {
        let urgency = score_dse_by_id("eat", ctx, inputs);
        if urgency > 0.0 {
            scores.push((Action::Eat, urgency + jitter(rng, s.jitter_range)));
        }
    }

    // --- Sleep (§2.3: WS of energy_deficit + day_phase + injury_rest) ---
    // The additive-not-multiplicative semantic noted in the old
    // inline comment is preserved by WS composition — Sleep remains
    // available as a pressure-release valve at low energy even
    // during feeding peaks.
    {
        let score = score_dse_by_id("sleep", ctx, inputs);
        scores.push((Action::Sleep, score + jitter(rng, s.jitter_range)));
    }

    // --- Hunt (§2.3: WS of hunger + scarcity + boldness + prey_nearby) ---
    if ctx.can_hunt {
        let urgency = score_dse_by_id("hunt", ctx, inputs);
        scores.push((Action::Hunt, urgency + jitter(rng, s.jitter_range)));
    }

    // --- Forage (§2.3: WS of hunger + scarcity + diligence) ---
    if ctx.can_forage {
        let urgency = score_dse_by_id("forage", ctx, inputs);
        scores.push((Action::Forage, urgency + jitter(rng, s.jitter_range)));
    }

    // --- Socialize (§2.3: WS of 6 axes through loneliness + inverted_need_penalty) ---
    if ctx.has_social_target {
        let score = score_dse_by_id("socialize", ctx, inputs);
        scores.push((Action::Socialize, score + jitter(rng, s.jitter_range)));
    }

    // --- Groom (§L2.10.10 sibling split: Groom_self + Groom_other;
    // Max composition retires, the planner reads whichever sibling
    // scores higher through the DSE registry). For backward
    // compatibility with the existing `Action::Groom` enum, this
    // emits a single score that's the max of the two siblings. Phase
    // 3d will teach selection about the sibling-id mapping so the
    // Action variant carries which sibling won.
    {
        let self_score = score_dse_by_id("groom_self", ctx, inputs);
        let other_score = if ctx.has_social_target {
            score_dse_by_id("groom_other", ctx, inputs)
        } else {
            0.0
        };
        scores.push((
            Action::Groom,
            self_score.max(other_score) + jitter(rng, s.jitter_range),
        ));
    }

    // --- Explore (§2.3: CP of curiosity + unexplored_nearby) ---
    {
        let score = score_dse_by_id("explore", ctx, inputs);
        scores.push((Action::Explore, score + jitter(rng, s.jitter_range)));
    }

    // --- Wander (§2.3: WS of curiosity + base_rate + playfulness) ---
    {
        let score = score_dse_by_id("wander", ctx, inputs);
        scores.push((Action::Wander, score + jitter(rng, s.jitter_range)));
    }

    // --- Flee (§2.3: CP of safety_deficit + boldness_inverse) ---
    if ctx.has_threat_nearby || ctx.needs.safety < s.flee_safety_threshold {
        let score = score_dse_by_id("flee", ctx, inputs);
        scores.push((Action::Flee, score + jitter(rng, s.jitter_range)));
    }

    // --- Fight (§2.3: WS of boldness + combat + health + safety + ally_count) ---
    // Outer gate retains the original `has_threat_nearby && allies ≥
    // min` precondition. Inside the DSE: the `fight_gating` Piecewise
    // curve on the health + safety axes encodes the old suppression
    // thresholds (drops to ~0.2 at health < 0.3, saturates at
    // health ≥ 0.5) — no external `health_factor` / `safety_factor`
    // multipliers needed.
    if ctx.has_threat_nearby && ctx.allies_fighting_threat >= s.fight_min_allies {
        let score = score_dse_by_id("fight", ctx, inputs);
        scores.push((Action::Fight, score + jitter(rng, s.jitter_range)));
    }

    // --- Patrol (§2.3: CP of safety_deficit + boldness) ---
    if ctx.needs.safety < s.patrol_safety_threshold {
        let score = score_dse_by_id("patrol", ctx, inputs);
        scores.push((Action::Patrol, score + jitter(rng, s.jitter_range)));
    }

    // --- Build (§2.3: WS of diligence + site_presence + repair_presence) ---
    if ctx.has_construction_site || ctx.has_damaged_building {
        let score = score_dse_by_id("build", ctx, inputs);
        scores.push((Action::Build, score + jitter(rng, s.jitter_range)));
    }

    // --- Farm (§2.3: CP of food_scarcity + diligence) ---
    // §4 (Phase 4b.4): the outer `ctx.has_garden` gate retires — the
    // Farm DSE's `.require("HasGarden")` eligibility filter resolves
    // against the `HasGarden` colony marker populated by the caller.
    {
        let urgency = score_dse_by_id("farm", ctx, inputs);
        if urgency > 0.0 {
            scores.push((Action::Farm, urgency + jitter(rng, s.jitter_range)));
        }
    }

    // --- Herbcraft (§L2.10.10 sibling split: gather + prepare + ward) ---
    // Each sub-mode's base score comes from its sibling DSE; the
    // retired `ward_corruption_emergency_bonus` flat additive (Phase
    // 4.2 ported it to a modifier; §13.1 retired the modifier) is
    // absorbed into the Logistic(8, 0.1) axis on
    // `territory_max_corruption` inside both `herbcraft_gather` and
    // `herbcraft_ward`. The outer Max + hint selection is a
    // selection-layer concern that stays here. The siege bonus
    // remains inline — it's a narrower-scope siege response, not a
    // corruption trigger.
    let herbcraft_hint;
    {
        let gather = if ctx.has_herbs_nearby {
            score_dse_by_id("herbcraft_gather", ctx, inputs)
        } else {
            0.0
        };
        let prepare = if ctx.has_remedy_herbs && ctx.colony_injury_count > 0 {
            score_dse_by_id("herbcraft_prepare", ctx, inputs)
        } else {
            0.0
        };
        // Ward requires ward herbs in inventory to execute. The earlier
        // "corruption-detected + thornbriar-available" branch that
        // scored ward via an inline emergency bonus is retired: the
        // cat now scores gather (boosted by the Logistic(8, 0.1) axis
        // on `territory_max_corruption` inside `herbcraft_gather`) to
        // collect thornbriar first, then scores ward on a later tick
        // once herbs are held.
        // §4 marker eligibility (Phase 4b.5): the
        // `ctx.ward_strength_low` conjunct retires — HerbcraftWardDse
        // now carries `.require("WardStrengthLow")`. The
        // `ctx.has_ward_herbs` check stays inline until a per-cat
        // inventory-marker port lands `HasWardHerbs`.
        let ward_eligible = ctx.has_ward_herbs;
        let mut ward = if ward_eligible {
            score_dse_by_id("herbcraft_ward", ctx, inputs)
        } else {
            0.0
        };
        if ctx.wards_under_siege && ctx.has_ward_herbs {
            ward += s.herbcraft_ward_siege_bonus * ctx.needs.level_suppression(2);
        }
        let best = gather.max(prepare).max(ward);
        herbcraft_hint = if best <= 0.0 {
            None
        } else if prepare >= gather && prepare >= ward {
            Some(CraftingHint::PrepareRemedy)
        } else if ward >= gather {
            Some(CraftingHint::SetWard)
        } else {
            Some(CraftingHint::GatherHerbs)
        };
        if best > 0.0 {
            scores.push((Action::Herbcraft, best + jitter(rng, s.jitter_range)));
        }
    }

    // --- PracticeMagic (§L2.10.10 sibling split — 6 sub-modes) ---
    // Outer gate: `magic_affinity + magic_skill > thresholds`.
    // Sub-mode base scores come from their sibling DSEs. The three
    // emergency bonuses (ward / cleanse / sensed-rot) retired in
    // §13.1 once their axis-level Logistic replacements landed:
    // `magic_durable_ward` grew a Logistic(8, 0.1) axis on
    // `nearby_corruption_level`; `magic_cleanse` swapped its
    // `tile_corruption` axis to `Logistic(8,
    // magic_cleanse_corruption_threshold)`; `magic_colony_cleanse`
    // swapped its `territory_max_corruption` axis to
    // Logistic(6, 0.3). The outer Max + hint selection stays here.
    let mut magic_hint: Option<CraftingHint> = None;
    if ctx.magic_affinity > s.magic_affinity_threshold && ctx.magic_skill > s.magic_skill_threshold
    {
        let scry = score_dse_by_id("magic_scry", ctx, inputs);
        // §4 marker eligibility (Phase 4b.5): the
        // `ctx.ward_strength_low` conjunct retires —
        // `DurableWardDse` now carries `.require("WardStrengthLow")`.
        // The `magic_skill` threshold stays inline (it's a §4.5
        // scalar, not a marker).
        let durable_ward = if ctx.magic_skill > s.magic_durable_ward_skill_threshold {
            score_dse_by_id("magic_durable_ward", ctx, inputs)
        } else {
            0.0
        };
        let cleanse = if ctx.on_corrupted_tile
            && ctx.tile_corruption > s.magic_cleanse_corruption_threshold
        {
            score_dse_by_id("magic_cleanse", ctx, inputs)
        } else {
            0.0
        };
        let colony_cleanse = score_dse_by_id("magic_colony_cleanse", ctx, inputs);
        let harvest = if ctx.carcass_nearby {
            score_dse_by_id("magic_harvest", ctx, inputs)
        } else {
            0.0
        };
        let commune = if ctx.on_special_terrain {
            score_dse_by_id("magic_commune", ctx, inputs)
        } else {
            0.0
        };
        let best = scry
            .max(durable_ward)
            .max(cleanse)
            .max(colony_cleanse)
            .max(harvest)
            .max(commune);
        // Pick the winning sub-action as a hint so the GOAP planner uses a
        // directed action list instead of cost-picking the cheapest option.
        magic_hint = if best <= 0.0 {
            None
        } else if durable_ward >= best {
            Some(CraftingHint::DurableWard)
        } else if colony_cleanse >= best || cleanse >= best {
            Some(CraftingHint::Cleanse)
        } else if harvest >= best {
            Some(CraftingHint::HarvestCarcass)
        } else {
            // Scry and Commune fall through to the generic Magic action list.
            Some(CraftingHint::Magic)
        };
        if best > 0.0 {
            scores.push((Action::PracticeMagic, best + jitter(rng, s.jitter_range)));
        }
    }

    // --- Coordinate (§2.3: WS of diligence + directive_count + ambition) ---
    if ctx.is_coordinator_with_directives {
        let score = score_dse_by_id("coordinate", ctx, inputs);
        scores.push((Action::Coordinate, score + jitter(rng, s.jitter_range)));
    }

    // --- Mentor (§2.3: WS of warmth + diligence + ambition) ---
    if ctx.has_mentoring_target {
        let score = score_dse_by_id("mentor", ctx, inputs);
        scores.push((Action::Mentor, score + jitter(rng, s.jitter_range)));
    }

    // --- Mate (§2.3: CP of mating_deficit + warmth — Logistic(6, 0.6)) ---
    if ctx.has_eligible_mate {
        let urgency = score_dse_by_id("mate", ctx, inputs);
        if urgency > 0.0 {
            scores.push((Action::Mate, urgency + jitter(rng, s.jitter_range)));
        }
    }

    // --- Cook (food-production tier; requires a Kitchen, raw food, and the
    //     cat not to be on the verge of starvation). Diligent cats cook more,
    //     and urgency scales with food scarcity — cooking is the colony's
    //     food-buffer multiplier, analogous to Farm. Level 2 suppression
    //     (phys only) matches Hunt/Forage: a fed cat will cook; an exhausted
    //     cat will still sleep first, but safety doesn't gate the action.
    //     Receives a directive bonus if a `DirectiveKind::Cook` is active.
    //
    // §4 marker eligibility (Phase 4b.5): `CookDse` now carries
    // `.require("HasFunctionalKitchen").require("HasRawFoodInStores")`;
    // the outer `cook_base_conditions && ctx.has_functional_kitchen`
    // gate retires. The `hunger > cook_hunger_gate` threshold is a
    // §4.5 scalar precondition and stays as an inline wrap so Cook
    // isn't scored while the cat is stuffed. The
    // `wants_cook_but_no_kitchen` latent signal (read by BuildPressure
    // in `goap.rs`) is preserved by disambiguating the zero-score case
    // against the raw ScoringContext booleans — "raw food is present
    // but no kitchen" is still the only trigger.
    let hungry_enough_to_cook = ctx.needs.hunger > s.cook_hunger_gate;
    let mut wants_cook_but_no_kitchen = false;
    if hungry_enough_to_cook {
        let score = score_dse_by_id("cook", ctx, inputs);
        if score > 0.0 {
            scores.push((Action::Cook, score + jitter(rng, s.jitter_range)));
        } else if ctx.has_raw_food_in_stores && !ctx.has_functional_kitchen {
            wants_cook_but_no_kitchen = true;
        }
    }

    // --- Caretake (§2.3: WS of kitten_urgency + compassion + is_parent) ---
    if ctx.hungry_kitten_urgency > 0.0 {
        let score = score_dse_by_id("caretake", ctx, inputs);
        scores.push((Action::Caretake, score + jitter(rng, s.jitter_range)));
    }

    // --- Idle (§2.3: WS of base_rate + incuriosity + playfulness_invert) ---
    // The always-available fallback. The base_rate axis's Linear
    // intercept carries `idle_base` and ClampMin floors at
    // `idle_minimum_floor` — post-composition floor per §2.3 baked
    // into the base axis rather than a §3.5 modifier.
    {
        let score = score_dse_by_id("idle", ctx, inputs);
        scores.push((Action::Idle, score + jitter(rng, s.jitter_range)));
    }

    // §3.5 post-scoring modifiers previously ran as imperative passes
    // here — Pride, Independence (solo / group), Patience, Tradition,
    // Fox-suppression, Corruption-suppression. All seven are now
    // registered in `crate::ai::modifier::default_modifier_pipeline`
    // and apply inside `evaluate_single` per-DSE. See
    // `src/ai/modifier.rs` (Pride / IndependenceSolo /
    // IndependenceGroup / Patience / Tradition /
    // FoxTerritorySuppression / CorruptionTerritorySuppression) for
    // the individual ports.

    ScoringResult {
        scores,
        herbcraft_hint,
        magic_hint,
        wants_cook_but_no_kitchen,
    }
}

// ---------------------------------------------------------------------------
// Context bonuses (applied after base scoring)
// ---------------------------------------------------------------------------

/// Boost action scores based on remembered events near the cat's position.
///
/// - `ResourceFound` memories boost Hunt and Forage.
/// - `Death` memories suppress Wander and Idle (safety instinct).
///
/// Both scale with memory strength and proximity to the remembered location.
pub fn apply_memory_bonuses(
    scores: &mut [(Action, f32)],
    memory: &Memory,
    pos: &Position,
    sc: &ScoringConstants,
) {
    for entry in &memory.events {
        let Some(loc) = &entry.location else { continue };
        let dist = pos.distance_to(loc);
        if dist > sc.memory_nearby_radius {
            continue;
        }

        let proximity = 1.0 - (dist / sc.memory_nearby_radius);
        let bonus = proximity * entry.strength;

        match entry.event_type {
            MemoryType::ResourceFound => {
                for (action, score) in scores.iter_mut() {
                    if matches!(action, Action::Hunt | Action::Forage) {
                        *score += bonus * sc.memory_resource_bonus;
                    }
                }
            }
            MemoryType::Death => {
                for (action, score) in scores.iter_mut() {
                    if matches!(action, Action::Wander | Action::Idle) {
                        *score -= bonus * sc.memory_death_penalty;
                    }
                }
            }
            MemoryType::ThreatSeen => {
                // Suppress exploration and hunting near known threat locations.
                for (action, score) in scores.iter_mut() {
                    if matches!(action, Action::Wander | Action::Explore | Action::Hunt) {
                        *score -= bonus * sc.memory_threat_penalty;
                    }
                }
            }
            _ => {}
        }
    }
}

/// Boost action scores based on what nearby cats are doing.
///
/// For each action (except Fight, which has its own dedicated ally bonus),
/// adds `cascading_bonus_per_cat * count` where `count` is the number of cats
/// within range performing that action. Creates emergent group behaviors.
pub fn apply_cascading_bonuses(
    scores: &mut [(Action, f32)],
    nearby_actions: &HashMap<Action, usize>,
    sc: &ScoringConstants,
) {
    for (action, score) in scores.iter_mut() {
        // Fight already has fight_ally_bonus_per_cat in its base scoring;
        // applying the generic cascade on top creates a positive feedback
        // loop that snowballs into colony-wide fight charges.
        if *action == Action::Fight {
            continue;
        }
        if let Some(&count) = nearby_actions.get(action) {
            *score += count as f32 * sc.cascading_bonus_per_cat;
        }
    }
}

/// Apply a coordinator's directive bonus to the target action's score.
///
/// Called after base scoring and cascading bonuses. The bonus is pre-computed
/// from the directive priority, coordinator social weight, and the target cat's
/// personality (diligence, independence, stubbornness).
pub fn apply_directive_bonus(scores: &mut [(Action, f32)], target_action: Action, bonus: f32) {
    for (action, score) in scores.iter_mut() {
        if *action == target_action {
            *score += bonus;
        }
    }
}

/// Boost action scores based on colony-wide knowledge of the environment.
///
/// ThreatSeen/Death entries near the cat boost Patrol scores.
/// ResourceFound entries near the cat boost Hunt/Forage scores.
pub fn apply_colony_knowledge_bonuses(
    scores: &mut [(Action, f32)],
    knowledge: &crate::resources::colony_knowledge::ColonyKnowledge,
    pos: &Position,
    sc: &ScoringConstants,
) {
    for entry in &knowledge.entries {
        let Some(loc) = &entry.location else { continue };
        let dist = pos.distance_to(loc);
        if dist > sc.colony_knowledge_radius {
            continue;
        }

        let proximity = 1.0 - (dist / sc.colony_knowledge_radius);
        let bonus = proximity * entry.strength * sc.colony_knowledge_bonus_scale;

        match entry.event_type {
            MemoryType::ThreatSeen | MemoryType::Death => {
                for (action, score) in scores.iter_mut() {
                    if matches!(action, Action::Patrol) {
                        *score += bonus;
                    }
                }
            }
            MemoryType::ResourceFound => {
                for (action, score) in scores.iter_mut() {
                    if matches!(action, Action::Forage | Action::Hunt) {
                        *score += bonus;
                    }
                }
            }
            _ => {}
        }
    }
}

/// Boost action scores based on an active player-set colony priority.
pub fn apply_priority_bonus(
    scores: &mut [(Action, f32)],
    priority: Option<crate::resources::colony_priority::PriorityKind>,
    sc: &ScoringConstants,
) {
    let Some(kind) = priority else { return };
    use crate::resources::colony_priority::PriorityKind;
    let bonus = sc.priority_bonus;
    let matching: &[Action] = match kind {
        PriorityKind::Food => &[Action::Hunt, Action::Forage, Action::Farm],
        PriorityKind::Defense => &[Action::Patrol, Action::Fight],
        PriorityKind::Building => &[Action::Build],
        PriorityKind::Exploration => &[Action::Explore],
    };
    for (action, score) in scores.iter_mut() {
        if matching.contains(action) {
            *score += bonus;
        }
    }
}

// ---------------------------------------------------------------------------
// Aspiration, preference, and fate bonuses
// ---------------------------------------------------------------------------

/// Boost action scores based on active aspirations.
///
/// For each active aspiration, adds a flat desire bonus to actions in the
/// aspiration's domain. This makes cats *want* to do things related to
/// their goals, without changing their skill at doing them.
pub fn apply_aspiration_bonuses(
    scores: &mut [(Action, f32)],
    aspirations: &crate::components::aspirations::Aspirations,
    sc: &ScoringConstants,
) {
    for asp in &aspirations.active {
        let matching = asp.domain.matching_actions();
        for (action, score) in scores.iter_mut() {
            if matching.contains(action) {
                *score += sc.aspiration_bonus;
            }
        }
    }
}

/// Adjust action scores based on personal likes and dislikes.
///
/// Like: +0.08 desire bonus. Dislike: -0.08 desire penalty.
/// Smaller than aspiration bonuses — preferences are background flavor.
pub fn apply_preference_bonuses(
    scores: &mut [(Action, f32)],
    preferences: &crate::components::aspirations::Preferences,
    sc: &ScoringConstants,
) {
    for (action, score) in scores.iter_mut() {
        match preferences.get(*action) {
            Some(crate::components::aspirations::Preference::Like) => {
                *score += sc.preference_like_bonus
            }
            Some(crate::components::aspirations::Preference::Dislike) => {
                *score -= sc.preference_dislike_penalty
            }
            None => {}
        }
    }
}

/// Boost action scores based on awakened fated connections.
///
/// - Fated love (awakened, partner visible): +0.15 to Socialize/Groom.
/// - Fated rival (awakened, rival nearby): +0.1 to Hunt/Patrol/Fight/Explore.
pub fn apply_fated_bonuses(
    scores: &mut [(Action, f32)],
    fated_love_visible: bool,
    fated_rival_nearby: bool,
    sc: &ScoringConstants,
) {
    if fated_love_visible {
        for (action, score) in scores.iter_mut() {
            if matches!(action, Action::Socialize | Action::Groom | Action::Mate) {
                *score += sc.fated_love_social_bonus;
            }
        }
    }
    if fated_rival_nearby {
        for (action, score) in scores.iter_mut() {
            if matches!(
                action,
                Action::Hunt | Action::Patrol | Action::Fight | Action::Explore
            ) {
                *score += sc.fated_rival_competition_bonus;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Survival floor
// ---------------------------------------------------------------------------

/// Ensure survival actions (Eat, Sleep) aren't outcompeted by bonus-inflated
/// higher-level actions when basic needs are critical.
///
/// When physiological satisfaction drops below 0.5, non-survival action scores
/// are compressed toward the highest survival score. At full starvation (phys=0),
/// no non-survival action can outscore Eat/Sleep. Flee is exempt — running from
/// a predator while starving is rational.
pub fn enforce_survival_floor(scores: &mut [(Action, f32)], needs: &Needs, sc: &ScoringConstants) {
    let phys = needs.physiological_satisfaction();
    if phys >= sc.survival_floor_phys_threshold {
        return;
    }

    let survival_ceiling = scores
        .iter()
        .filter(|(a, _)| matches!(a, Action::Eat | Action::Sleep))
        .map(|(_, s)| *s)
        .fold(0.0f32, f32::max);

    if survival_ceiling <= 0.0 {
        return;
    }

    let factor = phys / sc.survival_floor_phys_threshold;
    for (action, score) in scores.iter_mut() {
        if matches!(action, Action::Eat | Action::Sleep | Action::Flee) {
            continue;
        }
        if *score > survival_ceiling {
            *score = survival_ceiling + (*score - survival_ceiling) * factor;
        }
    }
}

// ---------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------

/// Pick the action with the highest score. Falls back to [`Action::Idle`] if
/// the slice is empty or all scores are non-finite.
pub fn select_best_action(scores: &[(Action, f32)]) -> Action {
    scores
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(action, _)| *action)
        .unwrap_or(Action::Idle)
}

/// Select an action using softmax (Boltzmann) sampling.
///
/// Instead of always picking the highest score, treats scores as weights
/// and samples probabilistically. Temperature controls variety:
/// - T → 0: converges to argmax (deterministic)
/// - T = 0.10: personality-primary ~45-60%, realistic secondary behaviors
/// - T → ∞: uniform random (personality irrelevant)
pub fn select_action_softmax(
    scores: &[(Action, f32)],
    rng: &mut impl Rng,
    sc: &ScoringConstants,
) -> Action {
    let temperature = sc.action_softmax_temperature;

    if scores.is_empty() {
        return Action::Idle;
    }

    let max_score = scores
        .iter()
        .map(|(_, s)| *s)
        .fold(f32::NEG_INFINITY, f32::max);
    let weights: Vec<f32> = scores
        .iter()
        .map(|(_, s)| ((s - max_score) / temperature).exp())
        .collect();
    let total: f32 = weights.iter().sum();

    let mut roll: f32 = rng.random::<f32>() * total;
    for (i, w) in weights.iter().enumerate() {
        roll -= w;
        if roll <= 0.0 {
            return scores[i].0;
        }
    }

    scores.last().map(|(a, _)| *a).unwrap_or(Action::Idle)
}

// ---------------------------------------------------------------------------
// Behavior gates
// ---------------------------------------------------------------------------

/// Apply behavior gate overrides for extreme personality values.
///
/// Checked after scoring, before action execution. Returns `Some(overridden)`
/// if a gate fires, `None` if the chosen action stands.
///
/// Gates (from personality.md):
/// - Boldness < 0.1: Fight → Flee (too timid)
/// - Sociability < 0.15: Socialize → Idle (too shy)
/// - Curiosity > 0.9: 20% chance override → Explore (compulsive explorer)
/// - Boldness > 0.9: Flee → Fight (reckless bravery)
/// - Compassion > 0.9 + injured nearby: override → Herbcraft (compulsive helper)
///
/// Stubbornness > 0.85 directive rejection is handled separately via the
/// `DirectiveRefused` event in the personality events system.
pub fn behavior_gate_check(
    chosen: Action,
    personality: &Personality,
    has_injured_nearby: bool,
    health_ratio: f32,
    rng: &mut impl Rng,
    sc: &ScoringConstants,
) -> Option<Action> {
    // Too timid to fight — always flee instead.
    if chosen == Action::Fight && personality.boldness < sc.gate_timid_fight_threshold {
        return Some(Action::Flee);
    }
    // Too shy to socialize — skip to idle.
    if chosen == Action::Socialize && personality.sociability < sc.gate_shy_socialize_threshold {
        return Some(Action::Idle);
    }
    // Reckless bravery — cannot flee, must fight. But only when healthy enough.
    if chosen == Action::Flee
        && personality.boldness > sc.gate_reckless_flee_threshold
        && health_ratio > sc.gate_reckless_health_threshold
    {
        return Some(Action::Fight);
    }
    // Compulsive helper — drop everything to aid an injured cat.
    if personality.compassion > sc.gate_compulsive_helper_threshold && has_injured_nearby {
        return Some(Action::Herbcraft);
    }
    // Compulsive explorer — chance per tick to ignore current action.
    if personality.curiosity > sc.gate_compulsive_explorer_threshold
        && !matches!(
            chosen,
            Action::Eat | Action::Sleep | Action::Flee | Action::Explore
        )
        && rng.random::<f32>() < sc.gate_compulsive_explorer_chance
    {
        return Some(Action::Explore);
    }
    None
}

// ---------------------------------------------------------------------------
// Disposition scoring (aggregate from action scores)
// ---------------------------------------------------------------------------

use crate::components::disposition::DispositionKind;

/// Aggregate per-action scores into per-disposition scores.
///
/// For each disposition, takes the MAX score among its constituent actions.
/// Actions not present in the score list contribute nothing (the disposition
/// may still appear with score 0.0 if no constituent scored).
///
/// Groom is special: it appears in both Resting (self-groom) and Socializing
/// (groom-other). The caller should set `self_groom_won` based on whether the
/// self-groom sub-score beat the other-groom sub-score during action scoring.
pub fn aggregate_to_dispositions(
    action_scores: &[(Action, f32)],
    self_groom_won: bool,
) -> Vec<(DispositionKind, f32)> {
    let mut disposition_scores: Vec<(DispositionKind, f32)> = DispositionKind::ALL
        .iter()
        .map(|kind| (*kind, 0.0f32))
        .collect();

    for &(action, score) in action_scores {
        // Skip Flee and Idle — not dispositions.
        if matches!(action, Action::Flee | Action::Idle) {
            continue;
        }

        // Groom routes to Resting or Socializing depending on which sub-score won.
        if action == Action::Groom {
            let target_kind = if self_groom_won {
                DispositionKind::Resting
            } else {
                DispositionKind::Socializing
            };
            if let Some((_, existing)) = disposition_scores
                .iter_mut()
                .find(|(k, _)| *k == target_kind)
            {
                *existing = existing.max(score);
            }
            continue;
        }

        if let Some(kind) = DispositionKind::from_action(action) {
            if let Some((_, existing)) = disposition_scores.iter_mut().find(|(k, _)| *k == kind) {
                *existing = existing.max(score);
            }
        }
    }

    // Remove dispositions with zero or negative scores (no constituent action was available).
    disposition_scores.retain(|(_, score)| *score > 0.0);
    disposition_scores
}

/// Select a disposition using softmax (Boltzmann) sampling.
///
/// Same algorithm as `select_action_softmax` but over disposition scores.
/// Falls back to `Resting` if the slice is empty.
pub fn select_disposition_softmax(
    scores: &[(DispositionKind, f32)],
    rng: &mut impl Rng,
    sc: &ScoringConstants,
) -> DispositionKind {
    let temperature = sc.disposition_softmax_temperature;

    if scores.is_empty() {
        return DispositionKind::Resting;
    }

    let max_score = scores
        .iter()
        .map(|(_, s)| *s)
        .fold(f32::NEG_INFINITY, f32::max);
    let weights: Vec<f32> = scores
        .iter()
        .map(|(_, s)| ((s - max_score) / temperature).exp())
        .collect();
    let total: f32 = weights.iter().sum();

    let mut roll: f32 = rng.random::<f32>() * total;
    for (i, w) in weights.iter().enumerate() {
        roll -= w;
        if roll <= 0.0 {
            return scores[i].0;
        }
    }

    scores
        .last()
        .map(|(k, _)| *k)
        .unwrap_or(DispositionKind::Resting)
}

// ---------------------------------------------------------------------------
// §L2.10.6 softmax-over-Intentions (flat-action-pool selection)
// ---------------------------------------------------------------------------

/// Select a disposition by running softmax over the flat action-score
/// pool (spec §L2.10.6). Replaces the legacy
/// `aggregate_to_dispositions → select_disposition_softmax` pipeline.
///
/// Peer-group aggregation (MAX across disposition constituents) concentrates
/// weight on whichever peer-group's top score dominates, which magnifies
/// into a monoculture over long soaks (the `Starvation 3 → 8` regression
/// documented in `docs/balance/substrate-phase-3.md` § Concordance).
/// Softmax-over-Intentions dissolves the peer-group collapse: each action
/// competes equally in the softmax pool with temperature-controlled variety.
///
/// Current behavior vs. legacy path is preserved with two action-level
/// transforms applied before softmax, so this function is a drop-in
/// replacement for the 2-step aggregate/softmax dance (documented inline
/// below).
///
/// `self_groom_won` disambiguates the Groom action's disposition (Resting
/// when self-groom dominates, Socializing when other-groom does).
/// `independence` is the cat's personality score; `sc` carries the
/// `intention_softmax_temperature` and `disposition_independence_penalty`
/// constants.
pub fn select_disposition_via_intention_softmax(
    scores: &[(Action, f32)],
    self_groom_won: bool,
    independence: f32,
    disposition_independence_penalty: f32,
    sc: &ScoringConstants,
    rng: &mut impl Rng,
) -> DispositionKind {
    select_disposition_via_intention_softmax_with_trace(
        scores,
        self_groom_won,
        independence,
        disposition_independence_penalty,
        sc,
        rng,
        None,
    )
}

/// §11.3 L3 softmax capture surface. When `sink` is `Some`, populates it
/// with the filtered pool, softmax weights/probabilities, temperature,
/// the RNG roll, and the picked Action. This is the only way to surface
/// the softmax distribution for replay since `rng.random::<f32>()` is
/// consumed in place and not recoverable post-hoc.
///
/// `sink` being `Some` is the load-bearing focal-cat detection gate.
/// Non-focal cats pass `None` and incur zero capture cost.
pub fn select_disposition_via_intention_softmax_with_trace(
    scores: &[(Action, f32)],
    self_groom_won: bool,
    independence: f32,
    disposition_independence_penalty: f32,
    sc: &ScoringConstants,
    rng: &mut impl Rng,
    mut sink: Option<&mut SoftmaxCapture>,
) -> DispositionKind {
    // Build the filtered pool: drop Flee and Idle (handled outside the
    // disposition selection layer) and any zero-scoring actions (the legacy
    // `aggregate_to_dispositions` also drops zero-scoring dispositions).
    let mut pool: Vec<(Action, f32)> = scores
        .iter()
        .filter(|(a, s)| !matches!(a, Action::Flee | Action::Idle) && *s > 0.0)
        .copied()
        .collect();

    // Port of the legacy disposition-level Independence penalty on
    // Coordinating + Socializing peer-groups. Applied at action level here
    // on the constituent actions of those dispositions. Groom routes to
    // Socializing when `self_groom_won == false` and gets the penalty in
    // that case only — matching legacy behavior where the penalty landed
    // on `DispositionKind::Socializing` post-aggregation.
    if independence > 0.0 {
        let penalty = independence * disposition_independence_penalty;
        for (action, score) in pool.iter_mut() {
            let penalize = matches!(
                action,
                Action::Coordinate | Action::Socialize | Action::Mentor
            ) || (*action == Action::Groom && !self_groom_won);
            if penalize {
                *score = (*score - penalty).max(0.0);
            }
        }
        // After penalty, drop anything that just dropped to 0.
        pool.retain(|(_, s)| *s > 0.0);
    }

    if pool.is_empty() {
        if let Some(sink) = sink.as_mut() {
            sink.temperature = sc.intention_softmax_temperature;
            sink.empty_pool = true;
        }
        return DispositionKind::Resting;
    }

    // Softmax over the flat Intention pool. Runs the standard max-shift
    // Boltzmann sampler at `intention_softmax_temperature`.
    let temperature = sc.intention_softmax_temperature;
    let max_score = pool
        .iter()
        .map(|(_, s)| *s)
        .fold(f32::NEG_INFINITY, f32::max);
    let weights: Vec<f32> = pool
        .iter()
        .map(|(_, s)| ((s - max_score) / temperature).exp())
        .collect();
    let total: f32 = weights.iter().sum();

    let raw_roll: f32 = rng.random::<f32>();
    let mut roll = raw_roll * total;
    let mut chosen_idx = pool.len() - 1;
    for (i, w) in weights.iter().enumerate() {
        roll -= w;
        if roll <= 0.0 {
            chosen_idx = i;
            break;
        }
    }
    let chosen_action = pool[chosen_idx].0;

    if let Some(sink) = sink {
        sink.temperature = temperature;
        sink.pool = pool.clone();
        sink.weights = weights;
        // Probabilities = weights / total (guarded against total == 0
        // by the empty-pool check above; Boltzmann weights with finite
        // scores always sum > 0).
        sink.probabilities = sink.weights.iter().map(|w| w / total).collect();
        sink.raw_roll = raw_roll;
        sink.chosen_idx = Some(chosen_idx);
        sink.chosen_action = Some(chosen_action);
        sink.empty_pool = false;
    }

    // Map the winning Intention → Disposition. Groom routes via
    // `self_groom_won`; all other dispositioned actions have a 1:1 mapping.
    if chosen_action == Action::Groom {
        return if self_groom_won {
            DispositionKind::Resting
        } else {
            DispositionKind::Socializing
        };
    }
    DispositionKind::from_action(chosen_action).unwrap_or(DispositionKind::Resting)
}

/// Captured snapshot of one softmax selection for §11.3 L3 replay.
///
/// `pool` / `weights` / `probabilities` are parallel arrays of length
/// N (the surviving Intentions); `raw_roll` is the `rng.random::<f32>()`
/// draw before scaling by the weight-sum so reruns can reproduce the
/// pick deterministically; `chosen_idx` indexes into the three arrays.
///
/// `empty_pool == true` signals the fallthrough case where every
/// Intention scored 0 — the caller returned `DispositionKind::Resting`
/// without invoking the RNG. Keeping the default `Vec::new()` shapes
/// lets consumers distinguish "softmax ran and picked" from
/// "softmax skipped entirely".
#[derive(Debug, Default, Clone)]
pub struct SoftmaxCapture {
    pub pool: Vec<(Action, f32)>,
    pub weights: Vec<f32>,
    pub probabilities: Vec<f32>,
    pub temperature: f32,
    pub raw_roll: f32,
    pub chosen_idx: Option<usize>,
    pub chosen_action: Option<Action>,
    pub empty_pool: bool,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::markers;
    use rand_chacha::rand_core::SeedableRng;
    use rand_chacha::ChaCha8Rng;
    use std::sync::OnceLock;

    fn seeded_rng(seed: u64) -> ChaCha8Rng {
        ChaCha8Rng::seed_from_u64(seed)
    }

    // --- Shared DseRegistry + ModifierPipeline for tests ---
    //
    // Each score_actions call needs an EvalInputs bundle with refs to
    // a live DseRegistry (carrying the ported DSEs — currently EatDse
    // only) and a ModifierPipeline (empty until Phase 3c ports the
    // §3.5.1 modifier catalog). Building these per-test is verbose;
    // we cache them via `OnceLock` and return fresh `EvalInputs`
    // pointing at the cached instances. Each test gets a clean
    // identity (cat=dummy entity, position=origin, tick=0) — tests
    // that need a specific entity/position override the relevant
    // field before calling `score_actions`.

    fn cached_registry() -> &'static DseRegistry {
        static REG: OnceLock<DseRegistry> = OnceLock::new();
        REG.get_or_init(|| {
            let scoring =
                crate::resources::sim_constants::ScoringConstants::default();
            let mut r = DseRegistry::new();
            r.cat_dses.push(crate::ai::dses::eat_dse());
            r.cat_dses.push(crate::ai::dses::hunt_dse());
            r.cat_dses.push(crate::ai::dses::forage_dse());
            r.cat_dses.push(crate::ai::dses::cook_dse());
            r.cat_dses.push(crate::ai::dses::flee_dse(&scoring));
            r.cat_dses.push(crate::ai::dses::fight_dse(&scoring));
            r.cat_dses.push(crate::ai::dses::sleep_dse(&scoring));
            r.cat_dses.push(crate::ai::dses::idle_dse(&scoring));
            r.cat_dses.push(crate::ai::dses::socialize_dse());
            r.cat_dses.push(crate::ai::dses::groom_self_dse());
            r.cat_dses.push(crate::ai::dses::groom_other_dse());
            r.cat_dses.push(crate::ai::dses::mentor_dse());
            r.cat_dses.push(crate::ai::dses::caretake_dse());
            r.cat_dses.push(crate::ai::dses::mate_dse());
            r.cat_dses.push(crate::ai::dses::patrol_dse(&scoring));
            r.cat_dses.push(crate::ai::dses::build_dse(&scoring));
            r.cat_dses.push(crate::ai::dses::farm_dse());
            r.cat_dses.push(crate::ai::dses::coordinate_dse(&scoring));
            r.cat_dses.push(crate::ai::dses::explore_dse());
            r.cat_dses.push(crate::ai::dses::wander_dse(&scoring));
            r.cat_dses.push(crate::ai::dses::herbcraft_gather_dse());
            r.cat_dses.push(crate::ai::dses::herbcraft_prepare_dse());
            r.cat_dses.push(crate::ai::dses::herbcraft_ward_dse());
            r.cat_dses.push(crate::ai::dses::scry_dse());
            r.cat_dses.push(crate::ai::dses::durable_ward_dse());
            r.cat_dses.push(crate::ai::dses::cleanse_dse(&scoring));
            r.cat_dses.push(crate::ai::dses::colony_cleanse_dse());
            r.cat_dses.push(crate::ai::dses::harvest_dse());
            r.cat_dses.push(crate::ai::dses::commune_dse());
            r
        })
    }

    fn cached_modifier_pipeline() -> &'static ModifierPipeline {
        static PIPELINE: OnceLock<ModifierPipeline> = OnceLock::new();
        PIPELINE.get_or_init(|| {
            // Match the production pipeline shape — the seven §3.5.1
            // foundational modifiers. The three corruption-emergency
            // modifiers retired in §13.1; their contribution now lives
            // in the axis-level Logistic curves on the corresponding
            // sibling DSEs.
            let scoring = crate::resources::sim_constants::ScoringConstants::default();
            crate::ai::modifier::default_modifier_pipeline(&scoring)
        })
    }

    fn cached_test_markers() -> &'static MarkerSnapshot {
        static M: OnceLock<MarkerSnapshot> = OnceLock::new();
        M.get_or_init(|| {
            // Default snapshot for scoring tests: every ported §4
            // colony marker is set so the corresponding DSE's
            // `.require(...)` eligibility gate opens. Tests that
            // explicitly check an absence path override `markers` on
            // the returned `EvalInputs`.
            let mut s = MarkerSnapshot::new();
            s.set_colony(markers::HasStoredFood::KEY, true);
            s.set_colony(markers::HasGarden::KEY, true);
            // Phase 4b.5 additions.
            s.set_colony(markers::HasFunctionalKitchen::KEY, true);
            s.set_colony(markers::HasRawFoodInStores::KEY, true);
            s.set_colony(markers::WardStrengthLow::KEY, true);
            s
        })
    }

    fn test_eval_inputs() -> EvalInputs<'static> {
        EvalInputs {
            cat: Entity::from_raw_u32(1).unwrap(),
            position: Position::new(0, 0),
            tick: 0,
            dse_registry: cached_registry(),
            modifier_pipeline: cached_modifier_pipeline(),
            markers: cached_test_markers(),
            focal_cat: None,
            focal_capture: None,
        }
    }

    fn default_personality() -> Personality {
        Personality {
            boldness: 0.5,
            sociability: 0.5,
            curiosity: 0.5,
            diligence: 0.5,
            warmth: 0.5,
            spirituality: 0.5,
            ambition: 0.5,
            patience: 0.5,
            anxiety: 0.5,
            optimism: 0.5,
            temper: 0.5,
            stubbornness: 0.5,
            playfulness: 0.5,
            loyalty: 0.5,
            tradition: 0.5,
            compassion: 0.5,
            pride: 0.5,
            independence: 0.5,
        }
    }

    fn default_scoring() -> ScoringConstants {
        ScoringConstants::default()
    }

    fn ctx<'a>(
        needs: &'a Needs,
        personality: &'a Personality,
        scoring: &'a ScoringConstants,
    ) -> ScoringContext<'a> {
        ScoringContext {
            scoring,
            needs,
            personality,
            food_available: true,
            can_hunt: true,
            can_forage: true,
            has_social_target: false,
            has_threat_nearby: false,
            allies_fighting_threat: 0,
            combat_effective: 0.05,
            is_incapacitated: false,
            has_construction_site: false,
            has_damaged_building: false,
            has_garden: false,
            food_fraction: 0.4,
            magic_affinity: 0.0,
            magic_skill: 0.0,
            herbcraft_skill: 0.0,
            has_herbs_nearby: false,
            has_herbs_in_inventory: false,
            has_remedy_herbs: false,
            has_ward_herbs: false,
            thornbriar_available: false,
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            tradition_location_bonus: 0.0,
            has_eligible_mate: false,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            caretake_compassion_bond_scale: 1.0,
            unexplored_nearby: 1.0,
            health: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
        }
    }

    /// Starving cat (hunger=0.1, energy=0.8) with food available should score Eat highest.
    #[test]
    fn starving_cat_scores_eat_highest() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.1;
        needs.energy = 0.8;

        let personality = default_personality();
        let mut rng = seeded_rng(1);

        let scores = score_actions(&ctx(&needs, &personality, &sc), &test_eval_inputs(), &mut rng).scores;
        let best = select_best_action(&scores);

        assert_eq!(
            best,
            Action::Eat,
            "starving cat should choose Eat; scores: {scores:?}"
        );

        // Confirm Eat is also strictly above Sleep
        let eat_score = scores.iter().find(|(a, _)| *a == Action::Eat).unwrap().1;
        let sleep_score = scores.iter().find(|(a, _)| *a == Action::Sleep).unwrap().1;
        assert!(
            eat_score > sleep_score,
            "Eat ({eat_score}) should beat Sleep ({sleep_score}) for a starving cat"
        );
    }

    /// Exhausted cat (energy=0.1, hunger=0.8) should score Sleep highest.
    #[test]
    fn exhausted_cat_scores_sleep_highest() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.energy = 0.1;
        needs.hunger = 0.8;

        let personality = default_personality();
        let mut rng = seeded_rng(2);

        let scores = score_actions(&ctx(&needs, &personality, &sc), &test_eval_inputs(), &mut rng).scores;
        let best = select_best_action(&scores);

        assert_eq!(
            best,
            Action::Sleep,
            "exhausted cat should choose Sleep; scores: {scores:?}"
        );

        let sleep_score = scores.iter().find(|(a, _)| *a == Action::Sleep).unwrap().1;
        let eat_score = scores.iter().find(|(a, _)| *a == Action::Eat).unwrap().1;
        assert!(
            sleep_score > eat_score,
            "Sleep ({sleep_score}) should beat Eat ({eat_score}) for an exhausted cat"
        );
    }

    /// Satisfied curious cat (all needs high, high curiosity) with no food available should
    /// not pick Eat or Sleep — Wander, Explore, or Idle should win.
    #[test]
    fn satisfied_curious_cat_does_not_eat_or_sleep() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        // All needs well-met
        needs.hunger = 0.95;
        needs.energy = 0.95;
        needs.temperature = 0.95;
        needs.safety = 0.95;
        needs.social = 0.95;
        needs.acceptance = 0.95;
        needs.respect = 0.95;
        needs.mastery = 0.95;
        needs.purpose = 0.95;

        let mut personality = default_personality();
        personality.curiosity = 0.9; // highly curious

        let mut rng = seeded_rng(3);

        // No food, no hunt/forage targets — only Wander/Idle/Sleep/Groom/Explore available
        let c = ScoringContext {
            scoring: &sc,
            needs: &needs,
            personality: &personality,
            food_available: false,
            can_hunt: false,
            can_forage: false,
            has_social_target: false,
            has_threat_nearby: false,
            allies_fighting_threat: 0,
            combat_effective: 0.05,
            is_incapacitated: false,
            has_construction_site: false,
            has_damaged_building: false,
            has_garden: false,
            food_fraction: 0.4,
            magic_affinity: 0.0,
            magic_skill: 0.0,
            herbcraft_skill: 0.0,
            has_herbs_nearby: false,
            has_herbs_in_inventory: false,
            has_remedy_herbs: false,
            has_ward_herbs: false,
            thornbriar_available: false,
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            tradition_location_bonus: 0.0,
            has_eligible_mate: false,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            caretake_compassion_bond_scale: 1.0,
            unexplored_nearby: 1.0,
            health: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
        };
        let scores = score_actions(&c, &test_eval_inputs(), &mut rng).scores;
        let best = select_best_action(&scores);

        assert!(
            best == Action::Wander || best == Action::Idle || best == Action::Explore,
            "satisfied cat should wander, explore, or idle, got {best:?}; scores: {scores:?}"
        );
        assert_ne!(best, Action::Eat, "no food available, Eat should not win");
        assert_ne!(
            best,
            Action::Sleep,
            "well-rested cat should not sleep; scores: {scores:?}"
        );
    }

    /// A bold hungry cat with hunt available should prefer Hunt over Forage.
    #[test]
    fn bold_cat_prefers_hunt_over_forage() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.2;

        let mut personality = default_personality();
        personality.boldness = 0.9;
        personality.diligence = 0.3;

        let mut rng = seeded_rng(10);

        let scores = score_actions(&ctx(&needs, &personality, &sc), &test_eval_inputs(), &mut rng).scores;
        let hunt_score = scores.iter().find(|(a, _)| *a == Action::Hunt).unwrap().1;
        let forage_score = scores.iter().find(|(a, _)| *a == Action::Forage).unwrap().1;

        assert!(
            hunt_score > forage_score,
            "bold cat should prefer Hunt ({hunt_score}) over Forage ({forage_score})"
        );
    }

    /// A diligent non-bold cat should prefer Forage over Hunt.
    #[test]
    fn diligent_cat_prefers_forage_over_hunt() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.2;

        let mut personality = default_personality();
        personality.boldness = 0.2;
        personality.diligence = 0.9;

        let mut rng = seeded_rng(11);

        let scores = score_actions(&ctx(&needs, &personality, &sc), &test_eval_inputs(), &mut rng).scores;
        let hunt_score = scores.iter().find(|(a, _)| *a == Action::Hunt).unwrap().1;
        let forage_score = scores.iter().find(|(a, _)| *a == Action::Forage).unwrap().1;

        assert!(
            forage_score > hunt_score,
            "diligent cat should prefer Forage ({forage_score}) over Hunt ({hunt_score})"
        );
    }

    /// A lonely social cat with a visible target should score Socialize highly.
    #[test]
    fn lonely_social_cat_scores_socialize_high() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.social = 0.1; // very lonely
        needs.hunger = 0.9;
        needs.energy = 0.9;
        needs.temperature = 0.9;

        let mut personality = default_personality();
        personality.sociability = 0.9;

        let mut rng = seeded_rng(20);

        let c = ScoringContext {
            scoring: &sc,
            needs: &needs,
            personality: &personality,
            food_available: true,
            can_hunt: true,
            can_forage: true,
            has_social_target: true,
            has_threat_nearby: false,
            allies_fighting_threat: 0,
            combat_effective: 0.05,
            is_incapacitated: false,
            has_construction_site: false,
            has_damaged_building: false,
            has_garden: false,
            food_fraction: 0.4,
            magic_affinity: 0.0,
            magic_skill: 0.0,
            herbcraft_skill: 0.0,
            has_herbs_nearby: false,
            has_herbs_in_inventory: false,
            has_remedy_herbs: false,
            has_ward_herbs: false,
            thornbriar_available: false,
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            tradition_location_bonus: 0.0,
            has_eligible_mate: false,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            caretake_compassion_bond_scale: 1.0,
            unexplored_nearby: 1.0,
            health: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
        };
        let scores = score_actions(&c, &test_eval_inputs(), &mut rng).scores;
        let socialize_score = scores
            .iter()
            .find(|(a, _)| *a == Action::Socialize)
            .unwrap()
            .1;
        let idle_score = scores.iter().find(|(a, _)| *a == Action::Idle).unwrap().1;

        assert!(
            socialize_score > idle_score,
            "lonely social cat should score Socialize ({socialize_score}) above Idle ({idle_score})"
        );
    }

    /// Cold cat should score Groom highly (self-groom for warmth).
    #[test]
    fn cold_cat_scores_groom_high() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.temperature = 0.1;
        needs.hunger = 0.9;
        needs.energy = 0.9;

        let personality = default_personality();
        let mut rng = seeded_rng(21);

        let scores = score_actions(&ctx(&needs, &personality, &sc), &test_eval_inputs(), &mut rng).scores;
        let groom_score = scores.iter().find(|(a, _)| *a == Action::Groom).unwrap().1;
        let idle_score = scores.iter().find(|(a, _)| *a == Action::Idle).unwrap().1;

        assert!(
            groom_score > idle_score,
            "cold cat should score Groom ({groom_score}) above Idle ({idle_score})"
        );
    }

    // --- Memory bonus tests ---

    use crate::components::mental::{Memory, MemoryEntry, MemoryType};
    use crate::components::physical::Position;
    use std::collections::HashMap;

    fn make_memory(event_type: MemoryType, location: Position, strength: f32) -> MemoryEntry {
        MemoryEntry {
            event_type,
            location: Some(location),
            involved: vec![],
            tick: 0,
            strength,
            firsthand: true,
        }
    }

    #[test]
    fn resource_memory_boosts_hunt_score() {
        let sc = default_scoring();
        let mut scores = vec![
            (Action::Hunt, 1.0),
            (Action::Forage, 1.0),
            (Action::Idle, 0.5),
        ];
        let mut memory = Memory::default();
        memory.remember(make_memory(
            MemoryType::ResourceFound,
            Position::new(5, 5),
            1.0,
        ));

        // Cat at (5, 5) — same tile as remembered resource.
        apply_memory_bonuses(&mut scores, &memory, &Position::new(5, 5), &sc);

        let hunt = scores.iter().find(|(a, _)| *a == Action::Hunt).unwrap().1;
        let idle = scores.iter().find(|(a, _)| *a == Action::Idle).unwrap().1;
        assert!(
            hunt > 1.0,
            "Hunt should be boosted above base 1.0; got {hunt}"
        );
        assert_eq!(idle, 0.5, "Idle should be unaffected; got {idle}");
    }

    #[test]
    fn death_memory_suppresses_wander() {
        let sc = default_scoring();
        let mut scores = vec![(Action::Wander, 1.0), (Action::Hunt, 1.0)];
        let mut memory = Memory::default();
        memory.remember(make_memory(MemoryType::Death, Position::new(5, 5), 1.0));

        apply_memory_bonuses(&mut scores, &memory, &Position::new(5, 5), &sc);

        let wander = scores.iter().find(|(a, _)| *a == Action::Wander).unwrap().1;
        let hunt = scores.iter().find(|(a, _)| *a == Action::Hunt).unwrap().1;
        assert!(
            wander < 1.0,
            "Wander should be suppressed near death site; got {wander}"
        );
        assert_eq!(
            hunt, 1.0,
            "Hunt should be unaffected by death memory; got {hunt}"
        );
    }

    #[test]
    fn distant_memories_have_less_effect() {
        let sc = default_scoring();
        let mut scores_near = vec![(Action::Hunt, 1.0)];
        let mut scores_far = vec![(Action::Hunt, 1.0)];
        let mut memory = Memory::default();
        memory.remember(make_memory(
            MemoryType::ResourceFound,
            Position::new(5, 5),
            1.0,
        ));

        apply_memory_bonuses(&mut scores_near, &memory, &Position::new(5, 5), &sc);
        apply_memory_bonuses(&mut scores_far, &memory, &Position::new(15, 5), &sc);

        let near = scores_near[0].1;
        let far = scores_far[0].1;
        assert!(
            near > far,
            "nearby memory should give bigger boost; near={near}, far={far}"
        );
    }

    #[test]
    fn decayed_memories_have_less_effect() {
        let sc = default_scoring();
        let mut scores_strong = vec![(Action::Hunt, 1.0)];
        let mut scores_weak = vec![(Action::Hunt, 1.0)];
        let mut memory_strong = Memory::default();
        memory_strong.remember(make_memory(
            MemoryType::ResourceFound,
            Position::new(5, 5),
            1.0,
        ));
        let mut memory_weak = Memory::default();
        memory_weak.remember(make_memory(
            MemoryType::ResourceFound,
            Position::new(5, 5),
            0.2,
        ));

        apply_memory_bonuses(
            &mut scores_strong,
            &memory_strong,
            &Position::new(5, 5),
            &sc,
        );
        apply_memory_bonuses(&mut scores_weak, &memory_weak, &Position::new(5, 5), &sc);

        let strong = scores_strong[0].1;
        let weak = scores_weak[0].1;
        assert!(
            strong > weak,
            "strong memory should give bigger boost; strong={strong}, weak={weak}"
        );
    }

    // --- Activity cascading tests ---

    #[test]
    fn cascading_boosts_matching_action() {
        let sc = default_scoring();
        let mut scores = vec![(Action::Hunt, 1.0), (Action::Idle, 0.5)];
        let nearby = HashMap::from([(Action::Hunt, 3)]);

        apply_cascading_bonuses(&mut scores, &nearby, &sc);

        let hunt = scores.iter().find(|(a, _)| *a == Action::Hunt).unwrap().1;
        assert!(
            (hunt - 1.24).abs() < 1e-5,
            "3 nearby hunters should add 0.24; got {hunt}"
        );
    }

    #[test]
    fn cascading_does_not_boost_unrelated_actions() {
        let sc = default_scoring();
        let mut scores = vec![(Action::Hunt, 1.0), (Action::Sleep, 0.5)];
        let nearby = HashMap::from([(Action::Hunt, 2)]);

        apply_cascading_bonuses(&mut scores, &nearby, &sc);

        let sleep = scores.iter().find(|(a, _)| *a == Action::Sleep).unwrap().1;
        assert_eq!(sleep, 0.5, "Sleep should be unaffected; got {sleep}");
    }

    // --- Flee / Fight / Patrol scoring tests ---

    #[test]
    fn cautious_cat_flees_when_threatened() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.safety = 0.2;

        let mut personality = default_personality();
        personality.boldness = 0.2; // cautious

        let mut rng = seeded_rng(30);

        let c = ScoringContext {
            scoring: &sc,
            needs: &needs,
            personality: &personality,
            food_available: true,
            can_hunt: true,
            can_forage: true,
            has_social_target: false,
            has_threat_nearby: true,
            allies_fighting_threat: 0,
            combat_effective: 0.15,
            is_incapacitated: false,
            has_construction_site: false,
            has_damaged_building: false,
            has_garden: false,
            food_fraction: 0.4,
            magic_affinity: 0.0,
            magic_skill: 0.0,
            herbcraft_skill: 0.0,
            has_herbs_nearby: false,
            has_herbs_in_inventory: false,
            has_remedy_herbs: false,
            has_ward_herbs: false,
            thornbriar_available: false,
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            tradition_location_bonus: 0.0,
            has_eligible_mate: false,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            caretake_compassion_bond_scale: 1.0,
            unexplored_nearby: 1.0,
            health: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
        };
        let scores = score_actions(&c, &test_eval_inputs(), &mut rng).scores;
        let best = select_best_action(&scores);

        assert_eq!(
            best,
            Action::Flee,
            "cautious cat with low safety should flee; scores: {scores:?}"
        );
    }

    #[test]
    fn bold_cat_fights_when_allies_present() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.safety = 0.3;

        let mut personality = default_personality();
        personality.boldness = 0.9;

        let mut rng = seeded_rng(31);

        let c = ScoringContext {
            scoring: &sc,
            needs: &needs,
            personality: &personality,
            food_available: true,
            can_hunt: true,
            can_forage: true,
            has_social_target: false,
            has_threat_nearby: true,
            allies_fighting_threat: 2,
            combat_effective: 0.35, // experienced hunter
            is_incapacitated: false,
            has_construction_site: false,
            has_damaged_building: false,
            has_garden: false,
            food_fraction: 0.4,
            magic_affinity: 0.0,
            magic_skill: 0.0,
            herbcraft_skill: 0.0,
            has_herbs_nearby: false,
            has_herbs_in_inventory: false,
            has_remedy_herbs: false,
            has_ward_herbs: false,
            thornbriar_available: false,
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            tradition_location_bonus: 0.0,
            has_eligible_mate: false,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            caretake_compassion_bond_scale: 1.0,
            unexplored_nearby: 1.0,
            health: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
        };
        let scores = score_actions(&c, &test_eval_inputs(), &mut rng).scores;
        let fight_score = scores.iter().find(|(a, _)| *a == Action::Fight).unwrap().1;
        let flee_score = scores.iter().find(|(a, _)| *a == Action::Flee);

        assert!(
            fight_score > 0.3,
            "bold cat with allies should have meaningful fight score; got {fight_score}"
        );
        // Bold cat shouldn't flee.
        if let Some((_, fs)) = flee_score {
            assert!(
                fight_score > *fs,
                "bold cat should prefer fight ({fight_score}) over flee ({fs})"
            );
        }
    }

    #[test]
    fn incapacitated_cat_only_scores_basic_actions() {
        let sc = default_scoring();
        // §13.1: the inline `if ctx.is_incapacitated` early-return
        // retired — incapacitation now flows through the §4.3
        // `Incapacitated` marker and `.forbid("Incapacitated")` on
        // every non-Eat/Sleep/Idle cat DSE. This test verifies the
        // `.forbid` filter set gates the same action set the inline
        // branch used to gate.
        //
        // Scenario: a hungry/tired cat so Eat and Sleep have
        // above-jitter scores under the Logistic curves. The §2.3
        // hangry anchor Logistic(8, 0.75) returns ~0 at `hunger=1.0`
        // (sated), which is correct behavior but breaks the
        // branch-coverage assertion this test makes; the adjustment
        // preserves intent while accommodating the curve shape.
        let mut needs = Needs::default();
        needs.hunger = 0.4;
        needs.energy = 0.3;
        let personality = default_personality();
        let mut rng = seeded_rng(40);

        let c = ScoringContext {
            scoring: &sc,
            needs: &needs,
            personality: &personality,
            food_available: true,
            can_hunt: true,
            can_forage: true,
            has_social_target: true,
            has_threat_nearby: true,
            allies_fighting_threat: 0,
            combat_effective: 0.1,
            is_incapacitated: true,
            has_construction_site: false,
            has_damaged_building: false,
            has_garden: false,
            food_fraction: 0.4,
            magic_affinity: 0.0,
            magic_skill: 0.0,
            herbcraft_skill: 0.0,
            has_herbs_nearby: false,
            has_herbs_in_inventory: false,
            has_remedy_herbs: false,
            has_ward_herbs: false,
            thornbriar_available: false,
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            tradition_location_bonus: 0.0,
            has_eligible_mate: false,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            caretake_compassion_bond_scale: 1.0,
            unexplored_nearby: 1.0,
            health: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
        };
        // Build a per-test MarkerSnapshot with Incapacitated set for
        // this cat (the cached shared snapshot only carries colony
        // markers). Copy the colony markers the cached snapshot sets
        // so Eat's `HasStoredFood` requirement still passes.
        let mut markers = MarkerSnapshot::new();
        markers.set_colony(markers::HasStoredFood::KEY, true);
        markers.set_colony(markers::HasGarden::KEY, true);
        markers.set_colony(markers::HasFunctionalKitchen::KEY, true);
        markers.set_colony(markers::HasRawFoodInStores::KEY, true);
        markers.set_colony(markers::WardStrengthLow::KEY, true);
        let cat_entity = Entity::from_raw_u32(1).unwrap();
        markers.set_entity(markers::Incapacitated::KEY, cat_entity, true);

        let base = test_eval_inputs();
        let inputs = EvalInputs {
            cat: cat_entity,
            markers: &markers,
            ..base
        };
        let scores = score_actions(&c, &inputs, &mut rng).scores;
        // §13.1: Hunt/Fight/Flee are forbidden for incapacitated cats
        // via `.forbid("Incapacitated")` on each DSE. `score_dse_by_id`
        // returns 0.0 for a filtered-out DSE, but `score_actions` still
        // pushes `0.0 + jitter(±jitter_range)` into the pool, so a
        // forbidden action can show above-zero from noise alone. The
        // correct invariant is magnitude: eligible actions
        // (Eat/Sleep/Idle) score well above jitter; forbidden actions
        // score at most `jitter_range` in magnitude.
        let jitter_range = sc.jitter_range;
        let get = |action: Action| -> f32 {
            scores.iter().find(|(a, _)| *a == action).map(|(_, s)| *s).unwrap_or(0.0)
        };
        assert!(
            get(Action::Eat) > jitter_range,
            "incapacitated cat should score Eat above jitter (got {})",
            get(Action::Eat)
        );
        assert!(
            get(Action::Sleep) > jitter_range,
            "incapacitated cat should score Sleep above jitter (got {})",
            get(Action::Sleep)
        );
        // Idle no longer carries the retired `incapacitated_idle_score`
        // constant — its urgency now runs through Idle's canonical axes,
        // which score low for a hungry/tired cat. The §13.1 invariant
        // this test guards is the *forbidden* set (Hunt/Fight/Flee), not
        // Idle's score magnitude.
        assert!(
            get(Action::Hunt).abs() <= jitter_range,
            "incapacitated cat's Hunt must be at most jitter-range (got {}, jitter {})",
            get(Action::Hunt),
            jitter_range
        );
        assert!(
            get(Action::Fight).abs() <= jitter_range,
            "incapacitated cat's Fight must be at most jitter-range (got {}, jitter {})",
            get(Action::Fight),
            jitter_range
        );
        assert!(
            get(Action::Flee).abs() <= jitter_range,
            "incapacitated cat's Flee must be at most jitter-range (got {}, jitter {})",
            get(Action::Flee),
            jitter_range
        );
    }

    #[test]
    fn threat_memory_suppresses_wander_near_threat() {
        let sc = default_scoring();
        let mut scores = vec![
            (Action::Wander, 1.0),
            (Action::Explore, 1.0),
            (Action::Hunt, 1.0),
            (Action::Idle, 0.5),
        ];
        let mut memory = Memory::default();
        memory.remember(make_memory(
            MemoryType::ThreatSeen,
            Position::new(5, 5),
            1.0,
        ));

        apply_memory_bonuses(&mut scores, &memory, &Position::new(5, 5), &sc);

        let wander = scores.iter().find(|(a, _)| *a == Action::Wander).unwrap().1;
        let explore = scores
            .iter()
            .find(|(a, _)| *a == Action::Explore)
            .unwrap()
            .1;
        let hunt = scores.iter().find(|(a, _)| *a == Action::Hunt).unwrap().1;
        let idle = scores.iter().find(|(a, _)| *a == Action::Idle).unwrap().1;

        assert!(
            wander < 1.0,
            "wander should be suppressed near threat; got {wander}"
        );
        assert!(
            explore < 1.0,
            "explore should be suppressed near threat; got {explore}"
        );
        assert!(
            hunt < 1.0,
            "hunt should be suppressed near threat; got {hunt}"
        );
        assert_eq!(idle, 0.5, "idle should be unaffected; got {idle}");
    }

    // --- Herbcraft / PracticeMagic scoring tests ---

    #[test]
    fn spiritual_cat_with_herbs_nearby_scores_herbcraft() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.8;
        needs.energy = 0.8;

        let mut personality = default_personality();
        personality.spirituality = 0.9;

        let mut rng = seeded_rng(50);

        let mut c = ctx(&needs, &personality, &sc);
        c.has_herbs_nearby = true;
        c.herbcraft_skill = 0.3;

        let scores = score_actions(&c, &test_eval_inputs(), &mut rng).scores;
        let herbcraft = scores.iter().find(|(a, _)| *a == Action::Herbcraft);

        assert!(
            herbcraft.is_some(),
            "spiritual cat with herbs nearby should score Herbcraft"
        );
        assert!(
            herbcraft.unwrap().1 > 0.0,
            "Herbcraft score should be positive"
        );
    }

    #[test]
    fn herbcraft_not_scored_without_herbs_or_inventory() {
        let sc = default_scoring();
        let needs = Needs::default();
        let personality = default_personality();
        let mut rng = seeded_rng(51);

        let c = ctx(&needs, &personality, &sc);
        // no herbs nearby, no inventory herbs

        let scores = score_actions(&c, &test_eval_inputs(), &mut rng).scores;
        let herbcraft = scores.iter().find(|(a, _)| *a == Action::Herbcraft);
        assert!(
            herbcraft.is_none(),
            "no herbs → no Herbcraft; scores: {scores:?}"
        );
    }

    #[test]
    fn practice_magic_requires_prereqs() {
        let sc = default_scoring();
        let needs = Needs::default();
        let personality = default_personality();
        let mut rng = seeded_rng(52);

        // Below prereqs: affinity 0.2 < 0.3 threshold
        let mut c = ctx(&needs, &personality, &sc);
        c.magic_affinity = 0.2;
        c.magic_skill = 0.3;

        let scores = score_actions(&c, &test_eval_inputs(), &mut rng).scores;
        let magic = scores.iter().find(|(a, _)| *a == Action::PracticeMagic);
        assert!(
            magic.is_none(),
            "below affinity threshold → no PracticeMagic"
        );

        // Below prereqs: skill 0.1 < 0.2 threshold
        let mut c2 = ctx(&needs, &personality, &sc);
        c2.magic_affinity = 0.5;
        c2.magic_skill = 0.1;

        let scores2 = score_actions(&c2, &test_eval_inputs(), &mut rng).scores;
        let magic2 = scores2.iter().find(|(a, _)| *a == Action::PracticeMagic);
        assert!(magic2.is_none(), "below skill threshold → no PracticeMagic");
    }

    #[test]
    fn magical_cat_on_corrupted_tile_scores_practice_magic() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.8;
        needs.energy = 0.8;

        let mut personality = default_personality();
        personality.spirituality = 0.8;

        let mut rng = seeded_rng(53);

        let mut c = ctx(&needs, &personality, &sc);
        c.magic_affinity = 0.6;
        c.magic_skill = 0.4;
        c.on_corrupted_tile = true;
        c.tile_corruption = 0.5;

        let scores = score_actions(&c, &test_eval_inputs(), &mut rng).scores;
        let magic = scores.iter().find(|(a, _)| *a == Action::PracticeMagic);

        assert!(
            magic.is_some(),
            "magical cat on corrupted tile should score PracticeMagic"
        );
        assert!(
            magic.unwrap().1 > 0.0,
            "PracticeMagic score should be positive"
        );
    }

    #[test]
    fn compassionate_cat_with_remedy_herbs_and_injured_ally_scores_herbcraft() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.8;
        needs.energy = 0.8;

        let mut personality = default_personality();
        personality.compassion = 0.9;

        let mut rng = seeded_rng(54);

        let mut c = ctx(&needs, &personality, &sc);
        c.has_remedy_herbs = true;
        c.has_herbs_in_inventory = true;
        c.herbcraft_skill = 0.4;
        c.colony_injury_count = 2;

        let scores = score_actions(&c, &test_eval_inputs(), &mut rng).scores;
        let herbcraft = scores.iter().find(|(a, _)| *a == Action::Herbcraft);

        assert!(
            herbcraft.is_some() && herbcraft.unwrap().1 > 0.15,
            "compassionate cat with remedy herbs and injured allies should score Herbcraft; got {herbcraft:?}"
        );
    }

    /// Average cat with met needs should pick Wander over Idle.
    #[test]
    fn wander_beats_idle_for_average_cat() {
        let sc = default_scoring();
        let needs = Needs::default();
        let personality = default_personality();
        let mut rng = seeded_rng(42);

        let c = ScoringContext {
            scoring: &sc,
            needs: &needs,
            personality: &personality,
            food_available: false,
            can_hunt: false,
            can_forage: false,
            has_social_target: false,
            has_threat_nearby: false,
            allies_fighting_threat: 0,
            combat_effective: 0.05,
            is_incapacitated: false,
            has_construction_site: false,
            has_damaged_building: false,
            has_garden: false,
            food_fraction: 0.5,
            magic_affinity: 0.0,
            magic_skill: 0.0,
            herbcraft_skill: 0.0,
            has_herbs_nearby: false,
            has_herbs_in_inventory: false,
            has_remedy_herbs: false,
            has_ward_herbs: false,
            thornbriar_available: false,
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            tradition_location_bonus: 0.0,
            has_eligible_mate: false,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            caretake_compassion_bond_scale: 1.0,
            unexplored_nearby: 1.0,
            health: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
        };
        let scores = score_actions(&c, &test_eval_inputs(), &mut rng).scores;
        let wander = scores.iter().find(|(a, _)| *a == Action::Wander).unwrap().1;
        let idle = scores.iter().find(|(a, _)| *a == Action::Idle).unwrap().1;
        assert!(
            wander > idle,
            "Wander ({wander:.3}) should beat Idle ({idle:.3}) for an average cat"
        );
    }

    /// Low food stores should boost Hunt score even when the cat isn't personally hungry.
    #[test]
    fn low_food_stores_boost_hunt_score() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.9; // not personally hungry
        needs.energy = 0.9;

        let mut personality = default_personality();
        personality.boldness = 0.6;

        let mut rng_full = seeded_rng(50);
        let mut rng_low = seeded_rng(50);

        let base = ScoringContext {
            scoring: &sc,
            needs: &needs,
            personality: &personality,
            food_available: true,
            can_hunt: true,
            can_forage: false,
            has_social_target: false,
            has_threat_nearby: false,
            allies_fighting_threat: 0,
            combat_effective: 0.05,
            is_incapacitated: false,
            has_construction_site: false,
            has_damaged_building: false,
            has_garden: false,
            food_fraction: 0.9,
            magic_affinity: 0.0,
            magic_skill: 0.0,
            herbcraft_skill: 0.0,
            has_herbs_nearby: false,
            has_herbs_in_inventory: false,
            has_remedy_herbs: false,
            has_ward_herbs: false,
            thornbriar_available: false,
            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            has_mentoring_target: false,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            tradition_location_bonus: 0.0,
            has_eligible_mate: false,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            caretake_compassion_bond_scale: 1.0,
            unexplored_nearby: 1.0,
            health: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
        };

        let scores_full = score_actions(&base, &test_eval_inputs(), &mut rng_full).scores;
        let hunt_full = scores_full
            .iter()
            .find(|(a, _)| *a == Action::Hunt)
            .unwrap()
            .1;

        let low = ScoringContext {
            food_fraction: 0.2,
            ..base
        };
        let scores_low = score_actions(&low, &test_eval_inputs(), &mut rng_low).scores;
        let hunt_low = scores_low
            .iter()
            .find(|(a, _)| *a == Action::Hunt)
            .unwrap()
            .1;

        assert!(
            hunt_low > hunt_full,
            "Hunt with low stores ({hunt_low:.3}) should exceed Hunt with full stores ({hunt_full:.3})"
        );
    }

    // --- Survival floor tests ---

    #[test]
    fn survival_floor_caps_build_when_starving() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.1; // critically hungry
        needs.energy = 0.9;
        needs.temperature = 0.9;

        // Simulate a bonus-inflated Build score beating Eat.
        let mut scores = vec![
            (Action::Eat, 1.6),   // hungry cat, high Eat score
            (Action::Build, 2.5), // bonus-inflated Build
            (Action::Idle, 0.1),
        ];

        enforce_survival_floor(&mut scores, &needs, &sc);

        let eat = scores.iter().find(|(a, _)| *a == Action::Eat).unwrap().1;
        let build = scores.iter().find(|(a, _)| *a == Action::Build).unwrap().1;
        assert!(
            eat >= build,
            "starving cat: Eat ({eat:.3}) should >= Build ({build:.3})"
        );
    }

    #[test]
    fn survival_floor_inactive_when_needs_met() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.8;
        needs.energy = 0.8;
        needs.temperature = 0.8;

        let mut scores = vec![
            (Action::Eat, 0.5),
            (Action::Build, 1.5),
            (Action::Idle, 0.1),
        ];
        let build_before = 1.5f32;

        enforce_survival_floor(&mut scores, &needs, &sc);

        let build = scores.iter().find(|(a, _)| *a == Action::Build).unwrap().1;
        assert_eq!(
            build, build_before,
            "well-fed cat: Build should be untouched"
        );
    }

    #[test]
    fn survival_floor_gradual() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.3; // moderately hungry
        needs.energy = 0.9;
        needs.temperature = 0.9;

        let mut scores = vec![
            (Action::Eat, 1.2),
            (Action::Build, 2.0),
            (Action::Idle, 0.1),
        ];

        enforce_survival_floor(&mut scores, &needs, &sc);

        let eat = scores.iter().find(|(a, _)| *a == Action::Eat).unwrap().1;
        let build = scores.iter().find(|(a, _)| *a == Action::Build).unwrap().1;
        // Build should be compressed but not fully capped.
        assert!(
            build < 2.0,
            "moderately hungry: Build ({build:.3}) should be compressed below 2.0"
        );
        assert!(
            build > eat,
            "moderately hungry: Build ({build:.3}) may still beat Eat ({eat:.3}) — partial suppression"
        );
    }

    #[test]
    fn survival_floor_exempts_flee() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.05; // nearly starving
        needs.energy = 0.9;
        needs.temperature = 0.9;

        let mut scores = vec![
            (Action::Eat, 1.8),
            (Action::Flee, 3.0),
            (Action::Build, 2.0),
        ];

        enforce_survival_floor(&mut scores, &needs, &sc);

        let flee = scores.iter().find(|(a, _)| *a == Action::Flee).unwrap().1;
        assert_eq!(flee, 3.0, "Flee should be exempt from survival floor");
    }

    // --- Disposition aggregation tests ---

    #[test]
    fn aggregate_maps_hunt_to_hunting() {
        let scores = vec![
            (Action::Hunt, 1.5),
            (Action::Eat, 1.0),
            (Action::Sleep, 0.8),
        ];
        let disp = aggregate_to_dispositions(&scores, true);
        let hunting = disp.iter().find(|(k, _)| *k == DispositionKind::Hunting);
        assert_eq!(hunting.unwrap().1, 1.5);
    }

    #[test]
    fn aggregate_takes_max_of_constituent_actions() {
        let scores = vec![(Action::Patrol, 0.5), (Action::Fight, 1.2)];
        let disp = aggregate_to_dispositions(&scores, true);
        let guarding = disp.iter().find(|(k, _)| *k == DispositionKind::Guarding);
        assert_eq!(guarding.unwrap().1, 1.2);
    }

    #[test]
    fn aggregate_routes_self_groom_to_resting() {
        let scores = vec![(Action::Groom, 0.9), (Action::Eat, 0.5)];
        let disp = aggregate_to_dispositions(&scores, true);
        let resting = disp.iter().find(|(k, _)| *k == DispositionKind::Resting);
        assert_eq!(resting.unwrap().1, 0.9);
        // Should NOT appear under Socializing
        let socializing = disp
            .iter()
            .find(|(k, _)| *k == DispositionKind::Socializing);
        assert!(socializing.is_none());
    }

    #[test]
    fn aggregate_routes_other_groom_to_socializing() {
        let scores = vec![(Action::Groom, 0.9), (Action::Socialize, 0.5)];
        let disp = aggregate_to_dispositions(&scores, false);
        let socializing = disp
            .iter()
            .find(|(k, _)| *k == DispositionKind::Socializing);
        assert_eq!(socializing.unwrap().1, 0.9);
    }

    #[test]
    fn aggregate_excludes_flee_and_idle() {
        let scores = vec![
            (Action::Flee, 3.0),
            (Action::Idle, 0.1),
            (Action::Hunt, 1.0),
        ];
        let disp = aggregate_to_dispositions(&scores, true);
        assert!(disp.iter().all(|(k, _)| *k != DispositionKind::Resting
            || disp
                .iter()
                .find(|(dk, _)| *dk == DispositionKind::Resting)
                .is_none()));
        // No disposition should have the Flee score
        assert!(disp.iter().all(|(_, s)| *s <= 1.0));
    }

    #[test]
    fn aggregate_omits_zero_score_dispositions() {
        let scores = vec![(Action::Hunt, 1.0)];
        let disp = aggregate_to_dispositions(&scores, true);
        // Only Hunting should appear
        assert_eq!(disp.len(), 1);
        assert_eq!(disp[0].0, DispositionKind::Hunting);
    }

    #[test]
    fn disposition_softmax_returns_resting_for_empty() {
        let sc = default_scoring();
        let mut rng = seeded_rng(42);
        assert_eq!(
            select_disposition_softmax(&[], &mut rng, &sc),
            DispositionKind::Resting,
        );
    }

    // --- MarkerSnapshot tests (§4 marker lookup surface) ---

    #[test]
    fn marker_snapshot_empty_returns_false() {
        let snap = MarkerSnapshot::new();
        let e = Entity::from_raw_u32(1).unwrap();
        assert!(!snap.has(markers::HasStoredFood::KEY, e));
    }

    #[test]
    fn marker_snapshot_colony_marker_true_for_any_entity() {
        let mut snap = MarkerSnapshot::new();
        snap.set_colony(markers::HasStoredFood::KEY, true);
        let a = Entity::from_raw_u32(1).unwrap();
        let b = Entity::from_raw_u32(99).unwrap();
        assert!(snap.has(markers::HasStoredFood::KEY, a));
        assert!(snap.has(markers::HasStoredFood::KEY, b));
        assert!(!snap.has(markers::Incapacitated::KEY, a));
    }

    #[test]
    fn marker_snapshot_colony_marker_clears_cleanly() {
        let mut snap = MarkerSnapshot::new();
        let e = Entity::from_raw_u32(1).unwrap();
        snap.set_colony(markers::HasStoredFood::KEY, true);
        assert!(snap.has(markers::HasStoredFood::KEY, e));
        snap.set_colony(markers::HasStoredFood::KEY, false);
        assert!(!snap.has(markers::HasStoredFood::KEY, e));
    }

    #[test]
    fn marker_snapshot_entity_marker_discriminates_on_entity() {
        let mut snap = MarkerSnapshot::new();
        let a = Entity::from_raw_u32(1).unwrap();
        let b = Entity::from_raw_u32(2).unwrap();
        snap.set_entity(markers::Incapacitated::KEY, a, true);
        assert!(snap.has(markers::Incapacitated::KEY, a));
        assert!(!snap.has(markers::Incapacitated::KEY, b));
    }

    #[test]
    fn marker_snapshot_entity_marker_clear_removes_only_named_cat() {
        let mut snap = MarkerSnapshot::new();
        let a = Entity::from_raw_u32(1).unwrap();
        let b = Entity::from_raw_u32(2).unwrap();
        snap.set_entity(markers::Incapacitated::KEY, a, true);
        snap.set_entity(markers::Incapacitated::KEY, b, true);
        snap.set_entity(markers::Incapacitated::KEY, a, false);
        assert!(!snap.has(markers::Incapacitated::KEY, a));
        assert!(snap.has(markers::Incapacitated::KEY, b));
    }

    // --- Behavior gate tests ---

    #[test]
    fn gate_timid_fight_becomes_flee() {
        let sc = default_scoring();
        let mut p = default_personality();
        p.boldness = 0.05;
        let mut rng = seeded_rng(1);
        assert_eq!(
            behavior_gate_check(Action::Fight, &p, false, 1.0, &mut rng, &sc),
            Some(Action::Flee),
        );
    }

    #[test]
    fn gate_shy_socialize_becomes_idle() {
        let sc = default_scoring();
        let mut p = default_personality();
        p.sociability = 0.1;
        let mut rng = seeded_rng(1);
        assert_eq!(
            behavior_gate_check(Action::Socialize, &p, false, 1.0, &mut rng, &sc),
            Some(Action::Idle),
        );
    }

    #[test]
    fn gate_reckless_flee_becomes_fight() {
        let sc = default_scoring();
        let mut p = default_personality();
        p.boldness = 0.95;
        let mut rng = seeded_rng(1);
        assert_eq!(
            behavior_gate_check(Action::Flee, &p, false, 1.0, &mut rng, &sc),
            Some(Action::Fight),
        );
    }

    #[test]
    fn gate_reckless_flee_not_when_injured() {
        let sc = default_scoring();
        let mut p = default_personality();
        p.boldness = 0.95;
        let mut rng = seeded_rng(1);
        // Below health threshold — reckless gate should NOT fire.
        assert_eq!(
            behavior_gate_check(Action::Flee, &p, false, 0.3, &mut rng, &sc),
            None,
        );
    }

    #[test]
    fn gate_compulsive_helper() {
        let sc = default_scoring();
        let mut p = default_personality();
        p.compassion = 0.95;
        let mut rng = seeded_rng(1);
        assert_eq!(
            behavior_gate_check(Action::Wander, &p, true, 1.0, &mut rng, &sc),
            Some(Action::Herbcraft),
        );
        // No injured nearby → no override.
        assert_eq!(
            behavior_gate_check(Action::Wander, &p, false, 1.0, &mut rng, &sc),
            None,
        );
    }

    #[test]
    fn gate_no_override_for_normal_personality() {
        let sc = default_scoring();
        let p = default_personality();
        let mut rng = seeded_rng(1);
        assert_eq!(
            behavior_gate_check(Action::Hunt, &p, false, 1.0, &mut rng, &sc),
            None,
        );
    }

    #[test]
    fn gate_compulsive_explorer_fires_probabilistically() {
        let sc = default_scoring();
        let mut p = default_personality();
        p.curiosity = 0.95;
        // Run 200 trials — expect roughly 20% to fire.
        let mut fires = 0;
        let mut rng = seeded_rng(42);
        for _ in 0..200 {
            if behavior_gate_check(Action::Patrol, &p, false, 1.0, &mut rng, &sc)
                == Some(Action::Explore)
            {
                fires += 1;
            }
        }
        assert!(
            fires > 20 && fires < 60,
            "compulsive explorer should fire ~20% of the time; fired {fires}/200"
        );
    }

    #[test]
    fn gate_compulsive_explorer_does_not_override_survival() {
        let sc = default_scoring();
        let mut p = default_personality();
        p.curiosity = 0.95;
        let mut rng = seeded_rng(1);
        // Eat, Sleep, Flee should never be overridden by compulsive explorer.
        for action in [Action::Eat, Action::Sleep, Action::Flee] {
            assert_eq!(
                behavior_gate_check(action, &p, false, 1.0, &mut rng, &sc),
                None,
                "compulsive explorer should not override {action:?}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Herbcraft hint tests
    // -----------------------------------------------------------------------

    #[test]
    fn herbcraft_hint_prepare() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.8;
        needs.energy = 0.8;

        let mut personality = default_personality();
        personality.compassion = 0.9;

        let mut rng = seeded_rng(70);

        let mut c = ctx(&needs, &personality, &sc);
        c.has_remedy_herbs = true;
        c.has_herbs_in_inventory = true;
        c.herbcraft_skill = 0.4;
        c.colony_injury_count = 2;
        // Also set herbs nearby so gather *could* fire — hint should still be PrepareRemedy.
        c.has_herbs_nearby = true;

        let result = score_actions(&c, &test_eval_inputs(), &mut rng);
        assert_eq!(
            result.herbcraft_hint,
            Some(CraftingHint::PrepareRemedy),
            "with remedy herbs + injuries, hint should be PrepareRemedy; got {:?}",
            result.herbcraft_hint
        );
    }

    #[test]
    fn herbcraft_hint_gather() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.8;
        needs.energy = 0.8;

        let mut personality = default_personality();
        personality.spirituality = 0.9;

        let mut rng = seeded_rng(71);

        let mut c = ctx(&needs, &personality, &sc);
        c.has_herbs_nearby = true;
        c.herbcraft_skill = 0.3;
        // No remedy herbs, no ward herbs — only gather is viable.

        let result = score_actions(&c, &test_eval_inputs(), &mut rng);
        assert_eq!(
            result.herbcraft_hint,
            Some(CraftingHint::GatherHerbs),
            "with only herbs nearby, hint should be GatherHerbs; got {:?}",
            result.herbcraft_hint
        );
    }

    #[test]
    fn herbcraft_hint_ward() {
        let sc = default_scoring();
        // Ward is Level 2 (Safety) — only physiological needs matter.
        let mut needs = Needs::default();
        needs.hunger = 0.8;
        needs.energy = 0.8;

        let mut personality = default_personality();
        personality.spirituality = 0.9;

        let mut rng = seeded_rng(72);

        let mut c = ctx(&needs, &personality, &sc);
        c.has_ward_herbs = true;
        c.has_herbs_in_inventory = true;
        c.ward_strength_low = true;
        c.herbcraft_skill = 0.3;
        // No herbs nearby, no remedy herbs — only ward is viable.

        let result = score_actions(&c, &test_eval_inputs(), &mut rng);
        assert_eq!(
            result.herbcraft_hint,
            Some(CraftingHint::SetWard),
            "with ward herbs + low ward strength, hint should be SetWard; got {:?}",
            result.herbcraft_hint
        );
    }

    #[test]
    fn ward_score_rises_with_territory_corruption() {
        // §13.1 rewritten: the `ward_corruption_emergency_bonus` flat
        // additive retired in favor of a Logistic(8, 0.1) axis on
        // `territory_max_corruption` in both `herbcraft_ward` and
        // `herbcraft_gather`. Absolute-threshold assertions no longer
        // apply (the modifier additive on top of a [0,1] CP is gone);
        // the ecological relationship "more corruption ⇒ stronger
        // ward pull" must still hold through the axis curve.
        let sc = default_scoring();
        let needs = Needs::default(); // all needs satisfied
        let mut personality = default_personality();
        personality.spirituality = 0.6;

        let ward_score_with_corruption = {
            let mut rng = seeded_rng(100);
            let mut c = ctx(&needs, &personality, &sc);
            c.has_ward_herbs = true;
            c.ward_strength_low = true;
            c.herbcraft_skill = 0.6;
            c.territory_max_corruption = 0.5;
            let result = score_actions(&c, &test_eval_inputs(), &mut rng);
            result
                .scores
                .iter()
                .find(|(a, _)| matches!(a, Action::Herbcraft))
                .map(|(_, s)| *s)
                .unwrap_or(0.0)
        };

        let ward_score_without_corruption = {
            let mut rng = seeded_rng(100);
            let mut c = ctx(&needs, &personality, &sc);
            c.has_ward_herbs = true;
            c.ward_strength_low = true;
            c.herbcraft_skill = 0.6;
            c.territory_max_corruption = 0.0;
            let result = score_actions(&c, &test_eval_inputs(), &mut rng);
            result
                .scores
                .iter()
                .find(|(a, _)| matches!(a, Action::Herbcraft))
                .map(|(_, s)| *s)
                .unwrap_or(0.0)
        };

        // Logistic(8, 0.1) on `territory_max_corruption` evaluates to
        // ~0.31 at 0.0 and ~0.96 at 0.5 — a 3× axis-output gain. The
        // CP composition blends that with the other axes, so the
        // aggregate ward_score must rise meaningfully but not
        // necessarily 3×. Assert strict monotone growth.
        assert!(
            ward_score_with_corruption > ward_score_without_corruption,
            "ward with corruption ({ward_score_with_corruption:.3}) must beat \
             ward without corruption ({ward_score_without_corruption:.3})"
        );
        // Magnitude witness: at corruption 0.5 the axis saturates past
        // its 0.1 midpoint, so the CP-composed score should pick up a
        // clearly-above-baseline boost. 1.25× is the conservative
        // lower bound that matches the axis shape without over-
        // constraining the compensation math.
        assert!(
            ward_score_with_corruption > 1.25 * ward_score_without_corruption,
            "axis surge should lift ward score ≥ 1.25× baseline; \
             got {ward_score_with_corruption:.3} vs {ward_score_without_corruption:.3}"
        );
    }

    // -------------------------------------------------------------------
    // §11 softmax capture tests — disposition-selection path parity +
    // capture-sink population + no-pool fallthrough.
    // -------------------------------------------------------------------

    #[test]
    fn softmax_capture_records_probabilities_sum_to_one() {
        // A two-action pool at modest temperature should produce a
        // valid probability distribution (non-negative, sums to ~1).
        // This is the first-line gate on the L3 record's softmax.probabilities.
        let sc = ScoringConstants::default();
        let scores = vec![(Action::Eat, 0.8), (Action::Sleep, 0.4)];
        let mut rng = seeded_rng(1);
        let mut capture = SoftmaxCapture::default();
        let _ = select_disposition_via_intention_softmax_with_trace(
            &scores,
            false,
            0.0,
            0.0,
            &sc,
            &mut rng,
            Some(&mut capture),
        );
        assert_eq!(capture.probabilities.len(), 2);
        let sum: f32 = capture.probabilities.iter().sum();
        assert!((sum - 1.0).abs() < 1e-4, "probabilities sum to {sum}, expected ~1");
        for p in &capture.probabilities {
            assert!(*p >= 0.0);
            assert!(*p <= 1.0);
        }
        assert!(capture.raw_roll >= 0.0 && capture.raw_roll <= 1.0);
        assert!(capture.chosen_idx.is_some());
        assert!(!capture.empty_pool);
    }

    #[test]
    fn softmax_capture_flags_empty_pool_fallthrough() {
        // When every action is Flee/Idle or zero-scoring the pool is
        // empty and the softmax doesn't fire. The capture sink should
        // still record the temperature + `empty_pool: true` so replay
        // frames can distinguish "softmax ran" from "fallthrough to
        // DispositionKind::Resting" unambiguously.
        let sc = ScoringConstants::default();
        let scores = vec![(Action::Flee, 0.9), (Action::Idle, 0.3)];
        let mut rng = seeded_rng(2);
        let mut capture = SoftmaxCapture::default();
        let chosen = select_disposition_via_intention_softmax_with_trace(
            &scores,
            false,
            0.0,
            0.0,
            &sc,
            &mut rng,
            Some(&mut capture),
        );
        assert_eq!(chosen, crate::components::disposition::DispositionKind::Resting);
        assert!(capture.empty_pool);
        assert!(capture.pool.is_empty());
        assert!(capture.probabilities.is_empty());
        assert!(capture.chosen_idx.is_none());
    }

    #[test]
    fn softmax_without_capture_matches_capture_variant() {
        // Parity check: calling the `_with_trace` variant with `None`
        // must yield the same DispositionKind as the plain variant for
        // the same (scores, rng-seed) inputs — proves the capture path
        // is observation-only.
        let sc = ScoringConstants::default();
        let scores = vec![(Action::Eat, 0.6), (Action::Socialize, 0.5), (Action::Patrol, 0.4)];
        let plain = select_disposition_via_intention_softmax(
            &scores,
            false,
            0.0,
            0.0,
            &sc,
            &mut seeded_rng(99),
        );
        let traced = select_disposition_via_intention_softmax_with_trace(
            &scores,
            false,
            0.0,
            0.0,
            &sc,
            &mut seeded_rng(99),
            None,
        );
        assert_eq!(plain, traced);
    }
}
