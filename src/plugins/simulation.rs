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

        app.add_systems(
            FixedUpdate,
            (
                // Chain 1: World simulation
                (
                    systems::time::advance_time
                        .run_if(systems::time::not_paused),
                    systems::weather::update_weather,
                    systems::wind::update_wind,
                    systems::time::emit_weather_transitions,
                    systems::magic::corruption_spread,
                    systems::magic::ward_decay,
                    systems::magic::herb_seasonal_check,
                    systems::magic::corruption_tile_effects,
                    systems::magic::spawn_shadow_fox_from_corruption,
                    systems::wildlife::spawn_wildlife,
                    systems::wildlife::wildlife_ai,
                    systems::wildlife::predator_hunt_prey,
                    systems::prey::prey_population,
                    systems::prey::prey_hunger,
                    systems::prey::prey_ai,
                    systems::wildlife::detect_threats,
                    systems::buildings::apply_building_effects,
                    systems::buildings::decay_building_condition,
                    systems::items::decay_items,
                )
                    .chain(),
                // Item pruning and food sync (split to stay under chain param limit).
                (
                    systems::items::prune_stored_items,
                    systems::items::sync_food_stores,
                )
                    .chain(),
                // Chain 2: Cat needs, mood, decision-making
                (
                    systems::needs::decay_needs,
                    systems::mood::update_mood,
                    systems::mood::mood_contagion,
                    systems::memory::decay_memories,
                    systems::coordination::evaluate_coordinators,
                    systems::coordination::assess_colony_needs,
                    systems::coordination::accumulate_build_pressure,
                )
                    .chain(),
                // Chain 3: Action resolution
                (
                    systems::task_chains::resolve_task_chains,
                    systems::magic::resolve_magic_task_chains,
                    systems::actions::resolve_actions,
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

        // Disposition systems — ordered relative to each other but standalone.
        // check_anxiety_interrupts → evaluate_dispositions → disposition_to_chain
        // → resolve_disposition_chains. These query disjoint entity sets by
        // component filter (With/Without<Disposition>) so they can't be chained
        // via Bevy's tuple-chain due to static analysis limitations.
        app.add_systems(
            FixedUpdate,
            systems::disposition::check_anxiety_interrupts,
        );
        app.add_systems(
            FixedUpdate,
            systems::disposition::evaluate_dispositions
                .after(systems::disposition::check_anxiety_interrupts),
        );
        // Flush commands so Disposition inserted by evaluate_dispositions is
        // visible to disposition_to_chain (and evaluate_actions) in the same tick.
        app.add_systems(
            FixedUpdate,
            bevy::ecs::schedule::ApplyDeferred
                .after(systems::disposition::evaluate_dispositions)
                .before(systems::disposition::disposition_to_chain),
        );
        app.add_systems(
            FixedUpdate,
            systems::disposition::disposition_to_chain
                .after(systems::disposition::evaluate_dispositions),
        );
        // Flush commands so TaskChain inserted by disposition_to_chain is
        // visible to resolve_disposition_chains in the same tick.
        app.add_systems(
            FixedUpdate,
            bevy::ecs::schedule::ApplyDeferred
                .after(systems::disposition::disposition_to_chain)
                .before(systems::disposition::resolve_disposition_chains),
        );
        app.add_systems(
            FixedUpdate,
            systems::disposition::resolve_disposition_chains
                .after(systems::disposition::disposition_to_chain)
                .before(systems::task_chains::resolve_task_chains),
        );

        // Standalone systems — registered after the chains but unordered
        // relative to each other. These exceed Bevy's chain param limit.
        app.add_systems(
            FixedUpdate,
            (
                systems::ai::evaluate_actions
                    .after(systems::disposition::disposition_to_chain),
                systems::personality_events::emit_personality_events,
                systems::ai::emit_periodic_events,
                systems::snapshot::emit_cat_snapshots
                    .after(systems::actions::resolve_actions),
                systems::snapshot::emit_position_traces
                    .after(systems::actions::resolve_actions),
                systems::fate::assign_fated_connections,
                systems::fate::awaken_fated_connections,
                systems::aspirations::select_aspirations,
                systems::aspirations::check_second_aspiration_slot,
                systems::aspirations::check_aspiration_abandonment,
                systems::aspirations::track_milestones,
            ),
        );
    }
}
