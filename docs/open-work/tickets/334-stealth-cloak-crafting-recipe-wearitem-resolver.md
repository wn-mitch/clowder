---
id: 334
title: Stealth-cloak crafting recipe + WearItem resolver
status: blocked
cluster: items-crafting
initiative: [smarter-cats, world-richness]
added: 2026-05-14
parked: null
blocked-by: [320, 322]
wires-method: [acquire_stealth_via_self_craft, acquire_stealth_via_commission]
supersedes: []
related-systems: [htn-methods.md, slot-inventory.md, crafting.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

128 epic Tier-2 glue ticket. The HTN spec's worked example
(Whiskers' stealth-cloak acquisition arc) requires:
1. A `StealthCloak` recipe + crafting substrate.
2. A `WearItem` resolver + wearable-slot substrate.
3. Hunt-resolver wearable-effect read.

All three are external dependencies that this ticket pulls
together. When landed, flips two methods from PendingSubstrate to
Live: `acquire_stealth_via_self_craft` and
`acquire_stealth_via_commission`.

This is the longest-dependency Tier-2 glue ticket — it depends
on crafting substrate AND slot-inventory substrate (both
referenced in `docs/systems/crafting.md` and
`docs/systems/slot-inventory.md` but not yet implemented).

## Scope

- StealthCloak recipe in the crafting substrate (assumes
  crafting substrate has landed by this ticket's start; if not,
  spin off a crafting-substrate ticket as predecessor).
- WearItem step resolver with full witness contract; replaces
  the #322 placeholder.
- Slot-inventory Component + writer (or wire to existing if
  slot-inventory substrate has landed).
- Hunt-resolver effect-read: stealth-cloak in wearable slot
  modifies stalk-success probability per CLAUDE.md "items are
  real" pillar (effects on action resolvers keyed to item
  identity).
- Flip `acquire_stealth_via_self_craft` AND
  `acquire_stealth_via_commission` from PendingSubstrate to
  Live in `populate_method_registry`. Author the full sub-goal
  sequences (acquire materials → reach workshop → craft → don;
  petition → await → retrieve → don).

## Out of scope

- Generic crafting substrate (separate ticket if not already
  landed).
- Generic slot-inventory substrate (separate ticket if not
  already landed).
- Other clothing / wearable items (this ticket lands only the
  stealth-cloak exemplar; other wearables follow as
  per-wearable tickets).

## Current state

128 promoted to epic 2026-05-14; full design at
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md).
Child #16 of 25, blocked on #320 + #322 + (external)
crafting-substrate + slot-inventory-substrate tickets. Batch D
Tier 2 — pace driven by external substrate, not by 128 internals.

## Approach

Per htn-methods.md §Worked example. The method definitions are
already shaped:

```rust
acquire_stealth_via_self_craft.sub_goals = &[
    Goal("stealth_materials_in_inventory"),
    Primitive { label: "reach_workshop", action: Action::Navigate, .. },
    Primitive { label: "craft_stealth_cloak", action: Action::Craft, .. },
    Primitive { label: "don_gear", action: Action::WearItem, .. },
];

acquire_stealth_via_commission.sub_goals = &[
    Primitive { label: "petition_for_gear", action: Action::PetitionCoordinator, .. },
    Goal("ordered_item_ready"),
    Primitive { label: "retrieve_finished_gear", action: Action::Navigate, .. },
    Primitive { label: "don_gear", action: Action::WearItem, .. },
];
```

This ticket implements the actions, the recipe, the slot, and
flips both methods to Live. Frontmatter `wires-method: [...]`
lists both for enforcement.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes (enforcement confirms both methods are
  Live and `wires-method` back-references match).
- `just soak-trace 42 <focal>` on a cat with Hunting aspiration
  + lacking stealth gear: picker emits `stealth_gear_acquired`,
  L2 evaluator picks one of the two methods, frame pushed,
  sub-goal advances visible.
- `just verdict logs/tuned-42` shows no regression on hunt /
  craft canaries.

## Log

- 2026-05-14: opened as 128 epic child #16 (Batch D Tier 2 glue;
  longest external-dependency chain).
