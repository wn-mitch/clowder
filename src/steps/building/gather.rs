use crate::components::skills::Skills;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `GatherMaterials`
///
/// **Real-world effect** — grows the actor's `building` skill by
/// a small amount. Note: this step is currently NOT produced by
/// the GOAP planner — `Construct` handles its own materials
/// delivery — and exists only as an enum-exhaustiveness fallback.
///
/// **Plan-level preconditions** — none active (see above).
///
/// **Runtime preconditions** — none; `ticks >= 5` time gate only.
///
/// **Witness** — `StepOutcome<()>`. Effect is unconditional.
///
/// **Feature emission** — none. The step isn't active in the
/// current planner; if reinstated, a `BuildingSkillGrew` Feature
/// could be added.
pub fn resolve_gather(
    ticks: u64,
    skills: &mut Skills,
    workshop_bonus: f32,
) -> StepOutcome<()> {
    if ticks >= 5 {
        skills.building += skills.growth_rate() * 0.005 * workshop_bonus;
        StepOutcome::bare(StepResult::Advance)
    } else {
        StepOutcome::bare(StepResult::Continue)
    }
}
