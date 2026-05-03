use crate::components::physical::{Health, Needs};
use crate::components::skills::Skills;
use crate::resources::sim_constants::DispositionConstants;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `EngageThreat`
///
/// **Real-world effect** — on completion (`ticks >= fight_duration`),
/// grows the cat's `combat` skill by `growth_rate() *
/// fight_combat_skill_growth` and boosts `needs.safety` by
/// `fight_safety_gain`. If health drops below `fight_bail_health_threshold`
/// at any tick, returns `Fail("morale_break")`.
///
/// **Plan-level preconditions** — emitted under
/// `ZoneIs(PlannerZone::PatrolZone)` by
/// `src/ai/planner/actions.rs::patrol_actions`. `ZoneIs` alone does
/// **not** prove wildlife is present — the step relies on the
/// scoring-layer having elevated EngageThreat because a sensed
/// threat was nearby; a drifting plan can arrive with no live
/// wildlife in range.
///
/// **Runtime preconditions** — no target-existence check in this
/// step: the real "is there wildlife?" test lives in
/// `src/systems/combat.rs`, which resolves actual damage. A follow-
/// up could tighten this by taking a wildlife-nearby snapshot
/// here; for now the witness fires whenever the skill/safety
/// side-effect actually runs (`ticks >= fight_duration`).
///
/// **Witness** — `StepOutcome<bool>`. `true` iff the Advance
/// branch ran (skill growth + safety gain applied). `false` on
/// Continue (still engaging) or Fail (morale break).
///
/// **Feature emission** — caller passes `Feature::ThreatEngaged`
/// (Positive) to `record_if_witnessed`. Distinct from the
/// existing `Feature::CombatResolved` — ThreatEngaged = "the step
/// reached its duration", CombatResolved = "combat terminated with
/// a winner".
pub fn resolve_fight_threat(
    ticks: u64,
    skills: &mut Skills,
    needs: &mut Needs,
    health: &Health,
    d: &DispositionConstants,
) -> StepOutcome<bool> {
    if health.current / health.max < d.fight_bail_health_threshold {
        return StepOutcome::unwitnessed(StepResult::Fail("morale_break".into()));
    }

    if ticks >= d.fight_duration {
        skills.combat += skills.growth_rate() * d.fight_combat_skill_growth;
        needs.safety = (needs.safety + d.fight_safety_gain).min(1.0);
        needs.mastery = (needs.mastery + d.fight_mastery_gain).min(1.0);
        StepOutcome::witnessed(StepResult::Advance)
    } else {
        StepOutcome::unwitnessed(StepResult::Continue)
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
        let health = Health {
            current: 0.2,
            max: 1.0,
            injuries: Vec::new(),
            total_starvation_damage: 0.0,
        };
        let outcome = resolve_fight_threat(0, &mut skills, &mut needs, &health, &d);
        assert!(
            matches!(outcome.result, StepResult::Fail(ref reason) if reason == "morale_break"),
            "expected morale_break Fail, got {:?}",
            outcome.result
        );
        assert!(!outcome.witness, "bail path must not set witness");
    }

    #[test]
    fn fight_continues_when_health_above_threshold() {
        let d = defaults();
        let mut skills = Skills::default();
        let mut needs = Needs::default();
        let health = Health::default();
        let outcome = resolve_fight_threat(0, &mut skills, &mut needs, &health, &d);
        assert!(matches!(outcome.result, StepResult::Continue));
        assert!(!outcome.witness, "mid-fight Continue must not set witness");
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
        let outcome = resolve_fight_threat(d.fight_duration, &mut skills, &mut needs, &health, &d);
        assert!(matches!(outcome.result, StepResult::Advance));
        assert!(
            outcome.witness,
            "Advance must set witness for Feature emission"
        );
        assert!(needs.safety > 0.5, "safety should have increased");
    }

    #[test]
    fn fight_bails_even_at_duration_tick() {
        let d = defaults();
        let mut skills = Skills::default();
        let mut needs = Needs::default();
        let health = Health {
            current: 0.1,
            max: 1.0,
            injuries: Vec::new(),
            total_starvation_damage: 0.0,
        };
        let outcome = resolve_fight_threat(d.fight_duration, &mut skills, &mut needs, &health, &d);
        assert!(
            matches!(outcome.result, StepResult::Fail(ref reason) if reason == "morale_break"),
            "bail should fire even at fight_duration tick"
        );
        assert!(!outcome.witness);
    }
}
