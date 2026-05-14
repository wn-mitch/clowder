use bevy::prelude::*;

use crate::ai::eval::DseRegistry;
use crate::ai::modifier::default_modifier_pipeline;
use crate::resources::sim_constants::ScoringConstants;
use crate::resources::SimConstants;
use crate::systems;
use crate::systems::influence_map::{
    CorruptionLens, InfluenceMap, InfluenceMapRegistry, PerSpeciesScentRef,
};

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
    registry.cat_dses.push(dses::hunt_dse(scoring));
    registry.target_taking_dses.push(dses::hunt_target_dse(scoring));
    registry.cat_dses.push(dses::forage_dse(scoring));
    registry.cat_dses.push(dses::cook_dse());
    registry.cat_dses.push(dses::flee_dse(scoring));
    registry.cat_dses.push(dses::fight_dse(scoring));
    // Ticket 104 — Hide/Freeze DSE. Phase 1 ships dormant: gated
    // behind the `HideEligible` marker which has no authoring system,
    // so it's never eligible. Awakens alongside the lift activation
    // in modifiers 105 (`AcuteHealthAdrenalineFreeze`) and 142
    // (`IntraspeciesConflictResponseFreeze`) in a future commit.
    registry.cat_dses.push(dses::hide_dse());
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
    // 035: Bury (self-state + target-taking pair). Gated by the
    // `HasUnburiedCorpse` substrate marker, so the DSE pair is
    // dormant for cats with no nearby unburied corpse.
    registry.cat_dses.push(dses::bury_dse());
    registry.target_taking_dses.push(dses::bury_target_dse());
    registry.cat_dses.push(dses::mentor_dse(scoring));
    registry.target_taking_dses.push(dses::mentor_target_dse());
    registry.cat_dses.push(dses::caretake_dse(scoring));
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
    registry.cat_dses.push(dses::herbcraft_ward_dse(scoring));
    registry.cat_dses.push(dses::scry_dse());
    registry.cat_dses.push(dses::durable_ward_dse());
    registry.cat_dses.push(dses::cleanse_dse(scoring));
    registry.cat_dses.push(dses::colony_cleanse_dse());
    registry.cat_dses.push(dses::harvest_dse());
    registry.cat_dses.push(dses::commune_dse());
    // 176: inventory-disposal DSEs ship dormant via default-zero
    // scoring (Linear slope=0, intercept=0). Registration plumbs
    // them through L2 / L3 / planner so the substrate is exercised
    // by the existing canaries (categorization, never-fired, etc.)
    // while the elections stay zero. Balance-tuning replaces the
    // zero curves with real overflow / colony-food considerations
    // in a follow-on once `ColonyStoresChronicallyFull` and the
    // saturation surfaces land.
    registry.cat_dses.push(dses::discarding_dse(scoring));
    registry.cat_dses.push(dses::trashing_dse(scoring));
    registry.cat_dses.push(dses::handing_dse(scoring));
    registry.cat_dses.push(dses::picking_up_dse());
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

/// Single source of truth for L1 trace coverage (ticket 207).
///
/// Every `impl InfluenceMap for X` in `src/` registers here; the
/// `emit_focal_trace` exclusive system walks `InfluenceMapRegistry`
/// blindly, so a missing entry silently drops a map from the focal
/// scrubber's L1 surface. `scripts/check_influence_map_registry.sh`
/// pairs this site against the trait-impl set at `just check` time
/// to catch the regression.
///
/// Resource-backed maps register via `register::<M>()`. Borrow-adapter
/// maps (`CorruptionLens` over `&TileMap`) register via
/// `register_with`; the closure constructs the adapter inline at
/// walk time so no wrapper Resource is needed. Per-species
/// adapters (ticket 062's `PerSpeciesScentRef`) follow the same
/// pattern — one `register_with` per species.
pub fn populate_influence_map_registry(registry: &mut InfluenceMapRegistry) {
    // Note (228): per-cat substrate (`RouteCostField`,
    // `escape_viability`, `fox_scent_level` at cat position) is
    // **not** registered here — this registry is world-keyed only.
    // Cat-keyed perception lives outside the registry; see
    // `src/components/route_cost_field.rs` for the cat-keyed family
    // and §4.7 of `docs/systems/ai-substrate-refactor.md` for the
    // substrate-vs-search-state boundary.
    use crate::resources::{
        CarcassScentMap, CatPatrolDeterrentMap, CatScentMap, ConstructionSiteMap,
        ExplorationMap, FoodLocationMap, FoxApproachCorridorMap, FoxScentMap,
        GardenLocationMap, GraveAuraMap, HerbLocationMap, KittenCryMap, PreyScentMaps,
        RecentAmbushMap, TileMap, WardCoverageMap, WardIntentMap,
    };

    registry.register::<FoxScentMap>();
    // Ticket 062 — per-species prey scent. Five `PerSpeciesScentRef`
    // borrow-adapters over `PreyScentMaps`, one per `PreyKind`. The
    // aggregate `PreyScentMap` `Resource` is retired; `PreyScentMaps`
    // itself is **not** registered (no aggregate `InfluenceMap` impl).
    for kind in [
        crate::components::prey::PreyKind::Mouse,
        crate::components::prey::PreyKind::Rat,
        crate::components::prey::PreyKind::Rabbit,
        crate::components::prey::PreyKind::Fish,
        crate::components::prey::PreyKind::Bird,
    ] {
        registry.register_with(move |world, pos| {
            world.get_resource::<PreyScentMaps>().map(|maps| {
                let adapter = PerSpeciesScentRef(maps.for_kind(kind), kind);
                (adapter.metadata(), adapter.base_sample(pos))
            })
        });
    }
    registry.register::<CarcassScentMap>();
    // 219: colony-shared recent-ambush event memory. Dormant in
    // scoring at land (no DSE reads it yet); registered so its samples
    // surface in `trace-*.jsonl` for soak-trace verification.
    registry.register::<RecentAmbushMap>();
    // 312: fox-approach corridor traffic map. Dormant in scoring at
    // land (`ward_fox_approach_corridor_weight = 0.0`); registered so
    // its samples surface in `trace-*.jsonl` for soak-trace
    // verification at first-light activation.
    registry.register::<FoxApproachCorridorMap>();
    registry.register::<CatScentMap>();
    // 256 R5: cat patrol deterrent — read by fox A* as routing cost.
    registry.register::<CatPatrolDeterrentMap>();
    registry.register::<ExplorationMap>();
    registry.register::<WardCoverageMap>();
    // 301: coordinator-stamped ward-placement intent. Substrate is
    // dormant at default `SimConstants` (semantics is
    // `SingleShotArgmax` so the populator short-circuits, and the
    // Path-B DSE weight is 0.0). Registered so the field surfaces in
    // `trace-*.jsonl` for soak-trace verification once activated.
    registry.register::<WardIntentMap>();
    registry.register::<FoodLocationMap>();
    registry.register::<GardenLocationMap>();
    registry.register::<ConstructionSiteMap>();
    registry.register::<KittenCryMap>();
    registry.register::<HerbLocationMap>();
    // 035: anti-corruption aura around buried graves.
    registry.register::<GraveAuraMap>();

    // CorruptionLens is a borrow adapter over TileMap.corruption — not
    // a Resource itself, so it can't go through the generic
    // `register::<M>()`. The closure builds the lens inline.
    registry.register_with(|world, pos| {
        world.get_resource::<TileMap>().map(|t| {
            let lens = CorruptionLens(t);
            (lens.metadata(), lens.base_sample(pos))
        })
    });
}

/// Startup system that populates [`InfluenceMapRegistry`]. Independent
/// of `register_dses_at_startup` — registration only touches the
/// registry, not other Resources, so it can run any time after
/// `setup_world_exclusive` inserts the resources the walkers will
/// later look up.
pub fn register_influence_maps_at_startup(mut registry: ResMut<InfluenceMapRegistry>) {
    populate_influence_map_registry(&mut registry);
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
        // Ticket 127 Commit B — bias-reader call sites emit this when
        // their resolver target matches the actor's
        // `JointIntention { Courtship }.partner`. Consumed by
        // `author_joint_intentions` to bump `last_interaction_tick`.
        app.add_message::<crate::ai::joint_intention::JointInteractionObserved>();
        // 258 — observable side-effects consumed by `belief_integrator` to
        // update per-cat mental models. Action resolvers emit variants at
        // completion; the integrator finds witnesses by sensing-range query.
        app.add_message::<crate::messages::witnessable_event::WitnessableEvent>();
        // 050 — fox-lifecycle mechanical events (no observer-side
        // semantics). Consumed by `fox_spatial`'s §4 marker authors so
        // `HasDen` / `HasCubs` author from event signals instead of
        // (purely) per-tick scans. Emitted at fox-spawn / den-claim /
        // cub-birth / den-loss sites in `wildlife.rs`.
        app.add_message::<crate::messages::fox_lifecycle::DenClaimed>();
        app.add_message::<crate::messages::fox_lifecycle::DenLost>();
        app.add_message::<crate::messages::fox_lifecycle::CubsBorn>();

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
        // Ticket 207 — InfluenceMapRegistry replaces the hand-bundled
        // `L1Maps` SystemParam in `trace_emit.rs`. Empty at build time;
        // populated by `register_influence_maps_at_startup` below.
        app.init_resource::<InfluenceMapRegistry>();
        // 176: chronicity tracker for `ColonyStoresChronicallyFull`.
        // Updated by `update_colony_building_markers` once per
        // `ScoringConstants::chronicity_window_ticks` ticks.
        app.init_resource::<crate::resources::stores_pressure::StoresPressureTracker>();
        app.add_systems(
            Startup,
            register_dses_at_startup.after(crate::plugins::setup::setup_world_exclusive),
        );
        // Registry registration is independent of resource setup —
        // walkers look up resources at *call* time, not at registration
        // time — so this can run alongside DSE registration without an
        // ordering constraint on `setup_world_exclusive`.
        app.add_systems(Startup, register_influence_maps_at_startup);

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
                        // Ticket 023 Phase A — coherence tick must run
                        // before `wildlife_ai` so a dissolving shadow-fox
                        // gets despawned (well, queued) before downstream
                        // shadowfox-bearing systems take decisions. Lives
                        // inside the existing wildlife `.chain()` block
                        // to avoid creating a new top-level schedule edge
                        // (ticket 061 precedent).
                        systems::wildlife::shadowfox_coherence_tick,
                        // Ticket 023 Phase B — motivation tick re-elects
                        // each shadow-fox's WildlifeAiState every
                        // `shadow_fox_motivation_tick_cadence` ticks
                        // (default 16). Runs after coherence so a
                        // shadow-fox that dissolves this tick won't be
                        // assigned a state it can't act on, and before
                        // `wildlife_ai` so the new state takes effect
                        // immediately.
                        systems::wildlife::shadowfox_motivation_tick,
                        // Ticket 023 Phase C — haunting-drain runs every
                        // tick to apply per-tick mood/safety drain on
                        // nearby cats and to tick the haunting-to-stalk
                        // escalation counter. Runs after motivation_tick
                        // (which writes the Haunting state) and before
                        // wildlife_ai (which executes the orbit-at-edge
                        // movement).
                        systems::wildlife::shadowfox_haunting_drain,
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
                        // 312: corridor-traffic populator + decay,
                        // scheduled alongside `fox_scent_tick` inside
                        // the existing wildlife `.chain()` block to
                        // avoid creating a new top-level schedule edge
                        // (ticket 061 precedent — adding an unordered
                        // sibling perturbs Bevy's topological sort and
                        // collapsed Hunting/Foraging on seed-42).
                        systems::wildlife::update_fox_approach_corridor_map,
                        systems::wildlife::update_recent_ambush_map,
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
                    // 308 — ground-truth colony reserves aggregator
                    // (thornbriar / remedy-herb counts across cat
                    // inventories + Stores buildings). Sibling to
                    // sync_food_stores; runs each tick so downstream
                    // observability and balance canaries can compare
                    // ground truth against per-cat
                    // ColonyReservesBelief.
                    systems::items::sync_colony_reserves,
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
                            // Ticket 087 — interoceptive perception.
                            // Authors LowHealth / SevereInjury /
                            // BodyDistressed from Health + Needs. Runs
                            // adjacent to update_injury_marker (same
                            // data sources, different markers); both
                            // run before the GOAP/scoring pipeline so
                            // DSE eligibility filters see fresh state.
                            systems::interoception::author_self_markers,
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
                            // Ticket 127 — JointIntention author with
                            // matchmaker + drop predicate + stage
                            // progression + cascade detection +
                            // mismatch tracking. Subsumes the prior
                            // §7.M L2 PairingActivity author (tickets
                            // 027b / 082 / 083 / 257). The substrate
                            // shift was activated post-Wave-2 hardening
                            // and post-272 mating-cadence stabilization
                            // so the food-economy lift (pair-socializing
                            // bias raising median food_fraction) stays
                            // in band; Farm dormancy under abundant
                            // food remains intended per ticket 084.
                            crate::ai::joint_intention::author_joint_intentions,
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
                        // Ticket 080 — clear `Reserved` markers whose
                        // `expires_tick` has lapsed.
                        crate::systems::plan_substrate::expire_reservations,
                        // Ticket 073 — bound per-cat `RecentTargetFailures`
                        // map size by expiring entries older than
                        // `target_failure_cooldown_ticks`.
                        systems::plan_substrate::sensors::prune_recent_target_failures,
                        // Ticket 123 — bound per-cat `RecentDispositionFailures`
                        // map size by expiring entries older than
                        // `disposition_failure_cooldown_ticks`.
                        systems::plan_substrate::sensors::prune_recent_disposition_failures,
                        systems::needs::eat_from_inventory,
                        systems::needs::decay_exploration,
                        systems::needs::stamp_passive_exploration,
                        systems::needs::update_exploration_centroid,
                        systems::needs::bond_proximity_social,
                        systems::fulfillment::decay_fulfillment,
                        systems::fulfillment::bond_proximity_social_warmth,
                        systems::fulfillment::update_body_condition,
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
                        // kitten-cry influence map after growth so
                        // matured kittens (KittenDependency removed in
                        // tick_kitten_growth) drop out of the same
                        // frame. Ticket 156 repurposed the map from
                        // Sight to Hearing channel.
                        //
                        // Ticket 161: this system also authors
                        // `IsParentOfHungryKitten` (merged from a
                        // separate Chain 2a author). Both subsystems
                        // share the same `&Needs` access on kittens
                        // and the same hunger-threshold predicate, so
                        // co-locating them avoids adding a new
                        // schedule conflict edge to Bevy's parallel
                        // scheduler — ticket 158's standalone author
                        // shifted the seed-42 trajectory at tick
                        // 1201300 by introducing such an edge.
                        systems::growth::update_kitten_cry_map,
                    )
                        .chain(),
                    // Chain 2b: mood + memory + coordination
                    (
                        systems::mood::update_mood,
                        systems::mood::mood_contagion,
                        systems::mood::bond_proximity_mood,
                        systems::memory::decay_memories,
                        // 308 — broadcast each cat's inventory snapshot
                        // on its stagger tick. Writes
                        // `WitnessableEvent::InventoryObserved` so
                        // `integrate_beliefs` Pass A can consume the
                        // event in the same tick (writer → reader
                        // within-tick is valid when writer is chained
                        // before reader).
                        systems::belief_integrator::gossip_inventory_observations,
                        // 258 — C3 belief substrate integrator. Pass A
                        // consumes WitnessableEvent messages → EMA updates
                        // on per-cat mental models; pass B implants species
                        // priors for nearby predators and decays facets
                        // toward priors on each cat's stagger tick.
                        systems::belief_integrator::integrate_beliefs,
                        // 261 — ActionAffordances substrate writer. Reads
                        // facets the integrator authored this tick (within
                        // a single `.chain()` block so the ordering is
                        // strict). Lands behavior-neutral: the resource
                        // populates but no DSE reads from it. Folded into
                        // Chain 2b (not a new top-level sibling) per the
                        // schedule-edge perturbation memory — adding a
                        // sibling can reshuffle Bevy's topological sort
                        // and perturb seed-42 on unrelated systems.
                        systems::affordance_writer::affordance_writer,
                        // 308 — author per-cat `HasLowWardReserve` from
                        // the just-updated `ColonyReservesBelief`. Runs
                        // after `integrate_beliefs` so the marker
                        // reflects same-tick belief state.
                        systems::items::update_low_ward_reserve_markers,
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
                    // 035: rebuild the grave-aura InfluenceMap from
                    // live `Grave` entities each tick. Lives in the
                    // late-tick batch alongside `cleanup_dead` and
                    // `cleanup_wildlife` because graves are spawned
                    // by `resolve_goap_plans`'s post-loop drain
                    // earlier in the tick — the rebuild must run
                    // after all spawns so the next tick's L1 trace
                    // sees the freshly-spawned aura.
                    systems::death::update_grave_aura_map,
                    systems::wildlife::cleanup_wildlife,
                    systems::narrative::generate_narrative,
                )
                    .chain(),
            )
                .chain(),
        );

        // GOAP systems — ordered pipeline replacing the old disposition systems.
        // check_modifier_preemption → evaluate_and_plan →
        // resolve_goap_plans → emit_plan_narrative.
        //
        // `check_modifier_preemption` and `evaluate_and_plan` must run
        // AFTER sync_food_stores so that food_available reflects the
        // current tick's item state, not a stale default of 0.0.
        //
        // Ticket 230 — the legacy `check_anxiety_interrupts` system
        // and its lone surviving `ThreatDetected` arm are retired.
        // Tickets 106/107/108/119 retired the four sibling arms in
        // favor of substrate-driven modifiers; 230 replaces the last
        // arm with `DispositionKind::Fleeing` (plan template
        // `[PickFleeTarget, Flee, HoldUntilSafe]`, commitment-aware
        // modifier guard via the disposition-tier early-skip in
        // `try_preempt_with_modifier_lurch`). The substrate-driven
        // preempt path (`check_modifier_preemption`) is now the sole
        // tier-1-acute interrupt surface.
        app.add_systems(
            FixedUpdate,
            systems::goap::check_modifier_preemption.after(systems::items::sync_food_stores),
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
        // Ticket 168 — colony-marker author chain. Runs after
        // sync_food_stores (so HasStoredFood reflects the current tick's
        // food state) and before evaluate_and_plan (so the snapshot
        // population reads up-to-date markers). Chained among themselves
        // for deterministic ordering — the same `reconsider_held_intentions`
        // schedule-edge perturbation that bit the 2026-04-23 attempt
        // (see comment at line 492 above) is the reason these are
        // sequentially chained rather than registered as siblings.
        app.add_systems(
            FixedUpdate,
            (
                systems::buildings::update_colony_building_markers,
                systems::magic::update_herb_availability_markers,
                systems::magic::update_ward_coverage_markers,
                systems::magic::update_ward_siege_marker,
            )
                .chain()
                .after(systems::items::sync_food_stores)
                .before(systems::goap::evaluate_and_plan),
        );
        // Flush the singleton `.insert()/.remove()` writes so
        // evaluate_and_plan's `Has<MarkerN>` reads see them within the
        // same tick.
        app.add_systems(
            FixedUpdate,
            bevy::ecs::schedule::ApplyDeferred
                .after(systems::magic::update_ward_siege_marker)
                .before(systems::goap::evaluate_and_plan),
        );
        app.add_systems(
            FixedUpdate,
            systems::goap::evaluate_and_plan
                .after(systems::goap::check_modifier_preemption)
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
        // Ticket 108 — write back current `safety_deficit` to
        // `PrevSafetyDeficit` *after* the scoring pass so next tick's
        // `evaluate_and_plan` / `evaluate_dispositions` see last tick's
        // value as `prev` and compute a non-zero rising-derivative
        // when safety drops over the tick boundary. If this ran
        // before scoring, the derivative would always be zero.
        app.add_systems(
            FixedUpdate,
            systems::plan_substrate::update_prev_safety_deficit
                .after(systems::goap::evaluate_and_plan)
                .after(systems::goap::resolve_goap_plans),
        );

        // Standalone systems — registered after the chains but unordered
        // relative to each other. These exceed Bevy's chain param limit.
        app.add_systems(
            FixedUpdate,
            (
                systems::disposition::cat_scent_tick.after(systems::goap::resolve_goap_plans),
                // 256 R5 — runs alongside cat_scent_tick (both
                // depend on resolve_goap_plans having set the cat's
                // current_action for this tick). Fox AI in the same
                // schedule (further down) consumes the deterrent
                // map in its A* call.
                systems::disposition::cat_patrol_deterrent_tick
                    .after(systems::goap::resolve_goap_plans),
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
