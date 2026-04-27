use crate::components::magic::{Inventory, RemedyKind};
use crate::components::skills::Skills;
use crate::resources::sim_constants::MagicConstants;
use crate::resources::time::TimeScale;
use crate::steps::StepResult;

/// # GOAP step resolver: `PrepareRemedy`
///
/// **Real-world effect** — consumes one herb of the required
/// kind from the actor's inventory; grows herbcraft skill. The
/// "remedy" itself is synthesized on the actor side when a
/// subsequent `ApplyRemedy` step fires — this step doesn't
/// spawn an item.
///
/// **Plan-level preconditions** — emitted by herbcraft planner;
/// `at_workshop` parameter controls the tick budget.
///
/// **Runtime preconditions** — `inventory.take_herb(required)`
/// must succeed or Fail("missing herb for remedy"). No silent-
/// advance surface.
///
/// **Witness** — returns plain `StepResult` (predates the
/// `StepOutcome<W>` convention). Success is implicit in Advance
/// (the take_herb call only succeeds on Advance path).
///
/// **Feature emission** — none currently. A
/// `RemedyPrepared` Positive Feature could be added, but for
/// now this is tracked via the downstream `ApplyRemedy` →
/// `RemedyApplied` signal.
pub fn resolve_prepare_remedy(
    ticks: u64,
    remedy: RemedyKind,
    at_workshop: bool,
    inventory: &mut Inventory,
    skills: &mut Skills,
    m: &MagicConstants,
    time_scale: &TimeScale,
) -> StepResult {
    let required_ticks = if at_workshop {
        m.prepare_remedy_duration_workshop.ticks(time_scale)
    } else {
        m.prepare_remedy_duration_default.ticks(time_scale)
    };
    if ticks >= required_ticks {
        let herb_needed = remedy.required_herb();
        if inventory.take_herb(herb_needed) {
            skills.herbcraft += skills.growth_rate() * m.herbcraft_prepare_skill_growth;
            StepResult::Advance
        } else {
            StepResult::Fail("missing herb for remedy".into())
        }
    } else {
        StepResult::Continue
    }
}
