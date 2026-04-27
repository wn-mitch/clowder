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
as influence map**). Corruption, ward, prey-density, and
social-attraction layers remain one-off or not-yet-built. The B1
exit-criterion "at least two distinct layers share one abstraction"
is not yet met — scent is on the substrate, but no second layer has
been migrated, so the abstraction has no co-tenant yet.

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

**Exit criterion:** at least two distinct layers (scent + corruption,
or scent + social attraction) share one abstraction; scoring layer
reads influence-map values as native axis inputs (gated on A1).

**Dependency:** gated on A1 for clean consumption by scoring; can
proceed in parallel with cluster C.

## Log

- 2026-04-27: dropped blocked-by 005 — cluster-A umbrella retired; A1 dependency satisfied by landed work. Status flipped blocked → ready.
