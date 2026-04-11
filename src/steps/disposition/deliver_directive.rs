use crate::components::physical::Needs;
use crate::resources::sim_constants::DispositionConstants;
use crate::steps::StepResult;

pub fn resolve_deliver_directive(
    ticks: u64,
    needs: &mut Needs,
    d: &DispositionConstants,
) -> StepResult {
    if ticks >= d.deliver_directive_duration {
        needs.respect = (needs.respect + d.deliver_directive_respect_gain).min(1.0);
        needs.social = (needs.social + d.deliver_directive_social_gain).min(1.0);
        StepResult::Advance
    } else {
        StepResult::Continue
    }
}
