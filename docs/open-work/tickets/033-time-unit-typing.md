---
id: 033
title: Time-unit typing — make ticks ↔ in-game time ↔ wall-clock a compile-time invariant
status: in-progress
cluster: null
added: 2026-04-26
parked: null
blocked-by: []
supersedes: []
related-systems: [time.md]
related-balance: [time-anchor-iteration-1.md]
landed-at: null
landed-on: null
---

## Why

Tick timing in Clowder is "all over the place" — there is no consistent "1 in-game day = X real time" anchor, which makes balance work impossible to peg. An audit of the current state surfaced three converging problems:

1. **Old-scale leftovers from the 2026-04-10 100→1000 ticks/day overhaul** survived because nothing forced consumers through a converter:
   - `CoordinationConstants::evaluate_interval = 100` (`src/resources/sim_constants.rs:2809`) — fires 10× per in-game day instead of once.
   - `AspirationConstants::second_slot_check_interval = 100` (`sim_constants.rs:2902`) — same shape.
   - `FertilityConstants::update_interval_ticks = 100` (`sim_constants.rs:2951`) — legacy comment in `src/systems/fertility.rs:7` still says "(100 ticks default)" from the old era.
   - `FertilityConstants::cycle_length_ticks = 10000` — hardcoded as raw ticks, not parameterized as a fraction of `ticks_per_season`.
   - `FoxEcologyConstants::scent_decay_per_tick = 0.0001` vs `PreyConstants::scent_decay_per_tick = 0.02` — 200× discrepancy.

2. **Headless and windowed disagree on tick rate.** Headless = 60 Hz (`src/main.rs:345-349`). Windowed = 1/5/20 Hz (`src/main.rs:116-122`). No shared anchor, no way to know that windowed Normal ticks 60× slower than headless.

3. **No real-time peg.** `events.jsonl` `_header` records `ticks_per_day_phase` and `ticks_per_season` but **not** the headless tick rate, so downstream tooling cannot reconstruct wall-clock time.

The structural fix: turn the discipline problem into a compile error, the same playbook as `StepOutcome<W>` + `check_step_contracts.sh`. Constants declared in canonical units (per in-game day / days / seasons) flow through a typed converter — the type system refuses raw `f32` arithmetic, and a CI lint catches the long tail.

## Scope (phased)

Each phase is one commit + landing entry + canary verification.

- **Phase 0 (this work) — foundation.** New `src/resources/time_units.rs` module (`RatePerDay`, `DurationDays`, `IntervalPerDay`, `DurationSeasons`, `Ticks` newtypes); new `TimeScale` resource derived from `SimConfig` plus user-facing `wall_seconds_per_game_day` peg; `--game-day-seconds N` CLI flag; both headless and windowed `Time<Fixed>` Hz derive from `TimeScale::tick_rate_hz()`; `events.jsonl` header gains `wall_seconds_per_game_day` and `headless_tick_rate_hz`; `scripts/check_time_units.sh` permissive gate wired into `just check`. **No constant migrations yet** — Phase 0 is a behavioral no-op.
- **Phase 1 — confirmed bugs.** Migrate the three 100-tick stragglers + fertility cycle parameterization + Fox/Prey scent reconciliation. Tied out per balance methodology in `docs/balance/time-anchor-iteration-1.md`.
- **Phase 2 — drain/decay cluster.** All `NeedsConstants` rates as `RatePerDay`. Behavioral no-op at default scale.
- **Phase 3 — magic & mood cluster.** `MagicConstants`, `MoodConstants` (sub-audit on the 5-/10-tick durations).
- **Phase 4 — combat/prey/fox/food cluster.** Largest single cluster.
- **Phase 5 — life-stage & test scaling.** Eliminate the test-only `TICKS_PER_SEASON = 2000` divergence in `src/components/identity.rs:148` and `src/world_gen/colony.rs:466`.
- **Phase 6 — tighten gate.** Remove `scripts/time_units_allowlist.txt`; raw `% N` near tick code becomes a hard fail.

## Out of scope

- Movement speeds (stalk/approach/chase) — spatial per-tick, not temporal. Stay raw `f32`.
- Probabilities (`pounce_base_rate`, `den_refill_base_chance`) — unitless. Stay raw.
- Save-game format — typed wrappers serialize as their inner `f32`/`u64`, identical bytes.
- Per-rate balance tuning — drift > ±10% requires a `time-anchor-iteration-1.md` four-artifact entry; no scope-creep into the typing work itself.

## Approach

User-confirmed via AskUserQuestion 2026-04-26:
- Compile-time enforcement via newtypes (no `Default::default()`-with-magic-value, no `From<f32>`/`Into<f32>`; explicit named `.per_tick(ts)`/`.ticks(ts)` calls).
- Canonical unit = per in-game day.
- Peg applies to **both** headless and windowed via shared `wall_seconds_per_game_day`.

The plan file owning the full design is at `~/.claude/plans/i-m-noticing-that-my-memoized-piglet.md`.

## Verification

Per-phase protocol mirrors CLAUDE.md's port workflow:

1. `just check` — cargo check + clippy + `check_step_contracts.sh` + new `check_time_units.sh`.
2. `just test` — newtype unit tests (round-trip per-day → per-tick → recovered; `IntervalPerDay::fires_at` matches old `tick % N` semantics; `DurationSeasons` scales with `ticks_per_season`).
3. `just soak 42` to a versioned dir per CLAUDE.md (`logs/tuned-42-time-phaseN/`).
4. `just verdict logs/tuned-42-time-phaseN` — pass. Phase 0 must be byte-identical post-header (constants block unchanged); only the two new header fields differ.
5. **Phase 4 peg test:** `cargo run --release -- --headless --seed 42 --duration 900 --game-day-seconds 30` produces ~30 000 ticks (vs ~54 000 default) with the same in-game footer within ±10% — the smoking-gun proof the peg works.

## Log

- 2026-04-26: Ticket opened. Phase 0 (foundation) lands as one commit. Adds typed wrappers (`RatePerDay`, `DurationDays`, `IntervalPerDay`, `DurationSeasons`, `Ticks`) in new `src/resources/time_units.rs`; `TimeScale` resource in `src/resources/time.rs`; `--game-day-seconds N` CLI flag; `wall_seconds_per_game_day` + `tick_rate_hz` fields in the events.jsonl `_header`; permissive `scripts/check_time_units.sh` CI gate wired into `just check`. Behaviorally a no-op — no FixedUpdate system reads `TimeScale` yet (verified via `rg TimeScale src/`); the only sim-visible mutations are two new header fields and the dual-build derivation of `Time<Fixed>` Hz from the same anchor (preserves headless 60 Hz, windowed 1/5/20 Hz). 29 new unit tests pass. Seed-42 release soak (`logs/tuned-42-time-phase0/`) completes with header fields populated; survival canaries hold within the CLAUDE.md noise band (Starvation = 2 — within parallel-scheduler tolerance per the rule; ShadowFoxAmbush = 5 ≤ 10; footer written). Continuity-canary failures (mentoring/burial/courtship = 0) are pre-existing per the project state and unrelated to Phase 0. Phase 1 (confirmed-bug migrations) is next.
- 2026-04-26: Phase 1 (confirmed-bug migrations + rename + reconciliation + scope add). Per user direction, every migrated constant is tuned to what makes ecological sense in the in-game-day frame, not just typed-equivalent. Seven fields rename + retype: `Coordination::evaluate_interval` and `Aspirations::second_slot_check_interval` become `IntervalPerDay::new(1.0)` (was raw `100`, silently 10×/day after the 2026-04-10 overhaul); `Fertility::update_interval_ticks → update_interval = IntervalPerDay::new(1.0)`; `Fertility::cycle_length_ticks → cycle_length = DurationSeasons::new(0.5)` (numerically identical at default scale); `FoxEcology::scent_decay_per_tick → scent_decay_rate = RatePerDay::new(0.1)` (territorial mark, value preserved); `Prey::scent_decay_per_tick → scent_decay_rate = RatePerDay::new(1.0)` (was `0.02/tick = 20/day`, fixing the 200× discrepancy that fed `goap.rs:4159`'s scent-led hunt path below the detect threshold in ~3 ticks); `Fate::assign_cooldown` becomes `IntervalPerDay::new(1.0)` (was `50` raw ticks = 20×/day burst at game start). All consumer systems gain `Res<TimeScale>`. `phase_from` refactored to take an explicit `cycle_length_ticks: u32` parameter so it stays pure-on-scalars and unit-testable without `TimeScale`. JSON header shape changes via `serde rename` + `alias` for backward-compat reading. New balance log `docs/balance/time-anchor-iteration-1.md` captures four-artifact tie-out for H1 (cadences) / H2 (prey scent) / H3 (fate). Verification: `just check` + `just test` (1332 unit + 13 integration tests pass; 29 → 30 typed-units tests including new once-per-day-cadence sanity test); seed-42 release soak `logs/tuned-42-time-phase1/`. Survival canaries strictly improve or hold against Phase 0: Starvation 2 → **0**, ShadowFoxAmbush 5 → 5, footer written, never_fired_expected_positives 11 → 10 (`CropHarvested` newly fires). Anxiety interrupts -95% (62260 → 3057), `wards_placed_total` +143% (78 → 190), `EngagePrey:*` plan failures effectively eliminated (~2400 → 0 across the top reasons). Continuity tally magnitudes dropped > 30% on grooming/play/mythic-texture (44 → 21, 348 → 109, 40 → 23) — hard gates (≥1) hold; the drop is hypothesis-aligned spillover (cats spend time-budget on active hunting/wards instead of idle play) but exceeds CLAUDE.md's >±30% scrutiny band. Tracked as ticket 034 (time-anchor continuity rebalance) for follow-on tuning. Phase 2 (drain/decay cluster — `NeedsConstants` rates as `RatePerDay`) is next.
