---
id: 345
title: §7.7.1 aspiration soft-class per-pair weights
status: parked
cluster: ai-substrate
initiative: []
added: 2026-05-14
parked: 2026-05-14
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Spec §7.7.1 names four conflict classes; ticket 056 landed the two
hard classes (`HardLogical`, `HardIdentity`) as a sparse per-chain
`incompatible_with` matrix. The soft classes (`SoftResource`,
`SoftEmotional`) get *default per-pair weights* per spec, but **no
consumer reads them today**:

- `SoftResource` tensions resolve "at the emitted-Intention layer via
  normal softmax" — the existing DSE softmax already arbitrates
  competing intentions on cat-hours, no per-pair weight needed.
- `SoftEmotional` tensions drop via §7.7 reconsideration when mood-
  drift fires — that's ticket 055's hysteresis layer, which reads a
  *per-arc valence target* (ticket 344), not per-pair weights.

Landing soft-class weights without a consumer would be a substrate
stub of the kind CLAUDE.md forbids. Parked until a §3.5.1 modifier or
a 055-follow-on materializes that genuinely needs per-pair soft
weights.

## Scope (when this unparks)

- Encoding: extend the `incompatible_with` shape or open a parallel
  `soft_conflicts: &'static [(...)]` table — soft classes are dense
  enough across cross-domain pairs that the per-chain encoding may not
  scale; reconsider when a consumer surfaces.
- Default per-pair weights per spec §7.7.1 table.
- Reader integration at the consumer's site.

## Out of scope

- Hard-pair matrix (landed in 056).
- Per-arc valence targets (ticket 344).

## Current state

Parked at open time. Unblocking signal: a balance investigation or
spec-side decision that names a per-pair soft weight as load-bearing
for emitted-Intention arbitration or for reconsideration drift.

## Approach

Defer. When a consumer surfaces, draft the encoding then — the
per-chain `incompatible_with` shape from 056 doesn't extend naturally
to soft pairs (you'd need to list nearly every cross-domain pair), so
a parallel sparse table or a domain-pair default with chain-level
overrides is the likely shape.

## Verification

Deferred until unparking.

## Related work

- 056 — sister ticket; landed the hard pairs.
- 344 — sister ticket; per-arc valence targets that 055 reads.
- 055 — soft-emotional drift detector; doesn't need per-pair soft
  weights, only per-arc valence targets.

## Log

- 2026-05-14: opened parked as a 056 follow-on (split-out per
  CLAUDE.md antipattern-migration discipline). No consumer exists
  today.
