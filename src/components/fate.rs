use bevy_ecs::prelude::*;

/// Placeholder entity for serde deserialization. Fated connection entity
/// references must be re-linked after loading a save.
fn placeholder_entity() -> Entity {
    Entity::from_bits(u64::MAX)
}

// ---------------------------------------------------------------------------
// Fated Love
// ---------------------------------------------------------------------------

/// A destined love. Mutual — both cats hold the component pointing at each other.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FatedLove {
    /// The fated partner. Skipped during serialisation because Entity IDs are
    /// not stable across save/load.
    #[serde(skip_serializing, skip_deserializing, default = "placeholder_entity")]
    pub partner: Entity,
    /// True once both cats have been within 5 tiles of each other.
    pub awakened: bool,
    /// Tick when the bond was assigned.
    pub assigned_tick: u64,
}

// ---------------------------------------------------------------------------
// Fated Rival
// ---------------------------------------------------------------------------

/// A destined rival. Mutual — both cats hold the component pointing at each other.
#[derive(Component, Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FatedRival {
    /// The fated rival. Skipped during serialisation.
    #[serde(skip_serializing, skip_deserializing, default = "placeholder_entity")]
    pub rival: Entity,
    /// True once both cats have competed (same action type in proximity).
    pub awakened: bool,
    /// Tick when the bond was assigned.
    pub assigned_tick: u64,
}

// ---------------------------------------------------------------------------
// Marker component
// ---------------------------------------------------------------------------

/// Inserted after a cat's fated connections have been evaluated.
/// Systems use `Without<FateAssigned>` to detect cats needing assignment.
#[derive(Component, Debug, Clone, Copy)]
pub struct FateAssigned;
