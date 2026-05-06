---
id: 196
title: verdict.py substrate-fired-≥1× probe (194 P7)
status: ready
cluster: process-discipline
added: 2026-05-06
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Closes 194 F3 / P7. Ticket 189 v1's "RNG noise" verdict came
from a 5-seed × 300s sweep. The 300s window cannot exercise
the disposal substrate at all — `ColonyStoresChronicallyFull`
needs chronicity windowing, `HasGroundCarcass` needs accumulated
overflow, `HasHandoffRecipient` needs kittens to be born. The
sweep measured schedule-edge RNG perturbation, not substrate
behavior, but its verdict was treated as authoritative for
both.

A sweep that is structurally incapable of exercising the
hypothesized mechanism is not authoritative for that mechanism.
The cheap probe: did the named substrate Feature(s) fire ≥ 1×
in any seed of the sweep? If not, the sweep is "unprovable at
this duration" for that hypothesis.

## Direction

Extend `scripts/verdict.py` (single-run) and
`scripts/sweep-stats.py` (multi-run) with a `--require-feature
<Feature::Name>` flag:

- Reads `positive_features_total` / `negative_events_total` /
  per-Feature counts from the run's footer (or the
  `SystemActivation` events stream if footer counters are
  insufficient).
- Returns `unprovable` (new exit code or band) when the named
  Feature fired 0× across all seeds of a sweep.
- For `verdict <run-dir>` mode, surfaces a top-level
  `features_fired: { <name>: <count> }` block for any Features
  the caller flagged via `--require-feature`.

Optionally extend the `next_steps` line so when a hypothesis-
naming caller passes `--require-feature` and the count is 0,
the verdict suggests *"increase soak duration to ≥ N ticks"* or
*"this run cannot evaluate the hypothesis on Feature X"*.

## Out of scope

- Auto-detecting which Features a hypothesis is "about" —
  caller passes them explicitly. This ticket adds the
  mechanism, not policy.
- Building a sweep-level CLI that loops `--require-feature`
  over every seed automatically — `sweep-stats.py` already
  iterates seeds; this just adds a column.

## Verification

- Unit: feed `verdict.py` a footer with `Feature::Foo: 0` and
  `--require-feature Foo` → reports unprovable.
- Integration: run against `logs/sweep-189-pre-178-mini` (a
  300s sweep where disposal substrate provably didn't fire)
  with `--require-feature ItemDropped` → reports unprovable
  for that Feature.
- Existing `just verdict <run-dir>` callers without the flag
  see no behavior change.

## Log

- 2026-05-06: opened from 194's closeout. Cluster
  `process-discipline`. Lightweight extension to verdict.py /
  sweep-stats.py — additive flag, no breaking changes.
