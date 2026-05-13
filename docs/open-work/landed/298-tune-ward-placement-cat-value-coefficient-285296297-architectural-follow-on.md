---
id: 298
title: tune ward placement cat_value coefficient (285+296+297 architectural follow-on)
status: done
cluster: balance
added: 2026-05-12
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [284-ward-anchor-tuning.md, 297-fox-patrol-topology-axis.md]
landed-at: 76d52578dc5b
landed-on: 2026-05-12
---

## Why

285 (anchor-weight magnitude), 296 (Logistic curve shape), and 297 (a new orthogonal threat-axis input) each tested a distinct threat-side lever against placement-argmax movement. Across six independent constant changes on three seeds (42, 99, 7), placement was byte-identical between baseline and treatment on every run. The architectural conclusion is documented in `docs/balance/297-fox-patrol-topology-axis.md` iter-2: the threat-axis composition `(fox_scent.max(corruption) + L(ambush) + L(carcass) + L(fox_intercept)).min(1.0)` is **rank-preserving for the argmax** once any threat-side input saturates on a sufficient number of tiles.

The argmax is then determined by the non-threat terms in the score formula at `src/systems/coordination.rs:1494`:

```rust
let score = unaddressed_threat + 0.3 * cat_value - distance_cost + jitter;
```

This ticket targets the `0.3 * cat_value` coefficient — the load-bearing argmax tiebreak among threat-saturated tiles. Quoting iter-2's conclusion: "the placement scorer's argmax is determined by the non-threat terms (`+ 0.3 * cat_value`, `- distance_cost`, jitter) once any single threat-side input saturates." This is the **first ticket testing a non-threat-axis lever** after three independent threat-axis levers were ruled out. The `0.3` was set in ticket 045 (`docs/balance/ward-perimeter-placement.md`) on first-light reasoning ("modest weight on cat_presence keeps placement biased toward where cats actually live") without explicit tuning, and has been hardcoded since.

## Scope

- Promote the hardcoded `0.3` at `coordination.rs:1494` to a `SimConstants.scoring` field (proposed: `ward_placement_cat_value_weight`, default `0.3` for byte-identical land).
- Write a `just hypothesize` spec sweeping `{0.1, 0.2, 0.3 baseline, 0.4}` across seeds 42 / 99 / 7 per the 285+296+297 triangulation discipline.
- Balance writeup in `docs/balance/298-ward-placement-cat-value-coefficient.md` with four-artifact concordance and a spatial-topology section (does placement actually shift between baseline and treatment? — the load-bearing observable, given that 285+296+297 all showed byte-identical placement).
- Pick the final default value based on whichever setting produces the largest movement in `shadow_foxes_avoided_ward_total` and `deaths_by_cause.ShadowFoxAmbush` without violating survival or continuity canaries.

## Out of scope

- `DIST_PENALTY_PER_TILE` at `coordination.rs:1428` — separate non-threat lever, separate ticket (299).
- The coarse `CANDIDATE_STEP = 5` candidate-generation grid at `coordination.rs:1421` — separate ticket (300); finer sampling is an orthogonal structural change.
- Threat-axis inputs (`w_ambush`, `w_carcass`, `w_fox_intercept`, Logistic `k/m`) — already ruled out by 285+296+297 across six constant changes × three seeds.
- Placement decision semantics (argmax-over-additive-sum → arrest-the-worst-violator descending-residual). Listed in 297 iter-2 as candidate lever #4; larger refactor in ticket 301.

## Current state

- Ticket 297 landed at commit `6756dbe465ec` (`docs/open-work/landed/297-...md`); `ward_fox_intercept_anchor_weight` ships first-light at `0.5`.
- `coordination.rs:1494` hardcodes `0.3 * cat_value` in the `for candidate in &candidates` scoring loop. The coefficient appears once; no other readers.
- `cat_value` is read from `maps.cat_presence.get(candidate.x, candidate.y)` at `coordination.rs:1468`, populated by the existing L1 `CatPresenceMap` substrate.
- `SimConstants.scoring` already holds the four 220/284/296/297 ward-placement knobs — the promotion path is mechanical and well-precedented at `src/resources/sim_constants.rs:1983-2070`.
- `DIST_PENALTY_PER_TILE = 0.005` remains hardcoded at `coordination.rs:1428` (intentionally — see the doc-comment at `:1422-1427`).

## Approach

1. **Promote `0.3` to `SimConstants.scoring`.** Add `ward_placement_cat_value_weight: f32` with `default_ward_placement_cat_value_weight() -> 0.3`, mirroring the doc-comment + `#[serde(default = …)]` pattern used by the 296/297 fields. Read the field at `coordination.rs:1494`.
2. **Substrate-no-op land.** Verify `just verdict` is clean against current baseline before the sweep (default unchanged, byte-identical placement expected — confirms the promotion is a pure refactor).
3. **Four-artifact sweep.** Author `docs/balance/hypothesis-298-cat-value-coefficient.yaml` (and seed-99 / seed-7 siblings) sweeping `ward_placement_cat_value_weight ∈ {0.1, 0.2, 0.3, 0.4}` against `shadow_foxes_avoided_ward_total`. Pre-register the concordance call: pass if **placement is no longer byte-identical between baseline and treatment** on at least one non-saturated seed.
4. **Spatial-topology section.** Per 297 iter-2's load-bearing pattern, post-hoc scan unique placement tiles and multiplicities between baseline and each treatment — the byte-identical-placement observation is what made the threat-axis inertness call rigorous, and the same observable is the right falsifier here.
5. **Structural-option candidate (CLAUDE.md bugfix discipline).** Even as a tuning ticket, name one structural alternative: **split `cat_value` into `cat_density` (where cats currently live) and `cat_movement_intensity` (where cats traverse, e.g., a decayed `cat_scent` L1 map analogous to `fox_scent`).** The current `CatPresenceMap` likely correlates with sleep/feed clusters, not corridors; a movement-intensity term could place wards on cat ↔ corruption travel paths rather than on residential tiles. Out of scope to *build*; in scope to *name* so a follow-on ticket has a structural anchor if the coefficient sweep also comes back byte-identical.

## Verification

- `just check` + `just test` green.
- `just hypothesize` concordance verdict on each of 42 / 99 / 7; ship the value that wins on `shadow_foxes_avoided_ward_total` magnitude without canary regression.
- **Spatial check:** placement is not byte-identical between baseline (`0.3`) and at least one treatment value on at least one seed. If every sweep arm produces byte-identical placement on every seed, the architectural conclusion sharpens once more (non-threat-axis terms are *also* rank-preserving here) and the structural-option candidate from step 5 becomes the next ticket.
- Continuity canaries hold (grooming, play, mentoring, courtship, mythic-texture each ≥ 1).
- Constants-drift-vs-baseline clean against the post-297 baseline.
- `just verdict` exit pass on each treatment soak.

## Log
- 2026-05-12: opened as the first of four follow-on tickets from 297's iter-2 architectural finding (`docs/balance/297-fox-patrol-topology-axis.md`). Three threat-axis levers (285 magnitude / 296 curve shape / 297 orthogonal axis) ruled out across six constant changes × three seeds; this ticket is lever #1 of the four non-threat-axis candidates named in 297 iter-2.
- 2026-05-12: 2026-05-12: iter-1 ran W=0.4 across seeds 42/99/7. First non-byte-identical lever in 285→298 sequence — seed-42 drops 2 wards at metric-irrelevant tile, seed-99 byte-identical, seed-7 adds 2 wards at fox-intercept tile (+5.1% metric). Magnitude too modest to ship 0.4; substrate-no-op promotion lands so knob is tunable. Opened ticket 303 for structural follow-on (CatResidenceMap split).
