---
id: 530
title: Hunt-success biology-band calibration — per-species locomotion + strike-window economy (uniform escape knobs proven inert)
status: ready
cluster: balance
orchestration: substrate-sensitive
initiative: [predator-prey-dynamics]
added: 2026-07-09
parked: null
blocked-by: [529]
supersedes: []
related-systems: []
related-balance: [266-prey-ai-bolt-scatter.md, fluid-movement-phase2.md]
landed-at: null
landed-on: null
---

## Why

Post-266 cat hunt success sits at **75.9% aggregate** (mouse 75–94%,
rabbit 70–84%, rat 72–79% across recent seed-42 families) against the
30–50% biology band the 0.4.0 plan reserved for Phase V. The escape
substrate is live and honest — 907–917 Bolt/Scatter elections per 900s,
pursuit locks demonstrably break — but success is decided DOWNSTREAM of
the elections, in two places the elections cannot reach:

1. **Chase kinematics.** `sprint_speed_mult = 3.0` (140 step-12 gate:
   the whole detection/alertness/catch economy was re-anchored at
   pre-140 parity, `chase_speed = 3`) versus a ground-prey flee cap of
   `prey_ground_max_speed × flee_speed = 1.0` (mouse/rat/rabbit all
   have `flee_speed() = 1` today, although the sprint constant's own
   doc-comment still describes a rabbit at 2.0). A 3:1 speed ratio
   makes every chase geometrically certain; only `chase_limit_*`
   timeouts save prey.
2. **The pounce strike window.** `pounce_awareness_idle = 0.95` — an
   unaware prey animal concedes a near-certain kill; the multiplicative
   success formula (`awareness × skill × distance × catch_difficulty ×
   density`) has no last-instant-reaction term.

Band-calibration iteration 1 (2026-07-09, run `tuned-42-9306c110`)
proved the uniform escape-side knobs inert: election thresholds
0.45→0.38 + `detection_base_chance` 0.10→0.14 changed escape cadence by
~1% and hunt success not at all (the election score distribution is
bimodal; detection is not binding). Reverted at
`docs/balance/266-prey-ai-bolt-scatter.md` §iteration 1. The plan's own
fallback clause applies: "per-PreyKind thresholds only if uniform
fails" — uniform failed.

## Scope

- **Per-species locomotion honesty** (the plan-sanctioned per-kind
  fallback): rabbits genuinely outrun cats — `rabbit flee_speed 1 → 2+`
  (restoring the value the sprint doc's economy was anchored against);
  mice/rats stay slow (real cats DO catch rodents at high rates; their
  escape is the strike window, not the straightaway).
- **Strike-window reaction**: lower `pounce_awareness_idle` from 0.95
  and/or add a last-instant-reaction term fed by the prey's alertness/
  bolt-affordance so the strike window composes with the 266 substrate
  instead of ignoring it.
- Fish wariness proxy if the fish attempt-share creeps again (the 516
  structural-calm distortion: fish never alert → `prey_calm ≡ 1.0`).
- Iterate four-artifact per lever family; target: aggregate in or near
  30–50% with **`Starvation == 0`** and prey populations at capacity.
- Watch the 140 step-12 history: sprint 2.4 measured 9.5% success and
  1.4 measured 15–22% — but those denominators were pre-467 fish-churn
  contaminated; do NOT treat them as evidence that the band is
  unreachable between 2.4 and 3.0.

## Out of scope

- Cat-side sprint nerfs as the primary lever (locomotion honesty cuts
  the other way: real cats DO sprint ~3× a mouse).
- The 529 orphan-provisioning pathology (blocker, see below).
- Shadowfox/wildlife predation posture (310/518 own it).

## Current state

**Blocked by 529**: the orphan-kitten starvation pathology fails the
`Starvation == 0` hard gate on trajectories independent of these knobs
(iteration 1 failed the gate with kill volume UP — a kitten starved
amid surplus after its caretaker died). Until 529 lands, every
calibration soak is a gate lottery — the park-behind-the-blocker rule
(`feedback_park_demographic_dependent_tuning`).

0.4.0 ships at the honest 75.9% (recorded in the release notes as
above-band, calibration deferred); the escape-cadence instruments
(PreyBoltStarted / PreyScatterStarted events) and per-species
`just q hunt-success --species=` breakdowns are in place for this
ticket's gates.

## Approach

One lever family per iteration, archive-vs-archive verdicts. Suggested
order: rabbit locomotion (clean per-species attribution in the species
breakdown) → strike-window reaction (mouse channel) → fish wariness if
share creeps. Predictions must state per-species bands, not aggregate
only — the aggregate is a mix-weighted average that moved AGAINST the
per-species direction twice in the 266 gates (fish reversal, herd
flushing).

## Verification

- Four-artifact per iteration; hard gates; prey-population capacity
  check (`PreyBred` cadence, den abandonment) — competent-prey +
  lower predation must not overshoot into prey overrun.
- Final: aggregate + per-species table in the balance doc; promote
  only after two consecutive clean-gate families.

## Log

- 2026-07-09: opened from band-calibration iteration 1's refutation
  (plan step 25). Evidence: `docs/balance/266-prey-ai-bolt-scatter.md`.
  Blocked on 529 (starvation-gate lottery).
