---
id: 063
title: Ward-strength promotion — first-class spatial axis (§5.6.3 row #3)
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

§5.6.3 row #3 calls for *full ward-strength* sampling as a first-class
spatial axis in scoring. Ticket 045 landed `WardCoverageMap` —
sufficient for ward placement (anti-clustering) but the spec wants
ward strength to participate in *other* DSEs' scoring too: a cat
threatened by a fox should weight retreat directions toward
high-ward-strength tiles; a coordinator deciding where to direct a
patrol should read ward-strength as a coverage axis.

Today the only consumer of the ward map is ward placement itself
(`ward_target.rs` reads it to avoid clustering). All other ward-aware
scoring still goes through aggregated boolean predicates
(`ward_strength_low`, `wards_under_siege`).

Inherits from ticket 045's "full ward-strength promotion still
deferred" landing log.

## Scope

1. **Audit consumers.** List every DSE / scoring site that today
   takes a boolean ward predicate and could instead read a continuous
   ward-strength sample. Candidates:
   - `Flee` / `Retreat` — prefer high-ward retreat directions.
   - `Patrol` (if it lands as a target-taking DSE) — coverage gradient.
   - `Caretake` target ranking — keep kittens in warded cells.
2. **`SpatialConsideration` cutover** for each audited consumer.
   Replace the boolean marker read with a `SpatialConsideration`
   sampling `"ward_coverage"` (already exists from ticket 045) at the
   relevant `CenterPolicy` (self for retreat, target for caretake).
3. **Decay path** — siege-pressure decay on `WardCoverageMap` (currently
   1.0 — wards never decay through the map's lens) is the *other*
   missing piece. Wire the per-tick reduction so wards under siege
   surface as falling map values rather than only via the
   `WardsUnderSiege` boolean.

## Out of scope

- Re-architecting `Ward` entity → map snapshot pipeline beyond the
  decay path (the substrate from ticket 045 is sound).
- Per-DSE numeric balance tuning (lives in ticket 052 + balance threads).
- Adding new ward kinds.

## Verification

- Lib tests: siege-decay path zeroes out a sieged ward's contribution
  over the expected number of ticks.
- Soak verdict on canonical seed-42 deep-soak after landing — must
  return exit 0.
- Per-DSE drift check via `just frame-diff` — any cutover from boolean
  ward predicate to spatial consideration is *expected* to shift
  scores; needs a hypothesis (predicting: warded cats retreat more
  reliably when the gradient lights up nearby coverage; magnitude
  depends on the curve choice ticket 052 ships).
- Focal trace: confirm `ward_coverage` samples appear in L1 records
  for cats whose DSEs have been cut over.

## Log

- 2026-04-27: opened from ticket 006 closeout. Inherits the
  deferral ticket 045 logged when `WardCoverageMap` landed.
