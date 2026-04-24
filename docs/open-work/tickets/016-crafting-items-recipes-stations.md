---
id: 016
title: Crafting — items, recipes, stations
status: ready
cluster: null
added: 2026-04-22
parked: null
blocked-by: []
supersedes: []
related-systems: [crafting.md]
related-balance: []
landed-at: null
landed-on: null
---

**Why it matters:** External proposal (user-sourced via
`/rank-sim-idea`) split out from a composite "OSRS-style inventory +
fantasy adventures" idea. Crafting is the load-bearing piece of the
three-way split (this entry, #17 slot-inventory, #18 ruin-clearings).
It's the only one of the three that is self-justifying on canary
grounds: a §5-first recipe catalog (preservation, play toys,
grooming tools, courtship gifts, mentorship tokens) targets the
ecological-variety continuity canary directly, and Phase 3 produces
wearables that unblock #17.

**Design captured at:** `docs/systems/crafting.md` (Aspirational,
2026-04-22). Phases 1–5 enumerated with required hypotheses per
phase (Phase 4 and Phase 5 added 2026-04-22 via decoration / place-
making expansion).

**Score:** V=5 F=4 R=3 C=3 H=3 = **540** — "worthwhile; plan
carefully" (300–1000 bucket). Promoted from 288 → 540 on 2026-04-22
when Phase 4 (Domestic refinement / folk-craft decorations) and
Phase 5 (Elevated cat-craft / collective multi-season) were added.
Originally rank 6 in `docs/systems-backlog-ranking.md`.

**Ship-order note:** Among the originally-split features, crafting
is the anchor and ships first. It de-risks #17 (slot-inventory gets
its first producer at Phase 3) and #18 (ruin-clearings loot has a
consumer once Phase 1 preservation recipes land). Phase 4
decorations become the second primary consumer of #20 (naming
substrate); Phase 5 is gated on aspirations-mastery arcs and is
long-horizon.

**Design constraints (load-bearing — drift re-triggers ranking):**
- §5-first catalog. No combat gear in the catalog. If a combat-gear
  recipe is ever proposed, re-rank the stub — F and H both drop.
- `CraftedItem` type carries narrative/identity fields only. No
  numeric capability modifiers on items themselves; action
  resolvers own the gameplay effect of using an item.
- **Decorations are place-anchored, not cat-anchored** (Phase 4+).
  A rug warms the hearth tile; a lamp illuminates a room. The cat
  who placed the decoration gets no personal bonus.
- Cat-native palette. Reed, bone, fur, feather, shell, rendered
  fat, berry/clay pigment. No metalwork / milled lumber /
  human-import materials.
- **Not-DF guardrail for Phase 5.** Phase 5 is collective (multi-cat)
  or cumulative (multi-season), never individual-rare-strike.
  `the-calling.md` owns individual mood-strike craft.
- Generalize `remedy_prep` and `ward_setting` into the unified
  catalog in Phase 1. Do not leave parallel code paths.

**Phase 5 gating (three conditions, all required):**
1. Colony-age ≥3 sim-years (materials accrete across seasons).
2. Material-scarcity (deep exploration / cleared ruins / cross-
   season storage inputs).
3. Skill-via-aspirations — new mastery arcs (`WeavingMastery`,
   `BoneShapingMastery`, `PigmentMastery`, `CairnMastery`) defined
   in `aspirations.rs`; at least one cat advanced on a relevant arc
   enables the recipe for the whole colony.

**Dependencies:** benefits from but does not hard-block on the A1
IAUS refactor. Phase 1 is independent. Phase 3 soft-depends on #17
existing. Phase 3 and 4 soft-depend on #20 (naming substrate) —
can ship with neutral-fallback generator. Phase 4 soft-depends on
`environmental-quality.md` (A-cluster refactor) — ships with a
minimal `TileAmenities` interface otherwise. **Phase 5 hard-depends
on** `aspirations.rs` skill-arc extension (new mastery arcs);
ships in same PR as Phase 5 or as a precursor PR.

**Required hypothesis per phase** (per `CLAUDE.md` Balance
Methodology) is recorded in the stub.

**Resume when:** picked up next in the §5-sideways work thread.
Phase 1 (food preservation) is the recommended pilot since it hits
the starvation canary most directly. Phase 4 should pair with #20
(naming) landing.
