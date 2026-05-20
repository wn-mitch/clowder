---
id: 430
title: Add just flamegraph recipe + macOS dtrace setup doc
status: done
cluster: tooling-diagnostics-ui
orchestration: swarm-safe
initiative: []
added: 2026-05-20
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 93eab6343f7b
landed-on: 2026-05-20
---

## Why

Flamegraph is the diagnostic tool we **know we should be using but haven't wired**. The auto-memory `feedback_perf_refactor_needs_flamegraph` captures the precedent: ticket 205's `CatSpatialIndex+caches` refactor went through with asymptotic-analysis-only review and produced a 1.78× wall-clock slowdown that a flamegraph would have caught pre-merge. Tickets 205 and 389 both *name* `cargo flamegraph` in their Approach sections, but zero landed tickets show evidence of actually running it — the tool is in design conversations and missing from the workflow surface. Ticket 428's verification revealed another candidate: a -14.6% per-tick wall-clock slowdown that asymptotic analysis didn't predict (the snapshot populate is O(n_kittens), but `resolve_goap_plans` is a hot path so the constant-factor cliff matters). Without a `just flamegraph` recipe + documented macOS setup, every future perf investigation either repeats the install dance from scratch or skips the profiler entirely. This ticket closes the wire-up gap so flamegraph becomes a one-line invocation in the same surface as `just verdict` / `just sweep-stats`.

## Scope

- Install `cargo-flamegraph` (provides the `cargo flamegraph` subcommand) and document the dependency in `CONTRIBUTING.md` or wherever dev-env setup lives.
- Add `just flamegraph SEED="42" DURATION="60"` recipe that invokes `cargo flamegraph --release -- --headless --seed {{SEED}} --duration {{DURATION}}` and writes the SVG to `logs/flamegraphs/<seed>-<commit>.svg`.
- Document the macOS-specific dance: `cargo flamegraph` on macOS uses `dtrace`, which requires `sudo` (and SIP-aware path quirks). Write `docs/diagnostics/flamegraph.md` covering install + permission setup, recipe usage, and how to read the SVG (bottom→top = entry→leaf; wider = hotter).
- Add a note to the bugfix-discipline section of `CLAUDE.md` listing when to reach for flamegraph (any landed change with ≥5% wall-clock tick-rate shift in soak verification; any perf refactor before merging).

## Out of scope

- **Linux flamegraph workflow** — uses `perf` instead of `dtrace`; can be a follow-on if/when we have Linux contributors. Document macOS-first.
- **Continuous performance gates in CI** — flamegraph is for local diagnosis; CI perf gates are a separate (much larger) project.
- **Optimizing any specific hot frame** — this is infrastructure; per-frame optimization is per-ticket.
- **Bench harness (Criterion)** — different tool, different use case (micro-benchmarks of pure functions). Mentioned in 389; not in scope here.

## Current state

Opened 2026-05-20 as a §428 follow-on. The post-fix soak ran -14.6% fewer ticks per wall-clock-second than the pre-fix baseline (75,902 → 64,834 ticks in `just soak`'s 900s wall-clock window); we want to attribute that to the snapshot populate vs. some other cause, but lack the tooling to do it efficiently. Existing references to flamegraph in the corpus: tickets 205, 389 (both unlanded perf-investigation tickets), plus the auto-memory `feedback_perf_refactor_needs_flamegraph`.

## Approach

1. `brew install flamegraph` or `cargo install flamegraph` — confirm which gives a clean `cargo flamegraph` subcommand on macOS.
2. Verify against the post-428 binary: `cargo flamegraph --release -- --headless --seed 42 --duration 60` — the short duration is intentional (flamegraphs need ~30s+ of samples; full soak is overkill for the SVG, and dtrace overhead can perturb wall-clock measurements).
3. Wire the just recipe; route the SVG output to `logs/flamegraphs/<seed>-<commit>.svg` and add `logs/flamegraphs/` to `.gitignore` (or commit small reference SVGs to `docs/diagnostics/baseline-flamegraphs/<commit>/` for comparison — TBD).
4. Write `docs/diagnostics/flamegraph.md` with: install, run, read (key visual patterns — wide tall stacks = hot, narrow short = cold; look for `resolve_goap_plans` width as the per-tick budget anchor).
5. Update CLAUDE.md "Bugfix discipline" + "Verification" sections — name flamegraph as the response to wall-clock tick-rate shifts above the noise floor.

## Verification

- `just flamegraph` produces an SVG without manual intervention (sudo prompts ok; document them).
- The SVG opens in a browser and is interactive (scroll, zoom, search). cargo-flamegraph's default output is interactive — verify with a fresh-eyes read.
- The §428 slowdown (-14.6%) gets attributed in a follow-on Log entry: "snapshot populate is N% of resolve_goap_plans" or "populate is sub-1% — the slowdown is elsewhere."

## Log

- 2026-05-20: opened as a §428 follow-on. The substrate-stub-fix verification surfaced a -14.6% wall-clock tick-rate cost; named flamegraph as the right diagnostic; discovered flamegraph has been mentioned in two open tickets (205, 389) and zero landed tickets, with no `just` recipe or install doc. Closing the wire-up gap.
- 2026-05-20: Validated end-to-end against post-428 binary: 60s soak seed 42, 59,813 samples @ 997Hz. samply + packed dSYM symbolicates correctly; samply_top.py emits a top-N table + target-substring attribution. cargo-flamegraph 0.6.x rejected on macOS (hard-coded xctrace, needs full Xcode). 431 (hot-frame catalog) used this recipe to surface passive_familiarity at 64% inclusive CPU.
