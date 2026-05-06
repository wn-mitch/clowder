---
id: 182
title: Courtship + burial canary regression on post-176 soak (pre-existing or 176-induced?)
status: done
cluster: ai-substrate
added: 2026-05-05
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 4db67313
landed-on: 2026-05-06
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
    `MatingOccurred` listed in `never_fired_expected_positives`,
    `CourtshipInteraction` listed in `never_fired_expected_positives`,
    `PairingIntentionEmitted` listed in `never_fired_expected_positives`.
  - `colony_score.bonds_formed = 10` (BondFormed Feature
    activation count = 10 — bonds DO form).
- `logs/tuned-42-pre-176-stages` (commit `3c7c6c35`): the
  collapsed post-175 soak — terminated early, no footer.
  Same `MatingOccurred = 0` shape per the parent-ticket
  evidence table.

## Ground-truth diagnosis (preliminary, from initial drill)

The L2 PairingActivity layer (`src/ai/pairing.rs::author_pairing_intentions`)
inserts `PairingActivity` and fires `Feature::PairingIntentionEmitted`
ONLY when `pick_partner` finds a candidate whose
`bond_tier_score` is non-zero. `bond_tier_score` returns 0.0 for
`Acquaintance` / `Companion` and 0.5 for `Friends`, 1.0 for
`Partners` / `Mates`.

`BondFormed` activation = 10 means 10 bond-tier transitions
fired, but **the Feature does not distinguish tier**. If all
10 were `Acquaintance` / `Companion` (the low tiers), no
candidate would clear `bond_tier_score > 0.0` and
`PairingIntentionEmitted` would never fire — exactly what we
observe.

Hypothesis: bond *formation* happens but bond *advancement*
to Friends is broken or under-firing. The advancement path
runs through `relationships::modify_fondness` /
`modify_familiarity` / `modify_romantic` plus the
`promote_bond` logic in `src/resources/relationships.rs`.

Independent of bond-tier: the courtship→mating chain ALSO
needs `Action::Mate` to be elected at L3 with a viable
target. The Mate DSE eligibility filter requires
`HasEligibleMate`, authored by
`mating::update_mate_eligibility_markers` based on relationship
tier. If no relationship reaches the marker's threshold the
DSE never scores.

## Investigation hooks

1. **`/logq trace` on a high-fondness cat pair** — find a pair
   in the post-176 soak whose `fondness * familiarity` is in
   the top decile and check (a) their bond tier, (b) whether
   `HasEligibleMate` ever gets authored on either side, (c)
   whether MateDse ever scores > 0 for either.
2. **`/logq events --type=BondTierAdvanced`** (if such an
   event exists) — confirm whether ANY bond reaches Friends
   in the run.
3. **L2 trace of an Adult cat with the highest `bonds_formed`
   count** — see whether `pick_partner` is being called at all.
4. **Compare to pre-175 baseline** — was the
   bond-advancement-to-Friends rate the same, or did it
   regress? The 5506 MatingOccurred figure pre-175 implies
   bond advancement was fast enough.

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
- 2026-05-05: initial drill confirmed `BondFormed` Feature
  fires (10 events) but `PairingIntentionEmitted` never does.
  L2 PairingActivity gates on `bond_tier_score > 0.0`
  (Friends tier or above); the 10 bond formations may all be
  at Acquaintance / Companion tier. Bond-advancement-to-Friends
  pipeline is the load-bearing layer to verify next.
  Pre-existing — same shape as the post-175 collapse soak;
  ticket 176 is not the cause.
- 2026-05-06: **closed by ticket 184's fix** (`4db67313`). The
  bond-advancement-to-Friends layer was not the load-bearing
  defect — the underlying cause was `CanHunt` being stripped
  from injured cats, which over-suppressed Hunt L3 elections,
  cascaded action-share to Patrol's Blind-commitment long
  plans, and starved every higher-tier disposition of
  selection bandwidth (mating eligibility included). With the
  over-gating removed, the post-184 seed-42 soak shows
  `MatingOccurred = 2`, `CourtshipInteraction = 1403`,
  `continuity_tallies.courtship = 1405` (was 0),
  `never_fired_expected_positives = 0` (cleared
  `MatingOccurred`, `CourtshipInteraction`,
  `PairingIntentionEmitted`). The bond-tier-advancement
  hypothesis from the initial drill was disconfirmed by the
  fix's effect — bonds did advance once cats had bandwidth to
  socialize. Burial remains at 0 in the post-184 soak; that
  half of 182's premise is genuinely separate (no cats died
  of old age in the 15-min window) and rolls into 187 / a
  future life-stage-coverage ticket if it persists across
  longer soaks.
