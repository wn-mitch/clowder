---
id: 468
title: CraftAtWorkshop recipe-not-satisfied — new plan-failure surface post-465
status: ready
cluster: items-crafting
initiative: []
added: 2026-05-25
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

The 465 verification soak (`logs/tuned-42-59e26d68`) surfaced a new
plan-failure mode: **`CraftAtWorkshop: no workshop recipe fully
satisfied by inventory`** at 964 events / 0.01364 per tick. The
baseline `logs/tuned-42-1799e798` had this rate at 0.0 (it didn't
fire). The verdict tool flagged it as `new-high-rate`.

Hypothesis: the rate is secondary to 465's hunt-pathing fix — better-
fed colony → more cats with time for crafting → more `CraftAtWorkshop`
plan attempts → more attempts that hit the recipe-not-satisfied path.
But the failure shape itself (a plan that consistently fails when its
preconditions weren't checked) suggests one of:

1. **DSE selects Craft without verifying inventory satisfies the
   recipe.** The Craft DSE's eligibility filter doesn't check
   `inventory.has_for_recipe(...)`; the planner builds a Craft plan
   then bails at execute-time.
2. **The recipe-satisfaction predicate is on the *wrong* inventory
   slot** (cat-personal vs Stores vs station-local) — cat plans
   against one inventory shape, execution checks another.
3. **Recipe priorities don't degrade** when partial inventory is
   available — every cat picks the *same* unsatisfiable recipe in
   parallel until it fails.

Each of (1) / (2) / (3) is a structural defect, not a parameter tune.

## Scope

- `/logq events logs/tuned-42-59e26d68 --kind=PlanFailure` filtered
  to `reason="CraftAtWorkshop: no workshop recipe fully satisfied by
  inventory"` — partition by cat, recipe, station. Identify whether
  the failure is one-cat-many-attempts or many-cats-converging.
- Layer-walk the Craft DSE: eligibility filter, scoring, plan
  template, recipe-satisfaction predicate, execution check. Promote
  `[suspect]` rows to `[verified-*]`.
- Pick a structural-option from {split, extend, rebind, retire} per
  CLAUDE.md "Bugfix discipline".

## Out of scope

- Hunt mechanics — 465 closed.
- Adding new recipes or stations.

## Current state

Blocked on 465 landing.

## Approach

Bugfix-shape ticket. Use `_template_bugfix.md`'s layer-walk + structural
option menu. Investigation pass via `/logq` first; then promote audit
rows before drafting candidates.

## Verification

- Post-fix soak: `CraftAtWorkshop: no workshop recipe fully satisfied
  by inventory` event count back to or below the baseline rate
  (≤100 / 67k ticks).
- `just verdict` survival/never-fired gates pass.
- No regression on hunt success (465's recovered numbers hold).

## Log

- 2026-05-25: opened from 465's verdict output flagging this as
  `new-high-rate`. Hypothesis: secondary effect of larger / better-fed
  population; needs investigation before next balance pass.
