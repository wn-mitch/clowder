# WorldSnapshots — cross-system per-tick aggregates

**Ticket 433** (the rescoped Stage F of 431). Authored 2026-05-20.

## Why

Several systems independently fold over colony state to compute the same per-tick aggregates. The cost varies — some are O(1) singleton queries, others are O(N) Vec rebuilds — but the architectural smell is identical: parallel substrate that *should* converge on a single read-only resource.

This doc names the audit findings, the substrate boundary the `WorldSnapshots` resource owns, and the first concrete hoist that lands alongside it. Future hoists slot into the same resource without re-architecting.

## Audit (2026-05-20)

The audit catalogued cross-system duplications visible in `src/plugins/simulation.rs`. Four candidates surfaced:

| # | Aggregate | Source site | Other readers | Chains involved | Per-tick cost |
|---|-----------|-------------|---------------|-----------------|---------------|
| 1 | Colony marker booleans (15× `Has<HasStoredFood>` / `HasGarden` / …) | `goap.rs:1477` reads `colony_state_query`; populates `MarkerSnapshot` | `disposition.rs:435` (the dormant `evaluate_dispositions`); `coordination.rs:896` reads `ColonyThornbriarChronicallyLow` standalone | Authors in Chain 2a, readers in Chain 4+ | O(1) singleton read × N readers |
| 2 | `cat_positions: Vec<(Entity, Position)>` aggregation | `goap.rs:1480` builds from `world_state.all_positions` | `fox_spatial::update_store_awareness_markers:82` + `update_den_threat_markers:135` (Chain 2a); `disposition.rs:466` (dormant) | Chain 2a + post-Chain-4 | O(N) × 3 builds |
| 3 | `food.fraction()` | `goap.rs:1479` | `coordination::assess_colony_needs:267`; `coordination::accumulate_build_pressure:930` | Chain 2b + post-Chain-4 | O(1) × 3 |
| 4 | `kitten_snapshot: Vec<KittenState>` | `goap.rs:1493` builds from `world_state.kitten_query` | `disposition.rs:475` (dormant) | Post-Chain-4 only | O(K) × 1 (effectively single-reader) |

### Caveats that limit the first hoist's scope

- **Disposition.rs's `evaluate_dispositions` is dormant** (`grep evaluate_dispositions src/plugins/simulation.rs` returns only docstring references — never scheduled). The "Consumer 1" rows above for #1, #2, #4 are mirror substrate that runs only in tests. Hoisting against dormant readers buys cleanliness, not CPU.
- **`cat_positions` has a temporal-mismatch problem**. `fox_spatial` runs in Chain 2a and reads positions *before* Chain 3's movement systems mutate them; `evaluate_and_plan` runs after Chain 4 and reads positions *after* those mutations. A single snapshot can't serve both consumers without behavior change. The clean fix needs two snapshots (head-of-tick + post-movement) or a precise `.before(movement_systems)` ordering — out of scope for the first hoist.
- **`food.fraction()` is O(1)**. The cost is real (three resource reads per tick) but trivial in absolute terms; the value of hoisting it is purely architectural — establish the single source of truth for `food_fraction` so future ticket-system additions inherit the read pattern.

## Design

```rust
#[derive(Resource, Default)]
pub struct WorldSnapshots {
    /// Tick at which this snapshot was populated. Read-only after the
    /// populator runs; consumers MAY assert `snapshot.tick == time.tick`
    /// in debug builds to catch ordering bugs.
    pub tick: u64,

    /// Cached `MarkerSnapshot`-shaped colony booleans, read once from
    /// the `ColonyState` singleton's `Has<>`-typed marker components.
    /// Built by `populate_world_snapshots` after every marker-author
    /// system in Chain 2a runs.
    pub colony_markers: ColonyMarkerBundle,

    /// `FoodStores::fraction()` precomputed once per tick. Reads
    /// `Res<FoodStores>` after `sync_food_stores` (Chain 1's items pass)
    /// has run.
    pub food_fraction: f32,

    /// Whether the colony has any food this tick (`!food.is_empty()`).
    /// Equivalent to `colony_markers.has_stored_food` today but kept
    /// separate because the substrate-of-record for the boolean is
    /// `FoodStores`, not the ColonyState marker.
    pub food_available: bool,
}
```

Where `ColonyMarkerBundle` is a plain struct of named booleans (one per `Has<>` field in the existing `colony_state_query`).

### Populator placement

`populate_world_snapshots` runs at the END of Chain 2a, after every marker-author system has authored its singleton-keyed markers. This makes the snapshot available to:

- `coordination::*` in Chain 2b (`assess_colony_needs`, `accumulate_build_pressure`) — currently reads `food.fraction()` and selected colony markers via their own queries.
- `evaluate_and_plan` after Chain 4 — currently reads `colony_state_query.single()` and `food.fraction()` inline.
- Any future per-tick system that needs colony aggregates.

The placement preserves the "marker authors run first" invariant in Chain 2a — the populator reads markers, doesn't author them.

### Out of scope for the first hoist

- **`cat_positions` hoist** — requires two snapshots (pre-movement + post-movement) or per-chain populator instances. Deferred until a second consumer with the same temporal alignment appears (e.g. a non-fox system that reads pre-movement positions).
- **`kitten_snapshot` hoist** — single production reader (`evaluate_and_plan`). Hoisting wouldn't save CPU; only relevant if a sibling per-tick caretake/fertility system emerges.
- **Disposition.rs's mirror queries** — `evaluate_dispositions` is dormant. Cleanup is a follow-on if/when it revives, or if the mirror substrate gets retired entirely.

## First concrete hoist (lands with this doc)

`colony_markers` + `food_fraction` + `food_available`. Goap's `evaluate_and_plan` retires its inline `colony_state_query.single()` read and reads `Res<WorldSnapshots>` instead. The change is byte-equivalent in behavior — same data, single source of truth.

Future hoists slot into the same Resource:

```rust
// Later ticket — when a second pre-movement cat-position consumer appears:
pub pre_movement_cat_positions: Vec<(Entity, Position)>,
```

## Verification

- `just check` + `just test` clean.
- `just soak-trace 42 Simba` + `just verdict` against the post-432 baseline. Behavior preserved (no colony-marker / food-fraction read changes the value the consumer sees).
- Debug-only invariant guard (under `#[cfg(debug_assertions)]`): every 100 ticks, the populator runs the singleton query a second time and asserts each marker boolean equals the snapshot's cached value. Catches regressions where a marker-author runs out of order, or a new author lands without updating the populator.

## Pattern for future hoists

When adding a new field to `WorldSnapshots`:

1. **Audit first** — name the second (production) consumer and confirm it's not dormant. Single-consumer hoists don't earn the Resource fragmentation cost.
2. **Temporal alignment** — name where in the tick the snapshot is captured. If consumers read at different times, name which alignment is correct and why.
3. **Refactor consumers** — replace inline queries / aggregations with `world_snapshots.foo` reads.
4. **Debug invariant** — assert the snapshot matches a re-derivation in debug builds.
