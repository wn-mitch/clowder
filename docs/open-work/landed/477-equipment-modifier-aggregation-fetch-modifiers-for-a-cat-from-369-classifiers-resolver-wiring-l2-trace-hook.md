---
id: 477
title: Equipment-modifier aggregation: fetch modifiers for a cat from 369 classifiers + resolver wiring + L2-trace hook
status: done
cluster: items-crafting
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-26
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 4701bc7b530c
landed-on: 2026-05-27
---

## Why
369 (warrior's kit) shipped the identity→property data layer — `ItemKind::weapon_class()`
/ `armor_class()` / `noise_class()` / `durability_tier()` / `equip_material()` in
`src/components/equipment.rs` — but **explicitly deferred the consumption layer.** 461 and
463 both name "resolver reads of material-property effects" as an un-opened follow-on. This
ticket is that follow-on: the uniform "fetch the relevant modifiers for this cat" seam so
items have real mechanical bite, applied in resolvers, **without** smearing `match item.kind`
effect-logic across every resolver and **without** bolting random/decoupled stat fields onto
items. It is the substrate that realizes the corrected "items have bite" pillar (see 476).

## Scope
- **Aggregation API** — new module `src/components/equipment_effects.rs` (sits atop
  `equipment.rs`, does not duplicate the classifiers). A uniform call (working name
  `equipment_modifiers_for(cat, &inventory, &wearables) -> EquipmentModifiers`) walks the
  cat's worn/carried items, reads the 369 classifiers scaled by `ItemSlot::quality`, and
  composes a typed aggregate. Composition precedent: `preservation_output_quality()` in
  `src/systems/preservation.rs`.
- **Resolver read sites** (named in 461/463): hunt-strike weapon-bonus; `take_damage`
  armor-reduction; ranged-attack sling-enable; movement/detection cloak-mask; noise-class
  detection-penalty. Each fetches the aggregate and applies it. No per-resolver identity
  matching.
- **`Feature::BoneWeaponSnapped` emitter** — 463 defers it here; snap gated on failed-strike
  + `DurabilityTier::Fragile`. Classify in `Feature::expected_to_fire_per_soak()`.
- **L2 / resolver-trace hook** — 377 + `feedback_audit_l3_disposition_mapping` require the
  inventory-read modifier to surface in the trace, not as a hidden post-L2 bonus. Today only
  the DSE layer has `ModifierPipeline::apply_with_trace` (see 163's §3.5.1 migration as the
  shape precedent); no resolver-level hook exists. This ticket builds it so a read like
  `take_damage: CuredHideBracers=present → armor_reduction=0.2` is legible in the trace.

## Out of scope
- Adornment social effects (017/370's mentorship-token fondness, courtship gift, naming) —
  a different axis; routed through the social/identity readers, not this combat-property
  aggregation.
- Rare-drop situational triggers and their inventory-read branches (377) — though 377 shares
  the trace hook this ticket builds and the `escape_from_predator` read site.
- Slot mechanics / `WearableSlots` (017).
- Doctrine doc edits (476).

## Current state
Blocked on **463** (`CraftItemAspiration` retires the resolver lex-pick and makes
warrior's-kit items actually exist in the seed-42 soak — the precondition for first-light
verifying that resolver reads fire organically). 461 (TanningFrame tuning) and the 369
classifiers are landed. The reads can be built and `just scenario`-tested against preset
inventory before 463, but soak first-light verification waits on 463.

## Approach
Build the aggregation module + the trace hook first (pure, unit-testable), then wire one
resolver read site at a time, verifying each surfaces in the L2 trace as a named modifier
before moving to the next. Effects derive from qualitative classifiers + `quality` only —
deterministic, no fresh RNG roll (mirrors 377's "world-state coincidence, not roll_d20"
discipline). See `docs/systems/crafting.md` Phase 2b / Material tiers for the intended
property→resolver mapping.

## Verification
- `just scenario` presetting cats with kit items: assert each resolver branch fires and
  surfaces in the trace as a named modifier (not a silent bonus).
- After 463: `just soak-trace 42 Simba` + `just verdict logs/tuned-42` — kit items exist
  organically, reads fire at plausible rates, `Feature::BoneWeaponSnapped` enrolled in the
  never-fired canary, hard survival gates hold.

## Related
- **369** — classifiers consumed (`src/components/equipment.rs`).
- **463** — unblocks (makes kit items exist; retires lex-pick).
- **334** — stealth-cloak consumes the cloak-mask read site + WearItem resolver.
- **377** — shares the resolver-trace hook + the `escape_from_predator` read site.
- **163** — §3.5.1 modifier-with-trace migration; shape precedent for the trace hook.

## Log
- 2026-05-26: opened as 369's deferred resolver-consumption layer (named out-of-scope in
  461 + 463). Blocked on 463. Companion to the 476 doctrine correction.
- 2026-05-27: 2026-05-27: landed foundation + 4 read sites (armor / weapon-strike+snap / cloak / noise-dormant). Hard gates pass (Starvation 0, ShadowFoxAmbush 0, never-fired 0, continuity grooming/play/mentoring/courtship all ok); verdict 'concern' is constants-drift only (new CombatConstants fields make the events.jsonl header non-comparable vs the pre-477 baseline). Opened 479 (ranged-attack mode for Sling, blocked-by 477).
