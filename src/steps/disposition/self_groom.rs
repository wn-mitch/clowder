use crate::components::grooming::GroomingCondition;
use crate::components::physical::Needs;
use crate::resources::sim_constants::DispositionConstants;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `SelfGroom`
///
/// **Real-world effect** — on completion (`ticks >=
/// self_groom_duration`), raises `needs.temperature` and the
/// `GroomingCondition` component (if present) by fixed amounts.
///
/// **Plan-level preconditions** — single-actor step; no target
/// required.
///
/// **Runtime preconditions** — none required; `GroomingCondition`
/// is optional (kittens may lack it and the step still runs
/// cleanly, just without the condition-tracking bump).
///
/// **Witness** — `StepOutcome<()>`. Effect is unconditional at
/// the Advance branch.
///
/// **Feature emission** — none. Distinct from `GroomedOther`
/// which tracks adult-to-adult grooming as a Positive social
/// signal; self-grooming is routine maintenance.
pub fn resolve_self_groom(
    ticks: u64,
    needs: &mut Needs,
    grooming: Option<&mut GroomingCondition>,
    d: &DispositionConstants,
) -> StepOutcome<()> {
    if ticks >= d.self_groom_duration {
        needs.temperature = (needs.temperature + d.self_groom_temperature_gain).min(1.0);
        if let Some(g) = grooming {
            g.0 = (g.0 + 0.15).min(1.0);
        }
        StepOutcome::bare(StepResult::Advance)
    } else {
        StepOutcome::bare(StepResult::Continue)
    }
}
