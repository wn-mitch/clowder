---
id: 517
title: Soak harness frame-hitch: per-tick systems re-run on unadvanced tick, double-applying state and duplicating emissions
status: ready
cluster: tooling-diagnostics-ui
orchestration: coherent-block
initiative: []
added: 2026-07-09
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: [310-s1-satiation-activation.md]
landed-at: null
landed-on: null
---

## Why

During 310 S1's gate-neutrality byte-check (`tuned-42-1effd660` vs
`tuned-42-ef08d805-s1-900s` — classifier-only delta, streams expected
identical minus footer), the reference run showed a frame-hitch artifact
at tick 1,262,700: a batch of per-tick events emitted **twice with
identical content** (`CourtshipDrifted` ×9 pairs, `DirectiveIssued` ×2,
`FoxPlanCreated` ×3), three `CatSnapshot` cadence emissions **dropped**
shortly after (ticks 1263300–1263500 missing from the 100-tick series),
and a genuine behavioral fork from tick 1,263,235 onward (345 ticks with
differing event multisets to run end). The streams were byte-identical
for 140,893 lines before the hitch.

The fork is the serious part: identical duplicate emissions alone would
be a log-writer artifact, but downstream divergence means state was
**double-applied** (courtship drift twice in one tick = real relationship
decay ×2). The likely shape: per-tick systems keyed to `TimeState.tick`
re-ran on a frame where the fixed-timestep accumulator did not advance
the tick (machine load / frame spike), so "per-tick" work executed twice
per tick. This silently breaks two load-bearing assumptions:

1. **Byte-identity gates** (485/351 precedent, 265 null-drift ladder)
   assume same-seed same-binary runs reproduce bit-exactly; a hitch
   makes the gate fail spuriously and can mis-attribute divergence to
   the change under test.
2. **Determinism of seed-42 comparisons generally** — any two runs under
   different machine load can fork trajectories mid-run.

## Scope

- Reproduce/diagnose: confirm the unadvanced-tick re-run hypothesis
  (instrument tick-advance vs Update-schedule executions, or audit the
  fixed-timestep driver in the headless soak path).
- Structural fix: per-tick simulation systems must run exactly once per
  tick regardless of frame cadence (run_if tick-advanced, or move the
  sim schedule onto the fixed-timestep runner in headless mode).
- Detection: a cheap post-run integrity check (duplicate same-tick
  identical-emission signature + cadence-series gap scan) wired into
  `just verdict` so a hitched run is flagged instead of silently
  gating/diagnosing on contaminated data.

## Out of scope

- CatSnapshot emitter redesign (the drop is a symptom).
- Re-litigating past byte-identity gates (351's gate compared two
  hitch-free runs — 428k lines identical; the 265 ladder likewise).

## Current state

Evidence preserved: `logs/tuned-42-ef08d805-s1-900s` (hitched run),
`logs/tuned-42-1effd660` (clean run, byte-identical prefix), analysis in
`docs/balance/310-s1-satiation-activation.md` §"Also surfaced".

## Approach

Start at the headless soak loop's timestep handling (where wall-clock
frames map to sim ticks); check whether the Update schedule is gated on
tick advance. The duplicate events all come from cadenced/per-tick
emitters, and the courtship-drift double-application narrows it to
systems that mutate on every Update run.

## Verification

- Unit/integration: drive the app with a simulated frame hitch (two
  Update runs, one tick advance) and assert per-tick systems applied
  their mutation once.
- Soak under artificial load reproduces zero duplicate-emission
  signatures post-fix.
- Integrity check flags the preserved hitched run
  (`tuned-42-ef08d805-s1-900s`) and passes the clean one.

## Log

- 2026-07-09: opened from 310 S1 gate work (release-plan step 23) with
  the full duplicate/drop/fork evidence. Not blocking S1 acceptance —
  the accepted artifact (`tuned-42-1effd660`) is clean and the neutrality
  claim closed via code-level read-site audit + 140k-line prefix
  identity. Priority consideration: step 24 (baseline re-promote) should
  use a hitch-checked run; the detection slice of this ticket would be
  timely before then.
