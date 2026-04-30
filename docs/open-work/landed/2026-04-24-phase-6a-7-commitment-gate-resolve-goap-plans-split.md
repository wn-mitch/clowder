---
id: 2026-04-24
title: "Phase 6a §7 commitment gate + `resolve_goap_plans` split"
status: done
cluster: null
landed-at: null
landed-on: 2026-04-24
---

# Phase 6a §7 commitment gate + `resolve_goap_plans` split

**What shipped:**

- `src/ai/commitment.rs` (~850 lines): `BeliefProxies` struct,
  `should_drop_intention` pure-function gate, `strategy_for_disposition`
  12-row §7.3 table, `proxies_for_plan` belief-proxy recipe, `record_drop`
  telemetry helper. 25 unit tests.
- `resolve_goap_plans` split: the ~4,500-line monolith split into
  `resolve_goap_plans` (~797 lines, per-cat loop + prologue gate + trip
  completion) and `dispatch_step_action` (~1,275 lines, `#[inline(never)]`,
  37-arm step-resolution match). Immutable pre-loop data bundled in
  `StepSnapshots`; mutable accumulators in `StepAccumulators`.
- Prologue gate wired into `resolve_goap_plans` per-cat loop: evaluates
  `should_drop_intention` per cat per tick; drops trigger plan removal +
  `Feature::CommitmentDropTriggered` telemetry.
- Per-DSE `default_strategy()` impls across 13 DSE files.
- Commitment telemetry at existing `disposition_complete` and
  `max_replans_exceeded` decision points (cold path, ~0.6% of iterations).
- `find_nearest_tile` north-bias fix (tiebreaker via `mix_hash`).

**Root cause of the regression (sessions 1–3):** LLVM optimization cliff.
Adding 4 cross-module function calls to the hot inner loop of the
~4,500-line `resolve_goap_plans` pushed the function past LLVM's
per-function optimization budget in release mode. Debug builds passed;
release builds caused colony collapse (Starvation=8, all continuity
tallies zero). The same functions worked fine when called on ~0.6% of
loop iterations (existing cold-path decision points) vs. 100% (prologue
gate). Empty-body probe (field reads only) passed — the regression was
not in the gate's logic but in LLVM's treatment of the enclosing function.
Full investigation:
[`docs/systems/phase-6a-commitment-gate-attempt.md`](systems/phase-6a-commitment-gate-attempt.md).

**Defensive follow-on:** `resolve_disposition_chains` in
`src/systems/disposition.rs` (~1,629 lines) has the identical risk
profile — per-cat-per-tick hot loop, large match dispatch, no
`#[inline(never)]`. Queued as a proactive split; see the open item below.
