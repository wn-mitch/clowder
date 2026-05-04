---
id: 162
title: Scenario harness — fast deterministic "what wins" experiments for AI decision triage
status: done
cluster: tooling
added: 2026-05-04
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 9d96235
landed-on: 2026-05-04
---

## Why

Bugfix triage today forces a full 15-minute `just soak` (or at minimum a
`just headless` with whole-colony spawn) every time we want to ask a small
question like "kitten cries near three cats — who commits to caretake?".
The cost makes hypothesis iteration painful and pushes the model toward
broad balance-style sweeps even when the question is narrow and structural.

The fix is a **scenario microexperiment harness**: spawn a tiny world (1–5
cats) with preloaded needs / personality / markers / position, optionally
seed influence-map cells, tick a handful of times, and read out which DSE
won and why. Wall-clock target ~3 seconds, vs. ~15 minutes for `just soak`.

The harness's value is not theoretical — there is an active kitten bugfix
loop in flight (ticket 158 + 161 work) that is paying the soak cost
repeatedly. Shipping `kitten_cry_basic` unblocks that loop on the day this
PR lands.

## Scope

1. **Foundation infrastructure:**
   - `WorldSetup` resource (enum: `Default | Scenario(closure)`) consulted by
     `setup_world_exclusive` (`src/plugins/setup.rs:88`); when `Scenario` is
     present, skip `build_new_world` and call the closure to populate the
     world.
   - Factor `build_founder_bundle()` out of `src/plugins/setup.rs:289-330`
     and `build_kitten_bundle()` out of `src/systems/pregnancy.rs:106-158`
     so `CatPreset::default_adult` / `default_kitten` call the same
     constructors the real spawn code does (drift control).
   - `CatPreset` builder over those bundles. Setters for needs / personality
     / markers / position / age / gender / kinship.
   - Environment helpers: `flat_grass_terrain`, `spawn_prey_at`,
     `spawn_herb_patch_at`, `mark_tile_corrupted`, `spawn_fox_at`.
   - `Scenario` struct (`name`, `default_focal`, `default_ticks`, `setup`)
     and a runner that builds the App, inserts `FocalTraceTarget`, ticks N
     times, and returns a `ScenarioReport` exposing per-tick winning
     disposition and the L2 score table.
   - `src/bin/scenario.rs` CLI binary + `just scenario <name>` recipe.
   - Drift CI check: a unit test asserting `CatPreset::default_adult()`
     inserts the same component archetype as the real founder spawn.

2. **Seven proof-of-concept scenarios** (one per archetype, sequenced
   easiest-to-hardest so unblock-value lands early):
   - `kitten_cry_basic` (ship-first; unblocks active kitten bugfix loop)
   - `wildlife_fight`
   - `fondness_kitten_imprint`
   - `hunt_acquisition_to_kill`
   - `exploration_ranging`
   - `ward_placement`
   - `farming_cycle` (may need `--time-scale Nx` flag for runner)

   Each scenario gets a corresponding `#[test]` in `tests/scenarios.rs`
   asserting the expected winning DSE for the focal cat at the appropriate
   tick. These tests are guards for future bugfixes: a structural change
   that regresses any of these archetypes fails the test suite cheaply.

3. **CLAUDE.md discipline update** (same PR):
   - Add `just scenario <name>` to the **Verifying a change** command list.
   - Insert a "Before running a soak, set up a scenario microexperiment"
     step in the **Bugfix discipline** section between the layer-walk audit
     and listing fix candidates. Without this wiring, the model keeps
     defaulting to `just soak` for triage and the harness rots.

## Out of scope

- **No replacement for `just soak`.** Survival canaries, drift, mythic-
  texture continuity all require full-world dynamics — those still live on
  the soak path.
- **No YAML scenario format.** Personality has 18 axes, needs 10 fields,
  markers 30+ ZSTs. Rust builder is more honest about the surface than a
  flat YAML file would be.
- **No directorial multi-step scripting** ("on tick 5, drop hunger to 0.1").
  Scenarios are static at-spawn state. If multi-step ever becomes needed,
  add a `Vec<(tick, fn(&mut World))>` field to the `Scenario` struct.
- **No termination predicates** ("stop when prey dies"). The default-ticks-
  per-scenario approach covers the listed archetypes; predicates can be a
  v2 follow-on.

## Approach

Sequencing — separate commits in the same push so each can be reverted
independently:

1. This ticket (first commit).
2. Foundation commit — `WorldSetup`, factored bundles, `CatPreset`, env
   helpers, runner, CLI, recipe, drift CI test.
3. `kitten_cry_basic` — unblocks active kitten bugfix loop. Lands next.
4. `wildlife_fight` + `fondness_kitten_imprint` — easy archetypes.
5. `hunt_acquisition_to_kill` + `exploration_ranging` — medium difficulty;
   require reading `src/systems/wildlife/` and `src/ai/dses/explore.rs`.
6. `ward_placement` + `farming_cycle` — hardest; cross multi-tick state
   machines (gather → place; plant → tend → harvest). Likely to surface
   harness-API gaps; if so, fold fixes back into the foundation commit.
7. CLAUDE.md update + open-work index regen — final commit.

The keystone for cheap subsequent scenarios is **`CatPreset` drift
control**. The founder bundle has ~25 components; if `CatPreset` misses one
(e.g., `SensorySpecies`, `RecentTargetFailures`), scenarios silently
produce wrong scoring with no error. Mitigations baked into the foundation
commit: extract a shared bundle constructor, add a drift CI test that
fails loudly if anyone adds a component to founder spawn without updating
the preset.

## Verification

- `cargo test --test scenarios` — all 7 scenarios green with their
  winning-DSE assertions, plus the drift CI check.
- `just scenario kitten_cry_basic` — prints score table in <5s wall-clock
  on release.
- `just scenario hunt_acquisition_to_kill` — observes a complete locate→
  stalk→pounce→kill sequence in the trace.
- `just check` — passes step-resolver / time-unit linters (no new
  resolvers; should be unaffected).
- Determinism cross-check: run any scenario twice with the same seed,
  diff stdout — byte-identical.
- Wall-clock budget: every scenario except `farming_cycle` runs in <10s on
  release; `farming_cycle` <60s with `--time-scale Nx` if needed.

## Log

- 2026-05-04: opened. Plan drafted at
  `~/.claude/plans/one-thing-i-m-noticing-vectorized-hopcroft.md`. User
  accepted "ship all 7 archetypes as proof-of-concept" expansion over the
  initial "ship kitten_cry only and defer rest" v1. Rationale: a harness
  that only handles one scenario is not yet proven; doing all 7 stress-
  tests the API across diverse subsystems (caretake, combat, hunt,
  herbcraft, farming, exploration, social) before locking it in.
- 2026-05-04: foundation + all 7 scenarios landed. Harness wall-clock
  is **0.02–0.03s per scenario** vs 15min for `just soak`. Surfaced
  on first run: warm/compassionate/adjacent-parent Mallow ranks
  Caretake at 0.10 (below Wander 0.43, Forage 0.62, Socialize 0.64,
  Groom 0.36) — exactly the bug class the parallel kitten-bugfix
  session is investigating. Discovered & fixed a focal-resolution
  timing bug: `target.entity` is None until `emit_focal_trace` runs,
  but scoring runs first, so cats committing to multi-tick plans on
  tick 1 never re-score. Runner now does a Startup-only `app.update()`
  followed by an explicit name→entity resolve before counting ticks,
  which made hunt / exploration / ward / fondness all start emitting
  L3 records on tick 1. CLAUDE.md updated to require `just scenario`
  before `just soak` for hypothesis triage.
