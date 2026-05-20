---
id: 431
title: Top-10 hot-frame remediation — passive_familiarity (64% CPU) + per-tick discipline event-driven sweep
status: in-progress
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-05-20
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

The first end-to-end flamegraph run on this codebase (2026-05-20, samply against the post-428 binary, 60s soak seed 42, 59,813 samples @ 997 Hz) surfaced a strikingly skewed CPU profile: **one system, `passive_familiarity`, consumes 64.43% of inclusive CPU time**, and its descendant `BTreeMap::entry` consumes 26.20% standalone. The next-hottest is `evaluate_and_plan` (the L2/L3 planner) at 24.37% inclusive. Together the top two systems eat ~89% of CPU; the remaining 25+ systems share the leftover ~11%. The §428 fix (`resolve_goap_plans` populate) registers at 1.88% inclusive — i.e. effectively free.

The doctrine this ticket codifies: **true per-tick actions should be relatively rare outside the top-level loops.** Most per-tick systems today are *modeling* per-tick work (continuous accumulation, decay, sense-pass) when the underlying state-change is event-driven. A bond's familiarity doesn't change every tick; it changes when two cats come into range — Bevy messages can carry that signal. A cat's relationship list isn't recomputed every tick from scratch — it changes only when a relationship is modified; a cached per-cat sum invalidated on mutation events would beat the per-tick `all_for` enumeration. The current per-tick discipline is "iterate everyone every tick"; the substrate already has Messages (`#[derive(Message)]`, `MessageWriter` / `MessageReader`) but few hot paths use them as cache-invalidation triggers.

This ticket is the **catalog + classification pass** for the top-10 hot CPU consumers. For each: identify the work it does, whether it's structurally per-tick or could be event-driven / cached, and the most plausible substrate that would reduce it. Implementation lands in follow-on tickets per item — this one is the strategic survey, the way `clusters.md` is for ticket taxonomy.

## Scope

Implement the five catalog fixes as six independently-verified stages (one commit per stage with a byte-identical `_footer` determinism gate before the next begins). Stages are ordered so each one's substrate becomes available to the next consumer:

- **Stage A** — author `CatMoved { entity, from, to }` Bevy `Message`; emit from every Position-mutation site under `src/steps/`; no consumer yet. Substrate-only verification.
- **Stage B** — `NearPairCache` resource (`BTreeMap<(Entity,Entity), i32>`, key in `(min, max)` order to preserve the existing `for i / for j>i` float-add order); `update_near_pair_cache` system reads `MessageReader<CatMoved>` and incrementally updates the cache; `passive_familiarity` iterates the cache instead of running an O(N²) sweep. **Expected CPU drop: 64.43% → < 5% inclusive.**
- **Stage C** — `MapTileChanged` Bevy `Message` + `RouteCostCache` resource keyed by `(Entity, GoalZoneId)`. Invalidate per-cat on `CatMoved`; coarse drop on `MapTileChanged` (refine later if measured). Consumer: `evaluate_and_plan` at [`src/systems/goap.rs:1301`](src/systems/goap.rs) via `cache.get_or_compute(cat, goal_zone)`. Addresses catalog rows #4 (`flood_dijkstra`) + #10 (`find_full_path`).
- **Stage D** — `RelationshipChanged { a, b }` Bevy `Message` emitted from every `Relationships::modify_*`. Per-entity `all_for_cache: BTreeMap<Entity, Vec<(Entity, Relationship)>>` invalidated on the message. Consumer: `emit_cat_snapshots` at [`src/systems/snapshot.rs:98-99`](src/systems/snapshot.rs). Addresses catalog row #5.
- **Stage E** — `ctx_scalars` HashMap arena (mirror ticket 427's pattern). Site: [`src/ai/scoring.rs:700-701`](src/ai/scoring.rs); caller `score_dse_by_id` at line ~1435. Pass a caller-owned `&mut HashMap<&'static str, f32>` cleared between calls to retire the per-DSE rehash cost. Addresses catalog row #9.
- **Stage F** — L1 marker-snapshot hoist (catalog #3 partial). Build `ColonyMarkerSnapshot` once per FixedUpdate in a new `populate_colony_marker_snapshot` system; per-cat `MarkerSnapshot::new()` at [`goap.rs:1480`](src/systems/goap.rs) becomes a thin per-cat overlay. **Decision deferred to end of Stage E**: if overlap with ticket 432's `WorldSnapshots` becomes meaningful, fold this stage into 432 rather than open a parallel substrate.
- **Stage G** — CLAUDE.md "ECS rules" doctrine update: codify *default to event-driven, justify per-tick* (the legitimate per-tick categories: plan execution, sense+score, time-dependent decay, movement/physics; everything else should be Bevy `Message` + cached state). Capture post-431 flamegraph as `docs/diagnostics/baseline-profiles/2026-05-20-post-431/` for future regression comparison.

Each stage's determinism gate is non-optional: `just soak-trace 42 Mallow` before + after; `_footer` lines must be byte-identical. The catalog (the original open-time investigation) is preserved verbatim below as a record of the diagnostic pass that motivated each stage.

## Out of scope

- **Bench harness or CI perf gates** — different ticket; this is local-diagnosis-driven.
- **Optimizing systems below the top 10** — the long tail is < 1% each; premature.
- **Ticket 432's `WorldSnapshots` cross-system dedupe** — Stage F may coordinate with it but cross-planner `kitten_snapshot` / `cat_positions` stays in 432.
- **Ticket 205's `CatSpatialIndex`** — 431 builds its own pair-set cache (`NearPairCache`) without a general spatial index. If 205 lands later, `NearPairCache` becomes a thin consumer of it.
- **`check_bonds` retire** (catalog honorable mention) — runs once per `bond_check_interval`, not per tick; deferred.
- **`joint_intention::author_joint_intentions`** (honorable mention 0.68% inclusive) — could fire on an `IntentionEmitted` message instead of per-tick; deferred.

## Current state

Opened 2026-05-20 as a §428 follow-on. The investigation surfaced when verifying §428's drain fix didn't cause the -14.6% wall-clock tick-rate drop — the flamegraph confirmed §428 was 1.88% (effectively free), and the *actual* CPU consumers came into focus. The profile is captured at `/tmp/samply-428-sym.json.gz` (790KB) + `.syms.json` (sidecar). Ticket 430 wires the recipe; this ticket interprets the output.

**2026-05-20 reopened.** Original landing as a "catalog-only" artifact violated the *antipattern follow-ups are non-optional* doctrine in CLAUDE.md. Scope shifted from "name the work" to "implement all five fixes against six independent seed-determinism gates" — see Stages A–G above. The catalog block below is the open-time investigation record, preserved verbatim because each row maps to a stage.

## The catalog (top 10 hot frames, 2026-05-20 profile)

Self % = where samples land directly. Incl % = anywhere in the call stack.

### 1. `clowder::systems::social::passive_familiarity` — Self 37.67% / Incl 64.43%

**What:** Nested `O(N²)` loop over all live cats; for each pair within `passive_familiarity_range`, calls `relationships.modify_familiarity(a, b, delta)`. With 10 cats that's 45 pairs/tick; the BTreeMap entry is the inner cost. [`src/systems/social.rs:22-38`](src/systems/social.rs)

**Per-tick?** Currently yes — runs every FixedUpdate iteration. The model: "every tick, near cats get a tiny familiarity bump."

**Cache-friendly?** Partially. The *pair-set* (which cats are near which) changes only when a cat moves. The *familiarity value* changes continuously while cats remain near each other. So:
- Pair-set: rebuild only on `CatMoved` message (per-cat movement event); cache the spatial near-pair list in a resource.
- Familiarity delta: still applied per-tick, but only over the cached near-pair set (small — typically a handful of pairs in close range, not the full N² scan).

**Bevy-message candidate:** YES. `CatMoved { entity, from, to }` → spatial index update → near-pair set update. The 26% `BTreeMap::entry` cost shrinks because we apply delta only to the small near-pair set, not on every (i, j) pair pass.

**Seed-determinism constraint:** Modifications to `Relationships` happen on a fixed iteration order today (i < j); near-pair caching must preserve that order. Iterate the cached set in sorted-by-(min_entity, max_entity) order, identical to the current i-loop's order.

**Recommended follow-on:** *Spatial-index-cached passive_familiarity (event-driven invalidation on CatMoved)*. Cluster: `social-coordination`. Likely 30-50% total CPU reduction.

### 2. `alloc::collections::btree::map::BTreeMap<K,V,A>::entry` — Self 26.20% / Incl 26.20%

**What:** The inner-loop cost of `passive_familiarity` (descendant of #1; same line). [`src/resources/relationships.rs:91-93`](src/resources/relationships.rs)

**Per-tick?** Yes, via #1.

**Cache-friendly?** Already discussed in #1.

**Bevy-message candidate:** Same as #1.

**Seed-determinism constraint:** **CRITICAL** — `Relationships` uses `BTreeMap` (not `HashMap`) deliberately. The doc-comment at [`relationships.rs:55-63`](src/resources/relationships.rs) names a 1-ULP drift in `social_weight` across same-seed runs when `HashMap` was tried. Any optimization here MUST preserve deterministic iteration order. Options: keep BTreeMap but reduce call frequency (per #1), or switch to `IndexMap` (insertion-ordered, deterministic, O(1) hash lookup), or build a parallel HashMap for hot lookups + keep BTreeMap for `all_for` enumerations that the coordinator sums over.

**Recommended follow-on:** Folded into #1's follow-on (reducing call frequency dominates; the data-structure swap is a secondary lever).

### 3. `clowder::systems::goap::evaluate_and_plan` — Self 18.58% / Incl 24.37%

**What:** The L1/L2/L3 planner — per-cat marker-snapshot build, DSE scoring, softmax election, plan synthesis. Calls `flood_dijkstra` (the largest descendant). [`src/systems/goap.rs:1301`](src/systems/goap.rs)

**Per-tick?** Yes — every cat with a `GoapPlan` (or seeking one) re-evaluates each tick.

**Cache-friendly?** Partially.
- The marker-snapshot construction is per-tick work that depends on colony-wide state — could be hoisted to once-per-tick rather than once-per-cat. (Most markers are colony-scoped; per-cat marker reads are already structured this way, but the cold-start cost is duplicated.)
- Per-cat DSE scoring is genuinely per-tick (scores depend on per-tick interoception).
- Routing decisions inside the planner could cache per-cat path costs across ticks until movement events invalidate them.

**Bevy-message candidate:** Partial. The path-cost cache is the strongest lever (see #4). The DSE scoring itself is genuinely per-tick.

**Seed-determinism constraint:** Yes — softmax pool order is load-bearing for tie-breaks.

**Recommended follow-on:** Two children — *L1 marker-snapshot once-per-tick* (cluster: `ai-substrate`), and *Per-cat path-cost cache invalidated on CatMoved* (cluster: `ai-substrate`, related to #4).

### 4. `clowder::ai::route_cost::flood_dijkstra` — Self 2.95% / Incl 3.52%

**What:** Dijkstra flood-fill for path-cost field building, called from `evaluate_and_plan` for route scoring. [`src/ai/route_cost.rs`](src/ai/route_cost.rs)

**Per-tick?** Yes — called per-replan / per-DSE eval.

**Cache-friendly?** Strongly. A cat at position P with the same goal G and the same world state has the same cost field. Invalidate on:
- `CatMoved` for the cat itself (path origin changed)
- `MapTileChanged` for the tile-cost overlays (terrain became corrupted, ward placed, etc.)
- `WildlifeMoved` for scent overlays

**Bevy-message candidate:** YES. Cache the `RouteCostField` per (cat, goal_zone) until invalidation. Hit rates should be high — cats often re-evaluate while standing still or with the world unchanged.

**Seed-determinism constraint:** Yes — but cache hits return the same field, so determinism preserved.

**Recommended follow-on:** *Per-cat path-cost cache with event-driven invalidation*. Cluster: `ai-substrate`. Companion to #3's L1-snapshot-once.

### 5. `<alloc::vec::Vec<T> as ...SpecFromIter<T,I>>::from_iter` — Self 1.47% / Incl 2.36%

**What:** Building Vecs from iterators. Top parent is `Relationships::all_for` (#14 below). [`src/resources/relationships.rs:113+`](src/resources/relationships.rs)

**Per-tick?** Yes — `all_for(entity)` returns a fresh `Vec<(Entity, &Relationship)>` per call.

**Cache-friendly?** Yes. The relationships involving entity E change only when `modify_*` is called on a pair containing E. Cache `all_for(entity)` results in a per-entity `Vec`, invalidate on `RelationshipChanged { a, b }` message emitted by `modify_*`.

**Bevy-message candidate:** YES. Add `RelationshipChanged` message; subscribe `emit_cat_snapshots` (the top consumer of `all_for`) to a cached version.

**Seed-determinism constraint:** Cache key is `Entity`; iteration order over relationships still comes from BTreeMap, so determinism preserved.

**Recommended follow-on:** *Cache `Relationships::all_for` per entity; invalidate on RelationshipChanged*. Cluster: `social-coordination`.

### 6. `core::hash::BuildHasher::hash_one` — Self 0.81% / Incl 2.32%

**What:** SipHasher hashing. Called from `HashMap::insert` (top parent `RawTable::reserve_rehash`). Cost driven by HashMap growth + rehash, not steady-state insert.

**Per-tick?** Yes (transitively, via #9).

**Cache-friendly?** Indirectly — fix the HashMap that's growing/rehashing (see #9).

**Bevy-message candidate:** No (low-level primitive).

**Recommended follow-on:** Folded into #9.

### 7. `clowder::systems::goap::resolve_goap_plans` — Self 0.03% / Incl 1.88%

**What:** GOAP step resolver dispatch — runs `dispatch_step_action` for each cat with a plan. The system §428 modified. [`src/systems/goap.rs:3427`](src/systems/goap.rs)

**Per-tick?** Yes — genuinely per-tick (plan execution must advance per-tick).

**Cache-friendly?** No — this IS the step-by-step execution loop. Per-tick is correct.

**Bevy-message candidate:** No — execution side, not modeling side.

**Recommended follow-on:** None. This is correctly per-tick.

### 8. `clowder::systems::goap::dispatch_step_action` — Self 0.01% / Incl 1.65%

**What:** Match dispatch over `GoapActionKind` to the right resolver. [`src/systems/goap.rs:5131`](src/systems/goap.rs)

**Per-tick?** Yes via #7.

**Cache-friendly?** No — dispatch by enum variant.

**Recommended follow-on:** None.

### 9. `hashbrown::map::HashMap<K,V,S,A>::insert` — Self 0.28% / Incl 1.63%

**What:** HashMap inserts. Top parent is `score_dse_by_id` (#12) — scoring inserts into a per-cat scratchpad. The top descendant is `RawTable::reserve_rehash` (#18), meaning the HashMap is growing across calls.

**Per-tick?** Yes via #11/#12.

**Cache-friendly?** YES — the scratchpad should be pre-sized. Currently it grows from empty each call, triggering rehashes. Pre-size to known max (DSE count) and reuse the allocation. Same pattern ticket 427 used for the per-tick allocation hotspots.

**Bevy-message candidate:** No (allocation pattern, not modeling).

**Recommended follow-on:** *Pre-size DSE scratchpad HashMap to retire rehash cost*. Cluster: `ai-substrate`. Cross-reference ticket 427 for the pattern.

### 10. `clowder::ai::route_cost::CatPathPlan::find_full_path` — Self 0.10% / Incl 1.62%

**What:** Full pathfinding pipeline. Called from `dispatch_step_action`. [`src/ai/route_cost.rs`](src/ai/route_cost.rs)

**Per-tick?** Yes when a cat is mid-step.

**Cache-friendly?** Partially — same lever as #4 (path-cost field cache).

**Recommended follow-on:** Folded into #4.

## Honorable mentions (positions 11-25, not top-10 but worth noting)

- **`clowder::systems::snapshot::emit_cat_snapshots`** (Incl 1.54%) — per-tick logging system. Writes a snapshot per cat per tick to events.jsonl. Consumes `Relationships::all_for` (#14, our #5). If we add `--snapshot-interval N` (already exists), this drops linearly with interval.
- **`clowder::resources::relationships::Relationships::all_for`** (Incl 1.52%) — see #5.
- **`clowder::systems::social::check_bonds`** (Incl 1.17%) — bond-formation system. Per-tick; could be `RelationshipChanged` driven.
- **`clowder::ai::joint_intention::author_joint_intentions`** (Incl 0.68%) — per-tick scan for joint-intention candidates. Could fire on `IntentionEmitted` message instead of per-tick.

## The doctrine to codify

The "true per-tick actions should be relatively rare outside top-level loops" rule. Concretely, the legitimate per-tick systems are:
- **Plan execution** (`resolve_goap_plans`, `dispatch_step_action`) — the step machine MUST advance per tick.
- **Sense and score** (`evaluate_and_plan`, DSE scoring) — needs are continuous; scores must reflect current state.
- **Time-dependent decay** (`decay_fulfillment`, hunger drift) — physical reality of time passing.
- **Movement/physics** — cats step toward goals.

Everything else should be event-driven:
- **State accumulation** (familiarity, bond strength) — fires on co-presence event, not on every tick.
- **Lookup/query results** (`all_for`, marker presence) — cached, invalidated on the mutation event.
- **Spatial queries** (near-pair sets, path-cost fields) — cached, invalidated on `CatMoved` / `MapTileChanged`.
- **Aggregations** (per-cat relationship sums, coordinator weights) — cached, invalidated on the underlying mutation.

The Bevy 0.18 substrate already supports this: `#[derive(Message)]` + `MessageReader<T>` + `MessageWriter<T>` (per CLAUDE.md's "ECS rules" section). The doctrine to add to that section: **default to event-driven, justify per-tick.**

## Verification

- For each top-10 follow-on landed: re-run flamegraph against the same seed; the targeted hot frame should drop substantially (e.g., #1 + #2 combined should drop from 64.43% inclusive to < 20%).
- Soak `just verdict` continues to pass (no behavioral regressions from caching strategies).
- Seed determinism gate: same-seed soaks before/after produce identical `_footer` snapshots (the Relationships seed-determinism note is the cautionary tale).
- Updated CLAUDE.md "ECS rules" section reviewed for the per-tick discipline doctrine.

## Log

- 2026-05-20: opened as a §428 follow-on. The first end-to-end flamegraph on this codebase surfaced `passive_familiarity` at 64.43% inclusive CPU — a striking single-system concentration. Cataloged top 10 hot frames with their per-tick / event-driven characterization. Doctrine to codify: per-tick is for execution + time-dependent physics; everything else should be message-driven with cached state. The Relationships BTreeMap seed-determinism constraint is the load-bearing nuance — naive HashMap swap is forbidden.
- 2026-05-20: catalog delivered as the audit table in this ticket body; baseline profile archived at `docs/diagnostics/baseline-profiles/2026-05-20-post-428/profile.json.gz` (+ `.syms.json` sidecar). The remaining Scope items — opening per-item follow-on tickets (passive_familiarity cache, path-cost cache, Relationships::all_for cache, DSE scratchpad pre-sizing) and updating CLAUDE.md "ECS rules" with the event-driven doctrine — are deferred to a future session; this ticket lands with the audit + baseline as the durable artifact, so any follow-on perf work has a snapshot to compare against. The doctrine memory is saved cross-session at `~/.claude/projects/-Users-will-mitchell-clowder/memory/project_per_tick_discipline_default_event_driven.md`.
- 2026-05-20: Catalog complete: passive_familiarity 64.43%, evaluate_and_plan 24.37%, flood_dijkstra 3.52%, then a long tail under 2%. Baseline samply profile archived at docs/diagnostics/baseline-profiles/2026-05-20-post-428/. Doctrine memory saved cross-session. Per-item follow-on tickets (cache + event-driven invalidation per the table) deferred to future sessions; this ticket's lasting value is the audit + reference profile.
- 2026-05-20 (reopen): the prior "catalog-only" landing was premature — five concrete fix items were named with file:line precision but never opened as follow-on tickets, violating CLAUDE.md's *antipattern follow-ups are non-optional* doctrine. Per user directive, scope is folded back into 431 itself as a six-stage event-driven sweep (Stages A–G in §Scope). Catalog block below preserved verbatim as the open-time investigation record. Each stage lands as an independent commit gated by byte-identical `_footer` against `just soak-trace 42 Mallow`. Final post-refactor flamegraph captures to `docs/diagnostics/baseline-profiles/2026-05-20-post-431/`.
