---
id: 507
title: Recalibrate social-perception and welfare constants against undiluted post-506 inputs (fulfillment -38pct, happiness -13pct, social_status_distress live for the first time)
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
Ticket 506 removed ~26k phantom prey/item pairs from the near-pair
pipeline. A side effect: perception scalars whose inputs average over
`iter_for(entity)` were silently DILUTED by hundreds of zero-fondness
prey entries — most visibly `social_status_distress`'s bond_asymmetry
arm, which was pinned ≈ 0 and is now live for the first time. Every
constant tuned between 431 (cache landing; the dilution predates it
via the old O(N²) sweep) and 506 was tuned against diluted inputs.
The post-506 four-artifact soak (`docs/balance/
near-pair-composition.md` iter 1, run `logs/tuned-42-3e4f7caf`) shows
the re-equilibrium: fulfillment -38%, happiness -13%, groom_self +31%
/ sleep +36% mean scores, one founder-dispersion window below the
10-tile floor (cuddle-puddle signature, 490 family), shadowfox ward
encounters ×9 (zero deaths — defense holds; the count jump is
clustering geometry, not a defense failure).

## Scope
- `just hypothesize` passes over `social_status_distress_*` weights,
  welfare-axis targets, and (if the dispersion floor keeps tripping)
  the 490/501 `WorkPressureAffiliativeYield` activation coupling.
- Decide per-axis whether the new equilibrium is the DESIRED baseline
  (the scalar finally measures reality — retune the downstream
  consumers, not the scalar) or the constants need rescaling.
- Update `docs/balance/healthy-colony.md` bands afterward.

## Out of scope
- Reverting 506 (the dilution was a defect, not a tuning surface).
- Prey AI / 266 interactions (Phase V re-baselines again anyway).

## Current state
Opened at 506's landing. The post-Phase-I baseline promotion
(`post-perf-recovery-social-undilution`, run 3e4f7caf) BAKES the new
equilibrium into `current.json` — subsequent landings won't re-trip
these rows; this ticket owns deciding whether the equilibrium is the
one we want.

## Approach
Constants-patch-shaped — `just hypothesize` specs fit directly.
Start with `social_status_distress` weight sensitivity (`just explain
interoception.social_status_distress_*`), then welfare targets.

## Verification
Four-artifact per hypothesize spec; canaries green; founder
dispersion ≥ floor in all windows OR explicitly re-banded.

## Log
- 2026-07-05: opened from 506's balance doc (near-pair-composition.md
  iter 1) per the antipattern-migration follow-on rule.
