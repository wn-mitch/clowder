use crate::components::physical::{Needs, Position};
use crate::resources::exploration_map::ExplorationMap;
use crate::resources::sim_constants::DispositionConstants;
use crate::steps::StepResult;

pub fn resolve_survey(
    ticks: u64,
    needs: &mut Needs,
    pos: &Position,
    exploration_map: &mut ExplorationMap,
    d: &DispositionConstants,
) -> StepResult {
    if ticks >= d.survey_duration {
        // Mark the tile explored and get discovery value.
        let discovery = exploration_map.explore_tile(pos.x, pos.y);

        // Colony discovery bonus: finding new ground earns respect.
        let colony_bonus = discovery * d.survey_colony_discovery_scale;
        needs.respect = (needs.respect + colony_bonus).min(1.0);

        // Personal discovery bonus: smaller purpose gain.
        let personal_bonus = discovery * d.survey_personal_discovery_scale;
        needs.purpose = (needs.purpose + d.survey_purpose_gain + personal_bonus).min(1.0);

        StepResult::Advance
    } else {
        StepResult::Continue
    }
}
