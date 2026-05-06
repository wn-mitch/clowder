---
id: 182
title: Courtship + burial canary regression on post-176 soak (pre-existing or 176-induced?)
status: ready
cluster: ai-substrate
added: 2026-05-05
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

The post-176-stages seed-42 soak (commit `75586184`) recovered
substantial survival vs the post-175 collapse (colony_score
1232 vs 1068, seasons_survived 4 vs 2, all four social
continuity canaries — grooming/mentoring/play/mythic-texture —
fired). But two continuity canaries still go dark:

| Canary | post-176 stages | pre-175 baseline | post-175 collapse |
|---|---|---|---|
| grooming | 286 | 237 | 38 |
| mentoring | 121 | 42 | 0 |
| play | 23 | ? | ? |
| mythic-texture | 11 | ≥1 | ≥1 |
| **courtship** | **0** | **5506** | **0** |
| **burial** | **0** | unknown | unknown |

`MatingOccurred` is on `never_fired_expected_positives`. Bonds
form (+233% vs baseline) but the courtship→mating pipeline
doesn't fire at all.

Meanwhile `deaths_starvation: 1` persists — one cat starves
late in the run despite the post-176 carcass-on-ground refactor
preserving food entities.

These regressions could be:

1. **Pre-existing** — already broken before 176 (the post-175
   collapse made it impossible to tell). Post-176's survival
   recovery exposes the underlying defect.
2. **176-induced** — something about the disposal-substrate
   landing changed scoring or planner behavior that
   incidentally suppresses courtship / burial. Less likely
   given the disposal DSEs ship default-zero, but possible
   via the new Action variants altering match exhaustivity in
   some scoring pathway.
3. **Saturation-floor over-suppression** — the new
   `colony_food_security` axis (default-zero weight) shouldn't
   affect anything at runtime, but worth verifying.

## Evidence

- `logs/tuned-42` (commit `75586184`): post-176-stages soak.
  - footer: `continuity_tallies.courtship = 0`,
    `continuity_tallies.burial = 0`,
    `deaths_by_cause.Starvation = 1`,
    `MatingOccurred` listed in `never_fired_expected_positives`.
  - bonds_formed = 10 (vs baseline 3) — bonds ARE forming.
- `logs/tuned-42-pre-176-stages` (commit `3c7c6c35`): the
  collapsed post-175 soak — terminated early, no footer.

## Investigation hooks

1. **L1 / L2 trace of a high-bondedness pair**: pick a cat with
   strong fondness/familiarity in the post-176 soak; trace
   their L2 to see where Courtship / Mate / CourtshipInteraction
   scores fall in the softmax. Use `just soak-trace 42 <name>`.
2. **Check L3 mapping**: `Action::Mate` → `DispositionKind::Mating`
   in `from_action`. Confirm Mating's plan template + goal
   predicate haven't drifted.
3. **Compare to pre-175 baseline run** to confirm whether
   courtship/burial fired BEFORE the 175 + 176 work landed.
4. **Run with default-zero disposal DSEs explicitly disabled**
   (skip registration) — if courtship recovers, the
   default-zero registration broke something via softmax-pool
   shape alone.

## Direction

Diagnostic-only ticket — open the layer-walk audit per
CLAUDE.md bugfix discipline. Fix shape depends on what the
investigation surfaces. Likely candidates:

- Courtship goal-predicate / plan-template drift since pre-175.
- Pairing→courtship→mating commitment chain broken.
- Burial DSE never authored against the new death distribution
  (ticket 157 territory).

## Out of scope

- Balance-tuning the saturation surfaces (181).
- Disposal DSE balance work (178).

## Verification

- Post-fix soak's `continuity_tallies.courtship ≥ 1`,
  `continuity_tallies.burial ≥ 1`.
- `MatingOccurred` no longer in
  `never_fired_expected_positives`.
- `deaths_starvation == 0` (the hard-gate).

## Log

- 2026-05-05: opened by ticket 176's closeout. Post-176 soak
  showed partial recovery vs post-175 collapse but two
  continuity canaries (courtship, burial) remain dark and one
  cat starves. Investigation needed to disambiguate
  pre-existing-but-masked from 176-induced.
