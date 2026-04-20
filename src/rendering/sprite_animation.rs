use std::time::Duration;

use bevy::prelude::*;

/// Cycles through animation frames on a timer.
///
/// Attached to entities with animated spritesheets. By default the system
/// advances `TextureAtlas.index` each time the timer fires, wrapping at
/// `frame_count`. If the entity also has an [`AnimationSequence`], that
/// component drives frame selection instead (supports out-of-order frame
/// sequences with repeats, as authored in the Fan-tasy Tileset TSX files).
#[derive(Component)]
pub struct AnimationTimer {
    timer: Timer,
    frame_count: u8,
}

impl AnimationTimer {
    /// Build a repeating animation timer.
    ///
    /// `frame_count` is used for the default `(index + 1) % frame_count`
    /// wrap-around when no [`AnimationSequence`] is present.
    pub fn new(frame_count: u8, frame_duration: Duration) -> Self {
        Self {
            timer: Timer::new(frame_duration, TimerMode::Repeating),
            frame_count,
        }
    }
}

/// Drives an atlas-index animation by walking an authored frame sequence.
///
/// The sequence is a list of column indices into the atlas (0-based within
/// the entity's row). `base` is the atlas-index offset for the row (e.g.
/// `row * cols_per_row`); the final `TextureAtlas.index` each tick is
/// `base + steps[cursor]`. This matches the TSX `<animation>` convention
/// where a frame order can revisit tiles (glow-pulse rhythms).
#[derive(Component)]
pub struct AnimationSequence {
    pub base: u16,
    pub steps: &'static [u8],
    pub cursor: u8,
}

pub fn tick_sprite_animations(
    time: Res<Time>,
    mut query: Query<(
        &mut AnimationTimer,
        &mut Sprite,
        Option<&mut AnimationSequence>,
    )>,
) {
    for (mut anim, mut sprite, seq) in &mut query {
        anim.timer.tick(time.delta());
        if !anim.timer.just_finished() {
            continue;
        }
        let Some(atlas) = &mut sprite.texture_atlas else {
            continue;
        };
        match seq {
            Some(mut seq) => {
                seq.cursor = (seq.cursor + 1) % seq.steps.len() as u8;
                atlas.index = seq.base as usize + seq.steps[seq.cursor as usize] as usize;
            }
            None => {
                atlas.index = (atlas.index + 1) % anim.frame_count as usize;
            }
        }
    }
}
