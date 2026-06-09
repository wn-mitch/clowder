---
id: 498
title: verdict throughput-drift channel + promote ratchet (480 child)
status: done
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-06-09
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 9cc21779
landed-on: 2026-06-11
---

## Why
Perf epic 480's stairstep regressions (p90 197 → 72 t/s over five weeks)
surfaced **weeks late**, via the `soak-throughput-over-time` LOESS chart,
because no per-landing gate watches throughput: `just ci` has no TPS
floor and `just verdict` had no throughput channel. Every landing already
runs verdict — putting the signal there converts a monthly forensic
exercise into a per-landing diff.

## Scope
- Footer fields `wall_elapsed_secs` (measured, from the existing
  `HeadlessRunStart(Instant)`) + `ticks_per_sec` in
  `src/plugins/headless_io.rs::emit_headless_footer`. Observability-only;
  `Instant` never feeds sim state.
- `scripts/verdict.py::throughput_drift` — metric preference:
  `ticks_per_sec` when both footers carry it (duration-invariant, honest
  for wipeout-shortened runs); else `elapsed_ticks` when `duration_secs`
  budgets match (works against every pre-existing archive baseline); else
  incomparable (`None`).
- Degradation-only bands sized for parallel-session contention noise:
  pass ≥ −15%, concern −15..−40%, strong-concern < −40%. Escalates the
  overall verdict to **concern at most, never fail** — single-run TPS can
  be faked by a busy machine; p90-across-runs (480's chart) stays the
  authoritative trend instrument. `next_steps` routes the caller to
  idle-rerun / `just sweep-stats` before any bisect.
- `scripts/promote.sh` ratchet: refuses to promote a candidate >15%
  slower than `logs/baselines/current.json` at a matching duration
  budget; `--accept-throughput-regression` overrides deliberately
  (distinct from `--force`, which only means "overwrite the label file").
  The baseline lineage can no longer quietly absorb stairsteps.

## Out of scope
- A hard CI TPS floor (`just ci`) — too noisy under parallel-session CPU
  contention; revisit if the verdict channel proves insufficient.
- Fixing the open 480 knives themselves (459, 205 — separate tickets).

## Current state
Implemented in this session alongside tickets 499 (score checkpoint) and
490's dispersion canary as the pre-behavior-change instrumentation wave.

## Approach
Mirrors ticket 125's `colony_score_drift` shape end-to-end: dataclass
field, drift function with band helper, `derive_overall` escalation
clause, `derive_next_steps` hint, text-mode line, stdlib-unittest file.

## Verification
- `tests/verdict/test_throughput_drift.py` (bands, metric preference,
  legacy-baseline fallback, incomparable cases, escalation cap,
  improvements-never-gate) — registered in `just test-verdict`.
- `just soak 42` + `just verdict` shows the `throughput` row against the
  active baseline.
- Deliberate slow candidate (debug build) → `just promote` refuses.

## Log
- 2026-06-09: opened + implemented in the same session (instrumentation
  wave, pre-490/459 work). Bands deliberately wide; concern-cap rationale
  recorded in §Scope.
- 2026-06-11: implemented same-session: footer tps fields, verdict throughput_drift (concern-capped), promote ratchet
