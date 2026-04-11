use crate::components::physical::Needs;
use crate::resources::sim_constants::DispositionConstants;
use crate::steps::StepResult;

pub fn resolve_sleep(
    ticks: u64,
    duration: u64,
    needs: &mut Needs,
    d: &DispositionConstants,
) -> StepResult {
    needs.energy = (needs.energy + d.sleep_energy_per_tick).min(1.0);
    needs.warmth = (needs.warmth + d.sleep_warmth_per_tick).min(1.0);
    if ticks >= duration {
        StepResult::Advance
    } else {
        StepResult::Continue
    }
}
