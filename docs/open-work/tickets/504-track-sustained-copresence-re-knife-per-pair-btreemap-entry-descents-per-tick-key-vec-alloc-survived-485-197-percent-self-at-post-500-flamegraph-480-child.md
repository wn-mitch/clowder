---
id: 504
title: track_sustained_copresence re-knife — per-pair BTreeMap entry descents + per-tick key-Vec alloc survived 485 (19.7 percent self at post-500 flamegraph, 480 child)
status: ready
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-07-05
parked: null
supersedes: []
blocked-by: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
`track_sustained_copresence` is the #1 hot frame at the post-500
flamegraph (`logs/flamegraphs/42-6571f9f6c0a8`): **19.66% self /
21.68% inclusive**, top children `BTreeMap::Keys` iteration + entry
walks. The 485 fix retired the per-tick full-map `retain` (lazy
`last_touched_tick` eviction + periodic GC) but left the same
knife-shape ticket 500 just removed from `passive_familiarity`: one
`tracker.pair_ticks.entry(key)` root-to-leaf descent **per cached
near-pair per tick** (sustained_copresence.rs:98), plus a per-tick
`Vec` collect of every cache key (line 93) whose stated aliasing
rationale is void — `Res<NearPairCache>` and
`ResMut<SustainedCoPresenceTracker>` are disjoint system params; there
is no aliasing to avoid. This is not erosion of 485 — the entry-walk
cost was always there and its share grew as the surrounding pie shrank
(459, 500).

## Scope
Same remedy as 500's `modify_familiarity_batch`, adapted for the
tracker's two-map read pattern:
- Merge-join co-walk of `cache.pairs` (sorted) and
  `tracker.pair_ticks.iter_mut()` (sorted, same `normalize_pair`
  canonicalization): matched keys run the increment/discontinuity
  logic in place; cache keys missing from the tracker are collected
  and inserted after the walk (new pairs). Threshold/cooldown/emit
  branches (rare — only at `count >= threshold`) keep their pointwise
  `last_emit` lookups. The despawned-endpoint `remove` arms
  (lines 135-142) collect keys during the walk, remove after (can't
  mutate mid-iter_mut).
- Delete the line-93 `pair_keys` Vec collect — iterate `cache.pairs`
  directly against the mutable tracker borrow.
- Emission order stays ascending key order (merge-join preserves it)
  → byte-identical event stream expected.
- Keep the 485 debug invariant (post-system every cache pair touched
  this tick) and the periodic GC unchanged.

## Out of scope
- `integrate_beliefs` (10.79% self at the same flamegraph, HashMap
  retains × 4 maps × ~6 facet decays per stagger cat) — sibling knife;
  decide after this lands whether its share still clears the bar
  (plan.md step 2 pre-authorizes knifing it before Phases IV–V add
  belief writers).
- `update_near_pair_cache` (0.96% self / 6.05% incl — mostly the
  rescan Vec build): 486 already landed its eviction fix; remainder is
  not knife-worthy this pass.

## Verification
- Gate: behavior-preserving — byte-identical event stream over the
  common tick range vs a pre-change soak at the same toolchain
  (Patrol-score ULP diffs attributable to 503 by exact signature are
  the only tolerated exception; structural diffs fail).
- `just flamegraph 42 60` pre (= `42-6571f9f6c0a8`) / post; symbol
  self-time target < 5%.
- `just check && just test`; `just verdict` pass incl.
  `throughput_drift`.

## Log
- 2026-07-05: opened as 480 child from the post-500 flamegraph, per
  plan.md Phase I step 2. Diagnosis pre-verified by code read
  (sustained_copresence.rs:93,98) — the 485 comment's aliasing
  rationale for the key-Vec is factually void.
