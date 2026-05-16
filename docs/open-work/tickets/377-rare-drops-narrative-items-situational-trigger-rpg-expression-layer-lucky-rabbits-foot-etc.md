---
id: 377
title: rare drops & narrative items: situational-trigger rpg-expression layer (lucky rabbit's foot etc.)
status: blocked
cluster: items-crafting
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-16
parked: null
blocked-by: [375]
supersedes: []
related-systems: [crafting.md, the-calling.md, naming.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why
Separate from 375's guaranteed meat-and-N-tuple, this is the **RPG-expression layer**: situationally-triggered rare drops keyed to specific narrative conditions (full-moon mouse kill, corruption-tile rat kill, twilight rabbit kill, storm fish-catch, mid-flight bird strike, ShadowFox banishment). Each rare drop is per-cat narrative texture — the texture of *"this cat carried a rabbit's foot through the winter, and when the ShadowFox came, the foot snapped"* — kept honest by Clowder's `crafting.md` §Design constraint that items are **not stat sticks**. Effects live on resolvers that branch on item identity, not on numeric modifier fields.

## Scope
- Add 6 new `ItemKind` variants in `src/components/items.rs`: `MoonpawTail`, `PlagueTooth`, `LuckyRabbitsFoot`, `OpalScale`, `HeartfeatherPinion`, `ShadowFangSliver`. Each carries `name`, `origin_tick`, `creator_entity`, `narrative_template_id` per `slot-inventory.md` `WearableItem` / `CarriedItem` shape. No numeric fields.
- New resource `src/resources/rare_drops.rs` with `RareDropTrigger` table mapping `(species, situational-condition) → rare ItemKind`. Single source of truth, readable, tweakable.
- Extend `resolve_engage_prey` in `src/systems/goap.rs` (post-375 multi-item spawn) to check situational triggers against existing world state (tick mod season-length for moon phase, tile `Corruption` resource, schedule-edge from `time.rs`, weather state from `weather.rs`, hunt-strike trajectory from resolver witness). **No fresh RNG roll** — drops emerge from world-state coincidence, not from `roll_d20()`.
- New message `RareDropOccurred` (`#[derive(Message)]` per ECS rules) consumed by naming-substrate matcher (`naming.md`) and Calling pipeline (`the-calling.md`).
- Add inventory-read branches in: `escape_from_predator`, `hunt_strike`, `take_damage`, `set_ward` resolvers. Each read site named in the design table below; the inventory-read modifier MUST surface in L2 trace (per `feedback_audit_l3_disposition_mapping`), not as a hidden post-L2 bonus.
- New `docs/systems/rare-drops.md` design doc; cross-link from `crafting.md` (Phase 3 / Phase 5 specialty inputs) and `the-calling.md`.

### Drop catalogue (initial)

| Source | Rare ItemKind | Situational trigger | Resolver-level effect |
|---|---|---|---|
| Mouse | `MoonpawTail` | Kill during full-moon tick window | Emits named event (mythic-texture canary anchor). Carrier read in fate-axis perception. |
| Rat | `PlagueTooth` | Kill on corruption-affected tile | `SetWard` resolver branches → shadow-detection range +1 tile (resolver-level, not item-level). |
| Rabbit | `LuckyRabbitsFoot` | Kill at twilight schedule-edge | One-shot fate-escape: `escape_from_predator` consumes the item, triggers guaranteed escape from one ambush. Emits Named Event. |
| Fish | `OpalScale` | Catch during storm | Adornment routing: 370 Shell Collar accepts OpalScale variant; gains naming-substrate hook ("the Opal-Collared cat"). |
| Bird | `HeartfeatherPinion` | Bird struck mid-flight (high-skill hunt) | Hunt-skill resolver read: carrier gains kill-quality bonus on *next* hunt only, then consumes. Identity-keyed, not stackable. |
| ShadowFox | `ShadowFangSliver` | Successful banishment (not flee, not driven-off) | Ward ingredient (strong shadow-detection) + naming-substrate input (Named Event tied to colony defense lore). |

## Out of scope
- Guaranteed-tier byproducts → 375.
- Adding rare drops to **terrain** harvestables (376 is producer-only; rare terrain drops can be a follow-on if 376 reveals natural triggers).
- Combat-substrate ticket itself — this ticket adds the read sites, but a future ticket may compose them into a unified combat-substrate axis.

## Current state
Blocked on 375 (`engage_prey` must be extended for multi-item spawn first; this ticket piggy-backs by adding probability-gated extras keyed to situational triggers).

## Approach
1. Land 375 first; rebase against its `engage_prey` extension.
2. Build `RareDropTrigger` table + situational-condition checks. Triggers read existing world state; introduce no new RNG seed.
3. Add inventory-read branches one resolver at a time; verify each branch surfaces in L2 trace as a named modifier (e.g. `escape_from_predator: LuckyRabbitsFoot=present → branch=guaranteed_escape`).
4. Wire `RareDropOccurred` message through naming + Calling pipelines.
5. Author `rare-drops.md` design doc with situational-trigger table + Calling integration intent.

**Design pillars:**
- "Items are real" — rare drops are spatial entities with carrying-cost (slot-inventory). They participate in inventory pressure like any other item.
- "Substrate over hacks" — resolver-level branch on item identity is visible substrate. Hidden post-L2 modifiers (the side-channel anti-pattern 163 retires) are explicitly forbidden.
- "Richer perception, better strategy" — rare drops introduce orthogonal narrative axes (per-cat fate texture) distinct from 378's aggregate-economy demand axis. Conflating them would re-introduce the single-axis-perception anti-pattern.

## Verification
- `just scenario rare-drop-triggers`: six sub-scenarios, one per rare drop, presetting the exact situational trigger. Assert each rare ItemKind spawns iff trigger fires; assert `RareDropOccurred` message emitted; assert naming-substrate matcher receives it.
- `just soak-trace 42 Simba` + `just verdict logs/tuned-42`:
  - No spurious rare drops under normal play.
  - Rare drops appear at narratively plausible rates.
  - Inventory-read branches surface in L2 trace as named modifiers, not silent bonus.
  - Hard survival gates hold; mythic-texture canary likely *boosted* by rare-drop Named Events.

## Log
- 2026-05-16: opened. Plan: `~/.claude/plans/i-d-like-to-do-bright-coral.md`. Blocked on 375 (multi-item spawn extension).
