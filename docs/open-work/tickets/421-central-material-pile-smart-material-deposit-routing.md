---
id: 421
title: Central material pile + smart material-deposit routing
status: ready
cluster: items-crafting
orchestration: substrate-sensitive
initiative: []
added: 2026-05-19
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

235 narrowed scope to herbs-only: `HasMaterialsInInventory` ships as
scaffolding (allowlisted under this ticket id) but no consumer reads
it because materials don't yet have a central deposit destination.
Ticket 38's pipeline routes materials from scattered `MaterialPile`
ground items → `ConstructionSite` directly, with no return-to-stash
sink. A cat clogged with `Wood` / `Stone` / `ShadowBone` who wants
to hunt currently drops the material at the cat's position (231's
`DropItem` fallback) — same narrative weakness 235 solved for herbs,
but with no Stores-equivalent destination to route through.

## Scope

- Introduce a colony `Stockpile` building (or extend `Stores` to
  also hold materials; decide which mirrors the architecture of
  herbs-in-`StoredHerbs` vs. a new `StoredMaterials` HashMap).
- Author `HasMaterialPileAccessible` per-cat reachability marker
  (mirror of 235's `HasHerbStashAccessible`; same
  `herb_stash_reachable_radius`-equivalent knob shape).
- Add a `DepositMaterials` resolver (mirror of
  `resolve_deposit_herbs_to_stores`) that transfers all material-
  category slots from cat inventory → the stockpile.
- Add the materials-deposit-prefix branch to the 5 plan templates
  235 already extended (`picking_up_actions`, `cooking_actions`,
  `caretaking_actions`, `herbalism_actions` all 3 Herbcraft variants,
  `hunting_actions`), gated on `HasMaterialsInInventory` +
  `HasMaterialPileAccessible` + `CarryingIs(Materials)` (introducing
  `Carrying::Materials` if it doesn't already exist alongside
  `Carrying::BuildMaterials`).
- Drop the `HasMaterialsInInventory 421` row from
  `scripts/substrate_stubs.allowlist` once the reader (the new
  materials-deposit prefix) ships.

## Out of scope

- Curio Cache routing — that's ticket 422 (blocked on 16's Cache
  building).
- Crafting-station integration (a Stockpile that *also* supplies
  recipes feels like a ticket-16 Phase 2 concern).

## Current state

Blocked on 235 landing (substrate scaffolding is in place there:
`HasMaterialsInInventory` writer + allowlist row naming this ticket).

## Approach

Mirror 235's substrate shape exactly. The deposit-prefix branch is
the same `GoapActionDef` cost-1 means-to-end + A\*-spliced
`TravelTo(<materials-zone>)` composition. Decide on Stores-extension
vs new-Stockpile based on whether construction sites would want to
read materials from the stockpile (sympathetic to ticket 16's
crafting-stations design).

## Verification

`just check && just test` — substrate-stub allowlist row drops.
Soak verdict pass + narrative readout: material ground-items
post-this should cluster near the stockpile, not at random cat
positions where they were dropped pre-235.

## Log
- 2026-05-19: opened as 235 follow-on (per CLAUDE.md "antipattern
  migration follow-ups are non-optional"). `HasMaterialsInInventory`
  marker is the open-time scaffolding 235 left for this ticket.
