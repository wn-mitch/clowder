---
id: 195
title: Plan-template stub-comment lint extension (194 P2)
status: ready
cluster: process-discipline
added: 2026-05-06
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Closes 194 F4 / P2. Ticket 194 traces 189's three-reframe
diagnostic delay back to a plan-template stub at
`src/ai/planner/actions.rs:274-285` — a comment saying *"Reuses
`MaterialPile` zone … until a more general `TargetGroundItem`
zone lands. Default-zero scoring keeps this dormant."* — which
185 violated by lifting PickingUp's curve without first wiring
the named successor zone. The comment was documentation, not a
gate.

Two other stub-shaped comments live at the same file
(`actions.rs:243` "default-zero scoring" and `actions.rs:351`
"until the deliver lands"). Today the corpus is exactly those
three. The intent is to close the failure mode at the lint
layer before the corpus grows.

## Direction

Extend `scripts/check_substrate_stubs.sh` with a third audit
pass:

1. Grep `src/ai/planner/{actions,goap_plan}.rs` for stub-
   shape phrases — *"default-zero"* / *"until X lands"* /
   *"stub"* / *"keeps this dormant"*. Each match becomes an
   implicit allowlist entry keyed by `<file>:<line>:<successor>`
   where `<successor>` is the named-but-not-yet-landed entity
   (zone, marker, DSE).
2. For each stub, the lint requires either:
   (a) an entry in `scripts/substrate_stubs.allowlist` naming
       the ticket that will land the successor, OR
   (b) the corresponding DSE's curve is still default-zero
       (`Linear { slope: 0.0, intercept: 0.0 }`), which keeps
       the stub dormant per its own comment.
3. Lint fails when (b) is violated — i.e., a DSE that uses the
   stubbed plan-template moves to non-zero scoring while the
   successor (e.g., `TargetGroundItem` zone) hasn't landed.

The tricky link is (b) — connecting "this comment" to "this
DSE's curve". A pragmatic first cut: have the stub comment
self-declare the DSE name in a structured tag, e.g.:

```rust
// STUB(picking_up): MaterialPile zone until TargetGroundItem lands.
// Default-zero scoring keeps this dormant.
```

Then the lint can resolve "is `picking_up` curve default-zero?"
by grepping `src/ai/dses/picking_up.rs` for
`Linear { slope: 0.0, intercept: 0.0 }`. This is a small
authorial discipline (existing 3 stubs need a one-line tag
update) in exchange for a hard gate.

## Out of scope

- Reformatting all existing stub comments to a structured
  schema beyond the 3 sites this ticket touches. New stubs
  follow the format; any future audit pass picks up additional
  sites incrementally.
- Auditing non-planner stubs (e.g., system-level "TODO when X
  lands"). Scope is plan-template stubs, where the failure
  mode is documented in 194.

## Verification

- `just check` runs `scripts/check_substrate_stubs.sh` per
  current invocation in `justfile`.
- Manual test: temporarily flip PickingUp's curve to non-zero
  in a scratch branch — the lint should fail naming the
  `MaterialPile` stub, the missing `TargetGroundItem` successor,
  and the absent allowlist entry. Revert.
- Existing allowlist entries must stay clean (no spurious
  failures on already-allowlisted markers).

## Log

- 2026-05-06: opened from 194's closeout. Cluster
  `process-discipline` (new). Three-stub corpus today; gate
  exists to keep it from growing into another 185-shape
  regression.
