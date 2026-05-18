---
id: 207
title: Phase 2D — InfluenceMap registry walk in emit_focal_trace
status: done
cluster: ai-substrate
initiative: []
added: 2026-05-07
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 0ab27b18
landed-on: 2026-05-07
---

## Why

Ticket 206 landed the missing-five-map L1 trace coverage by adding the
five `InfluenceMap` resources to a hand-bundled `L1Maps` `SystemParam`
and walking each one explicitly via an `emit_map!` macro. That closes
the focal-cat scrubber's surface gap, but keeps the hardcoded inline
walk pattern that ticket 048 first flagged for replacement on landing
of `CarcassScentMap`.

In-source pointer:

- `src/systems/trace_emit.rs:156-159` — comment "Still deferred to
  Phase 2D: a true registry walk (`Vec<Box<dyn InfluenceMap>>`
  registered at plugin build time) that would let new maps appear in
  L1 without editing this file. See ticket 207."

The cost of leaving it deferred is small but real: every new
`InfluenceMap` impl now requires a manual edit to two places in
`trace_emit.rs` (the `L1Maps` SystemParam fields *and* the `emit_map!`
walk), and a regression — adding the impl and the resource registration
without the trace_emit edits — silently drops trace coverage again.
This is exactly the original gap that 206 closed, primed to repeat.

## Scope

Replace the hand-bundled `L1Maps` SystemParam + 12-call `emit_map!`
block with a single registry walk.

Likely shape:

- A new resource — `InfluenceMapRegistry` or similar — carrying
  `Vec<Box<dyn InfluenceMap + Send + Sync>>`.
- Each `update_*_map` system's plugin-build site (probably co-located
  in `SimulationPlugin::build()` next to the existing `add_systems`
  for that map's update) registers the map into the registry at
  startup.
- `emit_focal_trace` reads `Res<InfluenceMapRegistry>` and iterates,
  calling `emit_l1_for_map` for each entry. The `TileMap` →
  `CorruptionLens` adapter either becomes a registered impl in its
  own right or stays special-cased (the lens borrows `&TileMap`, so
  it's not directly boxable; the adapter probably needs a wrapper
  resource).

`emit_l1_for_map` itself is already generic over `M: InfluenceMap +
?Sized`, so no callee work — this is a registration-side rewrite.

The trace surface (12 distinct `map` field values per planning tick)
is the contract that must hold pre/post: `/logq trace --layer L1` over
a soak-trace run should show the same twelve `map` keys before and
after, in the same order.

### Out of scope

- New maps. Adding a 13th map is out of scope for this refactor; the
  point is that the next addition no longer requires a `trace_emit.rs`
  edit.
- Per-prey-species `PreyScentMap` split (ticket 062, now blocked-by
  207).
- Any scoring-side changes — this is a trace-surface refactor.
- Registry-driven L2 / L3 emission. Phase 3 of the substrate refactor
  covers L2's per-DSE registry walk; that's a separate arc.

### Lint enforcement (added on 2026-05-07)

Pulled into scope from the original "structural design replaces the
missing lint" framing. The structural change closes the regression
vector by collapsing two edit sites into one, but a lint hardens
that property:

- `scripts/check_influence_map_registry.sh` — enumerates every
  `impl InfluenceMap for <Type>` in `src/`, verifies each has a
  paired `register::<<Type>>()` or `register_with(...)` call in
  `populate_influence_map_registry`, or an allowlist entry.
- `scripts/influence_map_registry.allowlist` — empty at landing;
  format mirrors `scripts/substrate_stubs.allowlist`.
- `just check` invokes the lint after the existing
  substrate-stub / step-resolver / time-unit lints.
- CLAUDE.md "Conventions" gains a new paragraph naming the rule and
  the enforcement script (parallel to the substrate-stubs paragraph
  ticket 160 added).

## Verification

- `just check` — substrate-stub + step-resolver + time-unit lints pass.
- `just test` — workspace tests pass; integration tests cover.
- `just soak-trace 42 Simba` — emits `logs/tuned-42-NEW/trace-Simba.jsonl`.
- `/logq trace logs/tuned-42-NEW --layer L1` — twelve distinct `map`
  values, same set as the post-206 baseline.
- `just frame-diff <baseline> logs/tuned-42-NEW` — zero L2/L3 drift;
  L1 row counts match exactly.

## Log

- 2026-05-07 — Opened on landing of ticket 206 per CLAUDE.md
  "Antipattern migration follow-ups are non-optional." 206 explicitly
  defers this in its `## What does NOT land` section.
- 2026-05-07 — Landed. `InfluenceMapRegistry` (Vec<L1Walker>)
  populated at startup by `populate_influence_map_registry`;
  `emit_focal_trace` converted to exclusive system that walks the
  registry. 12 impls registered: 11 Resource-backed via
  `register::<M>()`, 1 borrow-adapter (CorruptionLens) via
  `register_with`. Lint `scripts/check_influence_map_registry.sh`
  pulled into scope and wired into `just check`. CLAUDE.md gains
  "InfluenceMap registry stubs are forbidden" Conventions paragraph.
  062 flipped to `blocked-by: [207]` with Step 8 rewritten to
  register five `PerSpeciesScentRef` walkers via the new registry
  instead of editing `trace_emit.rs`. Verification: `just check`
  passes (lint reports "12 impl(s), all registered"); `just test`
  passes 1922 tests including new `registry_register_walks_resource_maps`
  + `registry_register_with_handles_borrow_adapter`; `/logq trace`
  confirms 12 distinct L1 map keys; `frame-diff` vs post-206
  baseline reports "no unacknowledged drift on tracked DSEs."
  Lint negative-case proven by temporarily commenting out a
  `register::<>()` call (lint exit 1 with offender named).
