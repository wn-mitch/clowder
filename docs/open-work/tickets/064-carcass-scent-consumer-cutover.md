---
id: 064
title: Carcass-scent consumer cutover — replace observer_smells_at (§5.6.3 row #6)
status: ready
cluster: null
added: 2026-04-27
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 048 landed `CarcassScentMap` as substrate-only — the producer
seeds the map per tick, but the consumer at `src/systems/goap.rs:1133–
1145` still uses the per-pair `observer_smells_at` sensing loop. The
spec (§5.6.3 row #6) calls for the carcass-aware DSEs (`HarvestCarcass`,
`CleanseCarcass`, scavenging foxes) to read the map directly via
`SpatialConsideration` rather than redo the per-pair scan.

Two motivations:
1. **Performance** — the per-pair `observer_smells_at` loop is O(cats ×
   carcasses) per tick. The map is O(cats) — one lookup per cat per
   tick.
2. **Substrate uniformity** — leaving carcass scent on a different
   sensing path than fox scent / prey scent makes the §5.6.6
   attenuation pipeline harder to reason about.

This is **balance-affecting**. The map's spatial grid resolution is
coarser than the per-pair scan's tile-exact distance check; behavior
shifts are expected. Per CLAUDE.md, drift > ±10% on a characteristic
metric requires hypothesis + soak verdict.

## Scope

1. **Replace the consumer** at `goap.rs:1133–1145`. The
   `observer_smells_at(carcass_scent_emitter)` call becomes a
   `CarcassScentMap::get(observer_pos)` read.
2. **Wire `SpatialConsideration`** for any DSE that currently uses
   the boolean `carcass_nearby` marker as an additive score axis
   (rather than a pure eligibility gate). Specifically: scavenger
   foxes' `Scavenging` disposition.
3. **Remove the dead per-pair path** if no other consumer reads it
   after step 1. (Spot-check: `prey_scent` path stays; this only
   touches the carcass branch.)
4. **Hypothesis + soak.** Predicted shifts:
   - More carcass discovery at mid-range (the map smooths the gradient
     so cats at distances 5–10 tiles get a stronger signal than they
     do today via the binary-radius observer-smells check).
   - Same total carcass-harvest count (the spatial coverage is
     equivalent in expectation; only the per-cat noise pattern shifts).
   Direction match + magnitude within ~2× of prediction → concordance.

## Out of scope

- Changing the `CarcassScentMap` producer (already correct).
- Sensitivity tuning of the carcass-scent decay rate (separate balance
  thread if the soak surfaces drift).

## Verification

- Lib tests: `goap.rs` carcass-aware unit tests update to read from
  the map; coverage stays equivalent.
- `just soak 42` to fresh log directory; `just verdict` must return
  exit 0.
- `just hypothesize` four-artifact run if the soak surfaces > ±10%
  drift on a characteristic metric (carcass-related: harvest count,
  cleansed-carcass count, scavenger-fed-cubs count).
- Focal trace: confirm `carcass_scent` sampling appears in L1
  records for cats with carcass-aware DSEs eligible.

## Log

- 2026-04-27: opened from ticket 006 closeout. Inherits the
  deferral ticket 048 logged when `CarcassScentMap` substrate
  landed.
