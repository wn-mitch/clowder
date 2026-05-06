use std::collections::HashMap;

use bevy::prelude::Entity;
use rand::Rng;

use crate::ai::dse::EvalCtx;
use crate::ai::considerations::LandmarkAnchor;
use crate::ai::eval::{evaluate_single, DseRegistry, ModifierPipeline};
use crate::ai::Action;
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
    /// §L2.10.7 colony-wide anchor lookup. Read by the cat-side
    /// `EvalCtx::anchor_position` closure for `LandmarkAnchor::Nearest{Kitchen,Stores,Garden}`
    /// — colony-wide single-instance buildings populated by
    /// `systems::buildings::update_colony_landmarks`.
    pub colony_landmarks: &'a crate::resources::ColonyLandmarks,
    /// §L2.10.7 unexplored-frontier centroid cache. Read by Explore
    /// (B16) via `LandmarkAnchor::UnexploredFrontierCentroid`.
    pub exploration_map: &'a crate::resources::ExplorationMap,
    /// §L2.10.7 territory-corruption centroid cache. Read by
    /// ColonyCleanse (B12) via
    /// `LandmarkAnchor::TerritoryCorruptionCentroid`.
    pub corruption_landmarks: &'a crate::resources::CorruptionLandmarks,
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
/// §L2.10.7 per-cat anchor positions. Populated once per scoring
/// tick by the cat-side `ScoringContext` builders (`goap.rs::eligible_dispositions`
/// and `disposition.rs::evaluate_dispositions`); read by the
/// `EvalCtx::anchor_position` closure in `score_dse_by_id` to resolve
/// `LandmarkSource::Anchor(LandmarkAnchor::*)` to a concrete tile
/// position.
///
/// Each field is `Option<Position>`; `None` means the anchor has no
/// resolvable position for this cat this tick (no threat in range,
/// no corrupted tile nearby, no construction site, etc.) and the
/// consideration scores 0.0 per the
/// `LandmarkSource::Anchor` substrate convention.
///
/// **Why an owned struct, not a reference.** ScoringContext borrows
/// shared resources; the anchor positions are computed per-cat per-tick
/// and don't outlive the builder. Owning them inside ScoringContext
/// keeps the closure capture trivial (no extra lifetime dance).
#[derive(Default, Debug, Clone, Copy)]
pub struct CatAnchorPositions {
    /// `LandmarkAnchor::NearestConstructionSite` — Build (B6).
    pub nearest_construction_site: Option<Position>,
    /// `LandmarkAnchor::NearestForageableCluster` — Forage (B7).
    pub nearest_forageable_cluster: Option<Position>,
    /// `LandmarkAnchor::NearestHerbPatch` — HerbcraftGather (B8).
    pub nearest_herb_patch: Option<Position>,
    /// `LandmarkAnchor::NearestPerimeterTile` — HerbcraftWard (B9).
    pub nearest_perimeter_tile: Option<Position>,
    /// `LandmarkAnchor::CoordinatorPerch` — Coordinate (B14).
    pub coordinator_perch: Option<Position>,
    /// `LandmarkAnchor::TerritoryPerimeterAnchor` — Patrol (B13).
    pub territory_perimeter_anchor: Option<Position>,
    /// `LandmarkAnchor::OwnSleepingSpot` — Sleep (B2).
    pub own_sleeping_spot: Option<Position>,
    /// `LandmarkAnchor::OwnSafeRestSpot` — Sleep, body-state safe-
    /// rest axis. Memory-derived; `None` if the cat has no Sleep
    /// memories yet (newly-spawned cats). Ticket 089.
    pub own_safe_rest_spot: Option<Position>,
    /// `LandmarkAnchor::OwnInjurySite` — future TendInjury DSE.
    /// `None` if the cat has no unhealed injuries. Ticket 089.
    pub own_injury_site: Option<Position>,
    /// `LandmarkAnchor::NearestThreat` — Flee (B15).
    pub nearest_threat: Option<Position>,
    /// `LandmarkAnchor::NearestCorruptedTile` — Cleanse (B1) +
    /// DurableWard (B11).
    pub nearest_corrupted_tile: Option<Position>,
}

pub struct ScoringContext<'a> {
    pub scoring: &'a ScoringConstants,
    /// §075 — disposition-tier tunables consulted by the
    /// `commitment_tenure_progress` scalar producer
    /// (`min_disposition_tenure_ticks`). Borrowed alongside `scoring`
    /// because `ScoringConstants` does not carry the tenure-window
    /// knob; `ctx_scalars` reads it through this field rather than
    /// each scoring site re-deriving the progress value.
    pub disposition_constants: &'a crate::resources::sim_constants::DispositionConstants,
    pub needs: &'a Needs,
    pub personality: &'a Personality,
    pub food_available: bool,
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
    /// Ticket 087 — interoceptive perception. Sum of unhealed-injury
    /// severity scores normalized into `[0, 1]` by
    /// `DispositionConstants::pain_normalization_max`. Computed at
    /// `ScoringContext` construction via
    /// `crate::systems::interoception::pain_level`.
    pub pain_level: f32,
    /// Ticket 087 — interoceptive perception. Composite body-state
    /// distress: max of {hunger_urgency, energy_deficit, thermal_deficit,
    /// health_deficit}. The unified "I am unwell" scalar consumed by the
    /// future §L2.10 distress-promotion Modifier (ticket 088). Computed
    /// at `ScoringContext` construction via
    /// `crate::systems::interoception::body_distress_composite`.
    pub body_distress_composite: f32,
    /// Ticket 090 — interoceptive perception. Mean of all six `Skills`
    /// field values normalized into `[0, 1]`; `skills.total() / 6.0`.
    /// High skill → high felt-competence. Freshly spawned cats ≈ 0.07.
    /// Computed via
    /// `crate::systems::interoception::mastery_confidence`.
    pub mastery_confidence: f32,
    /// Ticket 090 — interoceptive perception. `1.0` if the cat has at
    /// least one active `ActiveAspiration`, `0.0` if none or if the
    /// `Aspirations` component is absent. Binary presence signal —
    /// not a gradient. Computed via
    /// `crate::systems::interoception::purpose_clarity`.
    pub purpose_clarity: f32,
    /// Ticket 090 — interoceptive perception. Max of the two L4
    /// (esteem) need deficits: `max(1 - respect, 1 - mastery)`.
    /// Parallels `body_distress_composite` for the esteem tier.
    /// Range `[0, 1]`. Computed via
    /// `crate::systems::interoception::esteem_distress`.
    pub esteem_distress: f32,
    /// Ticket 103 — threat-coupled escape viability in `[0, 1]`.
    /// Pure physics: terrain openness around the cat minus a flat
    /// penalty when a dependent (kitten or pair-bonded mate) is
    /// present. `1.0` when no threat is nearby (the question is
    /// undefined-but-safe; consumers gate on threat presence first).
    /// **Single-axis** — ambient closed-space anxiety
    /// (claustrophobia / agoraphobia) is a *separate* signal owned
    /// by ticket 126's phobia modifier family, not folded here.
    /// Computed at construction via
    /// `crate::systems::interoception::escape_viability`.
    pub escape_viability: f32,
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
    /// Ticket 175 — coarse projection of the cat's `Inventory` into a
    /// single `Carrying` state. Computed once per scoring tick via
    /// `Carrying::from_inventory`; shared with the planner-side
    /// projection in `build_planner_state` (priority cascade
    /// `BuildMaterials > Prey > ForagedFood > Herbs > Nothing`).
    /// Consumed by `carry_affinity_bonus` to bias L2 DSE scores
    /// toward chains that consume the cat's current carry.
    pub carrying: crate::ai::planner::Carrying,
    // Ticket 014 Magic colony batch: `thornbriar_available` field
    // retired — was assigned in disposition.rs / goap.rs but never
    // read. The marker `ThornbriarAvailable` is authored colony-scope
    // by `magic::is_thornbriar_available` for any future DSE
    // eligibility consumer; the GOAP planner's `WorldState` carries
    // its own `thornbriar_available` for `StatePredicate` matching
    // (separate state machine, not migrated).
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
    // Ticket 014 Mentoring batch: `has_mentoring_target` field retired —
    // read via the `HasMentoringTarget` marker now. `MentorDse`'s
    // EligibilityFilter requires the marker via
    // `aspirations::update_mentoring_target_markers`, so the inline
    // outer gate at the Mentor scoring site is retired in lockstep.
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
    /// Tick when the cat's current disposition was last switched into.
    /// Source: `Disposition::disposition_started_tick`, written by
    /// `plan_substrate::record_disposition_switch` (072). Consumed by
    /// the §3.5.1 `CommitmentTenure` Modifier (075) through the
    /// `commitment_tenure_progress` scalar — the modifier applies an
    /// additive lift to the cat's incumbent disposition's constituent
    /// DSEs while `tick - disposition_started_tick <
    /// min_disposition_tenure_ticks`. Defaults to 0 at every
    /// `ScoringContext` construction site that doesn't have an active
    /// Disposition component to read; combined with
    /// `has_active_disposition = false`, the modifier short-circuits
    /// and applies no lift.
    pub disposition_started_tick: u64,
    /// Pre-computed tradition location bonus for the cat's current tile.
    /// Set to `tradition * 0.1` by the caller if the cat's current action
    /// matches a previously successful action at this tile, else 0.0.
    pub tradition_location_bonus: f32,
    // --- Reproduction context ---
    // Ticket 027 Bug 2: `has_eligible_mate` field retired — read via
    // the `HasEligibleMate` marker now (`ai::dses::mate::MateDse`
    // requires it on its EligibilityFilter).
    /// Urgency of nearby hungry kittens (0.0 if none).
    pub hungry_kitten_urgency: f32,
    /// Whether this cat is a parent of the hungriest nearby kitten.
    pub is_parent_of_hungry_kitten: bool,
    /// Hearing-channel perception of nearby kitten distress cries,
    /// sampled from `KittenCryMap` at the cat's tile (0.0–1.0). Painted
    /// by `update_kitten_cry_map` for any kitten whose hunger drops
    /// below `kitten_cry_hunger_threshold`. Consumed by `CaretakeDse`
    /// as a fourth axis (substrate-refactor §4.7 — the map is
    /// externally-authored substrate, this scalar is the single-axis
    /// perception bridge to the DSE consumer).
    pub kitten_cry_perceived: f32,
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
    // --- §L2.10.7 anchor positions ---
    /// Per-cat anchor positions for `LandmarkSource::Anchor` resolution.
    /// Populated by the ScoringContext builders (`goap.rs`, `disposition.rs`)
    /// once per scoring tick. See [`CatAnchorPositions`] doc for details.
    pub cat_anchors: CatAnchorPositions,
    // --- Disposition-failure cooldown signals: 1.0 = no recent failure
    // (no damp), 0.0 = just failed (full damp). One per failure-prone
    // `DispositionKind`. Read by `DispositionFailureCooldown` in
    // `src/ai/modifier.rs`.
    pub disposition_failure_signal_hunting: f32,
    pub disposition_failure_signal_foraging: f32,
    pub disposition_failure_signal_crafting: f32,
    pub disposition_failure_signal_caretaking: f32,
    pub disposition_failure_signal_building: f32,
    pub disposition_failure_signal_mating: f32,
    pub disposition_failure_signal_mentoring: f32,
    // --- Memory-event proximity sums: per-cat aggregate of
    // `proximity * strength` across the cat's `Memory.events` filtered
    // by event type. Each sum feeds the matching §3.5.1 modifier:
    // `MemoryResourceFoundLift` / `MemoryDeathPenalty` /
    // `MemoryThreatSeenSuppress`. Built once per scoring tick from
    // the same iteration the legacy `apply_memory_bonuses` ran.
    pub memory_resource_found_proximity_sum: f32,
    pub memory_death_proximity_sum: f32,
    pub memory_threat_seen_proximity_sum: f32,
    /// Σ proximity × strength across `ColonyKnowledge` entries with
    /// event type `ResourceFound`. Read by `ColonyKnowledgeLift`'s
    /// resource arm.
    pub colony_knowledge_resource_proximity: f32,
    /// Σ proximity × strength across `ColonyKnowledge` entries with
    /// event type `ThreatSeen` or `Death`. Read by
    /// `ColonyKnowledgeLift`'s threat arm.
    pub colony_knowledge_threat_proximity: f32,
    /// Ordinal of the active `ColonyPriority`: `-1` = none, `0` Food,
    /// `1` Defense, `2` Building, `3` Exploration. Read by
    /// `ColonyPriorityLift`.
    pub colony_priority_ordinal: f32,
    /// Per-action cascade counts: `cascade_counts[Action as usize]` is
    /// the number of nearby cats currently performing that action.
    /// Populated at builder time from the colony-wide action snapshot.
    /// `Fight` slot stays 0 — the legacy chain excluded Fight from
    /// cascading (Fight has its own `fight_ally_bonus_per_cat` baked
    /// into its scoring axes); the modifier carves Fight out
    /// independently.
    pub cascade_counts: [f32; CASCADE_COUNTS_LEN],
    /// Per-action count of active aspirations whose domain includes
    /// the action. Read by `AspirationLift`.
    pub aspiration_action_counts: [f32; CASCADE_COUNTS_LEN],
    /// Per-action preference signal: `+1.0` Like, `-1.0` Dislike,
    /// `0.0` no preference. Read by `PreferenceLift` (Like arm) and
    /// `PreferencePenalty` (Dislike arm).
    pub preference_signals: [f32; CASCADE_COUNTS_LEN],
    /// `1.0` when the cat has an awakened `FatedLove` and the partner
    /// is currently visible, else `0.0`. Read by `FatedLoveLift`.
    pub fated_love_visible: f32,
    /// `1.0` when the cat has an awakened `FatedRival` and the rival
    /// is nearby (sensory check), else `0.0`. Read by `FatedRivalLift`.
    pub fated_rival_nearby: f32,
    /// `Action as usize` of the cat's active directive target, or
    /// `-1.0` when no directive is active. Read by `ActiveDirectiveLift`.
    pub active_directive_action_ordinal: f32,
    /// Pre-multiplied directive bonus magnitude (priority × social
    /// weight × base × personality × relationships). Read by
    /// `ActiveDirectiveLift`.
    pub active_directive_bonus: f32,
}

/// Length of the per-action cascade-count array. Equals the number of
/// `Action` enum variants — index via `action as usize`.
// 158: bumped 22 → 23 to give `Action::GroomSelf` and
// `Action::GroomOther` distinct cascade / aspiration / preference
// slots after the `Action::Groom` umbrella retired.
// 155: bumped 23 → 30. `Action::Herbcraft` fanned into 3 sub-actions
// (net +2) and `Action::PracticeMagic` fanned into 6 sub-actions
// (net +5); each gets its own slot.
pub const CASCADE_COUNTS_LEN: usize = 30;

// ---------------------------------------------------------------------------
// ScoringResult
// ---------------------------------------------------------------------------

/// Bundles action scores with metadata the chain builder needs.
/// `herbcraft_hint` carries which sub-mode won during herbcraft scoring,
/// so the chain builder doesn't re-derive it via its own priority cascade.
/// 155: `herbcraft_hint` and `magic_hint` retired. The L3 softmax pool
/// now carries each former hint variant as its own first-class
/// `Action` entry (HerbcraftGather/Remedy/SetWard,
/// MagicScry/DurableWard/Cleanse/ColonyCleanse/Harvest/Commune); the
/// chosen sub-action is the L3-winning Action whose parent Disposition
/// matches `chosen`. No post-hoc tournament needed.
pub struct ScoringResult {
    pub scores: Vec<(Action, f32)>,
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
fn ctx_scalars(ctx: &ScoringContext, inputs: &EvalInputs) -> HashMap<&'static str, f32> {
    let mut m = HashMap::new();
    // Needs-as-urgency (deficit form).
    m.insert("hunger_urgency", (1.0 - ctx.needs.hunger).clamp(0.0, 1.0));
    // Food-stores scarcity (deficit fraction in `[0, 1]`).
    m.insert("food_scarcity", (1.0 - ctx.food_fraction).clamp(0.0, 1.0));
    // 176: colony food security — saturation form. High when the
    // colony's stores are well-stocked AND the cat's own hunger is
    // satisfied (Maslow-tier-1 secure). Stage 5 ships a simple
    // `min(food_fraction, hunger_satisfaction)` formulation; balance-
    // tuning can replace with starvation-recency-aware variants.
    // Consumed by Hunt / Forage DSEs as a saturation axis when their
    // weights are lifted from 0.0.
    let hunger_satisfaction = ctx.needs.hunger.clamp(0.0, 1.0);
    m.insert(
        "colony_food_security",
        ctx.food_fraction.clamp(0.0, 1.0).min(hunger_satisfaction),
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
    m.insert("energy_deficit", (1.0 - ctx.needs.energy).clamp(0.0, 1.0));
    m.insert("health_deficit", (1.0 - ctx.health).clamp(0.0, 1.0));
    // Ticket 087 — interoceptive perception. `pain_level` and
    // `body_distress_composite` are pre-computed at `ScoringContext`
    // construction by `crate::systems::interoception` helpers; the
    // deficit math for `health_deficit` above is the same value
    // `interoception::health_deficit(&Health)` produces (single source
    // of truth across the perception layer and the scoring surface).
    m.insert("pain_level", ctx.pain_level.clamp(0.0, 1.0));
    m.insert(
        "body_distress_composite",
        ctx.body_distress_composite.clamp(0.0, 1.0),
    );
    // Ticket 103 — threat-coupled escape viability. Pure physics
    // signal published for future Fight (102) / Freeze (105)
    // modifiers; no consumer at landing.
    m.insert(
        "escape_viability",
        ctx.escape_viability.clamp(0.0, 1.0),
    );
    // Ticket 108 — `ThreatProximityAdrenalineFlee` Modifier trigger.
    // **Phase 1 stub**: published as 0.0 always. The actual derivative
    // is `max(0, safety_deficit_now - safety_deficit_prev_tick)` —
    // computing it requires a `PrevSafetyDeficit(f32)` per-cat
    // Component plus a per-tick update system that snapshots the
    // current value after the scoring pass runs. That ECS plumbing
    // lands in the same Phase-3-or-Phase-4 commit that promotes 108's
    // lift from 0.0 to the swept-validated magnitude. With the lift
    // at 0.0 (Phase 1), this stub is bit-identical to baseline
    // regardless of value.
    m.insert("threat_proximity_derivative", 0.0);
    // Ticket 109 (Phase A) — `IntraspeciesConflictResponseFlight`
    // Modifier trigger. **Phase 1 stub**: published as 0.0 always.
    // The v1 composition `(status_diff_to_nearest_cat ×
    // proximity_factor)` requires a defensible status-differential
    // signal (no explicit dominance hierarchy exists yet —
    // `needs.respect` and bond strength are candidate proxies) plus
    // per-cat nearest-cat resolution. Both land alongside the lift's
    // promotion in the same Phase-3 commit; with the lift at 0.0
    // here, this stub is bit-identical to baseline.
    m.insert("social_status_distress", 0.0);
    // Ticket 090 — interoceptive perception. L4/L5 Maslow scalars.
    // `mastery_confidence` and `esteem_distress` are continuous [0, 1];
    // `purpose_clarity` is binary {0.0, 1.0}. All three are pre-computed
    // at `ScoringContext` construction by
    // `crate::systems::interoception` helpers.
    m.insert(
        "mastery_confidence",
        ctx.mastery_confidence.clamp(0.0, 1.0),
    );
    m.insert("purpose_clarity", ctx.purpose_clarity.clamp(0.0, 1.0));
    m.insert("esteem_distress", ctx.esteem_distress.clamp(0.0, 1.0));
    // Fight: combat_effective is already a `[0, 1]` composite index
    // upstream; flow through directly.
    m.insert("combat_effective", ctx.combat_effective.clamp(0.0, 1.0));
    // Fight: ally_count is raw count — the DSE's saturating-count
    // Composite handles normalization.
    m.insert("ally_count", ctx.allies_fighting_threat as f32);
    // Personality coefficients flow through directly as `[0, 1]`
    // inputs to each DSE's Linear identity curve.
    m.insert("boldness", ctx.personality.boldness.clamp(0.0, 1.0));
    m.insert("diligence", ctx.personality.diligence.clamp(0.0, 1.0));
    m.insert("sociability", ctx.personality.sociability.clamp(0.0, 1.0));
    m.insert("temper", ctx.personality.temper.clamp(0.0, 1.0));
    m.insert("playfulness", ctx.personality.playfulness.clamp(0.0, 1.0));
    m.insert("warmth", ctx.personality.warmth.clamp(0.0, 1.0));
    m.insert("ambition", ctx.personality.ambition.clamp(0.0, 1.0));
    m.insert("compassion", ctx.personality.compassion.clamp(0.0, 1.0));
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
    m.insert("social_deficit", (1.0 - ctx.needs.social).clamp(0.0, 1.0));
    // §7.2 satiation axis (ticket 122). Raw `social` (NOT a deficit) —
    // the Socialize DSE's curve maps high satiation → low score so the
    // producer side mirrors the OpenMinded gate's `still_goal` proxy
    // (`needs.social < social_satiation_threshold`). Lifting the gate's
    // predicate into IAUS scoring eliminates same-tick PlanCreated →
    // CommitmentDropOpenMinded round-trips on already-bonded cats.
    m.insert("social_satiation", ctx.needs.social.clamp(0.0, 1.0));
    m.insert("mating_deficit", (1.0 - ctx.needs.mating).clamp(0.0, 1.0));
    m.insert(
        "thermal_deficit",
        (1.0 - ctx.needs.temperature).clamp(0.0, 1.0),
    );
    m.insert("phys_satisfaction", ctx.phys_satisfaction.clamp(0.0, 1.0));
    m.insert("tile_corruption", ctx.tile_corruption.clamp(0.0, 1.0));
    // Caretake axes.
    m.insert("kitten_urgency", ctx.hungry_kitten_urgency.clamp(0.0, 1.0));
    m.insert(
        "is_parent_of_hungry_kitten",
        if ctx.is_parent_of_hungry_kitten {
            1.0
        } else {
            0.0
        },
    );
    m.insert(
        "kitten_cry_perceived",
        ctx.kitten_cry_perceived.clamp(0.0, 1.0),
    );
    // Exploration peer group.
    m.insert("curiosity", ctx.personality.curiosity.clamp(0.0, 1.0));
    m.insert("unexplored_nearby", ctx.unexplored_nearby.clamp(0.0, 1.0));
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
    m.insert("spirituality", ctx.personality.spirituality.clamp(0.0, 1.0));
    m.insert("herbcraft_skill", ctx.herbcraft_skill.clamp(0.0, 1.0));
    m.insert("magic_skill", ctx.magic_skill.clamp(0.0, 1.0));
    // Ward deficit: 1.0 when wards are low, 0 when fully warded.
    // `ward_strength_low` is the inline gate today; port as a 0/1
    // scalar so the sibling DSE sees it through Linear identity.
    m.insert(
        "ward_deficit",
        if ctx.ward_strength_low { 1.0 } else { 0.0 },
    );
    // Ticket 084 — Farm DSE herb-pressure axis. Mirrors the exact
    // condition `coordination.rs::evaluate_coordinators` uses to
    // repurpose a FoodCrops garden into a Thornbriar plot
    // (`ward_strength_low && !thornbriar_available`). When the
    // coordinator decides "the colony needs thornbriar," this scalar
    // signals the same demand to FarmDse, so a cat actually tends the
    // repurposed plot even with food stockpiles full. Colony-scoped
    // marker is authored before scoring at `goap.rs:941`; the
    // `markers.has` lookup ignores the entity parameter for
    // colony-scoped keys.
    m.insert(
        "farm_herb_pressure",
        if ctx.ward_strength_low
            && !inputs.markers.has(
                crate::components::markers::ThornbriarAvailable::KEY,
                inputs.cat,
            )
        {
            1.0
        } else {
            0.0
        },
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
    m.insert("day_phase", day_phase_scalar(ctx.day_phase));
    // §3.5.1 modifier-pipeline inputs for the seven foundational
    // modifiers (Pride, Independence-solo / -group, Patience,
    // Tradition, Fox-suppression, Corruption-suppression). Each modifier
    // reads its trigger and transform inputs through the canonical
    // scalar surface rather than carrying a per-field `ScoringContext`
    // accessor — keeps `ScoreModifier` pure and `EvalCtx` unchanged.
    m.insert("respect", ctx.respect.clamp(0.0, 1.0));
    m.insert("pride", ctx.personality.pride.clamp(0.0, 1.0));
    m.insert("independence", ctx.personality.independence.clamp(0.0, 1.0));
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
    // §075 commitment-tenure progress — the `CommitmentTenure`
    // Modifier consumes this to gate its additive lift on the cat's
    // incumbent disposition's constituent DSEs. Shape: 0.0 at
    // `tick == disposition_started_tick`, climbs linearly to 1.0
    // at the tenure-window edge, saturates at 1.0 thereafter.
    // The modifier applies its lift while progress < 1.0; outside
    // the window it returns the score unchanged.
    //
    // When the cat has no active disposition, progress is reported
    // as 1.0 (window already elapsed) so the modifier's outside-
    // window short-circuit fires — no lift is applied without an
    // incumbent disposition to lift.
    m.insert(
        crate::systems::plan_substrate::COMMITMENT_TENURE_INPUT,
        commitment_tenure_progress(
            ctx.has_active_disposition,
            ctx.disposition_started_tick,
            inputs.tick,
            ctx.disposition_constants.min_disposition_tenure_ticks,
        ),
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
    // Disposition-failure cooldown signals — read by
    // `DispositionFailureCooldown` (src/ai/modifier.rs).
    m.insert(
        "disposition_failure_signal_hunting",
        ctx.disposition_failure_signal_hunting,
    );
    m.insert(
        "disposition_failure_signal_foraging",
        ctx.disposition_failure_signal_foraging,
    );
    m.insert(
        "disposition_failure_signal_crafting",
        ctx.disposition_failure_signal_crafting,
    );
    m.insert(
        "disposition_failure_signal_caretaking",
        ctx.disposition_failure_signal_caretaking,
    );
    m.insert(
        "disposition_failure_signal_building",
        ctx.disposition_failure_signal_building,
    );
    m.insert(
        "disposition_failure_signal_mating",
        ctx.disposition_failure_signal_mating,
    );
    m.insert(
        "disposition_failure_signal_mentoring",
        ctx.disposition_failure_signal_mentoring,
    );
    // Memory-event proximity sums — read by the §3.5.1 memory
    // modifiers (src/ai/modifier.rs).
    m.insert(
        "memory_resource_found_proximity_sum",
        ctx.memory_resource_found_proximity_sum,
    );
    m.insert(
        "memory_death_proximity_sum",
        ctx.memory_death_proximity_sum,
    );
    m.insert(
        "memory_threat_seen_proximity_sum",
        ctx.memory_threat_seen_proximity_sum,
    );
    m.insert(
        "colony_knowledge_resource_proximity",
        ctx.colony_knowledge_resource_proximity,
    );
    m.insert(
        "colony_knowledge_threat_proximity",
        ctx.colony_knowledge_threat_proximity,
    );
    m.insert("colony_priority_ordinal", ctx.colony_priority_ordinal);
    // Per-action cascade counts — Σ nearby cats performing each
    // action. Read by `NeighborActionCascade`.
    for (idx, key) in CASCADE_COUNT_KEYS.iter().enumerate() {
        m.insert(key, ctx.cascade_counts[idx]);
    }
    // Per-action aspiration counts — how many active aspirations cover
    // each action. Read by `AspirationLift`.
    for (idx, key) in ASPIRATION_ACTION_KEYS.iter().enumerate() {
        m.insert(key, ctx.aspiration_action_counts[idx]);
    }
    // Per-action preference signals — Like = +1, Dislike = -1, none = 0.
    // Read by `PreferenceLift` / `PreferencePenalty`.
    for (idx, key) in PREFERENCE_KEYS.iter().enumerate() {
        m.insert(key, ctx.preference_signals[idx]);
    }
    m.insert("fated_love_visible", ctx.fated_love_visible);
    m.insert("fated_rival_nearby", ctx.fated_rival_nearby);
    m.insert(
        "active_directive_action_ordinal",
        ctx.active_directive_action_ordinal,
    );
    m.insert("active_directive_bonus", ctx.active_directive_bonus);
    m
}

/// Per-action ctx-scalar keys for cascade counts, indexed parallel to
/// `Action as usize` (entry `i` corresponds to the i-th `Action`
/// variant). Source of truth for both the scalar producer and the
/// `NeighborActionCascade` modifier.
pub const CASCADE_COUNT_KEYS: [&str; CASCADE_COUNTS_LEN] = [
    "cascade_count_eat",
    "cascade_count_sleep",
    "cascade_count_hunt",
    "cascade_count_forage",
    "cascade_count_wander",
    "cascade_count_idle",
    "cascade_count_socialize",
    "cascade_count_groom_self",
    "cascade_count_groom_other",
    "cascade_count_explore",
    "cascade_count_flee",
    "cascade_count_fight",
    "cascade_count_patrol",
    "cascade_count_build",
    "cascade_count_farm",
    // 155: Herbcraft / PracticeMagic fanned to 9 sub-actions.
    "cascade_count_herbcraft_gather",
    "cascade_count_herbcraft_remedy",
    "cascade_count_herbcraft_setward",
    "cascade_count_magic_scry",
    "cascade_count_magic_durable_ward",
    "cascade_count_magic_cleanse",
    "cascade_count_magic_colony_cleanse",
    "cascade_count_magic_harvest",
    "cascade_count_magic_commune",
    "cascade_count_coordinate",
    "cascade_count_mentor",
    "cascade_count_mate",
    "cascade_count_caretake",
    "cascade_count_cook",
    "cascade_count_hide",
];

/// Per-action ctx-scalar keys for aspiration counts, parallel to
/// `Action as usize`.
pub const ASPIRATION_ACTION_KEYS: [&str; CASCADE_COUNTS_LEN] = [
    "aspiration_action_eat",
    "aspiration_action_sleep",
    "aspiration_action_hunt",
    "aspiration_action_forage",
    "aspiration_action_wander",
    "aspiration_action_idle",
    "aspiration_action_socialize",
    "aspiration_action_groom_self",
    "aspiration_action_groom_other",
    "aspiration_action_explore",
    "aspiration_action_flee",
    "aspiration_action_fight",
    "aspiration_action_patrol",
    "aspiration_action_build",
    "aspiration_action_farm",
    // 155: Herbcraft / PracticeMagic fanned to 9 sub-actions.
    "aspiration_action_herbcraft_gather",
    "aspiration_action_herbcraft_remedy",
    "aspiration_action_herbcraft_setward",
    "aspiration_action_magic_scry",
    "aspiration_action_magic_durable_ward",
    "aspiration_action_magic_cleanse",
    "aspiration_action_magic_colony_cleanse",
    "aspiration_action_magic_harvest",
    "aspiration_action_magic_commune",
    "aspiration_action_coordinate",
    "aspiration_action_mentor",
    "aspiration_action_mate",
    "aspiration_action_caretake",
    "aspiration_action_cook",
    "aspiration_action_hide",
];

/// Per-action ctx-scalar keys for preference signals, parallel to
/// `Action as usize`.
pub const PREFERENCE_KEYS: [&str; CASCADE_COUNTS_LEN] = [
    "preference_for_eat",
    "preference_for_sleep",
    "preference_for_hunt",
    "preference_for_forage",
    "preference_for_wander",
    "preference_for_idle",
    "preference_for_socialize",
    "preference_for_groom_self",
    "preference_for_groom_other",
    "preference_for_explore",
    "preference_for_flee",
    "preference_for_fight",
    "preference_for_patrol",
    "preference_for_build",
    "preference_for_farm",
    // 155: Herbcraft / PracticeMagic fanned to 9 sub-actions.
    "preference_for_herbcraft_gather",
    "preference_for_herbcraft_remedy",
    "preference_for_herbcraft_setward",
    "preference_for_magic_scry",
    "preference_for_magic_durable_ward",
    "preference_for_magic_cleanse",
    "preference_for_magic_colony_cleanse",
    "preference_for_magic_harvest",
    "preference_for_magic_commune",
    "preference_for_coordinate",
    "preference_for_mentor",
    "preference_for_mate",
    "preference_for_caretake",
    "preference_for_cook",
    "preference_for_hide",
];

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
    // 150 R5a: `Eating` is appended at ordinal 13 rather than inserted
    // between Resting and Hunting so existing ordinals 1..=12 stay
    // stable. The ordinal is consumed by the Patience and
    // CommitmentTenure modifiers via
    // `modifier::constituent_dses_for_ordinal`; keeping the older
    // numbers stable means saved soaks and hand-written tests don't
    // need rebasing.
    match active {
        None => 0.0,
        Some(DispositionKind::Resting) => 1.0,
        Some(DispositionKind::Hunting) => 2.0,
        Some(DispositionKind::Foraging) => 3.0,
        Some(DispositionKind::Guarding) => 4.0,
        Some(DispositionKind::Socializing) => 5.0,
        Some(DispositionKind::Building) => 6.0,
        Some(DispositionKind::Farming) => 7.0,
        // 155: Herbalism inherits Crafting's ordinal-8 slot in-place
        // (the herbcraft DSE set was the bulk of Crafting's pool).
        // Witchcraft / Cooking append at ordinals 16 / 17 — same
        // append-only discipline established by 150 R5a / 154 / 158.
        Some(DispositionKind::Herbalism) => 8.0,
        Some(DispositionKind::Coordinating) => 9.0,
        Some(DispositionKind::Exploring) => 10.0,
        Some(DispositionKind::Mating) => 11.0,
        Some(DispositionKind::Caretaking) => 12.0,
        Some(DispositionKind::Eating) => 13.0,
        Some(DispositionKind::Mentoring) => 14.0,
        Some(DispositionKind::Grooming) => 15.0,
        Some(DispositionKind::Witchcraft) => 16.0,
        Some(DispositionKind::Cooking) => 17.0,
        // 176: inventory-disposal dispositions append at ordinals
        // 18-21 — same append-only discipline as 150/154/158/155 so
        // saved soaks and ordinal-equality tests stay valid.
        Some(DispositionKind::Discarding) => 18.0,
        Some(DispositionKind::Trashing) => 19.0,
        Some(DispositionKind::Handing) => 20.0,
        Some(DispositionKind::PickingUp) => 21.0,
    }
}

/// §075 — `CommitmentTenure` Modifier scalar producer. Returns the
/// fraction of the cat's disposition-tenure window that has elapsed:
/// `0.0` immediately after a switch, climbing linearly toward `1.0`
/// at the window edge, saturating at `1.0` thereafter. The
/// `CommitmentTenure` modifier (`src/ai/modifier.rs`) lifts the
/// incumbent disposition's constituent DSE scores while progress is
/// strictly less than `1.0`; outside the window the modifier returns
/// each score unchanged.
///
/// Cats with no active disposition report `1.0` (window already
/// elapsed) so the modifier's outside-window short-circuit fires —
/// no lift is applied without an incumbent to lift.
///
/// Defensive against `tick < disposition_started_tick` (clock-rewind
/// edge case during save/load): saturating subtraction floors the
/// elapsed window at 0, so progress is `0.0` rather than wrapping.
/// Defensive against `min_tenure_ticks == 0` (knob set to zero
/// effectively disables the modifier): returns `1.0` so the modifier
/// short-circuits.
fn commitment_tenure_progress(
    has_active_disposition: bool,
    disposition_started_tick: u64,
    tick: u64,
    min_tenure_ticks: u64,
) -> f32 {
    if !has_active_disposition || min_tenure_ticks == 0 {
        return 1.0;
    }
    let elapsed = tick.saturating_sub(disposition_started_tick);
    let clamped = elapsed.min(min_tenure_ticks);
    clamped as f32 / min_tenure_ticks as f32
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
fn score_dse_by_id(dse_id: &str, ctx: &ScoringContext, inputs: &EvalInputs) -> f32 {
    let Some(dse) = inputs.dse_registry.cat_dse(dse_id) else {
        return 0.0;
    };
    let scalars = ctx_scalars(ctx, inputs);
    let fetch_scalar =
        |name: &str, _entity: Entity| -> f32 { scalars.get(name).copied().unwrap_or(0.0) };
    // §4 marker lookup — consumes `EvalInputs::markers` populated by
    // the caller. `entity` is the evaluating cat when eligibility runs
    // against a per-cat marker; colony-scoped markers ignore it.
    let markers = inputs.markers;
    let has_marker = |name: &str, entity: Entity| -> bool { markers.has(name, entity) };
    let entity_position = |_: Entity| -> Option<Position> { None };
    // §L2.10.7 cat-side anchor resolution. Reads colony-wide anchors
    // from `EvalInputs` (ColonyLandmarks / ExplorationMap /
    // CorruptionLandmarks) and per-cat anchors from
    // `ScoringContext.cat_anchors` (populated once per scoring tick by
    // the builders in `goap.rs` / `disposition.rs`).
    let anchor_position = |a: LandmarkAnchor| -> Option<Position> {
        match a {
            // Colony-wide single-instance buildings.
            LandmarkAnchor::NearestKitchen => inputs.colony_landmarks.kitchen,
            LandmarkAnchor::NearestStores => inputs.colony_landmarks.stores,
            LandmarkAnchor::NearestGarden => inputs.colony_landmarks.garden,
            // Per-tick precomputed centroids.
            LandmarkAnchor::UnexploredFrontierCentroid => {
                inputs.exploration_map.frontier_centroid()
            }
            LandmarkAnchor::TerritoryCorruptionCentroid => inputs.corruption_landmarks.centroid(),
            // Per-cat anchors populated by the ScoringContext builder.
            LandmarkAnchor::NearestConstructionSite => ctx.cat_anchors.nearest_construction_site,
            LandmarkAnchor::NearestForageableCluster => ctx.cat_anchors.nearest_forageable_cluster,
            LandmarkAnchor::NearestHerbPatch => ctx.cat_anchors.nearest_herb_patch,
            LandmarkAnchor::NearestPerimeterTile => ctx.cat_anchors.nearest_perimeter_tile,
            LandmarkAnchor::CoordinatorPerch => ctx.cat_anchors.coordinator_perch,
            LandmarkAnchor::TerritoryPerimeterAnchor => ctx.cat_anchors.territory_perimeter_anchor,
            LandmarkAnchor::OwnSleepingSpot => ctx.cat_anchors.own_sleeping_spot,
            // Ticket 089 — interoceptive self-anchors.
            LandmarkAnchor::OwnSafeRestSpot => ctx.cat_anchors.own_safe_rest_spot,
            LandmarkAnchor::OwnInjurySite => ctx.cat_anchors.own_injury_site,
            LandmarkAnchor::NearestThreat => ctx.cat_anchors.nearest_threat,
            LandmarkAnchor::NearestCorruptedTile => ctx.cat_anchors.nearest_corrupted_tile,
            // Fox-side & centroid-only anchors aren't relevant to cat
            // scoring. Listed explicitly (no `_ => None`) so adding a
            // new anchor variant is a compilation gate, not a silent
            // resolve-to-None. Ticket 089.
            LandmarkAnchor::PreyBeliefCentroid
            | LandmarkAnchor::CatClusterCentroid
            | LandmarkAnchor::OwnDen
            | LandmarkAnchor::NearestVisibleStore
            | LandmarkAnchor::NearestMapEdge => None,
        }
    };
    let needs_ref = ctx.needs;
    let maslow = |tier: u8| needs_ref.level_suppression(tier);

    let eval_ctx = EvalCtx {
        cat: inputs.cat,
        tick: inputs.tick,
        entity_position: &entity_position,
        anchor_position: &anchor_position,
        has_marker: &has_marker,
        self_position: inputs.position,
        target: None,
        target_position: None,
        target_alive: None,
    };

    let focal_active = inputs.focal_capture.is_some() && inputs.focal_cat == Some(inputs.cat);

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
            // 175: apply carry-affinity bias on the post-trace
            // path too. Trace records pre-bias `final_score` for
            // forensic traceability — the bias is a post-eval
            // multiplier on what the L3 softmax sees, not a
            // mutation of the L2 evaluation.
            return apply_carry_affinity(
                scored.final_score,
                dse_id,
                ctx.carrying,
                ctx.scoring.carry_affinity_bonus,
            );
        }
        return 0.0;
    }

    let base = evaluate_single(
        dse,
        inputs.cat,
        &eval_ctx,
        &maslow,
        inputs.modifier_pipeline,
        &fetch_scalar,
    )
    .map(|s| s.final_score)
    .unwrap_or(0.0);
    apply_carry_affinity(base, dse_id, ctx.carrying, ctx.scoring.carry_affinity_bonus)
}

/// Ticket 175 — L2 carry-affinity bias. Multiplies a DSE's
/// pre-softmax score by `bonus` when the cat's current
/// `Carrying` projection maps to that DSE's terminal-product
/// chain. The principle: "use what you're holding" is a soft
/// preference, not a hard veto. Cats are biased toward chains
/// that consume their current carry; the planner stays
/// flexible enough to plan an alternative when bias is
/// overridden by acute need (the hunger-while-carrying-Prey
/// case still picks Eating).
///
/// `Carrying::Nothing` and unmapped DSEs return `base` verbatim
/// (no bias). Setting `bonus = 1.0` disables the bias entirely.
///
/// **Mapping** (must stay aligned with `Carrying::from_inventory`'s
/// projectable variants):
///
/// | Carrying | Boosted DSE IDs |
/// |---|---|
/// | `Prey` | `hunt` |
/// | `ForagedFood` | `forage` |
/// | `Herbs` | `herbcraft_prepare`, `herbcraft_ward`, `apply_remedy_target` |
/// | `BuildMaterials` | `build` |
/// | `Nothing` | (none — no bias applies) |
///
/// `RawFood` / `CookedFood` / `Remedy` are search-state-only
/// variants (set during A* expansion by chain effects) and
/// never appear from `from_inventory`; included here for
/// completeness so future projection extensions slot in
/// without a missing-arm regression.
pub fn apply_carry_affinity(
    base: f32,
    dse_id: &str,
    carrying: crate::ai::planner::Carrying,
    bonus: f32,
) -> f32 {
    use crate::ai::planner::Carrying;
    let matches = match (carrying, dse_id) {
        (Carrying::Prey, "hunt") => true,
        (Carrying::ForagedFood, "forage") => true,
        (Carrying::Herbs, "herbcraft_prepare" | "herbcraft_ward" | "apply_remedy_target") => true,
        (Carrying::BuildMaterials, "build") => true,
        // RawFood / CookedFood: cook-chain-internal. The cat
        // entering Cooking with raw or cooked food in inventory
        // is already covered by `Carrying::Prey` /
        // `Carrying::ForagedFood` projection (raw prey → Prey,
        // cooked rat → Prey via raw-kind match, cooked rabbit
        // etc. → ForagedFood). If a future projection extension
        // adds a RawFood/CookedFood inventory variant, the
        // boost target is `cook` for both.
        (Carrying::RawFood | Carrying::CookedFood, "cook") => true,
        // Remedy is search-state-only; never produced from
        // inventory by `from_inventory`. If a future extension
        // tracks held remedies, the boost target is the
        // applier DSE.
        (Carrying::Remedy, "apply_remedy_target") => true,
        // Carrying::Nothing or any unmapped (carry, dse) pair —
        // no bias.
        _ => false,
    };
    if matches { base * bonus } else { base }
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

    // --- Eat (§2.3 hangry anchor: Logistic(8, 0.5), recalibrated ticket 044) ---
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
    // §4 batch 2: inline `ctx.can_hunt` gate retired — HuntDse carries
    // `.require(CanHunt::KEY)`; score_dse_by_id returns 0 on ineligibility.
    {
        let urgency = score_dse_by_id("hunt", ctx, inputs);
        if urgency > 0.0 {
            scores.push((Action::Hunt, urgency + jitter(rng, s.jitter_range)));
        }
    }

    // --- Forage (§2.3: WS of hunger + scarcity + diligence) ---
    // §4 batch 2: inline `ctx.can_forage` gate retired — ForageDse carries
    // `.require(CanForage::KEY)`.
    {
        let urgency = score_dse_by_id("forage", ctx, inputs);
        if urgency > 0.0 {
            scores.push((Action::Forage, urgency + jitter(rng, s.jitter_range)));
        }
    }

    // --- Socialize (§2.3: WS of 6 axes through loneliness + inverted_need_penalty) ---
    if ctx.has_social_target {
        let score = score_dse_by_id("socialize", ctx, inputs);
        scores.push((Action::Socialize, score + jitter(rng, s.jitter_range)));
    }

    // --- Groom (158 / §L2.10.10 Phase 3d): sibling DSEs emit distinct
    // Action variants. The L3 softmax picks GroomSelf vs GroomOther
    // directly — no `Max`-collapse, no side-channel `self_groom_won`
    // resolver. Each Action routes via `from_action` to its own
    // DispositionKind (GroomSelf → Resting, GroomOther → Grooming).
    {
        let self_score = score_dse_by_id("groom_self", ctx, inputs);
        scores.push((Action::GroomSelf, self_score + jitter(rng, s.jitter_range)));
    }
    if ctx.has_social_target {
        let other_score = score_dse_by_id("groom_other", ctx, inputs);
        scores.push((Action::GroomOther, other_score + jitter(rng, s.jitter_range)));
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
    // `herbcraft_ward`. The siege bonus remains inline — it's a
    // narrower-scope siege response, not a corruption trigger.
    // 155: the post-softmax `herbcraft_hint` / `magic_hint` tournament
    // retired in favor of per-sub-action L3 entries.
    // --- Herbcraft (§155: 3-way sibling split) ---
    // Each sub-DSE pushes its own (sub-Action, score) pair into the
    // L3 softmax pool. The post-softmax tournament that picked a
    // hint between gather / prepare / ward retired — softmax now
    // picks the sub-action directly.
    {
        let gather = if ctx.has_herbs_nearby {
            score_dse_by_id("herbcraft_gather", ctx, inputs)
        } else {
            0.0
        };
        if gather > 0.0 {
            scores.push((Action::HerbcraftGather, gather + jitter(rng, s.jitter_range)));
        }
        let prepare = if ctx.has_remedy_herbs && ctx.colony_injury_count > 0 {
            score_dse_by_id("herbcraft_prepare", ctx, inputs)
        } else {
            0.0
        };
        if prepare > 0.0 {
            scores.push((Action::HerbcraftRemedy, prepare + jitter(rng, s.jitter_range)));
        }
        // §4 batch 2: inline `ctx.has_ward_herbs` gate retired —
        // HerbcraftWardDse carries `.require(CanWard::KEY)` which
        // subsumes Adult ∧ ¬Injured ∧ HasWardHerbs.
        let mut ward = score_dse_by_id("herbcraft_ward", ctx, inputs);
        if ward > 0.0 && ctx.wards_under_siege {
            ward += s.herbcraft_ward_siege_bonus * ctx.needs.level_suppression(2);
        }
        if ward > 0.0 {
            scores.push((Action::HerbcraftSetWard, ward + jitter(rng, s.jitter_range)));
        }
    }

    // --- PracticeMagic (§155: 6-way sibling split) ---
    // Outer gate: `magic_affinity + magic_skill > thresholds`.
    // Each sub-DSE pushes its own (sub-Action, score) pair. The
    // post-softmax tournament that picked a hint between scry /
    // durable_ward / cleanse / colony_cleanse / harvest / commune
    // retired — softmax now picks the sub-action directly.
    if ctx.magic_affinity > s.magic_affinity_threshold && ctx.magic_skill > s.magic_skill_threshold
    {
        let scry = score_dse_by_id("magic_scry", ctx, inputs);
        if scry > 0.0 {
            scores.push((Action::MagicScry, scry + jitter(rng, s.jitter_range)));
        }
        let durable_ward = if ctx.magic_skill > s.magic_durable_ward_skill_threshold {
            score_dse_by_id("magic_durable_ward", ctx, inputs)
        } else {
            0.0
        };
        if durable_ward > 0.0 {
            scores.push((
                Action::MagicDurableWard,
                durable_ward + jitter(rng, s.jitter_range),
            ));
        }
        let cleanse = if ctx.on_corrupted_tile
            && ctx.tile_corruption > s.magic_cleanse_corruption_threshold
        {
            score_dse_by_id("magic_cleanse", ctx, inputs)
        } else {
            0.0
        };
        if cleanse > 0.0 {
            scores.push((Action::MagicCleanse, cleanse + jitter(rng, s.jitter_range)));
        }
        let colony_cleanse = score_dse_by_id("magic_colony_cleanse", ctx, inputs);
        if colony_cleanse > 0.0 {
            scores.push((
                Action::MagicColonyCleanse,
                colony_cleanse + jitter(rng, s.jitter_range),
            ));
        }
        let harvest = if ctx.carcass_nearby {
            score_dse_by_id("magic_harvest", ctx, inputs)
        } else {
            0.0
        };
        if harvest > 0.0 {
            scores.push((Action::MagicHarvest, harvest + jitter(rng, s.jitter_range)));
        }
        let commune = if ctx.on_special_terrain {
            score_dse_by_id("magic_commune", ctx, inputs)
        } else {
            0.0
        };
        if commune > 0.0 {
            scores.push((Action::MagicCommune, commune + jitter(rng, s.jitter_range)));
        }
    }

    // --- Coordinate (§2.3: WS of diligence + directive_count + ambition) ---
    // §4 batch 1: inline `if ctx.is_coordinator_with_directives` guard
    // retired. The coordinate DSE now carries
    // `.require("IsCoordinatorWithDirectives")` on its EligibilityFilter,
    // so `score_dse_by_id` returns 0.0 for non-coordinator cats.
    {
        let score = score_dse_by_id("coordinate", ctx, inputs);
        if score > 0.0 {
            scores.push((Action::Coordinate, score + jitter(rng, s.jitter_range)));
        }
    }

    // --- Mentor (§2.3: WS of warmth + diligence + ambition) ---
    // Ticket 014 Mentoring batch: inline `if ctx.has_mentoring_target`
    // guard retired. `MentorDse` now carries
    // `.require(HasMentoringTarget::KEY)` on its EligibilityFilter, so
    // `score_dse_by_id` returns 0.0 for cats with no mentoring target.
    // (Mirrors the `mate` retire pattern below.)
    {
        let score = score_dse_by_id("mentor", ctx, inputs);
        if score > 0.0 {
            scores.push((Action::Mentor, score + jitter(rng, s.jitter_range)));
        }
    }

    // --- Mate (§2.3: CP of mating_deficit + warmth — Logistic(6, 0.6)) ---
    // Ticket 027 Bug 2: inline `if ctx.has_eligible_mate` guard
    // retired. The mate DSE now carries `.require(HasEligibleMate::KEY)`
    // on its EligibilityFilter, so `score_dse_by_id` returns 0.0 for
    // cats without the marker. (Mirrors the `coordinate` retire pattern
    // ~20 lines above.)
    {
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
    // §4.5 scalar precondition and stays as an inline wrap so a
    // *starving* cat doesn't wander off to the Kitchen instead of
    // eating — the gate fires only when the cat is at-least-half-full
    // (canonical semantic: `hunger=1.0` is sated, `hunger=0.0` is
    // starving, so `hunger > cook_hunger_gate (0.5)` means "hunger has
    // some headroom"). See sim_constants doc on `cook_hunger_gate`.
    // The `wants_cook_but_no_kitchen` latent signal (read by
    // BuildPressure in `goap.rs`) is preserved by disambiguating the
    // zero-score case against the raw ScoringContext booleans — "raw
    // food is present but no kitchen" is still the only trigger.
    //
    // 150 hygiene: comment polarity corrected from the pre-150 wording
    // "so Cook isn't scored while the cat is stuffed" — that read as
    // the opposite of what the gate does. The variable name
    // `hungry_enough_to_cook` is also misleading (it actually means
    // "satiated enough to cook"); the SimConstants doc on
    // `cook_hunger_gate` is the authoritative reference.
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
        wants_cook_but_no_kitchen,
    }
}

// ---------------------------------------------------------------------------
// Context bonuses (applied after base scoring)
// ---------------------------------------------------------------------------

/// Compute the per-cat memory-event proximity sums for the
/// `MemoryResourceFoundLift` / `MemoryDeathPenalty` /
/// `MemoryThreatSeenSuppress` modifiers. One pass over `memory.events`,
/// returning `(resource_found, death, threat_seen)` sums of
/// `proximity * strength`. Entries without a location or beyond
/// `memory_nearby_radius` contribute 0.
pub fn memory_proximity_sums(
    memory: &Memory,
    pos: &Position,
    sc: &ScoringConstants,
) -> (f32, f32, f32) {
    let mut resource_found = 0.0;
    let mut death = 0.0;
    let mut threat_seen = 0.0;
    for entry in &memory.events {
        let Some(loc) = &entry.location else { continue };
        let dist = pos.distance_to(loc);
        if dist > sc.memory_nearby_radius {
            continue;
        }
        let weight = (1.0 - dist / sc.memory_nearby_radius) * entry.strength;
        match entry.event_type {
            MemoryType::ResourceFound => resource_found += weight,
            MemoryType::Death => death += weight,
            MemoryType::ThreatSeen => threat_seen += weight,
            _ => {}
        }
    }
    (resource_found, death, threat_seen)
}

/// Aggregate the colony-wide action snapshot into per-action cascade
/// counts for the focal cat. Returns an array indexed by
/// `Action as usize`. Entries beyond `cascading_bonus_range` (Manhattan
/// distance) and the focal cat's own slot contribute 0. The Fight slot
/// stays 0 — the legacy chain excluded Fight from cascading and the
/// modifier preserves that.
pub fn compute_cascade_counts(
    action_snapshot: &[(Entity, Position, Action)],
    self_entity: Entity,
    self_pos: &Position,
    range: i32,
) -> [f32; CASCADE_COUNTS_LEN] {
    let mut counts = [0.0_f32; CASCADE_COUNTS_LEN];
    for &(other_entity, other_pos, other_action) in action_snapshot {
        if other_entity == self_entity {
            continue;
        }
        if other_action == Action::Fight {
            continue;
        }
        if self_pos.manhattan_distance(&other_pos) > range {
            continue;
        }
        let idx = other_action as usize;
        if idx < CASCADE_COUNTS_LEN {
            counts[idx] += 1.0;
        }
    }
    counts
}


/// Aggregate `ColonyKnowledge` entries into the two proximity sums
/// `ColonyKnowledgeLift` reads. Returns `(resource, threat)` where each
/// is Σ `proximity × strength` over qualifying entries (ResourceFound
/// for resource; ThreatSeen ∨ Death for threat). Entries beyond
/// `colony_knowledge_radius` contribute 0.
pub fn colony_knowledge_proximity_sums(
    knowledge: &crate::resources::colony_knowledge::ColonyKnowledge,
    pos: &Position,
    sc: &ScoringConstants,
) -> (f32, f32) {
    let mut resource = 0.0;
    let mut threat = 0.0;
    for entry in &knowledge.entries {
        let Some(loc) = &entry.location else { continue };
        let dist = pos.distance_to(loc);
        if dist > sc.colony_knowledge_radius {
            continue;
        }
        let weight = (1.0 - dist / sc.colony_knowledge_radius) * entry.strength;
        match entry.event_type {
            MemoryType::ResourceFound => resource += weight,
            MemoryType::ThreatSeen | MemoryType::Death => threat += weight,
            _ => {}
        }
    }
    (resource, threat)
}

/// Encode the active `PriorityKind` as an `f32` ordinal for the
/// `ColonyPriorityLift` modifier. `-1.0` = none.
pub fn colony_priority_ordinal(
    priority: Option<crate::resources::colony_priority::PriorityKind>,
) -> f32 {
    use crate::resources::colony_priority::PriorityKind;
    match priority {
        None => -1.0,
        Some(PriorityKind::Food) => 0.0,
        Some(PriorityKind::Defense) => 1.0,
        Some(PriorityKind::Building) => 2.0,
        Some(PriorityKind::Exploration) => 3.0,
    }
}

// ---------------------------------------------------------------------------
// Aspiration, preference, and fate bonuses
// ---------------------------------------------------------------------------

/// Per-action aspiration counts: how many active aspirations include
/// each action in their domain. Used by `AspirationLift`.
pub fn compute_aspiration_action_counts(
    aspirations: &crate::components::aspirations::Aspirations,
) -> [f32; CASCADE_COUNTS_LEN] {
    let mut counts = [0.0_f32; CASCADE_COUNTS_LEN];
    for asp in &aspirations.active {
        for action in asp.domain.matching_actions() {
            let idx = *action as usize;
            if idx < CASCADE_COUNTS_LEN {
                counts[idx] += 1.0;
            }
        }
    }
    counts
}

/// Per-action preference signal: `+1.0` for Like, `-1.0` for Dislike,
/// `0.0` for no preference. Used by `PreferenceLift` /
/// `PreferencePenalty`.
pub fn compute_preference_signals(
    preferences: &crate::components::aspirations::Preferences,
) -> [f32; CASCADE_COUNTS_LEN] {
    use crate::components::aspirations::Preference;
    let mut signals = [0.0_f32; CASCADE_COUNTS_LEN];
    for variant in ALL_ACTIONS {
        let idx = variant as usize;
        if idx >= CASCADE_COUNTS_LEN {
            continue;
        }
        signals[idx] = match preferences.get(variant) {
            Some(Preference::Like) => 1.0,
            Some(Preference::Dislike) => -1.0,
            None => 0.0,
        };
    }
    signals
}

/// Every Action variant — used to walk per-action arrays. The order
/// must mirror `Action`'s declaration order so cascade / aspiration /
/// preference keys map by `Action as usize` ordinal.
pub const ALL_ACTIONS: [Action; CASCADE_COUNTS_LEN] = [
    Action::Eat,
    Action::Sleep,
    Action::Hunt,
    Action::Forage,
    Action::Wander,
    Action::Idle,
    Action::Socialize,
    // 158: Action::Groom split into sibling variants. Each gets its
    // own cascade slot so per-Action neighbor counts and aspiration
    // tallies stay distinct between thermal self-care and allogrooming.
    Action::GroomSelf,
    Action::GroomOther,
    Action::Explore,
    Action::Flee,
    Action::Fight,
    Action::Patrol,
    Action::Build,
    Action::Farm,
    // 155: Herbcraft / PracticeMagic split into 9 sub-actions; each
    // gets its own cascade slot so per-Action neighbor counts and
    // aspiration tallies stay distinct.
    Action::HerbcraftGather,
    Action::HerbcraftRemedy,
    Action::HerbcraftSetWard,
    Action::MagicScry,
    Action::MagicDurableWard,
    Action::MagicCleanse,
    Action::MagicColonyCleanse,
    Action::MagicHarvest,
    Action::MagicCommune,
    Action::Coordinate,
    Action::Mentor,
    Action::Mate,
    Action::Caretake,
    Action::Cook,
    Action::Hide,
];


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
    // 155: routes to the HerbcraftRemedy sub-action (the apply-remedy
    // chain) since the override is specifically about helping an
    // injured cat — gather-only or set-ward sub-modes wouldn't.
    if personality.compassion > sc.gate_compulsive_helper_threshold && has_injured_nearby {
        return Some(Action::HerbcraftRemedy);
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
/// 158: the `Action::Groom` umbrella retired into sibling
/// `Action::GroomSelf` + `Action::GroomOther`, each with a 1:1
/// disposition mapping (`Resting` and `Grooming` respectively). The
/// `self_groom_won` resolver parameter retired with the split — the
/// L3 softmax pick directly carries the self-vs-other distinction.
pub fn aggregate_to_dispositions(
    action_scores: &[(Action, f32)],
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
/// Current behavior vs. legacy path is preserved with one action-level
/// transform applied before softmax, so this function is a drop-in
/// replacement for the 2-step aggregate/softmax dance (documented inline
/// below).
///
/// 158: the `self_groom_won` parameter retired — the `Action::Groom`
/// umbrella split into `Action::GroomSelf` + `Action::GroomOther`, each
/// with a 1:1 disposition mapping. The L3 softmax pick directly carries
/// the self-vs-other distinction; no resolver in between.
///
/// `independence` is the cat's personality score; `sc` carries the
/// `intention_softmax_temperature` and `disposition_independence_penalty`
/// constants.
pub fn select_disposition_via_intention_softmax(
    scores: &[(Action, f32)],
    independence: f32,
    disposition_independence_penalty: f32,
    sc: &ScoringConstants,
    rng: &mut impl Rng,
) -> DispositionKind {
    select_disposition_via_intention_softmax_with_trace(
        scores,
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

    // Snapshot for the L2-vs-pool invariant in tests/scenarios.rs:
    // pairs with the caller's pre-bonus snapshot to detect any code
    // that mutates scores between score_actions exit and softmax entry.
    let pool_pre_penalty_snapshot = pool.clone();

    // Port of the legacy disposition-level Independence penalty on
    // Coordinating + Socializing peer-groups. Applied at action level here
    // on the constituent actions of those dispositions.
    //
    // 158: pre-split, `Action::Groom` carried the penalty when
    // `self_groom_won == false` (matching the legacy
    // `DispositionKind::Socializing` post-aggregation behavior). Post-split,
    // the constituent action of `Socializing` is just `Action::Socialize`;
    // `GroomOther` rides its own `DispositionKind::Grooming` and is not in
    // the penalty set. This matches the legacy intent: independence
    // suppresses *coordination-with-others* (Coordinate / Socialize /
    // Mentor) and *not* the affiliative-touch behavior, which was always
    // a Resting-tier act when self_groom_won.
    if independence > 0.0 {
        let penalty = independence * disposition_independence_penalty;
        for (action, score) in pool.iter_mut() {
            let penalize = matches!(
                action,
                Action::Coordinate | Action::Socialize | Action::Mentor
            );
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
            sink.pool_pre_penalty = pool_pre_penalty_snapshot;
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
        sink.pool_pre_penalty = pool_pre_penalty_snapshot;
    }

    // 158: 1:1 mapping for every dispositioned action. The pre-158
    // `Action::Groom` umbrella required a side-channel `self_groom_won`
    // resolver; the sibling-Action split makes the L3 pick directly
    // determinative.
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
///
/// `pre_bonus_pool` and `pool_pre_penalty` lock the §11.3 L2-vs-pool
/// invariant: the score Vec must be identical at score_actions exit
/// and at softmax entry. The locked test in `tests/scenarios.rs`
/// asserts equality per-action.
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
    /// Score Vec snapshot at `score_actions` exit. Empty for non-focal
    /// cats (caller skips the snapshot when there's no trace consumer).
    pub pre_bonus_pool: Vec<(Action, f32)>,
    /// Post-filter, pre-Independence-penalty pool the softmax saw.
    /// Empty on the early-return empty-pool branch.
    pub pool_pre_penalty: Vec<(Action, f32)>,
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

    /// Ticket 175: the carry-affinity mapping is the L2 carry-
    /// affinity bias's structural contract. Adding a new
    /// `Carrying` variant or a new boost-target DSE ID must
    /// land an arm here; this test fails closed against silent
    /// reordering / missing arms / typo'd DSE ID strings.
    #[test]
    fn carry_affinity_mapping_is_complete_and_correct() {
        use crate::ai::planner::Carrying;
        let bonus = 1.5;
        // Boosted: carry maps to a chain-consuming DSE.
        assert_eq!(apply_carry_affinity(10.0, "hunt", Carrying::Prey, bonus), 15.0);
        assert_eq!(apply_carry_affinity(10.0, "forage", Carrying::ForagedFood, bonus), 15.0);
        assert_eq!(apply_carry_affinity(10.0, "herbcraft_prepare", Carrying::Herbs, bonus), 15.0);
        assert_eq!(apply_carry_affinity(10.0, "herbcraft_ward", Carrying::Herbs, bonus), 15.0);
        assert_eq!(apply_carry_affinity(10.0, "apply_remedy_target", Carrying::Herbs, bonus), 15.0);
        assert_eq!(apply_carry_affinity(10.0, "build", Carrying::BuildMaterials, bonus), 15.0);

        // Not boosted: carry doesn't match the DSE's chain.
        assert_eq!(apply_carry_affinity(10.0, "cook", Carrying::Prey, bonus), 10.0);
        assert_eq!(apply_carry_affinity(10.0, "hunt", Carrying::Herbs, bonus), 10.0);
        assert_eq!(apply_carry_affinity(10.0, "forage", Carrying::BuildMaterials, bonus), 10.0);
        assert_eq!(apply_carry_affinity(10.0, "build", Carrying::Prey, bonus), 10.0);

        // Carrying::Nothing — the orthogonality baseline. Bias
        // is zero across every DSE.
        for dse_id in ["hunt", "forage", "cook", "herbcraft_prepare", "herbcraft_ward",
                       "apply_remedy_target", "build", "eat", "sleep", "wander"] {
            assert_eq!(
                apply_carry_affinity(10.0, dse_id, Carrying::Nothing, bonus),
                10.0,
                "Carrying::Nothing should never bias any DSE (saw bias on '{}')", dse_id
            );
        }

        // bonus = 1.0 disables the bias entirely.
        assert_eq!(apply_carry_affinity(10.0, "hunt", Carrying::Prey, 1.0), 10.0);
        assert_eq!(apply_carry_affinity(10.0, "build", Carrying::BuildMaterials, 1.0), 10.0);
    }

    /// 155: helper for tests that need to ask "did any of the six
    /// Witchcraft sub-actions score?".
    fn is_magic_subaction(a: Action) -> bool {
        matches!(
            a,
            Action::MagicScry
                | Action::MagicDurableWard
                | Action::MagicCleanse
                | Action::MagicColonyCleanse
                | Action::MagicHarvest
                | Action::MagicCommune
        )
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
            let scoring = crate::resources::sim_constants::ScoringConstants::default();
            let mut r = DseRegistry::new();
            r.cat_dses.push(crate::ai::dses::eat_dse());
            r.cat_dses.push(crate::ai::dses::hunt_dse(&scoring));
            r.cat_dses.push(crate::ai::dses::forage_dse(&scoring));
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
            r.cat_dses.push(crate::ai::dses::explore_dse(&scoring));
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
            // §075 — `default_modifier_pipeline` now takes `&SimConstants`.
            let constants = crate::resources::sim_constants::SimConstants::default();
            crate::ai::modifier::default_modifier_pipeline(&constants)
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
            // §4 batch 2: capability markers — default test cat is
            // capable of everything so DSE eligibility gates open.
            let cat = Entity::from_raw_u32(1).unwrap();
            s.set_entity(markers::CanHunt::KEY, cat, true);
            s.set_entity(markers::CanForage::KEY, cat, true);
            s.set_entity(markers::CanWard::KEY, cat, true);
            s.set_entity(markers::CanCook::KEY, cat, true);
            s
        })
    }

    fn cached_colony_landmarks() -> &'static crate::resources::ColonyLandmarks {
        static L: OnceLock<crate::resources::ColonyLandmarks> = OnceLock::new();
        // Place every colony building at (0, 0) so the §L2.10.7
        // spatial axes evaluate at the closest cost when the test
        // cat is at the origin (the default `test_eval_inputs()`
        // position). Tests that exercise distance-based behavior
        // override these in their own `EvalInputs` literal.
        L.get_or_init(|| crate::resources::ColonyLandmarks {
            kitchen: Some(Position::new(0, 0)),
            stores: Some(Position::new(0, 0)),
            garden: Some(Position::new(0, 0)),
        })
    }

    fn cached_exploration_map() -> &'static crate::resources::ExplorationMap {
        static M: OnceLock<crate::resources::ExplorationMap> = OnceLock::new();
        M.get_or_init(crate::resources::ExplorationMap::default)
    }

    fn cached_corruption_landmarks() -> &'static crate::resources::CorruptionLandmarks {
        static C: OnceLock<crate::resources::CorruptionLandmarks> = OnceLock::new();
        C.get_or_init(crate::resources::CorruptionLandmarks::default)
    }

    fn test_eval_inputs() -> EvalInputs<'static> {
        EvalInputs {
            cat: Entity::from_raw_u32(1).unwrap(),
            position: Position::new(0, 0),
            tick: 0,
            dse_registry: cached_registry(),
            modifier_pipeline: cached_modifier_pipeline(),
            markers: cached_test_markers(),
            colony_landmarks: cached_colony_landmarks(),
            exploration_map: cached_exploration_map(),
            corruption_landmarks: cached_corruption_landmarks(),
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

    /// §075: leak a singleton `DispositionConstants` so the test
    /// `ctx()` helper can borrow it immutably for the lifetime of
    /// each test without forcing every caller to construct one.
    /// Tests don't tune the disposition constants, so a single static
    /// default suffices for every per-test `ScoringContext`.
    fn default_disposition_constants() -> &'static crate::resources::sim_constants::DispositionConstants {
        use std::sync::OnceLock;
        static DC: OnceLock<crate::resources::sim_constants::DispositionConstants> =
            OnceLock::new();
        DC.get_or_init(crate::resources::sim_constants::DispositionConstants::default)
    }

    fn ctx<'a>(
        needs: &'a Needs,
        personality: &'a Personality,
        scoring: &'a ScoringConstants,
    ) -> ScoringContext<'a> {
        ScoringContext {
            scoring,
            disposition_constants: default_disposition_constants(),
            needs,
            personality,
            food_available: true,

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
            carrying: crate::ai::planner::Carrying::Nothing,

            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            disposition_started_tick: 0,
            tradition_location_bonus: 0.0,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            kitten_cry_perceived: 0.0,
            caretake_compassion_bond_scale: 1.0,
            unexplored_nearby: 1.0,
            health: 1.0,
            pain_level: 0.0,
            body_distress_composite: 0.0,
            mastery_confidence: 0.0,
            purpose_clarity: 0.0,
            esteem_distress: 0.0,
            escape_viability: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
            social_warmth_deficit: 0.4,
            cat_anchors: crate::ai::scoring::CatAnchorPositions { own_sleeping_spot: Some(Position::new(0, 0)), nearest_forageable_cluster: Some(Position::new(0, 0)), nearest_construction_site: Some(Position::new(0, 0)), nearest_herb_patch: Some(Position::new(0, 0)), nearest_perimeter_tile: Some(Position::new(0, 0)), territory_perimeter_anchor: Some(Position::new(0, 0)), nearest_corrupted_tile: Some(Position::new(0, 0)), nearest_threat: Some(Position::new(0, 0)), coordinator_perch: Some(Position::new(0, 0)), own_safe_rest_spot: Some(Position::new(0, 0)), own_injury_site: Some(Position::new(0, 0)) },
            disposition_failure_signal_hunting: 1.0,
            disposition_failure_signal_foraging: 1.0,
            disposition_failure_signal_crafting: 1.0,
            disposition_failure_signal_caretaking: 1.0,
            disposition_failure_signal_building: 1.0,
            disposition_failure_signal_mating: 1.0,
            disposition_failure_signal_mentoring: 1.0,
            memory_resource_found_proximity_sum: 0.0,
            memory_death_proximity_sum: 0.0,
            memory_threat_seen_proximity_sum: 0.0,
            colony_knowledge_resource_proximity: 0.0,
            colony_knowledge_threat_proximity: 0.0,
            colony_priority_ordinal: -1.0,
            cascade_counts: [0.0; CASCADE_COUNTS_LEN],
            aspiration_action_counts: [0.0; CASCADE_COUNTS_LEN],
            preference_signals: [0.0; CASCADE_COUNTS_LEN],
            fated_love_visible: 0.0,
            fated_rival_nearby: 0.0,
            active_directive_action_ordinal: -1.0,
            active_directive_bonus: 0.0,
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

        let scores = score_actions(
            &ctx(&needs, &personality, &sc),
            &test_eval_inputs(),
            &mut rng,
        )
        .scores;
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

        let scores = score_actions(
            &ctx(&needs, &personality, &sc),
            &test_eval_inputs(),
            &mut rng,
        )
        .scores;
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
            disposition_constants: default_disposition_constants(),
            needs: &needs,
            personality: &personality,
            food_available: false,

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
            carrying: crate::ai::planner::Carrying::Nothing,

            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            disposition_started_tick: 0,
            tradition_location_bonus: 0.0,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            kitten_cry_perceived: 0.0,
            caretake_compassion_bond_scale: 1.0,
            unexplored_nearby: 1.0,
            health: 1.0,
            pain_level: 0.0,
            body_distress_composite: 0.0,
            mastery_confidence: 0.0,
            purpose_clarity: 0.0,
            esteem_distress: 0.0,
            escape_viability: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
            social_warmth_deficit: 0.4,
            cat_anchors: crate::ai::scoring::CatAnchorPositions { own_sleeping_spot: Some(Position::new(0, 0)), nearest_forageable_cluster: Some(Position::new(0, 0)), nearest_construction_site: Some(Position::new(0, 0)), nearest_herb_patch: Some(Position::new(0, 0)), nearest_perimeter_tile: Some(Position::new(0, 0)), territory_perimeter_anchor: Some(Position::new(0, 0)), nearest_corrupted_tile: Some(Position::new(0, 0)), nearest_threat: Some(Position::new(0, 0)), coordinator_perch: Some(Position::new(0, 0)), own_safe_rest_spot: Some(Position::new(0, 0)), own_injury_site: Some(Position::new(0, 0)) },
            disposition_failure_signal_hunting: 1.0,
            disposition_failure_signal_foraging: 1.0,
            disposition_failure_signal_crafting: 1.0,
            disposition_failure_signal_caretaking: 1.0,
            disposition_failure_signal_building: 1.0,
            disposition_failure_signal_mating: 1.0,
            disposition_failure_signal_mentoring: 1.0,
            memory_resource_found_proximity_sum: 0.0,
            memory_death_proximity_sum: 0.0,
            memory_threat_seen_proximity_sum: 0.0,
            colony_knowledge_resource_proximity: 0.0,
            colony_knowledge_threat_proximity: 0.0,
            colony_priority_ordinal: -1.0,
            cascade_counts: [0.0; CASCADE_COUNTS_LEN],
            aspiration_action_counts: [0.0; CASCADE_COUNTS_LEN],
            preference_signals: [0.0; CASCADE_COUNTS_LEN],
            fated_love_visible: 0.0,
            fated_rival_nearby: 0.0,
            active_directive_action_ordinal: -1.0,
            active_directive_bonus: 0.0,
        };
        // §L2.10.7: this test sets `food_available: false`,
        // `has_functional_kitchen: false`, etc. on the context, but
        // the cached test MarkerSnapshot enables every colony
        // marker — Cook would now win via its spatial axis on the
        // (0,0) cached kitchen landmark. Build a marker snapshot
        // that drops the food-related markers to keep the scenario
        // honest: no food in stores, no functional kitchen.
        let mut markers = MarkerSnapshot::new();
        markers.set_colony(markers::WardStrengthLow::KEY, true);
        let cat_entity = Entity::from_raw_u32(1).unwrap();
        markers.set_entity(markers::CanForage::KEY, cat_entity, true);
        markers.set_entity(markers::CanHunt::KEY, cat_entity, true);
        markers.set_entity(markers::CanWard::KEY, cat_entity, true);
        let base = test_eval_inputs();
        let inputs = EvalInputs {
            cat: cat_entity,
            markers: &markers,
            ..base
        };
        let scores = score_actions(&c, &inputs, &mut rng).scores;
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

        let scores = score_actions(
            &ctx(&needs, &personality, &sc),
            &test_eval_inputs(),
            &mut rng,
        )
        .scores;
        let hunt_score = scores.iter().find(|(a, _)| *a == Action::Hunt).unwrap().1;
        let forage_score = scores.iter().find(|(a, _)| *a == Action::Forage).unwrap().1;

        assert!(
            hunt_score > forage_score,
            "bold cat should prefer Hunt ({hunt_score}) over Forage ({forage_score})"
        );
    }

    /// A diligent non-bold cat should prefer Forage over Hunt.
    ///
    /// Uses moderate hunger (0.5 → urgency=0.5) so the shared hangry
    /// axis sits at its midpoint (~0.5) and personality differentiators
    /// (boldness for Hunt, diligence for Forage) still drive the
    /// choice. Higher urgency would saturate the hunger axis on both
    /// DSEs — under `Logistic(8, 0.5)` after ticket 044 recalibration,
    /// hunger=0.2 puts urgency=0.8 → score ~0.95, drowning the
    /// personality signal. The test's intent ("diligent → Forage")
    /// only reads cleanly at hunger levels where personality still
    /// has comparable weight.
    #[test]
    fn diligent_cat_prefers_forage_over_hunt() {
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.hunger = 0.5;

        let mut personality = default_personality();
        personality.boldness = 0.2;
        personality.diligence = 0.9;

        let mut rng = seeded_rng(11);

        let scores = score_actions(
            &ctx(&needs, &personality, &sc),
            &test_eval_inputs(),
            &mut rng,
        )
        .scores;
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
            disposition_constants: default_disposition_constants(),
            needs: &needs,
            personality: &personality,
            food_available: true,

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
            carrying: crate::ai::planner::Carrying::Nothing,

            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            disposition_started_tick: 0,
            tradition_location_bonus: 0.0,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            kitten_cry_perceived: 0.0,
            caretake_compassion_bond_scale: 1.0,
            unexplored_nearby: 1.0,
            health: 1.0,
            pain_level: 0.0,
            body_distress_composite: 0.0,
            mastery_confidence: 0.0,
            purpose_clarity: 0.0,
            esteem_distress: 0.0,
            escape_viability: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
            social_warmth_deficit: 0.4,
            cat_anchors: crate::ai::scoring::CatAnchorPositions { own_sleeping_spot: Some(Position::new(0, 0)), nearest_forageable_cluster: Some(Position::new(0, 0)), nearest_construction_site: Some(Position::new(0, 0)), nearest_herb_patch: Some(Position::new(0, 0)), nearest_perimeter_tile: Some(Position::new(0, 0)), territory_perimeter_anchor: Some(Position::new(0, 0)), nearest_corrupted_tile: Some(Position::new(0, 0)), nearest_threat: Some(Position::new(0, 0)), coordinator_perch: Some(Position::new(0, 0)), own_safe_rest_spot: Some(Position::new(0, 0)), own_injury_site: Some(Position::new(0, 0)) },
            disposition_failure_signal_hunting: 1.0,
            disposition_failure_signal_foraging: 1.0,
            disposition_failure_signal_crafting: 1.0,
            disposition_failure_signal_caretaking: 1.0,
            disposition_failure_signal_building: 1.0,
            disposition_failure_signal_mating: 1.0,
            disposition_failure_signal_mentoring: 1.0,
            memory_resource_found_proximity_sum: 0.0,
            memory_death_proximity_sum: 0.0,
            memory_threat_seen_proximity_sum: 0.0,
            colony_knowledge_resource_proximity: 0.0,
            colony_knowledge_threat_proximity: 0.0,
            colony_priority_ordinal: -1.0,
            cascade_counts: [0.0; CASCADE_COUNTS_LEN],
            aspiration_action_counts: [0.0; CASCADE_COUNTS_LEN],
            preference_signals: [0.0; CASCADE_COUNTS_LEN],
            fated_love_visible: 0.0,
            fated_rival_nearby: 0.0,
            active_directive_action_ordinal: -1.0,
            active_directive_bonus: 0.0,
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
        // 158: `Action::Groom` retired into sibling variants — thermal
        // self-care now scores under `Action::GroomSelf`. The test
        // pins the underlying invariant: a cold cat should prefer
        // self-grooming over idling.
        let sc = default_scoring();
        let mut needs = Needs::default();
        needs.temperature = 0.1;
        needs.hunger = 0.9;
        needs.energy = 0.9;

        let personality = default_personality();
        let mut rng = seeded_rng(21);

        let scores = score_actions(
            &ctx(&needs, &personality, &sc),
            &test_eval_inputs(),
            &mut rng,
        )
        .scores;
        let groom_score = scores
            .iter()
            .find(|(a, _)| *a == Action::GroomSelf)
            .unwrap()
            .1;
        let idle_score = scores.iter().find(|(a, _)| *a == Action::Idle).unwrap().1;

        assert!(
            groom_score > idle_score,
            "cold cat should score GroomSelf ({groom_score}) above Idle ({idle_score})"
        );
    }

    // --- Memory bonus tests ---

    use crate::components::mental::{Memory, MemoryEntry, MemoryType};
    use crate::components::physical::Position;

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
    fn memory_proximity_sums_split_by_event_type() {
        let sc = default_scoring();
        let mut memory = Memory::default();
        memory.remember(make_memory(
            MemoryType::ResourceFound,
            Position::new(5, 5),
            1.0,
        ));
        memory.remember(make_memory(MemoryType::Death, Position::new(6, 5), 1.0));
        memory.remember(make_memory(MemoryType::ThreatSeen, Position::new(5, 6), 1.0));

        let (resource, death, threat) =
            memory_proximity_sums(&memory, &Position::new(5, 5), &sc);
        assert!(resource > 0.0, "resource sum should fire on ResourceFound");
        assert!(death > 0.0, "death sum should fire on Death");
        assert!(threat > 0.0, "threat sum should fire on ThreatSeen");
        // Same-tile ResourceFound: proximity 1.0, strength 1.0 → sum = 1.0.
        assert!((resource - 1.0).abs() < 1e-6);
    }

    #[test]
    fn memory_proximity_sums_decay_with_distance_and_strength() {
        let sc = default_scoring();
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
        let near = memory_proximity_sums(&memory_strong, &Position::new(5, 5), &sc).0;
        let far = memory_proximity_sums(&memory_strong, &Position::new(15, 5), &sc).0;
        let weak = memory_proximity_sums(&memory_weak, &Position::new(5, 5), &sc).0;
        assert!(near > far, "nearby > far; near={near}, far={far}");
        assert!(near > weak, "strong > weak; strong={near}, weak={weak}");
    }

    #[test]
    fn memory_proximity_sums_skip_out_of_radius() {
        let sc = default_scoring();
        let mut memory = Memory::default();
        // Place at distance 100 — beyond memory_nearby_radius (15).
        memory.remember(make_memory(
            MemoryType::ResourceFound,
            Position::new(105, 5),
            1.0,
        ));
        let (r, d, t) = memory_proximity_sums(&memory, &Position::new(5, 5), &sc);
        assert_eq!(r, 0.0);
        assert_eq!(d, 0.0);
        assert_eq!(t, 0.0);
    }

    #[test]
    fn cascade_counts_aggregate_nearby_actions_excluding_self_and_fight() {
        let self_entity = Entity::from_raw_u32(1).unwrap();
        let snapshot = vec![
            (self_entity, Position::new(5, 5), Action::Hunt),
            (Entity::from_raw_u32(2).unwrap(), Position::new(5, 6), Action::Hunt),
            (Entity::from_raw_u32(3).unwrap(), Position::new(6, 5), Action::Hunt),
            (Entity::from_raw_u32(4).unwrap(), Position::new(5, 4), Action::Fight),
            (Entity::from_raw_u32(5).unwrap(), Position::new(20, 20), Action::Hunt),
        ];
        let counts = compute_cascade_counts(&snapshot, self_entity, &Position::new(5, 5), 5);
        assert_eq!(counts[Action::Hunt as usize], 2.0, "self + far cat excluded");
        assert_eq!(counts[Action::Fight as usize], 0.0, "Fight excluded by design");
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
            disposition_constants: default_disposition_constants(),
            needs: &needs,
            personality: &personality,
            food_available: true,

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
            carrying: crate::ai::planner::Carrying::Nothing,

            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            disposition_started_tick: 0,
            tradition_location_bonus: 0.0,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            kitten_cry_perceived: 0.0,
            caretake_compassion_bond_scale: 1.0,
            unexplored_nearby: 1.0,
            health: 1.0,
            pain_level: 0.0,
            body_distress_composite: 0.0,
            mastery_confidence: 0.0,
            purpose_clarity: 0.0,
            esteem_distress: 0.0,
            escape_viability: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
            social_warmth_deficit: 0.4,
            cat_anchors: crate::ai::scoring::CatAnchorPositions { own_sleeping_spot: Some(Position::new(0, 0)), nearest_forageable_cluster: Some(Position::new(0, 0)), nearest_construction_site: Some(Position::new(0, 0)), nearest_herb_patch: Some(Position::new(0, 0)), nearest_perimeter_tile: Some(Position::new(0, 0)), territory_perimeter_anchor: Some(Position::new(0, 0)), nearest_corrupted_tile: Some(Position::new(0, 0)), nearest_threat: Some(Position::new(0, 0)), coordinator_perch: Some(Position::new(0, 0)), own_safe_rest_spot: Some(Position::new(0, 0)), own_injury_site: Some(Position::new(0, 0)) },
            disposition_failure_signal_hunting: 1.0,
            disposition_failure_signal_foraging: 1.0,
            disposition_failure_signal_crafting: 1.0,
            disposition_failure_signal_caretaking: 1.0,
            disposition_failure_signal_building: 1.0,
            disposition_failure_signal_mating: 1.0,
            disposition_failure_signal_mentoring: 1.0,
            memory_resource_found_proximity_sum: 0.0,
            memory_death_proximity_sum: 0.0,
            memory_threat_seen_proximity_sum: 0.0,
            colony_knowledge_resource_proximity: 0.0,
            colony_knowledge_threat_proximity: 0.0,
            colony_priority_ordinal: -1.0,
            cascade_counts: [0.0; CASCADE_COUNTS_LEN],
            aspiration_action_counts: [0.0; CASCADE_COUNTS_LEN],
            preference_signals: [0.0; CASCADE_COUNTS_LEN],
            fated_love_visible: 0.0,
            fated_rival_nearby: 0.0,
            active_directive_action_ordinal: -1.0,
            active_directive_bonus: 0.0,
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
            disposition_constants: default_disposition_constants(),
            needs: &needs,
            personality: &personality,
            food_available: true,

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
            carrying: crate::ai::planner::Carrying::Nothing,

            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            disposition_started_tick: 0,
            tradition_location_bonus: 0.0,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            kitten_cry_perceived: 0.0,
            caretake_compassion_bond_scale: 1.0,
            unexplored_nearby: 1.0,
            health: 1.0,
            pain_level: 0.0,
            body_distress_composite: 0.0,
            mastery_confidence: 0.0,
            purpose_clarity: 0.0,
            esteem_distress: 0.0,
            escape_viability: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
            social_warmth_deficit: 0.4,
            cat_anchors: crate::ai::scoring::CatAnchorPositions { own_sleeping_spot: Some(Position::new(0, 0)), nearest_forageable_cluster: Some(Position::new(0, 0)), nearest_construction_site: Some(Position::new(0, 0)), nearest_herb_patch: Some(Position::new(0, 0)), nearest_perimeter_tile: Some(Position::new(0, 0)), territory_perimeter_anchor: Some(Position::new(0, 0)), nearest_corrupted_tile: Some(Position::new(0, 0)), nearest_threat: Some(Position::new(0, 0)), coordinator_perch: Some(Position::new(0, 0)), own_safe_rest_spot: Some(Position::new(0, 0)), own_injury_site: Some(Position::new(0, 0)) },
            disposition_failure_signal_hunting: 1.0,
            disposition_failure_signal_foraging: 1.0,
            disposition_failure_signal_crafting: 1.0,
            disposition_failure_signal_caretaking: 1.0,
            disposition_failure_signal_building: 1.0,
            disposition_failure_signal_mating: 1.0,
            disposition_failure_signal_mentoring: 1.0,
            memory_resource_found_proximity_sum: 0.0,
            memory_death_proximity_sum: 0.0,
            memory_threat_seen_proximity_sum: 0.0,
            colony_knowledge_resource_proximity: 0.0,
            colony_knowledge_threat_proximity: 0.0,
            colony_priority_ordinal: -1.0,
            cascade_counts: [0.0; CASCADE_COUNTS_LEN],
            aspiration_action_counts: [0.0; CASCADE_COUNTS_LEN],
            preference_signals: [0.0; CASCADE_COUNTS_LEN],
            fated_love_visible: 0.0,
            fated_rival_nearby: 0.0,
            active_directive_action_ordinal: -1.0,
            active_directive_bonus: 0.0,
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
        // hangry anchor Logistic(8, 0.5) (recalibrated ticket 044)
        // returns ~0 at `hunger=1.0` (sated), which is correct
        // behavior but breaks the branch-coverage assertion this test
        // makes; the adjustment preserves intent while accommodating
        // the curve shape.
        let mut needs = Needs::default();
        needs.hunger = 0.4;
        needs.energy = 0.3;
        let personality = default_personality();
        let mut rng = seeded_rng(40);

        let c = ScoringContext {
            scoring: &sc,
            disposition_constants: default_disposition_constants(),
            needs: &needs,
            personality: &personality,
            food_available: true,

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
            carrying: crate::ai::planner::Carrying::Nothing,

            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            disposition_started_tick: 0,
            tradition_location_bonus: 0.0,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            kitten_cry_perceived: 0.0,
            caretake_compassion_bond_scale: 1.0,
            unexplored_nearby: 1.0,
            health: 1.0,
            pain_level: 0.0,
            body_distress_composite: 0.0,
            mastery_confidence: 0.0,
            purpose_clarity: 0.0,
            esteem_distress: 0.0,
            escape_viability: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
            social_warmth_deficit: 0.4,
            cat_anchors: crate::ai::scoring::CatAnchorPositions { own_sleeping_spot: Some(Position::new(0, 0)), nearest_forageable_cluster: Some(Position::new(0, 0)), nearest_construction_site: Some(Position::new(0, 0)), nearest_herb_patch: Some(Position::new(0, 0)), nearest_perimeter_tile: Some(Position::new(0, 0)), territory_perimeter_anchor: Some(Position::new(0, 0)), nearest_corrupted_tile: Some(Position::new(0, 0)), nearest_threat: Some(Position::new(0, 0)), coordinator_perch: Some(Position::new(0, 0)), own_safe_rest_spot: Some(Position::new(0, 0)), own_injury_site: Some(Position::new(0, 0)) },
            disposition_failure_signal_hunting: 1.0,
            disposition_failure_signal_foraging: 1.0,
            disposition_failure_signal_crafting: 1.0,
            disposition_failure_signal_caretaking: 1.0,
            disposition_failure_signal_building: 1.0,
            disposition_failure_signal_mating: 1.0,
            disposition_failure_signal_mentoring: 1.0,
            memory_resource_found_proximity_sum: 0.0,
            memory_death_proximity_sum: 0.0,
            memory_threat_seen_proximity_sum: 0.0,
            colony_knowledge_resource_proximity: 0.0,
            colony_knowledge_threat_proximity: 0.0,
            colony_priority_ordinal: -1.0,
            cascade_counts: [0.0; CASCADE_COUNTS_LEN],
            aspiration_action_counts: [0.0; CASCADE_COUNTS_LEN],
            preference_signals: [0.0; CASCADE_COUNTS_LEN],
            fated_love_visible: 0.0,
            fated_rival_nearby: 0.0,
            active_directive_action_ordinal: -1.0,
            active_directive_bonus: 0.0,
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
            scores
                .iter()
                .find(|(a, _)| *a == action)
                .map(|(_, s)| *s)
                .unwrap_or(0.0)
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
        // 155: Herbcraft fanned to 3 sub-actions; gather is the
        // expected entry when herbs are nearby.
        let herbcraft = scores.iter().find(|(a, _)| *a == Action::HerbcraftGather);

        assert!(
            herbcraft.is_some(),
            "spiritual cat with herbs nearby should score HerbcraftGather"
        );
        assert!(
            herbcraft.unwrap().1 > 0.0,
            "HerbcraftGather score should be positive"
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
        // 155: none of the three Herbcraft sub-actions should appear
        // when the cat has no herbs and no inventory.
        let any_herb = scores.iter().any(|(a, _)| {
            matches!(
                a,
                Action::HerbcraftGather
                    | Action::HerbcraftRemedy
                    | Action::HerbcraftSetWard
            )
        });
        assert!(!any_herb, "no herbs → no Herbcraft; scores: {scores:?}");
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
        // 155: PracticeMagic fanned to 6 sub-actions; below the
        // affinity gate, none of them should appear.
        let any_magic = scores.iter().any(|(a, _)| is_magic_subaction(*a));
        assert!(!any_magic, "below affinity threshold → no Magic sub-action");

        // Below prereqs: skill 0.1 < 0.2 threshold
        let mut c2 = ctx(&needs, &personality, &sc);
        c2.magic_affinity = 0.5;
        c2.magic_skill = 0.1;

        let scores2 = score_actions(&c2, &test_eval_inputs(), &mut rng).scores;
        let any_magic2 = scores2.iter().any(|(a, _)| is_magic_subaction(*a));
        assert!(!any_magic2, "below skill threshold → no Magic sub-action");
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
        // 155: a magical cat on a corrupted tile should score
        // MagicCleanse (the gated tile-targeted sub-action).
        let cleanse = scores.iter().find(|(a, _)| *a == Action::MagicCleanse);

        assert!(
            cleanse.is_some(),
            "magical cat on corrupted tile should score MagicCleanse"
        );
        assert!(
            cleanse.unwrap().1 > 0.0,
            "MagicCleanse score should be positive"
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
        // 155: a compassionate cat with remedy herbs and an injured
        // ally should score HerbcraftRemedy specifically (the apply-
        // remedy chain), not the gather-only or set-ward sub-modes.
        let herbcraft = scores.iter().find(|(a, _)| *a == Action::HerbcraftRemedy);

        assert!(
            herbcraft.is_some() && herbcraft.unwrap().1 > 0.15,
            "compassionate cat with remedy herbs and injured allies should score HerbcraftRemedy; got {herbcraft:?}"
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
            disposition_constants: default_disposition_constants(),
            needs: &needs,
            personality: &personality,
            food_available: false,

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
            carrying: crate::ai::planner::Carrying::Nothing,

            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            disposition_started_tick: 0,
            tradition_location_bonus: 0.0,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            kitten_cry_perceived: 0.0,
            caretake_compassion_bond_scale: 1.0,
            unexplored_nearby: 1.0,
            health: 1.0,
            pain_level: 0.0,
            body_distress_composite: 0.0,
            mastery_confidence: 0.0,
            purpose_clarity: 0.0,
            esteem_distress: 0.0,
            escape_viability: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
            social_warmth_deficit: 0.4,
            cat_anchors: crate::ai::scoring::CatAnchorPositions { own_sleeping_spot: Some(Position::new(0, 0)), nearest_forageable_cluster: Some(Position::new(0, 0)), nearest_construction_site: Some(Position::new(0, 0)), nearest_herb_patch: Some(Position::new(0, 0)), nearest_perimeter_tile: Some(Position::new(0, 0)), territory_perimeter_anchor: Some(Position::new(0, 0)), nearest_corrupted_tile: Some(Position::new(0, 0)), nearest_threat: Some(Position::new(0, 0)), coordinator_perch: Some(Position::new(0, 0)), own_safe_rest_spot: Some(Position::new(0, 0)), own_injury_site: Some(Position::new(0, 0)) },
            disposition_failure_signal_hunting: 1.0,
            disposition_failure_signal_foraging: 1.0,
            disposition_failure_signal_crafting: 1.0,
            disposition_failure_signal_caretaking: 1.0,
            disposition_failure_signal_building: 1.0,
            disposition_failure_signal_mating: 1.0,
            disposition_failure_signal_mentoring: 1.0,
            memory_resource_found_proximity_sum: 0.0,
            memory_death_proximity_sum: 0.0,
            memory_threat_seen_proximity_sum: 0.0,
            colony_knowledge_resource_proximity: 0.0,
            colony_knowledge_threat_proximity: 0.0,
            colony_priority_ordinal: -1.0,
            cascade_counts: [0.0; CASCADE_COUNTS_LEN],
            aspiration_action_counts: [0.0; CASCADE_COUNTS_LEN],
            preference_signals: [0.0; CASCADE_COUNTS_LEN],
            fated_love_visible: 0.0,
            fated_rival_nearby: 0.0,
            active_directive_action_ordinal: -1.0,
            active_directive_bonus: 0.0,
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
            disposition_constants: default_disposition_constants(),
            needs: &needs,
            personality: &personality,
            food_available: true,
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
            carrying: crate::ai::planner::Carrying::Nothing,

            colony_injury_count: 0,
            ward_strength_low: false,
            on_corrupted_tile: false,
            tile_corruption: 0.0,
            nearby_corruption_level: 0.0,
            on_special_terrain: false,
            is_coordinator_with_directives: false,
            pending_directive_count: 0,
            prey_nearby: true,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            disposition_started_tick: 0,
            tradition_location_bonus: 0.0,
            hungry_kitten_urgency: 0.0,
            is_parent_of_hungry_kitten: false,
            kitten_cry_perceived: 0.0,
            caretake_compassion_bond_scale: 1.0,
            unexplored_nearby: 1.0,
            health: 1.0,
            pain_level: 0.0,
            body_distress_composite: 0.0,
            mastery_confidence: 0.0,
            purpose_clarity: 0.0,
            esteem_distress: 0.0,
            escape_viability: 1.0,
            fox_scent_level: 0.0,
            carcass_nearby: false,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            wards_under_siege: false,
            day_phase: DayPhase::Dawn,
            has_functional_kitchen: false,
            has_raw_food_in_stores: false,
            social_warmth_deficit: 0.4,
            cat_anchors: crate::ai::scoring::CatAnchorPositions { own_sleeping_spot: Some(Position::new(0, 0)), nearest_forageable_cluster: Some(Position::new(0, 0)), nearest_construction_site: Some(Position::new(0, 0)), nearest_herb_patch: Some(Position::new(0, 0)), nearest_perimeter_tile: Some(Position::new(0, 0)), territory_perimeter_anchor: Some(Position::new(0, 0)), nearest_corrupted_tile: Some(Position::new(0, 0)), nearest_threat: Some(Position::new(0, 0)), coordinator_perch: Some(Position::new(0, 0)), own_safe_rest_spot: Some(Position::new(0, 0)), own_injury_site: Some(Position::new(0, 0)) },
            disposition_failure_signal_hunting: 1.0,
            disposition_failure_signal_foraging: 1.0,
            disposition_failure_signal_crafting: 1.0,
            disposition_failure_signal_caretaking: 1.0,
            disposition_failure_signal_building: 1.0,
            disposition_failure_signal_mating: 1.0,
            disposition_failure_signal_mentoring: 1.0,
            memory_resource_found_proximity_sum: 0.0,
            memory_death_proximity_sum: 0.0,
            memory_threat_seen_proximity_sum: 0.0,
            colony_knowledge_resource_proximity: 0.0,
            colony_knowledge_threat_proximity: 0.0,
            colony_priority_ordinal: -1.0,
            cascade_counts: [0.0; CASCADE_COUNTS_LEN],
            aspiration_action_counts: [0.0; CASCADE_COUNTS_LEN],
            preference_signals: [0.0; CASCADE_COUNTS_LEN],
            fated_love_visible: 0.0,
            fated_rival_nearby: 0.0,
            active_directive_action_ordinal: -1.0,
            active_directive_bonus: 0.0,
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

    // --- Disposition aggregation tests ---

    #[test]
    fn aggregate_maps_hunt_to_hunting() {
        let scores = vec![
            (Action::Hunt, 1.5),
            (Action::Eat, 1.0),
            (Action::Sleep, 0.8),
        ];
        let disp = aggregate_to_dispositions(&scores);
        let hunting = disp.iter().find(|(k, _)| *k == DispositionKind::Hunting);
        assert_eq!(hunting.unwrap().1, 1.5);
    }

    #[test]
    fn aggregate_takes_max_of_constituent_actions() {
        let scores = vec![(Action::Patrol, 0.5), (Action::Fight, 1.2)];
        let disp = aggregate_to_dispositions(&scores);
        let guarding = disp.iter().find(|(k, _)| *k == DispositionKind::Guarding);
        assert_eq!(guarding.unwrap().1, 1.2);
    }

    #[test]
    fn aggregate_routes_self_groom_to_resting() {
        // 158: `Action::GroomSelf` maps directly to Resting; no
        // `self_groom_won` resolver in the loop.
        let scores = vec![(Action::GroomSelf, 0.9), (Action::Eat, 0.5)];
        let disp = aggregate_to_dispositions(&scores);
        let resting = disp.iter().find(|(k, _)| *k == DispositionKind::Resting);
        assert_eq!(resting.unwrap().1, 0.9);
        // Should NOT appear under Socializing or Grooming.
        assert!(disp
            .iter()
            .all(|(k, _)| *k != DispositionKind::Socializing && *k != DispositionKind::Grooming));
    }

    #[test]
    fn aggregate_routes_other_groom_to_grooming() {
        // 158: `Action::GroomOther` maps to the new `Grooming`
        // disposition (single-action template). Pre-158 this routed
        // to Socializing via the side-channel `self_groom_won == false`
        // branch, but the equivalent-effect plan-template caused A* to
        // pre-prune `GroomOther` after `SocializeWith` claimed the
        // single goal-state.
        let scores = vec![(Action::GroomOther, 0.9), (Action::Socialize, 0.5)];
        let disp = aggregate_to_dispositions(&scores);
        let grooming = disp.iter().find(|(k, _)| *k == DispositionKind::Grooming);
        assert_eq!(grooming.unwrap().1, 0.9);
    }

    #[test]
    fn aggregate_excludes_flee_and_idle() {
        let scores = vec![
            (Action::Flee, 3.0),
            (Action::Idle, 0.1),
            (Action::Hunt, 1.0),
        ];
        let disp = aggregate_to_dispositions(&scores);
        assert!(disp.iter().all(|(k, _)| *k != DispositionKind::Resting
            || !disp.iter().any(|(dk, _)| *dk == DispositionKind::Resting)));
        // No disposition should have the Flee score
        assert!(disp.iter().all(|(_, s)| *s <= 1.0));
    }

    #[test]
    fn aggregate_omits_zero_score_dispositions() {
        let scores = vec![(Action::Hunt, 1.0)];
        let disp = aggregate_to_dispositions(&scores);
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
            Some(Action::HerbcraftRemedy),
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
    // 155: Herbcraft sub-action L3 emission tests
    //
    // The pre-155 tests asserted on `result.herbcraft_hint` — the
    // post-softmax tournament winner. Post-155 the L3 pool carries
    // each sub-action as its own first-class entry; we assert the
    // pool contains the expected dominant sub-action for each
    // substrate shape.
    // -----------------------------------------------------------------------

    #[test]
    fn herbcraft_remedy_dominates_when_compassionate_with_injured() {
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
        // Also set herbs nearby so gather could fire — remedy should still
        // win the L3 sub-action contest within the Herbalism family.
        c.has_herbs_nearby = true;

        let result = score_actions(&c, &test_eval_inputs(), &mut rng);
        let remedy = result
            .scores
            .iter()
            .find(|(a, _)| *a == Action::HerbcraftRemedy)
            .map(|(_, s)| *s)
            .unwrap_or(0.0);
        let gather = result
            .scores
            .iter()
            .find(|(a, _)| *a == Action::HerbcraftGather)
            .map(|(_, s)| *s)
            .unwrap_or(0.0);
        assert!(
            remedy > gather && remedy > 0.0,
            "remedy must dominate gather when compassion + injured; remedy={remedy}, gather={gather}"
        );
    }

    #[test]
    fn herbcraft_gather_appears_when_only_substrate_for_gathering() {
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
        // §4 batch 2: override CanWard = false so ward DSE is ineligible.
        let mut snap = MarkerSnapshot::new();
        let cat = Entity::from_raw_u32(1).unwrap();
        snap.set_colony(markers::HasStoredFood::KEY, true);
        snap.set_colony(markers::HasGarden::KEY, true);
        snap.set_colony(markers::HasFunctionalKitchen::KEY, true);
        snap.set_colony(markers::HasRawFoodInStores::KEY, true);
        snap.set_colony(markers::WardStrengthLow::KEY, true);
        snap.set_entity(markers::CanHunt::KEY, cat, true);
        snap.set_entity(markers::CanForage::KEY, cat, true);
        // CanWard intentionally absent — cat has no ward herbs.
        snap.set_entity(markers::CanCook::KEY, cat, true);
        let inputs = EvalInputs {
            cat,
            position: Position::new(0, 0),
            tick: 0,
            dse_registry: cached_registry(),
            modifier_pipeline: cached_modifier_pipeline(),
            markers: &snap,
            colony_landmarks: cached_colony_landmarks(),
            exploration_map: cached_exploration_map(),
            corruption_landmarks: cached_corruption_landmarks(),
            focal_cat: None,
            focal_capture: None,
        };

        let result = score_actions(&c, &inputs, &mut rng);
        let gather = result.scores.iter().find(|(a, _)| *a == Action::HerbcraftGather);
        assert!(gather.is_some() && gather.unwrap().1 > 0.0,
            "with only herbs nearby, HerbcraftGather should score positive; scores: {:?}",
            result.scores);
    }

    #[test]
    fn herbcraft_setward_appears_when_ward_substrate_present() {
        let sc = default_scoring();
        // Ward is Level 2 (Safety) — only physiological needs matter.
        let mut needs = Needs::default();
        needs.hunger = 0.8;
        needs.energy = 0.8;

        let mut personality = default_personality();
        personality.spirituality = 0.9;

        let mut rng = seeded_rng(72);

        let mut c = ctx(&needs, &personality, &sc);

        c.has_herbs_in_inventory = true;
        c.ward_strength_low = true;
        c.herbcraft_skill = 0.3;
        // No herbs nearby, no remedy herbs — only ward is viable.

        let result = score_actions(&c, &test_eval_inputs(), &mut rng);
        let ward = result.scores.iter().find(|(a, _)| *a == Action::HerbcraftSetWard);
        assert!(ward.is_some() && ward.unwrap().1 > 0.0,
            "with ward herbs + low ward strength, HerbcraftSetWard should score positive; scores: {:?}",
            result.scores);
    }

    #[test]
    fn ward_score_rises_with_territory_corruption() {
        // §L2.10.7 retired: HerbcraftWard's `territory_max_corruption`
        // Logistic axis was replaced with a NearestPerimeterTile spatial
        // axis. Corruption-sensing for the magic system flows through
        // DurableWardDse (NearestCorruptedTile anchor) and
        // ColonyCleanseDse (TerritoryCorruptionCentroid anchor) — both
        // ported in B11/B12. HerbcraftWard scores are now driven by
        // skill + spirituality + perimeter proximity, independent of
        // corruption.
        //
        // This test's premise (corruption → stronger ward pull) no
        // longer matches the spec; the equivalent invariant now lives
        // on DurableWard's hotspot axis (assertion in
        // dses/practice_magic.rs::durable_ward_uses_corruption_hotspot_anchor).
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
            0.0,
            0.0,
            &sc,
            &mut rng,
            Some(&mut capture),
        );
        assert_eq!(capture.probabilities.len(), 2);
        let sum: f32 = capture.probabilities.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-4,
            "probabilities sum to {sum}, expected ~1"
        );
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
            0.0,
            0.0,
            &sc,
            &mut rng,
            Some(&mut capture),
        );
        assert_eq!(
            chosen,
            crate::components::disposition::DispositionKind::Resting
        );
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
        let scores = vec![
            (Action::Eat, 0.6),
            (Action::Socialize, 0.5),
            (Action::Patrol, 0.4),
        ];
        let plain = select_disposition_via_intention_softmax(
            &scores,
            0.0,
            0.0,
            &sc,
            &mut seeded_rng(99),
        );
        let traced = select_disposition_via_intention_softmax_with_trace(
            &scores,
            0.0,
            0.0,
            &sc,
            &mut seeded_rng(99),
            None,
        );
        assert_eq!(plain, traced);
    }

    #[test]
    fn explore_score_drops_with_saturation() {
        // Ticket 001 Sub-2 (post-§L2.10.7): Explore's exploration-axis
        // saturation moved from a `unexplored_nearby` Logistic scalar
        // to a `LandmarkAnchor::UnexploredFrontierCentroid` Linear
        // distance. The shape-level test now lives in
        // `src/ai/dses/explore.rs::tests::explore_frontier_distance_falls_off_linearly`.
        //
        // This higher-level scoring test just verifies the DSE still
        // produces above-zero scores when an unexplored frontier
        // exists (the cached test landmarks default to `None` for the
        // frontier centroid, so the spatial axis returns 0.0 — Explore
        // scores 0.0 too under CP). With the centroid set near the
        // cat, the score is positive.
        let sc = default_scoring();
        let needs = Needs::default();
        let mut personality = default_personality();
        personality.curiosity = 0.7;

        let ctx_fresh = ctx(&needs, &personality, &sc);

        let mut exploration_with_frontier = crate::resources::ExplorationMap::default();
        exploration_with_frontier.recompute_frontier_centroid(
            crate::resources::exploration_map::FRONTIER_THRESHOLD,
        );
        // Default 120×90 map → centroid at (60, 45). Place the cat
        // there so the spatial axis evaluates near 1.0 instead of
        // saturating to zero at the 40-tile range edge.
        let base = test_eval_inputs();
        let inputs_fresh = EvalInputs {
            position: Position::new(60, 45),
            exploration_map: &exploration_with_frontier,
            ..base
        };
        let score_fresh = score_dse_by_id("explore", &ctx_fresh, &inputs_fresh);
        // No frontier (default empty map): centroid = None → spatial
        // axis = 0 → CP gates Explore to 0.
        let score_no_frontier = score_dse_by_id("explore", &ctx_fresh, &test_eval_inputs());
        assert!(
            score_fresh > 0.0,
            "explore should score > 0 with a frontier; got {score_fresh}"
        );
        assert!(
            score_no_frontier <= 0.001,
            "explore should be gated near 0 without a frontier centroid; got {score_no_frontier}"
        );
    }

    // -----------------------------------------------------------------------
    // §075 commitment-tenure progress producer
    // -----------------------------------------------------------------------

    #[test]
    fn commitment_tenure_progress_zero_at_switch_tick() {
        // Cat just switched: tick == disposition_started_tick →
        // progress = 0.0 (full lift window remains).
        let p = commitment_tenure_progress(true, 100, 100, 200);
        assert!((p - 0.0).abs() < 1e-6, "got {p}");
    }

    #[test]
    fn commitment_tenure_progress_climbs_linearly_through_window() {
        // Halfway through a 200-tick window → progress = 0.5.
        let p = commitment_tenure_progress(true, 100, 200, 200);
        assert!((p - 0.5).abs() < 1e-6, "got {p}");
    }

    #[test]
    fn commitment_tenure_progress_saturates_at_one() {
        // Past the window edge → progress saturates at 1.0
        // (modifier short-circuits, no lift).
        let p = commitment_tenure_progress(true, 100, 5_000, 200);
        assert!((p - 1.0).abs() < 1e-6, "got {p}");
    }

    #[test]
    fn commitment_tenure_progress_one_when_no_active_disposition() {
        // No incumbent ⇒ producer reports 1.0 so the modifier's
        // outside-window short-circuit fires (no lift to apply).
        let p = commitment_tenure_progress(false, 100, 150, 200);
        assert!((p - 1.0).abs() < 1e-6, "got {p}");
    }

    #[test]
    fn commitment_tenure_progress_one_when_min_tenure_zero() {
        // Defensive: knob set to 0 effectively disables the modifier.
        // Returns 1.0 so the modifier short-circuits — and avoids the
        // divide-by-zero that would otherwise hit the elapsed/min calc.
        let p = commitment_tenure_progress(true, 100, 150, 0);
        assert!((p - 1.0).abs() < 1e-6, "got {p}");
    }

    #[test]
    fn commitment_tenure_progress_clock_rewind_floors_at_zero() {
        // Defensive: tick < disposition_started_tick (e.g., a save-load
        // restore that snapshots the started_tick from a later world
        // state). Saturating subtraction keeps progress at 0.0 rather
        // than wrapping into a huge u64 → 1.0 saturation mid-window.
        let p = commitment_tenure_progress(true, 500, 100, 200);
        assert!((p - 0.0).abs() < 1e-6, "got {p}");
    }
}
