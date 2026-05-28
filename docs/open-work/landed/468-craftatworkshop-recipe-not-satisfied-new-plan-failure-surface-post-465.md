---
id: 468
title: CraftAtWorkshop recipe-not-satisfied — new plan-failure surface post-465
status: done
cluster: items-crafting
orchestration: substrate-sensitive
initiative: []
added: 2026-05-25
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 39f7457c23d4
landed-on: 2026-05-28
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
- 2026-05-28: Rebind craft DSE eligibility to recipe-aware markers; retire CraftAt{Workshop,TanningFrame}(Option<RecipeId>) at the type level; sister-fix retrieve_craft_inputs to Fail on stores-empty. Soak seed-42: 0 recipe-not-satisfied events (down from 964), survival+continuity canaries pass.
- 2026-05-28: Post-landing verdict against baseline `tuned-42-d531318e` on `logs/tuned-42-f0814e94/` (stacks 186 on top of 468; 186 is behaviorally inert for this question — only fixes basket-at-base-cap admission). Verdict: **concern** with hard gates clean (survival pass: 0 starvation, 2 ShadowFox ambush ≤10 gate; continuity pass: grooming/play/mentoring/courtship all ≥1). Significant footer drift: structures_built 14→10 (−29%), shadow_foxes_avoided_ward_total 511→3582 (+601% rate-normalized), ward_siege_started_total 75→353 (+370%), wards_placed +41%, fulfillment colony score +48%, bonds_formed 33→29 (−12%). Pattern is consistent with rebind+retire reducing crafting throughput, with freed bandwidth flowing to patrol/ward per `project_l3_patrol_absorption_cascade.md`. Not a collapse — a behavioral re-balancing. Flag for downstream attention but no rollback warranted.
