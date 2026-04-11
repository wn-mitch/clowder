# Clowder

Cat colony simulation in a Redwall-inspired fantasy world. Rust + Bevy ECS, 2D pixel-art sprites.

## Commands

- `just run` / `just seed <N>` — run the sim (optionally with fixed seed)
- `just test` — run tests
- `just check` — cargo check + clippy
- `just ci` — all checks

## Conventions

- Conventional commits (`feat:`, `fix:`, `chore:`, `refactor:`, `test:`, `docs:`) — no scopes
- Branch naming: `wnmitch/<feature-name>`
- VCS: `jj` (not raw git)
- Design docs: `docs/systems/` — one stub per tunable system

## Design Principles

- **Utility AI:** Cats score actions per-tick via needs, personality, relationships, context. No behavior trees, no LLMs.
- **Maslow needs:** 5 levels (physiological → self-actualization). Lower levels suppress higher when critical.
- **Physical causality:** Objects don't teleport. Cats carry items, walk to destinations, deposit them. Actions are behavioral arcs with physical movement — not instant stat changes at a distance.
- **Emergent complexity:** Systems should tie into the narrative layer through unexpected interaction. Chain reactions between independent systems are the joy — design for them.

## ECS Rules

- Prefer `run_if` guards over early returns — gated systems skip query iteration entirely.
- Never `.clone()` resource data in per-tick systems. Borrow via `Res<T>`/`ResMut<T>`.
- Events are verbs: `SpawnCat`, `CatDied` — not `DeathEvent`. Define centrally, no circular flows.
- Bevy 0.18 uses **Messages** not Events: `#[derive(Message)]`, `MessageWriter<T>`, `MessageReader<T>`, `app.add_message::<T>()`. Register in both `SimulationPlugin` and headless `build_new_world`. Headless also needs `bevy_ecs::message::message_update_system` in the schedule and `MessageRegistry::register_message::<T>(&mut world)`.
- Components: plain structs/enums with `#[derive(Component)]`. Resources: `#[derive(Resource)]`.
- Prefer `Query<>` with explicit component access over broad world access.
- **Bevy 16-param limit**: systems with many parameters hit Bevy's tuple impl limit. Use `#[derive(SystemParam)]` bundles to group related params. Example: bundle all prey-related queries + message writers into a `PreySystemParams` struct. This is preferred over `Option<Res<T>>` hacks or removing needed params.
- **Query disjointness**: when splitting `Query<&mut Component>` into separate data/marker patterns, add `With<Marker>` to restore disjointness for paired `Without<Marker>` filters in other queries.

## Headless Mode

`build_schedule()` in `src/main.rs` is a **manual mirror** of `SimulationPlugin::build()` in `src/plugins/simulation.rs`. Change one, change both — they diverged silently before.

## Simulation Verification

After changes to AI scoring, needs tuning, decay rates, or economy parameters:

1. **Run `just score-track`** — benchmarks seeds 42, 99, 7, 2025, 314 and appends results to `logs/score_history.jsonl` tagged with the current jj changeset.
2. **Run `just score-diff`** — compares the latest benchmark against the previous one. Welfare axes should not regress, `deaths_starvation` should not climb, `features_active` should not drop.
3. **Starvation canary:** `deaths_starvation` climbing across seeds is the fastest signal something is wrong.
4. **Activation canary:** `features_active` dropping means a system went dead — check what constant change disabled it.
5. **Long soak:** For major changes, use `just score-track --duration 60` for deeper signal.
6. **Balance report:** `just balance-report` for per-cat charts and detailed diagnostics on a single run.

The score history file is gitignored (local machine state), but the `constants_hash` in each row lets you verify two machines ran identical tuning.

## Tuning Constants

All simulation knobs live in `src/resources/sim_constants.rs`. Each system reads from `Res<SimConstants>` — no inline magic numbers. The full constants struct serializes to JSON in the headless output header, and the `constants_hash` in `score_history.jsonl` fingerprints the configuration that produced each benchmark.

## Rendering

Tilemap uses plain Bevy `Sprite` entities — **not** `TilemapBundle`. bevy_ecs_tilemap's GPU pipeline silently renders all tiles as texture index 0 on macOS Metal. Base terrain at z=0, autotile overlays at z=1/2/3. F6/F7/F8 toggle overlay visibility.
