---
id: 460
title: Ward placement rate over-shoots after magic gate retirement
status: ready
cluster: magic-mythic
orchestration: substrate-sensitive
initiative: [mythic-texture]
added: 2026-05-23
parked: null
blocked-by: []
supersedes: []
related-systems: [project-vision.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 004 retired the `magic_affinity / magic_skill > threshold` outer
gate on the six magic DSEs. The ticket-004 verdict observed the predicted
ecological shift: cats with low affinity now participate in magic. But
the magnitude over-shot the prediction — `wards_placed_total` rose
+1050.4% per 10kt (4 → 21 raw wards in a half-duration soak), with
`ward_count_final` going from baseline 0 to observed 1 and
`ward_avg_strength_final` rising from 0 to 0.73. Whole-colony
`structures_built` rose +200% as cats redirected from social into
ward-placement.

The substrate cause: each magic DSE composes considerations as
`CompensatedProduct`, which is geometric-mean shaped. A kitten with
`magic_skill = 0.1` still scores ~0.5 in the CP composition (with other
axes around 0.5), because the geometric mean is more forgiving than the
plan assumed. The retired binary gate was the de facto suppressor;
without it, the CP shape isn't strict enough to keep low-skill cats away
from DurableWard when `WardStrengthLow` fires.

## Scope

- Audit per-DSE consideration curves on DurableWard / Cleanse / ColonyCleanse
  in `src/ai/dses/practice_magic.rs`. Decide between:
  - **Polynomial suppressor on `magic_skill`** (e.g., `Curve::Polynomial
    { exponent: 2, divisor: 1.0 }`) — kittens at skill=0.1 drop to 0.01
    instead of 0.1.
  - **Add a soft `magic_affinity` axis** (ticket 458's scope) and let
    high-affinity-low-skill cats still feel the pull while low-affinity-
    low-skill cats stay suppressed. May supersede this ticket if 458
    lands first.
- Run `just hypothesize` end-to-end. Predict: ward placement rate drops
  back into a sustainable range (target: 4–8 wards per 100k ticks, with
  the substrate still reachable for the predicted low-affinity adult
  participation).
- Bound `wards_placed_total` drift to ≤ +200%/10kt (significant but
  proportionate).

## Out of scope

- Re-introducing the binary affinity gate (the ticket-004 retirement is
  load-bearing; revert is not the right shape).
- Ward semantics / decay / strength curves (those are separate balance
  knobs in `WardConstants`).

## Current state

Ticket 004 landed 2026-05-23 (this session). Ticket 458 (soft considerations)
opened alongside it to give kittens with high affinity / no skill a path
into magic via a positive-intercept skill curve + an affinity axis.
This ticket (460) and 458 may interact — 458's curve softening could
*worsen* the ward over-shoot if not paired with a magic_skill suppressor
on DurableWard specifically.

## Approach

1. Run a baseline-rebuild soak post-004 (clean tree) to capture a
   true post-retirement ward-placement rate, not the rate confounded
   with the joint_intention 2.2× duration drift (see 459).
2. Decide between Polynomial suppression vs Affinity-axis addition.
   Prefer the latter (substrate-honest, mirrors the design intent of
   the project-vision passage) if 458 lands first.
3. Tune the curve. Use `just frame-diff` to confirm the change attributes
   cleanly to the DurableWard / Cleanse rows.

## Verification

1. `just check` / `just test` — gate.
2. `just hypothesize <spec.yaml>` — four-artifact methodology.
3. `just verdict logs/tuned-42-<sha>/` — survival + continuity hold,
   `wards_placed_total` drift band drops from "significant" to
   "noise" or "drift".
4. `just frame-diff <baseline-post-004> <new>` — DurableWard's per-cat
   score reduction is the dominant attribution; sibling magic DSEs
   drift <noise.

## Log

- 2026-05-23: opened from ticket 004's verdict findings. Raw evidence:
  `wards_placed_total` 4 → 21 (rate baseline 0.335/10kt → observed
  3.857/10kt = +1050%); `ward_count_final` 0 → 1; `ward_avg_strength_final`
  0 → 0.73. Parent commit cf6f36f5; baseline at bc0dcbeb
  (095-phase-1a-shadow). Interaction with 458 noted in Current state.
