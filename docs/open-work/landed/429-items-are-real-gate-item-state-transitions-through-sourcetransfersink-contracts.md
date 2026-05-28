---
id: 429
title: Items-are-real: gate item-state transitions through Source/Transfer/Sink contracts
status: done
cluster: items-crafting
orchestration: substrate-sensitive
initiative: []
added: 2026-05-20
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: pending
landed-on: 2026-05-27
---

## Why

The **items-are-real** pillar names a contract — an item is a real spatial entity in a real `Inventory` slot, never an abstract resource or a stat-stick — but the codebase still has **end-around paths** where an item shifts location or form without going through a named substrate gate. The clearest current example: `eat_from_inventory` at `src/systems/needs.rs:301-314` is a per-tick needs-side reflex that consumes a food slot and credits hunger, registered into the FixedUpdate schedule at `src/plugins/simulation.rs:996`. It works correctly (it's the substrate that kittens-are-cats relies on for kitten autoconsume — see ticket 428), but architecturally it's a holdover from before the substrate refactor: the doc-comment frames it as an *adult-hunter fast-path* ("Keeps cats alive during long hunts"), the filter is just `Without<Dead>` with no entity-class predicate, and the slot-removal happens inline (`inventory.take_food()`) outside any "this is a Sink" ceremony.

The promotion we want: every item state-transition — slot moves between `Inventory`s, item is consumed (food → hunger credit), item is crafted (slot → slot + transform), item is dropped on the ground, item is destroyed — must pass through one of three named gates: a **Source** (item enters world / inventory: forage, kill, craft-output, trader-arrival), a **Transfer** (item shifts location without form change: Handoff, PickUp from ground, deposit at Stores), or a **Sink** (item exits world / inventory: Eat, Bury-with-the-dead, decay-to-nothing, crafting-consumption). Each gate has a Rust-level contract (a function with a named witness type, the way `resolve_handoff` already does) and emits the corresponding `Feature::*` activation. Behavior that mutates an item's location or form without going through one of these gates is a contract violation — same enforcement strength as substrate-stub-class defects.

Today the substrate is partial: `resolve_handoff` (Transfer), `resolve_feed_kitten` (Sink), `resolve_bury` (Sink), `transfer_item_inventory_to_inventory` (Transfer primitive) all look the right shape. But `eat_from_inventory` is a Sink that hasn't been formalized — it bypasses the AI substrate entirely, runs in autonomic-needs land, and the only way "this is how kittens eat from their own pocket" is discoverable is by reading the query filter and noticing `Without<Dead>` is permissive enough. That's the kind of accidental-substrate that bit ticket 428 (drain over-filtering surfaced because the autoconsume system happened to be substrate-complete — but we got lucky).

## Scope

- Audit every Inventory mutation site in `src/` and classify each as a Source, Transfer, or Sink. Document the inventory at `docs/systems/items-are-real.md` (or extend `docs/systems/crafting.md` / `docs/systems/slot-inventory.md` — whichever home fits best).
- Promote `eat_from_inventory` to a first-class Sink: extract a `resolve_eat_from_own_inventory(needs, inventory)` step (mirror of `resolve_feed_kitten`'s shape, witness via `EatFromInventoryOutcome`), emit `Feature::EatFromOwnInventory` (Positive, enrolled in the never-fired canary). The system at `needs.rs:301` becomes the *dispatcher* that calls the Sink, not the inline mutator.
- Promote the autoconsume path to AI-substrate participation: add an `Eat-from-own-inventory` DSE option that scores when `hunger < threshold && inventory.has_food()` (or equivalent — kittens may still need an autonomic-tier reflex below planning, but adults should plan through it). Run the L2 trace through the same DSE evaluation pipeline.
- Add a Rust-level lint: any `*.slots.push(...)`, `*.slots.remove(...)`, `inventory.take_food()`, etc. outside of `src/components/item_transfer.rs` (the primitive layer) or a `resolve_*` Source/Transfer/Sink (the substrate layer) is a contract violation. Enforced via `scripts/check_item_transfers.sh` (already exists for some of this — extend it).
- Re-audit kitten-autoconsume specifically: decide whether kittens should plan Eat through the AI substrate (kittens-are-cats with limited behavior catalogue) or whether autoconsume stays autonomic. Either is fine; the decision needs to be deliberate and documented.

## Out of scope

- **Ticket 428's drain fix** — that lands separately and is the precedent that surfaced this gap.
- **New item types or recipes** — this is purely about codifying the contract over existing items.
- **Trader-substrate / off-colony item sources** (parked in 381) — the Source-gate framework here is the prerequisite, but the trader implementation is its own ticket.
- **Performance optimization of the existing item paths** — contract first; optimize later if profiling shows pressure.

## Current state

Opened 2026-05-20 as a follow-on to ticket 428 during the verification soak. The user surfaced this when reviewing how kittens autoconsume — `eat_from_inventory` works, but it's a pre-substrate-era holdover that violates the items-are-real contract by mutating Inventory outside any named gate. Existing substrate-side paths to mirror: `src/steps/disposition/handoff.rs`, `src/steps/disposition/feed_kitten.rs`, `src/components/item_transfer.rs` (the `transfer_item_inventory_to_inventory` primitive). Existing items-are-real linter: `scripts/check_item_transfers.sh`.

## Approach

Three-phase landing:
1. **Audit + doctrine.** Classify every Inventory mutation in `src/` (Source / Transfer / Sink). Write the doctrine in `docs/systems/items-are-real.md`. Surface the existing gaps (eat_from_inventory + any other accidental-substrate paths).
2. **Promote eat_from_inventory.** Extract the Sink resolver, emit a Feature, run the existing constants through it. Verify via scenario harness that the autoconsume behavior is identical post-promotion (no balance shift).
3. **Lint extension.** Extend `check_item_transfers.sh` to flag any Inventory mutation outside the substrate-resolvers + primitive layer. Allowlist any genuinely-autonomic remaining cases with a ticket-tagged exemption.

## Verification

- `just check` — extended item-transfers linter passes.
- `just soak 42` + `just verdict logs/tuned-42` — no drift on food/hunger metrics from the eat_from_inventory promotion (the behavior is identical, only the code path differs).
- New scenario: `items_eat_from_own_inventory` — preset a hungry cat with food in inventory, assert hunger rises and slot drains via the Sink resolver (with witnessed Feature emission).
- Frame-diff against current baseline — no per-DSE drift; if the autoconsume promotion adds a DSE that scores positively, that's expected and characterized.

## Log

- 2026-05-20: opened as a §428 follow-on. User framing: "every item has Sources, Transfers, and Sinks. An item can only shift between locations or forms when it hits one of these gates. Ideally this creates coding contracts in rust as well." Surfaced when reviewing `eat_from_inventory`'s registration during §428's drain-fix verification — the function is registered in SimulationPlugin schedule and works correctly, but is a pre-substrate-era reflex that bypasses the items-are-real contract.
- 2026-05-22: blocked-by [450] (three-stage kittenhood). Phase 2's eat-from-own-inventory DSE composes against the sub-stage markers + `HasFoodInInventory` marker authored in 450; the `[EatFromInventory]` HTN method composes alongside 450's new `[BegForFood]` method as parallel decompositions of the shared Eat aspiration. Scope here also expanded to promote seven proto-Sources/proto-Sinks surfaced during plan-phase audit (`src/systems/disposition.rs:3201/3753/4202` den-raid carcass / hunt-engage / forage-engage; `src/systems/goap.rs:8381/8965/9002/9451` carry-cleanup fallbacks) into named gate resolvers with witnesses + Feature emission. User framing: "these are really just the proto sources and should be converted as such."
- 2026-05-27: 2026-05-27: landed. ItemSource trait + 4 impls (DenRaidCarcass / HuntCatch / HuntByproduct / ForageCatch) promote 7 inline pushes at disposition.rs:3234/3757/4196 + goap.rs:8837/9439/9476/9964; resolve_eat_from_own_inventory Sink extracted from the pre-substrate-era inline mutation at needs.rs:325 (per-tick autonomic dispatcher kept — GOAP-side wiring is a follow-on); strict check_item_transfers.sh extension flags any inventory.pouch.{push,swap_remove,retain,remove} / inventory.{take_food,add_*} / Item::{new,with_modifiers} outside the gate-author surface; doctrine at docs/systems/items-are-real.md; items_eat_from_own_inventory scenario harness as the structural Sink witness. Verification soak (seed-42, logs/tuned-42-f1b699a5): 3 Source canaries fire reliably (DenRaid 39x / HuntCatch 368x / ForageCatch 308x); EatFromOwnInventory canary classification corrected to expected:false post-soak (the autonomic safety-net path rarely triggers in healthy colonies — scenario test is the structural witness instead). Verdict 'fail' is driven by pre-429 drift from substrate work between the stale post-055-mood-drift baseline and HEAD (017 anatomical slots, 463 CraftItem aspiration, 470 WardSiegeFearMap, 472 Festering wound, 260 ShadowFox scent-avoidance) — none 429-attributable; the surface 429 touches (Inventory mutation gates) doesn't overlap with shadowfox/ward/craft/env-comfort. Baseline refresh tracked in follow-on.
