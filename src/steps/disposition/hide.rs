use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `Hide` (ticket 104)
///
/// **Real-world effect** — none in Phase 1. Hide is dormant (gated
/// behind `HideEligible` marker that has no authoring system at
/// landing), so this resolver is unreachable at runtime today. The
/// Phase 1 contract is that `record_if_witnessed` MUST NOT fire
/// `Feature::HideFreezeFired` until the DSE actually gets selected
/// — which can't happen until both (a) the authoring system lands
/// and (b) modifier 105 (or 142) lifts Hide's score above competing
/// actions.
///
/// When activated in Phase 2/3: the cat holds its current position
/// for `freeze_ticks_remaining` ticks, no movement, no resource
/// consumption. The future "predator-loses-sight-of-frozen-cat"
/// mechanic is a separate sensing-system change, not implemented
/// here.
///
/// **Plan-level preconditions** — none. Hide is anxiety-interrupt
/// class (alongside `Flee` and `Idle`), not GOAP-planned. The
/// `CurrentAction.action = Action::Hide` setter is invoked
/// directly from the disposition layer when the freeze valence
/// modifier (105 or 142) wins the IAUS contest. No `StatePredicate`
/// guards.
///
/// **Runtime preconditions** — `freeze_ticks_remaining > 0`.
/// Returns `StepOutcome::unwitnessed(Continue)` while the counter
/// is positive (the cat is still frozen this tick); decrements via
/// the caller's `&mut u64` after this returns. When the counter
/// hits zero, returns `StepOutcome::unwitnessed(Advance)` so the
/// next-tick re-evaluation picks a follow-up action (typically
/// Idle once the threat has departed). The witness is `Some(())`
/// only on the freeze-completion tick (`Advance` branch) — that's
/// when `Feature::HideFreezeFired` should be recorded.
///
/// **Witness** — `StepOutcome<bool>`. `witnessed(Advance)` on the
/// freeze-completion tick, `unwitnessed` otherwise. The boolean
/// witness shape — rather than `Option<()>` — matches the
/// "feature fired this call?" semantics described in
/// `outcome.rs::Witnessed for bool`.
///
/// **Feature emission** — caller passes `Feature::HideFreezeFired`
/// (Positive) to `record_if_witnessed`. The Feature is classified
/// `expected_to_fire_per_soak() => false` initially (rare event,
/// exempt from the per-seed canary until the colony hits a
/// scenario producing freeze regularly post-105/142 activation).
pub fn resolve_hide(freeze_ticks_remaining: u64) -> StepOutcome<bool> {
    if freeze_ticks_remaining > 0 {
        return StepOutcome::unwitnessed(StepResult::Continue);
    }
    StepOutcome::witnessed(StepResult::Advance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hide_continues_while_counter_positive() {
        let out = resolve_hide(5);
        assert!(matches!(out.result, StepResult::Continue));
        assert!(!out.witness, "Continue ticks must not carry a witness");
    }

    #[test]
    fn hide_advances_with_witness_on_completion() {
        let out = resolve_hide(0);
        assert!(matches!(out.result, StepResult::Advance));
        assert!(
            out.witness,
            "Advance must carry the witness so the caller emits Feature::HideFreezeFired"
        );
    }
}
