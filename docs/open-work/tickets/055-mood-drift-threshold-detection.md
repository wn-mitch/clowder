---
id: 055
title: §7.7.d mood drift-threshold detection — sustain-duration + arc-misalignment trigger
status: blocked
cluster: null
added: 2026-04-27
parked: null
blocked-by: [056]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

`src/systems/mood.rs` valence today has no hysteresis or sustain-duration
detection. §7.7.d aspirations need "valence below X for N seasons AND
misalignment with active-arc expected-mood" to fire mood-driven
aspiration reconsideration. Without this layer, mood-driven aspiration
shifts can't fire — the trigger surface doesn't exist.

## Scope

- Hysteresis layer on `mood.rs`: track sustained-below-threshold
  duration per cat without flickering on transient swings.
- Arc-misalignment comparator: per-arc expected-valence target (lands
  via 056 aspiration catalog) compared against current sustained value.
- Reconsideration emission when both conditions satisfied for ≥ N
  seasons.

## Out of scope

- Aspiration catalog itself (that's 056).
- Per-arc valence target tuning (balance work post-056).

## Approach

Design-heavy — its own small balance thread. Mark requires
sustained-divergence-with-context detection; both sustain-duration
and arc-misalignment must hold simultaneously. The hysteresis and
the arc-misalignment comparator can land together once 056 supplies
per-arc expected-valence targets.

## Verification

- Unit tests on hysteresis: short dips don't trigger; sustained dips do.
- Unit tests on arc-misalignment: valence below threshold but matching
  arc expectation does NOT trigger; mismatched does.
- Integration test: full reconsideration fire end-to-end.
- `just check` green.
- Soak verdict: behavior shift on mood-driven aspiration cadence;
  hypothesis required per CLAUDE.md balance methodology.

## Log

- 2026-04-27: opened from ticket 013 retirement (spec-follow-on debts
  umbrella decomposition). Original sub-task 13.4 in spec
  `docs/systems/ai-substrate-refactor.md` §7.7.d.
