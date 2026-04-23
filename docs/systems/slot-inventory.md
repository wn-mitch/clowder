# Anatomical Slot Inventory

## Purpose
Replaces the flat `Inventory { slots: Vec<ItemSlot> }` in `src/components/magic.rs:242` with an anatomy-indexed wearable-slot structure plus a stackable consumable-pouch. Anatomical slots draw from the 13-part enumeration in `body-zones.md`. Wearables are **narrative/identity-only** — name, origin, creator, narrative-template hook. Crafted bags (from `crafting.md`) expand pouch capacity without introducing modifier stats.

Score: **V=2 F=3 R=4 C=4 H=4 = 384** — "worthwhile; plan carefully" per `systems-backlog-ranking.md`. **Do not ship standalone.** This is scaffolding without a producer. Gated on at least one wearable producer shipping: `crafting.md` Phase 3 (mentorship tokens, heirlooms), `the-calling.md` (Named Objects as wearable hooks), or `trade.md` (visitor-sourced worn objects). Absent a producer, the refactor is cost without benefit.

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
The `WearableItem` type carries exactly:

```
name: String,
origin_tick: Tick,
creator_entity: Option<Entity>,
narrative_template_id: NarrativeTemplateId,
```

**No numeric fields.** No `damage_reduction`, no `hunt_bonus`, no `armor_class`. Effects live entirely in:
- TUI inspect-view rendering (identity signal).
- `social.rs` fondness-gain modulation when observers see a wearable (mentorship token increases apprentice fondness).
- Courtship chain scoring (Gift Object carried in mouth slot modulates fondness gain in target).
- Narrative templates firing on equip / inherit / lose events.

If a future PR adds numeric capability modifiers to `WearableItem`, it is a thesis-breaking change: F drops 3→2, H drops 4→2, composite score falls from 384 to ~96 (earn-slot-only bucket). Treat such PRs as re-opening this stub's ranking, not as ordinary extension.

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
The only shadowfox risk here is the OSRS-misbuild trap. The type guardrail above is the primary mitigation; a secondary check is that PR reviewers flag any modifier-adjacent fields on wearables during code review.

## Tuning Notes
_Record observations and adjustments here during iteration._
