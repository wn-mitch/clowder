---
id: 419
title: WardPlaced rate regression 5→2 on seed-42 post-084 — investigate Herbcraft eligibility cadence
status: done
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-05-19
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: pending
landed-on: 2026-05-19
---

## Why

Ticket 084 Commit 3 landed cleanly on the substrate side (gather→deposit fires 108× on seed-42), but `Feature::WardPlaced` regressed 5→2 (−60%) vs the pre-084 baseline (`logs/tuned-42-pre-084/`, commit `3e0153fe`-dirty). Net ward economy still decays late-soak (`ward_count_final = 0` in both runs), but throughput dropped — fewer wards placed per soak means a wider window of unwarded perimeter.

Likely correlated with ticket 418 (retrieve-path not electing): pre-Commit-2, a cat at the herb patch would gather thornbriar (carrying it post-plan) and then shortly elect `HerbcraftSetWard` while still carrying. Post-Commit-2, the gather plan terminates at Stores; the cat must pick `HerbcraftSetWard` AGAIN at a later tick, which apparently happens less often.

## Scope

- Attribute the WardPlaced drop: L3-selection-mediated (HerbcraftSetWard picked less) vs planning-mediated (picked but plan fails). Compare `GoapPlanCreated` counts per disposition between baseline and post-Commit-3 runs.
- If 418 lands first and WardPlaced recovers, close this as fixed-by-418.
- If WardPlaced stays low even after 418, consider lifting `HerbcraftSetWard`'s appeal under acute `WardStrengthLow`.

## Out of scope

- Reverting 084 Commit 2's plan-template change.
- Tuning ward decay rates to absorb the throughput drop.

## Approach

`just q events logs/tuned-42 --kind GoapPlanCreated` filtered for HerbcraftSetWard plans; `just frame-diff` on the two trace sidecars for per-DSE score deltas.

## Verification

`Feature::WardPlaced ≥ 5` on a post-fix soak, OR a documented decision that the post-Commit-2 economy produces strictly fewer-but-better-placed wards and the lower throughput is intended.

## Log

- 2026-05-19: opened as 084 follow-on. `wards_placed_total = 2` in `logs/tuned-42/` vs `5` in `logs/tuned-42-pre-084/`. Likely fixed-by-418.
- 2026-05-19: **fixed-by-418 confirmed decisively.** Re-soaked seed-42 against current main (commits ab1f3f38 + c2ad7967, post-418-fix): `wards_placed_total = 21` (vs pre-fix 2, vs pre-084 baseline 5). Herbcraft `Thornward` placements restored, and the substrate change overshot the pre-084 baseline by 4× — the stash-and-retrieve economy is structurally healthier for ward placement than the old gather-then-immediately-set chain. `HerbsRetrieved = 18` (was 0). `CropTended = 4183` / `CropHarvested = 44` (were 0) — the entire 084 herb-pressure chain works downstream. Closing as fixed-by-418.
- 2026-05-19: Verification soak (logs/tuned-42, commits ab1f3f38 + c2ad7967) confirms WardPlaced 2 → 21 post-418-fix, vs pre-084 baseline 5. The marker-snapshot population gap closed by 418 unblocks the entire 084 substrate chain. Closing as fixed-by-418.
