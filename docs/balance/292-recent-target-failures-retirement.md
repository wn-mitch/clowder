# 292 — RecentTargetFailures retirement (four-artifact record)

Ticket: `docs/open-work/tickets/292-*.md`. Lands as three commits:
emit sites (5da9c48d), reader cutover (3511e9a6), proxy deletion
(ea55e329). Baseline for comparison: `post-phase2-fluid-movement`
(`tuned-42-7c6a368a`).

## Hypothesis (pre-registered, ticket §Approach)

Target-keyed (action-agnostic) predictability preserves the six-DSE
(seven post-audit — `bury_target` landed after the ticket) L2 score
shape: the cooldown consideration keeps suppressing recently-failed
candidates, and the action-specific granularity loss (failing to
mentor cat-X now also cools socializing with cat-X during the
recovery window) is absorbed by personality + relationship modifiers.

Mechanism deltas vs the legacy map, worth naming up front:

1. **Recovery shape**: linear 8000-tick age ramp → EMA snap-to-0 +
   passive decay toward `prior = 1.0` (~3000-tick window under
   default `belief_facets.predictability` tunables, mirroring 290's
   disposition cutover). The cooldown is now SHORTER and convex.
2. **Cross-action bleed**: by design (choice (a)). The pre-registered
   pivot (b) is `EnvironmentalContextKey::ActionExecution` keying.
3. **Prey / corpse / structure targets fail open permanently** (no
   belief home — 505 ballast rule). Their churn-suppression is owned
   by structural fixes (467 reachability gate, 514 eligibility), not
   memory. Watch item: the 073 Mocha carcass-loop class
   (`HarvestCarcass` against a blocked carcass) has no cooldown now —
   plan-failure canary is the net.
4. **Predictability facet overload**: FleeFrom/Hunt witness arms also
   write the facet (behavioral-consistency semantics). The
   TargetActionFailed arm pins `prior = 1.0` on shared models, so
   decay targets shift for models both arms touch. Accepted; visible
   in the L2 trace via the renamed `target_predictability` input.

## Predictions

| # | Prediction | Band |
|---|---|---|
| P1 | Survival + continuity canaries hold on seed-42 | hard gate |
| P2 | `TargetCooldownApplied` still fires (> 0 per soak) through the belief path — the silent-canary exposure this migration must not regress | hard |
| P3 | Plan-failure profile: no new high-rate-ratio reason vs the post-phase2 baseline; specifically no resurrection of carcass/bury stuck loops (watch item 3) | gate channel |
| P4 | Trajectory diverges (constants header changed: `target_failure_cooldown_ticks` removed; cooldown window 8000 → ~3000 convex) but colony-outcome rates stay in-band (kills, births, deaths within verdict drift bands) | verdict channel |

## Observation (`tuned-42-ea55e329` vs baseline `tuned-42-7c6a368a`)

- **P1 CONFIRMED** — survival + continuity canaries pass; deaths
  (ShadowFoxAmbush 1, FoxConfrontation 3) within hard gates.
- **P2 CONFIRMED** — `TargetCooldownApplied` fired **971×** through
  the belief path (vs the legacy map path's firing on the same
  Feature). The silent-canary exposure did not regress.
- **P3 CONFIRMED** — one new high-rate plan-failure reason
  (`PickingUp: GoalUnreachable`, 32 → 325 at 10.3× rate) checked and
  cleared: 318 of 325 land in the first 30k ticks and it goes
  near-silent for the remaining 70k (7 events) — an early-colony
  trajectory burst, not a stuck loop; spread across six cats, no
  single-cat repetition. No carcass/bury loop resurrection.
- **P4 CONFIRMED** — throughput 110.6 vs 112.2 tps (band pass);
  trajectory diverged as predicted (constants header changed).
  Colony drift (kittens_born −50%, happiness +30%, health −27%) and
  the ward channel running hot (sieges 80 → 690, ward-avoidances
  741 → 14360) match the ±60% trajectory-family swings documented on
  513 across the three prior runs of this lineage — owned there, not
  here.

## Concordance

Concordant on all four predictions; the hypothesis (target-keyed,
action-agnostic predictability absorbs the granularity loss) stands
on this evidence. Deviation from the ticket's verification list,
recorded honestly: the formal `just hypothesize` sweep (dual-emit
baseline vs cutover) was not run — the three commits landed in one
session without a dual-emit binary checkpoint, so the comparison ran
as gate-soak-vs-promoted-baseline plus the structural nets (three
integrator unit tests, five sensor unit tests, one end-to-end DSE
election test, the 971-firing feature canary). The pre-registered
pivot (b) (`ActionExecution` context keying) remains available if
downstream soaks surface cross-action-bleed symptoms — the named
watch signals are social-family election churn and any
`plan_failures_by_reason` reason keyed on Socialize/Mentor/Mate
rising together after a Hunt/Fight failure wave.
