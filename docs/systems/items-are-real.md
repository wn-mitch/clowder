# Items-are-real: the Source / Transfer / Sink contract

The **items-are-real** design pillar names a contract: an item is a real spatial entity (or value-typed inventory slot of a real kind), never an abstract resource and never a stat-stick. Items live in one of three locations — a building's `StoredItems::items`, a cat's `Inventory::pouch` slot, or the ground as an `Item` entity with `ItemLocation::OnGround`. Movement between those locations IS the substrate of the economy; every transition must pass through a named gate.

This doc names the three gates, their enforcement layers, and the audit table that classifies every Inventory-mutation site in `src/`.

## The three gates

- **Source** — an item enters the world or an Inventory from nothing. Forage success, hunt catch, den-raid carcass, craft output, trader arrival. The Source decides whether the new item lands in the actor's Inventory (if room) or as an `Item` entity on the ground (overflow). Every Source emits a `Feature::ItemSourced*` variant identifying its origin.
- **Transfer** — an item shifts location *without* form change. Cat-to-cat Handoff, deposit at Stores, retrieve from Stores, drop on ground, pick up from ground. Form is preserved (`ItemKind`, modifiers, quality); only the container changes. Every Transfer routes through a primitive in [`src/components/item_transfer.rs`](../../src/components/item_transfer.rs) and emits a `Feature::*` naming the move (e.g. `ItemHandedOff`, `ItemDroppedOnGround`).
- **Sink** — an item exits the world or an Inventory. Eat (food → hunger credit), feed-a-kitten (parent slot → kitten hunger), bury-with-the-dead (grave goods), decay-to-nothing, crafting-consumption (raw inputs spent on a recipe). Every Sink emits a `Feature::*` naming the disposition.

A useful disambiguator: **Source = entropy goes down (item created)**, **Sink = entropy goes up (item destroyed)**, **Transfer = entropy stays put (item relocates)**.

## Enforcement layers

Three parallel layers, mirroring [`src/components/item_transfer.rs`](../../src/components/item_transfer.rs)'s rustdoc:

1. **Compile-time (the `ItemSource` trait)** — Source impls live under [`src/components/item_gate.rs`](../../src/components/item_gate.rs). Each impl declares its `const FEATURE: Feature`, its `kind()` / `modifiers()`, and inherits the default `source(...)` body that handles push-or-overflow. The trait's `StepOutcome<Option<SourcePlacement>>` return type forces the caller to route Feature emission through `record_if_witnessed`. **Transfer** and **Sink** parallel traits are deferred (existing function-shape resolvers — `handoff`, `feed_kitten`, `cook`, `deposit_at_stores`, `bury`, etc. — already satisfy the function-shape contract from `StepOutcome<W>`).
2. **CI lint** — [`scripts/check_item_transfers.sh`](../../scripts/check_item_transfers.sh) flags any `*.pouch.push`, `*.pouch.swap_remove`, `*.pouch.retain`, `*.pouch.remove`, `inventory.take_food()`, `inventory.add_food*()`, `inventory.add_item*()`, or `commands.spawn(Item::` outside the gate-author surface (`src/components/item_transfer.rs`, `src/components/item_gate.rs` + submodules, `src/steps/**`). Allowlist at `scripts/item_transfers.allowlist` with required ticket id.
3. **Runtime witness** — every gate emits a `Feature::*` via `StepOutcome::record_if_witnessed`, enrolled in `Feature::expected_to_fire_per_soak` (`src/resources/system_activation.rs`). A gate that should fire in a healthy seed-42 soak but doesn't is a never-fired-canary failure.

## Audit table

Classified as of ticket 429 landing. **Status** is one of: `substrate-correct` (already routed through a named gate), `bypass` (mutates Inventory inline outside any gate), or `primitive` (it IS the gate / primitive layer).

| Site | Kind | Status | Resolver / Trait | Feature |
| --- | --- | --- | --- | --- |
| `src/components/item_transfer.rs::transfer_item_stores_to_inventory` | Transfer | primitive | self | — (caller emits) |
| `src/components/item_transfer.rs::transfer_item_inventory_to_stored` | Transfer | primitive | self | — |
| `src/components/item_transfer.rs::transfer_item_inventory_to_ground` | Transfer | primitive | self | — |
| `src/components/item_transfer.rs::transfer_item_inventory_to_inventory` | Transfer | primitive | self | — |
| `src/components/item_gate.rs::ItemSource` (trait) | Source | primitive | self | per-impl `FEATURE` |
| `src/steps/disposition/handoff.rs::resolve_handoff` | Transfer | substrate-correct | function-shape | `ItemHandedOff` |
| `src/steps/disposition/feed_kitten.rs::resolve_feed_kitten` | Sink | substrate-correct | function-shape | `KittenFed` |
| `src/steps/disposition/cook.rs::resolve_cook` | Sink/Source | substrate-correct | function-shape | `FoodCooked` |
| `src/steps/disposition/deposit_at_stores.rs::resolve_deposit_at_stores` | Transfer | substrate-correct | function-shape | `FoodDeposited` |
| `src/steps/disposition/deposit_herbs_to_stores.rs` | Transfer | substrate-correct | function-shape | `HerbsDeposited` |
| `src/steps/disposition/drop_item.rs::resolve_drop_item` | Transfer | substrate-correct | function-shape | `ItemDropped` |
| `src/steps/disposition/load_drying_rack.rs`, `load_smoking_rack.rs` | Transfer | substrate-correct | function-shape | `FoodLoadedFor*` |
| `src/steps/disposition/craft_at_workshop.rs::resolve_craft_at_workshop` | Sink + Source | substrate-correct | function-shape | `ItemCrafted` |
| `src/steps/disposition/pick_up.rs::resolve_pick_up` | Transfer | substrate-correct | function-shape | `ItemPickedUp` |
| `src/steps/disposition/wear_item.rs::resolve_wear_item` | Transfer | substrate-correct | function-shape | `ItemWorn` |
| `src/steps/disposition/eat_at_stores.rs` | Sink | substrate-correct | function-shape | `FoodEaten` |
| `src/steps/disposition/trash_at_midden.rs` | Transfer | substrate-correct | function-shape | `ItemTrashed` |
| `src/steps/disposition/eat_from_own_inventory.rs::resolve_eat_from_own_inventory` | Sink | substrate-correct (new in 429) | function-shape (dispatched per-tick by `systems::needs::eat_from_inventory`; follow-on ticket will plumb a `StepKind::EatFromOwnInventory` GOAP step for adult-side L2/L3 election) | `EatFromOwnInventory` |
| `src/components/item_gate/sources/den_raid_carcass.rs::DenRaidCarcassSource` | Source | substrate-correct (new in 429) | `ItemSource` trait | `ItemSourcedFromDenRaid` |
| `src/components/item_gate/sources/hunt_catch.rs::HuntCatchSource` | Source | substrate-correct (new in 429) | `ItemSource` trait | `ItemSourcedFromHuntCatch` |
| `src/components/item_gate/sources/hunt_byproduct.rs::HuntByproductSource` | Source | substrate-correct (new in 429) | `ItemSource` trait | `ByproductSpawned` (reused — 1:1 by construction) |
| `src/components/item_gate/sources/forage_catch.rs::ForageCatchSource` | Source | substrate-correct (new in 429) | `ItemSource` trait | `ItemSourcedFromForageCatch` |

Pre-429 the four `bypass` sites were `src/systems/needs.rs::eat_from_inventory` (Sink), and the seven inline `inventory.pouch.push(...)` sites at `src/systems/disposition.rs:3234/3757/4196` + `src/systems/goap.rs:8837/9439/9476/9964` (Sources). 429 promotes each through the substrate layer.

## Behavior change at land-time (429)

The legacy disposition-chain hunt/forage paths (`src/systems/disposition.rs:3757` + `:4196`) used to **silently drop** the catch on inventory-full — pre-429, the `else { … }` arm was missing. Post-429, the `ItemSource` trait's default impl spawns the catch as a ground `Item` (matching the canonical GOAP-side behavior at `goap.rs:9439`/`:9964`). This eliminates two silent-drop sites and modestly increases ground-item density during sweeps where cats forage/hunt at capacity. Survival canaries hold; verified in the 429 soak.

## Cross-references

- [`src/components/item_transfer.rs`](../../src/components/item_transfer.rs) — Transfer primitive layer; the rustdoc names the type-level invariant. Mirror its three-layer enforcement framing.
- [`src/components/item_gate.rs`](../../src/components/item_gate.rs) — Source trait layer (429).
- [`src/steps/outcome.rs`](../../src/steps/outcome.rs) — `StepOutcome<W>` + `record_if_witnessed`; the witness-bound Feature emission contract that all three gates rely on.
- [`docs/systems/slot-inventory.md`](slot-inventory.md) — `Inventory` struct mechanics + wearable-slot layer.
- [`docs/systems/crafting.md`](crafting.md) — recipes, materials, identity-keyed effects.
- CLAUDE.md §"Substrate stubs are forbidden" — the same enforcement-strength doctrine the gate contract follows.
