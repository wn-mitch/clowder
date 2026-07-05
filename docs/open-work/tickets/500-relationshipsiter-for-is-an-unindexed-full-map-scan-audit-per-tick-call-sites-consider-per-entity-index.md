---
id: 500
title: Relationships::iter_for is an unindexed full-map scan — audit per-tick call sites, consider per-entity index
status: ready
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-06-09
parked: null
supersedes: []
blocked-by: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
The 459 layer-walk found that `Relationships::iter_for(entity)`
(`src/resources/relationships.rs:125`) is **not** an indexed lookup — it
filters the entire pair-keyed BTreeMap on every call. Any per-tick path
that calls it per-actor (or worse, per-candidate) silently multiplies by
total pair count: the 453 Mates-exclusivity gates inside the courtship
matchmaker cost O(cats² × pairs) per tick and were the dominant
self-time in `author_joint_intentions`' 22% flamegraph share. The 427
doc-comment sells `iter_for` as the "no-alloc" alternative to `all_for`,
which hides the scan cost — callers reasonably assume O(degree).

This is the same knife-shape as the 480 epic's pair-keyed
`BTreeMap::retain` family (485/486): per-tick full-map traversal of
pair-keyed state.

## Scope
- Audit every `iter_for` / `all_for` call site reachable from a per-tick
  system (grep + schedule walk). Classify: per-actor-per-tick,
  per-candidate-per-tick (worst), event-driven (fine).
- For hot sites, either hoist to a once-per-tick precomputed set/map
  (the 459 fix shape — cheapest, behavior-preserving) or add a
  per-entity adjacency index to `Relationships` (BTreeMap<Entity,
  BTreeSet<Entity>> maintained on insert/remove; BTree for the 431
  iteration-order discipline). The index is the structural fix if ≥3
  hot sites remain after hoisting.
- Update the `iter_for` doc-comment to state the O(pairs) cost either
  way.

## Out of scope
- The 459 courtship-matchmaker sites (fixed via the per-tick
  `mates_bonded` set, commit d30f3f48).
- Coordinator-election sums (already documented as BTreeMap-order
  load-bearing; touch only with a determinism gate).

## Current state
Audit complete (2026-07-05, anchored on the 06-09 flamegraph at `bdc71254`,
`modify_familiarity` 17.15% incl / 16.55% self — top child `BTreeMap::entry`):

| call site | cadence | flamegraph-visible | disposition |
|---|---|---|---|
| `social.rs::passive_familiarity` → `modify_familiarity` per near-pair | per-pair-per-tick | **yes — the knife (16.55% self)** | [verified-hot] fixed — `modify_familiarity_batch` merge-join |
| `disposition.rs:983` → `social_status_distress` → `iter_for(nearest_other)` | per-cat-per-tick | no (≤ the 2.75% shared BTreeMap-iter frame) | [suspect-warm] left; re-audit if it surfaces post-500/502 |
| `mate_target.rs:205` per-candidate Mates scan | per-candidate at eval cadence | no | [suspect-warm] 459 sister-shape; hoist to a mates set if it ever surfaces |
| `joint_intention.rs:740/791` | `debug_assert!` only | release-free | fine |
| `aspirations.rs::track_milestones` FormBond arm | per-active-FormBond-aspiration | no | fine (event-scale) |
| `snapshot.rs:130` | snapshot cadence (arena-reused) | no | fine |
| `coordination.rs:44` | election cadence | no | out of scope (BTreeMap-order load-bearing) |
| `cat_inspect.rs:411` / `social.rs` tests | UI / test | n/a | fine |

Per the Approach's own rule (index only if ≥3 hot sites remain after
hoisting): **one** hot site existed; the per-entity adjacency index is not
justified. `iter_for` doc-comment now states the O(pairs) cost.

## Approach
1. `rg "iter_for|all_for" src/` → table of call sites × calling system ×
   cadence.
2. Flamegraph confirms which are visible before touching anything
   (memory: perf refactors need flamegraph pre/post).
3. Prefer hoisted per-tick snapshots over the index unless the audit
   shows broad pressure; preserve iteration order wherever sums feed
   tiebreaks.

## Verification
- `just ci` (incl. determinism gate) per change; byte-identical event
  stream over the common tick range for behavior-preserving swaps.
- `just flamegraph 42 60` pre/post; targeted symbol shrinks.
- `just verdict` survival + continuity canaries green.

## Log
- 2026-06-09: opened from 459's layer-walk (knife #1: Mates-gate scans).
- 2026-07-05: audit table landed (see Current state). Fix:
  `Relationships::modify_familiarity_batch` — single merge-join co-walk of
  `NearPairCache::pairs` and `Relationships::data` (both BTreeMaps over the
  same normalize_key canonicalization), replacing one O(log pairs) `entry`
  descent per near-pair per tick in `passive_familiarity`. Missing pairs
  collected and inserted after the walk (get_or_insert parity). Unit test
  asserts bit-identical familiarity vs N individual calls. Verification
  running: 900s soak-trace 42/Simba + verdict + post-flamegraph.
