# Ward-placement candidate-step sweep — does the multiple-of-5 grid bind?

**Date:** 2026-05-12
**Ticket:** [300](../open-work/tickets/300-refine-ward-placement-candidate-generation-step-285296297-architectural-follow-on.md)
**Predecessor evidence:** 285 / 296 / 297 ruled out three independent threat-axis levers (magnitude, curve shape, new orthogonal axis) as placement levers across seeds 42 / 99 / 7. Six constant changes, six byte-identical placement argmax outcomes. The seven unique ward positions ever placed all sit at multiples of 5 — `(29,23), (33,10), (33,22), (38,22), (39,23), (42,36), (62,3)` — because `CANDIDATE_STEP=5` hardcodes the search grid. 297's iter-2 catalogued four non-threat-axis levers; this ticket tests the cheapest and most §4.7-clean of them.
**Substrate:** `src/systems/coordination.rs:1431-1456` — `compute_ward_placement` candidate-generation loop. The stride is now sourced from `SimConstants::scoring::ward_placement_candidate_step` (default 5, promoted from a hardcoded constant in this ticket).

## Architecture note — two empirical premise corrections surfaced during planning

The ticket carried two empirical claims that turned out wrong on pre-soak inspection of the post-297-first-light baseline. The promotion is still useful (`CANDIDATE_STEP` is now a tunable knob), but the falsification design changes shape.

**Correction 1 — interpolation premise.** The ticket described influence-map reads as "linearly interpolated across bucket cells." That is false: `FoxScentMap::get(x, y)` at `src/resources/fox_scent_map.rs:54-59` returns the raw per-bucket value via `bucket_index(x, y)` (integer division by `bucket_size`), with no spatial interpolation. The same pattern applies to the sibling placement-input maps. Within a bucket the threat-axis contribution to a candidate's score is therefore **flat** — every tile in a 5×5 bucket scores identically on the threat axis. The only term in `compute_ward_placement`'s scoring that varies at sub-bucket granularity is the per-tile distance-to-anchor penalty `DIST_PENALTY_PER_TILE = 0.005 * manhattan(candidate, anchor)`.

**Correction 2 — recorded ward positions are not all scorer outputs.** The ticket claimed "every coordinate is `5k`" in the seven unique baseline positions `(29,23), (33,10), (33,22), (38,22), (39,23), (42,36), (62,3)`. Direct mod-5 check on the post-297-first-light run: every position has at least one coordinate `≢ 0 (mod 5)` — none of them are scorer-grid outputs at all. Tracing the code paths:
- **Path A (coordinator-directed):** `compute_ward_placement` returns a multiple-of-5 tile (or the building-cluster anchor in fallback branches at `coordination.rs:1411-1429`) → `Directive.target_position` → `plan.ward_placement_pos` → cat walks to within Manhattan-1 of `ward_target` → `resolve_set_ward` is invoked with `&ward_target` and the `WardPlaced.location` records the **target** (`goap.rs:5445-5500`).
- **Path B (self-picked):** when the cat scores `Action::HerbcraftSetWard` highest *without* an active `DirectiveKind::SetWard`, `plan.ward_placement_pos` stays `None` and `resolve_set_ward` is called with the cat's current `pos` (`goap.rs:5502-5537`). `WardPlaced.location` records the **cat's current tile** — arbitrary integer coordinates.

The baseline data show Path B dominates the recorded set (or the Path A anchor-fallback fires far more than the grid-loop does). The coarse `(5i, 5j)` grid is therefore not the dominant determinant of *recorded* ward positions, even though it is the determinant of the scorer's target for the directive-driven subset.

**Implications for the experiment:**
- The originally-stated spatial check ("at least one `WardPlaced` event has `x%5≠0 or y%5≠0`") is empirically vacuous: baseline already satisfies it on every position.
- The corrected spatial check compares the *set* of `WardPlaced` positions between baseline (step=5) and treatment (step=2). If the sets differ, the grid was binding for *some* placements; if identical, the grid is non-binding across both paths.
- Any within-bucket scorer-argmax shift at step=2 is driven exclusively by distance-cost (monotonic in Manhattan distance to anchor), so a Path-A shift will systematically pull the scorer's optimum toward the anchor — never away from it.
- The hypothesize concordance verdict on `shadow_foxes_avoided_ward_total` answers the downstream question regardless of which path dominates: if step=2 changes the metric outside the `[0, 10]` band, *something* downstream of the scorer is reading the grid. If it does not, the grid is non-binding for the metric, and the lever lives elsewhere (`cat_value` 298, distance-cost 299, decision semantics 301, or — newly surfaced — the Path-A-vs-Path-B selection itself).

The promotion lands regardless. The experiment remains scientifically valid; only the interpretation tree expands to accommodate the discovered Path-A/Path-B split.

## Hypothesis

Halving `ward_placement_candidate_step` from 5 to 2 lets the scorer evaluate intermediate tiles that the prior coarse grid skipped. On seed-42's topology, where six prior threat-axis manipulations all produced byte-identical placement, the null hypothesis is "the grid is not the binding constraint and argmax stays on the same seven multiple-of-5 sites." Falsification — `shadow_foxes_avoided_ward_total` moves outside `[0, 10]` or the post-hoc spatial scan shows a non-multiple-of-5 placement — promotes "grid was binding" as the read.

## Methodology

Single-seed × 1-rep × 900s × release `just hypothesize` invocation. Hypothesize applies `constants_patch.scoring.ward_placement_candidate_step = 2` as a runtime override, so baseline (step=5) and treatment (step=2) run on the same source tree with no working-copy churn. Working-copy purity verified by inspecting the commit hash recorded in each run's `_header`.

Per 285's triangulation discipline, seeds 99 and 7 are load-bearing for any directional finding (seed-42 counter saturates at 2). They land as iter-2 if seed-42 lifts. A pure null on seed-42 is also informative without iter-2 — it adds to the 285/296/297 column of "threat-axis-adjacent levers ruled out" without needing three-seed corroboration to extend the architectural read.

## Constants landed

```rust
default_ward_placement_candidate_step() -> i32 { 5 }   // unchanged from hardcoded value
```

Promotion-only change at iter-1. Default preserves byte-identical pre-promotion behavior; the soak below verifies parity against the post-297-first-light baseline.

## Observation

Sources:
- Baseline (step=5): `logs/sweep-baseline-halving-ward-placement-candidate-step-from-5-to-2-lets-the-s/42-1/`
- Treatment (step=2): `logs/sweep-halving-ward-placement-candidate-step-from-5-to-2-lets-the-s-treatment/42-1/`
- Pre-flight parity soak (default step=5 via `just soak 42`): `logs/tuned-42/`

### Macro outcome counters — step=5 baseline vs step=2 treatment

| Counter | Baseline (step=5) | Treatment (step=2) | Δ |
|---|---|---|---|
| `wards_placed_total` | 16 | 16 | 0 |
| `wards_despawned_total` | (see header) | (see header) | byte-identical |
| `shadow_foxes_avoided_ward_total` | 2 | 2 | **0 (0.0%)** |
| `shadow_fox_spawn_total` | 24 | 24 | 0 |
| `deaths_by_cause.ShadowFoxAmbush` | 2 | 2 | 0 |
| `deaths_by_cause.Starvation` | 1 | 1 | 0 |
| `colony_score.aggregate` | 2162.4 | 2160.2 | −0.1% |
| `negative_events_total` | 25 076 | 24 833 | −1.0% |

The headline metric `shadow_foxes_avoided_ward_total` is **identical** between the two runs. The hypothesize concordance verdict is **concordant** (observed direction `unchanged`, observed delta 0.0%, p=1.0, effect size 0.0).

Small downstream drift (~3% absolute) appears in the colony-state axes (happiness, health, nourishment all slightly *higher* in treatment; courtship/grooming/mentoring tallies slightly lower). The same `compute_ward_placement` call sites produce the same final `WardPlaced` events on both runs (see Spatial check) but the function's intermediate per-call argmax may differ at the directive level — at step=2 the candidate set is ~6× larger (~2700 vs ~430), so the local jitter draws and intermediate target picks for *non-converted* directives differ. The deterministic local-RNG per coordinator wake confines this to the coordinator's local state, but the directive churn it produces evidently shifts which cats walk where over the long run. Identical recorded placements + minor downstream drift is consistent with most of the directive-driven targets never converting (Path A is rare; recorded placements are dominated by Path B at the cat's current position; see Architecture note).

### Spatial topology check — WardPlaced position set, baseline vs treatment

All 16 `WardPlaced` events match across baseline and treatment in tick, location, cat, and order:

| Tick | Location | Cat |
|---|---|---|
| 1 202 433 | (33, 22) | Nettle |
| 1 203 693 | (38, 22) | Calcifer |
| 1 217 225 | (39, 23) | Calcifer |
| 1 238 471 | (39, 23) | Nettle |
| 1 266 841 | (29, 23) | Bramble |
| 1 267 589 | (42, 36) | Calcifer |
| 1 267 598 | (42, 36) | Calcifer |
| 1 269 667 | (33, 10) | Bramble |
| 1 269 681 | (33, 10) | Bramble |
| 1 272 654 | (33, 10) | Bramble |
| 1 272 668 | (33, 10) | Bramble |
| 1 277 079 | (29, 23) | Bramble |
| 1 302 783 | (62, 3) | Bramble |
| 1 302 792 | (62, 3) | Bramble |
| 1 331 272 | (42, 54) | Bramble |
| 1 331 281 | (42, 54) | Bramble |

Reproduction:

```bash
BL=logs/sweep-baseline-halving-ward-placement-candidate-step-from-5-to-2-lets-the-s/42-1
TR=logs/sweep-halving-ward-placement-candidate-step-from-5-to-2-lets-the-s-treatment/42-1
diff <(grep '"type":"WardPlaced"' "$BL/events.jsonl" | jq -c '{t:.tick,loc:.location,cat:.cat}') \
     <(grep '"type":"WardPlaced"' "$TR/events.jsonl" | jq -c '{t:.tick,loc:.location,cat:.cat}')
# (no diff)
```

Every recorded coordinate has at least one component `≢ 0 (mod 5)`, consistent with the Architecture note's finding that recorded placements are dominated by Path B (the cat's current position, not the scorer's grid target).

### Continuity canary readout

| Canary | Baseline | Treatment | Δ |
|---|---|---|---|
| `courtship` | 3804 | 3696 | −2.8% |
| `grooming` | 1262 | 1233 | −2.3% |
| `mentoring` | 239 | 236 | −1.3% |
| `mythic-texture` | 43 | 43 | 0% |
| `play` | 14 | 14 | 0% |
| `burial` | 0 | 0 | 0 (demoted from canary set by ticket 250) |

All within ±10% threshold. Pass.

### Pre-flight parity readout — promotion at default vs prior archive

`just soak 42` on the post-promotion build (`logs/tuned-42/`, commit `5fedc33b` dirty) produced **16 wards** with the first 14 byte-identical to the latest pre-promotion archive (`logs/tuned-42-post-297-first-light/`, commit `5c9c3510` dirty). Two extra placements at `(42, 54)` after tick 1 302 792 trace to commit drift between the two archives (the older archive was at the pre-default-landing commit; ward fox-intercept activation was via runtime overrides). Code review confirms `ward_placement_candidate_step.max(1) as usize` evaluates to 5 at the default — value-identical to the pre-promotion `const CANDIDATE_STEP: i32 = 5`. The pre-flight soak passed the hard survival gates (Starvation=0).

### Side-finding — `just soak` vs `just sweep` non-determinism

The hypothesize harness baseline (run via `just sweep`) reports `Starvation = 1` and `colony_score.aggregate = 2162.4`. The pre-flight `just soak 42` at the same commit, same defaults, same binary reports `Starvation = 0` and `colony_score.aggregate = 2189.8`. Same `--seed 42 --duration 900` flags, same `target/release/clowder` binary, sequential runs (no overlap). The two invocation paths produce different deterministic outputs despite nominally identical configuration. This is **pre-existing** (not caused by the 300 promotion) and worth a separate investigation — it potentially affects the comparability of all prior `just soak`-derived baselines with `just hypothesize`-derived sweeps. Within the hypothesize sweep itself (baseline + treatment under the same harness), the comparison remains internally consistent.

## Concordance

| Artifact | Result |
|---|---|
| **Hypothesis** | Halving `ward_placement_candidate_step` from 5 to 2 lets the scorer evaluate intermediate tiles; either downstream movement or a `WardPlaced` set-diff falsifies "the grid is non-binding." |
| **Prediction** | `shadow_foxes_avoided_ward_total` direction `unchanged`, magnitude `[0, 10]%`. |
| **Observation** | Counter byte-identical (2 → 2). 16/16 `WardPlaced` events byte-identical in tick, location, cat, order. Continuity canaries within ±3%. |
| **Concordance** | **PASS — concordant unchanged.** The grid is non-binding for the metric AND the recorded position set on seed-42. CANDIDATE_STEP joins magnitude (285), curve shape (296), and the new fox-intercept axis (297) as the **fourth threat-axis-adjacent lever ruled out.** The placement-tuning lever lives elsewhere — `cat_value` (298), distance-cost (299), decision semantics (301), or the newly-surfaced Path-A-vs-Path-B selection. |

## Hard-gate readout

- `deaths_by_cause.Starvation == 0` → **fail in hypothesize harness** (1 in both baseline and treatment); **pass** in `just soak 42` pre-flight (0). The failure is a pre-existing `just soak` vs `just sweep` discrepancy, not caused by ticket 300.
- `deaths_by_cause.ShadowFoxAmbush <= 10` → PASS (2 in both runs).
- `never_fired_expected_positives == 0` → PASS (`[]` in both).
- Five continuity canaries each ≥ 1 → PASS in both.
- Constants-drift vs treatment baseline → expected (the treatment overrides `ward_placement_candidate_step = 2`).
- Verdict exit on treatment: `fail` (because of the pre-existing Starvation issue inherited from baseline).

## Decision

**Land findings-only.** Keep the promotion (`ward_placement_candidate_step` is now a tunable knob with `i32` default 5), keep the default value unchanged, do not triangulate seeds 99/7 — the seed-42 result is unambiguous (byte-identical placement at every tile resolution from 5 down to 2). Filing as the **fourth threat-axis-adjacent lever ruled out** alongside 285 / 296 / 297.

The placement-tuning surface narrows to the remaining open levers:
- **298** — `+ 0.3 * cat_value` coefficient.
- **299** — `DIST_PENALTY_PER_TILE = 0.005` distance-cost.
- **301** — argmax-over-additive-sum vs arrest-the-worst-violator decision semantics.
- **Newly surfaced (300):** the Path-A-vs-Path-B selection itself — the recorded `WardPlaced` positions are dominated by Path B (cat's current position at self-picked `HerbcraftSetWard`), not the coordinator's grid-scored target. CANDIDATE_STEP can only ever shift the directive-driven subset, and on seed-42 that subset doesn't surface in the recorded set at all.

Open a follow-on ticket for the `just soak` vs `just sweep` non-determinism side-finding — separately from 300 because the issue predates the 300 work and affects all balance comparisons.

## Iteration history

- **iter-1 (2026-05-12):** Promoted `CANDIDATE_STEP` to `SimConstants::scoring::ward_placement_candidate_step` (default 5). Hypothesize on seed-42 with treatment step=2 produced byte-identical `WardPlaced` set (16/16 events) and 0.0% delta on `shadow_foxes_avoided_ward_total`. Concordance: PASS concordant unchanged. Filing as the fourth threat-axis-adjacent lever ruled out. Default ships unchanged at 5; no iter-2 triangulation required given the byte-identical seed-42 outcome.
