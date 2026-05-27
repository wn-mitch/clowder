---
id: 479
title: Ranged-attack mode for Sling — Action / phase / ammo model + read of EquipmentModifiers.ranged_enabled
status: ready
cluster: items-crafting
initiative: [world-richness]
added: 2026-05-27
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
369 classified the Sling as `WeaponClass::Ranged`, and 477 exposed
`EquipmentModifiers.ranged_enabled` (true when a cat carries a Sling) — but **no
resolver reads it**: there is no ranged-attack mode anywhere in the sim. The
Sling is craftable and aggregated but mechanically inert. This ticket is the
deferred consumer 477 split out (its scope note: "the largest sub-piece by a
margin; design depends on whether ranged is a parallel-to-stalk hunt-phase or
its own action"). It realizes the "items have bite" pillar for the one
remaining warrior's-kit weapon class.

## Scope
- A ranged-attack mechanism that fires when the cat carries a Sling
  (`em.ranged_enabled`) and prey is in the ranged band (further than
  `pounce_range`, within sight). Engagement-at-range per `equipment.rs`
  `WeaponClass::Ranged` doc: "ammunition is ambient fieldstone (no consumed-ammo
  entity in Phase 2b)."
- The read surfaces in the resolver trace as a named `L4Resolver` modifier
  (reuse 477's `FocalResolverSink`), never a hidden bonus.
- `Feature::RangedAttackLanded` (or similar) + `expected_to_fire_per_soak`
  classification + `Feature::ALL` entry + the parallel `category` / `feature_name`
  arms + the `EXPECTED_VARIANT_COUNT` sentinel bump (the four-surface checklist
  477 walked for `BoneWeaponSnapped`).
- A `just scenario equipment_sling_ranged` integration proof (mirror the 477
  `equipment_weapon_strike` scenario: Stores building + LightForest tile + prey
  cluster so the cat reliably hunts; assert the ranged-attack trace row fires).

## Out of scope
- Consumed-ammo entities (Phase 2b explicitly defers them; fieldstone is ambient).
- Metal/Loud ranged weapons (Phase 2c / 370).
- Re-tuning the Phase-2b armor / weapon-bonus / cloak magnitudes 477 shipped
  (those live in a future `docs/balance/equipment-effects.md` thread).

## Current state
Blocked on **477** (lands the aggregation API + `FocalResolverSink` resolver-trace
hook + `ranged_enabled` field). 477's `equipment_weapon_strike` scenario is the
shape precedent for the verification harness; its `weapon.pierce.bonus` trace row
is the shape precedent for the ranged trace row.

## Approach
**Open design question (resolve before coding):** parallel-phase vs new Action.
- **Parallel phase inside `resolve_engage_prey`** (`goap.rs`): add a `Ranged`
  band check before the stalk/pounce ladder — when `em.ranged_enabled` and the
  distance fits, run a ranged strike instead of closing. Reuses target
  acquisition, the kill/meat-spawn path, and avoids growing the
  `score_actions` dispatcher (the `project_score_actions_dispatch_antipattern`
  surface). **Recommended** unless the ranged decision needs to compete in L2
  against melee Hunt as a distinct DSE.
- **New `Action::RangedAttack`** + dedicated `resolve_ranged_attack` step +
  DSE registration + `from_action` mapping (`disposition.rs`). Heavier; only
  warranted if ranged is a genuinely separate Intention the softmax should weigh
  against Hunt.

See `docs/systems/crafting.md` Phase 2b for the fiber/sling material notes.

## Verification
- `just scenario equipment_sling_ranged` — assert the ranged read fires and
  surfaces in the focal trace; assert prey is taken at range.
- `just check && just test`.
- `just soak-trace 42 Simba` + `just verdict logs/tuned-42` — hard survival
  gates hold; the new Feature may be scenario-only (classify accordingly).

## Log
- 2026-05-27: opened as 477's deferred ranged-attack consumer (split out at
  477 plan review — carries genuine design questions warranting its own ticket).
  Blocked on 477.
