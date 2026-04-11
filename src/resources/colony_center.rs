use bevy_ecs::prelude::*;

use crate::components::physical::Position;

/// The colony's founding position, persisted at world generation.
///
/// Used as the origin for territory-based queries (corruption ring,
/// threat proximity) and anchors the decorative colony well sprite.
#[derive(Resource, Debug, Clone)]
pub struct ColonyCenter(pub Position);
