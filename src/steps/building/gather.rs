use crate::components::skills::Skills;
use crate::steps::StepResult;

pub fn resolve_gather(ticks: u64, skills: &mut Skills, workshop_bonus: f32) -> StepResult {
    if ticks >= 5 {
        skills.building += skills.growth_rate() * 0.005 * workshop_bonus;
        StepResult::Advance
    } else {
        StepResult::Continue
    }
}
