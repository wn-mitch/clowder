# Regression-measurement determinism

Re-running the same seed on the same binary on the same machine produces a
**byte-identical `events.jsonl`**. That property is what lets `just verdict`
and the rest of the regression-measurement tooling treat re-runs as a true
control — any drift in a re-run is now code drift, not schedule jitter.

This doc is about the *kind* of determinism the project guarantees and the
kind it deliberately doesn't, so balance work and regression chasing don't
talk past each other.

## What is guaranteed

**Same seed + same binary + same machine → byte-identical
`events.jsonl` over any common-prefix tick range.**

Verified on every commit by `tests/integration.rs::simulation_is_deterministic`
(wired into `just ci` via `just check-determinism`). The test runs the
canonical headless path twice with seed 42 and asserts the two
`events.jsonl` files are equal byte-for-byte after a fixed number of ticks.

The mechanism is four pieces working together:

1. **Single-threaded `FixedUpdate` and `Startup` schedules.**
   `SimulationPlugin::build` pins both to `ExecutorKind::SingleThreaded`.
   Bevy's `MultiThreadedExecutor` topologically-sorts the conflict graph but
   the chosen order varies across processes wherever the graph admits
   alternatives — so the standalone-systems group at
   `simulation.rs:425-444` was reordering across runs and shifting the
   `SimRng` consumption sequence. Single-threaded execution forces a stable
   topo order; the throughput cost is negligible for a ~50-cat sim.
2. **`BTreeMap` instead of `HashMap` wherever iteration order matters.**
   `HashMap` uses `RandomState` keyed on per-process OS entropy, so its
   iteration order is not stable across runs. The known-load-bearing
   conversions are:
   - `SystemActivation::counts` — iterated and `.sum()`'d into the colony
     activation score; `HashMap` order produced 1-ULP `f64` drift that
     propagated through `ColonyScore::aggregate` and downstream sorts.
   - `Memory::most_frequented` — `max_by_key` returns the last-iterated max
     on ties, so the chosen tile varied per process.
   - `ZoneDistances::distances` — iterated to build the GOAP planner's
     action list; the action list seeds the A* open-list insertion order,
     which is the equal-`f`-cost tiebreak. `HashMap` order let same-seed
     runs pick `TravelTo(Kitchen)` vs `TravelTo(ForagingGround)` for the
     same cat-state.
   - `Relationships::data` — `all_for(entity)` summed `f32`
     fondness/familiarity in iteration order; `HashMap` order produced
     1-ULP drift in `social_weight` for coordinator-election scoring.
   - `EventLog`'s tally maps and `EventKind::SystemActivation`'s emitted
     maps — output-only but `BTreeMap` for stable JSON key order in
     `events.jsonl`, so byte-level comparison works.
3. **Centralized `ChaCha8Rng`.** `SimRng` (`src/resources/rng.rs`) is the
   one RNG. Every system reads from `Res<SimRng>`/`ResMut<SimRng>`; no
   `thread_rng()`, no `rand::random()`, no per-system `StdRng::seed_from_u64`
   in the tick loop. This was already in place before the determinism
   work — the bug was upstream of the RNG.
4. **No wall-clock reads in the tick loop.** `Instant::now()` is captured
   once at startup as a budget anchor (`HeadlessRunStart`) and read only by
   the wall-time exit gate, never by simulation systems.

## What is *not* guaranteed

**Cross-commit determinism.** Two commits that change the system schedule
(add or remove a system, change a `.chain()`, reorder a `.before()`) will
walk different trajectories on the same seed *by design*. Adding a system
inserts new RNG-consumption points; removing one collapses some. Both are
correct; both diverge. This isn't a bug to chase — it's a property of
RNG-streamed-by-system-order semantics.

The frequent symptom: a refactor that's "just registering a new system"
visibly shifts seed-42 tallies. The shift comes from the schedule edge,
not from the new system's behavior. The codebase already documents
several incidents in `docs/open-work/landed/2026-04.md` where what looked
like balance regressions were structural ECS-noise.

**Cross-machine determinism.** Floating-point determinism across CPU
families isn't promised either — same binary, different ISA can yield
different `f32`/`f64` rounding under fused-multiply-add and similar. In
practice the project runs on macOS aarch64 + Linux x86_64 and we've not
seen drift, but the property isn't tested.

## Implication for regression measurement

- **Single-seed deltas are trustworthy *within a binary*.** A re-run is a
  true control. If `just verdict` reports a 30% delta on
  `deaths_by_cause.Starvation` between two seed-42 runs of the same
  commit, that delta is a real measurement of something — not noise.
- **Cross-commit, use multi-seed sweeps + Welch's t.** A single-seed
  cross-commit comparison is dominated by schedule-induced trajectory
  reshuffling. Use `just sweep` + `just sweep-stats --vs <baseline>` so
  the per-metric deltas average over schedule noise, and the effect-size
  bands (`significant` requires `|Δ| ≥ 30% AND p < 0.05 AND
  |Cohen's d| > 0.5`) discriminate real signal from reshuffling.
- **Seed mismatches are now caught.** `just verdict` and `just sweep-stats
  --vs` warn when the observed run's header `seed` doesn't match the
  baseline's; this used to be a silent failure mode where the drift table
  was bogus and nobody noticed.

## Failure mode if the test ever flips

If `just check-determinism` starts failing after a commit, **a new source
of nondeterminism crept into the sim path**. Most likely candidates,
ranked by historical frequency:

1. A new `HashMap`/`HashSet` whose iteration order leaks into a `f32`/`f64`
   sum, a `max_by_key`/`min_by_key`, or anything writing to game state.
   Replace with `BTreeMap`/`BTreeSet`, or sort before iterating.
2. A new system registered without ordering constraints in the standalone
   group, where the conflict graph admits multiple topo orders. Single-
   threaded execution should make this a non-issue, but it's worth
   sanity-checking.
3. A `thread_rng()` / `rand::random()` / `Instant::now()` call snuck into
   a tick-loop system. Grep for them.
4. A `par_iter` or `rayon` pool spawned in a hot loop.

Bisect over the `events.jsonl` diff: pin the first divergent event with
`cmp` + `jq -cS`, identify which system emitted it, and audit that
system's collections and RNG usage.

## Cross-references

- `tests/integration.rs::simulation_is_deterministic` — the gate.
- `src/plugins/simulation.rs` — `ExecutorKind::SingleThreaded` pin.
- `src/resources/system_activation.rs`, `src/resources/relationships.rs`,
  `src/components/mental.rs`, `src/ai/planner/mod.rs`,
  `src/resources/event_log.rs`, `src/systems/colony_score.rs` — the
  `BTreeMap` conversions.
- CLAUDE.md, "Verification" section — the comparability invariant
  ("runs are only comparable iff their headers match on `constants` and
  carry the same non-dirty `commit_hash`") refers to the same property
  documented here.
