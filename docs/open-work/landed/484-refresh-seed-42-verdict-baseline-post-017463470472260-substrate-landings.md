---
id: 484
title: Refresh seed-42 verdict baseline post-017/463/470/472/260 substrate landings
status: done
cluster: diagnostics-and-tooling
orchestration: substrate-sensitive
initiative: []
added: 2026-05-27
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 3e4f7caf
landed-on: 2026-07-05
---

## Why

`logs/baselines/current.json` (`post-055-mood-drift`, 2026-05-24, commit `1799e798`) is now stale enough that `just verdict <run>` against it fails by default on every fresh soak — every ticket landing trips the same false drift. The 429 landing soak (2026-05-27) verdict reported "fail" driven by:

- `shadow_foxes_avoided_ward_total: 0 → 511` — ticket 260 ShadowFox scent-avoidance landed since the baseline.
- `ward_siege_started_total: 0 → 75` — ticket 470 WardSiegeFearMap.
- `negative_events_total: +263%` — dominated by `EnvironmentalComfortNegative: 50402` (ticket 101's env-comfort feature pair) and `RouteCostFieldFallback: 23257`.
- `CraftAtWorkshop` plan-failures: 0 → 2041 — ticket 463 CraftItemAspiration substrate landed (plans without inputs are expected during the early arc).
- `fulfillment: +80%`, `structures_built: +27%`, `bonds_formed: +18%`, `welfare: +24.9%`, `nourishment: +22%` — cumulative effect of 017 anatomical slots, 334 WearItem, 477 equipment, 463 CraftItem, 472 Festering wound substrate work.

None of those drifts is 429-attributable (429's surface is Inventory mutation gates; doesn't overlap any of these). But the verdict's "fail" status hides actual regressions until the baseline is refreshed.

## Scope

- Run `just soak 42` against current HEAD (or a designated stable commit).
- `just verdict <run-dir>` against the existing baseline — confirm the drift signature matches the substrate-landing map above (sanity check).
- `just promote <run-dir> post-<sha>-substrate-stack-refresh` — promote the fresh run as the new `current.json` baseline. Pick a label that names the load-bearing substrate landings absorbed (017 / 334 / 463 / 470 / 472 / 260 / 477).
- Update `docs/balance/healthy-colony.md` if any of its target ranges drift past the new band thresholds.
- Open a follow-on if any of the drifts surface a real regression worth investigating (versus expected substrate-arc behavior).

## Out of scope

- Reverting any of the substrate landings — the drift IS the substrate's expected effect.
- Tuning new constants — this is a baseline-promotion ticket, not balance work.
- Multi-seed sweep — the canonical seed-42 single-soak baseline is the verdict gate; sweeps are separate.

## Current state

Opened 2026-05-27 at 429's landing. The stale baseline trips verdict failures on every recent soak. Land soon to avoid follow-on tickets being delayed by phantom drift.

## Approach

Single `just soak 42 && just verdict logs/tuned-42-<sha> && just promote logs/tuned-42-<sha> <label>` invocation. If the soak shows survival or continuity canary failures, file a real bugfix ticket; otherwise promote and move on.

## Verification

- `just verdict <new-run-dir>` returns `pass` (clean soak against itself as baseline trivially passes).
- After promotion, a re-run of the 429 landing soak through `just verdict` returns `pass` or `concern`-only (no `fail`).

## Log

- 2026-05-27: opened as a 429 follow-on. 429's verdict failed on pre-429 drift attributable to 017 / 463 / 470 / 472 / 260 / 477 / 334 substrate landings between the stale baseline (post-055-mood-drift) and HEAD. Baseline-refresh is the unblocker.
- 2026-07-05: scope satisfied by the Phase-I promote lineage: baseline post-phase1-perf-recovery-social-undilution promoted 2026-07-05 from logs/tuned-42-3e4f7caf (commit 3e4f7caf, +65.4% tps, ratchet clean). The 2026-06-11 post-459-490-instrumentation promotion was an interim refresh on the same lineage
