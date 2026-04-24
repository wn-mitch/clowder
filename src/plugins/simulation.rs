use bevy::prelude::*;

use crate::systems;

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
        // Register personality event observers (cascade handlers).
        systems::personality_events::register_observers(app);

        // Register messages.
        app.add_message::<crate::components::prey::PreyKilled>();
        app.add_message::<crate::components::prey::DenRaided>();
        app.add_message::<crate::components::goap_plan::PlanNarrative>();
        app.add_message::<crate::systems::magic::CorruptionPushback>();

        // L2 substrate resources + DSE registrations (§9 faction + §L2.10).
        // Phase 3b.2 registers the Eat reference DSE; Phase 3c fans out
        // the remaining 29 cat/fox DSEs through the same app-extension
        // trait. Mirrored in `src/main.rs::build_schedule`.
        app.insert_resource(crate::ai::faction::FactionRelations::canonical());
        app.init_resource::<crate::ai::eval::DseRegistry>();
        // §3.5 modifier pipeline — populated with the three Phase 4.2
        // corruption-response emergency bonuses. Uses default scoring
        // constants at plugin build time because `SimConstants` is
        // inserted later at Startup; the live values are re-bound in
        // the headless and save-load mirror sites per the same pattern
        // as fox DSEs.
        app.insert_resource(crate::ai::modifier::default_modifier_pipeline(
            &crate::resources::sim_constants::ScoringConstants::default(),
        ));
        {
            use crate::ai::eval::DseRegistryAppExt;
            // Plugin build runs before `setup_world_exclusive` (which
            // inserts SimConstants at the Startup schedule), so fox
            // DSEs that depend on tunable knot values read from a
            // default ScoringConstants here. The other three mirror
            // sites (headless, save-load, integration tests) register
            // after SimConstants is inserted and pass the live values.
            let default_scoring =
                crate::resources::sim_constants::ScoringConstants::default();
            app.add_dse(crate::ai::dses::eat_dse())
                .add_dse(crate::ai::dses::hunt_dse())
                .add_target_taking_dse(crate::ai::dses::hunt_target_dse())
                .add_dse(crate::ai::dses::forage_dse())
                .add_dse(crate::ai::dses::cook_dse())
                .add_dse(crate::ai::dses::flee_dse(&default_scoring))
                .add_dse(crate::ai::dses::fight_dse(&default_scoring))
                .add_target_taking_dse(crate::ai::dses::fight_target_dse())
                .add_dse(crate::ai::dses::sleep_dse(&default_scoring))
                .add_dse(crate::ai::dses::idle_dse(&default_scoring))
                .add_dse(crate::ai::dses::socialize_dse())
                .add_target_taking_dse(crate::ai::dses::socialize_target_dse())
                .add_dse(crate::ai::dses::groom_self_dse())
                .add_dse(crate::ai::dses::groom_other_dse())
                .add_target_taking_dse(crate::ai::dses::groom_other_target_dse())
                .add_dse(crate::ai::dses::mentor_dse())
                .add_target_taking_dse(crate::ai::dses::mentor_target_dse())
                .add_dse(crate::ai::dses::caretake_dse())
                .add_target_taking_dse(crate::ai::dses::caretake_target_dse())
                .add_dse(crate::ai::dses::mate_dse())
                .add_target_taking_dse(crate::ai::dses::mate_target_dse())
                .add_dse(crate::ai::dses::patrol_dse(&default_scoring))
                .add_dse(crate::ai::dses::build_dse(&default_scoring))
                .add_target_taking_dse(crate::ai::dses::build_target_dse())
                .add_dse(crate::ai::dses::farm_dse())
                .add_dse(crate::ai::dses::coordinate_dse(&default_scoring))
                .add_dse(crate::ai::dses::explore_dse())
                .add_dse(crate::ai::dses::wander_dse(&default_scoring))
                .add_dse(crate::ai::dses::herbcraft_gather_dse())
                .add_dse(crate::ai::dses::herbcraft_prepare_dse())
                .add_target_taking_dse(crate::ai::dses::apply_remedy_target_dse())
                .add_dse(crate::ai::dses::herbcraft_ward_dse())
                .add_dse(crate::ai::dses::scry_dse())
                .add_dse(crate::ai::dses::durable_ward_dse())
                .add_dse(crate::ai::dses::cleanse_dse(&default_scoring))
                .add_dse(crate::ai::dses::colony_cleanse_dse())
                .add_dse(crate::ai::dses::harvest_dse())
                .add_dse(crate::ai::dses::commune_dse())
                .add_fox_dse(crate::ai::dses::fox_patrolling_dse(&default_scoring))
                .add_fox_dse(crate::ai::dses::fox_hunting_dse(&default_scoring))
                .add_fox_dse(crate::ai::dses::fox_raiding_dse())
                .add_fox_dse(crate::ai::dses::fox_fleeing_dse())
                .add_fox_dse(crate::ai::dses::fox_avoiding_dse())
                .add_fox_dse(crate::ai::dses::fox_den_defense_dse())
                .add_fox_dse(crate::ai::dses::fox_resting_dse(&default_scoring))
                .add_fox_dse(crate::ai::dses::fox_feeding_dse())
                .add_fox_dse(crate::ai::dses::fox_dispersing_dse());
        }

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
                    systems::magic::ward_decay,
                    // Herb/flavor growth sub-chain: seasonal check resets stage,
                    // then growth advances, then flavors advance.
                    (
                        systems::magic::herb_seasonal_check,
                        systems::magic::advance_herb_growth,
                        systems::magic::advance_flavor_growth,
                        systems::magic::herb_regrowth,
                    )
                        .chain(),
                    systems::magic::corruption_tile_effects,
                    systems::magic::apply_corruption_pushback,
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
                        systems::wildlife::predator_stalk_cats,
                    )
                        .chain(),
                    systems::prey::prey_population,
                    systems::prey::prey_hunger,
                    systems::prey::prey_ai,
                    systems::prey::prey_scent_tick,
                    systems::prey::prey_den_lifecycle,
                    systems::wildlife::detect_threats,
                    systems::buildings::apply_building_effects,
                    systems::buildings::decay_building_condition,
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
                        // §4.3 marker authors — run before the GOAP/scoring
                        // pipeline so consumers see freshly-authored markers.
                        systems::incapacitation::update_incapacitation,
                        systems::growth::update_life_stage_markers,
                        systems::needs::decay_grooming,
                        systems::needs::eat_from_inventory,
                        systems::needs::decay_exploration,
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
                .run_if(bevy_ecs::prelude::resource_exists::<
                    crate::resources::FocalTraceTarget,
                >)
                .run_if(bevy_ecs::prelude::resource_exists::<
                    crate::resources::TraceLog,
                >)
                .run_if(bevy_ecs::prelude::resource_exists::<
                    crate::resources::FocalScoreCapture,
                >),
        );
    }
}
