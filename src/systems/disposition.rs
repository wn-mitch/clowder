use std::collections::HashMap;

use bevy_ecs::prelude::*;
use rand::Rng;

use crate::ai::pathfinding::{find_free_adjacent, step_toward};
use crate::ai::scoring::{score_actions, ScoringContext};
use crate::ai::{Action, CurrentAction};
use crate::components::building::{
    ConstructionSite, CropState, StoredItems, Structure, StructureType,
};
use crate::components::coordination::{ActiveDirective, Directive, DirectiveKind, DirectiveQueue};
use crate::components::disposition::{
    ActionHistory, ActionOutcome, ActionRecord, CraftingHint, Disposition, DispositionKind,
};
use crate::components::hunting_priors::HuntingPriors;
use crate::components::identity::{Gender, LifeStage, Name};
use crate::components::items::Item;
use crate::components::magic::{Harvestable, Herb, Inventory, Ward};
use crate::components::markers;
use crate::components::mental::Memory;
use crate::components::personality::Personality;
use crate::components::physical::{Dead, Health, InjuryKind, Needs, Position};
use crate::components::prey::{
    DenRaided, PreyAnimal, PreyConfig, PreyDen, PreyDensity, PreyKilled, PreyState,
};
use crate::components::skills::{MagicAffinity, Skills};
use crate::components::task_chain::{FailurePolicy, StepKind, TaskChain, TaskStep};
use crate::components::wildlife::WildAnimal;
use crate::resources::colony_hunting_map::ColonyHuntingMap;

/// Bundled system params for prey-related data in `resolve_disposition_chains`.
/// Avoids hitting Bevy's 16-parameter system limit.
#[derive(bevy_ecs::system::SystemParam)]
pub struct PreyHuntParams<'w, 's> {
    pub density: Res<'w, PreyDensity>,
    pub kill_writer: MessageWriter<'w, PreyKilled>,
    pub raid_writer: MessageWriter<'w, DenRaided>,
    pub exploration_map: ResMut<'w, crate::resources::ExplorationMap>,
    /// Health lookup for FightThreat bail-out. Lives here to stay under the
    /// 16-system-param limit — conceptually unrelated to prey hunting.
    pub health_query: Query<'w, 's, &'static Health>,
    /// Phase 2B — scent-detection grid. Cats sample
    /// `highest_nearby(pos, scent_search_radius)` to find prey-scent
    /// source tiles rather than running point-to-point
    /// `cat_smells_prey_windaware` against each prey entity.
    pub prey_scent_map: Res<'w, crate::resources::PreyScentMap>,
}
/// Bundled system params for narrative emission in `resolve_disposition_chains`.
/// Groups NarrativeLog + optional TemplateRegistry + context resources to stay
/// under Bevy's 16-param limit.
#[derive(bevy_ecs::system::SystemParam)]
pub struct NarrativeEmitter<'w> {
    pub log: ResMut<'w, crate::resources::narrative::NarrativeLog>,
    pub registry: Option<Res<'w, crate::resources::narrative_templates::TemplateRegistry>>,
    pub config: Res<'w, crate::resources::time::SimConfig>,
    pub weather: Res<'w, crate::resources::weather::WeatherState>,
    pub activation: Option<ResMut<'w, SystemActivation>>,
}
/// §4.3 marker queries for snapshot population. Bundled to avoid
/// hitting Bevy's 16-parameter system limit. Future marker batches
/// add their queries here rather than as top-level system params.
#[derive(bevy_ecs::system::SystemParam)]
#[allow(clippy::type_complexity)]
pub struct MarkerQueries<'w, 's> {
    pub life_stage: Query<
        'w,
        's,
        (
            Has<markers::Kitten>,
            Has<markers::Young>,
            Has<markers::Adult>,
            Has<markers::Elder>,
        ),
    >,
    /// §4 batch 1 + batch 2: per-cat inventory, state, role, capability.
    pub per_cat: Query<
        'w,
        's,
        (
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
        ),
    >,
    /// §4.2 State markers — split into a separate query so the
    /// per-cat tuple stays small and future State authors can extend
    /// here.
    pub state: Query<
        'w,
        's,
        (
            Has<markers::InCombat>,
            Has<markers::OnCorruptedTile>,
            Has<markers::OnSpecialTerrain>,
        ),
    >,
    /// Ticket 027 Bug 2 — eligibility marker for `MateDse` paired
    /// with ticket 103 — `Has<PairingActivity>` for the
    /// dependent-presence half of `escape_viability`. Bundling lets
    /// the disposition populator answer "does this cat have an
    /// active pair-bond?" without threading a separate query.
    pub mate_eligibility: Query<
        'w,
        's,
        (
            Has<markers::HasEligibleMate>,
            Has<crate::components::PairingActivity>,
        ),
    >,
    /// Ticket 014 Mentoring batch — Mentor / Apprentice / HasMentoringTarget.
    /// Authored by `aspirations::update_training_markers` (Mentor /
    /// Apprentice) and `aspirations::update_mentoring_target_markers`
    /// (HasMentoringTarget).
    pub mentoring: Query<
        'w,
        's,
        (
            Has<markers::Mentor>,
            Has<markers::Apprentice>,
            Has<markers::HasMentoringTarget>,
        ),
    >,
    /// Ticket 014 §4 sensing batch — broad-phase target-existence
    /// markers authored by `sensing::update_target_existence_markers`.
    pub target_existence: Query<
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
    /// Ticket 158 — kinship-channel substrate. Authored each tick by
    /// `growth::update_kitten_cry_map` (ticket 161 merged the author
    /// in there to avoid a new schedule conflict edge). The populate
    /// sites in `evaluate_dispositions` / `evaluate_and_plan` read
    /// this and pass the bool to `resolve_caretake_target` as the
    /// `parent_marker_active` fallback gate.
    pub parent_hungry_kitten: Query<'w, 's, Has<markers::IsParentOfHungryKitten>>,
}

use crate::resources::food::FoodStores;
use crate::resources::fox_scent_map::FoxScentMap;
use crate::resources::map::{Terrain, TileMap};

/// Bundled read-only resources for `disposition_to_chain`.
/// Groups map, food, relationships, and fox scent map to stay under Bevy's
/// 16-parameter system limit.
#[derive(bevy_ecs::system::SystemParam)]
pub struct ChainResources<'w> {
    pub map: Res<'w, TileMap>,
    pub food: Res<'w, FoodStores>,
    pub relationships: Res<'w, Relationships>,
    pub fox_scent_map: Res<'w, FoxScentMap>,
    /// Cat-presence influence map — sampled by `compute_ward_placement`
    /// to bias ward placement toward tiles where cats actually live.
    pub cat_presence_map: Res<'w, crate::resources::CatPresenceMap>,
    /// Hearing-channel kitten-cry broadcast (ticket 156). Sampled at
    /// each cat's position to populate `ScoringContext::kitten_cry_perceived`.
    pub kitten_cry_map: Res<'w, crate::resources::KittenCryMap>,
    /// Ward-coverage influence map — sampled by `compute_ward_placement`
    /// for anti-clustering (skip tiles already covered by other wards).
    pub ward_coverage_map: Res<'w, crate::resources::WardCoverageMap>,
    /// Mutable ledger of frustrated action desires — chain builders record
    /// misses here so the coordinator's BuildPressure can respond.
    pub unmet_demand: ResMut<'w, crate::resources::UnmetDemand>,
    /// §6.3 target-taking DSE lookup — chain builders route target
    /// selection through the registered DSE (Phase 4c.1 onward).
    pub dse_registry: Res<'w, crate::ai::eval::DseRegistry>,
    /// §9.1 base stance matrix — passed into target-taking resolvers
    /// for §9.3 candidate prefiltering. (Note: `evaluate_dispositions`
    /// is not registered in the schedule today; this resource is
    /// threaded for type-correctness, the no-op overlay closure
    /// below makes the prefilter a pass-through here.)
    pub faction_relations: Res<'w, crate::ai::faction::FactionRelations>,
    pub time: Res<'w, TimeState>,
}

use crate::resources::narrative_templates::{
    emit_event_narrative, MoodBucket, TemplateContext, VariableContext,
};
use crate::resources::relationships::Relationships;
use crate::resources::rng::SimRng;
use crate::resources::sim_constants::{DispositionConstants, SimConstants};
use crate::resources::system_activation::{Feature, SystemActivation};
use crate::resources::time::{DayPhase, Season, TimeScale, TimeState};

// ===========================================================================
// check_anxiety_interrupts
// ===========================================================================

/// Checks every tick whether a cat's disposition should be interrupted by
/// critical need states or threats. Runs BEFORE disposition evaluation.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn check_anxiety_interrupts(
    mut query: Query<
        (
            Entity,
            &Needs,
            &Personality,
            &Position,
            &Health,
            &mut CurrentAction,
            Option<&mut ActionHistory>,
        ),
        (With<Disposition>, Without<Dead>),
    >,
    dispositions: Query<&Disposition, Without<Dead>>,
    wildlife: Query<&Position, With<WildAnimal>>,
    time: Res<TimeState>,
    map: Res<TileMap>,
    constants: Res<SimConstants>,
    mut commands: Commands,
    mut activation: ResMut<SystemActivation>,
) {
    let d = &constants.disposition;
    for (entity, needs, personality, pos, health, mut current, history) in &mut query {
        let Ok(disposition) = dispositions.get(entity) else {
            continue;
        };

        let interrupt = check_interrupt(
            needs,
            personality,
            pos,
            health,
            disposition,
            &wildlife,
            d,
            &constants.sensory.cat,
        );
        let Some(reason) = interrupt else { continue };

        activation.record(Feature::AnxietyInterrupt);

        // Don't interrupt cats actively hunting (even if Resting disposition).
        if matches!(reason, InterruptReason::ThreatDetected { .. })
            && current.action == Action::Hunt
        {
            continue;
        }

        // Record the interruption in action history.
        if let Some(mut hist) = history {
            hist.record(ActionRecord {
                action: current.action,
                disposition: Some(disposition.kind),
                tick: time.tick,
                outcome: ActionOutcome::Interrupted,
            });
        }

        // Strip disposition and chain.
        commands.entity(entity).remove::<Disposition>();
        commands.entity(entity).remove::<TaskChain>();

        match reason {
            InterruptReason::ThreatDetected { threat_pos } => {
                // Immediate flee — don't wait for re-evaluation.
                let dx = pos.x - threat_pos.x;
                let dy = pos.y - threat_pos.y;
                let len = ((dx * dx + dy * dy) as f32).sqrt().max(1.0);
                let mut target = Position::new(
                    pos.x + (dx as f32 / len * d.flee_distance) as i32,
                    pos.y + (dy as f32 / len * d.flee_distance) as i32,
                );
                target.x = target.x.clamp(0, map.width - 1);
                target.y = target.y.clamp(0, map.height - 1);
                current.action = Action::Flee;
                current.ticks_remaining = 0;
                current.target_position = Some(target);
                current.target_entity = None;
            }
            _ => {
                // Let the cat re-evaluate next tick.
                current.ticks_remaining = 0;
            }
        }
    }
}

#[derive(Debug)]
enum InterruptReason {
    ThreatDetected { threat_pos: Position },
    CriticalSafety,
    CriticalHealth,
}

#[allow(clippy::too_many_arguments)]
fn check_interrupt(
    needs: &Needs,
    personality: &Personality,
    pos: &Position,
    health: &Health,
    disposition: &Disposition,
    wildlife: &Query<&Position, With<WildAnimal>>,
    d: &DispositionConstants,
    cat_profile: &crate::systems::sensing::SensoryProfile,
) -> Option<InterruptReason> {
    // Critical health check — fires for ALL dispositions, including Guarding.
    // A cat below the health threshold must re-evaluate immediately.
    if health.current / health.max < d.critical_health_threshold {
        return Some(InterruptReason::CriticalHealth);
    }

    // Tickets 106 + 107: the Starvation and Exhaustion arms used to live here,
    // each gated by a Resting/Hunting/Foraging exemption wrapper. Both arms
    // were vestigial in the post-091 regime — Phase 2 focal-trace soaks
    // confirmed `interrupts_by_reason.{Starvation, Exhaustion} == 0` even
    // under doubled need-decay, because the exemption wrapper structurally
    // shielded cats during the only times they reached the threshold.
    // Substrate replacements: `HungerUrgency` and `ExhaustionPressure`
    // modifiers in `src/ai/modifier.rs`. The GOAP urgency arms at
    // `goap.rs:615-637` are the actual food/sleep-routing drivers and
    // remain in place.

    // Guards are exempt from threat interrupts — they handle threats directly
    // via guard_threat_detection_range.
    if !matches!(disposition.kind, DispositionKind::Guarding) {
        // Check for nearby wildlife threats. Phase 2 migration: the
        // visual-only detection path now flows through the sensory
        // model's sight channel. See `cat_sees_threat_at`.
        let nearest_threat = wildlife
            .iter()
            .filter(|wp| crate::systems::sensing::cat_sees_threat_at(*pos, cat_profile, **wp))
            .min_by_key(|wp| pos.manhattan_distance(wp));

        if let Some(threat_pos) = nearest_threat {
            let dist = pos.manhattan_distance(threat_pos) as f32;
            let threat_urgency = 1.0 - (dist / d.threat_urgency_divisor);
            // Bold cats resist fleeing: threshold scales with boldness.
            let flee_threshold =
                d.flee_threshold_base + personality.boldness * d.flee_threshold_boldness_scale;
            if threat_urgency > flee_threshold {
                return Some(InterruptReason::ThreatDetected {
                    threat_pos: *threat_pos,
                });
            }
        }
    }

    // Critical safety check — guards are no longer exempt. A guard with
    // critically low safety should re-evaluate rather than standing in a
    // fight that's draining them.
    if needs.safety < d.critical_safety_threshold {
        return Some(InterruptReason::CriticalSafety);
    }

    None
}

// ===========================================================================
// evaluate_dispositions
// ===========================================================================

/// Bundle of side-effect params for evaluate_dispositions. Collapses commands,
/// rng, and the mating-eligibility snapshot into one SystemParam slot so the
/// outer function stays under Bevy's 16-param limit.
#[derive(bevy_ecs::system::SystemParam)]
pub struct EvalDispositionSideEffects<'w, 's> {
    pub rng: ResMut<'w, SimRng>,
    pub commands: Commands<'w, 's>,
    pub mating: crate::ai::mating::MatingFitnessParams<'w, 's>,
    pub dse_registry: Res<'w, crate::ai::eval::DseRegistry>,
    pub modifier_pipeline: Res<'w, crate::ai::eval::ModifierPipeline>,
    pub time: Res<'w, crate::resources::time::TimeState>,
    /// §11 focal-cat plumbing. Optional so non-traced runs pay nothing.
    pub focal_target: Option<Res<'w, crate::resources::FocalTraceTarget>>,
    pub focal_capture: Option<Res<'w, crate::resources::FocalScoreCapture>>,
    /// §4.3 marker queries for snapshot population. Future marker
    /// batches add their queries here.
    pub marker_queries: MarkerQueries<'w, 's>,
}

/// Read-only queries over stored-item state + kitten state. Bundled
/// into a SystemParam so the cat scoring systems
/// (evaluate_dispositions, evaluate_and_plan) can derive cooking
/// eligibility and Caretake urgency without blowing Bevy's 16-param
/// limit. The `kittens` query is bundled here (rather than in its
/// own SystemParam) purely as a 16-param workaround — the cooking
/// vs. kitten axes are thematically unrelated.
#[derive(bevy_ecs::system::SystemParam)]
pub struct CookingQueries<'w, 's> {
    pub stored_items: Query<'w, 's, &'static StoredItems>,
    pub items: Query<'w, 's, &'static Item>,
    pub kittens: Query<
        'w,
        's,
        (
            Entity,
            &'static Position,
            &'static Needs,
            &'static crate::components::KittenDependency,
        ),
        Without<Dead>,
    >,
}

impl CookingQueries<'_, '_> {
    /// True if any Stores building currently holds at least one uncooked
    /// food item.
    pub fn has_raw_food_in_stores(&self) -> bool {
        self.stored_items.iter().any(|stored| {
            stored.items.iter().copied().any(|e| {
                self.items
                    .get(e)
                    .is_ok_and(|it| it.kind.is_food() && !it.modifiers.cooked)
            })
        })
    }
}

/// For cats without a Disposition whose action has finished: score all actions,
/// aggregate to dispositions, select via softmax, insert Disposition component.
///
/// This replaces evaluate_actions for disposition-driven cats.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn evaluate_dispositions(
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
            ),
        ),
        (
            Without<Dead>,
            Without<Disposition>,
            // §Phase 5b — kittens are dependents, not planners. Mirror
            // the filter in `goap.rs::evaluate_and_plan` so the
            // disposition-chain path (if ever re-enabled) doesn't
            // regress the feed-kitten fix.
            Without<crate::components::KittenDependency>,
        ),
    >,
    all_positions: Query<(Entity, &Position, Option<&PreyAnimal>), Without<Dead>>,
    wildlife: Query<(Entity, &Position), With<WildAnimal>>,
    building_query: Query<(
        Entity,
        &Structure,
        &Position,
        Option<&ConstructionSite>,
        Option<&CropState>,
    )>,
    herb_query: Query<(Entity, &Herb, &Position), With<Harvestable>>,
    ward_query: Query<(&Ward, &Position)>,
    directive_queue_query: Query<(Entity, &DirectiveQueue)>,
    active_directive_query: Query<&ActiveDirective>,
    // Ticket 014 Mentoring batch — `skills_query` retired alongside the
    // `has_mentoring_target_fn` closure that was its only consumer.
    map: Res<TileMap>,
    food: Res<FoodStores>,
    relationships: Res<Relationships>,
    colony: super::ColonyContext,
    constants: Res<SimConstants>,
    cooking: CookingQueries,
    mut side_effects: EvalDispositionSideEffects,
    // Ticket 171 — read the four building-derived colony markers from the
    // `ColonyState` singleton. Bundles 169's deferred follow-on:
    // `evaluate_dispositions` no longer carries a `scan_colony_buildings`
    // call. Mirror of the `colony_state_query` slice in `goap.rs`.
    colony_state_query: Query<
        (
            Has<markers::HasFunctionalKitchen>,
            Has<markers::HasConstructionSite>,
            Has<markers::HasDamagedBuilding>,
            Has<markers::HasGarden>,
        ),
        With<markers::ColonyState>,
    >,
) {
    let rng = &mut *side_effects.rng;
    let commands = &mut side_effects.commands;
    let mating_fitness_params = &side_effects.mating;
    let sc = &constants.scoring;
    let d = &constants.disposition;
    let food_available = !food.is_empty();
    let food_fraction = food.fraction();

    // §4 marker snapshot (Phase 4b.2). Mirror of goap.rs — both scoring
    // paths must populate the same keys so the evaluator resolves
    // `EligibilityFilter::require` consistently across the two systems.
    let mut markers = crate::ai::scoring::MarkerSnapshot::new();
    markers.set_colony(markers::HasStoredFood::KEY, food_available);

    // Collect positions once. Ticket 014 §4 sensing batch retired the
    // per-cat `prey_positions` consumer (prey_nearby reads via marker
    // snapshot now); cat_positions stays for relationship-aware
    // resolvers later in the loop.
    let mut cat_positions: Vec<(Entity, Position)> = Vec::new();
    for (e, p, _prey) in all_positions.iter() {
        cat_positions.push((e, *p));
    }

    let wildlife_positions: Vec<(Entity, Position)> =
        wildlife.iter().map(|(e, p)| (e, *p)).collect();

    // §Phase 4c.3: snapshot kittens for Caretake urgency wiring.
    let kitten_snapshot: Vec<crate::ai::caretake_targeting::KittenState> = cooking
        .kittens
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

    // §4 colony-scoped marker predicates. Ticket 171 retired the
    // `scan_colony_buildings` call here; all four building-derived
    // markers now source from the `ColonyState` singleton (bundles
    // 169's deferred `disposition.rs` follow-on). Mirror of goap.rs.
    let (
        has_functional_kitchen,
        has_construction_site,
        has_damaged_building,
        has_garden,
    ) = colony_state_query
        .single()
        .expect("ColonyState singleton must exist (spawned by build_new_world / init_scenario_world_with)");
    markers.set_colony(markers::HasGarden::KEY, has_garden);
    markers.set_colony(markers::HasFunctionalKitchen::KEY, has_functional_kitchen);
    let has_raw_food_in_stores = cooking.has_raw_food_in_stores();
    markers.set_colony(markers::HasRawFoodInStores::KEY, has_raw_food_in_stores);

    // Ticket 014 §4 sensing batch — herb_positions retired here;
    // has_herbs_nearby is read via the HasHerbsNearby marker.
    // Ticket 014 Magic colony batch: shared helper + colony-scoped
    // marker. Retires the inline `herb_query.iter().any(...)` scan that
    // duplicated identical logic across this file and goap.rs.
    let thornbriar_available =
        crate::systems::magic::is_thornbriar_available(herb_query.iter().map(|(_, h, _)| h));
    markers.set_colony(markers::ThornbriarAvailable::KEY, thornbriar_available);

    let ward_strength_low = crate::systems::magic::is_ward_strength_low(
        ward_query.iter().map(|(w, _)| w),
        d.ward_strength_low_threshold,
    );
    markers.set_colony(markers::WardStrengthLow::KEY, ward_strength_low);
    // Ticket 014 Magic colony batch: `WardsUnderSiege` is left
    // unpopulated in the disposition path. The legacy
    // `evaluate_dispositions` doesn't carry a `wildlife_ai_query` system
    // param and this code path is no longer registered in the schedule
    // (substrate-refactor moved scoring into `goap::evaluate_and_plan`).
    // The previous behavior set `wards_under_siege: false` unconditionally
    // here; the missing snapshot entry preserves that — `markers.has`
    // returns false when the marker isn't set.

    let colony_injury_count = query
        .iter()
        .filter(|((_, _, _, _, _, _, _, health), _)| health.current < 1.0)
        .count();

    let directive_snapshot: HashMap<Entity, (usize, Option<Directive>)> = directive_queue_query
        .iter()
        .map(|(entity, q)| (entity, (q.directives.len(), q.directives.first().cloned())))
        .collect();

    // Snapshot per-cat fields needed by the mating eligibility gate.
    let current_day_phase = mating_fitness_params.current_day_phase();

    // Snapshot current actions for activity cascading.
    let action_snapshot: Vec<(Entity, Position, Action)> = query
        .iter()
        .map(
            |((entity, _, _, _, pos, _, _, _), (_, _, current, _, _, _, _, _))| {
                (entity, *pos, current.action)
            },
        )
        .collect();

    // Ticket 014 Mentoring batch — `has_mentoring_target_fn` closure
    // retired. The predicate now lives in
    // `aspirations::update_mentoring_target_markers`, the snapshot
    // population below routes the result through `MarkerSnapshot`, and
    // `MentorDse.eligibility()` requires `HasMentoringTarget::KEY`.

    for (
        (entity, _name, needs, personality, pos, memory, skills, health),
        (
            magic_aff,
            _inventory,
            mut current,
            aspirations,
            preferences,
            fated_love,
            fated_rival,
            fulfillment,
        ),
    ) in &mut query
    {
        if current.ticks_remaining != 0 {
            continue;
        }

        // §4 batch 2: can_hunt/can_forage retired — computed by
        // `update_capability_markers` and read from MarkerSnapshot below.

        // §6.5.6 target-taking DSE: replaces the Phase 4c.3 plain helper
        // with the four-axis bundle (nearness / kitten-hunger / kinship
        // Piecewise / isolation). `hungry_kitten_urgency` now reads the
        // aggregated Best score; `is_parent_of_hungry_kitten` stays
        // derived from any hungry own-kitten in range (bloodline override
        // fires regardless of argmax). Ticket 158 — parent_marker_active
        // promotes the closest hungry own-kitten as a fallback candidate
        // when the per-tick range gate excludes every in-range option.
        let parent_marker_active = side_effects
            .marker_queries
            .parent_hungry_kitten
            .get(entity)
            .unwrap_or(false);
        let caretake_resolution = crate::ai::dses::caretake_target::resolve_caretake_target(
            &side_effects.dse_registry,
            entity,
            *pos,
            &kitten_snapshot,
            &cat_positions,
            side_effects.time.tick,
            // Scorer pre-check; focal capture happens at the
            // step-resolution site, not here.
            None,
            parent_marker_active,
        );
        // §Phase 4c.4 alloparenting Reframe A: bond-weighted compassion.
        // Non-parent adults with a positive bond to the kitten's mother
        // get amplified compassion so colony raising actually fires.
        let caretake_bond_scale = crate::ai::caretake_targeting::caretake_compassion_bond_scale(
            entity,
            &caretake_resolution,
            sc.caretake_bond_compassion_boost_max,
            |a, b| relationships.get(a, b).map(|r| r.fondness),
        );

        // Ticket 014 §4 sensing batch — `has_social_target` /
        // `has_threat_nearby` now read from `MarkerSnapshot` after
        // `sensing::update_target_existence_markers` authors the ZSTs.
        // The inline `resolve_socialize_target` call here retires.
        // The disposition path is unregistered in the schedule today,
        // so the marker assignment below mirrors the live goap path
        // for forward-compat parity.

        // Allies-fighting still needs the nearest-threat position to
        // count co-fighting cats; use the same flat-range scan that
        // the author uses internally.
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
        // §4.3 per-cat marker population — mirror of goap.rs.
        markers.set_entity(markers::Incapacitated::KEY, entity, is_incapacitated);
        if let Ok((k, y, a, e)) = side_effects.marker_queries.life_stage.get(entity) {
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
        )) = side_effects.marker_queries.per_cat.get(entity)
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
        // OnSpecialTerrain. Authored by `combat::update_combat_marker`,
        // `magic::update_corrupted_tile_markers`, and
        // `sensing::update_terrain_markers` in Chain 2a.
        if let Ok((in_combat, on_corrupted_marker, on_special_marker)) =
            side_effects.marker_queries.state.get(entity)
        {
            markers.set_entity(markers::InCombat::KEY, entity, in_combat);
            markers.set_entity(markers::OnCorruptedTile::KEY, entity, on_corrupted_marker);
            markers.set_entity(markers::OnSpecialTerrain::KEY, entity, on_special_marker);
        }
        // Ticket 027 Bug 2 — HasEligibleMate authored by
        // `mating::update_mate_eligibility_markers`. Ticket 103 —
        // `has_pair_bond` is the second tuple element, used below for
        // the `escape_viability` dependent-presence term.
        let has_pair_bond = if let Ok((has_mate, has_pairing)) =
            side_effects.marker_queries.mate_eligibility.get(entity)
        {
            markers.set_entity(markers::HasEligibleMate::KEY, entity, has_mate);
            has_pairing
        } else {
            false
        };
        // Ticket 014 Mentoring batch — Mentor / Apprentice authored by
        // `aspirations::update_training_markers`; HasMentoringTarget by
        // `aspirations::update_mentoring_target_markers`.
        if let Ok((is_mentor, is_apprentice, has_mentoring_target)) =
            side_effects.marker_queries.mentoring.get(entity)
        {
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
            side_effects.marker_queries.target_existence.get(entity)
        {
            markers.set_entity(markers::HasThreatNearby::KEY, entity, threat);
            markers.set_entity(markers::HasSocialTarget::KEY, entity, social);
            markers.set_entity(markers::HasHerbsNearby::KEY, entity, herbs);
            markers.set_entity(markers::PreyNearby::KEY, entity, prey);
            markers.set_entity(markers::CarcassNearby::KEY, entity, carcass);
        }

        // Ticket 014 §4 sensing batch — `has_herbs_nearby` /
        // `prey_nearby` / `has_threat_nearby` / `has_social_target` /
        // `carcass_nearby` now read from `MarkerSnapshot` after
        // `sensing::update_target_existence_markers` authors the ZSTs.
        let has_herbs_nearby = markers.has(markers::HasHerbsNearby::KEY, entity);
        let prey_nearby = markers.has(markers::PreyNearby::KEY, entity);
        let has_threat_nearby = markers.has(markers::HasThreatNearby::KEY, entity);
        let has_social_target = markers.has(markers::HasSocialTarget::KEY, entity);
        let carcass_nearby = markers.has(markers::CarcassNearby::KEY, entity);

        let (on_corrupted_tile, tile_corruption, on_special_terrain) =
            if map.in_bounds(pos.x, pos.y) {
                let tile = map.get(pos.x, pos.y);
                (
                    tile.corruption > d.corrupted_tile_threshold,
                    tile.corruption,
                    matches!(tile.terrain, Terrain::FairyRing | Terrain::StandingStone),
                )
            } else {
                (false, 0.0, false)
            };

        // §L2.10.7: scan smell radius for the most-corrupted tile;
        // its position feeds the NearestCorruptedTile anchor (Cleanse,
        // DurableWard). Mirrors the goap.rs path that authors the
        // same anchor.
        let nearest_corrupted_tile: Option<crate::components::physical::Position> = {
            let r = sc.corruption_smell_range;
            let mut max_c: f32 = 0.0;
            let mut max_pos: Option<crate::components::physical::Position> = None;
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs() + dy.abs() > r {
                        continue;
                    }
                    let nx = pos.x + dx;
                    let ny = pos.y + dy;
                    if map.in_bounds(nx, ny) {
                        let c = map.get(nx, ny).corruption;
                        if c > max_c && c > d.corrupted_tile_threshold {
                            max_c = c;
                            max_pos = Some(crate::components::physical::Position::new(nx, ny));
                        }
                    }
                }
            }
            max_pos
        };

        // Ticket 027 Bug 2: inline `has_eligible_mate` retired —
        // `mating::update_mate_eligibility_markers` now authors the
        // `HasEligibleMate` ZST per tick, and `MateDse.eligibility()`
        // requires it. The marker is read via the snapshot below.

        let presence_memory_sums =
            crate::ai::scoring::memory_proximity_sums(memory, pos, sc);
        let presence_colony_knowledge_sums = colony
            .knowledge
            .as_ref()
            .map(|ck| crate::ai::scoring::colony_knowledge_proximity_sums(ck, pos, sc))
            .unwrap_or((0.0, 0.0));
        let presence_cascade_counts = crate::ai::scoring::compute_cascade_counts(
            &action_snapshot,
            entity,
            pos,
            d.cascading_bonus_range,
        );
        let presence_aspiration_action_counts = aspirations
            .map(crate::ai::scoring::compute_aspiration_action_counts)
            .unwrap_or([0.0; crate::ai::scoring::CASCADE_COUNTS_LEN]);
        let presence_preference_signals = preferences
            .map(crate::ai::scoring::compute_preference_signals)
            .unwrap_or([0.0; crate::ai::scoring::CASCADE_COUNTS_LEN]);
        let presence_love_visible = fated_love
            .filter(|l| l.awakened)
            .and_then(|l| cat_positions.iter().find(|(e, _)| *e == l.partner))
            .is_some_and(|(_, pp)| {
                crate::systems::sensing::observer_sees_at(
                    crate::components::SensorySpecies::Cat,
                    *pos,
                    &constants.sensory.cat,
                    *pp,
                    crate::components::SensorySignature::CAT,
                    d.fated_love_detection_range as f32,
                )
            });
        let presence_rival_nearby = fated_rival
            .filter(|r| r.awakened)
            .and_then(|r| cat_positions.iter().find(|(e, _)| *e == r.rival))
            .is_some_and(|(_, rp)| {
                crate::systems::sensing::observer_sees_at(
                    crate::components::SensorySpecies::Cat,
                    *pos,
                    &constants.sensory.cat,
                    *rp,
                    crate::components::SensorySignature::CAT,
                    d.fated_rival_detection_range as f32,
                )
            });
        let (presence_directive_action_ordinal, presence_directive_bonus) =
            if let Ok(directive) = active_directive_query.get(entity) {
                let fondness_factor = relationships
                    .get(entity, directive.coordinator)
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
            // Ticket 103 — threat-coupled escape viability. Dependent
            // presence is marker-only in v1: parent-of-living-kittens
            // (Parent ZST authored colony-wide by
            // `growth::update_parent_markers`) OR holds an active
            // pair-bond (`PairingActivity` component). Positional
            // refinement ("dependent within strike radius") parked as
            // ticket 128. `nearest_threat` above is `Option<&(Entity,
            // Position)>` from the wildlife scan; map to bare
            // `Option<Position>`.
            escape_viability: crate::systems::interoception::escape_viability(
                *pos,
                nearest_threat.map(|(_, p)| *p),
                &map,
                markers.has(markers::Parent::KEY, entity) || has_pair_bond,
                &constants.escape_viability,
            ),
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
            has_herbs_in_inventory: markers
                .has(crate::components::markers::HasHerbsInInventory::KEY, entity),
            has_remedy_herbs: markers.has(crate::components::markers::HasRemedyHerbs::KEY, entity),
            colony_injury_count,
            ward_strength_low,
            on_corrupted_tile,
            tile_corruption,
            nearby_corruption_level: 0.0, // legacy disposition path — not wired yet
            on_special_terrain,
            is_coordinator_with_directives: markers.has(
                crate::components::markers::IsCoordinatorWithDirectives::KEY,
                entity,
            ),
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
            kitten_cry_perceived: colony.kitten_cry_map.get(pos.x, pos.y),
            caretake_compassion_bond_scale: caretake_bond_scale,
            unexplored_nearby: colony.exploration_map.unexplored_fraction_nearby(
                pos.x,
                pos.y,
                d.explore_perception_radius,
                0.5,
            ),
            fox_scent_level: colony.fox_scent_map.get(pos.x, pos.y),
            // Ticket 014 §4 sensing batch — read via marker. Disposition
            // path doesn't carry a carcass-count snapshot; the marker
            // is the truthful source. `nearby_carcass_count` stays 0
            // here because the field is unused on this code path
            // (no DSE input reads it; goap.rs provides the count).
            carcass_nearby,
            nearby_carcass_count: 0,
            territory_max_corruption: 0.0,
            // Ticket 014 Magic colony batch — read via marker. Disposition
            // path doesn't populate WardsUnderSiege (no wildlife_ai_query
            // system param + path is unregistered), so this resolves to
            // false, matching pre-refactor behavior.
            wards_under_siege: markers.has(markers::WardsUnderSiege::KEY, entity),
            day_phase: current_day_phase,
            has_functional_kitchen,
            has_raw_food_in_stores,
            social_warmth_deficit: fulfillment.map_or(0.4, |f| f.social_warmth_deficit()),
            cat_anchors: crate::ai::scoring::CatAnchorPositions {
                nearest_corrupted_tile,
                nearest_construction_site:
                    crate::systems::buildings::nearest_construction_site(
                        building_query
                            .iter()
                            .map(|(_, s, p, site, _)| (s, p, site)),
                        *pos,
                    ),
                // §L2.10.7 Sleep anchor: cats sleep where they are
                // (no per-cat sleeping-spot component yet).
                own_sleeping_spot: Some(*pos),
                // §L2.10.7 Forage anchor: nearest forageable terrain.
                nearest_forageable_cluster: crate::ai::capabilities::nearest_matching_tile(
                    pos,
                    &map,
                    d.forage_terrain_search_radius,
                    |t| t.foraging_yield() > 0.0,
                ),
                // §L2.10.7 HerbcraftGather anchor: Manhattan-nearest
                // harvestable herb position.
                nearest_herb_patch: herb_query
                    .iter()
                    .map(|(_, _, p)| *p)
                    .min_by_key(|p| pos.manhattan_distance(p)),
                // §L2.10.7 Patrol / HerbcraftWard anchor — single
                // perimeter point offset from colony center.
                nearest_perimeter_tile: Some(crate::components::physical::Position::new(
                    colony.colony_center.0.x + d.patrol_perimeter_offset,
                    colony.colony_center.0.y,
                )),
                territory_perimeter_anchor: Some(crate::components::physical::Position::new(
                    colony.colony_center.0.x + d.patrol_perimeter_offset,
                    colony.colony_center.0.y,
                )),
                // §L2.10.7 Flee anchor: position of the nearest
                // wildlife threat already scanned for allies_fighting.
                nearest_threat: nearest_threat.map(|(_, p)| *p),
                // §L2.10.7 Coordinate anchor: colony center. The
                // coordinator's perch is approximated as the colony
                // origin tile — single-perch model. Future refinement
                // could store a per-coordinator perch component.
                coordinator_perch: Some(colony.colony_center.0),
                // Ticket 089 — interoceptive self-anchors.
                own_safe_rest_spot: crate::systems::interoception::own_safe_rest_spot(
                    memory,
                    d.safe_rest_threat_suppression_radius,
                ),
                own_injury_site: crate::systems::interoception::own_injury_site(health),
            },
            // No-damp signals: this path doesn't query
            // `RecentDispositionFailures`, so the modifier sees a
            // perpetual "no recent failure" signal here.
            disposition_failure_signal_hunting: 1.0,
            disposition_failure_signal_foraging: 1.0,
            disposition_failure_signal_crafting: 1.0,
            disposition_failure_signal_caretaking: 1.0,
            disposition_failure_signal_building: 1.0,
            disposition_failure_signal_mating: 1.0,
            disposition_failure_signal_mentoring: 1.0,
            memory_resource_found_proximity_sum: presence_memory_sums.0,
            memory_death_proximity_sum: presence_memory_sums.1,
            memory_threat_seen_proximity_sum: presence_memory_sums.2,
            colony_knowledge_resource_proximity: presence_colony_knowledge_sums.0,
            colony_knowledge_threat_proximity: presence_colony_knowledge_sums.1,
            colony_priority_ordinal: crate::ai::scoring::colony_priority_ordinal(
                colony.priority.as_ref().and_then(|cp| cp.active),
            ),
            cascade_counts: presence_cascade_counts,
            aspiration_action_counts: presence_aspiration_action_counts,
            preference_signals: presence_preference_signals,
            fated_love_visible: if presence_love_visible { 1.0 } else { 0.0 },
            fated_rival_nearby: if presence_rival_nearby { 1.0 } else { 0.0 },
            active_directive_action_ordinal: presence_directive_action_ordinal,
            active_directive_bonus: presence_directive_bonus,
        };

        // §11 trace plumbing — dormant except when running headless
        // with `--focal-cat`. `cat_presence_tick` runs on a different
        // cadence than `evaluate_and_plan`; both systems share the
        // FocalScoreCapture mutex, so captures from one pass don't
        // leak into another tick's replay frame.
        let focal_cat = side_effects.focal_target.as_deref().and_then(|t| t.entity);
        let focal_capture = side_effects.focal_capture.as_deref();
        let eval_inputs = crate::ai::scoring::EvalInputs {
            cat: entity,
            position: *pos,
            tick: side_effects.time.tick,
            dse_registry: &side_effects.dse_registry,
            modifier_pipeline: &side_effects.modifier_pipeline,
            markers: &markers,
            colony_landmarks: &colony.colony_landmarks,
            exploration_map: &colony.exploration_map,
            corruption_landmarks: &colony.corruption_landmarks,
            focal_cat,
            focal_capture,
        };
        let result = score_actions(&ctx, &eval_inputs, &mut rng.rng);
        let scores = result.scores;

        // 158: the side-channel `self_groom_won` resolver retired
        // here too (the duplicate of the `evaluate_and_plan` block —
        // a code-smell from the dual-system-call era). `Action::Groom`
        // split into sibling variants; the L3 softmax pick directly
        // routes to Resting (GroomSelf) or Grooming (GroomOther) via
        // `from_action`.

        // §L2.10.6 softmax-over-Intentions: flat-pool softmax in place of
        // the legacy `aggregate_to_dispositions → select_disposition_softmax`
        // path. Disposition-level independence penalty is ported to an
        // action-level transform inside the helper so behavior is preserved.
        //
        // §11.3 L3 capture — when `cat_presence_tick` is scoring the
        // focal cat, surface the softmax distribution + RNG roll for
        // replay. Mirror of the `evaluate_and_plan` capture block; both
        // systems share `FocalScoreCapture` via the interior mutex.
        let capture_this_cat = focal_capture.is_some() && focal_cat == Some(entity);
        let mut softmax_trace = capture_this_cat.then(crate::ai::scoring::SoftmaxCapture::default);
        let chosen = crate::ai::scoring::select_disposition_via_intention_softmax_with_trace(
            &scores,
            personality.independence,
            d.disposition_independence_penalty,
            sc,
            &mut rng.rng,
            softmax_trace.as_mut(),
        );
        if let (Some(capture), Some(trace)) = (focal_capture, softmax_trace) {
            capture.set_softmax(trace, side_effects.time.tick);
        }

        // Store all gate-open action scores for diagnostics (unchanged from
        // evaluate_actions). Truncation removed 2026-04-20 to match goap.rs.
        {
            let mut sorted = scores.clone();
            sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            current.last_scores = sorted;
        }

        // Insert the Disposition component. Chain creation happens in disposition_to_chain.
        // adopted_tick is 0 here; resolve_disposition_chains will set it from TimeState.
        let crafting_hint = if chosen == DispositionKind::Crafting {
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
            if cook_score > herbcraft_score && cook_score > magic_score {
                Some(CraftingHint::Cook)
            } else if magic_score > herbcraft_score {
                result.magic_hint.or(Some(CraftingHint::Magic))
            } else {
                result.herbcraft_hint
            }
        } else {
            None
        };
        let mut disp = Disposition::new(chosen, 0, personality);
        disp.crafting_hint = crafting_hint;
        // Ticket 072: route the disposition switch through
        // `plan_substrate::record_disposition_switch` so the new
        // `disposition_started_tick` field is consistently written
        // here (and at any future switch site). The pre-072 inline
        // body wrote `adopted_tick = 0` via `Disposition::new` and
        // had no `disposition_started_tick` to write — this call
        // writes `disposition_started_tick = 0` to match, preserving
        // the no-behavior-change invariant. 075 (`CommitmentTenure`)
        // is the first reader.
        crate::systems::plan_substrate::record_disposition_switch(&mut disp, chosen, 0);
        commands.entity(entity).insert(disp);

        // Keep ticks_remaining = 0 so disposition_to_chain picks it up this tick.
    }
}

// ===========================================================================
// disposition_to_chain
// ===========================================================================

/// For cats with a Disposition but no TaskChain: create the appropriate chain
/// based on disposition kind and current world state.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn disposition_to_chain(
    mut query: Query<
        (
            Entity,
            &Needs,
            &Personality,
            &Position,
            &Memory,
            &Skills,
            &MagicAffinity,
            &Inventory,
            &Health,
            &mut Disposition,
            &mut CurrentAction,
        ),
        (With<Disposition>, Without<Dead>, Without<TaskChain>),
    >,
    cat_positions: Query<(Entity, &Position, &Needs), Without<Dead>>,
    wildlife: Query<(Entity, &Position), With<WildAnimal>>,
    building_query: Query<(
        Entity,
        &Structure,
        &Position,
        Option<&ConstructionSite>,
        Option<&CropState>,
    )>,
    herb_query: Query<(Entity, &Herb, &Position), With<Harvestable>>,
    ward_query: Query<(&Ward, &Position)>,
    directive_queue_query: Query<(Entity, &DirectiveQueue)>,
    active_directive_query: Query<&ActiveDirective>,
    skills_query: Query<&Skills, Without<Dead>>,
    injured_cat_query: Query<(Entity, &Health, &Position), Without<Dead>>,
    kitten_query: Query<
        (
            Entity,
            &Position,
            &Needs,
            &crate::components::KittenDependency,
        ),
        Without<Dead>,
    >,
    res: ChainResources,
    constants: Res<SimConstants>,
    mut rng: ResMut<SimRng>,
    mut commands: Commands,
) {
    let d = &constants.disposition;
    // Pre-collect cat position pairs for social target selection.
    let cat_pos_list: Vec<(Entity, Position)> =
        cat_positions.iter().map(|(e, p, _)| (e, *p)).collect();
    // Snapshot per-cat `needs.temperature` for the §6.5.4 Groom-other
    // target-taking DSE. Keyed by entity so the resolver's closure
    // captures one HashMap<Entity, f32> rather than a whole query.
    let cat_temperature_map: std::collections::HashMap<Entity, f32> = cat_positions
        .iter()
        .map(|(e, _, n)| (e, n.temperature))
        .collect();
    // Snapshot kitten → (mother, father) parent pointers for the
    // §6.5.4 kinship axis. Bidirectional kinship is computed on the
    // fly by the resolver's `is_kin` closure.
    let kitten_parents_map: std::collections::HashMap<Entity, (Option<Entity>, Option<Entity>)> =
        kitten_query
            .iter()
            .map(|(e, _, _, dep)| (e, (dep.mother, dep.father)))
            .collect();

    // §Phase 4c.3: kitten snapshot for Caretake chain-building — the
    // winning kitten's Entity + Position thread into `build_caretaking_chain`
    // so the adult navigates TO the kitten after retrieving food.
    let kitten_snapshot: Vec<crate::ai::caretake_targeting::KittenState> = kitten_query
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

    // Anti-stacking: tiles that already have a cat on them.
    let occupied_tiles: std::collections::HashSet<Position> =
        cat_pos_list.iter().map(|(_, p)| *p).collect();

    let ward_strength_low = {
        let ward_count = ward_query.iter().count();
        if ward_count == 0 {
            true
        } else {
            let avg: f32 =
                ward_query.iter().map(|(w, _)| w.strength).sum::<f32>() / ward_count as f32;
            avg < d.ward_strength_low_threshold
        }
    };

    // Pre-collect injured cat positions for herbcraft targeting +
    // §6.5.7 ApplyRemedy DSE patient snapshots (health_fraction for
    // severity scoring). Both views read the same Health component
    // in one pass.
    let injured_patient_snapshot: Vec<crate::ai::dses::apply_remedy_target::PatientCandidate> =
        injured_cat_query
            .iter()
            .filter(|(_, h, _)| h.current < h.max)
            .map(
                |(e, h, p)| crate::ai::dses::apply_remedy_target::PatientCandidate {
                    entity: e,
                    position: *p,
                    health_fraction: if h.max > 0.0 {
                        (h.current / h.max).clamp(0.0, 1.0)
                    } else {
                        0.0
                    },
                },
            )
            .collect();
    let injured_cat_list: Vec<(Entity, Position)> = injured_cat_query
        .iter()
        .filter(|(_, h, _)| h.current < 1.0)
        .map(|(e, _, p)| (e, *p))
        .collect();

    for (
        entity,
        needs,
        personality,
        pos,
        memory,
        skills,
        magic_aff,
        inventory,
        health,
        disposition,
        mut current,
    ) in &mut query
    {
        // Check completion FIRST: if the disposition is already done, remove it.
        if should_complete_disposition(&disposition, needs, d) {
            commands.entity(entity).remove::<Disposition>();
            current.ticks_remaining = 0;
            continue;
        }

        // Pre-compute nearest Stores building for chains that need a return leg.
        let nearest_store = building_query
            .iter()
            .filter(|(_, s, _, site, _)| s.kind == StructureType::Stores && site.is_none())
            .min_by_key(|(_, _, bp, _, _)| pos.manhattan_distance(bp))
            .map(|(e, _, bp, _, _)| (e, *bp));

        // §6.5.6 target-taking DSE: argmax kitten flows into
        // `build_caretaking_chain`'s navigate-TO-kitten step. Same bundle
        // as evaluate_dispositions so scoring + chain-building see the
        // same winner. Ticket 158 — parent_marker_active is `false`
        // here because `disposition_to_chain` is unregistered in the
        // schedule today (the GOAP path owns chain-building); this site
        // mirrors `evaluate_dispositions` for forward-compat parity but
        // does not consume the kinship-channel substrate. If this path
        // is re-enabled, add a `parent_hungry_kitten` query param and
        // route the marker presence through here.
        let caretake_resolution = crate::ai::dses::caretake_target::resolve_caretake_target(
            &res.dse_registry,
            entity,
            *pos,
            &kitten_snapshot,
            &cat_pos_list,
            res.time.tick,
            // Chain-building side; the focal capture happens at the
            // GOAP step-resolver site (goap.rs: FeedKitten step).
            None,
            false,
        );

        // §6.5.1 + §6.5.2: resolve target-taking DSE winners once per
        // tick. Recomputed here (not threaded from
        // `evaluate_dispositions`) because the two systems run as a
        // pair per tick over the same frame-local state; the extra
        // evaluator passes are cheap relative to chain construction.
        // §9.3 prefilter inputs — `evaluate_dispositions` is not
        // registered in the schedule today (chain-building runs through
        // GOAP). The resolver requires the parameters for
        // type-correctness; the no-op overlay closure leaves the
        // prefilter as a pass-through here.
        let stance_overlays_noop = |_: Entity| crate::ai::faction::StanceOverlays::default();
        let socialize_target = crate::ai::dses::socialize_target::resolve_socialize_target(
            &res.dse_registry,
            entity,
            *pos,
            &cat_pos_list,
            &res.relationships,
            &res.faction_relations,
            &stance_overlays_noop,
            res.time.tick,
            // Chain-building side; the focal capture happens at the
            // GOAP step-resolver site (goap.rs: SocializeWith step).
            None,
            // Ticket 027b — `disposition_to_chain` is dead code
            // (`evaluate_dispositions` not scheduled today; chain-
            // building runs through GOAP). The L2 Intention lookup
            // lives in goap.rs's `resolve_goap_plans` SocializeWith
            // branch where the live wiring is. Pass `None` here
            // until/unless this path is revived.
            None,
            // Ticket 073 — same dead-code path; cooldown lookup happens
            // at the GOAP step-resolver site, not here.
            None,
            0,
            None,
        );
        let mate_target = crate::ai::dses::mate_target::resolve_mate_target(
            &res.dse_registry,
            entity,
            *pos,
            &cat_pos_list,
            &res.relationships,
            res.time.tick,
            // Chain-building side; the focal capture happens at the
            // GOAP step-resolver site (goap.rs: MateWith step).
            None,
            None,
            0,
            None,
        );
        // §6.5.3: resolve the mentor target-taking DSE. Skill-gap is the
        // dominant axis; weights renormalized from spec by dropping the
        // deferred `apprentice-receptivity` axis. Candidates share the
        // Socialize candidate pool (cats in range, excluding self); the
        // skill-lookup closure reads Skills from the frame-local query
        // so apprentice selection ranks on max-across-skills gap.
        let mentor_skills_lookup =
            |e: Entity| -> Option<Skills> { skills_query.get(e).ok().cloned() };
        let mentor_target = crate::ai::dses::mentor_target::resolve_mentor_target(
            &res.dse_registry,
            entity,
            *pos,
            &cat_pos_list,
            skills,
            &mentor_skills_lookup,
            &res.relationships,
            res.time.tick,
            // Chain-building side; the focal capture happens at the
            // GOAP step-resolver site (goap.rs: MentorWith step).
            None,
            None,
            0,
            None,
        );
        // §6.5.4: resolve the groom-other target-taking DSE. Adds
        // target-need-warmth + kinship axes that the legacy
        // `find_social_target` (fondness-only) could not see.
        let temperature_lookup =
            |e: Entity| -> Option<f32> { cat_temperature_map.get(&e).copied() };
        let is_kin = |a: Entity, b: Entity| -> bool {
            let a_parents = kitten_parents_map.get(&a);
            let b_parents = kitten_parents_map.get(&b);
            a_parents.is_some_and(|(m, f)| *m == Some(b) || *f == Some(b))
                || b_parents.is_some_and(|(m, f)| *m == Some(a) || *f == Some(a))
        };
        let groom_other_target = crate::ai::dses::groom_other_target::resolve_groom_other_target(
            &res.dse_registry,
            entity,
            *pos,
            &cat_pos_list,
            &temperature_lookup,
            &is_kin,
            &res.relationships,
            res.time.tick,
            // Chain-building side; the focal capture happens at
            // the GOAP step-resolver site (goap.rs: GroomOther step).
            None,
            None,
            0,
            None,
        );
        // §6.5.7: resolve patient for ApplyRemedy. Replaces the
        // `injured_cats.min_by_key(distance)` legacy pick with the
        // severity-weighted DSE ranking. The resolver returns None
        // if no injured cat is in range; the crafting chain falls
        // back to its legacy behavior in that case.
        let apply_remedy_target = crate::ai::dses::apply_remedy_target::resolve_apply_remedy_target(
            &res.dse_registry,
            entity,
            *pos,
            &injured_patient_snapshot,
            &is_kin,
            res.time.tick,
            // Chain-building side; apply_remedy has no dedicated
            // GOAP step-resolver (the crafting chain embeds it),
            // so the pre-check also carries None — focal-trace
            // coverage for apply_remedy is tracked in the
            // §6.5 multi-focal follow-on.
            None,
        );
        // §6.5.8: resolve work-site for Build. Replaces the
        // `(priority, distance)` legacy pick with the progress-
        // urgency + structural-condition weighted DSE.
        let build_candidates: Vec<crate::ai::dses::build_target::BuildCandidate> = building_query
            .iter()
            .filter_map(|(e, structure, bpos, site, _)| {
                if let Some(s) = site {
                    Some(crate::ai::dses::build_target::BuildCandidate {
                        entity: e,
                        position: *bpos,
                        kind: crate::ai::dses::build_target::BuildTargetKind::NewBuild,
                        progress: s.progress,
                        condition: 1.0,
                    })
                } else if structure.condition < 1.0 {
                    Some(crate::ai::dses::build_target::BuildCandidate {
                        entity: e,
                        position: *bpos,
                        kind: crate::ai::dses::build_target::BuildTargetKind::Repair,
                        progress: 0.0,
                        condition: structure.condition,
                    })
                } else {
                    None
                }
            })
            .collect();
        let build_target = crate::ai::dses::build_target::resolve_build_target(
            &res.dse_registry,
            entity,
            *pos,
            &build_candidates,
            res.time.tick,
            // Chain-building side; build has no dedicated GOAP
            // step-resolver picker (the construct-chain consumes the
            // winner directly), so focal-trace coverage for
            // build_target is tracked in the §6.5 multi-focal
            // follow-on.
            None,
        );

        let chain = match disposition.kind {
            DispositionKind::Resting => build_resting_chain(
                needs,
                pos,
                &building_query,
                &res.map,
                nearest_store,
                res.food.is_empty(),
                d,
                &mut rng.rng,
            ),
            // 150 R5a: legacy chain-building path (not registered in
            // any plugin since the GOAP migration). Eating's chain
            // mirrors the hunger arm of `build_resting_chain` —
            // Move→EatAtStores. The GOAP planner is the canonical
            // path; this arm exists so the dead-code path still type-
            // checks and historical tests don't break.
            DispositionKind::Eating => build_eating_chain(
                pos,
                &building_query,
                nearest_store,
                res.food.is_empty(),
            ),
            DispositionKind::Hunting => {
                build_hunting_chain(pos, memory, &res.map, nearest_store, &mut rng.rng)
            }
            DispositionKind::Foraging => {
                build_foraging_chain(pos, &res.map, nearest_store, &mut rng.rng)
            }
            DispositionKind::Guarding => build_guarding_chain(
                pos,
                health,
                &wildlife,
                &res.map,
                Some(&res.fox_scent_map),
                d,
                &constants.sensory.cat,
                &mut rng.rng,
            ),
            DispositionKind::Socializing => build_socializing_chain(
                socialize_target,
                mentor_target,
                groom_other_target,
                &cat_pos_list,
                personality,
                skills,
                &skills_query,
                d,
            ),
            DispositionKind::Building => {
                build_building_chain(entity, pos, &building_query, build_target, d, &mut commands)
            }
            DispositionKind::Farming => build_farming_chain(pos, &building_query),
            DispositionKind::Crafting => {
                let placement_maps = crate::systems::coordination::PlacementMaps {
                    fox_scent: &res.fox_scent_map,
                    cat_presence: &res.cat_presence_map,
                    ward_coverage: &res.ward_coverage_map,
                    tile_map: &res.map,
                };
                build_crafting_chain(
                    pos,
                    personality,
                    needs,
                    skills,
                    magic_aff,
                    inventory,
                    &herb_query,
                    &building_query,
                    &ward_query,
                    &cat_pos_list,
                    &injured_cat_list,
                    apply_remedy_target,
                    &res.map,
                    &placement_maps,
                    ward_strength_low,
                    d,
                    &mut rng.rng,
                    disposition.crafting_hint,
                )
            }
            DispositionKind::Coordinating => build_coordinating_chain(
                entity,
                pos,
                &directive_queue_query,
                &active_directive_query,
                &cat_pos_list,
                &skills_query,
                d,
                &mut commands,
            ),
            DispositionKind::Exploring => build_exploring_chain(pos, &res.map, d, &mut rng.rng),
            DispositionKind::Mating => build_mating_chain(mate_target, &cat_pos_list),
            DispositionKind::Caretaking => build_caretaking_chain(
                caretake_resolution.target,
                caretake_resolution.target_pos,
                nearest_store,
            ),
            // 154: legacy chain-building path mirrors Mating's single-
            // interaction shape. Dead-code arm (the GOAP planner is the
            // live path), retained for type-system completeness.
            DispositionKind::Mentoring => build_mentoring_chain(mentor_target, &cat_pos_list),
            // 158: legacy chain-building path mirrors Mentoring's
            // single-interaction shape. Dead-code arm (the GOAP planner
            // is the live path); the new `Grooming` disposition runs
            // its single-step `[GroomOther]` template through the
            // planner just like Mentoring's `[MentorCat]`.
            DispositionKind::Grooming => {
                build_grooming_chain(groom_other_target, &cat_pos_list)
            }
        };

        if let Some((mut chain, action)) = chain {
            // Anti-stacking: jitter MoveTo/PatrolTo destinations away from
            // tiles that already have a cat on them.
            if d.anti_stack_jitter {
                for step in &mut chain.steps {
                    if matches!(step.kind, StepKind::MoveTo | StepKind::PatrolTo) {
                        if let Some(ref mut target) = step.target_position {
                            if occupied_tiles.contains(target) {
                                if let Some(free) =
                                    find_free_adjacent(*target, *pos, &res.map, &occupied_tiles)
                                {
                                    *target = free;
                                }
                            }
                        }
                    }
                }
            }
            current.action = action;
            current.ticks_remaining = u64::MAX;
            current.target_position = chain.steps.first().and_then(|s| s.target_position);
            current.target_entity = chain.steps.first().and_then(|s| s.target_entity);
            commands.entity(entity).insert(chain);
        } else {
            // No valid chain could be built — remove disposition and idle.
            commands.entity(entity).remove::<Disposition>();
            current.action = Action::Idle;
            current.ticks_remaining = 0;
            current.target_position = None;
            current.target_entity = None;
        }
    }
}

/// Check whether a disposition's goal is met and should be cleared.
fn should_complete_disposition(
    disposition: &Disposition,
    needs: &Needs,
    d: &DispositionConstants,
) -> bool {
    match disposition.kind {
        // 150 R5a: legacy path mirrors the new GOAP completion arms.
        // Resting now gates on energy + temperature only; Eating gates
        // on hunger only.
        DispositionKind::Resting => {
            needs.energy >= d.resting_complete_energy
                && needs.temperature >= d.resting_complete_temperature
        }
        DispositionKind::Eating => needs.hunger >= d.resting_complete_hunger,
        _ => disposition.is_count_complete(),
    }
}

/// Respect earned on successful disposition completion.
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

/// Count witnesses (other cats) within Manhattan radius of `actor_pos`,
/// capped at `cap`. The actor itself is excluded.
///
/// Backs the §respect-restoration witness-multiplier: respect from
/// completing a task scales with social visibility. The canonical
/// live caller is `resolve_goap_plans`'s plan-completion block in
/// `src/systems/goap.rs` (the test-scheduled `resolve_disposition_chains`
/// below is not in production schedules; see
/// `docs/balance/respect-restoration.md` for the relocation).
pub fn count_witnesses_within_radius(
    actor_entity: Entity,
    actor_pos: &Position,
    positions: &[(Entity, Position)],
    radius: i32,
    cap: u32,
) -> u32 {
    let mut count: u32 = 0;
    for (e, p) in positions {
        if *e == actor_entity {
            continue;
        }
        if actor_pos.manhattan_distance(p) <= radius {
            count += 1;
            if count >= cap {
                return cap;
            }
        }
    }
    count
}

#[cfg(test)]
mod respect_witness_tests {
    use super::*;

    fn pos(x: i32, y: i32) -> Position {
        Position::new(x, y)
    }

    #[test]
    fn empty_positions_zero_witnesses() {
        let mut world = bevy_ecs::world::World::new();
        let actor = world.spawn_empty().id();
        assert_eq!(
            count_witnesses_within_radius(actor, &pos(5, 5), &[], 5, 4),
            0
        );
    }

    #[test]
    fn excludes_self() {
        let mut world = bevy_ecs::world::World::new();
        let actor = world.spawn_empty().id();
        let positions = vec![(actor, pos(5, 5))];
        assert_eq!(
            count_witnesses_within_radius(actor, &pos(5, 5), &positions, 5, 4),
            0
        );
    }

    #[test]
    fn counts_in_radius_excludes_out_of_radius() {
        let mut world = bevy_ecs::world::World::new();
        let actor = world.spawn_empty().id();
        let near1 = world.spawn_empty().id();
        let near2 = world.spawn_empty().id();
        let far = world.spawn_empty().id();
        let positions = vec![
            (near1, pos(7, 5)), // distance 2
            (near2, pos(5, 9)), // distance 4
            (far, pos(20, 20)), // distance 30
        ];
        assert_eq!(
            count_witnesses_within_radius(actor, &pos(5, 5), &positions, 5, 4),
            2
        );
    }

    #[test]
    fn cap_applies() {
        let mut world = bevy_ecs::world::World::new();
        let actor = world.spawn_empty().id();
        let positions: Vec<(Entity, Position)> = (0..10)
            .map(|i| (world.spawn_empty().id(), pos(5 + i % 3, 5)))
            .collect();
        // All 10 are within radius 5, but cap=4 must apply.
        assert_eq!(
            count_witnesses_within_radius(actor, &pos(5, 5), &positions, 5, 4),
            4
        );
    }
}

// ---------------------------------------------------------------------------
// Chain builders — one per disposition
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn build_resting_chain(
    needs: &Needs,
    pos: &Position,
    building_query: &Query<(
        Entity,
        &Structure,
        &Position,
        Option<&ConstructionSite>,
        Option<&CropState>,
    )>,
    _map: &TileMap,
    nearest_store: Option<(Entity, Position)>,
    food_empty: bool,
    d: &DispositionConstants,
    rng: &mut impl Rng,
) -> Option<(TaskChain, Action)> {
    // Pick the most urgent physiological need.
    let hunger_deficit = 1.0 - needs.hunger;
    let energy_deficit = 1.0 - needs.energy;
    let temperature_deficit = 1.0 - needs.temperature;

    if hunger_deficit >= energy_deficit && hunger_deficit >= temperature_deficit {
        if food_empty {
            // Stores are empty — hunt for food instead of walking to empty stores.
            let patrol_dir = (rng.random_range(-1i32..=1), rng.random_range(-1i32..=1));
            let mut steps = vec![TaskStep::new(StepKind::HuntPrey { patrol_dir })];
            if let Some((store_entity, store_pos)) = nearest_store {
                steps.push(TaskStep::new(StepKind::MoveTo).with_position(store_pos));
                steps.push(
                    TaskStep::new(StepKind::DepositAtStores)
                        .with_position(store_pos)
                        .with_entity(store_entity),
                );
            }
            let chain = TaskChain::new(steps, FailurePolicy::AbortChain);
            return Some((chain, Action::Hunt));
        }

        // Eat: walk to nearest Stores building.
        let nearest_store = building_query
            .iter()
            .filter(|(_, s, _, site, _)| s.kind == StructureType::Stores && site.is_none())
            .min_by_key(|(_, _, bp, _, _)| pos.manhattan_distance(bp))
            .map(|(e, _, bp, _, _)| (e, *bp));

        if let Some((store_entity, store_pos)) = nearest_store {
            let chain = TaskChain::new(
                vec![
                    TaskStep::new(StepKind::MoveTo).with_position(store_pos),
                    TaskStep::new(StepKind::EatAtStores)
                        .with_position(store_pos)
                        .with_entity(store_entity),
                ],
                FailurePolicy::AbortChain,
            );
            Some((chain, Action::Eat))
        } else {
            // No stores — just idle.
            None
        }
    } else if energy_deficit >= temperature_deficit {
        // Sleep in place.
        let sleep_ticks = ((1.0 - needs.energy) * d.sleep_duration_deficit_multiplier) as u64
            + d.sleep_duration_base;
        let chain = TaskChain::new(
            vec![TaskStep::new(StepKind::Sleep { ticks: sleep_ticks }).with_position(*pos)],
            FailurePolicy::AbortChain,
        );
        Some((chain, Action::Sleep))
    } else {
        // Self-groom.
        let chain = TaskChain::new(
            vec![TaskStep::new(StepKind::SelfGroom).with_position(*pos)],
            FailurePolicy::AbortChain,
        );
        Some((chain, Action::GroomSelf))
    }
}

/// 150 R5a: Eating's chain — `MoveTo(Stores) + EatAtStores`. The
/// canonical execution path is the GOAP planner (`src/systems/goap.rs`);
/// this helper exists so `evaluate_dispositions` (legacy, not plugin-
/// registered) still type-checks against the new DispositionKind variant.
#[allow(clippy::type_complexity)]
fn build_eating_chain(
    pos: &Position,
    building_query: &Query<(
        Entity,
        &Structure,
        &Position,
        Option<&ConstructionSite>,
        Option<&CropState>,
    )>,
    nearest_store: Option<(Entity, Position)>,
    food_empty: bool,
) -> Option<(TaskChain, Action)> {
    if food_empty {
        return None;
    }
    let store = nearest_store.or_else(|| {
        building_query
            .iter()
            .filter(|(_, s, _, site, _)| s.kind == StructureType::Stores && site.is_none())
            .min_by_key(|(_, _, bp, _, _)| pos.manhattan_distance(bp))
            .map(|(e, _, bp, _, _)| (e, *bp))
    });
    let (store_entity, store_pos) = store?;
    let chain = TaskChain::new(
        vec![
            TaskStep::new(StepKind::MoveTo).with_position(store_pos),
            TaskStep::new(StepKind::EatAtStores)
                .with_position(store_pos)
                .with_entity(store_entity),
        ],
        FailurePolicy::AbortChain,
    );
    Some((chain, Action::Eat))
}

fn build_hunting_chain(
    _pos: &Position,
    _memory: &Memory,
    _map: &TileMap,
    nearest_store: Option<(Entity, Position)>,
    rng: &mut impl Rng,
) -> Option<(TaskChain, Action)> {
    // HuntPrey handles all movement internally (scent search → stalk → pounce).
    // No MoveTo preamble — the cat starts hunting from its current position.
    let patrol_dir = (rng.random_range(-1i32..=1), rng.random_range(-1i32..=1));
    let mut steps = vec![TaskStep::new(StepKind::HuntPrey { patrol_dir })];
    if let Some((store_entity, store_pos)) = nearest_store {
        steps.push(TaskStep::new(StepKind::MoveTo).with_position(store_pos));
        steps.push(
            TaskStep::new(StepKind::DepositAtStores)
                .with_position(store_pos)
                .with_entity(store_entity),
        );
    }
    let chain = TaskChain::new(steps, FailurePolicy::AbortChain);
    Some((chain, Action::Hunt))
}

fn build_foraging_chain(
    _pos: &Position,
    _map: &TileMap,
    nearest_store: Option<(Entity, Position)>,
    rng: &mut impl Rng,
) -> Option<(TaskChain, Action)> {
    // ForageItem handles all movement internally (directional patrol + tile checks).
    let patrol_dir = (rng.random_range(-1i32..=1), rng.random_range(-1i32..=1));
    let mut steps = vec![TaskStep::new(StepKind::ForageItem { patrol_dir })];
    if let Some((store_entity, store_pos)) = nearest_store {
        steps.push(TaskStep::new(StepKind::MoveTo).with_position(store_pos));
        steps.push(
            TaskStep::new(StepKind::DepositAtStores)
                .with_position(store_pos)
                .with_entity(store_entity),
        );
    }
    let chain = TaskChain::new(steps, FailurePolicy::AbortChain);
    Some((chain, Action::Forage))
}

#[allow(clippy::too_many_arguments)]
fn build_guarding_chain(
    pos: &Position,
    health: &Health,
    wildlife: &Query<(Entity, &Position), With<WildAnimal>>,
    map: &TileMap,
    fox_scent: Option<&FoxScentMap>,
    d: &DispositionConstants,
    cat_profile: &crate::systems::sensing::SensoryProfile,
    rng: &mut impl Rng,
) -> Option<(TaskChain, Action)> {
    // If threat nearby, fight it. Phase 4 migration: visual channel
    // via the unified sensory model.
    let nearest_threat = wildlife
        .iter()
        .filter(|(_, wp)| {
            crate::systems::sensing::observer_sees_at(
                crate::components::SensorySpecies::Cat,
                *pos,
                cat_profile,
                **wp,
                crate::components::SensorySignature::WILDLIFE,
                d.guard_threat_detection_range as f32,
            )
        })
        .min_by_key(|(_, wp)| pos.manhattan_distance(wp));

    if let Some((threat_entity, threat_pos)) = nearest_threat {
        // If the threat is in high fox-scent territory (boundary encounter),
        // patrol toward and survey instead of full fight — let the fox's own
        // avoidance/standoff logic handle the interaction.
        let scent_at_threat = fox_scent.map_or(0.0, |fs| fs.get(threat_pos.x, threat_pos.y));
        if scent_at_threat > 0.3 {
            let chain = TaskChain::new(
                vec![
                    TaskStep::new(StepKind::PatrolTo).with_position(*threat_pos),
                    TaskStep::new(StepKind::Survey).with_position(*threat_pos),
                ],
                FailurePolicy::AbortChain,
            );
            return Some((chain, Action::Patrol));
        }

        // Health gate: cats below guard_fight_health_min patrol+survey
        // instead of engaging directly. Prevents wounded guards from
        // walking into fights they can't survive.
        let hp_ratio = health.current / health.max;
        if hp_ratio < d.guard_fight_health_min {
            let chain = TaskChain::new(
                vec![
                    TaskStep::new(StepKind::PatrolTo).with_position(*threat_pos),
                    TaskStep::new(StepKind::Survey).with_position(*threat_pos),
                ],
                FailurePolicy::AbortChain,
            );
            return Some((chain, Action::Patrol));
        }

        let chain = TaskChain::new(
            vec![
                TaskStep::new(StepKind::MoveTo).with_position(*threat_pos),
                TaskStep::new(StepKind::FightThreat)
                    .with_position(*threat_pos)
                    .with_entity(threat_entity),
            ],
            FailurePolicy::AbortChain,
        );
        return Some((chain, Action::Fight));
    }

    // Patrol toward the nearest fox scent boundary if detectable.
    let patrol_radius = d.guard_patrol_radius;
    let scent_target =
        fox_scent.and_then(|fs| fs.highest_nearby(pos.x, pos.y, patrol_radius as i32));
    let target = if let Some((sx, sy)) = scent_target {
        // Patrol toward the fox scent hotspot.
        Position::new(sx.clamp(0, map.width - 1), sy.clamp(0, map.height - 1))
    } else {
        // Fallback: random perimeter patrol.
        let center_x = map.width / 2;
        let center_y = map.height / 2;
        let angle: f32 = rng.random_range(0.0..std::f32::consts::TAU);
        let mut t = Position::new(
            center_x + (angle.cos() * patrol_radius) as i32,
            center_y + (angle.sin() * patrol_radius) as i32,
        );
        t.x = t.x.clamp(0, map.width - 1);
        t.y = t.y.clamp(0, map.height - 1);
        t
    };

    let chain = TaskChain::new(
        vec![
            TaskStep::new(StepKind::PatrolTo).with_position(target),
            TaskStep::new(StepKind::Survey).with_position(target),
        ],
        FailurePolicy::AbortChain,
    );
    Some((chain, Action::Patrol))
}

/// Build a task chain around a pre-resolved social target. §6.2 /
/// §6.5.1 / §6.5.3: `socialize_target` picks the Socialize / Groom
/// partner via [`crate::ai::dses::socialize_target::resolve_socialize_target`];
/// `mentor_target` picks the apprentice via
/// [`crate::ai::dses::mentor_target::resolve_mentor_target`]. When
/// the `can_mentor` branch fires, `mentor_target` is preferred so
/// apprentice selection ranks on skill-gap (§6.1 Critical fix); the
/// Socialize target is the fallback if no mentor candidate exists.
/// This function owns only the chain shape (sub-action pick + step
/// sequencing), not partner selection.
#[allow(clippy::too_many_arguments)]
fn build_socializing_chain(
    socialize_target: Option<Entity>,
    mentor_target: Option<Entity>,
    groom_other_target: Option<Entity>,
    cat_positions: &[(Entity, Position)],
    personality: &Personality,
    skills: &Skills,
    skills_query: &Query<&Skills, Without<Dead>>,
    d: &DispositionConstants,
) -> Option<(TaskChain, Action)> {
    // Decide sub-action: mentor if a skill-gap-picked apprentice exists
    // and the paired threshold check passes + warmth permits; groom if
    // warm; otherwise socialize. The paired threshold check preserves
    // the pre-refactor selectivity (same axis where self > high && other
    // < low); the mentor-target picker chooses *which* qualifying
    // apprentice by skill-gap magnitude rather than legacy fondness.
    let mentor_skills = [
        skills.hunting,
        skills.foraging,
        skills.herbcraft,
        skills.building,
        skills.combat,
        skills.magic,
    ];
    let self_has_mentor_level_skill = mentor_skills
        .iter()
        .any(|&s| s > d.mentor_skill_threshold_high);
    let can_mentor = self_has_mentor_level_skill
        && mentor_target.is_some_and(|mt| {
            skills_query.get(mt).is_ok_and(|other| {
                let other_arr = [
                    other.hunting,
                    other.foraging,
                    other.herbcraft,
                    other.building,
                    other.combat,
                    other.magic,
                ];
                mentor_skills.iter().zip(other_arr.iter()).any(|(&m, &a)| {
                    m > d.mentor_skill_threshold_high && a < d.mentor_skill_threshold_low
                })
            })
        });

    // §6.5.4: Groom branch prefers the warmth-/kinship-/adjacency-ranked
    // `groom_other_target` when available, falling through to
    // `socialize_target` if the groom-target DSE produced no winner
    // (e.g. all cats out of the near-step range). The fallback preserves
    // liveness — a cat who wants to groom still takes *some* partner
    // rather than stalling — while the preferred pick gives the caller
    // the §6.1-Critical warmth-responsive target.
    let (target_entity, step_kind, action) =
        if can_mentor && personality.warmth > d.mentor_temperature_threshold {
            (
                mentor_target.expect("can_mentor implies mentor_target.is_some()"),
                StepKind::MentorCat,
                Action::Mentor,
            )
        } else if personality.warmth > d.groom_temperature_threshold {
            let target = groom_other_target.or(socialize_target)?;
            (target, StepKind::GroomOther, Action::GroomOther)
        } else {
            (socialize_target?, StepKind::Socialize, Action::Socialize)
        };

    let target_pos = *cat_positions
        .iter()
        .find(|(e, _)| *e == target_entity)
        .map(|(_, p)| p)?;

    let chain = TaskChain::new(
        vec![
            TaskStep::new(StepKind::MoveTo).with_position(target_pos),
            TaskStep::new(step_kind)
                .with_position(target_pos)
                .with_entity(target_entity),
        ],
        FailurePolicy::AbortChain,
    );
    Some((chain, action))
}

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
fn build_building_chain(
    _entity: Entity,
    pos: &Position,
    building_query: &Query<(
        Entity,
        &Structure,
        &Position,
        Option<&ConstructionSite>,
        Option<&CropState>,
    )>,
    build_target: Option<Entity>,
    d: &DispositionConstants,
    _commands: &mut Commands,
) -> Option<(TaskChain, Action)> {
    // §6.5.8: prefer the build-target DSE's pick (progress-urgency +
    // structural-condition aware). Falls back to the legacy
    // `(priority, distance)` pick when the DSE returned None (e.g.
    // no candidate in range at evaluation time but the chain-builder
    // re-checks; or the DSE registry missing in tests).
    let dse_target = build_target.and_then(|e| building_query.get(e).ok());
    let legacy_target = building_query
        .iter()
        .filter(|(_, _, bpos, site, _)| {
            site.is_some() || pos.manhattan_distance(bpos) <= d.building_search_range
        })
        .min_by_key(|(_, _s, bpos, site, _)| {
            let priority = if site.is_some() { 0 } else { 1 };
            let dist = pos.manhattan_distance(bpos);
            (priority, dist)
        });
    let target = dse_target.or(legacy_target);

    let (target_entity, _structure, bpos, site, _) = target?;

    let chain = if site.is_some() {
        TaskChain::new(
            vec![
                TaskStep::new(StepKind::MoveTo).with_position(*bpos),
                TaskStep::new(StepKind::Construct)
                    .with_position(*bpos)
                    .with_entity(target_entity),
            ],
            FailurePolicy::AbortChain,
        )
    } else {
        StructureType::repair_chain(*bpos, target_entity)
    };

    Some((chain, Action::Build))
}

#[allow(clippy::type_complexity)]
fn build_farming_chain(
    pos: &Position,
    building_query: &Query<(
        Entity,
        &Structure,
        &Position,
        Option<&ConstructionSite>,
        Option<&CropState>,
    )>,
) -> Option<(TaskChain, Action)> {
    let garden = building_query
        .iter()
        .filter(|(_, s, _, site, _)| s.kind == StructureType::Garden && site.is_none())
        .min_by_key(|(_, _, bpos, _, _)| pos.manhattan_distance(bpos));

    let (garden_entity, _, garden_pos, _, _) = garden?;
    let chain = StructureType::farm_chain(*garden_pos, garden_entity);
    Some((chain, Action::Farm))
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn build_crafting_chain(
    pos: &Position,
    _personality: &Personality,
    _needs: &Needs,
    skills: &Skills,
    magic_aff: &MagicAffinity,
    inventory: &Inventory,
    herb_query: &Query<(Entity, &Herb, &Position), With<Harvestable>>,
    building_query: &Query<(
        Entity,
        &Structure,
        &Position,
        Option<&ConstructionSite>,
        Option<&CropState>,
    )>,
    ward_query: &Query<(&Ward, &Position)>,
    _cat_positions: &[(Entity, Position)],
    injured_cats: &[(Entity, Position)],
    apply_remedy_target: Option<Entity>,
    map: &TileMap,
    placement_maps: &crate::systems::coordination::PlacementMaps<'_>,
    ward_strength_low: bool,
    d: &DispositionConstants,
    rng: &mut impl Rng,
    hint: Option<CraftingHint>,
) -> Option<(TaskChain, Action)> {
    // Pre-compute ward placement position for SetWard chains.
    let ward_placement_pos = if ward_strength_low {
        let building_positions: Vec<Position> = building_query
            .iter()
            .filter(|(_, _, _, site, _)| site.is_none())
            .map(|(_, _, bpos, _, _)| *bpos)
            .collect();
        let ward_data: Vec<(Position, f32)> = ward_query
            .iter()
            .filter(|(w, _)| !w.inverted && w.strength > 0.01)
            .map(|(w, p)| (*p, w.repel_radius()))
            .collect();
        let center = Position::new(map.width / 2, map.height / 2);
        Some(crate::systems::coordination::compute_ward_placement(
            &building_positions,
            &ward_data,
            center,
            placement_maps,
            rng,
        ))
    } else {
        None
    };

    // Try hinted mode first.
    if let Some(h) = hint {
        if let Some(chain) = try_crafting_sub_mode(
            h,
            pos,
            skills,
            magic_aff,
            inventory,
            herb_query,
            building_query,
            injured_cats,
            apply_remedy_target,
            map,
            ward_strength_low,
            ward_placement_pos,
            d,
            rng,
        ) {
            return Some(chain);
        }
    }

    // Fallback: cascade through remaining modes, skipping the hint.
    // (Cook intentionally absent — it's resolved by the GOAP planner, not
    // this task-chain path.)
    for mode in [
        CraftingHint::GatherHerbs,
        CraftingHint::PrepareRemedy,
        CraftingHint::SetWard,
        CraftingHint::DurableWard,
        CraftingHint::Magic,
    ] {
        if Some(mode) == hint {
            continue;
        }
        if let Some(chain) = try_crafting_sub_mode(
            mode,
            pos,
            skills,
            magic_aff,
            inventory,
            herb_query,
            building_query,
            injured_cats,
            apply_remedy_target,
            map,
            ward_strength_low,
            ward_placement_pos,
            d,
            rng,
        ) {
            return Some(chain);
        }
    }

    None
}

/// Try to build a crafting chain for a specific sub-mode.
/// Returns `None` if the preconditions for that mode aren't met.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn try_crafting_sub_mode(
    mode: CraftingHint,
    pos: &Position,
    skills: &Skills,
    magic_aff: &MagicAffinity,
    inventory: &Inventory,
    herb_query: &Query<(Entity, &Herb, &Position), With<Harvestable>>,
    building_query: &Query<(
        Entity,
        &Structure,
        &Position,
        Option<&ConstructionSite>,
        Option<&CropState>,
    )>,
    injured_cats: &[(Entity, Position)],
    apply_remedy_target: Option<Entity>,
    map: &TileMap,
    ward_strength_low: bool,
    ward_placement_pos: Option<Position>,
    d: &DispositionConstants,
    _rng: &mut impl Rng,
) -> Option<(TaskChain, Action)> {
    use crate::components::magic::{HerbKind, RemedyKind, WardKind};

    match mode {
        CraftingHint::GatherHerbs => {
            let has_herbs_nearby = herb_query
                .iter()
                .any(|(_, _, hp)| pos.manhattan_distance(hp) <= d.crafting_herb_detection_range);

            if !has_herbs_nearby || skills.herbcraft <= d.crafting_herbcraft_skill_threshold {
                return None;
            }

            let nearest_herb = herb_query
                .iter()
                .filter(|(_, _, hp)| pos.manhattan_distance(hp) <= d.crafting_herb_detection_range)
                .min_by_key(|(_, _, hp)| pos.manhattan_distance(hp));

            let (herb_entity, _, herb_pos) = nearest_herb?;
            let chain = TaskChain::new(
                vec![
                    TaskStep::new(StepKind::MoveTo).with_position(*herb_pos),
                    TaskStep::new(StepKind::GatherHerb)
                        .with_position(*herb_pos)
                        .with_entity(herb_entity),
                ],
                FailurePolicy::AbortChain,
            );
            Some((chain, Action::Herbcraft))
        }

        CraftingHint::PrepareRemedy => {
            if !inventory.has_remedy_herb() {
                return None;
            }

            let remedy_kind = if inventory.has_herb(HerbKind::HealingMoss) {
                RemedyKind::HealingPoultice
            } else if inventory.has_herb(HerbKind::Moonpetal) {
                RemedyKind::EnergyTonic
            } else {
                RemedyKind::MoodTonic
            };

            let workshop_pos = building_query
                .iter()
                .filter(|(_, s, _, site, _)| s.kind == StructureType::Workshop && site.is_none())
                .map(|(_, _, bpos, _, _)| *bpos)
                .min_by_key(|bpos| pos.manhattan_distance(bpos));

            let mut steps = Vec::new();
            if let Some(wp) = workshop_pos {
                steps.push(TaskStep::new(StepKind::MoveTo).with_position(wp));
            }
            steps.push(TaskStep::new(StepKind::PrepareRemedy {
                remedy: remedy_kind,
            }));

            // After preparing, deliver the remedy. §6.5.7: prefer the
            // severity-weighted DSE-picked patient when available; fall
            // back to nearest-injured pick if the DSE returned None
            // (e.g. no injured cat in range). Wrong-direction
            // regressions would show up in continuity canaries —
            // RemedyApplied hasn't been firing in recent soaks, so the
            // fallback preserves liveness while the DSE takes over the
            // selection axis.
            let patient = apply_remedy_target
                .and_then(|e| injured_cats.iter().find(|(ie, _)| *ie == e))
                .or_else(|| {
                    injured_cats
                        .iter()
                        .min_by_key(|(_, ip)| pos.manhattan_distance(ip))
                });
            if let Some((patient_entity, patient_pos)) = patient {
                steps.push(
                    TaskStep::new(StepKind::ApplyRemedy {
                        remedy: remedy_kind,
                    })
                    .with_position(*patient_pos)
                    .with_entity(*patient_entity),
                );
            }

            let chain = TaskChain::new(steps, FailurePolicy::AbortChain);
            Some((chain, Action::Herbcraft))
        }

        CraftingHint::SetWard | CraftingHint::DurableWard => {
            let is_durable = matches!(mode, CraftingHint::DurableWard);
            // Thornwards need herbs; durable wards skip the herb check.
            if (!is_durable && !inventory.has_ward_herb()) || !ward_strength_low {
                return None;
            }
            if is_durable
                && (magic_aff.0 <= d.crafting_magic_affinity_threshold
                    || skills.magic <= d.crafting_magic_skill_threshold)
            {
                return None;
            }

            // Fallback to map-center anchor on the rare path where
            // `ward_strength_low` was false at computation time but the
            // cat still committed to ward (e.g. directive-driven warding
            // ahead of the colony-side gate). Influence-map scoring is
            // skipped in that branch; map center is a safe-enough drop.
            let ward_pos = ward_placement_pos
                .unwrap_or_else(|| Position::new(map.width / 2, map.height / 2));

            let kind = if is_durable {
                WardKind::DurableWard
            } else {
                WardKind::Thornward
            };
            let chain = TaskChain::new(
                vec![
                    TaskStep::new(StepKind::MoveTo).with_position(ward_pos),
                    TaskStep::new(StepKind::SetWard { kind }).with_position(ward_pos),
                ],
                FailurePolicy::AbortChain,
            );
            let action = if is_durable {
                Action::PracticeMagic
            } else {
                Action::Herbcraft
            };
            Some((chain, action))
        }

        CraftingHint::Magic => {
            if magic_aff.0 <= d.crafting_magic_affinity_threshold
                || skills.magic <= d.crafting_magic_skill_threshold
            {
                return None;
            }

            let on_special = if map.in_bounds(pos.x, pos.y) {
                matches!(
                    map.get(pos.x, pos.y).terrain,
                    Terrain::FairyRing | Terrain::StandingStone
                )
            } else {
                false
            };

            if on_special {
                let chain = TaskChain::new(
                    vec![TaskStep::new(StepKind::SpiritCommunion).with_position(*pos)],
                    FailurePolicy::AbortChain,
                );
                return Some((chain, Action::PracticeMagic));
            }

            let chain = TaskChain::new(
                vec![TaskStep::new(StepKind::Scry).with_position(*pos)],
                FailurePolicy::AbortChain,
            );
            Some((chain, Action::PracticeMagic))
        }

        CraftingHint::Cleanse => {
            if magic_aff.0 <= d.crafting_magic_affinity_threshold {
                return None;
            }
            let chain = TaskChain::new(
                vec![TaskStep::new(StepKind::CleanseCorruption).with_position(*pos)],
                FailurePolicy::AbortChain,
            );
            Some((chain, Action::PracticeMagic))
        }

        CraftingHint::HarvestCarcass => {
            // Legacy TaskChain path can't express HarvestCarcass cleanly;
            // the GOAP executor handles it.
            None
        }

        // Cook is executed entirely through the GOAP planner
        // (`GoapActionKind::RetrieveRawFood` / `Cook` / `DepositCookedFood`
        // in `src/systems/goap.rs`). The unmet-demand signal is emitted
        // from `score_cook` via `ScoringResult::wants_cook_but_no_kitchen`,
        // not from here.
        CraftingHint::Cook => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_coordinating_chain(
    entity: Entity,
    pos: &Position,
    directive_queue_query: &Query<(Entity, &DirectiveQueue)>,
    active_directive_query: &Query<&ActiveDirective>,
    cat_positions: &[(Entity, Position)],
    skills_query: &Query<&Skills, Without<Dead>>,
    d: &DispositionConstants,
    _commands: &mut Commands,
) -> Option<(TaskChain, Action)> {
    let (_, queue) = directive_queue_query.get(entity).ok()?;
    let directive = queue.directives.first()?.clone();

    // Find the best target for this directive.
    let target = cat_positions
        .iter()
        .filter(|(e, _)| *e != entity)
        .filter(|(e, _)| active_directive_query.get(*e).is_err())
        .filter(|(_, p)| pos.manhattan_distance(p) <= d.coordinating_target_range)
        .max_by(|(e_a, p_a), (e_b, p_b)| {
            let skill_a = skills_query
                .get(*e_a)
                .map_or(0.0, |s| match directive.kind {
                    DirectiveKind::Hunt => s.hunting,
                    DirectiveKind::Forage => s.foraging,
                    DirectiveKind::Build => s.building,
                    DirectiveKind::Fight | DirectiveKind::Patrol => s.combat,
                    DirectiveKind::Herbcraft | DirectiveKind::SetWard => s.herbcraft,
                    DirectiveKind::Cleanse => s.magic,
                    DirectiveKind::HarvestCarcass => s.herbcraft,
                    DirectiveKind::Cook => 0.0,
                });
            let skill_b = skills_query
                .get(*e_b)
                .map_or(0.0, |s| match directive.kind {
                    DirectiveKind::Hunt => s.hunting,
                    DirectiveKind::Forage => s.foraging,
                    DirectiveKind::Build => s.building,
                    DirectiveKind::Fight | DirectiveKind::Patrol => s.combat,
                    DirectiveKind::Herbcraft | DirectiveKind::SetWard => s.herbcraft,
                    DirectiveKind::Cleanse => s.magic,
                    DirectiveKind::HarvestCarcass => s.herbcraft,
                    DirectiveKind::Cook => 0.0,
                });
            let rank_a =
                skill_a - pos.manhattan_distance(p_a) as f32 * d.coordinating_distance_penalty;
            let rank_b =
                skill_b - pos.manhattan_distance(p_b) as f32 * d.coordinating_distance_penalty;
            rank_a
                .partial_cmp(&rank_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(e, p)| (*e, *p));

    let (target_entity, target_pos) = target?;

    let chain = TaskChain::new(
        vec![
            TaskStep::new(StepKind::MoveTo).with_position(target_pos),
            TaskStep::new(StepKind::DeliverDirective {
                kind: directive.kind,
                priority: directive.priority,
                directive_target: directive.target_position,
            })
            .with_position(target_pos)
            .with_entity(target_entity),
        ],
        FailurePolicy::AbortChain,
    );

    Some((chain, Action::Coordinate))
}

fn build_exploring_chain(
    pos: &Position,
    map: &TileMap,
    d: &DispositionConstants,
    rng: &mut impl Rng,
) -> Option<(TaskChain, Action)> {
    let dx: i32 = rng.random_range(-d.explore_range..=d.explore_range);
    let dy: i32 = rng.random_range(-d.explore_range..=d.explore_range);
    let mut target = Position::new(pos.x + dx, pos.y + dy);
    target.x = target.x.clamp(0, map.width - 1);
    target.y = target.y.clamp(0, map.height - 1);

    let chain = TaskChain::new(
        vec![
            TaskStep::new(StepKind::MoveTo).with_position(target),
            TaskStep::new(StepKind::Survey).with_position(target),
        ],
        FailurePolicy::AbortChain,
    );
    Some((chain, Action::Explore))
}

// ===========================================================================
// build_mating_chain
// ===========================================================================

/// Build a Mate chain around a pre-resolved partner. §6.2 / §6.5.2:
/// partner selection is owned by
/// [`crate::ai::dses::mate_target::resolve_mate_target`] upstream;
/// this function owns only the chain shape. The legacy
/// `romantic + fondness - 0.05 × dist` mixer and its inline bond
/// filter retire here in favor of the target-taking DSE's bundle.
fn build_mating_chain(
    mate_target: Option<Entity>,
    cat_positions: &[(Entity, Position)],
) -> Option<(TaskChain, Action)> {
    let partner = mate_target?;
    let partner_pos = *cat_positions
        .iter()
        .find(|(e, _)| *e == partner)
        .map(|(_, p)| p)?;

    let chain = TaskChain::new(
        vec![
            TaskStep::new(StepKind::MoveTo).with_position(partner_pos),
            TaskStep::new(StepKind::Socialize).with_entity(partner),
            TaskStep::new(StepKind::GroomOther).with_entity(partner),
            TaskStep::new(StepKind::MateWith).with_entity(partner),
        ],
        FailurePolicy::AbortChain,
    );
    Some((chain, Action::Mate))
}

/// 154: Mentoring chain — MoveTo apprentice, then MentorCat. Mirrors
/// `build_mating_chain` since both are single-interaction Pattern-B
/// dispositions. Legacy path (per the 150 R5a comment on the chain
/// dispatch); the GOAP planner is the live path.
fn build_mentoring_chain(
    mentor_target: Option<Entity>,
    cat_positions: &[(Entity, Position)],
) -> Option<(TaskChain, Action)> {
    let apprentice = mentor_target?;
    let apprentice_pos = *cat_positions
        .iter()
        .find(|(e, _)| *e == apprentice)
        .map(|(_, p)| p)?;

    let chain = TaskChain::new(
        vec![
            TaskStep::new(StepKind::MoveTo).with_position(apprentice_pos),
            TaskStep::new(StepKind::MentorCat)
                .with_position(apprentice_pos)
                .with_entity(apprentice),
        ],
        FailurePolicy::AbortChain,
    );
    Some((chain, Action::Mentor))
}

// ===========================================================================
// build_grooming_chain
// ===========================================================================

/// 158: legacy chain-building path for the new `Grooming` disposition.
/// Mirrors `build_mentoring_chain`'s single-interaction Pattern-B shape.
/// The GOAP planner (`grooming_actions()` returning the single
/// `[GroomOther]` step) is the live path; this helper exists so the
/// dead-code dispatch arm in `evaluate_dispositions` still type-checks.
fn build_grooming_chain(
    groom_other_target: Option<Entity>,
    cat_positions: &[(Entity, Position)],
) -> Option<(TaskChain, Action)> {
    let partner = groom_other_target?;
    let partner_pos = *cat_positions
        .iter()
        .find(|(e, _)| *e == partner)
        .map(|(_, p)| p)?;

    let chain = TaskChain::new(
        vec![
            TaskStep::new(StepKind::MoveTo).with_position(partner_pos),
            TaskStep::new(StepKind::GroomOther)
                .with_position(partner_pos)
                .with_entity(partner),
        ],
        FailurePolicy::AbortChain,
    );
    Some((chain, Action::GroomOther))
}

// ===========================================================================
// build_caretaking_chain
// ===========================================================================

/// Build a Caretake chain that retrieves food from the nearest
/// Stores and delivers it to the caretake-target kitten. Phase 4c.3
/// rewrite — the previous chain sent the adult to Stores with
/// `FeedKitten { target_entity = stores }`, which coupled with the
/// broken `resolve_feed_kitten` (credited the *adult's* social, not
/// the kitten's hunger) meant no kitten was ever fed. This 4-step
/// version models physical causality per CLAUDE.md: retrieve food,
/// carry it, deliver it to the kitten entity.
fn build_caretaking_chain(
    target_kitten: Option<Entity>,
    target_kitten_pos: Option<Position>,
    nearest_store: Option<(Entity, Position)>,
) -> Option<(TaskChain, Action)> {
    let kitten = target_kitten?;
    let kitten_pos = target_kitten_pos?;
    let (store_entity, store_pos) = nearest_store?;
    let chain = TaskChain::new(
        vec![
            TaskStep::new(StepKind::MoveTo).with_position(store_pos),
            TaskStep::new(StepKind::RetrieveAnyFoodFromStores).with_entity(store_entity),
            TaskStep::new(StepKind::MoveTo).with_position(kitten_pos),
            TaskStep::new(StepKind::FeedKitten).with_entity(kitten),
        ],
        FailurePolicy::AbortChain,
    );
    Some((chain, Action::Caretake))
}

/// Move one tile in direction (dx, dy). If blocked, try perpendicular or
/// reverse. Returns the new position. Guaranteed to attempt movement — never
/// returns the original position without trying alternatives.
fn patrol_move(pos: &Position, dx: i32, dy: i32, map: &TileMap) -> Position {
    // Primary direction.
    let primary = Position::new(pos.x + dx, pos.y + dy);
    if map.in_bounds(primary.x, primary.y) && map.get(primary.x, primary.y).terrain.is_passable() {
        return primary;
    }
    // Try perpendicular (swap dx/dy).
    let perp = Position::new(pos.x + dy, pos.y + dx);
    if map.in_bounds(perp.x, perp.y) && map.get(perp.x, perp.y).terrain.is_passable() {
        return perp;
    }
    // Try reverse.
    let rev = Position::new(pos.x - dx, pos.y - dy);
    if map.in_bounds(rev.x, rev.y) && map.get(rev.x, rev.y).terrain.is_passable() {
        return rev;
    }
    // Stuck — stay put.
    *pos
}

// ===========================================================================
// resolve_disposition_chains
// ===========================================================================

/// Deferred mentor-skill transfer, collected during the main loop and applied
/// in a post-loop pass to avoid double-borrowing `Skills`.
struct MentorEffect {
    apprentice: Entity,
    mentor_skills: Skills,
}

/// Immutable pre-loop snapshots consumed by [`dispatch_chain_step`].
struct ChainStepSnapshots {
    grooming: std::collections::HashMap<Entity, f32>,
    gender: std::collections::HashMap<Entity, Gender>,
    cat_tile_counts: std::collections::HashMap<Position, u32>,
}

/// Mutable accumulators written by [`dispatch_chain_step`], consumed by the
/// post-loop cleanup pass in [`resolve_disposition_chains`].
struct ChainStepAccumulators {
    mentor_effects: Vec<MentorEffect>,
    grooming_restorations: Vec<crate::steps::disposition::GroomOutcome>,
    kitten_feedings: Vec<Entity>,
}

/// Apply a step handler result to the task chain, syncing `CurrentAction`
/// targets when the active step changes.
fn apply_step_result(
    result: crate::steps::StepResult,
    chain: &mut TaskChain,
    current: &mut CurrentAction,
) {
    match result {
        crate::steps::StepResult::Continue => {}
        crate::steps::StepResult::Advance => {
            chain.advance();
            chain.sync_targets(current);
        }
        crate::steps::StepResult::Fail(reason) => {
            chain.fail_current(reason);
            // SkipStep policy advances to the next step — sync those targets.
            chain.sync_targets(current);
        }
    }
}

/// Resolves disposition-specific TaskChain steps (HuntPrey, ForageItem, etc.)
/// and handles disposition completion when chains finish.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn resolve_disposition_chains(
    mut cats: Query<
        (
            (
                Entity,
                &mut TaskChain,
                &mut CurrentAction,
                &mut Position,
                &mut Skills,
                &mut Needs,
                &mut Inventory,
                &Personality,
                &mut Memory,
            ),
            (
                &Name,
                &Gender,
                Option<&mut Disposition>,
                Option<&mut ActionHistory>,
                &mut HuntingPriors,
                Option<&mut crate::components::grooming::GroomingCondition>,
                &mut crate::components::mental::Mood,
                Option<&mut crate::components::fulfillment::Fulfillment>,
            ),
        ),
        (
            Without<Dead>,
            Without<Structure>,
            Without<PreyAnimal>,
            Without<PreyDen>,
        ),
    >,
    mut prey_query: Query<(Entity, &Position, &PreyConfig, &mut PreyState), With<PreyAnimal>>,
    mut stores_query: Query<&mut StoredItems>,
    items_query: Query<
        &Item,
        bevy_ecs::query::Without<crate::components::items::BuildMaterialItem>,
    >,
    // Disjoint from `cats` (which requires TaskChain) — reads skills of non-chain
    // entities like apprentices being mentored.
    mut unchained_skills: Query<&mut Skills, (Without<TaskChain>, Without<Structure>)>,
    map: Res<TileMap>,
    wind: Res<crate::resources::wind::WindState>,
    mut relationships: ResMut<Relationships>,
    mut narr: NarrativeEmitter<'_>,
    time: Res<TimeState>,
    mut rng: ResMut<SimRng>,
    mut colony_map: ResMut<ColonyHuntingMap>,
    den_query: Query<(Entity, &PreyDen, &Position), Without<PreyAnimal>>,
    mut prey_params: PreyHuntParams,
    constants: Res<SimConstants>,
    mut commands: Commands,
) {
    let d = &constants.disposition;

    let mut accum = ChainStepAccumulators {
        mentor_effects: Vec::new(),
        grooming_restorations: Vec::new(),
        kitten_feedings: Vec::new(),
    };
    let mut chains_to_remove: Vec<Entity> = Vec::new();

    let snaps = ChainStepSnapshots {
        // Grooming condition for target lookups during social interactions.
        grooming: cats
            .iter()
            .map(
                |((e, _, _, _, _, _, _, _, _), (_, _, _, _, _, g, _, _))| {
                    (e, g.map_or(0.8, |g| g.0))
                },
            )
            .collect(),
        // Gender snapshot — used by `MateWith` to look up the partner's
        // gender for §7.M.7.4's gestator-selection fix without double-
        // borrowing the mutable `cats` query.
        gender: cats
            .iter()
            .map(|((e, _, _, _, _, _, _, _, _), (_, g, _, _, _, _, _, _))| (e, *g))
            .collect(),
        // Tile occupancy for anti-stacking jitter on PatrolTo arrival.
        cat_tile_counts: {
            let mut counts = std::collections::HashMap::new();
            for ((_, _, _, pos, _, _, _, _, _), _) in &cats {
                *counts.entry(*pos).or_insert(0) += 1;
            }
            counts
        },
    };

    for (
        (
            cat_entity,
            mut chain,
            mut current,
            mut pos,
            mut skills,
            mut needs,
            mut inventory,
            personality,
            mut memory,
        ),
        (
            name,
            gender,
            disposition,
            history,
            mut hunting_priors,
            mut grooming,
            mut mood,
            mut fulfillment_opt,
        ),
    ) in &mut cats
    {
        // Den discovery: peek at current step kind without mutable borrow.
        // If the cat is hunting and near a den, raid it and advance the chain
        // before taking the mutable step borrow below.
        if let Some(step_ref) = chain.steps.get(chain.current_step) {
            if matches!(step_ref.kind, StepKind::HuntPrey { .. }) {
                use crate::components::magic::ItemSlot;
                let mut found_den = false;
                for (den_entity, den, den_pos) in den_query.iter() {
                    if pos.manhattan_distance(den_pos) <= d.den_discovery_range {
                        let discovery_chance = d.den_discovery_base_chance
                            + skills.hunting * d.den_discovery_skill_scale;
                        if rng.rng.random::<f32>() < discovery_chance && den.spawns_remaining > 0 {
                            // Raid: kill ~40% of remaining, capped at raid_drop.
                            let kills = ((den.spawns_remaining as f32 * d.den_raid_kill_fraction)
                                .ceil() as u32)
                                .min(den.raid_drop);
                            let drop_item = den.item_kind;
                            let den_name = den.den_name;
                            let den_pos_copy = *den_pos;

                            // Cat picks up what it can carry.
                            let den_corruption = if map.in_bounds(den_pos_copy.x, den_pos_copy.y) {
                                map.get(den_pos_copy.x, den_pos_copy.y).corruption
                            } else {
                                0.0
                            };
                            let den_mods = crate::components::items::ItemModifiers::with_corruption(
                                den_corruption,
                            );
                            for _ in 0..kills {
                                if !inventory.is_full() {
                                    inventory.slots.push(ItemSlot::Item(drop_item, den_mods));
                                } else {
                                    commands.spawn((
                                        crate::components::items::Item::with_modifiers(
                                            drop_item,
                                            d.den_dropped_item_quality,
                                            crate::components::items::ItemLocation::OnGround,
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

                            // Send raid message — den mutation happens in apply_den_raids.
                            prey_params.raid_writer.write(DenRaided {
                                den_entity,
                                kills,
                                item_kind: drop_item,
                                position: den_pos_copy,
                                den_name,
                            });

                            {
                                let terrain = if map.in_bounds(pos.x, pos.y) {
                                    map.get(pos.x, pos.y).terrain
                                } else {
                                    Terrain::Grass
                                };
                                let day_phase = DayPhase::from_tick(time.tick, &narr.config);
                                let season = Season::from_tick(time.tick, &narr.config);
                                let ctx = TemplateContext {
                                    action: crate::ai::Action::Hunt,
                                    day_phase,
                                    season,
                                    weather: narr.weather.current,
                                    mood_bucket: MoodBucket::Neutral,
                                    life_stage: LifeStage::Adult,
                                    has_target: true,
                                    terrain,
                                    event: Some("raid".into()),
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
                                    prey: Some(den_name),
                                    item: None,
                                    item_singular: None,
                                    quality: None,
                                };
                                emit_event_narrative(
                                    narr.registry.as_deref(),
                                    &mut narr.log,
                                    time.tick,
                                    format!("{} raids a {}!", name.0, den_name),
                                    crate::resources::narrative::NarrativeTier::Action,
                                    &ctx,
                                    &var_ctx,
                                    personality,
                                    &needs,
                                    &mut rng.rng,
                                );
                            }
                            chain.advance();
                            chain.sync_targets(&mut current);
                            found_den = true;
                            break;
                        }
                    }
                }
                if found_den {
                    continue;
                }
            }
        }

        let Some(step) = chain.current_mut() else {
            // Chain exhausted — handle completion or failure.
            chains_to_remove.push(cat_entity);
            if let Some(mut disp) = disposition {
                let outcome = if chain.is_succeeded() {
                    disp.completions += 1;
                    // Successful completions earn respect proportional to contribution.
                    let respect_gain = respect_for_disposition(disp.kind, d);
                    if respect_gain > 0.0 {
                        needs.respect = (needs.respect + respect_gain).min(1.0);
                    }
                    // NOTE: the §respect-restoration witness-multiplier used
                    // to live here, but this function (`resolve_disposition_chains`)
                    // is only registered in test schedules. The canonical live
                    // site is `resolve_goap_plans`'s plan-completion block in
                    // `src/systems/goap.rs`. See
                    // `docs/balance/respect-restoration.md`.
                    ActionOutcome::Success
                } else {
                    ActionOutcome::Failure
                };
                if let Some(mut hist) = history {
                    hist.record(ActionRecord {
                        action: current.action,
                        disposition: Some(disp.kind),
                        tick: time.tick,
                        outcome,
                    });
                }
            }
            current.ticks_remaining = 0;
            continue;
        };

        // Only handle disposition-specific steps; skip others.
        let is_disposition_step = matches!(
            step.kind,
            StepKind::HuntPrey { .. }
                | StepKind::ForageItem { .. }
                | StepKind::DepositAtStores
                | StepKind::EatAtStores
                | StepKind::Sleep { .. }
                | StepKind::SelfGroom
                | StepKind::Socialize
                | StepKind::GroomOther
                | StepKind::MentorCat
                | StepKind::PatrolTo
                | StepKind::FightThreat
                | StepKind::Survey
                | StepKind::DeliverDirective { .. }
                | StepKind::MateWith
                | StepKind::FeedKitten
                | StepKind::RetrieveFromStores { .. }
        );
        if !is_disposition_step {
            continue;
        }

        // Ensure step is in progress.
        if matches!(
            step.status,
            crate::components::task_chain::StepStatus::Pending
        ) {
            step.status =
                crate::components::task_chain::StepStatus::InProgress { ticks_elapsed: 0 };
        }

        let ticks = match &mut step.status {
            crate::components::task_chain::StepStatus::InProgress { ticks_elapsed } => {
                *ticks_elapsed += 1;
                *ticks_elapsed
            }
            _ => continue,
        };

        // Clone the step kind so the `step` borrow (from `chain.current_mut()`)
        // can be dropped before we pass `&mut chain` to the dispatch function.
        let step_kind = step.kind.clone();

        // ---- Dispatch on step kind ----
        // Extracted to a separate function to keep `resolve_disposition_chains`
        // under LLVM's optimization-cliff threshold (~4,500 lines).
        // See docs/open-work.md §"resolve_disposition_chains split".
        dispatch_chain_step(
            step_kind,
            ticks,
            cat_entity,
            &mut chain,
            &mut current,
            &mut pos,
            &mut skills,
            &mut needs,
            &mut inventory,
            personality,
            &mut memory,
            name,
            gender,
            &mut hunting_priors,
            grooming.as_deref_mut(),
            &mut fulfillment_opt,
            &mut prey_query,
            &mut stores_query,
            &items_query,
            &mut prey_params,
            &map,
            &wind,
            &mut relationships,
            &mut narr,
            &time,
            &mut rng,
            &mut colony_map,
            &constants,
            &mut commands,
            &snaps,
            &mut accum,
        );

        if chain.is_complete() {
            chains_to_remove.push(cat_entity);
            if let Some(mut disp) = disposition {
                let succeeded = !chain.is_failed();
                disp.completions += 1;
                if succeeded {
                    let respect_gain = respect_for_disposition(disp.kind, d);
                    if respect_gain > 0.0 {
                        needs.respect = (needs.respect + respect_gain).min(1.0);
                    }
                    // NOTE: the §respect-restoration witness-multiplier moved
                    // to `resolve_goap_plans`'s plan-completion block — this
                    // function is only scheduled in tests. See
                    // `docs/balance/respect-restoration.md`.
                    // Building completion grants extra mood boost ("built something").
                    if disp.kind == DispositionKind::Building {
                        mood.modifiers.push_back(
                            crate::components::mental::MoodModifier::new(
                                0.2,
                                100,
                                "built something",
                            )
                            .with_kind(crate::components::mental::MoodSource::Pride),
                        );
                    }
                }
                if let Some(mut hist) = history {
                    hist.record(ActionRecord {
                        action: current.action,
                        disposition: Some(disp.kind),
                        tick: time.tick,
                        outcome: if succeeded {
                            ActionOutcome::Success
                        } else {
                            ActionOutcome::Failure
                        },
                    });
                }
            }
            current.ticks_remaining = 0;
        }
    }

    for entity in chains_to_remove {
        commands.entity(entity).remove::<TaskChain>();
    }

    // Apply deferred kitten-feedings from FeedKitten steps (§Phase 4c.3).
    // Kitten also gains acceptance — being fed is the highest-signal
    // "cared for" event in a kitten's life.
    for kitten_entity in accum.kitten_feedings {
        if let Ok(((_, _, _, _, _, mut k_needs, _, _, _), _)) = cats.get_mut(kitten_entity) {
            k_needs.hunger = (k_needs.hunger + 0.5).min(1.0);
            k_needs.acceptance = (k_needs.acceptance + d.acceptance_per_kitten_fed).min(1.0);
        }
    }

    // Apply deferred grooming restorations from GroomOther steps.
    // Recipient also gains acceptance — being groomed is the receiver side
    // of social warmth, which otherwise has no sim-level restorer.
    // §7.W: also apply social_warmth delta to the groomed target.
    for groom in accum.grooming_restorations {
        if let Ok((
            (_, _, _, _, _, mut needs, _, _, _),
            (_, _, _, _, _, grooming, _, fulfillment),
        )) =
            cats.get_mut(groom.target)
        {
            if let Some(mut g) = grooming {
                g.0 = (g.0 + groom.grooming_delta).min(1.0);
            }
            needs.acceptance = (needs.acceptance + d.acceptance_per_groomed).min(1.0);
            if let Some(mut f) = fulfillment {
                f.social_warmth = (f.social_warmth + groom.social_warmth_delta).min(1.0);
            }
        }
    }

    // Apply deferred mentor effects: grow apprentice's weakest teachable skill.
    // The apprentice may have a TaskChain (in `cats`) or not (in `unchained_skills`).
    for effect in &accum.mentor_effects {
        // Try the unchained query first (more common — apprentice is usually idle).
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
            let _pairs: [(f32, f32); 6] = [
                (effect.mentor_skills.hunting, hunt),
                (effect.mentor_skills.foraging, forage),
                (effect.mentor_skills.herbcraft, herb),
                (effect.mentor_skills.building, build),
                (effect.mentor_skills.combat, combat),
                (effect.mentor_skills.magic, magic),
            ];
            let pairs: [(f32, f32); 6] = [
                (effect.mentor_skills.hunting, hunt),
                (effect.mentor_skills.foraging, forage),
                (effect.mentor_skills.herbcraft, herb),
                (effect.mentor_skills.building, build),
                (effect.mentor_skills.combat, combat),
                (effect.mentor_skills.magic, magic),
            ];
            // Skill with the largest teachable gap.
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
                // Write to whichever query holds this entity.
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
// dispatch_chain_step — the step-resolution match dispatch, extracted from
// `resolve_disposition_chains` to keep both functions under LLVM's
// optimization-cliff threshold.
// ===========================================================================

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn dispatch_chain_step(
    step_kind: StepKind,
    ticks: u64,
    cat_entity: Entity,
    chain: &mut TaskChain,
    current: &mut CurrentAction,
    pos: &mut Position,
    skills: &mut Skills,
    needs: &mut Needs,
    inventory: &mut Inventory,
    personality: &Personality,
    memory: &mut Memory,
    name: &Name,
    gender: &Gender,
    hunting_priors: &mut HuntingPriors,
    grooming: Option<&mut crate::components::grooming::GroomingCondition>,
    fulfillment_opt: &mut Option<Mut<crate::components::fulfillment::Fulfillment>>,
    prey_query: &mut Query<(Entity, &Position, &PreyConfig, &mut PreyState), With<PreyAnimal>>,
    stores_query: &mut Query<&mut StoredItems>,
    items_query: &Query<
        &Item,
        bevy_ecs::query::Without<crate::components::items::BuildMaterialItem>,
    >,
    prey_params: &mut PreyHuntParams,
    map: &TileMap,
    wind: &crate::resources::wind::WindState,
    relationships: &mut Relationships,
    narr: &mut NarrativeEmitter,
    time: &TimeState,
    rng: &mut SimRng,
    colony_map: &mut ColonyHuntingMap,
    constants: &SimConstants,
    commands: &mut Commands,
    snaps: &ChainStepSnapshots,
    accum: &mut ChainStepAccumulators,
) {
    let d = &constants.disposition;

    match step_kind {
        StepKind::HuntPrey { patrol_dir } => {
            let step = chain.current_mut().unwrap();
            // Multi-phase hunt: Search → Stalk → Pounce.
            // Phase is implicit from step.target_entity:
            //   None = Search (scent-based) or Approach (scent locked)
            //   Some = Stalk/Pounce (prey visible)
            use crate::components::magic::ItemSlot;
            use crate::components::prey::PreyAiState;

            if let Some(target_entity) = step.target_entity {
                // We have a locked target — check if it still exists.
                let Ok((_, prey_pos, prey_cfg, prey_state)) = prey_query.get(target_entity) else {
                    step.target_entity = None;
                    return;
                };
                let prey_pos = *prey_pos;
                let prey_is_fleeing = matches!(prey_state.ai_state, PreyAiState::Fleeing { .. });
                let prey_awareness = prey_state.ai_state;
                let catch_mod = prey_cfg.catch_difficulty;
                let item_kind = prey_cfg.item_kind;
                let species_name = prey_cfg.name;
                let flee_strategy = prey_cfg.flee_strategy;
                let dist = pos.manhattan_distance(&prey_pos);

                // Bird-specific: if prey teleported away, give up immediately.
                if prey_is_fleeing
                    && flee_strategy == crate::components::prey::FleeStrategy::Teleport
                {
                    step.target_entity = None;
                    return;
                }

                // Dynamic stalk-start: slow down before entering detection zone.
                let stalk_start =
                    (prey_cfg.alert_radius + d.stalk_start_buffer).max(d.stalk_start_minimum);

                // Determine pounce range from patience.
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
                    // Density-dependent vulnerability: crowded prey are easier to catch.
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
                                crate::components::items::ItemModifiers::with_corruption(
                                    catch_corruption,
                                ),
                            ));
                        }
                        skills.hunting += skills.growth_rate() * d.hunt_catch_skill_growth;

                        // Send kill event for den pressure tracking.
                        prey_params.kill_writer.write(PreyKilled {
                            kind: prey_cfg.kind,
                            position: prey_pos,
                        });

                        {
                            let terrain = if map.in_bounds(pos.x, pos.y) {
                                map.get(pos.x, pos.y).terrain
                            } else {
                                Terrain::Grass
                            };
                            let day_phase = DayPhase::from_tick(time.tick, &narr.config);
                            let season = Season::from_tick(time.tick, &narr.config);
                            let catch_desc = if catch_corruption > 0.3 {
                                format!("corrupted {}", species_name)
                            } else {
                                species_name.to_string()
                            };
                            let ctx = TemplateContext {
                                action: crate::ai::Action::Hunt,
                                day_phase,
                                season,
                                weather: narr.weather.current,
                                mood_bucket: MoodBucket::Neutral,
                                life_stage: LifeStage::Adult,
                                has_target: true,
                                terrain,
                                event: Some("catch".into()),
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
                                prey: Some(species_name),
                                item: None,
                                item_singular: None,
                                quality: None,
                            };
                            emit_event_narrative(
                                narr.registry.as_deref(),
                                &mut narr.log,
                                time.tick,
                                format!("{} catches a {}.", name.0, catch_desc),
                                crate::resources::narrative::NarrativeTier::Action,
                                &ctx,
                                &var_ctx,
                                personality,
                                needs,
                                &mut rng.rng,
                            );
                        }

                        hunting_priors.record_catch(&prey_pos);

                        // Multi-kill: if inventory has room, hunt for more.
                        if inventory.is_full() {
                            chain.advance();
                            chain.sync_targets(current);
                        } else {
                            step.target_entity = None; // Search for another target.
                        }
                    } else {
                        // Pounce failed — prey bolts.
                        if let Ok((_, _, _, mut prey_st)) = prey_query.get_mut(target_entity) {
                            prey_st.ai_state = PreyAiState::Fleeing {
                                from: cat_entity,
                                toward: None,
                                ticks: 0,
                            };
                        }

                        {
                            let terrain = if map.in_bounds(pos.x, pos.y) {
                                map.get(pos.x, pos.y).terrain
                            } else {
                                Terrain::Grass
                            };
                            let day_phase = DayPhase::from_tick(time.tick, &narr.config);
                            let season = Season::from_tick(time.tick, &narr.config);
                            let ctx = TemplateContext {
                                action: crate::ai::Action::Hunt,
                                day_phase,
                                season,
                                weather: narr.weather.current,
                                mood_bucket: MoodBucket::Neutral,
                                life_stage: LifeStage::Adult,
                                has_target: true,
                                terrain,
                                event: Some("miss".into()),
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
                                prey: Some(species_name),
                                item: None,
                                item_singular: None,
                                quality: None,
                            };
                            emit_event_narrative(
                                narr.registry.as_deref(),
                                &mut narr.log,
                                time.tick,
                                format!("{}'s quarry bolts.", name.0),
                                crate::resources::narrative::NarrativeTier::Micro,
                                &ctx,
                                &var_ctx,
                                personality,
                                needs,
                                &mut rng.rng,
                            );
                        }

                        let chase_limit = if personality.boldness > 0.7 {
                            d.chase_limit_bold
                        } else {
                            d.chase_limit_default
                        };
                        if ticks > chase_limit {
                            // Don't fail the whole chain — just drop target and
                            // resume searching. Cats are persistent hunters.
                            step.target_entity = None;
                        }
                    }
                } else if dist <= stalk_start {
                    let mut moved = false;
                    if prey_is_fleeing {
                        // === CHASE === sprint burst.
                        for _ in 0..d.chase_speed {
                            if let Some(next) = step_toward(pos, &prey_pos, map) {
                                *pos = next;
                                moved = true;
                            }
                        }
                    } else {
                        // === STALK === Deliberate approach, 1 tile/tick.
                        // Cats are agile ambush predators — they close quickly
                        // while relying on stealth to avoid detection.
                        if let Some(next) = step_toward(pos, &prey_pos, map) {
                            *pos = next;
                            moved = true;
                        }
                        // Anxiety check: nervous cat spooks prey.
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
                            step.target_entity = None; // Resume searching.
                        }
                    }
                    // Can't reach prey (terrain blocked) — give up after a few ticks.
                    if !moved && ticks > 10 {
                        step.target_entity = None;
                    }
                } else {
                    // === APPROACH === Trot toward scented prey.
                    let mut moved = false;
                    for _ in 0..d.approach_speed {
                        if let Some(next) = step_toward(pos, &prey_pos, map) {
                            *pos = next;
                            moved = true;
                        }
                    }
                    if dist > d.approach_give_up_distance || (!moved && ticks > 10) {
                        step.target_entity = None;
                    }
                }
            } else {
                // === SEARCH === (no target yet — move toward best-known hunting ground)
                // Priority: personal belief > colony belief > wind > patrol_dir.
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
                    patrol_dir
                };
                // Jitter — enough to explore but not lose the gradient.
                if rng.rng.random::<f32>() < d.search_jitter_chance {
                    dx = rng.rng.random_range(-1i32..=1);
                    dy = rng.rng.random_range(-1i32..=1);
                }
                // Ensure we actually move (avoid 0,0).
                if dx == 0 && dy == 0 {
                    dx = 1;
                }
                // Trot while searching to cover ground.
                for _ in 0..d.search_speed {
                    *pos = patrol_move(pos, dx, dy, map);
                }

                // Visual detection: spot nearby prey within 15 tiles.
                let visible_prey = prey_query
                    .iter()
                    .filter(|(_, pp, _, _)| {
                        crate::systems::sensing::observer_sees_at(
                            crate::components::SensorySpecies::Cat,
                            *pos,
                            &constants.sensory.cat,
                            **pp,
                            crate::components::SensorySignature::PREY,
                            d.search_visual_detection_range as f32,
                        )
                    })
                    .min_by_key(|(_, pp, _, _)| pos.manhattan_distance(pp));

                if let Some((prey_entity, _prey_pos_ref, _, _)) = visible_prey {
                    step.target_entity = Some(prey_entity);
                } else {
                    // Scan for prey scent via PreyScentMap (Phase 2B —
                    // grid-sampled influence map). Finds the strongest-
                    // scent bucket within `scent_search_radius`; the
                    // `min_by_key` below resolves to the prey entity
                    // closest to that source tile.
                    let scent_source = prey_params.prey_scent_map.highest_nearby(
                        pos.x,
                        pos.y,
                        d.scent_search_radius,
                    );
                    let scent_above_threshold = scent_source
                        .map(|(sx, sy)| {
                            prey_params.prey_scent_map.get(sx, sy) >= d.scent_detect_threshold
                        })
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
                        step.target_entity = Some(prey_entity);
                        hunting_priors.record_scent(prey_pos_ref);
                        {
                            let terrain = if map.in_bounds(pos.x, pos.y) {
                                map.get(pos.x, pos.y).terrain
                            } else {
                                Terrain::Grass
                            };
                            let day_phase = DayPhase::from_tick(time.tick, &narr.config);
                            let season = Season::from_tick(time.tick, &narr.config);
                            let ctx = TemplateContext {
                                action: crate::ai::Action::Hunt,
                                day_phase,
                                season,
                                weather: narr.weather.current,
                                mood_bucket: MoodBucket::Neutral,
                                life_stage: LifeStage::Adult,
                                has_target: false,
                                terrain,
                                event: Some("scent".into()),
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
                                item: None,
                                item_singular: None,
                                quality: None,
                            };
                            emit_event_narrative(
                                narr.registry.as_deref(),
                                &mut narr.log,
                                time.tick,
                                format!("{} catches a scent on the wind.", name.0),
                                crate::resources::narrative::NarrativeTier::Micro,
                                &ctx,
                                &var_ctx,
                                personality,
                                needs,
                                &mut rng.rng,
                            );
                        }
                    }
                }

                if ticks > d.search_timeout_ticks {
                    // Multi-kill: if we already have food, head to stores.
                    if inventory
                        .slots
                        .iter()
                        .any(|s| matches!(s, ItemSlot::Item(k, _) if k.is_food()))
                    {
                        chain.advance();
                        chain.sync_targets(current);
                    } else {
                        hunting_priors.record_failed_search(pos, ticks);
                        chain.fail_current("no scent found".into());
                    }
                }
            }
        }

        StepKind::ForageItem { patrol_dir } => {
            // Active foraging: directional patrol, check each tile.
            // Use patrol_dir with jitter and reverse-on-blocked (wildlife pattern).
            let mut dx = patrol_dir.0;
            let mut dy = patrol_dir.1;
            if dx == 0 && dy == 0 {
                dx = 1;
            } // ensure movement
              // Forage jitter.
            if rng.rng.random::<f32>() < d.forage_jitter_chance {
                dx = rng.rng.random_range(-1i32..=1);
                dy = rng.rng.random_range(-1i32..=1);
                if dx == 0 && dy == 0 {
                    dx = 1;
                }
            }
            *pos = patrol_move(pos, dx, dy, map);

            // Check current tile for forage yield.
            if map.in_bounds(pos.x, pos.y) {
                let tile = map.get(pos.x, pos.y);
                let forage_yield = tile.terrain.foraging_yield();
                if forage_yield > 0.0
                    && rng.rng.random::<f32>() < forage_yield * d.forage_yield_scale
                {
                    use crate::components::items::ItemKind;
                    use crate::components::magic::ItemSlot;
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
                            crate::components::items::ItemModifiers::with_corruption(
                                forage_corruption,
                            ),
                        ));
                    }
                    skills.foraging += skills.growth_rate() * d.forage_skill_growth;
                    {
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
                            action: crate::ai::Action::Forage,
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
                    }
                    chain.advance();
                    chain.sync_targets(current);
                } else if ticks > d.forage_timeout_ticks {
                    chain.fail_current("nothing found while foraging".into());
                }
            }
        }

        StepKind::DepositAtStores => {
            let step = chain.current_mut().unwrap();
            let target = step.target_entity;
            let deposit = crate::steps::disposition::resolve_deposit_at_stores(
                target,
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
            // §purpose-restoration: a successful deposit (Advance, not
            // rejected, not no-store) is a tangible asset added to the
            // colony pool. Skip the bump on rejected/no-store paths.
            if matches!(deposit.step, crate::steps::StepResult::Advance)
                && !deposit.rejected
                && !deposit.no_store
            {
                needs.purpose = (needs.purpose + d.purpose_per_deposit).min(1.0);
            }
            apply_step_result(deposit.step, chain, current);
        }

        StepKind::EatAtStores => {
            let step = chain.current_mut().unwrap();
            let target = step.target_entity;
            let outcome = crate::steps::disposition::resolve_eat_at_stores(
                ticks,
                target,
                needs,
                stores_query,
                items_query,
                commands,
                d,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::FoodEaten);
            apply_step_result(outcome.result, chain, current);
        }

        StepKind::Sleep { ticks: duration } => {
            let outcome = crate::steps::disposition::resolve_sleep(
                ticks, duration, needs, memory, pos, time.tick, d,
            );
            apply_step_result(outcome.result, chain, current);
        }

        StepKind::SelfGroom => {
            let outcome = crate::steps::disposition::resolve_self_groom(ticks, needs, grooming, d);
            apply_step_result(outcome.result, chain, current);
        }

        StepKind::Socialize => {
            let step = chain.current_mut().unwrap();
            let target = step.target_entity;
            let mut fallback_fulfillment = crate::components::fulfillment::Fulfillment::default();
            let fulfillment_ref = match fulfillment_opt.as_mut() {
                Some(f) => &mut **f,
                None => &mut fallback_fulfillment,
            };
            let outcome = crate::steps::disposition::resolve_socialize(
                ticks,
                cat_entity,
                target,
                needs,
                fulfillment_ref,
                hunting_priors,
                relationships,
                colony_map,
                &snaps.grooming,
                time.tick,
                &constants.social,
                d,
                &constants.fulfillment,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::Socialized);
            apply_step_result(outcome.result, chain, current);
        }

        StepKind::GroomOther => {
            let step = chain.current_mut().unwrap();
            let target = step.target_entity;
            let mut fallback_fulfillment = crate::components::fulfillment::Fulfillment::default();
            let fulfillment_ref = match fulfillment_opt.as_mut() {
                Some(f) => &mut **f,
                None => &mut fallback_fulfillment,
            };
            let outcome = crate::steps::disposition::resolve_groom_other(
                ticks,
                cat_entity,
                target,
                needs,
                fulfillment_ref,
                hunting_priors,
                relationships,
                colony_map,
                &snaps.grooming,
                time.tick,
                &constants.social,
                d,
                &constants.fulfillment,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::GroomedOther);
            if let Some(r) = outcome.witness {
                accum.grooming_restorations.push(r);
            }
            apply_step_result(outcome.result, chain, current);
        }

        StepKind::MentorCat => {
            let step = chain.current_mut().unwrap();
            let target = step.target_entity;
            let outcome = crate::steps::disposition::resolve_mentor_cat(
                ticks,
                cat_entity,
                target,
                needs,
                skills,
                relationships,
                time.tick,
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
            apply_step_result(result, chain, current);
        }

        StepKind::PatrolTo => {
            let step = chain.current_mut().unwrap();
            let target = step.target_position;
            let cached = &mut step.cached_path;
            let outcome = crate::steps::disposition::resolve_patrol_to(
                pos,
                target,
                cached,
                needs,
                map,
                d,
                &snaps.cat_tile_counts,
            );
            apply_step_result(outcome.result, chain, current);
        }

        StepKind::FightThreat => {
            let health = prey_params
                .health_query
                .get(cat_entity)
                .cloned()
                .unwrap_or_default();
            let outcome =
                crate::steps::disposition::resolve_fight_threat(ticks, skills, needs, &health, d);
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::ThreatEngaged);
            apply_step_result(outcome.result, chain, current);
        }

        StepKind::Survey => {
            let outcome = crate::steps::disposition::resolve_survey(
                ticks,
                needs,
                pos,
                &mut prey_params.exploration_map,
                d,
            );
            apply_step_result(outcome.result, chain, current);
        }

        StepKind::DeliverDirective {
            kind,
            priority,
            directive_target,
        } => {
            let step = chain.current_mut().unwrap();
            let outcome = crate::steps::disposition::resolve_deliver_directive(ticks, needs, d);
            if matches!(outcome.result, crate::steps::StepResult::Advance) {
                // Insert ActiveDirective on the target cat.
                if let Some(target) = step.target_entity {
                    commands.entity(target).insert(ActiveDirective {
                        kind,
                        priority,
                        coordinator: cat_entity,
                        coordinator_social_weight: needs.respect,
                        delivered_tick: time.tick,
                        target_position: directive_target,
                        target_entity: None,
                    });
                    // Gate on both witness (time-based success)
                    // AND a real directive target existing.
                    outcome.record_if_witnessed(
                        narr.activation.as_deref_mut(),
                        Feature::DirectiveDelivered,
                    );
                    // §purpose-restoration: completing a directive
                    // delivery is explicit colony-coordinated work.
                    needs.purpose = (needs.purpose + d.purpose_per_directive_completed).min(1.0);
                }
            }
            apply_step_result(outcome.result, chain, current);
        }

        StepKind::MateWith => {
            let step = chain.current_mut().unwrap();
            let target = step.target_entity;
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
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::MatingOccurred);
            // §Phase 5a: CourtshipInteraction for Tom×Tom or any
            // target-present, no-pregnancy Advance.
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
                // partner, not the initiator. `partner` on the
                // `Pregnant` struct is the other mate — so if the
                // initiator is the gestator, partner = target; if
                // the target is the gestator, partner = initiator.
                let partner = if gestator == cat_entity {
                    target.unwrap_or(cat_entity)
                } else {
                    cat_entity
                };
                commands
                    .entity(gestator)
                    .insert(crate::components::pregnancy::Pregnant::new(
                        time.tick,
                        partner,
                        litter_size,
                    ));
            }
            apply_step_result(outcome.result, chain, current);
        }

        StepKind::FeedKitten => {
            let step = chain.current_mut().unwrap();
            let target = step.target_entity;
            let outcome =
                crate::steps::disposition::resolve_feed_kitten(ticks, target, needs, inventory);
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::KittenFed);
            if let Some(kitten_entity) = outcome.witness {
                accum.kitten_feedings.push(kitten_entity);
            }
            apply_step_result(outcome.result, chain, current);
        }

        StepKind::RetrieveFromStores { kind } => {
            let step = chain.current_mut().unwrap();
            let target = step.target_entity;
            let outcome = crate::steps::disposition::resolve_retrieve_from_stores(
                ticks,
                kind,
                target,
                inventory,
                stores_query,
                items_query,
                commands,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::ItemRetrieved);
            apply_step_result(outcome.result, chain, current);
        }

        StepKind::RetrieveAnyFoodFromStores => {
            let step = chain.current_mut().unwrap();
            let target = step.target_entity;
            // §Phase 4c.4: swapped from the raw-only helper to the
            // any-food helper to match the step's name. The previous
            // call accepted only uncooked items, contradicting the
            // `RetrieveAnyFoodFromStores` semantic promised to
            // `build_caretaking_chain`. Since this disposition-chain
            // path is currently unscheduled (GOAP replaced it),
            // the bug never surfaced, but the rename keeps the two
            // systems aligned if the chain path is ever re-enabled.
            let outcome = crate::steps::disposition::resolve_retrieve_any_food_from_stores(
                ticks,
                target,
                inventory,
                stores_query,
                items_query,
                commands,
            );
            outcome.record_if_witnessed(narr.activation.as_deref_mut(), Feature::ItemRetrieved);
            apply_step_result(outcome.result, chain, current);
        }

        // Non-disposition steps are handled elsewhere.
        _ => {}
    }
}
// ---------------------------------------------------------------------------
// cat_presence_tick
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::system::SystemState;

    fn make_world_with_no_wildlife() -> World {
        World::new()
    }

    fn _make_world_with_wildlife_at(pos: Position) -> World {
        let mut world = World::new();
        world.spawn((
            pos,
            WildAnimal::new(crate::components::wildlife::WildSpecies::Fox),
        ));
        world
    }

    fn default_disposition(kind: DispositionKind) -> Disposition {
        Disposition {
            kind,
            adopted_tick: 0,
            disposition_started_tick: 0,
            completions: 0,
            target_completions: 3,
            crafting_hint: None,
        }
    }

    fn mid_personality() -> Personality {
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

    #[test]
    fn critical_health_interrupts_guarding() {
        let mut world = make_world_with_no_wildlife();
        let mut state: SystemState<Query<&Position, With<WildAnimal>>> =
            SystemState::new(&mut world);
        let wildlife = state.get(&world);

        let needs = Needs::default();
        let personality = mid_personality();
        let pos = Position { x: 5, y: 5 };
        let health = Health {
            current: 0.3,
            max: 1.0,
            injuries: Vec::new(),
            total_starvation_damage: 0.0,
        };
        let disposition = default_disposition(DispositionKind::Guarding);
        let d = SimConstants::default().disposition;

        let result = check_interrupt(
            &needs,
            &personality,
            &pos,
            &health,
            &disposition,
            &wildlife,
            &d,
            &SimConstants::default().sensory.cat,
        );
        assert!(
            matches!(result, Some(InterruptReason::CriticalHealth)),
            "guarding cat at 30% HP should get CriticalHealth interrupt, got {result:?}"
        );
    }

    #[test]
    fn healthy_guarding_cat_not_interrupted() {
        let mut world = make_world_with_no_wildlife();
        let mut state: SystemState<Query<&Position, With<WildAnimal>>> =
            SystemState::new(&mut world);
        let wildlife = state.get(&world);

        let needs = Needs::default();
        let personality = mid_personality();
        let pos = Position { x: 5, y: 5 };
        let health = Health::default(); // 1.0
        let disposition = default_disposition(DispositionKind::Guarding);
        let d = SimConstants::default().disposition;

        let result = check_interrupt(
            &needs,
            &personality,
            &pos,
            &health,
            &disposition,
            &wildlife,
            &d,
            &SimConstants::default().sensory.cat,
        );
        assert!(
            result.is_none(),
            "healthy guarding cat should not be interrupted, got {result:?}"
        );
    }

    #[test]
    fn critical_health_interrupts_resting() {
        let mut world = make_world_with_no_wildlife();
        let mut state: SystemState<Query<&Position, With<WildAnimal>>> =
            SystemState::new(&mut world);
        let wildlife = state.get(&world);

        let needs = Needs::default();
        let personality = mid_personality();
        let pos = Position { x: 5, y: 5 };
        let health = Health {
            current: 0.2,
            max: 1.0,
            injuries: Vec::new(),
            total_starvation_damage: 0.0,
        };
        let disposition = default_disposition(DispositionKind::Resting);
        let d = SimConstants::default().disposition;

        let result = check_interrupt(
            &needs,
            &personality,
            &pos,
            &health,
            &disposition,
            &wildlife,
            &d,
            &SimConstants::default().sensory.cat,
        );
        assert!(
            matches!(result, Some(InterruptReason::CriticalHealth)),
            "CriticalHealth should fire for any disposition, got {result:?}"
        );
    }

    #[test]
    fn health_just_above_threshold_no_interrupt() {
        let mut world = make_world_with_no_wildlife();
        let mut state: SystemState<Query<&Position, With<WildAnimal>>> =
            SystemState::new(&mut world);
        let wildlife = state.get(&world);

        let needs = Needs::default();
        let personality = mid_personality();
        let pos = Position { x: 5, y: 5 };
        let health = Health {
            current: 0.5,
            max: 1.0,
            injuries: Vec::new(),
            total_starvation_damage: 0.0,
        };
        let disposition = default_disposition(DispositionKind::Guarding);
        let d = SimConstants::default().disposition;

        let result = check_interrupt(
            &needs,
            &personality,
            &pos,
            &health,
            &disposition,
            &wildlife,
            &d,
            &SimConstants::default().sensory.cat,
        );
        assert!(
            result.is_none(),
            "cat at 50% HP (above 0.4 threshold) should not be interrupted, got {result:?}"
        );
    }

    // -----------------------------------------------------------------------
    // build_crafting_chain hint test
    // -----------------------------------------------------------------------

    #[test]
    fn chain_respects_prepare_hint() {
        use crate::components::magic::{GrowthStage, Harvestable, Herb, HerbKind, Inventory, Ward};
        use crate::components::skills::{MagicAffinity, Skills};
        use crate::resources::map::{Terrain, TileMap};
        use rand_chacha::rand_core::SeedableRng;
        use rand_chacha::ChaCha8Rng;

        let mut world = World::new();

        // Spawn a harvestable herb at (3, 3) — so GatherHerbs *could* fire.
        world.spawn((
            Herb {
                kind: HerbKind::HealingMoss,
                growth_stage: GrowthStage::Bloom,
                magical: false,
                twisted: false,
            },
            Harvestable,
            Position { x: 3, y: 3 },
        ));

        // Spawn injured cat entity before extracting queries (avoids borrow conflict).
        let injured_entity = world.spawn_empty().id();

        type CraftingQueries<'w, 's> = (
            Query<'w, 's, (Entity, &'static Herb, &'static Position), With<Harvestable>>,
            Query<
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
            Query<'w, 's, (&'static Ward, &'static Position)>,
            Query<'w, 's, (Entity, &'static StoredItems)>,
            Query<'w, 's, &'static Item>,
        );
        let mut state: SystemState<CraftingQueries> = SystemState::new(&mut world);
        let (herb_query, building_query, ward_query, _stored_items_query, _items_query) =
            state.get(&world);

        let pos = Position { x: 2, y: 2 }; // close to the herb
        let personality = mid_personality();
        let needs = Needs::default();
        let mut skills = Skills::default();
        skills.herbcraft = 0.5; // above threshold
        let magic_aff = MagicAffinity(0.0);
        let mut inventory = Inventory { slots: Vec::new() };
        inventory.add_herb(HerbKind::HealingMoss); // remedy herb

        // One injured cat nearby for the ApplyRemedy target.
        let injured_cats = vec![(injured_entity, Position { x: 4, y: 4 })];

        let map = TileMap::new(10, 10, Terrain::Grass);
        let d = SimConstants::default().disposition;
        let mut rng = ChaCha8Rng::seed_from_u64(99);
        let _unmet_demand = crate::resources::UnmetDemand::default();

        let fox_scent_map = crate::resources::FoxScentMap::default();
        let cat_presence_map = crate::resources::CatPresenceMap::default();
        let ward_coverage_map = crate::resources::WardCoverageMap::default();
        let placement_maps = crate::systems::coordination::PlacementMaps {
            fox_scent: &fox_scent_map,
            cat_presence: &cat_presence_map,
            ward_coverage: &ward_coverage_map,
            tile_map: &map,
        };
        let result = build_crafting_chain(
            &pos,
            &personality,
            &needs,
            &skills,
            &magic_aff,
            &inventory,
            &herb_query,
            &building_query,
            &ward_query,
            &[],
            &injured_cats,
            None,
            &map,
            &placement_maps,
            false,
            &d,
            &mut rng,
            Some(CraftingHint::PrepareRemedy),
        );

        let (chain, action) = result.expect("should produce a chain with PrepareRemedy hint");
        assert_eq!(action, Action::Herbcraft);

        // The chain should contain a PrepareRemedy step — NOT GatherHerb.
        let has_prepare = chain
            .steps
            .iter()
            .any(|s| matches!(s.kind, StepKind::PrepareRemedy { .. }));
        let has_gather = chain
            .steps
            .iter()
            .any(|s| matches!(s.kind, StepKind::GatherHerb));
        assert!(
            has_prepare,
            "chain should contain PrepareRemedy step; steps: {:?}",
            chain.steps.iter().map(|s| &s.kind).collect::<Vec<_>>()
        );
        assert!(
            !has_gather,
            "chain should NOT contain GatherHerb when hint is PrepareRemedy; steps: {:?}",
            chain.steps.iter().map(|s| &s.kind).collect::<Vec<_>>()
        );
    }
}

/// Deposit cat territorial presence for patrolling/guarding cats and decay
/// the presence map globally. Runs every tick.
pub fn cat_presence_tick(
    cats: Query<(&Position, &CurrentAction), Without<Dead>>,
    mut presence_map: ResMut<crate::resources::CatPresenceMap>,
    constants: Res<SimConstants>,
    time_scale: Res<TimeScale>,
) {
    let fc = &constants.fox_ecology;
    // Global decay — same per-day rate as fox scent decay for territorial-mark symmetry.
    presence_map.decay_all(fc.scent_decay_rate.per_tick(&time_scale));

    // Patrolling/guarding cats deposit presence at their position.
    let deposit = fc.scent_deposit; // reuse fox deposit rate for symmetry
    for (pos, action) in &cats {
        if matches!(
            action.action,
            Action::Patrol | Action::Fight | Action::Explore
        ) {
            presence_map.deposit(pos.x, pos.y, deposit);
        }
    }
}
