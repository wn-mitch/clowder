# Time anchor

**Status:** Built (2026-04-26, ticket 033 Phase 0 — typed wrappers + peg foundation; per-system migrations in subsequent phases.)

The single anchor connecting **ticks ↔ in-game time ↔ wall-clock time**. Live in [`src/resources/time.rs`](../../src/resources/time.rs) (`TimeScale` resource, `SimConfig` constants) and [`src/resources/time_units.rs`](../../src/resources/time_units.rs) (typed wrappers).

## The formulas

```
ticks_per_day        = SimConfig::ticks_per_day_phase × 4    # default: 1000
ticks_per_season     = SimConfig::ticks_per_season           # default: 20000
ticks_per_year       = ticks_per_season × 4                  # default: 80000

tick_rate_hz         = ticks_per_day / wall_seconds_per_game_day
in_game_days_elapsed = tick_count / ticks_per_day
wall_seconds_elapsed = tick_count / tick_rate_hz
```

Two simulation runs are behaviorally comparable iff their `SimConfig` and `wall_seconds_per_game_day` both match. Both fields land in the `events.jsonl` `_header` line for downstream tooling.

## The peg

`wall_seconds_per_game_day` is the user-facing knob that says *"one in-game day equals N wall-clock seconds."* It governs the FixedUpdate Hz for both build paths:

| Build       | How `wall_seconds_per_game_day` is set                                           | Default        | Resulting Hz |
|-------------|----------------------------------------------------------------------------------|----------------|--------------|
| Headless    | `--game-day-seconds N` CLI flag (parsed in `src/main.rs` → `AppArgs`)            | 16.6667        | 60 Hz        |
| Windowed    | `SimSpeed::wall_seconds_per_game_day(&config)` (`src/resources/time.rs`)         | 1000.0 (Normal)| 1 Hz         |
| Windowed Fast | (cycle key)                                                                    | 200.0          | 5 Hz         |
| Windowed VeryFast | (cycle key)                                                                | 50.0           | 20 Hz        |

The defaults preserve pre-ticket-033 behavior: headless verification still ticks at 60 Hz, windowed Normal still feels like 1 tick/second.

## The typed wrappers

Every temporal constant in [`src/resources/sim_constants.rs`](../../src/resources/sim_constants.rs) migrates (one cluster per ticket-033 phase) into one of these newtypes from [`src/resources/time_units.rs`](../../src/resources/time_units.rs):

| Type                | Purpose                                          | Conversion                                            |
|---------------------|--------------------------------------------------|-------------------------------------------------------|
| `RatePerDay`        | Drains, decays, regens (per in-game day)         | `rate.per_tick(&time_scale) → f32`                    |
| `DurationDays`      | Durations measured in in-game days               | `duration.ticks(&time_scale) → u64`                   |
| `DurationSeasons`   | Durations measured in in-game seasons            | `duration.ticks(&time_scale) → u64`                   |
| `IntervalPerDay`    | Cadence: "fires N times per in-game day"         | `interval.fires_at(tick, &time_scale) → bool`         |
| `Ticks`             | Newtype around a raw tick count (runtime values) | `tick.get() → u64` / `tick.0`                         |

**Ergonomics** — explicit named conversion only. There is no `From<f32>`, no `Into<f32>`, no `Default::default()` returning a magic value. A consumer that wants a per-tick number must call `.per_tick(&time_scale)` and pass the [`TimeScale`] resource — the type system refuses anything else. Same playbook as `StepOutcome<W>` in [`src/steps/outcome.rs`](../../src/steps/outcome.rs): the discipline becomes a compile error, not a code-review check.

## The CI gate

[`scripts/check_time_units.sh`](../../scripts/check_time_units.sh) (wired into `just check`) bans raw-literal `tick % N` and `tick.is_multiple_of(N)` patterns in `src/systems/`, `src/steps/`, `src/ai/` outside the allowlist. Field-driven modulos (e.g. `tick.is_multiple_of(c.evaluate_interval)`) are not flagged — the field itself is the migration unit.

The companion [`scripts/time_units_allowlist.txt`](../../scripts/time_units_allowlist.txt) shrinks one phase at a time. Phase 6 deletes both the allowlist and the allowlist mechanism.

## What does *not* go through the wrappers

- **Movement speeds** (stalk, approach, chase, scent_deposit_per_tick). These are *spatial* per-tick rates, not temporal — they describe distance per tick, where "tick" is just the discrete simulation step. Per the 2026-04-10 100→1000 ticks/day overhaul, movement speeds were intentionally not scaled when ticks/day changed.
- **Probabilities** (`pounce_base_rate`, `den_refill_base_chance`). Unitless [0,1] — no time component.
- **Distances, multipliers, modifiers**. No time component.

## Why this exists

The 2026-04-10 100→1000 ticks/day overhaul missed three "fires every 100 ticks" stragglers (`CoordinationConstants::evaluate_interval`, `AspirationConstants::second_slot_check_interval`, `FertilityConstants::update_interval_ticks`) because nothing forced consumers through a converter. Centralizing the constants in `sim_constants.rs` solved storage, not enforcement. The typed wrappers + CI gate close the gap by turning the discipline problem into a compile error and the long tail into a build-time fail.

## See also

- Ticket 033 ([`docs/open-work/tickets/033-time-unit-typing.md`](../open-work/tickets/033-time-unit-typing.md)) — phase tracker.
- `docs/balance/time-anchor-iteration-1.md` — Phase 1+ measured shifts.
- CLAUDE.md → "Tuning Constants" — names this doc as the canonical reference.
