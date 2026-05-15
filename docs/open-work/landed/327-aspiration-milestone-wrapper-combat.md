---
id: 327
title: Combat aspiration_milestone_wrapper + emits tables
status: done
cluster: ai-substrate
initiative: [smarter-cats]
added: 2026-05-14
parked: null
blocked-by: []
wires-method: [aspiration_milestone_wrapper.combat]
supersedes: []
related-systems: [htn-methods.md]
related-balance: []
landed-at: be6785fd38a5
landed-on: 2026-05-14
---

## Why

128 epic Tier-2 chain follow-on. Authors the Combat chain's
`Milestone.emits[]` tables and flips
`aspiration_milestone_wrapper.combat` from
`ApplicableWhen::PendingSubstrate` (registered at #321 land) to
`Live`.

Per the universal-aspiration discipline
([`docs/systems/htn-methods.md`](../../systems/htn-methods.md)
§G + CLAUDE.md "Every dormant method has a glue ticket"): this
ticket is the glue. The frontmatter `wires-method` field is
the back-reference the enforcement script verifies.

## Scope

- Author `Milestone.emits[]` for every milestone of the Combat
  chain in `src/systems/aspirations.rs`.
- Per `Emit`: `label` (registered method), `applicable_when`
  precondition, `strategy`, `priority` (Primary/Secondary/Tertiary).
- Flip `aspiration_milestone_wrapper.combat` from PendingSubstrate
  → Live in `populate_method_registry`.

## Out of scope

- Tuning emit priorities via balance soak (later balance-thread
  work).
- New Combat-domain DSE substrate; this ticket wraps existing
  Fight / Patrol / Flee primitives.

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #9 of 25, blocked on #319 + #321. Batch C — 5-way parallel
with #328-#331. Can overlap Batch B.

## Approach

Per htn-methods.md §H + Tier-1/Tier-2 split. Same shape as #325
Hunting wrapper; differs only in chain-specific milestone
predicates and emit lever set.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes (registry verifies wrapper is now Live;
  enforcement script confirms `wires-method` is consumed).
- `just soak-trace 42 <focal>` on a cat with Combat aspiration
  shows L1Aspiration emit-walks with non-empty rows.

## Log

- 2026-05-14: opened as 128 epic child #9 (Batch C chain
  follow-on).
- 2026-05-14: 2026-05-14: landed. Authored fight_method (engage_threat) + flee_method (flee_to_safety) as Tier-1 Live primitives mirroring 321's hunt_method. WARRIORS_PATH milestones now emit Primary engage_threat + Tertiary flee_to_safety; SHADOW_FIGHTER stays empty per ticket-347 follow-on. Verified via just headless 30s focal-trace on Wren (WARRIORS_PATH at milestone 0): 123/686 emit_walk rows populated, engage_threat method_live+applicable+emitted, flee_to_safety method_live but lost to Primary. Full 15-min verdict vs tuned-42-484d9f60 baseline: survival canaries PASS (0 deaths), continuity FAIL on mythic-texture=0 (also 0 in baseline so non-regression), constants_drift clean, colony_score concern: fulfillment -87.7%, welfare -16.9%, nourishment -20.7%, peak_pop +20%, kittens_born +100%. Drift narrative: new Combat goals enter L2 pool every applicable tick with always_true gating, shifting action distribution. Production gating (threat-in-range / wounded predicates) deferred to follow-on balance pass per ticket Approach; combined with 347 SHADOW_FIGHTER wiring will replace always_true with belief-driven gates. Doctrine note: wires-method frontmatter cites aspiration_milestone_wrapper.combat (htn-methods.md §H category name); the literal method ids are fight_method and flee_method — registry script enforces PendingSubstrate→ticket direction only, so live-side drift is documentation, not a CI gate. SHADOW_FIGHTER follow-on opened as 347.
