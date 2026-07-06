use std::collections::HashMap;

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::pathfinding::{find_path, step_toward};
use crate::ai::planner::actions::actions_for_disposition;
use crate::ai::planner::goals::goal_for_disposition;
use crate::ai::planner::{
    make_plan, Carrying, GoapActionKind, PlannerState, PlannerZone, ZoneDistances,
};
use crate::ai::scoring::{score_actions, ScoringContext};
use crate::ai::{Action, CurrentAction};
use crate::components::building::{
    ConstructionSite, CropState, StoredItems, Structure, StructureType,
};
use crate::components::coordination::{ActiveDirective, Directive, DirectiveKind, DirectiveQueue};
use crate::components::disposition::{ActionHistory, ActionOutcome, ActionRecord, DispositionKind};
use crate::components::goap_plan::{
    GoapPlan, PendingUrgencies, PlanEvent, PlanNarrative, StepExecutionState, UrgencyKind,
    UrgentNeed,
};
use crate::components::identity::{Gender, LifeStage, Name};
use crate::components::items::{Item, ItemKind};
use crate::components::magic::{Harvestable, Herb, HerbKind, Inventory, Ward};
use crate::components::markers;
use crate::components::mental::Memory;
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Health, Needs, Position};
use crate::components::prey::{
    DenRaided, PreyAnimal, PreyConfig, PreyDen, PreyDensity, PreyKilled, PreyKind, PreyState,
};
use crate::components::skills::{Corruption, MagicAffinity, Skills};
use crate::components::wildlife::WildAnimal;
use crate::resources::event_log::{EventKind, EventLog, HuntOutcome};
use crate::resources::exploration_map::ExplorationMap;
use crate::resources::food::FoodStores;
use crate::resources::map::{Terrain, TileMap};
use crate::resources::narrative_templates::{
    emit_event_narrative, MoodBucket, TemplateContext, VariableContext,
};
use crate::resources::relationships::Relationships;
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::{DispositionConstants, SimConstants};
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{DayPhase, Season, TimeState};

// ===========================================================================
// SystemParam bundles — keep system param counts under Bevy's 16-param limit
// ===========================================================================

#[derive(bevy_ecs::system::SystemParam)]
pub struct PreyHuntParams<'w, 's> {
    pub density: Res<'w, PreyDensity>,
    pub kill_writer: MessageWriter<'w, PreyKilled>,
    pub raid_writer: MessageWriter<'w, DenRaided>,
    pub exploration_map: ResMut<'w, crate::resources::ExplorationMap>,
    pub health_query: Query<'w, 's, &'static Health, With<PreyAnimal>>,
    /// Ticket 062 — per-species scent-detection registry. Cats sample
    /// `highest_nearby_any(pos, scent_search_radius)` (max-aggregate
    /// across all five sub-maps) to find prey-scent source tiles rather
    /// than running point-to-point `cat_smells_prey_windaware` against
    /// each prey entity. Per-species reads via
    /// `highest_nearby_for(kind, …)` are the dietary-specialization
    /// hook for future Hunt-DSE work.
    pub prey_scent_maps: Res<'w, crate::resources::PreyScentMaps>,
    /// Ticket 223 — fox scent map, read by cat A* path-cost overlays so
    /// cats route around fox territory. Lives in `PreyHuntParams`
    /// alongside `prey_scent_maps` because both are wildlife-scent
    /// substrates consumed by the same cat-side resolvers and because
    /// `resolve_disposition_chains` is at Bevy's 16-param ceiling —
    /// bundling here avoids a SystemParam refactor at the use sites.
    pub fox_scent_map: Res<'w, crate::resources::FoxScentMap>,
    /// Ticket 100 — aggregate substrate-vibration field. Read by
    /// `resolve_engage_prey` to (a) modulate `effective_stalk_distance`
    /// with the prey's ambient tremor reading and (b) drive the
    /// patient-cat opportunity-quality assessment before committing to
    /// the approach.
    pub tremor_map: Res<'w, crate::resources::TremorMap>,
}

#[derive(bevy_ecs::system::SystemParam)]
pub struct NarrativeEmitter<'w> {
    pub log: ResMut<'w, crate::resources::narrative::NarrativeLog>,
    pub registry: Option<Res<'w, crate::resources::narrative_templates::TemplateRegistry>>,
    pub config: Res<'w, crate::resources::time::SimConfig>,
    pub weather: Res<'w, crate::resources::weather::WeatherState>,
    pub activation: Option<ResMut<'w, SystemActivation>>,
    /// Ticket 127 — bias-reader call sites emit
    /// `JointInteractionObserved` here when an amplification fires.
    /// `author_joint_intentions` consumes the batch on the following
    /// tick to bump `JointIntention.last_interaction_tick`, which
    /// gates the `Approach → Courting` stage advance.
    pub joint_interaction:
        bevy_ecs::message::MessageWriter<'w, crate::ai::joint_intention::JointInteractionObserved>,
    /// 258 — C3 belief substrate. Resolvers emit observable side-effects
    /// here; `belief_integrator` consumes the messages and updates each
    /// in-range witness's mental models via EMA.
    pub witnessable:
        bevy_ecs::message::MessageWriter<'w, crate::messages::witnessable_event::WitnessableEvent>,
}

/// Bundles world-state queries for evaluate_and_plan to stay under 16 params.
#[allow(clippy::type_complexity)]
#[derive(bevy_ecs::system::SystemParam)]
pub struct WorldStateQueries<'w, 's> {
    pub all_positions:
        Query<'w, 's, (Entity, &'static Position, Option<&'static PreyAnimal>), Without<Dead>>,
    /// Ticket 138 — widened with `Option<&MovementBudget>` (field-level
    /// edit; safe under Bevy's schedule-edge-perturbation rule) so the
    /// ScoringContext builder can pass the threat's `per_tick` cadence
    /// into `escape_viability`. `Option<...>` defends against pre-138
    /// save-loaded wildlife missing the component.
    pub wildlife: Query<
        'w,
        's,
        (
            Entity,
            &'static Position,
            Option<&'static crate::components::MovementBudget>,
        ),
        With<WildAnimal>,
    >,
    pub building_query: Query<
        'w,
        's,
        (
            Entity,
            &'static Structure,
            &'static Position,
            Option<&'static ConstructionSite>,
            Option<&'static CropState>,
        ),
    >,
    pub herb_query: Query<'w, 's, (Entity, &'static Herb, &'static Position), With<Harvestable>>,
    pub ward_query: Query<'w, 's, (&'static Ward, &'static Position)>,
    pub directive_queue_query: Query<'w, 's, (Entity, &'static DirectiveQueue)>,
    pub active_directive_query: Query<'w, 's, &'static ActiveDirective>,
    // Ticket 014 Mentoring batch — `skills_query` retired alongside the
    // `has_mentoring_target_fn` closure (its only consumer).
    // Ticket 064 (§5.6.3 #6 cutover) retired the `carcass_query` snapshot
    // here; carcass scent is sampled from `ColonyContext.carcass_scent_map`.
    // The mutable resolver query lives on `MagicResolverParams`.
    pub wildlife_ai_query:
        Query<'w, 's, &'static crate::components::wildlife::WildlifeAiState, With<WildAnimal>>,
    pub stored_items_query: Query<'w, 's, &'static crate::components::building::StoredItems>,
    /// Read-only items query. Excludes ground-build-material items via
    /// `Without<BuildMaterialItem>` so it stays disjoint from
    /// `BuildingResolverParams::material_items` (mutable). Build-material
    /// items are not relevant to food/herb resolvers anyway.
    pub items_query: Query<
        'w,
        's,
        &'static crate::components::items::Item,
        Without<crate::components::items::BuildMaterialItem>,
    >,
    /// Ground build-material entities, used to author the `MaterialPile`
    /// planner zone in `evaluate_and_plan`. Disjoint from cats via
    /// `Without<GoapPlan>` and from buildings via `Without<Structure>`;
    /// disjoint from `items_query` via `With<BuildMaterialItem>`.
    pub material_items_query: Query<
        'w,
        's,
        (
            Entity,
            &'static crate::components::items::Item,
            &'static Position,
        ),
        (
            Without<GoapPlan>,
            Without<Structure>,
            With<crate::components::items::BuildMaterialItem>,
        ),
    >,
    /// Ticket 193: ground food/non-material item entities, used to author
    /// the `CarcassPile` planner zone (today's source: engage_prey
    /// overflow Items at `OnGround`; forward-compatible with carcass-
    /// as-container child Items once loot tables land). Mirror of
    /// `material_items_query` with the inverted `BuildMaterialItem`
    /// gate, which makes the two queries statically disjoint. Read-only
    /// `&Item` access keeps it disjoint from the cats query
    /// (cats lack `Item`) and `Without<BuildMaterialItem>` keeps it
    /// disjoint from `BuildingResolverParams::material_items`. The
    /// caller filters this set down to `OnGround` + `kind.is_food()`
    /// at snapshot time.
    pub food_items_query: Query<
        'w,
        's,
        (
            Entity,
            &'static crate::components::items::Item,
            &'static Position,
        ),
        (
            Without<GoapPlan>,
            Without<Structure>,
            Without<crate::components::items::BuildMaterialItem>,
        ),
    >,
    /// Phase 4c.3: kittens + their hunger + parentage for Caretake
    /// urgency wiring. Disjoint from the adult cats query by
    /// `With<KittenDependency>` — kittens carry the marker until the
    /// growth system strips it.
    pub kitten_query: Query<
        'w,
        's,
        (
            Entity,
            &'static Position,
            &'static crate::components::physical::Needs,
            &'static crate::components::KittenDependency,
        ),
        Without<Dead>,
    >,
    // Ticket 433 retired `colony_state_query` here in favor of
    // `PlanResources::world_snapshots.colony_markers`. The marker
    // booleans are now cached once per tick by
    // `populate_world_snapshots` and read from the resource; see
    // `docs/systems/world-snapshots.md`. Removing the field also
    // dropped a SystemParam slot, freeing future per-cat query
    // additions inside `WorldStateQueries`.
    /// Ticket 109 (Phase A) — read-only Age lookup for the focal cat's
    /// `nearest_other` to feed `social_status_distress`'s `age_diff`
    /// arm. Disjoint from the per-cat iteration query because both
    /// access `Age` immutably (Bevy allows aliased read-only borrows).
    pub age_query: Query<'w, 's, &'static crate::components::identity::Age, Without<Dead>>,
    /// Ticket 109 (Phase A) — read-only Needs lookup for cross-cat
    /// `respect` reads in `social_status_distress`'s `respect_diff`
    /// arm. Read-only alias of the per-cat iteration query's `&Needs`
    /// borrow.
    pub needs_query: Query<'w, 's, &'static Needs, Without<Dead>>,
    /// 035 — Dead-and-not-Buried colony cat positions for the
    /// `PlannerZone::CorpseTarget` zone in `build_zone_distances`.
    /// Disjoint from `all_positions` (which filters `Without<Dead>`).
    pub dead_cat_query: Query<
        'w,
        's,
        (Entity, &'static Position),
        (
            With<crate::components::identity::Species>,
            With<Dead>,
            Without<markers::Buried>,
        ),
    >,
    /// Ticket 246 — read-only `HeldIntention` lookup at the L2 author
    /// site (`evaluate_and_plan`'s `ScoringContext` construction). Feeds
    /// the three `IntentionMomentum` scalars that ticket 126 left
    /// dormant. Read-only and disjoint from the per-cat mutable
    /// iteration query because `&HeldIntention` doesn't conflict with
    /// the cats query's `&mut`-bound fields. Sister read-only query to
    /// `ExecutorContext.held_intentions` (in `resolve_goap_plans`) —
    /// both are read-only on the same archetype, permitted by Bevy's
    /// borrow checker.
    pub held_intentions: Query<'w, 's, &'static crate::components::HeldIntention>,
    /// Ticket 364 — read-only `HeldGoalStack` lookup at the L2 adopt
    /// hook. When the cat carries a non-empty stack whose top frame's
    /// current sub-goal is a `Primitive`, the adopt hook overrides
    /// `(chosen, chosen_action)` to the leaf's action and routes the
    /// plan-template through `htn_primitive_actions`. Disjoint from the
    /// per-cat mutable iteration query — `&HeldGoalStack` is read-only.
    pub held_goal_stacks: Query<'w, 's, &'static crate::components::HeldGoalStack>,
    /// 263 — read-only `LocationBeliefs` lookup for the
    /// `patrol_threat_recency` precomputed scalar at `ScoringContext`
    /// construction. Reads the cat's per-location facet at the patrol
    /// perimeter anchor bucket so the Patrol DSE (and any future
    /// location-keyed consumer) doesn't have to thread a separate
    /// query. Disjoint from the cats query because `&LocationBeliefs`
    /// is read-only here and the cats query never writes the
    /// component on the same iteration tick (the `belief_integrator`
    /// system writes once per tick before scoring).
    pub location_beliefs: Query<'w, 's, &'static crate::components::beliefs::LocationBeliefs>,
    /// 268 — read-only `PredatorBeliefs` lookup for the Hide DSE's
    /// per-target belief-facet scalars (`hide_recency_of_threat_cue`,
    /// `hide_perceived_intent_clarity`). Reads the cat's MentalModel
    /// of the `nearest_threat_entity`. Disjoint from the cats query
    /// for the same reason as `location_beliefs` — read-only on a
    /// Component the belief_integrator writes once per tick before
    /// scoring.
    pub predator_beliefs: Query<'w, 's, &'static crate::components::beliefs::PredatorBeliefs>,
    /// 268 — read-only `ContextBeliefs` lookup for the ambient-shock
    /// fallback. When `nearest_threat_entity` is `None` but
    /// `ContextBeliefs[HereNow].recency_of_threat_cue` is elevated
    /// (e.g. door-slam), the Hide DSE's recency axis reads from this
    /// path instead of `PredatorBeliefs`.
    pub context_beliefs: Query<'w, 's, &'static crate::components::beliefs::ContextBeliefs>,
    /// 487 — read-only `CurrentAction` lookup for cats with an active
    /// `GoapPlan`, so `evaluate_and_plan` can identify "currently being
    /// groomed" peers before authoring `HasGroomCandidate`. Disjoint
    /// from the per-cat iteration query (which is `Without<GoapPlan>`)
    /// and from the `held_intentions` / `held_goal_stacks` queries
    /// (different components). Read-only; `resolve_goap_plans` writes
    /// `CurrentAction` later in the schedule, so the values seen here
    /// reflect last tick's executor resolution — appropriate for the
    /// "who is being groomed right now" predicate.
    pub groom_actor_query:
        Query<'w, 's, (Entity, &'static CurrentAction), (With<GoapPlan>, Without<Dead>)>,
}

/// Bundles resources for evaluate_and_plan.
#[derive(bevy_ecs::system::SystemParam)]
pub struct PlanResources<'w, 's> {
    pub map: Res<'w, TileMap>,
    pub food: Res<'w, FoodStores>,
    pub relationships: Res<'w, Relationships>,
    pub constants: Res<'w, SimConstants>,
    pub time: Res<'w, TimeState>,
    pub colony_center: Res<'w, crate::resources::ColonyCenter>,
    pub dse_registry: Res<'w, crate::ai::eval::DseRegistry>,
    pub modifier_pipeline: Res<'w, crate::ai::eval::ModifierPipeline>,
    /// §11 focal-cat target, absent in every interactive build and in
    /// headless runs without `--focal-cat`. Read-only here so
    /// `score_dse_by_id` can gate trace capture on
    /// `focal_target.entity == Some(cat)`.
    pub focal_target: Option<Res<'w, crate::resources::FocalTraceTarget>>,
    /// §11 rich-trace capture sink. Same gating as `focal_target`.
    /// Uses interior-`Mutex` so `EvalInputs` holds a shared reference.
    pub focal_capture: Option<Res<'w, crate::resources::FocalScoreCapture>>,
    /// Ticket 126 — `Feature::IntentionAdopted` writer for the L2
    /// author site. `Option<ResMut>` because some test harnesses don't
    /// register the resource; matches `NarrativeEmitter`'s pattern in
    /// `resolve_goap_plans`.
    pub activation: Option<ResMut<'w, SystemActivation>>,
    /// Ticket 320 — HTN method registry. Read when the winning DSE's
    /// emitted `Intention::Goal { state }` matches a Live method's
    /// `goal_label`, in which case the L2 author site pushes a
    /// `GoalFrame` onto the cat's `HeldGoalStack`. At 320's land the
    /// registry holds only `PendingSubstrate` entries and the gate
    /// never fires; 321's picker (which authors the first `Goal`-
    /// shaped intentions) and 323's `courtship_method` (the first
    /// `Live` method) are the tickets that exercise this path.
    pub method_registry: Res<'w, crate::ai::methods::MethodRegistry>,
    /// Ticket 463 — recipe catalog for the HaveItem aspiration
    /// dispatch. When the cat holds
    /// `Intention::Goal(GoalKind::HaveItem(item))`, the L2 author
    /// site reads this registry to build the templated plan
    /// (`RetrieveCraftInputs(recipe.id)` prefix + zone travel +
    /// `CraftAt<Station>(recipe.id)`) via
    /// `crate::ai::planner::actions::craft_have_item_actions`.
    pub recipes: Res<'w, crate::resources::recipe_registry::RecipeRegistry>,
    /// 258 — `WitnessableEvent` emit from the
    /// `make_plan → None` site. Bundled here rather than as a separate
    /// SystemParam to keep `evaluate_and_plan` under Bevy's 16-param
    /// ceiling.
    pub witnessable:
        bevy_ecs::message::MessageWriter<'w, crate::messages::witnessable_event::WitnessableEvent>,
    /// Ticket 427 Step 1 — pre-allocated scratch buffers for the
    /// target-taking DSE resolvers (`resolve_*_target` under
    /// `src/ai/dses/`). Each wrapper clears its own slots and writes
    /// in-place so the underlying `Vec` / `HashMap` capacities persist
    /// across cat-ticks. ~355 MB/soak alloc reduction at the 500-cat
    /// projection.
    pub dse_scratchpad: ResMut<'w, crate::resources::DseTargetScratchpad>,
    /// Ticket 427 Step 2 — per-system bucket arena for
    /// `route_cost::flood_dijkstra`. Bundled into `PlanResources` rather
    /// than added as a top-level param so `evaluate_and_plan` stays
    /// under Bevy's 16-param ceiling. Outer Vec grows once to the flood
    /// budget; inner Vecs preserve capacity via the `mem::swap`-drain
    /// pattern inside the flood function.
    pub route_buckets:
        bevy_ecs::prelude::Local<'s, Vec<Vec<crate::components::physical::Position>>>,
    /// Ticket 427 Step 3 — cat A* planner scratch arena. Held as
    /// `Local<>` inside `PlanResources` so `evaluate_and_plan` stays
    /// under Bevy's 16-param ceiling. Preserves `Vec<SearchNode>` +
    /// `BinaryHeap` + `HashMap` capacities across cat-ticks (~20 MB/
    /// soak saved per the 427 survey).
    pub planner_scratch: bevy_ecs::prelude::Local<'s, crate::ai::planner::CatPlannerScratch>,
    /// Ticket 433 — cross-system per-tick snapshot (colony markers +
    /// food fraction + food_available boolean). Populated by
    /// `populate_world_snapshots` after every marker-author system
    /// runs; read here instead of `colony_state_query.single()` /
    /// `food.fraction()` inline calls. See
    /// `docs/systems/world-snapshots.md` for the substrate intent
    /// and the planned follow-on hoists.
    pub world_snapshots: Res<'w, crate::resources::world_snapshots::WorldSnapshots>,
}

/// Bundles magic resolver dependencies to keep resolve_goap_plans under 16 params.
/// The herb_query reads `&Position` (immutable), which would conflict with the
/// cats query's `&mut Position`. Disjointness is ensured by `Without<Herb>` on
/// the cats filter (herbs are never cats).
#[derive(bevy_ecs::system::SystemParam)]
pub struct MagicResolverParams<'w, 's> {
    pub herb_query: Query<
        'w,
        's,
        (
            Entity,
            &'static Herb,
            &'static crate::components::physical::Position,
        ),
        With<Harvestable>,
    >,
    pub pushback_writer: MessageWriter<'w, crate::systems::magic::CorruptionPushback>,
    /// Ticket 471 — emitted by `apply_misfire` so the per-event misfire
    /// stream is visible to the event log and to the festering-wound
    /// authoring path (ticket 472).
    pub misfire_writer: MessageWriter<'w, crate::messages::misfire_effect::MisfireEffect>,
    pub carcass_query: Query<
        'w,
        's,
        (
            Entity,
            &'static mut crate::components::wildlife::Carcass,
            &'static crate::components::physical::Position,
        ),
    >,
    /// Lookup of ActiveDirective by entity — used by Cleanse/HarvestCarcass
    /// resolvers to route the cat to the coordinator-specified target tile.
    pub active_directive_query: Query<'w, 's, &'static ActiveDirective>,
}

/// Bundles building queries for resolve_goap_plans.
/// Disjoint with the cats query because cats have `Without<Structure>` and
/// this query accesses `&mut Structure` — Bevy proves disjointness on Structure.
#[allow(clippy::type_complexity)]
#[derive(bevy_ecs::system::SystemParam)]
pub struct BuildingResolverParams<'w, 's> {
    pub buildings: Query<
        'w,
        's,
        (
            Entity,
            &'static mut Structure,
            Option<&'static mut ConstructionSite>,
            Option<&'static mut CropState>,
            &'static Position,
        ),
        (
            Without<crate::components::task_chain::TaskChain>,
            // 367: keep `&mut Structure` here statically disjoint from
            // the read-only `&Structure` in `drying_racks` /
            // `smoking_racks` below. Preservation racks are the only
            // archetypes carrying these state Components, so the
            // negative filter is a clean partition.
            Without<crate::components::building::DryingRackState>,
            Without<crate::components::building::SmokingRackState>,
        ),
    >,
    pub colony_score: Option<ResMut<'w, crate::resources::colony_score::ColonyScore>>,
    /// Ground build-material entities with positions and mutable Item
    /// access. Used both to author the `MaterialPile` planner zone
    /// (read-only iter) and to flip `Item::location` to `Carried(cat)`
    /// in `resolve_pickup_material` (mutable iter). Disjoint from cats
    /// via `Without<GoapPlan>`, from buildings via `Without<Structure>`,
    /// and from non-material items via `With<BuildMaterialItem>` (so
    /// the `&mut Item` access doesn't conflict with `items_query`'s
    /// read-only access).
    pub material_items: Query<
        'w,
        's,
        (
            Entity,
            &'static mut crate::components::items::Item,
            &'static Position,
        ),
        (
            Without<GoapPlan>,
            Without<Structure>,
            With<crate::components::items::BuildMaterialItem>,
        ),
    >,
    /// Ticket 193: ground food/non-material item entities with
    /// positions, used to author the `CarcassPile` planner zone in
    /// `resolve_goap_plans`'s prologue and to fill the dispatch arm
    /// `target_entity` for `PickUpItemFromGround`. Read-only mirror of
    /// `material_items` with the inverted `BuildMaterialItem` filter,
    /// which makes the two statically disjoint. Coexists with the
    /// top-level `items_query` (also read-only `&Item`,
    /// `Without<BuildMaterialItem>`) — both are read-only on
    /// overlapping archetypes, which is permitted by Bevy's borrow
    /// checker.
    pub food_items: Query<
        'w,
        's,
        (
            Entity,
            &'static crate::components::items::Item,
            &'static Position,
        ),
        (
            Without<GoapPlan>,
            Without<Structure>,
            Without<crate::components::items::BuildMaterialItem>,
        ),
    >,
    /// Ticket 084 — mutable per-Stores herb-stash aggregate. Disjoint
    /// from the `buildings` query above (which doesn't borrow
    /// `StoredHerbs`) and from the top-level `stores_query`
    /// (`&mut StoredItems`, different component). Used by the
    /// `DepositHerbs` / `RetrieveHerbs(_)` dispatch arms in
    /// `dispatch_step_action`.
    pub stored_herbs: Query<'w, 's, &'static mut crate::components::building::StoredHerbs>,
    /// 367 — mutable per-rack state for the preservation pipeline.
    /// `Structure` is borrowed read-only here (disjoint from the
    /// `&mut Structure` in `buildings` because we filter
    /// `With<DryingRackState>` / `With<SmokingRackState>` — only
    /// preservation racks carry those Components, and the `buildings`
    /// query's mutation sites only touch Garden / Construction-site
    /// arms which sit on different archetypes). Used by the three
    /// preservation step resolvers
    /// (`resolve_load_drying_rack` / `resolve_load_smoking_rack` /
    /// `resolve_tend_smoking_rack`).
    pub drying_racks: Query<
        'w,
        's,
        (
            Entity,
            &'static Position,
            &'static Structure,
            &'static mut crate::components::building::DryingRackState,
        ),
    >,
    pub smoking_racks: Query<
        'w,
        's,
        (
            Entity,
            &'static Position,
            &'static Structure,
            &'static mut crate::components::building::SmokingRackState,
        ),
    >,
}

/// Bundles resources for resolve_goap_plans.
/// Bundled marker queries for `evaluate_and_plan`'s snapshot population
/// pass. Wraps the §4 broad-phase target-existence markers (ticket 014)
/// and the §9.2 faction overlay markers (ticket 049). Bundled via
/// SystemParam derive so the parent system stays under Bevy's
/// 16-param limit.
#[allow(clippy::type_complexity)]
#[derive(bevy_ecs::system::SystemParam)]
pub struct TargetMarkerQueries<'w, 's> {
    pub target_existence_q: Query<
        'w,
        's,
        (
            Has<markers::HasThreatNearby>,
            Has<markers::HasSocialTarget>,
            Has<markers::HasHerbsNearby>,
            Has<markers::PreyNearby>,
            Has<markers::CarcassNearby>,
            Has<markers::HasUnburiedCorpse>,
            // Ticket 170 — `HideEligible` snapshot wire. Authored each
            // tick by `sensing::update_hide_eligible_markers`; mirrored
            // here so `score_actions` resolves the Hide DSE's
            // eligibility filter against the same source of truth.
            Has<markers::HideEligible>,
        ),
    >,
    pub faction_overlay_q: Query<
        'w,
        's,
        (
            Has<markers::Visitor>,
            Has<markers::HostileVisitor>,
            Has<markers::Banished>,
            Has<markers::BefriendedAlly>,
        ),
    >,
    /// Ticket 158 — kinship-channel substrate for the Caretake DSE
    /// scoring gate. Authored each tick by
    /// `growth::update_kitten_cry_map` (ticket 161 merged the author
    /// in there to avoid a new schedule conflict edge); read at the
    /// `resolve_caretake_target` call site to enable the
    /// own-kitten-anywhere fallback when the per-tick range gate
    /// would otherwise filter every candidate out.
    pub parent_hungry_kitten_q: Query<'w, 's, Has<markers::IsParentOfHungryKitten>>,
    /// Ticket 397 (Layer 1) — broader parent-state substrate plumbed
    /// into `MarkerSnapshot` so `score_actions` can gate Caretake's
    /// pool entry on "this cat structurally has a dependent kitten,"
    /// not just "an acutely hungry kitten is in range." `Parent` stays
    /// true through full natural maturity (set by
    /// `growth::update_parent_markers`); `HasJuvenileDependent` is
    /// the rear_kitten arc emit window subset. The mirror of `Parent`
    /// here also corrects the pre-existing silent stale read at
    /// `escape_viability(...)` (this file ~line 1955) which queried
    /// `markers.has(Parent::KEY, entity)` against a snapshot that
    /// never populated the key.
    pub parent_markers_q: Query<'w, 's, (Has<markers::Parent>, Has<markers::HasJuvenileDependent>)>,
    /// Ticket 321 — per-cat L1→L2 picker output. When the cat carries
    /// a non-empty `AspirationEmissions`, the L2 author site replaces
    /// the default `Intention::Activity { Idle }` wrap with
    /// `Intention::Goal { state: { label, achieved: |_, _| false },
    /// strategy }` from the highest-`Priority` emission row. 320's
    /// HTN frame-push gate downstream catches the Goal shape. The
    /// picker removes the Component entirely when no emission
    /// applies, so `q.get(entity)` returns `Err(_)` and the wrap
    /// defaults to the 126 Activity-Idle shape. Bundled here rather
    /// than as a standalone `evaluate_and_plan` parameter to keep
    /// the parent system under Bevy's 16-param limit.
    pub aspiration_emissions_q:
        Query<'w, 's, &'static crate::components::aspiration_emission::AspirationEmissions>,
    /// Ticket 367 — preservation per-cat markers. Bundled inside
    /// `TargetMarkerQueries` so adding six new `Has<>` rows doesn't
    /// push `evaluate_and_plan` past Bevy's 16-param limit, and so the
    /// `per_cat_markers_q` tuple stays under the 15-arity `QueryData`
    /// ceiling. `CanDry` / `CanSmoke` are capability markers (peers of
    /// `CanCook`); the four `Has*InInventory` rows mirror the inventory
    /// substrate the preservation DSEs read.
    pub preservation_markers_q: Query<
        'w,
        's,
        (
            Has<markers::CanDry>,
            Has<markers::CanSmoke>,
            Has<markers::HasRawFishInInventory>,
            Has<markers::HasRawOrganInInventory>,
            Has<markers::HasRawMeatInInventory>,
            Has<markers::HasFuelInInventory>,
            Has<markers::HasDryableInInventory>,
            Has<markers::HasSmokeableInInventory>,
            // 468: recipe-aware craft eligibility markers — fire iff
            // the cat's pouch satisfies the full input set of at least
            // one recipe at the matching station. Authored by
            // `items::update_inventory_markers`. Replaces the 457
            // `HasCraftInputInInventory` (recipe-agnostic any-input
            // gate) which over-fired the DSE.
            Has<markers::CanSatisfyAnyWorkshopRecipeFromPouch>,
            Has<markers::CanSatisfyAnyTanningFrameRecipeFromPouch>,
            // 457: Workshop-craft per-cat capability (`Adult ∧ ¬Injured`).
            // Bundled here alongside `CanDry` / `CanSmoke` since the
            // authoring system (`capabilities::update_capability_markers`)
            // is the same — keeps the parent `per_cat_markers_q` tuple
            // under the 15-arity ceiling.
            Has<markers::CanCraft>,
        ),
    >,
}

#[allow(clippy::type_complexity)]
#[derive(bevy_ecs::system::SystemParam)]
pub struct ExecutorContext<'w, 's> {
    pub map: ResMut<'w, TileMap>,
    pub wind: Res<'w, crate::resources::wind::WindState>,
    /// 293: read-only per-cat `LocationBeliefs` lookup for the search-
    /// step's `best_prey_direction` reader. Disjoint from the cats
    /// query because `&LocationBeliefs` is read-only here and the
    /// `belief_integrator` writer runs in a separate Chain block
    /// earlier in the tick.
    pub location_beliefs: Query<'w, 's, &'static crate::components::beliefs::LocationBeliefs>,
    pub time: Res<'w, TimeState>,
    pub time_scale: Res<'w, crate::resources::time::TimeScale>,
    pub constants: Res<'w, SimConstants>,
    pub event_log: Option<ResMut<'w, EventLog>>,
    /// 457: recipe catalog for the Workshop-craft dispatch
    /// (`GoapActionKind::CraftAtWorkshop`). Populated once at startup
    /// by `populate_recipe_registry` and consumed read-only here.
    pub recipes: Res<'w, crate::resources::recipe_registry::RecipeRegistry>,
    /// Ticket 364 — read-only `MethodRegistry` for the HTN advance hook's
    /// gate (matches the held frame's leaf primitive action against the
    /// completed plan's chosen action, so only HTN-leaf plan completions
    /// advance the frame).
    pub method_registry: Res<'w, crate::ai::methods::MethodRegistry>,
    /// Wildlife entities with positions, for `EngageThreat` target resolution.
    /// Excludes prey animals so cats don't try to "fight" rabbits as threats.
    pub wildlife: bevy_ecs::prelude::Query<
        'w,
        's,
        (Entity, &'static Position),
        (With<WildAnimal>, Without<Dead>, Without<PreyAnimal>),
    >,
    /// §6.5.9 fight-target DSE snapshot: read-only (Entity, Position,
    /// WildAnimal) tuple for threat-level + combat-advantage axes.
    /// Kept separate from `wildlife` above because that query is the
    /// legacy shape consumed by unrelated callers; extending it would
    /// ripple.
    pub wildlife_with_stats: bevy_ecs::prelude::Query<
        'w,
        's,
        (
            Entity,
            &'static Position,
            &'static crate::components::wildlife::WildAnimal,
        ),
        (Without<Dead>, Without<PreyAnimal>),
    >,
    /// §6.3 target-taking DSE lookup — cat-on-cat step resolvers
    /// (`SocializeWith`, `GroomOther`, `MentorCat`, `MateWith`) route
    /// target resolution through the registered DSEs, which retires
    /// the pre-4c `find_social_target` fondness-only helper.
    pub dse_registry: Res<'w, crate::ai::eval::DseRegistry>,
    /// §6.5.4 kinship lookup — `(kitten_entity) → (mother, father)`
    /// pointer table. Read-only, intentionally slim: drops `Position`
    /// (read from the cats query / `cat_positions` snapshot instead).
    /// Ticket 451 — disjointness from the mutable `cats` query is now
    /// by component access (cats holds `&mut Position` / `&mut Needs` /
    /// `&mut Inventory`; this query only touches `&KittenDependency` +
    /// `Has<RearKittenReleased>`), not by the §Phase-5b `Without<GoapPlan>`
    /// archetype filter — kittens now carry `GoapPlan` post-451 (they
    /// participate in L2 scoring + dispatch) so the legacy filter would
    /// empty the query.
    pub kitten_parentage: bevy_ecs::prelude::Query<
        'w,
        's,
        (
            Entity,
            &'static crate::components::KittenDependency,
            // 395: rear_kitten arc's one-shot Release marker.
            Has<crate::components::markers::RearKittenReleased>,
        ),
        (
            Without<Dead>,
            Without<Structure>,
            With<crate::components::KittenDependency>,
        ),
    >,
    /// §11 focal-cat target. Present only when `--focal-cat` wired the
    /// resource in the headless runner; absent in every interactive
    /// build. Used to gate §7.2 commitment + plan-failure trace
    /// capture at the de-facto branches inside this system.
    pub focal_target: Option<Res<'w, crate::resources::FocalTraceTarget>>,
    /// §11 rich-trace capture sink. Same gating as `focal_target`.
    pub focal_capture: Option<Res<'w, crate::resources::FocalScoreCapture>>,
    /// §9.1 base stance matrix; consumed by every target-taking DSE
    /// call site that pre-filters candidates by stance.
    pub faction_relations: Res<'w, crate::ai::faction::FactionRelations>,
    /// §9.2 overlay marker presence per entity. Read per-candidate to
    /// build a [`StanceOverlays`](crate::ai::faction::StanceOverlays)
    /// that feeds `resolve_stance` inside the §9.3 prefilter.
    pub faction_overlay_q: bevy_ecs::prelude::Query<
        'w,
        's,
        (
            Entity,
            Has<crate::components::markers::Visitor>,
            Has<crate::components::markers::HostileVisitor>,
            Has<crate::components::markers::Banished>,
            Has<crate::components::markers::BefriendedAlly>,
        ),
        Without<Dead>,
    >,
    /// Ticket 158 — kinship-channel substrate for Caretake plan
    /// rebinding. Same query as `TargetMarkerQueries::parent_hungry_kitten_q`
    /// in `evaluate_and_plan`; read at the goap.rs:3843 call site so
    /// the FeedKitten step's target-rebinding inherits the
    /// own-kitten-anywhere fallback when the per-tick range gate
    /// excludes every candidate.
    pub parent_hungry_kitten_q:
        bevy_ecs::prelude::Query<'w, 's, Has<crate::components::markers::IsParentOfHungryKitten>>,
    /// Ticket 126 — read-only `HeldIntention` lookup for the preempt
    /// trigger (3) check in `resolve_goap_plans`'s per-cat prologue.
    /// Disjoint from the mutable `cats` query because `&HeldIntention`
    /// is read-only.
    pub held_intentions:
        bevy_ecs::prelude::Query<'w, 's, &'static crate::components::HeldIntention>,
    /// Ticket 364 — read-only `HeldGoalStack` lookup at the
    /// advance / backtrack hook in `resolve_goap_plans`'s
    /// `plans_to_remove` drain. Consults the stack on Fulfilled
    /// (advance) and Abandoned (backtrack) plan endings; rewrites the
    /// stack via Commands when the method has remaining sub-goals.
    /// Read-only and disjoint from the mutable cats query.
    pub held_goal_stacks:
        bevy_ecs::prelude::Query<'w, 's, &'static crate::components::HeldGoalStack>,
    /// Ticket 127 — L2 JointIntention lookup (successor to
    /// `PairingActivity`). The `SocializeWith` step resolver reads
    /// this to pin the Intention partner at the top of the
    /// `target_partner_bond` axis. Disjoint from the mutable `cats`
    /// query in `resolve_goap_plans` because `&JointIntention` is
    /// read-only.
    pub joint_q:
        bevy_ecs::prelude::Query<'w, 's, &'static crate::components::JointIntention, Without<Dead>>,
    /// 035 — Dead-and-not-Buried colony cat snapshot (Entity, Position,
    /// Name, cause). Disjoint from the `cats` mut query (which filters
    /// `Without<Dead>`). Feeds `dead_cat_positions` /
    /// `dead_cat_names` in `ScoringSnapshots` so the `Bury` dispatch
    /// arm can pick a target and the post-loop drain can resolve a
    /// real name + cause for the `BurialFired` event and spawned
    /// `Grave` entity.
    pub dead_cats_q: bevy_ecs::prelude::Query<
        'w,
        's,
        (Entity, &'static Position, &'static Name, &'static Dead),
        (
            With<crate::components::identity::Species>,
            Without<markers::Buried>,
        ),
    >,
    /// Ticket 074 — read-only target-validity surface (Dead /
    /// Banished / Incapacitated / despawned). Bundled here so step
    /// resolvers reach `validate_target` through the same context they
    /// already hold; nothing else changes about the ExecutorContext
    /// borrow shape (the query is read-only, disjoint from the
    /// mutable `cats` query because cats are filtered `Without<Dead>`
    /// and we read `Has<Dead>` rather than `&Dead`).
    pub target_validity: crate::systems::plan_substrate::target::TargetValidityQuery<'w, 's>,
    /// 263 — `ActionAffordances` resource borrow for the
    /// SearchPrey path's per-target affordance read
    /// (`hunt_best_predation_affordance` axis) and the EngagePrey
    /// resolver's phase-band bias (C5). Threaded here so step
    /// resolvers reach the substrate through the existing context
    /// instead of needing a new SystemParam slot.
    pub action_affordances: Res<'w, crate::resources::action_affordances::ActionAffordances>,
    /// Ticket 427 Step 1 — pre-allocated scratch for target-taking DSE
    /// resolvers invoked from `resolve_goap_plans`. Same `DseTargetScratchpad`
    /// resource the planner system reaches through `PlanResources`; Bevy
    /// serializes the two systems on the `ResMut` overlap (they're
    /// already sequenced by the broader plan pipeline so the lost
    /// parallelism is a no-op).
    pub dse_scratchpad: ResMut<'w, crate::resources::DseTargetScratchpad>,
    /// Ticket 427 Step 3 — cat A* planner scratch for the replan
    /// fallback paths inside `resolve_goap_plans`. Separate `Local<>`
    /// from `PlanResources.planner_scratch` because the two systems
    /// own independent scratch lifetimes; both preserve capacity
    /// across cat-ticks within their own loop.
    pub planner_scratch: bevy_ecs::prelude::Local<'s, crate::ai::planner::CatPlannerScratch>,
}

impl<'w, 's> ExecutorContext<'w, 's> {
    /// Read §9.2 overlay markers off `e` from the ECS, returning a
    /// [`StanceOverlays`](crate::ai::faction::StanceOverlays) the §9.3
    /// prefilter can consume. Defaults to an all-`false` overlay when
    /// the entity is not in the query (despawned, dead, etc.).
    pub fn stance_overlays_of(&self, e: Entity) -> crate::ai::faction::StanceOverlays {
        stance_overlays_from_query(&self.faction_overlay_q, e)
    }
}

/// Ticket 427 Step 1 — free-function form of
/// [`ExecutorContext::stance_overlays_of`], capturing only the query
/// rather than the whole context. Lets callers build stance-overlay
/// closures whose only capture is `&ec.faction_overlay_q`, leaving
/// `&mut ec.dse_scratchpad` free as a disjoint field borrow.
#[allow(clippy::type_complexity)]
pub fn stance_overlays_from_query(
    query: &bevy_ecs::prelude::Query<
        '_,
        '_,
        (
            Entity,
            bevy_ecs::prelude::Has<crate::components::markers::Visitor>,
            bevy_ecs::prelude::Has<crate::components::markers::HostileVisitor>,
            bevy_ecs::prelude::Has<crate::components::markers::Banished>,
            bevy_ecs::prelude::Has<crate::components::markers::BefriendedAlly>,
        ),
        bevy_ecs::prelude::Without<Dead>,
    >,
    e: Entity,
) -> crate::ai::faction::StanceOverlays {
    match query.get(e) {
        Ok((_, visitor, hostile_visitor, banished, befriended_ally)) => {
            crate::ai::faction::StanceOverlays {
                visitor,
                hostile_visitor,
                banished,
                befriended_ally,
            }
        }
        Err(_) => crate::ai::faction::StanceOverlays::default(),
    }
}

/// Returns true when `cat_entity` matches the registered focal cat.
/// Zero-cost when `focal_target` isn't inserted (non-headless runs /
/// headless without `--focal-cat`): the inner `Option` is None and
/// the short-circuit returns false before any entity comparison.
fn ec_is_focal(ec: &ExecutorContext, cat_entity: Entity) -> bool {
    ec.focal_target
        .as_ref()
        .and_then(|t| t.entity)
        .map(|e| e == cat_entity)
        .unwrap_or(false)
}

// ===========================================================================
// check_modifier_preemption — ticket 118
// ===========================================================================

/// Substrate-driven plan preemption for acute-class lurch modifiers.
/// Closes the "modifier raises score but plan-completion momentum gates
/// behavior" gap surfaced in ticket 047 Phase 2 verification: Sleep
/// won the L2 softmax in 99.3% of injured-window ticks but was the
/// chosen action only 1.4% of them, because the cat was mid-plan in
/// Hunt/Forage/Patrol/Fight and those plans completed naturally before
/// the next softmax fired.
///
/// **Mechanism.** For each cat with an in-flight `GoapPlan` (and not
/// already in a recovery disposition), iterate the modifier pipeline
/// and ask each modifier `preempts_in_flight(ctx, fetch)`. The default
/// is `false`; lurch modifiers (047 / 102 / 105 / 108) override to
/// query their trigger scalar against the lurch threshold. On the
/// first `true`: drop the plan via `commands.remove::<GoapPlan>`,
/// reset `current.ticks_remaining = 0` so `evaluate_and_plan` re-elects
/// next tick, emit the abandoned narrative, record
/// `Feature::ModifierPreemption`, and push the focal-trace
/// `L3PlanFailure { reason: "modifier_preemption" }` row.
///
/// **Why a separate system, not just a modifier-pipeline tap inside
/// `evaluate_and_plan`.** `evaluate_and_plan` only runs for cats
/// without `GoapPlan` (or with `ticks_remaining == 0`); cats mid-plan
/// don't re-score. To preempt mid-plan, the substrate's gating must
/// run on the in-flight set per tick — exactly the schedule slot
/// `check_anxiety_interrupts` occupies. This system mirrors that
/// shape and is registered alongside it.
///
/// **Resting / Eating exemption.** Cats in those dispositions are
/// already recovering; preempting them creates the oscillation we're
/// fixing. Mirrors the legacy `check_anxiety_interrupts` exemption
/// (goap.rs:592 in the pre-119 codebase).
///
/// **Cost shape.** Iterates registered modifiers (~25 today) for each
/// cat with `GoapPlan` (~10–20 typical). Each `preempts_in_flight`
/// call is one or two scalar reads + a smoothstep ramp evaluation.
/// Pressure-class modifiers return on the default `false` immediately
/// (no scalar reads). Total per-tick cost is a few hundred float ops
/// — well below the existing `check_anxiety_interrupts` per-cat work.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn check_modifier_preemption(
    mut query: Query<
        (
            Entity,
            &Name,
            &Needs,
            &Health,
            &mut CurrentAction,
            Option<&mut ActionHistory>,
            Option<&crate::components::PrevSafetyDeficit>,
        ),
        (With<GoapPlan>, Without<Dead>),
    >,
    plans: Query<&GoapPlan, Without<Dead>>,
    modifier_pipeline: Res<crate::ai::eval::ModifierPipeline>,
    time: Res<TimeState>,
    mut commands: Commands,
    mut activation: ResMut<SystemActivation>,
    mut plan_writer: MessageWriter<PlanNarrative>,
    mut event_log: Option<ResMut<EventLog>>,
    focal_target: Option<Res<crate::resources::FocalTraceTarget>>,
    focal_capture: Option<Res<crate::resources::FocalScoreCapture>>,
) {
    // Static closures for the minimal `EvalCtx` the lurch modifiers'
    // `preempts_in_flight` predicates need (they only read scalars,
    // not markers / anchors / target). 'static lifetime so we don't
    // borrow per-tick state.
    static MARKER: fn(&str, Entity) -> bool = |_, _| false;
    static NO_ENTITY_POS: fn(Entity) -> Option<Position> = |_| None;
    static NO_ANCHOR_POS: fn(crate::ai::considerations::LandmarkAnchor) -> Option<Position> =
        |_| None;

    for (entity, name, needs, health, mut current, history, prev_safety_deficit) in &mut query {
        let Ok(plan) = plans.get(entity) else {
            continue;
        };

        // Recovery dispositions are already self-care — preempting
        // them just oscillates. Mirrors the legacy CriticalHealth
        // interrupt's exemption.
        //
        // 230: `Fleeing` joins the exemption list. Once committed to
        // a Fleeing plan, the cat is already responding to the
        // adrenaline lurch — the modifier guard composes with the
        // disposition's `SingleMinded` commitment proxy so the cat
        // accumulates hold ticks instead of re-firing
        // `PickFleeTarget` every tick. This was the entire shape of
        // the post-228 thrash spiral: 39,536× preempts in 100k ticks
        // because the (since-retired-by-251)
        // `AcuteHealthAdrenalineFlee::preempts_in_flight` returned
        // `true` whenever `flee_lift > 0`, regardless of the
        // in-flight plan.
        if matches!(
            plan.kind,
            DispositionKind::Resting | DispositionKind::Eating | DispositionKind::Fleeing
        ) {
            continue;
        }

        // Minimal scalar fetch — only the trigger scalars the three
        // remaining acute-class lurch modifiers (102 / 105 / 108)
        // consult in their `preempts_in_flight` predicates (post-251 —
        // 047's `AcuteHealthAdrenalineFlee` was retired). Aligned with the
        // canonical `scoring::ctx_scalars` keys (single source of
        // truth). Ticket 108 — the `threat_proximity_derivative`
        // computation here mirrors the ScoringContext builder's
        // (`(1 - safety) - prev`, prev = current for lazy-insert).
        let health_deficit = (1.0 - health.current / health.max).clamp(0.0, 1.0);
        let safety_deficit_now = (1.0 - needs.safety).clamp(0.0, 1.0);
        let safety_deficit_prev = prev_safety_deficit
            .map(|p| p.0)
            .unwrap_or(safety_deficit_now);
        let threat_proximity_derivative = crate::components::PrevSafetyDeficit::rising_derivative(
            safety_deficit_now,
            safety_deficit_prev,
        );
        let fetch_scalar = move |scalar: &str, _: Entity| -> f32 {
            match scalar {
                "health_deficit" => health_deficit,
                "threat_proximity_derivative" => threat_proximity_derivative,
                _ => 0.0,
            }
        };

        let eval_ctx = crate::ai::dse::EvalCtx {
            cat: entity,
            tick: time.tick,
            entity_position: &NO_ENTITY_POS,
            anchor_position: &NO_ANCHOR_POS,
            has_marker: &MARKER,
            self_position: Position::new(0, 0),
            target: None,
            target_position: None,
            target_alive: None,
            field_cost: None,
        };

        // Find the first acute modifier asking for behavioral
        // expression. The pipeline's iteration order is the
        // registration order set in `default_modifier_pipeline` —
        // 047 (Flee) → 102 (Fight) → 105 (Freeze) → 108 (ThreatProx).
        // Order doesn't load-bear here: the predicate is symmetric
        // under permutation (any `true` triggers preemption), so the
        // first `true` is sufficient.
        let triggered = modifier_pipeline
            .iter_passes()
            .find(|m| m.preempts_in_flight(&eval_ctx, &fetch_scalar));

        let Some(modifier) = triggered else { continue };

        activation.record(Feature::ModifierPreemption);

        // §11 focal-cat trace capture. Distinct from the §7.2
        // commitment branches — this preempts the gate entirely
        // (matching the legacy anxiety-interrupt path), so the row
        // surfaces as `L3PlanFailure` with `reason: "modifier_preemption"`.
        let is_focal = focal_target
            .as_ref()
            .and_then(|t| t.entity)
            .map(|e| e == entity)
            .unwrap_or(false);
        if is_focal {
            if let Some(capture) = focal_capture.as_deref() {
                let current_step = plan
                    .current()
                    .map(|s| format!("{:?}", s.action))
                    .unwrap_or_else(|| "none".into());
                capture.push_plan_failure(
                    crate::resources::trace_log::PlanFailureCapture {
                        reason: "modifier_preemption",
                        disposition: format!("{:?}", plan.kind),
                        detail: serde_json::json!({
                            "modifier": modifier.name(),
                            "in_flight_step": current_step,
                            "health_deficit": health_deficit,
                        }),
                    },
                    time.tick,
                );
                // Flip the L3 momentum row's `preempted` bool so the
                // compact L3 record reflects the preemption without
                // requiring trace consumers to scan plan_failures.
                capture.set_momentum_preempted(time.tick);
            }
        }

        if let Some(ref mut log) = event_log {
            let current_step = plan
                .current()
                .map(|s| format!("{:?}", s.action))
                .unwrap_or_else(|| "none".into());
            log.push(
                time.tick,
                EventKind::PlanInterrupted {
                    cat: name.0.clone(),
                    disposition: format!("{:?}", plan.kind),
                    reason: format!("modifier_preemption({})", modifier.name()),
                    current_step,
                    hunger: needs.hunger,
                    energy: needs.energy,
                    temperature: needs.temperature,
                },
            );
        }

        if let Some(mut hist) = history {
            hist.record(ActionRecord {
                action: current.action,
                disposition: Some(plan.kind),
                tick: time.tick,
                outcome: ActionOutcome::Interrupted,
            });
        }

        plan_writer.write(PlanNarrative {
            entity,
            kind: plan.kind,
            event: PlanEvent::Abandoned,
            completions: plan.trips_done,
        });

        // Drop the plan and reset the action gate so
        // `evaluate_and_plan` re-elects next tick. Mirrors the legacy
        // `check_anxiety_interrupts` exit shape exactly — using
        // commands.remove rather than the substrate's `try_preempt`
        // primitive because we don't have `&mut GoapPlan` access in
        // this read-only-plan query.
        commands.entity(entity).remove::<GoapPlan>();
        current.ticks_remaining = 0;
    }
}

// ===========================================================================
// check_anxiety_interrupts — soft-urgency accumulation for step-boundary
// preemption (ThreatNearby, CriticalSafety, hunger/exhaustion/thermal).
// Ticket 119 retired the function's namesake CriticalHealth hard-interrupt
// branch; the substrate-driven path (`check_modifier_preemption`, ticket
// 118) now drives behavioral re-election under acute health distress.
// The function name is preserved for schedule-stability; `accumulate_urgencies`
// is its remaining job.
// ===========================================================================

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn check_anxiety_interrupts(
    mut query: Query<
        (
            Entity,
            &Needs,
            &Personality,
            &Position,
            &mut PendingUrgencies,
        ),
        (With<GoapPlan>, Without<Dead>),
    >,
    plans: Query<&GoapPlan, Without<Dead>>,
    wildlife: Query<(Entity, &Position), (With<WildAnimal>, Without<Dead>, Without<PreyAnimal>)>,
    ward_query: Query<(&Ward, &Position)>,
    all_cats: Query<(Entity, &Position), (Without<Dead>, Without<WildAnimal>)>,
    building_query: Query<&Position, (With<Structure>, Without<ConstructionSite>)>,
    constants: Res<SimConstants>,
    colony_center: Res<crate::resources::ColonyCenter>,
) {
    let d = &constants.disposition;

    // Pre-collect data to avoid query conflicts in the loop.
    let wildlife_positions: Vec<(Position, Entity)> =
        wildlife.iter().map(|(e, p)| (*p, e)).collect();
    let ward_data: Vec<(Position, f32)> = ward_query
        .iter()
        .filter(|(w, _)| !w.inverted && w.strength > 0.01)
        .map(|(w, p)| (*p, w.repel_radius()))
        .collect();
    let cat_positions: Vec<(Entity, Position)> = all_cats.iter().map(|(e, p)| (e, *p)).collect();
    let building_positions: Vec<Position> = building_query.iter().copied().collect();

    for (entity, needs, personality, pos, mut urgencies) in &mut query {
        let Ok(plan) = plans.get(entity) else {
            continue;
        };

        // --- Accumulate soft urgencies for step-boundary evaluation ---
        accumulate_urgencies(
            needs,
            personality,
            pos,
            plan.kind,
            &wildlife_positions,
            &ward_data,
            &cat_positions,
            &colony_center.0,
            &building_positions,
            d,
            &constants.sensory.cat,
            entity,
            &mut urgencies,
        );
    }
}

// ---------------------------------------------------------------------------
// Urgency accumulation — runs every tick, writes to PendingUrgencies
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn accumulate_urgencies(
    needs: &Needs,
    personality: &Personality,
    pos: &Position,
    kind: DispositionKind,
    wildlife_positions: &[(Position, Entity)],
    ward_data: &[(Position, f32)],
    cat_positions: &[(Entity, Position)],
    colony_center: &Position,
    building_positions: &[Position],
    d: &DispositionConstants,
    cat_profile: &crate::systems::sensing::SensoryProfile,
    entity: Entity,
    urgencies: &mut PendingUrgencies,
) {
    urgencies.needs.clear();

    // --- Starvation (maslow 1) ---
    // 150 R5a: Eating is now the canonical "I'm fixing my hunger"
    // disposition; firing a Starvation urgency mid-Eating would just
    // re-elect the same disposition.
    //
    // 511 — Resting REMOVED from this exclusion. The old rationale
    // ("a legacy Resting plan addresses hunger via the three-need
    // recipe") is stale: Resting plans do not feed, and excluding
    // them here — combined with the strictly-less-than Maslow-tier
    // preemption below — let a Rest-looping cat ride hunger from
    // sated to starvation with no interrupt (Duskkit-45,
    // tickets 511; the kitten spent its critical window walking
    // Resting travel legs, so step-level hunger-wakes in
    // sleep/self_groom could not fire either).
    if !matches!(
        kind,
        DispositionKind::Eating | DispositionKind::Hunting | DispositionKind::Foraging
    ) && needs.hunger < d.starvation_interrupt_threshold
    {
        urgencies.needs.push(UrgentNeed {
            kind: UrgencyKind::Starvation,
            maslow_tier: 1,
            intensity: 1.0 - (needs.hunger / d.starvation_interrupt_threshold).max(0.001),
            threat_pos: None,
        });
    }
    // Critical starvation override for Hunting/Foraging — when a
    // production-coded cat dips below the critical-hunger threshold
    // even mid-disposition, fire the urgency so the cat re-elects to
    // Eating (or Resting if stores are empty). 150 R1's eat-the-catch
    // path normally rescues these cats before they reach this depth,
    // but if the catch fails (low success_chance, prey teleports, etc.)
    // the urgency is the safety net.
    if matches!(kind, DispositionKind::Hunting | DispositionKind::Foraging)
        && needs.hunger < d.critical_hunger_interrupt_threshold
    {
        urgencies.needs.push(UrgentNeed {
            kind: UrgencyKind::Starvation,
            maslow_tier: 1,
            intensity: 1.0 - (needs.hunger / d.critical_hunger_interrupt_threshold).max(0.001),
            threat_pos: None,
        });
    }

    // --- Exhaustion (maslow 1) ---
    if !matches!(
        kind,
        DispositionKind::Resting | DispositionKind::Hunting | DispositionKind::Foraging
    ) && needs.energy < d.exhaustion_interrupt_threshold
    {
        urgencies.needs.push(UrgentNeed {
            kind: UrgencyKind::Exhaustion,
            maslow_tier: 1,
            intensity: 1.0 - (needs.energy / d.exhaustion_interrupt_threshold).max(0.001),
            threat_pos: None,
        });
    }

    // --- CriticalSafety (maslow 2) ---
    if needs.safety < d.critical_safety_threshold {
        urgencies.needs.push(UrgentNeed {
            kind: UrgencyKind::CriticalSafety,
            maslow_tier: 2,
            intensity: 1.0 - (needs.safety / d.critical_safety_threshold).max(0.001),
            threat_pos: None,
        });
    }

    // --- ThreatNearby (maslow 2, contextual) ---
    if !matches!(kind, DispositionKind::Guarding) {
        if let Some(threat) = evaluate_threat_context(
            pos,
            personality,
            wildlife_positions,
            ward_data,
            cat_positions,
            colony_center,
            building_positions,
            d,
            cat_profile,
            entity,
        ) {
            urgencies.needs.push(threat);
        }
    }
}

// ---------------------------------------------------------------------------
// Contextual threat evaluation — the "zoo vs bush" formula
// ---------------------------------------------------------------------------

/// Evaluates whether a nearby threat warrants an urgency, considering the cat's
/// full environmental context. A cat at the stores with wards and allies barely
/// reacts. A cat alone in the wilderness drops everything.
#[allow(clippy::too_many_arguments)]
fn evaluate_threat_context(
    pos: &Position,
    personality: &Personality,
    wildlife_positions: &[(Position, Entity)],
    ward_data: &[(Position, f32)],
    cat_positions: &[(Entity, Position)],
    colony_center: &Position,
    building_positions: &[Position],
    d: &DispositionConstants,
    cat_profile: &crate::systems::sensing::SensoryProfile,
    entity: Entity,
) -> Option<UrgentNeed> {
    // Phase 2 migration: the visual-only detection path now flows
    // through the sensory model's sight channel. See `cat_sees_threat_at`.
    let nearest = wildlife_positions
        .iter()
        .filter(|(wp, _)| crate::systems::sensing::cat_sees_threat_at(*pos, cat_profile, *wp))
        .min_by_key(|(wp, _)| pos.tile_distance_squared(wp));

    let (threat_pos, _) = nearest?;
    let dist = pos.distance_to(threat_pos);

    // Base urgency: inverse distance.
    let base_urgency = (1.0 - dist / d.threat_urgency_divisor).max(0.0);
    if base_urgency <= 0.0 {
        return None;
    }

    // Ward protection: inside a ward's repel radius dampens threat.
    let within_ward = ward_data
        .iter()
        .any(|(wp, radius)| (pos.chebyshev_distance(wp) as f32) < *radius);
    let ward_factor = if within_ward {
        d.threat_ward_dampening
    } else {
        1.0
    };

    // Colony proximity: near buildings or colony center dampens threat.
    // Radial perception of safety — "can I see/be near the colony" —
    // uses `euclidean_distance` per the 494 split.
    let near_buildings = building_positions
        .iter()
        .any(|bp| pos.euclidean_distance(bp) <= d.threat_building_safety_range);
    let colony_factor = if near_buildings {
        d.threat_colony_building_dampening
    } else {
        let colony_dist = pos.distance_to(colony_center);
        let normalized = (colony_dist / d.threat_colony_radius).min(1.0);
        d.threat_colony_center_dampening + (1.0 - d.threat_colony_center_dampening) * normalized
    };

    // Allies: each nearby cat reduces perceived threat (diminishing returns).
    // Radial co-perception of allies — visual/auditory awareness of
    // nearby cats — uses `euclidean_distance` per the 494 split.
    let ally_count = cat_positions
        .iter()
        .filter(|(e, cp)| *e != entity && pos.euclidean_distance(cp) <= d.threat_ally_range)
        .count()
        .min(d.allies_fighting_cap);
    let ally_factor = 1.0 / (1.0 + ally_count as f32 * d.threat_ally_dampening_per_cat);

    // Boldness: bold cats feel less threatened.
    let boldness_factor = 1.0 - personality.boldness * d.flee_threshold_boldness_scale;

    let intensity = base_urgency * ward_factor * colony_factor * ally_factor * boldness_factor;

    if intensity > d.flee_threshold_base {
        Some(UrgentNeed {
            kind: UrgencyKind::ThreatNearby,
            maslow_tier: 2,
            intensity,
            threat_pos: Some(*threat_pos),
        })
    } else {
        None
    }
}

// ===========================================================================
// evaluate_and_plan — scores dispositions, invokes planner, inserts GoapPlan
// ===========================================================================

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn evaluate_and_plan(
    mut query: Query<
        (
            (
                Entity,
                &Name,
                &Needs,
                &Personality,
                &Position,
                &Memory,
                &Skills,
                &Health,
                // Ticket 095 Phase 1 Stage B — anatomical pain substrate.
                &crate::components::CatBodyModel,
            ),
            (
                &MagicAffinity,
                &Inventory,
                &mut CurrentAction,
                Option<&crate::components::aspirations::Aspirations>,
                Option<&crate::components::aspirations::Preferences>,
                Option<&crate::components::fate::FatedLove>,
                Option<&crate::components::fate::FatedRival>,
                Option<&crate::components::fulfillment::Fulfillment>,
                // Ticket 290 — `ContextBeliefs` from the C3 subjective
                // belief substrate (258). The `DispositionExecution(kind)`
                // entry's `predictability` facet is the read source for
                // `disposition_cooldown_signal` (Ticket 123 IAUS-side
                // mirror of the planner's `make_plan → None` veto).
                // `None` means ContextBeliefs hasn't spawned yet (test
                // paths) — the sensor fail-opens to 1.0.
                Option<&crate::components::beliefs::ContextBeliefs>,
                // Ticket 108 — last tick's `safety_deficit` snapshot
                // for the `ThreatProximityAdrenaline` rising-only
                // derivative. Optional because save-loaded cats
                // (pre-108 saves) get the lazy-insert path in
                // `update_prev_safety_deficit`.
                Option<&crate::components::PrevSafetyDeficit>,
                // Ticket 109 (Phase A) — focal cat's birth tick for
                // the `social_status_distress` age_diff arm. Optional
                // because some test paths spawn cats without `Age`;
                // `None` falls through to the 0.0 age_diff branch.
                Option<&crate::components::identity::Age>,
            ),
        ),
        (
            Without<Dead>,
            Without<GoapPlan>,
            // Ticket 451 — the §Phase 5b `Without<KittenDependency>`
            // filter retired. Kittens now enter L2 scoring; the
            // per-DSE life-stage gate (`CatDse::life_stages`) restricts
            // their pool to stage-appropriate DSEs (Eat / Sleep / Idle /
            // Wander / Hide / Flee / Socialize / Groom / Explore plus
            // the kitten-specific `BegForFood` siblings). The FeedKitten
            // +0.5 hunger drain that §Phase 5b protected migrates to
            // the unified cats query in `resolve_goap_plans` (kittens
            // appear there post-451 because they have `GoapPlan`).
        ),
    >,
    world_state: WorldStateQueries,
    mut res: PlanResources,
    mating_fitness_params: crate::ai::mating::MatingFitnessParams,
    colony: super::ColonyContext<'_>,
    mut rng: ResMut<SimRng>,
    mut commands: Commands,
    mut plan_writer: MessageWriter<PlanNarrative>,
    mut event_log: Option<ResMut<EventLog>>,
    mut unmet_demand: ResMut<crate::resources::UnmetDemand>,
    life_stage_q: Query<(
        Has<markers::Kitten>,
        Has<markers::Young>,
        Has<markers::Adult>,
        Has<markers::Elder>,
        // 450 kitten sub-stages + mentee-side gate.
        Has<markers::NewbornKitten>,
        Has<markers::EyesOpenKitten>,
        Has<markers::JuvenileKitten>,
        Has<markers::MentorableAge>,
    )>,
    per_cat_markers_q: Query<(
        Has<markers::Injured>,
        Has<markers::HasHerbsInInventory>,
        Has<markers::HasRemedyHerbs>,
        Has<markers::HasWardHerbs>,
        Has<markers::IsCoordinatorWithDirectives>,
        // §4 batch 2: capability markers.
        Has<markers::CanHunt>,
        Has<markers::CanForage>,
        Has<markers::CanWard>,
        Has<markers::CanWardFromSupply>,
        Has<markers::CanCook>,
        // 235: per-class inventory-content markers (siblings to
        // HasHerbsInInventory). Written by `items::update_inventory_markers`.
        Has<markers::HasMaterialsInInventory>,
        Has<markers::HasCuriosInInventory>,
        // 450: generic food-in-inventory marker for the Eat method cascade
        // ([BegForFood] requires `¬HasFoodInInventory`); a 429 follow-on
        // will extend `EatDse`'s eligibility filter to ALSO accept this
        // marker so the planner builds the 1-step `[EatFromOwnInventory]`
        // chain when pocket food is present. 429 itself scopes the
        // items-are-real Sink contract; the GOAP-side wiring is balance
        // work that follows separately.
        Has<markers::HasFoodInInventory>,
    )>,
    // §4.2 State markers — split into a separate query so the per-cat
    // tuple stays small and future State authors can extend here.
    state_markers_q: Query<(
        Has<markers::InCombat>,
        Has<markers::OnCorruptedTile>,
        Has<markers::OnSpecialTerrain>,
    )>,
    // Ticket 027 Bug 2 — HasEligibleMate authored by
    // `mating::update_mate_eligibility_markers`, paired with ticket
    // 103 — `Has<JointIntention>` (any practice; 127 successor to
    // `Has<PairingActivity>`) for the dependent-presence half of
    // `escape_viability`. Bundling both in one query keeps the
    // mate/joint-practice state colocated and stays under the
    // SystemParam count budget.
    mate_eligibility_q: Query<(
        Has<markers::HasEligibleMate>,
        Has<crate::components::JointIntention>,
    )>,
    // Ticket 014 Mentoring batch — Mentor / Apprentice / HasMentoringTarget
    // authored by `aspirations::update_training_markers` and
    // `aspirations::update_mentoring_target_markers`.
    mentoring_q: Query<(
        Has<markers::Mentor>,
        Has<markers::Apprentice>,
        Has<markers::HasMentoringTarget>,
    )>,
    // Bundled marker queries (§4 sensing + §9.2 faction overlays).
    // Bundled via SystemParam derive so `evaluate_and_plan` stays under
    // Bevy's 16-param limit per CLAUDE.md ECS rules.
    marker_qs: TargetMarkerQueries,
) {
    let sc = &res.constants.scoring;
    let d = &res.constants.disposition;
    // Ticket 168 — six colony-scoped markers authored on the
    // `ColonyState` singleton by the colony-marker author chain
    // (buildings.rs::update_colony_building_markers,
    // magic.rs::update_{herb_availability,ward_coverage,ward_siege}_markers).
    //
    // Ticket 433 — read from `WorldSnapshots` instead of re-querying
    // the singleton. The populator runs once per tick after every
    // marker author + ApplyDeferred, so the cached bundle reflects the
    // current tick's authored state. Substrate-equivalent to the prior
    // inline `colony_state_query.single()` read but single source of
    // truth — see `docs/systems/world-snapshots.md`.
    let ws = &*res.world_snapshots;
    let cm = ws.colony_markers;
    let (
        has_functional_kitchen,
        has_raw_food_in_stores,
        food_available,
        thornbriar_available,
        ward_strength_low,
        wards_under_siege,
        has_construction_site,
        has_damaged_building,
        has_garden,
        colony_stores_chronically_full,
        has_midden,
        has_ground_carcass,
        has_dependent_cat,
        has_stored_thornbriar,
        colony_thornbriar_chronically_low,
        has_functional_drying_rack,
        has_functional_smoking_rack,
        has_loaded_smoking_rack_off_cooldown,
        has_dryable_in_stores,
        has_smokeable_in_stores,
        has_functional_workshop,
        has_functional_tanning_frame,
    ) = (
        cm.has_functional_kitchen,
        cm.has_raw_food_in_stores,
        ws.food_available,
        cm.thornbriar_available,
        cm.ward_strength_low,
        cm.wards_under_siege,
        cm.has_construction_site,
        cm.has_damaged_building,
        cm.has_garden,
        cm.colony_stores_chronically_full,
        cm.has_midden,
        cm.has_ground_carcass,
        cm.has_dependent_cat,
        cm.has_stored_thornbriar,
        cm.colony_thornbriar_chronically_low,
        cm.has_functional_drying_rack,
        cm.has_functional_smoking_rack,
        cm.has_loaded_smoking_rack_off_cooldown,
        cm.has_dryable_in_stores,
        cm.has_smokeable_in_stores,
        cm.has_functional_workshop,
        cm.has_functional_tanning_frame,
    );
    let food_fraction = ws.food_fraction;

    // §4 marker snapshot. Populated once at system start from the
    // `ColonyState` singleton (colony-scoped markers, ticket 168) plus
    // per-cat queries below. Passed by reference through `EvalInputs`
    // so `EligibilityFilter::require(marker)` rows resolve without
    // each DSE carrying its own query bundle.
    let mut markers = crate::ai::scoring::MarkerSnapshot::new();
    markers.set_colony(markers::HasStoredFood::KEY, food_available);

    let mut cat_positions: Vec<(Entity, Position)> = Vec::new();
    let mut prey_positions: Vec<Position> = Vec::new();
    for (e, p, prey) in world_state.all_positions.iter() {
        cat_positions.push((e, *p));
        if prey.is_some() {
            prey_positions.push(*p);
        }
    }

    // Ticket 138 — carry per_tick alongside position so the
    // `escape_viability` mobility-differential term has access to the
    // threat's cadence at scoring time. Defaults to 1.0 for save-
    // loaded wildlife missing the MovementBudget component.
    let wildlife_positions: Vec<(Entity, Position, f32)> = world_state
        .wildlife
        .iter()
        .map(|(e, p, b)| (e, *p, b.map(|mb| mb.per_tick).unwrap_or(1.0)))
        .collect();

    // §Phase 4c.3: snapshot kittens for Caretake urgency wiring.
    let kitten_snapshot: Vec<crate::ai::caretake_targeting::KittenState> = world_state
        .kitten_query
        .iter()
        .map(
            |(e, p, needs, dep)| crate::ai::caretake_targeting::KittenState {
                entity: e,
                pos: *p,
                hunger: needs.hunger,
                mother: dep.mother,
                father: dep.father,
            },
        )
        .collect();

    // §4 colony-scoped marker predicates. All nine colony-scoped markers
    // (ticket 168 batch + 169's HasConstructionSite / HasDamagedBuilding +
    // 171's HasGarden) are now bound from the `colony_state_query`
    // readout above. The previous `scan_colony_buildings` call here
    // existed only to source `has_garden` — retired in 171.
    markers.set_colony(markers::HasGarden::KEY, has_garden);
    markers.set_colony(markers::HasFunctionalKitchen::KEY, has_functional_kitchen);
    markers.set_colony(markers::HasRawFoodInStores::KEY, has_raw_food_in_stores);
    markers.set_colony(markers::HasConstructionSite::KEY, has_construction_site);
    markers.set_colony(markers::HasDamagedBuilding::KEY, has_damaged_building);
    // 176: ColonyStoresChronicallyFull — see
    // `update_colony_building_markers` for the chronicity tracking
    // that toggles the underlying ECS-level marker.
    markers.set_colony(
        markers::ColonyStoresChronicallyFull::KEY,
        colony_stores_chronically_full,
    );
    // 178: HasMidden — Trashing DSE eligibility filter. Authored by
    // the same `update_colony_building_markers` system from any
    // existing `StructureType::Midden` building.
    markers.set_colony(markers::HasMidden::KEY, has_midden);
    // 185: HasGroundCarcass — PickingUp DSE eligibility filter for
    // emergent scavenging. Authored by `update_colony_building_markers`
    // from any uncleansed/unharvested carcass in the colony.
    markers.set_colony(markers::HasGroundCarcass::KEY, has_ground_carcass);
    // 188 / 410: HasDependentCat — Handing AND Caretake DSE eligibility
    // filter. Authored by `update_colony_building_markers` from any
    // care-dependent cat (currently any living kitten). Adults give
    // care to dependents.
    markers.set_colony(markers::HasDependentCat::KEY, has_dependent_cat);
    // 084: HasStoredThornbriar — gates the `RetrieveHerbs(Thornbriar)`
    // planner action precondition and (Commit 2) the
    // `CanWardFromSupply` combined eligibility marker. Authored by
    // `update_colony_building_markers` from per-Stores `StoredHerbs`
    // aggregates.
    markers.set_colony(markers::HasStoredThornbriar::KEY, has_stored_thornbriar);

    // 367: preservation-station colony markers. Failure to populate
    // these into the snapshot would silently shut off `DryFoodDse` /
    // `SmokeMeatDse` / `TendSmokingRackDse` eligibility — exactly the
    // §209/084 class of bug that the third-clause discipline (and
    // `check_marker_snapshot_wiring.sh`) exists to catch. Source of
    // truth for each predicate is `update_colony_building_markers`;
    // these lines mirror the cached bundle into the MarkerSnapshot the
    // DSE eligibility filters actually consult.
    markers.set_colony(
        markers::HasFunctionalDryingRack::KEY,
        has_functional_drying_rack,
    );
    markers.set_colony(
        markers::HasFunctionalSmokingRack::KEY,
        has_functional_smoking_rack,
    );
    markers.set_colony(
        markers::HasLoadedSmokingRackOffCooldown::KEY,
        has_loaded_smoking_rack_off_cooldown,
    );
    // 367 follow-on — colony has ≥1 RawFish or RawOrgan in
    // `StoredItems`. Read by the per-cat `HasDryableAccessible`
    // composite below; that composite is what `DryFoodDse`
    // eligibility actually consults. Wired here (not only as a
    // ColonyState component) because the eligibility filter resolves
    // markers via the snapshot.
    markers.set_colony(markers::HasDryableInStores::KEY, has_dryable_in_stores);
    // 443 — colony has raw meat AND fuel in stores; read by the
    // per-cat `HasSmokeableAccessible` composite below.
    markers.set_colony(markers::HasSmokeableInStores::KEY, has_smokeable_in_stores);
    // 457 — Workshop availability. Reader: `CraftAtWorkshopDse`
    // eligibility filter via `MarkerSnapshot.has(...)`. Wired here
    // (not only as a ColonyState component) per the third-clause
    // discipline (§209 / §084).
    markers.set_colony(markers::HasFunctionalWorkshop::KEY, has_functional_workshop);
    // 369 — TanningFrame availability. Same shape + discipline as
    // Workshop. Reader: `CraftAtTanningFrameDse` eligibility filter.
    markers.set_colony(
        markers::HasFunctionalTanningFrame::KEY,
        has_functional_tanning_frame,
    );
    // 084 Commit 3: ColonyThornbriarChronicallyLow — chronicity latch
    // sampled at `chronicity_window_ticks` boundaries against the
    // colony-wide stash total. Reader (this commit): `FarmDse`'s
    // `farm_herb_pressure` axis (MarkerConsideration). Reader
    // (coordinator-side): `accumulate_build_pressure`'s farming-gate
    // disjunction. Mirrors the `ColonyStoresChronicallyFull` shape.
    markers.set_colony(
        markers::ColonyThornbriarChronicallyLow::KEY,
        colony_thornbriar_chronically_low,
    );

    let herb_positions: Vec<(Entity, Position, HerbKind)> = world_state
        .herb_query
        .iter()
        .map(|(e, herb, p)| (e, *p, herb.kind))
        .collect();

    markers.set_colony(markers::ThornbriarAvailable::KEY, thornbriar_available);
    markers.set_colony(markers::WardStrengthLow::KEY, ward_strength_low);

    // Territory corruption — max corruption in the ring around colony center.
    let territory_max_corruption = {
        let mc = &res.constants.magic;
        let inner = mc.territory_corruption_inner_radius as i32;
        let outer = mc.territory_corruption_outer_radius as i32;
        let cx = res.colony_center.0.x();
        let cy = res.colony_center.0.y();
        let mut max_c = 0.0f32;
        for y in (cy - outer)..=(cy + outer) {
            for x in (cx - outer)..=(cx + outer) {
                if !res.map.in_bounds(x, y) {
                    continue;
                }
                let dist = (x - cx).abs() + (y - cy).abs();
                if dist >= inner && dist <= outer {
                    max_c = max_c.max(res.map.get(x, y).corruption);
                }
            }
        }
        max_c
    };

    markers.set_colony(markers::WardsUnderSiege::KEY, wards_under_siege);

    let colony_injury_count = query
        .iter()
        .filter(|((_, _, _, _, _, _, _, health, _), _)| health.current < 1.0)
        .count();

    let directive_snapshot: HashMap<Entity, (usize, Option<Directive>)> = world_state
        .directive_queue_query
        .iter()
        .map(|(entity, q)| (entity, (q.directives.len(), q.directives.first().cloned())))
        .collect();

    let action_snapshot: Vec<(Entity, Position, Action)> = query
        .iter()
        .map(
            |((entity, _, _, _, pos, _, _, _, _), (_, _, current, _, _, _, _, _, _, _, _))| {
                (entity, *pos, current.action)
            },
        )
        .collect();

    // 487 — peers mid-`GroomOther` (both the actor and the actor's
    // `target_entity` last tick). Consumed by
    // `viable_groom_candidate_for` below to mask chain-grooming
    // participants out of every other cat's candidate set so a new
    // cat doesn't join the pile while the chain is in flight. Both
    // groomer AND groomee land in the set: the groomee is unavailable
    // (receiving care), the groomer is unavailable (delivering it) —
    // without both exclusions the pile survives by chain-extension,
    // because a groomer-A is still a viable target for cat-C while
    // A is busy grooming B. Read from `CurrentAction` (set last tick
    // by `resolve_goap_plans`) over only cats `With<GoapPlan>`; cats
    // currently planning (in this query) haven't started executing
    // yet, so they can't be active groomers.
    let currently_groomed: std::collections::HashSet<Entity> = {
        let mut set = std::collections::HashSet::new();
        for (actor, ca) in world_state.groom_actor_query.iter() {
            if matches!(ca.action, Action::GroomOther) {
                set.insert(actor);
                if let Some(target) = ca.target_entity {
                    set.insert(target);
                }
            }
        }
        set
    };

    // Ticket 014 Mentoring batch — `has_mentoring_target_fn` closure
    // retired. The predicate now lives in
    // `aspirations::update_mentoring_target_markers`, the snapshot
    // population below routes the result through `MarkerSnapshot`, and
    // `MentorDse.eligibility()` requires `HasMentoringTarget::KEY`.

    // Pre-compute stores positions for zone distance calculations.
    let stores_positions: Vec<Position> = world_state
        .building_query
        .iter()
        .filter(|(_, s, _, _, _)| s.kind == StructureType::Stores)
        .map(|(_, _, p, _, _)| *p)
        .collect();

    // Pre-compute kitchen positions (completed only) for zone distance.
    let kitchen_positions: Vec<Position> = world_state
        .building_query
        .iter()
        .filter(|(_, s, _, site, _)| s.kind == StructureType::Kitchen && site.is_none())
        .map(|(_, _, p, _, _)| *p)
        .collect();

    // 367: Pre-compute preservation-station positions for zone
    // resolution. Same shape as `kitchen_positions` — completed
    // buildings only. The load + cooldown discrimination happens at
    // resolver-time; the zone resolver just answers "where is the
    // nearest rack of this kind?".
    let drying_rack_positions: Vec<Position> = world_state
        .building_query
        .iter()
        .filter(|(_, s, _, site, _)| s.kind == StructureType::DryingRack && site.is_none())
        .map(|(_, _, p, _, _)| *p)
        .collect();
    let smoking_rack_positions: Vec<Position> = world_state
        .building_query
        .iter()
        .filter(|(_, s, _, site, _)| s.kind == StructureType::SmokingRack && site.is_none())
        .map(|(_, _, p, _, _)| *p)
        .collect();
    // 457: Pre-compute Workshop positions for `PlannerZone::Workshop`
    // zone resolution. Same shape as `kitchen_positions` /
    // `drying_rack_positions` — completed buildings only. Recipe and
    // input checks happen at resolver-time.
    let workshop_positions: Vec<Position> = world_state
        .building_query
        .iter()
        .filter(|(_, s, _, site, _)| s.kind == StructureType::Workshop && site.is_none())
        .map(|(_, _, p, _, _)| *p)
        .collect();
    // 369: Pre-compute TanningFrame positions for
    // `PlannerZone::TanningFrame` zone resolution. Same shape as
    // `workshop_positions`.
    let tanning_frame_positions: Vec<Position> = world_state
        .building_query
        .iter()
        .filter(|(_, s, _, site, _)| s.kind == StructureType::TanningFrame && site.is_none())
        .map(|(_, _, p, _, _)| *p)
        .collect();

    // 035: Pre-compute dead-cat positions for the `CorpseTarget`
    // zone in `build_zone_distances`. Disjoint from the other
    // position snapshots — the dead cats are filtered `With<Dead>`
    // upstream in `WorldStateQueries::dead_cat_query`.
    let dead_cat_positions: Vec<(Entity, Position)> = world_state
        .dead_cat_query
        .iter()
        .map(|(e, p)| (e, *p))
        .collect();

    // Snapshot per-cat fields needed by the mating eligibility gate.
    let current_day_phase = mating_fitness_params.current_day_phase();

    for (
        (entity, name, needs, personality, pos, memory, skills, health, body_model),
        (
            magic_aff,
            inventory,
            mut current,
            aspirations,
            preferences,
            fated_love,
            fated_rival,
            fulfillment,
            context_beliefs,
            prev_safety_deficit,
            focal_age,
        ),
    ) in &mut query
    {
        if current.ticks_remaining != 0 {
            continue;
        }

        // §4 batch 2: can_hunt/can_forage retired — computed by
        // `update_capability_markers` and read from MarkerSnapshot below.

        // §6.5.6 target-taking DSE: four-axis bundle (nearness /
        // kitten-hunger / kinship Piecewise / isolation) drives
        // `hungry_kitten_urgency` and surfaces the argmax kitten for the
        // FeedKitten step below. `is_parent_of_hungry_kitten` stays
        // bloodline-override (any own-kitten in range, not just argmax).
        // Ticket 158 — the `parent_marker_active` flag promotes the
        // adult's closest hungry own-kitten as a fallback candidate
        // when the per-tick range gate excludes every in-range option,
        // so a parent at the colony heart whose kittens momentarily
        // drift out of the 12-tile gate still gets a non-zero urgency
        // and clears the `if hungry_kitten_urgency > 0.0` scoring gate.
        let parent_marker_active = marker_qs
            .parent_hungry_kitten_q
            .get(entity)
            .unwrap_or(false);
        let caretake_resolution = crate::ai::dses::caretake_target::resolve_caretake_target(
            &res.dse_registry,
            entity,
            *pos,
            &kitten_snapshot,
            &cat_positions,
            res.time.tick,
            // Scorer pre-check; focal capture happens at the
            // step-resolution site (goap.rs: FeedKitten step).
            None,
            parent_marker_active,
            &mut res.dse_scratchpad,
        );
        // §Phase 4c.4 alloparenting Reframe A: bond-weighted compassion.
        // See disposition.rs companion site.
        let caretake_bond_scale = crate::ai::caretake_targeting::caretake_compassion_bond_scale(
            entity,
            &caretake_resolution,
            sc.caretake_bond_compassion_boost_max,
            |a, b| res.relationships.get(a, b).map(|r| r.fondness),
        );

        // Ticket 014 §4 sensing batch — `has_social_target` /
        // `has_threat_nearby` now read from `MarkerSnapshot` after
        // `sensing::update_target_existence_markers` authors the ZSTs.
        // The inline `resolve_socialize_target` bool-only call retires;
        // the L2 step-resolution site at goap.rs:~2038 still calls the
        // resolver to pick the actual target.

        // Allies-fighting still needs the nearest-threat position to
        // count co-fighting cats. Threat-radius and ally-radius reads
        // are radial visual/auditory perception, so they use
        // `euclidean_distance` — the 494 escape hatch — rather than
        // the Chebyshev tactical metric. Mirror of the parallel scan
        // in `disposition.rs::evaluate_dispositions`.
        let nearest_threat = wildlife_positions
            .iter()
            .filter(|(_, wp, _)| pos.euclidean_distance(wp) <= d.wildlife_threat_range)
            .min_by_key(|(_, wp, _)| pos.tile_distance_squared(wp));

        let allies_fighting_threat = if let Some(&(_, threat_pos, _)) = nearest_threat {
            action_snapshot
                .iter()
                .filter(|(e, ally_pos, action)| {
                    *e != entity
                        && *action == Action::Fight
                        && ally_pos.euclidean_distance(&threat_pos) <= d.allies_fighting_range
                })
                .count()
                .min(d.allies_fighting_cap)
        } else {
            0
        };

        let combat_effective =
            skills.combat + skills.hunting * d.combat_effective_hunting_cross_train;
        // 095 Phase 1 Stage B — anatomical pain replaces severe-injury count.
        let max_pain: f32 = res.constants.combat.body_zone_pain_weights.iter().sum();
        let is_incapacitated = max_pain > 0.0
            && (body_model.total_pain(&res.constants.combat.body_zone_pain_weights) / max_pain)
                > res.constants.combat.pain_incapacitation_threshold;
        // §4.3 per-cat marker population. Bit-for-bit mirrors the
        // inline `is_incapacitated` above — kept side-by-side so
        // `MarkerSnapshot::has("Incapacitated", entity)` resolves
        // identically to `ScoringContext.is_incapacitated` for any
        // DSE that later wires `.forbid("Incapacitated")` (§13.1).
        markers.set_entity(markers::Incapacitated::KEY, entity, is_incapacitated);
        if let Ok((k, y, a, e, newborn, eyes_open, juvenile, mentorable)) = life_stage_q.get(entity)
        {
            markers.set_entity(markers::Kitten::KEY, entity, k);
            markers.set_entity(markers::Young::KEY, entity, y);
            markers.set_entity(markers::Adult::KEY, entity, a);
            markers.set_entity(markers::Elder::KEY, entity, e);
            // 450 sub-stage + mentorable populates. NewbornKitten /
            // EyesOpenKitten / JuvenileKitten are mutually exclusive
            // within `Kitten`; MentorableAge = `JuvenileKitten ∨ Young ∨ Adult`.
            markers.set_entity(markers::NewbornKitten::KEY, entity, newborn);
            markers.set_entity(markers::EyesOpenKitten::KEY, entity, eyes_open);
            markers.set_entity(markers::JuvenileKitten::KEY, entity, juvenile);
            markers.set_entity(markers::MentorableAge::KEY, entity, mentorable);
        }
        // §4 batch 1 + batch 2: per-cat markers read from authored ZSTs.
        // 367: read preservation markers via the bundled sibling query
        // inside `marker_qs`. Split off from `per_cat_markers_q` because
        // adding eight rows pushed that tuple past Bevy's `QueryData`
        // arity limit (15) and the parent system past the 16-param
        // SystemParam budget. False defaults if the entity somehow
        // isn't in the sibling query.
        let (
            can_dry,
            can_smoke,
            has_raw_fish,
            has_raw_organ,
            has_raw_meat,
            has_fuel,
            has_dryable,
            has_smokeable,
            can_satisfy_workshop,
            can_satisfy_tanning,
            can_craft,
        ) = marker_qs.preservation_markers_q.get(entity).unwrap_or((
            false, false, false, false, false, false, false, false, false, false, false,
        ));
        if let Ok((
            injured,
            has_herbs,
            has_remedy,
            has_ward,
            is_coord_dir,
            can_hunt,
            can_forage,
            can_ward,
            can_ward_from_supply,
            can_cook,
            has_materials,
            has_curios,
            has_food_in_inventory,
        )) = per_cat_markers_q.get(entity)
        {
            markers.set_entity(markers::Injured::KEY, entity, injured);
            markers.set_entity(markers::HasHerbsInInventory::KEY, entity, has_herbs);
            markers.set_entity(markers::HasRemedyHerbs::KEY, entity, has_remedy);
            markers.set_entity(markers::HasWardHerbs::KEY, entity, has_ward);
            // 450 — generic food-in-inventory marker.
            markers.set_entity(
                markers::HasFoodInInventory::KEY,
                entity,
                has_food_in_inventory,
            );
            markers.set_entity(
                markers::IsCoordinatorWithDirectives::KEY,
                entity,
                is_coord_dir,
            );
            // §4 batch 2: capability markers.
            markers.set_entity(markers::CanHunt::KEY, entity, can_hunt);
            markers.set_entity(markers::CanForage::KEY, entity, can_forage);
            markers.set_entity(markers::CanWard::KEY, entity, can_ward);
            // 084 Commit-3 follow-on (418): CanWardFromSupply read+populate.
            // The marker is authored by `update_capability_markers` but
            // the eligibility filter on `HerbcraftWardDse` reads it via
            // `MarkerSnapshot.has(...)` — without this line the snapshot
            // returns `false` and Herbcraft ward placement silently
            // dies (verified: 0 Thornward placements on seed-42 vs 4
            // pre-084-baseline).
            markers.set_entity(
                markers::CanWardFromSupply::KEY,
                entity,
                can_ward_from_supply,
            );
            markers.set_entity(markers::CanCook::KEY, entity, can_cook);
            // 235: per-class inventory-content markers (scaffolding for
            // class-specific deposit routing — reader for HasMaterialsIn-
            // Inventory ships with the 235-follow-on material-pile ticket;
            // reader for HasCuriosInInventory ships with ticket 16's Cache).
            markers.set_entity(markers::HasMaterialsInInventory::KEY, entity, has_materials);
            markers.set_entity(markers::HasCuriosInInventory::KEY, entity, has_curios);
            // 367 — preservation per-cat markers. Missing any of these
            // set_entity lines would silently mask the new DSE
            // eligibility filters (third-clause discipline; precedent
            // §209 / §084).
            markers.set_entity(markers::CanDry::KEY, entity, can_dry);
            markers.set_entity(markers::CanSmoke::KEY, entity, can_smoke);
            markers.set_entity(markers::HasRawFishInInventory::KEY, entity, has_raw_fish);
            markers.set_entity(markers::HasRawOrganInInventory::KEY, entity, has_raw_organ);
            markers.set_entity(markers::HasRawMeatInInventory::KEY, entity, has_raw_meat);
            markers.set_entity(markers::HasFuelInInventory::KEY, entity, has_fuel);
            markers.set_entity(markers::HasDryableInInventory::KEY, entity, has_dryable);
            markers.set_entity(markers::HasSmokeableInInventory::KEY, entity, has_smokeable);
            // 367 follow-on — `HasDryableAccessible` composite. Widens
            // `DryFoodDse` eligibility past "cat already has dryable in
            // inventory" to "cat could conceivably go dry something":
            //   has_dryable_inv OR (has_free_slot AND colony has
            //   dryable in stores).
            // Without this, a cat that just deposited at Stores has an
            // empty inventory, the narrow `HasDryableInInventory`
            // marker is off, and DryFood is permanently ineligible —
            // the racks-but-zero-loading defect that motivated this
            // commit.
            let has_free_slot = !inventory.is_full();
            let has_dryable_accessible = has_dryable || (has_free_slot && has_dryable_in_stores);
            markers.set_entity(
                markers::HasDryableAccessible::KEY,
                entity,
                has_dryable_accessible,
            );
            // 443 — `HasSmokeableAccessible` composite. Mirrors
            // `HasDryableAccessible` for the two-ingredient smoking
            // chain. A cat with smokeable inventory fires the left
            // disjunct; a cat with a free slot and smokeable+fuel
            // in stores fires the right disjunct. Either is enough
            // to make `SmokeMeatDse` eligible — the plan template's
            // `[RetrieveSmokeableMeat, RetrieveSmokeableFuel, SmokeMeat]`
            // sequence handles the retrieve legs.
            let has_smokeable_accessible =
                has_smokeable || (has_free_slot && has_smokeable_in_stores);
            markers.set_entity(
                markers::HasSmokeableAccessible::KEY,
                entity,
                has_smokeable_accessible,
            );
            // 457 — Workshop-craft per-cat capability. CanCraft mirrors
            // CanCook / CanDry / CanSmoke (Adult ∧ ¬Injured).
            markers.set_entity(markers::CanCraft::KEY, entity, can_craft);
            // 468 — recipe-aware craft eligibility markers. Replaces
            // the 457 `HasCraftInputInInventory` (recipe-agnostic any-
            // input gate). Each marker fires iff the cat's pouch alone
            // satisfies the full input set of at least one recipe at
            // the matching station.
            markers.set_entity(
                markers::CanSatisfyAnyWorkshopRecipeFromPouch::KEY,
                entity,
                can_satisfy_workshop,
            );
            markers.set_entity(
                markers::CanSatisfyAnyTanningFrameRecipeFromPouch::KEY,
                entity,
                can_satisfy_tanning,
            );
        }
        // §4.2 State markers — InCombat / OnCorruptedTile /
        // OnSpecialTerrain. Authored in Chain 2a alongside the other §4
        // marker authors; predicate parity with the inline
        // `on_corrupted_tile` / `on_special_terrain` computations
        // below is enforced by the author systems' rustdoc and tests.
        if let Ok((in_combat, on_corrupted_marker, on_special_marker)) = state_markers_q.get(entity)
        {
            markers.set_entity(markers::InCombat::KEY, entity, in_combat);
            markers.set_entity(markers::OnCorruptedTile::KEY, entity, on_corrupted_marker);
            markers.set_entity(markers::OnSpecialTerrain::KEY, entity, on_special_marker);
        }
        // Per-cat stores-reachability. Authored here (pre-`score_actions`)
        // because `HasFoodStorageAccessible` is required by `PickingUpDse`
        // eligibility — checked inside `score_actions`, so a later
        // author site would miss the L2 pass. `HasHerbStashAccessible`
        // shares the same geometry (food and herbs deposit to the same
        // `Stores` building) and is consumed downstream by the plan-
        // template's `HasMarker` predicate; co-authoring keeps the two
        // markers in lockstep. See `herb_stash_accessible_for` for the
        // reachability math.
        let stores_reachable =
            herb_stash_accessible_for(pos, &stores_positions, d.herb_stash_reachable_radius);
        markers.set_entity(
            markers::HasHerbStashAccessible::KEY,
            entity,
            stores_reachable,
        );
        markers.set_entity(
            markers::HasFoodStorageAccessible::KEY,
            entity,
            stores_reachable,
        );
        // 487 — `HasGroomCandidate` author. Mirrors the structural
        // shape of `HasFoodStorageAccessible` (484 precedent): a
        // per-cat reachability marker authored from a colony-wide
        // scan, required on the `GroomOtherDse` eligibility filter.
        // See `viable_groom_candidate_for` for the predicate.
        markers.set_entity(
            markers::HasGroomCandidate::KEY,
            entity,
            viable_groom_candidate_for(entity, pos, &cat_positions, &currently_groomed),
        );
        // Ticket 027 Bug 2 — HasEligibleMate authored by
        // `mating::update_mate_eligibility_markers`. Ticket 103 —
        // second tuple element is `Has<PairingActivity>`; carried out
        // of the snapshot block so the populator below can read it
        // for `escape_viability`'s dependent-presence term.
        let has_pair_bond = if let Ok((has_mate, has_pairing)) = mate_eligibility_q.get(entity) {
            markers.set_entity(markers::HasEligibleMate::KEY, entity, has_mate);
            has_pairing
        } else {
            false
        };
        // Ticket 014 Mentoring batch — Mentor / Apprentice authored by
        // `aspirations::update_training_markers`; HasMentoringTarget by
        // `aspirations::update_mentoring_target_markers`.
        if let Ok((is_mentor, is_apprentice, has_mentoring_target)) = mentoring_q.get(entity) {
            markers.set_entity(markers::Mentor::KEY, entity, is_mentor);
            markers.set_entity(markers::Apprentice::KEY, entity, is_apprentice);
            markers.set_entity(
                markers::HasMentoringTarget::KEY,
                entity,
                has_mentoring_target,
            );
        }
        // Ticket 014 §4 sensing batch — broad-phase target-existence
        // markers authored by `sensing::update_target_existence_markers`.
        // Ticket 170 extends the tuple with `HideEligible` (authored by
        // the sibling `sensing::update_hide_eligible_markers`).
        if let Ok((threat, social, herbs, prey, carcass, unburied, hide_eligible)) =
            marker_qs.target_existence_q.get(entity)
        {
            markers.set_entity(markers::HasThreatNearby::KEY, entity, threat);
            markers.set_entity(markers::HasSocialTarget::KEY, entity, social);
            markers.set_entity(markers::HasHerbsNearby::KEY, entity, herbs);
            markers.set_entity(markers::PreyNearby::KEY, entity, prey);
            markers.set_entity(markers::CarcassNearby::KEY, entity, carcass);
            // 035: HasUnburiedCorpse populates the snapshot so the L3
            // scoring gate (`score_actions::if inputs.markers.has(...)`)
            // and the Bury DSE's eligibility filter both read from
            // the same source of truth.
            markers.set_entity(markers::HasUnburiedCorpse::KEY, entity, unburied);
            // Ticket 170 — Hide DSE eligibility filter reads
            // `HideEligible` via `MarkerSnapshot::has(...)`.
            markers.set_entity(markers::HideEligible::KEY, entity, hide_eligible);
        }
        // Ticket 049 §9.2 — faction overlay markers (Visitor /
        // HostileVisitor / Banished / BefriendedAlly). The runtime §9.3
        // prefilter reads these via `ExecutorContext::stance_overlays_of`
        // (a parallel Has<...> query); the snapshot mirror keeps
        // `MarkerSnapshot::has(KEY, entity)` consistent for diagnostics.
        if let Ok((visitor, hostile_visitor, banished, befriended_ally)) =
            marker_qs.faction_overlay_q.get(entity)
        {
            markers.set_entity(markers::Visitor::KEY, entity, visitor);
            markers.set_entity(markers::HostileVisitor::KEY, entity, hostile_visitor);
            markers.set_entity(markers::Banished::KEY, entity, banished);
            markers.set_entity(markers::BefriendedAlly::KEY, entity, befriended_ally);
        }
        // Ticket 397 Layer 1 — Parent + HasJuvenileDependent mirror
        // into `MarkerSnapshot`. `score_actions` reads `Parent` to gate
        // Caretake's pool entry every tick the cat structurally has a
        // dependent kitten (per §L2.10.4 — every DSE whose emit
        // precondition holds belongs in the candidate pool). Authored
        // each tick by `growth::update_parent_markers` (Chain 2a,
        // before this loop).
        if let Ok((is_parent, has_juv_dep)) = marker_qs.parent_markers_q.get(entity) {
            markers.set_entity(markers::Parent::KEY, entity, is_parent);
            markers.set_entity(markers::HasJuvenileDependent::KEY, entity, has_juv_dep);
        }

        // Ticket 014 §4 sensing batch — `has_herbs_nearby` /
        // `prey_nearby` now read from `MarkerSnapshot`. Ticket 064 (§5.6.3
        // #6 cutover) further retired the inline `nearby_carcass_count`
        // loop: carcass-aware DSEs now consume the `carcass_scent_at_position`
        // perception scalar (read from `CarcassScentMap` at ScoringContext
        // build time), and the boolean facet still reads from
        // `MarkerSnapshot::CarcassNearby`.
        let has_herbs_nearby = markers.has(markers::HasHerbsNearby::KEY, entity);
        let prey_nearby = markers.has(markers::PreyNearby::KEY, entity);
        let has_threat_nearby = markers.has(markers::HasThreatNearby::KEY, entity);
        let has_social_target = markers.has(markers::HasSocialTarget::KEY, entity);

        let (on_corrupted_tile, tile_corruption, on_special_terrain) =
            if res.map.in_bounds(pos.x(), pos.y()) {
                let tile = res.map.get(pos.x(), pos.y());
                (
                    tile.corruption > d.corrupted_tile_threshold,
                    tile.corruption,
                    matches!(tile.terrain, Terrain::FairyRing | Terrain::StandingStone),
                )
            } else {
                (false, 0.0, false)
            };

        // "Smell the rot": sample the map within corruption_smell_range tiles
        // and take the max. This lets cats proactively react to corruption
        // before they're standing on it.
        // §L2.10.7: also track the *position* of the most-corrupted
        // tile so the §L2.10.7 NearestCorruptedTile anchor (consumed
        // by Cleanse + DurableWard) can resolve to a concrete
        // coordinate. None when no tile in the smell radius is above
        // the corrupted_tile_threshold — the consideration scores 0
        // and the CP gate suppresses the DSE.
        let (nearby_corruption_level, nearest_corrupted_tile) = {
            let r = sc.corruption_smell_range as i32;
            let mut max_c: f32 = 0.0;
            let mut max_pos: Option<crate::components::physical::Position> = None;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() + dy.abs() > r {
                        continue; // Manhattan radius
                    }
                    let nx = pos.x() + dx;
                    let ny = pos.y() + dy;
                    if res.map.in_bounds(nx, ny) {
                        let c = res.map.get(nx, ny).corruption;
                        if c > max_c {
                            max_c = c;
                            if c > d.corrupted_tile_threshold {
                                max_pos = Some(crate::components::physical::Position::new(nx, ny));
                            }
                        }
                    }
                }
            }
            (max_c, max_pos)
        };

        // Ticket 027 Bug 2: inline `has_eligible_mate` retired —
        // `mating::update_mate_eligibility_markers` authors the
        // `HasEligibleMate` ZST per tick; `MateDse.eligibility()`
        // requires it via the marker snapshot populated above.

        let memory_sums = crate::ai::scoring::memory_proximity_sums(memory, pos, sc);
        let colony_knowledge_sums = colony
            .knowledge
            .as_ref()
            .map(|ck| crate::ai::scoring::colony_knowledge_proximity_sums(ck, pos, sc))
            .unwrap_or((0.0, 0.0));
        let cascade_counts = crate::ai::scoring::compute_cascade_counts(
            &action_snapshot,
            entity,
            pos,
            d.cascading_bonus_range,
        );
        let aspiration_action_counts = aspirations
            .map(crate::ai::scoring::compute_aspiration_action_counts)
            .unwrap_or([0.0; crate::ai::scoring::CASCADE_COUNTS_LEN]);
        let preference_signals = preferences
            .map(crate::ai::scoring::compute_preference_signals)
            .unwrap_or([0.0; crate::ai::scoring::CASCADE_COUNTS_LEN]);
        let love_visible = fated_love
            .filter(|l| l.awakened)
            .and_then(|l| cat_positions.iter().find(|(e, _)| *e == l.partner))
            .is_some_and(|(_, pp)| {
                crate::systems::sensing::observer_sees_at(
                    crate::components::SensorySpecies::Cat,
                    *pos,
                    &res.constants.sensory.cat,
                    *pp,
                    crate::components::SensorySignature::CAT,
                    d.fated_love_detection_range,
                )
            });
        let rival_nearby = fated_rival
            .filter(|r| r.awakened)
            .and_then(|r| cat_positions.iter().find(|(e, _)| *e == r.rival))
            .is_some_and(|(_, rp)| {
                crate::systems::sensing::observer_sees_at(
                    crate::components::SensorySpecies::Cat,
                    *pos,
                    &res.constants.sensory.cat,
                    *rp,
                    crate::components::SensorySignature::CAT,
                    d.fated_rival_detection_range,
                )
            });
        let (active_directive_action_ordinal, active_directive_bonus) =
            if let Ok(directive) = world_state.active_directive_query.get(entity) {
                // 487 — colony-self directives carry `coordinator: None`.
                // The fondness factor falls through to `fondness_default`
                // (the same neutral midpoint a coordinator with no recorded
                // relationship would yield); the social-weight multiplier
                // already reflects the colony-self constant because
                // `assess_colony_needs` writes
                // `colony_self_directive_weight` into
                // `coordinator_social_weight` at delivery time.
                let fondness_factor = directive
                    .coordinator
                    .and_then(|c| res.relationships.get(entity, c))
                    .map_or(d.fondness_default, |r| (r.fondness + 1.0) / 2.0);
                let bonus = directive.priority
                    * directive.coordinator_social_weight
                    * d.directive_bonus_base_weight
                    * personality.diligence
                    * fondness_factor
                    * (1.0 - personality.independence * d.directive_independence_penalty)
                    * (1.0 - personality.stubbornness * d.directive_stubbornness_penalty);
                (directive.kind.to_action() as usize as f32, bonus)
            } else {
                (-1.0, 0.0)
            };

        // Ticket 228 — per-cat route-cost field. Flooded once per
        // replan with overlay-aware edge weights (terrain +
        // {boldness-conditioned | patrol-tuned} fox-scent +
        // corruption). The L2 `Consideration::Field` evaluator reads
        // this via `EvalCtx.field_cost`; step resolvers (commit 10+)
        // read the inserted Component to walk the gradient.
        //
        // 256 R4 — Guarding-disposed cats (proxy: cats whose previous
        // action was `Action::Patrol`) get patrol-tuned overlay
        // weights instead of the boldness-derived weight Flee uses.
        // Patrol cats avoid corruption and fox-scent corridors more
        // aggressively because the patrol role is precisely about
        // not walking the colony into rot or ambush corridors.
        // The proxy carries a one-tick lag on first entry into
        // Guarding (the field still gates Patrol's L2 score, just
        // with non-patrol overlay weights for that one tick).
        let cat_route_cost_field = {
            let fox_overlay =
                crate::ai::pathfinding::FoxScentOverlay::new(&colony.fox_scent_map, sc);
            let corr_overlay = crate::ai::pathfinding::CorruptionOverlay::new(&res.map, sc);
            let (fox_w, corr_w) = if current.action == Action::Patrol {
                (
                    sc.patrol_path_fox_scent_weight,
                    sc.patrol_path_corruption_weight,
                )
            } else {
                let w = crate::ai::pathfinding::cat_path_weight_from_boldness(personality.boldness);
                (w, w)
            };
            // 508 — the replan-time route-cost field prices the
            // cat's own threat beliefs; boldness/patrol weighting
            // matches the fox-scent axis (threat-of-death respects
            // the same personality conditioning).
            let threat_overlay = world_state
                .location_beliefs
                .get(entity)
                .ok()
                .map(|lb| crate::ai::pathfinding::ThreatBeliefOverlay::new(lb, sc));
            let mut overlays: Vec<crate::ai::pathfinding::WeightedOverlay> = vec![
                crate::ai::pathfinding::WeightedOverlay::new(&fox_overlay, fox_w),
                crate::ai::pathfinding::WeightedOverlay::new(&corr_overlay, corr_w),
            ];
            if let Some(t) = threat_overlay.as_ref() {
                overlays.push(crate::ai::pathfinding::WeightedOverlay::new(t, fox_w));
            }
            crate::ai::route_cost::flood_dijkstra(
                *pos,
                &res.map,
                &overlays,
                sc.route_cost_flood_budget,
                res.time.tick,
                &mut res.route_buckets,
            )
        };

        // Ticket 228 — replan-time anchor resolution for Hunt /
        // Wander route-cost axes. Resolved here (not in
        // CatAnchorPositions builder) so they share the same
        // replan-cadence as the field flood.
        let cat_nearest_prey = colony
            .prey_scent_maps
            .highest_nearby_any(pos.x(), pos.y(), d.scent_search_radius)
            .map(|(x, y)| Position::new(x, y));
        let cat_wander_target = {
            let radius = (8.0 + personality.curiosity.clamp(0.0, 1.0) * 12.0) as i32;
            let recandidate = sc.wander_recandidate_ticks.max(1);
            let seed = (res.time.tick / recandidate) ^ entity.to_bits();
            let dx = (seed as i32).rem_euclid(2 * radius + 1) - radius;
            let dy = ((seed >> 16) as i32).rem_euclid(2 * radius + 1) - radius;
            let candidate = Position::new(pos.x() + dx, pos.y() + dy);
            res.map
                .in_bounds(candidate.x(), candidate.y())
                .then_some(candidate)
        };

        // Persist the field as a Component so step resolvers
        // (commit 10+) can sample `cost_at` and walk the gradient
        // in subsequent ticks. Re-inserted on every replan; stale
        // fields are detected by `origin_tick` mismatch.
        commands.entity(entity).insert(cat_route_cost_field.clone());

        let ctx = ScoringContext {
            scoring: sc,
            disposition_constants: d,
            needs,
            personality,
            food_available,
            has_social_target,
            has_threat_nearby,
            allies_fighting_threat,
            combat_effective,
            health: health.current,
            // Ticket 087 — interoceptive perception. Compute via the
            // perception module's helpers so the scalar derivation lives
            // in one place across all consumers.
            pain_level: crate::systems::interoception::pain_level(
                body_model,
                &res.constants.combat.body_zone_pain_weights,
                d.pain_normalization_max,
            ),
            body_distress_composite: crate::systems::interoception::body_distress_composite(
                needs, health,
            ),
            // Ticket 090 — interoceptive perception. `skills` and
            // `aspirations` already bound from the cat query.
            mastery_confidence: crate::systems::interoception::mastery_confidence(skills),
            purpose_clarity: crate::systems::interoception::purpose_clarity(aspirations),
            esteem_distress: crate::systems::interoception::esteem_distress(needs),
            // Ticket 103 — threat-coupled escape viability.
            // `nearest_threat` here (line ~1232 above) is the same
            // `Option<&(Entity, Position)>` shape as the disposition
            // populator. Dependent presence is marker-only in v1:
            // Parent ZST OR active pair-bond. Positional refinement
            // parked as ticket 128.
            escape_viability: crate::systems::interoception::escape_viability(
                *pos,
                nearest_threat.map(|(_, p, _)| *p),
                // Ticket 138 — cats are always at 1.0 in v1; see
                // disposition.rs companion site for rationale.
                1.0,
                // Threat cadence from the wildlife scan tuple.
                nearest_threat.map(|(_, _, pt)| *pt).unwrap_or(1.0),
                &res.map,
                markers.has(markers::Parent::KEY, entity) || has_pair_bond,
                &res.constants.escape_viability,
            ),
            // Ticket 108 — `safety_deficit_now - prev` rising-only.
            // First-tick / lazy-insert cats see prev = now (derivative
            // = 0). The companion `update_prev_safety_deficit` system
            // writes the snapshot back after this scoring pass.
            threat_proximity_derivative: {
                let now = (1.0 - needs.safety).clamp(0.0, 1.0);
                let prev = prev_safety_deficit.map(|p| p.0).unwrap_or(now);
                crate::components::PrevSafetyDeficit::rising_derivative(now, prev)
            },
            // Ticket 109 (Phase A) — composite social-status pressure.
            // Reads cross-cat Age + Needs via WorldStateQueries
            // lookups; `Relationships` resource gives bond data.
            social_status_distress: crate::systems::interoception::social_status_distress(
                entity,
                *pos,
                focal_age,
                needs.respect,
                &cat_positions,
                |e| world_state.age_query.get(e).ok().map(|a| a.born_tick),
                |e| world_state.needs_query.get(e).ok().map(|n| n.respect),
                &res.relationships,
                res.time.tick,
                sc.social_perception_radius,
                sc.social_status_distress_age_normalization_ticks,
                sc.social_status_distress_respect_weight,
                sc.social_status_distress_age_weight,
                sc.social_status_distress_bond_weight,
            ),
            is_incapacitated,
            has_construction_site,
            has_damaged_building,
            has_garden,
            food_fraction,
            inventory_food_fraction: inventory.food_count() as f32
                / crate::components::magic::Inventory::MAX_SLOTS as f32,
            magic_affinity: magic_aff.0,
            magic_skill: skills.magic,
            herbcraft_skill: skills.herbcraft,
            has_herbs_nearby,
            // §4 batch 1: read from authored markers via MarkerSnapshot.
            has_herbs_in_inventory: markers.has(markers::HasHerbsInInventory::KEY, entity),
            has_remedy_herbs: markers.has(markers::HasRemedyHerbs::KEY, entity),
            // 175: shared with planner-side projection.
            carrying: crate::ai::planner::Carrying::from_inventory(inventory),
            colony_injury_count,
            ward_strength_low,
            on_corrupted_tile,
            tile_corruption,
            nearby_corruption_level,
            on_special_terrain,
            is_coordinator_with_directives: markers
                .has(markers::IsCoordinatorWithDirectives::KEY, entity),
            pending_directive_count: directive_snapshot.get(&entity).map_or(0, |(len, _)| *len),
            prey_nearby,
            phys_satisfaction: needs.physiological_satisfaction(),
            respect: needs.respect,
            has_active_disposition: false,
            active_disposition: None,
            disposition_started_tick: 0,
            tradition_location_bonus: 0.0,
            hungry_kitten_urgency: caretake_resolution.urgency,
            is_parent_of_hungry_kitten: caretake_resolution.is_parent,
            parenting: colony.parenting_scalars.get(entity),
            kitten_cry_perceived: colony.kitten_cry_map.get(pos.x(), pos.y()),
            caretake_compassion_bond_scale: caretake_bond_scale,
            unexplored_nearby: colony.exploration_map.unexplored_fraction_nearby(
                pos.x(),
                pos.y(),
                d.explore_perception_radius,
                0.5,
            ),
            fox_scent_level: colony.fox_scent_map.get(pos.x(), pos.y()),
            // 294: per-cat `LocationBeliefs.recency_of_threat_cue`
            // sampled at the cat's current bucket — the per-cat
            // substrate replacement for the retired colony-wide
            // `RecentAmbushMap`. Reads 0.0 when the cat has no belief
            // entry for that bucket (never witnessed a nearby ambush)
            // OR the cat is missing the `LocationBeliefs` component
            // (test paths, freshly spawned). The `patrol_threat_recency`
            // field below uses the same shape against the patrol-sector
            // anchor; this read is at the cat's actual position.
            recent_ambush_at_position: world_state
                .location_beliefs
                .get(entity)
                .ok()
                .and_then(|lb| {
                    let key = crate::components::beliefs::bucket_position(pos.x(), pos.y());
                    lb.models.get(&key).map(|m| m.recency_of_threat_cue.value)
                })
                .unwrap_or(0.0),
            carcass_scent_at_position: colony.carcass_scent_map.get(pos.x(), pos.y()),
            // 301: coordinator-stamped ward-placement intent at cat's
            // position. Dormant at default — the resource is allocated
            // but unwritten because the populator short-circuits under
            // `ward_placement_semantics == SingleShotArgmax`.
            ward_intent_at_position: colony.ward_intent_map.get(pos.x(), pos.y()),
            // 101: env-quality influence-map samples at the cat's tile.
            // The four mood-relevant maps feed
            // `EnvironmentalQualityModifier`; `local_corruption` is
            // surfaced as a perception scalar for future DSE consumers.
            local_comfort: colony.comfort_map.get(pos.x(), pos.y()),
            local_cleanliness: colony.cleanliness_map.get(pos.x(), pos.y()),
            local_beauty: colony.beauty_map.get(pos.x(), pos.y()),
            local_mystery: colony.mystery_map.get(pos.x(), pos.y()),
            local_corruption: colony.corruption_influence_map.get(pos.x(), pos.y()),
            // 209: per-cat proxy for colony-tension. `(1 - safety)` is
            // the cat's current threat-deficit; consumed by the
            // `TensionDefusionGroomLift` modifier (dormant at 0.0).
            // Follow-on: aggregate across colony.
            colony_tension_recent: (1.0 - needs.safety).clamp(0.0, 1.0),
            // 263: per-cat `LocationBeliefs.recency_of_threat_cue`
            // sampled at the cat's patrol perimeter anchor bucket. Reads
            // 0.0 when the cat has no belief entry for that bucket OR
            // the cat is missing the `LocationBeliefs` component
            // entirely (test paths, freshly spawned). Mirrors
            // `recent_ambush_at_position` (which reads the colony-shared
            // RecentAmbushMap) but per-cat-subjective.
            patrol_threat_recency: world_state
                .location_beliefs
                .get(entity)
                .ok()
                .and_then(|lb| {
                    let anchor = colony
                        .ward_coverage_map
                        .sector_centroid(
                            crate::resources::ward_coverage_map::patrol_sector_id(
                                res.time.tick,
                                entity,
                                d.patrol_sector_grid_w,
                                d.patrol_sector_grid_h,
                                d.patrol_sector_rotation_ticks,
                            ),
                            d.patrol_sector_grid_w,
                            d.patrol_sector_grid_h,
                        )
                        .unwrap_or_else(|| {
                            crate::components::physical::Position::new(
                                res.colony_center.0.x() + d.patrol_perimeter_offset,
                                res.colony_center.0.y(),
                            )
                        });
                    let key = crate::components::beliefs::bucket_position(anchor.x(), anchor.y());
                    lb.models.get(&key).map(|m| m.recency_of_threat_cue.value)
                })
                .unwrap_or(0.0),
            // 268: Hide DSE belief-facet scalars. Read predator beliefs
            // at the nearest-threat entity (creature-specific) and
            // context beliefs at HereNow (ambient). Recency is the max
            // of the two so either path can drive Hide; intent clarity
            // only meaningful for a specific entity.
            hide_recency_of_threat_cue: {
                let creature_recency = nearest_threat
                    .map(|&(t, _, _)| t)
                    .and_then(|t| {
                        world_state
                            .predator_beliefs
                            .get(entity)
                            .ok()
                            .and_then(|pb| pb.models.get(&t).map(|m| m.recency_of_threat_cue.value))
                    })
                    .unwrap_or(0.0);
                let ambient_recency = world_state
                    .context_beliefs
                    .get(entity)
                    .ok()
                    .and_then(|cb| {
                        cb.models
                            .get(&crate::components::beliefs::EnvironmentalContextKey::HereNow)
                            .map(|m| m.recency_of_threat_cue.value)
                    })
                    .unwrap_or(0.0);
                creature_recency.max(ambient_recency).clamp(0.0, 1.0)
            },
            hide_perceived_intent_clarity: nearest_threat
                .map(|&(t, _, _)| t)
                .and_then(|t| {
                    world_state
                        .predator_beliefs
                        .get(entity)
                        .ok()
                        .and_then(|pb| pb.models.get(&t).map(|m| m.perceived_intent_clarity.value))
                })
                .unwrap_or(0.0)
                .clamp(0.0, 1.0),
            // Ticket 014 §4 sensing batch — read via marker. After ticket
            // 064 the marker's predicate is "CarcassScentMap > 0 at this
            // cat's tile", and `carcass_scent_at_position` provides the
            // magnitude axis (see `carcass_scent_at_position` field above).
            carcass_nearby: markers.has(markers::CarcassNearby::KEY, entity),
            territory_max_corruption,
            // Ticket 014 Magic colony batch — read via marker.
            wards_under_siege: markers.has(markers::WardsUnderSiege::KEY, entity),
            day_phase: current_day_phase,
            has_functional_kitchen,
            has_raw_food_in_stores,
            social_warmth_deficit: fulfillment.map_or(0.4, |f| f.social_warmth_deficit()),
            cat_anchors: crate::ai::scoring::CatAnchorPositions {
                nearest_corrupted_tile,
                nearest_construction_site: crate::systems::buildings::nearest_construction_site(
                    world_state
                        .building_query
                        .iter()
                        .map(|(_, s, p, site, _)| (s, p, site)),
                    *pos,
                ),
                // Ticket 439 — per-cat nearest-rack anchors. Built from
                // the same `drying_rack_positions` / `smoking_rack_positions`
                // slices the planner zone resolver consumes (goap.rs:1733-
                // 1744), so the L2 spatial axis and the L3 plan target
                // agree on what counts as a "rack." Pre-439 the three
                // preservation DSEs (DryFood / SmokeMeat / TendSmokingRack)
                // used `LandmarkAnchor::NearestKitchen` as a Commit-4
                // placeholder — see `dry_food.rs:97`, `smoke_meat.rs:73`,
                // `tend_smoking_rack.rs:78`.
                nearest_drying_rack: drying_rack_positions
                    .iter()
                    .min_by_key(|p| pos.tile_distance_squared(p))
                    .copied(),
                nearest_smoking_rack: smoking_rack_positions
                    .iter()
                    .min_by_key(|p| pos.tile_distance_squared(p))
                    .copied(),
                // §L2.10.7 Sleep anchor: cats sleep where they are
                // (no per-cat assigned sleeping spot exists today —
                // future component could replace this fallback). The
                // spatial axis evaluates to ~1.0 and Sleep's other
                // axes (energy_deficit, day_phase, injury_rest) drive
                // selection.
                own_sleeping_spot: Some(*pos),
                // §L2.10.7 Forage anchor: nearest forageable terrain
                // tile within forage_terrain_search_radius. None when
                // no forageable terrain in range — CanForage marker
                // gates the DSE entirely so this scan is wasted only
                // when the marker is true.
                nearest_forageable_cluster: crate::ai::capabilities::nearest_matching_tile(
                    pos,
                    &res.map,
                    d.forage_terrain_search_radius,
                    |t| t.foraging_yield() > 0.0,
                ),
                // §L2.10.7 HerbcraftGather anchor: Manhattan-nearest
                // harvestable herb position from world_state.herb_query.
                // None when no herbs in the world — HasHerbsNearby
                // marker (eligibility) gates the DSE entirely.
                nearest_herb_patch: world_state
                    .herb_query
                    .iter()
                    .map(|(_, _, p)| *p)
                    .min_by_key(|p| pos.tile_distance_squared(p)),
                // §L2.10.7 HerbcraftWard anchor: a perimeter anchor
                // offset from the colony center. (Distinct from the
                // ticket-256 patrol anchor below; HerbcraftWard's
                // anchor stays geometrically simple for now.)
                nearest_perimeter_tile: Some(crate::components::physical::Position::new(
                    res.colony_center.0.x() + d.patrol_perimeter_offset,
                    res.colony_center.0.y(),
                )),
                // 256 R3: per-replan ward-sector centroid. The cat's
                // patrol beat rotates through ward-protected sectors
                // of the demesne; falls back to the legacy static
                // offset when the WardCoverageMap has no coverage
                // (early-game, pre-ward).
                territory_perimeter_anchor: colony
                    .ward_coverage_map
                    .sector_centroid(
                        crate::resources::ward_coverage_map::patrol_sector_id(
                            res.time.tick,
                            entity,
                            d.patrol_sector_grid_w,
                            d.patrol_sector_grid_h,
                            d.patrol_sector_rotation_ticks,
                        ),
                        d.patrol_sector_grid_w,
                        d.patrol_sector_grid_h,
                    )
                    .or_else(|| {
                        Some(crate::components::physical::Position::new(
                            res.colony_center.0.x() + d.patrol_perimeter_offset,
                            res.colony_center.0.y(),
                        ))
                    }),
                // §L2.10.7 Flee anchor: position of the nearest
                // wildlife threat already scanned for allies_fighting.
                nearest_threat: nearest_threat.map(|&(_, p, _)| p),
                // 263: paired entity id for entity-pair affordance reads.
                // Same source as `nearest_threat` (the wildlife scan
                // tuple); both Some/None together.
                nearest_threat_entity: nearest_threat.map(|&(e, _, _)| e),
                // §L2.10.7 Coordinate anchor: colony center as the
                // coordinator's perch (single-perch model).
                coordinator_perch: Some(res.colony_center.0),
                // Ticket 089 — interoceptive self-anchors.
                own_safe_rest_spot: crate::systems::interoception::own_safe_rest_spot(
                    memory,
                    d.safe_rest_threat_suppression_radius,
                ),
                own_injury_site: crate::systems::interoception::own_injury_site(body_model, *pos),
                // Ticket 228 — populated by the replan-time anchor
                // resolution above. Hunt's route-cost axis samples
                // through `nearest_prey`; Wander's through
                // `wander_target`.
                nearest_prey: cat_nearest_prey,
                wander_target: cat_wander_target,
            },
            route_cost_field: Some(&cat_route_cost_field),
            disposition_failure_signal_hunting:
                crate::systems::plan_substrate::disposition_cooldown_signal(
                    context_beliefs,
                    crate::components::disposition::DispositionKind::Hunting,
                ),
            disposition_failure_signal_foraging:
                crate::systems::plan_substrate::disposition_cooldown_signal(
                    context_beliefs,
                    crate::components::disposition::DispositionKind::Foraging,
                ),
            // 155: `Crafting` retired into Herbalism / Witchcraft /
            // Cooking. The per-disposition recent-failure signal field
            // keeps its existing name for backwards compatibility with
            // saved soaks and tuning-constant consumers; it now reads
            // the Herbalism failure rate (the bulk of pre-155 Crafting
            // plan failures came from herbcraft sub-modes per ticket
            // 152's audit). Witchcraft / Cooking failure tracking is
            // follow-on work — see ticket 155 § "Out of scope".
            disposition_failure_signal_crafting:
                crate::systems::plan_substrate::disposition_cooldown_signal(
                    context_beliefs,
                    crate::components::disposition::DispositionKind::Herbalism,
                ),
            disposition_failure_signal_caretaking:
                crate::systems::plan_substrate::disposition_cooldown_signal(
                    context_beliefs,
                    crate::components::disposition::DispositionKind::Caretaking,
                ),
            disposition_failure_signal_building:
                crate::systems::plan_substrate::disposition_cooldown_signal(
                    context_beliefs,
                    crate::components::disposition::DispositionKind::Building,
                ),
            disposition_failure_signal_mating:
                crate::systems::plan_substrate::disposition_cooldown_signal(
                    context_beliefs,
                    crate::components::disposition::DispositionKind::Mating,
                ),
            disposition_failure_signal_mentoring:
                crate::systems::plan_substrate::disposition_cooldown_signal(
                    context_beliefs,
                    crate::components::disposition::DispositionKind::Mentoring,
                ),
            memory_resource_found_proximity_sum: memory_sums.0,
            memory_death_proximity_sum: memory_sums.1,
            memory_threat_seen_proximity_sum: memory_sums.2,
            colony_knowledge_resource_proximity: colony_knowledge_sums.0,
            colony_knowledge_threat_proximity: colony_knowledge_sums.1,
            colony_priority_ordinal: crate::ai::scoring::colony_priority_ordinal(
                colony.priority.as_ref().and_then(|cp| cp.active),
            ),
            cascade_counts,
            aspiration_action_counts,
            preference_signals,
            fated_love_visible: if love_visible { 1.0 } else { 0.0 },
            fated_rival_nearby: if rival_nearby { 1.0 } else { 0.0 },
            active_directive_action_ordinal,
            active_directive_bonus,
            // Ticket 246 — IntentionMomentum scalars wired from
            // `Option<&HeldIntention>`. The modifier short-circuits on
            // `lift_factor <= 0.0` so the `None` arm (zeroes) is the
            // dormant default; the `Some` arm populates from
            // `commitment_strength × intention_momentum_lift × decay_factor`
            // with the held action's ordinal and the source provenance
            // (130's hook).
            intention_held_action_ordinal: world_state
                .held_intentions
                .get(entity)
                .ok()
                .map(|h| (h.held_action as usize as f32) + 1.0)
                .unwrap_or(0.0),
            intention_momentum_lift_factor: world_state
                .held_intentions
                .get(entity)
                .ok()
                .map(|h| {
                    h.commitment_strength
                        * d.intention_momentum_lift
                        * h.decay_factor(res.time.tick, d.intention_momentum_decay_ticks)
                })
                .unwrap_or(0.0),
            intention_source_ordinal: world_state
                .held_intentions
                .get(entity)
                .ok()
                .map(|h| h.source.ordinal() as f32)
                .unwrap_or(0.0),
            // 263: borrow the per-tick ActionAffordances resource so
            // entity-pair affordance reads inside `ctx_scalars` and
            // future consideration closures route through one source
            // of truth. `affordance_writer` rebuilds the resource each
            // tick before scoring runs. Sourced from `ColonyContext`
            // so the disposition-pipeline mirror site can read the
            // same handle without separately bundling it.
            action_affordances: &colony.action_affordances,
        };

        let focal_cat = res.focal_target.as_deref().and_then(|t| t.entity);
        let focal_capture = res.focal_capture.as_deref();
        let eval_inputs = crate::ai::scoring::EvalInputs {
            cat: entity,
            position: *pos,
            tick: res.time.tick,
            dse_registry: &res.dse_registry,
            modifier_pipeline: &res.modifier_pipeline,
            markers: &markers,
            colony_landmarks: &colony.colony_landmarks,
            exploration_map: &colony.exploration_map,
            corruption_landmarks: &colony.corruption_landmarks,
            focal_cat,
            focal_capture,
        };
        let result = score_actions(&ctx, &eval_inputs, &mut rng.rng);
        // Record latent Cook desire so the coordinator's BuildPressure
        // channel for Kitchen rises when enough cats want to cook but
        // no Kitchen exists.
        if result.wants_cook_but_no_kitchen {
            unmet_demand.record(crate::components::building::StructureType::Kitchen);
        }
        let scores = result.scores;

        // Snapshot for the L2-vs-pool invariant in tests/scenarios.rs:
        // nothing should mutate the score Vec between this point and the
        // softmax. Focal cat only — non-focal cats skip the trace path.
        let pre_bonus_pool_snapshot = if focal_cat == Some(entity) && focal_capture.is_some() {
            Some(scores.clone())
        } else {
            None
        };

        // 158: the side-channel `self_groom_won` resolver retired.
        // `Action::Groom` split into sibling `Action::GroomSelf` /
        // `Action::GroomOther`, each scored independently in
        // `score_actions` and routed via `from_action` (Resting /
        // Grooming respectively). The resolver's parallel scoring
        // formula is no longer needed because the L3 softmax pick
        // directly carries the self-vs-other distinction.

        // §L2.10.6 softmax-over-Intentions: softmax the flat action pool
        // directly, then map the winning Intention to its disposition. The
        // helper preserves the legacy disposition-level independence penalty
        // by applying it as an action-level transform on Coordinate /
        // Socialize / Mentor before softmax.
        //
        // §11.3 L3 capture — when the focal cat is selecting, surface
        // the pool + probabilities + RNG roll to `FocalScoreCapture` so
        // `emit_focal_trace` can reconstruct the full selection record.
        let capture_this_cat = focal_capture.is_some() && focal_cat == Some(entity);
        let mut softmax_trace = capture_this_cat.then(crate::ai::scoring::SoftmaxCapture::default);
        // Ticket 232 — body-state-coupled L3 softmax temperature.
        // Computed per tick from the cat's `ScoringContext` so the L3
        // draw sharpens to the floor when body distress or rising-
        // threat is high, and broadens to the ceiling when the cat is
        // calm.
        let softmax_temperature = crate::ai::scoring::softmax_temperature(&ctx, sc);
        let softmax_outcome =
            crate::ai::scoring::select_disposition_via_intention_softmax_with_trace(
                &scores,
                personality.independence,
                d.disposition_independence_penalty,
                softmax_temperature,
                &mut rng.rng,
                softmax_trace.as_mut(),
            );
        let chosen = softmax_outcome.chosen;
        if let (Some(capture), Some(mut trace)) = (focal_capture, softmax_trace) {
            if let Some(snap) = pre_bonus_pool_snapshot {
                trace.pre_bonus_pool = snap;
            }
            capture.set_softmax(trace, res.time.tick);
        }

        // Store all gate-open action scores, sorted descending, for
        // diagnostics. Truncation removed 2026-04-20 so scoring-competition
        // analysis can see ranks beyond the top few (e.g., Mate vs Socialize
        // on shared ticks).
        //
        // Ticket 427 Step 7 — `clone_from` reuses the existing
        // `current.last_scores` allocation across cat-ticks instead of
        // discarding+allocating per call. Hot path: every cat every tick.
        current.last_scores.clone_from(&scores);
        current
            .last_scores
            .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // 155: post-softmax `CraftingHint` recovery retired. The L3
        // softmax's chosen Action is now itself the sub-mode picker —
        // `Action::HerbcraftGather` / `HerbcraftRemedy` / `HerbcraftSetWard`
        // fan into Herbalism; the six `MagicX` variants fan into
        // Witchcraft; `Action::Cook` is its own Disposition. Directive
        // routing is handled at `DirectiveKind::to_action` (Cleanse →
        // MagicColonyCleanse, HarvestCarcass → MagicHarvest, Cook → Cook),
        // which in turn maps to the parent Disposition via `from_action`.
        //
        // The chosen sub-action is the highest-scoring Action in the
        // softmax pool whose parent Disposition matches `chosen`. For
        // single-constituent Dispositions this trivially picks the only
        // constituent. For Herbalism / Witchcraft / Cooking it picks
        // the L3-winning sub-action.
        let mut chosen = chosen;
        let mut chosen_action = scores
            .iter()
            .filter(|(a, _)| DispositionKind::from_action(*a) == Some(chosen))
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(a, _)| *a)
            .or_else(|| chosen.constituent_actions().first().copied())
            .unwrap_or(Action::Idle);

        // 364 D1 — HTN frame-pin adopt hook. If the cat carries a
        // non-empty `HeldGoalStack` whose top frame's current sub-goal is
        // a `Primitive { action, .. }`, override (chosen, chosen_action)
        // to the leaf primitive. The stack was authored either by 320's
        // initial author (Intention::Goal at the L2 author site) or by
        // commit (b)'s advance hook on a prior tick. The plan-template
        // branch below (`actions_for_disposition` vs
        // `htn_primitive_actions`) routes through the HTN builder when
        // this fires.
        // Gate the adopt hook to MULTI-STEP methods only
        // (`sub_goal_count > 1`). Single-step methods (hunt_method,
        // forage_method, etc.) don't need frame-pinning — their leaf
        // action is what the L3 softmax already picks via the parent
        // disposition. Pinning Hunt here would route the plan template
        // through `htn_primitive_actions`, which only supports the 6
        // HTN-leaf actions (kitten + mourn arcs) and panics on Hunt /
        // Forage / Patrol / etc. Multi-step methods (rear_kitten,
        // mourn_at_grave) are the structural case where frame-pin is
        // load-bearing: the cat must walk through sub-goals across
        // ticks, and the softmax can't independently re-derive Wean →
        // Teach → Release sequencing.
        // Ticket 397 Layer 3 — narrow §L2.10.6 precedent: when the L2/L3
        // softmax just picked Caretake, do not let the frame-pin discard
        // that selection. Caretake's score is high precisely when a
        // dependent kitten is in acute need (kitten_urgency axis, weight
        // 0.45); preempting it with a maturity-bump primitive
        // (Wean/Teach/Release — none of which feed) strands the kitten.
        // The held rear_kitten frame stays on the stack as durable
        // commitment per §8.4 ("never exclude the incumbent"); next tick
        // softmax samples again, and if Caretake's score drops (kitten
        // sated after feeding), the pin resumes walking sub-goals. This
        // is the narrow precedent for the full §L2.10.6 softmax-over-
        // Intentions wrap-site rework (deferred to the 060 epic) — the
        // architectural answer is "Caretake-as-Intention and rear_kitten-
        // as-Intention compete in one pool with persistence-bonus"; the
        // narrow guard here approximates that outcome for the load-
        // bearing acute-Caretake case without rewriting the full pool
        // machinery. With this guard, 395's reactive-emit yield rule
        // (`has_dependent_kitten`'s `!IsParentOfHungryKitten` clause)
        // becomes structurally unnecessary and retires in the same land.
        //
        // The guard zeros `frame_pinned_primitive` itself (not just the
        // chosen_action override) so the downstream plan-template branch
        // at line ~2549 routes Caretake through `actions_for_disposition`
        // (its real plan template, [TravelTo(SocialTarget), Caretake])
        // instead of `htn_primitive_actions` (which panics on Caretake —
        // that function only handles Wean/Teach/Release/Vigil/GriefSit
        // /ReleaseGrief).
        // 334: the pinned primitive carries an optional `ItemKind` payload
        // (from `TargetHint::CraftItem`) so the Craft leaf can route the
        // plan template through `craft_have_item_actions(item, …)` (the 463
        // HaveItem craft path) — `htn_primitive_actions` only emits a single
        // leaf and can't express the retrieve+travel+craft triple.
        let softmax_winner_preempts_pin = chosen_action == Action::Caretake;
        let frame_pinned_primitive: Option<(Action, Option<ItemKind>)> =
            if softmax_winner_preempts_pin {
                None
            } else {
                world_state
                    .held_goal_stacks
                    .get(entity)
                    .ok()
                    .and_then(|stack| stack.top())
                    .filter(|frame| frame.sub_goal_count > 1)
                    .and_then(|frame| {
                        let method = res.method_registry.lookup_by_id(frame.method)?;
                        match method.sub_goals.get(frame.sub_goal_index)? {
                            crate::ai::methods::SubGoal::Primitive {
                                action,
                                target_hint,
                                ..
                            } => {
                                let craft_item = match target_hint {
                                    crate::ai::methods::TargetHint::CraftItem(item) => Some(*item),
                                    _ => None,
                                };
                                Some((*action, craft_item))
                            }
                            crate::ai::methods::SubGoal::Goal(_) => None,
                        }
                    })
            };
        if let Some((leaf_action, _)) = frame_pinned_primitive {
            chosen_action = leaf_action;
            if let Some(disp) = DispositionKind::from_action(leaf_action) {
                chosen = disp;
            }
        }

        // Build planner state and zone distances.
        let construction_pos: Vec<(Entity, Position)> = world_state
            .building_query
            .iter()
            .filter(|(_, _, _, site, _)| site.is_some())
            .map(|(e, _, p, _, _)| (e, *p))
            .collect();
        let farm_pos: Vec<Position> = world_state
            .building_query
            .iter()
            .filter(|(_, s, _, site, _)| s.kind == StructureType::Garden && site.is_none())
            .map(|(_, _, p, _, _)| *p)
            .collect();
        let material_pile_positions: Vec<(Entity, Position, ItemKind)> = world_state
            .material_items_query
            .iter()
            .filter(|(_, item, _)| {
                matches!(
                    item.location,
                    crate::components::items::ItemLocation::OnGround
                ) && item.kind.material().is_some()
            })
            .map(|(e, item, p)| (e, *p, item.kind))
            .collect();
        // Ticket 193: snapshot OnGround food-Item entities for the
        // `CarcassPile` zone resolver. Engage_prey overflow drops
        // these at the kill tile when inventory is full; PickingUp's
        // plan template routes through this zone to retrieve them.
        let food_pile_positions: Vec<(Entity, Position, ItemKind)> = world_state
            .food_items_query
            .iter()
            .filter(|(_, item, _)| {
                matches!(
                    item.location,
                    crate::components::items::ItemLocation::OnGround
                ) && item.kind.is_food()
            })
            .map(|(e, item, p)| (e, *p, item.kind))
            .collect();
        let construction_materials_complete: HashMap<Entity, bool> = world_state
            .building_query
            .iter()
            .filter_map(|(e, _, _, site, _)| site.map(|s| (e, s.materials_complete())))
            .collect();
        // Ticket 096: author the per-cat `MaterialsAvailable` substrate
        // marker against this cat's nearest reachable site. The
        // planner consults it via `HasMarker(MaterialsAvailable::KEY)`
        // on the substrate-branch of `Construct`.
        markers.set_entity(
            markers::MaterialsAvailable::KEY,
            entity,
            materials_available_for(pos, &construction_pos, &construction_materials_complete),
        );
        // Ticket 231: author the per-cat `HasFreeSlot` substrate marker
        // from this cat's `Inventory`. Read by the substrate-path
        // variant of the four pickup-class plan actions; the plan-path
        // variant composes via DropItem-as-prefix
        // (`HasFreeSlotThisPlan(true)`, search-state).
        markers.set_entity(markers::HasFreeSlot::KEY, entity, !inventory.is_full());
        // Ticket 235 / shuffle-fix: per-cat stores-reachability marker
        // authoring moved up to the pre-`score_actions` block (see the
        // sibling site near `MarkerSnapshot::set_entity(Incapacitated…)`).
        // Keeping these here would set the marker AFTER score_actions
        // has already consulted eligibility — fine for `HasHerbStash-
        // Accessible` (consumed by the planner, runs later) but breaks
        // `HasFoodStorageAccessible` (consumed by PickingUp eligibility,
        // checked inside `score_actions`).
        let planner_state = build_planner_state(
            pos,
            needs,
            inventory,
            0,
            &res.map,
            &stores_positions,
            &construction_pos,
            &farm_pos,
            &herb_positions,
            &material_pile_positions,
            &food_pile_positions,
            d,
        );
        let zone_distances = build_zone_distances(
            pos,
            &res.map,
            &stores_positions,
            &construction_pos,
            &farm_pos,
            &herb_positions,
            &kitchen_positions,
            &cat_positions,
            &material_pile_positions,
            &food_pile_positions,
            &drying_rack_positions,
            &smoking_rack_positions,
            &workshop_positions,
            &tanning_frame_positions,
            &dead_cat_positions,
            entity,
            d,
        );
        // 364 D1 — when frame-pinned, route plan-template through
        // htn_primitive_actions (travel + single Pattern-B step keyed to
        // the primitive's GoapActionKind) instead of the disposition's
        // full action catalog. The dispatch arm at
        // `evaluate_step_for_action` then resolves the target + runs the
        // resolver.
        // 463 — HaveItem aspiration override. When the cat holds
        // `Intention::Goal(GoalKind::HaveItem(item))` AND just elected
        // `Crafting`, swap the disposition's single-step crafting
        // template for the dual-arm `craft_have_item_actions` template
        // that prefixes `RetrieveCraftInputs(recipe.id)` so the cat
        // walks to Stores, pulls the recipe's specific inputs, and
        // crafts the aspired item (not the lex-first satisfied recipe
        // the legacy `pick_satisfied_recipe` would pick). Dormant
        // until 463's `CraftItemAspiration` emits HaveItem rows; the
        // override is a no-op for every existing cat-tick.
        // 463 commit 8: scan ALL aspiration emission rows for a
        // HaveItem `goal_kind`, not just `HeldIntention`. The
        // HeldIntention reads only the AspirationEmissions::winner()
        // row (lowest Priority), so the picker's `Priority::Secondary`
        // CraftItem row gets overwritten by any Primary aspiration
        // (Hunting "First Blood", Warrior's Path "engage_threat") and
        // never reaches the held intention. Reading the rows directly
        // lets the HaveItem dispatch fire whenever the picker emitted a
        // CraftItem row this tick, even if Hunting also emitted.
        let have_item_target: Option<crate::components::items::ItemKind> = marker_qs
            .aspiration_emissions_q
            .get(entity)
            .ok()
            .and_then(|emissions| {
                emissions.rows.iter().find_map(|row| match row.goal_kind {
                    Some(crate::ai::dse::GoalKind::HaveItem(item)) => Some(item),
                    _ => None,
                })
            });
        let mut actions = if let Some((Action::Craft, Some(item))) = frame_pinned_primitive {
            // 334: the pinned Craft leaf of `acquire_stealth_via_self_craft`
            // reuses the 463 HaveItem craft template (retrieve → travel →
            // craft) with the cloak's recipe pinned by `TargetHint::CraftItem`.
            crate::ai::planner::actions::craft_have_item_actions(
                item,
                &res.recipes,
                &zone_distances,
            )
        } else if frame_pinned_primitive.is_some() {
            crate::ai::planner::actions::htn_primitive_actions(chosen_action, &zone_distances)
        } else if let (Some(item), DispositionKind::Crafting) = (have_item_target, chosen) {
            crate::ai::planner::actions::craft_have_item_actions(
                item,
                &res.recipes,
                &zone_distances,
            )
        } else {
            actions_for_disposition(chosen, chosen_action, &zone_distances)
        };
        // Posse override: when a Fight directive is active on the cat and
        // they've landed in Guarding disposition, replace the generic
        // action list (which A* solves with cheapest = Survey) with a
        // single EngageThreat step. The posse mechanic depends on cats
        // converging on and engaging the target shadow-fox rather than
        // wandering their patrol zone.
        let fight_directive_target = if chosen == DispositionKind::Guarding {
            if let Ok(directive) = world_state.active_directive_query.get(entity) {
                if directive.kind == DirectiveKind::Fight {
                    actions = vec![crate::ai::planner::GoapActionDef {
                        kind: GoapActionKind::EngageThreat,
                        cost: 1,
                        preconditions: vec![],
                        effects: vec![crate::ai::planner::StateEffect::IncrementTrips],
                    }];
                    directive
                        .target_position
                        .map(|tp| (tp, directive.target_entity))
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        // 092 substrate handoff: the planner reads the same
        // `MarkerSnapshot` the IAUS scoring layer just built (line 874+),
        // so `HasMarker(...)` predicates on `EatAtStores`, `SetWard`, and
        // the Resting partial-goal branch all consult one source of truth.
        let plan_ctx = crate::ai::planner::PlanContext {
            markers: &markers,
            entity,
        };
        // Read for the PlanningFailed event below — markers are the
        // authoritative source of `HasStoredFood` (093 substrate doctrine).
        let planner_has_stored_food = markers.has(markers::HasStoredFood::KEY, entity);
        // 364 D1 — when frame-pinned, override the goal to
        // `InteractionDone(true)` (matches the HTN leaf's
        // `SetInteractionDone(true)` effect). The chosen disposition's
        // own goal (e.g., `Caretaking::TripsAtLeast`) wouldn't be
        // satisfied by the leaf's effects, so the plan would fail.
        // 334 — the pinned Craft leaf routes through `craft_have_item_actions`
        // whose terminal `CraftAtWorkshop` step completes via `IncrementTrips`,
        // not `SetInteractionDone`. Use the Crafting trips goal
        // (`TripsAtLeast(1)`) for that pin so the plan is satisfiable.
        let goal = match frame_pinned_primitive {
            Some((Action::Craft, Some(_))) => {
                goal_for_disposition(DispositionKind::Crafting, 0, &plan_ctx)
            }
            Some(_) => crate::ai::planner::GoalState {
                predicates: vec![crate::ai::planner::StatePredicate::InteractionDone(true)],
            },
            None => goal_for_disposition(chosen, 0, &plan_ctx),
        };

        let plan_outcome = make_plan(
            planner_state,
            &actions,
            &goal,
            12,
            1000,
            &plan_ctx,
            &mut res.planner_scratch,
        );
        if let Ok(steps) = plan_outcome {
            // 150 R5a: an empty plan means the goal is *already*
            // satisfied at planning time (e.g., the cat picked Eating
            // at hunger=0.84 — already above the resting-complete
            // threshold, so HungerOk(true) is met before any action
            // runs). Don't commit to a 0-step plan: it would exhaust
            // immediately, increment trips_done, fire
            // `disposition_complete`, and re-elect — burning a tick
            // per cycle and starving longer-form activities (Mentor,
            // Mate, Burial) of the contiguous tick budget they need.
            // Pre-150 this branch was unreachable for tier-1 dispositions
            // because Resting's three-need goal was almost never met at
            // planning time; with Eating's single-axis goal it became
            // reachable. Fall through silently — no PlanningFailed,
            // no cooldown — and let the cat re-elect on the next tick
            // when scoring runs again.
            if steps.is_empty() {
                continue;
            }
            let mut plan = GoapPlan::new(chosen, chosen_action, res.time.tick, personality, steps);
            // 150 R5a: Eating shares Resting's runaway-replan cap. Both
            // are physiological-completion dispositions; reusing the
            // existing `resting_max_replans` keeps the comparability
            // invariant from breaking on the events.jsonl header.
            if matches!(chosen, DispositionKind::Resting | DispositionKind::Eating) {
                plan.max_replans = d.resting_max_replans;
            }
            // 155: ward placement position flows from a directive when
            // the cat picked the Herbalism SetWard sub-action.
            if chosen_action == Action::HerbcraftSetWard {
                if let Ok(directive) = world_state.active_directive_query.get(entity) {
                    if directive.kind == DirectiveKind::SetWard {
                        plan.ward_placement_pos = directive.target_position;
                    }
                }
            }
            // Flow posse target (Fight directive) into the first step's
            // target_entity so EngageThreat doesn't re-pick by nearest.
            if let Some((_target_pos, Some(target_entity))) = fight_directive_target {
                if let Some(slot) = plan.step_state.first_mut() {
                    slot.target_entity = Some(target_entity);
                }
            }
            // §Phase 4c.4: persist the Caretake target kitten through
            // the plan. The §6.5.6 target-taking DSE uses
            // `CARETAKE_TARGET_RANGE = 12` from the adult's position —
            // by the time FeedKitten executes, the adult has walked to
            // Stores and the kitten is typically out of range, so
            // re-running the resolver at step-time returns `target=None`
            // and the feeding silently no-ops (StepResult::Advance,
            // fed=None, no KittenFed activation). Seeding the FeedKitten
            // step's target_entity now locks the kitten chosen at
            // disposition-selection time, mirroring how socialize_target /
            // mate_target flow their resolver output into the executor
            // rather than asking the executor to re-resolve from a stale
            // position.
            if chosen == DispositionKind::Caretaking {
                if let Some(kitten) = caretake_resolution.target {
                    if let Some(feed_idx) = plan
                        .steps
                        .iter()
                        .position(|s| s.action == GoapActionKind::FeedKitten)
                    {
                        plan.step_state[feed_idx].target_entity = Some(kitten);
                    }
                }
            }

            if let Some(ref mut log) = event_log {
                log.push(
                    res.time.tick,
                    EventKind::PlanCreated {
                        cat: name.0.clone(),
                        disposition: format!("{:?}", chosen),
                        steps: plan
                            .steps
                            .iter()
                            .map(|s| format!("{:?}", s.action))
                            .collect(),
                        hunger: needs.hunger,
                        energy: needs.energy,
                        temperature: needs.temperature,
                        food_available,
                    },
                );
            }

            plan_writer.write(PlanNarrative {
                entity,
                kind: chosen,
                event: PlanEvent::Adopted,
                completions: 0,
            });

            // Ticket 126 — author the actor-private HeldIntention
            // alongside the GoapPlan. Strength is the
            // softmax-temperature-normalised margin between the
            // chosen Action and the score-space runner-up. The
            // IntentionMomentum modifier reads commitment_strength ×
            // intention_momentum_lift × decay_factor through the
            // scalar surface; ScoringContext construction in the next
            // tick will populate those scalars from this Component.
            //
            // Ticket 321 — when the L1→L2 picker
            // (`crate::systems::aspiration_picker`) authored an
            // `AspirationEmissions` Component for this cat this
            // tick, the highest-`Priority` emission row replaces the
            // default `Intention::Activity { Idle }` wrap with
            // `Intention::Goal { state, strategy }`. 320's HTN
            // frame-push gate immediately below catches the Goal
            // shape and walks `MethodRegistry`. The picker removes
            // the Component entirely when no emission applies, so
            // `get(entity)` returns `Err(_)` and the wrap defaults
            // to the 126 Activity-Idle shape (byte-identical to
            // pre-321 behavior).
            //
            // Per `docs/systems/ai-substrate-refactor.md` §L2.10.6
            // the full mechanism is softmax-over-Intentions across
            // the combined `{DSE-Activity-default} ∪ {emitted-Goals}`
            // pool; that formal resolution lives in §8.2 and is a
            // follow-on ticket. 321 ships the producer interface +
            // a priority-based override at the wrap site — at land
            // the emissions pool is degenerate (Hunting "First Blood"
            // is the only Live emission slice), so the priority
            // override and the formal softmax converge.
            let strategy = crate::ai::commitment::strategy_for_disposition(chosen);
            let (held_intention, intention_source) = match marker_qs
                .aspiration_emissions_q
                .get(entity)
                .ok()
                .and_then(|e| e.winner().cloned())
            {
                Some(row) => {
                    // Ticket 463 — exhaustive match on the row's typed
                    // `goal_kind`. `Some(kind)` carries a runtime-shaped
                    // goal (e.g. `CraftItemAspiration`'s per-cat-per-tick
                    // winning recipe as `GoalKind::HaveItem(item)`).
                    // `None` preserves the pre-463 path: every static
                    // aspiration emits a label-only row whose achievement
                    // fires via 320's frame-pop, not via this predicate.
                    let state = match row.goal_kind {
                        Some(kind) => crate::ai::dse::GoalState { kind },
                        None => {
                            crate::ai::dse::GoalState::predicate(row.label, |_world, _entity| false)
                        }
                    };
                    let goal = crate::ai::dse::Intention::Goal {
                        state,
                        strategy: row.strategy,
                    };
                    let source =
                        crate::components::IntentionSource::AspirationEmitted { chain: row.chain };
                    (goal, source)
                }
                None => {
                    let activity = crate::ai::dse::Intention::Activity {
                        // 126: placeholder Intention shape — the
                        // held DSE's identity rides on
                        // `held_action` for the modifier's
                        // round-trip, and `source` records
                        // provenance. Future tickets (128 HTN,
                        // 127 joint-intentions) will refine the
                        // Intention contents from each DSE's
                        // emit().
                        kind: crate::ai::dse::ActivityKind::Idle,
                        termination: crate::ai::dse::Termination::UntilInterrupt,
                        strategy,
                    };
                    (activity, crate::components::IntentionSource::SelfMotivated)
                }
            };
            let commitment_strength =
                crate::components::held_intention::commitment_strength_from_margin(
                    softmax_outcome.margin(),
                    softmax_temperature,
                );
            // Ticket 400 — Caretake target plumbing. Caretake's `emit`
            // returns an `Intention::Goal { state: "kitten_fed", .. }`
            // with no embedded target; the resolved kitten lives on
            // `caretake_resolution.target`. The L2 ParentingActivity
            // suppression mechanic (`populate_parenting_scalars`) needs
            // `HeldIntention.target` to know which kitten the partner
            // is caretaking — otherwise a partner caretaking kitten A
            // would over-suppress my Caretake bias for kitten B (e.g.
            // multi-litter colonies). Threading the resolved target in
            // here keeps the suppression target-specific.
            let held_target = if chosen_action == crate::ai::Action::Caretake {
                caretake_resolution.target
            } else {
                None
            };
            let held = crate::components::HeldIntention::new(
                held_intention,
                chosen_action,
                held_target,
                res.time.tick,
                commitment_strength,
                None,
                intention_source,
            );

            // Ticket 320 — HTN method-stack authorship. If the held
            // intention is `Intention::Goal { state, .. }` AND the
            // registry has a non-dormant method for `state.label`,
            // push a `GoalFrame` per visited method (recursing into
            // compound sub-goals) and insert a `HeldGoalStack`
            // alongside the `HeldIntention`. Cap recursion at
            // `MAX_GOAL_STACK_DEPTH`; emit `Feature::MethodAdopted`
            // per frame and `Feature::MethodDepthExceeded` on cap.
            //
            // At 320's land this branch is reachable but never taken
            // in production: the registry contains no `Live` methods
            // (only `PendingSubstrate` entries that the dormant
            // filter rejects), and the L2 author above wraps the
            // chosen action in `Intention::Activity { Idle, .. }` —
            // never `Goal`. 321 (picker emits `Goal`-shaped
            // intentions) and 323 (first Live method) are the
            // tickets that exercise this path.
            if let crate::ai::dse::Intention::Goal { state, .. } = &held.intention {
                // 364 — preserve an existing HeldGoalStack when the held
                // intention's label matches the current top frame's
                // goal_label. The advance hook (resolve_goap_plans) owns
                // sub_goal_index updates across ticks; rebuilding the
                // stack from scratch every tick would reset the cursor
                // to 0 and trap the arc on its first sub-goal. The
                // rebuild path still fires when the cat starts a NEW
                // Goal (different label or no existing stack).
                let preserve_existing = world_state
                    .held_goal_stacks
                    .get(entity)
                    .ok()
                    .and_then(|s| s.top())
                    .map(|frame| frame.goal_label == state.label())
                    .unwrap_or(false);
                if preserve_existing {
                    // Keep the advance hook's stack as-is.
                } else {
                    let mut stack = crate::components::HeldGoalStack::empty();
                    let mut next_label: Option<&'static str> = Some(state.label());
                    let mut depth_exceeded = false;
                    while let Some(label) = next_label {
                        let Some(spec) = res.method_registry.lookup_spec_dormant_filtered(label)
                        else {
                            // No method for this label; the gate stops
                            // expanding. If at least one frame was
                            // already pushed (a parent method whose
                            // compound sub-goal had no method), the
                            // stack carries the partial decomposition;
                            // the leaf is held via `HeldIntention` per
                            // the 126 no-method fallback.
                            break;
                        };
                        let frame = crate::components::GoalFrame::new(
                            spec.id,
                            spec.goal_label,
                            spec.sub_goals.len(),
                            res.time.tick,
                            None,
                            held.source.clone(),
                        );
                        if stack.push(frame).is_err() {
                            depth_exceeded = true;
                            break;
                        }
                        if let Some(activation) = res.activation.as_deref_mut() {
                            activation.record(Feature::MethodAdopted);
                        }
                        // Step into sub_goals[0]. Compound entries
                        // recurse via the registry; primitive entries
                        // terminate the walk — the primitive is held via
                        // `HeldIntention`.
                        next_label = match spec.sub_goals.first() {
                            Some(crate::ai::methods::SubGoal::Goal(g)) => Some(g.label()),
                            _ => None,
                        };
                    }
                    if depth_exceeded {
                        if let Some(activation) = res.activation.as_deref_mut() {
                            activation.record(Feature::MethodDepthExceeded);
                        }
                    }
                    if !stack.is_empty() {
                        commands.entity(entity).insert(stack);
                    }
                }
            }

            commands.entity(entity).insert(held);
            // Ticket 248 — surgically apply the IntentionMomentum
            // modifier's lift to `current.last_scores[chosen_action]`.
            // `score_actions` at the L2 author site (above) ran when
            // the cat was `Without<HeldIntention>`, so the modifier's
            // `lift_factor` scalar was 0.0 and the recorded held_score
            // is un-lifted. The in-line trigger-3 preempt block
            // (below; reads `current.last_scores[held_action]` on
            // subsequent ticks) compares against
            // `held_score + intention_preempt_margin`; without this
            // write the threshold understates the held DSE's honest
            // score by exactly the modifier's lift. (Trigger-3 still
            // gates on the strength regime boundary to handle a
            // separate softmax-low-margin oscillation; see the gate's
            // doc-comment.) On the adoption tick `decay_factor == 1.0`,
            // so the lift reduces to
            // `commitment_strength × intention_momentum_lift`
            // (mirroring `IntentionMomentum::apply` in
            // `src/ai/modifier.rs:902-933`). Bevy's deferred command
            // buffer means we can't re-run `score_actions` here to let
            // the modifier produce the lift naturally — the inserted
            // `HeldIntention` isn't visible to a query until the next
            // flush — so the lift is written directly.
            let intention_momentum_lift = commitment_strength * d.intention_momentum_lift;
            if intention_momentum_lift > 0.0 {
                if let Some(entry) = current
                    .last_scores
                    .iter_mut()
                    .find(|(a, _)| *a == chosen_action)
                {
                    entry.1 += intention_momentum_lift;
                }
                current
                    .last_scores
                    .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            }
            if let Some(activation) = res.activation.as_deref_mut() {
                activation.record(crate::resources::system_activation::Feature::IntentionAdopted);
            }

            current.ticks_remaining = u64::MAX;
            commands.entity(entity).insert(plan);
        } else {
            // 172: extract the typed reason from the `Err` arm. The
            // `Ok` arm is handled above; this branch is reachable
            // only when `plan_outcome` is `Err`.
            let reason = match plan_outcome {
                Err(r) => r,
                Ok(_) => unreachable!(
                    "Ok arm consumed by the if let above — \
                     reaching this match would mean the plan_outcome \
                     variable was reassigned, which it isn't"
                ),
            };
            // Ticket 123 — author the disposition-failure memory
            // before the event push. The IAUS-side cooldown reads
            // ContextBeliefs[DispositionExecution(chosen)].predictability
            // on the next tick to suppress the same-disposition re-pick
            // (3059 wasted planning rounds in seed-42's 1500-tick
            // cold-start window came from the unbroken retry loop).
            // 290 (Commit B) — sole writer is now the WitnessableEvent
            // emit; belief_integrator's SelfPlanFailed handler updates
            // the predictability facet. RDF + its lazy-insert path
            // retired.
            res.witnessable.write(
                crate::messages::witnessable_event::WitnessableEvent::SelfPlanFailed {
                    cat: entity,
                    disposition: chosen,
                    position: *pos,
                    tick: res.time.tick,
                },
            );
            if let Some(ref mut log) = event_log {
                // Ticket 091: surface the silent `make_plan → None`
                // path. Pre-091 this branch emitted nothing — the
                // cat just idled with `ticks_remaining = 0` and
                // replanned next tick. When IAUS elects a
                // disposition (e.g., Foraging) but the GOAP planner
                // can't satisfy it (e.g., no reachable foraging
                // zone, or `Carrying` vetoes ForageItem), the
                // producer side collapses with no canary-visible
                // signal. The footer field
                // `planning_failures_by_disposition` is the cheap
                // pre-trace disambiguator for that pattern.
                //
                // 172: `reason` carries the typed
                // `PlanningFailureReason` so the footer's new
                // `planning_failures_by_reason` map can attribute
                // failures by cause (NoApplicableActions /
                // GoalUnreachable / NodeBudgetExhausted) — the cull
                // that distinguishes substrate-eligibility issues
                // from search-budget issues from action-effect
                // issues.
                log.push(
                    res.time.tick,
                    EventKind::PlanningFailed {
                        cat: name.0.clone(),
                        disposition: format!("{:?}", chosen),
                        reason,
                        hunger: needs.hunger,
                        energy: needs.energy,
                        temperature: needs.temperature,
                        food_available,
                        has_stored_food: planner_has_stored_food,
                    },
                );
            }
        }
    }
}

// ===========================================================================
// resolve_goap_plans — executor dispatching to step resolver helpers
// ===========================================================================

struct MentorEffect {
    apprentice: Entity,
    mentor_skills: Skills,
}

/// Immutable pre-loop snapshots consumed by `dispatch_step_action` and the
/// prologue/epilogue replanning paths. All data is owned — no lifetimes.
struct StepSnapshots {
    grooming: HashMap<Entity, f32>,
    gender: HashMap<Entity, Gender>,
    stores_positions: Vec<Position>,
    stores_entities: Vec<(Entity, Position)>,
    /// Ticket 177: per-tick snapshot of completed Midden buildings,
    /// used by the `TrashItemAtMidden` dispatch arm to resolve the
    /// target entity's `Position` without widening `stores_query`
    /// (which would conflict with `BuildingResolverParams::buildings`
    /// on the `Structure`/`Position` archetype).
    midden_entities: Vec<(Entity, Position)>,
    kitchen_positions: Vec<Position>,
    construction_positions: Vec<(Entity, Position)>,
    farm_positions: Vec<Position>,
    herb_positions: Vec<(Entity, Position, HerbKind)>,
    /// Ground items whose `kind.material()` is `Some(_)`. Authored each
    /// tick from a `Without<GoapPlan>` items query so the planner's
    /// `PlannerZone::MaterialPile` resolves to the nearest haulable pile.
    material_pile_positions: Vec<(Entity, Position, ItemKind)>,
    /// Ticket 193: ground items whose `kind.is_food()` and whose
    /// `location` is `OnGround` — engage_prey overflow Items today,
    /// future carcass-as-container child Items tomorrow. Authored once
    /// per tick alongside `material_pile_positions`. Consumers:
    /// `PlannerZone::CarcassPile` zone resolution (planning + replans)
    /// and the `PickUpItemFromGround` dispatch arm's `target_entity`
    /// fallback.
    food_pile_positions: Vec<(Entity, Position, ItemKind)>,
    /// Ticket 092: per-tick `MarkerSnapshot` for the colony markers the
    /// planner consults via `StatePredicate::HasMarker(...)` —
    /// `HasStoredFood`, `ThornbriarAvailable`. Authored once per tick
    /// from the same world state the IAUS substrate uses, so L2 (DSE
    /// eligibility) and L3 (planner preconditions) cannot disagree on a
    /// marker-authored fact during this tick's replans. Replaces the
    /// 091-era `has_stored_food: bool` mirror.
    planner_markers: crate::ai::scoring::MarkerSnapshot,
    workshop_bonus: f32,
    season_mod: f32,
    builders_per_site: HashMap<Entity, usize>,
    cat_positions: Vec<(Entity, Position)>,
    /// 487 follow-on — peers mid-`GroomOther` (both actor and that
    /// actor's `target_entity`) at the start of this tick's
    /// step-dispatch pass. Built once from `cats.iter()` so the
    /// `resolve_groom_other_target` call at the GroomOther dispatch
    /// arm can exclude in-flight pile participants symmetrically with
    /// the `HasGroomCandidate` marker author at line 1734 — without
    /// this, the marker gates eligibility on a non-groomed peer
    /// existing, but the resolver could still pick a mid-groom peer
    /// as the new target and extend the chain.
    currently_groomed: std::collections::HashSet<Entity>,
    injured_cat_positions: Vec<(Entity, Position)>,
    cat_skills: HashMap<Entity, Skills>,
    cat_temperature: HashMap<Entity, f32>,
    /// Ticket 452 — per-cat `GroomingCondition.0` snapshot for the
    /// `target_grooming_deficit` axis on `GroomOtherTargetDse`. Built
    /// from the sibling `grooming_q` system param so the outer
    /// mutable `cats` iteration stays disjoint. Cats lacking the
    /// component (save-loaded pre-452) score 0.0 deficit via the
    /// resolver's `.unwrap_or(0.0)` fallthrough.
    cat_grooming: HashMap<Entity, f32>,
    kitten_parents: HashMap<Entity, (Option<Entity>, Option<Entity>)>,
    kitten_snapshot: Vec<crate::ai::caretake_targeting::KittenState>,
    building_snapshot: Vec<(Entity, StructureType, Position, bool, bool)>,
    /// 035: Dead-and-not-Buried colony cat positions. Built once per
    /// `evaluate_and_plan` tick and consumed by both
    /// `resolve_zone_position`'s `CorpseTarget` arm and
    /// `resolve_bury_target`'s candidate scan.
    dead_cat_positions: Vec<(Entity, Position)>,
    /// 035: name lookup for dead cats so `EventKind::BurialFired`'s
    /// `deceased` field is a real name rather than `entity:N`.
    dead_cat_names: HashMap<Entity, String>,
    /// 367: preservation-station positions for zone resolution.
    /// Completed structures only (built sites — construction sites
    /// are excluded). The load + cooldown discrimination happens at
    /// resolver-time; the zone resolver answers "where is the nearest
    /// rack of this kind?".
    drying_rack_positions: Vec<Position>,
    smoking_rack_positions: Vec<Position>,
    /// 457: Workshop positions for zone resolution.
    workshop_positions: Vec<Position>,
    /// 369: TanningFrame positions for zone resolution. Same shape
    /// as `workshop_positions` — built once per tick from the
    /// building snapshot, drives `PlannerZone::TanningFrame` zone
    /// lookups for `CraftAtTanningFrame` plan execution.
    tanning_frame_positions: Vec<Position>,
}

/// Mutable accumulators written by `dispatch_step_action`, consumed by the
/// post-loop cleanup pass in `resolve_goap_plans`.
struct StepAccumulators {
    mentor_effects: Vec<MentorEffect>,
    grooming_restorations: Vec<crate::steps::disposition::GroomOutcome>,
    kitten_feedings: Vec<Entity>,
    /// Ticket 177: actor-and-recipient pairs queued by the `HandoffItem`
    /// dispatch arm. The actor's `&mut Inventory` is borrowed inside the
    /// per-cat loop, so the actual transfer (which needs both inventories)
    /// runs in the post-loop drain via `cats.get_many_mut([actor, recipient])`.
    handoff_pending: Vec<HandoffPending>,
    /// 035: completed burials queued for post-loop drain. Each
    /// `BuryOutcome` triggers (1) `commands.entity(deceased).insert(Buried)`,
    /// (2) `commands.entity(deceased).despawn()`, (3) `commands.spawn((Grave { ... }, Position { ... }))`.
    bury_completions: Vec<crate::steps::disposition::BuryOutcome>,
    /// 364: kitten-arc HTN advances queued by Wean/Teach/Release dispatch
    /// arms. Each entry mutates the kitten's `KittenDependency` in the
    /// post-loop drain (read via `ec.kitten_parentage`, write via
    /// `commands.entity(kitten).insert(updated_dep)` or `.remove::<…>()`).
    /// Mirrors `kitten_feedings` shape — disjoint from the outer `cats`
    /// query so no query-conflict.
    kitten_rearing_advances: Vec<KittenRearingAdvance>,
}

/// 364: one entry per witnessed HTN leaf primitive in the kitten arc.
/// The post-loop drain reads `ec.kitten_parentage.get(target)` to learn
/// the current `KittenDependency` state, then writes the updated value.
#[derive(Debug, Clone, Copy)]
enum KittenRearingAdvance {
    Wean(Entity),
    Teach(Entity),
    Release(Entity),
}

/// 364: outcome of the HTN advance / backtrack hook against a non-empty
/// `HeldGoalStack` on a plan-ending tick. The `plans_to_remove` drain
/// branches on this: `AdvanceTo` / `BacktrackTo` keep the cat's HTN
/// frame alive (next tick's L2 author rebuilds `HeldIntention` from the
/// pinned sub-goal); `Done` clears both `HeldIntention` and
/// `HeldGoalStack` and emits the underlying `IntentionFulfilled` /
/// `IntentionAbandoned` Feature.
enum StackOutcome {
    AdvanceTo(crate::components::HeldGoalStack),
    #[allow(dead_code)] // wired by sibling-method backtrack — deferred.
    BacktrackTo(crate::components::HeldGoalStack),
    /// The completed plan was unrelated to the held HTN leaf (e.g.,
    /// the cat ran a Hunt plan while carrying a rear_kitten frame).
    /// Preserve the stack as-is; only HeldIntention is cleared.
    PreserveStackOnly(crate::components::HeldGoalStack),
    Done,
}

/// 364: Fulfilled-path advance. Clone the stack, increment top frame's
/// `sub_goal_index`, recursively pop frames whose sub_goals are
/// exhausted. Returns `AdvanceTo(updated)` when a sub-goal remains
/// (anywhere up the stack), `Done` when the stack ran out.
fn htn_advance_or_pop(stack: crate::components::HeldGoalStack) -> StackOutcome {
    let mut updated = stack;
    loop {
        let Some(top) = updated.top_mut() else {
            return StackOutcome::Done;
        };
        top.sub_goal_index += 1;
        if top.sub_goal_index < top.sub_goal_count {
            return StackOutcome::AdvanceTo(updated);
        }
        // This frame is exhausted. Pop and let the parent frame advance
        // on the next iteration (caller treats child completion as
        // "parent's current sub-goal is satisfied" → bump parent's
        // index too). If no parent, fall through to Done.
        let _ = updated.pop();
    }
}

/// 364: Abandoned-path backtrack/abandon. For 364 scope (rear_kitten
/// is the only Live method for `"kitten_reared"`), Backtrack ≡ Abandon
/// because no sibling Live method exists to walk to. Pops the top
/// frame and falls through to the parent (which is then treated as
/// also-abandoned, recursively). When sibling methods land, this
/// function consults `top.method.failure_strategy` and walks
/// `MethodRegistry::iter_applicable_for(...)` to pick a successor
/// method, emitting `BacktrackTo(..)` instead.
fn htn_abandon_or_pop(stack: crate::components::HeldGoalStack) -> StackOutcome {
    let mut updated = stack;
    while updated.pop().is_some() {
        // Abandoning a child propagates to the parent today. When
        // sibling-method backtrack lands (multi-method goal_label
        // coverage), this loop consults the parent's failure_strategy
        // and branches before pop.
    }
    StackOutcome::Done
}

/// Ticket 177: the per-pair payload queued by a successful Handoff
/// dispatch-arm pre-flight. The post-loop drain re-fetches both
/// inventories via `cats.get_many_mut([actor, recipient])` and runs
/// `resolve_handoff` to perform the actual slot transfer.
#[derive(Debug, Clone, Copy)]
struct HandoffPending {
    actor: Entity,
    recipient: Entity,
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn resolve_goap_plans(
    mut cats: Query<
        (
            (
                Entity,
                &mut GoapPlan,
                &mut CurrentAction,
                &mut Position,
                &mut Skills,
                &mut Needs,
                &mut Inventory,
                &Personality,
                &Name,
                // Ticket 017 — worn equip slots. Read by the weapon-strike
                // bonus in `resolve_engage_prey`; mutated by the craft
                // resolvers to auto-equip a freshly-crafted wearable.
                // Required (inserted at spawn).
                &mut crate::components::equipment::WearableSlots,
            ),
            (
                &Gender,
                Option<&mut ActionHistory>,
                Option<&mut crate::components::grooming::GroomingCondition>,
                &mut crate::components::mental::Mood,
                &mut Health,
                &MagicAffinity,
                &mut Corruption,
                &mut Memory,
                &mut PendingUrgencies,
                Option<&mut crate::components::fulfillment::Fulfillment>,
                // Ticket 073 — per-cat recently-failed target memory.
                // Optional because the component is lazy-inserted on
                // first failure (cats that never fail a target don't
                // pay for the HashMap allocation).
                Option<&mut crate::components::RecentTargetFailures>,
                // Ticket 228 — per-cat route-cost field, inserted at
                // replan in `evaluate_and_plan`. Optional because cats
                // that haven't replanned yet (or whose component was
                // recently removed) won't have it; `dispatch_step_action`
                // falls back to A* via `CatPathPlan::AStarFallback`.
                Option<&crate::components::RouteCostField>,
                // Ticket 095 Phase 1 Stage B — body-zone substrate.
                &crate::components::CatBodyModel,
                // Ticket 463 — per-cat ring buffer of recent crafts,
                // written here on a witnessed craft so the
                // aspiration's anti-monotony term reads the recency
                // map next tick. Optional + lazy-inserted via commands
                // (mirrors `RecentTargetFailures`) so cats that never
                // craft don't pay the archetype shift — preserves
                // seed-1's scenario-test invariance (memory
                // `learning_bevy_schedule_edge_perturbation`).
                Option<&mut crate::components::recent_crafts::CatRecentCrafts>,
                // 140 step 6 — per-tick movement desire; migrated
                // resolvers (TravelTo / PatrolTo / FleeTravel) write
                // it, the Chain-4 integrator consumes it. Required
                // (cat blueprint bundle).
                &mut crate::components::physical::DesiredVelocity,
            ),
        ),
        (
            Without<Dead>,
            Without<Structure>,
            Without<PreyAnimal>,
            Without<PreyDen>,
            Without<Herb>,
            Without<crate::components::wildlife::Carcass>,
            Without<WildAnimal>,
        ),
    >,
    mut prey_query: Query<(Entity, &Position, &PreyConfig, &mut PreyState), With<PreyAnimal>>,
    mut stores_query: Query<&mut StoredItems>,
    items_query: Query<&Item, Without<crate::components::items::BuildMaterialItem>>,
    mut unchained_skills: Query<&mut Skills, (Without<GoapPlan>, Without<Structure>)>,
    mut relationships: ResMut<Relationships>,
    mut narr: NarrativeEmitter<'_>,
    mut rng: ResMut<SimRng>,
    den_query: Query<(Entity, &PreyDen, &Position), Without<PreyAnimal>>,
    mut prey_params: PreyHuntParams,
    mut commands: Commands,
    mut ec: ExecutorContext,
    mut building_params: BuildingResolverParams,
    mut magic_params: MagicResolverParams,
    mut plan_writer: MessageWriter<PlanNarrative>,
) {
    // Ticket 126 — drops carry their lifecycle classification so the
    // cleanup loop can fire `IntentionFulfilled` (achievement-driven)
    // vs `IntentionAbandoned` (every other branch). Local enum kept
    // out of `commitment.rs` so the activation surface stays tied to
    // the per-cat loop's flow control.
    #[derive(Debug, Clone, Copy)]
    enum IntentionEnding {
        Fulfilled,
        // The `IntentionAbandonReason` payload is unused at the
        // activation-emit site today (the per-cause classification lives
        // in the §7.2 trace pipeline). Kept to preserve future
        // `failure_strategy: Retry` branching that consults the reason —
        // 364's backtrack hook will read it once sibling Live methods
        // give Backtrack a non-trivial walk to perform.
        #[allow(dead_code)]
        Abandoned(crate::components::IntentionAbandonReason),
    }
    let mut plans_to_remove: Vec<(Entity, IntentionEnding)> = Vec::new();

    // Pre-collect building and herb data to avoid query conflicts with cats.
    //
    // Ticket 439 — `BuildingResolverParams.buildings` filters out
    // `With<DryingRackState>` / `With<SmokingRackState>` to keep
    // `&mut Structure` disjoint from the read-only `drying_racks` /
    // `smoking_racks` queries (see comment at the SystemParam
    // definition above). That static partition means rack entities
    // never appear in `building_snapshot` unless we chain them in
    // here from the rack-specific queries. Pre-439 the chain was
    // missing entirely, so the downstream `drying_rack_positions`
    // and `smoking_rack_positions` filters (~line 3813-3822) always
    // returned empty slices for completed racks. Marker writers in
    // `buildings.rs` use a separate `Query<(&Structure,
    // Option<&ConstructionSite>)>` *without* the partition, so they
    // saw the racks correctly — but the step-executor's
    // `resolve_zone_position::DryingRack` (goap.rs:9777) returned
    // `None` for the empty slice, surfacing as
    // `"TravelTo(DryingRack): no reachable zone target"` (1095×
    // DryingRack + 719× SmokingRack in the post-437 soak
    // `logs/tuned-42-44a82ba0`; 1178× SmokingRack in the post-438
    // soak `logs/tuned-42-75deed49`). Rack entities have no
    // `ConstructionSite` once built (the construction-completion
    // path at `steps/building/construct.rs:99-141` removes it and
    // inserts `Structure::new(blueprint)` with `condition: 1.0`
    // followed by the `DryingRackState::default()` / `SmokingRackState`
    // insert), so `site.is_some()` on the chained rack rows is
    // always `false` — they're known-completed by construction.
    // The `crop` flag is `false` for racks (no `CropState`).
    let building_snapshot: Vec<(Entity, StructureType, Position, bool, bool)> = building_params
        .buildings
        .iter()
        .map(|(e, s, site, crop, p)| (e, s.kind, *p, site.is_some(), crop.is_some()))
        .chain(
            building_params
                .drying_racks
                .iter()
                .map(|(e, p, s, _state)| (e, s.kind, *p, false, false)),
        )
        .chain(
            building_params
                .smoking_racks
                .iter()
                .map(|(e, p, s, _state)| (e, s.kind, *p, false, false)),
        )
        .collect();

    let stores_positions: Vec<Position> = building_snapshot
        .iter()
        .filter(|(_, kind, _, _, _)| *kind == StructureType::Stores)
        .map(|(_, _, p, _, _)| *p)
        .collect();

    let stores_entities: Vec<(Entity, Position)> = building_snapshot
        .iter()
        .filter(|(_, kind, _, _, _)| *kind == StructureType::Stores)
        .map(|(e, _, p, _, _)| (*e, *p))
        .collect();

    // Ticket 091: source the planner's `HasStoredFood` from `StoredItems`
    // directly (not the `FoodStores` resource cache, which `sync_food_stores`
    // refreshes once per tick and can lag a step behind a withdraw within
    // the same tick). Mirrors the IAUS-substrate authoring at goap.rs:919.
    let has_stored_food = stores_entities.iter().any(|(e, _)| {
        stores_query.get(*e).is_ok_and(|stored| {
            stored
                .items
                .iter()
                .copied()
                .any(|ie| items_query.get(ie).is_ok_and(|it| it.kind.is_food()))
        })
    });

    // Ticket 092: build the per-tick planner-facing `MarkerSnapshot`
    // alongside `has_stored_food`. Carries the colony-scoped markers the
    // planner gates on via `HasMarker(...)`. `evaluate_and_plan` builds
    // its own snapshot from the full `world_state` query set; this
    // replan-side snapshot covers the subset the planner actually
    // consults at replan time (HasStoredFood, ThornbriarAvailable,
    // and the per-cat `MaterialsAvailable` authored below once
    // `construction_materials_complete` is in scope).
    let mut planner_markers = {
        let mut m = crate::ai::scoring::MarkerSnapshot::new();
        if has_stored_food {
            m.set_colony(markers::HasStoredFood::KEY, true);
        }
        let thornbriar_available = crate::systems::magic::is_thornbriar_available(
            magic_params.herb_query.iter().map(|(_, h, _)| h),
        );
        if thornbriar_available {
            m.set_colony(markers::ThornbriarAvailable::KEY, true);
        }
        // 084: HasStoredThornbriar at plan time so the
        // `RetrieveHerbs(Thornbriar)` planner action's precondition
        // resolves correctly during mid-execution replans. Source the
        // current stash level directly from `building_params.stored_herbs`
        // (same source the `update_colony_building_markers` writer reads)
        // so plan-time and L2 marker state match.
        let has_stored_thornbriar = building_params
            .stored_herbs
            .iter()
            .any(|sh| sh.count(crate::components::magic::HerbKind::Thornbriar) > 0);
        if has_stored_thornbriar {
            m.set_colony(markers::HasStoredThornbriar::KEY, true);
        }
        // 084 Commit 3: ColonyThornbriarChronicallyLow is NOT plumbed
        // here. It's a DSE-scoring input (via FarmDse's
        // MarkerConsideration), not a GOAP planner-action precondition.
        // L2 reads the marker through the `MarkerSnapshot` built in
        // `evaluate_and_plan` from `colony_state_query`; replan-time
        // doesn't need to re-fetch it because no `StatePredicate::HasMarker`
        // in `actions.rs` references it.
        m
    };

    // Only completed kitchens count — a construction site can't be cooked at.
    let kitchen_entities: Vec<(Entity, Position)> = building_snapshot
        .iter()
        .filter(|(_, kind, _, is_site, _)| *kind == StructureType::Kitchen && !*is_site)
        .map(|(e, _, p, _, _)| (*e, *p))
        .collect();
    let kitchen_positions: Vec<Position> = kitchen_entities.iter().map(|(_, p)| *p).collect();

    // 367: completed preservation-station positions for zone resolution.
    // Mirrors the `kitchen_positions` shape — completed buildings only;
    // load / cooldown discrimination lives at resolver-time.
    let drying_rack_positions: Vec<Position> = building_snapshot
        .iter()
        .filter(|(_, kind, _, is_site, _)| *kind == StructureType::DryingRack && !*is_site)
        .map(|(_, _, p, _, _)| *p)
        .collect();
    let smoking_rack_positions: Vec<Position> = building_snapshot
        .iter()
        .filter(|(_, kind, _, is_site, _)| *kind == StructureType::SmokingRack && !*is_site)
        .map(|(_, _, p, _, _)| *p)
        .collect();
    // 457: Workshop positions for `PlannerZone::Workshop` zone resolution.
    let workshop_positions: Vec<Position> = building_snapshot
        .iter()
        .filter(|(_, kind, _, is_site, _)| *kind == StructureType::Workshop && !*is_site)
        .map(|(_, _, p, _, _)| *p)
        .collect();
    // 369: TanningFrame positions for `PlannerZone::TanningFrame`.
    let tanning_frame_positions: Vec<Position> = building_snapshot
        .iter()
        .filter(|(_, kind, _, is_site, _)| *kind == StructureType::TanningFrame && !*is_site)
        .map(|(_, _, p, _, _)| *p)
        .collect();

    // Ticket 177: completed Middens — used by the `TrashItemAtMidden`
    // dispatch arm to resolve the target entity's `Position` without
    // widening `stores_query` to overlap `Structure`/`Position`.
    let midden_entities: Vec<(Entity, Position)> = building_snapshot
        .iter()
        .filter(|(_, kind, _, is_site, _)| *kind == StructureType::Midden && !*is_site)
        .map(|(e, _, p, _, _)| (*e, *p))
        .collect();

    let construction_positions: Vec<(Entity, Position)> = building_snapshot
        .iter()
        .filter(|(_, _, _, is_site, _)| *is_site)
        .map(|(e, _, p, _, _)| (*e, *p))
        .collect();

    let farm_positions: Vec<Position> = building_snapshot
        .iter()
        .filter(|(_, kind, _, is_site, _)| *kind == StructureType::Garden && !*is_site)
        .map(|(_, _, p, _, _)| *p)
        .collect();

    let herb_positions: Vec<(Entity, Position, HerbKind)> = magic_params
        .herb_query
        .iter()
        .map(|(e, herb, p)| (e, *p, herb.kind))
        .collect();

    // Ground material piles (Wood / Stone laid out by the founding wagon-
    // dismantling spawn or any future on-the-ground deposit). Filter to
    // items whose kind maps to a build `Material` and that are still
    // `OnGround` (not yet picked up).
    let material_pile_positions: Vec<(Entity, Position, ItemKind)> = building_params
        .material_items
        .iter()
        .filter(|(_, item, _)| {
            matches!(
                item.location,
                crate::components::items::ItemLocation::OnGround
            ) && item.kind.material().is_some()
        })
        .map(|(e, item, p)| (e, *p, item.kind))
        .collect();

    // Ticket 193: ground food/carcass-pile Items — engage_prey overflow
    // drops these at the kill tile when inventory is full and the cat
    // isn't self-eating. Source for `PlannerZone::CarcassPile` zone
    // resolution and the `PickUpItemFromGround` dispatch arm's nearest-
    // target fallback (mirror of `material_pile_positions`).
    let food_pile_positions: Vec<(Entity, Position, ItemKind)> = building_params
        .food_items
        .iter()
        .filter(|(_, item, _)| {
            matches!(
                item.location,
                crate::components::items::ItemLocation::OnGround
            ) && item.kind.is_food()
        })
        .map(|(e, item, p)| (e, *p, item.kind))
        .collect();

    // Per-site materials_complete map (see StepSnapshots::
    // construction_materials_complete). Coordinator-spawned sites are
    // prefunded → true; founding wagon-dismantling sites are non-
    // prefunded → false until cats finish hauling.
    let construction_materials_complete: HashMap<Entity, bool> = building_params
        .buildings
        .iter()
        .filter_map(|(e, _, site, _, _)| site.map(|s| (e, s.materials_complete())))
        .collect();

    // Ticket 096: author the per-cat `MaterialsAvailable` substrate
    // marker. `Construct`'s substrate-branch precondition consults it
    // via `HasMarker(MaterialsAvailable::KEY)`; the plan-branch
    // (`MaterialsDeliveredThisPlan(true)`) covers the in-flight
    // haul→deliver→construct compose case where the marker still reads
    // false at plan entry.
    let herb_stash_radius = ec.constants.disposition.herb_stash_reachable_radius;
    // 487 — planner-markers parity with `evaluate_and_plan`'s
    // `HasGroomCandidate` author. Build `currently_groomed` and the
    // colony-wide cat position snapshot from `cats` itself rather than
    // a new SystemParam query. The position snapshot is identical to
    // what `cats` iterates below; pre-collecting once avoids paying
    // the query overhead per-cat in the marker loop.
    let groom_cat_positions: Vec<(Entity, Position)> = cats
        .iter()
        .map(|((entity, _, _, pos, _, _, _, _, _, _), _)| (entity, *pos))
        .collect();
    // 487 — both groomer and groomee enter the set; see
    // `evaluate_and_plan`'s analogous block for the reasoning.
    let groom_currently_groomed: std::collections::HashSet<Entity> = {
        let mut set = std::collections::HashSet::new();
        for ((actor, _, current, _, _, _, _, _, _, _), _) in &cats {
            if matches!(current.action, Action::GroomOther) {
                set.insert(actor);
                if let Some(target) = current.target_entity {
                    set.insert(target);
                }
            }
        }
        set
    };
    for ((entity, _, _, pos, _, _, inventory, _, _, _), _) in &cats {
        planner_markers.set_entity(
            markers::MaterialsAvailable::KEY,
            entity,
            materials_available_for(
                pos,
                &construction_positions,
                &construction_materials_complete,
            ),
        );
        // Ticket 231: HasFreeSlot per-cat snapshot from `Inventory`.
        // Read by the substrate-path variant of pickup-class plan
        // actions; the plan-path variant uses HasFreeSlotThisPlan.
        planner_markers.set_entity(markers::HasFreeSlot::KEY, entity, !inventory.is_full());
        // Ticket 235: per-cat HasHerbStashAccessible (sibling of
        // MaterialsAvailable). Snapshot parity with the
        // `evaluate_and_plan` author site is required for the
        // planner-replay path.
        let stores_reachable = herb_stash_accessible_for(pos, &stores_positions, herb_stash_radius);
        planner_markers.set_entity(
            markers::HasHerbStashAccessible::KEY,
            entity,
            stores_reachable,
        );
        // PickingUp eligibility gate — sibling of HasHerbStashAccessible
        // with identical geometry; planner-replay parity with the
        // `evaluate_and_plan` author site above.
        planner_markers.set_entity(
            markers::HasFoodStorageAccessible::KEY,
            entity,
            stores_reachable,
        );
        // 487 — `HasGroomCandidate` planner-markers parity with the
        // `evaluate_and_plan` author site. Same predicate (see
        // `viable_groom_candidate_for`) so the planner-replay path
        // gates `GroomOtherDse` eligibility identically to the L2
        // scoring path.
        planner_markers.set_entity(
            markers::HasGroomCandidate::KEY,
            entity,
            viable_groom_candidate_for(entity, pos, &groom_cat_positions, &groom_currently_groomed),
        );
    }

    // Count cats adjacent to each construction site (for multi-builder bonuses).
    // Ticket 427 Step 5 — `with_capacity` pre-sizes from the
    // building-snapshot length so the HashMap avoids rehash churn as it
    // fills. Full alloc elimination (move to `Local<...>`) deferred
    // because `StepSnapshots` would need an `'a` lifetime parameter,
    // which ripples through ~80 read sites.
    let builders_per_site: HashMap<Entity, usize> = {
        let cat_pos_list: Vec<Position> = cats
            .iter()
            .map(|((_, _, _, pos, _, _, _, _, _, _), _)| *pos)
            .collect();
        let mut counts = HashMap::with_capacity(building_snapshot.len());
        for (site_e, _, site_pos, is_site, _) in &building_snapshot {
            if *is_site {
                let n = cat_pos_list
                    .iter()
                    .filter(|cp| cp.chebyshev_distance(site_pos) <= 1)
                    .count();
                if n > 0 {
                    counts.insert(*site_e, n);
                }
            }
        }
        counts
    };

    let snaps = StepSnapshots {
        grooming: cats
            .iter()
            .map(
                |(
                    (e, _, _, _, _, _, _, _, _, _),
                    (_, _, g, _, _, _, _, _, _, _, _, _, _, _, _),
                )| { (e, g.as_ref().map_or(0.8, |g| g.0)) },
            )
            .collect(),
        // Gender snapshot for §7.M.7.4's `resolve_mate_with` partner lookup —
        // lets the MateWith step pick the gestation-capable partner without
        // double-borrowing the mutable `cats` query.
        gender: cats
            .iter()
            .map(
                |(
                    (e, _, _, _, _, _, _, _, _, _),
                    (g, _, _, _, _, _, _, _, _, _, _, _, _, _, _),
                )| { (e, *g) },
            )
            .collect(),
        stores_positions,
        stores_entities,
        midden_entities,
        kitchen_positions,
        construction_positions,
        farm_positions,
        herb_positions,
        material_pile_positions,
        food_pile_positions,
        planner_markers,
        // 367 — preservation-station position snapshots threaded
        // into the zone resolver.
        drying_rack_positions,
        smoking_rack_positions,
        // 457 — Workshop positions threaded into the zone resolver.
        workshop_positions,
        // 369 — TanningFrame positions threaded into the zone resolver.
        tanning_frame_positions,
        workshop_bonus: if building_snapshot
            .iter()
            .any(|(_, kind, _, _, _)| *kind == StructureType::Workshop)
        {
            1.3
        } else {
            1.0
        },
        // Seasonal modifier for farming — simplified to 1.0 pending SimConfig
        // access in ExecutorContext. Tunable later.
        season_mod: 1.0,
        builders_per_site,
        cat_positions: cats
            .iter()
            .map(|((e, _, _, pos, _, _, _, _, _, _), _)| (e, *pos))
            .collect(),
        // 487 follow-on — pre-build the in-flight GroomOther set so the
        // dispatch arm's `resolve_groom_other_target` call can exclude
        // mid-groom peers symmetrically with the marker author. See
        // `StepSnapshots::currently_groomed` doc-comment.
        currently_groomed: {
            let mut set = std::collections::HashSet::new();
            for ((actor, _, current, _, _, _, _, _, _, _), _) in &cats {
                if matches!(current.action, Action::GroomOther) {
                    set.insert(actor);
                    if let Some(target) = current.target_entity {
                        set.insert(target);
                    }
                }
            }
            set
        },
        injured_cat_positions: cats
            .iter()
            .filter(|(_, (_, _, _, _, health, _, _, _, _, _, _, _, _, _, _))| {
                health.current < health.max
            })
            .map(|((e, _, _, pos, _, _, _, _, _, _), _)| (e, *pos))
            .collect(),
        // §6.5.3 mentor-target DSE snapshot: candidate-side Skills lookup
        // table. Built once per tick so the MentorCat branch can rank
        // apprentices by skill-gap without re-borrowing `cats` (which is
        // mutably held by the outer loop).
        cat_skills: cats
            .iter()
            .map(|((e, _, _, _, skills, _, _, _, _, _), _)| (e, (*skills).clone()))
            .collect(),
        // §6.5.4 groom-other-target DSE snapshot: candidate-side
        // `needs.temperature` lookup. Same rationale as skills — the outer
        // loop mutably holds `cats`, so we materialize a read-only map for
        // the GroomOther branch's `resolve_groom_other_target` call.
        cat_temperature: cats
            .iter()
            .map(|((e, _, _, _, _, needs, _, _, _, _), _)| (e, needs.temperature))
            .collect(),
        // Ticket 452 — per-cat `GroomingCondition.0` snapshot for the
        // `target_grooming_deficit` axis on `GroomOtherTargetDse`.
        // Pulled from the same `cats.iter()` pass that produces
        // `cat_temperature`; `GroomingCondition` lives in the cats
        // query's second inner tuple at position 4 (see the query
        // def at the top of `resolve_goap_plans`). Cats lacking the
        // component (save-loaded pre-component) are filtered out;
        // the resolver's `.unwrap_or(0.0)` fallthrough treats their
        // absence as 0.0 deficit.
        cat_grooming: cats
            .iter()
            .filter_map(
                |(
                    (e, _, _, _, _, _, _, _, _, _),
                    (_, _, gc, _, _, _, _, _, _, _, _, _, _, _, _),
                )| { gc.map(|g| (e, g.0)) },
            )
            .collect(),
        // §6.5.4 kinship lookup — `(kitten_entity) → (mother, father)`.
        // Bidirectional `is_kin` is computed per-call by the resolver
        // closure. Reads `ExecutorContext::kitten_parentage` — kittens
        // don't carry `GoapPlan`, so this query is disjoint from the
        // outer mutable `cats` iteration.
        kitten_parents: ec
            .kitten_parentage
            .iter()
            .map(|(e, dep, _released)| (e, (dep.mother, dep.father)))
            .collect(),
        // 035: dead-cat snapshot for burial target picking + post-loop
        // drain. The `cats` query is `Without<Dead>` so this is disjoint.
        dead_cat_positions: ec.dead_cats_q.iter().map(|(e, p, _, _)| (e, *p)).collect(),
        dead_cat_names: ec
            .dead_cats_q
            .iter()
            .map(|(e, _, name, _)| (e, name.0.clone()))
            .collect(),
        // §428 / 451: populate kitten_snapshot from `ec.kitten_parentage`
        // (slim — entity / parentage / RearKittenReleased) + an
        // immutable hunger + position lookup pulled from the unified
        // cats query (kittens carry `GoapPlan` post-451). The HandoffItem
        // goap-path resolver at line 7322 reads this snapshot to recover
        // a recipient when
        // `target_entity` was cleared mid-plan by one of the eight
        // `disposition.rs` clear sites. The previous `Vec::new()` was a
        // substrate-stub class defect (same class as tickets 209 / 084):
        // marker authored + DSE elects + planner emits, but the resolver
        // read from an empty Vec and hard-failed with `handoff: no
        // recipient on disposition (no dependent cat in colony)` 177k+
        // times per overnight soak.
        //
        // Ticket 451 — `kitten_needs` retired; pull immutable `&Needs`
        // for the hunger snapshot via the cats query (kittens have
        // `GoapPlan` post-451 so they appear there). Position similarly
        // sourced from cats since the slim `kitten_parentage` no longer
        // carries it.
        kitten_snapshot: {
            let kitten_hunger: std::collections::HashMap<Entity, (f32, Position)> = cats
                .iter()
                .map(|((e, _, _, pos, _, needs, _, _, _, _), _)| (e, (needs.hunger, *pos)))
                .collect();
            ec.kitten_parentage
                .iter()
                .map(|(entity, dep, _released)| {
                    let (hunger, pos) = kitten_hunger
                        .get(&entity)
                        .copied()
                        .unwrap_or((1.0, Position::new(0, 0)));
                    crate::ai::caretake_targeting::KittenState {
                        entity,
                        pos,
                        hunger,
                        mother: dep.mother,
                        father: dep.father,
                    }
                })
                .collect()
        },
        building_snapshot,
    };

    let mut accum = StepAccumulators {
        mentor_effects: Vec::new(),
        grooming_restorations: Vec::new(),
        // §Phase 4c.3: deferred kitten-feedings — the cats query already
        // owns &mut Needs over every non-dead cat (including kittens), so
        // updates are collected here and applied in a second pass.
        kitten_feedings: Vec::new(),
        handoff_pending: Vec::new(),
        bury_completions: Vec::new(),
        kitten_rearing_advances: Vec::new(),
    };

    for (
        (
            cat_entity,
            mut plan,
            mut current,
            mut pos,
            mut skills,
            mut needs,
            mut inventory,
            personality,
            name,
            mut wearables,
        ),
        (
            gender,
            history,
            mut grooming,
            mut mood,
            mut health,
            magic_aff,
            mut corruption,
            mut memory,
            mut urgencies,
            mut fulfillment_opt,
            mut recent_failures,
            route_cost_field,
            body_model,
            mut recent_crafts,
            mut desired_velocity,
        ),
    ) in &mut cats
    {
        let d = &ec.constants.disposition;

        // §7.2 commitment gate — evaluate whether to drop the held intention.
        let strategy = crate::ai::commitment::strategy_for_disposition(plan.kind);
        let unexplored_nearby = prey_params.exploration_map.unexplored_fraction_nearby(
            pos.x(),
            pos.y(),
            d.explore_perception_radius,
            0.5,
        );
        let proxies = crate::ai::commitment::proxies_for_plan(&plan, &needs, d, unexplored_nearby);
        if crate::ai::commitment::should_drop_intention(strategy, proxies) {
            let branch = if proxies.achievement_believed {
                crate::ai::commitment::DropBranch::Achieved
            } else if !proxies.achievable_believed {
                crate::ai::commitment::DropBranch::ReplanCap
            } else {
                crate::ai::commitment::DropBranch::DroppedGoal
            };
            crate::ai::commitment::record_drop(narr.activation.as_deref_mut(), strategy, branch);
            current.ticks_remaining = 0;
            // Ticket 126 — map §7.2 DropBranch onto the lifecycle.
            // 288 — MoraleBreak is not produced by the §7.2 belief-proxy
            // gate (it's emitted only by the step-fail dispatcher
            // below), so it cannot reach this arm. Treat as unreachable
            // and fall through with an explicit guard so the compiler
            // keeps the §7.2 path honest if MoraleBreak ever gets
            // wired here.
            let ending = match branch {
                crate::ai::commitment::DropBranch::Achieved => IntentionEnding::Fulfilled,
                crate::ai::commitment::DropBranch::ReplanCap
                | crate::ai::commitment::DropBranch::MoraleBreak => IntentionEnding::Abandoned(
                    crate::components::IntentionAbandonReason::BecameImpossible,
                ),
                crate::ai::commitment::DropBranch::DroppedGoal => IntentionEnding::Abandoned(
                    crate::components::IntentionAbandonReason::DesireDrift,
                ),
            };
            plans_to_remove.push((cat_entity, ending));
            continue;
        }

        // Ticket 126 — Trigger (3) preempt. Reads the cat's held
        // intention plus the cached `current.last_scores` (one-tick-
        // stale; populated at the previous `evaluate_and_plan`). If a
        // non-held DSE's score exceeds
        // `held_score + intention_preempt_margin`, drop the plan with
        // `Preempted`.
        //
        // **Substrate-honest formula (ticket 248).** Pre-248 the
        // formula carried a `commitment_strength × intention_momentum_lift`
        // middle term to compensate for a timing defect: the L2 author
        // site captured `last_scores` BEFORE inserting `HeldIntention`
        // on fresh adoption, so the recorded `held_score` never saw the
        // `IntentionMomentum` modifier's lift. 248 fixed the timing by
        // surgically applying the lift to `last_scores[chosen_action]`
        // at the L3 adoption site (above), so `held_score` here now
        // reflects the modifier's lift directly. The middle term would
        // double-count and is retired. Trigger-3 is now `held_score
        // (lifted) + margin` — the honest "is the next-best DSE
        // meaningfully better than the lifted held score" check.
        //
        // **The strength regime gate is still load-bearing (ticket
        // 248).** 248's verification soak with the gate at 0.0 still
        // collapsed (5,000-tick PickUp lock; preserved at
        // `logs/tuned-42-post-248-boundary-zero-collapsed/`), proving
        // the gate addresses a separate failure mode from the lift-
        // timing defect: **softmax-low-margin oscillation.** Low
        // `commitment_strength` reflects a near-tie softmax pick,
        // meaning the runner-up's `last_scores` entry is barely below
        // `held_score`. Without the gate, trigger-3 fires next tick,
        // §7.2 dual-removal clears the held intention, the re-election
        // picks a similarly thin chosen_action, and the cycle locks.
        // The gate at 0.5 says: "if the softmax was a close call,
        // don't preempt — let the natural §7.2 path handle drops."
        // See the field doc-comment on
        // `DispositionConstants::intention_preempt_strength_regime_boundary`
        // for the full rationale.
        //
        // **History (tickets 126 → 246 → 247 → 248).** 126 introduced
        // a function-local `PREEMPT_STRENGTH_FLOOR = 0.5` gate after
        // observing 230× wall-clock slowdown on a strength-0 cat. 246
        // wired the `IntentionMomentum` modifier and tried to retire
        // the floor; the soak collapsed (5,580 ticks vs 122,758
        // healthy, 99.5% PickUp/Drop lock, 0 Stores built, 1172
        // Resting GoalUnreachable). 247 diagnosed the timing defect
        // and renamed the floor as a substrate-side branch
        // (`intention_preempt_strength_regime_boundary`, default 0.5),
        // pricing the failure mode without resolving it. 248 owns the
        // substrate-correct fix at the L3 adoption site (above) —
        // `held_score` now carries the lift directly, the middle
        // compensation term is retired — but the gate stays at 0.5
        // because retiring it re-introduces the lock under the
        // softmax-low-margin dynamic above.
        //
        // Trigger (4) target_invalidates_intention is wired in
        // `commitment.rs` but not consulted here in 126 because
        // `HeldIntention.target` is always `None` at the C3 author
        // site (target tracking lands with 127/129).
        if let Ok(held) = ec.held_intentions.get(cat_entity) {
            if held.commitment_strength >= d.intention_preempt_strength_regime_boundary {
                let held_score = current
                    .last_scores
                    .iter()
                    .find(|(a, _)| *a == held.held_action)
                    .map(|(_, s)| *s)
                    .unwrap_or(0.0);
                let top_non_held = current
                    .last_scores
                    .iter()
                    .filter(|(a, _)| *a != held.held_action)
                    .map(|(_, s)| *s)
                    .fold(f32::NEG_INFINITY, f32::max);
                if top_non_held.is_finite() && held_score > 0.0 {
                    // 248: middle term `commitment_strength ×
                    // intention_momentum_lift` retired. `held_score`
                    // now reflects the IntentionMomentum lift
                    // directly via the L3 adoption-site write.
                    let preempt_threshold = held_score + d.intention_preempt_margin;
                    if top_non_held > preempt_threshold {
                        plan_writer.write(PlanNarrative {
                            entity: cat_entity,
                            kind: plan.kind,
                            event: PlanEvent::Abandoned,
                            completions: plan.trips_done,
                        });
                        current.ticks_remaining = 0;
                        plans_to_remove.push((
                            cat_entity,
                            IntentionEnding::Abandoned(
                                crate::components::IntentionAbandonReason::Preempted,
                            ),
                        ));
                        continue;
                    }
                }
            }
        }

        // ---- Plan exhausted: handle trip completion / replanning ----
        if plan.is_exhausted() {
            plan.trips_done += 1;
            let respect_gain = respect_for_disposition(plan.kind, d);
            if respect_gain > 0.0 {
                needs.respect = (needs.respect + respect_gain).min(1.0);
            }
            // §respect-restoration iter 1 (relocated): witness-multiplier
            // on top of the baseline respect_for_disposition. Respect from
            // completing a task scales with social visibility up to
            // `respect_witness_cap` other cats within `respect_witness_radius`.
            // The twin writes that used to live in `resolve_disposition_chains`
            // were in a test-only schedule; this is the canonical live site.
            // See `docs/balance/respect-restoration.md`.
            let witnesses = crate::systems::disposition::count_witnesses_within_radius(
                cat_entity,
                &pos,
                &snaps.cat_positions,
                d.respect_witness_radius,
                d.respect_witness_cap,
            );
            if witnesses > 0 {
                needs.respect = (needs.respect + d.respect_per_witness * witnesses as f32).min(1.0);
            }

            // Building completion mood boost.
            if plan.kind == DispositionKind::Building {
                mood.modifiers.push_back(
                    crate::components::mental::MoodModifier::new(0.2, 100, "built something")
                        .with_kind(crate::components::mental::MoodSource::Pride),
                );
            }

            // Check if disposition goal is fully met.
            //
            // 150 R5a: Resting drops hunger from the three-need check
            // (Eating owns hunger now). Eating gets its own arm so the
            // count-based fallback (`trips_done >= target_trips`)
            // doesn't spin on `target_trips=u32::MAX`.
            let disposition_complete = match plan.kind {
                DispositionKind::Resting => {
                    needs.energy >= d.resting_complete_energy
                        && needs.temperature >= d.resting_complete_temperature
                }
                DispositionKind::Eating => needs.hunger >= d.resting_complete_hunger,
                _ => plan.trips_done >= plan.target_trips,
            };

            if disposition_complete {
                if let Some(mut hist) = history {
                    hist.record(ActionRecord {
                        action: current.action,
                        disposition: Some(plan.kind),
                        tick: ec.time.tick,
                        outcome: ActionOutcome::Success,
                    });
                }
                // §7.2 de-facto `achievement_believed` branch. The
                // pluggable Phase 6a gate is deferred; this path is
                // the effective "gate fired with achieved" until it
                // lands. Telemetry + trace capture mirror what the
                // pluggable gate will emit, so replay tooling won't
                // need to diff the shapes later.
                let strategy = crate::ai::commitment::strategy_for_disposition(plan.kind);
                crate::ai::commitment::record_drop(
                    narr.activation.as_deref_mut(),
                    strategy,
                    crate::ai::commitment::DropBranch::Achieved,
                );
                if ec_is_focal(&ec, cat_entity) {
                    let proxies = crate::ai::commitment::proxies_for_plan(
                        &plan,
                        &needs,
                        &ec.constants.disposition,
                        unexplored_nearby,
                    );
                    crate::ai::commitment::record_commitment_decision(
                        ec.focal_capture.as_deref(),
                        ec.time.tick,
                        &plan,
                        strategy,
                        proxies,
                        true,
                        crate::ai::commitment::DropBranch::Achieved.as_str(),
                    );
                }
                plan_writer.write(PlanNarrative {
                    entity: cat_entity,
                    kind: plan.kind,
                    event: PlanEvent::Completed,
                    completions: plan.trips_done,
                });
                current.ticks_remaining = 0;
                plans_to_remove.push((cat_entity, IntentionEnding::Fulfilled));
            } else {
                // Need more trips — replan from current state.
                let planner_state = build_planner_state(
                    &pos,
                    &needs,
                    &inventory,
                    plan.trips_done,
                    &ec.map,
                    &snaps.stores_positions,
                    &snaps.construction_positions,
                    &snaps.farm_positions,
                    &snaps.herb_positions,
                    &snaps.material_pile_positions,
                    &snaps.food_pile_positions,
                    d,
                );
                let zone_distances = build_zone_distances(
                    &pos,
                    &ec.map,
                    &snaps.stores_positions,
                    &snaps.construction_positions,
                    &snaps.farm_positions,
                    &snaps.herb_positions,
                    &snaps.kitchen_positions,
                    &snaps.cat_positions,
                    &snaps.material_pile_positions,
                    &snaps.food_pile_positions,
                    &snaps.drying_rack_positions,
                    &snaps.smoking_rack_positions,
                    &snaps.workshop_positions,
                    &snaps.tanning_frame_positions,
                    &snaps.dead_cat_positions,
                    cat_entity,
                    d,
                );
                let actions =
                    actions_for_disposition(plan.kind, plan.chosen_action, &zone_distances);
                let plan_ctx = crate::ai::planner::PlanContext {
                    markers: &snaps.planner_markers,
                    entity: cat_entity,
                };
                let goal = goal_for_disposition(plan.kind, plan.trips_done, &plan_ctx);

                if let Ok(new_steps) = make_plan(
                    planner_state,
                    &actions,
                    &goal,
                    12,
                    1000,
                    &plan_ctx,
                    &mut ec.planner_scratch,
                ) {
                    plan.replan(new_steps);
                } else {
                    // Can't plan next trip — complete anyway. The typed
                    // failure reason isn't surfaced here because this
                    // path doesn't emit a `PlanningFailed` event (it
                    // completes the existing plan rather than recording
                    // a new failure). 172 leaves this site untouched
                    // semantically.
                    current.ticks_remaining = 0;
                    // Ticket 126 — trip-target met but next replan
                    // failed. The `disposition_complete` predicate
                    // wasn't true (we wouldn't be in this `else`
                    // branch otherwise), so this is an abandon, not a
                    // fulfilment. Use `BecameImpossible` to mirror the
                    // §7.2 ReplanCap mapping.
                    plans_to_remove.push((
                        cat_entity,
                        IntentionEnding::Abandoned(
                            crate::components::IntentionAbandonReason::BecameImpossible,
                        ),
                    ));
                }
            }
            continue;
        }

        // ---- Get current step and tick ----
        let step_idx = plan.current_step;
        let step = &plan.steps[step_idx];
        let action_kind = step.action;

        // Initialize step state on first tick.
        if plan.step_state[step_idx].ticks_elapsed == 0 {
            current.action = action_kind.to_action(plan.kind, plan.chosen_action);
            current.target_position = plan.step_state[step_idx].target_position;
            current.target_entity = plan.step_state[step_idx].target_entity;
        }

        plan.step_state[step_idx].ticks_elapsed += 1;
        let ticks = plan.step_state[step_idx].ticks_elapsed;

        // ---- Dispatch on action kind ----
        // Extracted to a separate function to keep `resolve_goap_plans`
        // under LLVM's optimization-cliff threshold (~4,500 lines).
        // See docs/systems/phase-6a-commitment-gate-attempt.md
        // §"LLVM optimization cliff".
        let step_result = dispatch_step_action(
            action_kind,
            step_idx,
            ticks,
            cat_entity,
            &mut plan,
            &mut current,
            &mut pos,
            &mut desired_velocity,
            &mut skills,
            &mut needs,
            &mut inventory,
            &mut wearables,
            personality,
            name,
            gender,
            grooming.as_deref_mut(),
            &mut mood,
            &mut health,
            magic_aff,
            &mut corruption,
            &mut memory,
            &mut fulfillment_opt,
            &mut relationships,
            &mut narr,
            &mut rng,
            &mut prey_query,
            &mut stores_query,
            &items_query,
            &den_query,
            &mut prey_params,
            &mut commands,
            &mut ec,
            &mut building_params,
            &mut magic_params,
            &snaps,
            &mut accum,
            recent_failures.as_deref(),
            route_cost_field,
            body_model,
            recent_crafts.as_deref_mut(),
        );

        // Re-derive `d` after the dispatch call so the immutable borrow
        // doesn't span across the `&mut ec` parameter above.
        let d = &ec.constants.disposition;

        // Global safety net: no single step should run indefinitely.
        let step_result = if matches!(step_result, crate::steps::StepResult::Continue)
            && ticks > d.global_step_timeout_ticks
        {
            crate::steps::StepResult::Fail("global step timeout".into())
        } else {
            step_result
        };

        // Apply step result.
        match step_result {
            crate::steps::StepResult::Continue => {}
            crate::steps::StepResult::Advance => {
                // --- Step boundary: evaluate pending urgencies ---
                let mut preempted = false;
                if let Some(urgent) = urgencies.highest() {
                    let current_maslow = plan.kind.maslow_tier();
                    // An urgency preempts only if its maslow tier is strictly
                    // lower (more fundamental) than the current plan's.
                    //
                    // 511 exception — Starvation-vs-Resting is a
                    // tier-1-vs-tier-1 pair the strict comparison can
                    // never break, and Resting plans self-perpetuate
                    // through the held-intention replan path without
                    // fresh elections: a resting cat could starve to
                    // death un-interrupted. A starving body overrides
                    // rest. (Generalizing equal-tier trade-offs is the
                    // §7 commitment layer's job — ticket 509.)
                    let starvation_breaks_rest = urgent.kind == UrgencyKind::Starvation
                        && plan.kind == DispositionKind::Resting;
                    if urgent.maslow_tier < current_maslow || starvation_breaks_rest {
                        // Preserve Hunt/Herbcraft guard for threats.
                        // 155: `Action::Herbcraft` retired; the three
                        // sub-actions (Gather/Remedy/SetWard) all carry
                        // the same threat-suppression semantics.
                        let suppressed = urgent.kind == UrgencyKind::ThreatNearby
                            && matches!(
                                current.action,
                                Action::Hunt
                                    | Action::HerbcraftGather
                                    | Action::HerbcraftRemedy
                                    | Action::HerbcraftSetWard
                            );

                        if !suppressed {
                            if let Some(ref mut log) = ec.event_log {
                                let current_step = plan
                                    .current()
                                    .map(|s| format!("{:?}", s.action))
                                    .unwrap_or_else(|| "none".into());
                                log.push(
                                    ec.time.tick,
                                    EventKind::PlanInterrupted {
                                        cat: name.0.clone(),
                                        disposition: format!("{:?}", plan.kind),
                                        reason: format!(
                                            "urgency {:?} (tier {}) preempted tier {} plan",
                                            urgent.kind, urgent.maslow_tier, current_maslow
                                        ),
                                        current_step,
                                        hunger: needs.hunger,
                                        energy: needs.energy,
                                        temperature: needs.temperature,
                                    },
                                );
                            }

                            // Compute the flee target (if any) for ThreatNearby,
                            // then dispatch into `plan_substrate::try_preempt`
                            // which owns the load-bearing
                            // `current.ticks_remaining = 0` reset (ticket 041)
                            // alongside the `plan.current_step = plan.steps.len()`
                            // exhaustion mark. Ticket 072 lifted these from the
                            // inline body so the fix is API-owned.
                            let preempt_kind = if urgent.kind == UrgencyKind::ThreatNearby {
                                if let Some(threat_pos) = urgent.threat_pos {
                                    let dx = pos.x() - threat_pos.x();
                                    let dy = pos.y() - threat_pos.y();
                                    let len = ((dx * dx + dy * dy) as f32).sqrt().max(1.0);
                                    let fd = d.flee_distance;
                                    let mut target = Position::new(
                                        pos.x() + (dx as f32 / len * fd) as i32,
                                        pos.y() + (dy as f32 / len * fd) as i32,
                                    );
                                    target.set_tile(
                                        target.x().clamp(0, ec.map.width - 1),
                                        target.y().clamp(0, ec.map.height - 1),
                                    );
                                    crate::systems::plan_substrate::PreemptKind::ThreatFlee {
                                        flee_target: target,
                                    }
                                } else {
                                    crate::systems::plan_substrate::PreemptKind::ThreatWithoutPosition
                                }
                            } else {
                                crate::systems::plan_substrate::PreemptKind::NonThreat
                            };
                            let _outcome = crate::systems::plan_substrate::try_preempt(
                                &mut plan,
                                &mut current,
                                preempt_kind,
                                None, // RecentTargetFailures lands in 073
                            );
                            // Force GoapPlan removal this tick so
                            // `evaluate_and_plan` (which filters
                            // `Without<GoapPlan>`) picks the cat up next
                            // tick. Without this, a ThreatNearby preempt
                            // sets `Action::Flee` and marks the plan
                            // exhausted, but the cat retains its
                            // GoapPlan. The trip-completion branch then
                            // replans (since trips_done < target_trips),
                            // so `is_exhausted()` flips back to false
                            // and the cat carries the same plan
                            // indefinitely. Action::Flee is set-and-
                            // forget — no resolver releases it — so the
                            // cat freezes in Flee even as hunger
                            // collapses. Witnessed in ticket 038
                            // verification: cats locked in Flee for
                            // 5000+ ticks → starvation deaths despite
                            // ample on-the-ground food.
                            // Ticket 126 — Maslow-driven preempt is
                            // the reconsideration trigger that drops
                            // the held intention; classify as
                            // `Preempted`.
                            plans_to_remove.push((
                                cat_entity,
                                IntentionEnding::Abandoned(
                                    crate::components::IntentionAbandonReason::Preempted,
                                ),
                            ));

                            plan_writer.write(PlanNarrative {
                                entity: cat_entity,
                                kind: plan.kind,
                                event: PlanEvent::Abandoned,
                                completions: plan.trips_done,
                            });

                            preempted = true;
                        }
                    }
                }
                urgencies.needs.clear();

                if preempted {
                    continue;
                }

                plan.advance();
                // Sync CurrentAction targets for the new step.
                if let Some(state) = plan.current_state() {
                    current.target_position = state.target_position;
                    current.target_entity = state.target_entity;
                }
                if let Some(step) = plan.current() {
                    current.action = step.action.to_action(plan.kind, plan.chosen_action);
                }
            }
            crate::steps::StepResult::Fail(ref fail_reason) => {
                if let Some(ref mut log) = ec.event_log {
                    let step_name = plan
                        .current()
                        .map(|s| format!("{:?}", s.action))
                        .unwrap_or_else(|| "none".into());
                    log.push(
                        ec.time.tick,
                        EventKind::PlanStepFailed {
                            cat: name.0.clone(),
                            disposition: format!("{:?}", plan.kind),
                            step: step_name,
                            step_index: plan.current_step,
                            reason: fail_reason.clone(),
                            hunger: needs.hunger,
                            energy: needs.energy,
                            temperature: needs.temperature,
                        },
                    );
                }

                // Ticket 288 — `morale_break` rebinds to commitment
                // release. The substrate's own signal that the cat has
                // lost the will to engage; consequence is "drop the
                // disposition so L3 can re-elect", not "stay inside
                // disposition and replan". Without this branch, a
                // wounded Guarding cat replans inside Guarding to
                // `[TravelTo(PatrolZone), Survey]` and walks back into
                // ambush range (Cedar's death pattern, post-271 soak).
                //
                // No `record_step_failure` call (morale_break is state,
                // not topology — a different cat or the same cat post-
                // heal can engage successfully) and no `failed_action`
                // / `failed_target` passed into `abandon_plan` (no
                // RecentTargetFailures cooldown — see ticket 288 risks).
                //
                // This block mirrors the ReplanCap abandon path below
                // (record_drop + focal capture + PlanNarrative +
                // history + abandon_plan + plans_to_remove); the two
                // share a TODO to extract a `release_commitment`
                // helper alongside the NoPlanPossible path that already
                // duplicates the same gesture.
                // 511 — `starvation_override` rides the same release
                // gesture: a starving body overrides the held
                // commitment (Blind Resting can neither achieve nor
                // drop while hunger prevents rest completion, and the
                // urgency preempt only fires at Advance boundaries —
                // the hunger-wake produces Fail boundaries). Dropping
                // the disposition forces a real election where Eat
                // wins at starvation scores.
                if fail_reason == "morale_break" || fail_reason == "starvation_override" {
                    let strategy = crate::ai::commitment::strategy_for_disposition(plan.kind);
                    crate::ai::commitment::record_drop(
                        narr.activation.as_deref_mut(),
                        strategy,
                        crate::ai::commitment::DropBranch::MoraleBreak,
                    );
                    if ec_is_focal(&ec, cat_entity) {
                        let proxies = crate::ai::commitment::proxies_for_plan(
                            &plan,
                            &needs,
                            &ec.constants.disposition,
                            unexplored_nearby,
                        );
                        crate::ai::commitment::record_commitment_decision(
                            ec.focal_capture.as_deref(),
                            ec.time.tick,
                            &plan,
                            strategy,
                            proxies,
                            true,
                            crate::ai::commitment::DropBranch::MoraleBreak.as_str(),
                        );
                        if let Some(capture) = ec.focal_capture.as_deref() {
                            let step_name = plan
                                .current()
                                .map(|s| format!("{:?}", s.action))
                                .unwrap_or_else(|| "none".into());
                            capture.push_plan_failure(
                                crate::resources::trace_log::PlanFailureCapture {
                                    reason: "morale_break",
                                    disposition: format!("{:?}", plan.kind),
                                    detail: serde_json::json!({
                                        "step": step_name,
                                        "step_index": plan.current_step,
                                    }),
                                },
                                ec.time.tick,
                            );
                        }
                    }
                    plan_writer.write(PlanNarrative {
                        entity: cat_entity,
                        kind: plan.kind,
                        event: PlanEvent::Abandoned,
                        completions: plan.trips_done,
                    });
                    if let Some(mut hist) = history {
                        hist.record(ActionRecord {
                            action: current.action,
                            disposition: Some(plan.kind),
                            tick: ec.time.tick,
                            outcome: ActionOutcome::Failure,
                        });
                    }
                    let _abandoned = crate::systems::plan_substrate::abandon_plan(
                        &mut current,
                        &mut plan,
                        crate::components::AbandonReason::MoraleBreak,
                        None,
                        None,
                        recent_failures.as_deref_mut(),
                        ec.time.tick,
                    );
                    plans_to_remove.push((
                        cat_entity,
                        IntentionEnding::Abandoned(
                            crate::components::IntentionAbandonReason::BecameImpossible,
                        ),
                    ));
                    continue;
                }

                // Record the failed action so replanning can exclude it.
                // Ticket 072: routed through `plan_substrate::record_step_failure`
                // — body is verbatim today; 073 extends it to update
                // `RecentTargetFailures` for cross-plan target memory.
                let failed_action = plan.current().map(|s| s.action);
                let failed_target = plan.current_state().and_then(|s| s.target_entity);
                if let Some(action) = failed_action {
                    // Ticket 073 — lazy-insert `RecentTargetFailures` on
                    // first failure for save-loaded cats that pre-date
                    // the component (the live-spawn bundle adds it for
                    // every new cat). The mutation lands next tick
                    // because Commands buffer until apply; that's
                    // acceptable since the cooldown signal degrades
                    // gracefully (single-tick miss vs the 8000-tick
                    // cooldown window).
                    if recent_failures.is_none() && failed_target.is_some() {
                        commands
                            .entity(cat_entity)
                            .insert(crate::components::RecentTargetFailures::default());
                    }
                    crate::systems::plan_substrate::record_step_failure(
                        &mut plan,
                        action,
                        crate::components::PlanFailureReason::Other,
                        failed_target,
                        recent_failures.as_deref_mut(),
                        ec.time.tick,
                    );
                }

                // Attempt replanning.
                let planner_state = build_planner_state(
                    &pos,
                    &needs,
                    &inventory,
                    plan.trips_done,
                    &ec.map,
                    &snaps.stores_positions,
                    &snaps.construction_positions,
                    &snaps.farm_positions,
                    &snaps.herb_positions,
                    &snaps.material_pile_positions,
                    &snaps.food_pile_positions,
                    d,
                );
                let zone_distances = build_zone_distances(
                    &pos,
                    &ec.map,
                    &snaps.stores_positions,
                    &snaps.construction_positions,
                    &snaps.farm_positions,
                    &snaps.herb_positions,
                    &snaps.kitchen_positions,
                    &snaps.cat_positions,
                    &snaps.material_pile_positions,
                    &snaps.food_pile_positions,
                    &snaps.drying_rack_positions,
                    &snaps.smoking_rack_positions,
                    &snaps.workshop_positions,
                    &snaps.tanning_frame_positions,
                    &snaps.dead_cat_positions,
                    cat_entity,
                    d,
                );
                let mut actions =
                    actions_for_disposition(plan.kind, plan.chosen_action, &zone_distances);
                actions.retain(|a| !plan.failed_actions.contains(&a.kind));
                let plan_ctx = crate::ai::planner::PlanContext {
                    markers: &snaps.planner_markers,
                    entity: cat_entity,
                };
                let goal = goal_for_disposition(plan.kind, plan.trips_done, &plan_ctx);

                if let Ok(new_steps) = make_plan(
                    planner_state,
                    &actions,
                    &goal,
                    12,
                    1000,
                    &plan_ctx,
                    &mut ec.planner_scratch,
                ) {
                    if plan.replan(new_steps) {
                        if let Some(ref mut log) = ec.event_log {
                            log.push(
                                ec.time.tick,
                                EventKind::PlanReplanned {
                                    cat: name.0.clone(),
                                    disposition: format!("{:?}", plan.kind),
                                    replan_count: plan.replan_count,
                                    new_steps: plan
                                        .steps
                                        .iter()
                                        .map(|s| format!("{:?}", s.action))
                                        .collect(),
                                    hunger: needs.hunger,
                                    energy: needs.energy,
                                    temperature: needs.temperature,
                                },
                            );
                        }
                        plan_writer.write(PlanNarrative {
                            entity: cat_entity,
                            kind: plan.kind,
                            event: PlanEvent::Replanned,
                            completions: plan.trips_done,
                        });
                    } else {
                        // Max replans exceeded.
                        // §7.2 `achievable_believed == false` hard-fail
                        // channel. `record_drop` fires the branch-
                        // specific `CommitmentDropReplanCap` counter
                        // alongside the aggregate. Narrative emission
                        // (`PlanEvent::Abandoned`) stays below so the
                        // event log keeps its current shape.
                        let strategy = crate::ai::commitment::strategy_for_disposition(plan.kind);
                        crate::ai::commitment::record_drop(
                            narr.activation.as_deref_mut(),
                            strategy,
                            crate::ai::commitment::DropBranch::ReplanCap,
                        );
                        if ec_is_focal(&ec, cat_entity) {
                            let proxies = crate::ai::commitment::proxies_for_plan(
                                &plan,
                                &needs,
                                &ec.constants.disposition,
                                unexplored_nearby,
                            );
                            crate::ai::commitment::record_commitment_decision(
                                ec.focal_capture.as_deref(),
                                ec.time.tick,
                                &plan,
                                strategy,
                                proxies,
                                true,
                                crate::ai::commitment::DropBranch::ReplanCap.as_str(),
                            );
                            if let Some(capture) = ec.focal_capture.as_deref() {
                                capture.push_plan_failure(
                                    crate::resources::trace_log::PlanFailureCapture {
                                        reason: "replan_cap",
                                        disposition: format!("{:?}", plan.kind),
                                        detail: serde_json::json!({
                                            "replan_count": plan.replan_count,
                                            "max_replans": plan.max_replans,
                                        }),
                                    },
                                    ec.time.tick,
                                );
                            }
                        }
                        plan_writer.write(PlanNarrative {
                            entity: cat_entity,
                            kind: plan.kind,
                            event: PlanEvent::Abandoned,
                            completions: plan.trips_done,
                        });
                        if let Some(mut hist) = history {
                            hist.record(ActionRecord {
                                action: current.action,
                                disposition: Some(plan.kind),
                                tick: ec.time.tick,
                                outcome: ActionOutcome::Failure,
                            });
                        }
                        // Ticket 072: routed through `plan_substrate::abandon_plan`.
                        // The function owns `current.ticks_remaining = 0`; the
                        // caller still pushes onto `plans_to_remove` because that
                        // collection is loop-local and substrate doesn't own it.
                        // Ticket 073 — pass the failed action+target so the
                        // substrate writes them onto `RecentTargetFailures`
                        // before the plan's `failed_actions` set is destroyed.
                        let abandon_action = failed_action;
                        let abandon_target = failed_target;
                        if recent_failures.is_none() && abandon_target.is_some() {
                            commands
                                .entity(cat_entity)
                                .insert(crate::components::RecentTargetFailures::default());
                        }
                        let _abandoned = crate::systems::plan_substrate::abandon_plan(
                            &mut current,
                            &mut plan,
                            crate::components::AbandonReason::ReplanCap,
                            abandon_action,
                            abandon_target,
                            recent_failures.as_deref_mut(),
                            ec.time.tick,
                        );
                        plans_to_remove.push((
                            cat_entity,
                            IntentionEnding::Abandoned(
                                crate::components::IntentionAbandonReason::BecameImpossible,
                            ),
                        ));
                    }
                } else {
                    // No plan possible — abandon.
                    plan_writer.write(PlanNarrative {
                        entity: cat_entity,
                        kind: plan.kind,
                        event: PlanEvent::Abandoned,
                        completions: plan.trips_done,
                    });
                    if let Some(mut hist) = history {
                        hist.record(ActionRecord {
                            action: current.action,
                            disposition: Some(plan.kind),
                            tick: ec.time.tick,
                            outcome: ActionOutcome::Failure,
                        });
                    }
                    // Ticket 072: routed through `plan_substrate::abandon_plan`.
                    // Ticket 073 — same memory-bridge as the ReplanCap branch.
                    let abandon_action = failed_action;
                    let abandon_target = failed_target;
                    if recent_failures.is_none() && abandon_target.is_some() {
                        commands
                            .entity(cat_entity)
                            .insert(crate::components::RecentTargetFailures::default());
                    }
                    let _abandoned = crate::systems::plan_substrate::abandon_plan(
                        &mut current,
                        &mut plan,
                        crate::components::AbandonReason::NoPlanPossible,
                        abandon_action,
                        abandon_target,
                        recent_failures.as_deref_mut(),
                        ec.time.tick,
                    );
                    plans_to_remove.push((
                        cat_entity,
                        IntentionEnding::Abandoned(
                            crate::components::IntentionAbandonReason::BecameImpossible,
                        ),
                    ));
                }
            }
        }
    }

    // Remove completed/abandoned plans.
    // Ticket 126 — `HeldIntention` is removed alongside `GoapPlan`
    // and the lifecycle Feature fires per the recorded ending. The
    // `IntentionAbandoned` activation counter is unparameterised; the
    // per-cause classification rides on the focal-cat trace's
    // `L3Commitment.abandon_reason` field (populated above by
    // `record_drop`'s callers when the §7.2 gate fired). Drop reasons
    // outside the §7.2 gate (Preempted via the anxiety-flee branch,
    // BecameImpossible via the abandon-plan branches) trace through
    // the activation counter only.
    for (entity, ending) in plans_to_remove {
        commands.entity(entity).remove::<GoapPlan>();

        // 364 D1 — advance / backtrack hook. Consult the cat's
        // HeldGoalStack ONLY when the plan that just ended had an
        // HTN-leaf primitive as its last step (Wean / Teach / Release
        // for the kitten arc; Vigil / GriefSit / ReleaseGrief for
        // mourn). The `plan.steps.last()` field reflects the actual
        // current plan structure — including any replan that
        // swapped a Wean plan for a Caretake plan. Replanned plans
        // wouldn't end with the HTN leaf, so this gate keeps them
        // from advancing the frame.
        //
        // On Fulfilled + htn-leaf-last-step: advance the frame
        // (`htn_advance_or_pop`).
        // On Abandoned + htn-leaf-last-step: consult
        // `top.method.failure_strategy` (currently Backtrack ≡
        // Abandon because rear_kitten is the only Live method for
        // "kitten_reared"; sibling-method backtrack lands when a
        // second Live method exists).
        // Otherwise: leave the frame alone via `PreserveStackOnly`
        // (multi-step) or clear it (single-step).
        let stack_now = ec.held_goal_stacks.get(entity).ok().cloned();
        // 364 / 334 — advance the held frame only when the plan that just
        // ended was built FOR the frame's currently-pinned leaf primitive.
        // The plan's `chosen_action` is set to the pinned leaf at frame-pin
        // time (`evaluate_and_plan` ~2896), so comparing it to the frame's
        // current sub-goal action is the exact signal the `MethodRegistry`
        // field's doc-comment describes. A replan rebuilds the plan with a
        // different `chosen_action`, so this naturally excludes the
        // "replanned away from the leaf" case the prior hardcoded
        // GoapActionKind set guarded against — AND it covers the 334 Craft
        // leg (whose terminal `CraftAtWorkshop` is shared with the non-HTN
        // 463 HaveItem path) without a brittle terminal-kind classifier.
        // The plan is still in the world (the remove command above is
        // deferred); a read-only get returns the current chosen action.
        let plan_chosen_action: Option<crate::ai::Action> = cats
            .get(entity)
            .ok()
            .map(|((_, plan, _, _, _, _, _, _, _, _), _)| plan.chosen_action);
        let pinned_primitive_action: Option<crate::ai::Action> = stack_now
            .as_ref()
            .and_then(|s| s.top())
            .filter(|frame| frame.sub_goal_count > 1)
            .and_then(|frame| {
                let method = ec.method_registry.lookup_by_id(frame.method)?;
                match method.sub_goals.get(frame.sub_goal_index)? {
                    crate::ai::methods::SubGoal::Primitive { action, .. } => Some(*action),
                    crate::ai::methods::SubGoal::Goal(_) => None,
                }
            });
        let plan_was_htn_leaf = match (pinned_primitive_action, plan_chosen_action) {
            (Some(pinned), Some(chosen)) => pinned == chosen,
            _ => false,
        };
        let _ = accum; // accumulator unused at this site
        let top_is_multi_step = stack_now
            .as_ref()
            .and_then(|s| s.top())
            .map(|f| f.sub_goal_count > 1)
            .unwrap_or(false);
        let stack_outcome = match stack_now {
            Some(stack) if !stack.is_empty() && plan_was_htn_leaf => match ending {
                IntentionEnding::Fulfilled => htn_advance_or_pop(stack),
                IntentionEnding::Abandoned(_) => htn_abandon_or_pop(stack),
            },
            Some(stack) if !stack.is_empty() && top_is_multi_step => {
                // Multi-step method, plan wasn't an HTN-leaf dispatch
                // (the cat was running a non-leaf plan — e.g., a
                // replan from a failed HTN leaf). Preserve the stack
                // as-is. HeldIntention is still cleared below (the
                // next tick's L2 author rebuilds it from the pinned
                // leaf via the adopt hook).
                StackOutcome::PreserveStackOnly(stack)
            }
            _ => StackOutcome::Done,
        };

        commands
            .entity(entity)
            .remove::<crate::components::HeldIntention>();
        match &stack_outcome {
            StackOutcome::AdvanceTo(updated_stack) => {
                commands.entity(entity).insert(updated_stack.clone());
            }
            StackOutcome::BacktrackTo(updated_stack) => {
                commands.entity(entity).insert(updated_stack.clone());
            }
            StackOutcome::PreserveStackOnly(stack) => {
                // Re-insert to ensure the stack survives any hypothetical
                // future Commands flush race; this is the existing
                // committed state, not a mutation.
                commands.entity(entity).insert(stack.clone());
            }
            StackOutcome::Done => {
                commands
                    .entity(entity)
                    .remove::<crate::components::HeldGoalStack>();
            }
        }

        if let Some(activation) = narr.activation.as_deref_mut() {
            match (&stack_outcome, &ending) {
                (StackOutcome::AdvanceTo(_), _) => {
                    activation.record(Feature::SubGoalAdvanced);
                }
                (StackOutcome::BacktrackTo(_), _) => {
                    activation.record(Feature::MethodBacktracked);
                }
                (StackOutcome::PreserveStackOnly(_), IntentionEnding::Fulfilled) => {
                    activation.record(Feature::IntentionFulfilled);
                }
                (StackOutcome::PreserveStackOnly(_), IntentionEnding::Abandoned(_)) => {
                    activation.record(Feature::IntentionAbandoned);
                }
                (StackOutcome::Done, IntentionEnding::Fulfilled) => {
                    activation.record(Feature::IntentionFulfilled);
                }
                (StackOutcome::Done, IntentionEnding::Abandoned(_)) => {
                    activation.record(Feature::IntentionAbandoned);
                }
            }
        }
    }

    let d = &ec.constants.disposition;

    // Deferred grooming restorations — apply grooming condition delta and
    // §7.W social_warmth delta to the groomed target.
    for groom in accum.grooming_restorations {
        if let Ok((_, (_, _, grooming, _, _, _, _, _, _, fulfillment, _, _, _, _, _))) =
            cats.get_mut(groom.target)
        {
            if let Some(mut g) = grooming {
                g.0 = (g.0 + groom.grooming_delta).min(1.0);
            }
            if let Some(mut f) = fulfillment {
                f.social_warmth = (f.social_warmth + groom.social_warmth_delta).min(1.0);
            }
        }
    }

    // Ticket 177 / §428 / 451: deferred handoffs. The dispatch arm
    // pre-validated the actor side and queued the (actor, recipient)
    // pair.
    //
    // Ticket 451 unified kittens into the cats query (they carry
    // `GoapPlan` post-451). Both adult and kitten recipients now live
    // on `cats`; `cats.get_many_mut([actor, recipient])` grabs both
    // `&mut Inventory` borrows in one call regardless of life stage.
    // The kitten branch retained below as a fallback in case the
    // actor-recipient pair fails the `get_many_mut` (e.g., one entity
    // despawned between dispatch and drain).
    //
    // Pre-§428: the kitten branch silently dropped — the resolver at
    // goap.rs:7322 found a kitten recipient (post the same ticket's
    // R2b snapshot populate), pushed `HandoffPending`, and the drain
    // here Errd on `get_many_mut` because the kitten wasn't in `cats`.
    // 451 retires that asymmetry by including kittens in `cats`.
    //
    // Hunger consumption from the kitten's own Inventory is a separate
    // substrate concern (not in scope here).
    for pending in std::mem::take(&mut accum.handoff_pending) {
        if pending.actor == pending.recipient {
            continue;
        }
        let Ok([mut actor_row, mut recipient_row]) =
            cats.get_many_mut([pending.actor, pending.recipient])
        else {
            // Either entity despawned between dispatch and drain.
            continue;
        };
        let actor_inv: &mut Inventory = &mut actor_row.0 .6;
        let recipient_inv: &mut Inventory = &mut recipient_row.0 .6;
        let outcome =
            crate::steps::disposition::resolve_handoff(actor_inv, pending.recipient, recipient_inv);
        outcome.record_if_witnessed(
            narr.activation.as_deref_mut(),
            crate::resources::system_activation::Feature::ItemHandedOff,
        );
    }

    // §Phase 4c.4 / ticket 451: deferred kitten-feedings. +0.5 hunger
    // per feed. Ticket 451 unified kittens into the cats query (they
    // carry `GoapPlan` post-451), so the +0.5 hunger drain reads
    // `cats.get_mut` directly — no separate `kitten_needs` query
    // anymore. The drain runs post-loop after the mutable iterator
    // over cats has dropped.
    for kitten_entity in accum.kitten_feedings {
        if let Ok(mut row) = cats.get_mut(kitten_entity) {
            let needs: &mut Needs = &mut row.0 .5;
            needs.hunger = (needs.hunger + 0.5).min(1.0);
        }
    }

    // 035: deferred burials. For each completed BuryOutcome:
    //   1. Insert `Buried` on the corpse (defensive against same-tick
    //      double-fire; sensing's `update_target_existence_markers`
    //      filters `(With<Dead>, Without<Buried>)`).
    //   2. Despawn the corpse entity (its grace period collapses to
    //      now — burial is the deliberate, witnessed end of the
    //      grace).
    //   3. Spawn a fresh entity carrying `Grave + Position` at the
    //      corpse's tile. The new entity has no other components, so
    //      it's invisible to every existing per-cat query — it shows
    //      up only via the `GraveAuraMap` rebuild and any future
    //      Grave-aware system (kitten-rest-at-grave, monument
    //      landmarks, etc.).
    for outcome in std::mem::take(&mut accum.bury_completions) {
        commands.entity(outcome.deceased).insert(markers::Buried);
        commands.entity(outcome.deceased).despawn();
        commands.spawn((
            crate::components::grave::Grave {
                deceased_name: outcome.deceased_name,
                tick_buried: outcome.tick,
                cause: outcome.cause,
            },
            outcome.position,
        ));
    }

    // 364: kitten-arc HTN advances. Reads the kitten's current
    // KittenDependency via the disjoint `kitten_parentage` query, then
    // writes the updated component via Commands. The mutation lands at
    // the next command-buffer flush — semantically indistinguishable from
    // immediate mutation since the resolver gates on its own state read
    // and won't re-witness once the new state is observable.
    let weaned_threshold = ec.constants.kitten_rearing.weaned_threshold;
    let teach_done_threshold = ec.constants.kitten_rearing.teach_done_threshold;
    let curriculum_size = ec.constants.kitten_rearing.teach_curriculum_size;
    for advance in std::mem::take(&mut accum.kitten_rearing_advances) {
        let (target, action_tag) = match advance {
            KittenRearingAdvance::Wean(e) => (e, "Wean"),
            KittenRearingAdvance::Teach(e) => (e, "Teach"),
            KittenRearingAdvance::Release(e) => (e, "Release"),
        };
        let Ok((_, dep, _released)) = ec.kitten_parentage.get(target) else {
            // Kitten despawned between the per-cat loop and now (e.g.,
            // death cascade). Skip silently — the HTN frame will abandon
            // via backtrack hook (commit b) on the next plan boundary.
            let _ = action_tag;
            continue;
        };
        match advance {
            KittenRearingAdvance::Wean(_) => {
                commands
                    .entity(target)
                    .insert(crate::components::KittenDependency {
                        mother: dep.mother,
                        father: dep.father,
                        maturity: dep.maturity.max(weaned_threshold),
                        skills_learned: dep.skills_learned,
                    });
            }
            KittenRearingAdvance::Teach(_) => {
                commands
                    .entity(target)
                    .insert(crate::components::KittenDependency {
                        mother: dep.mother,
                        father: dep.father,
                        maturity: dep.maturity.max(teach_done_threshold),
                        skills_learned: dep.skills_learned.saturating_add(1).min(curriculum_size),
                    });
            }
            KittenRearingAdvance::Release(_) => {
                // 395 / R13: Release is symbolic — Feature::KittenReleased
                // was already witnessed in the per-cat dispatch. Author
                // the kitten-side RearKittenReleased ZST so:
                //   (a) the second parent's concurrent frame sees
                //       released_by_arc=true and the picker returns
                //       None → R11 Advance → no double-witness.
                //   (b) update_parent_markers stops re-authoring
                //       HasJuvenileDependent for the near-mature window
                //       — the arc emit shuts off for this kitten.
                // KittenDependency removal stays gated on natural
                // maturity (>= 1.0); the canonical site is
                // `tick_kitten_growth`. This drain branch only fires
                // when picker timing happens to coincide with that
                // boundary; Commands::remove is idempotent with growth's
                // queued remove.
                commands
                    .entity(target)
                    .insert(crate::components::markers::RearKittenReleased);
                if dep.maturity >= 1.0 {
                    commands
                        .entity(target)
                        .remove::<crate::components::KittenDependency>();
                }
            }
        }
    }

    // Deferred mentor effects.
    for effect in &accum.mentor_effects {
        let app_skills_result = if let Ok(s) = unchained_skills.get(effect.apprentice) {
            Some((
                s.hunting,
                s.foraging,
                s.herbcraft,
                s.building,
                s.combat,
                s.magic,
                s.growth_rate(),
            ))
        } else if let Ok(((_, _, _, _, s, _, _, _, _, _), _)) = cats.get(effect.apprentice) {
            Some((
                s.hunting,
                s.foraging,
                s.herbcraft,
                s.building,
                s.combat,
                s.magic,
                s.growth_rate(),
            ))
        } else {
            None
        };
        if let Some((hunt, forage, herb, build, combat, magic, growth_rate)) = app_skills_result {
            let pairs: [(f32, f32); 6] = [
                (effect.mentor_skills.hunting, hunt),
                (effect.mentor_skills.foraging, forage),
                (effect.mentor_skills.herbcraft, herb),
                (effect.mentor_skills.building, build),
                (effect.mentor_skills.combat, combat),
                (effect.mentor_skills.magic, magic),
            ];
            if let Some((idx, _)) = pairs
                .iter()
                .enumerate()
                .filter(|(_, (m, a))| {
                    *m > d.mentor_skill_threshold_high && *a < d.mentor_skill_threshold_low
                })
                .max_by(|(_, (am, aa)), (_, (bm, ba))| {
                    (am - aa)
                        .partial_cmp(&(bm - ba))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            {
                let growth = growth_rate * d.apprentice_skill_growth_multiplier;
                if let Ok(mut s) = unchained_skills.get_mut(effect.apprentice) {
                    match idx {
                        0 => s.hunting += growth,
                        1 => s.foraging += growth,
                        2 => s.herbcraft += growth,
                        3 => s.building += growth,
                        4 => s.combat += growth,
                        5 => s.magic += growth,
                        _ => {}
                    }
                } else if let Ok(((_, _, _, _, mut s, _, _, _, _, _), _)) =
                    cats.get_mut(effect.apprentice)
                {
                    match idx {
                        0 => s.hunting += growth,
                        1 => s.foraging += growth,
                        2 => s.herbcraft += growth,
                        3 => s.building += growth,
                        4 => s.combat += growth,
                        5 => s.magic += growth,
                        _ => {}
                    }
                }
            }
        }
    }
}

// ===========================================================================
// dispatch_step_action — the step-resolution match dispatch, extracted from
// `resolve_goap_plans` to keep both functions under LLVM's optimization-cliff
// threshold. See docs/systems/phase-6a-commitment-gate-attempt.md
// §"LLVM optimization cliff".
// ===========================================================================

#[allow(clippy::too_many_arguments)]
// Prevent LLVM from re-merging this with the caller. The split exists to keep
// both function bodies under LLVM's optimization budget; inlining would undo it.
#[inline(never)]
fn dispatch_step_action(
    action_kind: GoapActionKind,
    step_idx: usize,
    ticks: u64,
    cat_entity: Entity,
    plan: &mut GoapPlan,
    current: &mut CurrentAction,
    pos: &mut Position,
    // 140 step 6 — movement desire for migrated resolvers (TravelTo /
    // FleeTravel); consumed by the Chain-4 integrator.
    desired_velocity: &mut crate::components::physical::DesiredVelocity,
    skills: &mut Skills,
    needs: &mut Needs,
    inventory: &mut Inventory,
    // Ticket 017 — worn equip slots. Read by `resolve_engage_prey`'s
    // weapon-strike bonus; mutated by the craft resolvers to auto-equip a
    // freshly-crafted wearable. The pouch (`inventory`) holds carried items;
    // only worn gear contributes to combat/hunt modifiers.
    wearables: &mut crate::components::equipment::WearableSlots,
    personality: &Personality,
    name: &Name,
    gender: &Gender,
    grooming: Option<&mut crate::components::grooming::GroomingCondition>,
    mood: &mut crate::components::mental::Mood,
    health: &mut Health,
    magic_aff: &MagicAffinity,
    corruption: &mut Corruption,
    memory: &mut Memory,
    fulfillment_opt: &mut Option<Mut<crate::components::fulfillment::Fulfillment>>,
    relationships: &mut Relationships,
    narr: &mut NarrativeEmitter,
    rng: &mut SimRng,
    prey_query: &mut Query<(Entity, &Position, &PreyConfig, &mut PreyState), With<PreyAnimal>>,
    stores_query: &mut Query<&mut StoredItems>,
    items_query: &Query<
        &Item,
        bevy_ecs::query::Without<crate::components::items::BuildMaterialItem>,
    >,
    den_query: &Query<(Entity, &PreyDen, &Position), Without<PreyAnimal>>,
    prey_params: &mut PreyHuntParams,
    commands: &mut Commands,
    ec: &mut ExecutorContext,
    building_params: &mut BuildingResolverParams,
    magic_params: &mut MagicResolverParams,
    snaps: &StepSnapshots,
    accum: &mut StepAccumulators,
    // Ticket 073 — per-cat recently-failed target memory. Threaded
    // through `dispatch_step_action` so the six target-DSE branches
    // can pass the cooldown sensor input into their resolvers.
    recent_failures: Option<&crate::components::RecentTargetFailures>,
    // Ticket 228 — per-cat route-cost field component (inserted at
    // replan by `evaluate_and_plan`). Threaded so the `cat_path_plan!`
    // macro can construct `CatPathPlan::Field` when fresh, falling
    // back to A* via `CatPathPlan::AStarFallback` (and emitting
    // `Feature::RouteCostFieldFallback`) when stale or out-of-budget.
    route_cost_field: Option<&crate::components::RouteCostField>,
    // Ticket 095 Phase 1 Stage B — body-zone substrate. Provides
    // `health_derived` for the 046-Layer-1 carry in EngageThreat.
    body_model: &crate::components::CatBodyModel,
    // Ticket 463 — per-cat ring buffer of recent crafts; the craft
    // caller arms (`CraftAtWorkshop` / `CraftAtTanningFrame`) write
    // the witnessed `RecipeId` here on success so the aspiration's
    // anti-monotony score reads recency next tick. `Option<&mut>`
    // because the component is lazy-inserted on first craft —
    // `None` means "no recent crafts" (the absence is itself the
    // initial state).
    recent_crafts: Option<&mut crate::components::recent_crafts::CatRecentCrafts>,
) -> crate::steps::StepResult {
    let d = &ec.constants.disposition;

    // Ticket 228 — cat-side path-plan constructor. Returns a
    // `CatPathPlan` per call: `Field(&route_cost_field)` when the
    // per-cat replan-time flood is fresh and reaches `$to`, otherwise
    // `AStarFallback { fox, corr, weight }` (which emits
    // `Feature::RouteCostFieldFallback` for the canary). The
    // arm-scope construction is what unblocks the immutable borrow on
    // `prey_params.fox_scent_map` from conflicting with `&mut
    // prey_params` borrows in the SearchPrey / EngagePrey arms.
    // Substrate, not search state (§4.7). Subsumes the retired
    // ticket-223 `cat_overlays_pair!` macro.
    macro_rules! cat_path_plan {
        ($to:expr) => {{
            let __fox = crate::ai::pathfinding::FoxScentOverlay::new(
                &prey_params.fox_scent_map,
                &ec.constants.scoring,
            );
            let __corr =
                crate::ai::pathfinding::CorruptionOverlay::new(&ec.map, &ec.constants.scoring);
            let __weight =
                crate::ai::pathfinding::cat_path_weight_from_boldness(personality.boldness);
            // 508 — the routing cat's own threat beliefs price
            // witnessed-ambush ground into the fallback route.
            let __threat = ec.location_beliefs.get(cat_entity).ok().map(|lb| {
                crate::ai::pathfinding::ThreatBeliefOverlay::new(lb, &ec.constants.scoring)
            });
            let __current_tick = ec.time.tick;
            let __window = ec.constants.scoring.route_cost_replan_window_ticks;
            match route_cost_field {
                Some(__field)
                    if !crate::ai::route_cost::CatPathPlan::should_fall_back_at(
                        __field,
                        $to,
                        __current_tick,
                        __window,
                    ) =>
                {
                    crate::ai::route_cost::CatPathPlan::Field {
                        field: __field,
                        fox: Some(__fox),
                        corr: Some(__corr),
                        threat: __threat,
                        weight: __weight,
                    }
                }
                _ => {
                    if let Some(__act) = narr.activation.as_deref_mut() {
                        __act.record(
                            crate::resources::system_activation::Feature::RouteCostFieldFallback,
                        );
                    }
                    crate::ai::route_cost::CatPathPlan::AStarFallback {
                        fox: __fox,
                        corr: __corr,
                        threat: __threat,
                        weight: __weight,
                    }
                }
            }
        }};
    }

    // Ticket 074 — runtime guard for audit gap #4. Validate the
    // step's `target_entity` at entry so a plan that committed to a
    // since-dead/banished/incapacitated/despawned entity fails fast
    // (rather than the resolver's `if let Some(target)` block silently
    // running on a stale ID, or `TravelTo(target)` re-pathfinding to
    // empty space tick after tick). The IAUS-time
    // `EligibilityFilter::require_alive` gate catches the *new-plan*
    // case (cat picking a stale target); this catches the *mid-plan*
    // case (target died after the plan committed). Belt-and-suspenders.
    //
    // Resolver bodies remain unchanged — the contract is "step
    // resolvers run only with valid targets"; the gate enforces it.
    //
    // 035 carve-out: `Bury` is the inverted case — its target *must*
    // be Dead. Skipping the alive-gate for Bury (and the prefix
    // `TravelTo(CorpseTarget)` which carries the same target_entity
    // forward via `carry_target_forward`) lets the burial chain run
    // against the dead colony-mate without the gate rejecting it.
    // The Bury dispatch arm has its own validity check via the
    // `dead_cat_positions` snapshot lookup — a target that's been
    // despawned (cleanup_dead) or buried since the plan committed
    // returns None for `target_position`/`target_name`/`target_cause`
    // and the resolver Advances unwitnessed, mirroring grooming's
    // missing-target degradation.
    let skip_alive_gate = matches!(
        action_kind,
        GoapActionKind::Bury | GoapActionKind::TravelTo(PlannerZone::CorpseTarget)
    );
    if !skip_alive_gate {
        if let Some(target) = plan.step_state[step_idx].target_entity {
            // 487 follow-on — `FeedKitten` targets newborn kittens
            // who are themselves `Incapacitated` by design
            // (incapacitation.rs ORs `Has<NewbornKitten>` into the
            // marker). Without this carve-out the generic alive-gate
            // rejects the very target the step exists to serve and
            // surfaces 100+ false-positive `PlanStepFailed` events
            // per 5-min soak.
            let permit_incapacitated_newborn = matches!(action_kind, GoapActionKind::FeedKitten);
            if let Err(reason) = crate::systems::plan_substrate::validate_target_for_step(
                target,
                permit_incapacitated_newborn,
                &ec.target_validity,
            ) {
                // Failure name encodes the invalidity flavor for the
                // narrative trace; the existing `PlanFailureReason::TargetDespawned`
                // path consumes the failure regardless of subkind.
                return crate::steps::StepResult::Fail(format!(
                    "target invalid at step entry: {reason:?}"
                ));
            }
        }
    }

    match action_kind {
        GoapActionKind::TravelTo(zone) => {
            // Ticket 228 — `cat_path_plan!` resolves to `Field` (gradient-
            // walk) when the per-cat RouteCostField reaches the resolved
            // zone target, otherwise `AStarFallback` (which records
            // `Feature::RouteCostFieldFallback` for the staleness canary).
            // The destination passed to the macro is the zone-resolved
            // target where known; on the first dispatch tick the target
            // is `None`, so we fall back to A* via the cat's current
            // position as the staleness probe (a position the field
            // always reaches at cost 0).
            let target_for_plan = plan.step_state[step_idx].target_position.unwrap_or(*pos);
            let path_plan = cat_path_plan!(target_for_plan);
            resolve_travel_to(
                zone,
                &mut plan.step_state[step_idx],
                pos,
                &ec.map,
                &path_plan,
                &prey_params.exploration_map,
                &snaps.stores_positions,
                &snaps.construction_positions,
                &snaps.farm_positions,
                &snaps.herb_positions,
                &snaps.kitchen_positions,
                &snaps.cat_positions,
                &snaps.material_pile_positions,
                &snaps.food_pile_positions,
                &snaps.drying_rack_positions,
                &snaps.smoking_rack_positions,
                &snaps.workshop_positions,
                &snaps.tanning_frame_positions,
                &snaps.dead_cat_positions,
                cat_entity,
                d,
                desired_velocity,
                &ec.constants.movement,
            )
        }

        GoapActionKind::SearchPrey => {
            // Ticket 427 Step 1 — capture only `&ec.faction_overlay_q`
            // for the stance closure so `&mut ec.dse_scratchpad` is a
            // disjoint field borrow at the same call site.
            let faction_overlay_q = &ec.faction_overlay_q;
            let is_focal = ec_is_focal(ec, cat_entity);
            // 293: thread the cat's own `LocationBeliefs` for the
            // per-cat `best_prey_direction` lookup inside the search
            // step. The query lives on the goap system's separate
            // `location_beliefs` field; lookup is `O(1)`.
            let cat_loc_beliefs = ec.location_beliefs.get(cat_entity).ok();
            resolve_search_prey(
                &mut plan.step_state[step_idx],
                ticks,
                pos,
                cat_loc_beliefs,
                prey_query,
                den_query,
                inventory,
                skills,
                prey_params,
                &ec.map,
                &ec.wind,
                narr,
                &ec.time,
                rng,
                commands,
                cat_entity,
                personality,
                name,
                gender,
                needs,
                d,
                &ec.constants.sensory.cat,
                &ec.dse_registry,
                &ec.faction_relations,
                &|e: Entity| stance_overlays_from_query(faction_overlay_q, e),
                is_focal,
                ec.focal_capture.as_deref(),
                recent_failures,
                ec.constants
                    .planning_substrate
                    .target_failure_cooldown_ticks,
                &ec.action_affordances,
                &mut ec.dse_scratchpad,
            )
        }

        GoapActionKind::EngagePrey => {
            // Get prey target from previous SearchPrey step's state, or from
            // our own state (set during replan).
            // Ticket 072: routed through `plan_substrate::carry_target_forward`.
            // Ticket 074: the validity check inside `carry_target_forward`
            // now drops dead/banished/incapacitated/despawned prior
            // targets so the EngagePrey step doesn't engage a stale
            // entity reference. The substrate's `None` return surfaces
            // through the caller's existing `PlanStepFailed` path.
            let _carried = crate::systems::plan_substrate::carry_target_forward(
                &mut plan.step_state,
                step_idx,
                &ec.target_validity,
                None, // RecentTargetFailures lands in 073
            );
            // 477 — focal-cat resolver-trace sink for the equipment
            // weapon-strike read. Built from `ec`'s focal-trace resources
            // (headless-only); `None` in interactive runs / non-focal cats.
            let engage_focal_sink = crate::resources::trace_log::FocalResolverSink::new(
                ec.focal_capture.as_deref(),
                ec.focal_target.as_deref(),
                ec.time.tick,
            );
            resolve_engage_prey(
                &mut plan.step_state[step_idx],
                ticks,
                pos,
                inventory,
                wearables,
                skills,
                prey_query,
                prey_params,
                &ec.map,
                &ec.constants.scoring,
                narr,
                &ec.time,
                rng,
                commands,
                cat_entity,
                personality,
                name,
                gender,
                needs,
                d,
                ec.event_log.as_deref_mut(),
                // 263: ActionAffordances borrow for the C5 stalk-start
                // phase-band bias. Dormant by default; reads only fire
                // when the bias knob is non-zero.
                &ec.action_affordances,
                // 375: per-species guaranteed byproduct table.
                &ec.constants.prey_byproducts,
                // 100: ambient opportunity-quality reads + Stalk/Pounce
                // stamping. The cat's `CurrentAction` is mutated on
                // phase entry so `tremor_tick` (next tick) reads the
                // correct emission multiplier.
                &ec.constants.sensory,
                ec.location_beliefs.get(cat_entity).ok(),
                current,
                // 477: combat constants + focal sink for the equipment
                // weapon-strike bonus + bone-snap canary.
                &ec.constants.combat,
                engage_focal_sink.as_ref(),
            )
        }

        GoapActionKind::DepositPrey
        | GoapActionKind::DepositFood
        | GoapActionKind::DepositCookedFood => {
            // Resolve nearest store as target.
            if plan.step_state[step_idx].target_entity.is_none() {
                plan.step_state[step_idx].target_entity = snaps
                    .stores_entities
                    .iter()
                    .min_by_key(|(_, sp)| pos.tile_distance_squared(sp))
                    .map(|(e, _)| *e);
            }
            let deposit = crate::steps::disposition::resolve_deposit_at_stores(
                plan.step_state[step_idx].target_entity,
                inventory,
                skills,
                pos,
                stores_query,
                items_query,
                commands,
                d,
            );
            if deposit.storage_upgraded {
                if let Some(ref mut act) = narr.activation {
                    act.record(Feature::StorageUpgraded);
                }
            }
            if deposit.rejected {
                if let Some(ref mut act) = narr.activation {
                    act.record(Feature::DepositRejected);
                }
            }
            if deposit.no_store {
                if let Some(ref mut act) = narr.activation {
                    act.record(Feature::DepositFailedNoStore);
                }
            }
            deposit.step
        }

        GoapActionKind::ForageItem => resolve_forage_item(
            &mut plan.step_state[step_idx],
            ticks,
            pos,
            inventory,
            skills,
            &ec.map,
            narr,
            &ec.time,
            rng,
            personality,
            name,
            gender,
            needs,
            d,
            commands,
        ),

        GoapActionKind::EatAtStores => {
            if plan.step_state[step_idx].target_entity.is_none() {
                plan.step_state[step_idx].target_entity = snaps
                    .stores_entities
                    .iter()
                    .min_by_key(|(_, sp)| pos.tile_distance_squared(sp))
                    .map(|(e, _)| *e);
            }
            let outcome = crate::steps::disposition::resolve_eat_at_stores(
                ticks,
                plan.step_state[step_idx].target_entity,
                needs,
                Some(mood),
                stores_query,
                items_query,
                commands,
                d,
                &ec.constants.crafting,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::FoodEaten);
            outcome.result
        }

        GoapActionKind::Sleep => {
            let duration = d.sleep_duration_base
                + ((1.0 - needs.energy) * d.sleep_duration_deficit_multiplier) as u64;
            // Corruption degrades rest quality.
            let tile_corruption = if ec.map.in_bounds(pos.x(), pos.y()) {
                ec.map.get(pos.x(), pos.y()).corruption
            } else {
                0.0
            };
            let outcome = crate::steps::disposition::resolve_sleep(
                ticks,
                duration,
                needs,
                memory,
                pos,
                ec.time.tick,
                d,
            );
            if tile_corruption > 0.0 {
                let penalty = tile_corruption * (1.0 - ec.constants.magic.corruption_rest_penalty);
                needs.energy = (needs.energy - d.sleep_energy_per_tick * penalty).max(0.0);
            }
            outcome.result
        }

        GoapActionKind::SelfGroom => {
            let outcome = crate::steps::disposition::resolve_self_groom(ticks, needs, grooming, d);
            if matches!(outcome.result, crate::steps::StepResult::Advance) {
                if let Some(ref mut log) = ec.event_log {
                    log.push(
                        ec.time.tick,
                        EventKind::GroomingFired {
                            cat: name.0.clone(),
                            target: None,
                        },
                    );
                }
            }
            outcome.result
        }

        GoapActionKind::SocializeWith => {
            // Resolve social target on first tick via the §6.5.1
            // target-taking DSE. Phase 4c.1: replaces
            // `find_social_target` (fondness-only) with the single
            // source of truth `resolve_socialize_target` — closes
            // the §6.2 silent-divergence gap with
            // `disposition.rs::build_socializing_chain`.
            if plan.step_state[step_idx].target_entity.is_none() {
                // §11 focal-cat hook: emits per-candidate ranking
                // into `FocalScoreCapture` on the focal cat's
                // turn, so the socialize_target L2 record
                // carries a `targets` block with every
                // candidate's score + the winner. Non-focal
                // cats pass `None` and pay zero cost.
                let focal_hook = if ec_is_focal(ec, cat_entity) {
                    ec.focal_capture
                        .as_deref()
                        .map(|cap| crate::ai::target_dse::FocalTargetHook {
                            capture: cap,
                            // `Entity::Debug` is the cheapest stable
                            // label; name resolution would need a
                            // snapshot this system doesn't carry.
                            // Trace tooling can join against
                            // events.jsonl on the same Entity id.
                            name_lookup: &|e: Entity| format!("{e:?}"),
                        })
                } else {
                    None
                };
                // Ticket 427 Step 1 — capture only `&ec.faction_overlay_q`
                // (not `&ec`) so `&mut ec.dse_scratchpad` further down
                // is a disjoint field borrow.
                let faction_overlay_q = &ec.faction_overlay_q;
                let stance_overlays = |e: Entity| stance_overlays_from_query(faction_overlay_q, e);
                // Ticket 027b §7.M / 127 — look up the L2
                // JointIntention partner (Courtship practice) so
                // `socialize_target::bond_score` can pin the Intention
                // partner at 1.0 regardless of bond tier. Filtered on
                // `practice == Courtship` so the snapshot is single-
                // source-of-truth for the Courtship-vs-other-practices
                // distinction. Falls back to `None` for cats without
                // a Courtship JI.
                let joint_partner = ec
                    .joint_q
                    .get(cat_entity)
                    .ok()
                    .filter(|j| {
                        j.practice == crate::components::joint_intention::PracticeKind::Courtship
                    })
                    .map(|j| j.partner);
                plan.step_state[step_idx].target_entity =
                    crate::ai::dses::socialize_target::resolve_socialize_target(
                        &ec.dse_registry,
                        cat_entity,
                        *pos,
                        &snaps.cat_positions,
                        relationships,
                        &ec.faction_relations,
                        &stance_overlays,
                        ec.time.tick,
                        focal_hook,
                        joint_partner,
                        recent_failures,
                        ec.constants
                            .planning_substrate
                            .target_failure_cooldown_ticks,
                        narr.activation.as_deref_mut(),
                        &mut ec.dse_scratchpad,
                    );
            }
            // §7.W: construct a temporary Fulfillment for cats without the
            // component (save-loaded before §7.W). The write-back is a no-op
            // for those cats — only the inflow matters for the witness.
            let mut fallback_fulfillment = crate::components::fulfillment::Fulfillment::default();
            let fulfillment_ref = match fulfillment_opt.as_mut() {
                Some(f) => &mut **f,
                None => &mut fallback_fulfillment,
            };
            // 257 / Commit B — pairing-bias multiplier when the social
            // target equals the actor's PairingActivity.partner. The
            // helper returns `(1.0, false)` when no Intention is held
            // or the target differs.
            // Ticket 127 — switched from PairingActivity to
            // JointIntention { practice: Courtship }. Snapshot is
            // pre-filtered to Courtship at the .filter() call so the
            // single-practice pattern stays trivially identical to
            // the prior pairing_bias_for shape.
            let (pairing_bias, amplified) = crate::components::joint_intention::joint_bias_for(
                ec.joint_q
                    .get(cat_entity)
                    .ok()
                    .filter(|j| {
                        j.practice == crate::components::joint_intention::PracticeKind::Courtship
                    })
                    .map(|j| j.partner),
                plan.step_state[step_idx].target_entity,
                ec.constants.practices.courtship.bias_multiplier,
            );
            if amplified {
                if let Some(act) = narr.activation.as_deref_mut() {
                    act.record(Feature::JointBiasApplied {
                        practice: crate::components::joint_intention::PracticeKind::Courtship,
                    });
                }
                // Ticket 127 — feed `last_interaction_tick` so the
                // Approach→Courting stage advance fires once any
                // paired-resolver interaction has occurred.
                if let Some(partner) = plan.step_state[step_idx].target_entity {
                    narr.joint_interaction.write(
                        crate::ai::joint_intention::JointInteractionObserved {
                            entity: cat_entity,
                            partner,
                            practice: crate::components::joint_intention::PracticeKind::Courtship,
                            tick: ec.time.tick,
                        },
                    );
                }
            }
            // 368: Phase 2 — PlayBundle presence on the actor scales
            // social fondness gain. Target-side check is a follow-on
            // (would require querying the target's `Inventory`); the
            // actor-side check is sufficient for first-light canary
            // emission. Caller emits `Feature::PlayBundleEngaged` on
            // a witnessed Advance when the multiplier was > 1.0.
            let bundle_multiplier =
                if inventory.has_item(crate::components::items::ItemKind::PlayBundle) {
                    ec.constants.crafting.play_bundle_social_multiplier
                } else {
                    1.0
                };
            let outcome = crate::steps::disposition::resolve_socialize(
                ticks,
                cat_entity,
                plan.step_state[step_idx].target_entity,
                needs,
                fulfillment_ref,
                relationships,
                &snaps.grooming,
                ec.time.tick,
                &ec.constants.social,
                d,
                &ec.constants.fulfillment,
                pairing_bias,
                bundle_multiplier,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::Socialized);
            if bundle_multiplier > 1.0 && outcome.witness {
                if let Some(act) = narr.activation.as_deref_mut() {
                    act.record(Feature::PlayBundleEngaged);
                }
            }
            if matches!(outcome.result, crate::steps::StepResult::Advance) {
                magic_params
                    .pushback_writer
                    .write(crate::systems::magic::CorruptionPushback {
                        position: *pos,
                        radius: 2.0,
                        amount: 0.01,
                    });
            }
            outcome.result
        }

        GoapActionKind::GroomOther => {
            // §6.5.4: replace the fondness-only `find_social_target`
            // picker with the warmth-/kinship-/adjacency-ranked
            // groom-other target DSE. Closes the silent divergence
            // with disposition.rs's sub-action pick and retires
            // `find_social_target` (GroomOther was the last caller
            // after the Socialize / Mate / Mentor ports).
            if plan.step_state[step_idx].target_entity.is_none() {
                let temperature_lookup =
                    |e: Entity| -> Option<f32> { snaps.cat_temperature.get(&e).copied() };
                let grooming_lookup =
                    |e: Entity| -> Option<f32> { snaps.cat_grooming.get(&e).copied() };
                let is_kin = |a: Entity, b: Entity| -> bool {
                    let a_parents = snaps.kitten_parents.get(&a);
                    let b_parents = snaps.kitten_parents.get(&b);
                    a_parents.is_some_and(|(m, f)| *m == Some(b) || *f == Some(b))
                        || b_parents.is_some_and(|(m, f)| *m == Some(a) || *f == Some(a))
                };
                // §11 focal-cat hook: mirror socialize/goap.rs:~2557.
                let focal_hook = if ec_is_focal(ec, cat_entity) {
                    ec.focal_capture
                        .as_deref()
                        .map(|cap| crate::ai::target_dse::FocalTargetHook {
                            capture: cap,
                            name_lookup: &|e: Entity| format!("{e:?}"),
                        })
                } else {
                    None
                };
                plan.step_state[step_idx].target_entity =
                    crate::ai::dses::groom_other_target::resolve_groom_other_target(
                        &ec.dse_registry,
                        cat_entity,
                        *pos,
                        &snaps.cat_positions,
                        &temperature_lookup,
                        &grooming_lookup,
                        &is_kin,
                        relationships,
                        ec.time.tick,
                        focal_hook,
                        recent_failures,
                        ec.constants
                            .planning_substrate
                            .target_failure_cooldown_ticks,
                        narr.activation.as_deref_mut(),
                        &mut ec.dse_scratchpad,
                        Some(&snaps.currently_groomed),
                    );
            }
            // §7.W: construct a temporary Fulfillment for cats without the
            // component (save-loaded before §7.W). The write-back is a no-op
            // for those cats — only the inflow matters for the witness.
            let mut fallback_fulfillment = crate::components::fulfillment::Fulfillment::default();
            let fulfillment_ref = match fulfillment_opt.as_mut() {
                Some(f) => &mut **f,
                None => &mut fallback_fulfillment,
            };
            // Ticket 127 — switched from PairingActivity to
            // JointIntention { practice: Courtship }. Snapshot is
            // pre-filtered to Courtship at the .filter() call so the
            // single-practice pattern stays trivially identical to
            // the prior pairing_bias_for shape.
            let (pairing_bias, amplified) = crate::components::joint_intention::joint_bias_for(
                ec.joint_q
                    .get(cat_entity)
                    .ok()
                    .filter(|j| {
                        j.practice == crate::components::joint_intention::PracticeKind::Courtship
                    })
                    .map(|j| j.partner),
                plan.step_state[step_idx].target_entity,
                ec.constants.practices.courtship.bias_multiplier,
            );
            if amplified {
                if let Some(act) = narr.activation.as_deref_mut() {
                    act.record(Feature::JointBiasApplied {
                        practice: crate::components::joint_intention::PracticeKind::Courtship,
                    });
                }
                // Ticket 127 — feed `last_interaction_tick` so the
                // Approach→Courting stage advance fires once any
                // paired-resolver interaction has occurred.
                if let Some(partner) = plan.step_state[step_idx].target_entity {
                    narr.joint_interaction.write(
                        crate::ai::joint_intention::JointInteractionObserved {
                            entity: cat_entity,
                            partner,
                            practice: crate::components::joint_intention::PracticeKind::Courtship,
                            tick: ec.time.tick,
                        },
                    );
                }
            }
            // 368: Phase 2 — GroomingBrush presence scales the
            // fondness delta. Caller emits `Feature::GroomingBrushUsed`
            // on a witnessed Advance when the multiplier was > 1.0.
            let brush_multiplier =
                if inventory.has_item(crate::components::items::ItemKind::GroomingBrush) {
                    ec.constants.crafting.groom_brush_fondness_multiplier
                } else {
                    1.0
                };
            let outcome = crate::steps::disposition::resolve_groom_other(
                ticks,
                cat_entity,
                plan.step_state[step_idx].target_entity,
                needs,
                fulfillment_ref,
                relationships,
                &snaps.grooming,
                ec.time.tick,
                &ec.constants.social,
                d,
                &ec.constants.fulfillment,
                pairing_bias,
                brush_multiplier,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::GroomedOther);
            if brush_multiplier > 1.0 && matches!(outcome.result, crate::steps::StepResult::Advance)
            {
                if let Some(act) = narr.activation.as_deref_mut() {
                    act.record(Feature::GroomingBrushUsed);
                }
            }
            if let Some(r) = outcome.witness {
                // 258 — observable side-effect for the belief substrate.
                // `belief_integrator` updates witnesses' affiliation_history
                // facet on the actor. Emitted only on the witnessed-Advance
                // branch so a stalled GroomOther step doesn't spuriously
                // signal a completed grooming.
                narr.witnessable.write(
                    crate::messages::witnessable_event::WitnessableEvent::Groom {
                        actor: cat_entity,
                        target: r.target,
                        position: *pos,
                        tick: ec.time.tick,
                    },
                );
                accum.grooming_restorations.push(r);
            }
            if matches!(outcome.result, crate::steps::StepResult::Advance) {
                if let Some(ref mut log) = ec.event_log {
                    log.push(
                        ec.time.tick,
                        EventKind::GroomingFired {
                            cat: name.0.clone(),
                            target: plan.step_state[step_idx]
                                .target_entity
                                .map(|e| format!("entity:{}", e.index())),
                        },
                    );
                }
            }
            outcome.result
        }

        GoapActionKind::MentorCat => {
            if plan.step_state[step_idx].target_entity.is_none() {
                // §6.5.3: replace the fondness-only `find_social_target`
                // picker with the skill-gap-ranked mentor target DSE.
                // Closes the silent-divergence with disposition.rs's
                // sub-action pick and the §6.1-Critical skill-gap gap.
                let skills_lookup =
                    |e: Entity| -> Option<Skills> { snaps.cat_skills.get(&e).cloned() };
                // §11 focal-cat hook: mirror socialize/goap.rs:~2557.
                let focal_hook = if ec_is_focal(ec, cat_entity) {
                    ec.focal_capture
                        .as_deref()
                        .map(|cap| crate::ai::target_dse::FocalTargetHook {
                            capture: cap,
                            name_lookup: &|e: Entity| format!("{e:?}"),
                        })
                } else {
                    None
                };
                plan.step_state[step_idx].target_entity =
                    crate::ai::dses::mentor_target::resolve_mentor_target(
                        &ec.dse_registry,
                        cat_entity,
                        *pos,
                        &snaps.cat_positions,
                        skills,
                        &skills_lookup,
                        relationships,
                        ec.time.tick,
                        focal_hook,
                        recent_failures,
                        ec.constants
                            .planning_substrate
                            .target_failure_cooldown_ticks,
                        narr.activation.as_deref_mut(),
                        &mut ec.dse_scratchpad,
                    );
            }
            // Ticket 127 — switched from PairingActivity to
            // JointIntention { practice: Courtship }. Snapshot is
            // pre-filtered to Courtship at the .filter() call so the
            // single-practice pattern stays trivially identical to
            // the prior pairing_bias_for shape.
            let (pairing_bias, amplified) = crate::components::joint_intention::joint_bias_for(
                ec.joint_q
                    .get(cat_entity)
                    .ok()
                    .filter(|j| {
                        j.practice == crate::components::joint_intention::PracticeKind::Courtship
                    })
                    .map(|j| j.partner),
                plan.step_state[step_idx].target_entity,
                ec.constants.practices.courtship.bias_multiplier,
            );
            if amplified {
                if let Some(act) = narr.activation.as_deref_mut() {
                    act.record(Feature::JointBiasApplied {
                        practice: crate::components::joint_intention::PracticeKind::Courtship,
                    });
                }
                // Ticket 127 — feed `last_interaction_tick` so the
                // Approach→Courting stage advance fires once any
                // paired-resolver interaction has occurred.
                if let Some(partner) = plan.step_state[step_idx].target_entity {
                    narr.joint_interaction.write(
                        crate::ai::joint_intention::JointInteractionObserved {
                            entity: cat_entity,
                            partner,
                            practice: crate::components::joint_intention::PracticeKind::Courtship,
                            tick: ec.time.tick,
                        },
                    );
                }
            }
            let outcome = crate::steps::disposition::resolve_mentor_cat(
                ticks,
                cat_entity,
                plan.step_state[step_idx].target_entity,
                needs,
                skills,
                relationships,
                ec.time.tick,
                d,
                pairing_bias,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::MentoredCat);
            let crate::steps::StepOutcome { result, witness } = outcome;
            if let Some((apprentice, mentor_skills)) = witness {
                accum.mentor_effects.push(MentorEffect {
                    apprentice,
                    mentor_skills,
                });
            }
            if matches!(result, crate::steps::StepResult::Advance) {
                if let Some(ref mut log) = ec.event_log {
                    log.push(
                        ec.time.tick,
                        EventKind::MentoringFired {
                            mentor: name.0.clone(),
                            apprentice: plan.step_state[step_idx]
                                .target_entity
                                .map(|e| format!("entity:{}", e.index()))
                                .unwrap_or_else(|| "unknown".into()),
                        },
                    );
                }
            }
            result
        }

        GoapActionKind::Bury => {
            // 035: pick the corpse via the bury target DSE if not yet
            // selected, then run the bury resolver. On completion the
            // post-loop drain inserts `Buried` on the corpse, despawns
            // it, and spawns a `Grave` entity at the same position.
            if plan.step_state[step_idx].target_entity.is_none() {
                let is_kin = |a: Entity, b: Entity| -> bool {
                    let a_parents = snaps.kitten_parents.get(&a);
                    let b_parents = snaps.kitten_parents.get(&b);
                    a_parents.is_some_and(|(m, f)| *m == Some(b) || *f == Some(b))
                        || b_parents.is_some_and(|(m, f)| *m == Some(a) || *f == Some(a))
                };
                let focal_hook = if ec_is_focal(ec, cat_entity) {
                    ec.focal_capture
                        .as_deref()
                        .map(|cap| crate::ai::target_dse::FocalTargetHook {
                            capture: cap,
                            name_lookup: &|e: Entity| format!("{e:?}"),
                        })
                } else {
                    None
                };
                plan.step_state[step_idx].target_entity =
                    crate::ai::dses::bury_target::resolve_bury_target(
                        &ec.dse_registry,
                        cat_entity,
                        *pos,
                        &snaps.dead_cat_positions,
                        &is_kin,
                        relationships,
                        ec.time.tick,
                        focal_hook,
                        recent_failures,
                        ec.constants
                            .planning_substrate
                            .target_failure_cooldown_ticks,
                        narr.activation.as_deref_mut(),
                        &mut ec.dse_scratchpad,
                    );
            }
            // Stash the deceased's position + name + cause so the
            // post-loop drain can spawn the Grave even after the corpse
            // entity is despawned.
            let target = plan.step_state[step_idx].target_entity;
            let (target_position, target_name, target_cause) = match target {
                Some(t) => {
                    let pos_opt = snaps
                        .dead_cat_positions
                        .iter()
                        .find(|(e, _)| *e == t)
                        .map(|(_, p)| *p);
                    let name_opt = snaps.dead_cat_names.get(&t).cloned();
                    let cause_opt = ec.dead_cats_q.get(t).ok().map(|(_, _, _, dead)| dead.cause);
                    (pos_opt, name_opt, cause_opt)
                }
                None => (None, None, None),
            };
            // §7.W: construct a temporary Fulfillment for cats without
            // the component (mirrors GroomOther dispatch).
            let mut fallback_fulfillment = crate::components::fulfillment::Fulfillment::default();
            let fulfillment_ref = match fulfillment_opt.as_mut() {
                Some(f) => &mut **f,
                None => &mut fallback_fulfillment,
            };
            let outcome = crate::steps::disposition::resolve_bury(
                ticks,
                target,
                target_position,
                target_name.clone(),
                target_cause,
                fulfillment_ref,
                ec.time.tick,
                d,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::BurialPerformed);
            if matches!(outcome.result, crate::steps::StepResult::Advance) {
                if let Some(ref mut log) = ec.event_log {
                    let deceased_label = target_name
                        .clone()
                        .or_else(|| target.map(|e| format!("entity:{}", e.index())))
                        .unwrap_or_else(|| "unknown".into());
                    log.push(
                        ec.time.tick,
                        EventKind::BurialFired {
                            cat: name.0.clone(),
                            deceased: deceased_label,
                        },
                    );
                }
            }
            if let Some(o) = outcome.witness {
                accum.bury_completions.push(o);
            }
            outcome.result
        }

        GoapActionKind::PatrolArea => {
            if plan.step_state[step_idx].target_position.is_none() {
                plan.step_state[step_idx].target_position = find_random_nearby_tile(
                    pos,
                    &ec.map,
                    d.guard_patrol_radius as i32,
                    |t| t.is_passable(),
                    &mut rng.rng,
                );
            }
            // Ticket 228 — patrol target is the heart of the 209/223/224
            // ticket cluster (fox-territory suppression at decision time).
            // Falling back to *pos as the staleness probe when the target
            // hasn't been resolved yet keeps `should_fall_back_at` well-
            // defined; the resolver itself fails fast on `None` target.
            let target_pos = plan.step_state[step_idx].target_position.unwrap_or(*pos);
            let path_plan = cat_path_plan!(target_pos);
            crate::steps::disposition::resolve_patrol_to(
                pos,
                plan.step_state[step_idx].target_position,
                &mut plan.step_state[step_idx].cached_path,
                needs,
                &ec.map,
                &path_plan,
                d,
                desired_velocity,
                &ec.constants.movement,
            )
            .result
        }

        GoapActionKind::EngageThreat => {
            // §6.5.9: resolve the threat target via the fight-target
            // DSE. Replaces the pre-refactor nearest-wildlife pick
            // with a weighted (distance, threat-level, combat-
            // advantage, ally-proximity) ranking. The coordinator
            // Fight-directive path upstream still seeds
            // `target_entity` before this branch runs, so posse
            // cohesion is unaffected — this picker only fires for
            // un-directed EngageThreat steps.
            // step_state.target_entity is copied into CurrentAction.target_entity
            // only at ticks_elapsed == 0 (before dispatch), so we must also write
            // current.target_entity directly here for resolve_combat to pick it up.
            if plan.step_state[step_idx].target_entity.is_none() {
                let candidates: Vec<crate::ai::dses::fight_target::ThreatCandidate> = ec
                    .wildlife_with_stats
                    .iter()
                    .map(
                        |(e, wp, wa)| crate::ai::dses::fight_target::ThreatCandidate {
                            entity: e,
                            position: *wp,
                            species: wa.species,
                            threat_power: wa.threat_power,
                        },
                    )
                    .collect();
                let ally_positions: Vec<Position> = snaps
                    .cat_positions
                    .iter()
                    .filter_map(|(e, p)| if *e == cat_entity { None } else { Some(*p) })
                    .collect();
                // 095 Phase 1 Stage B — 046-Layer-1 carry: feed
                // `combat_advantage_normalized` the body-zone-derived
                // health rather than raw Health.current/max. Spec
                // §IAUS Integration §2.
                let weights = &ec.constants.combat.body_zone_pain_weights;
                let max_pain: f32 = weights.iter().sum();
                let self_health_fraction = body_model.health_derived(weights, max_pain);
                let _ = health;
                // §11 focal-cat hook: mirror socialize/goap.rs:~2557.
                let focal_hook = if ec_is_focal(ec, cat_entity) {
                    ec.focal_capture
                        .as_deref()
                        .map(|cap| crate::ai::target_dse::FocalTargetHook {
                            capture: cap,
                            name_lookup: &|e: Entity| format!("{e:?}"),
                        })
                } else {
                    None
                };
                // Ticket 427 Step 1 — capture only `&ec.faction_overlay_q`
                // so `&mut ec.dse_scratchpad` is a disjoint field borrow.
                let faction_overlay_q = &ec.faction_overlay_q;
                let stance_overlays = |e: Entity| stance_overlays_from_query(faction_overlay_q, e);
                let picked = crate::ai::dses::fight_target::resolve_fight_target(
                    &ec.dse_registry,
                    cat_entity,
                    *pos,
                    &candidates,
                    skills.combat,
                    self_health_fraction,
                    &ally_positions,
                    &ec.faction_relations,
                    &stance_overlays,
                    ec.time.tick,
                    focal_hook,
                    recent_failures,
                    ec.constants
                        .planning_substrate
                        .target_failure_cooldown_ticks,
                    narr.activation.as_deref_mut(),
                    &mut ec.dse_scratchpad,
                );
                plan.step_state[step_idx].target_entity = picked;
                current.target_entity = picked;
            }
            // Move toward the target until adjacent. Without this step,
            // posse-directed cats would set Action::Fight where they
            // stood and wait for the fox to walk to them — which never
            // happens because shadow-foxes avoid wards and cats. Posse
            // formation requires cats to actually converge on the fox.
            let target_pos_opt: Option<Position> = plan.step_state[step_idx]
                .target_entity
                .and_then(|t| ec.wildlife.get(t).ok().map(|(_, p)| *p));
            let fight_outcome = if let Some(target_pos) = target_pos_opt {
                let dist = pos.chebyshev_distance(&target_pos);
                if dist > 1 {
                    // 140 step 7 — desire-based approach over the
                    // smoothed corridor (staleness re-path when the
                    // threat moved is inside the helper).
                    let path_plan = cat_path_plan!(target_pos);
                    path_plan.desire_step_along_smoothed(
                        pos,
                        target_pos,
                        &mut plan.step_state[step_idx].cached_path,
                        &ec.map,
                        desired_velocity,
                        &ec.constants.movement,
                    );
                    crate::steps::StepOutcome::<bool>::unwitnessed(
                        crate::steps::StepResult::Continue,
                    )
                } else {
                    crate::steps::disposition::resolve_fight_threat(ticks, skills, needs, health, d)
                }
            } else {
                crate::steps::disposition::resolve_fight_threat(ticks, skills, needs, health, d)
            };
            fight_outcome
                .record_if_witnessed(narr.activation.as_deref_mut(), Feature::ThreatEngaged);
            fight_outcome.result
        }

        GoapActionKind::Survey => {
            crate::steps::disposition::resolve_survey(
                ticks,
                needs,
                pos,
                &mut prey_params.exploration_map,
                d,
            )
            .result
        }

        GoapActionKind::DeliverDirective => {
            // TODO: resolve directive kind and target from the
            // coordination system so witness can reflect actual
            // delivery, not just time-out.
            let outcome = crate::steps::disposition::resolve_deliver_directive(ticks, needs, d);
            outcome
                .record_if_witnessed(narr.activation.as_deref_mut(), Feature::DirectiveDelivered);
            outcome.result
        }

        GoapActionKind::MateWith => {
            // §6.5.2: resolve mating partner on first tick via the
            // target-taking DSE. Replaces `find_social_target`
            // (fondness-only, **no bond filter**) — the silent
            // divergence was the more dangerous variant since the
            // goap path could pick a non-partner as the mating
            // target once Mating disposition won selection.
            if plan.step_state[step_idx].target_entity.is_none() {
                // §11 focal-cat hook: mirror socialize/goap.rs:~2557.
                let focal_hook = if ec_is_focal(ec, cat_entity) {
                    ec.focal_capture
                        .as_deref()
                        .map(|cap| crate::ai::target_dse::FocalTargetHook {
                            capture: cap,
                            name_lookup: &|e: Entity| format!("{e:?}"),
                        })
                } else {
                    None
                };
                plan.step_state[step_idx].target_entity =
                    crate::ai::dses::mate_target::resolve_mate_target(
                        &ec.dse_registry,
                        cat_entity,
                        *pos,
                        &snaps.cat_positions,
                        relationships,
                        ec.time.tick,
                        focal_hook,
                        recent_failures,
                        ec.constants
                            .planning_substrate
                            .target_failure_cooldown_ticks,
                        narr.activation.as_deref_mut(),
                        &mut ec.dse_scratchpad,
                    );
            }
            let target = plan.step_state[step_idx].target_entity;
            let target_gender = target.and_then(|t| snaps.gender.get(&t).copied());
            // 368: Phase 2 — CourtshipGift presence on the courting
            // cat scales the romantic delta. Caller emits
            // `Feature::CourtshipGiftOffered` on a witnessed Advance
            // when the multiplier was > 1.0.
            let gift_multiplier =
                if inventory.has_item(crate::components::items::ItemKind::CourtshipGift) {
                    ec.constants.crafting.courtship_gift_romantic_multiplier
                } else {
                    1.0
                };
            let outcome = crate::steps::disposition::resolve_mate_with(
                ticks,
                cat_entity,
                *gender,
                target,
                target_gender,
                needs,
                relationships,
                gift_multiplier,
            );
            // MatingOccurred fires only when a pregnancy was produced.
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::MatingOccurred);
            if gift_multiplier > 1.0
                && matches!(outcome.result, crate::steps::StepResult::Advance)
                && target.is_some()
            {
                if let Some(act) = narr.activation.as_deref_mut() {
                    act.record(Feature::CourtshipGiftOffered);
                }
            }
            // §Phase 5a: CourtshipInteraction — the resolver's
            // witness type can't distinguish "no target" from
            // "target, no gestation" (Tom×Tom), so the caller
            // emits this one directly when an Advance happened
            // with a target but no pregnancy.
            if matches!(outcome.result, crate::steps::StepResult::Advance)
                && outcome.witness.is_none()
                && target.is_some()
            {
                if let Some(ref mut act) = narr.activation {
                    act.record(Feature::CourtshipInteraction);
                }
            }
            if let Some((gestator, litter_size)) = outcome.witness {
                // 295 — observable side-effect for the belief substrate.
                // `belief_integrator` updates witnesses' affiliation_history
                // facet on the actor when they observe a successful mating.
                // Emitted only when conception happened (witness is Some) —
                // Tom×Tom encounters skip this and only fire the
                // CourtshipInteraction Feature above.
                narr.witnessable.write(
                    crate::messages::witnessable_event::WitnessableEvent::Mate {
                        actor: cat_entity,
                        target: gestator,
                        position: *pos,
                        tick: ec.time.tick,
                    },
                );
                // §7.M.7.4: Pregnant lands on the gestation-capable
                // partner. `partner` on the Pregnant struct is the
                // other mate — so if the initiator is the gestator,
                // partner = target; otherwise partner = initiator.
                let partner = if gestator == cat_entity {
                    target.unwrap_or(cat_entity)
                } else {
                    cat_entity
                };
                commands
                    .entity(gestator)
                    .insert(crate::components::pregnancy::Pregnant::new(
                        ec.time.tick,
                        partner,
                        litter_size,
                    ));
                if let Some(ref mut elog) = ec.event_log {
                    elog.push(
                        ec.time.tick,
                        EventKind::MatingOccurred {
                            partner_a: name.0.clone(),
                            partner_b: format!("{partner:?}"),
                            location: (pos.x(), pos.y()),
                        },
                    );
                }
                magic_params
                    .pushback_writer
                    .write(crate::systems::magic::CorruptionPushback {
                        position: *pos,
                        radius: 2.0,
                        amount: 0.03,
                    });
            }
            outcome.result
        }

        GoapActionKind::FeedKitten => {
            // §6.5.6 target-taking DSE fallback. Primary seeding
            // happens at plan-creation time in the disposition-chain
            // path via `caretake_resolution.target`; this fallback
            // fires only if the plan arrived here without a seeded
            // target (e.g. save-load without the step_state field).
            // The goap-path `kitten_snapshot` is intentionally empty
            // (see above — avoiding &mut Needs query conflict), so
            // the fallback typically returns `None` and the step
            // no-ops cleanly. Retained so call-site shapes stay
            // parallel to the `resolve_caretake`-era code.
            if plan.step_state[step_idx].target_entity.is_none() {
                // §11 focal-cat hook: mirror socialize/goap.rs:~2557.
                // Typically a no-op here (empty snapshot on the goap
                // path), but wired for consistency with the other
                // step-resolution sites — returns None on empty
                // per-target list so zero cost on non-firing ticks.
                let focal_hook = if ec_is_focal(ec, cat_entity) {
                    ec.focal_capture
                        .as_deref()
                        .map(|cap| crate::ai::target_dse::FocalTargetHook {
                            capture: cap,
                            name_lookup: &|e: Entity| format!("{e:?}"),
                        })
                } else {
                    None
                };
                let parent_marker_active =
                    ec.parent_hungry_kitten_q.get(cat_entity).unwrap_or(false);
                plan.step_state[step_idx].target_entity =
                    crate::ai::dses::caretake_target::resolve_caretake_target(
                        &ec.dse_registry,
                        cat_entity,
                        *pos,
                        &snaps.kitten_snapshot,
                        &[],
                        ec.time.tick,
                        focal_hook,
                        parent_marker_active,
                        &mut ec.dse_scratchpad,
                    )
                    .target;
            }
            let outcome = crate::steps::disposition::resolve_feed_kitten(
                ticks,
                plan.step_state[step_idx].target_entity,
                needs,
                inventory,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::KittenFed);
            if let Some(kitten_entity) = outcome.witness {
                // 295 — observable side-effect for the belief substrate.
                // `belief_integrator` updates witnesses' affiliation_history
                // facet on the caregiver when they observe a successful
                // KittenFed step.
                narr.witnessable.write(
                    crate::messages::witnessable_event::WitnessableEvent::Care {
                        caregiver: cat_entity,
                        kitten: kitten_entity,
                        position: *pos,
                        tick: ec.time.tick,
                    },
                );
                accum.kitten_feedings.push(kitten_entity);
            }
            outcome.result
        }

        GoapActionKind::RetrieveFoodForKitten => {
            // §Phase 4c.4: predecessor step for FeedKitten in the
            // GOAP Caretake plan. Retrieves any food item (raw or
            // cooked) from the nearest Stores so the adult's
            // inventory has something to transfer in FeedKitten.
            // Parallels RetrieveRawFood above but without the raw-
            // only filter — kittens eat either form.
            if plan.step_state[step_idx].target_entity.is_none() {
                plan.step_state[step_idx].target_entity = snaps
                    .stores_entities
                    .iter()
                    .min_by_key(|(_, sp)| pos.tile_distance_squared(sp))
                    .map(|(e, _)| *e);
            }
            let outcome = crate::steps::disposition::resolve_retrieve_any_food_from_stores(
                ticks,
                plan.step_state[step_idx].target_entity,
                inventory,
                stores_query,
                items_query,
                commands,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::ItemRetrieved);
            outcome.result
        }

        // 084: herb-stash deposit / retrieve. Mirrors the food-side
        // dispatch arms — `DepositHerbs` resolves the nearest Stores
        // and transfers every inventory herb slot into `StoredHerbs`;
        // `RetrieveHerbs(kind)` takes one herb of `kind` from
        // `StoredHerbs` back into the actor's inventory. Witnesses
        // gate `HerbsDeposited` / `HerbsRetrieved` Feature emission
        // per the StepOutcome contract.
        GoapActionKind::DepositHerbs => {
            if plan.step_state[step_idx].target_entity.is_none() {
                plan.step_state[step_idx].target_entity = snaps
                    .stores_entities
                    .iter()
                    .min_by_key(|(_, sp)| pos.tile_distance_squared(sp))
                    .map(|(e, _)| *e);
            }
            let capacity = ec.constants.scoring.stores_herb_capacity_per_kind;
            let outcome = crate::steps::disposition::resolve_deposit_herbs_to_stores(
                plan.step_state[step_idx].target_entity,
                inventory,
                &mut building_params.stored_herbs,
                capacity,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::HerbsDeposited);
            outcome.result
        }

        GoapActionKind::RetrieveHerbs(kind) => {
            if plan.step_state[step_idx].target_entity.is_none() {
                // Pick the nearest Stores that actually has ≥1 of `kind`
                // stashed. Without this filter the cat would happily
                // walk to an empty stash and `unwitnessed(Advance)` —
                // wasted travel.
                plan.step_state[step_idx].target_entity = snaps
                    .stores_entities
                    .iter()
                    .filter(|(e, _)| {
                        building_params
                            .stored_herbs
                            .get(*e)
                            .is_ok_and(|sh| sh.count(kind) > 0)
                    })
                    .min_by_key(|(_, sp)| pos.tile_distance_squared(sp))
                    .map(|(e, _)| *e);
            }
            let outcome = crate::steps::disposition::resolve_retrieve_herbs_from_stores(
                plan.step_state[step_idx].target_entity,
                kind,
                inventory,
                &mut building_params.stored_herbs,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::HerbsRetrieved);
            outcome.result
        }

        GoapActionKind::GatherHerb => {
            if plan.step_state[step_idx].target_entity.is_none() {
                // When the plan includes SetWard, target Thornbriar specifically.
                // Otherwise SetWard fails at runtime ("no thornbriar for ward")
                // because the cat gathered the wrong herb type.
                let wants_thornbriar = plan
                    .steps
                    .iter()
                    .any(|s| matches!(s.action, GoapActionKind::SetWard));
                plan.step_state[step_idx].target_entity = snaps
                    .herb_positions
                    .iter()
                    .filter(|(_, _, kind)| !wants_thornbriar || *kind == HerbKind::Thornbriar)
                    .min_by_key(|(_, hp, _)| pos.tile_distance_squared(hp))
                    .map(|(e, _, _)| *e);
            }
            // 308: capture the target herb kind BEFORE the resolver runs —
            // the resolver despawns the herb on Advance, so the kind lookup
            // has to happen first. Used to classify the ReserveDeposited
            // emit by ResourceKind on success.
            let gathered_kind = plan.step_state[step_idx]
                .target_entity
                .and_then(|e| magic_params.herb_query.get(e).ok().map(|(_, h, _)| h.kind));
            let result = crate::steps::magic::resolve_gather_herb(
                ticks,
                plan.step_state[step_idx].target_entity,
                inventory,
                skills,
                &magic_params.herb_query,
                commands,
                &ec.constants.magic,
                &ec.time_scale,
            );
            if matches!(result, crate::steps::StepResult::Advance) {
                if let Some(ref mut act) = narr.activation {
                    act.record(Feature::GatherHerbCompleted);
                }
                if let Some(kind) = gathered_kind {
                    if let Some(resource) =
                        crate::components::magic::ResourceKind::from_herb_kind(kind)
                    {
                        narr.witnessable.write(
                            crate::messages::witnessable_event::WitnessableEvent::ReserveDeposited {
                                actor: cat_entity,
                                kind: resource,
                                position: *pos,
                                tick: ec.time.tick,
                            },
                        );
                    }
                }
            }
            result
        }

        GoapActionKind::SetWard => {
            // Walk to ward placement target if one was set by the coordinator.
            if let Some(ward_target) = plan.ward_placement_pos {
                if pos.chebyshev_distance(&ward_target) > 1 {
                    // 140 step 7 — desire-based approach.
                    let path_plan = cat_path_plan!(ward_target);
                    path_plan.desire_step_along_smoothed(
                        pos,
                        ward_target,
                        &mut plan.step_state[step_idx].cached_path,
                        &ec.map,
                        desired_velocity,
                        &ec.constants.movement,
                    );
                    crate::steps::StepResult::Continue
                } else {
                    // 155: ward kind is determined by the L3-picked
                    // sub-action — `MagicDurableWard` for the magic-
                    // specialist branch, `HerbcraftSetWard` (or any
                    // other) for the thornward branch.
                    let ward_kind = if plan.chosen_action == Action::MagicDurableWard {
                        crate::components::magic::WardKind::DurableWard
                    } else {
                        crate::components::magic::WardKind::Thornward
                    };
                    let result = crate::steps::magic::resolve_set_ward(
                        ticks,
                        cat_entity,
                        ward_kind,
                        &name.0,
                        inventory,
                        magic_aff,
                        skills,
                        mood,
                        corruption,
                        health,
                        &ward_target,
                        &mut rng.rng,
                        commands,
                        &mut narr.log,
                        ec.event_log.as_deref_mut(),
                        &mut magic_params.misfire_writer,
                        ec.time.tick,
                        &ec.constants.magic,
                        &ec.constants.combat,
                        &ec.time_scale,
                        // 301: Path A — `plan.ward_placement_pos` is
                        // `Some` only when the cat is acting on an
                        // `ActiveDirective::SetWard` whose target the
                        // coordinator chose via `compute_ward_placement`.
                        true,
                        // 365 — crafter provenance for CraftedItem.
                        Some(cat_entity),
                    );
                    if matches!(result, crate::steps::StepResult::Advance) {
                        if let Some(ref mut act) = narr.activation {
                            act.record(Feature::WardPlaced);
                        }
                        // 308: emit ReserveConsumed only for thornward —
                        // DurableWard doesn't consume thornbriar.
                        if ward_kind == crate::components::magic::WardKind::Thornward {
                            narr.witnessable.write(
                                crate::messages::witnessable_event::WitnessableEvent::ReserveConsumed {
                                    actor: cat_entity,
                                    kind: crate::components::magic::ResourceKind::Thornbriar,
                                    position: ward_target,
                                    tick: ec.time.tick,
                                },
                            );
                        }
                        // Mastery iter 2 + purpose new-thread: SetWard
                        // is a high-cadence skilled colony-positive
                        // action. STUB — see ticket 016 Phase 5.
                        let d = &ec.constants.disposition;
                        needs.mastery = (needs.mastery + d.mastery_per_magic_success).min(1.0);
                        needs.purpose = (needs.purpose + d.purpose_per_ward_set).min(1.0);
                    }
                    result
                }
            } else {
                let ward_kind = if plan.chosen_action == Action::MagicDurableWard {
                    crate::components::magic::WardKind::DurableWard
                } else {
                    crate::components::magic::WardKind::Thornward
                };
                let result = crate::steps::magic::resolve_set_ward(
                    ticks,
                    cat_entity,
                    ward_kind,
                    &name.0,
                    inventory,
                    magic_aff,
                    skills,
                    mood,
                    corruption,
                    health,
                    pos,
                    &mut rng.rng,
                    commands,
                    &mut narr.log,
                    ec.event_log.as_deref_mut(),
                    &mut magic_params.misfire_writer,
                    ec.time.tick,
                    &ec.constants.magic,
                    &ec.constants.combat,
                    &ec.time_scale,
                    // 301: Path B — `plan.ward_placement_pos` is
                    // `None` here, meaning the cat self-picked
                    // `HerbcraftSetWard` from its DSE and is planting
                    // at its current position. The coordinator's
                    // descending-residual algorithm doesn't touch
                    // this case.
                    false,
                    // 365 — crafter provenance for CraftedItem.
                    Some(cat_entity),
                );
                if matches!(result, crate::steps::StepResult::Advance) {
                    if let Some(ref mut act) = narr.activation {
                        act.record(Feature::WardPlaced);
                    }
                    if ward_kind == crate::components::magic::WardKind::Thornward {
                        narr.witnessable.write(
                            crate::messages::witnessable_event::WitnessableEvent::ReserveConsumed {
                                actor: cat_entity,
                                kind: crate::components::magic::ResourceKind::Thornbriar,
                                position: *pos,
                                tick: ec.time.tick,
                            },
                        );
                    }
                    let d = &ec.constants.disposition;
                    needs.mastery = (needs.mastery + d.mastery_per_magic_success).min(1.0);
                    needs.purpose = (needs.purpose + d.purpose_per_ward_set).min(1.0);
                }
                result
            }
        }

        GoapActionKind::PrepareRemedy => {
            let remedy = inventory
                .first_remedy_kind()
                .unwrap_or(crate::components::magic::RemedyKind::HealingPoultice);
            let at_workshop = snaps.building_snapshot.iter().any(|(_, kind, p, _, _)| {
                *kind == StructureType::Stores && pos.chebyshev_distance(p) <= 1
            });
            let result = crate::steps::magic::resolve_prepare_remedy(
                ticks,
                remedy,
                at_workshop,
                inventory,
                skills,
                &ec.constants.magic,
                &ec.time_scale,
            );
            if matches!(result, crate::steps::StepResult::Advance) {
                narr.witnessable.write(
                    crate::messages::witnessable_event::WitnessableEvent::ReserveConsumed {
                        actor: cat_entity,
                        kind: crate::components::magic::ResourceKind::RemedyHerb,
                        position: *pos,
                        tick: ec.time.tick,
                    },
                );
                if let Some(ref mut act) = narr.activation {
                    act.record(Feature::RemedyPrepared);
                }
            }
            result
        }

        GoapActionKind::ApplyRemedy => {
            if plan.step_state[step_idx].target_entity.is_none() {
                if let Some((patient_e, patient_pos)) = snaps
                    .injured_cat_positions
                    .iter()
                    .filter(|(e, _)| *e != cat_entity)
                    .min_by_key(|(_, cp)| pos.tile_distance_squared(cp))
                {
                    plan.step_state[step_idx].target_entity = Some(*patient_e);
                    plan.step_state[step_idx].target_position = Some(*patient_pos);
                }
            }
            let remedy = inventory
                .first_remedy_kind()
                .unwrap_or(crate::components::magic::RemedyKind::HealingPoultice);
            let patient_alive = plan.step_state[step_idx]
                .target_entity
                .map(|e| snaps.cat_positions.iter().any(|(ce, _)| *ce == e))
                .unwrap_or(false);
            let target_pos = plan.step_state[step_idx].target_position.unwrap_or(*pos);
            let path_plan = cat_path_plan!(target_pos);
            let (result, gratitude) = crate::steps::magic::resolve_apply_remedy(
                remedy,
                cat_entity,
                plan.step_state[step_idx].target_position,
                plan.step_state[step_idx].target_entity,
                patient_alive,
                &mut plan.step_state[step_idx].cached_path,
                pos,
                skills,
                inventory,
                &ec.map,
                &path_plan,
                commands,
                &mut narr.log,
                ec.time.tick,
                &ec.constants.magic,
            );
            if let Some((patient, healer, gain)) = gratitude {
                relationships.modify_fondness(patient, healer, gain);
            }
            result
        }

        GoapActionKind::Scry => {
            let result = crate::steps::magic::resolve_scry(
                ticks,
                cat_entity,
                &name.0,
                magic_aff,
                skills,
                memory,
                mood,
                corruption,
                health,
                pos,
                &ec.map,
                &mut rng.rng,
                commands,
                &mut narr.log,
                &mut magic_params.misfire_writer,
                ec.time.tick,
                &ec.constants.magic,
                &ec.constants.combat,
                &ec.time_scale,
            );
            if matches!(result, crate::steps::StepResult::Advance) {
                if let Some(ref mut act) = narr.activation {
                    act.record(Feature::ScryCompleted);
                }
            }
            result
        }

        GoapActionKind::SpiritCommunion => {
            let act = &mut narr.activation;
            let result = crate::steps::magic::resolve_spirit_communion(
                ticks,
                cat_entity,
                &name.0,
                magic_aff,
                skills,
                mood,
                corruption,
                health,
                pos,
                &mut rng.rng,
                commands,
                &mut narr.log,
                &mut magic_params.misfire_writer,
                ec.time.tick,
                act.as_deref_mut().unwrap(),
                &ec.constants.magic,
                &ec.constants.combat,
                &ec.time_scale,
            );
            if matches!(result, crate::steps::StepResult::Advance) {
                magic_params
                    .pushback_writer
                    .write(crate::systems::magic::CorruptionPushback {
                        position: *pos,
                        radius: 4.0,
                        amount: 0.08,
                    });
            }
            result
        }

        GoapActionKind::CleanseCorruption => {
            // On the first tick, resolve the target corrupted tile from
            // the active directive OR the nearest corruption the cat can
            // see. This is the fix that makes directed cleanse actually
            // walk to the hotspot instead of scrubbing an already-clean
            // patch of grass at the cat's feet.
            if plan.step_state[step_idx].target_position.is_none() {
                let directive_target = magic_params
                    .active_directive_query
                    .get(cat_entity)
                    .ok()
                    .and_then(|d| d.target_position);
                plan.step_state[step_idx].target_position =
                    directive_target.or_else(|| nearest_corrupted_tile(pos, &ec.map, 8));
            }

            // Walk toward the target if we have one and we're not adjacent.
            if let Some(target) = plan.step_state[step_idx].target_position {
                if pos.chebyshev_distance(&target) > 0 {
                    // 140 step 7 — desire-based approach.
                    let path_plan = cat_path_plan!(target);
                    path_plan.desire_step_along_smoothed(
                        pos,
                        target,
                        &mut plan.step_state[step_idx].cached_path,
                        &ec.map,
                        desired_velocity,
                        &ec.constants.movement,
                    );
                    crate::steps::StepResult::Continue
                } else {
                    // Arrived: perform the cleanse.
                    let result = crate::steps::magic::resolve_cleanse_corruption(
                        ticks,
                        cat_entity,
                        &name.0,
                        magic_aff,
                        skills,
                        corruption,
                        mood,
                        health,
                        pos,
                        &mut ec.map,
                        &mut rng.rng,
                        commands,
                        &mut narr.log,
                        &mut magic_params.misfire_writer,
                        ec.time.tick,
                        &ec.constants.magic,
                        &ec.constants.combat,
                        &ec.time_scale,
                    );
                    if matches!(result, crate::steps::StepResult::Advance) {
                        if let Some(ref mut act) = narr.activation {
                            act.record(Feature::CleanseCompleted);
                        }
                        // Check carcasses within 1 tile — corruption
                        // spreads from a carcass to adjacent tiles, so a
                        // cat cleansing a hotspot may be standing next to
                        // (not on) the actual source.
                        for (_, mut carcass, cp) in &mut magic_params.carcass_query {
                            if !carcass.cleansed && pos.chebyshev_distance(cp) <= 1 {
                                carcass.cleansed = true;
                                if let Some(ref mut act) = narr.activation {
                                    act.record(Feature::CarcassCleansed);
                                }
                            }
                        }
                        // Mastery iter 2 + purpose new-thread: Cleanse
                        // is a high-skill colony-positive action.
                        let d = &ec.constants.disposition;
                        needs.mastery = (needs.mastery + d.mastery_per_magic_success).min(1.0);
                        needs.purpose = (needs.purpose + d.purpose_per_colony_action).min(1.0);
                    }
                    result
                }
            } else {
                // No corruption found within reach — the crisis has eased
                // since the directive was issued. Advance without effect.
                crate::steps::StepResult::Advance
            }
        }

        GoapActionKind::HarvestCarcass => {
            // Resolve target: directive-targeted carcass entity preferred,
            // otherwise nearest unharvested carcass.
            if plan.step_state[step_idx].target_entity.is_none() {
                let directive_target = magic_params
                    .active_directive_query
                    .get(cat_entity)
                    .ok()
                    .and_then(|d| d.target_position);
                if let Some(target_pos) = directive_target {
                    plan.step_state[step_idx].target_entity = magic_params
                        .carcass_query
                        .iter()
                        .filter(|(_, c, _)| !c.harvested)
                        .min_by_key(|(_, _, cp)| cp.tile_distance_squared(&target_pos))
                        .map(|(e, _, _)| e);
                } else {
                    plan.step_state[step_idx].target_entity = magic_params
                        .carcass_query
                        .iter()
                        .filter(|(_, c, _)| !c.harvested)
                        .min_by_key(|(_, _, cp)| pos.tile_distance_squared(cp))
                        .map(|(e, _, _)| e);
                }
                // Cache the carcass position for pathfinding.
                if let Some(carcass_entity) = plan.step_state[step_idx].target_entity {
                    if let Ok((_, _, cp)) = magic_params.carcass_query.get(carcass_entity) {
                        plan.step_state[step_idx].target_position = Some(*cp);
                    }
                }
            }

            if let Some(carcass_entity) = plan.step_state[step_idx].target_entity {
                // Walk to the carcass if we aren't on it yet.
                let walking = plan.step_state[step_idx]
                    .target_position
                    .is_some_and(|target| pos.chebyshev_distance(&target) > 0);

                if walking {
                    let target = plan.step_state[step_idx].target_position.unwrap();
                    // 140 step 7 — desire-based approach.
                    let path_plan = cat_path_plan!(target);
                    path_plan.desire_step_along_smoothed(
                        pos,
                        target,
                        &mut plan.step_state[step_idx].cached_path,
                        &ec.map,
                        desired_velocity,
                        &ec.constants.movement,
                    );
                    crate::steps::StepResult::Continue
                } else if ticks
                    >= ec
                        .constants
                        .magic
                        .harvest_carcass_duration
                        .ticks(&ec.time_scale)
                {
                    if let Ok((_, mut carcass, _)) =
                        magic_params.carcass_query.get_mut(carcass_entity)
                    {
                        carcass.harvested = true;
                        let harvest_corruption = if ec.map.in_bounds(pos.x(), pos.y()) {
                            ec.map.get(pos.x(), pos.y()).corruption
                        } else {
                            0.0
                        };
                        // Ticket 482: the ShadowBone yield is now sourced
                        // through the items-are-real gate. Pre-482, the
                        // pre-substrate-era `inventory.add_item_with_modifiers`
                        // call returned `false` on full pouch and the
                        // return value was ignored — a silent drop. The
                        // trait's default push-or-overflow body lands the
                        // bone on the ground in that corner case and fires
                        // `OverflowToGround` as the canary witness.
                        use crate::components::item_gate::sources::HarvestCarcassSource;
                        use crate::components::item_gate::{
                            ItemSource, SourceCtx, SourcePlacement,
                        };
                        let harvest_pos = *pos;
                        let outcome = HarvestCarcassSource {
                            modifiers: crate::components::items::ItemModifiers::with_corruption(
                                harvest_corruption,
                            ),
                        }
                        .source(&mut SourceCtx {
                            inventory: Some(&mut *inventory),
                            commands: &mut *commands,
                            default_position: harvest_pos,
                        });
                        outcome.record_if_witnessed(
                            narr.activation.as_deref_mut(),
                            HarvestCarcassSource::FEATURE,
                        );
                        if matches!(outcome.witness, Some(SourcePlacement::Ground { .. })) {
                            if let Some(act) = narr.activation.as_deref_mut() {
                                act.record(
                                    crate::resources::system_activation::Feature::OverflowToGround,
                                );
                            }
                        }
                        corruption.0 =
                            (corruption.0 + ec.constants.magic.harvest_corruption_gain).min(1.0);
                        skills.herbcraft +=
                            skills.growth_rate() * ec.constants.magic.herbcraft_gather_skill_growth;
                        if let Some(ref mut act) = narr.activation {
                            act.record(Feature::CarcassHarvested);
                        }
                    }
                    crate::steps::StepResult::Advance
                } else {
                    crate::steps::StepResult::Continue
                }
            } else {
                crate::steps::StepResult::Fail("no carcass nearby".into())
            }
        }

        GoapActionKind::Construct => {
            if plan.step_state[step_idx].target_entity.is_none() {
                plan.step_state[step_idx].target_entity = snaps
                    .construction_positions
                    .iter()
                    .min_by_key(|(_, cp)| pos.tile_distance_squared(cp))
                    .map(|(e, _)| *e);
            }
            // Ticket 228 — destination for the cat_path_plan! reachability
            // probe is the resolved site position (or the cat itself on
            // the first dispatch tick before snapshot lookup). The
            // `unwrap_or(*pos)` keeps the staleness check well-defined
            // when the target hasn't been resolved yet.
            let target_pos = plan.step_state[step_idx]
                .target_entity
                .and_then(|e| snaps.construction_positions.iter().find(|(ce, _)| *ce == e))
                .map(|(_, p)| *p)
                .unwrap_or(*pos);
            let path_plan = cat_path_plan!(target_pos);
            let outcome = crate::steps::building::resolve_construct(
                plan.step_state[step_idx].target_entity,
                pos,
                &mut plan.step_state[step_idx].cached_path,
                skills,
                snaps.workshop_bonus,
                &snaps.builders_per_site,
                &mut building_params.buildings,
                &ec.map,
                &path_plan,
                commands,
                &mut building_params.colony_score,
                desired_velocity,
                &ec.constants.movement,
            );
            if matches!(outcome.result, crate::steps::StepResult::Advance) {
                if let Some(ref mut act) = narr.activation {
                    act.record(Feature::BuildingConstructed);
                }
                if let Some(ref mut elog) = ec.event_log {
                    elog.push(
                        ec.time.tick,
                        EventKind::BuildingConstructed {
                            kind: "structure".into(),
                            location: (pos.x(), pos.y()),
                        },
                    );
                }
                // Mastery iter 2 + purpose new-thread: completing a
                // building is a high-impact colony-positive action.
                let d = &ec.constants.disposition;
                needs.mastery = (needs.mastery + d.mastery_per_build_tick).min(1.0);
                needs.purpose = (needs.purpose + d.purpose_per_colony_action).min(1.0);
            }
            outcome.result
        }

        GoapActionKind::TendCrops => {
            if plan.step_state[step_idx].target_entity.is_none() {
                plan.step_state[step_idx].target_entity = snaps
                    .building_snapshot
                    .iter()
                    .filter(|(_, kind, _, _, has_crop)| *kind == StructureType::Garden && *has_crop)
                    .min_by_key(|(_, _, gp, _, _)| pos.tile_distance_squared(gp))
                    .map(|(e, _, _, _, _)| *e);
            }
            let target_pos = plan.step_state[step_idx]
                .target_entity
                .and_then(|e| {
                    snaps
                        .building_snapshot
                        .iter()
                        .find(|(be, _, _, _, _)| *be == e)
                        .map(|(_, _, gp, _, _)| *gp)
                })
                .unwrap_or(*pos);
            let path_plan = cat_path_plan!(target_pos);
            let outcome = crate::steps::building::resolve_tend(
                plan.step_state[step_idx].target_entity,
                pos,
                &mut plan.step_state[step_idx].cached_path,
                skills,
                snaps.season_mod,
                snaps.workshop_bonus,
                &mut building_params.buildings,
                &ec.map,
                &path_plan,
                desired_velocity,
                &ec.constants.movement,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::CropTended);
            // Mastery iter 2 + purpose new-thread: each tend tick
            // (witnessed Advance) contributes a small per-event bump.
            // Per-tick cadence keeps this from saturating quickly.
            if matches!(outcome.result, crate::steps::StepResult::Advance) {
                let d = &ec.constants.disposition;
                needs.mastery = (needs.mastery + d.mastery_per_successful_tend).min(1.0);
                needs.purpose = (needs.purpose + d.purpose_per_colony_action).min(1.0);
            }
            outcome.result
        }

        GoapActionKind::HarvestCrops => {
            if plan.step_state[step_idx].target_entity.is_none() {
                plan.step_state[step_idx].target_entity = snaps
                    .building_snapshot
                    .iter()
                    .filter(|(_, kind, _, _, has_crop)| *kind == StructureType::Garden && *has_crop)
                    .min_by_key(|(_, _, gp, _, _)| pos.tile_distance_squared(gp))
                    .map(|(e, _, _, _, _)| *e);
            }
            let outcome = crate::steps::building::resolve_harvest(
                plan.step_state[step_idx].target_entity,
                pos,
                &snaps.stores_entities,
                &mut building_params.buildings,
                stores_query,
                commands,
            );
            // §Phase 4c.4 + §Phase 5a: emit CropHarvested only
            // when items actually landed in Stores (or a
            // Thornbriar herb spawned). Paired with CropTended —
            // a split between the two signals (tend firing,
            // harvest never) would indicate the tend loop isn't
            // actually advancing crops to full growth, which the
            // canary surfaces.
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::CropHarvested);
            outcome.result
        }

        GoapActionKind::GatherMaterials => {
            // Pick up a material pile from the ground. Founding wagon-
            // dismantling pipeline: the nearest pile is captured the
            // first time this step is reached, then the resolver paths
            // toward it and flips the item to Carried(cat).
            if plan.step_state[step_idx].target_entity.is_none() {
                plan.step_state[step_idx].target_entity = snaps
                    .material_pile_positions
                    .iter()
                    .min_by_key(|(_, mp, _)| pos.tile_distance_squared(mp))
                    .map(|(e, _, _)| *e);
            }
            let target = plan.step_state[step_idx].target_entity;
            let cached = &mut plan.step_state[step_idx].cached_path;
            let target_pos = target
                .and_then(|e| {
                    snaps
                        .material_pile_positions
                        .iter()
                        .find(|(me, _, _)| *me == e)
                        .map(|(_, mp, _)| *mp)
                })
                .unwrap_or(*pos);
            let path_plan = cat_path_plan!(target_pos);
            let outcome = crate::steps::building::resolve_pickup_material(
                target,
                cat_entity,
                pos,
                cached,
                inventory,
                &mut building_params.material_items,
                &ec.map,
                &path_plan,
                desired_velocity,
                &ec.constants.movement,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::MaterialPickedUp);
            outcome.result
        }

        GoapActionKind::DeliverMaterials => {
            // Drop one carried material unit at the nearest unfunded
            // ConstructionSite. The cat's inventory may carry Wood or
            // Stone (or both); we deliver the first build-material slot
            // that the site still needs, falling back to the first
            // build-material slot we find if the per-material check
            // doesn't constrain it.
            if plan.step_state[step_idx].target_entity.is_none() {
                plan.step_state[step_idx].target_entity = snaps
                    .construction_positions
                    .iter()
                    .min_by_key(|(_, cp)| pos.tile_distance_squared(cp))
                    .map(|(e, _)| *e);
            }
            let material_carried = inventory.pouch.iter().find_map(|s| s.kind.material());
            match material_carried {
                Some(material) => {
                    let outcome = crate::steps::building::resolve_deliver(
                        material,
                        plan.step_state[step_idx].target_entity,
                        inventory,
                        &mut building_params.buildings,
                    );
                    outcome.record_if_witnessed(
                        narr.activation.as_deref_mut(),
                        Feature::MaterialsDelivered,
                    );
                    outcome.result
                }
                None => {
                    // Reached the site empty-handed — planner believed
                    // we'd be carrying. Fail so the plan re-routes
                    // through Pickup.
                    crate::steps::StepResult::Fail(
                        "no build-material in inventory to deliver".into(),
                    )
                }
            }
        }

        GoapActionKind::RetrieveRawFood => {
            if plan.step_state[step_idx].target_entity.is_none() {
                plan.step_state[step_idx].target_entity = snaps
                    .stores_entities
                    .iter()
                    .min_by_key(|(_, sp)| pos.tile_distance_squared(sp))
                    .map(|(e, _)| *e);
            }
            let outcome = crate::steps::disposition::resolve_retrieve_raw_food_from_stores(
                ticks,
                plan.step_state[step_idx].target_entity,
                inventory,
                stores_query,
                items_query,
                commands,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::ItemRetrieved);
            outcome.result
        }

        // 367 follow-on — `RetrieveDryable`: tighter item-kind filter
        // than `RetrieveRawFood` (drying recipes accept only
        // `RawFish` / `RawOrgan`). Same target-selection +
        // Feature-emission shape as the sibling above.
        GoapActionKind::RetrieveDryable => {
            if plan.step_state[step_idx].target_entity.is_none() {
                plan.step_state[step_idx].target_entity = snaps
                    .stores_entities
                    .iter()
                    .min_by_key(|(_, sp)| pos.tile_distance_squared(sp))
                    .map(|(e, _)| *e);
            }
            let outcome = crate::steps::disposition::resolve_retrieve_dryable_from_stores(
                ticks,
                plan.step_state[step_idx].target_entity,
                inventory,
                stores_query,
                items_query,
                commands,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::ItemRetrieved);
            outcome.result
        }

        // 443 — `RetrieveSmokeable`: retrieves raw meat AND fuel from
        // Stores into the cat's inventory in one stores visit. The
        // resolver handles the two-ingredient case and no-ops for
        // items the cat already carries. Same target-selection shape
        // as `RetrieveDryable`.
        GoapActionKind::RetrieveSmokeable => {
            if plan.step_state[step_idx].target_entity.is_none() {
                plan.step_state[step_idx].target_entity = snaps
                    .stores_entities
                    .iter()
                    .min_by_key(|(_, sp)| pos.tile_distance_squared(sp))
                    .map(|(e, _)| *e);
            }
            let outcome = crate::steps::disposition::resolve_retrieve_smokeable_from_stores(
                ticks,
                plan.step_state[step_idx].target_entity,
                inventory,
                stores_query,
                items_query,
                commands,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::ItemRetrieved);
            outcome.result
        }

        // 462 — `RetrieveCraftInputs(recipe_id)`: parameterized
        // retrieve over arbitrary recipe input sets. The resolver
        // looks up `recipe.inputs` from `RecipeRegistry` at runtime
        // and pulls each `RecipeInput { kind, count }` from the
        // nearest Stores. Same target-selection shape as
        // `RetrieveSmokeable`/`RetrieveDryable`. Dormant in 462:
        // no plan template emits this variant; Commit 3 widens
        // `Action::Craft`'s template to emit it when the cat holds
        // an `Intention::Goal(HaveItem(_))`, and 463 emits the
        // `HaveItem` Intention from `CraftItemAspiration`.
        GoapActionKind::RetrieveCraftInputs(recipe_id) => {
            if plan.step_state[step_idx].target_entity.is_none() {
                plan.step_state[step_idx].target_entity = snaps
                    .stores_entities
                    .iter()
                    .min_by_key(|(_, sp)| pos.tile_distance_squared(sp))
                    .map(|(e, _)| *e);
            }
            let outcome = crate::steps::disposition::resolve_retrieve_craft_inputs(
                recipe_id,
                &ec.recipes,
                ticks,
                plan.step_state[step_idx].target_entity,
                inventory,
                stores_query,
                items_query,
                commands,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::ItemRetrieved);
            outcome.result
        }

        GoapActionKind::Cook => {
            let outcome =
                crate::steps::disposition::resolve_cook(ticks, inventory, d, &ec.time_scale);
            // Mastery iter 2: Cook fires only when a real raw→cooked
            // flip happens (witness = true). Witnessed Advance is the
            // mastery gate; bare Advance with no witness means no
            // food was actually flipped — no mastery.
            if outcome.witness {
                let dc = &ec.constants.disposition;
                needs.mastery = (needs.mastery + dc.mastery_per_successful_cook).min(1.0);
                needs.purpose = (needs.purpose + dc.purpose_per_colony_action).min(1.0);
            }
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::FoodCooked);
            outcome.result
        }

        GoapActionKind::ExploreSurvey => {
            // Survey at a distant tile.
            crate::steps::disposition::resolve_survey(
                ticks,
                needs,
                pos,
                &mut prey_params.exploration_map,
                d,
            )
            .result
        }

        // 176 stage 3 wired Drop; ticket 177 wires the three siblings.
        //
        // Drop — `inventory + pos + commands` only; resolver removes
        // an item-slot and spawns an OnGround `Item`.
        //
        // Trash — uses the per-tick `snaps.midden_entities` snapshot to
        // resolve the target's `Position` (widening `stores_query` to
        // also borrow `&Structure`/`&Position` would conflict with
        // `BuildingResolverParams::buildings` and refuse to compile).
        //
        // PickUp — calls the resolver directly with the existing
        // `items_query`. The `Without<BuildMaterialItem>` filter on
        // that query makes a build-material target Fail at the
        // resolver, which is the correct semantics (build materials
        // don't move through the disposal pipeline).
        //
        // Handoff — pre-validates the actor side (target set, actor
        // has a transferable slot) and queues a `HandoffPending`. The
        // actual transfer + feature emission happen in the post-loop
        // drain because Bevy's borrow checker forbids holding two
        // `&mut Inventory` borrows from the same cats query inside
        // the per-cat loop. See `handoff_pending` on `StepAccumulators`.
        //
        // Disposal DSEs ship default-zero (Linear slope=0,
        // intercept=0) so these arms remain unreachable at runtime
        // until ticket 178 lifts the weights — at which point the
        // Handoff "optimistic Advance + post-loop drain" caveat
        // graduates from academic to live (the items-are-real contract
        // on `transfer_item_inventory_to_inventory` keeps the source
        // untouched on `DestinationFull`, so no item is destroyed).
        GoapActionKind::DropItem => {
            // Ticket 231: drop_priority is goal-aware, so thread the
            // active disposition + the cat's hunger satiation +
            // colony's construction-site presence so the resolver
            // picks the lowest-priority slot for the cat's current
            // state.
            let has_construction_site = snaps.planner_markers.has(
                crate::components::markers::HasConstructionSite::KEY,
                cat_entity,
            );
            let outcome = crate::steps::disposition::resolve_drop_item(
                inventory,
                *pos,
                plan.kind,
                needs.hunger,
                has_construction_site,
                commands,
            );
            outcome.record_if_witnessed(
                narr.activation.as_deref_mut(),
                crate::resources::system_activation::Feature::ItemDropped,
            );
            outcome.result
        }
        GoapActionKind::TrashItemAtMidden => {
            // 178: resolve nearest Midden as target if not already set —
            // mirrors the `DepositFood` / `EatAtStores` fallback above.
            // The plan template uses `PlannerZone::Wilds` as a placeholder
            // for the Midden zone (a `PlannerZone::Midden` variant is
            // future work); the dispatch arm threads the nearest Midden
            // entity from `snaps.midden_entities` so the resolver has a
            // real target without per-DSE target-picker plumbing.
            if plan.step_state[step_idx].target_entity.is_none() {
                plan.step_state[step_idx].target_entity = snaps
                    .midden_entities
                    .iter()
                    .min_by_key(|(_, mp)| pos.tile_distance_squared(mp))
                    .map(|(e, _)| *e);
            }
            let target = plan.step_state[step_idx].target_entity;
            let Some(midden_entity) = target else {
                return crate::steps::StepResult::Fail(
                    "trash: no target midden on disposition (none in colony)".to_string(),
                );
            };
            let Some(midden_pos) = snaps
                .midden_entities
                .iter()
                .find(|(e, _)| *e == midden_entity)
                .map(|(_, p)| *p)
            else {
                return crate::steps::StepResult::Fail(
                    "trash: target midden not in snapshot (despawned, in-construction, \
                     or not a Midden)"
                        .to_string(),
                );
            };
            let Ok(mut stored) = stores_query.get_mut(midden_entity) else {
                return crate::steps::StepResult::Fail(
                    "trash: target midden lacks StoredItems".to_string(),
                );
            };
            let outcome = crate::steps::disposition::resolve_trash_at_midden(
                inventory,
                midden_entity,
                &mut stored,
                midden_pos,
                commands,
            );
            outcome.record_if_witnessed(
                narr.activation.as_deref_mut(),
                crate::resources::system_activation::Feature::ItemTrashed,
            );
            outcome.result
        }
        GoapActionKind::PickUpItemFromGround => {
            // Ticket 193: resolve nearest OnGround food-Item as target if
            // not already set. Mirrors `TrashItemAtMidden`'s nearest-
            // Midden fallback above. The `HasGroundCarcass` colony marker
            // (re-wired to gate on `food_pile_positions`) guarantees ≥1
            // pickable Item exists when we reach this arm. The TravelTo
            // step routed via `PlannerZone::CarcassPile` brought the cat
            // adjacent to *some* food-Item tile; we pick whichever is
            // closest now in case the world shifted between travel and
            // dispatch (other cat picked up the original target, etc.).
            if plan.step_state[step_idx].target_entity.is_none() {
                plan.step_state[step_idx].target_entity = snaps
                    .food_pile_positions
                    .iter()
                    .min_by_key(|(_, fp, _)| pos.tile_distance_squared(fp))
                    .map(|(e, _, _)| *e);
            }
            let target = plan.step_state[step_idx].target_entity;
            let outcome = crate::steps::disposition::resolve_pick_up_from_ground(
                inventory,
                target,
                items_query,
                commands,
            );
            outcome.record_if_witnessed(
                narr.activation.as_deref_mut(),
                crate::resources::system_activation::Feature::ItemRetrieved,
            );
            outcome.result
        }
        GoapActionKind::HandoffItem => {
            // 188 / 410: resolve nearest hungry kitten as recipient if
            // not already set. Mirrors `TrashItemAtMidden`'s nearest-
            // Midden fallback above. The `HasDependentCat` colony marker
            // gates both Handing and Caretake DSEs upstream, so the
            // kitten roster is non-empty when we reach this arm under
            // normal conditions. We still guard for the empty case
            // because eligibility is sampled at L2 and the snapshot can
            // race with a kitten despawn between filter and resolve.
            // Pick the closest kitten with the lowest hunger
            // satisfaction so adults feed the most-in-need nearby
            // dependent. Per-cat picker (multi-axis) is ticket 192.
            if plan.step_state[step_idx].target_entity.is_none() {
                plan.step_state[step_idx].target_entity = snaps
                    .kitten_snapshot
                    .iter()
                    // Sort by (hunger ascending, distance ascending) — most
                    // hungry kittens first, then nearest. f32 doesn't
                    // implement Ord; use partial_cmp + manhattan as tie-break.
                    .min_by(|a, b| {
                        a.hunger
                            .partial_cmp(&b.hunger)
                            .unwrap_or(std::cmp::Ordering::Equal)
                            .then_with(|| {
                                pos.tile_distance_squared(&a.pos)
                                    .cmp(&pos.tile_distance_squared(&b.pos))
                            })
                    })
                    .map(|k| k.entity);
            }
            let Some(recipient) = plan.step_state[step_idx].target_entity else {
                return crate::steps::StepResult::Fail(
                    "handoff: no recipient on disposition (no dependent cat in colony)".to_string(),
                );
            };
            let has_transferable = !inventory.pouch.is_empty();
            if !has_transferable {
                return crate::steps::StepResult::Fail(
                    "handoff: no transferable slot in actor inventory".to_string(),
                );
            }
            accum.handoff_pending.push(HandoffPending {
                actor: cat_entity,
                recipient,
            });
            crate::steps::StepResult::Advance
        }
        // 230: Fleeing chain dispatch — `[PickFleeTarget, Flee, HoldUntilSafe]`.
        // Concrete resolvers live under `src/steps/disposition/`. This
        // arm threads the per-replan `RouteCostField` (the
        // `route_cost_field` parameter, attached to the cat at line
        // 1648-1698 by `evaluate_and_plan`) plus the nearest-threat
        // position (from `ec.wildlife`) into the picker, and the
        // current-tick safety need into the hold step.
        GoapActionKind::PickFleeTarget => {
            let threat_pos = ec
                .wildlife
                .iter()
                .map(|(_, tp)| *tp)
                .min_by_key(|tp| pos.tile_distance_squared(tp));
            let outcome = crate::steps::disposition::resolve_pick_flee_target(
                *pos,
                route_cost_field,
                threat_pos,
                d.flee_distance,
                &ec.map,
            );
            if let Some(target) = outcome.witness {
                plan.step_state[step_idx].target_position = Some(target);
            }
            outcome.record_if_witnessed(
                narr.activation.as_deref_mut(),
                crate::resources::system_activation::Feature::FleeTargetPicked,
            );
            outcome.result
        }
        GoapActionKind::Flee => {
            // Umbrella travel leg — delegate to the same `cat_path_plan!`
            // machinery `TravelTo` uses (gradient-walk on
            // `RouteCostField`, A* fallback). The picked target lives
            // on `plan.step_state[step_idx-1].target_position` via the
            // standard `carry_target_forward` pipeline; when not yet
            // propagated, fall back to the cat's current position (the
            // macro's staleness probe — staying put is benign).
            let _carried = crate::systems::plan_substrate::carry_target_forward(
                &mut plan.step_state,
                step_idx,
                &ec.target_validity,
                None,
            );
            let target_for_plan = plan.step_state[step_idx].target_position.unwrap_or(*pos);
            let path_plan = cat_path_plan!(target_for_plan);
            // 295: source the threat Entity from the same nearest-wildlife
            // scan PickFleeTarget uses (line 6298-6302). The threat may
            // differ between Pick and Flee ticks if a closer predator
            // approached — the witness records what the cat is *currently*
            // fleeing from, which is the observably-correct threat for
            // any onlooker. When wildlife is transiently empty, the cat
            // still walks toward its picked flee target (the resolver
            // doesn't gate on `threat` for movement); the FleeFrom emit
            // is simply skipped for that tick since there's no threat
            // entity to attribute the witness to.
            let threat = ec
                .wildlife
                .iter()
                .min_by_key(|(_, tp)| pos.tile_distance_squared(tp))
                .map(|(e, _)| e);
            let outcome = crate::steps::disposition::resolve_flee_travel(
                pos,
                target_for_plan,
                threat,
                &path_plan,
                &ec.map,
                desired_velocity,
                &ec.constants.movement,
            );
            if let Some(w) = outcome.witness {
                // 295 — observable side-effect for the belief substrate.
                // `belief_integrator` reads FleeFrom to update predictability
                // on the fleer (this cat) and perceived_violence_capability
                // on the threat. Fires on the Advance branch (cat reached
                // flee target) AND when a threat was nameable; a
                // still-walking Continue or a threat-less Advance produces
                // no witness.
                narr.witnessable.write(
                    crate::messages::witnessable_event::WitnessableEvent::FleeFrom {
                        fleer: cat_entity,
                        threat: w.threat,
                        position: *pos,
                        tick: ec.time.tick,
                    },
                );
            }
            outcome.result
        }
        GoapActionKind::HoldUntilSafe => {
            let outcome = crate::steps::disposition::resolve_hold_until_safe(
                ticks,
                *pos,
                route_cost_field,
                needs.safety,
                d.flee_hold_ticks,
                d.route_cost_safe_threshold,
                d.flee_safety_need_threshold,
            );
            outcome.record_if_witnessed(
                narr.activation.as_deref_mut(),
                crate::resources::system_activation::Feature::FleeRecovered,
            );
            outcome.result
        }
        // 364: HTN-driven primitives — kitten arc. The dispatch closure
        // (D1, commit b) pins chosen_action from
        // `HeldGoalStack.frames[top].sub_goals[idx]` at the
        // evaluate_and_plan author site; the plan-template emits a
        // single-action plan that lands here. Each arm resolves the
        // target kitten via `dependent_kitten_target` picker (filtered by
        // mother + per-action maturity band), reads the current
        // KittenDependency state via `ec.kitten_parentage`, runs the pure
        // resolver, records the Feature if witnessed, and queues a
        // post-loop drain entry (`accum.kitten_rearing_advances`) that
        // performs the actual component mutation.
        GoapActionKind::Wean => dispatch_htn_kitten_primitive(
            crate::ai::Action::Wean,
            step_idx,
            cat_entity,
            *pos,
            plan,
            ec,
            snaps,
            accum,
            narr,
        ),
        GoapActionKind::Teach => dispatch_htn_kitten_primitive(
            crate::ai::Action::Teach,
            step_idx,
            cat_entity,
            *pos,
            plan,
            ec,
            snaps,
            accum,
            narr,
        ),
        GoapActionKind::Release => dispatch_htn_kitten_primitive(
            crate::ai::Action::Release,
            step_idx,
            cat_entity,
            *pos,
            plan,
            ec,
            snaps,
            accum,
            narr,
        ),
        // 357: HTN-driven primitives — mourn arc. Real wiring deferred:
        // the §7.7.b grief-event-emission debt authors the `Mourning`
        // marker on colony-mate death. Until that lands, `mourn_at_grave`
        // is never adopted and these arms are unreachable in production.
        GoapActionKind::Vigil => crate::steps::disposition::resolve_vigil().result,
        GoapActionKind::GriefSit => crate::steps::disposition::resolve_grief_sit().result,
        GoapActionKind::ReleaseGrief => crate::steps::disposition::resolve_release_grief().result,

        // 367 — Phase 1b preservation dispatch.
        GoapActionKind::DryFood => {
            // Load resolver consults the cat's inventory + the
            // nearest idle drying rack, drains the inputs, stamps
            // the rack's state. The per-tick `systems::preservation`
            // chain (Commit 5) then advances progress under Clear
            // weather and spawns the output entity at completion.
            // Proximity gate: 3 tiles — the cat is at the rack's
            // tile (ZoneIs precondition) but the per-tile resolver
            // accommodates the 2×2 footprint.
            let outcome = crate::steps::disposition::resolve_load_drying_rack(
                *pos,
                inventory,
                skills,
                &ec.constants.crafting,
                &mut building_params.drying_racks,
                3.0,
            );
            outcome.record_if_witnessed(
                narr.activation.as_deref_mut(),
                crate::resources::system_activation::Feature::FoodLoadedOnDryingRack,
            );
            outcome.result
        }
        GoapActionKind::SmokeMeat => {
            let outcome = crate::steps::disposition::resolve_load_smoking_rack(
                *pos,
                inventory,
                skills,
                &mut building_params.smoking_racks,
                3.0,
                &ec.constants.crafting,
            );
            outcome.record_if_witnessed(
                narr.activation.as_deref_mut(),
                crate::resources::system_activation::Feature::MeatLoadedOnSmokingRack,
            );
            outcome.result
        }
        GoapActionKind::TendSmokingRack => {
            // The tend resolver returns
            // `StepOutcome<Option<bool>>`: witness `None` = no tend
            // (Fail), `Some(false)` = intermediate tend
            // (progress < 1.0), `Some(true)` = completion tend that
            // spawned the `SmokedMeat` `Item`. Every successful tend
            // emits `SmokingRackTended`; completion tends ALSO emit
            // `MeatSmoked`.
            let outcome = crate::steps::disposition::resolve_tend_smoking_rack(
                *pos,
                ec.time.tick,
                &mut building_params.smoking_racks,
                3.0,
                &ec.constants.crafting,
                commands,
            );
            outcome.record_if_witnessed(
                narr.activation.as_deref_mut(),
                crate::resources::system_activation::Feature::SmokingRackTended,
            );
            if outcome.witness == Some(true) {
                if let Some(act) = narr.activation.as_deref_mut() {
                    act.record(crate::resources::system_activation::Feature::MeatSmoked);
                }
            }
            outcome.result
        }
        GoapActionKind::BegForFood => {
            // 450: kitten beg cycle. The cry-map stamping + parent
            // marker authoring happens in `growth::update_kitten_cry_map`
            // keyed off the L3 election (`CurrentAction.action ==
            // BegForFood`). This resolver's role is the canary signal:
            // every cycle completion emits `Feature::KittenBegged` so
            // the seed-42 footer catches a silently-dead Begging
            // disposition.
            let outcome = crate::steps::disposition::resolve_beg_for_food(
                plan.step_state[step_idx].ticks_elapsed,
                cat_entity,
                *pos,
                needs.hunger,
            );
            outcome.record_if_witnessed(
                narr.activation.as_deref_mut(),
                crate::resources::system_activation::Feature::KittenBegged,
            );
            outcome.result
        }
        // 457 + 463 commit 8: Workshop-craft dispatch parameterized by
        // `RecipeId`. The recipe identity flows from the held HaveItem
        // Intention through the plan template (`craft_have_item_actions`)
        // — the resolver crafts exactly the named recipe, retiring the
        // pre-463 lex-pick at the resolver level.
        GoapActionKind::CraftAtWorkshop(recipe_id) => {
            let outcome = crate::steps::disposition::resolve_craft_at_workshop(
                recipe_id,
                *pos,
                inventory,
                wearables,
                &ec.recipes,
                &snaps.workshop_positions,
                3.0,
            );
            // Ticket 463 — on a witnessed craft, append the actual
            // recipe id to the cat's ring buffer so
            // CraftItemAspiration's anti-monotony term reads recency
            // next tick. Lazy-insert pattern: if the component is
            // absent (cat's first-ever craft), spawn it with the new
            // entry pre-recorded.
            if let Some(recipe_id) = outcome.witness.as_ref() {
                record_recent_craft(
                    cat_entity,
                    recent_crafts,
                    commands,
                    *recipe_id,
                    ec.time.tick,
                );
            }
            outcome.record_if_witnessed(
                narr.activation.as_deref_mut(),
                crate::resources::system_activation::Feature::ItemCrafted,
            );
            outcome.result
        }
        // 369 + 463 commit 8: TanningFrame-craft dispatch parameterized
        // by `RecipeId`. Sibling to the Workshop arm; same shape.
        GoapActionKind::CraftAtTanningFrame(recipe_id) => {
            let outcome = crate::steps::disposition::resolve_craft_at_tanning_frame(
                recipe_id,
                *pos,
                inventory,
                wearables,
                &ec.recipes,
                &snaps.tanning_frame_positions,
                3.0,
            );
            if let Some(recipe_id) = outcome.witness.as_ref() {
                record_recent_craft(
                    cat_entity,
                    recent_crafts,
                    commands,
                    *recipe_id,
                    ec.time.tick,
                );
            }
            outcome.record_if_witnessed(
                narr.activation.as_deref_mut(),
                crate::resources::system_activation::Feature::ItemCrafted,
            );
            outcome.result
        }
        // 334: don the first equippable wearable from the cat's pouch into
        // its anatomical slot (or swap the occupant). Idempotent — when the
        // wearable was already auto-equipped on craft (017), the resolver
        // witnesses success without re-equipping (no Feature recorded).
        GoapActionKind::WearItem => {
            let outcome = crate::steps::disposition::resolve_wear_item(inventory, wearables);
            outcome.record_if_witnessed(
                narr.activation.as_deref_mut(),
                crate::resources::system_activation::Feature::ItemWorn,
            );
            outcome.result
        }
    }
}

/// Ticket 463 — record a witnessed craft on the cat's `CatRecentCrafts`
/// ring buffer. When the component is already present (post-first-
/// craft cat), writes in-place. When absent (first-craft cat), uses
/// `commands.entity().insert(...)` to lazy-insert a freshly-recorded
/// component — the archetype shift only fires for cats who actually
/// craft, preserving non-crafting cats' archetype identity (memory
/// `learning_bevy_schedule_edge_perturbation`).
fn record_recent_craft(
    cat_entity: Entity,
    recent_crafts: Option<&mut crate::components::recent_crafts::CatRecentCrafts>,
    commands: &mut Commands,
    recipe_id: crate::components::recipe::RecipeId,
    tick: u64,
) {
    match recent_crafts {
        Some(rc) => rc.record(recipe_id, tick),
        None => {
            let mut rc = crate::components::recent_crafts::CatRecentCrafts::default();
            rc.record(recipe_id, tick);
            commands.entity(cat_entity).insert(rc);
        }
    }
}

/// 364: kitten-arc dispatch helper. Resolves the target via the
/// per-action `dependent_kitten_target` picker, reads
/// `KittenDependency` state, runs the resolver, records the Feature,
/// queues the post-loop drain entry. Returns the underlying
/// [`StepResult`](crate::steps::StepResult).
#[allow(clippy::too_many_arguments)]
fn dispatch_htn_kitten_primitive(
    action: crate::ai::Action,
    step_idx: usize,
    cat_entity: Entity,
    pos: Position,
    plan: &mut GoapPlan,
    ec: &mut ExecutorContext,
    snaps: &StepSnapshots,
    accum: &mut StepAccumulators,
    narr: &mut NarrativeEmitter,
) -> crate::steps::StepResult {
    use crate::resources::system_activation::Feature;
    let weaned_threshold = ec.constants.kitten_rearing.weaned_threshold;
    let teach_done_threshold = ec.constants.kitten_rearing.teach_done_threshold;
    let release_threshold = ec.constants.kitten_rearing.release_threshold;
    let curriculum_size = ec.constants.kitten_rearing.teach_curriculum_size;

    // 395: picker filters candidates by (mother OR father) +
    // per-action maturity band + !released_by_arc; returns the nearest
    // match. Wean is `[0, weaned_threshold)`, Teach is
    // `[weaned_threshold, teach_done_threshold)`, Release is
    // `[release_threshold, 1.0)` (the near-mature window — Release
    // fires "at max age").
    if plan.step_state[step_idx].target_entity.is_none() {
        let kittens = build_dependent_kitten_snapshot(&ec.kitten_parentage, &snaps.cat_positions);
        plan.step_state[step_idx].target_entity =
            crate::ai::dses::dependent_kitten_target::resolve_dependent_kitten_target(
                action,
                &ec.dse_registry,
                cat_entity,
                pos,
                &kittens,
                weaned_threshold,
                teach_done_threshold,
                release_threshold,
                ec.time.tick,
                None,
                &mut ec.dse_scratchpad,
            );
    }

    let Some(target) = plan.step_state[step_idx].target_entity else {
        // 395 / R11: differentiate two failure modes:
        //   - This cat is a parent (mother or father) of a dependent
        //     kitten but the kitten is past or before this sub-goal's
        //     maturity band → substrate-clean `Advance` so the HTN
        //     frame's sub_goal_index moves on; next tick's dispatch
        //     resolves the band-correct primitive. Without this,
        //     `GoalFrame::new`'s hard-coded `sub_goal_index = 0`
        //     causes per-tick Wean failures (2439 per soak pre-395).
        //   - This cat has no dependent kitten at all → real `Fail`
        //     via the HTN backtrack hook (consults the frame's
        //     `MethodFailure`).
        // 395 extends 333/364's mother-only check to symmetric
        // mother-OR-father per the "both parents pitch in" decision.
        let is_parent_of_any_dependent = ec.kitten_parentage.iter().any(|(_, dep, _released)| {
            dep.mother == Some(cat_entity) || dep.father == Some(cat_entity)
        });
        if is_parent_of_any_dependent {
            return crate::steps::StepResult::Advance;
        }
        return crate::steps::StepResult::Fail(format!(
            "{action:?}: no dependent kitten in range/band"
        ));
    };

    // Read current KittenDependency state via the disjoint
    // `kitten_parentage` query.
    let dep_state = ec.kitten_parentage.get(target).ok().map(|(_, d, _)| d);

    match action {
        crate::ai::Action::Wean => {
            let current_maturity = dep_state.map(|d| d.maturity).unwrap_or(1.0);
            let outcome =
                crate::steps::disposition::resolve_wean(target, current_maturity, weaned_threshold);
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::KittenWeaned);
            if let Some(advanced) = outcome.witness {
                accum
                    .kitten_rearing_advances
                    .push(KittenRearingAdvance::Wean(advanced));
            }
            outcome.result
        }
        crate::ai::Action::Teach => {
            let current_maturity = dep_state.map(|d| d.maturity).unwrap_or(1.0);
            let current_skills = dep_state
                .map(|d| d.skills_learned)
                .unwrap_or(curriculum_size);
            let outcome = crate::steps::disposition::resolve_teach(
                target,
                current_maturity,
                current_skills,
                teach_done_threshold,
                curriculum_size,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::SkillTaught);
            if let Some(advanced) = outcome.witness {
                accum
                    .kitten_rearing_advances
                    .push(KittenRearingAdvance::Teach(advanced));
            }
            outcome.result
        }
        crate::ai::Action::Release => {
            let kitten_has_dependency = dep_state.is_some();
            let outcome = crate::steps::disposition::resolve_release(target, kitten_has_dependency);
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::KittenReleased);
            if let Some(advanced) = outcome.witness {
                accum
                    .kitten_rearing_advances
                    .push(KittenRearingAdvance::Release(advanced));
            }
            outcome.result
        }
        other => crate::steps::StepResult::Fail(format!(
            "dispatch_htn_kitten_primitive: unsupported action {other:?}"
        )),
    }
}

/// 364 / 395: build the kitten snapshot consumed by
/// `resolve_dependent_kitten_target`. Ticket 451 — `kitten_parentage`
/// is now slim (no `Position`); the `cat_positions` snapshot in
/// `StepSnapshots` covers kittens too because they carry `GoapPlan`
/// post-451 and therefore appear in the cats query that builds it.
#[allow(clippy::type_complexity)]
fn build_dependent_kitten_snapshot(
    kitten_parentage: &Query<
        (
            Entity,
            &crate::components::KittenDependency,
            Has<crate::components::markers::RearKittenReleased>,
        ),
        (
            Without<Dead>,
            Without<Structure>,
            With<crate::components::KittenDependency>,
        ),
    >,
    cat_positions: &[(Entity, Position)],
) -> Vec<crate::ai::dses::dependent_kitten_target::DependentKittenState> {
    let pos_lookup: std::collections::HashMap<Entity, Position> =
        cat_positions.iter().copied().collect();
    kitten_parentage
        .iter()
        .map(|(entity, dep, released)| {
            crate::ai::dses::dependent_kitten_target::DependentKittenState {
                entity,
                pos: pos_lookup
                    .get(&entity)
                    .copied()
                    .unwrap_or(Position::new(0, 0)),
                maturity: dep.maturity,
                mother: dep.mother,
                father: dep.father,
                released_by_arc: released,
            }
        })
        .collect()
}

// ===========================================================================
// emit_plan_narrative
// ===========================================================================

#[allow(clippy::too_many_arguments)]
pub fn emit_plan_narrative(
    mut messages: MessageReader<PlanNarrative>,
    names: Query<(&Name, &Gender, &Personality, &Needs, &Position)>,
    map: Res<TileMap>,
    time: Res<TimeState>,
    config: Res<crate::resources::time::SimConfig>,
    weather: Res<crate::resources::weather::WeatherState>,
    registry: Option<Res<crate::resources::narrative_templates::TemplateRegistry>>,
    mut log: ResMut<crate::resources::narrative::NarrativeLog>,
    mut rng: ResMut<SimRng>,
    mut history_query: Query<&mut ActionHistory>,
) {
    for msg in messages.read() {
        // Dedup: don't narrate repeated Adopted events for the same disposition.
        if msg.event == PlanEvent::Adopted {
            if let Ok(mut hist) = history_query.get_mut(msg.entity) {
                if hist.last_narrated_disposition == Some(msg.kind) {
                    continue;
                }
                hist.last_narrated_disposition = Some(msg.kind);
                hist.replans_narrated = 0;
            }
        }

        // Throttle Completed events: suppress repeated completions for the
        // same disposition within 500 ticks (e.g., rest/rested cycles).
        if msg.event == PlanEvent::Completed {
            if let Ok(mut hist) = history_query.get_mut(msg.entity) {
                if let Some((kind, tick)) = hist.last_completed_tick {
                    if kind == msg.kind && time.tick.saturating_sub(tick) < 500 {
                        continue;
                    }
                }
                hist.last_completed_tick = Some((msg.kind, time.tick));
            }
        }

        // Throttle Replanned events: max 1 replan narrative per plan lifecycle.
        if msg.event == PlanEvent::Replanned {
            if let Ok(mut hist) = history_query.get_mut(msg.entity) {
                if hist.replans_narrated >= 1 {
                    continue;
                }
                hist.replans_narrated += 1;
            }
        }

        let Ok((name, gender, personality, needs, pos)) = names.get(msg.entity) else {
            continue;
        };

        let action = msg.kind.constituent_actions()[0];
        let event_tag = match msg.event {
            PlanEvent::Adopted => "plan_adopted",
            PlanEvent::Completed => "plan_complete",
            PlanEvent::Replanned => "plan_replanned",
            PlanEvent::Abandoned => "plan_abandoned",
        };

        let terrain = if map.in_bounds(pos.x(), pos.y()) {
            map.get(pos.x(), pos.y()).terrain
        } else {
            Terrain::Grass
        };
        let day_phase = DayPhase::from_tick(time.tick, &config);
        let season = Season::from_tick(time.tick, &config);

        let ctx = TemplateContext {
            action,
            day_phase,
            season,
            weather: weather.current,
            mood_bucket: MoodBucket::Neutral,
            life_stage: LifeStage::Adult,
            has_target: false,
            terrain,
            event: Some(event_tag.into()),
        };
        let var_ctx = VariableContext {
            name: &name.0,
            gender: *gender,
            weather: weather.current,
            day_phase,
            season,
            life_stage: LifeStage::Adult,
            fur_color: "unknown",
            other: None,
            prey: None,
            item: None,
            item_singular: None,
            quality: None,
        };

        let fallback = match msg.event {
            PlanEvent::Adopted => format!("{} sets out to {}.", name.0, msg.kind.verb_infinitive()),
            PlanEvent::Completed => {
                format!("{} finishes {}.", name.0, msg.kind.label().to_lowercase())
            }
            PlanEvent::Replanned => format!("{} adjusts course.", name.0),
            PlanEvent::Abandoned => format!("{} gives up.", name.0),
        };

        emit_event_narrative(
            registry.as_deref(),
            &mut log,
            time.tick,
            fallback,
            crate::resources::narrative::NarrativeTier::Action,
            &ctx,
            &var_ctx,
            personality,
            needs,
            &mut rng.rng,
        );
    }
}

// ===========================================================================
// Helper: resolve TravelTo
// ===========================================================================

#[allow(clippy::too_many_arguments)]
fn resolve_travel_to(
    zone: PlannerZone,
    state: &mut StepExecutionState,
    pos: &mut Position,
    map: &TileMap,
    path_plan: &crate::ai::route_cost::CatPathPlan<'_>,
    exploration_map: &ExplorationMap,
    stores_positions: &[Position],
    construction_positions: &[(Entity, Position)],
    farm_positions: &[Position],
    herb_positions: &[(Entity, Position, HerbKind)],
    kitchen_positions: &[Position],
    cat_positions: &[(Entity, Position)],
    material_pile_positions: &[(Entity, Position, ItemKind)],
    food_pile_positions: &[(Entity, Position, ItemKind)],
    drying_rack_positions: &[Position],
    smoking_rack_positions: &[Position],
    workshop_positions: &[Position],
    tanning_frame_positions: &[Position],
    dead_cat_positions: &[(Entity, Position)],
    cat_entity: Entity,
    d: &DispositionConstants,
    // 140 step 6 — the resolver now expresses movement desire instead
    // of writing Position per tick; the Chain-4 integrator moves the
    // cat. Position writes remain ONLY at arrival (snap / anti-stack
    // jitter — step 7 replaces those with separation steering).
    desired: &mut crate::components::physical::DesiredVelocity,
    movement: &crate::resources::sim_constants::MovementConstants,
) -> crate::steps::StepResult {
    if state.target_position.is_none() {
        state.target_position = resolve_zone_position(
            zone,
            pos,
            map,
            exploration_map,
            stores_positions,
            construction_positions,
            farm_positions,
            herb_positions,
            kitchen_positions,
            cat_positions,
            material_pile_positions,
            food_pile_positions,
            drying_rack_positions,
            smoking_rack_positions,
            workshop_positions,
            tanning_frame_positions,
            dead_cat_positions,
            cat_entity,
            d,
        );
    }
    let Some(target) = state.target_position else {
        return crate::steps::StepResult::Fail("no reachable zone target".into());
    };

    // 140 step 6 — smoothed corridor + desire-based movement. The
    // cached path holds the STRING-PULLED waypoints (sparse tile
    // centers of the retained corridor — `find_smoothed_path`), not
    // the dense A* tile chain; the cat seeks the first waypoint with
    // a DesiredVelocity and the Chain-4 integrator does the moving
    // (momentum, Euclidean speed cap, wall-slide). Arrival semantics
    // unchanged: containing-tile adjacency to the target.
    if state.cached_path.is_none() {
        state.cached_path = path_plan.find_smoothed_path(*pos, target, map);
    }

    if let Some(ref mut path) = state.cached_path {
        // 140 step 7 — arrival is CONTAINING-TILE EQUALITY (walked,
        // not snapped). The legacy resolver advanced at chebyshev<=1
        // and then TELEPORTED onto the target tile; same-tile
        // consumers (PickUpItemFromGround, deposit interactions)
        // depend on ending ON the tile, so the no-teleport equivalent
        // is to keep seeking until the mover's containing tile IS the
        // target tile (the final smoothed waypoint is the target
        // center — the seek carries us there; Position Eq is
        // tile-keyed). The stacked-jitter teleport stays retired —
        // separation handles crowding.
        if *pos == target {
            return crate::steps::StepResult::Advance;
        }
        // Pop waypoints the integrator has carried us within reach of.
        while let Some(wp) = path.first().copied() {
            if pos.0.distance(wp.0) <= movement.waypoint_arrival_radius {
                path.remove(0);
            } else {
                break;
            }
        }
        let aim = path.first().copied().unwrap_or(target);
        desired.0 = Some(crate::ai::steering::seek(
            pos.0,
            aim.0,
            movement.cat_max_speed,
        ));
    } else {
        // No path found — step toward target directly via the same
        // `CatPathPlan` (gradient-walk's `next_step` falls back to
        // greedy `step_toward` under the AStarFallback / NoOverlay
        // arms; 228).
        let before = *pos;
        if let Some(next) = path_plan.next_step(*pos, target, map) {
            *pos = next;
        }
        if pos.chebyshev_distance(&target) <= 1 {
            return crate::steps::StepResult::Advance;
        }
        // Early exit: pathfinding found no path and greedy step made no progress.
        if *pos == before {
            state.no_move_ticks += 1;
        } else {
            state.no_move_ticks = 0;
        }
        if state.no_move_ticks > d.travel_no_path_stuck_ticks {
            return crate::steps::StepResult::Fail("no path and stuck".into());
        }
    }

    // Timeout: if stuck for too long, fail.
    if state.ticks_elapsed > d.travel_timeout_ticks {
        return crate::steps::StepResult::Fail("travel timeout".into());
    }

    crate::steps::StepResult::Continue
}

// ===========================================================================
// Helper: resolve SearchPrey (transplanted from HuntPrey search phase)
// ===========================================================================

#[allow(clippy::too_many_arguments)]
fn resolve_search_prey(
    state: &mut StepExecutionState,
    ticks: u64,
    pos: &mut Position,
    // 293: the cat's own per-bucket prey-yield beliefs. `None` for
    // cats spawned without a `LocationBeliefs` component (test setups
    // pre-258); the search step still works using wind / patrol_dir.
    loc_beliefs: Option<&crate::components::beliefs::LocationBeliefs>,
    prey_query: &Query<(Entity, &Position, &PreyConfig, &mut PreyState), With<PreyAnimal>>,
    den_query: &Query<(Entity, &PreyDen, &Position), Without<PreyAnimal>>,
    inventory: &mut Inventory,
    skills: &mut Skills,
    prey_params: &mut PreyHuntParams,
    map: &TileMap,
    wind: &crate::resources::wind::WindState,
    narr: &mut NarrativeEmitter<'_>,
    time: &TimeState,
    rng: &mut SimRng,
    commands: &mut Commands,
    _cat_entity: Entity,
    personality: &Personality,
    name: &Name,
    gender: &Gender,
    needs: &Needs,
    d: &DispositionConstants,
    cat_profile: &crate::systems::sensing::SensoryProfile,
    dse_registry: &crate::ai::eval::DseRegistry,
    // §9.3 stance prefilter inputs: required by `resolve_hunt_target`
    // to drop befriended prey (Prey → Ally upgrade rejects Hunt).
    relations: &crate::ai::faction::FactionRelations,
    stance_overlays: &dyn Fn(Entity) -> crate::ai::faction::StanceOverlays,
    // §11 focal-cat hook inputs: the two pieces needed to build a
    // `FocalTargetHook` locally without threading the ExecutorContext.
    is_focal: bool,
    focal_capture: Option<&crate::resources::FocalScoreCapture>,
    // Ticket 073 — per-cat recently-failed target memory (cooldown
    // sensor input) and the cooldown window in ticks. Caller pulls
    // `recent_failures.as_deref()` from the cats query and the cooldown
    // ticks from `SimConstants::planning_substrate`.
    recent_failures: Option<&crate::components::RecentTargetFailures>,
    cooldown_ticks: u64,
    // 263: ActionAffordances resource for the hunt-target 5th
    // per-target axis (`hunt_best_predation_affordance`). Threaded
    // from `ColonyContext.action_affordances` at the
    // `GoapActionKind::SearchPrey` arm.
    action_affordances: &crate::resources::action_affordances::ActionAffordances,
    // Ticket 427 Step 1 — DSE target scratchpad threaded through to
    // `resolve_hunt_target`.
    scratch: &mut crate::resources::DseTargetScratchpad,
) -> crate::steps::StepResult {
    use crate::components::item_gate::sources::DenRaidCarcassSource;
    use crate::components::item_gate::{ItemSource, SourceCtx, SourcePlacement};

    // Den discovery check.
    for (den_entity, den, den_pos) in den_query.iter() {
        if pos.distance_to(den_pos) <= d.den_discovery_range {
            let discovery_chance =
                d.den_discovery_base_chance + skills.hunting * d.den_discovery_skill_scale;
            if rng.rng.random::<f32>() < discovery_chance && den.spawns_remaining > 0 {
                let kills = ((den.spawns_remaining as f32 * d.den_raid_kill_fraction).ceil()
                    as u32)
                    .min(den.raid_drop);
                let drop_item = den.item_kind;
                let den_name = den.den_name;
                let den_pos_copy = *den_pos;

                let den_corruption = if map.in_bounds(den_pos_copy.x(), den_pos_copy.y()) {
                    map.get(den_pos_copy.x(), den_pos_copy.y()).corruption
                } else {
                    0.0
                };
                let den_mods =
                    crate::components::items::ItemModifiers::with_corruption(den_corruption);
                for _ in 0..kills {
                    let ground_position = Position::new(
                        den_pos_copy.x() + rng.rng.random_range(-1..=1i32),
                        den_pos_copy.y() + rng.rng.random_range(-1..=1i32),
                    );
                    let outcome = DenRaidCarcassSource {
                        kind: drop_item,
                        modifiers: den_mods,
                        ground_quality: d.den_dropped_item_quality,
                        ground_position,
                    }
                    .source(&mut SourceCtx {
                        inventory: Some(&mut *inventory),
                        commands: &mut *commands,
                        default_position: ground_position,
                    });
                    outcome.record_if_witnessed(
                        narr.activation.as_deref_mut(),
                        DenRaidCarcassSource::FEATURE,
                    );
                    if matches!(outcome.witness, Some(SourcePlacement::Ground { .. })) {
                        if let Some(act) = narr.activation.as_deref_mut() {
                            act.record(
                                crate::resources::system_activation::Feature::OverflowToGround,
                            );
                        }
                    }
                }

                // 293: substrate emission for the den-raid kill —
                // integrator's `Hunt` arm lifts the actor's
                // `LocationBeliefs.prey_yield` at the bucket.
                narr.witnessable.write(
                    crate::messages::witnessable_event::WitnessableEvent::Hunt {
                        hunter: _cat_entity,
                        prey_kind: den.kind,
                        position: den_pos_copy,
                        success: true,
                        tick: time.tick,
                    },
                );

                prey_params.raid_writer.write(DenRaided {
                    den_entity,
                    kills,
                    item_kind: drop_item,
                    position: den_pos_copy,
                    den_name,
                });

                emit_hunt_narrative(
                    narr,
                    time,
                    rng,
                    map,
                    pos,
                    name,
                    gender,
                    personality,
                    needs,
                    "raid",
                    &format!("{} raids a {}!", name.0, den_name),
                    Some(den_name),
                    None,
                );

                // Den raid counts as finding prey — advance.
                return crate::steps::StepResult::Advance;
            }
        }
    }

    // 293: search movement priority — per-cat prey-yield belief > wind >
    // patrol_dir. Drops the legacy "colony-belief gradient" fallback
    // because the colony view is now derived from per-cat beliefs; any
    // cat with positive evidence about a bucket already surfaces in
    // their own LocationBeliefs. The cross-cat aggregate read survives
    // on the visualization snapshot only (snapshot.rs).
    let belief_dir = loc_beliefs.and_then(|lb| {
        crate::systems::belief_aggregation::best_prey_direction(
            lb,
            *pos,
            d.search_belief_radius,
            crate::resources::colony_hunting_map::DEFAULT_PRIOR,
        )
    });
    let (wx, wy) = wind.direction();
    let (mut dx, mut dy) = if let Some((bx, by)) = belief_dir {
        (bx, by)
    } else if wx.abs() > d.search_wind_direction_threshold
        || wy.abs() > d.search_wind_direction_threshold
    {
        (-(wx.signum() as i32), -(wy.signum() as i32))
    } else {
        state.patrol_dir
    };

    if rng.rng.random::<f32>() < d.search_jitter_chance {
        dx = rng.rng.random_range(-1i32..=1);
        dy = rng.rng.random_range(-1i32..=1);
    }
    if dx == 0 && dy == 0 {
        dx = 1;
    }
    let before = *pos;
    for _ in 0..d.search_speed {
        *pos = patrol_move(pos, dx, dy, map);
    }
    // If stuck at terrain edge, randomize direction to escape.
    if *pos == before {
        state.patrol_dir = (
            rng.rng.random_range(-1i32..=1),
            rng.rng.random_range(-1i32..=1),
        );
        let (ndx, ndy) = state.patrol_dir;
        *pos = patrol_move(pos, ndx, ndy, map);
    }

    // Visual detection → §6.5.5 hunt-target DSE. Replaces the
    // pre-refactor `min_by_key(tile_distance_squared)` pick — the legacy
    // path picked the nearest prey regardless of yield, so a Mouse at
    // range 5 was always chosen over a Rabbit at range 7 even though
    // the Rabbit delivers 1.3× food value. §6.1 Partial fix: the DSE
    // scores distance (quadratic falloff), species yield, and
    // alertness together.
    let visible: Vec<crate::ai::dses::hunt_target::PreyCandidate> = prey_query
        .iter()
        .filter(|(_, pp, _, _)| {
            crate::systems::sensing::observer_sees_at(
                crate::components::SensorySpecies::Cat,
                *pos,
                cat_profile,
                **pp,
                crate::components::SensorySignature::PREY,
                d.search_visual_detection_range,
            )
        })
        .map(
            |(e, pp, pc, ps)| crate::ai::dses::hunt_target::PreyCandidate {
                entity: e,
                position: *pp,
                kind: pc.kind,
                alertness: ps.alertness,
            },
        )
        .collect();

    if !visible.is_empty() {
        // §11 focal-cat hook: mirror socialize/goap.rs:~2557.
        let focal_hook = if is_focal {
            focal_capture.map(|cap| crate::ai::target_dse::FocalTargetHook {
                capture: cap,
                name_lookup: &|e: Entity| format!("{e:?}"),
            })
        } else {
            None
        };
        let picked = crate::ai::dses::hunt_target::resolve_hunt_target(
            dse_registry,
            _cat_entity,
            *pos,
            // 100: cat boldness powers the prey_alertness_tolerance axis.
            personality.boldness,
            &visible,
            relations,
            stance_overlays,
            time.tick,
            focal_hook,
            recent_failures,
            cooldown_ticks,
            // No activation tracker threaded through this helper today;
            // the cooldown application still applies via the IAUS axis,
            // just no `Feature::TargetCooldownApplied` count from the
            // SearchPrey path. The other 5 target DSEs cover the soak
            // canary; revisit if hunt-cooldown observability becomes a
            // live diagnostic question.
            None,
            // 263: ActionAffordances borrow for the 5th per-target
            // axis. Dormant at default — reads return 0.0 and the
            // axis is omitted from the composition anyway.
            action_affordances,
            scratch,
        );
        if let Some(prey_entity) = picked {
            state.target_entity = Some(prey_entity);
            return crate::steps::StepResult::Advance;
        }
    }

    // Scent detection via PreyScentMaps (ticket 062 — per-species
    // grid-sampled influence map). Finds the strongest-scent bucket
    // within `scent_search_radius` using the max-aggregate read across
    // all five sub-maps; `min_by_key` resolves to the prey entity
    // closest to that source tile.
    let scent_source =
        prey_params
            .prey_scent_maps
            .highest_nearby_any(pos.x(), pos.y(), d.scent_search_radius);
    let scent_above_threshold = scent_source
        .map(|(sx, sy)| prey_params.prey_scent_maps.get_any(sx, sy) >= d.scent_detect_threshold)
        .unwrap_or(false);
    let scented_prey = if scent_above_threshold {
        let (sx, sy) = scent_source.unwrap();
        let source = Position::new(sx, sy);
        prey_query
            .iter()
            .min_by_key(|(_, pp, _, _)| source.tile_distance_squared(pp))
    } else {
        None
    };

    if let Some((prey_entity, prey_pos_ref, _, _)) = scented_prey {
        state.target_entity = Some(prey_entity);
        // 293: substrate-only scent detection. Integrator's
        // `HuntScentDetected` arm lifts the actor's
        // `LocationBeliefs.prey_yield` at the bucket.
        narr.witnessable.write(
            crate::messages::witnessable_event::WitnessableEvent::HuntScentDetected {
                actor: _cat_entity,
                prey_kind: prey_query
                    .iter()
                    .find(|(e, _, _, _)| *e == prey_entity)
                    .map(|(_, _, cfg, _)| cfg.kind)
                    .unwrap_or(crate::components::prey::PreyKind::Mouse),
                position: *prey_pos_ref,
                tick: time.tick,
            },
        );
        emit_hunt_narrative(
            narr,
            time,
            rng,
            map,
            pos,
            name,
            gender,
            personality,
            needs,
            "scent",
            &format!("{} catches a scent on the wind.", name.0),
            None,
            None,
        );
        return crate::steps::StepResult::Advance;
    }

    // Timeout.
    if ticks > d.search_timeout_ticks {
        if inventory.pouch.iter().any(|s| s.kind.is_food()) {
            // Have food from earlier — advance to deposit.
            return crate::steps::StepResult::Advance;
        }
        // 293: substrate-only failed-search emission. Integrator's
        // `HuntSearchYieldedNoPrey` arm drops the actor's
        // `LocationBeliefs.prey_yield` at the bucket, scaled by
        // tiles_searched.
        narr.witnessable.write(
            crate::messages::witnessable_event::WitnessableEvent::HuntSearchYieldedNoPrey {
                actor: _cat_entity,
                position: *pos,
                tiles_searched: ticks,
                tick: time.tick,
            },
        );
        return crate::steps::StepResult::Fail("no scent found".into());
    }

    crate::steps::StepResult::Continue
}

// ===========================================================================
// Helper: resolve EngagePrey (transplanted from HuntPrey stalk/chase/pounce)
// ===========================================================================

/// Ticket 149 — emit a `HuntAttempt` event and record `Feature::HuntAttempted`
/// for the never-fired canary. Called from every terminal return path of
/// `resolve_engage_prey` (kill, lost-during-*, abandoned). The activation
/// record is witness-gated by virtue of being on the terminal path:
/// `StepResult::Continue` returns never call this, and the `Continue` →
/// `Continue` → ... → `Advance|Fail` chain is exactly one attempt.
// 295: extended to centralize WitnessableEvent::Hunt emission. The
// substrate's hunt success-flag derives directly from `HuntOutcome` —
// Killed* → success=true, LostDuring* → success=false, Abandoned → no
// emit (the cat never engaged or the target vanished, so there's no
// witness-relevant hunt event to report). Centralizing here means all
// 8 call sites in `resolve_engage_prey` route their belief-substrate
// signal through one place.
#[allow(clippy::too_many_arguments)]
fn record_hunt_attempt(
    event_log: Option<&mut EventLog>,
    activation: Option<&mut crate::resources::system_activation::SystemActivation>,
    witnessable: Option<
        &mut bevy_ecs::message::MessageWriter<crate::messages::witnessable_event::WitnessableEvent>,
    >,
    cat_name: &str,
    cat_entity: Entity,
    species: &str,
    prey_kind: PreyKind,
    prey_pos: Position,
    outcome: HuntOutcome,
    time_tick: u64,
    ticks_elapsed: u64,
    start_distance: i32,
    failure_reason: Option<String>,
) {
    let start_tick = time_tick.saturating_sub(ticks_elapsed);
    if let Some(elog) = event_log {
        elog.push(
            time_tick,
            EventKind::HuntAttempt {
                cat: cat_name.to_string(),
                prey_species: species.to_string(),
                location: (prey_pos.x(), prey_pos.y()),
                outcome,
                start_tick,
                end_tick: time_tick,
                start_distance,
                failure_reason,
            },
        );
    }
    if let Some(act) = activation {
        act.record(crate::resources::system_activation::Feature::HuntAttempted);
    }
    if let Some(w) = witnessable {
        let success = match outcome {
            HuntOutcome::Killed
            | HuntOutcome::KilledAndReplanned
            | HuntOutcome::KilledAndConsumed => Some(true),
            HuntOutcome::LostDuringApproach
            | HuntOutcome::LostDuringStalk
            | HuntOutcome::LostDuringChase => Some(false),
            HuntOutcome::Abandoned => None,
        };
        if let Some(success) = success {
            w.write(crate::messages::witnessable_event::WitnessableEvent::Hunt {
                hunter: cat_entity,
                prey_kind,
                position: prey_pos,
                success,
                tick: time_tick,
            });
        }
    }
}

// Ticket 149 — `event_log.as_deref_mut()` at outcome-emission sites is the
// canonical re-borrow idiom for `Option<&mut EventLog>` across multiple
// mutually-exclusive return paths. Clippy flags it as a no-op, but the
// alternative (move at each site) trips NLL when the function has more than
// a handful of terminal returns. Allow at the function level.
#[allow(clippy::too_many_arguments, clippy::needless_option_as_deref)]
fn resolve_engage_prey(
    state: &mut StepExecutionState,
    ticks: u64,
    pos: &mut Position,
    inventory: &mut Inventory,
    // Ticket 017 — worn equip slots. Read for the weapon-strike bonus;
    // mutated to remove a wielded bone weapon when it snaps on a miss.
    wearables: &mut crate::components::equipment::WearableSlots,
    skills: &mut Skills,
    prey_query: &mut Query<(Entity, &Position, &PreyConfig, &mut PreyState), With<PreyAnimal>>,
    prey_params: &mut PreyHuntParams,
    map: &TileMap,
    scoring: &crate::resources::sim_constants::ScoringConstants,
    narr: &mut NarrativeEmitter<'_>,
    time: &TimeState,
    rng: &mut SimRng,
    commands: &mut Commands,
    cat_entity: Entity,
    personality: &Personality,
    name: &Name,
    gender: &Gender,
    // 150 R1: `&mut Needs` (was `&Needs`) so a hungry catch can be
    // consumed in-place — see the consume-on-spot branch after the
    // POUNCE arm. Was read-only because the legacy hunt cycle pushed
    // the catch to inventory and deferred all hunger restoration to
    // EatAtStores.
    needs: &mut Needs,
    d: &DispositionConstants,
    mut event_log: Option<&mut EventLog>,
    // 263: ActionAffordances borrow for the C5 stalk_start phase-band
    // bias. The bias is dormant by default (`hunt_stalk_chase_affordance_bias
    // = 0.0`); at non-zero values, the stalk_start threshold is
    // multiplied by `(1 + bias * (a_stalk - a_chase))` clamped to
    // `±bias`. `pounce_range` is NOT biased — the leap is a physics
    // invariant the catch math relies on.
    affordances: &crate::resources::action_affordances::ActionAffordances,
    // 375 — per-species guaranteed byproduct table. Each successful kill
    // spawns the meat plus the species' fixed byproduct list. Independent
    // of `crafting.organ_drop_chance` (367's probabilistic mammal+bird
    // organ roll, which continues to fire on top).
    prey_byproducts: &crate::resources::sim_constants::PreyByproductConstants,
    // 100 — per-species tremor `base_range` lookup for the
    // `effective_stalk_distance` species_push term.
    sensory: &crate::resources::sim_constants::SensoryConstants,
    // 508 — the routing cat's own threat beliefs (None on test paths
    // without the component); prices witnessed-ambush ground into
    // chase-step overlays.
    location_beliefs: Option<&crate::components::beliefs::LocationBeliefs>,
    // 100 — cat's CurrentAction. The resolver stamps `Action::Stalk`
    // on stalk-phase entry and `Action::Pounce` on pounce-phase entry
    // so `tremor_tick` reads the right multiplier next tick.
    current_action: &mut CurrentAction,
    // 477 — combat constants for the equipment weapon-strike bonus +
    // bone-snap roll, and the focal-cat resolver-trace sink so the
    // weapon modifier surfaces as a named L4Resolver row.
    combat: &crate::resources::sim_constants::CombatConstants,
    focal_sink: Option<&crate::resources::trace_log::FocalResolverSink>,
) -> crate::steps::StepResult {
    use crate::components::prey::PreyAiState;

    let Some(target_entity) = state.target_entity else {
        return crate::steps::StepResult::Fail("no prey target for engage".into());
    };

    let Ok((_, prey_pos, prey_cfg, prey_state)) = prey_query.get(target_entity) else {
        // Prey despawned between ticks. Real abandonment of an in-flight
        // attempt iff `attempt_start_distance` was already captured on a
        // prior tick — emit with placeholders since species/position are
        // gone with the entity. If start_distance is None, no real engage
        // tick ever ran for this target, so skip emission.
        if let Some(start_dist) = state.attempt_start_distance {
            record_hunt_attempt(
                event_log.as_deref_mut(),
                narr.activation.as_deref_mut(),
                // 295: Abandoned never emits a Hunt witnessable event,
                // so the `PreyKind::Mouse` placeholder below is dead.
                Some(&mut narr.witnessable),
                &name.0,
                cat_entity,
                "unknown",
                PreyKind::Mouse,
                Position::new(0, 0),
                HuntOutcome::Abandoned,
                time.tick,
                ticks,
                start_dist,
                Some("prey despawned".into()),
            );
        }
        return crate::steps::StepResult::Fail("prey despawned".into());
    };

    let prey_pos = *prey_pos;
    let prey_is_fleeing = matches!(prey_state.ai_state, PreyAiState::Fleeing { .. });
    let prey_awareness = prey_state.ai_state;
    let catch_mod = prey_cfg.catch_difficulty;
    let item_kind = prey_cfg.item_kind;
    let species_name = prey_cfg.name;
    // 295: capture `kind` up-front for the same reason `species_name`
    // is — `record_hunt_attempt` is called from multiple branches that
    // share their `prey_query` borrow with `prey_query.get_mut` calls,
    // and keeping `prey_cfg.kind` accesses live across those mutable
    // borrows trips NLL. PreyKind is Copy, so this is a no-cost capture.
    let prey_kind = prey_cfg.kind;

    // Ticket 223 — cat-side path-cost overlays for predation
    // step_toward loops. Substrate, not search state (§4.7). Borrows
    // `prey_params.fox_scent_map` for `cat_overlays`'s lifetime; the
    // function mutates other (disjoint) prey_params fields like
    // `kill_writer` and `exploration_map`, which Bevy SystemParam
    // permits via per-field reborrow.
    let fox_overlay =
        crate::ai::pathfinding::FoxScentOverlay::new(&prey_params.fox_scent_map, scoring);
    let corr_overlay = crate::ai::pathfinding::CorruptionOverlay::new(map, scoring);
    // Ticket 224 — per-cat boldness weight on threat-cost overlays.
    // Bold cats chase prey through fox territory more readily; timid
    // cats detour. Complementary to the L2 boldness axis on Hunt
    // (scoring.rs:649) — that axis decides *whether* to hunt; this
    // weight decides *where* the chase route runs.
    let w = crate::ai::pathfinding::cat_path_weight_from_boldness(personality.boldness);
    // 508 — chase steps also price the cat's own witnessed-ambush
    // beliefs (a chase must not thread the shadowfox haunting ground
    // the travel route just detoured around).
    let threat_overlay =
        location_beliefs.map(|lb| crate::ai::pathfinding::ThreatBeliefOverlay::new(lb, scoring));
    let mut cat_overlays: Vec<crate::ai::pathfinding::WeightedOverlay> = vec![
        crate::ai::pathfinding::WeightedOverlay::new(&fox_overlay, w),
        crate::ai::pathfinding::WeightedOverlay::new(&corr_overlay, w),
    ];
    if let Some(t) = threat_overlay.as_ref() {
        cat_overlays.push(crate::ai::pathfinding::WeightedOverlay::new(t, w));
    }
    let flee_strategy = prey_cfg.flee_strategy;
    let dist = pos.chebyshev_distance(&prey_pos);

    // Ticket 149 — capture start_distance on the first tick this attempt
    // executes. Stays Some for the lifetime of this StepExecutionState; the
    // next attempt (after a kill / loss / abandon) uses a fresh state.
    if state.attempt_start_distance.is_none() {
        state.attempt_start_distance = Some(dist);
    }
    let start_distance = state.attempt_start_distance.unwrap_or(dist);

    // Bird teleport — give up immediately.
    if prey_is_fleeing && flee_strategy == crate::components::prey::FleeStrategy::Teleport {
        record_hunt_attempt(
            event_log.as_deref_mut(),
            narr.activation.as_deref_mut(),
            Some(&mut narr.witnessable),
            &name.0,
            cat_entity,
            species_name,
            prey_kind,
            prey_pos,
            HuntOutcome::Abandoned,
            time.tick,
            ticks,
            start_distance,
            Some("prey teleported".into()),
        );
        return crate::steps::StepResult::Fail("prey teleported".into());
    }

    // 100 — per-cat effective stalk distance. Replaces the constant
    // `(alert_radius + stalk_start_buffer).max(stalk_start_minimum)`
    // with a continuous personality-modulated computation. The legacy
    // base (`stalk_start_minimum + stalk_start_buffer`) is recovered
    // when patience=1, alertness=0, species_sens=0 and ambient reads
    // are zero. Bold cats (patience≈0) collapse to a near-minimum stalk
    // distance; patient cats expand it. Ambient reads only contribute
    // at non-trivial patience (the `× patience` scaling), so bold cats
    // skip the read by construction.
    let species_sens = crate::systems::sensing::prey_tremor_sensitivity(prey_kind, sensory);
    let tremor_at_prey = prey_params.tremor_map.get(prey_pos.x(), prey_pos.y());
    let scent_settle = prey_params
        .prey_scent_maps
        .for_kind(prey_kind)
        .get(prey_pos.x(), prey_pos.y());
    let raw_stalk_distance = d.stalk_start_minimum
        + d.stalk_start_buffer * personality.patience
        + d.alertness_push * prey_state.alertness
        + d.species_push * species_sens
        + personality.patience * d.tremor_push * tremor_at_prey
        - personality.patience * d.scent_settle_push * scent_settle;
    // Clamp to `[min, min + 2 × buffer]` so a clean settled-prey read
    // can't collapse the stalk distance below the personality-neutral
    // base, and a perfect-storm reading can't more-than-double it.
    let stalk_low = d.stalk_start_minimum;
    let stalk_high = stalk_low + 2.0 * d.stalk_start_buffer;
    let stalk_start_base = raw_stalk_distance.clamp(stalk_low, stalk_high).round() as i32;
    // 263: affordance-biased stalk-start. Bias is dormant by default;
    // at non-zero `hunt_stalk_chase_affordance_bias`, high stalk
    // affordance widens the stalk band (cat begins stalking from
    // farther out); high chase affordance narrows it (cat transitions
    // to chase sooner). The bias factor is bounded to `±bias` so the
    // band can never collapse or balloon beyond a controlled fraction
    // of its physical base.
    let stalk_start = {
        let bias = scoring.hunt_stalk_chase_affordance_bias.clamp(0.0, 1.0);
        if bias > 0.0 {
            let a_stalk = affordances.read(
                cat_entity,
                target_entity,
                crate::resources::action_affordances::ActionKind::Stalk,
            );
            let a_chase = affordances.read(
                cat_entity,
                target_entity,
                crate::resources::action_affordances::ActionKind::Chase,
            );
            let raw = bias * (a_stalk - a_chase);
            let factor = (1.0 + raw.clamp(-bias, bias)).max(0.0);
            ((stalk_start_base as f32) * factor).round() as i32
        } else {
            stalk_start_base
        }
    };
    let pounce_range: i32 = if personality.patience > 0.7 {
        d.pounce_range_patient as i32
    } else if personality.patience < 0.3 {
        d.pounce_range_impatient as i32
    } else {
        d.pounce_range_default as i32
    };

    if dist <= pounce_range {
        // === POUNCE ===
        // 100: stamp Action::Pounce so this tick's `tremor_tick`
        // deposit uses the peak (≈2.0×) multiplier. The pounce range
        // is by construction inside the terminal grab window, so the
        // tremor spike is "too late" feedback — that's the design.
        current_action.action = Action::Pounce;
        let awareness_base = match prey_awareness {
            PreyAiState::Idle | PreyAiState::Grazing { .. } => d.pounce_awareness_idle,
            PreyAiState::Alert { .. } => d.pounce_awareness_alert,
            // 140 step 10 — an airborne escaping bird is at least as
            // aware as a ground-fleeing target.
            PreyAiState::Fleeing { .. } | PreyAiState::BurstFlight { .. } => {
                d.pounce_awareness_fleeing
            }
        };
        let distance_mod = match dist {
            0..=1 => d.pounce_distance_close_mod,
            2 => d.pounce_distance_mid_mod,
            _ => d.pounce_distance_far_mod,
        };
        let density = prey_params
            .density
            .0
            .get(&prey_cfg.kind)
            .copied()
            .unwrap_or(d.pounce_density_threshold);
        let density_bonus = if density > d.pounce_density_threshold {
            1.0 + (density - d.pounce_density_threshold)
        } else {
            1.0
        };
        let base_success_chance = awareness_base
            * (d.pounce_skill_base + skills.hunting * d.pounce_skill_scale)
            * distance_mod
            * catch_mod
            * density_bonus;

        // 477 — equipment weapon-strike bonus. A wielded melee weapon
        // raises the catch threshold by a class-keyed, quality-scaled
        // amount, surfaced in the resolver trace as a named modifier
        // (never a hidden post-hoc bonus). Ranged weapons contribute
        // nothing here (their mode is the 477 follow-up).
        let em = crate::components::equipment_effects::equipment_modifiers_for(wearables, combat);
        let weapon_bonus = em.weapon.map(|w| w.strike_bonus(combat)).unwrap_or(0.0);
        let success_chance = (base_success_chance + weapon_bonus).clamp(0.0, 1.0);
        if let Some(sink) = focal_sink.filter(|_| weapon_bonus > 0.0) {
            let label = match em.weapon.map(|w| w.class) {
                Some(crate::components::equipment::WeaponClass::Pierce) => "weapon.pierce.bonus",
                Some(crate::components::equipment::WeaponClass::Slash) => "weapon.slash.bonus",
                Some(crate::components::equipment::WeaponClass::Blunt) => "weapon.blunt.bonus",
                _ => "weapon.bonus",
            };
            sink.record(
                cat_entity,
                "resolve_engage_prey",
                label,
                base_success_chance,
                success_chance,
            );
        }

        if rng.rng.random::<f32>() < success_chance {
            // Catch!
            commands.entity(target_entity).despawn();
            let catch_corruption = if map.in_bounds(prey_pos.x(), prey_pos.y()) {
                map.get(prey_pos.x(), prey_pos.y()).corruption
            } else {
                0.0
            };

            // 150 R1 — eat-the-catch. A cat below `production_self_eat_threshold`
            // consumes the prey on the spot instead of carrying it home; the
            // hunger arithmetic mirrors `resolve_eat_at_stores` (food_value
            // × freshness, raw prey so no cooked multiplier). This closes
            // the structural starvation hole where hungry hunters walked
            // food past their own mouths to deposit it. Skill gain, kill
            // event, narrative beat, and prey-density bookkeeping all
            // still fire — the catch is real either way; only the
            // disposition of the carcass changes.
            let consumed_in_place = needs.hunger < d.production_self_eat_threshold;
            let modifiers =
                crate::components::items::ItemModifiers::with_corruption(catch_corruption);
            if consumed_in_place {
                let freshness = (1.0 - catch_corruption * d.corruption_food_penalty).max(0.0);
                needs.hunger = (needs.hunger + item_kind.food_value() * freshness).min(1.0);
                if let Some(act) = narr.activation.as_deref_mut() {
                    act.record(crate::resources::system_activation::Feature::FoodEaten);
                }
            } else {
                // Ticket 429: items-are-real Source gate. The trait's
                // default push-or-overflow body unifies the prior
                // explicit if/else (inventory-push vs ground-spawn at
                // `prey_pos`). `OverflowToGround` is emitted in
                // addition to the gate's own Feature when the ground
                // arm fires.
                use crate::components::item_gate::sources::HuntCatchSource;
                use crate::components::item_gate::{ItemSource, SourceCtx, SourcePlacement};
                let outcome = HuntCatchSource {
                    kind: item_kind,
                    modifiers,
                }
                .source(&mut SourceCtx {
                    inventory: Some(&mut *inventory),
                    commands: &mut *commands,
                    default_position: prey_pos,
                });
                outcome
                    .record_if_witnessed(narr.activation.as_deref_mut(), HuntCatchSource::FEATURE);
                if matches!(outcome.witness, Some(SourcePlacement::Ground { .. })) {
                    if let Some(act) = narr.activation.as_deref_mut() {
                        act.record(crate::resources::system_activation::Feature::OverflowToGround);
                    }
                }
            }

            // 375 — byproduct spawn. Each guaranteed byproduct is its
            // own physical entity / inventory slot (items-are-real); a
            // single rabbit kill therefore yields 4 items (meat + hide
            // + bone + sinew). Per-item inventory `is_full()` checks
            // are independent — items 2/3/4 overflow individually if
            // capacity fills mid-spawn. Modifiers (corruption) mirror
            // the meat: the byproduct shares the carcass's spatial
            // origin, so a corrupted catch yields corrupted byproducts.
            // Byproducts are non-food (`is_food() == false`), so the
            // self-eat branch is structurally inapplicable.
            //
            // Ticket 429: HuntByproductSource trait dispatch. Reuses
            // the existing `ByproductSpawned` Positive canary (375) as
            // its FEATURE so this site emits 1:1 with prior behavior;
            // `OverflowToGround` fires additionally when the trait's
            // overflow arm trips.
            for &byp_kind in prey_byproducts.for_kind(prey_kind) {
                use crate::components::item_gate::sources::HuntByproductSource;
                use crate::components::item_gate::{ItemSource, SourceCtx, SourcePlacement};
                let outcome = HuntByproductSource {
                    kind: byp_kind,
                    modifiers,
                }
                .source(&mut SourceCtx {
                    inventory: Some(&mut *inventory),
                    commands: &mut *commands,
                    default_position: prey_pos,
                });
                outcome.record_if_witnessed(
                    narr.activation.as_deref_mut(),
                    HuntByproductSource::FEATURE,
                );
                if matches!(outcome.witness, Some(SourcePlacement::Ground { .. })) {
                    if let Some(act) = narr.activation.as_deref_mut() {
                        act.record(crate::resources::system_activation::Feature::OverflowToGround);
                    }
                }
            }

            skills.hunting += skills.growth_rate() * d.hunt_catch_skill_growth;

            prey_params.kill_writer.write(PreyKilled {
                kind: prey_cfg.kind,
                position: prey_pos,
            });
            if let Some(ref mut elog) = event_log {
                elog.push(
                    time.tick,
                    EventKind::PreyKilled {
                        cat: name.0.clone(),
                        species: species_name.to_string(),
                        location: (prey_pos.x(), prey_pos.y()),
                    },
                );
            }

            let catch_desc = if consumed_in_place {
                if catch_corruption > 0.3 {
                    format!(
                        "{} catches and devours a corrupted {} where it falls.",
                        name.0, species_name
                    )
                } else {
                    format!(
                        "{} catches and eats a {} on the spot.",
                        name.0, species_name
                    )
                }
            } else if catch_corruption > 0.3 {
                format!("{} catches a corrupted {}.", name.0, species_name)
            } else {
                format!("{} catches a {}.", name.0, species_name)
            };
            emit_hunt_narrative(
                narr,
                time,
                rng,
                map,
                pos,
                name,
                gender,
                personality,
                needs,
                "catch",
                &catch_desc,
                Some(species_name),
                None,
            );

            if consumed_in_place {
                // Plan dies here — the cat's hunt-and-deposit chain is
                // moot once the prey is consumed. Fail forces a replan;
                // the now-fed cat re-elects (typically Resting for
                // sleep/groom, or Hunt again if drives are still high).
                state.target_entity = None;
                record_hunt_attempt(
                    event_log.as_deref_mut(),
                    narr.activation.as_deref_mut(),
                    Some(&mut narr.witnessable),
                    &name.0,
                    cat_entity,
                    species_name,
                    prey_kind,
                    prey_pos,
                    HuntOutcome::KilledAndConsumed,
                    time.tick,
                    ticks,
                    start_distance,
                    None,
                );
                return crate::steps::StepResult::Fail("consumed catch in place".into());
            }
            if inventory.is_full() {
                record_hunt_attempt(
                    event_log.as_deref_mut(),
                    narr.activation.as_deref_mut(),
                    Some(&mut narr.witnessable),
                    &name.0,
                    cat_entity,
                    species_name,
                    prey_kind,
                    prey_pos,
                    HuntOutcome::Killed,
                    time.tick,
                    ticks,
                    start_distance,
                    None,
                );
                return crate::steps::StepResult::Advance;
            } else {
                // Multi-kill: reset target, keep searching.
                state.target_entity = None;
                record_hunt_attempt(
                    event_log.as_deref_mut(),
                    narr.activation.as_deref_mut(),
                    Some(&mut narr.witnessable),
                    &name.0,
                    cat_entity,
                    species_name,
                    prey_kind,
                    prey_pos,
                    HuntOutcome::KilledAndReplanned,
                    time.tick,
                    ticks,
                    start_distance,
                    None,
                );
                return crate::steps::StepResult::Fail("seeking another target".into());
            }
        } else {
            // Miss — prey bolts.
            // 477 — a fragile (bone) weapon may snap on the failed strike.
            // Deterministic-from-state roll (no fresh affix): fragile gate
            // + per-strike snap chance. On snap, remove the weapon from its
            // worn slot (017 — items-are-real: the wielded tool is gone) and
            // fire the durability canary.
            let snapped = em.weapon.and_then(|w| {
                if w.fragile && rng.rng.random::<f32>() < combat.bone_weapon_snap_chance_on_miss {
                    w.kind.equip_slot().and_then(|slot| wearables.take(slot))
                } else {
                    None
                }
            });
            if snapped.is_some() {
                if let Some(sink) = focal_sink {
                    sink.record(
                        cat_entity,
                        "resolve_engage_prey",
                        "weapon.bone_snap",
                        1.0,
                        0.0,
                    );
                }
                if let Some(act) = narr.activation.as_deref_mut() {
                    act.record(crate::resources::system_activation::Feature::BoneWeaponSnapped);
                }
                emit_hunt_narrative(
                    narr,
                    time,
                    rng,
                    map,
                    pos,
                    name,
                    gender,
                    personality,
                    needs,
                    "miss",
                    &format!("{}'s bone weapon snaps against the strike.", name.0),
                    Some(species_name),
                    None,
                );
            }
            if let Ok((_, _, _, mut prey_st)) = prey_query.get_mut(target_entity) {
                prey_st.ai_state = PreyAiState::Fleeing {
                    from: cat_entity,
                    toward: None,
                    ticks: 0,
                };
            }

            emit_hunt_narrative(
                narr,
                time,
                rng,
                map,
                pos,
                name,
                gender,
                personality,
                needs,
                "miss",
                &format!("{}'s quarry bolts.", name.0),
                Some(species_name),
                None,
            );

            let chase_limit = if personality.boldness > 0.7 {
                d.chase_limit_bold
            } else {
                d.chase_limit_default
            };
            if ticks > chase_limit {
                record_hunt_attempt(
                    event_log.as_deref_mut(),
                    narr.activation.as_deref_mut(),
                    Some(&mut narr.witnessable),
                    &name.0,
                    cat_entity,
                    species_name,
                    prey_kind,
                    prey_pos,
                    HuntOutcome::LostDuringChase,
                    time.tick,
                    ticks,
                    start_distance,
                    Some("chase timeout".into()),
                );
                return crate::steps::StepResult::Fail("chase timeout".into());
            }
        }
    } else if dist <= stalk_start {
        if prey_is_fleeing {
            // === CHASE ===
            let mut moved = false;
            for _ in 0..d.chase_speed {
                if let Some(next) = step_toward(pos, &prey_pos, map, &cat_overlays) {
                    *pos = next;
                    moved = true;
                }
            }
            if moved {
                state.no_move_ticks = 0;
            } else {
                state.no_move_ticks += 1;
            }
            if state.no_move_ticks > d.chase_stuck_ticks {
                record_hunt_attempt(
                    event_log.as_deref_mut(),
                    narr.activation.as_deref_mut(),
                    Some(&mut narr.witnessable),
                    &name.0,
                    cat_entity,
                    species_name,
                    prey_kind,
                    prey_pos,
                    HuntOutcome::LostDuringChase,
                    time.tick,
                    ticks,
                    start_distance,
                    Some("stuck while chasing".into()),
                );
                return crate::steps::StepResult::Fail("stuck while chasing".into());
            }
            let chase_limit = if personality.boldness > 0.7 {
                d.chase_limit_bold
            } else {
                d.chase_limit_default
            };
            if ticks > chase_limit {
                record_hunt_attempt(
                    event_log.as_deref_mut(),
                    narr.activation.as_deref_mut(),
                    Some(&mut narr.witnessable),
                    &name.0,
                    cat_entity,
                    species_name,
                    prey_kind,
                    prey_pos,
                    HuntOutcome::LostDuringChase,
                    time.tick,
                    ticks,
                    start_distance,
                    Some("chase timeout".into()),
                );
                return crate::steps::StepResult::Fail("chase timeout".into());
            }
        } else {
            // === STALK ===
            // 100: stamp Action::Stalk so this tick's `tremor_tick`
            // deposit uses the stalk (≈0.2×) multiplier. The load-
            // bearing bit: a stalking cat barely registers on the
            // tremor map, so prey can't alert on the cat's motion
            // alone — only sight at close range remains.
            current_action.action = Action::Stalk;
            let mut moved = false;
            if let Some(next) = step_toward(pos, &prey_pos, map, &cat_overlays) {
                *pos = next;
                moved = true;
            }
            if personality.anxiety > d.anxiety_spook_threshold
                && rng.rng.random::<f32>() < d.anxiety_spook_chance
            {
                if let Ok((_, _, _, mut prey_st)) = prey_query.get_mut(target_entity) {
                    prey_st.ai_state = PreyAiState::Fleeing {
                        from: cat_entity,
                        toward: None,
                        ticks: 0,
                    };
                }
                record_hunt_attempt(
                    event_log.as_deref_mut(),
                    narr.activation.as_deref_mut(),
                    Some(&mut narr.witnessable),
                    &name.0,
                    cat_entity,
                    species_name,
                    prey_kind,
                    prey_pos,
                    HuntOutcome::LostDuringStalk,
                    time.tick,
                    ticks,
                    start_distance,
                    Some("anxiety spooked prey".into()),
                );
                return crate::steps::StepResult::Fail("anxiety spooked prey".into());
            }
            if moved {
                state.no_move_ticks = 0;
            } else {
                state.no_move_ticks += 1;
            }
            if state.no_move_ticks > d.chase_stuck_ticks {
                record_hunt_attempt(
                    event_log.as_deref_mut(),
                    narr.activation.as_deref_mut(),
                    Some(&mut narr.witnessable),
                    &name.0,
                    cat_entity,
                    species_name,
                    prey_kind,
                    prey_pos,
                    HuntOutcome::LostDuringStalk,
                    time.tick,
                    ticks,
                    start_distance,
                    Some("stuck while stalking".into()),
                );
                return crate::steps::StepResult::Fail("stuck while stalking".into());
            }
        }
    } else {
        // === APPROACH ===
        let mut moved = false;
        for _ in 0..d.approach_speed {
            // Greedy step_toward returns None in a concave-terrain local-minimum
            // (rustdoc on `step_toward`). Without a fallback the cat freezes for
            // `chase_stuck_ticks` and bails. A few specific map tiles on any
            // given seed catch many repeated hunt attempts at the same trap —
            // see ticket 465. Fall back to A* once; take just the next step.
            let next = step_toward(pos, &prey_pos, map, &cat_overlays).or_else(|| {
                find_path(*pos, prey_pos, map, &cat_overlays).and_then(|p| p.into_iter().next())
            });
            if let Some(next) = next {
                *pos = next;
                moved = true;
            } else {
                break;
            }
        }
        if moved {
            state.no_move_ticks = 0;
        } else {
            state.no_move_ticks += 1;
        }
        if dist > d.approach_give_up_distance as i32 || state.no_move_ticks > d.chase_stuck_ticks {
            let reason = if dist > d.approach_give_up_distance as i32 {
                "lost prey during approach"
            } else {
                "stuck during approach"
            };
            record_hunt_attempt(
                event_log.as_deref_mut(),
                narr.activation.as_deref_mut(),
                Some(&mut narr.witnessable),
                &name.0,
                cat_entity,
                species_name,
                prey_kind,
                prey_pos,
                HuntOutcome::LostDuringApproach,
                time.tick,
                ticks,
                start_distance,
                Some(reason.into()),
            );
            return crate::steps::StepResult::Fail("lost prey during approach".into());
        }
    }

    crate::steps::StepResult::Continue
}

// ===========================================================================
// Helper: resolve ForageItem (transplanted from ForageItem step)
// ===========================================================================

#[allow(clippy::too_many_arguments)]
fn resolve_forage_item(
    state: &mut StepExecutionState,
    ticks: u64,
    pos: &mut Position,
    inventory: &mut Inventory,
    skills: &mut Skills,
    map: &TileMap,
    narr: &mut NarrativeEmitter<'_>,
    time: &TimeState,
    rng: &mut SimRng,
    personality: &Personality,
    name: &Name,
    gender: &Gender,
    // 150 R1: `&mut Needs` (was `&Needs`) — see resolve_engage_prey for
    // the consume-on-spot branch this enables.
    needs: &mut Needs,
    d: &DispositionConstants,
    // 176: spawn the foraged `Item` entity at the cat's position
    // when inventory is full (instead of the silent-skip path that
    // pre-176 dropped the item entirely). Items-are-real invariant
    // says every successful forage produces a real entity even when
    // the cat can't carry it — leaves a real `OnGround` item for a
    // future `Action::PickUp` to retrieve.
    commands: &mut Commands,
) -> crate::steps::StepResult {
    use crate::components::items::ItemKind;

    let (mut dx, mut dy) = state.patrol_dir;
    if dx == 0 && dy == 0 {
        dx = 1;
    }
    if rng.rng.random::<f32>() < d.forage_jitter_chance {
        dx = rng.rng.random_range(-1i32..=1);
        dy = rng.rng.random_range(-1i32..=1);
        if dx == 0 && dy == 0 {
            dx = 1;
        }
    }
    *pos = patrol_move(pos, dx, dy, map);

    if map.in_bounds(pos.x(), pos.y()) {
        let tile = map.get(pos.x(), pos.y());
        let forage_yield = tile.terrain.foraging_yield() * (1.0 - tile.corruption).max(0.0);
        if forage_yield > 0.0 && rng.rng.random::<f32>() < forage_yield * d.forage_yield_scale {
            let item_kind = match tile.terrain {
                Terrain::DenseForest => {
                    if rng.rng.random::<bool>() {
                        ItemKind::Mushroom
                    } else {
                        ItemKind::Nuts
                    }
                }
                Terrain::LightForest => {
                    if rng.rng.random::<bool>() {
                        ItemKind::Nuts
                    } else {
                        ItemKind::Berries
                    }
                }
                _ => {
                    if rng.rng.random::<bool>() {
                        ItemKind::Berries
                    } else {
                        ItemKind::Roots
                    }
                }
            };
            let forage_corruption = if map.in_bounds(pos.x(), pos.y()) {
                map.get(pos.x(), pos.y()).corruption
            } else {
                0.0
            };

            // 150 R1 — eat-the-catch (forage edition). A hungry cat
            // consumes the foraged item on the spot rather than walking
            // it home. Mirror's `resolve_eat_at_stores` arithmetic;
            // foraged items are raw so no cooked multiplier applies.
            let consumed_in_place = needs.hunger < d.production_self_eat_threshold;
            let modifiers =
                crate::components::items::ItemModifiers::with_corruption(forage_corruption);
            if consumed_in_place {
                let freshness = (1.0 - forage_corruption * d.corruption_food_penalty).max(0.0);
                needs.hunger = (needs.hunger + item_kind.food_value() * freshness).min(1.0);
                if let Some(act) = narr.activation.as_deref_mut() {
                    act.record(crate::resources::system_activation::Feature::FoodEaten);
                }
            } else {
                // Ticket 429: items-are-real Source gate. The trait's
                // default push-or-overflow body unifies the prior
                // inventory-push vs ground-spawn arms.
                use crate::components::item_gate::sources::ForageCatchSource;
                use crate::components::item_gate::{ItemSource, SourceCtx, SourcePlacement};
                let forage_pos = *pos;
                let outcome = ForageCatchSource {
                    kind: item_kind,
                    modifiers,
                }
                .source(&mut SourceCtx {
                    inventory: Some(&mut *inventory),
                    commands: &mut *commands,
                    default_position: forage_pos,
                });
                outcome.record_if_witnessed(
                    narr.activation.as_deref_mut(),
                    ForageCatchSource::FEATURE,
                );
                if matches!(outcome.witness, Some(SourcePlacement::Ground { .. })) {
                    if let Some(act) = narr.activation.as_deref_mut() {
                        act.record(crate::resources::system_activation::Feature::OverflowToGround);
                    }
                }
            }
            skills.foraging += skills.growth_rate() * d.forage_skill_growth;

            // 368 — Phase 2 ingredient drop. Terrain-conditioned
            // secondary spawn alongside the foraged food: woody tiles
            // shed Twigs; Grass tiles offer Fiber or Flower. Drops as
            // an `OnGround` `Item` entity at the cat's current tile,
            // keeping food inventory pressure unchanged. A subsequent
            // cat can plan `Action::PickUp` to gather it for the
            // Workshop behavioral-tool recipes.
            //
            // RNG-frugal ordering: roll the drop-chance first; only
            // consume the Fiber/Flower selection RNG on Grass tiles
            // where a drop actually lands. Minimises seed-42
            // perturbation vs the pre-368 baseline.
            let ingredient_terrain_eligible = matches!(
                tile.terrain,
                Terrain::DenseForest | Terrain::LightForest | Terrain::Grass
            );
            if ingredient_terrain_eligible
                && rng.rng.random::<f32>() < d.forage_ingredient_drop_chance
            {
                let ing = match tile.terrain {
                    Terrain::DenseForest | Terrain::LightForest => ItemKind::Twig,
                    Terrain::Grass => {
                        if rng.rng.random::<bool>() {
                            ItemKind::Fiber
                        } else {
                            ItemKind::Flower
                        }
                    }
                    _ => unreachable!("guarded by ingredient_terrain_eligible above"),
                };
                let ing_modifiers =
                    crate::components::items::ItemModifiers::with_corruption(forage_corruption);
                // Ticket 482: herbcraft-ingredient drop sourced through
                // the items-are-real gate. `AlwaysGround` policy — the
                // cat is foraging the primary catch (already sourced
                // above via ForageCatchSource); the ingredient is a
                // tile-side world emission, not a pickup. Inventory
                // stays `Some(&mut *inventory)` for consistency with
                // sibling sites, but the policy bypasses it.
                use crate::components::item_gate::sources::ForageIngredientSource;
                use crate::components::item_gate::{ItemSource, SourceCtx};
                let ingredient_pos = *pos;
                let outcome = ForageIngredientSource {
                    kind: ing,
                    modifiers: ing_modifiers,
                }
                .source(&mut SourceCtx {
                    inventory: Some(&mut *inventory),
                    commands: &mut *commands,
                    default_position: ingredient_pos,
                });
                outcome.record_if_witnessed(
                    narr.activation.as_deref_mut(),
                    ForageIngredientSource::FEATURE,
                );
                // No `OverflowToGround` emission — `AlwaysGround` by
                // construction; the canary stays meaningful only on
                // `InventoryFirst` sites that overflow under pouch-full.
            }

            let item_name = if consumed_in_place {
                if forage_corruption > 0.3 {
                    format!("eats a corrupted {} on the spot", item_kind.name())
                } else {
                    format!("nibbles {} where they grow", item_kind.name())
                }
            } else if forage_corruption > 0.3 {
                format!("corrupted {}", item_kind.name())
            } else {
                item_kind.name().to_string()
            };
            let terrain = if map.in_bounds(pos.x(), pos.y()) {
                map.get(pos.x(), pos.y()).terrain
            } else {
                Terrain::Grass
            };
            let day_phase = DayPhase::from_tick(time.tick, &narr.config);
            let season = Season::from_tick(time.tick, &narr.config);
            let ctx = TemplateContext {
                action: Action::Forage,
                day_phase,
                season,
                weather: narr.weather.current,
                mood_bucket: MoodBucket::Neutral,
                life_stage: LifeStage::Adult,
                has_target: false,
                terrain,
                event: Some("find".into()),
            };
            let var_ctx = VariableContext {
                name: &name.0,
                gender: *gender,
                weather: narr.weather.current,
                day_phase,
                season,
                life_stage: LifeStage::Adult,
                fur_color: "unknown",
                other: None,
                prey: None,
                item: Some(&item_name),
                item_singular: Some(item_kind.singular_name()),
                quality: None,
            };
            let fallback = if consumed_in_place {
                format!("{} {}.", name.0, item_name)
            } else {
                format!("{} finds {}.", name.0, item_name)
            };
            emit_event_narrative(
                narr.registry.as_deref(),
                &mut narr.log,
                time.tick,
                fallback,
                crate::resources::narrative::NarrativeTier::Action,
                &ctx,
                &var_ctx,
                personality,
                needs,
                &mut rng.rng,
            );
            // 150 R1: consumed-in-place fails the plan to force replan
            // (no item to deposit; cat is now fed). Otherwise advance to
            // the deposit step.
            return if consumed_in_place {
                crate::steps::StepResult::Fail("consumed forage in place".into())
            } else {
                crate::steps::StepResult::Advance
            };
        }
    }

    if ticks > d.forage_timeout_ticks {
        return crate::steps::StepResult::Fail("nothing found while foraging".into());
    }

    crate::steps::StepResult::Continue
}

// ===========================================================================
// Helper: narrative emission
// ===========================================================================

#[allow(clippy::too_many_arguments)]
fn emit_hunt_narrative(
    narr: &mut NarrativeEmitter<'_>,
    time: &TimeState,
    rng: &mut SimRng,
    map: &TileMap,
    pos: &Position,
    name: &Name,
    gender: &Gender,
    personality: &Personality,
    needs: &Needs,
    event: &str,
    fallback: &str,
    prey: Option<&str>,
    item: Option<&str>,
) {
    let terrain = if map.in_bounds(pos.x(), pos.y()) {
        map.get(pos.x(), pos.y()).terrain
    } else {
        Terrain::Grass
    };
    let day_phase = DayPhase::from_tick(time.tick, &narr.config);
    let season = Season::from_tick(time.tick, &narr.config);
    let ctx = TemplateContext {
        action: Action::Hunt,
        day_phase,
        season,
        weather: narr.weather.current,
        mood_bucket: MoodBucket::Neutral,
        life_stage: LifeStage::Adult,
        has_target: prey.is_some(),
        terrain,
        event: Some(event.into()),
    };
    let var_ctx = VariableContext {
        name: &name.0,
        gender: *gender,
        weather: narr.weather.current,
        day_phase,
        season,
        life_stage: LifeStage::Adult,
        fur_color: "unknown",
        other: None,
        prey,
        item,
        item_singular: None,
        quality: None,
    };
    let tier = if event == "catch" || event == "raid" {
        crate::resources::narrative::NarrativeTier::Action
    } else {
        crate::resources::narrative::NarrativeTier::Micro
    };
    emit_event_narrative(
        narr.registry.as_deref(),
        &mut narr.log,
        time.tick,
        fallback.to_string(),
        tier,
        &ctx,
        &var_ctx,
        personality,
        needs,
        &mut rng.rng,
    );
}

// ===========================================================================
// Spatial helpers (transplanted from disposition.rs)
// ===========================================================================

fn patrol_move(pos: &Position, dx: i32, dy: i32, map: &TileMap) -> Position {
    let primary = Position::new(pos.x() + dx, pos.y() + dy);
    if map.in_bounds(primary.x(), primary.y())
        && map.get(primary.x(), primary.y()).terrain.is_passable()
    {
        return primary;
    }
    let perp = Position::new(pos.x() + dy, pos.y() + dx);
    if map.in_bounds(perp.x(), perp.y()) && map.get(perp.x(), perp.y()).terrain.is_passable() {
        return perp;
    }
    let rev = Position::new(pos.x() - dx, pos.y() - dy);
    if map.in_bounds(rev.x(), rev.y()) && map.get(rev.x(), rev.y()).terrain.is_passable() {
        return rev;
    }
    *pos
}

// §4 batch 2: scoring-path callers retired (capability markers replace them);
// kept for test coverage of `find_nearest_tile`.
#[cfg(test)]
fn has_nearby_tile(
    from: &Position,
    map: &TileMap,
    radius: i32,
    predicate: impl Fn(Terrain) -> bool,
) -> bool {
    find_nearest_tile(from, map, radius, predicate).is_some()
}

/// splitmix64 finalizer — pure function, no state, strong avalanche.
/// Used to deterministically break ties among equidistant candidates in
/// `find_nearest_tile` without consuming the global RNG stream.
fn mix_hash(a: i32, b: i32, c: i32, d: i32) -> u64 {
    let mut x = (a as u32 as u64)
        ^ ((b as u32 as u64) << 32)
        ^ (c as u32 as u64).rotate_left(16)
        ^ ((d as u32 as u64) << 48);
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn find_nearest_tile(
    from: &Position,
    map: &TileMap,
    radius: i32,
    predicate: impl Fn(Terrain) -> bool,
) -> Option<Position> {
    let mut best: Option<(Position, i32, u64)> = None;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let p = Position::new(from.x() + dx, from.y() + dy);
            if !map.in_bounds(p.x(), p.y()) {
                continue;
            }
            let tile = map.get(p.x(), p.y());
            if !predicate(tile.terrain) {
                continue;
            }
            let dist = from.chebyshev_distance(&p);
            if dist == 0 {
                continue;
            }
            let tie = mix_hash(from.x(), from.y(), p.x(), p.y());
            let replace = match best {
                None => true,
                Some((_, d, _)) if dist < d => true,
                Some((_, d, t)) if dist == d && tie < t => true,
                _ => false,
            };
            if replace {
                best = Some((p, dist, tie));
            }
        }
    }
    best.map(|(p, _, _)| p)
}

fn find_random_nearby_tile(
    from: &Position,
    map: &TileMap,
    radius: i32,
    predicate: impl Fn(Terrain) -> bool,
    rng: &mut impl Rng,
) -> Option<Position> {
    let mut candidates: Vec<(Position, f32)> = Vec::new();
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let p = Position::new(from.x() + dx, from.y() + dy);
            if map.in_bounds(p.x(), p.y()) {
                let tile = map.get(p.x(), p.y());
                if predicate(tile.terrain) {
                    let dist = from.chebyshev_distance(&p);
                    if dist > 0 {
                        candidates.push((p, 1.0 / (dist as f32 * dist as f32)));
                    }
                }
            }
        }
    }
    if candidates.is_empty() {
        return None;
    }
    let total: f32 = candidates.iter().map(|(_, w)| w).sum();
    let mut roll: f32 = rng.random::<f32>() * total;
    for (pos, weight) in &candidates {
        roll -= weight;
        if roll <= 0.0 {
            return Some(*pos);
        }
    }
    Some(candidates.last().unwrap().0)
}

fn respect_for_disposition(kind: DispositionKind, d: &DispositionConstants) -> f32 {
    match kind {
        DispositionKind::Hunting => d.respect_gain_hunting,
        DispositionKind::Foraging => d.respect_gain_foraging,
        DispositionKind::Guarding => d.respect_gain_guarding,
        DispositionKind::Building => d.respect_gain_building,
        DispositionKind::Coordinating => d.respect_gain_coordinating,
        DispositionKind::Socializing => d.respect_gain_socializing,
        _ => 0.0,
    }
}

// ===========================================================================
// Zone resolution and planner state construction
// ===========================================================================

/// Substrate-aligned `PlannerZone::Wilds` resolves through the same
/// `ExplorationMap::frontier_centroid` the IAUS `Explore` DSE scores
/// against (`LandmarkAnchor::UnexploredFrontierCentroid` →
/// `src/ai/scoring.rs`). Closes the L2↔L3 feasibility-language drift
/// Pick a passable perimeter tile at Chebyshev `offset` from `anchor`,
/// trying cardinal directions in `[+x, -x, +y, -y]` order. Returns the
/// first in-bounds passable candidate, or `None` if all four are
/// blocked.
///
/// Ticket 494 — used by the [`PlannerZone::PatrolZone`] and
/// [`PlannerZone::RestingSpot`] branches of [`resolve_zone_position`].
/// Pre-494 those branches did a blind `Position::new(sp.x() + offset,
/// sp.y())` that landed on water / wall / out-of-bounds whenever the
/// store hugged the map edge or sat against a wall, stamping
/// unreachable plan targets and surfacing 500+ "no path and stuck"
/// `PlanStepFailed` events per 15-min soak under the patrol DSE's
/// post-492 elevation.
///
/// Direction priority preserves the pre-494 "+x preferred" bias so
/// stores along the colony's east edge still draw cats to their west
/// side rather than rotating placement; only when +x is blocked do we
/// rotate. If all four cardinals are blocked the caller filters this
/// store out of the iterator and the picker falls through to the next
/// nearest store with a usable perimeter.
fn perimeter_offset_position(anchor: &Position, offset: i32, map: &TileMap) -> Option<Position> {
    for (dx, dy) in [(offset, 0), (-offset, 0), (0, offset), (0, -offset)] {
        let p = Position::new(anchor.x() + dx, anchor.y() + dy);
        if map.in_bounds(p.x(), p.y()) && map.get(p.x(), p.y()).terrain.is_passable() {
            return Some(p);
        }
    }
    None
}

/// `find_nearest_tile(...).or(Some(*pos))` previously authored: when no
/// frontier and no nearby passable tile resolves, returns `None` so the
/// planner surfaces `no_plan_found` instead of stamping a degenerate
/// self-target. Ticket 121 (substrate-over-override epic 093).
#[allow(clippy::too_many_arguments)]
fn resolve_zone_position(
    zone: PlannerZone,
    pos: &Position,
    map: &TileMap,
    exploration_map: &ExplorationMap,
    stores_positions: &[Position],
    construction_positions: &[(Entity, Position)],
    farm_positions: &[Position],
    herb_positions: &[(Entity, Position, HerbKind)],
    kitchen_positions: &[Position],
    cat_positions: &[(Entity, Position)],
    material_pile_positions: &[(Entity, Position, ItemKind)],
    food_pile_positions: &[(Entity, Position, ItemKind)],
    // 367: preservation-station positions for zone resolution.
    drying_rack_positions: &[Position],
    smoking_rack_positions: &[Position],
    // 457: Workshop positions for `PlannerZone::Workshop` zone resolution.
    workshop_positions: &[Position],
    // 369: TanningFrame positions for `PlannerZone::TanningFrame`.
    tanning_frame_positions: &[Position],
    // 035: Dead-and-not-Buried cat positions for the `CorpseTarget`
    // zone. Disjoint from `cat_positions` (which is `Without<Dead>`).
    dead_cat_positions: &[(Entity, Position)],
    cat_entity: Entity,
    d: &DispositionConstants,
) -> Option<Position> {
    match zone {
        PlannerZone::Stores => stores_positions
            .iter()
            .min_by_key(|sp| pos.tile_distance_squared(sp))
            .copied(),
        PlannerZone::HuntingGround => {
            find_nearest_tile(pos, map, d.hunt_terrain_search_radius as i32, |t| {
                matches!(t, Terrain::DenseForest | Terrain::LightForest)
            })
        }
        PlannerZone::ForagingGround => {
            find_nearest_tile(pos, map, d.forage_terrain_search_radius as i32, |t| {
                t.foraging_yield() > 0.0
            })
        }
        PlannerZone::Farm => farm_positions
            .iter()
            .min_by_key(|fp| pos.tile_distance_squared(fp))
            .copied(),
        PlannerZone::ConstructionSite => construction_positions
            .iter()
            .min_by_key(|(_, cp)| pos.tile_distance_squared(cp))
            .map(|(_, p)| *p),
        PlannerZone::HerbPatch => herb_positions
            .iter()
            .min_by_key(|(_, hp, _)| pos.tile_distance_squared(hp))
            .map(|(_, p, _)| *p),
        PlannerZone::Kitchen => kitchen_positions
            .iter()
            .min_by_key(|kp| pos.tile_distance_squared(kp))
            .copied(),
        // Ticket 494 — pre-494 picked the nearest store, then blindly
        // offset +x by 1 to land "next to" it. If that +x tile was
        // water / wall / out-of-bounds, the planner stamped an
        // unreachable target and the resolver burned through cycles
        // failing pathfinding. Now filter to stores whose perimeter
        // has at least one passable cardinal tile and return that
        // tile, preserving the "next to the store" semantic without
        // the blind-offset reachability hazard.
        PlannerZone::RestingSpot => stores_positions
            .iter()
            .filter_map(|sp| perimeter_offset_position(sp, 1, map).map(|p| (sp, p)))
            .min_by_key(|(sp, _)| pos.tile_distance_squared(sp))
            .map(|(_, p)| p)
            .or(Some(*pos)),
        PlannerZone::SocialTarget => cat_positions
            .iter()
            .filter(|(other, _)| *other != cat_entity)
            .min_by_key(|(_, op)| pos.tile_distance_squared(op))
            .map(|(_, p)| *p),
        PlannerZone::Wilds => exploration_map
            .frontier_centroid()
            .filter(|p| map.in_bounds(p.x(), p.y()) && map.get(p.x(), p.y()).terrain.is_passable())
            .or_else(|| find_nearest_tile(pos, map, 20, |t| t.is_passable())),
        // Ticket 494 — same shape as RestingSpot. Pre-494 the nearest
        // store's `+guard_patrol_radius` x-offset was the patrol
        // anchor; with the patrol DSE rebalanced under the Chebyshev
        // realignment (and any future system that elevates patrol),
        // an unreachable +x offset surfaces 500+ "no path and stuck"
        // failures per soak when the store hugs the map edge or sits
        // against a wall.
        PlannerZone::PatrolZone => stores_positions
            .iter()
            .filter_map(|sp| {
                perimeter_offset_position(sp, d.guard_patrol_radius as i32, map).map(|p| (sp, p))
            })
            .min_by_key(|(sp, _)| pos.tile_distance_squared(sp))
            .map(|(_, p)| p)
            .or(Some(*pos)),
        // Ticket 495 — filter to passable in-bounds tiles before the
        // nearest-pick. Material piles can sit at construction or
        // demolition sites; if a site spans an impassable cell (water-
        // adjacent build, wall under construction) the pile entity may
        // outlive the source tile's passability. Parallel symmetry
        // with the CarcassPile fix below; no documented failure rate
        // for MaterialPile yet.
        PlannerZone::MaterialPile => material_pile_positions
            .iter()
            .filter(|(_, mp, _)| {
                map.in_bounds(mp.x(), mp.y()) && map.get(mp.x(), mp.y()).terrain.is_passable()
            })
            .min_by_key(|(_, mp, _)| pos.tile_distance_squared(mp))
            .map(|(_, p, _)| *p),
        // Ticket 495 — filter to passable in-bounds tiles. Fish
        // carcasses spawn at `prey_pos` which is `Terrain::Water` per
        // `Fish::habitat` (`species/fish.rs:30`). Pre-495 the picker
        // returned the water tile unconditionally; A* refused
        // (`pathfinding.rs:268-273` rejects impassable destinations)
        // and the resolver burned 1099 "no path and stuck" failures
        // per soak. Filter at the picker so the next-nearest
        // *reachable* carcass wins. Submerged-item state and a
        // shore-adjacent Fish spawn site are R4/R5 substrate
        // follow-ons.
        PlannerZone::CarcassPile => food_pile_positions
            .iter()
            .filter(|(_, fp, _)| {
                map.in_bounds(fp.x(), fp.y()) && map.get(fp.x(), fp.y()).terrain.is_passable()
            })
            .min_by_key(|(_, fp, _)| pos.tile_distance_squared(fp))
            .map(|(_, p, _)| *p),
        // 035: nearest unburied colony-mate corpse. The dead-cat
        // snapshot is built upstream in `ScoringSnapshots`. Excludes
        // self defensively (dead self can't be the cat planning).
        PlannerZone::CorpseTarget => dead_cat_positions
            .iter()
            .filter(|(other, _)| *other != cat_entity)
            .min_by_key(|(_, dp)| pos.tile_distance_squared(dp))
            .map(|(_, p)| *p),
        // 367: nearest preservation-station tile. Built-once-per-tick
        // snapshots filter to completed structures only. Load /
        // cooldown discrimination happens at resolver-time — the zone
        // resolver answers "where is the nearest rack of this kind?"
        // and the per-action resolver verifies idle / off-cooldown
        // state when the cat actually arrives. If the rack got loaded
        // mid-plan the resolver fails cleanly and the planner re-picks.
        PlannerZone::DryingRack => drying_rack_positions
            .iter()
            .min_by_key(|dp| pos.tile_distance_squared(dp))
            .copied(),
        PlannerZone::SmokingRack => smoking_rack_positions
            .iter()
            .min_by_key(|sp| pos.tile_distance_squared(sp))
            .copied(),
        // 457: nearest Workshop. Same shape as DryingRack/SmokingRack;
        // recipe selection happens at resolver-time, not zone-resolve-time.
        PlannerZone::Workshop => workshop_positions
            .iter()
            .min_by_key(|wp| pos.tile_distance_squared(wp))
            .copied(),
        // 369: nearest TanningFrame. Same shape as Workshop — recipe
        // selection (HideBracers vs HidePlatedWrap) happens at
        // resolver-time, not zone-resolve-time.
        PlannerZone::TanningFrame => tanning_frame_positions
            .iter()
            .min_by_key(|tp| pos.tile_distance_squared(tp))
            .copied(),
    }
}

/// Find the most corrupted tile within `radius` tiles of `origin`.
/// Returns `None` if no tile has corruption above 0.05.
fn nearest_corrupted_tile(origin: &Position, map: &TileMap, radius: i32) -> Option<Position> {
    let mut best: Option<(Position, f32)> = None;
    for dy in -radius..=radius {
        for dx in -radius..=radius {
            if dx.abs() + dy.abs() > radius {
                continue;
            }
            let p = Position::new(origin.x() + dx, origin.y() + dy);
            if !map.in_bounds(p.x(), p.y()) {
                continue;
            }
            let c = map.get(p.x(), p.y()).corruption;
            if c > 0.05 && best.as_ref().is_none_or(|(_, bc)| c > *bc) {
                best = Some((p, c));
            }
        }
    }
    best.map(|(p, _)| p)
}

#[allow(clippy::too_many_arguments)]
fn build_planner_state(
    pos: &Position,
    needs: &Needs,
    inventory: &Inventory,
    trips_done: u32,
    map: &TileMap,
    stores_positions: &[Position],
    construction_positions: &[(Entity, Position)],
    farm_positions: &[Position],
    herb_positions: &[(Entity, Position, HerbKind)],
    material_pile_positions: &[(Entity, Position, ItemKind)],
    food_pile_positions: &[(Entity, Position, ItemKind)],
    d: &DispositionConstants,
) -> PlannerState {
    let zone = classify_zone(
        pos,
        map,
        stores_positions,
        construction_positions,
        farm_positions,
        herb_positions,
        material_pile_positions,
        food_pile_positions,
    );
    // Ticket 096: the world-fact "this cat's nearest reachable site
    // has `materials_complete()` true" lives in the substrate as the
    // `MaterialsAvailable` marker, authored per-cat in
    // `materials_available_for` at the planner-marker build site.
    // The search-state field `materials_delivered_this_plan` starts
    // false here and is flipped by `DeliverMaterials`'s effect during
    // A* expansion.
    //
    // Ticket 175: the inventory→Carrying projection lives on
    // `Carrying::from_inventory` so the L2 carry-affinity bonus
    // (`scoring::carry_affinity_bonus`) sees exactly the same state
    // the planner does.
    let carrying = Carrying::from_inventory(inventory);

    // `herb_positions` is still consumed above by `classify_zone` for
    // `PlannerZone::HerbPatch` mapping. The prior `thornbriar_available`
    // mirror (consumed by Crafting/SetWard preconditions) was retired in
    // 092 — that precondition now consults the
    // `markers::ThornbriarAvailable` colony marker via
    // `StatePredicate::HasMarker(...)`, which the substrate authors at
    // `evaluate_and_plan` line 941 (and `resolve_goap_plans` per-tick).
    PlannerState {
        zone,
        carrying,
        trips_done,
        hunger_ok: needs.hunger >= d.planner_hunger_ok_threshold,
        energy_ok: needs.energy >= d.planner_energy_ok_threshold,
        temperature_ok: needs.temperature >= d.planner_temperature_ok_threshold,
        interaction_done: false,
        construction_done: false,
        prey_found: false,
        farm_tended: false,
        materials_delivered_this_plan: false,
        // 230: `Fleeing` plans always start without a picked target;
        // `PickFleeTarget` is the first step in `fleeing_actions()`,
        // and `Flee` / `HoldUntilSafe` are gated on it.
        flee_target_picked: false,
        // 231: pickup-class plans always start without a planned drop;
        // the substrate-path variant of pickup actions reads
        // `HasFreeSlot` (the marker) and the plan-path variant reads
        // this search-state flag, set by DropItem-as-prefix.
        has_free_slot_this_plan: false,
        // 463: HaveItem craft plans always start without a planned
        // retrieve; `RetrieveCraftInputs(_)` flips this and unblocks
        // the plan-path arm of `CraftAt<Station>`.
        has_craft_inputs_this_plan: false,
    }
}

/// Ticket 096 substrate authoring: returns whether this cat's nearest
/// reachable construction site has `materials_complete() == true`.
/// Mirrors the per-cat semantics of the old `PlannerState.materials_available`
/// field — if no site is reachable, defaults to `true` so non-Building
/// planning isn't gated by a non-existent fact. Consumed at the
/// planner-marker build site (and `evaluate_and_plan`) to author the
/// `MaterialsAvailable` marker, which `Construct`'s substrate-branch
/// precondition consults.
fn materials_available_for(
    pos: &Position,
    construction_positions: &[(Entity, Position)],
    construction_materials_complete: &HashMap<Entity, bool>,
) -> bool {
    construction_positions
        .iter()
        .min_by_key(|(_, cp)| pos.tile_distance_squared(cp))
        .map(|(entity, _)| {
            construction_materials_complete
                .get(entity)
                .copied()
                .unwrap_or(true)
        })
        .unwrap_or(true)
}

/// Ticket 235 substrate authoring: returns whether at least one `Stores`
/// building is within `radius` Manhattan tiles of this cat. Authors the
/// per-cat `HasHerbStashAccessible` marker, which the deposit-prefix
/// branch of pickup-class plan templates reads to decide whether
/// `[TravelTo(Stores), DepositHerbs(prefix), <goal>]` is a viable
/// alternative to `[DropItem, <goal>]` for freeing an inventory slot.
///
/// Returns `false` when no Stores exist (degenerate early-game state)
/// — the deposit branch becomes structurally inapplicable, A\* falls
/// back to DropItem.
///
/// Mirrors the per-cat geometric shape of `materials_available_for`.
/// Pathfinding is left to the resolver at execution time; Manhattan is
/// the right grain for plan-template gating.
fn herb_stash_accessible_for(pos: &Position, stores_positions: &[Position], radius: f32) -> bool {
    stores_positions
        .iter()
        .any(|sp| pos.distance_to(sp) <= radius)
}

/// 487 substrate authoring: returns whether at least one peer is a
/// viable allogrooming target for `entity` at `pos`. Authors the per-
/// cat `HasGroomCandidate` marker, which gates `GroomOtherDse`
/// eligibility so the broad-phase `has_social_target` admission can't
/// collapse the founder cohort into chain-grooming dominance (the
/// "cuddle puddle" 484 unmasked).
///
/// Predicate mirrors the candidate-pool slice of
/// `resolve_groom_other_target` (within
/// `GROOM_OTHER_TARGET_RANGE` Manhattan tiles, excluding self) plus a
/// narrowing the resolver doesn't apply at target-pick time:
/// `currently_groomed` excludes peers who are themselves the target of
/// another cat's in-flight `GroomOther` step. That makes the marker
/// strictly stricter than the resolver — a cat won't *start* allo-
/// grooming if every in-range peer is already mid-pile; existing
/// chains still complete. Predicate stays cheap (one O(neighbors)
/// HashSet probe per cat) so authoring is O(N²) worst-case across the
/// colony, same as the existing `update_target_existence_markers`
/// neighbor scan.
///
/// Does NOT filter `Incapacitated` peers here — the actor-side
/// `forbid(Incapacitated)` on `GroomOtherDse` already handles the
/// self-incapacitated case, and the target-side `temperature_lookup`
/// in `resolve_groom_other_target` skips peers without `Needs`
/// (dead / incapacitated → skipped at resolver time). Tightening the
/// marker further is a follow-on if a verdict gate ever wants it.
fn viable_groom_candidate_for(
    entity: Entity,
    pos: &Position,
    cat_positions: &[(Entity, Position)],
    currently_groomed: &std::collections::HashSet<Entity>,
) -> bool {
    let range = crate::ai::dses::groom_other_target::GROOM_OTHER_TARGET_RANGE as i32;
    cat_positions.iter().any(|(other, other_pos)| {
        *other != entity
            && !currently_groomed.contains(other)
            && pos.chebyshev_distance(other_pos) <= range
    })
}

#[allow(clippy::too_many_arguments)]
fn classify_zone(
    pos: &Position,
    map: &TileMap,
    stores_positions: &[Position],
    construction_positions: &[(Entity, Position)],
    farm_positions: &[Position],
    herb_positions: &[(Entity, Position, HerbKind)],
    material_pile_positions: &[(Entity, Position, ItemKind)],
    food_pile_positions: &[(Entity, Position, ItemKind)],
) -> PlannerZone {
    if stores_positions
        .iter()
        .any(|sp| pos.chebyshev_distance(sp) <= 2)
    {
        return PlannerZone::Stores;
    }
    // Ticket 193: CarcassPile classifies ahead of ConstructionSite so a
    // food-Item dropped near a founding site (overflow on a kill near
    // a wagon-dismantling layout) is recognised as the pickup target,
    // not the construction zone. Mirrors `MaterialPile`'s classify
    // radius (≤ 1 tile) for the same reason.
    if food_pile_positions
        .iter()
        .any(|(_, fp, _)| pos.chebyshev_distance(fp) <= 1)
    {
        return PlannerZone::CarcassPile;
    }
    // MaterialPile classifies before ConstructionSite — a pile placed
    // adjacent to a founding site (the wagon-dismantling layout) sits
    // within the site's classify radius too. The cat's plan needs to
    // see "I'm at a pile" first to gate the pickup action.
    if material_pile_positions
        .iter()
        .any(|(_, mp, _)| pos.chebyshev_distance(mp) <= 1)
    {
        return PlannerZone::MaterialPile;
    }
    if construction_positions
        .iter()
        .any(|(_, cp)| pos.chebyshev_distance(cp) <= 2)
    {
        return PlannerZone::ConstructionSite;
    }
    if farm_positions
        .iter()
        .any(|fp| pos.chebyshev_distance(fp) <= 2)
    {
        return PlannerZone::Farm;
    }
    if herb_positions
        .iter()
        .any(|(_, hp, _)| pos.chebyshev_distance(hp) <= 3)
    {
        return PlannerZone::HerbPatch;
    }
    if map.in_bounds(pos.x(), pos.y()) {
        let terrain = map.get(pos.x(), pos.y()).terrain;
        if matches!(terrain, Terrain::DenseForest | Terrain::LightForest) {
            return PlannerZone::HuntingGround;
        }
        if terrain.foraging_yield() > 0.0 {
            return PlannerZone::ForagingGround;
        }
    }
    PlannerZone::Wilds
}

#[allow(clippy::too_many_arguments)]
fn build_zone_distances(
    pos: &Position,
    map: &TileMap,
    stores_positions: &[Position],
    construction_positions: &[(Entity, Position)],
    farm_positions: &[Position],
    herb_positions: &[(Entity, Position, HerbKind)],
    kitchen_positions: &[Position],
    cat_positions: &[(Entity, Position)],
    material_pile_positions: &[(Entity, Position, ItemKind)],
    food_pile_positions: &[(Entity, Position, ItemKind)],
    drying_rack_positions: &[Position],
    smoking_rack_positions: &[Position],
    workshop_positions: &[Position],
    tanning_frame_positions: &[Position],
    dead_cat_positions: &[(Entity, Position)],
    cat_entity: Entity,
    d: &DispositionConstants,
) -> ZoneDistances {
    let mut distances = ZoneDistances::default();

    let zone_positions: Vec<(PlannerZone, Option<Position>)> = vec![
        (
            PlannerZone::Stores,
            stores_positions
                .iter()
                .min_by_key(|sp| pos.tile_distance_squared(sp))
                .copied(),
        ),
        (
            PlannerZone::HuntingGround,
            find_nearest_tile(pos, map, d.hunt_terrain_search_radius as i32, |t| {
                matches!(t, Terrain::DenseForest | Terrain::LightForest)
            }),
        ),
        (
            PlannerZone::ForagingGround,
            find_nearest_tile(pos, map, d.forage_terrain_search_radius as i32, |t| {
                t.foraging_yield() > 0.0
            }),
        ),
        (
            PlannerZone::Farm,
            farm_positions
                .iter()
                .min_by_key(|fp| pos.tile_distance_squared(fp))
                .copied(),
        ),
        (
            PlannerZone::ConstructionSite,
            construction_positions
                .iter()
                .min_by_key(|(_, cp)| pos.tile_distance_squared(cp))
                .map(|(_, p)| *p),
        ),
        (
            PlannerZone::HerbPatch,
            herb_positions
                .iter()
                .min_by_key(|(_, hp, _)| pos.tile_distance_squared(hp))
                .map(|(_, p, _)| *p),
        ),
        (
            PlannerZone::Kitchen,
            kitchen_positions
                .iter()
                .min_by_key(|kp| pos.tile_distance_squared(kp))
                .copied(),
        ),
        (
            PlannerZone::RestingSpot,
            stores_positions
                .iter()
                .min_by_key(|sp| pos.tile_distance_squared(sp))
                .map(|sp| Position::new(sp.x() + 1, sp.y())),
        ),
        (
            PlannerZone::SocialTarget,
            cat_positions
                .iter()
                .filter(|(other, _)| *other != cat_entity)
                .min_by_key(|(_, op)| pos.tile_distance_squared(op))
                .map(|(_, p)| *p),
        ),
        (PlannerZone::Wilds, Some(*pos)),
        (
            PlannerZone::PatrolZone,
            stores_positions
                .iter()
                .min_by_key(|sp| pos.tile_distance_squared(sp))
                .map(|sp| Position::new(sp.x() + d.guard_patrol_radius as i32, sp.y())),
        ),
        // Ticket 495 — parallel passability filter to keep the
        // DSE-scoring distance in agreement with what
        // `resolve_zone_position` will actually pick. Without this,
        // a Fish carcass on water lifts the CarcassPile nearness
        // axis even though the planner can't reach it.
        (
            PlannerZone::MaterialPile,
            material_pile_positions
                .iter()
                .filter(|(_, mp, _)| {
                    map.in_bounds(mp.x(), mp.y()) && map.get(mp.x(), mp.y()).terrain.is_passable()
                })
                .min_by_key(|(_, mp, _)| pos.tile_distance_squared(mp))
                .map(|(_, p, _)| *p),
        ),
        (
            PlannerZone::CarcassPile,
            food_pile_positions
                .iter()
                .filter(|(_, fp, _)| {
                    map.in_bounds(fp.x(), fp.y()) && map.get(fp.x(), fp.y()).terrain.is_passable()
                })
                .min_by_key(|(_, fp, _)| pos.tile_distance_squared(fp))
                .map(|(_, p, _)| *p),
        ),
        // 035: nearest unburied colony-mate corpse, excluding self.
        // Resolves to None when no Dead-and-not-Buried cat exists in
        // the snapshot — the burial plan template's `ZoneIs(CorpseTarget)`
        // precondition then has no `TravelTo` action that reaches the
        // zone, and the planner returns `GoalUnreachable`.
        (
            PlannerZone::CorpseTarget,
            dead_cat_positions
                .iter()
                .filter(|(other, _)| *other != cat_entity)
                .min_by_key(|(_, dp)| pos.tile_distance_squared(dp))
                .map(|(_, p)| *p),
        ),
        // 367: nearest preservation stations. Resolves to None when
        // none is built — the plan template's `ZoneIs(...)` precondition
        // then yields `GoalUnreachable`, the DSE marker gates upstream
        // catch the same condition and the action doesn't score in
        // the first place.
        (
            PlannerZone::DryingRack,
            drying_rack_positions
                .iter()
                .min_by_key(|p| pos.tile_distance_squared(p))
                .copied(),
        ),
        (
            PlannerZone::SmokingRack,
            smoking_rack_positions
                .iter()
                .min_by_key(|p| pos.tile_distance_squared(p))
                .copied(),
        ),
        // 457: nearest Workshop. Same Resolves-to-None semantics as the
        // 367 preservation racks — eligibility gates upstream prevent
        // the Crafting plan from forming when no Workshop is built.
        (
            PlannerZone::Workshop,
            workshop_positions
                .iter()
                .min_by_key(|p| pos.tile_distance_squared(p))
                .copied(),
        ),
        // 369: nearest TanningFrame. Same semantics as Workshop.
        (
            PlannerZone::TanningFrame,
            tanning_frame_positions
                .iter()
                .min_by_key(|p| pos.tile_distance_squared(p))
                .copied(),
        ),
    ];

    // Build pairwise distances between reachable zones.
    for &(from_zone, from_pos) in &zone_positions {
        let Some(fp) = from_pos else { continue };
        for &(to_zone, to_pos) in &zone_positions {
            if from_zone == to_zone {
                continue;
            }
            let Some(tp) = to_pos else { continue };
            let dist = fp.chebyshev_distance(&tp) as u32;
            let cost = (dist / 3).max(1); // Scale down: 3 tiles ≈ 1 planning cost.
            distances.set(from_zone, to_zone, cost);
        }
    }

    distances
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: on an open map with a permissive predicate, the old
    /// `find_nearest_tile` always returned `(from.x(), from.y() - 1)` because
    /// the row-major scan visits -y neighbors first and the strict `<`
    /// comparison never replaced them. The deterministic tiebreak must
    /// pick a different tile for at least the canonical center origin.
    #[test]
    fn find_nearest_tile_not_north_biased_from_center() {
        let map = TileMap::new(41, 41, Terrain::Grass);
        let center = Position::new(20, 20);
        let result = find_nearest_tile(&center, &map, 20, |t| t.is_passable())
            .expect("open map must have a passable neighbor");
        assert_ne!(
            result,
            Position::new(center.x(), center.y() - 1),
            "tiebreak regressed: still returning the -y neighbor"
        );
    }

    /// Across many origin positions on an all-passable map, the chosen
    /// direction must spread across buckets — no single cardinal direction
    /// captures more than 40% of results. Current (pre-fix) code lands
    /// 100% in the (0, -1) bucket; the splitmix tiebreak should flatten
    /// the distribution.
    #[test]
    fn find_nearest_tile_distributes_directions() {
        let map = TileMap::new(41, 41, Terrain::Grass);
        let mut buckets: std::collections::HashMap<(i32, i32), u32> =
            std::collections::HashMap::new();
        let mut total = 0u32;
        for ox in 5..13 {
            for oy in 5..13 {
                let from = Position::new(ox, oy);
                let Some(p) = find_nearest_tile(&from, &map, 20, |t| t.is_passable()) else {
                    continue;
                };
                let key = ((p.x() - from.x()).signum(), (p.y() - from.y()).signum());
                *buckets.entry(key).or_default() += 1;
                total += 1;
            }
        }
        assert!(
            total >= 60,
            "expected at least 60 sampled origins, got {total}"
        );
        let max_bucket = buckets.values().copied().max().unwrap_or(0);
        let max_ratio = max_bucket as f32 / total as f32;
        assert!(
            max_ratio <= 0.4,
            "direction distribution is still axis-biased: max bucket {max_bucket}/{total} = {max_ratio:.2}, buckets={buckets:?}"
        );
    }

    /// Pure function: identical inputs must produce identical output
    /// across repeated calls. This is the seed-42 reproducibility
    /// contract for the tile picker.
    #[test]
    fn find_nearest_tile_is_deterministic() {
        let map = TileMap::new(41, 41, Terrain::Grass);
        let from = Position::new(12, 17);
        let a = find_nearest_tile(&from, &map, 20, |t| t.is_passable());
        let b = find_nearest_tile(&from, &map, 20, |t| t.is_passable());
        assert_eq!(a, b);
        assert!(a.is_some());
    }

    /// Existence semantics must survive the refactor: when only one
    /// passable tile sits within the radius, both `find_nearest_tile` and
    /// `has_nearby_tile` must report it. Guards against accidentally
    /// dropping candidates through the new tiebreak arms.
    #[test]
    fn find_nearest_tile_returns_unique_candidate() {
        let mut map = TileMap::new(10, 10, Terrain::Water);
        map.set(5, 2, Terrain::Grass);
        let from = Position::new(4, 2);
        let found = find_nearest_tile(&from, &map, 5, |t| t.is_passable());
        assert_eq!(found, Some(Position::new(5, 2)));
        assert!(has_nearby_tile(&from, &map, 5, |t| t.is_passable()));

        let far = Position::new(0, 9);
        assert_eq!(find_nearest_tile(&far, &map, 2, |t| t.is_passable()), None);
        assert!(!has_nearby_tile(&far, &map, 2, |t| t.is_passable()));
    }

    /// The tiebreak must not compromise the minimum-distance invariant:
    /// the returned tile's manhattan distance equals the true minimum
    /// over all predicate-satisfying tiles in the radius box.
    #[test]
    fn find_nearest_tile_preserves_minimum_distance() {
        let mut map = TileMap::new(21, 21, Terrain::Water);
        // A ring of passable tiles at chebyshev distance 3 from (10, 10),
        // plus one isolated passable tile at distance 5. The picker must
        // return some distance-3 tile, never the distance-5 one. Cardinal
        // tiles only — diagonals like (11, 8) are chebyshev=2 under the
        // 8-direction movement metric and would tie-break ahead of the ring.
        let ring: Vec<(i32, i32)> = vec![(10, 7), (10, 13), (7, 10), (13, 10)];
        for (x, y) in &ring {
            map.set(*x, *y, Terrain::Grass);
        }
        map.set(15, 10, Terrain::Grass); // distance 5 decoy
        let from = Position::new(10, 10);
        let result =
            find_nearest_tile(&from, &map, 10, |t| t.is_passable()).expect("ring is populated");
        assert_eq!(from.chebyshev_distance(&result), 3);
        assert!(ring.contains(&(result.x(), result.y())));
    }

    /// The mixing hash must avalanche well enough that small input
    /// perturbations produce very different outputs — otherwise the
    /// distribution test above is flaky by accident. Sanity check.
    #[test]
    fn mix_hash_varies_with_inputs() {
        let h1 = mix_hash(10, 10, 10, 9);
        let h2 = mix_hash(10, 10, 10, 11);
        let h3 = mix_hash(10, 10, 9, 10);
        let h4 = mix_hash(10, 10, 11, 10);
        assert_ne!(h1, h2);
        assert_ne!(h1, h3);
        assert_ne!(h1, h4);
        assert_ne!(h2, h3);
    }

    // -----------------------------------------------------------------------
    // Ticket 121 — substrate-aligned `PlannerZone::Wilds` resolution.
    // -----------------------------------------------------------------------

    fn resolve_wilds(
        cat: Position,
        map: &TileMap,
        exploration: &ExplorationMap,
    ) -> Option<Position> {
        let d = DispositionConstants::default();
        let entity = Entity::from_raw_u32(1).unwrap();
        resolve_zone_position(
            PlannerZone::Wilds,
            &cat,
            map,
            exploration,
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            &[],
            // 367: drying / smoking rack positions empty for the
            // Wilds test path — this resolver fixture only exercises
            // the Wilds zone arm.
            &[],
            &[],
            // 457: workshop_positions empty (same rationale).
            &[],
            // 369: tanning_frame_positions empty (same rationale).
            &[],
            &[],
            entity,
            &d,
        )
    }

    /// Substrate alignment: when `ExplorationMap` has authored a
    /// frontier-centroid, `PlannerZone::Wilds` must return that centroid —
    /// the same point `LandmarkAnchor::UnexploredFrontierCentroid` resolves
    /// to in `score_dse_by_id`. By construction, the IAUS Explore DSE and
    /// the GOAP planner agree on "where the wilds are."
    #[test]
    fn wilds_targets_frontier_centroid_when_present() {
        let map = TileMap::new(41, 41, Terrain::Grass);
        let mut exploration = ExplorationMap::new(41, 41);
        // Mark the left half (x in 0..20) as explored. The right half stays
        // at 0.0 (below FRONTIER_THRESHOLD = 0.5), so the centroid lands
        // somewhere in (20..41, 0..41).
        for y in 0..41 {
            for x in 0..20 {
                exploration.explore_tile(x, y);
            }
        }
        exploration
            .recompute_frontier_centroid(crate::resources::exploration_map::FRONTIER_THRESHOLD);
        let centroid = exploration
            .frontier_centroid()
            .expect("right half is unexplored");
        assert!(centroid.x() >= 20, "centroid sits in the unexplored half");

        let cat = Position::new(5, 5);
        let resolved = resolve_wilds(cat, &map, &exploration);
        assert_eq!(
            resolved,
            Some(centroid),
            "Wilds must resolve to the same anchor IAUS Explore scores against"
        );
    }

    /// When the frontier is empty (fully-explored world) the resolver falls
    /// through to the `find_nearest_tile` scan. The result must still be a
    /// real adjacent passable tile — never the cat's own position. This
    /// closes the degenerate self-target the pre-121 `.or(Some(*pos))`
    /// fallback authored.
    #[test]
    fn wilds_falls_back_to_passable_distant_tile_when_frontier_empty() {
        let map = TileMap::new(21, 21, Terrain::Grass);
        let mut exploration = ExplorationMap::new(21, 21);
        for y in 0..21 {
            for x in 0..21 {
                exploration.explore_tile(x, y);
            }
        }
        exploration
            .recompute_frontier_centroid(crate::resources::exploration_map::FRONTIER_THRESHOLD);
        assert!(exploration.frontier_centroid().is_none());

        let cat = Position::new(10, 10);
        let resolved = resolve_wilds(cat, &map, &exploration).expect("open map has neighbors");
        assert_ne!(
            resolved, cat,
            "fallback must never return the cat's own tile (degenerate path)"
        );
        assert!(cat.chebyshev_distance(&resolved) >= 1);
    }

    /// When neither the frontier nor any nearby passable tile resolves,
    /// `PlannerZone::Wilds` returns `None`. The planner then surfaces this
    /// as `no_plan_found` (an observable signal post-091), instead of the
    /// pre-121 silent self-target that masked the failure as a successful
    /// Travel.
    #[test]
    fn wilds_returns_none_when_frontier_empty_and_no_passable_neighbor() {
        let mut map = TileMap::new(21, 21, Terrain::Water);
        // The cat stands on the only passable tile. `find_nearest_tile`
        // skips dist == 0, so no candidate exists.
        map.set(10, 10, Terrain::Grass);
        let mut exploration = ExplorationMap::new(21, 21);
        for y in 0..21 {
            for x in 0..21 {
                exploration.explore_tile(x, y);
            }
        }
        exploration
            .recompute_frontier_centroid(crate::resources::exploration_map::FRONTIER_THRESHOLD);
        assert!(exploration.frontier_centroid().is_none());

        let cat = Position::new(10, 10);
        assert_eq!(
            resolve_wilds(cat, &map, &exploration),
            None,
            "no frontier + no reachable passable neighbor → fail visibly"
        );
    }

    /// Ticket 495 — `CarcassPile` picker must skip food piles sitting
    /// on impassable terrain (Fish carcasses spawn on `Terrain::Water`
    /// per `Fish::habitat`). Without this filter, A* refuses the
    /// destination and the resolver bleeds "no path and stuck" cycles.
    #[test]
    fn carcasspile_picker_skips_impassable_tile() {
        use crate::components::items::ItemKind;
        let mut map = TileMap::new(20, 20, Terrain::Grass);
        // Make tile (5, 5) water — this is the "Fish-carcass-on-water"
        // shape. The further passable pile at (12, 5) must win.
        map.set(5, 5, Terrain::Water);

        let near_water = Entity::from_raw_u32(1).unwrap();
        let far_grass = Entity::from_raw_u32(2).unwrap();
        let food_piles = [
            (near_water, Position::new(5, 5), ItemKind::RawFish),
            (far_grass, Position::new(12, 5), ItemKind::RawMouse),
        ];

        // Cat at (3, 5). Near (5, 5) is Chebyshev 2; far (12, 5) is
        // Chebyshev 9. Pre-495 picker returns (5, 5) and A* fails.
        // Post-495 picker filters water and returns (12, 5).
        let cat_pos = Position::new(3, 5);
        let picked = food_piles
            .iter()
            .filter(|(_, fp, _)| {
                map.in_bounds(fp.x(), fp.y()) && map.get(fp.x(), fp.y()).terrain.is_passable()
            })
            .min_by_key(|(_, fp, _)| cat_pos.tile_distance_squared(fp))
            .map(|(_, p, _)| *p);
        assert_eq!(picked, Some(Position::new(12, 5)));
    }

    /// Ticket 495 — when *every* food pile sits on impassable terrain,
    /// the picker must return None so the resolver surfaces
    /// "no reachable zone target" cleanly (the planner then drops the
    /// PickingUp disposition and replans) rather than stamping an
    /// unreachable target.
    #[test]
    fn carcasspile_picker_returns_none_when_all_impassable() {
        use crate::components::items::ItemKind;
        let mut map = TileMap::new(20, 20, Terrain::Grass);
        map.set(5, 5, Terrain::Water);
        map.set(10, 10, Terrain::Water);

        let entity_a = Entity::from_raw_u32(1).unwrap();
        let entity_b = Entity::from_raw_u32(2).unwrap();
        let food_piles = [
            (entity_a, Position::new(5, 5), ItemKind::RawFish),
            (entity_b, Position::new(10, 10), ItemKind::RawFish),
        ];

        let cat_pos = Position::new(3, 5);
        let picked = food_piles
            .iter()
            .filter(|(_, fp, _)| {
                map.in_bounds(fp.x(), fp.y()) && map.get(fp.x(), fp.y()).terrain.is_passable()
            })
            .min_by_key(|(_, fp, _)| cat_pos.tile_distance_squared(fp))
            .map(|(_, p, _)| *p);
        assert_eq!(picked, None);
    }

    // ----- 364: htn_advance_or_pop / htn_abandon_or_pop -------------------

    use crate::ai::methods::MethodId;
    use crate::components::{GoalFrame, HeldGoalStack};

    fn make_frame(id: &'static str, sub_goal_count: usize, sub_goal_index: usize) -> GoalFrame {
        let mut frame = GoalFrame::new(
            MethodId(id),
            "test_goal",
            sub_goal_count,
            0,
            None,
            crate::components::IntentionSource::SelfMotivated,
        );
        frame.sub_goal_index = sub_goal_index;
        frame
    }

    #[test]
    fn advance_increments_cursor_when_subgoal_remains() {
        let mut stack = HeldGoalStack::empty();
        stack.push(make_frame("rear_kitten", 3, 0)).unwrap();
        let outcome = htn_advance_or_pop(stack);
        match outcome {
            StackOutcome::AdvanceTo(s) => {
                let top = s.top().expect("stack non-empty after advance");
                assert_eq!(top.sub_goal_index, 1);
                assert_eq!(top.method.0, "rear_kitten");
            }
            other => panic!(
                "expected AdvanceTo, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }

    #[test]
    fn advance_pops_when_cursor_at_end() {
        // sub_goal_index=2 of count=3 — increment to 3, equal to count → pop.
        let mut stack = HeldGoalStack::empty();
        stack.push(make_frame("rear_kitten", 3, 2)).unwrap();
        let outcome = htn_advance_or_pop(stack);
        assert!(matches!(outcome, StackOutcome::Done));
    }

    #[test]
    fn advance_propagates_through_parent_frame() {
        // Parent at index 0 of 2, child at index 1 of 2. Child advance pops
        // (cursor would hit 2 == count), then parent's index advances from
        // 0 to 1 — sub_goals remain on parent → AdvanceTo with parent on top.
        let mut stack = HeldGoalStack::empty();
        stack.push(make_frame("parent_method", 2, 0)).unwrap();
        stack.push(make_frame("child_method", 2, 1)).unwrap();
        let outcome = htn_advance_or_pop(stack);
        match outcome {
            StackOutcome::AdvanceTo(s) => {
                assert_eq!(s.depth(), 1, "child frame popped");
                let top = s.top().expect("parent frame remains");
                assert_eq!(top.method.0, "parent_method");
                assert_eq!(top.sub_goal_index, 1, "parent's index advanced");
            }
            other => panic!(
                "expected AdvanceTo, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }

    #[test]
    fn advance_done_when_both_frames_exhausted() {
        // Parent at index 1 of 2, child at index 1 of 2. Child pops, parent
        // index advances to 2 == count, parent pops. Stack empty → Done.
        let mut stack = HeldGoalStack::empty();
        stack.push(make_frame("parent_method", 2, 1)).unwrap();
        stack.push(make_frame("child_method", 2, 1)).unwrap();
        let outcome = htn_advance_or_pop(stack);
        assert!(matches!(outcome, StackOutcome::Done));
    }

    #[test]
    fn abandon_clears_stack() {
        // Today (364 scope) Backtrack ≡ Abandon — pops all frames.
        let mut stack = HeldGoalStack::empty();
        stack.push(make_frame("parent_method", 2, 0)).unwrap();
        stack.push(make_frame("child_method", 3, 1)).unwrap();
        let outcome = htn_abandon_or_pop(stack);
        assert!(matches!(outcome, StackOutcome::Done));
    }

    #[test]
    fn self_craft_two_step_frame_walks_craft_then_wear_then_pops() {
        // 334: `acquire_stealth_via_self_craft` is `[Craft, WearItem]`. After
        // the Craft leg (sub_goal 0) completes, the frame must advance to the
        // WearItem leg (sub_goal 1), not pop. After WearItem completes the
        // frame pops. (The gate that decides *whether* to call this — the
        // `pinned == plan.chosen_action` check — lives inline in
        // `resolve_goap_plans` and is exercised by the soak; this test
        // pins the structural walk the gate drives.)
        let mut stack = HeldGoalStack::empty();
        stack
            .push(make_frame("acquire_stealth_via_self_craft", 2, 0))
            .unwrap();
        let after_craft = htn_advance_or_pop(stack);
        let stack = match after_craft {
            StackOutcome::AdvanceTo(s) => {
                let top = s.top().expect("frame remains after Craft leg");
                assert_eq!(top.sub_goal_index, 1, "advanced to the WearItem leg");
                s
            }
            other => panic!(
                "expected AdvanceTo after Craft leg, got {:?}",
                std::mem::discriminant(&other)
            ),
        };
        let after_wear = htn_advance_or_pop(stack);
        assert!(
            matches!(after_wear, StackOutcome::Done),
            "frame pops after the WearItem leg completes",
        );
    }
}
