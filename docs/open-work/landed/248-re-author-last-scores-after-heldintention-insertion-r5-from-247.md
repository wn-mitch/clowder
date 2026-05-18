---
id: 248
title: Re-author last_scores after HeldIntention insertion (R5 from 247)
status: done
cluster: ai-substrate
initiative: []
added: 2026-05-08
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 111987ae767b
landed-on: 2026-05-08
---

## Why
247 promoted H3 to `[verified-defect]`: at `src/systems/goap.rs`,
`last_scores` is captured at line 2182 (end of `evaluate_and_plan`)
BEFORE `HeldIntention` is inserted at line 2469 in the L3 adoption
branch. On a fresh-adoption tick, the recorded `held_score` never
sees the `IntentionMomentum` modifier's lift. The trigger-3 formula
`held_score + commitment_strength × intention_momentum_lift +
intention_preempt_margin` re-adds that lift in the threshold
calculation — but for low `commitment_strength` (e.g. 0.1 × 0.10
lift = 0.01), the compensation collapses below the 0.05 margin
noise floor, undefending the held intention. 247 priced this defect
via the `intention_preempt_strength_regime_boundary` (default 0.5,
skip trigger-3 below) but did NOT fix the underlying timing issue.

This ticket owns the substrate-correct fix. With `last_scores`
honestly reflecting the modifier's lift on the held DSE, the
formula's middle term becomes redundant and the regime boundary can
retire to 0.0 without re-collapsing — closing 246's failure mode
properly.

## Scope
- Re-author `last_scores` so it includes the `IntentionMomentum`
  modifier's lift on the held DSE, OR re-score the held DSE live at
  the trigger-3 read site so the comparison uses honest scores.
- Verify post-fix that setting
  `intention_preempt_strength_regime_boundary` to 0.0 no longer
  collapses the soak (replicates 246's failure scenario as a
  regression test).
- Update `goap.rs:3070-3144` rustdoc and the
  `intention_preempt_strength_regime_boundary` doc comment to
  reflect that the gate has retired (or to document why it's still
  load-bearing if R5's chosen variant is partial).

## Out of scope
- Tuning `intention_momentum_lift` / `intention_preempt_margin` /
  `intention_momentum_decay_ticks` — separate balance concerns.
- DispositionFailureCooldown coverage gaps (H7) — see ticket 249.

## Current state
- 247 landed at sha `7cd1b00b` (R4: substrate-side branch making
  the floor's effect a named tuning constant).
- The IntentionMomentum modifier wiring (246) is in place at
  `src/systems/goap.rs:2069-2097` and `src/ai/modifier.rs:902-928`.
- The defective sequencing is `evaluate_and_plan` writing
  `last_scores` at line 2182 then the L3 adoption branch inserting
  `HeldIntention` at line 2469.
- 126's plan ruled out the schedule-edge perturbation of
  re-scoring at trigger-3 read time; this ticket should evaluate
  whether constraining the live re-score to the single held DSE
  (rather than a full pool re-score) keeps the cost bounded enough
  to revisit.

## Approach
Two structural candidates from 247's fix-candidate menu:

**A — Re-author `last_scores` after the L2 author site inserts
`HeldIntention`.** Either move the `last_scores` capture to AFTER
the adoption branch in `evaluate_and_plan`, or have the L3 adoption
branch re-write the held DSE's entry in `last_scores` after
inserting `HeldIntention`. Sequencing change; no live re-score.

**B — Live re-score the held DSE at trigger-3 read time.** Inside
the trigger-3 block at `src/systems/goap.rs:3070-3144`, re-invoke
the modifier pipeline for the single held DSE so the comparison
uses an up-to-date lifted score. 126's plan flagged the
schedule-edge perturbation risk; constraining to one DSE may keep
the cost bounded.

Pick A or B based on a layer-walk of `evaluate_and_plan`'s
sequencing constraints (other readers of `last_scores`, RNG state
preservation, `pre_bonus_pool_snapshot` for the L2-vs-pool
invariant in `tests/scenarios.rs`).

## Verification
1. **Regression scenario** — `intention_momentum_pickup_lock`
   continues to pass.
2. **Floor-retirement soak** — set
   `intention_preempt_strength_regime_boundary = 0.0` in a test
   override or temporary constant; run `just soak-trace 42 Mallow`;
   confirm duration_drift_pct < 20% vs the post-247 baseline AND
   continuity canaries match. This reproduces 246's failing
   scenario as a positive verification: with R5 in place, removing
   the gate should not re-collapse.
3. **Frame-diff** — `just frame-diff` between post-247 and
   post-R5 traces shows the IntentionMomentum modifier lifting
   `held_score` in `last_scores` directly (not just in the formula).
4. **L2-vs-pool invariant** — the snapshot test in
   `tests/scenarios.rs` continues to pass (no mutation of the
   score Vec between snapshot and softmax).

## Log
- 2026-05-08: opened from 247's §Out of scope. H3 row promoted in
  247 via code-side queries; this ticket owns the substrate-correct
  fix that lets 247's `intention_preempt_strength_regime_boundary`
  retire.
- 2026-05-08: verification: just soak-trace 42 Mallow at boundary=0.5, 122,013 ticks, aggregate 2195 vs 2196 baseline (-0.0%), zero deaths, planning_failures Hunting=40/Foraging=28/Guarding=2 matching post-247 baseline; boundary=0.0 collapsed at ~5,000 ticks (preserved as evidence at logs/tuned-42-post-248-boundary-zero-collapsed/), proving 247's regime gate is still load-bearing for softmax-low-margin oscillation rather than the lift-timing defect. just frame-diff vs post-247: concordance ok.
