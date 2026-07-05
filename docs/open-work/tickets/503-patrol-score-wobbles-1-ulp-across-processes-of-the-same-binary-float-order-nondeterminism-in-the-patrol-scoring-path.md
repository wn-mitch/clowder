---
id: 503
title: Patrol score wobbles 1 ULP across processes of the same binary — float-order nondeterminism in the Patrol scoring path
status: ready
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-07-05
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
Two 900s seed-42 soaks of the byte-identical main binary
(`logs/tuned-42-25deac3d-run1` vs `logs/tuned-42-25deac3d`) differ on
exactly 5 event lines in ~70k — all of them cat Mallow's `Patrol`
score in consecutive `CatSnapshot`s (ticks 1214300–1214700):
`0.3091246` vs `0.30912462`, 1 ULP. Every other DSE score, every
position, every event is byte-identical, and the streams re-converge
after the window. The 500-landing comparison independently caught the
same signature on a different pair of binaries: cat "max", tick
1218500, `Patrol 0.27115503` vs `0.271155`. **Every cross-process ULP
divergence observed to date is the Patrol score** — single-source. A
1-ULP score wobble is one softmax tie away from a full behavioral
fork, which would silently invalidate the byte-identity gates the
0.4.0 plan leans on (and would be much harder to attribute than this
ticket's controlled evidence).

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Patrol DSE inputs | `src/ai/dses/patrol.rs` | CompensatedProduct over safety_deficit / boldness / safety-upper-gate + Spatial(TerritoryPerimeterAnchor) + patrol_threat_recency (263) | `[verified-correct]` (shape) |
| Anchor derivation | `src/systems/goap.rs:~2578` + `ward_coverage_map.rs::sector_centroid` | anchor = weighted centroid over `marks` (f32 sums, fixed row-major bucket order — order stable; VALUES come from `stamp_ward` accumulation) | `[verified-correct]` (iteration order) / `[suspect]` (mark values) |
| Ward stamping | `src/systems/magic.rs:228-236` | rebuilds coverage by iterating `Query<(&Ward, &Position)>` and `stamp_ward` `+=` per bucket — if Bevy query order differs across processes for an identical entity history, per-bucket f32 sums differ by ULP | `[suspect]` — prime candidate; needs promote via instrumentation |
| RouteCost read | `sim_constants.rs:2568` (`OwnRouteCost` at anchor) | flood_dijkstra — BinaryHeap, deterministic given same anchor; ULP in anchor could shift the sampled bucket only at bucket edges (would be a bigger-than-ULP score jump — not what we see) | `[verified-correct]` (as non-source of 1-ULP shape) |
| Wobble persistence | 5 consecutive snapshots then gone | consistent with a stable ward-set configuration whose stamp ORDER differed between processes; window closes when the ward set changes | `[suspect]` (interpretation) |

## Fix candidates

**Parameter-level:**
- R1 — snapshot-quantize `last_scores` in the event log (round to 1e-4).
  Rejected out of hand: hides the wobble, doesn't fix the fork risk.

**Structural (promote `[suspect]` rows first — instrument, don't guess):**
- R2 (**stamp-order pin**) — collect `(Ward, Position)` rows, sort by
  a stable key (entity index) before `stamp_ward` accumulation.
  Cheap; fixes the prime-candidate site if instrumentation confirms
  query-order variance.
- R3 (**order-free accumulation**) — make `stamp_ward` bucket sums
  order-independent (e.g., accumulate in f64, or collect
  contributions per bucket and sum in sorted order). Heavier; only if
  R2's sort is shown insufficient.
- R4 (**anchor snap**) — sector_centroid returns tile-quantized
  `Position` already? Verify — if the anchor is already integer-snapped,
  the ULP must enter later (would refute the stamping theory and
  point at the score pipeline's modifier layers — ticket 163's
  off-trace bonus stack).

## Recommended direction
Instrument first (per promote-audit-rows-first): log
`sector_centroid` inputs/outputs + anchor + Patrol consideration
outputs for the wobbling cat window in two processes; confirm which
float first differs. Then R2 (sort before stamp) if stamping is
confirmed, else follow the promoted trail. Do NOT ship a fix off the
`[suspect]` rows.

## Out of scope
- The RecipeRegistry HashMap tie-flip — separate mechanism, fixed in
  502.
- Byte-gate tooling tolerance: comparisons during 0.4.0 may attribute
  Patrol-score-only ULP diffs to this ticket explicitly (name the
  ticket id in the landing log when doing so) — but only for
  score-field diffs matching this exact signature; any structural
  divergence still fails the gate.

## Verification
- Two same-binary 900s seed-42 soaks with zero differing lines over
  the common range (modulo footer wall-clock + end-of-run tail).
- `just check && just test` green; verdict pass.

## Log
- 2026-07-05: opened from the 500/502 landing gates' three-way soak
  comparison. All observed cross-process ULP divergence is
  Patrol-score; evidence table above. Not caused by (and not fixed
  by) 500/502 — pre-existing on main at 25deac3d.
