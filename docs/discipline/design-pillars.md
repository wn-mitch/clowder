# Design pillars

Four load-bearing rules that decide *which kind of fix is allowed* before parameter tuning is on the table. Each has at least one "ruined us" precedent. The pillar one-liners appear in CLAUDE.md; their expansion lives here.

---

## Pillar 1 — Items are real, and items have bite

Items are spatial world entities with real physical constraints — never abstract resources or invisible inventory. They carry *real mechanical effects* grounded in identity, material, and craftsmanship — the way a Borderlands manufacturer shapes a gun's character — never random-rolled affixes or decoupled `+5`-style stat sticks (the OSRS/PoE trap).

**Mechanism:** an item declares qualitative identity/material properties (369's `weapon_class` / `armor_class` / `noise_class` / `durability_tier` / `equip_material` classifiers, scaled by `quality`); a *uniform modifier-aggregation layer* composes the effects across everything a cat wears/carries; resolvers fetch the aggregate and apply it, and the modifier surfaces in the L2/resolver trace (never a hidden post-hoc bonus).

**Why** —
- **175** — Inventory.add succeeds before Stores.remove + despawn — an item is never both held and on the ground.
- **189** — carrying-cost is a load-bearing tradeoff, not tunable away.
- **193** — zone-mismatch defects surface *because* items occupy zones.
- **369** — identity→property classifiers.
- **476** — retired the cosmetic-only overcorrection that mistook "no boring stat-sticks" for "no effects."

**Apply:** declare the property on the item's identity/material classifier, compose it in the aggregation layer, fetch-and-apply in the resolver — never bolt a random or decoupled numeric field onto an item, never smear `match item.kind` effect-logic across individual resolvers.

**Doctrine:** [`docs/systems/crafting.md`](../systems/crafting.md), [`docs/systems/slot-inventory.md`](../systems/slot-inventory.md).

---

## Pillar 2 — Substrate over hacks

Prefer substrate-side levers (DSE axes, considerations, markers, eligibility filters, scoring shape visible in the L2 trace) over hidden side-channels (interrupts, overrides, gates, silent-advances, post-hoc modifier passes that mutate per-Action scores after L2 emit).

**Why** —
- **087 / 093 / 163** — made the antipattern visible and started retiring it.
- **091 / 111** — showed the failure mode of getting the sequencing wrong: partial substrate adoption or premature umbrella retirement collapses behavior during transition.

**Apply:** substrate axes land first, the corresponding hack retires second — never the reverse. If the L2 trace doesn't explain the choice, the encoding is wrong.

---

## Pillar 3 — Richer perception, better strategy

As cats understand their environment in good chunks — orthogonal axes that each encode a distinct situation, not a louder single alarm — they make more strategic decisions and welfare improves.

**Why** —
- **087 / 148** — substrate refactors that decomposed single-channel signals into orthogonal axes shifted behavior from blanket response to situation-appropriate.
- **181 iter-2** (memory `project_l3_patrol_absorption_cascade`) — the inverse: substrate that elevates an action without decomposing far enough to price its true cost produced the L3 patrol absorption cascade, where Patrol elevation exposed cats to ShadowFoxes and starved the colony 24k ticks later.

**Apply:**
- Prefer adding orthogonal axes over amplifying existing ones.
- Compose personality / phobias / ambient context at the modifier layer, never inside the underlying perception scalar (memory `feedback_single_axis_perception_scalars`).
- Welfare canaries must hold across any perception-layer change (`just verdict` vs baseline).

---

## Pillar 4 — Commitment is one mechanism, not two

Multi-tick decomposition (HTN sub-goal chains, plan templates, GOAP step lists, `HeldGoalStack` as frame state) is substrate — it encodes *how* a held Intention executes. But *which* Intention is held this tick belongs at §L2.10.6 softmax + §7.4 persistence-bonus, the spec's single commitment layer. Never wire a parallel commitment substrate (wrap-site override, frame-pin that discards the softmax winner, post-softmax priority-override) alongside it.

**Why** — the 364→397 kitten-arc cluster: 364's HTN frame-pin (`goap.rs:2410-2491`) + wrap-site override (`goap.rs:2733-2790`) accumulated four follow-on patches (394 R11, 395 R13 / yield rule, 397 lift / cooldown bypass / pin-guard) stacked on a §L2.10.6 deferral, until §7.M.2's `RaiseOffspringAspiration` was identified as the spec-mandated convergence. Pattern matches **087 / 093 / 163** (post-L2 score mutations) and **148** (single-channel signals decomposed into orthogonal axes), with the pillar-#3 perception-and-strategy payoff sitting on the substrate side.

**Apply:** when a multi-tick aspiration needs to compete with per-tick DSEs, emit it as an Intention into the unified softmax pool and hold via §7.4 persistence — never pin `chosen_action` to bypass softmax, never override the wrap-site emit. The HTN method registry (decomposition) is fine; the frame-pin (commitment) is not. If the L2 trace doesn't show the held Intention's persistence-bonus offset, the encoding is wrong.
