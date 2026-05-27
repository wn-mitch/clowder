# Anatomical Slot Inventory

## Purpose
Replaces the flat `Inventory { slots: Vec<ItemSlot> }` in `src/components/magic.rs:242` with an anatomy-indexed wearable-slot structure plus a stackable consumable-pouch. Anatomical slots draw from the 13-part enumeration in `body-zones.md`. Wearables carry **identity** (name, origin, creator, narrative hook, material) **and identity-keyed mechanical effects** — never random or decoupled stat-stick fields. Effects derive from the item's identity/material classifiers (per `crafting.md` rule #1) and are applied via the uniform modifier-aggregation layer (ticket 477), not stored as floats on the wearable. Crafted bags (from `crafting.md`) expand pouch capacity without introducing random stat rolls.

**Do not ship standalone.** This is scaffolding without a producer. Gated on at least one wearable producer shipping: `crafting.md` Phase 3 (mentorship tokens, heirlooms), `the-calling.md` (Named Objects as wearable hooks), or `trade.md` (visitor-sourced worn objects). Absent a producer, the refactor is cost without benefit.

## Status — shipped in ticket 017 (Built, against the 369 producer)
The producer gate was met by the **369 warrior's-kit** (8 craftable equippables, organic in seed-42 per 463), so 017 shipped the worn-slot substrate the 477 aggregation layer reads under. What landed:
- `Inventory { pouch: Vec<ItemSlot>, pouch_capacity: u16 }` — the carry bag (was `slots`). Capacity defaults to `MAX_SLOTS`; the Crafted Bag `bag_capacity_bonus` override is **deferred to 370** (no bag producer yet).
- `WearableSlots` component (`src/components/equipment.rs`) — `EquipSlot`-keyed (`Wielded` / `Paws` / `Body` / `Cape`; `Collar` / `Ear` / `Tail` reserved for 370 adornments). One item per slot. `equip_slot(ItemKind)` is an exhaustive classifier.
- `equipment_modifiers_for` reads `WearableSlots` only (worn ≠ carried). A cat wields exactly one weapon (one `Wielded` slot), so the pre-017 multi-weapon ranking was retired.
- **Auto-equip on craft**: the 8 kit recipes route to `ItemDestination::EquippedSlot`, deposited into the matching slot at craft time (pouch fallback if occupied). Deliberate don/doff/swap (`Action::WearItem`) is **334**.
- The **`WearableItem` identity type** below (name / origin_tick / creator_entity / narrative_event_tag) is **deferred to 370**, where the first adornment producer populates it. 017's slots hold the existing `ItemSlot`/`ItemKind` equipment, keyed on `ItemKind` + `ItemSlot.quality`.

## Slot enumeration
| Slot | Underlying BodyPart | Typical Wearable |
|------|---------------------|------------------|
| Collar | Neck (narrative adjunct to Throat) | Woven collar, charm, named-object pendant |
| Ear | Ears | Notched tag, ear-ring |
| Front Paw L / R | Front Left / Right Paw | Wrapped paw (minor narrative) |
| Rear Paw L / R | Rear Left / Right Paw | Rarely used; reserved |
| Tail | Tail | Tail ribbon (courtship), tail tag (mentorship inheritance) |
| Back | Flanks | Satchel, ceremonial drape |
| Mouth | Mouth / Jaw | Reuses existing `Carried(Entity)` for prey/herb/gift |

## Consumable pouch
Stackable consumables (herbs, preserved food, remedy doses, thornbriar, raw crafting materials) live in a separate capacity-limited pouch. Default capacity matches the current `Inventory::MAX_SLOTS`. Crafted Bag items (from `crafting.md`) add capacity via a `bag_capacity_bonus` field on the bag, not on the cat.

## Type guardrail (load-bearing invariant)
The `WearableItem` type carries identity, not random stats:

```
kind: WearableKind,         // the item identity — drives all effects
name: String,
origin_tick: Tick,
creator_entity: Option<Entity>,
narrative_event_tag: String, // matches TemplateRegistry's `event` field; no NarrativeTemplateId type exists
quality: f32,                // craftsmanship scalar, [0,1]
```

**No random or decoupled numeric fields.** No `damage_reduction` / `hunt_bonus` / `armor_class` floats *bolted onto the item*. Effects are keyed to `kind` (+ `quality`) and applied through the uniform modifier-aggregation layer (ticket 477) — the same seam functional wearables (bracers, cloak, spear → 369/334) use — never via `match item.kind` smeared across resolvers. By natural axis:
- **Adornment pieces** (mentorship token, collar, tiara, pin → 370) carry *social/identity* effects: observer fondness modulation, courtship-gift romantic gain, naming-substrate hooks, narrative on equip/inherit/lose, TUI inspect identity signal.
- **Functional wearables** (bracers, cloak, spear → 369/334) carry *hunt/combat/stealth* effects via the same aggregation layer.

If a future PR adds **random-rolled or decoupled** capability floats to `WearableItem` (PoE-style affixes), that is a thesis-breaking change and re-opens this ranking. Identity/material-grounded effects composed by the aggregation layer are the sanctioned shape, not a violation.

## Migration from current flat inventory
| Before | After |
|--------|-------|
| `Inventory { slots: Vec<ItemSlot> }` | `Inventory { pouch: Vec<ItemSlot>, pouch_capacity: u16 }` (stackable consumables) |
| (no wearable concept) | `WearableSlots { collar, ear, tail, back, mouth, paws }` — new component added alongside |
| `inventory.add_herb(...)` / `add_item(...)` | Same API on the pouch field; no consumer-site behavior change |
| `Inventory::MAX_SLOTS` constant | Becomes default `pouch_capacity`; overridden by Crafted Bag bonus |

Consumer sites (known finite set): `persistence.rs`, `plugins/setup.rs`, `components/task_chain.rs` (harvest, remedy prep, ward setting), `systems/needs.rs::eat_from_inventory`, any `magic.rs` sites that check inventory contents. Migration is mechanical; no GOAP or scoring refactor.

## Dependencies
- Hard-gated on at least one wearable producer (`crafting.md` Phase 3, `the-calling.md`, or `trade.md`).
- Reuses body-part enumeration from `body-zones.md` (avoid duplicating the anatomy list — import).
- No hard dep on A1 IAUS refactor; this is a component + consumer refactor, not a scoring change.

## Shadowfox watch
The shadowfox risk here is the OSRS/PoE stat-stick trap. The mitigation is *not* "no effects" — wearables have real bite — but that effects stay identity/material-grounded, deterministic (no affix RNG), composed by the aggregation layer (ticket 477), and trace-visible. PR reviewers flag *random-rolled or decoupled* capability floats on wearables, not identity-keyed effect-data.

## Tuning Notes
_Record observations and adjustments here during iteration._
