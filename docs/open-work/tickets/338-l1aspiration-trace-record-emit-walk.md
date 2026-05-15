---
id: 338
title: L1Aspiration trace record — emit-walk per aspiration
status: ready
cluster: tooling-diagnostics-ui
orchestration: swarm-safe
initiative: [smarter-cats]
added: 2026-05-14
parked: null
blocked-by: []
supersedes: []
related-systems: [htn-methods.md, ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

128 epic trace-surface integration. The L1→L2 emission picker
(#321) is the new transform between aspiration-substrate and
L2 scoring. Per §11.1 Curvature-at-every-layer principle, this
transform needs a Curvature trace record so balance work can
predict-and-verify aspiration emission shifts.

Per
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md)
§Trace + inspection surface, the record shape:

```json
{"layer": "L1Aspiration", "tick": N, "cat": "...",
 "aspiration": "<chain>", "milestone": N,
 "emit_walk": [
   {"label": "...", "applicable": bool, "method_live": bool,
    "emitted": bool},
   ...
 ],
 "fallback_used": bool}
```

## Scope

- New `L1Aspiration` record type in
  `src/resources/trace_log.rs`.
- Emission per focal cat per tick per active aspiration.
- Walks the milestone's `emits[]` table; records per-row
  applicability + method-liveness + emission decision.
- `fallback_used` boolean if the domain-affinity fallback fired.

## Out of scope

- L3Commitment.method_stack (#337).
- CatSnapshot.goal_stack (#339).
- Aggregating L1Aspiration emit-walks across the soak (later
  log-analytics work).

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #20 of 25, blocked on #321. Batch E — parallel with
#337 / #339.

## Approach

Per htn-methods.md §H and §Trace + inspection surface. Registry-
walked emission per §11.5: emitter walks `Aspirations.active`
and the picker's per-aspiration outcome — no hardcoded
aspiration-name emission.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes.
- `just soak-trace 42 <focal>` on a cat with active aspirations
  produces L1Aspiration records per-tick-per-aspiration with
  populated emit_walk arrays.

## Log

- 2026-05-14: opened as 128 epic child #20 (Batch E cross-cutting).
