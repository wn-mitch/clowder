---
id: 025
title: Hawk and snake GOAP planner domains
status: in-progress
cluster: null
added: 2026-04-24
parked: null
blocked-by: []
supersedes: []
related-systems: [wildlife]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Hawks and snakes use hardcoded state machines in `wildlife_ai` (Circling / Waiting). Foxes have a full GOAP system with Maslow-structured dispositions. Giving hawks and snakes their own GOAP domains creates species-distinct behavior rather than a shared state machine, and establishes the pattern for future wildlife.

## Current state

**Landed (Phase 1 — decision substrate):**
- `src/ai/hawk_planner/` — `HawkDomain` implementing `GoapDomain` with 4 zones (Sky, HuntingGround, Perch, MapEdge), 5 action types, 4 dispositions (Hunting, Soaring, Fleeing, Resting), action/goal/predicate/effect tables. 3 planner tests.
- `src/ai/snake_planner/` — `SnakeDomain` implementing `GoapDomain` with 4 zones (Cover, HuntingGround, BaskingSpot, MapEdge), 5 action types, 4 dispositions (Ambushing, Foraging, Basking, Fleeing). 3 planner tests.
- `src/ai/hawk_scoring.rs` — `HawkNeeds` (1-level Maslow), `HawkPersonality` (boldness, patience), `HawkScoringContext`, softmax disposition selection. 3 scoring tests.
- `src/ai/snake_scoring.rs` — `SnakeNeeds` (2-level Maslow: survival + thermoregulation), `SnakePersonality` (aggression, patience), `SnakeScoringContext`, softmax selection. 3 scoring tests.
- `src/ai/dses/hawk_*.rs` — 3 DSE factories (hunting, fleeing, resting) with 2 axes each. 17 tests.
- `src/ai/dses/snake_*.rs` — 4 DSE factories (ambushing, foraging, fleeing, basking) with 1-2 axes each. 13 tests.
- `src/ai/eval.rs` — `DseRegistry` extended with `hawk_dses` / `snake_dses` slots + lookup methods + `add_hawk_dse` / `add_snake_dse` extension methods.
- 53 new tests total, all passing.

**Remaining (Phase 2 — runtime wiring):**
- ECS components: `HawkState`, `SnakeState` (lifecycle, hunger), `HawkAiPhase`, `SnakeAiPhase` in `src/components/wildlife.rs`
- GOAP systems: `hawk_evaluate_and_plan`, `hawk_resolve_goap_plans` in `src/systems/hawk_goap.rs`; same for snake
- Step resolvers: `src/steps/hawk/mod.rs`, `src/steps/snake/mod.rs`
- Registration: add systems + DSEs to `SimulationPlugin::build` and both `build_schedule` paths
- Replace hardcoded hawk/snake AI in `wildlife_ai` with GOAP-driven behavior
- Attach `HawkState`/`SnakeState` at spawn in `spawn_initial_dens` / edge-spawner
- SimConstants: hawk/snake-specific tuning knobs

## Plan

### 1. Approach

Mirror the fox GOAP architecture beat-for-beat. Each species adds a per-tick `evaluate_and_plan` (commits a `*GoapPlan` component when one is missing) and `resolve_goap_plans` (dispatches the current step to a resolver under `src/steps/<species>/`). Step resolvers follow the §GOAP-Step-Resolver-Contract (5 rustdoc headings, `StepOutcome<W>` shapes, `record_if_witnessed`-only Feature emission). Hawk reuses the fox's `record_if_witnessed` discipline; snake adds a thermoregulation tier (Maslow 2). Hardcoded `WildlifeAiState::Circling`/`Waiting` machinery is retired only in the final commit of the sequence so the tree stays green throughout. Single registration site is `SimulationPlugin::build` (per ticket 030, headless and windowed both consume it).

### 2. Components (`src/components/wildlife.rs`)

All four new components derive `Component, Debug, Clone, serde::Serialize, serde::Deserialize`. Reference template: `FoxState` (`src/components/wildlife.rs:235-296`) and `FoxAiPhase` (`src/components/wildlife.rs:208-232`).

**`HawkState`** — per-hawk mutable state (analog to `FoxState` minus territory/breeding):

- `hunger: f32` — `0.0` = full, `1.0` = starving (matches `FoxState::hunger` semantics, NOT `HawkNeeds::hunger`'s 1.0-satisfied semantics; `sync_hawk_needs` does the inversion).
- `satiation_ticks: u64` — ticks remaining before hunger resumes decaying. Set on successful `DiveAttack`.
- `age_ticks: u64` — ticks since spawn (no life-stage tier; hawks are mono-stage adults).
- `post_action_cooldown: u64` — cooldown after `DiveAttack`/`FleeSky`.
- `starvation_ticks: u64` — consecutive ticks at `hunger >= 1.0`. Death at `hawk.starvation_death_duration`.
- `last_perch_tick: u64` — used by Resting scoring pressure.
- `last_dive_tick: u64` — used by Hunting scoring pressure / patience curve.

**`HawkAiPhase`** — high-level rendering/narrative phase (analog to `FoxAiPhase`):

```text
Soaring { center_x: i32, center_y: i32, angle: f32 }   // default airborne
HuntingPrey { target: Option<u64> }                    // diving on a target
Perched { ticks: u64 }                                  // resting on perch
Fleeing { dx: i32, dy: i32 }                            // toward map edge
```

**`SnakeState`** — per-snake mutable state with thermoregulation:

- `hunger: f32` — same semantics as `HawkState::hunger`.
- `satiation_ticks: u64`.
- `warmth: f32` — `0.0..=1.0`. Decays each tick when `!on_warm_terrain`; reset to `1.0` after `Bask`.
- `age_ticks: u64`.
- `post_action_cooldown: u64`.
- `starvation_ticks: u64`.
- `last_strike_tick: u64`.
- `last_bask_tick: u64`.

**`SnakeAiPhase`**:

```text
Waiting                                  // ambush, default still
Stalking { target_x: i32, target_y: i32 } // closing on prey
Striking { target: Option<u64> }         // strike attempt this tick
Basking { ticks: u64 }                    // thermoregulating
Fleeing { dx: i32, dy: i32 }              // toward cover/edge
```

**Hunger model** (both species):

- Per-tick decay handled by a new `hawk_needs_tick` / `snake_needs_tick` system (siblings of `fox_needs_tick`, `src/systems/wildlife.rs`). Rate: `hawk.hunger_decay_rate: RatePerDay` / `snake.hunger_decay_rate: RatePerDay`.
- Satiation reset on successful `DiveAttack`/`Strike` step (caller of step resolver applies — same shape as `FoxGoapActionKind::KillPrey` branch in `fox_goap.rs:701-720`).
- Starvation threshold at `*.starvation_death_duration: DurationDays` (death emits `Feature::DeathStarvation` via the existing wildlife-death pipeline).

**`HawkNeeds` / `HawkPersonality` / `SnakeNeeds` / `SnakePersonality`** — already exist in `src/ai/hawk_scoring.rs:25-77` and `src/ai/snake_scoring.rs:25-89`. Add `#[derive(bevy_ecs::prelude::Component, ...)]` in-place (no move — matches Phase 1 location). Then they can be attached at spawn alongside `*State` and queried by the new GOAP systems, matching the fox pattern (`FoxPersonality`/`FoxNeeds` in `src/components/fox_personality.rs:16,72` both derive `Component`).

### 3. Messages

All registered via `app.add_message::<T>()` in `SimulationPlugin::build` adjacent to the existing `app.add_message::<crate::components::prey::PreyKilled>()` line (`src/plugins/simulation.rs:140`). Verb names per CLAUDE.md.

- `HawkDiveLanded { hawk: Entity, prey: Entity, position: (i32,i32) }` — written by `resolve_dive_attack` step (via the calling system), read by narrative + canary. (No new write site for prey-kill itself; `predator_hunt_prey` already despawns prey and emits `Feature::FoxHuntedPrey` for foxes — extended to also cover hawks/snakes.)
- `SnakeStrikeLanded { snake: Entity, prey: Entity, position: (i32,i32) }` — symmetric.
- `HawkDied { hawk: Entity, cause: WildlifeDeathCause }` — written by `hawk_lifecycle_tick` (new system, see §4), read by event log. Mirrors existing `Feature::FoxDied` story.
- `SnakeDied { snake: Entity, cause: WildlifeDeathCause }` — symmetric.

`WildlifeDeathCause` is a new tiny enum colocated with these messages in `src/components/wildlife.rs` (`Starvation | OldAge | Combat | Other`).

### 4. GOAP systems

#### `src/systems/hawk_goap.rs` (template: `src/systems/fox_goap.rs:1-845`)

Three top-level public systems plus private helpers, mirroring the fox shape:

- `pub fn sync_hawk_needs` — analog of `sync_fox_needs` (`fox_goap.rs:54-97`). Reads `HawkState` + `Health`, populates `HawkNeeds.hunger = 1.0 - state.hunger` and `health_fraction`.
- `pub fn hawk_evaluate_and_plan` — analog of `fox_evaluate_and_plan` (`fox_goap.rs:379-552`). Bundles queries via `#[derive(SystemParam)]` (`HawkPlanQueries`) to stay under Bevy's 16-param limit (see Risks §12). Reads `HawkState`, `Position`, `HawkNeeds`, `HawkPersonality`, `cats`, `prey`, the `DseRegistry` (`hawk_dses` slot), `ModifierPipeline`, `SimRng`, `TimeState`, `SimConfig`, `SimConstants`, `EventLog`. Builds `HawkScoringContext`, calls `score_hawk_dispositions` + `select_hawk_disposition_softmax`, builds `HawkPlannerState` (`build_planner_state` helper analog to `fox_goap.rs:306-334`), calls `make_plan::<HawkDomain>`, inserts `HawkGoapPlan`. `run_if(systems::time::not_paused)`.
- `pub fn hawk_resolve_goap_plans` — analog of `fox_resolve_goap_plans` (`fox_goap.rs:559-845`). Dispatches each `HawkGoapActionKind` to a resolver under `src/steps/hawk/`. Sets `HawkAiPhase` + `WildlifeAiState` per `phase_for_action`/`target_for_action` helpers (mirror `fox_goap.rs:1005-1068`).
- `pub fn hawk_lifecycle_tick` — small private system for hunger decay + age + starvation death (writes `HawkDied`). Mirrors the relevant slice of `fox_lifecycle_tick` (no breeding/cubs).

Schedule placement (in `SimulationPlugin::build`): inside the existing wildlife `.chain()` block at `src/plugins/simulation.rs:217-231`. Insertion order:

```text
  spawn_wildlife,
  wildlife_ai,                      // (final commit: shrinks to fox-only legacy paths + ShadowFox)
  fox_movement,
  fox_needs_tick,
  hawk_needs_tick,                  // NEW
  snake_needs_tick,                 // NEW
  sync_fox_needs,
  sync_hawk_needs,                  // NEW
  sync_snake_needs,                 // NEW
  fox_evaluate_and_plan,
  hawk_evaluate_and_plan,           // NEW
  snake_evaluate_and_plan,          // NEW
  fox_resolve_goap_plans,
  hawk_resolve_goap_plans,          // NEW
  snake_resolve_goap_plans,         // NEW
  feed_cubs_at_dens,
  resolve_paired_confrontations,
  fox_ai_decision,
  fox_scent_tick,
  predator_hunt_prey,               // extended to apply hawk/snake satiation
  carcass_decay,
  carcass_scent_tick,
  predator_stalk_cats,
  hawk_lifecycle_tick,              // NEW
  snake_lifecycle_tick,             // NEW
```

The outer wrapper is already `.chain()` so explicit `before/after` edges are unnecessary. The chain is currently 14 systems — the 6 new entries push it to 20, still under Bevy's 20-tuple limit (room for one more before nesting needed).

#### `src/systems/snake_goap.rs` — symmetric to hawk_goap.rs

Same three systems (`sync_snake_needs`, `snake_evaluate_and_plan`, `snake_resolve_goap_plans`) plus `snake_lifecycle_tick`. Snake-specific: `SnakeScoringContext` includes `on_warm_terrain` (read via `map.get(pos.x, pos.y).terrain` against `WARM_TERRAINS = [Rock, Sand]`); `sync_snake_needs` decays `warmth` when `!on_warm_terrain` and resets it on `Bask` advance.

#### `*GoapPlan` components

New files `src/components/hawk_goap_plan.rs` and `src/components/snake_goap_plan.rs`, structurally identical to `src/components/fox_goap_plan.rs:11-98`:

- `HawkGoapPlan { steps: Vec<PlannedStep<HawkDomain>>, current_step, kind: HawkDispositionKind, adopted_tick, trips_done, target_trips, step_state: Vec<HawkStepState>, replan_count, max_replans, failed_actions: HashSet<HawkGoapActionKind> }`
- `HawkStepState { ticks_elapsed, target_entity, target_position, cached_path, phase: StepPhase, patrol_dir, no_move_ticks }` (identical shape to `FoxStepState`).
- Snake equivalents.

### 5. Step resolvers

#### `src/steps/hawk/mod.rs` (template: `src/steps/fox/mod.rs`)

One file `mod.rs` per fox precedent (the entire fox tree is currently a single file at `src/steps/fox/mod.rs`). Each `pub fn resolve_*` carries the **5 required rustdoc headings** (Real-world effect / Plan-level preconditions / Runtime preconditions / Witness / Feature emission) — `scripts/check_step_contracts.sh` greps for them.

Mapping `HawkGoapActionKind` → resolver:

| Action | Resolver | Witness shape | Feature | enrolled? |
|---|---|---|---|---|
| `SoarTo(zone)` | `resolve_soar_to(pos, step, map) -> StepOutcome<()>` | `()` | none | n/a |
| `SpotPrey` | `resolve_spot_prey(pos, prey_positions, step) -> StepOutcome<bool>` | `bool` (true iff prey visible this tick) | `Feature::HawkSpottedPrey` | **true** |
| `DiveAttack` | `resolve_dive_attack(pos, hawk_state, prey_positions, step) -> StepOutcome<Option<Entity>>` | `Option<Entity>` (kill target) | `Feature::HawkDiveLanded` | **true** |
| `Rest` | `resolve_rest(step, ticks_to_rest) -> StepOutcome<()>` | `()` | none | n/a |
| `FleeSky` | `resolve_flee_sky(pos, step, map) -> StepOutcome<()>` | `()` | none | n/a |

#### `src/steps/snake/mod.rs`

| Action | Resolver | Witness shape | Feature | enrolled? |
|---|---|---|---|---|
| `SlideTo(zone)` | `resolve_slide_to(pos, step, map) -> StepOutcome<()>` | `()` | none | n/a |
| `SetAmbush` | `resolve_set_ambush(snake_state, step) -> StepOutcome<()>` | `()` | none | n/a |
| `Strike` | `resolve_strike(pos, snake_state, prey_positions, step) -> StepOutcome<Option<Entity>>` | `Option<Entity>` | `Feature::SnakeStruckPrey` | **true** |
| `Bask` | `resolve_bask(snake_state, step, ticks_to_bask) -> StepOutcome<bool>` | `bool` (true on completion) | `Feature::SnakeBasked` | **true** |
| `Retreat` | `resolve_retreat(pos, step, map) -> StepOutcome<()>` | `()` | none | n/a |

#### Feature additions (`src/resources/system_activation.rs`)

New positive variants added to `Feature` enum, `Feature::ALL`, `feature_name`, and `category` (all `Positive`):

- `HawkSpottedPrey`, `HawkDiveLanded`, `HawkPerched`, `HawkFled`, `HawkDied`
- `SnakeStruckPrey`, `SnakeBasked`, `SnakeAmbushed`, `SnakeRetreated`, `SnakeDied`

**Never-fired canary classification** (`Feature::expected_to_fire_per_soak`):

- `expected_to_fire_per_soak() => true`: `HawkSpottedPrey`, `HawkDiveLanded`, `SnakeStruckPrey`, `SnakeAmbushed`. Hawks and snakes are edge-spawned with non-trivial probability and the seed-42 soak should reliably see at least one of each.
- `expected_to_fire_per_soak() => false`: `HawkPerched`, `HawkFled`, `HawkDied`, `SnakeBasked`, `SnakeRetreated`, `SnakeDied` — bursty / state-dependent ambient signals (matches existing `FoxStandoffEscalated`/`FoxRetreated`/`FoxDenEstablished` pattern at `system_activation.rs:528-533`).

**Update** the `expected_to_fire_per_soak_classification` test (`system_activation.rs:1133`) to include the new positive trunks.

### 6. Wildlife AI replacement

Files touched: `src/systems/wildlife.rs`.

**Migration order (atomic cutover deferred to final commit so intermediate commits stay green):**

1. Commits 1–5 add the GOAP systems and DSE wiring. The new `*_evaluate_and_plan` / `*_resolve_goap_plans` systems gate themselves behind `Without<HawkGoapPlan>` (etc.) on insertion, and on `With<HawkState>` on resolution. During the intermediate commits, hawks/snakes spawn WITHOUT `HawkState`/`SnakeState`, so the new systems are no-ops.
2. Commit 6 (cutover) does three things in one diff:
   a. Attach `HawkState`/`HawkAiPhase`/`HawkNeeds`/`HawkPersonality` (and snake equivalents) at spawn (see §7).
   b. In `wildlife_ai` (`src/systems/wildlife.rs:45-258`) gate the `Circling`/`Waiting` arms with `if !animal.species.uses_goap()` (or equivalently a `Without<HawkState>` query filter on the existing `Query`). The current `Without<FoxState>` filter expands to `Without<FoxState>, Without<HawkState>, Without<SnakeState>`. ShadowFox keeps the legacy state machine.
   c. Extend `predator_hunt_prey` (`wildlife.rs:639-757`) to apply satiation + hunger drop on `HawkState` and `SnakeState` mirror to the existing fox branch at `wildlife.rs:725-731`. Emit `Feature::HawkDiveLanded` / `Feature::SnakeStruckPrey` here when the `*AiPhase` matches `HuntingPrey`/`Striking` respectively.

**Surviving helpers** (NOT deleted): `is_patrol_terrain`, `is_spawn_terrain`, `pick_edge_spawn`, `initial_ai_state` (still used by `spawn_wildlife` to choose the *initial* `WildlifeAiState`, which is overwritten by the GOAP resolver on tick 1 but is kept so legacy systems and rendering have a sane default), and the `WildlifeAiState::Circling`/`Waiting` arms in `wildlife_ai` (still serve ShadowFox).

**Deleted/rewritten in commit 6:** the `Circling`/`Waiting` branches' filtering — they no longer execute for entities with `HawkState`/`SnakeState`. No outright function deletion is required; the disjointness is enforced by the query filter. A follow-on ticket (see §11) considers extracting the ShadowFox-only branches into a dedicated `shadow_fox_ai` system once hawks/snakes are off the path.

### 7. Spawn wiring

#### Edge spawner (`spawn_wildlife`, `src/systems/wildlife.rs:294-348`)

In the `commands.spawn((...))` tuple at `wildlife.rs:323-332`, branch on `species`:

```text
WildSpecies::Hawk => spawn additionally with:
  HawkState { hunger: 0.5, ..default },
  HawkAiPhase::Soaring { center_x, center_y, angle: 0.0 },
  HawkNeeds::default(),
  HawkPersonality::random(&mut rng.rng),

WildSpecies::Snake => spawn additionally with:
  SnakeState { hunger: 0.5, warmth: 0.7, ..default },
  SnakeAiPhase::Waiting,
  SnakeNeeds::default(),
  SnakePersonality::random(&mut rng.rng),
```

Fox / ShadowFox branches are unchanged.

#### Initial spawn (`spawn_initial_wildlife`, `src/systems/wildlife.rs:1098-1218`)

At the spawn loop `wildlife.rs:1202-1215` (the `for (species, pos, ai) in spawn_positions`), add per-species component bundles identical to the edge-spawner branch above. Use `world.spawn((...,)).id()` rather than `commands.spawn` since this runs on `&mut World`.

`spawn_initial_fox_dens` (`wildlife.rs:1224-1340`) is **not** touched — foxes are out of scope.

### 8. Registration

All edits in `src/plugins/simulation.rs` (single source of truth — no separate `build_schedule` paths exist; `SimulationPlugin` is consumed by both windowed `main.rs:89` and headless `main.rs:387`).

#### Messages (insert near `simulation.rs:140`)

```text
app.add_message::<crate::components::wildlife::HawkDiveLanded>();
app.add_message::<crate::components::wildlife::SnakeStrikeLanded>();
app.add_message::<crate::components::wildlife::HawkDied>();
app.add_message::<crate::components::wildlife::SnakeDied>();
```

#### DSE factories (`populate_dse_registry`, `simulation.rs:16-77`)

After the fox-DSE block at `simulation.rs:67-76`:

```text
registry.hawk_dses.push(dses::hawk_hunting_dse());
registry.hawk_dses.push(dses::hawk_fleeing_dse());
registry.hawk_dses.push(dses::hawk_resting_dse());
// Soaring is the implicit fallback — no DSE per Phase 1 design.

registry.snake_dses.push(dses::snake_ambushing_dse());
registry.snake_dses.push(dses::snake_foraging_dse());
registry.snake_dses.push(dses::snake_fleeing_dse());
registry.snake_dses.push(dses::snake_basking_dse());
```

Uses the existing `add_hawk_dse` / `add_snake_dse` extension methods only if a future caller registers a la carte — `populate_dse_registry` writes the slot directly, matching the fox precedent at `simulation.rs:67`.

#### Systems (in the wildlife `.chain()` at `simulation.rs:217-231`)

See §4 for the full insertion list. The 20-system tuple limit is the hard ceiling; current chain is 14, post-Phase-2 is 20.

#### Components

No explicit registration required — Bevy components are auto-registered when first used via `Query<&T>`.

### 9. SimConstants

New section in `src/resources/sim_constants.rs` adjacent to `FoxEcologyConstants` (currently `sim_constants.rs:2811-3000`). Header note: **adding fields to `SimConstants` breaks comparability with pre-Phase-2 `events.jsonl` headers** — `just verdict` will refuse to compare against pre-cutover baselines. Logged in §10.

The `SimConstants` struct (`sim_constants.rs:24-32`) gains two fields:

```text
#[serde(default)]
pub hawk_ecology: HawkEcologyConstants,
#[serde(default)]
pub snake_ecology: SnakeEcologyConstants,
```

Using `#[serde(default)]` so legacy save files still load (Default impls populate the new fields).

#### `HawkEcologyConstants` (new struct)

| Field | Type | Default | Doc-comment intent |
|---|---|---|---|
| `hunger_decay_rate` | `RatePerDay` | `0.15` | Hunger increase per in-game day. Hawks burn energy in flight; faster decay than fox. |
| `satiation_after_dive_kill` | `DurationDays` | `0.7` | Satiation duration after a successful dive. |
| `flee_health_threshold` | `f32` | `0.4` | Health fraction below which hawk switches to Fleeing. |
| `cat_avoidance_range` | `i32` | `4` | Tile range at which a hawk avoids healthy adult cats. |
| `dive_range` | `i32` | `6` | Tile range from which hawk initiates a dive. |
| `perch_search_radius` | `i32` | `15` | Tiles searched for a Perch zone. |
| `starvation_death_duration` | `DurationDays` | `2.0` | Sustained starvation before death. |
| `post_action_cooldown` | `DurationDays` | `0.4` | Cooldown after dive/flee. |
| `softmax_temperature` | `f32` | `0.15` | Disposition selection temperature (mirror `fox_softmax_temperature`). |
| `outnumbered_flee_count` | `usize` | `2` | Cats nearby that trigger flee. |

#### `SnakeEcologyConstants` (new struct)

| Field | Type | Default | Doc-comment intent |
|---|---|---|---|
| `hunger_decay_rate` | `RatePerDay` | `0.05` | Hunger decay; snakes are slow-metabolism, slower than foxes. |
| `warmth_decay_rate` | `RatePerDay` | `0.4` | Warmth decay when off warm terrain. |
| `bask_warmth_restore` | `f32` | `1.0` | Warmth set after a complete Bask. |
| `bask_duration` | `DurationDays` | `0.3` | Ticks to fully bask. |
| `satiation_after_strike_kill` | `DurationDays` | `2.0` | Snakes eat infrequently; satiate longer. |
| `flee_health_threshold` | `f32` | `0.5` | Snakes flee earlier than foxes. |
| `strike_range` | `i32` | `1` | Tile range for strike. |
| `cover_search_radius` | `i32` | `8` | Search radius for Cover zone. |
| `starvation_death_duration` | `DurationDays` | `5.0` | Long; snakes survive lean periods. |
| `post_action_cooldown` | `DurationDays` | `0.5` | Post-strike cooldown. |
| `softmax_temperature` | `f32` | `0.15` | Disposition selection temperature. |
| `cold_threshold` | `f32` | `0.3` | Warmth below which snake forces Basking disposition. |

**Comparability invariant**: every soak after Phase 2 lands writes a header with the new fields populated; pre-Phase-2 logs become incomparable. `just verdict` must be re-baselined against the first post-cutover seed-42 soak (handled by §10).

### 10. Verification plan

| Step | Pass criterion |
|---|---|
| `just check` | step-contract grep finds 5 rustdoc headings on each new `pub fn resolve_*`; time-units lint clean. |
| `cargo nextest run --features all` | all green. New tests:<br>`tests/hawk_goap_smoke.rs` — 1 hawk in a sandbox world, runs ≥100 ticks, asserts `HawkGoapPlan` is inserted and progresses.<br>`tests/snake_goap_smoke.rs` — symmetric.<br>Inline tests in each new step resolver mirroring `src/steps/fox/mod.rs:170-220` (advance-on-target, fail-without-target, durations).<br>Inline tests in `src/systems/{hawk,snake}_goap.rs` mirroring fox patterns. |
| `just soak 42` → `logs/tuned-42/` | runs without crash, header includes new SimConstants fields, footer written. |
| `just verdict logs/tuned-42` | hard gates: `deaths_by_cause.Starvation == 0`, `deaths_by_cause.ShadowFoxAmbush <= 10`, `never_fired_expected_positives == 0` (the new `HawkSpottedPrey`/`HawkDiveLanded`/`SnakeStruckPrey`/`SnakeAmbushed` must each fire ≥1×). |
| Continuity canaries | grooming · play · mentoring · burial · courtship · mythic-texture each ≥ 1, unchanged. |
| Drift expectations | `deaths_by_cause.WildlifeCombat` (currently `0.9 ± 0.9` at `docs/balance/healthy-colony.md:30`) is the most likely metric to move — direction unclear (more intelligent hunting could raise it; smarter fleeing could lower it). **Predicted drift: within ±30% of baseline (well under the ±10% hypothesis-required threshold given the ±100% baseline noise band).** Other metrics — `food_fraction`, `safety`, mood medians — should not move >±10% because hawks/snakes don't directly interact with cat food economy or mood except via combat. If `WildlifeCombat` drift exceeds ±30%, the four-artifact methodology applies (`just hypothesize`); otherwise document the observed delta in a new `docs/balance/wildlife-goap-cutover.md` as a single-iteration baseline shift. |
| `never_fired_expected_positives == 0` | enforced by the hard gate above; ticking the new positive features off requires the seed-42 soak to actually exercise hunting + ambush. If the soak is too short to witness, lower `hawk.spawn_chance` floor or drop `enrolled` to `false` (do NOT exempt without evidence). |

### 11. Out of scope / follow-on tickets

Per CLAUDE.md, each subscope below is opened as a concrete ticket in the same commit that lands the parent. Proposed titles:

- **`tickets/NNN-hawk-snake-balance-iteration.md`** — first tuning pass on `HawkEcologyConstants` / `SnakeEcologyConstants` after a multi-seed sweep. `status: ready`, `blocked-by: [025]`. Cites `docs/balance/wildlife-goap-cutover.md` for the post-cutover baseline.
- **`tickets/NNN-hawk-snake-perceptual-facts.md`** — author `docs/systems/hawk-ecology.md` and `docs/systems/snake-ecology.md` perceptual-fact docs once behavior is observed in the wild. `status: ready`, `blocked-by: [025]`.
- **`tickets/NNN-rebuild-sensitivity-map-post-wildlife-goap.md`** — `just rebuild-sensitivity-map` to bring the 22 new SimConstants fields into `just explain`'s rho column. `status: ready`, `blocked-by: [025]`.
- **`tickets/NNN-extract-shadow-fox-ai-system.md`** — pull the ShadowFox-only branches of `wildlife_ai` into a dedicated `shadow_fox_ai` system; the function is the only legacy state-machine path remaining post-cutover. `status: ready`, `blocked-by: [025]`.
- **`tickets/NNN-hawk-snake-goap-narrative-coverage.md`** — narrative templates for `HawkSpottedPrey`, `HawkDiveLanded`, `SnakeStruckPrey`, `SnakeBasked`, `SnakeAmbushed` so the writer's-toolkit shows non-empty narration on these features. `status: ready`, `blocked-by: [025]`.

### 12. Risks

- **Bevy 16-param limit.** Both `*_evaluate_and_plan` will breach 16 params unless wrapped in `#[derive(SystemParam)]` bundle structs (`HawkPlanQueries`, `HawkPlanCtx`). The fox version (`fox_goap.rs:379`) currently sits at the limit using `#[allow(clippy::too_many_arguments)]` plus a dedicated marker query — same pattern works here, but bundling is cleaner. Predict: introduce `HawkPlanCtx`/`SnakePlanCtx` SystemParam structs from day 1.
- **Query disjointness.** `wildlife_ai`, `predator_stalk_cats`, `predator_hunt_prey` all query `&WildAnimal` mutably or with `Position`. New GOAP systems also touch `WildAnimal`/`Position`/`Health`. Each new system must use `Without<FoxState>, Without<HawkState>, Without<SnakeState>` filters appropriately, and the legacy `wildlife_ai` query at `wildlife.rs:46` widens its `Without` set in commit 6. Conflict scan via `cargo check` after the cutover commit is mandatory.
- **Comparability-invariant break.** Adding `hawk_ecology` + `snake_ecology` to `SimConstants` means *every* `events.jsonl` written before this lands becomes incomparable. The cutover commit must include a fresh `just promote logs/tuned-42 wildlife-goap-cutover` to lock in the new baseline (`docs/balance/healthy-colony.md` will need an iteration note). `just verdict` against any pre-cutover log will refuse on header-constants mismatch — expected and correct.
- **Never-fired canary trap.** Enrolling `HawkSpottedPrey` etc. as `expected_to_fire_per_soak() => true` makes them hard gates. If hawks fail to spawn on seed-42 (low `spawn_chance`, edge-terrain availability), the canary fires RED. Verify by running the soak BEFORE enrolling, then promote to `true` only after observing ≥1 fire. If a feature legitimately may not fire in 15 min, keep it `false` and leave a comment explaining why (matches `FoxStandoffEscalated` precedent).
- **Determinism.** The new GOAP systems consume `SimRng` (softmax selection, jitter). The wildlife `.chain()` is single-threaded already (`simulation.rs:122`), so insertion order is the only RNG-determinism concern. Mirror fox's RNG-consumption order: scoring → softmax → planner state build → `make_plan`.
- **Population at-spawn shape.** Edge-spawned hawks/snakes start with `*Personality::random` consuming RNG; this shifts the SimRng sequence relative to pre-cutover runs and *is* the comparability break. Documented above.
- **`predator_hunt_prey` extension.** The fox-only `Feature::FoxHuntedPrey` emission at `wildlife.rs:728` is in a hot path. Adding parallel hawk/snake branches there couples the kill-attribution Feature emission to a system the resolvers don't own. **Decision:** keep kill-attribution in `predator_hunt_prey` (consistent with fox), do NOT emit those Features from the resolvers. The resolver emits `HawkDiveLanded` (the dive *attempt*) and the kill-attribution is `predator_hunt_prey`'s job. This means `HawkDiveLanded` may fire without a prey kill — that's correct: it's the dive event, not the kill event.

### 13. Commit sequence

Each commit must leave `just check` and `cargo nextest run --features all` green.

1. **`feat: add HawkState/SnakeState lifecycle components`** — new types in `src/components/wildlife.rs`; add `Component` derive to `HawkNeeds`/`HawkPersonality`/`SnakeNeeds`/`SnakePersonality` in-place; new `WildlifeDeathCause` enum; new message types (registered in plugin). No spawn-side changes yet — types exist but aren't attached. Tests: component construction, default values.
2. **`feat: add HawkGoapPlan/SnakeGoapPlan plan components`** — new files `src/components/hawk_goap_plan.rs`, `snake_goap_plan.rs`; mirror `fox_goap_plan.rs`. Tests: advance, replan, exhaustion (mirror `fox_goap_plan.rs:127-180`).
3. **`feat: add hawk/snake step resolvers under src/steps`** — new files `src/steps/hawk/mod.rs`, `src/steps/snake/mod.rs` with all 10 `pub fn resolve_*` carrying the 5 rustdoc headings; new `Feature` variants registered (NOT yet enrolled — `expected_to_fire_per_soak` returns `false` for now). Tests: per resolver advance/fail/timeout cases.
4. **`feat: add hawk_goap and snake_goap systems`** — new files `src/systems/hawk_goap.rs`, `snake_goap.rs`. Wire into the `SimulationPlugin` chain. The systems are no-ops because no entity has `HawkState`/`SnakeState` yet. Add `populate_dse_registry` calls for the existing hawk/snake DSEs. Tests: smoke (`tests/hawk_goap_smoke.rs`, `tests/snake_goap_smoke.rs`) spawning a synthetic hawk + verifying plan insertion.
5. **`feat: add HawkEcologyConstants and SnakeEcologyConstants`** — extend `src/resources/sim_constants.rs` with the two new structs and `#[serde(default)]` slots in `SimConstants`. Wire into `hawk_needs_tick` / `snake_needs_tick` (which still no-op until commit 6 attaches the components). Tests: defaults, serde round-trip with legacy header (forward-compat).
6. **`refactor: cut hawks and snakes over to GOAP`** — the atomic cutover. Edit `spawn_wildlife` and `spawn_initial_wildlife` to attach `HawkState`/`HawkAiPhase`/`HawkNeeds`/`HawkPersonality` (and snake equivalents). Widen `wildlife_ai`'s `Without<FoxState>` filter to also exclude `HawkState`/`SnakeState`. Extend `predator_hunt_prey` to apply hawk/snake satiation. Promote the four common positive Features (`HawkSpottedPrey`/`HawkDiveLanded`/`SnakeStruckPrey`/`SnakeAmbushed`) to `expected_to_fire_per_soak() => true`. Run `just soak 42`, observe firing, run `just verdict`, run `just promote logs/tuned-42 wildlife-goap-cutover`. Open the five follow-on tickets enumerated in §11 in the same commit.
7. **`docs: append wildlife-GOAP-cutover iteration to balance log`** — append to `docs/balance/healthy-colony.md` (or new `docs/balance/wildlife-goap-cutover.md`) recording the metric drift observed in commit 6's verdict. Updates `docs/wiki/systems.md` via `just wiki`. Closes ticket 025 (`status: done`, `landed-at: <sha>`, move to `landed/`).
