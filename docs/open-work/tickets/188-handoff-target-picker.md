---
id: 188
title: Handoff target-picker — pick the recipient cat at L2
status: ready
cluster: ai-substrate
added: 2026-05-06
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 177 wired the Handoff dispatch arm but explicitly parked
recipient-picking — for stage-1 wiring, the handoff target is
threaded as `target_entity` from whatever produces the
`DispositionKind::Handing`, which today is **nothing** (the
Handing DSE ships default-zero, no L2 target picker exists).

Once 178 lifts disposal DSE weights and Handing actually fires,
the dispatch arm will Fail with `"handoff: no recipient on
disposition"` on every attempt — there's no system populating
`target_entity` for Handing dispositions. The L2 target DSE
needs to pick a recipient (proximity? hunger gradient?
fondness? leader-coordinator?) and thread the choice into
`Disposition::target_entity` the way `groom_other_target` and
`socialize_target` already do for their respective dispositions.

Parked rather than ready because:
1. 178 must land first — without disposal DSE weights, this
   ticket can't be exercised at runtime.
2. The "right" recipient policy is a balance question, not a
   wiring question. Defer to balance work after 178 surfaces
   the actual handoff-rate signal.

## Direction (sketch)

- Add a `handing_target_dse` mirroring
  `src/ai/dses/groom_other_target.rs` — scores candidate
  recipient cats by proximity + hunger + fondness or some
  combination grounded in observable substrate.
- Wire it into `populate_dse_registry` in
  `src/plugins/simulation.rs`.
- Hook the picker into the Handing disposition planning path
  the same way `resolve_groom_other_target` is called from
  `goap.rs` around the `GroomOther` arm.

## Out of scope

- The recipient-picking policy itself (balance, not structural).
- Multi-recipient broadcast / coordinator-directed handoff.

## Log

- 2026-05-06: opened by 177's closeout. 177 explicitly parked
  this scope ("for stage-1 wiring the handoff target can be
  threaded as `target_entity` from the disposition layer";
  no such layer exists today). Becomes load-bearing as soon as
  178 lifts the Handing DSE above default-zero.
- 2026-05-06: unparked. Ticket 178 landed; the Handing DSE
  eligibility filter now requires `HasHandoffRecipient`
  (allowlisted in `scripts/substrate_stubs.allowlist`). This
  ticket lands the marker writer alongside the curve lift.
  Removing the allowlist entry is a same-commit step on land
  day.
