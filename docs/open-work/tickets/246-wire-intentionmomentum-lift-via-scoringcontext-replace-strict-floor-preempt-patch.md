---
id: 246
title: Wire IntentionMomentum lift via ScoringContext (replace strict-floor preempt patch)
status: in-progress
cluster: C
added: 2026-05-08
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 126 (BDI intention substrate) landed C1+C2+C3+C4 in `5fb11706` with a
load-bearing band-aid: `PREEMPT_STRENGTH_FLOOR = 0.5` at
`src/systems/goap.rs:3062`. The floor masks zero-strength values in a stale
`last_scores` read because the `IntentionMomentum` modifier (registered in the
default pipeline) is structurally wired but **dormant** — its three input
scalars at `src/systems/goap.rs:2062-2064` are hard-coded to `0.0` with a
comment naming this as the C3 wiring slot. The substrate-over-hack design
pillar dictates the resolution shape: the substrate axis landed first
(`IntentionMomentum`, in 126), the hack retires second (the 0.5 floor, in
246).

## Scope

- Wire the three `IntentionMomentum` scalars at the L2 author site
  (`src/systems/goap.rs:2059-2087`) from `Option<&HeldIntention>` instead
  of zeroing. **Landed.**
- ~~Retire `PREEMPT_STRENGTH_FLOOR = 0.5`~~ — **withdrawn**. Verification
  soak with the floor removed collapsed (15× sim-throughput slowdown,
  cats locked in PickUp/Drop loops, 0 Stores built, 1172 Resting
  GoalUnreachable failures). The 246 plan's "cliff does NOT recur"
  prediction was wrong. The floor stays as a documented load-bearing
  hack; a follow-on bugfix ticket owns the diagnosis. See `## Log`.
- Existing modifier-side unit tests in `src/ai/modifier.rs::tests`
  already cover the gated-boost contract from ticket 126's C2 commit
  (`intention_momentum_dormant_when_no_held_intention`,
  `…_when_lift_factor_zero`, `…_lifts_held_dse_only`,
  `…_does_not_resurrect_zero_score`, `…_stacks_with_commitment_tenure`).
  No new modifier-side test added in 246 — coverage is complete at the
  modifier surface.
- Scenario-side coverage of the wired path was attempted and withdrawn
  (see comment block in `src/scenarios/modifier_preempts_hunt.rs::tests`):
  the scenario harness's focal trace cannot capture the orphan-Held
  re-election window because `pre_resolve_focal` runs after the first
  `app.update()`, and `update_capability_markers` strips `CanHunt` after
  the cat moves out of forest-nearby radius. Direct `eprintln!` debug
  probes during implementation confirmed the wiring is correct (factor
  ramps from `0.0992` → `0.0950` over 30 ticks under canonical
  constants); the modifier is called with `lift > 0` and `ord = 3 (Hunt)`
  every tick the cat is orphan-Held. Coverage shifts to the verification
  soak's `just frame-diff` (per §Verification step 3 below) which sees
  the modifier delta at colony scale.

## Out of scope

- Live re-scoring at preempt time. 126's plan
  (`/Users/will.mitchell/.claude/plans/work-126-drifting-widget.md:53-55`)
  rules this out: stale `last_scores` is "one-tick-stale; avoids
  schedule-edge perturbation" — keep the snapshot cadence.
- Plan-failure replan re-scoring while `HeldIntention` persists. All §7.2
  drop branches batch-remove `HeldIntention` with `GoapPlan` via
  `goap.rs:3734-3748`. No coverage gap to fill.
- Trust-weighted `IntentionSource` lift — ticket 130. The `_source` read
  at `src/ai/modifier.rs:926` stays as a stub.
- Coordinator-directive integration — tickets 057 + 081 own the writers.
- Trigger-4 (`target_invalidates_intention`) consultation —
  `HeldIntention.target` is always `None` at the current author site (per
  the comment at `goap.rs:3060`). Lands with 127/129.
- `DispositionConstants` tuning to recover the bonds_formed -23% /
  kittens_born -75% regression that landed with 126. Predict direction in
  the PR description; measure on the post-246 soak; open a follow-on
  tuning ticket only if recovery is insufficient.
- The ≤+0.10 over-defense bias in modifier-preempt-aftermath
  re-elections. Bounded; only fires in the rare orphan-then-preempt path.
  Acceptable; not opened as a follow-on.

## Current state

- `IntentionMomentum` modifier registered at
  `src/ai/modifier.rs:888-933` (in pipeline at `src/ai/modifier.rs:3492`).
  Reads three scalars via `fetch_scalar` closure.
- `ScoringContext` (`src/ai/scoring.rs:209`) carries the three input
  fields (`intention_held_action_ordinal`, `intention_momentum_lift_factor`,
  `intention_source_ordinal`) — added by 126 C2 alongside the modifier
  registration. No struct surgery needed in 246.
- L2 author site at `src/systems/goap.rs:2062-2064` zeroes the three
  fields with the comment naming C3 as the wiring slot.
- The strict-floor patch sits at `src/systems/goap.rs:3062-3099` inside
  `resolve_goap_plans`'s trigger-3 preempt check.
- `HeldIntention` insertion: `goap.rs:2427-2436` (post-scoring author).
  Removal: `goap.rs:3738` (batched with `GoapPlan` for §7.2 drops).
  Modifier-preempt orphan path: `goap.rs:820` removes only `GoapPlan`,
  leaving `HeldIntention` live for the next-tick re-election (the modifier's
  live-fire window).

## Approach

Plan: `/Users/will.mitchell/.claude/plans/work-246-jaunty-comet.md`.

Two load-bearing edits:

1. **Wire the scalars** at `goap.rs:2059-2064` from
   `world_state.held_intentions.get(entity).ok()`:
   - `intention_held_action_ordinal`: `(held.held_action as usize as f32) + 1.0`
     (offset-by-one ordinal encoding per `src/ai/modifier.rs:798-803`); `0.0`
     when none.
   - `intention_momentum_lift_factor`:
     `held.commitment_strength * d.intention_momentum_lift * held.decay_factor(res.time.tick, d.intention_momentum_decay_ticks)`;
     `0.0` when none.
   - `intention_source_ordinal`: `0.0` for `SelfMotivated`, `1.0` for
     `CoordinatorDirective`; `0.0` when none.

   Adding `held_intentions: Query<&HeldIntention>` to `WorldStateQueries`
   mirrors the existing read-only-query fields (e.g., `active_directive_query`).
   Per `learning_bevy_schedule_edge_perturbation`, field-level edits to
   existing system queries are not the leading suspect for schedule-edge
   perturbation; both `evaluate_and_plan` and `resolve_goap_plans` are
   `.chain()`d so the parallel-access set unchanged matters anyway.

2. **Retire the floor** at `goap.rs:3062-3099`:
   - Delete `const PREEMPT_STRENGTH_FLOOR: f32 = 0.5;` and the
     `if held.commitment_strength >= PREEMPT_STRENGTH_FLOOR` gate.
   - Keep the inner threshold formula (lines 3078-3081) unchanged.
   - Update the surrounding comment block to name the modifier as the
     substrate that defends weak-strength intentions; remove the
     strict-floor rationale.

The decision to keep the formula's `+ commitment_strength × intention_momentum_lift`
addend (instead of also removing it for symmetry with the modifier's lift) is
load-bearing: the formula's middle term correctly compensates in the
*steady-state* path, where `last_scores` was populated at adoption-tick
*before* `HeldIntention` existed (modifier was zero, held score in
`last_scores` is un-lifted). Removing it would under-defend in the common
case to gain symmetry in a rare case (modifier-preempt aftermath, where the
lift is double-counted by ≤+0.10). Plan §"Resolved design" has the full
analysis.

### Structural-option menu (CLAUDE.md "Bugfix discipline")

- **Split** — N/A. Modifier and preempt formula stay separate concerns.
- **Extend** — chosen. Extend `ScoringContext` construction at the L2
  author site to populate three scalars from `Option<&HeldIntention>`.
- **Rebind** — considered live-rescoring at preempt time. Rejected: 126's
  plan explicitly rules it out (schedule-edge perturbation).
- **Retire** — chosen. The `PREEMPT_STRENGTH_FLOOR = 0.5` gate retires.

## Verification

1. **Pre-PR baseline**: `just soak 42` on current `main` (post-126).
   Capture: footer counts (`IntentionAdopted`, `IntentionFulfilled`,
   `IntentionAbandoned{Preempted}`); `bonds_formed`; `kittens_born`;
   wall-clock cats/sec (the 2.75 → 0.38 cliff proxy).
2. **Post-wiring soak**: `just soak-trace 42 <focal>` (focal cat per
   `logs/baselines/current.json`, per `feedback_focal_trace_default`). Run
   `just verdict <run-dir>` — pass = canaries hold (no Starvation, no
   ShadowFoxAmbush spike, all six continuity canaries ≥1,
   `never_fired_expected_positives == 0`).
3. **Frame-diff**: `just frame-diff <pre-246> <post-246>` — confirm
   per-DSE final_score for cats with `HeldIntention` shifted by the
   expected `commitment_strength × 0.10 × decay_factor` magnitude on the
   held DSE specifically. The L3 capture should show `lift_factor > 0`
   post-modifier-preempt (was always `0.0` pre-246).
4. **Sweep-stats**: `just sweep-stats <pre-246-baseline> --vs <post-246>`
   — Welch's t on `bonds_formed`, `kittens_born`,
   `IntentionAbandoned{Preempted}/IntentionAdopted` ratio. Pass band: no
   regression on bonds/kittens vs 126's already-degraded levels (marginal
   recovery or flat is success); preempt-ratio rises modestly as
   weak-strength intentions are no longer floor-masked.
5. **Unit test**: in `src/ai/modifier.rs::tests`. Asserts `apply` with
   non-zero scalars returns `score + lift`; ordinal mismatch / `lift <= 0` /
   `score <= 0` short-circuits return `score` unchanged.
6. **Scenario test**: in `src/scenarios/`. Preload cat with `HeldIntention`
   (orphan-style, no `GoapPlan`); assert focal trace shows
   `intention_momentum_lift_factor > 0` and held DSE's final score above
   its un-lifted score.

## Log
- 2026-05-08: opened (frontmatter only) on land of 126 per CLAUDE.md
  "Antipattern migration follow-ups are non-optional".
- 2026-05-08: in-progress — plan
  `/Users/will.mitchell/.claude/plans/work-246-jaunty-comet.md`.
- 2026-05-08: scenario-side test withdrawn before landing.
  `pre_resolve_focal` runs after the first `app.update()` (so the cat's
  initial Hunt election is invisible to focal trace), and
  `update_capability_markers` strips `CanHunt` after one r_g_p movement
  step (so Hunt's L2 row goes to `eligible: false` with empty
  `modifier_deltas` from tick 2 onward). Direct `eprintln!` probes
  confirmed the wiring fires correctly (`intention_momentum_lift_factor
  = 0.0992` on the first orphan-Held re-election tick, decaying to
  `0.0950` by tick 30). Coverage rests on the 5 existing modifier-side
  unit tests + the verification soak's `just frame-diff` per
  §Verification step 3.
- 2026-05-08: post-wiring soak with floor removed collapsed at
  `tuned-42-post-246-floor-removed-collapsed`. 5580 ticks observed vs
  baseline ~106k (94.8% duration drift, ~15× sim-throughput slowdown).
  Cats locked in a PickUp/Drop loop (13863 PickingUp plan adoptions,
  13568 ItemDropped events, 99.5% of all CatSnapshot actions = PickUp).
  0 Stores built → no `RestingSpot` zone → 1172 Resting GoalUnreachable
  + 526 Guarding GoalUnreachable plan failures slamming the planner.
  12 expected-positive Features never fired (`HuntAttempted`,
  `FoodEaten`, `BondFormed`, `BuildingConstructed`, `FoodCooked`,
  `MatingOccurred`, `GroomedOther`, `MentoredCat`, `BurialPerformed`,
  `CourtshipInteraction`, `PairingIntentionEmitted`,
  `GatherHerbCompleted`). User observed visually: cats converge on
  ground items, cluster, and freeze. The 246 plan's "cliff does NOT
  recur" hypothesis was wrong — `last_scores` reflects the modifier's
  lift only on the orphan-Held re-election (rare), not on the much
  more common §7.2-Achieved re-election where last_scores was
  populated before HeldIntention existed. With the floor removed,
  trigger-3's threshold collapses for low-strength intentions and the
  planner is hammered with replans. Floor restored; wiring kept.
  Follow-on ticket TBD owns the diagnosis. Scenario
  `intention_momentum_pickup_lock` did NOT repro at 3-cat / 60-tick
  scale — kept as a positive guard against future scenario-scale
  regressions.
