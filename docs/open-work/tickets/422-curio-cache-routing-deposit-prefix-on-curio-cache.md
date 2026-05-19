---
id: 422
title: Curio Cache routing — deposit-prefix on curio Cache
status: blocked
cluster: items-crafting
orchestration: substrate-sensitive
initiative: []
added: 2026-05-19
parked: null
blocked-by: [16]
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

235 narrowed scope to herbs-only: `HasCuriosInInventory` ships as
scaffolding (allowlisted under ticket 16) but no consumer reads it
because curios have no destination today. Per 235's design note,
curios stay droppable-anywhere (the v1 behavior from ticket 231)
until ticket 16's `Cache` building lands as a destination. Once it
does, curios should route the same way herbs do post-235: cat
clogged with `ShinyPebble` / `GlassShard` / `ColorfulShell` who
wants to hunt routes through the Cache to deposit usefully rather
than dropping the curio in the dirt.

## Scope

- Author `HasCurioCacheAccessible` per-cat reachability marker
  (mirror of 235's `HasHerbStashAccessible`).
- Add a `DepositCurios` resolver (mirror of
  `resolve_deposit_herbs_to_stores`) that transfers all
  `ItemCategory::Curiosity` slots → the Cache's per-curio HashMap.
- Add the curios-deposit-prefix branch to the relevant plan
  templates. Curios are picked up *incidentally* during Hunting +
  Foraging (the `drop_priority` curio = 0.05 anchor reflects this),
  so the prefix likely only needs to land on `hunting_actions` +
  `foraging_actions` (or whichever forage variant exists post-16);
  PickingUp / Cooking / Caretaking / Herbalism aren't sources of
  curio inventory pressure.
- Drop the `HasCuriosInInventory 16` row from
  `scripts/substrate_stubs.allowlist` once the reader ships.

## Out of scope

- Material routing — that's ticket 421.
- Curio scoring (e.g., a `CurioCollect` DSE that elects curios as a
  goal rather than a side-effect) — orthogonal to the deposit-
  routing question.

## Current state

Blocked on ticket 16's Cache building. `HasCuriosInInventory` writer
is in place from 235; this ticket adds the reader once the
destination exists.

## Approach

Mirror 235's substrate shape exactly: per-cat reachability marker +
deposit-prefix `GoapActionDef` gated on the marker + carrying-state
+ inventory-content marker. A\* splices `TravelTo(<Cache-zone>)`.
Use the same `herb_stash_reachable_radius` analogue (per-ticket
config knob on `DispositionConstants`).

## Verification

`just check && just test` — allowlist row drops; new unit tests on
the deposit-prefix branch + reachability author. Soak: post-this,
curio ground-items should cluster near Cache tiles.

## Log
- 2026-05-19: opened as 235 follow-on. `HasCuriosInInventory` marker
  is the open-time scaffolding 235 left for this ticket; reader
  lands here once ticket 16's Cache building exists.
