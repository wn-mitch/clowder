---
id: 317
title: retire flee_threat_distance Power-Invert if frame-diff shows redundancy with Affordance(Flee)
status: blocked
cluster: ai-substrate
initiative: []
added: 2026-05-13
parked: null
blocked-by: [315]
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

After 315 activates `flee_affordance_weight`, the Flee DSE carries two axes that both encode proximity-to-threat: the legacy `flee_threat_distance` (Power-Invert curve on the NearestThreat anchor, range=12) and the new `flee_affordance` (which the affordance writer composes as `(1 - proximity)` plus cover_self / my_health / violence_cap). If a frame-diff between the activation soak and the pre-263 baseline shows the two axes drive Flee similarly on the same threat conditions, the Power-Invert axis is redundant and adds dimension without information. Conditional on data, not on principle — keep this ticket explicitly contingent on the activation soak's measured shape.

## Scope

- Run `just frame-diff <pre-263-baseline> <post-315-activation>` and inspect the Flee DSE row.
- If the per-axis contribution of `flee_affordance` saturates the dimension `flee_threat_distance` previously occupied (correlation > 0.9 across focal trace ticks), retire the Power-Invert axis.
- If retirement: delete the Spatial consideration in `src/ai/dses/flee.rs:124–130`, drop its weight from the CP, regenerate unit tests, and verify no soak regression.
- If NOT redundant (axes carry independent signal): close this ticket as "verified non-redundant" with the frame-diff evidence stored in the Log.

## Out of scope

- Activating any axes — 315 owns activation.
- Retiring `health_deficit` or other Flee axes — those are orthogonal (interoceptive vs exteroceptive).

## Current state

Blocked-by 315. The decision flips on activation-soak data.

## Approach

1. Read the frame-diff output for the Flee DSE.
2. If correlation evidence supports retirement, edit `src/ai/dses/flee.rs` to drop the Power-Invert axis + weight; update tests; run `just soak` + `just verdict`.
3. Add a Log entry capturing the frame-diff numbers either way.

## Verification

- Frame-diff numbers in the Log support the retirement (or rule it out).
- Soak post-retirement shows no canary regression vs the 315-activation baseline.
- Flee DSE's L2 score distribution stays within ±10% of the 315-activation baseline.

## Log

- 2026-05-13: opened as 263 follow-on after the activation pathway was scoped. Decision conditional on 315's measured shape.
