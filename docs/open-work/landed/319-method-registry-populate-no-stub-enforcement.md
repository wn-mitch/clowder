---
id: 319
title: Method registry — populate + no-stub enforcement
status: done
cluster: tooling-diagnostics-ui
initiative: [smarter-cats]
added: 2026-05-14
parked: null
blocked-by: []
supersedes: []
related-systems: [htn-methods.md]
related-balance: []
landed-at: 6c04071b9925
landed-on: 2026-05-14
---

## Why

128 epic infrastructure. The HTN method registry is the single
source of truth for which methods exist, which are live, and
which are dormant pending substrate. This ticket lands the
registry primitives and the enforcement script that makes the
"every dormant method has a glue ticket" discipline a CI gate.

Without this, dormant methods leak into the codebase without
glue tickets and the natural-trees-never-sprout failure mode
takes over.

## Scope

- `src/ai/methods/mod.rs` — `Method`, `SubGoal`, `MethodFailure`,
  `MethodId`, `ApplicableWhen` types per
  [`docs/systems/htn-methods.md`](../../systems/htn-methods.md)
  §Architecture.
- `MethodRegistry` resource + `populate_method_registry` system
  function (parallel to `populate_dse_registry` /
  `populate_influence_map_registry` from ticket 207).
- `scripts/check_method_registry.sh` — verifies (1) every
  `ApplicableWhen::PendingSubstrate { blocker }` names an **open**
  ticket in `docs/open-work/tickets/`, AND (2) that ticket's
  frontmatter carries `wires-method: [<method-id>...]`
  referencing back. Both directions enforced.
- `scripts/methods.allowlist` — escape valve mirroring
  `scripts/substrate_stubs.allowlist`.
- `just methods --pending` — audit-surface recipe listing all
  PendingSubstrate methods with their blockers.
- Wire `check_method_registry.sh` into `just check`.

## Out of scope

- Authoring any specific methods (those are #320 onward).
- Extending `just open-ticket` with `--wires-method` (deferred —
  this session's children get their `wires-method` field via
  post-open frontmatter Edit, not via script flag).

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
This is child #1 of 25; gates 22 of the remaining 24 children
(parallel with #322 in Batch A).

## Approach

Mirror ticket 207's `populate_influence_map_registry` +
`check_influence_map_registry.sh` shape exactly. The data
structure choice is `&'static [Method]` const tables per
F.E.A.R.-style HTN (per htn-methods.md §Literature alignment).

`ApplicableWhen` is the typed-dormancy enum from htn-methods.md
§Dormant-method discipline:

```rust
pub enum ApplicableWhen {
    Live(fn(&World, Entity) -> bool),
    PendingSubstrate {
        blocker: &'static str,
        eventual: fn(&World, Entity) -> bool,
    },
}
```

`MethodRegistry::lookup` returns `None` for `PendingSubstrate`
methods. The L2 evaluator's no-method fallback (existing 126
adoption path) handles non-decomposed goals.

## Verification

- `cargo check --all-targets` passes.
- `just check` runs `check_method_registry.sh`; passes when no
  methods are registered yet (vacuously) and fails if a
  PendingSubstrate method lands without its glue ticket.
- `just methods --pending` runs and prints empty list (no
  methods registered yet).
- Manual test: register a stub `PendingSubstrate` method with
  a fake blocker, confirm `just check` fails.

## Log

- 2026-05-14: opened as 128 epic child #1 (Batch A infrastructure).
- 2026-05-14: 2026-05-14: landed. MethodRegistry + populate fn + scripts/check_method_registry.sh bidirectional gate + just methods audit surface (single parse source-of-truth: bash --list-json, Python formatter). Empty registry at landing (vacuous-pass); first PendingSubstrate method in 320+ exercises the bidirectional check. Bundled with pre-session 128 epic kickoff (htn-methods.md design doc, tickets 319-343 opened, CLAUDE.md HTN-discipline + dormant-method-glue sections, 060/128 epic dashboards updated).
