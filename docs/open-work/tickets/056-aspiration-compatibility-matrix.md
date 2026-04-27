---
id: 056
title: §7.7.1 aspiration compatibility matrix — hard/soft conflict enumeration
status: ready
cluster: null
added: 2026-04-27
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

§7.7.1 commits four conflict classes in spec — hard-logical /
hard-identity / soft-resource / soft-emotional — but the specific
hard-logical + hard-identity pair list is enumeration work against the
stabilized aspiration catalog in `src/systems/aspirations.rs`. Without
the matrix, aspiration compatibility checks fall back to ad-hoc
per-callsite logic.

## Scope

- Per-pair compatibility matrix data structure (likely
  `HashMap<(AspirationKind, AspirationKind), ConflictClass>` or
  equivalent table form).
- Enumerate the hard-logical pairs (mutually-exclusive aspirations
  whose simultaneous active state is incoherent).
- Enumerate the hard-identity pairs (aspirations whose simultaneous
  hold contradicts the cat's identity / personality).
- Soft-resource and soft-emotional classes get their default per-pair
  weights for the scoring layer.
- Wire compatibility checks at aspiration insertion / reconsideration
  sites.

## Out of scope

- Mood drift-threshold detection — that's 055, which gates on this
  ticket landing the per-arc valence targets.
- New aspiration kinds; this ticket *enumerates* against the existing
  catalog.

## Approach

Read `src/components/aspirations.rs` for current `AspirationKind`
variants. Walk pairwise combinations; classify each per spec §7.7.1.
Add unit tests covering each hard-class pair so the table can't drift
from intent.

## Verification

- Unit tests per pair classification.
- Integration test: a cat carrying aspiration A cannot accept
  aspiration B if (A, B) is hard-incompatible.
- `just check` green.
- Soak verdict: behavior shift expected on aspiration cadence /
  reconsideration timing; hypothesis required per CLAUDE.md balance
  methodology.

## Log

- 2026-04-27: opened from ticket 013 retirement (spec-follow-on debts
  umbrella decomposition). Original sub-task 13.5 in spec
  `docs/systems/ai-substrate-refactor.md` §7.7.1.
