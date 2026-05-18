---
id: 400
title: L2 ParentingActivity — implementation per 399 design
status: done
cluster: social-coordination
orchestration: substrate-sensitive
initiative: [smarter-cats, htn-method-composition]
added: 2026-05-17
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, htn-methods.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-18
---

## Why

399 resolved the design space for L2 ParentingActivity: drop 398's
`is_mother` L1 gate, replace the uniform `AspirationLift(+0.2 Caretake)`
with a personality-conditional `ParentingActivityModifier` composed
across five orthogonal scales of parenting (Presence / Provision /
Protection / Cultural / Autonomy), and treat parenthood as a lifelong
relational stance encoded via `Vec<RelationshipTo>` with multiple
`ParentalKind` adoption pathways.

This ticket implements that design. The substrate work is structurally
honest at 399's land moment — once this ticket lands, every cat with a
biological offspring (or a transitively-adopted in-law) expresses
parenthood through personality-weighted bias rather than a sex-gated
uniform lift, and the dispersion mechanism + JointIntention-aware
suppression resolves the pre-398 HandoffItem cascade without
re-introducing the override layer 398 retired.

See plan: `/Users/will.mitchell/.claude/plans/let-s-start-399-i-m-jaunty-dawn.md`.

## Scope

Four implementation phases (commit per phase or bundled — implementor's call):

**Phase 1: Substrate prep**
- New file `src/components/parenting_activity.rs` — `ParentingActivity` Component, `RelationshipTo` struct, `ParentalKind` enum (Biological + InLaw shipped; BondFormed + Adopted declared but unwired, gated to follow-on tickets 403/404).
- New file `src/systems/parenting_activity.rs` — per-tick `parental_engagement` update system; lifecycle management (insert on KittenBirth, persist through dissolution events, drop only on self-death).
- Modify `src/systems/aspirations.rs:856-916` (`adopt_kinship_aspiration`) — drop `is_mother` gate; widen to `Has<Parent>`; insert `ParentingActivity` Component with one `RelationshipTo(kind=Biological)` entry per dependent kitten.
- Modify `src/plugins/simulation.rs` — register Component + per-tick system.

**Phase 2: ParentingActivityModifier + bias formulas**
- Modify `src/ai/modifier.rs` — add `ParentingActivityModifier` implementing `ScoreModifier`. Five bias formulas (caretake / provision / protect / cultural_teach / autonomy_teach) iterating over `Vec<RelationshipTo>`.
- Modify `src/ai/eval.rs` — register the new modifier in `ModifierPipeline`.
- Modify `src/ai/dses/caretake.rs` — replace `is_parent` (0/1) axis in WeightedSum with `parental_engagement` (gradient) read.
- Neuter the existing Kinship-domain side of `compute_aspiration_action_counts` for Caretake (the +0.2 uniform lift; the new modifier handles it personality-conditionally now).

**Phase 3: InLaw adoption rule (~30 lines)**
- Modify `src/systems/joint_intention.rs` — on `JointIntention.stage` transition to `Bonded`, mirror InLaw `RelationshipTo` entries on each partner's biological parents.

**Phase 4: Tuning constants + L2 trace + scenario tests**
- Modify `src/resources/sim_constants.rs` — add `ENGAGEMENT_BUILD_RATE` (~0.001), `ENGAGEMENT_DECAY_RATE` (~0.0001), `ENGAGEMENT_RANGE_TILES` (~5), `MATURED_RESIDUAL_FACTOR` (=0.15), `JOINT_SUPPRESSION_FACTOR` (=0.3), asymptote weights `W_N=0.30, W_D=0.20, W_P=0.20, W_C=0.15, W_A=0.15`.
- Surface per-scale values + JointIntention suppression events in L2 trace records.
- Update `just inspect` to render parental 5-vector if cat has ParentingActivity Component.
- Add `src/scenarios/parenting_father_provisions.yaml`, `parenting_joint_suppression.yaml`, `parenting_grief_kitten_death.yaml` (per ticket 162 scenario discipline).

## Out of scope

Per 399's resolution (these have separate follow-on tickets blocked-by 400):

- **Target-axis refinements** (tickets 401, 402): Hunt-toward-partner-Stores axis and Patrol-near-nest axis. Whole-DSE biases ship in 400; target-direction shifts come later.
- **BondFormed / Adopted adoption rules** (tickets 403, 404): 400 declares the enum variants but leaves their adoption-rule logic to dedicated follow-on tickets. The Vec<RelationshipTo> architecture is ready from day one.
- **Family ritual substrate** (ticket 405): RitualKind enum + RitualWitness Component + bond-multiplier transmission. Shares the `bond_multiplier` function with 400's InLaw adoption.
- **Mastery substrate** (ticket 406): `teach_skill_bias` references `mastery(action)` which doesn't exist. 400 ships the formula referencing it; 406 adds the substrate that provides the read.
- **§7.7.b grief cascade proper** (ticket 407): 400 ships the foundation (persistent state + frustrated target-taking); 407 adds explicit mourning DSEs, vigil behaviors, decay rates.
- **Asymptote weights balance tuning** (ticket 408): 400 ships starting-point weights; 408 runs hypothesis-driven balance work to refine.

## Current state

Blocked by 399 (design ticket). Once 399 lands, this ticket promotes to ready.

The current `Has<Parent>` marker semantic ("has at least one dependent kitten alive") stays unchanged — do NOT rename in 400. It remains useful for short-window checks (Caretake target-eligibility, current dependency-window queries); `ParentingActivity` Component is the new lifelong-relationship layer. A future cleanup ticket can address the marker-name conflation if desired.

## Approach

See plan file for substrate shape (Component definition, modifier formula, lifecycle, InLaw rule), implementation phases, file modification list, tuning constants, and verification commands.

Key precedent: `JointIntention` Component (`src/components/joint_intention.rs`, ticket 127) is the closest existing analog — Vec-of-relationships, observable practice-state, modifier reads. ParentingActivityModifier should mirror the `KittenCryCaretakeLift` modifier pattern (`src/ai/modifier.rs`) for the per-DSE per-target accumulator shape.

## Verification

Hard gates:
- `plan_failure_canary[HandoffItem: no recipient]` does NOT regress relative to 398's baseline (the JointIntention-aware suppression is the load-bearing fix for the two-high-compassion corner case).
- Pebblekit-67-equivalent kittens survive (398's survival gate preserved).
- All five continuity canaries hold (grooming, play, mentoring, courtship, mythic-texture).
- `never_fired_expected_positives == 0`.

Soak + focal-trace expectations:
- Standard mother (high compassion): Caretake-bias dominant; `caretake_bias` ~0.4-0.6 in L2 trace.
- Hard-working union man (high diligence + low compassion): Hunt-bias dominant; `provision_bias` ~0.4; `caretake_bias` ~0.1.
- Cat with low loyalty + low compassion + high independence: all positive biases near zero; cat drifts naturally.
- Mess-archetype cat (high temper, low diligence): oscillating L3 chosen action across windows (verify in `just q trace`); validates topic 7's emerges-from-existing-dynamics hypothesis.

Scenario tests (per ticket 162):
- `parenting_father_provisions` — verify provision_bias > caretake_bias for diligent-low-compassion father within first ~50 ticks.
- `parenting_joint_suppression` — verify exactly one parent commits to Caretake (the other yields via JointIntention suppression).
- `parenting_grief_kitten_death` — verify ParentingActivity persists past kitten death; engagement decays but doesn't drop; Caretake target-taking finds nothing.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- · **129** (blocked, social-coordination, score 0.87) — Care DSEs over perceivable intentions
- · **279** (ready, social-coordination, score 0.86) — Body-cue-driven joint adoption (compose 127 with 242 + 243)
- · **341** (ready, process-discipline, score 0.85 (cross-cluster)) — Retarget 057 blocked-by from 126 to 128

<!-- linkages:end -->
## Log

- 2026-05-17: opened as 399's implementation follow-on. Design resolved in 399's `## Log` and in plan file `/Users/will.mitchell/.claude/plans/let-s-start-399-i-m-jaunty-dawn.md`. Blocked-by 399 until that lands.
- 2026-05-18: implementation complete across two local jj commits (substrate + diagnostics/scenarios/suppression-target-plumbing). `just check` + `just test` clean (2278 tests pass, including 2 new ParentingActivity integration tests). 15-min seed-42 soak verdict: **concern**. Survival pass (0 starvation, 0 ambush). `HandoffItem: no recipient (no kittens in colony)` plan failure regresses 26.9× baseline — the JointIntention-aware suppression mechanic works as designed (target-specific via `HeldIntention.target` plumbing, verified by unit test) but doesn't address the "no kittens at all" cascade. Opened ticket 410 with the layer-walk audit and 5 candidate fixes (R3 eligibility filter on Caretake DSE recommended). Status flipped `ready` → `in-progress`; landing waits for 410's fix to clear the canary regression.
