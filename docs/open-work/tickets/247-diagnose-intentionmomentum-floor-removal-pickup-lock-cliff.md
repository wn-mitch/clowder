---
id: 247
title: Diagnose IntentionMomentum + floor-removal PickUp-lock cliff
status: blocked
cluster: null
added: 2026-05-08
parked: null
blocked-by: [246]
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

<!--
Bugfix-shape ticket. Use this template (rather than `_template.md`) when the
work is a fix to observed defective behavior. The "Bugfix discipline" section
of CLAUDE.md REQUIRES at least one structural-revision candidate per fix-shape
decision tree — the slots below force that to be drafted, named, and considered.
-->

## Why

Ticket 246 wired the `IntentionMomentum` modifier via `ScoringContext`
(scalars now populated from `Option<&HeldIntention>` at the L2 author site
in `src/systems/goap.rs:2059-2087`) AND attempted to retire the
`PREEMPT_STRENGTH_FLOOR = 0.5` strict-floor patch at
`src/systems/goap.rs:3062`. The wiring landed cleanly. The floor removal
collapsed the soak: 5,580 ticks observed vs ~106k baseline (94.8% duration
drift), with cats locked in PickUp/Drop loops at 99.5% of all CatSnapshot
actions, 0 Stores built, 1,172 Resting GoalUnreachable + 526 Guarding
GoalUnreachable plan failures slamming the planner. 12 expected-positive
Features never fired (HuntAttempted, FoodEaten, BuildingConstructed,
MatingOccurred, …). User observed visually: cats converge on ground items
and freeze in clusters. The floor was restored at the end of 246; this
ticket owns the diagnosis and the substrate-correct fix that lets the floor
retire.

## Hot context (from 246's investigation — promote rows below before any fix)

- **Failing run** (preserved as evidence):
  `logs/tuned-42-post-246-floor-removed-collapsed/`. Seed 42, commit
  `33f326ad` (dirty), focal Mallow.
- **Healthy comparison** (wiring kept, floor restored):
  `logs/tuned-42` at the same commit produces 122,758 ticks, aggregate
  2196, courtship 3315, mentoring 557, only `BurialPerformed`
  never-fired. Wiring is benign with the floor in place.
- **Pre-246 baseline**: `logs/tuned-42-pre-246-e8485ac7/` (commit
  `e8485ac7`, dormant modifier + floor present). Aggregate 2078,
  courtship 2629, mentoring 421.
- **246's failed hypothesis** (from the plan at
  `/Users/will.mitchell/.claude/plans/work-246-jaunty-comet.md`): the
  modifier's lift would defend `held_score` in `last_scores` enough to
  keep `preempt_threshold` non-trivial without the floor. **Wrong**.
  `last_scores` is populated when `evaluate_and_plan` runs, which is
  only when the cat is `Without<GoapPlan>` — typically right after a
  §7.2-Achieved drop, where the previous tick's HeldIntention was
  already removed alongside the GoapPlan (per `goap.rs:3766-3769`).
  So `last_scores[held]` reflects the modifier's lift only in the
  narrow `check_modifier_preemption` orphan window (56 occurrences in
  the 5,580-tick collapsed run). Everywhere else, `last_scores[held]`
  is un-lifted and the formula's middle term must compensate — but
  that requires `commitment_strength × 0.10`, which collapses to zero
  for low-strength intentions. Without the floor's `>= 0.5` gate,
  trigger-3 fires constantly for low-strength held intentions,
  preempting plans, slamming the planner with replans.
- **Key data** (collapsed run):
  - `IntentionAdopted: 14,816` vs `IntentionFulfilled: 13,612` —
    cats churn through ~14k PickUp adoptions. Adoption rate ≈ 2.5
    per tick across 8 cats vs pre-246's 0.36/tick.
  - `CommitmentDropTriggered: 13,995` (10× the per-tick rate vs
    pre-246's 0.26/tick). Most are SingleMinded "Achieved" drops —
    PickUp plans complete in 1 tick.
  - `ItemDropped: 13,568` ≈ same magnitude as PickUp adoptions. Cats
    are perpetually Drop-PickUp-Drop-PickUp because no Stores ever
    get built (no deposit target → inventory fills → DropItem-as-
    prefix from ticket 231 fires on every PickUp).
  - `planning_failures_by_disposition` post-246: Resting=1172,
    Guarding=526, Hunting=75, Foraging=70 (vs pre-246's 0/2/40/28).
    Resting fails because no Stores → `RestingSpot` zone resolves to
    `None` (per `goap.rs:7752-7757`) → Sleep step unreachable.
  - `Resting` and `Guarding` have NO `DispositionFailureCooldown`
    entry (per `src/ai/modifier.rs::DispositionFailureCooldown::signal_key`)
    — they re-elect immediately after a planning failure, slamming
    the planner.
- **Scenario non-repro**: 3 cats + 5 ground items + no Stores at 60
  ticks (`src/scenarios/intention_momentum_pickup_lock.rs`) does NOT
  reproduce the lock. Colony-scale dynamics required (cat density,
  continuous item generation from prey, plan-failure cascades).

## Current architecture (layer-walk audit)

Walk every layer of the AI pipeline relevant to the defect. Tag each
load-bearing fact `[verified-correct]` (you read the code or a recent run
and it matches the assumption), `[suspect]` (you haven't verified, or it
looks wrong), or `[needs-promote]` (auto-prefilled by `/ticket-from-session`
from a hypothesis the Plan agent couldn't promote — the next session
promotes via a fresh query before any candidate that depends on the row).
A row tagged `[suspect]` or `[needs-promote]` MUST be addressed by at
least one of the fix candidates below.

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | `src/components/markers.rs::HasGroundCarcass` | Re-asserts each tick from ground food items; gates PickingUp eligibility | `[needs-promote]` |
| L2 DSE scores | `src/ai/dses/picking_up.rs` (DSE) + `src/ai/modifier.rs::IntentionMomentum` (lift) | PickingUp scores high when ground items present + free slot; modifier lifts held DSE only in orphan-Held window | `[needs-promote]` |
| L3 softmax | `src/systems/goap.rs::evaluate_and_plan` softmax + last_scores capture (line 2182) | `last_scores` written at end of e_a_p, BEFORE HeldIntention exists for new adoptions; reflects modifier lift only in orphan-Held re-elections | `[needs-promote]` |
| Action→Disposition mapping | `src/components/disposition.rs::from_action` (line 287) | `Action::PickUp → DispositionKind::PickingUp` (1:1) | `[verified-correct]` |
| Plan template | `src/ai/planner/actions.rs::picking_up_actions` (~line 1034) + `resting_actions` (line 149) + `goap.rs::build_zone_distances::RestingSpot` (line 7752) | RestingSpot zone = nearest Stores + (1, 0); resolves None if no Stores → Sleep step unreachable → Resting GoalUnreachable | `[needs-promote]` |
| Completion proxy | `src/components/commitment.rs::strategy_for_disposition` (line 235) + `should_drop_intention` | PickingUp = SingleMinded; achievement_believed at trips_done >= 1 (one PickUp completes the plan); §7.2 drop removes both GoapPlan and HeldIntention via Commands at goap.rs:3766-3769 | `[needs-promote]` |
| Trigger-3 preempt | `src/systems/goap.rs:3070-3132` | `preempt_threshold = held_score + commitment_strength × 0.10 + 0.05`. With floor: skipped if strength < 0.5. Without floor: fires for any HeldIntention; collapses to `held_score + 0.05` for low-strength → any noise crosses → constant churn | `[needs-promote]` |
| Cooldown coverage | `src/ai/modifier.rs::DispositionFailureCooldown::signal_key` | Covers Hunt/Forage/Cook/Caretake/Build/Mate/Mentor. Does NOT cover Resting/Guarding/PickingUp/Discarding/Trashing/Handing/Socializing/Exploring/Mating/Burying — these can re-elect immediately after planning failure | `[needs-promote]` |

## Fix candidates

**Parameter-level options** (each requires the layer-walk rows to be
promoted before they can be ranked — DO NOT promote without a fresh query
that distinguishes from 246's failed framing per Reframe discipline):

- **R1** — Reduce `intention_preempt_margin` (default 0.05) toward 0.
  Trigger-3 fires less often. Risk: regresses commitment-tenure-style
  oscillation guard for high-strength intentions too.
- **R2** — Make `commitment_strength_from_margin` floor at some minimum
  (e.g., 0.3) instead of clamping at 0. Substrate-correct version of
  the floor: held intentions always defend, just by varying amounts.
  Risk: cats over-defend low-margin elections, harder to escape bad
  initial picks.
- **R3** — Extend `DispositionFailureCooldown` to cover Resting,
  Guarding, PickingUp, etc. Stops the "fail planning → re-elect same
  thing immediately" loop without touching trigger-3. Risk: doesn't
  address the underlying low-strength preempt problem; cats may still
  churn between dispositions.

**Structural options** (at least one MUST be drafted, even if it doesn't win):

- **R4 (extend)** — Branch the trigger-3 formula on
  `commitment_strength` regime: high-strength uses the current formula
  (substrate defends via lift × 0.10); low-strength routes through
  natural §7.2 drop only. Effectively re-implements the floor as a
  substrate-side branch with a documented rationale (commitment
  strength below the noise threshold can't meaningfully defend its
  intention).
- **R5 (rebind)** — Re-author `last_scores` after the L2 author site
  inserts HeldIntention, OR have trigger-3 re-score the held DSE live
  (re-introducing the schedule-edge perturbation that 126's plan
  ruled out — but maybe constraining to single-DSE re-score keeps the
  cost bounded). Lets the formula's middle term be honest.
- **R6 (split)** — Split the trigger-3 path: high-strength path uses
  `held_score + lift + margin` (formula needs lift in `last_scores`);
  low-strength path uses an absolute-margin guard (`top_non_held >
  some_constant`) that doesn't depend on held_score. Two semantically
  distinct preempt rules instead of one parameterized rule.
- **R7 (retire)** — Retire trigger-3 entirely; rely on
  `check_modifier_preemption` + §7.2 drops + cooldown extension (R3)
  for all reconsideration. Removes the load-bearing hack but loses
  the "single-minded but not stupid" knob the original 126 design
  named. Validate that the loss is acceptable via a dedicated
  reconsideration scenario.

## Recommended direction
TBD — promote the layer-walk rows first. Strong prior: R4 (extend) is the
substrate-correct shape because it preserves the floor's effect (skip
trigger-3 for low-strength) while making the rationale visible at the read
site rather than masked behind an opaque constant. R5 (live re-score) is
worth investigating only if R4 turns out to dampen too much.

## Out of scope
- Re-retiring the floor in this ticket without the diagnosis being clean
  — that's exactly what 246 attempted. The fix candidate must explicitly
  pre-soak before claiming the floor can be retired.
- The `last_scores` schedule-edge perturbation question (live re-score at
  preempt time). 126's plan ruled this out; revisiting requires its own
  ticket.
- DispositionFailureCooldown coverage gaps for non-Hunt/Forage/Cook
  dispositions (Resting / Guarding / Socializing / etc.). Surfaced here as
  contributing to the cliff but the broader cooldown audit is a sibling
  concern; spin out if R3 is the recommended fix.

## Verification
1. **Pre-fix baseline**: `logs/tuned-42-post-246-floor-removed-collapsed/`
   already captures the failing state. New fix runs against this as
   the "must beat" baseline.
2. **Post-fix soak**: `just soak-trace 42 Mallow` then
   `just verdict <run-dir>`. Pass = duration_drift_pct < 20% vs
   `tuned-42-pre-246-e8485ac7`, all six continuity canaries ≥ 1, no
   never_fired_expected_positives.
3. **Frame-diff**: `just frame-diff <pre-246> <post-fix>` shows
   `intention_momentum` modifier delta on the held DSE during orphan
   re-elections (proves wiring still fires) AND no PickUp domination
   in the focal action distribution.
4. **Scenario**: `intention_momentum_pickup_lock` continues to pass
   (no scenario-scale lock).

## Log
- 2026-05-08: opened from 246's failed floor removal. Hot context
  preserved above. Layer-walk rows are `[needs-promote]` — fresh
  queries required before any candidate is ranked. 246 left the floor
  in place; this ticket owns the substrate-correct retirement.
