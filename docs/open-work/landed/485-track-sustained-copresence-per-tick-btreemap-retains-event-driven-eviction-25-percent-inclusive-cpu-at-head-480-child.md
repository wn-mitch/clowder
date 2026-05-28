---
id: 485
title: track_sustained_copresence per-tick BTreeMap retains — event-driven eviction (25 percent inclusive CPU at HEAD, 480 child)
status: done
cluster: ai-substrate
initiative: []
added: 2026-05-28
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: d82cd645
landed-on: 2026-05-28
---

## Why
`track_sustained_copresence` (ticket 279, landed 2026-05-26) is the #1 hot
frame at HEAD: **25.37% inclusive CPU**, with its dominant cost at frame #2 of
the entire profile — `<BTreeMap as ExtractIf>::next` at **28.13% self**. The
system maintains two `BTreeMap`s keyed by canonicalized pair-`(Entity,Entity)`
(`pair_ticks` and `last_emit`) and calls `.retain()` on both every tick to
garbage-collect entries whose pair is no longer in `NearPairCache.pairs`.
With ~28 candidate pairs at peak population (8 cats), each retain still walks
the full map every tick, and those two retains are most of the system's cost.
The retain-pattern is a structural anti-pattern shared with 486
(`update_near_pair_cache`'s death-retain).

## Scope
- Replace the two unconditional `BTreeMap::retain` calls in
  `src/systems/sustained_copresence.rs` (lines 78-80 and 154-157) with an
  event-driven eviction path.
- Preserve emission semantics byte-for-byte on seed-42 — this is
  behavior-preserving perf work, not a balance change.

## Out of scope
- The sustained-copresence semantics themselves (threshold, cooldown,
  symmetric emit) — untouched.
- The `NearPairCache` write path itself — that's ticket 486.

## Current state
HEAD = 50f5fb77; 60s seed-42 flamegraph at `logs/flamegraphs/42-50f5fb77e342/`
shows:
- `track_sustained_copresence` self 7.69% / inclusive 25.37%
- Child `<BTreeMap as ExtractIf>::next` (frame #2 of whole profile) self 28.13% — the retains
- Sibling `<BTreeMap as Iter>::next` (frame #12) self 1.94% — the `cache.pairs.keys()` walk on line 84

Seed-42 daily p90 dropped from 71.9 t/s (05-26) to 60.4 t/s (HEAD) — coincident
with 279's landing and its accumulator state growing.

## Approach
Three candidate shapes, in increasing surgery:

1. **Lazy-eviction-in-loop.** Inside the main loop (which already walks
   `cache.pairs.keys()`), naturally only touch pairs that are still live.
   Detect stale entries by adding a `last_touched_tick: u64` to the value
   struct; do a single retain every N ticks (e.g., N = cooldown) to batch GC.
   Cuts retain cost ~Nx; tiny diff.
2. **Drive eviction off a `NearPairDropped` Message** emitted by
   `update_near_pair_cache` when a pair is retained-out. Cleanest substrate
   shape; matches the 431 Stage B pattern. Requires touching 486's site too
   — couple with 486 or land in sequence.
3. **Replace the `pair_ticks`+`last_emit` `BTreeMap`s with a slab keyed by
   `NearPairCache` index.** Too invasive; defer unless 1 and 2 don't move
   the needle.

Recommended: start with (1) — lazy + batched retain — because it's a single
file's diff with no cross-system coupling, and confirm via flamegraph + verdict
before considering (2).

**Determinism gate.** Wrap both `pair_ticks` and `last_emit` mutations in a
`#[cfg(debug_assertions)]` invariant assert that compares the cached state
against a freshly-computed reference set during a 60s smoke run. Drop the
assert before commit per 431 Stage B precedent.

## Verification
- `just flamegraph 42 60` + `samply_top.py --target track_sustained_copresence`
  — inclusive% should fall from 25.37% to under 10%.
- `just soak 42` + `just verdict logs/tuned-42-<sha>` — all hard survival
  gates + continuity canaries hold (this is behavior-preserving).
- Cross-check the `5×` SustainedCoPresence Feature is still firing in the
  footer's `continuity_tallies` (the 279 invariant).
- p90 ticks/sec on the next post-fix soak should be visibly higher than
  HEAD's 60.4 t/s baseline.

## Log
- 2026-05-28: opened from 480 flamegraph-bisect Phase 1. HEAD profile
  `42-50f5fb77e342` ranks copresence at 25.37% inclusive, retains at 28.13%
  self. Recommended approach (1) — lazy + batched retain.
- 2026-05-28: landed at `d82cd645` via approach (1) — added `last_touched_tick`
  to `pair_ticks` values; main loop resets count to 1 on discontinuity (no
  per-tick retain). Periodic batched GC every 5 cooldowns (~1000 ticks).
  Debug-only invariant assert kept (last_touched == tick for every cache
  pair, modulo despawn-mid-tick removals).

  **Flamegraph (60s seed-42, samply 997 Hz, same HEAD commit pre/post):**
  | frame | pre | post |
  |---|---:|---:|
  | track_sustained_copresence inclusive | **25.37%** | **12.44%** |
  | track_sustained_copresence self | 7.69% | 10.81% |
  | BTreeMap ExtractIf::next (frame #2 pre) | 28.13% (parent: copresence) | 14.42% (parent: update_near_pair_cache) |

  **Apples-to-apples 60s soak comparison:**
  - pre-fix `50f5fb77` 60s: 7369 ticks → **122.8 t/s**
  - post-fix `d82cd645` 60s: 8464 ticks → **141.1 t/s**
  - delta: **+14.9%**

  **Determinism verification.** Within the common tick range (post-fix's first
  7369 sim-ticks against pre-fix's full 7369), event-type counts are
  byte-identical; first 1000 events sequence-identical. No behavior change.
  See `logs/short-prefix-50f5fb77/` and `logs/short-485-b70f9d3f/`.

  Next knife: 459 (author_joint_intentions) — now top frame at 25.4% inclusive
  share post-fix. 486 (update_near_pair_cache) is now #2 hot retain.
