---
id: 342
title: Phase-2 :unordered method bodies + partial-order
status: parked
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: [smarter-cats]
added: 2026-05-14
parked: 2026-05-14
blocked-by: []
supersedes: []
related-systems: [htn-methods.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

128 epic Phase-2 enrichment, parked at open-time. SHOP2 supports
`:unordered` keyword on method bodies for partial-order
decomposition — sub-goals that can execute in any order. F.E.A.R.-
style total-order HTN (the Phase-1 commit) only supports ordered
task lists.

Parked because:
1. Authoring complexity is real — unordered task bodies need a
   different decomposition algorithm and a different trace
   surface.
2. No Phase-1 method authored to date demonstrates need.
3. The L2 softmax already handles cross-Intention parallelism;
   partial-order within one method is a different concern.

Unparks when a real use case demands parallel sub-goals (e.g., a
ceremonial method with multiple participants each performing one
of several ordered-among-themselves sub-tasks). Document the
trigger condition in the unpark log entry.

## Scope (when unparked)

- Add `unordered: bool` field to `Method` shape.
- Implement non-deterministic sub-goal selection within
  unordered bodies (probably softmax over applicable next
  sub-goals at each advance step).
- Extend trace surface to carry the chosen ordering per-cat
  per-frame.
- Update enforcement script if needed.

## Out of scope

- Phase-1 work (all ordered methods; this ticket exists only as
  a placeholder for future enrichment).

## Current state

Parked 2026-05-14. Per
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md)
§Future / out of scope.

## Log

- 2026-05-14: opened parked as 128 epic child #24 (Phase-2
  future enrichment).
