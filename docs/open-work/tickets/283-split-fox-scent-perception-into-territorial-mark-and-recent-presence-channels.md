---
id: 283
title: Split fox-scent perception into territorial-mark and recent-presence channels
status: ready
cluster: belief-perception
initiative: [predator-prey-dynamics, full-sensory-perception]
added: 2026-05-11
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [256-patrol-route-cost.md]
landed-at: null
landed-on: null
---

## Why

Surfaced during a perception-accuracy audit of `logs/tuned-42` for
ticket 273. `FoxScentMap`'s scent_decay_rate is intentionally slow
(`RatePerDay::new(0.1)`, ~10 in-game day half-life — `src/resources/sim_constants.rs:4240-4247`)
because fox scent doubles as a *territorial mark* that needs to
persist for fox-vs-fox / fox-vs-prey reasoning. That timescale is
correct for those consumers, but it's wrong for Patrol-class
consumers that want a fast "fox was here recently" signal. Ticket
228/256 resolved the mismatch by **removing Patrol's read of
`fox_scent_level` entirely** (`src/ai/dses/patrol.rs:109` — "v2
stopgap") because chronic stale-scent firing was elevating Patrol
days after foxes had left. That was the right local move but the
wrong global shape: the signal Patrol wanted exists in concept
(recently-here fox) but doesn't exist as a separate scalar; instead
the existing slow-decay scalar was deemed unusable. This ticket
introduces a second perception channel with a fast (hours-scale)
decay so Patrol-class consumers can read "recent fox presence"
without disturbing the existing 10-day territorial-mark scalar.

## Scope

- Add a second fox-scent perception channel. Two shapes possible —
  pick during design:
  - **Separate map** (`RecentFoxPresenceMap` or similar) co-deposited
    by `fox_scent_tick` (`src/systems/wildlife.rs:2383`) with its own
    fast decay (hours-scale, e.g. `RatePerDay::new(2.0)` — ~12-hour
    half-life). Bigger blast radius but cleanest separation.
  - **Layered scalar on the existing map**, where authors deposit
    into both layers each tick and readers pick which layer to
    sample. Smaller diff, but couples the two semantics.
- New scalar (e.g. `recent_fox_presence`) exposed via `ctx_scalars()`
  in `src/ai/scoring.rs:601`.
- Re-introduce a Patrol read of the new fast-decay channel. This
  partially undoes ticket 228/256's v2 stopgap, but with the right
  signal shape this time. Likely as the brake on Patrol's exit gate
  (Patrol closes when both recent-scent AND recent-memory agree no
  threat), rather than as a primary input.
- Consider revisiting `patrol_route_cost` (ticket 256, currently
  weight 0) to use the fast-decay channel instead of the slow one.

## Out of scope

- Tuning the existing 10-day territorial-mark scalar. It stays as
  is for fox-vs-fox / fox-vs-prey reasoning.
- Adding ambush-event integration — that's ticket 219's
  RecentAmbushMap (different ground truth: kills, not presence).
- Per-cat memory decay on ThreatSeen — separate fix in the audit's
  gap #1. May be opened as its own ticket; could become 283's
  sibling in the doctrine framework.
- Patrol's safety-deficit-driven primary scoring shape. This ticket
  adds an input/gate, not a rewrite.

## Current state

Audit at `.claude/plans/let-s-work-273-dig-enchanted-wirth.md`
(2026-05-11) documents the fox-scent timescale mismatch (gap #2)
and proposes the split-construct fix. Existing precedent:
- `src/resources/fox_scent_map.rs` — current single-channel map
- `src/systems/wildlife.rs:2383-2410` — `fox_scent_tick` deposit +
  decay system
- `src/ai/dses/patrol.rs:109` — comment marking the v2 removal
- `docs/open-work/landed/256-*.md` (when looked up) — the Patrol
  route-cost ticket that worked around the mismatch
- ticket 282 (sibling) — temporal-integration doctrine that
  motivates this ticket's shape

## Approach

Defer detailed design until 282 (doctrine) lands so the timescale +
authoring-side choices follow the rubric. Sketch:
1. Pick separate-map vs layered-scalar (separate-map favored —
   cleaner; matches `CarcassScentMap` precedent for a sibling map).
2. Author both maps in `fox_scent_tick` (one deposit, two writes).
3. Expose `recent_fox_presence` scalar.
4. Add a Patrol consumer (likely as exit-gate brake), preserving
   the safety-deficit-driven primary score. Land behind a feature
   gate / scenario test so any L3 share shift is bisectable.

## Verification

- `just sweep-stats logs/sweep-283 --vs logs/sweep-baseline` —
  Patrol L3 share should drop during low-fox-activity windows but
  hold during active raids. Welch's t on `patrol_share_overall`
  expected negative-direction with `d` in the small-to-medium band.
- `just q anomalies logs/tuned-42 (post-283)` — never-fired
  positives should hold; survival canaries (Starvation, ShadowFox)
  unchanged or improved.
- Focal-cat trace on Cedar (high-threat-memory cat): Patrol score
  should track `recent_fox_presence` more visibly than today's
  pure safety-deficit shape.

## Log
- 2026-05-11: opened. Surfaced by the perception-accuracy audit
  for ticket 273 (gap #2). Defers detailed design until 282
  (temporal-integration doctrine) lands.
