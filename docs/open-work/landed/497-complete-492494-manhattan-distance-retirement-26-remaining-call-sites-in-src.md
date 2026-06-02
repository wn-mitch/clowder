---
id: 497
title: complete 492/494 manhattan_distance retirement (~26 remaining call-sites in src/)
status: done
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-06-02
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 1f6d1b28
landed-on: 2026-06-02
---

## Why

Tickets 492 and 494 jointly retired `Position::manhattan_distance` as the
default spatial metric in sim code, replacing it with `distance_to`
(Chebyshev, matching 8-direction movement cost per
`pathfinding.rs::heuristic`) and `euclidean_distance` (escape hatch for
radial sensing — scent diffusion, ward-glow falloff, sound amplitude,
visual perception). The deprecation lint at `src/components/physical.rs:186`
fires on every remaining call-site.

The 494 closure log claims the migration was complete: "Completed the
in-progress 492 call-site migration (~70 sites in goap.rs from
`manhattan_distance` to `distance_to` or `tile_distance_squared`) as the
precondition for a clean build." But `grep -rn manhattan_distance src/`
excluding the definition site returns ~26 surviving call-sites, all of
which raise the `cargo clippy --all-targets -- -D warnings` step inside
`just check` to error. CI's separate jobs apparently mask this; `just check`
locally fails on the deprecation lint.

These call-sites need per-site substrate-aware decisions, not blind
mechanical replacement — that's why they were left behind. Each is
either tactical reach (Chebyshev) or radial sensing (Euclidean), and
the right answer depends on the call-site's intent.

## Scope

1. **Audit each of the ~26 remaining `manhattan_distance` call-sites in
   `src/` (exclude `src/components/physical.rs:189` where the method is
   defined)**. Categorize each into:
   - **Tactical reach** — adjacency / within-N-tile interactions that
     should match 8-direction movement → `chebyshev_distance` (or
     `distance_to` if the call expects Chebyshev as the default).
   - **Radial sensing** — perception, sound, scent, ward glow, visual
     range → `euclidean_distance`.
   - **Pathfinding heuristic** — already covered by
     `pathfinding.rs::heuristic`; if a call is doing a hand-rolled
     heuristic, replace with `distance_to`.

2. **Migrate each call-site** with the substrate-correct metric.
   Document non-obvious choices in a brief inline comment ("// Chebyshev
   because this is a 4-tile structure-adjacency check, not a perception
   read").

3. **Verify `just check` passes** — `cargo clippy --all-targets --all-features
   -- -D warnings` should return zero deprecation warnings on
   `manhattan_distance`.

4. **Verify no test regression** — `cargo test --release` should pass
   in full. Targeted check on `surrounded_colony`, `goap`, and any
   scenario tests touched.

5. **Run `just verdict logs/tuned-42-9b3f5d43`** against the new
   baseline `post-496-chebyshev-radial-split` to confirm no soak-level
   regression from the per-call-site migrations.

## Out of scope

- **The remaining clippy::useless_vec lints** in `src/systems/goap.rs`
  test code. Adjacent style debt; punt to a separate `style:` commit
  or include if trivial.
- **Migrating tests that intentionally exercise Manhattan distance.**
  `src/components/physical.rs:456` (the unit test for
  `manhattan_distance` itself) should stay on Manhattan. Any other
  test asserting "Manhattan distance is N" rather than "the cat
  reaches the target" needs case-by-case judgment.
- **Removing the `manhattan_distance` method entirely.** The 494
  retirement preserved it for test parity and external tooling
  (`physical.rs:182-188`). This ticket does not change that — it just
  retires the production call-sites.

## Current state

Discovered 2026-06-02 during 494 closure when `just check` failed on the
deprecation lint. Counts:

- ~26 remaining call-sites in `src/` (per `grep -rn manhattan_distance
  src/ | grep -v 'src/components/physical.rs' | wc -l`)
- ~22 are in `src/systems/goap.rs` (most prominent: planner-zone
  reach checks at lines 11014 / 11020, hunt-range at 11214, test
  asserts at 11340 / 11455)
- A handful elsewhere; full inventory at start of work.

The 494 commit `2eacc01b` migrated ~70 of the goap.rs sites; the
remaining ~26 either weren't touched by the sweep or were intentionally
deferred for per-site judgment.

## Approach

Single-pass migration with substrate-aware judgment. Workflow per
call-site:

1. Read the surrounding code context (5-10 lines either side).
2. Classify intent: tactical reach, radial sensing, or pathfinding heuristic.
3. Pick the substrate-correct metric and migrate.
4. Add a brief inline comment if the choice is non-obvious.

Group related call-sites in a single commit when they share intent
(e.g., "fix: 497 — migrate planner-zone reach checks to chebyshev_distance").

Recommended commit shape: 3-5 focused commits, each ~5-8 call-sites,
each with its own substrate-honest justification in the message.
Avoids a single 26-site mega-commit that masks the per-site reasoning.

## Verification

- `cargo clippy --all-targets --all-features -- -D warnings` passes.
- `just check` passes end-to-end (incidentally validates the substrate-stub,
  silent-canary, and compile-time-contract enforcement is unaffected).
- `cargo test --release` passes.
- `just verdict logs/tuned-42-9b3f5d43` against the
  `post-496-chebyshev-radial-split` baseline returns `pass` (no soak-level
  drift from the migration).
- Optional: `just soak-trace` short run on seed 42 to confirm focal-cat
  L1/L2/L3 decision shape is unchanged.

## Log

- 2026-06-02: opened from 494 closure. `just check`'s clippy step
  surfaced ~26 surviving `manhattan_distance` call-sites that the
  492/494 sweep missed. Not in 494's scope (which targeted the spatial
  metric realignment for plan-failure regressions). Each surviving
  call-site needs a tactical-vs-radial decision before migration.
- 2026-06-02: 26 manhattan_distance call-sites retired (22 production + 2 tests + 2 comments). Threat-context block uniformly Chebyshev. Two test fragilities surfaced and fixed at the test layer: rat-byproduct ambient-kill conflation (pre-existing 464-class defect) and picking_up_scavenging seed-42 budget. Follow-on: goap.rs:1318 near_buildings still on euclidean.
