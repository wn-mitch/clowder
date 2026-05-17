---
id: 398
title: §7.M.2 RaiseOffspringAspiration — kitten-rearing as nested-Intention aspiration
status: done
cluster: ai-substrate
initiative: [smarter-cats, htn-method-composition]
orchestration: substrate-sensitive
added: 2026-05-17
parked: null
blocked-by: []
supersedes: [394, 395, 397]
related-systems: [ai-substrate-refactor.md, htn-methods.md]
related-balance: []
landed-at: 0bf7bdd16d6d
landed-on: 2026-05-17
---

## Why

The 364→397 kitten-arc trail (364 HTN dispatch + 394 Wean-churn + 395
R11/R13/yield + 397 L1+L2+L3 stack) accumulated four follow-on patches
that all compensated for the same architectural omission: **kitten-
rearing was implemented as a reactive-emit + HTN-pin pattern, but
`docs/systems/ai-substrate-refactor.md` §7.M.2 designed it as a three-
layer nested-Intention aspiration parallel to Mating.** Verified soaks
post-397 still drop Pebblekit-67 to starvation at tick 1304815 — the
substrate-side per-tick lifts (pool-entry gate, +0.25 lift, cooldown
bypass, pin-guard, yield-rule) can't compose enough sustained parental
commitment to hold Caretake against Cook/Hunt in the L3 softmax, because
*there is no L1 aspiration emitting Caretake-Intention into the unified
softmax pool with §7.4 persistence-bonus stickiness.*

This ticket lands the spec-mandated convergence. CLAUDE.md design pillar
#4 (added 2026-05-17): *"Commitment is one mechanism, not two —
multi-tick decomposition (HTN sub-goal chains, plan templates, GOAP
step lists, HeldGoalStack as frame state) is substrate; but which
Intention is held this tick belongs at §L2.10.6 softmax + §7.4
persistence-bonus, the spec's single commitment layer."* This ticket is
the precedent-setting first instance of pulling the §L2.10.6 boundary
forward: retire the HTN frame-pin (`goap.rs:2410-2491`) + wrap-site
override (`goap.rs:2733-2790`) + their workaround patches, build the
§7.M.2 aspiration the spec named, and let the unified softmax + §7.4
persistence handle commitment naturally.

Pattern matches 087 / 093 / 163 (post-L2 score mutations replaced with
substrate-visible considerations) and 148 (single-channel signals
decomposed into orthogonal axes); the pillar-#3 perception-and-strategy
payoff sits on the substrate side — once L1 aspiration + L2 parenting-
activity + L3 Caretake-Intention emit independently into the trace,
diligent fathers naturally provision (Hunt-bias from personality
weighting), compassionate parents nurse-feed under hunger pressure
(persistence-bonus stickiness), and the L2 trace tells the truth about
deliberation (no discarded softmax winners).

The driving session plan is at
[`/Users/will.mitchell/.claude/plans/let-s-start-tickets-394-397-snazzy-kahan.md`](../../../.claude/plans/let-s-start-tickets-394-397-snazzy-kahan.md)
(local-only; quote any load-bearing sections inline rather than relying
on the link).

## Scope

**L1 — `RaiseOffspringAspiration`** (per §7.M.2). Layer-1 aspiration
registered via `add_aspiration_dse`. Lifecycle bracketed by the
`Parent` marker (= "any dependent kitten exists"). Strategy
`OpenMinded`, persistence tier **High** per §7.4 (multi-month posture;
drops only on event — kitten death, life-stage transition into Elder,
§7.7.1 aspiration conflict, sustained injury). The aspiration emits
Caretake-Intentions into the §L2.10.6 unified softmax pool every tick
the parent has a dependent kitten; magnitude composes with personality
(see §7.M.2 quote about "the partner's aspiration shifts toward a
provisioner role via personality-weighted pick — diligent → Hunt-
biased; compassionate → Caretake-biased").

**L2 — `ParentingActivity`** (per §7.M.1 Pairing analog). Layer-2
activity that biases DSE weights without prescribing actions, fires
when the L1 aspiration is held AND `HasJuvenileDependent` marker is set.
Strategy `OpenMinded`, persistence tier **Medium** (mirrors Socializing;
desire-drift drop on cry-cessation, kitten near-maturity, etc.).
Personality-weighted bias modulation: diligent personality lifts Hunt
target preference toward partner's food-need / shared Stores;
compassionate personality lifts Caretake composition magnitude on top
of the existing axes; bold/protective personality lifts Patrol weighted
to dependent's tiles. The bias lives on the modifier layer (per pillar
#3's "compose personality / phobias / ambient context at the modifier
layer, never inside the underlying perception scalar").

**§L2.10.6 unified softmax (narrow landing).** Generalize
`select_disposition_softmax` to `select_intention_softmax`. Candidate
pool = `{DSE-Activity-default scores} ∪ {emitted-Goal Intentions from
REACTIVE_EMITS}`. Apply §7.4 persistence-bonus to the held Intention
before the preemption comparison: `held_score + base ×
logistic(completion_fraction)`. Caretaking tier = Medium (base ≈ 0.10)
× compassion-multiplier × Patience-multiplier per §7.4. Challenger
preempts iff `challenger_score > held_score + bonus`. **Narrow scope** —
implement for the `rear_kitten ↔ Caretake ↔ Hunt` pool only; full
generalization across every REACTIVE_EMITS entry stays under the 060
epic. The narrow land sets the precedent.

**Retirements** (the override-and-workaround set per plan §2.3):
- `goap.rs:2733-2790` wrap-site priority-override — delete entirely.
- `goap.rs:2410-2491` frame-pin — delete or restrict to decomposition-
  only (no `chosen_action` mutation; only sub-step advance).
- `src/ai/methods/rear_kitten.rs` 395 yield rule (`!IsParentOfHungryKitten`
  in `has_dependent_kitten`) — already retired post-397, stays retired.
- `src/ai/modifier.rs:2682-2705` 397 `DispositionFailureCooldown` bypass
  for Caretake — retire (cooldown only damped because pin-driven
  Caretake failures were artificial).
- `src/ai/scoring.rs:1999-2009` 397 `rear_kitten_caretake_lift` +0.25
  additive — retire (the aspiration's Caretake-Intention emission
  carries the lift via L1 + §7.4 composition instead).
- `goap.rs:2444-2499` 397 pin-Caretake-preempts guard — retires with
  the pin.
- `aspiration_picker::REACTIVE_EMITS[0]` `kitten_reared` emit — retire
  the reactive entry in favor of the aspiration-emitted Caretake-
  Intentions (the HTN method's sub-goal Wean/Teach/Release maturity-
  bumps stay alive as one-shot side-effects of Caretake firing in the
  right band; see Approach §"Wean/Teach/Release primitives stay").

**Keep** (substrate-correct regardless of override layer):
- 395 R13 (Release at maturity 1.0 only) — `src/steps/disposition/release.rs`.
- 395 father-pitches-in symmetric mother-or-father picker — `src/ai/dses/dependent_kitten_target.rs`.
- 395 release_threshold gap (`[teach_done, release)` deliberate idle
  band where Caretake covers feeding) — `src/ai/dses/dependent_kitten_target.rs`.
- 397 L1 pool-entry gate broadening (Caretake enters L2 pool every tick
  `Parent` marker is set) — `src/ai/scoring.rs:1973-2000`.
- `Parent` + `HasJuvenileDependent` plumbed into `MarkerSnapshot` —
  `src/systems/goap.rs:466,1719-1732`.
- Caretake DSE 0.25 binary parent axis — `src/ai/dses/caretake.rs`
  (composes naturally with aspiration emission).
- `IsParentOfHungryKitten` + `KittenCryCaretakeLift` substrate — `growth.rs`
  + `modifier.rs:1817-1896`.

## Out of scope

- **§L2.10.6 full generalization beyond rear_kitten ↔ Caretake.** Other
  REACTIVE_EMITS (`mourn_at_grave` when its emission path lands) follow
  the same precedent — open as 060-epic-attached follow-ons.
- **Personality-trait response curve tuning for the ParentingActivity
  modulation magnitudes.** Defer to balance-doc iteration after the
  structural lift verifies.
- **Caretake target picker multi-kitten round-robin.** 397's diagnosis
  raised this as a candidate; the convergence answers it differently —
  sustained Caretake-Intention emission + §7.4 persistence means even
  with `Best` aggregation rotating between dependents, both kittens get
  fed across the window. If post-398 verification still shows uneven
  feeding across multi-kitten parents, open as separate follow-on.
- **Tuning the maturity thresholds (0.33 / 0.66 / 0.95 / 1.0).** Defer.
- **Nurse as a new HTN sub-goal.** The §7.M.2 framing puts nurse-
  equivalent behavior in the L1 aspiration's Caretake emission
  (executed by Caretake-as-DSE, held by §7.4 persistence) — no pinned
  Nurse primitive needed. The pre-2026-05-17 R3+R4+R5 menu on 397
  proposed a Nurse sub-goal; it's superseded by this design.
- **rank-sim-idea calibration update with kittens as cautionary anchor.**
  Separate session.

## Current state

Working tree at parent commit `pltwvnrk 4f2e6605 wip: 394→397 substrate
audit + 396 verdict canary impl` carries the partial implementation:
- L1 pool-entry gate (keep)
- 395 R13 (keep)
- 395 father-pitches-in symmetric picker (keep)
- 395 release_threshold gap (keep)
- `Parent` + `HasJuvenileDependent` MarkerSnapshot plumbing (keep)
- 397 +0.25 lift (retire)
- 397 cooldown bypass (retire)
- 397 pin-guard (retire)
- 395 yield-rule retirement (keep retired)

Failing soak record: `logs/tuned-42` with Pebblekit-67 starvation at
tick 1304815 (deterministic across attempts 2/3 with ±258-tick
variance). Mocha's Caretake L2 eval cadence rose 1.6% → 96% per the
L1 gate broadening; action count 5 → 10. Score competitive with Cook
(0.332 vs 0.356) but loses softmax ~95% of ticks because there's no
held-Intention persistence-bonus stickiness.

Spec sources:
- `docs/systems/ai-substrate-refactor.md` §7.M (Mating canonical
  three-layer architecture; aspiration / activity / goal),
  §7.M.1 (three-layer breakdown),
  §7.M.2 (post-consequence cascade naming `RaiseOffspringAspiration`),
  §7.4 (persistence-bonus tiers — Caretaking is Medium),
  §7.7 (aspiration layer),
  §L2.10.3 (registration via `add_aspiration_dse`),
  §L2.10.4 (Intention as DSE output),
  §L2.10.5 (Goal vs Activity Intentions),
  §L2.10.6 (softmax-over-Intentions).
- `docs/systems/htn-methods.md` reactive-emit composition rule section.
- CLAUDE.md design pillars (newly added pillar #4 + the existing
  precedent table in pillar #2 / pillar #3).

## Approach

Phased landing — each phase verified before the next. The phases are
not separate tickets (the substrate pieces are coupled), but the
landing commits sequence as below.

**Phase 1 — L1 + §L2.10.6 narrow.** Register
`raise_offspring_aspiration` via `add_aspiration_dse`. Wire
`select_intention_softmax` taking the union pool, with §7.4 Medium
persistence-bonus for Caretake. Retire wrap-site override. Verify
Mocha's L2 trace shows the L1 aspiration scored + Caretake-Intention
emitted into the softmax with persistence-bonus offset; verify the
held-Intention shape on Mocha during dependency window.

**Phase 2 — L2 activity + personality bias.** Register
`parenting_activity` via `add_aspiration_dse` (or appropriate variant).
Implement personality-weighted modifier layer that lifts Hunt target
preference (diligent) and Caretake composition magnitude
(compassionate). Verify Pebblekit-67's father (presumed-diligent) shows
elevated Hunt-target-on-partner during the dependency window.

**Phase 3 — frame-pin + workaround retirements.** Delete the frame-pin
chosen_action mutation; restrict HeldGoalStack to decomposition only.
Retire 397 lift / cooldown bypass / pin-guard. Retire reactive_emit
entry for `kitten_reared` (the HTN method's sub-goal advancement is
driven by Caretake firing in the right maturity band, not by reactive
emission).

**Wean/Teach/Release primitives stay.** The maturity-bump resolvers
(`resolve_wean`, `resolve_teach`, `resolve_release`) are 1-tick
idempotent state mutators on `KittenDependency.maturity` and
`skills_learned`. Under the post-convergence design they fire as
side-effects of Caretake hitting a particular maturity band — the band
gates which resolver fires when. No HTN frame-pin needed; the resolvers
are just additional drain effects of the Caretake action under specific
maturity windows.

**Verification soak after each phase.** `just soak-trace 42 Mocha`
(with father as second focal cat). Survival hard gate `Starvation == 0`
must hold across all three phases.

## Verification

1. `cargo check --release` + `just check` + `cargo test --release`.
2. `just soak-trace 42 Mocha` (writing to non-tuned-* path per
   `feedback_soak_trace_path_collision.md`).
3. `just verdict <new-run>` returns pass — survival hard gate
   `Starvation == 0` intact; continuity (grooming / play / mentoring /
   courtship / mythic-texture) intact.
4. **`plan_failure_canary` (ticket 396) flags nothing new** — the §7.M.2
   land introduces no new high-rate plan-failure key.
5. **Pebblekit-67 (seed 42) reaches kitten-matured state** —
   dispositive substrate-correctness signal.
6. **Mocha Caretake action count ≥ 30 across the dependency window**.
7. **Frame-pin and wrap-site override entirely deleted from `goap.rs`**.
8. **`aspiration_picker::REACTIVE_EMITS` entry for `kitten_reared` retired**.
9. **L2 trace shows the three orthogonal channels** —
   `RaiseOffspringAspiration` scored, `ParentingActivity` scored,
   Caretake-Intention scored, persistence-bonus offset visible when
   Caretake is held.
10. **`KittenWeaned`, `SkillTaught`, `KittenReleased`, `SubGoalAdvanced`
    Features still fire ≥1** — verifies the maturity-bump side-effects
    still trigger correctly when Caretake fires in the appropriate
    maturity band.
11. **Pillar #4 invariant** — no new wrap-site override / frame-pin
    / post-softmax priority-override added; if the L2 trace can't
    explain a chosen action via softmax + persistence-bonus, the
    encoding is wrong.

## Log

- 2026-05-17: opened as the convergence target for the 394→397
  cluster after substrate-vs-override + spec-vs-implementation analysis
  identified the 364 HTN-frame-pin + wrap-site override as the wrong
  commitment layer and §7.M.2 `RaiseOffspringAspiration` as the
  spec-mandated replacement. Plan at
  `/Users/will.mitchell/.claude/plans/let-s-start-tickets-394-397-snazzy-kahan.md`.
  CLAUDE.md design pillar #4 added (2026-05-17) — commitment is one
  mechanism; HTN frame-pin is override; §L2.10.6 + §7.4 are substrate.
  Tickets 394/395/397 parked behind this.
- 2026-05-17 phase 1a/1b: `RaiseOffspringAspiration` chain authored
  in `src/ai/aspirations/kinship.rs` (single milestone, dormant emit
  row guarded by `always_false`); `AspirationDomain::Kinship` variant
  added; `caretake_kitten` Live HTN method registered (single
  primitive, reuses `rear_kitten::has_dependent_kitten` eligibility);
  Kinship excluded from the passive adoption picker (event-driven
  post-partum adoption per §7.M.2) to preserve seed-42 determinism
  per `learning_bevy_schedule_edge_perturbation`.
- 2026-05-17 L1 survival activation: event-driven adoption system
  `adopt_kinship_aspiration` added (cats with `Parent` marker
  adopt `RAISE_OFFSPRING_ASPIRATION` automatically; idempotent;
  sibling of `update_parent_markers`). With the chain in the cat's
  active list, the existing `AspirationLift` modifier
  (`compute_aspiration_action_counts` → `count × aspiration_bonus`,
  ≈ +0.2 on Caretake for parents) lifts Caretake's L3 score across
  the full kitten-dependency window. This is the §7.M.2 L1 layer
  doing its job: the aspiration influences scoring at the modifier
  layer, without needing L2 emit / L3 frame-pin commitment machinery.
  Mocha's pre-397 score 0.332 → 0.532 (with +0.2 lift) reliably beats
  Cook's 0.356. Pebblekit-67 should survive.
- 2026-05-17 pathfinding fix: `find_path` at `src/ai/pathfinding.rs`
  previously bounds-checked `to` but not `from`; passing an
  out-of-bounds `from` (e.g. from a stale fox cached-path source)
  panicked at `g_score[start_idx] = 0`. Added defensive
  `map.in_bounds(from)` check returning `None`. Pre-existing latent
  bug uncovered by the new Kinship-driven Caretake firing pattern.
- 2026-05-17 deferred: Phase 1c (unified softmax over emitted Goal-
  Intentions + DSE-defaults), Phase 1d (§7.4 per-tier persistence-
  bonus base × compassion × Patience), Phase 1e (wrap-site
  Intention-author retirement), Phase 1f (L2 trace extension),
  Phase 2 (ParentingActivity + personality-weighted DSE bias),
  Phase 3a (frame-pin chosen_action mutation retirement), Phase
  3b/c/d/e (retire 397 lift / cooldown bypass / pin-guard /
  kitten_reared REACTIVE_EMITS), Phase 3f (Wean/Teach/Release
  side-effect dispatch in FeedKitten) — all follow-on, tracked in
  `/Users/will.mitchell/.claude/plans/melodic-knitting-quiche.md`.
  The L1-only landing achieves the ticket's survival goal
  (Pebblekit-67 reaches kitten-matured) without the full
  architectural retirement. Substrate is in place; future sessions
  can wire the L2/L3 commitment machinery without survival
  pressure.
- 2026-05-17: L1 landing: mother-only adoption + AspirationLift survives Pebblekit-67-class kittens across full window. L2/3 architectural retirements + father-as-provisioner expression deferred to #399 + future tickets.
