---
id: 397
title: rear_kitten arc clean completion exposes kitten survival regression
status: done
cluster: ai-substrate
initiative: [smarter-cats, htn-method-composition]
orchestration: substrate-sensitive
added: 2026-05-16
parked: 2026-05-17
blocked-by: []
supersedes: []
related-systems: [htn-methods.md, ai-substrate-refactor.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-17
---

## Why

After 395's R11 + R13 + predicate refinement + release_threshold +
reactive-emit yield rule closed the original Wean-churn + premature-
removal defects, the verification soak
(`logs/tuned-42-395-with-yield`, 2026-05-16) still surfaces 1
starvation: Pebblekit-67 dies at tick 1304815, ~36k ticks after
birth, at natural maturity ~0.45. The hard survival gate
`deaths_by_cause.Starvation == 0` fails.

**Root cause (verified via log forensic, 2026-05-17 planning pass).**
The rear_kitten arc shipped its L2 integration as a stopgap for
§L2.10.6 (softmax-over-Intentions), which the substrate-refactor
spec defers under the 060 epic. The stopgap composition:
- **Wrap-site priority-override** (`src/systems/goap.rs:2733-2790`)
  unconditionally replaces the softmax-winning Activity wrap with
  the rear_kitten emit's Goal.
- **Frame-pin** (`src/systems/goap.rs:2410-2449`) overrides
  `chosen_action` to the leaf primitive (Wean/Teach/Release)
  whenever the frame is held.
- **395's reactive-emit yield rule**
  (`src/ai/methods/rear_kitten.rs:103-109`) suppresses the entire
  arc emit when `IsParentOfHungryKitten` is set — a workaround for
  the wrap-site override's preemption of Caretake.

The verified mechanism: during Pebblekit-67's dependency window,
Mocha's L2 trace shows **Caretake evaluated only 45 times in 2888
ticks (1.6%)**. The DSE was structurally absent from her L2 pool
98.4% of ticks because the gate at `src/ai/scoring.rs:1960`
(`if ctx.hungry_kitten_urgency > 0.0`) requires either an in-range
hungry kitten OR the `IsParentOfHungryKitten` fallback marker —
neither of which fires reliably for a sated-trending dependent
kitten. When the gate did pass, Caretake's score averaged 0.332,
competitive with Cook (0.356); but Cook entered the pool 2803/2888
ticks vs Caretake's 45/2888. Result: Mocha fired Caretake 5 times,
Wean 13, Teach 2, Release 2, Cook 200 across the 36k-tick window.

The user's reframing during planning: the original HTN intent was
an **L2 score lift** — "the cat sees all the L1 stimuli but she also
knows she wants to take care of her kitten." Not a hard override that
discards softmax winners. That maps to §L2.10.6 +
§8.4 (softmax-over-Intentions with persistence-bonus on the held
Intention). This ticket lands §L2.10.6 for the rear_kitten↔Caretake
pair — narrow precedent for the 060 epic's full generalization.

## Hot context

- **Failing run:** `logs/tuned-42-395-with-yield` (2026-05-16,
  commit 6f69f0c5 + dirty 395 working tree + 395 yield rule).
  Verdict: `fail`. Footer: `deaths_by_cause.Starvation = 1`,
  `kittens_matured = 0`, `kittens_born = 2`. Decedent:
  Pebblekit-67 at tick 1304815, born 1268610 (mother Mocha).
  Two kittens born: Pebblekit-67 (1533v30, tick 1268610) and a
  second kitten (725v55, tick 1300085, location [46,17]).
- **Verified Mocha action distribution during Pebblekit-67's
  36115-tick dependency window (via `just q actions ... --cat=Mocha
  --tick-range=1268610..1304815`):** Cook 200 (55.2%) · GroomOther
  many · Forage many · Wean 13 · Caretake 5 · Teach 2 · Release 2.
- **Verified L2 eval cadence (via `just q trace ...
  --layer=L2 --tick-range=...`):** Caretake **45 evals / 2888
  ticks (1.6%)**, avg score 0.332. Cook 2803 evals (97%) avg 0.356.
  Socialize 2689 avg 0.538. Wander avg 0.578. Caretake's score is
  competitive when scored; the bug is pool entry, not score
  magnitude.
- **Comparison archive (pre-yield 395):** `logs/tuned-42-395-pre-yield`.
  Same 1 starvation, same kitten — the yield rule alone is not
  sufficient.
- **Reference healthy run:** `logs/tuned-42-attempt11` (pre-R11,
  Wean churn ×2439, **0 starvations** because the churn's
  abandon→replan boundaries incidentally gave Caretake fresh L2
  evals).
- **Baseline current.json:** `logs/tuned-42-095-phase-1a-shadow`.
- **Spec sections that own the proper composition:**
  `docs/systems/ai-substrate-refactor.md` §L2.10.4 (DSEs emit
  Intention, not Action), §L2.10.6 (softmax-over-Intentions),
  §8.2 (deliberation pipeline), §8.4 (persistence-bonus gating —
  challenger must beat `held_score + bonus`).

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | `src/components/markers.rs::IsParentOfHungryKitten` (authored by `growth.rs::update_kitten_cry_map`) | Set on parents whose dependent kittens have hunger < `kitten_cry_hunger_threshold` (0.5 satiety). Symmetric across mother + father. | `[verified-correct]` |
| L1 markers | `src/components/markers.rs::HasJuvenileDependent` (authored by `growth.rs::update_parent_markers`) | Set on parents whose dependent kittens are in the early arc window [0, teach_done_threshold) OR near-mature window [release_threshold, 1.0). Drives the rear_kitten reactive emit's primary gate. NOT currently surfaced into `MarkerSnapshot` for scoring-layer consumption. | `[verified-defect]` — substrate exists but isn't plumbed to the scoring gate |
| L2 pool-entry gate | `src/ai/scoring.rs:1960` (`if ctx.hungry_kitten_urgency > 0.0`) | Caretake enters the L2 scores vec only when the upstream `caretake_resolution.urgency > 0`. Resolution returns 0 unless a hungry kitten is in range OR the `IsParentOfHungryKitten` fallback marker is active. Pool-entry rate verified: 45/2888 ticks (1.6%) in Mocha's Pebblekit-67 window. | `[verified-defect]` — gate is too narrow; structurally drops Caretake from softmax 98% of the time when a kitten is sated-trending |
| L2 DSE — Caretake | `src/ai/dses/caretake.rs::CaretakeDse` | WeightedSum {kitten_urgency=0.45, caretake_compassion=0.30, is_parent_of_hungry_kitten=0.25, colony_food_security=0.0 dormant}. Tier 3 (Maslow Love/Belonging). Modifier lift `KittenCryCaretakeLift` is per-tile additive. | `[verified-correct]` shape; score magnitude (0.332 avg observed) is competitive with Cook (0.356) WHEN scored |
| L2 wrap-site override | `src/systems/goap.rs:2733-2790` | Unconditionally replaces the softmax-winning Activity wrap with the rear_kitten emit's Goal whenever the reactive emit fires. **This IS the §L2.10.1 anti-pattern §L2.10.6 explicitly retires.** | `[verified-defect]` — to be retired by Layer 2 |
| L2 frame-pin | `src/systems/goap.rs:2410-2449` | When `HeldGoalStack.top.sub_goal_count > 1`, overrides `chosen_action` with the frame's current leaf primitive. Routes plan template through `htn_primitive_actions`. Fires unconditionally on a held multi-step frame. | `[verified-defect]` — should fire only when the softmax-selected Intention matches the held frame's goal_label (Layer 3) |
| L2 emit-gate (395 stopgap) | `src/ai/methods/rear_kitten.rs::has_dependent_kitten` (`!IsParentOfHungryKitten`) | Yields the entire reactive emit when the marker is set. Workaround for the wrap-site override's preemption of Caretake. **Doubles Caretake firings 3 → 6** but doesn't cross survival threshold. | `[verified-defect]` — stand-in until §L2.10.6 lands per `docs/systems/htn-methods.md` reactive-emit composition rule section; to be retired by Layer 2 |
| Method registry | `src/ai/methods/rear_kitten.rs::rear_kitten()` | 3 sub-goals: Wean → Teach → Release. Spec at `htn-methods.md:694` calls for 4: nurse → wean → teach → release. Wean/Teach/Release are 1-tick maturity-bump resolvers with no nutrition effect. | `[verified-correct]` for the post-§L2.10.6 framing — "Nurse" doesn't need to be a pinned primitive once Caretake competes properly in the Intention softmax |
| L3 — Action→Disposition mapping | `src/components/disposition.rs::from_action` | `Action::Caretake → Some(DispositionKind::Caretaking)`; `Action::Wean/Teach/Release → None` (HTN-leaf-only). | `[verified-correct]` |
| Plan template — Caretaking | `src/ai/planner/actions.rs::actions_for_disposition` | Caretaking → `[TravelTo(SocialTarget), Caretake]`. Standard 2-step plan. Mutation site at `goap.rs:2924-2934` writes +0.5 to kitten's hunger satiety (entity-keyed, position-independent). | `[verified-correct]` |
| Plan template — HTN primitives | `src/ai/planner/actions.rs::htn_primitive_actions` | Panics on `Action::Caretake` — only handles Wean/Teach/Release/Vigil/GriefSit/ReleaseGrief. Under Layer 3's gate, Caretake-Intention routes through `actions_for_disposition` (not htn_primitive_actions), so the panic is structurally unreachable. | `[verified-correct]` — Layer 3 makes this gap irrelevant; no extension needed |
| Resolver — Caretake mutation | `src/steps/disposition/caretake.rs` does not exist; the +0.5 hunger pass at `goap.rs:2924-2934` is the actual feeding site | Entity-keyed mutation on the kitten's `Needs.hunger` field; runs in the post-loop drain. | `[verified-correct]` |
| Maturity bumps | `src/steps/disposition/{wean,teach,release}.rs` | `resolve_wean` → `max(maturity, weaned_threshold)`; `resolve_teach` → `max(maturity, teach_done_threshold)` + `skills_learned += 1`; `resolve_release` → conditional removal of `KittenDependency` (post-395 R13, only at maturity 1.0). All 1-tick idempotent bumps; no nutrition effect. | `[verified-correct]` |

## Fix candidates

**Recommended structural direction — three-layer §L2.10.6 landing:**

- **L1 (extend pool-entry gate)** — surface `HasJuvenileDependent`
  into `MarkerSnapshot` from `evaluate_and_plan` in `src/systems/goap.rs`,
  then broaden the Caretake gate at `src/ai/scoring.rs:1960` from
  `if ctx.hungry_kitten_urgency > 0.0` to also pass when
  `inputs.markers.has(HasJuvenileDependent::KEY, inputs.cat)`. Caretake
  enters the L2 pool every tick the cat structurally has a juvenile
  dependent. When no hungry kitten is in range, the score is just
  compassion (~0.15) — Caretake doesn't outcompete other DSEs.
  When the kitten IS hungry, the score rises via the existing
  `kitten_urgency` axis (0.45 weight × Quadratic-amplified hunger
  deficit). Verification: focal trace shows Caretake eval count near
  total-tick count (vs 45/2888 now). No behavior regression expected;
  Layer 2 composes on top.
- **L2 (retire wrap-site override; implement §L2.10.6)** — retire
  `src/systems/goap.rs:2733-2790`. Generalize
  `src/ai/scoring.rs::select_disposition_softmax` (1194-1231) to
  `select_intention_softmax`: candidate pool =
  `{DSE-Activity-default scores} ∪ {emitted-Goal Intentions from
  REACTIVE_EMITS}`. Apply persistence-bonus per §8.4 to the held
  Intention's score before the preemption comparison. **395's
  reactive-emit yield rule** (`src/ai/methods/rear_kitten.rs:103-109`)
  retires in the same commit — superseded by spec composition.
  Verification: Mocha's Caretake action count rises from 5 to ≥30
  across a Pebblekit-window soak; survival gate `Starvation == 0`
  holds.
- **L3 (gate frame-pin on selected Intention)** — change the pin at
  `src/systems/goap.rs:2410-2449` to fire only when the softmax-
  selected Intention matches the held frame's `goal_label`. When the
  cat has a rear_kitten frame held AND softmax picks
  Caretake-as-`kitten_fed`, the pin does NOT fire — Caretake's
  normal plan template runs. The held frame stays on the stack
  (per §8.4's "never exclude the incumbent"); next tick, softmax
  samples again, and if rear_kitten wins, the pin resumes walking
  sub-goals. Verification: Wean/Teach/Release firing count rises
  beyond 17/window when softmax picks rear_kitten on co-located
  ticks; substrate Features (`KittenWeaned`, `SkillTaught`,
  `KittenReleased`, `SubGoalAdvanced`) all fire ≥1.

**Considered, not chosen** — the pre-2026-05-17 R3/R4/R5/R6 menu
proposed adding a Nurse sub-goal pinned to `Action::Caretake` + a
re-banded early window + per-band yield. The user reframed the
problem ("L2 lift, not pin") which mapped cleanly to §L2.10.6.
R3+R4+R5 would have shipped another pin (another stopgap on the
same deferred boundary); the three-layer fix above lands §L2.10.6
itself for the rear_kitten↔Caretake pair. Per the
[feedback-deferred-spec-patch-stack](file:../../../.claude/projects/-Users-will-mitchell-clowder/memory/feedback_deferred_spec_patch_stack.md)
lesson, the third+ patch on a deferred-spec area should pull the
boundary forward.

**Parameter-level options** (deferred — applicable as balance-doc
follow-on once L1-L3 land, if survival cadence is still marginal):
- R1 — boost `kitten_urgency` curve steepness or shift midpoint
  in `src/ai/dses/caretake.rs`.
- R2 — boost `KittenCryCaretakeLift` magnitude in
  `src/ai/modifier.rs`.

## Recommended direction

**L1 + L2 + L3 (combined, phased landing).** Each layer verified
before the next. See `Fix candidates` above for per-layer scope.
This is the first concrete §L2.10.6 land — narrow precedent for the
060 epic's full generalization across all REACTIVE_EMITS.

The full plan document with file-path-level detail is at
`/Users/will.mitchell/.claude/plans/lets-get-ready-to-quiet-whistle.md`.

## Out of scope

- **§L2.10.6 generalization to all REACTIVE_EMITS.** Layer 2 lands
  the spec composition narrowly for `rear_kitten ↔ Caretake`. Full
  generalization across every entry in
  `src/ai/aspiration_picker::REACTIVE_EMITS` stays under the 060
  epic. The narrow land sets the precedent.
- **`mourn_at_grave` frame-pin behavior.** Layer 3's gate change
  only touches the kitten-arc case if mourn_at_grave is not
  actively emitting in the test soak. If it is, Layer 3 needs to
  handle the general "selected-Intention vs held-Intention"
  composition for both arcs; verify whether mourn_at_grave is
  Live in `populate_method_registry` before implementation.
- **Nurse sub-goal addition.** The htn-methods.md:694 spec calls
  for "nurse → wean → teach → release" but with the §L2.10.6
  composition in place, Caretake-as-Intention competes naturally
  in the L2 softmax pool — there's no need for a pinned Nurse
  primitive. The 4-stage spec is satisfied by the 3 HTN primitives
  + Caretake-as-DSE-pool (Caretake plays the Nurse role
  structurally, not as a pinned sub-goal).
- **`teach_curriculum_size > 1`.** Pre-existing scope limit; out
  of 397.
- **Tuning persistence-bonus magnitude.** Layer 2 ships with a
  literal-derived default (~0.10, derived from the Cook−Caretake
  score gap); a balance-doc iteration after L1-L3 land tunes the
  bonus based on observed Caretake cadence + survival outcomes.

## Verification

1. `cargo check --release` + `just check` + `cargo test --release`.
2. `just soak-trace 42 Mocha 900` (fresh path,
   `feedback_soak_trace_path_collision.md`).
3. `just verdict <new-run>` passes — **survival hard gate intact:
   `Starvation == 0`**; courtship / grooming / mentoring
   continuity intact.
4. **`kittens_matured > 0`** for Pebblekit-67 or equivalent —
   dispositive signal that Caretake fired enough times across the
   dependency window. Birth tick 1268610 + natural-growth window
   80k ticks → ETA tick 1348610. Run ends ~1322641; partial-
   maturity-progress (not full 1.0) is acceptable evidence if
   trajectory is positive.
5. **`KittenWeaned`, `SkillTaught`, `KittenReleased`,
   `SubGoalAdvanced` Features fire ≥1.**
6. **Mocha Caretake action count ≥ 30** during the dependency
   window (vs 5 currently). Sanity-check via
   `just q actions <run> --cat=Mocha --tick-range=<birth>..<end>`.
7. **Caretake L2 eval count near total-tick count for Mocha**
   (vs 45/2888 now). Verify via
   `just q trace <run> Mocha --layer=L2 --top-dses=25`.
8. **§L2.10.6 visible in trace.** Both Caretake-Intention and
   rear_kitten-Intention scored per tick, with persistence-bonus
   offset applied to the held one. Structural proof the spec
   composition lands cleanly, not just behavioral verdict.

## Open follow-ons that land with 397

- **mourn_at_grave yield-marker / §L2.10.6-equivalent.** If
  mourn_at_grave is Live and emits during the test soak, Layer
  3's gate change applies to it equally; document in the landing
  commit. If dormant, open as a follow-on for when its emission
  path lands.
- **Balance-doc iteration on persistence-bonus magnitude** — open
  separately once L1-L3 land and the per-tick Caretake-vs-rear_kitten
  competition is observable in the focal trace.

## Log

- 2026-05-16: opened as follow-on from 395's verification soak
  (`logs/tuned-42-395-with-yield`, 1 starvation despite
  R11+R13+predicate+yield rule). Original framing: clean arc
  completion eliminated the pre-R11 Wean-churn's incidental
  Caretake rescue; the yield rule (3 → 6 Caretake firings)
  insufficient. Drafted R3+R4+R5 (Nurse sub-goal + re-band +
  per-band yield) as recommended structural direction. 395
  parked behind 397.
- 2026-05-17: **direction pivot to §L2.10.6.** Planning pass
  surfaced via log forensic that Caretake was L2-evaluated only
  45/2888 ticks (1.6%) during Pebblekit-67's window — the bug is
  pool-entry cadence, not score magnitude (Caretake's avg score
  0.332 when scored is competitive with Cook 0.356). User
  reframed: "the original HTN intent was an L2 lift, not a pin —
  the cat sees the L1 stimuli but she also knows she wants to
  take care of her kitten." That maps to §L2.10.6 +
  §8.4 (softmax-over-Intentions with persistence-bonus on the
  held Intention). Recommended direction rewritten as three-
  layer §L2.10.6 landing: Layer 1 (Caretake enters L2 pool every
  tick a dependent kitten exists) + Layer 2 (retire wrap-site
  override + 395 yield rule; implement softmax over Intentions
  for rear_kitten ↔ Caretake) + Layer 3 (frame-pin gated on
  selected-Intention identity, not held-frame existence).
  R3/R4/R5/R6 moved to "considered, not chosen" — they would
  have added another stopgap on the same deferred boundary.
  Pattern matches `feedback_deferred_spec_patch_stack` — on the
  third+ patch of a deferred-spec area, pull the boundary forward.
  Full plan at
  `/Users/will.mitchell/.claude/plans/lets-get-ready-to-quiet-whistle.md`.
- 2026-05-17 (implementation): Layer 1 + Layer 2 (narrow) +
  Layer 3 (narrow) + cooldown bypass shipped. Files:
  `src/systems/goap.rs:466,1719-1729,2444-2491` (Parent +
  HasJuvenileDependent into MarkerSnapshot; pin-Caretake-preempts
  guard zeros `frame_pinned_primitive` so the plan template
  routes through `actions_for_disposition` correctly),
  `src/ai/scoring.rs:1973-2000` (Caretake pool gate on Parent
  marker + +0.25 lift on HasJuvenileDependent),
  `src/ai/modifier.rs:2671-2710` (DispositionFailureCooldown
  bypass for Caretake when HasJuvenileDependent),
  `src/ai/methods/rear_kitten.rs:44,103-109` (395 yield rule
  retired). New constant
  `ScoringConstants::rear_kitten_caretake_lift = 0.25`. All
  2272 lib tests pass.
- 2026-05-17 (verification — partial pass): four soaks run
  (`logs/tuned-42-397-attempt1-panic`,
  `logs/tuned-42-397-attempt2-no-lift`,
  `logs/tuned-42-397-attempt3-lift-no-cooldown-bypass`, latest
  `logs/tuned-42`). Survival hard gate STILL fails — **same
  Pebblekit-67 death at tick 1304815 in every soak**, RNG-
  deterministic across all four implementations. Mocha's Caretake
  L2 eval count rose from 45/2888 (1.6%) baseline to 2532/2645
  (96%) — Layer 1 working. Scenario harness shows Caretake winning
  L3 softmax 71.68% with the cooldown bypass — Layer 2 + bypass
  working. Mocha Caretake action count rose from 5 (pre-fix) to
  10 (post-full-stack) — modest improvement, not survival-
  decisive.
- 2026-05-17 (diagnosis follow-on): the deterministic same-tick
  death across structural variants strongly suggests Pebblekit-67's
  starvation is NOT primarily an L2-composition / commitment-
  substrate issue. The verified facts: Mocha has 2 dependents
  during the late dependency window (Pebblekit-67 born tick
  1268610 + second kitten born tick 1300114, only 4701 ticks
  before Pebblekit's death). The Caretake target picker uses
  `Best` aggregation — picks ONE candidate per evaluation. When
  Mocha has two hungry dependents, her Caretake firings split
  between them; the picker may consistently favor the newer /
  more-isolated kitten. **The structural L2 work in this ticket
  is sound** (Caretake pool entry, lift, cooldown bypass, pin
  guard all verified). Pebblekit-67's specific survival appears
  blocked by a different cluster — multi-kitten Caretake target
  selection — which is out-of-scope for the §L2.10.6 substrate
  precedent. Open follow-on ticket recommended: "Caretake target
  picker: per-tick round-robin or hunger-floor-prioritization
  across multiple dependents."
- 2026-05-17: **Parked, blocked on 398** (`§7.M.2
  RaiseOffspringAspiration — kitten-rearing as nested-Intention
  aspiration`). Session reframe identified that 397's narrow
  §L2.10.6 land (pool-gate + lift + cooldown bypass + pin-guard)
  was itself another patch on the deferred-spec boundary rather
  than the spec-mandated convergence per `docs/systems/ai-substrate-refactor.md`
  §7.M.2 (post-mating cascade names a `RaiseOffspringAspiration`
  that emits Caretake-Intentions into the unified softmax pool
  with §7.4 persistence). CLAUDE.md design pillar #4
  (2026-05-17) — *"commitment is one mechanism, not two."*
  Substrate pieces from 397 that survive the convergence: L1
  pool-entry gate broadening (Caretake enters L2 pool every tick
  the `Parent` marker is set — keep), `Parent` +
  `HasJuvenileDependent` plumbed into `MarkerSnapshot` (keep).
  Pieces that retire with the override: +0.25 lift
  (`rear_kitten_caretake_lift`), `DispositionFailureCooldown`
  bypass for Caretake, pin-Caretake-preempts guard — all
  workarounds for the frame-pin and wrap-site override. The
  multi-kitten picker recommendation from the prior log entry is
  also superseded: with sustained Caretake-Intention emission +
  §7.4 persistence under 398, even `Best` aggregation across
  multiple dependents produces adequate cross-kitten feeding
  cadence. Plan at
  `/Users/will.mitchell/.claude/plans/let-s-start-tickets-394-397-snazzy-kahan.md`
  (local).
- 2026-05-17: Superseded by 398 — L1 broadening at scoring.rs:1973-2000 (Parent || hungry_kitten_urgency entry-gate) ships as kept-substrate. The +0.25 lift, DispositionFailureCooldown bypass, and pin-Caretake-preempts guard remain in code (load-bearing for the current frame-pin-based dispatch); their architectural retirement is part of the deferred L2/L3 follow-on tracked via #399 and the §L2.10.6 unified-softmax phase. Pebblekit-67-class kittens survive via 398's AspirationLift mechanism rather than via the per-tick +0.25 compensation.
