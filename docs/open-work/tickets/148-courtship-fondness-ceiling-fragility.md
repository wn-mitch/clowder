---
id: 148
title: Courtship-chain fondness ceiling vs gate fragility
status: in-progress
cluster: ai-substrate
added: 2026-05-02
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [146-distress-substrate-inert.md]
landed-at: null
landed-on: null
---

## Why

Ticket 146's investigation surfaced a knife-edge fragility in the
courtship-initiation chain: the most-bonded Adult-Adult-eligible pair
in seed-42 (Mocha+Birch) tops out at fondness 0.297 under the
removal-bare regime vs 0.303 under baseline (088 active). The
courtship-drift gate (`social.courtship_fondness_gate`) sits at 0.30 —
six thousandths from either ceiling.

The ENTIRE colony-wide `continuity_tallies.courtship` value is driven
by this one dyad's CourtshipDrifted events (verified: all 999 baseline
events are Mocha+Birch). Whether the chain produces ANY courtship
texture in seed-42 hinges on whether one specific cat's max-fondness
crosses one specific gate.

This is fragile by design. Investigate.

## Hypotheses to test

(a) **Fondness ceiling problem.** The Socialize / GroomOther rate
constants (`fondness_grooming_floor`/scale, `passive_familiarity_rate`,
relationship-update deltas in social.rs) keep the natural fondness
ceiling around 0.30 for the most-bonded Adult pair in any short-soak
window. Raise them.

(b) **Colony-coherence problem.** Adult founders (Mocha, Mallow,
Nettle in seed-42 — only 3 of 8 cats start as Adult per the
60% Young / 30% Adult / 10% Elder founder distribution) don't
co-locate enough to socialize with each other. They bond with whoever
is nearest, which is usually a Young cat. Mocha+Birch is the only
cross-stage Friends bond that forms. Adult-Adult pairs (Mocha+Mallow,
Mocha+Nettle, Mallow+Nettle) never reach fondness > 0.3 in any of the
146 soaks.

(c) **Founder-age distribution problem.** Seed-42 happens to roll 5/8
Young founders. With 3 Adult cats, the romantic-eligible pool is
small. Other seeds may not have this fragility.

## Scope

1. Multi-seed scan (10+ seeds): does the Mocha+Birch-shape phenomenon
   reproduce, or is seed-42 atypical?
2. If reproducible: per-seed measure of the most-bonded Adult-pair
   fondness ceiling. Is 0.30 always the ceiling, or seed-dependent?
3. Pick one hypothesis from (a)/(b)/(c) based on the data and run
   single-knob `just hypothesize` to test the fix.

## Out of scope

- Mating consummation (`MatingOccurred` chain). The chain ends at
  Partners bond formation; this ticket addresses ONLY the courtship
  drift → bond escalation gating.
- 088 retirement. Closed under 111 + 146.

## Log

- 2026-05-02: Opened from 146 close-out. Mocha+Birch evidence preserved
  in `logs/tuned-42-baseline-0783194/` (999 events) vs
  `logs/tuned-42-111-removal-bare/` (0 events).
- 2026-05-02: **History note** — commit `705ac36f` ("docs: 149 — open
  hunt-success disambiguation ticket") accidentally bundled this
  ticket's in-flight WIP (this file's edits, `scripts/diag_courtship_ceiling.py`,
  `src/plugins/setup.rs`, `src/resources/sim_constants.rs`,
  `src/world_gen/colony.rs`, `src/world_gen/custom_cats.rs`) alongside
  the 149 ticket creation. The misleading commit-message subject is
  acknowledged here (forward fix); no data lost. Future session: treat
  148's content above as landed in `705ac36f` and continue from there.
- 2026-05-03: **Post-balancing evidence — courtship signal collapsed
  to zero.** Surfaced during ticket 152's tier-1 disposition-collapse
  audit on `logs/032-soak-treatment/` (seed 42, header
  `commit_hash_short=883e9f3+dirty` — post-150 Eating split + 032
  threshold-gated cliff WIP). Footer fields:
  - `continuity_tallies.courtship = 0` — collapsed from **999** in the
    pre-150 baseline (`tuned-42-baseline-0783194`). The Mocha+Birch
    dyad's CourtshipDrifted firings appear to have stopped entirely,
    not just narrowed.
  - `never_fired_expected_positives` now includes `CourtshipInteraction`
    and `PairingIntentionEmitted` (both new entries — neither was on
    the pre-150 list).
  This is a *qualitative* shift, not a six-thousandths fragility
  question: the courtship chain is dark across the entire 8-season
  soak. Hypothesis (a) "fondness ceiling" is now the leading
  candidate — either the ceiling dropped further below 0.30
  post-balancing, OR a structural change in 150 (Eating split) /
  032 (threshold-gated cliff) starved Adult-Adult socialization
  airtime entirely. Action: rerun the `diag_courtship_ceiling.py`
  diagnostic (or its successor) against `032-soak-treatment` to
  measure the post-balancing fondness ceiling for the Mocha+Birch
  dyad and the Adult-Adult pool, then compare against the pre-150
  ceiling values to see whether (a)'s threshold dropped.
