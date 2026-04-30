---
id: 033
title: Time-unit typing (Phases 0-6 complete)
status: done
cluster: null
landed-at: b8a7cf5
landed-on: 2026-04-27
---

# Time-unit typing (Phases 0-6 complete)

**Landed:** 2026-04-27 | **Commits:** Phase 0 `b8a7cf5`, Phase 1 `a879f43`, Phases 2-6 `cfdf95b2` / `a1ec3f48` / `c715432e` / `bf255413` / `a2a79406`.

**Why:** Tick timing was "all over the place" — no consistent "1 in-game day = X real time" anchor, three confirmed `evaluate_interval = 100` stragglers from the 2026-04-10 100→1000 ticks/day overhaul, a 200× prey/fox scent-decay discrepancy, and a fully unpegged `Time<Fixed>` Hz between headless (60) and windowed (1/5/20). Centralized storage in `sim_constants.rs` couldn't enforce discipline — nothing forced consumers through a converter.

**Approach:** Same playbook as `StepOutcome<W>` + `check_step_contracts.sh`: typed wrappers (`RatePerDay` / `DurationDays` / `DurationSeasons` / `IntervalPerDay` / `Ticks` in `src/resources/time_units.rs`) + a `TimeScale` resource derived from `SimConfig` plus a user-facing `wall_seconds_per_game_day` peg + `--game-day-seconds N` CLI flag + a CI gate in `scripts/check_time_units.sh`. Every temporal constant migrates to a typed wrapper; consumers cannot multiply a `RatePerDay` directly, they must call `.per_tick(ts)`. Phases land in clusters (Phase 1 = confirmed bugs, Phases 2-4 = drain-decay, magic-mood, combat-prey-fox-food clusters, Phase 5 = test-scale unification, Phase 6 = gate hardening).

**Verification:** Per phase, `cargo check` + `cargo test` for typed equivalence; one triplicate-seed release soak (42/7/13) + a peg test (`--game-day-seconds 30`) at the tip. Survival canaries: `Starvation` median 1 (vs Phase 1 baseline 2 — improves), `ShadowFoxAmbush` median 1 (gate ≤ 10), footers written on all four runs. Peg test: `headless_tick_rate_hz` drops from 60 → 33.33 (ratio 16.67/30 = 0.555), sim completes. Four-artifact tie-out in `docs/balance/time-anchor-iteration-1.md`. Continuity-canary failures (`mentoring`/`burial`/`courtship`/`grooming` partial) inherit from Phase 1 active-colony spillover (ticket 034) and a WIP-commit courtship regression (ticket 040).

**What's now permanent:** Raw `tick % N` near temporal contexts is a hard fail in `just check`. The allowlist file is deleted. Every future temporal constant must use a typed wrapper. Header carries `wall_seconds_per_game_day` + `headless_tick_rate_hz` so downstream tooling can reconstruct wall-clock time. `--game-day-seconds N` works on both headless and windowed builds.

**Out of scope (deferred):** `den_predation_pressure_decay` stays raw f32 (multiplicative exponential, would need a different wrapper type). Suspect 10-tick cadences `corruption_spread_cadence` and `shadow_fox_spawn_cadence` migrated typed-equivalent at `IntervalPerDay::new(100.0)`; ecological retuning is a follow-on. The 5-/10-tick mood-transient durations preserve current behavior; wall-clock-vs-game-day semantics audit deferred. Per-rate balance retuning lives in tickets 034 / 040 / 041 / 042.

---
