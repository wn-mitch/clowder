---
id: 024
title: §7.W Fulfillment register — MVP container + social_warmth axis
status: done
cluster: null
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
