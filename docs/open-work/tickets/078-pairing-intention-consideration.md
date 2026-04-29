---
id: 078
title: Backport bond_score's Intention pin to a target_pairing_intention Consideration
status: blocked
cluster: planning-substrate
added: 2026-04-29
parked: null
blocked-by: [072]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [027-l2-pairing-activity.md]
landed-at: null
landed-on: null
---

## Why

027b Commit B's `bond_score` at `src/ai/dses/socialize_target.rs:193` contains:

```
if pairing_partner == Some(target) {
    return 1.0;
}
```

This is a **post-IAUS pin** that subverts the partner-bond consideration's curve. It's the same MacGyvered pattern the planning-substrate hardening sub-epic explicitly closes — branching on a special case in a resolver body to override the IAUS pick rather than expressing the Intention partner's lift through the score economy.

This ticket replaces the pin with a first-class IAUS Consideration. The Intention partner's lift now flows through the same engine as fondness / novelty / partner-bond / species-compat / cooldown — fully traceable in the score breakdown, tunable via curve parameters, and inspectable in the modifier-pipeline trace.

Parent: ticket 071. Blocked by ticket 072 (`plan_substrate` API publishes `PAIRING_INTENTION_INPUT`).

## Scope

- Restore `src/ai/dses/socialize_target.rs:187–195`'s `bond_score` to its pre-027b graduated form (Friends → 0.5, Partners/Mates → 1.0, None → 0.0). Drop the `pairing_partner: Option<Entity>` parameter.
- Drop the `pairing_partner` argument from `resolve_socialize_target`'s signature (currently line 251). Drop `unpinned_bond_score` (line 208) — no longer needed once the pin is gone.
- Drop the `pairing_partner` lookup from `goap.rs:2982–3008` (the resolver-call site that adds it). Drop `pairing_q` from `ExecutorContext` (line 388) if no other resolver consumes it directly.
- New IAUS sensor `target_pairing_intention(cat, target) -> f32` published on `EvalInputs` (`src/ai/scoring.rs:30`). Reads `PairingActivity` directly: returns 1.0 if `target == pairing.partner` else 0.0.
- New `Consideration::Scalar(ScalarConsideration::new(plan_substrate::PAIRING_INTENTION_INPUT, intention_cliff_curve))` on `socialize_target_dse()` as the next axis. Cliff curve: `Piecewise [(0.0, 0.0), (0.5, 0.0), (0.5, 1.0), (1.0, 1.0)]` (or equivalent step shape; the input is binary 0/1 so a simple Linear or Cliff with breakpoint at 0.5 also works).
- Renormalize the existing axis weights so steady-state scores match pre-078 on a non-Intention target.
- `Feature::PairingBiasApplied` continues firing from a wrapper sensor that compares the IAUS pick against the Intention partner — same observability, expressed cleanly through the engine.

## Out of scope

- Changing the L2 PairingActivity author or drop logic — those are correct as-is.
- Adding similar Intention-coherence considerations to other target DSEs (`groom_other_target`, `mate_target`) — 027b explicitly scoped those to ticket 027c. This ticket only backports the existing pin.
- Ticket 027b's reactivation (line uncomment) — that's ticket 082.

## Approach

Files:

- `src/ai/dses/socialize_target.rs:83` (`socialize_target_dse()`) — add the Consideration with `intention_cliff_curve`; renormalize weights.
- `src/ai/dses/socialize_target.rs:187–215` — strip the pin from `bond_score`; drop `pairing_partner` parameter from `bond_score` and delete `unpinned_bond_score`.
- `src/ai/dses/socialize_target.rs::resolve_socialize_target` (entry ~244) — drop the `pairing_partner: Option<Entity>` parameter; the IAUS picks the Intention partner via the new consideration.
- `src/ai/scoring.rs::EvalInputs` — publish the `target_pairing_intention(cat, target) -> f32` sensor reading `PairingActivity` directly (registered with `plan_substrate::PAIRING_INTENTION_INPUT` per 072's constant).
- `src/systems/goap.rs:2982–3008` — drop the `pairing_partner` lookup from the resolver-call site.
- `src/systems/goap.rs::ExecutorContext::pairing_q` (line 388) — remove if no remaining direct readers (sensor reads `PairingActivity` via the IAUS path now).

## Verification

- `just check && just test` green.
- `just check` includes `check_iaus_coherence.sh` (ticket 079) and passes — the pin is gone.
- **Regression-on-purpose unit test**: with a Friends-bonded Intention partner, the IAUS path produces the same final score as the old pin within fp tolerance. The pin's effect is preserved; only the expression changes.
- Existing test `bond_score_pins_pairing_intention_partner_at_one` (currently at `socialize_target.rs:537–559`) is rewritten to assert the consideration's contribution, not the pin's return value.
- `just soak 42 && just verdict logs/tuned-42-078` — hard gates pass; behavior should be bit-identical (or near-identical within fp tolerance) to pre-078 since the consideration replicates the pin's effect.

## Log

- 2026-04-29: Opened under sub-epic 071.
