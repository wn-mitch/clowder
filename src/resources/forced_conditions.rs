use bevy_ecs::prelude::Resource;

use crate::resources::weather::Weather;

/// Diagnostic override that pins simulation conditions to fixed values for
/// controlled-sweep verification runs. Headless-only (the interactive build
/// never sets this).
///
/// When `weather` is `Some`, `update_weather` replaces `WeatherState::current`
/// with the pinned variant every tick and suppresses the natural transition
/// roll. The sweep-comparison pipeline uses this to isolate Phase 5b
/// activation effects on a single weather condition without waiting for the
/// condition to arise naturally in a 15-minute run.
///
/// Serialized into the event-log header so two run directories can't be
/// accidentally compared when their forced conditions differ.
#[derive(Resource, Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ForcedConditions {
    pub weather: Option<Weather>,
}
