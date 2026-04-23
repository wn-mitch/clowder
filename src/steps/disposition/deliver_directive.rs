use crate::components::physical::Needs;
use crate::resources::sim_constants::DispositionConstants;
use crate::steps::{StepOutcome, StepResult};

/// # GOAP step resolver: `DeliverDirective`
///
/// **Real-world effect** — on completion (`ticks >=
/// deliver_directive_duration`), boosts `needs.respect` and
/// `needs.social` on the actor. This is a stub resolver — it does
/// not currently verify that a coordinator actually assigned a
/// directive or that the directive was successfully communicated
/// to the target. See the TODO at the call site.
///
/// **Plan-level preconditions** — emitted under
/// `ZoneIs(PlannerZone::SocialTarget)` by
/// `src/ai/planner/actions.rs::directive_actions`. The coordinator
/// pipeline in `src/systems/coordination.rs` assigns directives
/// separately — this step doesn't check for their presence.
///
/// **Runtime preconditions** — time-only gate. The step
/// intentionally has no target-existence check: it always Advances
/// on time-out. Witness is gated on reaching the Advance branch
/// (needs-side effects applied) — not on any evidence that a real
/// directive was delivered to a real target.
///
/// **Witness** — `StepOutcome<bool>`. `true` iff the Advance
/// branch ran this call. Until the stub is tightened, this is the
/// best we can do — a follow-up could take a
/// `directive_assigned: bool` input.
///
/// **Feature emission** — caller passes
/// `Feature::DirectiveDelivered` (Positive) to
/// `record_if_witnessed`. Previously the caller emitted this on
/// every `Advance` unconditionally — §Phase 5a aligns the gating
/// pattern with the rest of the audit.
pub fn resolve_deliver_directive(
    ticks: u64,
    needs: &mut Needs,
    d: &DispositionConstants,
) -> StepOutcome<bool> {
    if ticks >= d.deliver_directive_duration {
        needs.respect = (needs.respect + d.deliver_directive_respect_gain).min(1.0);
        needs.social = (needs.social + d.deliver_directive_social_gain).min(1.0);
        StepOutcome::witnessed(StepResult::Advance)
    } else {
        StepOutcome::unwitnessed(StepResult::Continue)
    }
}
