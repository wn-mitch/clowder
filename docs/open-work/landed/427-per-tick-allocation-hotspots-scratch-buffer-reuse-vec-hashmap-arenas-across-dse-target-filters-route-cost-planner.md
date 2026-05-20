---
id: 427
title: Per-tick allocation hotspots scratch buffer reuse Vec HashMap arenas across DSE target filters route_cost planner
status: done
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-05-19
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: a20985f2
landed-on: 2026-05-19
---

## Why

Follow-on to ticket 423's perf survey. The user observed that ticks-reached
dims as colony size grows ("bodes very badly for dwarf-fortress sized sims").
Ticket 423 retired the per-cat O(radius²) cover-disc scan and recovered 9.7%
tick-rate. The wider survey surfaced a much larger CPU + allocator hotspot:
**per-tick `Vec` / `HashMap` allocations across DSE target filters,
`route_cost`, and the GOAP planner**.

Stack-ranked findings (estimated bytes per 15-min soak at 500-cat
projection):

| Rank | Hotspot | File:line | Bytes/soak |
|------|---------|-----------|-----------|
| 1 | DSE target-filter scratch (11+ DSEs each alloc `Vec<Entity>` + `Vec<Position>` + 1–3 HashMaps per cat per tick) | `src/ai/dses/fight_target.rs:339`, `build_target.rs:206`, `mentor_target.rs:231`, `herbcraft_target.rs:161`, `apply_remedy_target.rs:186`, … | ~355 MB |
| 2 | Dijkstra bucket-queue flood `Vec<Vec<Position>>` per route-cost call | `src/ai/route_cost.rs:71` | ~96 MB |
| 3 | A* planner `SearchNode` arena `Vec<SearchNode>` capacity 256 per search | `src/ai/planner/mod.rs:636`, `src/ai/planner/core.rs:120` | ~20 MB |
| 4 | DSE HashMap overlays (threat_map, skills_by_entity, density+maturity, etc.) | per-DSE files | ~11 MB |
| 5 | HashMap tile-occupancy counts per cat | `src/systems/goap.rs:3637, 3669` | ~9.5 MB |
| 6 | `Relationships::all_for(entity)` returns owned `Vec<(Entity, &Relationship)>` per call | `src/systems/interoception.rs:394`, `snapshot.rs:99`, `coordination.rs:39`, `aspirations.rs:578` | ~800 KB |
| 7 | `scores.clone()` focal-trace capture | `goap.rs:2449`, `disposition.rs:1202` | ~350 KB |
| 8 | String formatting in narratives | scattered `format!()` in goap.rs / disposition.rs | ~625 KB |
| 9 | Kitten snapshot vecs | `disposition.rs:466, 1315`, `goap.rs:1415` | ~240 KB |
| 10 | Misc per-tick cat-position lists | `goap.rs:1024, 3734`, `disposition.rs:1294` | ~24 KB |

**Aggregate: ~490 MB/soak.** With proper scratch-buffer reuse, projected
~115 MB/soak — a **76% allocation reduction**. Estimated tick-rate
improvement: **3–5%** beyond what 423 recovered.

## Scope

Sequenced by ROI (largest hotspot first). Each sub-step is its own
self-contained refactor; nothing has cross-step coupling beyond the shared
`SystemParam` pattern.

### Step 1 — DSE target-filter scratchpad (highest ROI)

- **New `Resource<DseTargetScratchpad>`** holding pre-allocated
  `entities: Vec<Entity>`, `positions: Vec<Position>`, and 3 rotating
  `HashMap` pools (entity-keyed `f32`, entity-keyed `Option<i32>` for
  distances, etc.).
- Thread it through all 11+ target-DSE functions via a `SystemParam` so
  each `score_target_taking` impl writes to the scratchpad instead of
  allocating fresh Vecs.
- Pattern: `scratchpad.entities.clear(); for cand in ... { scratchpad.entities.push(cand); }`
  — vector capacity is preserved across calls.

### Step 2 — Dijkstra bucket-queue arena

- `src/ai/route_cost.rs:71` allocates `Vec<Vec<Position>>` of size
  `max_cost + 1` (up to 256 buckets) per call.
- Move to a `Resource<RouteCostBucketArena>` that owns the outer +
  inner Vecs. `clear()` between calls. Wrap in a guard so concurrent
  callers don't trample each other (or use a per-system local).

### Step 3 — A* planner SearchNode arena

- `src/ai/planner/core.rs:120` allocates `Vec<SearchNode>` cap=256 per
  search. ~2-3 searches per cat per tick.
- Mirror Step 2: `Resource<PlannerSearchArena>` with pre-allocated
  `Vec<SearchNode>`, cleared per search.

### Step 4 — `Relationships::all_for` no-alloc iterator

- Currently returns `Vec<(Entity, &Relationship)>` — allocator-heavy
  for what is conceptually a filter.
- Add `fn iter_for(&self, entity: Entity) -> impl Iterator<Item = (Entity, &Relationship)> + '_`
  alongside the existing `all_for`. Migrate callers in
  `interoception.rs:394`, `snapshot.rs:99`, `coordination.rs:39`,
  `aspirations.rs:578`. Keep `all_for` for the rare callers that
  genuinely need a `Vec`.

### Step 5 — HashMap tile-occupancy + DSE overlays

- `goap.rs:3637, 3669` HashMap allocs per call → `Resource<TileOccupancyScratch>`.
- DSE-specific HashMaps (`threat_map`, `skills_by_entity`, etc.) → bundle
  into `DseTargetScratchpad` from Step 1 if reusable; else per-DSE
  scratch resources.

### Step 6 — String formatting

- Audit `format!()` calls in `goap.rs` / `disposition.rs` step
  resolvers. Most are narrative-payload strings. Convert to
  `write!(&mut scratch_string, ...)` with a pre-allocated buffer, or
  to static `&'static str` templates with parameterized fill via a
  pre-sized String.

### Step 7 — `scores.clone()` focal-trace capture

- `goap.rs:2449`, `disposition.rs:1202` clone `Vec<(Action, f32)>` for
  trace emission. Use a SmallVec or a per-system scratch buffer.

## Out of scope

- Tuning DSE-specific behavior (this is allocator hygiene, not balance work).
- Changes to L2/L3 scoring semantics — fixes must be byte-identical-output.
- Cat-spatial-index work — owned by ticket 205 (Phase 2 of the perf sweep).
- Event-driven marker authoring — owned by ticket 426 and the broader
  reframe in the ticket-205-plan.

## Current state

Opened mid-session 2026-05-19 as a follow-on to 423's landing. Survey
done; no implementation has started. Ticket 205 + 425 + 426 are the
sibling perf-sweep follow-ons.

## Approach

Multi-step ticket — each step is its own commit + verdict so regressions
are bisectable. Step 1 (DSE target scratchpad) is the single biggest win;
steps 2-3 are next; steps 4-7 are progressively smaller. The first three
steps capture ~95% of the byte savings.

## Verification

Per-step:
- `just check` + `just test` pass.
- `just soak-trace 42 Simba` → `just verdict` against the
  pre-step baseline. Survival + canaries unchanged. `frame-diff`
  concordance ok.
- `elapsed_ticks` improvement vs the pre-step baseline.

End-state:
- Aggregate `elapsed_ticks` improvement ≥ 3% over the post-205+423
  baseline (the bytes-saved estimate predicts 3-5%).
- No new clippy warnings.

## Log

- 2026-05-19: opened as follow-on to 423. Memory-cost survey done via
  Explore subagent — see ticket body for findings table. User
  directive: "dig for other memory costly operations and open in a
  followup ticket." Estimated 3–5% additional tick-rate recovery.
- 2026-05-19: all 7 steps landed; +3.9% throughput (75903 ticks/900s vs 73035 pre-427 baseline); verdict PASS (survival, continuity, constants_drift clean); frame-diff concordance ok (byte-identical DSE scoring; Δ mean < ±0.01 absolute on all 15 tracked DSEs, drift is from extra ticks running, not from scoring changes)
