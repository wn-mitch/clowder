---
id: 364
title: 357 follow-on — D1 dispatch closure (frame-pin + advance) + D2 reactive emission + real Wean/Teach/Release resolvers
status: ready
cluster: ai-substrate
orchestration: coherent-block
block: htn-method-composition
initiative: [smarter-cats, htn-method-composition]
added: 2026-05-15
parked: null
blocked-by: []
supersedes: []
related-systems: [htn-methods.md, ai-substrate-refactor.md]
related-balance: []
wires-method: []
landed-at: null
landed-on: null
---

## Why

#357 shipped the dispatch *scaffold* (six `GoapActionKind` variants
for the HTN-method primitives, matching `to_action` arms, dispatch
arms in `evaluate_step_for_action` that route to the existing dormant
resolvers). The mechanism that *closes the loop* — reading the
`HeldGoalStack` frame at the L2 author site to pin `chosen_action`
to the leaf primitive, advancing `sub_goal_index` on
`IntentionFulfilled`, and emitting `kitten_reared` reactively for
queens with dependent kittens — was not in 357's land. The methods
sit Live in the registry but are never adopted because no path emits
the matching goal labels, and even if one did, no path pins
`chosen_action` from the held frame.

This is the load-bearing closure that makes `rear_kitten` (and later
`mourn_at_grave`) actually fire in a soak. Without it, KittenWeaned /
KittenTaught / KittenReleased Features stay at zero in the activation
footer — the dispatch scaffold is a no-op.

## Scope

Per `~/.claude/plans/work-357-purrfect-flask.md` decisions D1, D2,
D5 (kitten-arc only), and D6 (rear_kitten-side Features only). The
mourn-arc remains deferred per 357's §Out of scope (no `Mourning`
writer until §7.7.b ships).

- **D1 adoption hook.** Extend `evaluate_and_plan` in
  `src/systems/goap.rs` (around line 2361 — `chosen_action`
  derivation): when the cat carries a non-empty `HeldGoalStack` whose
  top frame's current sub-goal is `SubGoal::Primitive { action,
  target_hint, .. }`, override `chosen_action` with the leaf
  primitive's action, replacing the disposition-softmax winner. Add
  a new `htn_primitive_actions(action, distances) -> Vec<GoapActionDef>`
  plan-template builder in `src/ai/planner/actions.rs` returning a
  single-action Pattern-B plan keyed to the primitive (zone is
  `SocialTarget` for the kitten arc — kittens are alive cats in
  `cat_positions`). Route to it from the L2 evaluator at the call
  site that currently invokes `actions_for_disposition` (line 2461).

- **D1 advance hook.** Extend `resolve_goap_plans` (the existing 126
  `IntentionFulfilled` emission point — grep `IntentionFulfilled` in
  `src/systems/goap.rs`): when a held leaf primitive fulfills AND the
  cat carries a `HeldGoalStack` whose top frame's leaf matches the
  fulfilled primitive, increment `top_mut().sub_goal_index`. If
  `sub_goal_index < sub_goal_count`, adopt the next primitive as the
  new `HeldIntention`. Else pop the frame, propagate fulfillment
  upward. Emit `Feature::SubGoalAdvanced` on advance,
  `Feature::IntentionFulfilled` on terminal completion.

- **D1 backtrack hook.** Extend `resolve_goap_plans` (existing 126
  `IntentionAbandonReason::*` site): consult top frame's
  `method.failure_strategy`. For `Backtrack`, try the next applicable
  method for the same `goal_label` (registry lookup); for `Abandon`,
  pop the frame; for `Retry`, reset `sub_goal_index = 0` and
  increment retry counter. Emit `Feature::MethodBacktracked` on
  backtrack.

- **D2 reactive emission.** Extend
  `src/systems/aspiration_picker.rs::pick_aspiration_emissions` with
  a parallel `ReactiveEmit` table consumed in a new step 1.5
  (between the in-flight commitment check at step 1 and the
  milestone `emits[]` walk at step 2). New shape:
  ```rust
  pub struct ReactiveEmit {
      pub label: &'static str,
      pub applicable_when: fn(&World, Entity) -> bool,
      pub strategy: CommitmentStrategy,
      pub priority: Priority,
  }
  const REACTIVE_EMITS: &[ReactiveEmit] = &[
      ReactiveEmit {
          label: "kitten_reared",
          applicable_when: has_dependent_kitten,
          strategy: CommitmentStrategy::Sticky,
          priority: Priority::Primary,
      },
      // mourn_at_grave's "process_grief" deferred — needs Mourning
      // writer per §7.7.b.
  ];
  ```
  Reactive emits take priority over milestone emits because they
  represent a *demand* (active dependent kitten) rather than an
  aspirational chain.

- **D5 dependent_kitten target picker.** New file
  `src/ai/dses/dependent_kitten_target.rs`. Mirrors
  `caretake_target.rs` shape: candidate query is
  `Query<(Entity, &KittenDependency)>` filtered by
  `kd.mother == Some(self)`. Axes: nearness (Quadratic 1.5),
  maturity-against-action-threshold (action-parameterized — Wean
  wants `maturity < weaned_threshold`; Teach wants
  `weaned ≤ maturity < teach_done`; Release wants
  `maturity >= teach_done`), recent-failure cooldown. Composition
  `WeightedSum`, aggregation `Best`. Register in
  `populate_dse_registry` (`src/plugins/simulation.rs`). Action
  parameter passes from the held frame.

- **Real resolvers.** Replace the dormant stubs at
  `src/steps/disposition/{wean, teach, release}.rs`:
  - `resolve_wean(target_kitten: Entity, commands: &mut Commands)` —
    advance the kitten's `KittenDependency.maturity` to
    `max(current, weaned_threshold)`. Witness `Some(kitten)` iff
    maturity advanced; `None` if already past.
  - `resolve_teach(...)` — advance to `teach_done` threshold.
  - `resolve_release(...)` — remove `KittenDependency` from the
    kitten (cascades the `Parent` marker removal in `growth.rs`
    via `update_parent_markers`).
  Threshold constants in `src/resources/sim_constants.rs`
  (defaults: weaned=0.33, teach_done=0.66, full_release=1.0 —
  matches the existing 4-season maturation curve).

- **D6 Feature promotion.** In `src/resources/system_activation.rs`,
  flip `KittenWeaned`/`KittenTaught`/`KittenReleased` to `true` in
  `expected_to_fire_per_soak()`. Promotion is conditional on a
  cutover soak observing them firing reliably (seed 42 baseline);
  if they don't fire, debug the chain and only flip after the
  trace shows them firing.

## Out of scope

- **Mourn arc dispatch** (Vigil / GriefSit / ReleaseGrief). Blocked
  on the §7.7.b grief-event-emission debt that authors `Mourning`
  on colony-mate death — without it, `has_active_mourning` is
  always false and `mourn_at_grave` is never adopted. The 357
  scaffold has stub dispatch arms for these; real wiring lands when
  §7.7.b ships.
- Father / partner involvement in rearing (#333 §Out of scope).
- Per-kitten grief emission on death (#333 §Out of scope).
- Balance tuning for maturity-bump rates (deferred until AI
  substrate stabilizes per CLAUDE.md). The threshold defaults
  named in §Scope are starting points; soak verdict drives any
  retune.

## Current architecture (layer-walk audit)

| Layer | Component / file | Fact | Status |
|---|---|---|---|
| HTN method registry | `src/ai/methods/rear_kitten.rs` | rear_kitten is `Live(has_dependent_kitten)`; sub_goals = [Wean, Teach, Release] with `TargetHint::DependentKitten` | `[verified-correct]` (357) |
| Parent marker writer | `src/systems/growth.rs:294` | `update_parent_markers` authors `Parent` on cats with living KittenDependency descendants | `[verified-correct]` (014) |
| Action enum | `src/ai/mod.rs` | `Action::Wean`/`Teach`/`Release` registered with modifier-id slots | `[verified-correct]` (332/333 substrate) |
| GoapActionKind | `src/ai/planner/mod.rs` | Variants exist; to_action arms route to the leaf primitive's Action | `[verified-correct]` (357 scaffold) |
| Dispatch arm | `src/systems/goap.rs` | Arms call `crate::steps::disposition::resolve_wean()` etc.; resolvers are dormant stubs returning `Fail` | `[verified-defect-resolver]` (357 scaffold + needs real resolvers) |
| HeldGoalStack consultation | `src/systems/goap.rs::evaluate_and_plan` | **No code reads `HeldGoalStack.frames[N].sub_goal_index` to override chosen_action** — confirmed gap | `[verified-defect]` |
| Advance hook | `src/systems/goap.rs::resolve_goap_plans` | **No path increments `sub_goal_index` on leaf fulfillment** — confirmed gap | `[verified-defect]` |
| Reactive emission | `src/systems/aspiration_picker.rs` | **No code emits `kitten_reared` goal label** — confirmed gap (rear_kitten has `domain: None`; no aspiration milestone references it) | `[verified-defect]` |

All `[verified-defect]` rows are addressed by §Scope items above.

## Approach

Per `docs/systems/htn-methods.md` §L2 evaluator integration (lines
235-258) and §Lifecycle (lines 333-364). The frame-pin shape is
spec-named: "Adopt the leaf as `HeldIntention`." The advance shape
is spec-named: "Increment top frame's `sub_goal_index`. If
`sub_goal_index < method.sub_goals.len()`: adopt the next sub-goal."

This is the substrate-over-overrides path, not a post-hoc modifier
on per-Action scores. The trace surface (per htn-methods.md §11.5)
already exposes `L3Commitment.method_stack` — verifying via
`just inspect <queen-name>` reads the same substrate.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes.
- `cargo test --release` passes.
- `just soak-trace 42 <queen-with-kitten>` shows:
  - `rear_kitten` method frame on the cat's `HeldGoalStack`.
  - Trace records show `SubGoalAdvanced` Features per kitten-arc
    milestone.
  - `KittenWeaned` Feature count non-zero in the footer.
- `just verdict logs/tuned-42` shows no regression on
  generational-continuity canaries.
- `just inspect <queen-name>` renders the goal-stack section
  (`ccf0a5fd` substrate) showing the rear_kitten method history.

## Log

- 2026-05-15: opened as the focused continuation of #357. The 357
  scaffold landed at `daf18486` (six `GoapActionKind` variants +
  dispatch arms + to_action arms); 357's substrate recovery landed
  at `4c211d5b`; 357's gate refinement landed at `4aa6f775`. This
  ticket picks up at the dispatch-closure mechanism proper. The
  plan file at `~/.claude/plans/work-357-purrfect-flask.md`
  carries the multi-decision rationale; this ticket carries the
  executable steps.
