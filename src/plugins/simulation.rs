use bevy::prelude::*;

use crate::ai::eval::DseRegistry;
use crate::ai::modifier::default_modifier_pipeline;
use crate::resources::sim_constants::ScoringConstants;
use crate::resources::SimConstants;
use crate::systems;

/// Populates a [`DseRegistry`] with the canonical 30 cat-DSE + 9
/// fox-DSE catalog plus all target-taking DSEs, using the supplied
/// [`ScoringConstants`].
///
/// Single source of truth for DSE catalog membership. Tests that
/// build a `DseRegistry` inline (`tests/integration.rs`) intentionally
/// do *not* call this function — they cherry-pick a subset.
pub fn populate_dse_registry(registry: &mut DseRegistry, scoring: &ScoringConstants) {
    use crate::ai::dses;
    registry.cat_dses.push(dses::eat_dse());
    registry.cat_dses.push(dses::hunt_dse());
    registry.target_taking_dses.push(dses::hunt_target_dse());
    registry.cat_dses.push(dses::forage_dse());
    registry.cat_dses.push(dses::cook_dse());
    registry.cat_dses.push(dses::flee_dse(scoring));
    registry.cat_dses.push(dses::fight_dse(scoring));
    registry.target_taking_dses.push(dses::fight_target_dse());
    registry.cat_dses.push(dses::sleep_dse(scoring));
    registry.cat_dses.push(dses::idle_dse(scoring));
    registry.cat_dses.push(dses::socialize_dse());
    registry
        .target_taking_dses
        .push(dses::socialize_target_dse());
    registry.cat_dses.push(dses::groom_self_dse());
    registry.cat_dses.push(dses::groom_other_dse());
    registry
        .target_taking_dses
        .push(dses::groom_other_target_dse());
    registry.cat_dses.push(dses::mentor_dse());
    registry.target_taking_dses.push(dses::mentor_target_dse());
    registry.cat_dses.push(dses::caretake_dse());
    registry
        .target_taking_dses
        .push(dses::caretake_target_dse());
    registry.cat_dses.push(dses::mate_dse());
    registry.target_taking_dses.push(dses::mate_target_dse());
    registry.cat_dses.push(dses::patrol_dse(scoring));
    registry.cat_dses.push(dses::build_dse(scoring));
    registry.target_taking_dses.push(dses::build_target_dse());
    registry.cat_dses.push(dses::farm_dse());
    registry.cat_dses.push(dses::coordinate_dse(scoring));
    registry.cat_dses.push(dses::explore_dse(scoring));
    registry.cat_dses.push(dses::wander_dse(scoring));
    registry.cat_dses.push(dses::herbcraft_gather_dse());
    registry
        .target_taking_dses
        .push(dses::herbcraft_target_dse());
    registry.cat_dses.push(dses::herbcraft_prepare_dse());
    registry
        .target_taking_dses
        .push(dses::apply_remedy_target_dse());
    registry.cat_dses.push(dses::herbcraft_ward_dse());
    registry.cat_dses.push(dses::scry_dse());
    registry.cat_dses.push(dses::durable_ward_dse());
    registry.cat_dses.push(dses::cleanse_dse(scoring));
    registry.cat_dses.push(dses::colony_cleanse_dse());
    registry.cat_dses.push(dses::harvest_dse());
    registry.cat_dses.push(dses::commune_dse());
    registry.fox_dses.push(dses::fox_patrolling_dse(scoring));
    registry.fox_dses.push(dses::fox_hunting_dse(scoring));
    registry.fox_dses.push(dses::fox_raiding_dse());
    registry.fox_dses.push(dses::fox_fleeing_dse());
    registry.fox_dses.push(dses::fox_avoiding_dse());
    registry.fox_dses.push(dses::fox_den_defense_dse());
    registry.fox_dses.push(dses::fox_resting_dse(scoring));
    registry.fox_dses.push(dses::fox_feeding_dse());
    registry.fox_dses.push(dses::fox_dispersing_dse());
}

/// Startup system that populates [`DseRegistry`] and the §3.5
/// modifier pipeline from live [`SimConstants`]. Runs after
/// `setup_world_exclusive` so SimConstants is in place.
pub fn register_dses_at_startup(
    constants: Res<SimConstants>,
    mut registry: ResMut<DseRegistry>,
    mut commands: Commands,
) {
    let scoring = &constants.scoring;
    populate_dse_registry(&mut registry, scoring);
    // §075 — `default_modifier_pipeline` takes `&SimConstants` so the
    // `CommitmentTenure` modifier can reach `DispositionConstants`
    // (`oscillation_score_lift`).
    commands.insert_resource(default_modifier_pipeline(&constants));
}

/// Registers all simulation systems on `FixedUpdate` in the same order as the
/// original `build_schedule()`.
///
/// Four chained groups run sequentially:
///   1. World simulation (weather, corruption, wildlife, buildings, items)
///   2. Cat needs, mood, and decision-making
///   3. Action resolution
///   4. Social, combat, death, cleanup, narrative
///
/// Standalone systems (AI evaluation, fate, aspirations) run after the chains
/// but are unordered relative to each other.
pub struct SimulationPlugin;

impl Plugin for SimulationPlugin {
    fn build(&self, app: &mut App) {
        // Determinism: pin the simulation schedules to a single-threaded
        // executor. The standalone systems group below is unordered relative
        // to itself, and Bevy's MultiThreadedExecutor picks a topological
        // order that varies across processes when the conflict graph admits
        // alternatives — that shifts the SimRng-consumption sequence and
        // breaks same-seed replay (verified: two seed-42 runs of the same
        // binary diverged at the first SystemActivation tick). Single-
        // threaded execution forces a stable order; the throughput cost is
        // negligible for a ~50-cat headless sim. Pinning Startup as well
        // covers worldgen, even though its current systems are explicitly
        // chained.
        use bevy::ecs::schedule::ExecutorKind;
        app.edit_schedule(Startup, |s| {
            s.set_executor_kind(ExecutorKind::SingleThreaded);
        });
        app.edit_schedule(FixedUpdate, |s| {
            s.set_executor_kind(ExecutorKind::SingleThreaded);
        });

        // World construction — terrain, cats, all sim resources. Owned
        // by the plugin so any host (windowed App, headless App in
        // ticket 030) gets the simulation populated by adding the
        // single plugin. The system reads `AppArgs` (seed, load_path,
        // …) which the host inserts before `add_plugins`.
        app.add_systems(Startup, crate::plugins::setup::setup_world_exclusive);

        // Register personality event observers (cascade handlers).
        systems::personality_events::register_observers(app);

        // Register messages.
        app.add_message::<crate::components::prey::PreyKilled>();
        app.add_message::<crate::components::prey::DenRaided>();
        app.add_message::<crate::components::goap_plan::PlanNarrative>();
        app.add_message::<crate::systems::magic::CorruptionPushback>();

        // L2 substrate resources (§9 faction + §L2.10). FactionRelations
        // is a constant lookup — fine to insert at build time.
        // DseRegistry starts empty; populated by `register_dses_at_startup`
        // (Startup-after-`setup_world_exclusive`) so fox DSEs etc. read
        // live `SimConstants` instead of `ScoringConstants::default()`.
        // The §3.5 modifier pipeline is also built by that Startup
        // system. Single-site registration — eliminates the prior
        // three-mirror burden flagged in CLAUDE.md.
        app.insert_resource(crate::ai::faction::FactionRelations::canonical());
        app.init_resource::<DseRegistry>();
        app.add_systems(
            Startup,
            register_dses_at_startup.after(crate::plugins::setup::setup_world_exclusive),
        );

        // Snapshot positions before any simulation system moves entities.
        // The rendering layer interpolates between PreviousPosition and Position.
        app.add_systems(
            FixedUpdate,
            crate::rendering::entity_sprites::snapshot_previous_positions
                .before(systems::time::advance_time),
        );

        app.add_systems(
            FixedUpdate,
            (
                // Chain 1: World simulation
                (
                    systems::time::advance_time.run_if(systems::time::not_paused),
                    systems::weather::update_weather,
                    systems::wind::update_wind,
                    systems::time::emit_weather_transitions,
                    systems::magic::corruption_spread,
                    // Ward decay → coverage rebuild: rebuild reads
                    // post-decay strength so the L1 `ward_coverage`
                    // map is always one tick fresh.
                    (
                        systems::magic::ward_decay,
                        systems::magic::update_ward_coverage_map,
                    )
                        .chain(),
                    // Herb/flavor growth sub-chain: seasonal check resets stage,
                    // then growth advances, then flavors advance.
                    //
                    // Ticket 061 note — `update_herb_location_map`
                    // (defined in `magic.rs`) is intentionally NOT
                    // scheduled here. Adding it shifts Bevy's
                    // topological sort enough to collapse Hunting and
                    // Foraging dispositions to zero on a seed-42 soak,
                    // matching the `reconsider_held_intentions`
                    // precedent documented at `simulation.rs:425-433`.
                    // The producer is registered separately (along
                    // with the marker cutover and the
                    // `herbcraft_target_dse` consumer wiring) in a
                    // follow-on that absorbs the scheduling shift via
                    // wider verification (likely ticket 052's
                    // spatial-consideration sweep).
                    (
                        systems::magic::herb_seasonal_check,
                        systems::magic::advance_herb_growth,
                        systems::magic::advance_flavor_growth,
                        systems::magic::herb_regrowth,
                    )
                        .chain(),
                    systems::magic::corruption_tile_effects,
                    systems::magic::apply_corruption_pushback,
                    // §L2.10.7 — recompute the territory corruption
                    // centroid after spread + tile effects so AI
                    // consumers (ColonyCleanseDse via
                    // LandmarkAnchor::TerritoryCorruptionCentroid)
                    // read the post-mutation centroid next frame.
                    systems::magic::update_corruption_landmarks,
                    systems::magic::spawn_shadow_fox_from_corruption,
                    (
                        systems::wildlife::spawn_wildlife,
                        systems::wildlife::wildlife_ai,
                        systems::wildlife::fox_movement,
                        systems::wildlife::fox_needs_tick,
                        systems::fox_goap::sync_fox_needs,
                        systems::fox_goap::fox_evaluate_and_plan,
                        systems::fox_goap::fox_resolve_goap_plans,
                        systems::fox_goap::feed_cubs_at_dens,
                        systems::fox_goap::resolve_paired_confrontations,
                        systems::wildlife::fox_ai_decision,
                        systems::wildlife::fox_scent_tick,
                        systems::wildlife::predator_hunt_prey,
                        systems::wildlife::carcass_decay,
                        systems::wildlife::carcass_scent_tick,
                        systems::wildlife::predator_stalk_cats,
                    )
                        .chain(),
                    systems::prey::prey_population,
                    systems::prey::prey_hunger,
                    systems::prey::prey_ai,
                    systems::prey::prey_scent_tick,
                    systems::prey::prey_den_lifecycle,
                    systems::wildlife::detect_threats,
                    // Building-side sub-chain: passive effects, decay,
                    // and the §5.6.3 colony-faction influence-map
                    // writers (ticket 006). Nested to stay under
                    // Bevy's 20-system tuple limit on the outer chain.
                    // Map writers run *after* `decay_building_condition`
                    // so effectiveness gates read post-decay values.
                    (
                        systems::buildings::apply_building_effects,
                        systems::buildings::decay_building_condition,
                        systems::buildings::update_colony_landmarks,
                        systems::buildings::update_food_location_map,
                        systems::buildings::update_garden_location_map,
                        systems::buildings::update_construction_site_map,
                    )
                        .chain(),
                    systems::items::decay_items,
                )
                    .chain(),
                // Item pruning, food sync, den pressure/raids, orphan prey.
                (
                    systems::items::prune_stored_items,
                    systems::items::sync_food_stores,
                    systems::prey::update_den_pressure,
                    systems::prey::apply_den_raids,
                    systems::prey::orphan_prey_adopt_or_found,
                )
                    .chain(),
                // Chain 2: Cat needs, markers, mood, coordination.
                // Split into 2a/2b sub-chains to stay under Bevy's
                // 20-system tuple limit on `.chain()`.
                (
                    // Chain 2a: needs + marker authors + reproduction + growth
                    (
                        systems::needs::decay_needs,
                        // §4 marker authors — run before the GOAP/scoring
                        // pipeline so consumers see freshly-authored
                        // markers. Grouped as a nested sub-tuple to keep
                        // the outer Chain 2a under Bevy's 20-system tuple
                        // limit; sub-chain order matches the dependency
                        // chain (life-stage / injury / inventory /
                        // directive feed into capability + mate
                        // eligibility).
                        (
                            systems::incapacitation::update_incapacitation,
                            systems::growth::update_life_stage_markers,
                            systems::needs::update_injury_marker,
                            systems::items::update_inventory_markers,
                            systems::coordination::update_directive_markers,
                            // §4 batch — Mate eligibility marker. Reads
                            // the full `mating::has_eligible_mate`
                            // predicate (season + sated/happy + fertility
                            // + Partners bond + orientation compat) and
                            // writes `HasEligibleMate`.
                            // `MateDse::eligibility()` requires this
                            // marker, so the DSE returns 0.0 for cats
                            // whose gate is closed.
                            crate::ai::mating::update_mate_eligibility_markers,
                            // §7.M L2 PairingActivity (ticket 027b) —
                            // **activation deferred**. Registering
                            // `crate::ai::pairing::author_pairing_intentions`
                            // here perturbs Bevy 0.18's topological sort
                            // enough to flip seed-42's late-soak food/eat
                            // cadence: the canonical 15-min release deep-
                            // soak with the system active produced 3
                            // starvation deaths (clustered tick 1344K, last
                            // 11% of the run) versus zero pre-027b at the
                            // same parent commit (cef9137). The author
                            // system body is a true no-op when no Friends-
                            // bonded reproductive pair exists, so the
                            // regression must come from scheduler
                            // reshuffling, not from any side effect inside
                            // the system. Deferral mirrors ticket 061's
                            // producer-scaffold precedent ("DSE is
                            // registered but dormant; no production caller
                            // yet"). The substrate is otherwise complete:
                            // PairingActivity component, drop-gate
                            // predicate, bias wiring on
                            // `socialize_target.rs::bond_score`, and
                            // `Feature::Pairing*` activation variants all
                            // remain in place. To activate: re-add the
                            // line below and follow up with the four-
                            // artifact balance methodology against a
                            // multi-seed sweep.
                            //
                            //     crate::ai::pairing::author_pairing_intentions,
                            //
                            // §4 batch 2: capability markers — reads
                            // life-stage, injury, inventory markers
                            // authored above.
                            crate::ai::capabilities::update_capability_markers,
                            // §4.2 State markers — InCombat reads
                            // CurrentAction; OnCorruptedTile and
                            // OnSpecialTerrain read TileMap. Independent
                            // of each other and of the upstream marker
                            // authors, but registered here so the
                            // MarkerSnapshot population in the GOAP /
                            // disposition scoring loops sees them.
                            systems::combat::update_combat_marker,
                            systems::magic::update_corrupted_tile_markers,
                            systems::sensing::update_terrain_markers,
                            // Ticket 014 Mentoring batch — Mentor /
                            // Apprentice authored from `Training`;
                            // HasMentoringTarget from skill-gap
                            // sensing predicate.
                            systems::aspirations::update_training_markers,
                            systems::aspirations::update_mentoring_target_markers,
                            // Ticket 014 Parent marker — active
                            // parenthood authored from
                            // `KittenDependency` references.
                            systems::growth::update_parent_markers,
                            // Ticket 014 §4 sensing batch — broad-phase
                            // target-existence: HasThreatNearby,
                            // HasSocialTarget, HasHerbsNearby, PreyNearby,
                            // CarcassNearby. Single author owns five
                            // markers to amortize the per-cat sensing scans.
                            systems::sensing::update_target_existence_markers,
                            // Ticket 014 §4 fox markers — 7 authors
                            // grouped into a sub-tuple so the outer
                            // chain stays under Bevy's 20-system tuple
                            // limit. Authors are independent of each
                            // other; chain ordering is informational.
                            (
                                systems::fox_spatial::update_store_awareness_markers,
                                systems::fox_spatial::update_den_threat_markers,
                                systems::fox_spatial::update_ward_detection_markers,
                                systems::fox_spatial::update_cub_marker,
                                systems::fox_spatial::update_cub_hunger_markers,
                                systems::fox_spatial::update_juvenile_dispersal_markers,
                                systems::fox_spatial::update_den_marker,
                            )
                                .chain(),
                            // Ticket 049 §9.2 BefriendedAlly author —
                            // toggles the marker on cats and wildlife
                            // when their cross-species familiarity
                            // crosses the threshold (no production
                            // signal source today; runs as a no-op
                            // until trade or a non-hostile-contact
                            // accumulator lands).
                            systems::social::befriend_wildlife,
                        )
                            .chain(),
                        systems::needs::decay_grooming,
                        systems::needs::eat_from_inventory,
                        systems::needs::decay_exploration,
                        systems::needs::stamp_passive_exploration,
                        systems::needs::update_exploration_centroid,
                        systems::needs::bond_proximity_social,
                        systems::fulfillment::decay_fulfillment,
                        systems::fulfillment::bond_proximity_social_warmth,
                        systems::pregnancy::tick_pregnancy,
                        // Fertility transitions (§7.M.7) — run after
                        // tick_pregnancy so `RemovedComponents<Pregnant>`
                        // from the birth path reaches
                        // `handle_post_partum_reinsert` in the same frame.
                        systems::fertility::handle_post_partum_reinsert,
                        systems::fertility::update_fertility_phase,
                        systems::growth::tick_kitten_growth,
                        systems::growth::kitten_mood_aura,
                        // Ticket 006 §5.6.3 row #13 — re-stamp the
                        // kitten-urgency influence map after growth so
                        // matured kittens (KittenDependency removed in
                        // tick_kitten_growth) drop out of the same
                        // frame.
                        systems::growth::update_kitten_urgency_map,
                    )
                        .chain(),
                    // Chain 2b: mood + memory + coordination
                    (
                        systems::mood::update_mood,
                        systems::mood::mood_contagion,
                        systems::mood::bond_proximity_mood,
                        systems::memory::decay_memories,
                        systems::coordination::evaluate_coordinators,
                        systems::coordination::assess_colony_needs,
                        systems::coordination::dispatch_urgent_directives,
                        systems::coordination::accumulate_build_pressure,
                        systems::coordination::spawn_construction_sites,
                    )
                        .chain(),
                )
                    .chain(),
                // Chain 3: Action resolution (disposition system handles all action selection)
                (
                    systems::task_chains::resolve_task_chains,
                    systems::magic::resolve_magic_task_chains,
                    systems::magic::apply_remedy_effects,
                    systems::buildings::process_gates,
                    systems::buildings::tidy_buildings,
                )
                    .chain(),
                // Chain 4: Social, combat, death, cleanup, narrative
                (
                    systems::social::passive_familiarity,
                    systems::personality_friction::personality_friction,
                    systems::social::check_bonds,
                    systems::colony_knowledge::update_colony_knowledge,
                    systems::combat::resolve_combat,
                    systems::combat::heal_injuries,
                    systems::wildlife::fox_lifecycle_tick,
                    systems::wildlife::fox_confrontation_tick,
                    systems::wildlife::fox_store_raid_tick,
                    systems::magic::personal_corruption_effects,
                    systems::death::check_death,
                    systems::coordination::flag_coordinator_death,
                    systems::coordination::expire_directives,
                    systems::death::cleanup_dead,
                    systems::wildlife::cleanup_wildlife,
                    systems::narrative::generate_narrative,
                )
                    .chain(),
            )
                .chain(),
        );

        // GOAP systems — ordered pipeline replacing the old disposition systems.
        // check_anxiety_interrupts → evaluate_and_plan → resolve_goap_plans → emit_plan_narrative.
        //
        // Both check_anxiety_interrupts and evaluate_and_plan must run AFTER
        // sync_food_stores so that food_available reflects the current tick's
        // item state, not a stale default of 0.0.
        app.add_systems(
            FixedUpdate,
            systems::goap::check_anxiety_interrupts.after(systems::items::sync_food_stores),
        );
        // §7.2 commitment gate (Phase 6a) is not a stand-alone system —
        // it's inlined into `resolve_goap_plans`'s per-cat loop
        // prologue via `crate::ai::commitment::{strategy_for_disposition,
        // proxies_for_plan, should_drop_intention, record_drop}`. The
        // 2026-04-23 PM attempt registered a `reconsider_held_intentions`
        // system between `check_anxiety_interrupts` and
        // `evaluate_and_plan`; its schedule presence reshuffled
        // ordering enough to starve the colony (see
        // `docs/open-work.md` #5). The inlined form shifts the gate's
        // effect by one tick (replacement next tick instead of same
        // tick) without new scheduler edges.
        app.add_systems(
            FixedUpdate,
            systems::goap::evaluate_and_plan
                .after(systems::goap::check_anxiety_interrupts)
                .after(systems::items::sync_food_stores),
        );
        // Flush commands so GoapPlan inserted by evaluate_and_plan is
        // visible to resolve_goap_plans in the same tick.
        app.add_systems(
            FixedUpdate,
            bevy::ecs::schedule::ApplyDeferred
                .after(systems::goap::evaluate_and_plan)
                .before(systems::goap::resolve_goap_plans),
        );
        app.add_systems(
            FixedUpdate,
            systems::goap::resolve_goap_plans
                .after(systems::goap::evaluate_and_plan)
                .before(systems::task_chains::resolve_task_chains),
        );
        app.add_systems(
            FixedUpdate,
            systems::goap::emit_plan_narrative.after(systems::goap::resolve_goap_plans),
        );

        // Standalone systems — registered after the chains but unordered
        // relative to each other. These exceed Bevy's chain param limit.
        app.add_systems(
            FixedUpdate,
            (
                systems::disposition::cat_presence_tick.after(systems::goap::resolve_goap_plans),
                systems::personality_events::emit_personality_events,
                systems::ai::emit_periodic_events,
                systems::snapshot::emit_cat_snapshots.after(systems::goap::resolve_goap_plans),
                systems::snapshot::emit_position_traces.after(systems::goap::resolve_goap_plans),
                systems::snapshot::emit_spatial_snapshots,
                systems::colony_score::emit_colony_score,
                systems::fate::assign_fated_connections,
                systems::fate::awaken_fated_connections,
                systems::aspirations::select_aspirations,
                systems::aspirations::check_second_aspiration_slot,
                systems::aspirations::check_aspiration_abandonment,
                systems::aspirations::track_milestones,
            ),
        );

        // §11 trace emitter — headless-only in practice. Gated on
        // FocalTraceTarget + TraceLog resources; neither is inserted by
        // the interactive setup path, so this system never fires outside
        // headless runs that pass --focal-cat. Registered here (not just
        // in build_schedule) to satisfy the manual-mirror invariant in
        // CLAUDE.md's Headless Mode section.
        app.add_systems(
            FixedUpdate,
            systems::trace_emit::emit_focal_trace
                .after(systems::goap::resolve_goap_plans)
                .run_if(bevy_ecs::prelude::resource_exists::<crate::resources::FocalTraceTarget>)
                .run_if(bevy_ecs::prelude::resource_exists::<crate::resources::TraceLog>)
                .run_if(bevy_ecs::prelude::resource_exists::<crate::resources::FocalScoreCapture>),
        );
    }
}
