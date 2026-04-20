use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::rendering::camera::GameCamera;
use crate::rendering::sprite_animation::AnimationTimer;
use crate::rendering::sprite_assets::SpriteAssets;
use crate::resources::time::{Season, SimConfig, TimeState};
use crate::resources::weather::Weather;
use crate::resources::WeatherState;

// ---------------------------------------------------------------------------
// Weather effect identifiers
// ---------------------------------------------------------------------------

/// Identifies a specific VFX spritesheet (or the fog flat-tint special case).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WeatherEffect {
    Rain,
    Snow,
    Wind,
    AutumnLeaves,
    Fireflies,
    GodRays,
    Sakura,
    Fog,
}

/// Marker component on each active weather overlay entity.
#[derive(Component)]
pub struct WeatherOverlay {
    pub effect: WeatherEffect,
    pub target_alpha: f32,
}

/// Tracks currently active overlay entities for reconciliation.
#[derive(Resource, Default)]
pub struct WeatherOverlayState {
    pub active: Vec<(Entity, WeatherEffect)>,
}

// ---------------------------------------------------------------------------
// Desired overlay computation
// ---------------------------------------------------------------------------

/// An overlay we want active this frame.
struct DesiredOverlay {
    effect: WeatherEffect,
    alpha: f32,
    z: f32,
}

/// Determine which overlays should be active given the current weather and season.
fn desired_overlays(weather: Weather, season: Season, is_night: bool) -> Vec<DesiredOverlay> {
    let mut out = Vec::new();

    match weather {
        Weather::Clear => {
            if is_night {
                out.push(DesiredOverlay {
                    effect: WeatherEffect::Fireflies,
                    alpha: 0.15,
                    z: 51.0,
                });
            } else {
                // Seasonal ambient
                let (effect, alpha) = match season {
                    Season::Spring => (WeatherEffect::Sakura, 0.3),
                    Season::Summer => (WeatherEffect::GodRays, 0.2),
                    Season::Autumn => (WeatherEffect::AutumnLeaves, 0.3),
                    Season::Winter => (WeatherEffect::Snow, 0.15),
                };
                out.push(DesiredOverlay {
                    effect,
                    alpha,
                    z: 51.5,
                });
            }
        }
        Weather::Overcast => {} // no overlays
        Weather::LightRain => {
            out.push(DesiredOverlay {
                effect: WeatherEffect::Rain,
                alpha: 0.4,
                z: 51.0,
            });
        }
        Weather::HeavyRain => {
            out.push(DesiredOverlay {
                effect: WeatherEffect::Rain,
                alpha: 0.8,
                z: 51.0,
            });
        }
        Weather::Snow => {
            out.push(DesiredOverlay {
                effect: WeatherEffect::Snow,
                alpha: 1.0,
                z: 51.0,
            });
        }
        Weather::Fog => {
            out.push(DesiredOverlay {
                effect: WeatherEffect::Fog,
                alpha: 0.3,
                z: 51.0,
            });
        }
        Weather::Wind => {
            out.push(DesiredOverlay {
                effect: WeatherEffect::Wind,
                alpha: 0.5,
                z: 52.0,
            });
            if season == Season::Autumn {
                out.push(DesiredOverlay {
                    effect: WeatherEffect::AutumnLeaves,
                    alpha: 0.3,
                    z: 51.5,
                });
            }
        }
        Weather::Storm => {
            out.push(DesiredOverlay {
                effect: WeatherEffect::Rain,
                alpha: 0.8,
                z: 51.0,
            });
            out.push(DesiredOverlay {
                effect: WeatherEffect::Wind,
                alpha: 0.6,
                z: 52.0,
            });
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

/// Startup: insert the overlay state resource.
pub fn setup_weather_overlay_state(mut commands: Commands) {
    commands.insert_resource(WeatherOverlayState::default());
}

/// Each frame, reconcile active overlays with what the current weather demands.
pub fn update_weather_overlays(
    mut commands: Commands,
    sprite_assets: Res<SpriteAssets>,
    weather: Res<WeatherState>,
    time: Res<TimeState>,
    config: Res<SimConfig>,
    mut state: ResMut<WeatherOverlayState>,
    mut overlay_q: Query<&mut WeatherOverlay>,
) {
    let season = time.season(&config);

    // Determine if it's night: day progress 3.0-4.0 is night phase.
    let ticks_per_day = config.ticks_per_day_phase * 4;
    let phase_progress = (time.tick % ticks_per_day) as f32 / config.ticks_per_day_phase as f32;
    let is_night = phase_progress >= 3.0;

    let desired = desired_overlays(weather.current, season, is_night);

    // Remove overlays that are no longer desired.
    state.active.retain(|&(entity, effect)| {
        let still_wanted = desired.iter().any(|d| d.effect == effect);
        if !still_wanted {
            commands.entity(entity).despawn();
        }
        still_wanted
    });

    // Spawn new overlays or update alpha on existing ones.
    for d in &desired {
        if let Some(&(entity, _)) = state.active.iter().find(|(_, e)| *e == d.effect) {
            // Already active — update target alpha (weather intensity may have changed).
            if let Ok(mut overlay) = overlay_q.get_mut(entity) {
                overlay.target_alpha = d.alpha;
            }
            continue;
        }

        let entity = spawn_overlay(&mut commands, &sprite_assets, d);
        state.active.push((entity, d.effect));
    }
}

/// After update_weather_overlays, apply target alpha to overlay sprites.
///
/// Preserves the existing RGB (important for fog's gray tint) and only
/// modulates the alpha channel.
pub fn apply_weather_alpha(mut overlays: Query<(&WeatherOverlay, &mut Sprite)>) {
    for (overlay, mut sprite) in &mut overlays {
        let linear = sprite.color.to_linear();
        sprite.color = Color::LinearRgba(LinearRgba::new(
            linear.red,
            linear.green,
            linear.blue,
            overlay.target_alpha,
        ));
    }
}

/// Keep weather overlays centered on the camera and sized to fill the viewport.
#[allow(clippy::type_complexity)]
pub fn sync_weather_overlay_positions(
    camera_q: Query<(&Transform, &Projection), With<GameCamera>>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    mut overlays: Query<(&mut Transform, &mut Sprite), (With<WeatherOverlay>, Without<GameCamera>)>,
) {
    let Ok((cam_tf, projection)) = camera_q.single() else {
        return;
    };
    let Ok(window) = window_q.single() else {
        return;
    };

    let scale = match projection {
        Projection::Orthographic(ortho) => ortho.scale,
        _ => 1.0,
    };

    // Visible area in world units, with 20% margin to prevent edge gaps during pan.
    let visible_w = window.width() * scale * 1.2;
    let visible_h = window.height() * scale * 1.2;

    for (mut tf, mut sprite) in &mut overlays {
        tf.translation.x = cam_tf.translation.x;
        tf.translation.y = cam_tf.translation.y;
        // z stays as spawned (51.0, 51.5, 52.0)
        sprite.custom_size = Some(Vec2::new(visible_w, visible_h));
    }
}

// ---------------------------------------------------------------------------
// Spawn helper
// ---------------------------------------------------------------------------

fn spawn_overlay(
    commands: &mut Commands,
    assets: &SpriteAssets,
    desired: &DesiredOverlay,
) -> Entity {
    if desired.effect == WeatherEffect::Fog {
        // Fog: flat white-gray tint, no animation.
        return commands
            .spawn((
                Sprite {
                    image: assets.white_pixel.clone(),
                    color: Color::srgba(0.85, 0.85, 0.9, desired.alpha),
                    custom_size: Some(Vec2::new(2000.0, 2000.0)), // resized by sync system
                    ..default()
                },
                Transform::from_xyz(0.0, 0.0, desired.z),
                WeatherOverlay {
                    effect: desired.effect,
                    target_alpha: desired.alpha,
                },
            ))
            .id();
    }

    let texture = effect_texture(assets, desired.effect);
    let frame_duration = effect_frame_duration(desired.effect);

    commands
        .spawn((
            Sprite {
                image: texture,
                color: Color::srgba(1.0, 1.0, 1.0, desired.alpha),
                custom_size: Some(Vec2::new(2000.0, 2000.0)), // resized by sync system
                texture_atlas: Some(TextureAtlas {
                    layout: assets.weather_layout.clone(),
                    index: 0,
                }),
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, desired.z),
            AnimationTimer::new(48, frame_duration),
            WeatherOverlay {
                effect: desired.effect,
                target_alpha: desired.alpha,
            },
        ))
        .id()
}

fn effect_texture(assets: &SpriteAssets, effect: WeatherEffect) -> Handle<Image> {
    match effect {
        WeatherEffect::Rain => assets.weather_rain_texture.clone(),
        WeatherEffect::Snow => assets.weather_snow_texture.clone(),
        WeatherEffect::Wind => assets.weather_wind_texture.clone(),
        WeatherEffect::AutumnLeaves => assets.weather_autumn_leaves_texture.clone(),
        WeatherEffect::Fireflies => assets.weather_fireflies_texture.clone(),
        WeatherEffect::GodRays => assets.weather_god_rays_texture.clone(),
        WeatherEffect::Sakura => assets.weather_sakura_texture.clone(),
        WeatherEffect::Fog => unreachable!("fog uses white_pixel, not a spritesheet"),
    }
}

/// Per-effect frame duration. Snow/god rays are slower; rain/wind faster.
fn effect_frame_duration(effect: WeatherEffect) -> std::time::Duration {
    use std::time::Duration;
    match effect {
        WeatherEffect::Rain => Duration::from_millis(60),
        WeatherEffect::Wind => Duration::from_millis(70),
        WeatherEffect::Snow => Duration::from_millis(100),
        WeatherEffect::AutumnLeaves => Duration::from_millis(90),
        WeatherEffect::Fireflies => Duration::from_millis(100),
        WeatherEffect::GodRays => Duration::from_millis(120),
        WeatherEffect::Sakura => Duration::from_millis(90),
        WeatherEffect::Fog => Duration::from_millis(100), // unused
    }
}
