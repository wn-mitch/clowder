---
id: 247
title: Diagnose IntentionMomentum + floor-removal PickUp-lock cliff
status: done
cluster: ai-substrate
added: 2026-05-08
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 7cd1b00ba71d
landed-on: 2026-05-08
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
| L1 markers | `src/components/markers.rs::HasGroundCarcass` (lines 500-504); writer at `src/systems/goap.rs:1291-1294`; reader at `src/ai/dses/picking_up.rs:93-94` | Re-asserts each tick from ground food items via colony-marker-author scan over Items with `location == OnGround` and `kind.is_food()`; gates PickingUp DSE via `require(HasGroundCarcass::KEY)` | `[verified-correct]` |
| L2 DSE scores | `src/ai/dses/picking_up.rs:62-96` (DSE composition: inverted Logistic over `colony_food_security` + health-deficit Linear damping); `src/ai/modifier.rs:902-928` IntentionMomentum (short-circuits on `lift_factor <= 0.0`, gates on `dse_id_for_action(held_action)` match, adds `lift` to score) | PickingUp scores high when ground items present + free slot; modifier reads `intention_held_action_ordinal` + `intention_momentum_lift_factor` scalars and lifts the held DSE | `[verified-correct]` |
| L3 softmax | `src/systems/goap.rs:2182` (`last_scores` capture) AND `src/systems/goap.rs:2469` (`HeldIntention` insertion at adoption branch) | `last_scores` is captured BEFORE `HeldIntention` is authored on fresh adoption — the recorded `held_score` never sees the modifier's lift. **The trigger-3 formula `held_score + commitment_strength × intention_momentum_lift + intention_preempt_margin` is designed to re-add the missing lift; for low `commitment_strength` the compensation collapses below margin noise, undefending the held intention.** | **`[verified-defect]`** |
| Action→Disposition mapping | `src/components/disposition.rs:287` | `Action::PickUp => Some(Self::PickingUp)` (1:1) | `[verified-correct]` |
| Plan template / RestingSpot zone | `src/systems/goap.rs:7766-7771` `RestingSpot` zone resolution via `.iter().min_by_key(...).map(...)` over `stores_positions` | When `stores_positions` is empty, `.map()` yields `None` → `RestingSpot` zone unresolved → `ZoneIs(RestingSpot)` precondition fails → Sleep step unreachable → `Resting:GoalUnreachable` (1172 occurrences in collapsed run footer) | `[verified-correct]` |
| Completion proxy + §7.2 dual-removal | `src/ai/commitment.rs:235` (`PickingUp => SingleMinded`); drop trigger at `src/ai/commitment.rs:155-164` (SingleMinded drops on `achieved \|\| unachievable`); `src/systems/goap.rs:3779-3783` removes both `GoapPlan` and `HeldIntention` in same `plans_to_remove` iteration | PickingUp completes in 1 tick (`trips_done >= 1`), then §7.2 dual-removal clears HeldIntention alongside the plan — leaves the cat re-electing on the next tick with last_scores still un-lifted | `[verified-correct]` |
| Trigger-3 preempt | `src/systems/goap.rs:3070-3144` (formula); `intention_momentum_lift = 0.10`, `intention_preempt_margin = 0.05` (header dump from collapsed run) | `preempt_threshold = held_score + commitment_strength × 0.10 + 0.05`. For `commitment_strength = 0.1`, compensation = 0.01 — far below the 0.05 margin floor. Pre-247 floor at 0.5 was equivalent to: "only run trigger-3 when commitment compensation ≥ margin." | `[verified-correct]` (floor's effect is correct; only the encoding was opaque) |
| Cooldown coverage | `src/ai/modifier.rs:2673-2686` `DispositionFailureCooldown::signal_key` match arms | Covered: Hunt, Forage, Cook, HerbcraftGather, HerbcraftPrepare, HerbcraftWard, MagicScry, MagicDurableWard, MagicCleanse, MagicColonyCleanse, MagicHarvest, MagicCommune, Caretake, Build, Mate, Mentor. **Uncovered:** Resting, Guarding, PickingUp, Discarding, Trashing, Handing, Socializing, Exploring, Mating, Burying, Grooming, Coordinating. Resting=1172 + Guarding=526 GoalUnreachable in collapsed run footer concentrated in uncovered set. | **`[verified-defect]`** (Phase D follow-on; out of scope for 247) |

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
- 2026-05-08: **Phase A diagnostic** on
  `logs/tuned-42-post-246-floor-removed-collapsed/` confirmed the
  ticket's runtime claims via skill surface (`just q run-summary`,
  `actions`, `footer`, `anomalies`): final_tick=1,205,580 (5,580
  simulated), 99.55% PickUp action distribution,
  `planning_failures_by_disposition` Resting=1172 / Guarding=526 /
  Hunting=75 / Foraging=70 (all `GoalUnreachable`), continuity
  tallies all collapsed except grooming=3, 12 expected-positive
  Features never fired. Header constants confirmed
  `intention_momentum_lift=0.10`, `intention_preempt_margin=0.05`.
  The collapsed run's trace-Mallow.jsonl is header-only (focal cat
  emitted no L2/L3 records before collapse), so the bimodal
  `commitment_strength` distribution couldn't be queried directly —
  but the action-distribution + planning-failure breakdown is
  conclusive evidence of low-strength PickUp adoptions churning at
  ~2.5/tick. **Code-side promotion** of the layer-walk rows replaced
  `[needs-promote]` with verified status (see audit table above):
  H3 (`last_scores` capture at `goap.rs:2182` precedes `HeldIntention`
  insertion at line 2469) and H7 (`DispositionFailureCooldown::signal_key`
  match arms at `modifier.rs:2673-2686` omit Resting / Guarding /
  PickingUp / Discarding / Trashing / Handing / Socializing /
  Exploring / Mating / Burying / Grooming / Coordinating) are
  `[verified-defect]`; H1, H2, H4, H5, H6, plus the trigger-3 row
  itself, are `[verified-correct]`.
- 2026-05-08: **Phase B (R4 implementation).** Replaced function-local
  `const PREEMPT_STRENGTH_FLOOR: f32 = 0.5;` at `src/systems/goap.rs:3107`
  with a read of `d.intention_preempt_strength_regime_boundary` (new
  field on `DispositionConstants`, default 0.5). Added field +
  default-fn + `Default` impl entry at `src/resources/sim_constants.rs`.
  Updated trigger-3 rustdoc to reframe the gate as a *named substrate-
  side branch* with the substrate-correctness rationale (modifier
  compensation `commitment_strength × intention_momentum_lift`
  collapses below `intention_preempt_margin` noise floor → §7.2
  natural drop is the honest fall-through), keeping the 246-history
  paragraph and naming this ticket. `just check` clean (cargo check,
  clippy, step-contract, time-units, iaus-coherence, substrate-stubs,
  items-are-real, InfluenceMap registry).
- 2026-05-08: **Phase C verification.**
  (1) `cargo test focal_does_not_lock_on_pickup` (regression guard) —
  passes; scenario does not lock at scenario scale post-R4 (matches
  pre-R4 behavior since the lock is colony-scale only).
  (2) `just soak-trace 42 Mallow` followed by `just verdict logs/tuned-42
  --baseline logs/tuned-42-post-246-floor-restored-33f326ad/events.jsonl`
  — duration_drift_pct=1.9% (well under 20%), aggregate score
  2193.89 vs 2196.15 baseline (-0.1%, band:pass), welfare +4.4%,
  zero deaths from any cause, planning_failures
  Resting=0 / Guarding=2 / Hunting=40 / Foraging=28 (vs collapsed
  run's 1172 / 526 / 75 / 70 — cliff is gone), continuity tallies
  courtship=3223 / grooming=1804 / mentoring=546 / play=14 /
  mythic-texture=27 / burial=0 (only burial unchanged from baseline,
  pre-existing condition), single never-fired Feature is
  `BurialPerformed` (also pre-existing). The `verdict: fail` is
  driven entirely by `burial=0`, which is identical in the post-246
  floor-restored baseline → not introduced by R4.
  (3) `just frame-diff logs/tuned-42-post-246-floor-restored-33f326ad/trace-Mallow.jsonl
  logs/tuned-42/trace-Mallow.jsonl` — concordance: ok, no per-DSE
  drift on tracked DSEs.
  Conclusion: R4 preserves the floor's effect identically; the named
  substrate-side branch is byte-equivalent to the function-local
  constant in expected behavior. Floor retirement (boundary → 0.0)
  remains gated on a future ticket per the rustdoc + sim_constants
  doc-comment forward pointer.
- 2026-05-08: **Phase D evaluation.** R3 follow-on (cooldown coverage
  for Resting / Guarding / et al.) NOT opened. Post-R4 planning-failure
  counts for Resting (=0) and Guarding (=2) match the post-246
  floor-restored baseline; the cooldown gap remains `[verified-defect]`
  but is not load-bearing once trigger-3 churn is resolved (the
  Resting cascade in the collapsed run was driven by no-Stores →
  no-RestingSpot → GoalUnreachable, not by the cooldown gap itself).
  If a future ticket revisits cooldown coverage, cite 247's H7 audit
  row.
- 2026-05-09: **H7 closure attempt via 249 — closed without
  landing.** 249 was opened the next day as the cooldown-coverage
  follow-on despite Phase D's "not load-bearing" conclusion. The
  in-session audit reframed it as a TargetExistence-marker fix
  (author `ColonyHasStores`, gate `SleepDse.eligibility` on it).
  Verification soak surfaced an 11× regression in
  `acute_health_adrenaline_flee` modifier-preemption rate (4,228 →
  32,902; baseline ~347/10kt → current ~3,830/10kt — back to pre-230
  levels), because the DSE-eligibility gate starved the 047 modifier's
  Sleep-lift landing target during cold-start, undoing 230's
  substrate-aware preempt-rate reduction. 249 was rolled back without
  landing; H7 remains `[verified-defect]` but not load-bearing per
  Phase D, and the DSE-eligibility-vs-plan-template-gate distinction
  is documented in §3.5.5 / §4.3 of
  `docs/systems/ai-substrate-refactor.md` plus
  `src/ai/modifier.rs::DispositionFailureCooldown` rustdoc (which
  *did* land — independently true architectural understanding).
  Three follow-on tickets opened from the audit; see the §Log of
  closed-without-landing 249 for the IDs.
