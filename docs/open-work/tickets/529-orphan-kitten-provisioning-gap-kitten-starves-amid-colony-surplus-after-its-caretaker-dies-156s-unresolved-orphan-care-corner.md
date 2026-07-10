---
id: 529
title: Orphan-kitten provisioning gap — kitten starves amid colony surplus after its caretaker dies (156's unresolved orphan-care corner)
status: ready
cluster: social-coordination
orchestration: substrate-sensitive
initiative: []
added: 2026-07-09
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: [266-prey-ai-bolt-scatter.md]
landed-at: null
landed-on: null
---

## Why

A kitten can starve to death while the colony sits on a food surplus,
if its caretaker dies mid-kittenhood. Observed on
`logs/tuned-42-9306c110` (band-calibration iteration 1, 2026-07-09):
Finchkit-18 (born ~tick 1229223) starved at 1300180 — ~43k ticks after
Calcifer (its likely provisioning parent, partnered Mocha at 1207150)
died of injury at 1257395. The colony was NOT short of food: 735 prey
kills that run (more than the accepted baseline), nourishment 0.692,
zero other hunger signals. The kitten spent its final stretch churning
plans — 1,993 `PlanCreated` / 1,714 `PlanStepFailed` at a 35.6-tick
cadence, 1,629 of them `SelfGroom: starvation_override` — the
starvation override kept interrupting its plans, but nothing in the
colony converted that distress into food delivery.

This is exactly the corner ticket 156's log named and never resolved:
156 landed the KittenCryMap hearing-channel broadcast +
`KittenCryCaretakeLift` so NON-PARENT adults perceive kitten distress,
and its log says the residual failure is "a different defect shape
(orphan-care path, spawn locality, pathfinding, or coordination — TBD
by investigation)", pointing at follow-on 158 — which was subsequently
repurposed for the GroomedOther structural fix. The orphan-care path
has had no ticket since. Sibling landed context: 187
(RetrieveFoodForKitten plan-fail cluster), 164 ((38,22) cohort), 204
(bond-gated caretake), 399 follow-ons (adoption thresholds — the
BondFormed/Adopted parenthood stances exist; see
`project_parenthood_is_relational_stance`).

## Scope

- Layer-walk the orphan chain per the bugfix discipline: on caretaker
  death, what stops? Candidates (verify, don't assume): (a) the
  Caretake/FeedKitten target-DSE eligibility keys on a parent/bond
  relation that despawned with the caretaker; (b) the kitten's cry
  still broadcasts but every eligible adult scores below threshold
  without the kinship/bond multiplier; (c) the adoption path (399
  family) exists but its formation threshold is too slow relative to
  starvation at kitten metabolism.
- Structural fix at the substrate layer (eligibility/bond succession or
  adoption-rate), not a starvation-clamp hack.
- Scenario: caretaker-death-mid-kittenhood → another adult provisions
  within the starvation window → kitten survives. Deterministic, per
  the 156/164 scenario family.

## Out of scope

- Ticket 514's `MentorCat: target invalid: Incapacitated` churn (811
  occurrences in the same run — separate, already ticketed).
- Hunt-band calibration (plan step 25 owns it; this pathology is
  orthogonal to hunt tuning — it reproduced under a food SURPLUS).
- 501 cuddle-puddle / founder-dispersion work.

## Current state

Fresh from the failing run. Evidence preserved in
`logs/tuned-42-9306c110` (events + trace) and
`docs/balance/266-prey-ai-bolt-scatter.md` (calibration iteration-1
post-mortem). Chain-rare: needs caretaker-death × active-kitten
overlap, so soaks won't reliably reproduce — scenario-first per
`feedback_chain_rare_events`.

## Approach

Start with the layer-walk audit (L3 Caretake/FeedKitten disposition
mapping per `feedback_audit_l3_disposition_mapping`), position-scan the
kitten vs adults in the failing window first
(`feedback_verify_spatial_premise`) — if no adult was ever in range,
this is spawn-locality/pathfinding, not scoring. Then the structural
menu: eligibility rebind (bond-or-proximity fallback when no living
bonded caretaker), adoption-threshold acceleration on caretaker death,
or a colony-level orphan directive through the coordinator substrate.

## Verification

- The caretaker-death scenario above, asserting provisioning resumes.
- Soak: `Starvation == 0` hard gate green across the standard 900s
  gate; no regression in KittenFed / Caretake counts (156's +13%
  baseline).

## Log

- 2026-07-09: opened from the band-calibration iteration-1 FAIL
  post-mortem (plan step 25). Full diagnosis in
  `docs/balance/266-prey-ai-bolt-scatter.md`.
