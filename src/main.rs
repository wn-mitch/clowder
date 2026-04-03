use std::io;
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
use clowder::components::identity::{Age, Name, Species};
use clowder::components::mental::{Memory, Mood};
use clowder::components::physical::{Health, Needs, Position};
use clowder::components::skills::{Corruption, MagicAffinity, Training};
use clowder::resources::{
    NarrativeLog, NarrativeTier, SimConfig, SimRng, TimeState, TileMap, Terrain, WeatherState,
};
use clowder::tui::AppView;
use clowder::world_gen::colony::{find_colony_site, generate_starting_cats};
use clowder::world_gen::terrain::generate_terrain;

fn main() -> io::Result<()> {
    // -----------------------------------------------------------------------
    // Parse seed from CLI args
    // -----------------------------------------------------------------------
    let seed = parse_seed_arg();

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
    let result = run(&mut terminal, seed);

    // -----------------------------------------------------------------------
    // Cleanup terminal
    // -----------------------------------------------------------------------
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn parse_seed_arg() -> u64 {
    let args: Vec<String> = std::env::args().collect();
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        if arg == "--seed" {
            if let Some(val) = iter.next() {
                if let Ok(n) = val.parse::<u64>() {
                    return n;
                }
            }
        }
    }
    42
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, seed: u64) -> io::Result<()> {
    // -----------------------------------------------------------------------
    // Setup ECS world
    // -----------------------------------------------------------------------
    let config = SimConfig {
        seed,
        ..SimConfig::default()
    };
    let mut sim_rng = SimRng::new(seed);

    // Generate terrain.
    let mut map = generate_terrain(80, 60, &mut sim_rng.rng);

    // Find colony site.
    let colony_site = find_colony_site(&map, &mut sim_rng.rng);

    // Place starting structures around the colony centre.
    map.set(colony_site.x, colony_site.y, Terrain::Hearth);
    // Den 2 tiles to the left (clamp to map bounds).
    let den_x = (colony_site.x - 2).max(0);
    map.set(den_x, colony_site.y, Terrain::Den);
    // Stores 2 tiles to the right.
    let stores_x = (colony_site.x + 2).min(map.width - 1);
    map.set(stores_x, colony_site.y, Terrain::Stores);

    // Generate cat blueprints before moving map into the world.
    let cat_blueprints = generate_starting_cats(8, &mut sim_rng.rng);

    // Build ECS world.
    let mut world = World::new();
    world.insert_resource(TimeState::default());
    world.insert_resource(config);
    world.insert_resource(WeatherState::default());
    world.insert_resource(NarrativeLog::default());
    world.insert_resource(map);
    world.insert_resource(sim_rng);

    // -----------------------------------------------------------------------
    // Spawn cats
    // -----------------------------------------------------------------------
    for (i, cat) in cat_blueprints.into_iter().enumerate() {
        let offset_x = (i as i32 % 5) - 2; // -2, -1, 0, 1, 2
        let offset_y = (i as i32 / 5) - 1; // -1, 0

        let (spawn_x, spawn_y) = {
            let map_ref = world.resource::<TileMap>();
            (
                (colony_site.x + offset_x).clamp(0, map_ref.width - 1),
                (colony_site.y + offset_y).clamp(0, map_ref.height - 1),
            )
        };

        world.spawn((
            (
                Name(cat.name),
                Species::Cat,
                Age { born_tick: 0 },
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
                cat.skills,
                MagicAffinity(cat.magic_affinity),
                Corruption(0.0),
                Training::default(),
                CurrentAction::default(),
            ),
        ));
    }

    // -----------------------------------------------------------------------
    // Build schedule
    // -----------------------------------------------------------------------
    let mut schedule = Schedule::default();
    schedule.add_systems(
        (
            clowder::systems::time::advance_time,
            clowder::systems::weather::update_weather,
            clowder::systems::needs::decay_needs,
            clowder::systems::ai::evaluate_actions,
            clowder::systems::actions::resolve_actions,
            clowder::systems::narrative::generate_narrative,
        )
            .chain(),
    );

    // -----------------------------------------------------------------------
    // Initial narrative entry
    // -----------------------------------------------------------------------
    {
        let mut log = world.resource_mut::<NarrativeLog>();
        log.push(
            0,
            "A small group of cats settles in a clearing.".to_string(),
            NarrativeTier::Significant,
        );
    }

    // -----------------------------------------------------------------------
    // Main loop
    // -----------------------------------------------------------------------
    const FRAME_DURATION: Duration = Duration::from_millis(33); // ~30 fps

    loop {
        let frame_start = Instant::now();

        // --- Handle input (non-blocking) ------------------------------------
        if event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('p') => {
                            let mut time = world.resource_mut::<TimeState>();
                            time.paused = !time.paused;
                        }
                        KeyCode::Char('s') => {
                            let mut time = world.resource_mut::<TimeState>();
                            time.speed = time.speed.cycle();
                        }
                        _ => {}
                    }
                }
            }
        }

        // --- Tick simulation ------------------------------------------------
        let ticks = {
            let time = world.resource::<TimeState>();
            if time.paused {
                0
            } else {
                time.speed.ticks_per_update()
            }
        };
        for _ in 0..ticks {
            schedule.run(&mut world);
        }

        // --- Render TUI -----------------------------------------------------
        terminal.draw(|frame| {
            // Collect cat positions from ECS.
            let cat_positions: Vec<(String, Position)> = world
                .query::<(&Name, &Position)>()
                .iter(&world)
                .map(|(name, pos)| (name.0.clone(), *pos))
                .collect();

            // Build a stable borrow-friendly representation.
            let cat_pos_refs: Vec<(&str, Position)> = cat_positions
                .iter()
                .map(|(name, pos)| (name.as_str(), *pos))
                .collect();

            let map = world.resource::<TileMap>();
            let narrative = world.resource::<NarrativeLog>();
            let time = world.resource::<TimeState>();
            let config = world.resource::<SimConfig>();
            let weather = world.resource::<WeatherState>();
            let cat_count = cat_pos_refs.len();

            let view = AppView {
                map,
                cat_positions: cat_pos_refs,
                narrative,
                time,
                config,
                weather,
                cat_count,
            };
            view.render(frame);

            // Keep cat_positions alive until end of closure.
            drop(cat_positions);
        })?;

        // --- Frame timing ---------------------------------------------------
        let elapsed = frame_start.elapsed();
        if elapsed < FRAME_DURATION {
            std::thread::sleep(FRAME_DURATION - elapsed);
        }
    }

    Ok(())
}
