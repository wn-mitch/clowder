---
id: 337
title: L3Commitment trace gains method_stack field
status: blocked
cluster: tooling-diagnostics-ui
initiative: [smarter-cats]
added: 2026-05-14
parked: null
blocked-by: [320]
supersedes: []
related-systems: [htn-methods.md, ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

128 epic trace-surface integration. The §11 instrumentation
invariant says trace records walk registries (§11.5). This
ticket extends the L3Commitment record per
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md)
§Trace + inspection surface:

```json
{"layer": "L3", "method_stack": [
   {"method": "...", "goal": "...", "sub_goal_index": N,
    "of": M, "target": "...", "source": "..."},
   ...
 ],
 "leaf_intention": { ... }}
```

The emitter walks `HeldGoalStack.frames` at trace time — no
hardcoded per-method emission code, mirroring §5.6.9's L1
extensibility anti-goal.

## Scope

- Extend `L3Commitment` struct in
  `src/resources/trace_log.rs` with a `method_stack:
  Vec<MethodFrameTraceRecord>` field.
- `MethodFrameTraceRecord` carries: `method` (slug), `goal`
  (slug), `sub_goal_index`, `sub_goal_count`, `target` (stable
  slug not Entity), `source` string.
- Registry-walked emission: the focal-cat trace emitter reads
  the cat's `HeldGoalStack`, walks frames, populates records.
- Backward-compat: pre-128 trace records have an empty
  method_stack; diff tooling treats absence as empty.

## Out of scope

- The CatSnapshot snapshot field (#339).
- The L1Aspiration record (#338).
- Inspect-rendering (#336).

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #19 of 25, blocked on #320. Batch E — parallel with
#338 / #339.

## Approach

Per htn-methods.md §Trace + inspection surface. Schema extension
to existing L3Commitment record. Per §11.5 invariant: no
method-specific emission code.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes.
- `just soak-trace 42 <focal>` on a cat with active methods
  produces a trace where every L3 line has a populated
  `method_stack` array.

## Log

- 2026-05-14: opened as 128 epic child #19 (Batch E cross-cutting).
