# ECS rules (Bevy 0.18)

Tactical rules for using Bevy 0.18 in Clowder. The substrate-refactor spec ([`ai-substrate-refactor.md`](ai-substrate-refactor.md)) is the conceptual home for AI substrate; this doc covers the ECS-shape rules that govern *how* substrate is wired in Bevy.

## Messages, not Events

```rust
#[derive(Message)]
struct SpawnCat { /* … */ }

// Register in SimulationPlugin::build() — windowed and headless paths share that plugin (ticket 030).
app.add_message::<SpawnCat>();

// Read / write
fn system(mut writer: MessageWriter<SpawnCat>, reader: MessageReader<SpawnCat>) { /* … */ }
```

Names are verbs (`SpawnCat`, `CatDied`), **not** `*Event`.

## Default to event-driven; justify per-tick

True per-tick systems are rare and load-bearing — limit them to:

- (a) **Plan execution** (`resolve_goap_plans`, `dispatch_step_action`) — the step machine must advance per tick.
- (b) **Sense + score** (`evaluate_and_plan`, DSE scoring) — needs are continuous so scores reflect current state.
- (c) **Time-dependent decay** (`decay_fulfillment`, hunger drift) — physical reality of time passing.
- (d) **Movement / physics.**

Everything else should fire on a Bevy `Message` against cached state:

- **State accumulation** (familiarity, bond strength) — on a co-presence event, not every tick.
- **Lookup / query results** (`all_for`, marker presence) — cached, invalidated on the mutation event.
- **Spatial queries** (near-pair sets, path-cost fields) — cached, invalidated on `CatMoved` / `MapTileChanged`.
- **Aggregations** (per-cat relationship sums, coordinator weights) — cached, invalidated on the underlying mutation.

**Precedent: ticket 431.** `passive_familiarity` was running an O(N²) sweep at 64.43% inclusive CPU on a profile where the actual state change (familiarity drift between near pairs) is event-driven. Retiring the sweep behind a `CatMoved`-driven `NearPairCache` was the substrate-correct fix.

**Seed-determinism trap when retiring per-tick sweeps:** the iteration order of the data structure carrying the cache (e.g., `BTreeMap`) is often load-bearing for tie-breaking in downstream sorts. Preserve it on any swap, and gate the swap with a debug-only invariant assertion against the pre-cache pair set so divergences localize at the first divergent tick rather than surfacing as drift weeks later.

Cross-reference: memory `project_per_tick_discipline_default_event_driven.md`.

## Resource borrowing

- Prefer `run_if` guards over early returns.
- Never `.clone()` resource data in per-tick systems — borrow via `Res<T>` / `ResMut<T>`.

## Bevy 16-param limit

Bundle related queries / writers in `#[derive(SystemParam)]` structs. Preferred over `Option<Res<T>>` hacks.

## Query disjointness

Splitting `Query<&mut C>` by marker: pair `With<M>` and `Without<M>` against sibling queries so the borrow checker can prove they don't overlap.
