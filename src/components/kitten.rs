use bevy_ecs::prelude::*;

/// Marks an entity as a dependent kitten that hasn't reached independence.
///
/// Maturity advances linearly from 0.0 to 1.0 over 4 seasons. At 1.0 this
/// component is removed and the cat gains full capabilities.
///
/// Parent entity references may become stale if a parent dies and is
/// despawned — the growth system handles this gracefully.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KittenDependency {
    #[serde(skip)]
    pub mother: Option<Entity>,
    #[serde(skip)]
    pub father: Option<Entity>,
    pub maturity: f32,
}

impl KittenDependency {
    pub fn new(mother: Entity, father: Entity) -> Self {
        Self {
            mother: Some(mother),
            father: Some(father),
            maturity: 0.0,
        }
    }
}
