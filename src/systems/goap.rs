use std::collections::HashMap;

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::pathfinding::{find_free_adjacent, find_path, step_toward};
use crate::ai::planner::actions::actions_for_disposition;
use crate::ai::planner::goals::goal_for_disposition;
use crate::ai::planner::{
    make_plan, Carrying, GoapActionKind, PlannerState, PlannerZone, ZoneDistances,
};
use crate::ai::scoring::{
    apply_aspiration_bonuses, apply_cascading_bonuses, apply_colony_knowledge_bonuses,
    apply_directive_bonus, apply_fated_bonuses, apply_memory_bonuses, apply_preference_bonuses,
    apply_priority_bonus, score_actions, ScoringContext,
};
use crate::ai::{Action, CurrentAction};
use crate::components::building::{
    ConstructionSite, CropState, StoredItems, Structure, StructureType,
};
use crate::components::coordination::{ActiveDirective, Directive, DirectiveKind, DirectiveQueue};
use crate::components::disposition::{
    ActionHistory, ActionOutcome, ActionRecord, CraftingHint, DispositionKind,
};
use crate::components::goap_plan::{
    GoapPlan, PendingUrgencies, PlanEvent, PlanNarrative, StepExecutionState, UrgencyKind,
    UrgentNeed,
};
use crate::components::hunting_priors::HuntingPriors;
use crate::components::identity::{Gender, LifeStage, Name};
use crate::components::items::{Item, ItemKind, ItemLocation};
use crate::components::magic::{Harvestable, Herb, HerbKind, Inventory, Ward};
use crate::components::markers;
use crate::components::mental::Memory;
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Health, InjuryKind, Needs, Position};
use crate::components::prey::{
    DenRaided, PreyAnimal, PreyConfig, PreyDen, PreyDensity, PreyKilled, PreyState,
};
use crate::components::skills::{Corruption, MagicAffinity, Skills};
use crate::components::wildlife::WildAnimal;
use crate::resources::colony_hunting_map::ColonyHuntingMap;
use crate::resources::event_log::{EventKind, EventLog};
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
    /// Phase 2B — scent-detection grid. Cats sample
    /// `highest_nearby(pos, scent_search_radius)` to find prey-scent
    /// source tiles rather than running point-to-point
    /// `cat_smells_prey_windaware` against each prey entity.
    pub prey_scent_map: Res<'w, crate::resources::PreyScentMap>,
}

#[derive(bevy_ecs::system::SystemParam)]
pub struct NarrativeEmitter<'w> {
    pub log: ResMut<'w, crate::resources::narrative::NarrativeLog>,
    pub registry: Option<Res<'w, crate::resources::narrative_templates::TemplateRegistry>>,
    pub config: Res<'w, crate::resources::time::SimConfig>,
    pub weather: Res<'w, crate::resources::weather::WeatherState>,
    pub activation: Option<ResMut<'w, SystemActivation>>,
}

/// Bundles world-state queries for evaluate_and_plan to stay under 16 params.
#[allow(clippy::type_complexity)]
#[derive(bevy_ecs::system::SystemParam)]
pub struct WorldStateQueries<'w, 's> {
    pub all_positions:
        Query<'w, 's, (Entity, &'static Position, Option<&'static PreyAnimal>), Without<Dead>>,
    pub wildlife: Query<'w, 's, (Entity, &'static Position), With<WildAnimal>>,
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
    pub carcass_query: Query<
        'w,
        's,
        (
            &'static crate::components::wildlife::Carcass,
            &'static Position,
        ),
    >,
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
}

/// Bundles resources for evaluate_and_plan.
#[derive(bevy_ecs::system::SystemParam)]
pub struct PlanResources<'w> {
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
        Without<crate::components::task_chain::TaskChain>,
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
}

#[allow(clippy::type_complexity)]
#[derive(bevy_ecs::system::SystemParam)]
pub struct ExecutorContext<'w, 's> {
    pub map: ResMut<'w, TileMap>,
    pub wind: Res<'w, crate::resources::wind::WindState>,
    pub time: Res<'w, TimeState>,
    pub time_scale: Res<'w, crate::resources::time::TimeScale>,
    pub constants: Res<'w, SimConstants>,
    pub event_log: Option<ResMut<'w, EventLog>>,
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
    /// §Phase 4c.4 kitten-feeding side effect: the main `cats` query
    /// in `resolve_goap_plans` requires `&mut GoapPlan`, which
    /// kittens don't have (see `src/systems/pregnancy.rs` — kittens
    /// ship without a `GoapPlan` bundle). The deferred kitten-feeding
    /// post-loop therefore must reach `&mut Needs` on kittens via a
    /// disjoint query. `Without<GoapPlan>` proves disjointness from
    /// the cats query for the borrow checker; `Without<Dead>`/
    /// `Without<Structure>` mirror the cats query's base filters so
    /// we don't accidentally grant +0.5 hunger to a dead cat or a
    /// Structure. Previously, `cats.get_mut(kitten_entity)` silently
    /// returned `Err(NoSuchEntity)` for every kitten — the
    /// `KittenFed` activation would fire but the real-world hunger
    /// credit was dropped, so every kitten that was "fed" still
    /// starved (Pebblekit-34, Hazelkit-10, Reedkit-33 in the v3
    /// soak; fourth silent-advance-class bug on this Caretake
    /// pipeline in a week).
    pub kitten_needs: bevy_ecs::prelude::Query<
        'w,
        's,
        &'static mut crate::components::physical::Needs,
        (Without<GoapPlan>, Without<Dead>, Without<Structure>),
    >,
    /// §6.5.4 groom-other kinship lookup — read-only snapshot of
    /// `(kitten_entity) → (mother, father)` pointers. Disjoint from
    /// the mutable `cats` query by `With<KittenDependency>` (kittens
    /// don't carry a `GoapPlan` so the cats query excludes them).
    pub kitten_parentage: bevy_ecs::prelude::Query<
        'w,
        's,
        (Entity, &'static crate::components::KittenDependency),
        Without<Dead>,
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
    /// Ticket 027b §7.M — L2 PairingActivity Intention lookup. The
    /// `SocializeWith` step resolver reads this to pin the Intention
    /// partner at the top of `target_partner_bond` axis. Disjoint
    /// from the mutable `cats` query in `resolve_goap_plans` because
    /// `&PairingActivity` is read-only.
    pub pairing_q: bevy_ecs::prelude::Query<
        'w,
        's,
        &'static crate::components::PairingActivity,
        Without<Dead>,
    >,
    /// Ticket 074 — read-only target-validity surface (Dead /
    /// Banished / Incapacitated / despawned). Bundled here so step
    /// resolvers reach `validate_target` through the same context they
    /// already hold; nothing else changes about the ExecutorContext
    /// borrow shape (the query is read-only, disjoint from the
    /// mutable `cats` query because cats are filtered `Without<Dead>`
    /// and we read `Has<Dead>` rather than `&Dead`).
    pub target_validity: crate::systems::plan_substrate::target::TargetValidityQuery<'w, 's>,
}

impl<'w, 's> ExecutorContext<'w, 's> {
    /// Read §9.2 overlay markers off `e` from the ECS, returning a
    /// [`StanceOverlays`](crate::ai::faction::StanceOverlays) the §9.3
    /// prefilter can consume. Defaults to an all-`false` overlay when
    /// the entity is not in the query (despawned, dead, etc.).
    pub fn stance_overlays_of(&self, e: Entity) -> crate::ai::faction::StanceOverlays {
        match self.faction_overlay_q.get(e) {
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
// check_anxiety_interrupts — hard interrupt for CriticalHealth only;
// all other critical needs accumulate as pending urgencies evaluated at
// step boundaries in resolve_goap_plans.
// ===========================================================================

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn check_anxiety_interrupts(
    mut query: Query<
        (
            Entity,
            &Name,
            &Needs,
            &Personality,
            &Position,
            &Health,
            &mut CurrentAction,
            &mut PendingUrgencies,
            Option<&mut ActionHistory>,
        ),
        (With<GoapPlan>, Without<Dead>),
    >,
    plans: Query<&GoapPlan, Without<Dead>>,
    wildlife: Query<(Entity, &Position), (With<WildAnimal>, Without<Dead>, Without<PreyAnimal>)>,
    ward_query: Query<(&Ward, &Position)>,
    all_cats: Query<(Entity, &Position), (Without<Dead>, Without<WildAnimal>)>,
    building_query: Query<&Position, (With<Structure>, Without<ConstructionSite>)>,
    time: Res<TimeState>,
    constants: Res<SimConstants>,
    colony_center: Res<crate::resources::ColonyCenter>,
    mut commands: Commands,
    mut activation: ResMut<SystemActivation>,
    mut plan_writer: MessageWriter<PlanNarrative>,
    mut event_log: Option<ResMut<EventLog>>,
    focal_target: Option<Res<crate::resources::FocalTraceTarget>>,
    focal_capture: Option<Res<crate::resources::FocalScoreCapture>>,
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

    for (entity, name, needs, personality, pos, health, mut current, mut urgencies, history) in
        &mut query
    {
        let Ok(plan) = plans.get(entity) else {
            continue;
        };

        // --- Hard interrupt: CriticalHealth only ---
        // A critically injured cat that chose Resting is already recovering;
        // interrupting it creates the same oscillation we're fixing.
        if plan.kind != DispositionKind::Resting
            && health.current / health.max < d.critical_health_threshold
        {
            activation.record(Feature::AnxietyInterrupt);

            // §11 focal-cat trace capture for the §7.5 Maslow
            // preemption path. Distinct from the §7.2 commitment
            // branches — this preempts the gate entirely per spec,
            // so the record surfaces as `L3PlanFailure` with
            // `reason: "anxiety_interrupt"` rather than an
            // `L3Commitment` row.
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
                            reason: "anxiety_interrupt",
                            disposition: format!("{:?}", plan.kind),
                            detail: serde_json::json!({
                                "health_ratio": health.current / health.max,
                                "critical_threshold": d.critical_health_threshold,
                                "preempted_step": current_step,
                            }),
                        },
                        time.tick,
                    );
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
                        reason: "CriticalHealth".into(),
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

            commands.entity(entity).remove::<GoapPlan>();
            current.ticks_remaining = 0;
            continue;
        }

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
    if !matches!(
        kind,
        DispositionKind::Resting | DispositionKind::Hunting | DispositionKind::Foraging
    ) && needs.hunger < d.starvation_interrupt_threshold
    {
        urgencies.needs.push(UrgentNeed {
            kind: UrgencyKind::Starvation,
            maslow_level: 1,
            intensity: 1.0 - (needs.hunger / d.starvation_interrupt_threshold).max(0.001),
            threat_pos: None,
        });
    }
    // Critical starvation override for Hunting/Foraging.
    if matches!(kind, DispositionKind::Hunting | DispositionKind::Foraging)
        && needs.hunger < d.critical_hunger_interrupt_threshold
    {
        urgencies.needs.push(UrgentNeed {
            kind: UrgencyKind::Starvation,
            maslow_level: 1,
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
            maslow_level: 1,
            intensity: 1.0 - (needs.energy / d.exhaustion_interrupt_threshold).max(0.001),
            threat_pos: None,
        });
    }

    // --- CriticalSafety (maslow 2) ---
    if needs.safety < d.critical_safety_threshold {
        urgencies.needs.push(UrgentNeed {
            kind: UrgencyKind::CriticalSafety,
            maslow_level: 2,
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
        .min_by_key(|(wp, _)| pos.manhattan_distance(wp));

    let (threat_pos, _) = nearest?;
    let dist = pos.manhattan_distance(threat_pos) as f32;

    // Base urgency: inverse distance.
    let base_urgency = (1.0 - dist / d.threat_urgency_divisor).max(0.0);
    if base_urgency <= 0.0 {
        return None;
    }

    // Ward protection: inside a ward's repel radius dampens threat.
    let within_ward = ward_data
        .iter()
        .any(|(wp, radius)| (pos.manhattan_distance(wp) as f32) < *radius);
    let ward_factor = if within_ward {
        d.threat_ward_dampening
    } else {
        1.0
    };

    // Colony proximity: near buildings or colony center dampens threat.
    let near_buildings = building_positions
        .iter()
        .any(|bp| pos.manhattan_distance(bp) <= d.threat_building_safety_range);
    let colony_factor = if near_buildings {
        d.threat_colony_building_dampening
    } else {
        let colony_dist = pos.manhattan_distance(colony_center) as f32;
        let normalized = (colony_dist / d.threat_colony_radius).min(1.0);
        d.threat_colony_center_dampening + (1.0 - d.threat_colony_center_dampening) * normalized
    };

    // Allies: each nearby cat reduces perceived threat (diminishing returns).
    let ally_count = cat_positions
        .iter()
        .filter(|(e, cp)| *e != entity && pos.manhattan_distance(cp) <= d.threat_ally_range)
        .count()
        .min(d.allies_fighting_cap);
    let ally_factor = 1.0 / (1.0 + ally_count as f32 * d.threat_ally_dampening_per_cat);

    // Boldness: bold cats feel less threatened.
    let boldness_factor = 1.0 - personality.boldness * d.flee_threshold_boldness_scale;

    let intensity = base_urgency * ward_factor * colony_factor * ally_factor * boldness_factor;

    if intensity > d.flee_threshold_base {
        Some(UrgentNeed {
            kind: UrgencyKind::ThreatNearby,
            maslow_level: 2,
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
                // Ticket 123 — IAUS-side mirror of the planner's
                // `make_plan → None` veto. Lazy-inserted on first
                // failure; `None` here means the cat has never failed
                // a disposition and the consideration scores 1.0
                // (no penalty).
                Option<&mut crate::components::RecentDispositionFailures>,
            ),
        ),
        (
            Without<Dead>,
            Without<GoapPlan>,
            // §Phase 5b — kittens are dependents, not autonomous planners.
            // Before this filter, `evaluate_and_plan` inserted GoapPlan
            // on kittens too; the `kitten_needs` post-loop query
            // (`Without<GoapPlan>`) then silently excluded them, so
            // adults' `+0.5` hunger restoration from FeedKitten never
            // landed. Maplekit-83 starved on seed 42 despite 12
            // successful feedings — the activation fired every time
            // but the query returned NoSuchEntity on every restoration
            // call.
            Without<crate::components::KittenDependency>,
        ),
    >,
    world_state: WorldStateQueries,
    res: PlanResources,
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
        Has<markers::CanCook>,
    )>,
    // §4.2 State markers — split into a separate query so the per-cat
    // tuple stays small and future State authors can extend here.
    state_markers_q: Query<(
        Has<markers::InCombat>,
        Has<markers::OnCorruptedTile>,
        Has<markers::OnSpecialTerrain>,
    )>,
    // Ticket 027 Bug 2 — HasEligibleMate authored by
    // `mating::update_mate_eligibility_markers`. Solo query so future
    // related markers (HasEligiblePartnerCandidate per §7.M Bug 3) can
    // sit alongside without disturbing the State tuple.
    mate_eligibility_q: Query<Has<markers::HasEligibleMate>>,
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
    let food_available = !res.food.is_empty();
    let food_fraction = res.food.fraction();

    // §4 marker snapshot. Populated once at system start from Resources
    // and Queries, then passed by reference through `EvalInputs` so
    // `EligibilityFilter::require(marker)` rows resolve without each
    // DSE carrying its own query bundle. Colony-scoped markers follow
    // the same "compute from caller-visible state" pattern established
    // in Phase 4b.2 — `ColonyState` singleton promotion is a later
    // refactor. `HasGarden` is populated below after the existing
    // `has_garden` binding computes the same predicate.
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

    let wildlife_positions: Vec<(Entity, Position)> =
        world_state.wildlife.iter().map(|(e, p)| (e, *p)).collect();

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

    // §4 colony-scoped marker predicates — shared helpers eliminate
    // duplication with disposition.rs (previously computed identically in both).
    let bldg_state = crate::systems::buildings::scan_colony_buildings(
        world_state
            .building_query
            .iter()
            .map(|(_, s, _, site, _)| (s, site)),
        d.damaged_building_threshold,
    );
    let has_construction_site = bldg_state.has_construction_site;
    let has_damaged_building = bldg_state.has_damaged_building;
    let has_garden = bldg_state.has_garden;
    let has_functional_kitchen = bldg_state.has_functional_kitchen;
    markers.set_colony(markers::HasGarden::KEY, has_garden);
    markers.set_colony(markers::HasFunctionalKitchen::KEY, has_functional_kitchen);
    let has_raw_food_in_stores = world_state.stored_items_query.iter().any(|stored| {
        stored.items.iter().copied().any(|e| {
            world_state
                .items_query
                .get(e)
                .is_ok_and(|it| it.kind.is_food() && !it.modifiers.cooked)
        })
    });
    markers.set_colony(markers::HasRawFoodInStores::KEY, has_raw_food_in_stores);

    let herb_positions: Vec<(Entity, Position, HerbKind)> = world_state
        .herb_query
        .iter()
        .map(|(e, herb, p)| (e, *p, herb.kind))
        .collect();

    // Ticket 014 Magic colony batch: shared helper + colony-scoped
    // marker. Retires the per-cat inline scan at the ScoringContext
    // assignment below by computing once at the colony scope.
    let thornbriar_available = crate::systems::magic::is_thornbriar_available(
        world_state.herb_query.iter().map(|(_, h, _)| h),
    );
    markers.set_colony(markers::ThornbriarAvailable::KEY, thornbriar_available);

    let ward_strength_low = crate::systems::magic::is_ward_strength_low(
        world_state.ward_query.iter().map(|(w, _)| w),
        d.ward_strength_low_threshold,
    );
    markers.set_colony(markers::WardStrengthLow::KEY, ward_strength_low);

    // Snapshot actionable carcasses for scoring.
    let carcass_positions: Vec<Position> = world_state
        .carcass_query
        .iter()
        .filter(|(c, _)| !c.cleansed || !c.harvested)
        .map(|(_, p)| *p)
        .collect();

    // Territory corruption — max corruption in the ring around colony center.
    let territory_max_corruption = {
        let mc = &res.constants.magic;
        let inner = mc.territory_corruption_inner_radius;
        let outer = mc.territory_corruption_outer_radius;
        let cx = res.colony_center.0.x;
        let cy = res.colony_center.0.y;
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

    // Ticket 014 Magic colony batch: shared helper + colony-scoped
    // marker. Retires the inline `wildlife_ai_query.iter().any(...)`
    // scan.
    let wards_under_siege =
        crate::systems::magic::is_any_ward_under_siege(world_state.wildlife_ai_query.iter());
    markers.set_colony(markers::WardsUnderSiege::KEY, wards_under_siege);

    let colony_injury_count = query
        .iter()
        .filter(|((_, _, _, _, _, _, _, health), _)| health.current < 1.0)
        .count();

    let directive_snapshot: HashMap<Entity, (usize, Option<Directive>)> = world_state
        .directive_queue_query
        .iter()
        .map(|(entity, q)| (entity, (q.directives.len(), q.directives.first().cloned())))
        .collect();

    let action_snapshot: Vec<(Entity, Position, Action)> = query
        .iter()
        .map(
            |((entity, _, _, _, pos, _, _, _), (_, _, current, _, _, _, _, _, _))| {
                (entity, *pos, current.action)
            },
        )
        .collect();

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

    // Snapshot per-cat fields needed by the mating eligibility gate.
    let current_day_phase = mating_fitness_params.current_day_phase();

    for (
        (entity, name, needs, personality, pos, memory, skills, health),
        (
            magic_aff,
            inventory,
            mut current,
            aspirations,
            preferences,
            fated_love,
            fated_rival,
            fulfillment,
            mut recent_disposition_failures,
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
        // count co-fighting cats; the same flat-range scan that the
        // author uses internally.
        let nearest_threat = wildlife_positions
            .iter()
            .filter(|(_, wp)| pos.manhattan_distance(wp) <= d.wildlife_threat_range)
            .min_by_key(|(_, wp)| pos.manhattan_distance(wp));

        let allies_fighting_threat = if let Some(&(_, threat_pos)) = nearest_threat {
            action_snapshot
                .iter()
                .filter(|(e, ally_pos, action)| {
                    *e != entity
                        && *action == Action::Fight
                        && ally_pos.manhattan_distance(&threat_pos) <= d.allies_fighting_range
                })
                .count()
                .min(d.allies_fighting_cap)
        } else {
            0
        };

        let combat_effective =
            skills.combat + skills.hunting * d.combat_effective_hunting_cross_train;
        let is_incapacitated = health
            .injuries
            .iter()
            .any(|inj| inj.kind == InjuryKind::Severe && !inj.healed);
        // §4.3 per-cat marker population. Bit-for-bit mirrors the
        // inline `is_incapacitated` above — kept side-by-side so
        // `MarkerSnapshot::has("Incapacitated", entity)` resolves
        // identically to `ScoringContext.is_incapacitated` for any
        // DSE that later wires `.forbid("Incapacitated")` (§13.1).
        markers.set_entity(markers::Incapacitated::KEY, entity, is_incapacitated);
        if let Ok((k, y, a, e)) = life_stage_q.get(entity) {
            markers.set_entity(markers::Kitten::KEY, entity, k);
            markers.set_entity(markers::Young::KEY, entity, y);
            markers.set_entity(markers::Adult::KEY, entity, a);
            markers.set_entity(markers::Elder::KEY, entity, e);
        }
        // §4 batch 1 + batch 2: per-cat markers read from authored ZSTs.
        if let Ok((
            injured,
            has_herbs,
            has_remedy,
            has_ward,
            is_coord_dir,
            can_hunt,
            can_forage,
            can_ward,
            can_cook,
        )) = per_cat_markers_q.get(entity)
        {
            markers.set_entity(markers::Injured::KEY, entity, injured);
            markers.set_entity(markers::HasHerbsInInventory::KEY, entity, has_herbs);
            markers.set_entity(markers::HasRemedyHerbs::KEY, entity, has_remedy);
            markers.set_entity(markers::HasWardHerbs::KEY, entity, has_ward);
            markers.set_entity(
                markers::IsCoordinatorWithDirectives::KEY,
                entity,
                is_coord_dir,
            );
            // §4 batch 2: capability markers.
            markers.set_entity(markers::CanHunt::KEY, entity, can_hunt);
            markers.set_entity(markers::CanForage::KEY, entity, can_forage);
            markers.set_entity(markers::CanWard::KEY, entity, can_ward);
            markers.set_entity(markers::CanCook::KEY, entity, can_cook);
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
        // Ticket 027 Bug 2 — HasEligibleMate authored by
        // `mating::update_mate_eligibility_markers`.
        if let Ok(has_mate) = mate_eligibility_q.get(entity) {
            markers.set_entity(markers::HasEligibleMate::KEY, entity, has_mate);
        }
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
        if let Ok((threat, social, herbs, prey, carcass)) =
            marker_qs.target_existence_q.get(entity)
        {
            markers.set_entity(markers::HasThreatNearby::KEY, entity, threat);
            markers.set_entity(markers::HasSocialTarget::KEY, entity, social);
            markers.set_entity(markers::HasHerbsNearby::KEY, entity, herbs);
            markers.set_entity(markers::PreyNearby::KEY, entity, prey);
            markers.set_entity(markers::CarcassNearby::KEY, entity, carcass);
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

        // Ticket 014 §4 sensing batch — `has_herbs_nearby` /
        // `prey_nearby` now read from `MarkerSnapshot`. The inline
        // `observer_sees_at` scans retire here. `nearby_carcass_count`
        // remains an inline count: the count is consumed by ScoringContext
        // separately from the boolean `carcass_nearby` marker (used by
        // magic_harvest siblings as a magnitude axis), so the count
        // computation stays here while the boolean reads from snapshot.
        let has_herbs_nearby = markers.has(markers::HasHerbsNearby::KEY, entity);
        let prey_nearby = markers.has(markers::PreyNearby::KEY, entity);
        let has_threat_nearby = markers.has(markers::HasThreatNearby::KEY, entity);
        let has_social_target = markers.has(markers::HasSocialTarget::KEY, entity);

        let nearby_carcass_count = carcass_positions
            .iter()
            .filter(|cp| {
                crate::systems::sensing::observer_smells_at(
                    crate::components::SensorySpecies::Cat,
                    *pos,
                    &res.constants.sensory.cat,
                    **cp,
                    crate::components::SensorySignature::CARCASS,
                    sc.carcass_detection_range as f32,
                )
            })
            .count();

        let (on_corrupted_tile, tile_corruption, on_special_terrain) =
            if res.map.in_bounds(pos.x, pos.y) {
                let tile = res.map.get(pos.x, pos.y);
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
            let r = sc.corruption_smell_range;
            let mut max_c: f32 = 0.0;
            let mut max_pos: Option<crate::components::physical::Position> = None;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() + dy.abs() > r {
                        continue; // Manhattan radius
                    }
                    let nx = pos.x + dx;
                    let ny = pos.y + dy;
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
                &health.injuries,
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
            is_incapacitated,
            has_construction_site,
            has_damaged_building,
            has_garden,
            food_fraction,
            magic_affinity: magic_aff.0,
            magic_skill: skills.magic,
            herbcraft_skill: skills.herbcraft,
            has_herbs_nearby,
            // §4 batch 1: read from authored markers via MarkerSnapshot.
            has_herbs_in_inventory: markers.has(markers::HasHerbsInInventory::KEY, entity),
            has_remedy_herbs: markers.has(markers::HasRemedyHerbs::KEY, entity),
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
            caretake_compassion_bond_scale: caretake_bond_scale,
            unexplored_nearby: colony.exploration_map.unexplored_fraction_nearby(
                pos.x,
                pos.y,
                d.explore_perception_radius,
                0.5,
            ),
            fox_scent_level: colony.fox_scent_map.get(pos.x, pos.y),
            // Ticket 014 §4 sensing batch — read via marker. The
            // marker's predicate is "any uncleansed-or-unharvested
            // carcass within carcass_detection_range" (matches the
            // `nearby_carcass_count > 0` invariant exactly).
            carcass_nearby: markers.has(markers::CarcassNearby::KEY, entity),
            nearby_carcass_count,
            territory_max_corruption,
            // Ticket 014 Magic colony batch — read via marker.
            wards_under_siege: markers.has(markers::WardsUnderSiege::KEY, entity),
            day_phase: current_day_phase,
            has_functional_kitchen,
            has_raw_food_in_stores,
            social_warmth_deficit: fulfillment.map_or(0.4, |f| f.social_warmth_deficit()),
            cat_anchors: crate::ai::scoring::CatAnchorPositions {
                nearest_corrupted_tile,
                nearest_construction_site:
                    crate::systems::buildings::nearest_construction_site(
                        world_state
                            .building_query
                            .iter()
                            .map(|(_, s, p, site, _)| (s, p, site)),
                        *pos,
                    ),
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
                    .min_by_key(|p| pos.manhattan_distance(p)),
                // §L2.10.7 Patrol / HerbcraftWard anchor: a perimeter
                // anchor offset from the colony center. Single-point
                // approximation — the cat patrols toward this anchor
                // along the colony's outer ring. Future refinement:
                // multi-point perimeter sampling.
                nearest_perimeter_tile: Some(crate::components::physical::Position::new(
                    res.colony_center.0.x + d.patrol_perimeter_offset,
                    res.colony_center.0.y,
                )),
                territory_perimeter_anchor: Some(crate::components::physical::Position::new(
                    res.colony_center.0.x + d.patrol_perimeter_offset,
                    res.colony_center.0.y,
                )),
                // §L2.10.7 Flee anchor: position of the nearest
                // wildlife threat already scanned for allies_fighting.
                nearest_threat: nearest_threat.map(|&(_, p)| p),
                // §L2.10.7 Coordinate anchor: colony center as the
                // coordinator's perch (single-perch model).
                coordinator_perch: Some(res.colony_center.0),
                // Ticket 089 — interoceptive self-anchors.
                own_safe_rest_spot: crate::systems::interoception::own_safe_rest_spot(
                    memory,
                    d.safe_rest_threat_suppression_radius,
                ),
                own_injury_site: crate::systems::interoception::own_injury_site(health),
            },
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
        let mut scores = result.scores;

        // Ticket 123 — damp the IAUS scores of dispositions that
        // recently hit `make_plan → None`. Runs before the additive
        // bonus layers so a recently-failed disposition is dimmed
        // down before memory / colony-knowledge / cascading bonuses
        // potentially re-lift it; that ordering matches the
        // `apply_*_bonuses` chain's intent (bonuses fight for
        // attention against a baseline that already reflects
        // cross-tick failure history). Non-applicable cats (no
        // `RecentDispositionFailures` component, or all entries
        // expired) pass through unchanged.
        crate::systems::plan_substrate::apply_disposition_failure_cooldown(
            &mut scores,
            recent_disposition_failures.as_deref(),
            res.time.tick,
            res.constants.planning_substrate.disposition_failure_cooldown_ticks,
        );

        // Apply all bonus layers.
        apply_memory_bonuses(&mut scores, memory, pos, sc);
        if let Some(ref ck) = colony.knowledge {
            apply_colony_knowledge_bonuses(&mut scores, ck, pos, sc);
        }
        if let Some(ref cp) = colony.priority {
            apply_priority_bonus(&mut scores, cp.active, sc);
        }
        let mut nearby_actions = HashMap::new();
        for &(other_entity, other_pos, other_action) in &action_snapshot {
            if other_entity != entity
                && pos.manhattan_distance(&other_pos) <= d.cascading_bonus_range
            {
                *nearby_actions.entry(other_action).or_insert(0usize) += 1;
            }
        }
        apply_cascading_bonuses(&mut scores, &nearby_actions, sc);
        if let Some(asp) = aspirations {
            apply_aspiration_bonuses(&mut scores, asp, sc);
        }
        if let Some(pref) = preferences {
            apply_preference_bonuses(&mut scores, pref, sc);
        }
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
                    d.fated_love_detection_range as f32,
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
                    d.fated_rival_detection_range as f32,
                )
            });
        apply_fated_bonuses(&mut scores, love_visible, rival_nearby, sc);
        if let Ok(directive) = world_state.active_directive_query.get(entity) {
            let fondness_factor = res
                .relationships
                .get(entity, directive.coordinator)
                .map_or(d.fondness_default, |r| (r.fondness + 1.0) / 2.0);
            let bonus = directive.priority
                * directive.coordinator_social_weight
                * d.directive_bonus_base_weight
                * personality.diligence
                * fondness_factor
                * (1.0 - personality.independence * d.directive_independence_penalty)
                * (1.0 - personality.stubbornness * d.directive_stubbornness_penalty);
            apply_directive_bonus(&mut scores, directive.kind.to_action(), bonus);
        }

        // Groom routing.
        let self_groom_score = (1.0 - needs.temperature)
            * sc.self_groom_temperature_scale
            * needs.level_suppression(1);
        let other_groom_score = if has_social_target {
            personality.warmth * (1.0 - needs.social) * needs.level_suppression(2)
        } else {
            0.0
        };
        let self_groom_won = self_groom_score >= other_groom_score;

        // §L2.10.6 softmax-over-Intentions: softmax the flat action pool
        // directly, then map the winning Intention to its disposition. The
        // helper preserves the legacy disposition-level independence penalty
        // by applying it as an action-level transform on Coordinate /
        // Socialize / Mentor (and Groom when socializing) before softmax.
        //
        // §11.3 L3 capture — when the focal cat is selecting, surface
        // the pool + probabilities + RNG roll to `FocalScoreCapture` so
        // `emit_focal_trace` can reconstruct the full selection record.
        let capture_this_cat = focal_capture.is_some() && focal_cat == Some(entity);
        let mut softmax_trace = capture_this_cat.then(crate::ai::scoring::SoftmaxCapture::default);
        let chosen = crate::ai::scoring::select_disposition_via_intention_softmax_with_trace(
            &scores,
            self_groom_won,
            personality.independence,
            d.disposition_independence_penalty,
            sc,
            &mut rng.rng,
            softmax_trace.as_mut(),
        );
        if let (Some(capture), Some(trace)) = (focal_capture, softmax_trace) {
            capture.set_softmax(trace, res.time.tick);
        }

        // Store all gate-open action scores, sorted descending, for
        // diagnostics. Truncation removed 2026-04-20 so scoring-competition
        // analysis can see ranks beyond the top few (e.g., Mate vs Socialize
        // on shared ticks).
        {
            let mut sorted = scores.clone();
            sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            current.last_scores = sorted;
        }

        let crafting_hint = if chosen == DispositionKind::Crafting {
            // If a corruption-response directive is active, route the plan
            // directly to the matching narrow action.
            let directive_hint = world_state
                .active_directive_query
                .get(entity)
                .ok()
                .and_then(|d| match d.kind {
                    DirectiveKind::Cleanse => Some(CraftingHint::Cleanse),
                    DirectiveKind::HarvestCarcass => Some(CraftingHint::HarvestCarcass),
                    _ => None,
                });

            if let Some(h) = directive_hint {
                Some(h)
            } else {
                let herbcraft_score = scores
                    .iter()
                    .find(|(a, _)| *a == Action::Herbcraft)
                    .map(|(_, s)| *s)
                    .unwrap_or(0.0);
                let magic_score = scores
                    .iter()
                    .find(|(a, _)| *a == Action::PracticeMagic)
                    .map(|(_, s)| *s)
                    .unwrap_or(0.0);
                let cook_score = scores
                    .iter()
                    .find(|(a, _)| *a == Action::Cook)
                    .map(|(_, s)| *s)
                    .unwrap_or(0.0);
                // Cook must strictly dominate both peers — ties go to the
                // legacy Magic/Herbcraft routing. Mirrors the dead-code
                // reference at `disposition.rs:947-969` which had this
                // branch but was never ported when `evaluate_and_plan`
                // took over from `evaluate_dispositions`. Without it,
                // softmax-picked Cook intentions silently route to
                // PracticeMagic and `Feature::FoodCooked` never fires
                // (ticket 036).
                if cook_score > herbcraft_score && cook_score > magic_score {
                    Some(CraftingHint::Cook)
                } else if magic_score > herbcraft_score {
                    result.magic_hint.or(Some(CraftingHint::Magic))
                } else {
                    result.herbcraft_hint
                }
            }
        } else {
            None
        };

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
            entity,
            d,
        );
        let mut actions = actions_for_disposition(chosen, crafting_hint, &zone_distances);
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
        let goal = goal_for_disposition(chosen, 0, &plan_ctx);

        if let Some(steps) = make_plan(planner_state, &actions, &goal, 12, 1000, &plan_ctx) {
            let mut plan = GoapPlan::new(chosen, res.time.tick, personality, steps, crafting_hint);
            if chosen == DispositionKind::Resting {
                plan.max_replans = d.resting_max_replans;
            }
            // Flow ward placement position from coordinator directive.
            if crafting_hint == Some(CraftingHint::SetWard) {
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

            current.ticks_remaining = u64::MAX;
            commands.entity(entity).insert(plan);
        } else {
            // Ticket 123 — author the disposition-failure memory
            // before the event push. The IAUS-side cooldown reads
            // this on the next tick to suppress the same-disposition
            // re-pick (3059 wasted planning rounds in seed-42's
            // 1500-tick cold-start window came from the unbroken
            // retry loop). Lazy-insert via Commands when the cat
            // doesn't yet have the component — Commands buffer until
            // apply, but the cooldown signal degrades gracefully
            // (single-tick miss vs the 4000-tick cooldown window).
            if let Some(ref mut recent) = recent_disposition_failures {
                recent.record(chosen, res.time.tick);
            } else {
                let mut fresh = crate::components::RecentDispositionFailures::default();
                fresh.record(chosen, res.time.tick);
                commands.entity(entity).insert(fresh);
            }
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
                log.push(
                    res.time.tick,
                    EventKind::PlanningFailed {
                        cat: name.0.clone(),
                        disposition: format!("{:?}", chosen),
                        reason: "no_plan_found".into(),
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
    cat_tile_counts: HashMap<Position, u32>,
    stores_positions: Vec<Position>,
    stores_entities: Vec<(Entity, Position)>,
    kitchen_positions: Vec<Position>,
    construction_positions: Vec<(Entity, Position)>,
    farm_positions: Vec<Position>,
    herb_positions: Vec<(Entity, Position, HerbKind)>,
    /// Ground items whose `kind.material()` is `Some(_)`. Authored each
    /// tick from a `Without<GoapPlan>` items query so the planner's
    /// `PlannerZone::MaterialPile` resolves to the nearest haulable pile.
    material_pile_positions: Vec<(Entity, Position, ItemKind)>,
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
    injured_cat_positions: Vec<(Entity, Position)>,
    cat_skills: HashMap<Entity, Skills>,
    cat_temperature: HashMap<Entity, f32>,
    kitten_parents: HashMap<Entity, (Option<Entity>, Option<Entity>)>,
    kitten_snapshot: Vec<crate::ai::caretake_targeting::KittenState>,
    building_snapshot: Vec<(Entity, StructureType, Position, bool, bool)>,
}

/// Mutable accumulators written by `dispatch_step_action`, consumed by the
/// post-loop cleanup pass in `resolve_goap_plans`.
struct StepAccumulators {
    mentor_effects: Vec<MentorEffect>,
    grooming_restorations: Vec<crate::steps::disposition::GroomOutcome>,
    kitten_feedings: Vec<Entity>,
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
            ),
            (
                &Gender,
                Option<&mut ActionHistory>,
                &mut HuntingPriors,
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
    mut colony_map: ResMut<ColonyHuntingMap>,
    den_query: Query<(Entity, &PreyDen, &Position), Without<PreyAnimal>>,
    mut prey_params: PreyHuntParams,
    mut commands: Commands,
    mut ec: ExecutorContext,
    mut building_params: BuildingResolverParams,
    mut magic_params: MagicResolverParams,
    mut plan_writer: MessageWriter<PlanNarrative>,
) {
    let mut plans_to_remove: Vec<Entity> = Vec::new();

    // Pre-collect building and herb data to avoid query conflicts with cats.
    let building_snapshot: Vec<(Entity, StructureType, Position, bool, bool)> = building_params
        .buildings
        .iter()
        .map(|(e, s, site, crop, p)| (e, s.kind, *p, site.is_some(), crop.is_some()))
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
        m
    };

    // Only completed kitchens count — a construction site can't be cooked at.
    let kitchen_entities: Vec<(Entity, Position)> = building_snapshot
        .iter()
        .filter(|(_, kind, _, is_site, _)| *kind == StructureType::Kitchen && !*is_site)
        .map(|(e, _, p, _, _)| (*e, *p))
        .collect();
    let kitchen_positions: Vec<Position> = kitchen_entities.iter().map(|(_, p)| *p).collect();

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
    for ((entity, _, _, pos, _, _, _, _, _), _) in &cats {
        planner_markers.set_entity(
            markers::MaterialsAvailable::KEY,
            entity,
            materials_available_for(pos, &construction_positions, &construction_materials_complete),
        );
    }

    // Count cats adjacent to each construction site (for multi-builder bonuses).
    let builders_per_site: HashMap<Entity, usize> = {
        let cat_pos_list: Vec<Position> = cats
            .iter()
            .map(|((_, _, _, pos, _, _, _, _, _), _)| *pos)
            .collect();
        let mut counts = HashMap::new();
        for (site_e, _, site_pos, is_site, _) in &building_snapshot {
            if *is_site {
                let n = cat_pos_list
                    .iter()
                    .filter(|cp| cp.manhattan_distance(site_pos) <= 1)
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
                |((e, _, _, _, _, _, _, _, _), (_, _, _, g, _, _, _, _, _, _, _, _))| {
                    (e, g.as_ref().map_or(0.8, |g| g.0))
                },
            )
            .collect(),
        // Gender snapshot for §7.M.7.4's `resolve_mate_with` partner lookup —
        // lets the MateWith step pick the gestation-capable partner without
        // double-borrowing the mutable `cats` query.
        gender: cats
            .iter()
            .map(|((e, _, _, _, _, _, _, _, _), (g, _, _, _, _, _, _, _, _, _, _, _))| (e, *g))
            .collect(),
        cat_tile_counts: {
            let mut counts = HashMap::new();
            for ((_, _, _, pos, _, _, _, _, _), _) in &cats {
                *counts.entry(*pos).or_insert(0) += 1;
            }
            counts
        },
        stores_positions,
        stores_entities,
        kitchen_positions,
        construction_positions,
        farm_positions,
        herb_positions,
        material_pile_positions,
        planner_markers,
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
            .map(|((e, _, _, pos, _, _, _, _, _), _)| (e, *pos))
            .collect(),
        injured_cat_positions: cats
            .iter()
            .filter(|(_, (_, _, _, _, _, health, _, _, _, _, _, _))| health.current < health.max)
            .map(|((e, _, _, pos, _, _, _, _, _), _)| (e, *pos))
            .collect(),
        // §6.5.3 mentor-target DSE snapshot: candidate-side Skills lookup
        // table. Built once per tick so the MentorCat branch can rank
        // apprentices by skill-gap without re-borrowing `cats` (which is
        // mutably held by the outer loop).
        cat_skills: cats
            .iter()
            .map(|((e, _, _, _, skills, _, _, _, _), _)| (e, (*skills).clone()))
            .collect(),
        // §6.5.4 groom-other-target DSE snapshot: candidate-side
        // `needs.temperature` lookup. Same rationale as skills — the outer
        // loop mutably holds `cats`, so we materialize a read-only map for
        // the GroomOther branch's `resolve_groom_other_target` call.
        cat_temperature: cats
            .iter()
            .map(|((e, _, _, _, _, needs, _, _, _), _)| (e, needs.temperature))
            .collect(),
        // §6.5.4 kinship lookup — `(kitten_entity) → (mother, father)`.
        // Bidirectional `is_kin` is computed per-call by the resolver
        // closure. Reads `ExecutorContext::kitten_parentage` — kittens
        // don't carry `GoapPlan`, so this query is disjoint from the
        // outer mutable `cats` iteration.
        kitten_parents: ec
            .kitten_parentage
            .iter()
            .map(|(e, dep)| (e, (dep.mother, dep.father)))
            .collect(),
        // §Phase 4c.3: kitten snapshot for goap-path Caretake / FeedKitten.
        // Built from the main cats query itself (immutable pre-loop
        // iteration) to avoid a separate kitten query that would conflict
        // with `&mut Needs`. Only kittens with GoapPlan end up here; the
        // disposition path (`resolve_disposition_chains`) captures
        // kittens on the chain-building branch separately.
        kitten_snapshot: Vec::new(),
        building_snapshot,
    };

    let mut accum = StepAccumulators {
        mentor_effects: Vec::new(),
        grooming_restorations: Vec::new(),
        // §Phase 4c.3: deferred kitten-feedings — the cats query already
        // owns &mut Needs over every non-dead cat (including kittens), so
        // updates are collected here and applied in a second pass.
        kitten_feedings: Vec::new(),
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
        ),
        (
            gender,
            history,
            mut hunting_priors,
            mut grooming,
            mut mood,
            mut health,
            magic_aff,
            mut corruption,
            mut memory,
            mut urgencies,
            mut fulfillment_opt,
            mut recent_failures,
        ),
    ) in &mut cats
    {
        let d = &ec.constants.disposition;

        // §7.2 commitment gate — evaluate whether to drop the held intention.
        let strategy = crate::ai::commitment::strategy_for_disposition(plan.kind);
        let unexplored_nearby = prey_params.exploration_map.unexplored_fraction_nearby(
            pos.x,
            pos.y,
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
            plans_to_remove.push(cat_entity);
            continue;
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
            let disposition_complete = match plan.kind {
                DispositionKind::Resting => {
                    needs.hunger >= d.resting_complete_hunger
                        && needs.energy >= d.resting_complete_energy
                        && needs.temperature >= d.resting_complete_temperature
                }
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
                plans_to_remove.push(cat_entity);
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
                    cat_entity,
                    d,
                );
                let actions =
                    actions_for_disposition(plan.kind, plan.crafting_hint, &zone_distances);
                let plan_ctx = crate::ai::planner::PlanContext {
                    markers: &snaps.planner_markers,
                    entity: cat_entity,
                };
                let goal = goal_for_disposition(plan.kind, plan.trips_done, &plan_ctx);

                if let Some(new_steps) =
                    make_plan(planner_state, &actions, &goal, 12, 1000, &plan_ctx)
                {
                    plan.replan(new_steps);
                } else {
                    // Can't plan next trip — complete anyway.
                    current.ticks_remaining = 0;
                    plans_to_remove.push(cat_entity);
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
            current.action = action_kind.to_action(plan.kind);
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
            &mut skills,
            &mut needs,
            &mut inventory,
            personality,
            name,
            gender,
            &mut hunting_priors,
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
            &mut colony_map,
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
                    let current_maslow = plan.kind.maslow_level();
                    // An urgency preempts only if its maslow level is strictly
                    // lower (more fundamental) than the current plan's.
                    if urgent.maslow_level < current_maslow {
                        // Preserve Hunt/Herbcraft guard for threats.
                        let suppressed = urgent.kind == UrgencyKind::ThreatNearby
                            && matches!(current.action, Action::Hunt | Action::Herbcraft);

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
                                            "urgency {:?} (level {}) preempted level {} plan",
                                            urgent.kind, urgent.maslow_level, current_maslow
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
                                    let dx = pos.x - threat_pos.x;
                                    let dy = pos.y - threat_pos.y;
                                    let len = ((dx * dx + dy * dy) as f32).sqrt().max(1.0);
                                    let fd = d.flee_distance;
                                    let mut target = Position::new(
                                        pos.x + (dx as f32 / len * fd) as i32,
                                        pos.y + (dy as f32 / len * fd) as i32,
                                    );
                                    target.x = target.x.clamp(0, ec.map.width - 1);
                                    target.y = target.y.clamp(0, ec.map.height - 1);
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
                            plans_to_remove.push(cat_entity);

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
                    current.action = step.action.to_action(plan.kind);
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

                // Record the failed action so replanning can exclude it.
                // Ticket 072: routed through `plan_substrate::record_step_failure`
                // — body is verbatim today; 073 extends it to update
                // `RecentTargetFailures` for cross-plan target memory.
                let failed_action = plan.current().map(|s| s.action);
                let failed_target = plan
                    .current_state()
                    .and_then(|s| s.target_entity);
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
                        commands.entity(cat_entity).insert(
                            crate::components::RecentTargetFailures::default(),
                        );
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
                    cat_entity,
                    d,
                );
                let mut actions =
                    actions_for_disposition(plan.kind, plan.crafting_hint, &zone_distances);
                actions.retain(|a| !plan.failed_actions.contains(&a.kind));
                let plan_ctx = crate::ai::planner::PlanContext {
                    markers: &snaps.planner_markers,
                    entity: cat_entity,
                };
                let goal = goal_for_disposition(plan.kind, plan.trips_done, &plan_ctx);

                if let Some(new_steps) =
                    make_plan(planner_state, &actions, &goal, 12, 1000, &plan_ctx)
                {
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
                            commands.entity(cat_entity).insert(
                                crate::components::RecentTargetFailures::default(),
                            );
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
                        plans_to_remove.push(cat_entity);
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
                        commands.entity(cat_entity).insert(
                            crate::components::RecentTargetFailures::default(),
                        );
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
                    plans_to_remove.push(cat_entity);
                }
            }
        }
    }

    // Remove completed/abandoned plans.
    for entity in plans_to_remove {
        commands.entity(entity).remove::<GoapPlan>();
    }

    let d = &ec.constants.disposition;

    // Deferred grooming restorations — apply grooming condition delta and
    // §7.W social_warmth delta to the groomed target.
    for groom in accum.grooming_restorations {
        if let Ok((_, (_, _, _, grooming, _, _, _, _, _, _, fulfillment, _))) =
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

    // §Phase 4c.4: deferred kitten-feedings. +0.5 hunger per feed.
    // Uses the disjoint `ec.kitten_needs` query because kittens don't
    // have `GoapPlan` — the previous version routed through `cats`,
    // which requires `&mut GoapPlan` and therefore excluded every
    // kitten. `KittenFed` activations fired (the adult consumed food
    // from inventory) but the kitten-side hunger credit was silently
    // dropped. See `ExecutorContext::kitten_needs` doc for the full
    // diagnosis.
    for kitten_entity in accum.kitten_feedings {
        if let Ok(mut k_needs) = ec.kitten_needs.get_mut(kitten_entity) {
            k_needs.hunger = (k_needs.hunger + 0.5).min(1.0);
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
        } else if let Ok(((_, _, _, _, s, _, _, _, _), _)) = cats.get(effect.apprentice) {
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
                } else if let Ok(((_, _, _, _, mut s, _, _, _, _), _)) =
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
    skills: &mut Skills,
    needs: &mut Needs,
    inventory: &mut Inventory,
    personality: &Personality,
    name: &Name,
    gender: &Gender,
    hunting_priors: &mut HuntingPriors,
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
    colony_map: &mut ColonyHuntingMap,
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
) -> crate::steps::StepResult {
    let d = &ec.constants.disposition;

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
    if let Some(target) = plan.step_state[step_idx].target_entity {
        if let Err(reason) = crate::systems::plan_substrate::validate_target(
            target,
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

    match action_kind {
        GoapActionKind::TravelTo(zone) => resolve_travel_to(
            zone,
            &mut plan.step_state[step_idx],
            pos,
            &ec.map,
            &prey_params.exploration_map,
            &snaps.cat_tile_counts,
            &snaps.stores_positions,
            &snaps.construction_positions,
            &snaps.farm_positions,
            &snaps.herb_positions,
            &snaps.kitchen_positions,
            &snaps.cat_positions,
            &snaps.material_pile_positions,
            cat_entity,
            d,
        ),

        GoapActionKind::SearchPrey => resolve_search_prey(
            &mut plan.step_state[step_idx],
            ticks,
            pos,
            hunting_priors,
            colony_map,
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
            &|e: Entity| ec.stance_overlays_of(e),
            ec_is_focal(ec, cat_entity),
            ec.focal_capture.as_deref(),
            recent_failures,
            ec.constants
                .planning_substrate
                .target_failure_cooldown_ticks,
        ),

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
            resolve_engage_prey(
                &mut plan.step_state[step_idx],
                ticks,
                pos,
                inventory,
                skills,
                hunting_priors,
                prey_query,
                prey_params,
                &ec.map,
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
                    .min_by_key(|(_, sp)| pos.manhattan_distance(sp))
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
        ),

        GoapActionKind::EatAtStores => {
            if plan.step_state[step_idx].target_entity.is_none() {
                plan.step_state[step_idx].target_entity = snaps
                    .stores_entities
                    .iter()
                    .min_by_key(|(_, sp)| pos.manhattan_distance(sp))
                    .map(|(e, _)| *e);
            }
            let outcome = crate::steps::disposition::resolve_eat_at_stores(
                ticks,
                plan.step_state[step_idx].target_entity,
                needs,
                stores_query,
                items_query,
                commands,
                d,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::FoodEaten);
            outcome.result
        }

        GoapActionKind::Sleep => {
            let duration = d.sleep_duration_base
                + ((1.0 - needs.energy) * d.sleep_duration_deficit_multiplier) as u64;
            // Corruption degrades rest quality.
            let tile_corruption = if ec.map.in_bounds(pos.x, pos.y) {
                ec.map.get(pos.x, pos.y).corruption
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
                let stance_overlays = |e: Entity| ec.stance_overlays_of(e);
                // Ticket 027b §7.M — look up the L2 PairingActivity
                // partner so `socialize_target::bond_score` can pin
                // the Intention partner at 1.0 regardless of bond
                // tier. Falls back to `None` for cats without an
                // Intention (the steady-state for non-reproductive
                // or partnerless cats).
                let pairing_partner = ec
                    .pairing_q
                    .get(cat_entity)
                    .ok()
                    .map(|p| p.partner);
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
                        pairing_partner,
                        recent_failures,
                        ec.constants
                            .planning_substrate
                            .target_failure_cooldown_ticks,
                        narr.activation.as_deref_mut(),
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
            let outcome = crate::steps::disposition::resolve_socialize(
                ticks,
                cat_entity,
                plan.step_state[step_idx].target_entity,
                needs,
                fulfillment_ref,
                hunting_priors,
                relationships,
                colony_map,
                &snaps.grooming,
                ec.time.tick,
                &ec.constants.social,
                d,
                &ec.constants.fulfillment,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::Socialized);
            if matches!(outcome.result, crate::steps::StepResult::Advance) {
                magic_params
                    .pushback_writer
                    .write(crate::systems::magic::CorruptionPushback {
                        position: *pos,
                        radius: 2,
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
                        &is_kin,
                        relationships,
                        ec.time.tick,
                        focal_hook,
                        recent_failures,
                        ec.constants
                            .planning_substrate
                            .target_failure_cooldown_ticks,
                        narr.activation.as_deref_mut(),
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
            let outcome = crate::steps::disposition::resolve_groom_other(
                ticks,
                cat_entity,
                plan.step_state[step_idx].target_entity,
                needs,
                fulfillment_ref,
                hunting_priors,
                relationships,
                colony_map,
                &snaps.grooming,
                ec.time.tick,
                &ec.constants.social,
                d,
                &ec.constants.fulfillment,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::GroomedOther);
            if let Some(r) = outcome.witness {
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
                    );
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
            crate::steps::disposition::resolve_patrol_to(
                pos,
                plan.step_state[step_idx].target_position,
                &mut plan.step_state[step_idx].cached_path,
                needs,
                &ec.map,
                d,
                &snaps.cat_tile_counts,
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
                let self_health_fraction = if health.max > 0.0 {
                    (health.current / health.max).clamp(0.0, 1.0)
                } else {
                    0.0
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
                let stance_overlays = |e: Entity| ec.stance_overlays_of(e);
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
                let dist = pos.manhattan_distance(&target_pos);
                if dist > 1 {
                    if plan.step_state[step_idx].cached_path.is_none()
                        || plan.step_state[step_idx]
                            .cached_path
                            .as_ref()
                            .and_then(|p| p.last())
                            .is_some_and(|last| *last != target_pos)
                    {
                        plan.step_state[step_idx].cached_path =
                            crate::ai::pathfinding::find_path(*pos, target_pos, &ec.map);
                    }
                    if let Some(ref mut path) = plan.step_state[step_idx].cached_path {
                        if let Some(next) = path.first().copied() {
                            path.remove(0);
                            *pos = next;
                        }
                    }
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
                    );
            }
            let target = plan.step_state[step_idx].target_entity;
            let target_gender = target.and_then(|t| snaps.gender.get(&t).copied());
            let outcome = crate::steps::disposition::resolve_mate_with(
                ticks,
                cat_entity,
                *gender,
                target,
                target_gender,
                needs,
                relationships,
            );
            // MatingOccurred fires only when a pregnancy was produced.
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::MatingOccurred);
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
                            location: (pos.x, pos.y),
                        },
                    );
                }
                magic_params
                    .pushback_writer
                    .write(crate::systems::magic::CorruptionPushback {
                        position: *pos,
                        radius: 2,
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
                plan.step_state[step_idx].target_entity =
                    crate::ai::dses::caretake_target::resolve_caretake_target(
                        &ec.dse_registry,
                        cat_entity,
                        *pos,
                        &snaps.kitten_snapshot,
                        &[],
                        ec.time.tick,
                        focal_hook,
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
                    .min_by_key(|(_, sp)| pos.manhattan_distance(sp))
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
                    .min_by_key(|(_, hp, _)| pos.manhattan_distance(hp))
                    .map(|(e, _, _)| *e);
            }
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
            }
            result
        }

        GoapActionKind::SetWard => {
            // Walk to ward placement target if one was set by the coordinator.
            if let Some(ward_target) = plan.ward_placement_pos {
                if pos.manhattan_distance(&ward_target) > 1 {
                    if plan.step_state[step_idx].cached_path.is_none() {
                        plan.step_state[step_idx].cached_path =
                            crate::ai::pathfinding::find_path(*pos, ward_target, &ec.map);
                    }
                    if let Some(ref mut path) = plan.step_state[step_idx].cached_path {
                        if let Some(next) = path.first().copied() {
                            path.remove(0);
                            *pos = next;
                        }
                    }
                    crate::steps::StepResult::Continue
                } else {
                    let ward_kind = match plan.crafting_hint {
                        Some(crate::components::disposition::CraftingHint::DurableWard) => {
                            crate::components::magic::WardKind::DurableWard
                        }
                        _ => crate::components::magic::WardKind::Thornward,
                    };
                    let result = crate::steps::magic::resolve_set_ward(
                        ticks,
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
                        ec.time.tick,
                        &ec.constants.magic,
                        &ec.constants.combat,
                        &ec.time_scale,
                    );
                    if matches!(result, crate::steps::StepResult::Advance) {
                        if let Some(ref mut act) = narr.activation {
                            act.record(Feature::WardPlaced);
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
                let ward_kind = match plan.crafting_hint {
                    Some(crate::components::disposition::CraftingHint::DurableWard) => {
                        crate::components::magic::WardKind::DurableWard
                    }
                    _ => crate::components::magic::WardKind::Thornward,
                };
                let result = crate::steps::magic::resolve_set_ward(
                    ticks,
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
                    ec.time.tick,
                    &ec.constants.magic,
                    &ec.constants.combat,
                    &ec.time_scale,
                );
                if matches!(result, crate::steps::StepResult::Advance) {
                    if let Some(ref mut act) = narr.activation {
                        act.record(Feature::WardPlaced);
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
                *kind == StructureType::Stores && pos.manhattan_distance(p) <= 1
            });
            crate::steps::magic::resolve_prepare_remedy(
                ticks,
                remedy,
                at_workshop,
                inventory,
                skills,
                &ec.constants.magic,
                &ec.time_scale,
            )
        }

        GoapActionKind::ApplyRemedy => {
            if plan.step_state[step_idx].target_entity.is_none() {
                if let Some((patient_e, patient_pos)) = snaps
                    .injured_cat_positions
                    .iter()
                    .filter(|(e, _)| *e != cat_entity)
                    .min_by_key(|(_, cp)| pos.manhattan_distance(cp))
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
            let (result, gratitude) = crate::steps::magic::resolve_apply_remedy(
                remedy,
                cat_entity,
                plan.step_state[step_idx].target_position,
                plan.step_state[step_idx].target_entity,
                patient_alive,
                &mut plan.step_state[step_idx].cached_path,
                pos,
                skills,
                &ec.map,
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
                        radius: 4,
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
                if pos.manhattan_distance(&target) > 0 {
                    if plan.step_state[step_idx].cached_path.is_none() {
                        plan.step_state[step_idx].cached_path =
                            crate::ai::pathfinding::find_path(*pos, target, &ec.map);
                    }
                    if let Some(ref mut path) = plan.step_state[step_idx].cached_path {
                        if !path.is_empty() {
                            *pos = path.remove(0);
                        }
                    }
                    crate::steps::StepResult::Continue
                } else {
                    // Arrived: perform the cleanse.
                    let result = crate::steps::magic::resolve_cleanse_corruption(
                        ticks,
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
                            if !carcass.cleansed && pos.manhattan_distance(cp) <= 1 {
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
                        .min_by_key(|(_, _, cp)| cp.manhattan_distance(&target_pos))
                        .map(|(e, _, _)| e);
                } else {
                    plan.step_state[step_idx].target_entity = magic_params
                        .carcass_query
                        .iter()
                        .filter(|(_, c, _)| !c.harvested)
                        .min_by_key(|(_, _, cp)| pos.manhattan_distance(cp))
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
                    .is_some_and(|target| pos.manhattan_distance(&target) > 0);

                if walking {
                    let target = plan.step_state[step_idx].target_position.unwrap();
                    if plan.step_state[step_idx].cached_path.is_none() {
                        plan.step_state[step_idx].cached_path =
                            crate::ai::pathfinding::find_path(*pos, target, &ec.map);
                    }
                    if let Some(ref mut path) = plan.step_state[step_idx].cached_path {
                        if !path.is_empty() {
                            *pos = path.remove(0);
                        }
                    }
                    crate::steps::StepResult::Continue
                } else if ticks >= ec.constants.magic.harvest_carcass_duration.ticks(&ec.time_scale) {
                    if let Ok((_, mut carcass, _)) =
                        magic_params.carcass_query.get_mut(carcass_entity)
                    {
                        carcass.harvested = true;
                        let harvest_corruption = if ec.map.in_bounds(pos.x, pos.y) {
                            ec.map.get(pos.x, pos.y).corruption
                        } else {
                            0.0
                        };
                        inventory.add_item_with_modifiers(
                            crate::components::items::ItemKind::ShadowBone,
                            crate::components::items::ItemModifiers::with_corruption(
                                harvest_corruption,
                            ),
                        );
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
                    .min_by_key(|(_, cp)| pos.manhattan_distance(cp))
                    .map(|(e, _)| *e);
            }
            let outcome = crate::steps::building::resolve_construct(
                plan.step_state[step_idx].target_entity,
                pos,
                &mut plan.step_state[step_idx].cached_path,
                skills,
                snaps.workshop_bonus,
                &snaps.builders_per_site,
                &mut building_params.buildings,
                &ec.map,
                commands,
                &mut building_params.colony_score,
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
                            location: (pos.x, pos.y),
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
                    .min_by_key(|(_, _, gp, _, _)| pos.manhattan_distance(gp))
                    .map(|(e, _, _, _, _)| *e);
            }
            let outcome = crate::steps::building::resolve_tend(
                plan.step_state[step_idx].target_entity,
                pos,
                &mut plan.step_state[step_idx].cached_path,
                skills,
                snaps.season_mod,
                snaps.workshop_bonus,
                &mut building_params.buildings,
                &ec.map,
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
                    .min_by_key(|(_, _, gp, _, _)| pos.manhattan_distance(gp))
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
                    .min_by_key(|(_, mp, _)| pos.manhattan_distance(mp))
                    .map(|(e, _, _)| *e);
            }
            let target = plan.step_state[step_idx].target_entity;
            let cached = &mut plan.step_state[step_idx].cached_path;
            let outcome = crate::steps::building::resolve_pickup_material(
                target,
                cat_entity,
                pos,
                cached,
                inventory,
                &mut building_params.material_items,
                &ec.map,
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
                    .min_by_key(|(_, cp)| pos.manhattan_distance(cp))
                    .map(|(e, _)| *e);
            }
            let material_carried = inventory.slots.iter().find_map(|s| match s {
                crate::components::magic::ItemSlot::Item(k, _) => k.material(),
                _ => None,
            });
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
                    .min_by_key(|(_, sp)| pos.manhattan_distance(sp))
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
    }
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

        let terrain = if map.in_bounds(pos.x, pos.y) {
            map.get(pos.x, pos.y).terrain
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
    exploration_map: &ExplorationMap,
    cat_tile_counts: &HashMap<Position, u32>,
    stores_positions: &[Position],
    construction_positions: &[(Entity, Position)],
    farm_positions: &[Position],
    herb_positions: &[(Entity, Position, HerbKind)],
    kitchen_positions: &[Position],
    cat_positions: &[(Entity, Position)],
    material_pile_positions: &[(Entity, Position, ItemKind)],
    cat_entity: Entity,
    d: &DispositionConstants,
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
            cat_entity,
            d,
        );
    }
    let Some(target) = state.target_position else {
        return crate::steps::StepResult::Fail("no reachable zone target".into());
    };

    // Use cached A* path.
    if state.cached_path.is_none() {
        state.cached_path = find_path(*pos, target, map);
    }

    if let Some(ref mut path) = state.cached_path {
        if let Some(next) = path.first().copied() {
            path.remove(0);
            *pos = next;
        }
        if pos.manhattan_distance(&target) <= 1 {
            // Anti-stacking jitter.
            if cat_tile_counts.get(&target).copied().unwrap_or(0) > 1 {
                let occupied: std::collections::HashSet<Position> = cat_tile_counts
                    .keys()
                    .filter(|p| cat_tile_counts[p] > 1)
                    .copied()
                    .collect();
                if let Some(adj) = find_free_adjacent(target, *pos, map, &occupied) {
                    *pos = adj;
                }
            } else {
                *pos = target;
            }
            return crate::steps::StepResult::Advance;
        }
    } else {
        // No path found — step toward target directly.
        let before = *pos;
        if let Some(next) = step_toward(pos, &target, map) {
            *pos = next;
        }
        if pos.manhattan_distance(&target) <= 1 {
            return crate::steps::StepResult::Advance;
        }
        // Early exit: A* found no path and greedy movement made no progress.
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
    hunting_priors: &mut HuntingPriors,
    colony_map: &mut ColonyHuntingMap,
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
) -> crate::steps::StepResult {
    use crate::components::magic::ItemSlot;

    // Den discovery check.
    for (den_entity, den, den_pos) in den_query.iter() {
        if pos.manhattan_distance(den_pos) <= d.den_discovery_range {
            let discovery_chance =
                d.den_discovery_base_chance + skills.hunting * d.den_discovery_skill_scale;
            if rng.rng.random::<f32>() < discovery_chance && den.spawns_remaining > 0 {
                let kills = ((den.spawns_remaining as f32 * d.den_raid_kill_fraction).ceil()
                    as u32)
                    .min(den.raid_drop);
                let drop_item = den.item_kind;
                let den_name = den.den_name;
                let den_pos_copy = *den_pos;

                let den_corruption = if map.in_bounds(den_pos_copy.x, den_pos_copy.y) {
                    map.get(den_pos_copy.x, den_pos_copy.y).corruption
                } else {
                    0.0
                };
                let den_mods =
                    crate::components::items::ItemModifiers::with_corruption(den_corruption);
                for _ in 0..kills {
                    if !inventory.is_full() {
                        inventory.slots.push(ItemSlot::Item(drop_item, den_mods));
                    } else {
                        commands.spawn((
                            crate::components::items::Item::with_modifiers(
                                drop_item,
                                d.den_dropped_item_quality,
                                ItemLocation::OnGround,
                                den_mods,
                            ),
                            Position::new(
                                den_pos_copy.x + rng.rng.random_range(-1..=1i32),
                                den_pos_copy.y + rng.rng.random_range(-1..=1i32),
                            ),
                        ));
                    }
                }

                hunting_priors.record_catch(&den_pos_copy);
                colony_map.beliefs.record_catch(&den_pos_copy);

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

    // Search movement: belief > colony belief > wind > patrol_dir.
    let belief_dir = hunting_priors.best_direction(pos, d.search_belief_radius);
    let colony_dir = colony_map
        .beliefs
        .best_direction(pos, d.search_belief_radius);
    let (wx, wy) = wind.direction();
    let (mut dx, mut dy) = if let Some((bx, by)) = belief_dir {
        (bx, by)
    } else if let Some((cx, cy)) = colony_dir {
        (cx, cy)
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
    // pre-refactor `min_by_key(manhattan_distance)` pick — the legacy
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
                d.search_visual_detection_range as f32,
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
        );
        if let Some(prey_entity) = picked {
            state.target_entity = Some(prey_entity);
            return crate::steps::StepResult::Advance;
        }
    }

    // Scent detection via PreyScentMap (Phase 2B — grid-sampled
    // influence map). Finds the strongest-scent bucket within
    // `scent_search_radius`; `min_by_key` resolves to the prey
    // entity closest to that source tile.
    let scent_source =
        prey_params
            .prey_scent_map
            .highest_nearby(pos.x, pos.y, d.scent_search_radius);
    let scent_above_threshold = scent_source
        .map(|(sx, sy)| prey_params.prey_scent_map.get(sx, sy) >= d.scent_detect_threshold)
        .unwrap_or(false);
    let scented_prey = if scent_above_threshold {
        let (sx, sy) = scent_source.unwrap();
        let source = Position::new(sx, sy);
        prey_query
            .iter()
            .min_by_key(|(_, pp, _, _)| source.manhattan_distance(pp))
    } else {
        None
    };

    if let Some((prey_entity, prey_pos_ref, _, _)) = scented_prey {
        state.target_entity = Some(prey_entity);
        hunting_priors.record_scent(prey_pos_ref);
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
        if inventory
            .slots
            .iter()
            .any(|s| matches!(s, ItemSlot::Item(k, _) if k.is_food()))
        {
            // Have food from earlier — advance to deposit.
            return crate::steps::StepResult::Advance;
        }
        hunting_priors.record_failed_search(pos, ticks);
        return crate::steps::StepResult::Fail("no scent found".into());
    }

    crate::steps::StepResult::Continue
}

// ===========================================================================
// Helper: resolve EngagePrey (transplanted from HuntPrey stalk/chase/pounce)
// ===========================================================================

#[allow(clippy::too_many_arguments)]
fn resolve_engage_prey(
    state: &mut StepExecutionState,
    ticks: u64,
    pos: &mut Position,
    inventory: &mut Inventory,
    skills: &mut Skills,
    hunting_priors: &mut HuntingPriors,
    prey_query: &mut Query<(Entity, &Position, &PreyConfig, &mut PreyState), With<PreyAnimal>>,
    prey_params: &mut PreyHuntParams,
    map: &TileMap,
    narr: &mut NarrativeEmitter<'_>,
    time: &TimeState,
    rng: &mut SimRng,
    commands: &mut Commands,
    cat_entity: Entity,
    personality: &Personality,
    name: &Name,
    gender: &Gender,
    needs: &Needs,
    d: &DispositionConstants,
    mut event_log: Option<&mut EventLog>,
) -> crate::steps::StepResult {
    use crate::components::magic::ItemSlot;
    use crate::components::prey::PreyAiState;

    let Some(target_entity) = state.target_entity else {
        return crate::steps::StepResult::Fail("no prey target for engage".into());
    };

    let Ok((_, prey_pos, prey_cfg, prey_state)) = prey_query.get(target_entity) else {
        // Prey despawned.
        return crate::steps::StepResult::Fail("prey despawned".into());
    };

    let prey_pos = *prey_pos;
    let prey_is_fleeing = matches!(prey_state.ai_state, PreyAiState::Fleeing { .. });
    let prey_awareness = prey_state.ai_state;
    let catch_mod = prey_cfg.catch_difficulty;
    let item_kind = prey_cfg.item_kind;
    let species_name = prey_cfg.name;
    let flee_strategy = prey_cfg.flee_strategy;
    let dist = pos.manhattan_distance(&prey_pos);

    // Bird teleport — give up immediately.
    if prey_is_fleeing && flee_strategy == crate::components::prey::FleeStrategy::Teleport {
        return crate::steps::StepResult::Fail("prey teleported".into());
    }

    let stalk_start = (prey_cfg.alert_radius + d.stalk_start_buffer).max(d.stalk_start_minimum);
    let pounce_range: i32 = if personality.patience > 0.7 {
        d.pounce_range_patient
    } else if personality.patience < 0.3 {
        d.pounce_range_impatient
    } else {
        d.pounce_range_default
    };

    if dist <= pounce_range {
        // === POUNCE ===
        let awareness_base = match prey_awareness {
            PreyAiState::Idle | PreyAiState::Grazing { .. } => d.pounce_awareness_idle,
            PreyAiState::Alert { .. } => d.pounce_awareness_alert,
            PreyAiState::Fleeing { .. } => d.pounce_awareness_fleeing,
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
        let success_chance = awareness_base
            * (d.pounce_skill_base + skills.hunting * d.pounce_skill_scale)
            * distance_mod
            * catch_mod
            * density_bonus;

        if rng.rng.random::<f32>() < success_chance {
            // Catch!
            commands.entity(target_entity).despawn();
            let catch_corruption = if map.in_bounds(prey_pos.x, prey_pos.y) {
                map.get(prey_pos.x, prey_pos.y).corruption
            } else {
                0.0
            };
            if !inventory.is_full() {
                inventory.slots.push(ItemSlot::Item(
                    item_kind,
                    crate::components::items::ItemModifiers::with_corruption(catch_corruption),
                ));
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
                        location: (prey_pos.x, prey_pos.y),
                    },
                );
            }

            let catch_desc = if catch_corruption > 0.3 {
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

            hunting_priors.record_catch(&prey_pos);

            if inventory.is_full() {
                return crate::steps::StepResult::Advance;
            } else {
                // Multi-kill: reset target, keep searching.
                state.target_entity = None;
                return crate::steps::StepResult::Fail("seeking another target".into());
            }
        } else {
            // Miss — prey bolts.
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
                return crate::steps::StepResult::Fail("chase timeout".into());
            }
        }
    } else if dist <= stalk_start {
        if prey_is_fleeing {
            // === CHASE ===
            let mut moved = false;
            for _ in 0..d.chase_speed {
                if let Some(next) = step_toward(pos, &prey_pos, map) {
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
                return crate::steps::StepResult::Fail("stuck while chasing".into());
            }
            let chase_limit = if personality.boldness > 0.7 {
                d.chase_limit_bold
            } else {
                d.chase_limit_default
            };
            if ticks > chase_limit {
                return crate::steps::StepResult::Fail("chase timeout".into());
            }
        } else {
            // === STALK ===
            let mut moved = false;
            if let Some(next) = step_toward(pos, &prey_pos, map) {
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
                return crate::steps::StepResult::Fail("anxiety spooked prey".into());
            }
            if moved {
                state.no_move_ticks = 0;
            } else {
                state.no_move_ticks += 1;
            }
            if state.no_move_ticks > d.chase_stuck_ticks {
                return crate::steps::StepResult::Fail("stuck while stalking".into());
            }
        }
    } else {
        // === APPROACH ===
        let mut moved = false;
        for _ in 0..d.approach_speed {
            if let Some(next) = step_toward(pos, &prey_pos, map) {
                *pos = next;
                moved = true;
            }
        }
        if moved {
            state.no_move_ticks = 0;
        } else {
            state.no_move_ticks += 1;
        }
        if dist > d.approach_give_up_distance || state.no_move_ticks > d.chase_stuck_ticks {
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
    needs: &Needs,
    d: &DispositionConstants,
) -> crate::steps::StepResult {
    use crate::components::items::ItemKind;
    use crate::components::magic::ItemSlot;

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

    if map.in_bounds(pos.x, pos.y) {
        let tile = map.get(pos.x, pos.y);
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
            let forage_corruption = if map.in_bounds(pos.x, pos.y) {
                map.get(pos.x, pos.y).corruption
            } else {
                0.0
            };
            if !inventory.is_full() {
                inventory.slots.push(ItemSlot::Item(
                    item_kind,
                    crate::components::items::ItemModifiers::with_corruption(forage_corruption),
                ));
            }
            skills.foraging += skills.growth_rate() * d.forage_skill_growth;

            let item_name = if forage_corruption > 0.3 {
                format!("corrupted {}", item_kind.name())
            } else {
                item_kind.name().to_string()
            };
            let terrain = if map.in_bounds(pos.x, pos.y) {
                map.get(pos.x, pos.y).terrain
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
            emit_event_narrative(
                narr.registry.as_deref(),
                &mut narr.log,
                time.tick,
                format!("{} finds {}.", name.0, item_name),
                crate::resources::narrative::NarrativeTier::Action,
                &ctx,
                &var_ctx,
                personality,
                needs,
                &mut rng.rng,
            );
            return crate::steps::StepResult::Advance;
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
    let terrain = if map.in_bounds(pos.x, pos.y) {
        map.get(pos.x, pos.y).terrain
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
    let primary = Position::new(pos.x + dx, pos.y + dy);
    if map.in_bounds(primary.x, primary.y) && map.get(primary.x, primary.y).terrain.is_passable() {
        return primary;
    }
    let perp = Position::new(pos.x + dy, pos.y + dx);
    if map.in_bounds(perp.x, perp.y) && map.get(perp.x, perp.y).terrain.is_passable() {
        return perp;
    }
    let rev = Position::new(pos.x - dx, pos.y - dy);
    if map.in_bounds(rev.x, rev.y) && map.get(rev.x, rev.y).terrain.is_passable() {
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
            let p = Position::new(from.x + dx, from.y + dy);
            if !map.in_bounds(p.x, p.y) {
                continue;
            }
            let tile = map.get(p.x, p.y);
            if !predicate(tile.terrain) {
                continue;
            }
            let dist = from.manhattan_distance(&p);
            if dist == 0 {
                continue;
            }
            let tie = mix_hash(from.x, from.y, p.x, p.y);
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
            let p = Position::new(from.x + dx, from.y + dy);
            if map.in_bounds(p.x, p.y) {
                let tile = map.get(p.x, p.y);
                if predicate(tile.terrain) {
                    let dist = from.manhattan_distance(&p);
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
    cat_entity: Entity,
    d: &DispositionConstants,
) -> Option<Position> {
    match zone {
        PlannerZone::Stores => stores_positions
            .iter()
            .min_by_key(|sp| pos.manhattan_distance(sp))
            .copied(),
        PlannerZone::HuntingGround => {
            find_nearest_tile(pos, map, d.hunt_terrain_search_radius, |t| {
                matches!(t, Terrain::DenseForest | Terrain::LightForest)
            })
        }
        PlannerZone::ForagingGround => {
            find_nearest_tile(pos, map, d.forage_terrain_search_radius, |t| {
                t.foraging_yield() > 0.0
            })
        }
        PlannerZone::Farm => farm_positions
            .iter()
            .min_by_key(|fp| pos.manhattan_distance(fp))
            .copied(),
        PlannerZone::ConstructionSite => construction_positions
            .iter()
            .min_by_key(|(_, cp)| pos.manhattan_distance(cp))
            .map(|(_, p)| *p),
        PlannerZone::HerbPatch => herb_positions
            .iter()
            .min_by_key(|(_, hp, _)| pos.manhattan_distance(hp))
            .map(|(_, p, _)| *p),
        PlannerZone::Kitchen => kitchen_positions
            .iter()
            .min_by_key(|kp| pos.manhattan_distance(kp))
            .copied(),
        PlannerZone::RestingSpot => stores_positions
            .iter()
            .min_by_key(|sp| pos.manhattan_distance(sp))
            .map(|sp| Position::new(sp.x + 1, sp.y))
            .or(Some(*pos)),
        PlannerZone::SocialTarget => cat_positions
            .iter()
            .filter(|(other, _)| *other != cat_entity)
            .min_by_key(|(_, op)| pos.manhattan_distance(op))
            .map(|(_, p)| *p),
        PlannerZone::Wilds => exploration_map
            .frontier_centroid()
            .filter(|p| map.in_bounds(p.x, p.y) && map.get(p.x, p.y).terrain.is_passable())
            .or_else(|| find_nearest_tile(pos, map, 20, |t| t.is_passable())),
        PlannerZone::PatrolZone => stores_positions
            .iter()
            .min_by_key(|sp| pos.manhattan_distance(sp))
            .map(|sp| Position::new(sp.x + d.guard_patrol_radius as i32, sp.y))
            .or(Some(*pos)),
        PlannerZone::MaterialPile => material_pile_positions
            .iter()
            .min_by_key(|(_, mp, _)| pos.manhattan_distance(mp))
            .map(|(_, p, _)| *p),
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
            let p = Position::new(origin.x + dx, origin.y + dy);
            if !map.in_bounds(p.x, p.y) {
                continue;
            }
            let c = map.get(p.x, p.y).corruption;
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
    );
    // Ticket 096: the world-fact "this cat's nearest reachable site
    // has `materials_complete()` true" lives in the substrate as the
    // `MaterialsAvailable` marker, authored per-cat in
    // `materials_available_for` at the planner-marker build site.
    // The search-state field `materials_delivered_this_plan` starts
    // false here and is flipped by `DeliverMaterials`'s effect during
    // A* expansion.
    let carrying = if inventory.slots.iter().any(|s| {
        matches!(
            s,
            crate::components::magic::ItemSlot::Item(k, _) if k.material().is_some()
        )
    }) {
        Carrying::BuildMaterials
    } else if inventory
        .slots
        .iter()
        .any(|s| matches!(s, crate::components::magic::ItemSlot::Item(k, _) if k.is_food()))
    {
        if inventory.slots.iter().any(|s| {
            matches!(
                s,
                crate::components::magic::ItemSlot::Item(
                    crate::components::items::ItemKind::RawMouse
                        | crate::components::items::ItemKind::RawRat
                        | crate::components::items::ItemKind::RawBird
                        | crate::components::items::ItemKind::RawFish
                        | crate::components::items::ItemKind::RawRabbit,
                    _
                )
            )
        }) {
            Carrying::Prey
        } else {
            Carrying::ForagedFood
        }
    } else if inventory
        .slots
        .iter()
        .any(|s| matches!(s, crate::components::magic::ItemSlot::Herb(_)))
    {
        Carrying::Herbs
    } else {
        Carrying::Nothing
    };

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
        .min_by_key(|(_, cp)| pos.manhattan_distance(cp))
        .map(|(entity, _)| {
            construction_materials_complete
                .get(entity)
                .copied()
                .unwrap_or(true)
        })
        .unwrap_or(true)
}

fn classify_zone(
    pos: &Position,
    map: &TileMap,
    stores_positions: &[Position],
    construction_positions: &[(Entity, Position)],
    farm_positions: &[Position],
    herb_positions: &[(Entity, Position, HerbKind)],
    material_pile_positions: &[(Entity, Position, ItemKind)],
) -> PlannerZone {
    if stores_positions
        .iter()
        .any(|sp| pos.manhattan_distance(sp) <= 2)
    {
        return PlannerZone::Stores;
    }
    // MaterialPile classifies before ConstructionSite — a pile placed
    // adjacent to a founding site (the wagon-dismantling layout) sits
    // within the site's classify radius too. The cat's plan needs to
    // see "I'm at a pile" first to gate the pickup action.
    if material_pile_positions
        .iter()
        .any(|(_, mp, _)| pos.manhattan_distance(mp) <= 1)
    {
        return PlannerZone::MaterialPile;
    }
    if construction_positions
        .iter()
        .any(|(_, cp)| pos.manhattan_distance(cp) <= 2)
    {
        return PlannerZone::ConstructionSite;
    }
    if farm_positions
        .iter()
        .any(|fp| pos.manhattan_distance(fp) <= 2)
    {
        return PlannerZone::Farm;
    }
    if herb_positions
        .iter()
        .any(|(_, hp, _)| pos.manhattan_distance(hp) <= 3)
    {
        return PlannerZone::HerbPatch;
    }
    if map.in_bounds(pos.x, pos.y) {
        let terrain = map.get(pos.x, pos.y).terrain;
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
    cat_entity: Entity,
    d: &DispositionConstants,
) -> ZoneDistances {
    let mut distances = ZoneDistances::default();

    let zone_positions: Vec<(PlannerZone, Option<Position>)> = vec![
        (
            PlannerZone::Stores,
            stores_positions
                .iter()
                .min_by_key(|sp| pos.manhattan_distance(sp))
                .copied(),
        ),
        (
            PlannerZone::HuntingGround,
            find_nearest_tile(pos, map, d.hunt_terrain_search_radius, |t| {
                matches!(t, Terrain::DenseForest | Terrain::LightForest)
            }),
        ),
        (
            PlannerZone::ForagingGround,
            find_nearest_tile(pos, map, d.forage_terrain_search_radius, |t| {
                t.foraging_yield() > 0.0
            }),
        ),
        (
            PlannerZone::Farm,
            farm_positions
                .iter()
                .min_by_key(|fp| pos.manhattan_distance(fp))
                .copied(),
        ),
        (
            PlannerZone::ConstructionSite,
            construction_positions
                .iter()
                .min_by_key(|(_, cp)| pos.manhattan_distance(cp))
                .map(|(_, p)| *p),
        ),
        (
            PlannerZone::HerbPatch,
            herb_positions
                .iter()
                .min_by_key(|(_, hp, _)| pos.manhattan_distance(hp))
                .map(|(_, p, _)| *p),
        ),
        (
            PlannerZone::Kitchen,
            kitchen_positions
                .iter()
                .min_by_key(|kp| pos.manhattan_distance(kp))
                .copied(),
        ),
        (
            PlannerZone::RestingSpot,
            stores_positions
                .iter()
                .min_by_key(|sp| pos.manhattan_distance(sp))
                .map(|sp| Position::new(sp.x + 1, sp.y)),
        ),
        (
            PlannerZone::SocialTarget,
            cat_positions
                .iter()
                .filter(|(other, _)| *other != cat_entity)
                .min_by_key(|(_, op)| pos.manhattan_distance(op))
                .map(|(_, p)| *p),
        ),
        (PlannerZone::Wilds, Some(*pos)),
        (
            PlannerZone::PatrolZone,
            stores_positions
                .iter()
                .min_by_key(|sp| pos.manhattan_distance(sp))
                .map(|sp| Position::new(sp.x + d.guard_patrol_radius as i32, sp.y)),
        ),
        (
            PlannerZone::MaterialPile,
            material_pile_positions
                .iter()
                .min_by_key(|(_, mp, _)| pos.manhattan_distance(mp))
                .map(|(_, p, _)| *p),
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
            let dist = fp.manhattan_distance(&tp) as u32;
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
    /// `find_nearest_tile` always returned `(from.x, from.y - 1)` because
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
            Position::new(center.x, center.y - 1),
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
                let key = ((p.x - from.x).signum(), (p.y - from.y).signum());
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
        // A ring of passable tiles at manhattan distance 3 from (10, 10),
        // plus one isolated passable tile at distance 5. The picker must
        // return some distance-3 tile, never the distance-5 one.
        let ring: Vec<(i32, i32)> = vec![
            (10, 7),
            (10, 13),
            (7, 10),
            (13, 10),
            (11, 8),
            (9, 12),
            (12, 9),
            (8, 11),
        ];
        for (x, y) in &ring {
            map.set(*x, *y, Terrain::Grass);
        }
        map.set(15, 10, Terrain::Grass); // distance 5 decoy
        let from = Position::new(10, 10);
        let result =
            find_nearest_tile(&from, &map, 10, |t| t.is_passable()).expect("ring is populated");
        assert_eq!(from.manhattan_distance(&result), 3);
        assert!(ring.contains(&(result.x, result.y)));
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
        exploration.recompute_frontier_centroid(
            crate::resources::exploration_map::FRONTIER_THRESHOLD,
        );
        let centroid = exploration
            .frontier_centroid()
            .expect("right half is unexplored");
        assert!(centroid.x >= 20, "centroid sits in the unexplored half");

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
        exploration.recompute_frontier_centroid(
            crate::resources::exploration_map::FRONTIER_THRESHOLD,
        );
        assert!(exploration.frontier_centroid().is_none());

        let cat = Position::new(10, 10);
        let resolved = resolve_wilds(cat, &map, &exploration).expect("open map has neighbors");
        assert_ne!(
            resolved, cat,
            "fallback must never return the cat's own tile (degenerate path)"
        );
        assert!(cat.manhattan_distance(&resolved) >= 1);
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
        exploration.recompute_frontier_centroid(
            crate::resources::exploration_map::FRONTIER_THRESHOLD,
        );
        assert!(exploration.frontier_centroid().is_none());

        let cat = Position::new(10, 10);
        assert_eq!(
            resolve_wilds(cat, &map, &exploration),
            None,
            "no frontier + no reachable passable neighbor → fail visibly"
        );
    }
}
