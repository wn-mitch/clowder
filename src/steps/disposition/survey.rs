use crate::components::physical::{Needs, Position};
use crate::resources::exploration_map::ExplorationMap;
use crate::resources::sim_constants::DispositionConstants;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `Survey`
///
/// **Real-world effect** — on completion, marks a disc of tiles
/// (radius `survey_explore_radius`) as explored in the
/// `ExplorationMap` and adds respect + purpose need deltas
/// proportional to the mean discovery value across the disc.
///
/// **Plan-level preconditions** — emitted under `ZoneIs(Wilds)` by
/// `src/ai/planner/actions.rs::explore_actions`; a `ExploreSurvey`
/// in GOAP lands the actor at a distant tile before this step.
///
/// **Runtime preconditions** — none; the tile-exploration pass
/// runs unconditionally at the Advance branch.
///
/// **Witness** — `StepOutcome<()>`. The effect is deterministic
/// once the step fires; no failure path silently Advances.
///
/// **Feature emission** — none currently. The discovery axis is
/// tracked via respect/purpose rather than a Feature counter.
pub fn resolve_survey(
    ticks: u64,
    needs: &mut Needs,
    pos: &Position,
    exploration_map: &mut ExplorationMap,
    d: &DispositionConstants,
) -> StepOutcome<()> {
    if ticks >= d.survey_duration {
        let discovery = exploration_map.explore_area(pos.x, pos.y, d.survey_explore_radius);

        let colony_bonus = discovery * d.survey_colony_discovery_scale;
        needs.respect = (needs.respect + colony_bonus).min(1.0);

        let personal_bonus = discovery * d.survey_personal_discovery_scale;
        needs.purpose = (needs.purpose + d.survey_purpose_gain + personal_bonus).min(1.0);

        needs.mastery = (needs.mastery + d.survey_mastery_gain).min(1.0);

        StepOutcome::bare(StepResult::Advance)
    } else {
        StepOutcome::bare(StepResult::Continue)
    }
}
