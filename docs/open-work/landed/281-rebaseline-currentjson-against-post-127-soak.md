---
id: 281
title: Rebaseline current.json against post-127 soak
status: done
cluster: ai-substrate
added: 2026-05-11
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 83187bcf1768
landed-on: 2026-05-11
---

## Why
The active baseline `post-231-pre-burial` (promoted 2026-05-08) predates the
127 joint-intention substrate landing (commits `bcabded7` → `b5455647`,
landed 2026-05-11). Every post-127 `just verdict` now reports drift that
conflates two unrelated changes: (a) the substrate shape (PairingActivity
retired, JointIntention semantics in its place) and (b) the chronic kitten
starvation surfaced by ticket 273 / parked into 282 + 283. Without a
post-127 reference run, 282/283's substrate work cannot be measured —
verdict drift would always read as the sum of substrate-shift + perception
fix, with no way to attribute either signal.

## Scope
- Promote `logs/tuned-42` as `post-127-joint-intention`.
- Activate it via `current.json` (the default `just promote` behavior; not `--no-current`).
- Record the regressed-floor caveat in this ticket's Log + Approach so cold-session readers know what they're measuring against.

## Out of scope
- Running a new clean soak at the landed tip (`f6b6572c` / `b5455647`). The existing `logs/tuned-42` was generated at commit B of the 127 chain (`4bcae2de`) with `commit_dirty: true`. A fresh clean soak was considered and rejected: kitten starvation is now structural per ticket 273's perception-audit findings, so any post-127 soak would fail the survival canary regardless of the specific tip commit. Re-running would not produce a "passing" baseline; it would only produce a marginally cleaner version of the same failure shape.
- Fixing the survival regression. That's the explicit job of 282 (temporal-integration doctrine) + 283 (split fox-scent perception). This ticket only fixes the comparison floor against which those fixes will be measured.
- Demoting the old `post-231-pre-burial` baseline file. It stays on disk so historical comparisons remain possible via `--baseline` flags.

## Current state
**Promoted:** `logs/baselines/post-127-joint-intention.json`, `current.json` rewritten.

**Baseline run:** `logs/tuned-42` — seed 42, commit `4bcae2de` (dirty), 900s soak.

**Footer snapshot (the regressed floor 282/283 will improve on):**
- Survival: `Starvation: 1`, `ShadowFoxAmbush: 2`, `WildlifeCombat: 1`, `Injury: 3` — **fails hard survival gate** by design.
- Kittens: `kittens_born: 1`, `kittens_matured: 0` (vs post-231 baseline `4 / 0`).
- Population: `peak_population: 8` (vs post-231 `12`); `seasons_survived: 6` (vs `5`).
- Continuity: `grooming: 896 · play: 14 · mentoring: 215 · courtship: 2487 · mythic-texture: 35 · burial: 0` — all canaries fire ≥1.
- Welfare components: `nourishment: 0.69 · happiness: 0.70 · health: 0.81` healthy; `shelter: 0.00 · fulfillment: 0.00` collapsed.
- `colony_score.aggregate: 1977.5` (vs post-231 `2202.1`, -10.2%).

**Why these numbers are acceptable as a baseline despite hard-gate failure:** 282 and 283 are designed to lift kitten starvation; verdict reports against `post-127-joint-intention` will read as *direction of improvement on the known-failure footer*, not as "did we regress?". This is the same pattern as a regression-test snapshot for a known-bad state — the value is in measuring delta, not in the snapshot being intrinsically healthy.

## Approach
1. `just promote logs/tuned-42 post-127-joint-intention` (done — logged below).
2. No code changes; baseline pointer rewrite is the entire deliverable.

## Verification
- `logs/baselines/current.json` `label` field reads `post-127-joint-intention`.
- `logs/baselines/post-127-joint-intention.json` exists with full footer snapshot.
- `just verdict logs/tuned-42` now reports `constants_drift_vs_baseline: clean` + `seed_match_vs_baseline: match` (vs the prior `drift` against `post-231-pre-burial`) — confirming the baseline self-matches.
- Successor verdicts (after 282 / 283 land) will surface footer drift attributable to the substrate fix, not the substrate shift.

## Log
- 2026-05-11: Promoted `logs/tuned-42` as `post-127-joint-intention`; current.json activated. Accepted the regressed-floor caveat (1 starvation, dirty intermediate commit `4bcae2de`) because 282 + 283 are the substrate-side fixes that will lift the floor; rebaselining now keeps their verdict drift signals legible.
- 2026-05-11: Promoted via just promote logs/tuned-42 post-127-joint-intention. Regressed-floor accepted (1 starvation, dirty 4bcae2de) — 282/283 are the fixes that lift it.
