use bevy_ecs::prelude::*;

/// Marks an entity as a dependent kitten that hasn't reached independence.
///
/// Maturity advances linearly from 0.0 to 1.0 over 4 seasons. At 1.0 this
/// component is removed and the cat gains full capabilities.
///
/// Parent entity references may become stale if a parent dies and is
/// despawned — the growth system handles this gracefully.
///
/// `skills_learned` is incremented by `resolve_teach` over the Teach phase
/// of the `rear_kitten` HTN method (ticket 364). Substrate-only at 364 land:
/// the count exists so downstream memory/personality attribution can read it
/// without re-authoring substrate; no consumer reads it today.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KittenDependency {
    #[serde(skip)]
    pub mother: Option<Entity>,
    #[serde(skip)]
    pub father: Option<Entity>,
    pub maturity: f32,
    #[serde(default)]
    pub skills_learned: u8,
}

impl KittenDependency {
    pub fn new(mother: Entity, father: Entity) -> Self {
        Self {
            mother: Some(mother),
            father: Some(father),
            maturity: 0.0,
            skills_learned: 0,
        }
    }
}

/// Skill labels demonstrated during the Teach phase of `rear_kitten`. The
/// table is rotated through by `resolve_teach`; the index used per call is
/// `KittenDependency.skills_learned % len()`. Substrate-only at 364 land —
/// the strings exist so narrative / memory layers can attribute them later.
pub const KITTEN_SKILL_CURRICULUM: &[&str] = &["stalk", "pounce", "groom", "forage", "hide"];
