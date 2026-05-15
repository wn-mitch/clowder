---
id: 336
title: just inspect renders the goal stack + aspiration set
status: ready
cluster: tooling-diagnostics-ui
orchestration: substrate-sensitive
initiative: [smarter-cats]
added: 2026-05-14
parked: null
blocked-by: []
supersedes: []
related-systems: [htn-methods.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

128 epic inspection-surface integration. The user-load-bearing
constraint at the epic level is: *"track and see via inspection
all cat aspirations like this. All aspirations go through this
layer."*

This ticket extends `examples/inspect_cat.rs` with a new
`print_aspirations` section that renders, per-tick over the run:

- The cat's current aspiration set (`Aspirations.active`).
- The current goal stack (`HeldGoalStack.frames`).
- The current leaf intention (`HeldIntention`).
- Recent method events (`MethodAdopted` / `SubGoalAdvanced` /
  `MethodBacktracked` extracted from CatSnapshot history).

Single source of truth: the inspector reads the same
`HeldGoalStack` + `Aspirations` substrate the trace + snapshot
surfaces read. Single inspection invariant per
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md)
§Inspection surface.

## Scope

- New `print_aspirations(snapshot)` function in
  `examples/inspect_cat.rs`.
- Renders the data from #339's `CatSnapshot.goal_stack +
  active_aspirations` fields.
- Registry-walked per §11.5: the renderer never hardcodes a
  method name; it pulls method id slugs from the snapshot and
  presents them.
- Adds an "aspiration history" panel showing transitions
  (adoption / advancement / abandonment) from the CatSnapshot
  stream.

## Out of scope

- The CatSnapshot fields themselves (#339 owns those).
- The L3 trace's method_stack (#337); this ticket renders
  CatSnapshot only.
- Visual / TUI prettification — text-table is sufficient.

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #18 of 25, blocked on #320 + #321. Batch E — 4-way
parallel with #337-#339.

## Approach

Per htn-methods.md §Inspection invariant. Render the same shape
the spec's worked-example demonstrates (Whiskers' goal stack
mid-acquisition).

## Verification

- `cargo check --all-targets` passes.
- `just check` passes.
- `cargo run --example inspect_cat -- Whiskers --events logs/tuned-42/events.jsonl`
  produces a report with the new aspirations section populated
  (assuming Tier-1 methods are Live in the run).

## Log

- 2026-05-14: opened as 128 epic child #18 (Batch E cross-cutting).
