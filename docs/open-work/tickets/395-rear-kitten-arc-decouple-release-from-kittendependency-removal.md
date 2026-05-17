---
id: 395
title: rear_kitten arc — decouple Release from KittenDependency removal
status: parked
cluster: ai-substrate
initiative: [smarter-cats, htn-method-composition]
orchestration: substrate-sensitive
added: 2026-05-16
parked: 2026-05-17
blocked-by: [398]
supersedes: []
related-systems: [htn-methods.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 394's Phase A investigation surfaced two coupled defects in
the 364 rear_kitten arc:

1. **Wean failure churn** (2439 events per soak): new frames always
   start at `sub_goal_index = 0` (Wean), but the kitten may be past
   the Wean band by the time the picker runs — every dispatch then
   fails, abandons, re-emits, loops. Documented in 394.
2. **Premature `KittenDependency` removal**: `resolve_release`
   removes `KittenDependency` from the kitten when the kitten's
   maturity reaches the Release band threshold (default 0.66) — but
   `tick_kitten_growth` (the canonical maturation system) only
   removes `KittenDependency` at maturity 1.0. A kitten released by
   the arc at maturity 0.66 falls out of Caretake's target pool
   (which iterates only entities `With<KittenDependency>`), so no
   adult feeds them. The kitten is conceptually "independent" but
   physiologically unable to self-feed → starves.

394 attempted a one-line fix for (1) — R11, the dispatch advances
on band-mismatch — and ran the verification soak: Wean failures
dropped 2439 → 9 (R11 works) but **two starvations appeared**
(Pebblekit-67 + Pebblekit-34). The mechanism: R11 made the arc
complete faster (Wean→Teach→Release within ~600 ticks of birth);
Release removed `KittenDependency`; the kitten could no longer be
Caretaken; hunger dropped; starved. R11 was reverted at the goap.rs
level — substrate-stability pillar treats `Starvation > 0` as
non-negotiable.

This ticket fixes both defects together. R11 alone is necessary but
insufficient.

## Hot context

- **Verified-defect run:** `logs/tuned-42-394-r11` (R11-only
  attempt). 2 starvations: Pebblekit-67 died tick 1280417 (lived
  11,717 ticks); Pebblekit-34 died tick 1312071. Both via
  premature KittenDependency removal — Pebblekit-67's PlanCreated
  event at tick 1269232 (only 622 ticks after birth) confirms his
  `KittenDependency` was removed by then (kittens are excluded from
  `evaluate_and_plan` via `Without<KittenDependency>`).
- **Second verification run:** `logs/tuned-42-395` (R11 + R13 +
  predicate refinement + release_threshold + father-pitches-in).
  Premature-removal defect closed (no Release fires until maturity
  ≥ 0.95 per R13's drain gate). New starvation surfaced:
  Pebblekit-67 still died at maturity ~0.45 — but via a *different*
  mechanism. Cause: 364's HTN dispatch closure pins
  `chosen_action` (`goap.rs:2410-2449`) to the in-flight
  `rear_kitten` frame's leaf primitive (Wean/Teach/Release),
  preempting the L3 softmax. Even when Caretake won L2 softmax for
  the hungry Pebblekit-67, both the L2 wrap-site emission override
  (`goap.rs:2733-2790`) and the pin discarded that win, forcing
  Mocha to attempt Wean (which drains maturity, doesn't feed) or
  R11-Advance through the bands and pop. Pre-R11, the 2439 Wean
  failures/soak incidentally fed Caretake via abandon-reevaluate
  plan boundaries; R11 + R13 + predicate refinement closed the
  churn AND fixed the premature-removal defect but exposed the
  pinning starvation pathway.
- **Baseline reference:** `logs/tuned-42-d633bcc5` (pre-364).
- **Pre-R11 archive:** `logs/tuned-42-attempt11` (no R11, has 2439
  Wean failures but 0 deaths — Wean churn keeps Mocha from
  completing the arc fast enough to trigger the Release defect).
- **`tick_kitten_growth`** at `src/systems/growth.rs:41`:
  `dep.maturity = (dep.maturity + rate).min(1.0)` with rate =
  `1.0 / (4.0 * ticks_per_season)`. KittenDependency removed at
  maturity ≥ 1.0 (line ~48). Full natural lifecycle: 80,000 ticks
  at default `ticks_per_season = 20000`.
- **`resolve_release` + drain** at
  `src/systems/goap.rs:7209-7222` + drain site:
  `commands.entity(target).remove::<KittenDependency>()`. Fires
  when picker matches (kitten in Release band, maturity ≥ 0.66
  pre-395; maturity ≥ 0.95 post-395 R13).
- **Reactive emit + L2 wrap-site override.**
  `aspiration_picker::REACTIVE_EMITS[0]` (`kitten_reared`) fires
  per tick whenever `has_dependent_kitten(world, cat)` returns
  true. The L2 wrap site at `goap.rs:2733-2790` then unilaterally
  replaces the softmax-winning Activity-default with
  `Intention::Goal { kitten_reared }` (priority-override; the
  formal §L2.10.6 softmax-over-Intentions across `{DSE-Activity-
  default} ∪ {emitted-Goals}` is not implemented). The L2 author
  pushes the `rear_kitten` frame; subsequent ticks see the pin
  override `chosen_action` to the frame's current leaf.
- **`IsParentOfHungryKitten` substrate (ticket 161).** Authored
  per tick by `update_kitten_cry_map` (`growth.rs:152-200`) on
  parents whose dependent kittens have `hunger <
  kitten_cry_hunger_threshold`. Symmetric across both parents.
  Already consumed by `caretake_target::resolve_caretake_target`
  as the own-kitten-anywhere fallback signal. This is the
  pre-existing substrate that the 395 yield rule consumes — no
  new marker required.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Natural maturation | `src/systems/growth.rs::tick_kitten_growth` | Advances maturity by `1.0/(4*ticks_per_season)` per tick. Removes `KittenDependency` at maturity ≥ 1.0. Full lifecycle = 80,000 ticks. | `[verified-correct]` |
| Arc maturity bumps | `src/steps/disposition/{wean,teach}.rs` resolvers | `resolve_wean` bumps maturity to `max(current, 0.33)`; `resolve_teach` bumps to `max(current, 0.66)`. Effect: speeds up the kitten's perceived maturity by ~50k ticks. | `[verified-correct]` (intended per 364 spec) |
| Arc completion | `src/steps/disposition/release.rs` + drain | `resolve_release` returns witness; drain removes `KittenDependency`. Pre-395 fires at m ≥ 0.66; post-R13 fires at m ≥ 1.0 (natural maturity). | `[verified-correct]` (R13 closed the premature-removal defect) |
| Caretake target pool | `src/systems/goap.rs:1362 kitten_snapshot` | Built from `kitten_query` which requires `KittenDependency`. Kittens without `KittenDependency` are absent from the pool. | `[verified-correct]` |
| Wean churn (loop) | `dispatch_htn_kitten_primitive` + `htn_abandon_or_pop` | Picker fails when kitten is past band → Fail → htn_abandon_or_pop pops frame → reactive emit re-creates frame at sub_goal_index=0 → loops. | `[verified-correct]` (R11 + R13 + predicate refinement closed it; logs/tuned-42-395 shows Wean failures = 0) |
| Reactive emit predicate | `aspiration_picker::REACTIVE_EMITS[0].applicable_when` → `has_dependent_kitten` (`ai/methods/rear_kitten.rs:75-80`) | Returns true whenever cat has `Parent + HasJuvenileDependent`. No yield gate against acute caretaking urgency. | `[verified-defect]` — emits every tick, including when own kitten is starving |
| L2 wrap-site override | `goap.rs:2733-2790` | Priority-override: any aspiration emission UNCONDITIONALLY replaces the softmax-winning Activity wrap with the emitted Goal. The formal §L2.10.6 softmax-over-Intentions across `{DSE-Activity-default} ∪ {emitted-Goals}` is deferred. | `[verified-defect, deferred]` — won't be fixed in 395 (out of scope); 395 routes around it via the reactive-emit yield rule below |
| Frame-pin override | `goap.rs:2410-2449` | When `HeldGoalStack.top.sub_goal_count > 1`, overrides `chosen_action` with the frame's current leaf primitive. Routes plan template through `htn_primitive_actions` (single-action Pattern-B step), bypassing the disposition's full action catalog. | `[verified-correct]` for multi-step methods generally; load-bearing for advancing Wean → Teach → Release across ticks. The defect is upstream (emit unconditional), not in the pin itself |

## Fix candidates

**Two layers of fix are required.** Either alone is insufficient.

**Layer 1: substrate churn** — pick one:

- R11 (**extend** dispatch — Advance on band-mismatch, Fail on
  kitten-gone). Already drafted in 394; cleanest. When picker
  returns None: check `has_any_dependent` → Advance (no witness);
  else Fail.
- R10 (**extend** frame creation — initial sub_goal_index reflects
  kitten's current maturity band). Requires plumbing method-specific
  kitten state to `GoalFrame::new`. Larger blast radius.

**Layer 2: premature release** — pick one (or combination):

- R12 (**rebind** — Release is symbolic, doesn't remove
  KittenDependency). `resolve_release` witnesses `Feature::KittenReleased`
  but the drain DOES NOT remove `KittenDependency`. Removal stays
  in `tick_kitten_growth` at maturity 1.0. Requires also gating the
  reactive emit predicate on "any dependent kitten with maturity <
  release threshold" so the arc doesn't churn after completion.
- R13 (**extend** Release with maturity check). `resolve_release`
  conditional: remove `KittenDependency` only if `maturity ≥ 1.0`
  (the natural threshold). Otherwise witness without removal.
  Simpler than R12 — no predicate refinement needed (the predicate
  flips false only when `tick_kitten_growth` eventually removes
  the dependency at maturity 1.0).
- R14 (**rebind** Caretake to include "recently-released" kittens).
  Caretake's target pool also includes kittens with `RecentlyReleased`
  marker (authored when `KittenDependency` is removed via Release).
  Tracks the kitten through a "post-release immature" window.
  Larger change — requires a new marker + writers.

## Recommended direction

**R11 + R13 + predicate refinement + release_threshold + father-
pitches-in + reactive-emit yield (combined fix).**

The first five address the original Wean-churn / premature-removal
defects (in this commit's `src/` working tree already; see "Hot
context" for the verification-soak surfacing the residual
starvation). The sixth — the reactive-emit yield — closes the
pinning-induced starvation pathway that `logs/tuned-42-395`
surfaced.

R12 is the substrate-cleanest option for the original premature-
removal defect but requires modifying the predicate too; R13 is
smaller and equally protective. R14 is the largest change and
doesn't address the underlying coarseness of the maturity model.

**Reactive-emit yield (composition rule for HTN methods).** Extend
`has_dependent_kitten` (`src/ai/methods/rear_kitten.rs`) with
`!IsParentOfHungryKitten`. The substrate is already authored —
ticket 161's `update_kitten_cry_map` stamps the marker on parents
whose dependent kittens have `hunger < kitten_cry_hunger_threshold`,
already consumed by `caretake_target` as the own-kitten-anywhere
fallback signal. Consuming it here adds no new substrate.

Mechanism: when set, the reactive emit returns false → no
`Intention::Goal { kitten_reared }` emission → the L2 wrap site's
priority override doesn't fire → no rear_kitten frame is pushed →
the L2 softmax winner (Caretake, with high score because the
kitten cry urgency is what set the marker in the first place)
executes as authored. The frame from prior ticks survives via
`resolve_goap_plans`'s `PreserveStackOnly` path; once the kitten
is fed, the marker clears and the arc resumes mid-stride on the
next tick.

The rule (per-method yield marker consumed at `applicable_when`)
is documented in `docs/systems/htn-methods.md` as the
**reactive-emit composition rule** — the substrate-clean stand-in
until §L2.10.6's formal softmax-over-Intentions lands. Sets the
precedent for `mourn_at_grave` to declare its own yield marker
(likely `HasAcuteSafetyNeed` / `HasAcuteHungerNeed`) when its
`Mourning` insertion path lands.

## Out of scope

- Tuning the maturity threshold defaults (0.33 / 0.66 / 0.95 / 1.0).
  Defer to balance-doc iteration after R11 + R13 + reactive-emit
  yield land.
- Per-kitten frame tracking (currently one frame per parent). Out
  of scope; the reactive-emit-after-completion churn is acceptable
  if it's all Advance-only (no plan failures).
- The plan-failure canary in `verdict` (separate follow-on, tracked
  on 394 originally — open as 396).
- **Adding a Nurse sub-goal to the rear_kitten method.** The spec
  at `htn-methods.md:595` describes the arc as "nurse → wean →
  teach → release". With the reactive-emit yield rule in place,
  the Nurse stage is implicit in the L2 DSE pool — when the kitten
  is hungry, the parent yields to Caretake (the Nurse-equivalent
  action); when the kitten is not hungry, the parent does its own
  per-tick DSEs (Hunt, Forage, Groom, Mate). Adding Nurse as a
  pinned frame sub-goal would pin `chosen_action = Caretake`
  across the entire nurse window, suppressing those other
  behaviors — cats don't nurse continuously. The yield rule
  achieves Nurse-semantics for a fraction of the structural cost.
- **§L2.10.6 formal softmax-over-Intentions.** The proper
  composition path is `{DSE-Activity-default} ∪ {emitted-Goals}`
  going through a unified softmax with shared scoring units. That
  retires both the priority-override at the wrap site and the
  per-method yield rule. Substantial structural change; tracked
  separately under the 060 epic.
- **Yielding at the frame-pin (`goap.rs:2410`).** Yielding there
  is one layer too low — the L2 wrap has already pushed the frame
  by then, so un-pinning produces inconsistent state. The yield
  belongs at the emit predicate, where it short-circuits both the
  wrap-site override and the pin in one stroke. Documented as an
  anti-pattern in the htn-methods.md composition-rule section.

## Open follow-ons that land with 395 (per "antipattern migration
follow-ups" discipline in CLAUDE.md)

- **`mourn_at_grave` yield-marker authoring.** When the §7.7.b
  grief-event-emission debt ships (i.e., the Mourning Component is
  inserted on colony-mate death), `mourn_at_grave` becomes
  Live-in-practice. Its `applicable_when` predicate must declare
  yield markers per the same rule — likely `HasAcuteSafetyNeed`
  (predator-near-self) and/or `HasAcuteHungerNeed` (starvation
  imminent). The composition-rule section's precedent table tracks
  this; open the follow-on as part of mourn's wiring ticket, not
  ahead of it.

## Verification

1. `cargo check --release` + `just check` + `cargo test --release`.
2. Fresh `just soak-trace 42 Mocha` (writing to a non-tuned-* path
   per `feedback_soak_trace_path_collision.md`).
3. `just verdict <new-run>` passes — **survival hard gate intact:
   `Starvation == 0`**.
4. **Wean failure count ≤ 100** in `plan_failures_by_reason` (R11
   eliminates the loop; some Wean-Advance traversals may still
   surface as bookkeeping events).
5. **PlanReplanned for Mocha ≤ 50** (was 1621 in attempt11; should
   be near baseline 24).
6. **`kittens_matured > 0`** (R13 ensures `KittenDependency` removal
   only at natural maturity 1.0, so kittens born early enough in the
   soak should still reach maturity).
7. After verification, flip the four Features to `true` in
   `expected_to_fire_per_soak()`: `KittenWeaned`, `SkillTaught`,
   `KittenReleased`, `SubGoalAdvanced`.

## Log

- 2026-05-16: opened as follow-on from 394 Phase A. R11 was
  attempted in 394 and reverted because it exposed the
  Release-removes-KittenDependency-prematurely defect (2 starvations
  in the R11 verification soak). 395 combines R11 + R13 to address
  both layers together. 394 is parked blocked on this.
- 2026-05-16 (composition-rule expansion): first verification soak
  on the R11 + R13 + predicate refinement + release_threshold +
  father-pitches-in code (`logs/tuned-42-395-pre-yield`) still
  showed Pebblekit-67 starving at maturity ~0.45. Layer-walk
  identified the residual mechanism: 364's frame-pin
  (`goap.rs:2410-2449`) unilaterally overrides `chosen_action`
  whenever a multi-step HeldGoalStack frame is in flight, AND
  the L2 wrap-site override (`goap.rs:2733-2790`) unilaterally
  replaces softmax-winning Activity wraps with emitted Goals.
  The two overrides together discard the L2 softmax winner
  whenever the rear_kitten reactive emit fires — including when
  Caretake's score for a hungry dependent kitten is high.
  Scope expanded to add the reactive-emit yield rule (per-method
  `applicable_when` consults pre-authored substrate markers
  naming acute domain urgency). Implementation: one-line predicate
  extension consuming the existing `IsParentOfHungryKitten`
  substrate (ticket 161); no new marker / constant / author /
  tests required. Composition rule documented in
  `docs/systems/htn-methods.md` as the reactive-emit yield rule
  precedent that `mourn_at_grave` will follow when its
  `Mourning` insertion path lands.
- 2026-05-16 (yield-rule verification + park): second verification
  soak with the yield rule added (`logs/tuned-42`) shows 1
  starvation, same victim (Pebblekit-67) at tick 1304815,
  maturity ~0.45 — same as pre-yield. Diagnosis via
  `/diagnose-collapse`: yield rule DID double Mocha's Caretake
  firings (3 → 6 in 36k ticks of dependency) but didn't cross
  the survival threshold. Root cause is structural: pre-R11
  Wean-churn (2439 plan failures) had an incidental rescue
  effect — every abandon→replan boundary gave Caretake a fresh
  L2 evaluation chance. R11 (substrate-correct) eliminated the
  churn AND the incidental rescue. R13 (also substrate-correct)
  removed the premature-removal shortcut that previously cut
  the kitten's dependency window short. Result: clean arc
  completion + full natural dependency window (~80k ticks) +
  insufficient Caretake cadence under current L2 balance. The
  395 yield rule is a necessary substrate piece but not
  sufficient on its own. 395 parked behind 397 (kitten survival
  regression with Nurse sub-goal as the recommended structural
  direction). Both land together as a paired commit per the
  `project_pair_baseline_drift_attribution` discipline. The
  yield-rule code stays in the working tree as foundation for
  397's stack.
- 2026-05-17: **Re-parked, blocked on 398** (`§7.M.2
  RaiseOffspringAspiration — kitten-rearing as nested-Intention
  aspiration`). Session dissection identified that 397 itself was
  another patch on the same deferred-spec boundary (HTN frame-pin
  + wrap-site override as stopgap for §L2.10.6) rather than the
  spec-mandated convergence. CLAUDE.md design pillar #4
  (2026-05-17) — *"commitment is one mechanism, not two."*
  Substrate pieces from 395 that survive the convergence:
  R13 Release-at-maturity-1.0 (keep), father-pitches-in symmetric
  picker (keep), release_threshold gap (keep). Pieces that retire
  with the override: R11 dispatch-Advance-on-band-mismatch (patch
  on the HTN-pin), yield rule (workaround for the wrap-site
  override). Plan at
  `/Users/will.mitchell/.claude/plans/let-s-start-tickets-394-397-snazzy-kahan.md`
  (local).
