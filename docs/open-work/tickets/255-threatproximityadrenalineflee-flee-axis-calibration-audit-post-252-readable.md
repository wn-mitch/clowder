---
id: 255
title: ThreatProximityAdrenalineFlee Flee-axis calibration audit (post-252 readable)
status: ready
cluster: ai-substrate
added: 2026-05-10
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

`ThreatProximityAdrenalineFlee` (ticket 108, `src/ai/modifier.rs:2334`)
lifts both `Action::Flee` and `Action::Sleep` scores when the
`threat_proximity_derivative` ramps. Pre-252, the L3 softmax filter at
`scoring.rs:2411` excluded `Action::Flee` from the disposition pool —
the modifier's Flee-axis lift was architecturally orphaned (only the
Sleep-axis lift mattered, per the 047 / 108 "in-pool partner"
doctrine). 252 lifted the filter; the Flee-axis lift is now reachable.

The lift was originally tuned on the assumption that Flee would NEVER
win L3 (it was filtered). With 252 making Flee a real contender, the
calibration may now over-elect Fleeing relative to its substrate
intent. Independently of whether 254 (PickFleeTarget witness fix)
makes the elected Fleeing actually move the cat, this ticket
audits whether the Flee-axis lift magnitude is right for the new
"Flee can win" regime.

## Scope

- Audit `flee_lift` (`sim_constants.rs:1635`, currently set per
  ticket 108) against post-254 soak data. Determine whether Flee's
  share of L3 election is at the doctrinal target for "rare,
  threat-driven" or has crept into "common, dominates Sleep".
- If miscalibrated, draft a hypothesis spec and run
  `just hypothesize` per CLAUDE.md balance discipline.
- Confirm or reframe the doctrinal claim "Sleep is the in-pool
  partner; Flee is rare" — that framing inherits from 047 (now
  retired by 251). Post-251, `health_deficit`-driven Sleep lift
  comes from the Logistic axis on Sleep DSE itself; the
  108-modifier's Sleep-axis lift may now be redundant.

## Out of scope

- The PickFleeTarget witness contract (ticket 254 owns that).
- Re-architecting the modifier pipeline composition (108's order
  in `default_modifier_pipeline` is fine).
- Sleep DSE's `health_deficit` Logistic axis re-tuning (that's
  251's territory).

## Current state

108 still in `default_modifier_pipeline` (`src/ai/modifier.rs:3501`).
Constants `acute_health_adrenaline_threshold` (preserved for 102 / 105),
`flee_lift = 0.6`, `sleep_lift = 0.5` (read from the most recent soak
header at `logs/tuned-42-post-252-fleeing-collapse/events.jsonl:1`).

## Approach

1. Read the 108 modifier scoring shape; trace the lift magnitudes
   through the L2→L3 pipeline post-252 (now that Flee is in the
   softmax pool).
2. Sample post-254 soak: how often does Flee win L3 across cats and
   ticks? Compare to pre-252 (where it was 0). Sweet spot: rare but
   non-zero.
3. If `flee_lift = 0.6` produces too-high adoption, sweep candidate
   values via `just hypothesize`.
4. If 108's Sleep-axis lift is now redundant with 251's Sleep-DSE
   Logistic axis, propose retiring `sleep_lift` from 108.

## Verification

- Post-254 soak: Flee adoption rate < some threshold (TBD — start at
  "matches the rate of credible threat-proximity events per soak").
- Hypothesis-concordance check if any constant moves > 10%.
- Survival canaries hold.

## Log

- 2026-05-10: opened from 252 land. The Flee-axis orphan question
  has been latent since 108 landed; 252 makes it actionable.
