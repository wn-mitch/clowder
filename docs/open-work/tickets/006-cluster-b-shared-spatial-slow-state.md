---
id: 006
title: Shared spatial slow-state (Cluster B)
status: ready
cluster: B
added: 2026-04-20
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, refactor-plan.md, scoring-layer-second-order.md]
related-balance: []
landed-at: null
landed-on: null
---

**Why this is a cluster:** scoring layer currently lacks uniform access
to shared, spatially-varying state. The scent map in `wind.rs` +
`sensing.rs` is a one-off; corruption, wards, predator danger, prey
opportunity, and social attraction are each implemented differently
or recomputed per query. Influence maps (Mark, Dahlberg) are the
canonical form of this abstraction.

### B1. Generalize influence maps

**Why it matters:** Influence maps are the spatial form of
"consideration inputs shared across actions" — exactly what
`docs/balance/scoring-layer-second-order.md` framing #1 identifies as
missing. Generalizing to a uniform system would (a) standardize how
spatial considerations are consumed by scoring, (b) give
pair-stickiness a natural home (social attraction field pulls bonded
cats together), (c) give the strategist-coordinator a spatial
substrate.

**Current state:** Phase 2B landed the generalized influence-map
substrate + migrated scent onto it (see Landed: **Phase 2B — Scent
as influence map**). Ticket 048 added `CarcassScentMap`
(§5.6.3 #6, substrate-only — consumer cutover deferred). Ticket 045
added `WardCoverageMap` (§5.6.3 #3 ward-coverage view; full
ward-strength promotion still deferred). Three live layers now share
the abstraction, so the original "≥2 layers" exit criterion is met.

**Re-scoped exit criterion (2026-04-27 audit):** all §5.6.3 rows
landed, deferred to a successor feature with an open ticket, or
explicitly out-of-scope. The `≥2 layers` bar was a refactor-pre-flight
proof-of-substrate; the real work is completing the spec catalog so
DSEs that want spatial inputs aren't silently degraded.

### §5.6.3 absent-map checklist

Eight rows from the §5.6.3 catalog with current status:

- [x] **#6 carcass scent** — *landed substrate* (ticket 048,
  `CarcassScentMap`). Consumer cutover at `goap.rs:1133–1145` still
  uses per-pair `observer_smells_at`; balance-affecting follow-on.
- [~] **#3 ward strength** — *partial* (ticket 045 added
  `WardCoverageMap` for placement scoring). Full §5.6.3 #3 promotion
  to a first-class spatial axis in scoring is still deferred per
  ticket 048's landing log.
- [~] **#5 prey-species split** — *partial*. `PreyScentMap` lives on
  the substrate; per-prey-species split (`mouse_scent`,
  `rabbit_scent`, `bird_scent` separated) deferred per ticket 048's
  landing log.
- [ ] **#4 food-location** — *absent*. Wanted by Forage / Eat for
  stockpile-aware scoring; today proxies via inline iteration.
- [ ] **#7 herb-location** — *absent*. Wanted by herbcraft DSEs;
  today proxies via per-pair sensing.
- [ ] **#8 construction site** — *absent*. Wanted by Build target
  ranking; today proxies via construction-component iteration.
- [ ] **#9 garden / crop** — *absent*. Wanted by Tend / Harvest
  spatial routing.
- [ ] **#13 kitten-urgency** — *absent*. Wanted by Caretake target
  ranking; today proxies via per-kitten lookup.

Each row's "wanted by DSE X" column is also gated on ticket 052
(§L2.10.7 plan-cost feedback) — `SpatialConsideration` is the
consumer-side substrate that reads these maps. Order: 052 lands the
consumer surface; this ticket lands the producer maps; per-DSE
cutover is the join.

**Touch points:**
- `src/systems/wind.rs` + `sensing.rs` — scent already migrated to
  the influence-map substrate (Phase 2B).
- `src/systems/magic.rs` — corruption field, ward field (still
  ad-hoc; candidate next migrations).
- `src/systems/prey.rs` — prey density (already sort of an influence
  map; candidate next migration).
- `src/ai/considerations.rs::SpatialConsideration` — consumer side
  wired in Phase 3a; the evaluator samples maps via `MapKey` lookup.

**Preparation reading:**
- **"Modular Tactical Influence Maps"** — Dave Mark, *Game AI Pro 2*
  ch. 30, free PDF at
  <http://www.gameaipro.com/GameAIPro2/GameAIPro2_Chapter30_Modular_Tactical_Influence_Maps.pdf>
  — THE definitive written reference; read first
- "Lay of the Land: Smarter AI Through Influence Maps" (Dave Mark,
  GDC 2014, GDC Vault) — the original pure-influence-maps talk
- "Spatial Knowledge Representation through Modular Scalable
  Influence Maps" (Dave Mark, GDC 2018, GDC Vault) — most recent
  full treatment, best on implementation details
- *Already watched:* "Building a Better Centaur" (GDC 2015) — fusion
  of utility AI + influence maps at scale; the architectural move
  this task implements
- Nick Mercer Unity reference implementation:
  <https://github.com/NickMercer/InfluenceMap>

**Exit criterion (re-scoped 2026-04-27):** every §5.6.3 row above is
either landed, has an open successor ticket, or is marked explicitly
out-of-scope; scoring layer reads influence-map values as native axis
inputs through `SpatialConsideration` (ticket 052) where the row's
DSE consumer wants distance-shaped scoring.

**Dependency:** gated on A1 for clean consumption by scoring (now
satisfied); can proceed in parallel with cluster C. Landing the
producer maps before ticket 052's consumer cutover is fine —
`SpatialConsideration` will read whichever maps exist when it lands.

## Log

- 2026-04-27: dropped blocked-by 005 — cluster-A umbrella retired; A1 dependency satisfied by landed work. Status flipped blocked → ready.
- 2026-04-27: re-scoped per substrate-refactor audit. Original "≥2 layers share an abstraction" exit criterion was already met (scent + carcass + ward-coverage), but the spec's §5.6.3 row catalog had 8 absent rows that no ticket enumerated. Promoted the exit criterion to "all §5.6.3 rows landed/deferred-with-ticket/out-of-scope" and added the absent-map checklist above. No status flip; ticket stays `ready`.
