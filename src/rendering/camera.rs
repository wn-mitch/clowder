use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use noise::{NoiseFn, Perlin};

use crate::ai::{Action, CurrentAction};
use crate::components::identity::Species;
use crate::components::physical::Position;
use crate::rendering::tilemap_sync::{TILE_PX, TILE_SCALE};
use crate::resources::map::TileMap;
use crate::resources::{NarrativeLog, NarrativeTier};

/// Marker for the main game camera.
#[derive(Component)]
pub struct GameCamera;

// ---------------------------------------------------------------------------
// Camera state machine
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CameraMode {
    /// Slow Perlin-noise drift across the map.
    Drift,
    /// Hovering near interesting activity.
    Linger,
    /// Smooth pan to a significant event location.
    Follow,
    /// Manual control (WASD/scroll).
    Override,
    /// Camera tracks a specific entity's visual position (inspector lock).
    LockedOn(Entity),
}

/// The camera "brain" — drives all camera movement.
#[derive(Resource)]
pub struct CameraBrain {
    pub mode: CameraMode,
    pub target: Vec2,
    /// Elapsed drift time (Perlin input).
    pub drift_time: f64,
    /// Perlin noise generators for x and y drift.
    pub perlin_x: Perlin,
    pub perlin_y: Perlin,
    /// Timer for returning to Drift from Linger/Follow/Override.
    pub mode_timer: f32,
    /// Last tick we checked NarrativeLog for significant events.
    pub last_checked_narrative: u64,
    /// Seconds since last manual input (for Override timeout).
    pub idle_seconds: f32,
    /// Map bounds in world pixels.
    pub map_min: Vec2,
    pub map_max: Vec2,
}

impl CameraBrain {
    pub fn new(map_width: i32, map_height: i32) -> Self {
        let world_px = TILE_PX * TILE_SCALE;
        let center = Vec2::new(
            map_width as f32 / 2.0 * world_px,
            map_height as f32 / 2.0 * world_px,
        );
        Self {
            mode: CameraMode::Drift,
            target: center,
            drift_time: 100.0, // Start away from noise origin
            perlin_x: Perlin::new(42),
            perlin_y: Perlin::new(137),
            mode_timer: 0.0,
            last_checked_narrative: 0,
            idle_seconds: 0.0,
            map_min: Vec2::ZERO,
            map_max: Vec2::new(map_width as f32 * world_px, map_height as f32 * world_px),
        }
    }

    fn clamp_target(&mut self) {
        self.target = self.target.clamp(self.map_min, self.map_max);
    }
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Startup: spawn the camera and initialize the brain.
pub fn setup_camera(mut commands: Commands, map: Res<TileMap>) {
    let world_px = TILE_PX * TILE_SCALE;
    let center_x = (map.width as f32 / 2.0) * world_px;
    let center_y = (map.height as f32 / 2.0) * world_px;

    commands.spawn((
        Camera2d,
        Transform::from_xyz(center_x, center_y, 999.0),
        Projection::Orthographic(OrthographicProjection {
            scale: 0.5,
            ..OrthographicProjection::default_2d()
        }),
        GameCamera,
    ));

    commands.insert_resource(CameraBrain::new(map.width, map.height));
}

/// Main camera update: runs the state machine, then lerps the camera.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn camera_update(
    mut commands: Commands,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut scroll_events: MessageReader<MouseWheel>,
    mut brain: ResMut<CameraBrain>,
    mut query: Query<(&mut Transform, &mut Projection), With<GameCamera>>,
    time: Res<Time>,
    cats: Query<(&Position, &CurrentAction), With<Species>>,
    cat_transforms: Query<&Transform, (With<Species>, Without<GameCamera>)>,
    narrative: Res<NarrativeLog>,
    map: Res<TileMap>,
    inspection: Res<crate::ui_data::InspectionState>,
) {
    let Ok((mut transform, mut projection)) = query.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    let world_px = TILE_PX * TILE_SCALE;
    let map_h = map.height as f32;

    // --- Detect manual input → Override ---
    let has_scroll = {
        let mut any = false;
        let current_scale = match &*projection {
            Projection::Orthographic(ortho) => ortho.scale,
            _ => 1.0,
        };
        let mut new_scale = current_scale;
        for event in scroll_events.read() {
            any = true;
            let zoom_delta = -event.y * 0.1;
            new_scale = (new_scale + zoom_delta).clamp(0.2, 5.0);
        }
        if new_scale != current_scale {
            if let Projection::Orthographic(ref mut ortho) = *projection {
                ortho.scale = new_scale;
            }
        }
        any
    };

    let pan_keys = keyboard.pressed(KeyCode::ArrowLeft)
        || keyboard.pressed(KeyCode::ArrowRight)
        || keyboard.pressed(KeyCode::ArrowUp)
        || keyboard.pressed(KeyCode::ArrowDown)
        || keyboard.pressed(KeyCode::KeyA)
        || keyboard.pressed(KeyCode::KeyD)
        || keyboard.pressed(KeyCode::KeyW)
        || keyboard.pressed(KeyCode::KeyS);

    let has_manual_input = pan_keys || has_scroll;

    if has_manual_input {
        brain.mode = CameraMode::Override;
        brain.idle_seconds = 0.0;
    }

    // Escape returns to Drift, but only when no inspect panels are open
    // (let main handle_input consume Escape for panel dismiss first).
    if keyboard.just_pressed(KeyCode::Escape)
        && brain.mode == CameraMode::Override
        && inspection.mode == crate::ui_data::InspectionMode::None
    {
        brain.mode = CameraMode::Drift;
    }

    // --- Sync camera lock with inspector selection ---
    match &inspection.mode {
        crate::ui_data::InspectionMode::CatInspect(entity) => {
            if brain.mode != CameraMode::Override {
                brain.mode = CameraMode::LockedOn(*entity);
            }
        }
        _ => {
            if matches!(brain.mode, CameraMode::LockedOn(_)) {
                brain.mode = CameraMode::Drift;
            }
        }
    }

    // --- Run current mode ---
    match brain.mode {
        CameraMode::Drift => {
            brain.drift_time += dt as f64 * 0.15;

            // Noise drives a gentle VELOCITY, not a position. The camera
            // wanders like a leaf on water — continuous, no snapping.
            let vx = brain.perlin_x.get([brain.drift_time, 0.5]) as f32;
            let vy = brain.perlin_y.get([0.5, brain.drift_time]) as f32;
            let drift_speed = world_px * 0.4; // ~0.4 tiles per second
            brain.target.x += vx * drift_speed * dt;
            brain.target.y += vy * drift_speed * dt;

            // Soft bounce off map edges — steer back toward center when near bounds.
            let margin = world_px * 4.0;
            let center = (brain.map_min + brain.map_max) * 0.5;
            let pull_strength = 0.3 * dt;
            if brain.target.x < brain.map_min.x + margin
                || brain.target.x > brain.map_max.x - margin
            {
                brain.target.x += (center.x - brain.target.x) * pull_strength;
            }
            if brain.target.y < brain.map_min.y + margin
                || brain.target.y > brain.map_max.y - margin
            {
                brain.target.y += (center.y - brain.target.y) * pull_strength;
            }
            brain.clamp_target();

            // Check for nearby interesting activity → Linger
            let cam_pos = Vec2::new(transform.translation.x, transform.translation.y);
            let viewport_radius = 300.0; // ~6 tiles
            let mut interesting_nearby = 0;
            let mut activity_center = Vec2::ZERO;
            for (pos, action) in &cats {
                if is_interesting_action(&action.action) {
                    let entity_world = grid_to_world_vec2(pos, map_h, world_px);
                    if cam_pos.distance(entity_world) < viewport_radius {
                        interesting_nearby += 1;
                        activity_center += entity_world;
                    }
                }
            }
            if interesting_nearby > 0 {
                activity_center /= interesting_nearby as f32;
                brain.target = activity_center;
                brain.mode = CameraMode::Linger;
                brain.mode_timer = 0.0;
            }

            // Check for significant narrative events → Follow
            check_significant_events(&mut brain, &narrative, &cats, map_h, world_px);
        }

        CameraMode::Linger => {
            brain.mode_timer += dt;
            // Re-check if activity is still nearby
            let mut still_interesting = false;
            for (pos, action) in &cats {
                if is_interesting_action(&action.action) {
                    let entity_world = grid_to_world_vec2(pos, map_h, world_px);
                    if brain.target.distance(entity_world) < 400.0 {
                        still_interesting = true;
                        break;
                    }
                }
            }
            if !still_interesting || brain.mode_timer > 8.0 {
                brain.mode = CameraMode::Drift;
            }

            // Still check for significant events
            check_significant_events(&mut brain, &narrative, &cats, map_h, world_px);
        }

        CameraMode::Follow => {
            brain.mode_timer += dt;
            // Hold on event location, then return to drift
            if brain.mode_timer > 6.0 {
                brain.mode = CameraMode::Drift;
            }
        }

        CameraMode::LockedOn(entity) => {
            if let Ok(cat_tf) = cat_transforms.get(entity) {
                brain.target = cat_tf.translation.truncate();
                brain.clamp_target();
            } else {
                // Entity despawned — release the lock.
                brain.mode = CameraMode::Drift;
            }
        }

        CameraMode::Override => {
            brain.idle_seconds += dt;
            let current_scale = match &*projection {
                Projection::Orthographic(ortho) => ortho.scale,
                _ => 1.0,
            };
            let pan_speed = 500.0 * current_scale * dt;
            let mut direction = Vec2::ZERO;
            if keyboard.pressed(KeyCode::ArrowLeft) || keyboard.pressed(KeyCode::KeyA) {
                direction.x -= 1.0;
            }
            if keyboard.pressed(KeyCode::ArrowRight) || keyboard.pressed(KeyCode::KeyD) {
                direction.x += 1.0;
            }
            if keyboard.pressed(KeyCode::ArrowDown) || keyboard.pressed(KeyCode::KeyS) {
                direction.y -= 1.0;
            }
            if keyboard.pressed(KeyCode::ArrowUp) || keyboard.pressed(KeyCode::KeyW) {
                direction.y += 1.0;
            }
            if direction != Vec2::ZERO {
                direction = direction.normalize();
                // In Override mode, move camera directly (no lerp target).
                transform.translation.x += direction.x * pan_speed;
                transform.translation.y += direction.y * pan_speed;
            }
            // Clamp to map bounds.
            transform.translation.x = transform
                .translation
                .x
                .clamp(brain.map_min.x, brain.map_max.x);
            transform.translation.y = transform
                .translation
                .y
                .clamp(brain.map_min.y, brain.map_max.y);
            // Update target to current position so lerp doesn't fight manual control.
            brain.target = Vec2::new(transform.translation.x, transform.translation.y);

            // Return to drift after 15s idle
            if brain.idle_seconds > 15.0 {
                brain.mode = CameraMode::Drift;
            }
        }
    }

    // --- Lerp camera toward target (except Override, which moves directly) ---
    if brain.mode != CameraMode::Override {
        let lerp_speed = match brain.mode {
            CameraMode::Drift => 0.4,       // Very floaty — cloud on a breeze
            CameraMode::Linger => 0.8,      // Settles gently near activity
            CameraMode::Follow => 1.8,      // Purposeful but not jarring
            CameraMode::LockedOn(_) => 1.2, // Smooth entity tracking
            CameraMode::Override => unreachable!(),
        };
        let current = Vec2::new(transform.translation.x, transform.translation.y);
        let new_pos = current.lerp(brain.target, (lerp_speed * dt).min(1.0));
        let clamped = new_pos.clamp(brain.map_min, brain.map_max);
        transform.translation.x = clamped.x;
        transform.translation.y = clamped.y;
    }

    // --- Screenshot with F5 ---
    if keyboard.just_pressed(KeyCode::F5) {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk("/tmp/clowder_screenshot.png"));
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_interesting_action(action: &Action) -> bool {
    matches!(
        action,
        Action::Hunt
            | Action::Fight
            | Action::Socialize
            | Action::PracticeMagic
            | Action::Herbcraft
            | Action::Build
            | Action::Farm
            | Action::Mentor
            | Action::Flee
    )
}

fn grid_to_world_vec2(pos: &Position, map_height: f32, world_px: f32) -> Vec2 {
    Vec2::new(
        pos.x as f32 * world_px,
        (map_height - 1.0 - pos.y as f32) * world_px,
    )
}

fn check_significant_events(
    brain: &mut CameraBrain,
    narrative: &NarrativeLog,
    cats: &Query<(&Position, &CurrentAction), With<Species>>,
    map_h: f32,
    world_px: f32,
) {
    let new_significant = narrative
        .entries
        .iter()
        .rev()
        .take(5)
        .any(|e| e.tick > brain.last_checked_narrative && e.tier == NarrativeTier::Significant);

    if new_significant {
        brain.last_checked_narrative = narrative.entries.back().map_or(0, |e| e.tick);
        // Find the nearest cat to follow — significant events usually involve cats.
        if let Some((pos, _)) = cats.iter().next() {
            brain.target = grid_to_world_vec2(pos, map_h, world_px);
            brain.mode = CameraMode::Follow;
            brain.mode_timer = 0.0;
        }
    }
}

// ---------------------------------------------------------------------------
// Auto-screenshot (debug utility)
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub struct AutoScreenshot {
    timer: bevy::time::Timer,
    taken: bool,
}

impl Default for AutoScreenshot {
    fn default() -> Self {
        Self {
            timer: bevy::time::Timer::from_seconds(2.0, bevy::time::TimerMode::Once),
            taken: false,
        }
    }
}

pub fn auto_screenshot(mut commands: Commands, time: Res<Time>, mut state: ResMut<AutoScreenshot>) {
    if state.taken {
        return;
    }
    state.timer.tick(time.delta());
    if state.timer.just_finished() {
        state.taken = true;
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk("/tmp/clowder_screenshot.png"));
        eprintln!("Auto-screenshot → /tmp/clowder_screenshot.png");
    }
}
