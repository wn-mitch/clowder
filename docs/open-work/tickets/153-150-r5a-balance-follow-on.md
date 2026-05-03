---
id: 153
title: 150 R5a balance follow-on — Resting score-mass and courtship recovery
status: ready
cluster: balance
added: 2026-05-03
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 150 R5a split `DispositionKind::Eating` out of `Resting` to
remove the plan-duration cost asymmetry that caused hungry-but-not-
tired cats to commit to Sleep + SelfGroom when picking Eat. The
load-bearing structural fix landed and passes the survival gate
(`deaths_starvation=0`).

But the split shifted the L3 disposition mix in an unintended way.
Pre-150, Resting's score at the action-aggregation layer was
`MAX(Eat, Sleep, Groom)` — any of the three needs being unmet
dragged cats into Resting. Post-150, Resting only covers Sleep +
Groom; hunger no longer pulls cats into the Resting commitment.
Hunger now pulls them into the new `Eating` disposition, which is a
single-trip plan that completes in tens of ticks rather than the
hundreds-of-ticks Resting plans Pre-150 wove around an EatAtStores +
Sleep + SelfGroom chain.

The net behavioral shift in seed-42 (15-min release deep-soak):

| Disposition | Pre-150 (rough estimate) | Post-150 |
|---|---|---|
| Resting | ~1,900 plans | **441** plans |
| Eating | n/a | 193 plans |
| Exploring | (lower) | **73,304** plans |
| Hunting | (lower) | 63,577 plans |
| Foraging | (lower) | 42,693 plans |
| Socializing | (higher) | 22,337 plans |

Cats now spend dramatically more tick-budget in production
dispositions (Hunting / Foraging / Exploring) and less in
sustained-presence dispositions (Resting / Socializing-grouped). The
downstream effects:

- `continuity_tallies.courtship`: **999 → 0**. Courtship drift
  requires fondness + familiarity above gates, both of which
  accumulate during sustained social proximity. Cats roaming alone
  on patrol/hunt loops don't accumulate the fondness needed.
- `colony_score.aggregate`: 997 → 830 (**−17%**) — outside the ±10%
  drift band per CLAUDE.md balance discipline.
- `colony_score.health`: 0.24 → 0.10 (**−61%**).
- `colony_score.nourishment`: 0.82 → 0.63 (**−24%**).
- `colony_score.welfare`: 0.34 → 0.26 (**−23%**).
- `bonds_formed`: 3 → 2 (**−33%**).

The survival canaries pass (Starvation=0, ShadowFoxAmbush=6); this
is a colony-quality regression, not a survival regression.

## Hypothesis

R5a removed the implicit "Eat is part of Resting" coupling that made
hungry cats also commit to Sleep + SelfGroom. That coupling, while
structurally over-broad, served as a balance ballast: it forced
periodic Resting commitments that gave the colony cozy social
contexts (cats grooming each other / sleeping near each other) where
fondness accumulated.

The fix is *not* to re-couple Eat with Resting (that re-introduces
the structural defect 150 closes). It's to lift Resting's L3 score
through different signals so cats keep commiting to Resting at
healthy rates *for the right reasons* — accumulated tiredness, social
desire, ambient anxiety — rather than as a side effect of hunger.

## Investigation steps

1. **L2 score-distribution audit on the new run.** Use `just q
   trace` (or focal-cat subagent) on a fresh `just soak-trace 42`
   run. Per cat, sample L2 DSE scores tick-by-tick for: `eat`,
   `hunt`, `forage`, `sleep` (if it has a DSE), the resting/idle
   path. Identify whether Resting's constituent DSE scores are
   simply low post-150 (because Sleep/Groom never reach high score
   without acute energy/temp deficits).

2. **Plan-duration check.** Confirm the actual mean plan duration of
   Resting plans. The hypothesis is they're now long-and-rare
   instead of short-and-frequent. The R5b alternative would extend
   the Resting plan template conditionally; this audit informs
   whether R5b would help.

3. **Constituent-action audit.** Currently `Resting.constituent_actions
   = [Sleep, Groom]`. With Groom routing to Resting only when
   self-groom-won, ambient grooming for warmth always lands in
   Resting. If Sleep is gating on energy_deficit and Groom on
   temperature_deficit, Resting's score has narrow trigger windows.
   Consider lifting via:
   - A third "ambient-rest" axis (low-energy-and-low-stress band).
   - Restoring Eat as a Resting constituent for *scoring only* (the
     L3 selection is an aggregate-MAX; once Resting wins the cat
     commits to Sleep+Groom plan, not the legacy three-need plan).

4. **Compare against the R5b alternative.** Per the 150 plan,
   R5b kept the Eat→Resting mapping but conditionally branched the
   plan template on entry-needs. If Sleep+Groom alone doesn't supply
   enough Resting score-mass, R5b's "extend the umbrella" approach
   may have been the better structural choice. Open `tickets/154`
   if R5b regression-ports cleanly.

## Out of scope

- Reverting R5a — the structural fix is sound; the load-bearing
  starvation gate depends on it.
- The pre-existing `mentoring=0` and `burial=0` canary failures
  (those failed in baseline before 150 too).

## Log

- 2026-05-03: Opened as 150-landing balance follow-on. The
  structural R5a split is correct; tuning the L3 score-mass
  redistribution is the open work.
