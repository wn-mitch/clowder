---
id: 339
title: CatSnapshot gains goal_stack + active_aspirations fields
status: ready
cluster: tooling-diagnostics-ui
orchestration: substrate-sensitive
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

128 epic snapshot-surface integration. Cross-run analysis tools
(`logdb`, `just inspect`, `just verdict`) read from the
`events.jsonl` `CatSnapshot` line, not from per-focal trace
sidecars. The aspirational layer needs a per-tick snapshot
projection so these tools can render and query aspiration state.

Per
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md)
§Trace + inspection surface:

```rust
pub struct CatSnapshot {
    // ... existing fields ...
    pub goal_stack: Vec<GoalFrameSnapshot>,
    pub active_aspirations: Vec<AspirationSnapshot>,
}

pub struct GoalFrameSnapshot {
    pub method: String,
    pub goal_label: String,
    pub sub_goal_index: usize,
    pub sub_goal_count: usize,
    pub target: Option<String>,
    pub source: String,
}
```

## Scope

- Extend `CatSnapshot` in `src/resources/event_log.rs` with
  `goal_stack` + `active_aspirations` fields.
- Snapshot emitter reads `HeldGoalStack` + `Aspirations` and
  serializes per-frame state with stable string slugs (no
  Entity refs).
- Backward-compat: older readers tolerate the new fields via
  `#[serde(default)]` per `SimConstants` precedent.

## Out of scope

- The per-focal L3 trace surface (#337).
- The L1Aspiration trace record (#338).
- Inspect-rendering (#336 consumes this ticket's output).
- logdb schema extension (separate ticket if needed — schema
  evolution is downstream of this field landing).

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #21 of 25, blocked on #320 + #321. Batch E — parallel
with #336-#338.

## Approach

Per htn-methods.md §Trace + inspection surface. Snapshot emitter
in `event_log.rs` walks the cat's components at snapshot time.
Stable slug serialization (string method id, not method-id
ordinal) preserves cross-run comparability.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes (header-comparability invariant
  preserved — new fields don't bump constants).
- `just soak 42` produces `events.jsonl` with populated
  goal_stack + active_aspirations on CatSnapshot lines.
- `cargo run --example inspect_cat` reads the new fields and
  renders the new aspiration section (#336 dependency).

## Log

- 2026-05-14: opened as 128 epic child #21 (Batch E cross-cutting).
