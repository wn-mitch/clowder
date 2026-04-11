use crate::components::magic::{Inventory, RemedyKind};
use crate::components::skills::Skills;
use crate::resources::sim_constants::MagicConstants;
use crate::steps::StepResult;

pub fn resolve_prepare_remedy(
    ticks: u64,
    remedy: RemedyKind,
    at_workshop: bool,
    inventory: &mut Inventory,
    skills: &mut Skills,
    m: &MagicConstants,
) -> StepResult {
    let required_ticks = if at_workshop {
        m.prepare_remedy_ticks_workshop
    } else {
        m.prepare_remedy_ticks_default
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
