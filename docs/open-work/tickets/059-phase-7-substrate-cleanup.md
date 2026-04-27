---
id: 059
title: Phase 7 substrate cleanup — `ScoringContext` removal, §10 unblock map sweep, spec drift reconcile
status: parked
cluster: null
added: 2026-04-27
parked: 2026-04-27
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Phase 7 of the AI substrate refactor (`docs/systems/ai-substrate-refactor.md`)
calls for a cleanup pass after the structural work lands: delete the legacy
`ScoringContext` / `FoxScoringContext` types entirely, refresh
`docs/wiki/systems.md`'s §10 unblock map (Aspirational → Built/Partial as
features have landed), and reconcile spec-vs-code drift (named example:
ticket 044 moved `hangry` midpoint 0.75 → 0.5 in code, but the spec doc
still says 0.75 in 9 places).

Today the cleanup is happening incrementally inside other tickets — 027
deleted `ScoringContext.has_eligible_mate`, 051 will retire 7
`FoxScoringContext` boolean fields. But there's no ticket that owns
"finish the cleanup and reconcile the doc." This ticket is the catch-all
for that drift, opened so the dropped-pieces concern has an explicit home.

## Scope

1. **Delete `ScoringContext` entirely.** After 052 (§L2.10.7 plan-cost
   feedback) lands its consumer surface, the remaining
   `ScoringContext` fields should all have marker-or-snapshot
   equivalents. Whittle the residual fields down to zero, then
   delete the struct + its `build_scoring_context` function.
2. **Delete `FoxScoringContext` entirely.** Pair with 051's
   field-retire; after that lands, finish the type removal.
3. **Sweep `docs/wiki/systems.md` §10 unblock map.** Each row's
   status (Built / Partial / Aspirational) should reflect 2026-04-27
   reality. Ticket 014 closeout, 045/048 influence maps, and Phase
   6a commitment gate have all moved rows; the unblock map likely
   has stale entries. Auto-regenerated via `just wiki` from
   `SimulationPlugin::build()`, but the §10 narrative may need a
   hand-edit.
4. **Reconcile spec-vs-code drift.** Sweep `ai-substrate-refactor.md`
   for stale numbers / outdated TODOs / references to retired
   constants. Known examples:
   - `hangry` midpoint cited as 0.75 in 9 places; code on 0.5 since
     ticket 044 (2026-04-27).
   - "absent pending PR" callouts on shipped components (e.g.
     §7.M.7.5 Fertility phase mapping — the component lives in
     `src/components/fertility.rs` already).
   - References to `has_active_disposition` (§3.5.3 / §4.4) — flag
     for deletion if still in code; remove from spec.

## Out of scope

- **Per-feature ticket work that survives the cleanup.** This ticket
  doesn't carry feature work or balance changes; it's drift
  reconciliation only.
- **The §10 *features* themselves** (Environmental Quality, Sensory,
  Body Zones, Mental Breaks, Recreation, Disease, Sleep, Calling,
  Fox/Hawk AI parity, Strategist Coordinator). Each is its own
  epic; this ticket only touches their *status* in the unblock map,
  not their implementation.
- **Editing `refactor-plan.md`.** That doc is a plan-of-record,
  archival; leave it as-is.

## Current state

Parked at open. Pick up after ticket 052 (§L2.10.7 plan-cost
feedback) lands so the `ScoringContext` field-removal sweep happens
*after* the last new structural consumer of `ScoringContext` is
written. Doing it before 052 would require re-touching the type
once 052 lands.

## Approach

Sequence as three commits:

1. `refactor: delete ScoringContext + FoxScoringContext` — the
   delete-after-052 commit. Zero behavior change; tests guarantee
   the marker-snapshot path replaces every read site.
2. `docs: sweep §10 unblock map status` — `just wiki` regen + any
   needed hand-edit of the §10 narrative section.
3. `docs: reconcile ai-substrate-refactor spec-vs-code drift` — the
   numeric and TODO sweep. Cross-reference each landed ticket's
   "spec deviation" notes (ticket 044's hangry note is the model).

## Verification

- `just check` clean per commit.
- `just test` 1432+ tests still passing post-`ScoringContext`
  removal (no behavior change expected; the type was already a
  marker-snapshot mirror after the cluster-A landing).
- `just soak 42` survives with verdict `pass` post-cleanup commit.
- `docs/wiki/systems.md` §10 reflects 2026-04-27 reality on visual
  inspection.
- `grep -n "0.75" docs/systems/ai-substrate-refactor.md` returns
  zero matches in the hangry-midpoint context.

## Log

- 2026-04-27: opened from substrate-refactor audit. Parked-by-default;
  pick up after ticket 052 lands so the `ScoringContext` field-removal
  sweep happens after the last new structural consumer.
