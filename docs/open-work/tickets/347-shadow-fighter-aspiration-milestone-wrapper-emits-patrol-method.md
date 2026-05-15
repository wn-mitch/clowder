---
id: 347
title: Shadow Fighter aspiration_milestone_wrapper + emits + patrol_method
status: ready
cluster: ai-substrate
orchestration: coherent-block
block: htn-method-composition
initiative: [smarter-cats, htn-method-composition]
added: 2026-05-14
parked: null
blocked-by: []
wires-method: [patrol_method]
supersedes: []
related-systems: [htn-methods.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

327 (Combat aspiration_milestone_wrapper) narrowed scope to WARRIORS_PATH
only — the Fight-based chain. SHADOW_FIGHTER is the Patrol-based Combat
chain, and its three milestones (`First Watch`, `Eyes in the Dark`,
`Shadow Fighter`) still carry `emits: &[]`. This ticket wires it,
mirroring 327's Combat-domain pattern but with a Patrol primitive.

Per the universal-aspiration discipline
([`docs/systems/htn-methods.md`](../../systems/htn-methods.md) §G +
CLAUDE.md "Every dormant method has a glue ticket"): the
`wires-method` frontmatter is the back-reference for `patrol_method`,
which 347 authors and registers as Live.

## Scope

- Author `patrol_method` in `src/ai/methods/patrol.rs` — a Tier-1 Live
  primitive HTN method mirroring `fight_method`'s 327 shape:
  `MethodId("patrol_method")`, `goal_label: "patrol_route"`,
  `applicable_when: Live(always_true)`, single `SubGoal::Primitive`
  binding `Action::Patrol` to a new `TargetHint::PatrolRoute` variant
  (extend the enum if needed), `failure_strategy: Abandon`,
  `domain: Some(AspirationDomain::Combat)`.
- Author `SHADOW_FIGHTER_EMITS` in `src/ai/aspirations/combat.rs`
  with at least one Primary row emitting `patrol_route`. A Tertiary
  `flee_to_safety` row (reusing `flee_method`, already Live from 327)
  is recommended as the survival fallback — the same survival logic
  applies whether the Combat cat's track is Fight- or Patrol-based.
- Apply `SHADOW_FIGHTER_EMITS` to all three SHADOW_FIGHTER milestones.
- Register `patrol_method` in `populate_method_registry`
  (`src/plugins/simulation.rs`) alongside the 327 block.

## Out of scope

- Tuning emit priorities via balance soak (later balance-thread work).
- Production gating predicates (`applicable_when` tightening — e.g.
  "perimeter unwatched" for Patrol, "wounded" for Flee) — those land
  in a follow-on balance pass alongside 327's similar pass.
- New Combat-domain DSE substrate; this ticket wraps the existing
  Patrol DSE the same way 327 wrapped Fight + Flee.

## Current state

327 landed 2026-05-14 with `fight_method` + `flee_method` as Live
Tier-1 primitives and `WARRIOR_EMITS` filling the four
WARRIORS_PATH milestones. SHADOW_FIGHTER milestones still carry
`emits: &[]` — picker falls through to domain-affinity for any
Shadow Fighter-aspiring cat (Cedar + Simba in seed-42, verified by
focal-trace dump during 327 implementation).

## Approach

Mirror 327's pattern exactly:
1. Extend `TargetHint` in `src/ai/methods/mod.rs` with a `PatrolRoute`
   variant (rustdoc-cite 347, mirror the `Threat` / `SafeGround` style
   from 327).
2. Author `src/ai/methods/patrol.rs` with `patrol_method()`
   constructor, mirroring `src/ai/methods/fight.rs`.
3. `src/ai/methods/mod.rs` — add `pub mod patrol;`.
4. `src/ai/aspirations/combat.rs` — author `SHADOW_FIGHTER_EMITS`
   const (Primary `patrol_route` + Tertiary `flee_to_safety`), apply
   to all three SHADOW_FIGHTER milestones.
5. `src/plugins/simulation.rs::populate_method_registry` — push
   `patrol_method()` after the 327 block.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes (registry verifies `patrol_method` is Live;
  `wires-method` frontmatter consumed).
- `just headless --seed 42 --duration 30 --focal-cat Cedar` (Cedar
  has SHADOW_FIGHTER aspiration in seed-42, verified during 327
  implementation) shows L1Aspiration emit-walk rows with
  `patrol_route` and `flee_to_safety` labels, both with
  `method_live: true`, `emitted: true` for the Primary row.
- `just verdict logs/afk-347-verify/` — survival canaries hold;
  Patrol activation tally moves in the predicted direction relative
  to the post-327 baseline.

## Log

- 2026-05-14: opened as 327's narrowing follow-on. 327 landed the
  Combat domain's first two Tier-1 methods (`fight_method`,
  `flee_method`) and wired WARRIORS_PATH only; 347 finishes the
  Combat-domain wiring with `patrol_method` + SHADOW_FIGHTER emits.
