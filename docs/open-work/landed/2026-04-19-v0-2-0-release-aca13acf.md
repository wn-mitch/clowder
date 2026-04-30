---
id: 2026-04-19
title: "v0.2.0 release — `aca13acf`"
status: done
cluster: null
landed-at: null
landed-on: 2026-04-19
---

# v0.2.0 release — `aca13acf`

The `chore: release v0.2.0` commit bundled in-flight threads that had been
staged as "uncommitted" in earlier revisions of this document. Kept here
rather than deleted because the archived baselines and report pointers
remain useful for retros.

- **Balance: `eat_from_inventory_threshold: 0.05 → 0.4`** — seed-42 15-min
  soak: starvation 2→1, below-0.3 hunger 1.06%→0.50%, stores mean 85%→92%,
  leisure action-time +18%, colony survives +2 sim-weeks. Report at
  `docs/balance/eat-inventory-threshold.report.md`. Baselines:
  `logs/tuned-42-archive-apr17/`, `logs/tuned-42-baseline-eat-threshold/`,
  `logs/tuned-42/`. Pre-existing: `check_canaries.sh` still fails on
  `Starvation == 0` (now 1, was 2).
- **Docs reframe** — CLAUDE.md opening rewrite + Systems inventory +
  continuity canaries + `src/main.rs:346` line reference correction;
  `docs/systems/project-vision.md` new (thesis, influences, design
  corollaries); this file introduced.

## Mentor snapshot "never applied" — obsolete (no commit, 2026-04-19)

Prior follow-on item claimed `resolve_mentor_cat` produces a snapshot that
is never consumed. Verified false: the snapshot IS drained in the live
GOAP path at `src/systems/goap.rs:2672–2743` (biggest teachable skill gap
gets `growth_rate * apprentice_skill_growth_multiplier` added to the
apprentice's `Skills`). The `disposition.rs:3157` consumer is in
`resolve_disposition_chains`, which is not registered in either
`SimulationPlugin::build()` or `build_schedule()` — dead code.

Mentor *does* teach when it fires. Mentor firing 0× in the seed-42 soak
is a target-availability problem, already covered by follow-on #1.
