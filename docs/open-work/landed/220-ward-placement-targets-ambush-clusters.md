---
id: 220
title: ward placement targets ambush clusters
status: done
cluster: ai-substrate
added: 2026-05-07
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 5348be2d9abe
landed-on: 2026-05-11
---

## Why
210's mechanism investigation showed ward placement is currently
geometric perimeter spray rather than threat-targeted. The post-210
soak placed 29 wards but 38 ShadowFox ambushes still landed — wards
worked (267 `ShadowFoxAvoidedWard`, 45 `ShadowFoxBanished`) but were
in the wrong tiles. Empirically, 60-70% of ambushes happened in
2-3 hot-zone tile clusters near the colony center; wards were
elsewhere.

The fix is to anchor `compute_ward_placement()`'s threat scoring on
the `RecentAmbushMap` substrate (ticket 219). Wards placed at
ambush-cluster centroids cover the empirical hot zones rather than
the geometric perimeter.

## Scope
- Add a sigmoid-shaped lift on `RecentAmbushMap` to the threat term
  in `compute_ward_placement()` at
  `src/systems/coordination.rs::compute_ward_placement`. Gated by
  `ScoringConstants::ward_ambush_anchor_weight` (default `0.0`).
- Restore the parallel `CarcassScentMap` consumer originally scoped
  in 209 §Scope line 74 but trimmed from the actual landing: same
  shape lift on the same threat term, gated by
  `ScoringConstants::ward_recency_anchor_weight` (default `0.0`).
  The underlying `CarcassScentMap` substrate is already in place
  from Phase 2C and writes correctly today; only the placement
  consumer was missing.
- Add `carcass_scent_at_position: f32` to `ScoringContext` and
  populate it in both `disposition.rs` and `goap.rs`. Emitted in
  `ctx_scalars` for trace observability; no DSE reads it at land.
- Tests: `compute_ward_placement_dormant_at_default_weights`,
  `ward_placement_shifts_to_ambush_hotspot_when_tuned`,
  `ward_placement_shifts_to_carcass_hotspot_when_tuned`,
  `ward_anchor_weights_ship_dormant`.

## Out of scope
- The substrate `RecentAmbushMap` itself — that's ticket 219.
- Tuning either weight from 0.0 → positive value — that's a
  follow-on balance ticket after 220 lands dormant.
- Reactive ward removal / migration (move wards as hot zones shift)
  — separate follow-on if the static placement still drifts.
- Ward types other than the standard `Thornward` — start with the
  current ward and generalize if soak data shows ward-type matters.
- Adding a DSE axis on `herbcraft_ward_dse`. That layer scores cat
  motivation ("should the cat want to ward now?"), not placement.
  Per the architectural verification in §Current architecture
  below, the placement-tile question lives in
  `compute_ward_placement()`, not in the DSE.

## Current architecture (verified)

`herbcraft_ward_dse` is `CompensatedProduct` with 3 axes
(`spirituality`, `herbcraft_skill`, distance to
`NearestPerimeterTile`). It emits `Intention::Goal { label:
"ward_placed" }` with **no target tile** — placement-tile
selection happens downstream in
`compute_ward_placement()` (a pure function over `PlacementMaps`,
~430 candidate tiles at every 5-tile bucket-aligned position) which
scored on `max(fox_scent, corruption) - coverage + 0.3 ×
cat_presence - distance_cost` before this ticket.

`RecentAmbushMap` (from 219) and `CarcassScentMap` (from Phase 2C)
were both already running, depositing, and decaying — only the
placement-side consumer was missing. The 209 §Scope line 74
"recency-weighted variant reading CarcassScentMap" anchor on
`herbcraft_ward_dse` was scoped but never written into the diff
(see `landed/209-*.md` log lines 150-167 for the rollback
sequence; the rollback removed only the `HasGroomingCandidate`
marker, but the carcass-scent perception scalar and ward consumer
were trimmed from the actual landing as well).

## Approach
1. **`compute_ward_placement` threat-term lift**
   (`src/systems/coordination.rs`): extend the per-candidate scoring
   loop with `ambush_lift + carcass_lift` added into the threat
   term before the `.min(1.0)` clamp. Each lift is
   `w × logistic_8_05(map.get(candidate.x, candidate.y))` with the
   curve named in the original §Scope (`Logistic(steepness=8.0,
   midpoint=0.5)`). When `w == 0.0` the lift is exactly zero — the
   sigmoid evaluation is short-circuited so dormant runs incur no
   extra arithmetic. **Dormancy invariant: byte-identical formula
   to pre-220 at default weights** (`fox_scent.max(corruption) +
   0 + 0 = fox_scent.max(corruption)`, then `.min(1.0)` is a no-op
   because both inputs are documented `[0, 1]`).
2. **Two new constants** in `ScoringConstants`
   (`src/resources/sim_constants.rs`):
   `ward_ambush_anchor_weight` and `ward_recency_anchor_weight`,
   both default `0.0`, each with the 3-part pattern (struct field
   with `#[serde(default = "...")]`, default-impl initialization,
   default function).
3. **PlacementMaps + WardPlacementSignals + ChainResources**
   extended with `recent_ambush` and `carcass_scent` references.
   Five `PlacementMaps { ... }` construction sites updated; the
   `compute_ward_placement` signature now takes `constants:
   &SimConstants` so it can read the weights.
4. **Perception scalar** (`src/ai/scoring.rs`): add
   `carcass_scent_at_position: f32` to `ScoringContext`, populate
   from `colony.carcass_scent_map.get(pos.x, pos.y)` in
   `disposition.rs:935` and `goap.rs:1923`, emit via `ctx_scalars`
   for trace observability.
5. **ColonyContext** (`src/systems/mod.rs`): add
   `carcass_scent_map: Res<'w, CarcassScentMap>` so scoring sites
   can sample it.

## Verification
- `just check` — substrate-stub lint, InfluenceMap registry lint,
  step-resolver lint (none affected, gate must pass).
- `just test` — 4 new unit tests, all passing:
  - `ward_anchor_weights_ship_dormant` (constants default to 0.0).
  - `ward_placement_dormant_at_default_weights` (byte-identical
    placement when ambush/carcass signals are deposited and
    weights are at default 0.0).
  - `ward_placement_shifts_to_ambush_hotspot_when_tuned`
    (positive weight biases placement toward the ambush peak).
  - `ward_placement_shifts_to_carcass_hotspot_when_tuned`
    (positive weight biases placement toward the carcass peak).
- Full `cargo test --lib`: 2075 passed, 0 failed.
- Soak inspection (post-tune, not part of the dormant landing):
  `just soak-trace 42 Wren` with
  `ward_ambush_anchor_weight=0.3` should show `WardPlaced`
  events clustering on tiles with non-zero
  `recent_ambush_at_position`.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-08 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed **  6** (done, —, score 0.88 (cross-cluster)) — Cluster-B shared spatial slow-state closeout
- · **221** (blocked, ai-substrate, score 0.88) — caretake gates on ambush-recency at kitten tile
- ✓ landed ** 71** (done, planning-substrate, score 0.87 (cross-cluster)) — Planning-substrate hardening — gird against the stuck-cat bug class (sub-epic)

<!-- linkages:end -->
## Log
- 2026-05-07: opened from 210 closeout, blocked on 219 (the
  `RecentAmbushMap` substrate it consumes).
- 2026-05-11: unblocked by 219's landing. Architectural review
  revised the fix-shape: ticket originally prescribed a DSE-axis
  change on `herbcraft_ward_dse`, but verification showed that
  layer scores cat motivation (emits `Intention::Goal` with no
  target tile) — placement-tile selection happens in
  `compute_ward_placement()`. Fix-shape moved one layer up to the
  placement function so the §Why (wards placed at empirical hot
  zones) is actually addressed. Bundled the CarcassScentMap
  consumer restoration originally scoped in 209 §Scope line 74:
  the substrate was already running from Phase 2C, only the
  perception-scalar + placement-consumer pieces were missing.
  Original "Current state" framing about a parallel
  CarcassScentMap axis on the DSE was stale — corrected here.
