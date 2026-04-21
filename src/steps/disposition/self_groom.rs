use crate::components::grooming::GroomingCondition;
use crate::components::physical::Needs;
use crate::resources::sim_constants::DispositionConstants;
use crate::steps::StepResult;

pub fn resolve_self_groom(
    ticks: u64,
    needs: &mut Needs,
    grooming: Option<&mut GroomingCondition>,
    d: &DispositionConstants,
) -> StepResult {
    if ticks >= d.self_groom_duration {
        needs.temperature = (needs.temperature + d.self_groom_temperature_gain).min(1.0);
        if let Some(g) = grooming {
            g.0 = (g.0 + 0.15).min(1.0);
        }
        StepResult::Advance
    } else {
        StepResult::Continue
    }
}
