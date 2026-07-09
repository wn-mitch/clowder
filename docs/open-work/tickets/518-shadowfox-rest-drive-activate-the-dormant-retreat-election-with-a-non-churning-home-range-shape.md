---
id: 518
title: ShadowFox rest drive — activate the dormant retreat election with a non-churning home-range shape
status: ready
cluster: wildlife
orchestration: substrate-sensitive
initiative: [predator-prey-dynamics]
added: 2026-07-09
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: [310-s4-dse-scoring.md]
landed-at: null
landed-on: null
---

## Why

310 S4 shipped the `shadowfox_retreat` DSE election-DORMANT
(`shadow_fox_retreat_election_scale` 0.0) after two gate iterations
could not find a non-churning shape: fed foxes shuttled their home
range at 184-299 retreat elections per 900s. The mechanism chain: prey
kills hold satiation above the stalk threshold for ~4.8k-tick stretches
(decay 0.001 per 16-tick cadence); any wandering state (nocturnal
patrol, haunting) carries the fox past the home-range radius; the
fed-and-far candidate then wins quiet elections; arrival releases; the
next wander re-arms it. Eligibility-distance (1.5 → 6.0) and den-rest
arrival (Waiting) both landed and reduced but did not close the loop.
S2's event-driven post-ambush retreat remains the retreat mechanism
meanwhile.

The missing substrate is a REST drive: a fed shadow-fox should *stay*
at its den (coherence recovering on corrupted ground) rather than be
pulled out by nocturnal patrol and boomerang home. Retreat-election
churn is a symptom of rest not existing as a competing candidate.

## Scope

- A rest candidate/drive in the motivation softmax: pressured when fed
  (satiation high) and coherence sub-full; its state holds the fox at
  the den (Waiting or a dedicated Resting state) and it should outscore
  nocturnal patrol while fed, so patrol stops yanking rested foxes out
  of the home range.
- Interplay rules: hunger returning (satiation decaying below the stalk
  threshold) hands over to the hunt; corruption drives (Coherence /
  Resonance / Dread / Entropy) may still interrupt rest — dread
  especially (a vulnerable cat near the den is an opportunity).
- Lift `shadow_fox_retreat_election_scale` to first light with a
  four-artifact gate: RetreatEntered per 900s must land at
  O(fed-periods) (single digits to low tens), not the shuttle regime.
- Scenario: fed-far fox retreats once, rests until hunger returns, then
  hunts — the full cycle in one deterministic run.

## Out of scope

- Predation-posture tuning (steps 24/25 of the 0.4.0 plan own it).
- The 023 corruption-drive scoring (unchanged).
- Pack coordination (ticket 310's deferred follow-on).

## Current state

Everything else of 310 S4 is live: hunt/patrol DSE candidates,
kill-site + ward filters at all selections, corruption-keyed ambush
affordance (first-light 0.10), night scalar, den-rest arrival, the
retired legacy roll. The retreat DSE is registered + dispatcher-wired
+ unit/activation-tested (`fed_far_fox_elects_retreat_when_scale_lifted`)
— only its election candidacy is scaled to zero.

## Approach

Likely shape: a rest candidate `shadowfox_rest` (or a rest pressure
folded into the retreat DSE as its terminal state) whose score rises
with satiation × (1 − coherence) and falls with den distance — so rest
dominates AT the den while fed, retreat dominates FAR while fed, hunt
dominates while hungry, and patrol only wins quiet hungry-ish nights.
Watch the WeightedSum conjunction trap (310 S4 record §iterations 2-4):
rest-at-den is satiation AND den-proximity — gate at eligibility or use
multiplicative composition.

## Verification

- Four-artifact soak vs the current accepted stream; RetreatEntered and
  Haunting cadence bands as above; hard gates.
- The full-cycle scenario; the existing dormancy test flips to an
  activation test.

## Log

- 2026-07-09: opened from 310 S4's close (release-plan step 23). Full
  churn evidence and the five-iteration history in
  `docs/balance/310-s4-dse-scoring.md`.
