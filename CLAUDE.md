# Clowder

A colony sim about a clowder of cats living in a world with its own weight — honest ecology with a mythic undercurrent. *Watership Down meets Timberborn, starring cats.*

**Stack:** Rust + Bevy ECS 0.18, 2D pixel-art sprites.

See [`docs/systems/project-vision.md`](docs/systems/project-vision.md) for the full design thesis — what kind of game this is, what it isn't, and how that shapes balance and feature work.

## Commands

- `just run` / `just seed <N>` — run the sim (optionally with fixed seed)
- `just headless [ARGS]` — headless diagnostic run (debug build by default). Use `cargo run --release -- --headless ...` for verification work. See **Simulation Verification** below.
- `just test` — run tests
- `just check` — cargo check + clippy
- `just ci` — all checks

## Conventions

- Conventional commits (`feat:`, `fix:`, `chore:`, `refactor:`, `test:`, `docs:`) — no scopes
- **Solo-to-main workflow:** this is a personal project; commits push directly to `main` without PR review. Feature branches (`wnmitch/<name>`) are optional and used only when a change is large/experimental enough that the author wants a staging area. The global-CLAUDE feature-branch convention does **not** apply here.
- VCS: `jj` (not raw git)
- Design docs: `docs/systems/` — one stub per tunable system

## Design Principles

- **Utility AI + GOAP:** Cats score actions per-tick via needs, personality, relationships, context (`src/ai/scoring.rs`). The winning disposition drives a GOAP planner (`src/systems/goap.rs`) that sequences concrete steps. No behavior trees, no LLMs.
- **Maslow needs:** 5 levels (physiological → self-actualization). Lower levels suppress higher when critical.
- **Physical causality:** Objects don't teleport. Cats carry items, walk to destinations, deposit them. Actions are behavioral arcs with physical movement — not instant stat changes at a distance.
- **Ecological-magical-realist world:** Magic, fate, the Calling, wards, corruption are *ecological phenomena with metaphysical weight*, not a separate narrative layer waiting to be unlocked. Tune them as part of the ecosystem.
- **Honest world, no director:** No difficulty scaling. No RimWorld-style storyteller. Seasons, weather, migration, predator-prey oscillation, corruption cycles *are* the event generator. Cats earn their stories by surviving a world that doesn't care.
- **Emergent complexity:** Chain reactions between independent systems are the joy — design for them. The Dwarf Fortress beer-cats-puke-depression spiral is the gold standard.

## Long-horizon coordination

Three indexes track work across sessions. Read them before starting any new
system, balance change, or non-trivial refactor — do not default to opening a
new thread.

- **`docs/open-work.md`** — tactical queue: uncommitted changes, follow-ons, pre-existing issues. Entries are pointers, not plans.
- **`docs/wiki/systems.md`** — auto-generated status of every `docs/systems/*` stub (Built / Partial / Aspirational) with registered functions. Regenerate via `scripts/generate_wiki.py` if stale.
- **`docs/balance/*.md`** — per-balance-thread iteration logs. New iterations append to the existing file (see `unified-difficulty-posture.md` for the Iteration 1 → 2 → 3 pattern).

### Before starting new work

1. Check `docs/open-work.md` for an in-flight entry matching the request.
2. Check `docs/wiki/systems.md` — if the request names a system, confirm its current Built/Partial/Aspirational status before proposing changes.
3. If the request advances an in-flight thread: proceed.
4. If the request does not match any entry: say so, name whether it advances `docs/systems/project-vision.md` §5 (broaden sideways: grooming, play, mentoring, burial, courtship, preservation, generational knowledge) or a continuity canary, and confirm with the user before writing code.

### When work completes, defers, or is opened

- Landed work: move the entry from `docs/open-work.md` to a "Landed" section with commit hash, or delete if trivial — same commit that ships the change.
- Deferred work: leave the entry in place; add a "Blocked by:" or "Resume when:" line.
- New open items surfaced mid-session: append to the appropriate `open-work.md` section before closing out.
- Balance changes that produce a new iteration: append the iteration to the thread's existing `docs/balance/*.md` file rather than creating a new one.
- Any change to `SimulationPlugin::build()` (system added/removed): regenerate `docs/wiki/systems.md` in the same commit.

## ECS Rules

- Prefer `run_if` guards over early returns — gated systems skip query iteration entirely.
- Never `.clone()` resource data in per-tick systems. Borrow via `Res<T>`/`ResMut<T>`.
- Events are verbs: `SpawnCat`, `CatDied` — not `DeathEvent`. Define centrally, no circular flows.
- Bevy 0.18 uses **Messages** not Events: `#[derive(Message)]`, `MessageWriter<T>`, `MessageReader<T>`, `app.add_message::<T>()`. Register in both `SimulationPlugin` and headless `build_new_world`. Headless also needs `bevy_ecs::message::message_update_system` in the schedule and `MessageRegistry::register_message::<T>(&mut world)`.
- Components: plain structs/enums with `#[derive(Component)]`. Resources: `#[derive(Resource)]`.
- Prefer `Query<>` with explicit component access over broad world access.
- **Bevy 16-param limit**: systems with many parameters hit Bevy's tuple impl limit. Use `#[derive(SystemParam)]` bundles to group related params. Example: bundle all prey-related queries + message writers into a `PreySystemParams` struct. This is preferred over `Option<Res<T>>` hacks or removing needed params.
- **Query disjointness**: when splitting `Query<&mut Component>` into separate data/marker patterns, add `With<Marker>` to restore disjointness for paired `Without<Marker>` filters in other queries.

## Systems inventory

Design docs for each tunable system live in `docs/systems/`. Major modules and what they do:

- **`src/systems/goap.rs`** — GOAP planner; turns a winning disposition into a concrete step sequence. Single largest file in the project (~4k lines). Central to cat decision-making.
- **`src/systems/disposition.rs`** — Step resolvers: the concrete ECS effects of each action (eat, sleep, hunt, socialize, groom, etc.).
- **`src/systems/coordination.rs`** — Coordinator governance, build-pressure directives, work assignment across the colony.
- **`src/systems/magic.rs`** — Herbcraft, wards, corruption spread, shadowfox spawning from corruption, seasonal herb growth.
- **`src/systems/fate.rs`** — Fated pairs, prophetic visions, destiny modifiers.
- **`src/systems/aspirations.rs`** — Long-horizon personal goals (mastery arcs for hunting, combat, crafting, socializing).
- **`src/systems/prey.rs`, `wildlife.rs`, `fox_goap.rs`** — Prey ecology (density, dens, reproduction, fear) and wild-animal AI (foxes, hawks, snakes, shadowfoxes).
- **`src/systems/needs.rs`, `mood.rs`** — Maslow hierarchy tracking and mood valence/arousal cascade.
- **`src/systems/sensing.rs`** — Four-channel perception (sight, hearing, scent, tremor).
- **`src/systems/weather.rs`, `wind.rs`, `time.rs`** — Diurnal phase, seasonal cycle, weather transitions, wind direction (scent vector).
- **`src/systems/social.rs`, `pregnancy.rs`, `growth.rs`** — Relationships, gossip, courtship, reproduction, life-stage progression.
- **`src/systems/combat.rs`, `death.rs`** — Combat resolution, injury, mortality, grief cascade.
- **`src/systems/memory.rs`, `colony_knowledge.rs`** — Per-cat memory and colony-level shared knowledge (social transmission).
- **`src/systems/narrative.rs`** — Template-driven narrative line emission across tiered severity (Micro / Action / Significant / Danger / Nature).

## Headless Mode

`build_schedule()` in `src/main.rs` is a **manual mirror** of `SimulationPlugin::build()` in `src/plugins/simulation.rs`. Change one, change both — they diverged silently before.

## Simulation Verification

**`just headless` is the canonical diagnostic tool.** It's a thin wrapper over `cargo run -- --headless`, runs the sim under the same Bevy schedule as the interactive build (see Headless Mode above — the schedule is mirrored, not shared), writes two JSONL files, and exits early if the colony wipes. Everything else (`score-track`, `score-diff`, `balance-report`) is a Python convenience script layered on top — the JSONL output is ground truth.

### Invocation and flags

`just headless [--seed N] [--duration SECS] [--log PATH] [--event-log PATH] [--snapshot-interval TICKS] [--trace-positions N] [--test-map]`

- `--seed N` — fixed RNG seed (default: random; printed to stderr). Required for reproducibility and diffs.
- `--duration SECS` — wall-clock sim duration in seconds (default 600 = 10 min). `--duration 60` is a smoke-test; `--duration 900` (15 min) is the canonical deep-soak (see below).
- `--log PATH` — narrative log output (default `logs/narrative.jsonl`). Tiered entries: Micro / Action / Significant / Danger / Nature.
- `--event-log PATH` — structured event log output (default `logs/events.jsonl`). Machine-readable: spawns, deaths, plan failures, feature activations.
- `--snapshot-interval TICKS` — per-cat snapshot cadence (default 100).
- `--test-map`, `--trace-positions N` — seldom needed; see `parse_args` in `src/main.rs`.

### The constants-hash header

Line 1 of `logs/events.jsonl` is a JSON header with `seed`, `duration_secs`, `commit_hash` / `commit_hash_short` / `commit_dirty` / `commit_time` (emitted by `build.rs`), a `sim_config` block (`ticks_per_day_phase`, `ticks_per_season`, `seed`) used to derive season/day boundaries from tick values, and the **full `SimConstants` dump**. This is how you confirm which tuning produced which run — two machines are comparable iff their headers match byte-for-byte on the `constants` field **and** carry the same `commit_hash` with `commit_dirty == false`. Never diff sim outcomes without first diffing headers. A `commit_dirty: true` header means the log cannot be reproduced from the commit alone; dashboards and scripts should surface this rather than compare silently. `logs/narrative.jsonl` carries the same commit fields (minus `sim_config` and `constants`) for narrative-only analyses.

### Canonical deep-soak: seed 42 at 15 minutes

The reference verification run is **seed 42, `--duration 900` (15 minutes wall), release build**. 15 minutes is long enough for corruption to climb above 0.7, shadow-foxes to spawn, cats to build multi-generational routines, and the mortality distribution to stabilize. Anything shorter (60s, 5 min) misses the phases where most balance problems surface.

```bash
just soak 42    # writes logs/tuned-42/{events,narrative}.jsonl
```

(equivalent to `cargo run --release -- --headless --seed 42 --duration 900 ...`)

Debug mode is ~4× slower than release and produces far less sim time per second of wall — **always `--release` for verification**; debug is for development-time feedback only. Save the footer from each run (grep `_footer` in the event log) before and after any tuning change to produce a diff.

Multi-seed sweeps (seeds 99/7/2025/314) are a follow-up for claims you want to generalize — only do them once a single-seed deep-soak looks right.

### Diagnostic queries

jq recipes for reading `events.jsonl` / `narrative.jsonl` live in
`docs/diagnostics/log-queries.md`. For routine checks:

- `just check-canaries LOGFILE` — runs the four canary queries, exits non-zero on failure.
- `just diff-constants BASE NEW` — verifies two runs are behaviorally comparable.

### Canaries

Canaries split into two groups. **Survival canaries** catch the colony dying or degenerating. **Continuity canaries** catch the world showing only a narrow slice of its range (survival lock, flat mythic texture). Both classes are hard — a silent mythic register is a bug on par with a starvation cascade, per the ecological-magical-realist framing (see `docs/systems/project-vision.md`).

**Survival canaries:**

- **Starvation canary:** `deaths_by_cause.Starvation` climbing in the deep-soak is the fastest signal something is wrong. Target: 0 on seed 42.
- **Shadowfox canary:** `deaths_by_cause.ShadowFoxAmbush` > 2 on a 15-min deep-soak means the ward/corruption defense pipeline is failing — see `docs/systems/shadowfox_wards.md` once created.
- **Activation canary:** a previously-active `Feature::*` in the **Positive** or **Neutral** category dropping to 0 means a system went dead. Positive dormancy is the real concern (a healthy-colony signal stopped firing); negative dormancy is fine (no bad events). The `SystemActivation` event in `logs/events.jsonl` splits counts into `positive`/`negative`/`neutral` groups — compare each against a known-good baseline. See `src/resources/system_activation.rs` for the classification.
- **Wipeout canary:** headless prints `All cats dead at tick N. Ending early.` to stderr and terminates — any run that wipes in under `--duration` is a regression.

**Continuity canaries** (currently not all passing — drive follow-on balance work):

- **Generational continuity:** at least one kitten reaches adulthood on a seed-42 `--duration 900` soak. A colony that survives but doesn't reproduce is failing to show generational play.
- **Ecological variety:** each of grooming, play, mentoring, burial, courtship must fire ≥1× per soak. All-zero on any means survival lock has collapsed the behavioral range.
- **Mythic texture:** ≥1 named event per sim year (Calling fired, banishment, visitor arrival, named object crafted). A silent mythic register means the world's metaphysical weight has flattened.

Telemetry for continuity canaries is not yet wired into `logs/events.jsonl` — that's a follow-on plan.

### What the interactive build shares with headless

`build_schedule()` in `src/main.rs:346` is a manual mirror of `SimulationPlugin::build()` in `src/plugins/simulation.rs`. Any new system, message, or resource must be added to **both**; drift between them has caused silent divergence before (see the Headless Mode subsection above).

## Tuning Constants

All simulation knobs live in `src/resources/sim_constants.rs`. Each system reads from `Res<SimConstants>` — no inline magic numbers. The full constants struct serializes to JSON in the `logs/events.jsonl` header; two headless runs are only comparable if their headers match on the `constants` field.

## Balance Methodology

**Drift in sim behavior (mortality rates, hunt success, ambush frequency, any characteristic metric) is acceptable if and only if it can be provably tied to an increase in verisimilitude.** Drift without a predicting hypothesis is a bug, not a feature.

Every balance-affecting change ships as a testable hypothesis of the form:

> *{ecological or perceptual fact}* ⇒ *{predicted direction and rough magnitude of metric shift}*

Acceptance requires four artifacts:

1. **Hypothesis** — the ecological/behavioral claim being modeled, with a real-world grounding (predator behavior, perception research, causal chain).
2. **Prediction** — direction and rough magnitude of the expected metric shift (e.g. "ShadowFoxAmbush count rises ~2× during fog windows").
3. **Observation** — measured shift from an A/B headless run (multi-seed sweep + forced-condition runs where relevant).
4. **Concordance** — direction matches prediction and magnitude is within ~2×. Direction mismatch = reject. Magnitude > 2× off = investigate second-order effects before accepting.

Drift ≤ ±10% on a characteristic metric is within measurement noise and does not require a written hypothesis. Drift > ±10% requires the full four artifacts. Drift > ±30% requires additional scrutiny before acceptance.

Canaries (`Starvation` = 0, `ShadowFoxAmbush` ≤ 5, `banishments` ≥ 3, colony survives day 180 on seed 42) are hard gates — they must pass regardless of hypothesis or concordance.

This rule applies to all balance work, not just the feature driving a given session. A refactor that changes sim behavior is a balance change and must tie out the same way.

## Rendering

Tilemap uses plain Bevy `Sprite` entities — **not** `TilemapBundle`. bevy_ecs_tilemap's GPU pipeline silently renders all tiles as texture index 0 on macOS Metal. Base terrain at z=0, autotile overlays at z=1/2/3. F6/F7/F8 toggle overlay visibility.
