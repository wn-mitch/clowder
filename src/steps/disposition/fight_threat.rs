use crate::components::physical::{Health, Needs};
use crate::components::skills::Skills;
use crate::resources::sim_constants::DispositionConstants;
use crate::steps::StepResult;

pub fn resolve_fight_threat(
    ticks: u64,
    skills: &mut Skills,
    needs: &mut Needs,
    health: &Health,
    d: &DispositionConstants,
) -> StepResult {
    // Bail out if health drops below threshold — morale break.
    if health.current / health.max < d.fight_bail_health_threshold {
        return StepResult::Fail("morale_break".into());
    }

    if ticks >= d.fight_duration {
        skills.combat += skills.growth_rate() * d.fight_combat_skill_growth;
        needs.safety = (needs.safety + d.fight_safety_gain).min(1.0);
        StepResult::Advance
    } else {
        StepResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> DispositionConstants {
        DispositionConstants::default()
    }

    #[test]
    fn fight_bails_when_health_below_threshold() {
        let d = defaults();
        let mut skills = Skills::default();
        let mut needs = Needs::default();
        // Health well below the 0.35 bail threshold.
        let health = Health {
            current: 0.2,
            max: 1.0,
            injuries: Vec::new(),
        };
        let result = resolve_fight_threat(0, &mut skills, &mut needs, &health, &d);
        assert!(
            matches!(result, StepResult::Fail(ref reason) if reason == "morale_break"),
            "expected morale_break Fail, got {result:?}"
        );
    }

    #[test]
    fn fight_continues_when_health_above_threshold() {
        let d = defaults();
        let mut skills = Skills::default();
        let mut needs = Needs::default();
        let health = Health::default(); // 1.0 / 1.0 = 1.0, well above 0.35
        let result = resolve_fight_threat(0, &mut skills, &mut needs, &health, &d);
        assert!(
            matches!(result, StepResult::Continue),
            "expected Continue, got {result:?}"
        );
    }

    #[test]
    fn fight_advances_after_duration_with_healthy_cat() {
        let d = defaults();
        let mut skills = Skills::default();
        let mut needs = Needs {
            safety: 0.5,
            ..Default::default()
        };
        let health = Health::default();
        let result = resolve_fight_threat(d.fight_duration, &mut skills, &mut needs, &health, &d);
        assert!(
            matches!(result, StepResult::Advance),
            "expected Advance, got {result:?}"
        );
        assert!(needs.safety > 0.5, "safety should have increased");
    }

    #[test]
    fn fight_bails_even_at_duration_tick() {
        let d = defaults();
        let mut skills = Skills::default();
        let mut needs = Needs::default();
        // At duration tick but critically injured — bail takes priority.
        let health = Health {
            current: 0.1,
            max: 1.0,
            injuries: Vec::new(),
        };
        let result = resolve_fight_threat(d.fight_duration, &mut skills, &mut needs, &health, &d);
        assert!(
            matches!(result, StepResult::Fail(ref reason) if reason == "morale_break"),
            "bail should fire even at fight_duration tick"
        );
    }
}
