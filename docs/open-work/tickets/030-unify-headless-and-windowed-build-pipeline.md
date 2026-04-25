---
id: 030
title: Unify headless and windowed build pipeline — kill the manual mirror
status: ready
cluster: null
added: 2026-04-25
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

The headless-mirror-drift pattern is now a documented recurring
failure mode (CLAUDE.md → "Headless Mode" + "Simulation Verification"
sections). Three regressions have surfaced from it in 2026 alone:

- **2026-04-21:** `bevy_ecs::message::message_update_system` not
  registered in headless schedule, breaking every `Message<T>` flow.
  Patched by adding the system + every `MessageRegistry::register_message`
  call to the headless setup.
- **2026-04 ongoing:** Three `add_dse` mirror sites
  (`SimulationPlugin::build` + headless `build_schedule` fresh-world
  + headless `build_schedule` save-load path) per the AI substrate
  refactor's port workflow. CLAUDE.md flags this explicitly as
  *"DSE registration sites: three"*.
- **2026-04-25** (just diagnosed in ticket 028): four personality-
  event observers (`on_play_initiated`, `on_temper_flared`,
  `on_directive_refused`, `on_pride_crisis`) silently never run
  in headless because `register_observers` was never mirrored from
  the plugin path. Has been broken since the observer pattern was
  introduced.

Each regression is mechanically the same: a registration that landed
in `SimulationPlugin::build` was not copied into
`src/main.rs::build_schedule`, and the bug is silent (no panic, no
warning — just behaviorally invisible cats / silent message channels
/ unregistered DSEs that score zero). The dataset captured by
`logs/baseline-2026-04-25/` is partially compromised by exactly
this pattern: the `play` continuity canary reports zero not because
play doesn't fire, but because the observer that records it doesn't
exist in headless.

The cost compounds with every new system/observer/message landing.
Continuing to patch each instance individually is treating the
symptom; this ticket is the cure.

## Scope

Refactor headless to use the same `App` + plugin pipeline as the
windowed build, parameterized by which Bevy plugins are mounted.

### End-state architecture

- **`SimulationPlugin`** — unchanged in role, but becomes the *sole*
  authoritative registration site for messages, observers, DSEs,
  shared resources, and FixedUpdate systems. Every present and
  future "register X" call lives here, period.
- **`HeadlessIoPlugin`** — new. Owns headless-only
  responsibilities: argv parsing, JSONL writers (events, narrative,
  optional trace sidecar), focal-cat resource insertion, tick-budget
  exit. Headless inserts this plugin in addition to `SimulationPlugin`.
- **`RenderingPlugin`** (or whatever the windowed-only systems
  bundle is called) — owns sprite rendering, camera, F-key overlays,
  HUD. Windowed inserts this in addition to `SimulationPlugin`.
- **`main.rs::main()`** — branches on `--headless`:

  ```text
  let mut app = App::new();
  if args.headless {
      app.add_plugins(MinimalPlugins);
      app.add_plugins(SimulationPlugin);
      app.add_plugins(HeadlessIoPlugin::from(args));
  } else {
      app.add_plugins(DefaultPlugins);
      app.add_plugins(SimulationPlugin);
      app.add_plugins(RenderingPlugin);
  }
  app.run();
  ```

  The headless tick-budget enforcement happens via a system that
  writes `AppExit::Success` on the message bus when
  `time.tick >= target`. No more manual `for _ in 0..N { schedule.run(world) }`.

### Resource setup

The headless `setup_world` body — terrain generation, cat spawn,
ColonyKnowledge / ColonyPriority / ColonyHuntingMap initialization,
SimConstants insertion, DSE registry build, modifier pipeline build
— moves into a `Startup`-scheduled system or two. Order is
expressed via Bevy `SystemSet` chaining (e.g.
`SetupWorld → SpawnCats → RegisterDses`) rather than top-to-bottom
function flow.

The save/load path (`--load <path>`) becomes a *fourth* startup
system that runs conditionally on the CLI flag, replacing the
seeded-world generation. This naturally absorbs the third manual-
mirror site (the save-load DSE registration).

### Migration order

1. **Step 1 (cheap, no behavior change):** make `SimulationPlugin`
   register-everything. Move the post-build `app.insert_resource(...)`
   calls in `main.rs:78ish` into the plugin. Verify windowed build
   unchanged (no system added, no system removed, no resource missed).
2. **Step 2:** introduce `HeadlessIoPlugin`. Move JSONL writer setup,
   CLI flag wiring, and focal-trace resource inserts into it.
3. **Step 3:** introduce a startup system pipeline for world
   generation. Drop in headless first; verify against the
   2026-04-25 baseline dataset on a fresh run.
4. **Step 4:** delete `build_schedule()`. The function and its
   manual-mirror burden disappear. Update CLAUDE.md to remove the
   "manual mirror" warnings.
5. **Step 5:** retire the regression-guard test from ticket 028.
   The architectural fix supersedes it.

### Bevy 0.18 specifics

- Observers are registered via `app.add_observer(...)` (already
  used in `register_observers`). Once `SimulationPlugin` owns this,
  `World::add_observer` direct-from-headless becomes unnecessary.
- Messages: `app.add_message::<T>()` is the single registration
  path. The `MessageRegistry::register_message` + manual schedule
  insertion of `message_update_system` go away.
- DSE registry: stays a normal `Resource`. `App::insert_resource`
  before `Startup` runs is fine; the resource is then mutated by
  startup systems.
- App tick rate: headless wants determinism without real-time
  pacing. Bevy's `FixedUpdate` schedule with
  `Time<Fixed>::set_timestep` to a fixed step + manual `app.update()`
  loop in headless gives the equivalent of today's
  `schedule.run(&mut world)` without parallel-scheduler differences.

## Out of scope

- The actual rendering plugin — windowed mode already works; the
  refactor preserves it untouched.
- Distinguishing systems that run in `FixedUpdate` vs. `Update`.
  Today's `build_schedule` puts everything in one schedule; this
  ticket preserves that, with the option to split later if a
  performance reason emerges.
- Test-harness `setup_world` calls in
  `tests/{ecology,combat,goap_*}/...`. Test setup uses raw `World`
  + `Schedule` and may stay that way; the regression class for
  tests is bounded by `cargo test` which fails noisily, unlike the
  silent headless-mirror drift.

## Current state

**Diagnostic:** the manual mirror has produced three landed
regressions in two months. Ticket 028's specific patch is in flight;
this ticket is the structural fix that prevents the next ten.

**Reference points in code:**
- `src/plugins/simulation.rs` — current plugin definition.
- `src/main.rs:78` — current windowed App build.
- `src/main.rs:376–971` — current headless World/Schedule build
  (the manual mirror).
- `src/main.rs:545` — `build_schedule` function definition.

## Approach

Land the five steps as separate commits, each verified against
the baseline dataset. Step 1 is the riskiest (it's a no-op
behavior change spanning a lot of code); Steps 2–4 are mechanical
once Step 1 holds. Step 5 is a documentation update.

The first verification gate is fresh baseline-dataset capture
post-Step-1: header parity must hold (same `commit_hash` across
the new build path), survival canaries must hold, every continuity
tally must be in the same envelope as the 2026-04-25 dataset
(no new DSEs missing from registration, no observers dropped).

## Verification

After each step:

1. `cargo check --all-targets && cargo clippy --all-targets --all-features -- -D warnings`.
2. `cargo test` — all integration tests green.
3. `just baseline-dataset 2026-04-26-pipeline-step<N>` — full
   27-run capture.
4. Diff REPORT.md against `logs/baseline-2026-04-25/REPORT.md`.
   Survival canaries match; continuity tallies match within ±2σ;
   header parity holds.
5. `just run` — windowed build still launches and is interactive.

After Step 4 (`build_schedule` deleted):

6. `git grep build_schedule` returns zero hits.
7. CLAUDE.md's Headless Mode section is rewritten to drop the
   manual-mirror caveat.

## Log

- 2026-04-25: Ticket opened. User flagged the recurring nature of
  this drift after ticket 028 surfaced the play-canary regression.
  Three documented regressions in 2026 (message-update system,
  DSE 3-site registration, observer cascades).
