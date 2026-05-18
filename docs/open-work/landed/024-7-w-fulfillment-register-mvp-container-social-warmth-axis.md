---
id: 024
title: §7.W Fulfillment register — MVP container + social_warmth axis
status: done
cluster: ai-substrate
initiative: []
landed-at: fc7f5e9
landed-on: 2026-04-24
---

# §7.W Fulfillment register — MVP container + social_warmth axis

**Landed-at:** `fc7f5e9` (HEAD-reachable). The frontmatter recorded `47047261`; that was a hidden jj revision rewritten into the current commit during rebase. Bundled with ticket 012 (warmth split phase 3 was blocked on this MVP).

**Why:** Ticket 012 (warmth split) phase 3 was blocked on §7.W — the Fulfillment register specified in `docs/systems/ai-substrate-refactor.md` §7.W.1. No container component existed for fulfillment axes. Without it, `social_warmth` had nowhere to live and the warmth conflation (hearth-warmth drowning loneliness) persisted.

**Scope (MVP).** Minimum viable container that unblocks ticket 012 phase 3:

- `Fulfillment` component (`src/components/fulfillment.rs`) with `social_warmth` axis
- Per-tick decay system with isolation-accelerated drain
- Bond-proximity passive restoration
- Scoring-layer integration (`social_warmth_deficit` in `ctx_scalars`)
- Snapshot/event-log emission
- Constants in `SimConstants`
- Spawn-site and schedule registration (3 sites each)
- Unit + system tests

**Out of scope (deferred to follow-on tickets).** §7.W spec features that land later on top of the MVP container: Sensitization (per-axis positive-feedback loop) — corruption/compulsion content; Tolerance (diminishing per-unit yield) — pairs with sensitization; Source-diversity-modulated decay — requires multiple axes contributing; Mood integration (§7.W.2 losing-axis deficit → valence drop); Additional axes (spiritual, mastery, corruption-capture).

**Approach.** Flat struct matching the `Needs` pattern — one named field per axis. Restructured to enum-keyed map only when axis count justifies it. Design spec in `docs/systems/ai-substrate-refactor.md` §7.W.0–§7.W.8; warmth-split spec in `docs/systems/warmth-split.md`.

**Verification.** `just check` + `just test` pass. Seed-42 900s release soak: survival + continuity canaries hold. `social_warmth` appears in `CatSnapshot` events. Constants header includes new fulfillment fields.

## Folded-in subticket: §7.W Fulfillment register MVP + warmth split phase 3

**Landed-on:** 2026-04-24. Originally tracked as a separate file `landed/024-7-w-fulfillment-register-mvp-warmth-split-phase-3.md` claiming `id: 024`. Folded into the 024 parent during Linear migration prep.

**What shipped:**

- `Fulfillment` component (`src/components/fulfillment.rs`) with `social_warmth`
  axis — the §7.W container that gives cats a fulfillment register independent
  of the Maslow needs hierarchy.
- Per-tick decay system with isolation-accelerated drain (2.5× when no cats
  within range 3). Bond-proximity passive restoration for nearby bonded
  companions.
- Warmth split (ticket 012 phase 3): `groom_other` and `socialize` step
  resolvers now feed `social_warmth` (fulfillment register) instead of
  `needs.temperature`. A cat near a hearth can now be physically warm and
  socially starving — the conflation that drowned loneliness is resolved.
- Scoring integration: `social_warmth_deficit` wired into `ctx_scalars` for
  DSE consumption. Three DSE consideration files updated (`groom_other.rs`,
  `groom_self.rs`, `socialize.rs`).
- UI inspect bar for social_warmth in `cat_inspect.rs`.
- Snapshot/event-log emission of `social_warmth` in `CatSnapshot`.
- Narrative editor dashboard updated for the new field.
- Constants in `FulfillmentConstants` (`src/resources/sim_constants.rs`).
- Spawn-site registration (staggered initial values) and schedule registration
  at all 3 sites.
- 3 new unit tests for the socialize warmth inflow.

**Verification (seed 42, 900s release soak):**

- Survival canaries: starvation=0, shadowfox=0, footer written. ✓
- `never_fired_expected_positives`: 13 entries — pre-existing, unchanged from
  prior commit. Not a regression.
- Continuity: grooming=13. Other continuity canaries (play, mentoring, burial,
  courtship, mythic-texture) at 0 — pre-existing, tracked in balance backlog.
- GroomingFired: 13 events. Socializing disposition: 2024 snapshots.

**Deferred:**

- Ticket 012 phase 4 (balance retune) — hypothesis: removing social-grooming
  from temperature inflow may require drain-rate compensation. Deferred until
  substrate stabilizes per ticket 014 balance-tuning deferral policy.
- Sensitization, tolerance, source-diversity decay, mood integration,
  additional axes — all out-of-scope per ticket 024 §Out of scope.

