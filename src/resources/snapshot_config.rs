use bevy_ecs::prelude::*;

/// Controls emission rates for diagnostic event log channels.
///
/// Each interval is in simulation ticks. A value of 0 disables that channel.
/// CLI flags `--trace-positions <N>` and `--snapshot-interval <N>` override
/// the defaults.
#[derive(Resource, Debug, Clone)]
pub struct SnapshotConfig {
    /// Full CatSnapshot (personality, needs, skills, relationships). Expensive.
    pub full_snapshot_interval: u64,
    /// Lightweight position+action trace per cat. Cheap — safe at interval=1.
    pub position_trace_interval: u64,
    /// FoodLevel + PopulationSnapshot.
    pub economy_interval: u64,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            full_snapshot_interval: 100,
            position_trace_interval: 0, // off by default
            economy_interval: 100,
        }
    }
}
