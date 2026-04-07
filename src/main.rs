use std::io::{self, BufRead, Write as _};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;

use clowder::ai::CurrentAction;
use clowder::components::identity::{Age, Appearance, Name, Species};
use clowder::components::mental::{Memory, Mood};
use clowder::components::physical::{Dead, Health, Needs, Position};
use clowder::components::magic::Inventory;
use clowder::components::skills::{Corruption, MagicAffinity, Training};
use clowder::persistence;
use clowder::resources::{
    EventLog, FoodStores, NarrativeLog, NarrativeTier, Relationships, SimConfig, SimRng,
    TemplateRegistry, TimeState, TileMap, WeatherState,
};
use clowder::resources::time::DayPhase;
use clowder::tui::{AppView, FocusMode};
use clowder::world_gen::colony::{find_colony_site, generate_starting_cats, spawn_starting_buildings};
use clowder::world_gen::terrain::generate_terrain;

/// Parsed CLI arguments.
struct CliArgs {
    seed: u64,
    load_path: Option<PathBuf>,
    headless: bool,
    duration_secs: u64,
    log_path: Option<PathBuf>,
    load_log_path: Option<PathBuf>,
    event_log_path: Option<PathBuf>,
}

fn main() -> io::Result<()> {
    let args = parse_args();

    if args.headless {
        return run_headless(args);
    }

    // -----------------------------------------------------------------------
    // Setup terminal
    // -----------------------------------------------------------------------
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    // Run the simulation and restore terminal even on error.
    let result = run(&mut terminal, args);

    // -----------------------------------------------------------------------
    // Cleanup terminal
    // -----------------------------------------------------------------------
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn parse_args() -> CliArgs {
    let args: Vec<String> = std::env::args().collect();
    let mut seed = 42u64;
    let mut load_path = None;
    let mut headless = false;
    let mut duration_secs = 60u64;
    let mut log_path = None;
    let mut load_log_path = None;
    let mut event_log_path = None;
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--seed" => {
                if let Some(val) = iter.next() {
                    if let Ok(n) = val.parse::<u64>() {
                        seed = n;
                    }
                }
            }
            "--load" => {
                if let Some(path) = iter.next() {
                    load_path = Some(PathBuf::from(path));
                }
            }
            "--headless" => {
                headless = true;
            }
            "--duration" => {
                if let Some(val) = iter.next() {
                    if let Ok(n) = val.parse::<u64>() {
                        duration_secs = n;
                    }
                }
            }
            "--log" => {
                if let Some(path) = iter.next() {
                    log_path = Some(PathBuf::from(path));
                }
            }
            "--load-log" => {
                if let Some(path) = iter.next() {
                    load_log_path = Some(PathBuf::from(path));
                }
            }
            "--event-log" => {
                if let Some(path) = iter.next() {
                    event_log_path = Some(PathBuf::from(path));
                }
            }
            _ => {}
        }
    }

    if !headless && duration_secs != 60 {
        eprintln!("Warning: --duration has no effect without --headless");
    }

    CliArgs {
        seed,
        load_path,
        headless,
        duration_secs,
        log_path,
        load_log_path,
        event_log_path,
    }
}

const SAVE_PATH: &str = "saves/autosave.json";

fn build_schedule() -> Schedule {
    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            // World simulation
            (
                clowder::systems::time::advance_time
                    .run_if(clowder::systems::time::not_paused),
                clowder::systems::weather::update_weather,
                clowder::systems::time::emit_weather_transitions,
                clowder::systems::magic::corruption_spread,
                clowder::systems::magic::ward_decay,
                clowder::systems::magic::herb_seasonal_check,
                clowder::systems::magic::corruption_tile_effects,
                clowder::systems::magic::spawn_shadow_fox_from_corruption,
                clowder::systems::wildlife::spawn_wildlife,
                clowder::systems::wildlife::wildlife_ai,
                clowder::systems::wildlife::predator_hunt_prey,
                clowder::systems::prey::prey_population,
                clowder::systems::prey::prey_hunger,
                clowder::systems::wildlife::detect_threats,
                clowder::systems::buildings::apply_building_effects,
                clowder::systems::buildings::decay_building_condition,
                clowder::systems::items::decay_items,
                clowder::systems::items::sync_food_stores,
            )
                .chain(),
            // Cat needs, mood, and decision-making
            (
                clowder::systems::needs::decay_needs,
                clowder::systems::mood::update_mood,
                clowder::systems::mood::mood_contagion,
                clowder::systems::memory::decay_memories,
                clowder::systems::coordination::evaluate_coordinators,
                clowder::systems::coordination::assess_colony_needs,
            )
                .chain(),
            // Action resolution (evaluate_actions added separately — exceeds chain param limit)
            (
                clowder::systems::task_chains::resolve_task_chains,
                clowder::systems::magic::resolve_magic_task_chains,
                clowder::systems::actions::resolve_actions,
                clowder::systems::magic::apply_remedy_effects,
                clowder::systems::buildings::process_gates,
                clowder::systems::buildings::tidy_buildings,
            )
                .chain(),
            // Social, combat, death, cleanup, narrative
            (
                clowder::systems::social::passive_familiarity,
                clowder::systems::social::check_bonds,
                clowder::systems::colony_knowledge::update_colony_knowledge,
                clowder::systems::combat::resolve_combat,
                clowder::systems::combat::heal_injuries,
                clowder::systems::magic::personal_corruption_effects,
                clowder::systems::death::check_death,
                clowder::systems::coordination::flag_coordinator_death,
                clowder::systems::coordination::expire_directives,
                clowder::systems::death::cleanup_dead,
                clowder::systems::wildlife::cleanup_wildlife,
                clowder::systems::narrative::generate_narrative,
            )
                .chain(),
        )
            .chain(),
    );
    // These systems exceed Bevy's chain param limit — register separately.
    schedule.add_systems(clowder::systems::ai::evaluate_actions);
    schedule.add_systems(clowder::systems::ai::emit_periodic_events);
    schedule.add_systems(clowder::systems::snapshot::emit_cat_snapshots);
    // Fate and aspiration lifecycle.
    schedule.add_systems(clowder::systems::fate::assign_fated_connections);
    schedule.add_systems(clowder::systems::fate::awaken_fated_connections);
    schedule.add_systems(clowder::systems::aspirations::select_aspirations);
    schedule.add_systems(clowder::systems::aspirations::check_second_aspiration_slot);
    schedule.add_systems(clowder::systems::aspirations::check_aspiration_abandonment);
    schedule.add_systems(clowder::systems::aspirations::track_milestones);
    schedule
}

fn setup_world(args: &CliArgs) -> io::Result<World> {
    let mut world = if let Some(ref load_path) = args.load_path {
        let save = persistence::load_from_file(load_path)?;
        let mut w = World::new();
        persistence::load_world(&mut w, save);
        w
    } else {
        build_new_world(args.seed)?
    };

    load_templates(&mut world);
    load_zodiac_data(&mut world);
    load_aspiration_data(&mut world);

    if args.load_path.is_none() {
        let current_tick = world.resource::<clowder::resources::TimeState>().tick;
        let mut log = world.resource_mut::<NarrativeLog>();
        log.push(
            current_tick,
            "A small group of cats settles in a clearing.".to_string(),
            NarrativeTier::Significant,
        );
    }

    if let Some(ref path) = args.load_log_path {
        load_log_file(&mut world, path)?;
    }

    // Always insert the event log for mechanical debugging.
    world.insert_resource(EventLog::default());

    // Ensure new resources exist (may be absent from older saves).
    if !world.contains_resource::<clowder::resources::ColonyKnowledge>() {
        world.insert_resource(clowder::resources::ColonyKnowledge::default());
    }
    if !world.contains_resource::<clowder::resources::ColonyPriority>() {
        world.insert_resource(clowder::resources::ColonyPriority::default());
    }
    if !world.contains_resource::<clowder::systems::wildlife::DetectionCooldowns>() {
        world.insert_resource(clowder::systems::wildlife::DetectionCooldowns::default());
    }

    Ok(world)
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, args: CliArgs) -> io::Result<()> {
    let mut world = setup_world(&args)?;
    let mut schedule = build_schedule();

    // -----------------------------------------------------------------------
    // Main loop
    // -----------------------------------------------------------------------
    const FRAME_DURATION: Duration = Duration::from_millis(33); // ~30 fps
    let mut tick_accumulator = Duration::ZERO;
    let mut last_frame = Instant::now();
    let mut focus_mode = FocusMode::None;

    loop {
        let now = Instant::now();
        let elapsed = now - last_frame;
        last_frame = now;

        // --- Handle input (non-blocking) ------------------------------------
        if event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match &focus_mode {
                        FocusMode::None => match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => break,
                            KeyCode::Char('p') => {
                                let mut time = world.resource_mut::<TimeState>();
                                time.paused = !time.paused;
                            }
                            KeyCode::Char('s') => {
                                let mut time = world.resource_mut::<TimeState>();
                                time.speed = time.speed.cycle();
                            }
                            KeyCode::Char('f') | KeyCode::Char('F') => {
                                // Enter selecting mode: collect living cats.
                                let cats: Vec<(Entity, String)> = world
                                    .query_filtered::<(Entity, &Name), bevy_ecs::query::Without<clowder::components::physical::Dead>>()
                                    .iter(&world)
                                    .map(|(e, n)| (e, n.0.clone()))
                                    .collect();
                                if !cats.is_empty() {
                                    focus_mode = FocusMode::Selecting { cats, index: 0 };
                                }
                            }
                            KeyCode::Char('P') => {
                                let mut priority = world.resource_mut::<clowder::resources::ColonyPriority>();
                                priority.active = clowder::resources::PriorityKind::cycle(priority.active);
                            }
                            KeyCode::Char('z') | KeyCode::Char('Z') => {
                                let map = world.resource::<TileMap>();
                                let cursor = Position::new(map.width / 2, map.height / 2);
                                focus_mode = FocusMode::ZoneDesignate {
                                    cursor,
                                    kind: clowder::components::zone::ZoneKind::BuildHere,
                                };
                            }
                            KeyCode::Char('i') | KeyCode::Char('I') => {
                                let map = world.resource::<TileMap>();
                                let cursor = Position::new(map.width / 2, map.height / 2);
                                focus_mode = FocusMode::TileInspect { cursor };
                            }
                            _ => {}
                        },
                        FocusMode::Selecting { cats, index } => match key.code {
                            KeyCode::Up => {
                                let new_index = if *index == 0 { cats.len() - 1 } else { index - 1 };
                                focus_mode = FocusMode::Selecting {
                                    cats: cats.clone(),
                                    index: new_index,
                                };
                            }
                            KeyCode::Down => {
                                let new_index = (index + 1) % cats.len();
                                focus_mode = FocusMode::Selecting {
                                    cats: cats.clone(),
                                    index: new_index,
                                };
                            }
                            KeyCode::Enter => {
                                let entity = cats[*index].0;
                                focus_mode = FocusMode::Inspecting(entity);
                            }
                            KeyCode::Esc | KeyCode::Char('f') | KeyCode::Char('F') => {
                                focus_mode = FocusMode::None;
                            }
                            _ => {}
                        },
                        FocusMode::Inspecting(_) => match key.code {
                            KeyCode::Esc | KeyCode::Char('f') | KeyCode::Char('F') => {
                                focus_mode = FocusMode::None;
                            }
                            KeyCode::Char('q') => break,
                            _ => {}
                        },
                        FocusMode::TileInspect { cursor } => {
                            let map = world.resource::<TileMap>();
                            let mut c = *cursor;
                            match key.code {
                                KeyCode::Up => c.y = (c.y - 1).max(0),
                                KeyCode::Down => c.y = (c.y + 1).min(map.height - 1),
                                KeyCode::Left => c.x = (c.x - 1).max(0),
                                KeyCode::Right => c.x = (c.x + 1).min(map.width - 1),
                                KeyCode::Esc | KeyCode::Char('i') | KeyCode::Char('I') => {
                                    focus_mode = FocusMode::None;
                                    continue;
                                }
                                KeyCode::Char('q') => break,
                                _ => {}
                            }
                            focus_mode = FocusMode::TileInspect { cursor: c };
                        },
                        FocusMode::ZoneDesignate { cursor, kind } => {
                            let map = world.resource::<TileMap>();
                            let mut c = *cursor;
                            let mut k = *kind;
                            match key.code {
                                KeyCode::Up => c.y = (c.y - 1).max(0),
                                KeyCode::Down => c.y = (c.y + 1).min(map.height - 1),
                                KeyCode::Left => c.x = (c.x - 1).max(0),
                                KeyCode::Right => c.x = (c.x + 1).min(map.width - 1),
                                KeyCode::Tab => k = k.cycle(),
                                KeyCode::Enter => {
                                    // Place zone marker entity.
                                    world.spawn((
                                        clowder::components::zone::Zone { kind: k },
                                        c,
                                    ));
                                    focus_mode = FocusMode::None;
                                    continue;
                                }
                                KeyCode::Esc | KeyCode::Char('z') | KeyCode::Char('Z') => {
                                    focus_mode = FocusMode::None;
                                    continue;
                                }
                                KeyCode::Char('q') => break,
                                _ => {}
                            }
                            focus_mode = FocusMode::ZoneDesignate { cursor: c, kind: k };
                        },
                    }
                }
            }
        }

        // --- Tick simulation (time-accumulator based) -----------------------
        let ticks_this_frame = {
            let time = world.resource::<TimeState>();
            if time.paused {
                0
            } else {
                tick_accumulator += elapsed;
                let tick_interval = Duration::from_secs_f64(1.0 / time.speed.ticks_per_second());
                let mut count = 0u32;
                // Cap at 10 ticks per frame to prevent spiral of death
                while tick_accumulator >= tick_interval && count < 10 {
                    tick_accumulator -= tick_interval;
                    count += 1;
                }
                count
            }
        };
        for _ in 0..ticks_this_frame {
            schedule.run(&mut world);
        }

        // --- Build inspect data if needed -----------------------------------
        let inspect_data = if let FocusMode::Inspecting(entity) = focus_mode {
            use clowder::components::identity::Age;
            use clowder::components::mental::Mood;
            use clowder::components::skills::Skills;
            use clowder::components::coordination::{ActiveDirective, Coordinator, DirectiveKind};
            use clowder::components::zodiac::ZodiacSign;
            use clowder::components::fate::{FatedLove, FatedRival};
            use clowder::components::aspirations::{Aspirations, Preference, Preferences};
            use clowder::tui::inspect::{build_inspect_data, AspirationDisplay, RelationshipEntry};

            let config = world.resource::<SimConfig>();
            let tick = world.resource::<TimeState>().tick;
            let relationships = world.resource::<Relationships>();
            let aspiration_registry = world.get_resource::<clowder::resources::AspirationRegistry>();

            // Try to get the focused entity's components.
            let data = world.get::<Name>(entity).and_then(|name| {
                let age = world.get::<Age>(entity)?;
                let needs = world.get::<Needs>(entity)?;
                let mood = world.get::<Mood>(entity)?;
                let current = world.get::<CurrentAction>(entity)?;
                let skills = world.get::<Skills>(entity)?;

                let is_coordinator = world.get::<Coordinator>(entity).is_some();
                let active_directive = world.get::<ActiveDirective>(entity).map(|ad| {
                    let kind_str = match ad.kind {
                        DirectiveKind::Hunt => "Hunt",
                        DirectiveKind::Forage => "Forage",
                        DirectiveKind::Build => "Build",
                        DirectiveKind::Fight => "Fight",
                        DirectiveKind::Patrol => "Patrol",
                        DirectiveKind::Herbcraft => "Herbcraft",
                        DirectiveKind::SetWard => "Set Ward",
                    };
                    let coord_name = world
                        .get::<Name>(ad.coordinator)
                        .map_or("unknown".to_string(), |n| n.0.clone());
                    format!("{kind_str} (by {coord_name})")
                });

                // Build top 3 relationships by fondness.
                let mut rels: Vec<RelationshipEntry> = relationships
                    .all_for(entity)
                    .into_iter()
                    .filter_map(|(other, rel)| {
                        world.get::<Name>(other).map(|n| RelationshipEntry {
                            name: n.0.clone(),
                            fondness: rel.fondness,
                            bond: rel.bond,
                        })
                    })
                    .collect();
                rels.sort_by(|a, b| b.fondness.partial_cmp(&a.fondness).unwrap_or(std::cmp::Ordering::Equal));
                rels.truncate(3);

                // Phase 9 data.
                let zodiac = world.get::<ZodiacSign>(entity).map(|z| z.label().to_string());

                let fated_love = world.get::<FatedLove>(entity).and_then(|fl| {
                    world.get::<Name>(fl.partner).map(|n| (n.0.clone(), fl.awakened))
                });
                let fated_rival = world.get::<FatedRival>(entity).and_then(|fr| {
                    world.get::<Name>(fr.rival).map(|n| (n.0.clone(), fr.awakened))
                });

                let (asp_display, completed_asp) = world.get::<Aspirations>(entity)
                    .map(|asp| {
                        let display: Vec<AspirationDisplay> = asp.active.iter().map(|a| {
                            let (milestone_name, target) = aspiration_registry
                                .and_then(|reg| reg.chain_by_name(&a.chain_name))
                                .and_then(|chain| chain.milestones.get(a.current_milestone))
                                .map(|m| {
                                    let t = match &m.condition {
                                        clowder::components::aspirations::MilestoneCondition::ActionCount { count, .. } => *count,
                                        clowder::components::aspirations::MilestoneCondition::Mentor { count } => *count,
                                        _ => 1,
                                    };
                                    (m.name.clone(), t)
                                })
                                .unwrap_or_else(|| ("???".to_string(), 1));
                            AspirationDisplay {
                                chain_name: a.chain_name.clone(),
                                milestone_name,
                                progress: a.progress,
                                target,
                            }
                        }).collect();
                        (display, asp.completed.clone())
                    })
                    .unwrap_or_default();

                let (likes, dislikes) = world.get::<Preferences>(entity)
                    .map(|prefs| {
                        let l: Vec<String> = prefs.action_preferences.iter()
                            .filter(|(_, p)| *p == Preference::Like)
                            .map(|(a, _)| format!("{a:?}"))
                            .collect();
                        let d: Vec<String> = prefs.action_preferences.iter()
                            .filter(|(_, p)| *p == Preference::Dislike)
                            .map(|(a, _)| format!("{a:?}"))
                            .collect();
                        (l, d)
                    })
                    .unwrap_or_default();

                Some(build_inspect_data(
                    &name.0,
                    age.stage(tick, config.ticks_per_season),
                    needs,
                    mood.valence,
                    current,
                    skills,
                    rels,
                    is_coordinator,
                    active_directive,
                    zodiac,
                    fated_love,
                    fated_rival,
                    asp_display,
                    completed_asp,
                    likes,
                    dislikes,
                ))
            });

            // If entity is dead/despawned, drop out of inspect mode.
            if data.is_none() {
                focus_mode = FocusMode::None;
            }
            data
        } else {
            None
        };

        // --- Render TUI -----------------------------------------------------
        terminal.draw(|frame| {
            // Collect cat display data from ECS.
            use clowder::tui::map::CatDisplay;
            let time_snap = world.resource::<TimeState>();
            let config_snap = world.resource::<SimConfig>();
            let tick = time_snap.tick;
            let tps = config_snap.ticks_per_season;
            let cat_positions: Vec<CatDisplay> = world
                .query::<(&Name, &Position, &Age, &Appearance, Option<&Dead>)>()
                .iter(&world)
                .map(|(name, pos, age, appearance, dead)| CatDisplay {
                    name: name.0.clone(),
                    pos: *pos,
                    life_stage: age.stage(tick, tps),
                    fur_color: appearance.fur_color.clone(),
                    is_dead: dead.is_some(),
                })
                .collect();

            // Collect wildlife positions with behavior state.
            use clowder::components::wildlife::{WildAnimal, WildSpecies, WildlifeAiState};
            use clowder::tui::map::WildlifeBehavior;
            let wildlife_positions: Vec<(WildSpecies, Position, WildlifeBehavior)> = world
                .query::<(&WildAnimal, &Position, &WildlifeAiState)>()
                .iter(&world)
                .map(|(animal, pos, ai)| {
                    let behavior = match ai {
                        WildlifeAiState::Patrolling { .. } | WildlifeAiState::Circling { .. } => {
                            WildlifeBehavior::Roaming
                        }
                        WildlifeAiState::Waiting => WildlifeBehavior::Ambushing,
                        WildlifeAiState::Fleeing { .. } => WildlifeBehavior::Fleeing,
                    };
                    (animal.species, *pos, behavior)
                })
                .collect();

            // Collect ward and herb positions for TUI rendering.
            use clowder::components::magic::{Ward, Herb, Harvestable};
            use clowder::tui::map::{WardDisplay, HerbDisplay};
            let ward_positions: Vec<WardDisplay> = world
                .query::<(&Ward, &Position)>()
                .iter(&world)
                .map(|(ward, pos)| WardDisplay { pos: *pos, inverted: ward.inverted })
                .collect();
            let herb_positions: Vec<HerbDisplay> = world
                .query::<(&Herb, &Position, Option<&Harvestable>)>()
                .iter(&world)
                .filter(|(_, _, h)| h.is_some()) // only show harvestable herbs
                .map(|(_, pos, _)| HerbDisplay { pos: *pos })
                .collect();

            // Collect zone markers for map overlay.
            use clowder::tui::map::ZoneDisplay;
            let zone_positions: Vec<ZoneDisplay> = world
                .query::<(&clowder::components::zone::Zone, &Position)>()
                .iter(&world)
                .map(|(z, pos)| ZoneDisplay { pos: *pos, kind: z.kind })
                .collect();

            // Query building at cursor for tile inspect display (before resource borrows).
            let building_at_cursor = if let FocusMode::TileInspect { cursor } = &focus_mode {
                use clowder::components::building::{Structure, ConstructionSite, CropState, GateState};
                use clowder::tui::tile_inspect::BuildingInfo;

                world
                    .query::<(&Structure, &Position, Option<&ConstructionSite>, Option<&CropState>, Option<&GateState>)>()
                    .iter(&world)
                    .find(|(_, pos, _, _, _)| pos.x == cursor.x && pos.y == cursor.y)
                    .map(|(s, _, site, crop, gate)| BuildingInfo {
                        structure: s.clone(),
                        construction_site: site.cloned(),
                        crop_state: crop.cloned(),
                        gate_state: gate.cloned(),
                    })
            } else {
                None
            };

            // Collect coordinator names for status bar.
            use clowder::components::coordination::Coordinator;
            let coordinator_names: Vec<String> = world
                .query_filtered::<&Name, bevy_ecs::query::With<Coordinator>>()
                .iter(&world)
                .map(|name| name.0.clone())
                .collect();

            // Compute colony vitals from ECS queries.
            use clowder::tui::log::ColonyVitals;
            use clowder::components::building::Structure;

            let vitals = {
                let mut mood_sum = 0.0_f32;
                let mut health_sum = 0.0_f32;
                let mut safety_sum = 0.0_f32;
                let mut corruption_sum = 0.0_f32;
                let mut cat_n = 0_usize;
                let mut any_corruption = false;

                for (mood, health, needs, corruption) in world
                    .query_filtered::<(&Mood, &Health, &Needs, Option<&Corruption>), bevy_ecs::query::Without<Dead>>()
                    .iter(&world)
                {
                    // Map mood valence from [-1, 1] to [0, 1]
                    mood_sum += (mood.valence + 1.0) / 2.0;
                    health_sum += health.current;
                    safety_sum += needs.safety;
                    if let Some(c) = corruption {
                        if c.0 > 0.0 {
                            any_corruption = true;
                        }
                        corruption_sum += c.0;
                    }
                    cat_n += 1;
                }

                let mut bldg_condition_sum = 0.0_f32;
                let mut bldg_n = 0_usize;
                for structure in world
                    .query::<&Structure>()
                    .iter(&world)
                {
                    bldg_condition_sum += structure.condition;
                    bldg_n += 1;
                }

                let n = cat_n.max(1) as f32;
                ColonyVitals {
                    avg_mood: mood_sum / n,
                    avg_health: health_sum / n,
                    avg_safety: safety_sum / n,
                    avg_bldg_condition: if bldg_n > 0 { Some(bldg_condition_sum / bldg_n as f32) } else { None },
                    avg_bldg_cleanliness: None, // TODO: add when cleanliness is actively used
                    avg_corruption: if any_corruption { Some(corruption_sum / n) } else { None },
                }
            };

            // Compute activity counts for status bar.
            use std::collections::HashMap;
            let mut action_map: HashMap<clowder::ai::Action, usize> = HashMap::new();
            for action in world
                .query_filtered::<&CurrentAction, bevy_ecs::query::Without<Dead>>()
                .iter(&world)
            {
                *action_map.entry(action.action).or_default() += 1;
            }
            let mut activity_counts: Vec<(clowder::ai::Action, usize)> = action_map.into_iter().collect();
            activity_counts.sort_by(|a, b| b.1.cmp(&a.1));

            let map = world.resource::<TileMap>();
            let narrative = world.resource::<NarrativeLog>();
            let time = world.resource::<TimeState>();
            let config = world.resource::<SimConfig>();
            let weather = world.resource::<WeatherState>();
            let food = world.resource::<FoodStores>();
            let cat_count = cat_positions.len();

            let view = AppView {
                map,
                cat_positions,
                wildlife_positions,
                ward_positions,
                herb_positions,
                zone_positions,
                narrative,
                time,
                config,
                weather,
                food,
                cat_count,
                focus: &focus_mode,
                inspect_data: inspect_data.as_ref(),
                building_at_cursor,
                coordinator_names,
                priority: world.get_resource::<clowder::resources::ColonyPriority>()
                    .and_then(|cp| cp.active),
                vitals,
                activity_counts,
            };
            view.render(frame);
        })?;

        // --- Frame timing ---------------------------------------------------
        let frame_elapsed = now.elapsed();
        if frame_elapsed < FRAME_DURATION {
            std::thread::sleep(FRAME_DURATION - frame_elapsed);
        }
    }

    // -----------------------------------------------------------------------
    // Autosave on quit
    // -----------------------------------------------------------------------
    let save_path = std::path::Path::new(SAVE_PATH);
    if let Err(e) = persistence::save_to_file(&mut world, save_path) {
        eprintln!("Warning: failed to autosave: {e}");
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Headless mode
// ---------------------------------------------------------------------------

fn run_headless(args: CliArgs) -> io::Result<()> {
    let mut world = setup_world(&args)?;
    let mut schedule = build_schedule();

    // Ensure the simulation is unpaused.
    {
        let mut time = world.resource_mut::<TimeState>();
        time.paused = false;
    }

    // Resolve log paths.
    let log_path = args.log_path.unwrap_or_else(|| PathBuf::from("logs/narrative.jsonl"));
    let event_log_path = args.event_log_path.unwrap_or_else(|| PathBuf::from("logs/events.jsonl"));
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if let Some(parent) = event_log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut file = std::fs::File::create(&log_path)?;
    let mut event_file = std::fs::File::create(&event_log_path)?;
    writeln!(
        file,
        "{}",
        serde_json::json!({"_header": true, "seed": args.seed, "duration_secs": args.duration_secs})
    )?;
    writeln!(
        event_file,
        "{}",
        serde_json::json!({"_header": true, "seed": args.seed, "duration_secs": args.duration_secs})
    )?;

    let duration = Duration::from_secs(args.duration_secs);
    let start = Instant::now();
    let mut ticks: u64 = 0;
    let mut last_flushed: u64 = 0;
    let mut last_events_flushed: u64 = 0;

    // Flush any entries already present (e.g. the initial narrative entry).
    flush_new_entries(&world, &mut file, &mut last_flushed)?;

    while start.elapsed() < duration {
        schedule.run(&mut world);
        ticks += 1;
        flush_new_entries(&world, &mut file, &mut last_flushed)?;
        flush_event_entries(&world, &mut event_file, &mut last_events_flushed)?;

        if ticks.is_multiple_of(1000) {
            let elapsed = start.elapsed().as_secs();
            let time = world.resource::<TimeState>();
            let config = world.resource::<SimConfig>();
            eprint!(
                "\r  [{elapsed}s] tick {} (sim day {})    ",
                time.tick,
                TimeState::day_number(time.tick, config),
            );
        }
    }

    // Final flush.
    flush_new_entries(&world, &mut file, &mut last_flushed)?;
    flush_event_entries(&world, &mut event_file, &mut last_events_flushed)?;

    let time = world.resource::<TimeState>();
    let config = world.resource::<SimConfig>();
    eprintln!(
        "\nHeadless complete: {} schedule runs, sim day {}, {} narrative / {} event entries",
        ticks,
        TimeState::day_number(time.tick, config),
        last_flushed,
        last_events_flushed,
    );
    eprintln!("  narrative → {}", log_path.display());
    eprintln!("  events    → {}", event_log_path.display());

    // Autosave.
    let save_path = std::path::Path::new(SAVE_PATH);
    if let Err(e) = persistence::save_to_file(&mut world, save_path) {
        eprintln!("Warning: failed to autosave: {e}");
    }

    Ok(())
}

fn flush_new_entries(
    world: &World,
    file: &mut std::fs::File,
    last_flushed: &mut u64,
) -> io::Result<()> {
    let log = world.resource::<NarrativeLog>();
    let config = world.resource::<SimConfig>();
    let new_count = log.total_pushed.saturating_sub(*last_flushed);
    if new_count == 0 {
        return Ok(());
    }
    let start = log.entries.len().saturating_sub(new_count as usize);
    for entry in log.entries.range(start..) {
        let day = TimeState::day_number(entry.tick, config);
        let phase = DayPhase::from_tick(entry.tick, config);
        let tier_label = match entry.tier {
            NarrativeTier::Micro => "Micro",
            NarrativeTier::Action => "Action",
            NarrativeTier::Significant => "Significant",
        };
        writeln!(
            file,
            "{}",
            serde_json::json!({
                "tick": entry.tick,
                "day": day,
                "phase": phase.label(),
                "tier": tier_label,
                "text": entry.text,
            })
        )?;
    }
    *last_flushed = log.total_pushed;
    Ok(())
}

fn flush_event_entries(
    world: &World,
    file: &mut std::fs::File,
    last_flushed: &mut u64,
) -> io::Result<()> {
    let log = world.resource::<EventLog>();
    let new_count = log.total_pushed.saturating_sub(*last_flushed);
    if new_count == 0 {
        return Ok(());
    }
    let start = log.entries.len().saturating_sub(new_count as usize);
    for entry in log.entries.range(start..) {
        writeln!(
            file,
            "{}",
            serde_json::to_string(entry).unwrap_or_default()
        )?;
    }
    *last_flushed = log.total_pushed;
    Ok(())
}

fn load_log_file(world: &mut World, path: &std::path::Path) -> io::Result<()> {
    let file = std::fs::File::open(path)?;
    let reader = io::BufReader::new(file);
    let mut loaded = 0u64;
    for line in reader.lines() {
        let line = line?;
        let v: serde_json::Value = serde_json::from_str(&line).map_err(|e| {
            io::Error::new(io::ErrorKind::InvalidData, format!("bad JSON in log: {e}"))
        })?;
        // Skip header lines.
        if v.get("_header").is_some() {
            continue;
        }
        let tick = v["tick"].as_u64().unwrap_or(0);
        let text = v["text"].as_str().unwrap_or("").to_string();
        let tier = match v["tier"].as_str().unwrap_or("Action") {
            "Micro" => NarrativeTier::Micro,
            "Significant" => NarrativeTier::Significant,
            _ => NarrativeTier::Action,
        };
        let mut log = world.resource_mut::<NarrativeLog>();
        log.push(tick, text, tier);
        loaded += 1;
    }
    eprintln!("Loaded {loaded} log entries from {}", path.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// World construction helpers
// ---------------------------------------------------------------------------

fn build_new_world(seed: u64) -> io::Result<World> {
    let config = SimConfig {
        seed,
        ..SimConfig::default()
    };
    let mut sim_rng = SimRng::new(seed);

    // Generate terrain.
    let mut map = generate_terrain(80, 60, &mut sim_rng.rng);

    // Set initial corruption and mystery on special tiles.
    clowder::world_gen::herbs::initialize_tile_magic(&mut map, &mut sim_rng.rng);

    // Find colony site.
    let colony_site = find_colony_site(&map, &mut sim_rng.rng);

    // Start the clock high enough that cats can have varied ages.
    let start_tick: u64 = 100_000;

    let cat_blueprints = generate_starting_cats(
        8,
        start_tick,
        config.ticks_per_season,
        &mut sim_rng.rng,
    );

    // Build ECS world.
    let mut world = World::new();

    // Spawn starting buildings (sets terrain tiles and creates entities).
    spawn_starting_buildings(&mut world, colony_site, &mut map);

    world.insert_resource(TimeState {
        tick: start_tick,
        paused: false,
        speed: clowder::resources::SimSpeed::Normal,
    });
    world.insert_resource(config);
    world.insert_resource(WeatherState::default());
    world.insert_resource(clowder::resources::time::TransitionTracker::default());
    world.insert_resource(NarrativeLog::default());
    world.insert_resource(clowder::resources::ColonyKnowledge::default());
    world.insert_resource(clowder::resources::ColonyPriority::default());
    world.insert_resource(FoodStores::default());
    world.insert_resource(clowder::systems::wildlife::DetectionCooldowns::default());
    world.insert_resource(map);
    world.insert_resource(sim_rng);

    // Spawn cats.
    let mut entity_ids: Vec<Entity> = Vec::with_capacity(8);
    for (i, cat) in cat_blueprints.into_iter().enumerate() {
        let offset_x = (i as i32 % 5) - 2;
        let offset_y = (i as i32 / 5) - 1;

        let (spawn_x, spawn_y) = {
            let map_ref = world.resource::<TileMap>();
            (
                (colony_site.x + offset_x).clamp(0, map_ref.width - 1),
                (colony_site.y + offset_y).clamp(0, map_ref.height - 1),
            )
        };

        let entity = world.spawn((
            (
                Name(cat.name),
                Species,
                Age { born_tick: cat.born_tick },
                cat.gender,
                cat.orientation,
                cat.personality,
                cat.appearance,
                Position::new(spawn_x, spawn_y),
                Health::default(),
                Needs::default(),
                Mood::default(),
                Memory::default(),
            ),
            (
                cat.zodiac_sign,
                cat.skills,
                MagicAffinity(cat.magic_affinity),
                Corruption(0.0),
                Training::default(),
                CurrentAction::default(),
                Inventory::default(),
            ),
        )).id();
        entity_ids.push(entity);
    }

    // Initialize relationships between all pairs.
    {
        let mut relationships = Relationships::default();
        let mut rng = world.resource_mut::<SimRng>();
        for i in 0..entity_ids.len() {
            for j in (i + 1)..entity_ids.len() {
                relationships.init_pair(entity_ids[i], entity_ids[j], &mut rng.rng);
            }
        }
        world.insert_resource(relationships);
    }

    // Spawn initial wildlife far from the colony.
    clowder::systems::wildlife::spawn_initial_wildlife(&mut world, colony_site);

    // Spawn initial prey animals across their habitats.
    clowder::systems::prey::spawn_initial_prey(&mut world);

    // Spawn herbs based on terrain and current season.
    let current_season = {
        let time = world.resource::<TimeState>();
        let config = world.resource::<SimConfig>();
        time.season(config)
    };
    clowder::world_gen::herbs::spawn_herbs(&mut world, current_season);

    Ok(world)
}

fn load_aspiration_data(world: &mut World) {
    let path = std::path::Path::new("assets/narrative/aspirations");
    match clowder::resources::AspirationRegistry::load_from_dir(path) {
        Ok(registry) => {
            world.insert_resource(registry);
        }
        Err(e) => {
            eprintln!("Warning: failed to load aspiration data: {e}");
        }
    }
}

fn load_zodiac_data(world: &mut World) {
    let path = std::path::Path::new("assets/data/zodiac.ron");
    match clowder::resources::ZodiacData::load(path) {
        Ok(data) => {
            world.insert_resource(data);
        }
        Err(e) => {
            eprintln!("Warning: failed to load zodiac data: {e}");
        }
    }
}

fn load_templates(world: &mut World) {
    let template_path = std::path::Path::new("assets/narrative");
    match TemplateRegistry::load_from_dir(template_path) {
        Ok(registry) => {
            world.insert_resource(registry);
        }
        Err(e) => {
            eprintln!("Warning: failed to load narrative templates: {e}");
        }
    }
}
