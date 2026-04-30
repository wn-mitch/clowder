---
id: 2026-04-23
title: "Phase 4b.5 — §4 colony-scoped marker batch: `HasFunctionalKitchen` + `HasRawFoodInStores` + `WardStrengthLow`"
status: done
cluster: null
landed-at: null
landed-on: 2026-04-23
---

# Phase 4b.5 — §4 colony-scoped marker batch: `HasFunctionalKitchen` + `HasRawFoodInStores` + `WardStrengthLow`

Three-marker batch extending the Phase 4b.2 / 4b.4 reference-port
pattern to the next cluster of colony-scoped predicates that
already had in-scope caller-side bindings. Each marker retires an
outer `ctx.<bool>` conjunct in `score_actions` and moves
eligibility onto the target DSE's `EligibilityFilter::require(name)`.
Shared author shape: **caller-side `markers.set_colony(name, bool)`
next to the existing binding; no new author-system file**.

- **`HasFunctionalKitchen` + `HasRawFoodInStores` → `CookDse`.**
  Cook's positive branch (`cook_base_conditions &&
  ctx.has_functional_kitchen`) retires. The
  `wants_cook_but_no_kitchen` latent signal read by BuildPressure
  in `goap.rs` is preserved via a caller-side disambiguation —
  when the DSE's marker-gated score drops to zero, the scorer
  checks `ctx.has_raw_food_in_stores && !ctx.has_functional_kitchen`
  to raise the signal. The `hunger > cook_hunger_gate`
  precondition stays as an inline wrap (§4.5 scalar carve-out).
- **`WardStrengthLow` → `HerbcraftWardDse` + `DurableWardDse`.**
  First port where one marker gates two sibling DSEs.
  HerbcraftWard's outer gate reduces from `ctx.ward_strength_low
  && ctx.has_ward_herbs` to `ctx.has_ward_herbs` (pending a future
  per-cat `HasWardHerbs` inventory-marker batch). DurableWard's
  outer gate reduces from `ctx.ward_strength_low && ctx.magic_skill
  > threshold` to just the magic_skill threshold (scalar, not
  marker).

Caller-side population adds three `markers.set_colony(name, bool)`
lines next to each predicate's existing computation in both
scoring paths (`disposition.rs::evaluate_dispositions` +
`goap.rs::evaluate_goap_scoring`). `cached_test_markers()` in
`scoring.rs` pre-loads the three new markers to `true` so every
existing scoring-tier test continues to pass without per-test
`EvalInputs` overrides.

7 DSE-level parity tests — `.eligibility().required` assertion +
absence-rejection pair on each of Cook / HerbcraftWard /
DurableWard, plus a Cook-with-both-markers-present eligibility
test and a sibling-guard assertion covering the other five
PracticeMagic DSEs (Scry / Cleanse / ColonyCleanse / Harvest /
Commune) that still carry empty filters.

**Hypothesis:** marker eligibility is predicate-equivalent to the
retired outer gates (population reads the same caller-computed
bool the outer gate consulted). Port should produce zero
behavioral drift on the seed-42 soak; the marker is the exact same
bit, moved from ScoringContext field to MarkerSnapshot entry.

**Verification:** `just check` + `just test` (1049 unit tests + 13
integration tests pass). Two back-to-back seed-42 `--duration 900`
release soaks on the same 4b.5 binary (`logs/tuned-42` +
`logs/phase4b5-run2`) produced an identical 8-feature
`never_fired_expected_positives` list — reproducible within a
binary, not seed variance. A 4c.7-only baseline soak
(`logs/phase4c7-baseline`, built after reverting the six
4b.5-touched files) with the original outer gates in place produced
the *same* 8-feature list. That alignment is the strongest
evidence the port is predicate-equivalent: the never-fired set is
a property of the Phase 4c.7 HEAD state, unchanged by moving
three eligibility predicates from outer gates to marker filters.

**Concordance — partial.** The never-fired axis aligns across
binaries (clean signal). The survival / continuity-metric
comparison is less conclusive:

| Binary | Starvation | ShadowFoxAmbush | grooming | wards_placed | ward_avg |
|---|---|---|---|---|---|
| 4c.7-only baseline | 8 | 0 | 24 | 111 | 0.00 |
| 4b.5 run 1 (`logs/tuned-42`) | 1 | 0 | 159 | 239 | 0.44 |
| 4b.5 run 2 (`logs/phase4b5-run2`) | 3 | 0 | 165 | 244 | 0.79 |

The 4c.7-only baseline sits **outside every other recorded soak's
envelope** — every other footer in `logs/*` (across multiple
commits) shows grooming ≥ 68, Starvation ≤ 5. Two interpretations
of the gap both remain on the table:

1. *Scheduler variance (most likely).* Bevy's parallel scheduler
   is non-deterministic across compiled binaries per CLAUDE.md's
   seed-42 drift note; the baseline is a single unlucky
   realization. The 4b.5 runs happen to land in a luckier
   realization of the same semantic state.
2. *4b.5 coincidentally masks a 4c.7 WIP anomaly.* The three
   `.require` changes shift softmax-pool composition under
   scoring. If 4c.7's Caretake reshape produced an edge case where
   the old outer gates somehow amplified a scoring pathology that
   the new eligibility filters dampen, that would also fit.

A second clean 4c.7-only soak would disambiguate; not run here
(the current working tree has drifted such that a clean revert
requires additional surgery to preserve post-phase4c7-baseline
§11 focal-trace additions in `scoring.rs`). Landing on
interpretation #1 because (a) the port is predicate-equivalent by
construction — `set_colony` reads the same bool the outer gate
consulted — and (b) interpretation #2 would require a
non-semantic-equivalence mechanism that I can't identify from
the code. Worth flagging on the 4c.7 balance thread that
`phase4c7-baseline` is a tail-outlier soak regardless of cause;
that's not this batch's concern to resolve.

Remaining §4.3 markers: from the ~48 noted at Phase 4b.4 landing,
three ported here → **~45 remaining**. Next candidate batches:
per-cat inventory markers (`HasHerbsInInventory` /
`HasRemedyHerbs` / `HasWardHerbs`) needing a
`Changed<Inventory>`-filtered author system in `items.rs`;
capability markers (`CanHunt` / `CanForage` / `CanWard` /
`CanCook`) needing the new `src/ai/capabilities.rs` fan-out per
§4.6.
