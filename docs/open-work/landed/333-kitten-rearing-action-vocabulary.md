---
id: 333
title: Kitten-rearing action vocabulary — flip rear_kitten to Live
status: done
cluster: life-cycle
orchestration: coherent-block
block: htn-method-composition
initiative: [smarter-cats, generational-continuity, htn-method-composition]
added: 2026-05-14
parked: null
blocked-by: []
wires-method: [rear_kitten]
supersedes: []
related-systems: [htn-methods.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-15
---

## Why

128 epic Tier-2 glue ticket. Authors the action vocabulary
required to flip the `rear_kitten` PendingSubstrate method to
Live: Wean / Teach / Release primitives keyed to
`KittenDependency`.

Today `KittenDependency` (in `src/components/kitten.rs`) is the
maturity-tracker on kittens; mothers care for kittens via the
per-tick Caretake DSE without a multi-tick "I am rearing kitten
X" commitment. This ticket adds the multi-stage rearing arc as a
method-decomposed aspiration on the mother.

## Scope

- New `Action::Wean`, `Action::Teach`, `Action::Release`
  variants (or refine via #322's batch; details settled during
  implementation).
- Placeholder-then-real resolvers per variant.
- Mother-side `RearKittenIntent` substrate or extension of
  existing CaretakeDse — TBD design choice.
- Flip `rear_kitten` from PendingSubstrate to Live in
  `populate_method_registry`. Author the four sub-goals: nurse
  → wean → teach → release.

## Out of scope

- Kitten-side perception of mother's intent (banned by 126
  actor-private discipline; mutually-public projection is 127
  JointIntention territory if needed).
- Tuning rearing duration / yield (balance-thread work).
- Father / partner involvement in rearing (separate aspiration
  if added later).

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #15 of 25, blocked on #320 + #322. Batch D Tier 2.

## Approach

Per htn-methods.md §G Tier-2 + §Migration catalogue.
`rear_kitten(target_kitten)` keyed by the kitten Entity; one
method frame per kitten the mother is rearing. Frontmatter
`wires-method: [rear_kitten]` enforced.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes (enforcement confirms wires-method
  back-reference).
- `just soak-trace 42 <mother>` on a queen with a kitten shows
  the `rear_kitten` method frame; sub-goal advances as kitten
  maturity progresses.

## Log

- 2026-05-14: opened as 128 epic child #15 (Batch D Tier 2 glue).
- 2026-05-15: same dispatch gap as #332 (which see). The HTN
  substrate doesn't override `chosen_action`, so the
  `Action::Wean` / `Action::Teach` / `Action::Release` resolvers
  can't fire even after this lands; the verification step
  ("`rear_kitten` method frame on the stack") is structurally
  deferred under the coherent-block discipline (verdict-skipped
  intermediate; verdict fires at the #128 anchor). A
  consolidated dispatch follow-on covering both #332 and #333
  opens immediately after this lands.
- 2026-05-15: per §Scope's "TBD design choice" — no
  `RearKittenIntent` Component is introduced. The §4.7 classifier
  flags it as additive substrate that `KittenDependency.mother`
  already covers (the durable, mutually-public link is the
  fact; the HTN method frame on the mother's `HeldGoalStack`
  carries the commitment).
- 2026-05-15: lands with: `Action::Wean` / `Action::Teach` /
  `Action::Release` resolver upgrades to
  `StepOutcome<Option<Entity>>` (Wean, Release) and
  `StepOutcome<bool>` (Teach) with the five rustdoc headings;
  `TargetHint::DependentKitten` variant; method literal flipped
  to `ApplicableWhen::Live(cat_is_alive)` with the precise
  reverse-lookup gate moved to the dispatch follow-on
  (`fn(&World, Entity) -> bool` can't enumerate archetypes from
  `&World` in Bevy 0.18; the kitten-target picker authored in
  the dispatch ticket owns the same lookup); three new
  `Feature::*` variants (`KittenWeaned`, `SkillTaught`,
  `KittenReleased`, all Positive valence,
  `expected_to_fire_per_soak() => false` pending dispatch
  follow-on). Out-of-scope per §Scope: kitten-side perception
  of mother's intent (banned by 126 actor-private discipline);
  tuning rearing duration / yield (balance-thread work).
