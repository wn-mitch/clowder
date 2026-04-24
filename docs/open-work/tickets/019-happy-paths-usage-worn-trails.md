---
id: 019
title: Happy paths — usage-worn trails
status: blocked
cluster: null
added: 2026-04-22
parked: null
blocked-by: [020]
supersedes: []
related-systems: [paths.md]
related-balance: []
landed-at: null
landed-on: null
---

**Why it matters:** Cats concentrate movement between high-utility
destinations; repeated traversal compresses terrain into speed-boosted
trails, and prey learn to avoid them (ecology of fear extended to
traffic). Worn enough, paths become a **civilizational marker** — the
colony writing its own behavioral history into the world as physical
grain. Path segments register with the `naming.md` substrate for
event-anchored naming ("The Last Trace of Cedar"), turning routine
geography into named ground.

**Design captured at:** `docs/systems/paths.md` (Aspirational,
2026-04-22).

**Score:** V=4 F=5 R=3 C=4 H=2 = **480** — "worthwhile; plan
carefully" (300–1000 bucket).

**Substrate reuse (the cost-saver):** path wear is additive to the
`InfluenceMap` scaffolding (`src/systems/influence_map.rs` §5.6.9 —
`(Channel, Faction)`-keyed registry; "14th map" is a registration,
not a schema change). Naming rides on #20 (see below), which is a
precursor.

**Scope discipline (load-bearing — keeps H=2):**
1. Wear decays. No permanent tiles absent ongoing use.
2. Speed boost ≤1.25×, non-stacking. No runaway.
3. Prey avoidance is proportional, not binary.
4. Max 6 named segments per sim-year (name-spam guardrail, lives in
   `naming.md`'s shared ceiling).
5. Paths don't gate hunt scoring.

**Open scope questions (paths-local):**
1. Anti-monopoly threshold (placeholder 15% pending Phase 1
   observation).
2. Whether foxes / prey benefit from path speed-boost (default: no).

**Canaries to ship in same PR (4 total):**
1. Path-formation — ≥1 trail segment ≥6 tiles persists day 90→180.
2. Anti-monopoly — no single tile > 15% seasonal colony traversal.
3. Named-landmark — ≥1 named path per sim-year, independent of
   Calling.
4. Name-spam — ≤6 named segments per sim-year (shared with #20).

**Dependencies:** soft-gated on tilemap rendering stability. Named-
path output soft-depends on #20 (naming substrate). No A1 hard-gate
(pathfinding weight is below the GOAP layer). Added as rank 4a in
`docs/systems-backlog-ranking.md`.

**Shadowfox watch:** shares self-reinforcing feedback loop and new
prey-fear input with shadowfoxes; decisive differences (no mortality-
spike failure mode, continuous not Poisson, canaries are formation-
quality not survival) keep H=2 not H=1. Scope disciplines 1, 2, and 5
are the brakes.

**Resume when:** tilemap rendering stable; #20 naming substrate has
shipped or is shipping in the same PR.
