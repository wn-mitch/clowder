---
id: 387
title: Multi-generation lineage substrate (kin-tracking depth >=3)
status: ready
cluster: life-cycle
orchestration: coherent-block
block: worldgen-prehistory
initiative: [generational-continuity, worldgen-prehistory]
added: 2026-05-16
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

009's exit criterion includes "starting cats have ≥2-generation lineage referenceable by name." Today `src/components/social.rs` tracks per-cat relationships among live entities, but parents-of-parents (grandparents) are unreachable when only the living cats are spawned as entities. A pre-sim founder colony's dynastic depth — the names Silverpaw, Brindle, the great-grandmother who taught the current matriarch to hunt — needs to be encoded as a *historical* graph that survives the Phase-1 → Phase-3 boundary even though most ancestor entities have despawned.

## Scope

- A `Lineage` substrate that references ancestor identities by name + stable ID + per-ancestor facts (cause of death, age at death, named events they witnessed)
- The lineage graph is populated DURING Phase-1 (every birth records the parent IDs) and PERSISTS after Phase-1 cleans up dead-cat entities
- Lookup by name resolves an ancestor identity even when no living entity carries that name
- Lineage-depth canary: ≥80% of starting cats have ≥2-generation reachable lineage at t=0

## Out of scope

- The sim-loop mode that runs Phase-1 (#385)
- Asymmetric bond propagation across generations (sibling/mentor-of-mentor relationships across death boundaries) — could be subsumed here or split out as follow-on
- The narrative-template work that CONSUMES lineage references at runtime (#391)
- Per-cat `MentalModel` beliefs ABOUT ancestors (those are #386's territory; this leg is the *ground-truth* graph, not subjective beliefs)

## Current state

Aspirational — gated on `worldgen-prehistory` block activation (see [9]). No live blockers within this block; could be developed independently and used by #386, #388, #391.

## Approach

Lineage is colony-scope substrate (a `Resource` mapping `CatId → Lineage`) so it survives entity despawn. Populated lazily on birth (Phase-1 `MatingOccurred` / kitten-spawn events) and preserved across the Phase-1 → Phase-3 boundary. Per-cat `Lineage` component on living cats is a thin index into the colony-scope graph.

## Verification

- Phase-1 termination state: lineage graph non-empty, ≥80% of surviving cats have parents recorded, ≥60% have grandparents recorded (depth-3 ≥ 2 ancestors)
- Name-lookup test: pick a random surviving cat; resolve `grandparent` by name; confirm the resolution succeeds even when no living entity carries that name
- Compatibility with the `KittenMatured` continuity canary (lineage extends with each maturation in Phase-3)

## Log

- 2026-05-16: opened as leg of `worldgen-prehistory` coherent-block (see [9])
