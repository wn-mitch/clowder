---
id: 435
title: RecipeInput::AnyOf substrate (collapse 4 smoked.* recipes to 1; unblock generalized fuel input)
status: ready
cluster: items-crafting
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-21
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
`RecipeInput` (src/components/recipe.rs:36-39) is currently
`{ kind: ItemKind, count: u32 }` — a single concrete input kind per slot.
Flagged at recipe.rs:31-34 as a future need: *"A future 'consumes any-of-{kinds}'
shape (e.g. 'any fuel') would extend this with a new variant rather than
adding a flag field."* Ticket 367 Commit 5 worked around the absence by
registering 4 parallel `preserve.smoked.{mouse,rat,rabbit,bird}` recipes —
one per raw meat kind — all producing the same `ItemKind::SmokedMeat`. This
works but rots: every new smokeable meat (Phase 2 hide-source carcasses,
future prey variants) requires another duplicate registration, and the
"any fuel" generalization (replace `Wood` with `any material that burns`)
is structurally blocked.

## Scope
- Extend `RecipeInput` from a struct to an enum: `One { kind, count }` plus `AnyOf { kinds: Vec<ItemKind>, count }`.
- Update all 4 smoked.* registrations to a single `preserve.smoked` entry with `AnyOf { kinds: [RawMouse, RawRat, RawRabbit, RawBird] }`.
- Update the load resolver (`src/steps/disposition/load_smoking_rack.rs`) to consume *any* slot whose kind matches the `AnyOf` set, not just `is_raw_meat()`.
- Audit existing recipe call sites in `populate_recipe_registry` for opportunistic consolidation (single-kind entries stay as `One` for clarity; multi-kind candidates collapse).

## Out of scope
- Generalized fuel substrate (any-burnable, not just Wood) — that's a separate ticket once `AnyOf` exists; covered by the parent 016 epic but not load-bearing here.
- Quantity scaling (e.g. "any 2 herbs, can be the same or different"). The `AnyOf` shape should be additive — keep `count` semantics as "this many slots total from the set."

## Current state
Substrate landed in 367 Commits 1-7 (2026-05-21). Four `preserve.smoked.*` recipes exist as parallel entries. Marker at recipe.rs:31-34 anticipates this ticket.

## Approach
Convert `RecipeInput` to an enum. Migrate every existing call site (probably ~10 sites across remedy / ward / preserve recipes) — the migration is mechanical: existing `RecipeInput { kind, count }` becomes `RecipeInput::One { kind, count }`. Update the smoking load resolver's `is_raw_meat()` check to consult the `AnyOf` set from the recipe registry instead of the hard-coded predicate.

## Verification
- All existing recipes parse + register cleanly post-migration.
- 4 smoked.* entries collapse to 1 in the registry dump (just-recipe-list or similar).
- 367's first-light Features (`MeatLoadedOnSmokingRack`, `SmokingRackTended`, `MeatSmoked`) still fire ≥ 1 on seed-42.
- Unit test for the load resolver: cat with `RawMouse` slot + `Wood` slot loads smoking rack via the collapsed recipe; cat with `RawBird` slot + `Wood` slot also loads via the same recipe.

## Log
- 2026-05-21: opened as 367 antipattern-migration follow-on.
