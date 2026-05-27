---
id: 478
title: ItemSet enum on item kinds (retire is_warriors_kit recipe metadata)
status: blocked
cluster: items-crafting
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-26
parked: null
blocked-by: [463]
supersedes: []
related-systems: [crafting.md, slot-inventory.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

463 added `is_warriors_kit: bool` to `Recipe` (`src/components/recipe.rs`) so `CraftItemAspiration`'s scoring could lift the threat-cue term on the 8 warrior's-kit recipes. The placement is wrong on two axes:

1. **The classification belongs to the item, not to the recipe.** A spear *is* a warrior's-kit item regardless of which recipe produced it, who made it, or which inputs it consumed. The boolean encodes a property of `ItemKind`, but lives on the `Recipe` record that points to it — so the truth has to be re-asserted at every authoring site, and a future recipe that produces the same output but is *not* warrior's-kit (e.g. a polishing or repair recipe) would either lie (carry `is_warriors_kit: false` on the recipe even though its output is kit) or duplicate the data (a separate `is_warriors_kit` on the item).

2. **A bool can't carry future categories.** "Warrior's kit" is one of several item sets the substrate will want to classify in time (behavioral-tool set, mourn-set, mythic-texture set, station-byproduct set, etc.). Each follow-on category would add a new `is_X` bool to `Recipe`, growing the record without expressing the family structure. The substrate-honest shape is a sum type — an `ItemSet` (or `ItemCategory`) enum — keyed on `ItemKind` via a `pub fn item_set(kind: ItemKind) -> ItemSet` or `impl ItemKind { fn set(self) -> ItemSet }`.

## Scope

- Add `ItemSet` enum (variants: `WarriorsKit`, `BehavioralTool`, `Preservation`, `Adornment`, `Material`, `Foraged`, `Prey`, `Other`-ish — author's call on the exact partition; cover the existing ItemKind surface).
- Add `ItemKind::item_set(self) -> ItemSet` (exhaustive `match`; per CLAUDE.md "Prefer compile-time contracts" — adding a new ItemKind variant becomes a compile error until classified).
- Rewrite `CraftItemAspiration`'s threat-cue read to call `recipe.output.item_kind.item_set() == ItemSet::WarriorsKit` instead of `recipe.is_warriors_kit`.
- Delete `Recipe::is_warriors_kit` and every `is_warriors_kit:` literal at populate sites in `src/plugins/simulation.rs` + test fixtures.

## Out of scope

- `discipline_skill_affinity: Option<SkillKind>` — this one *does* belong to the recipe (it ties the act of crafting to the discipline being practiced, which is recipe-level not item-level). Keep it as-is.
- Slot-aware equipment semantics (depends on 017) — `ItemSet` is for categorical reads, not for slot resolution.
- Material-property modifier aggregation (depends on 477) — those classifiers (`weapon_class`, `armor_class`, `noise_class`, etc.) compose the modifier layer; `ItemSet` is the category label, not the property bundle.

## Current state

- 463 lands `is_warriors_kit: bool` on `Recipe` with 8 populated `true` sites (the warrior's-kit recipes).
- `ItemKind` enum is exhaustive in `src/components/items.rs`.
- One read site so far: `CraftItemAspiration::score(recipe)` reads `recipe.is_warriors_kit` to gate the threat-cue term. No other read sites exist.

## Approach

1. Add `ItemSet` enum + `ItemKind::item_set()` method. Exhaustive `match` over `ItemKind` variants. `just check` green.
2. Rewrite the `CraftItemAspiration` read site to use `recipe.output.item_kind.item_set()`. `just check` + `cargo test --release --lib` green.
3. Delete `Recipe::is_warriors_kit`. Compile sweep — every populate site loses the line. `just check` + tests green.
4. Soak: zero behavior delta vs 463 final (read path is equivalent — same 8 recipes match).

## Verification

- `just check` + `cargo test --release --lib` green.
- `just soak-trace 42 Simba 900` shows identical `have_<item>` emissions vs 463's baseline.
- `just frame-diff` shows zero DSE drift.
- Audit: `grep -rn "is_warriors_kit" src/` returns no matches post-land.

## Log

- 2026-05-26: opened from 463 mid-implementation user feedback ("warriors kit belongs in its own item-set enum, not on the recipe"). The bool was a pragmatic shortcut to land the aspiration's scoring; the substrate-correct shape is an ItemSet enum keyed on ItemKind.
