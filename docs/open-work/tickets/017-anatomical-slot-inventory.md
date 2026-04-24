---
id: 017
title: Anatomical slot inventory
status: blocked
cluster: null
added: 2026-04-22
parked: null
blocked-by: [016]
supersedes: []
related-systems: [slot-inventory.md]
related-balance: []
landed-at: null
landed-on: null
---

**Why it matters:** Split-out piece of the 2026-04-22 composite
proposal. Refactors the flat `Inventory { slots: Vec<ItemSlot> }`
(`src/components/magic.rs:242`) into an anatomy-indexed wearable-slot
structure plus a stackable consumable-pouch. Anatomical slot
enumeration imports from `body-zones.md`.

**Design captured at:** `docs/systems/slot-inventory.md`
(Aspirational, 2026-04-22).

**Score:** V=2 F=3 R=4 C=4 H=4 = **384** — "worthwhile; plan
carefully" (300–1000 bucket). Added as rank 5 in
`docs/systems-backlog-ranking.md`.

**Ship-order note: do not ship standalone.** Score reflects
isolated-feature value, but lived utility is gated on at least one
wearable producer existing. Candidate producers, thesis-fit
ordered:
1. `crafting.md` Phase 3 (mentorship tokens, heirlooms) — see #16.
2. `the-calling.md` (Named Objects as wearable hooks).
3. `trade.md` (visitor-sourced worn objects).

Without a producer this is cost without benefit.

**Type guardrail (load-bearing invariant):** `WearableItem` carries
`name`, `origin_tick`, `creator_entity`, `narrative_template_id`
only. No numeric capability modifiers. If a future PR adds modifier
fields to the wearable type, F drops 3→2 and H drops 4→2 (composite
falls from 384 to ~96) — treat such PRs as re-opening this ranking.

**Dependencies:** hard-gated on a producer; otherwise migration is
mechanical over a known finite consumer set (5–6 call sites:
`persistence.rs`, `plugins/setup.rs`, `components/task_chain.rs`,
`systems/needs.rs::eat_from_inventory`, relevant `magic.rs` sites).

**Resume when:** #16 reaches Phase 3, or `the-calling.md` lands
with Named Objects surfacing as wearable candidates. Do not pick
up before either.
