---
id: 486
title: update_near_pair_cache death-retain — drive eviction off CatDied Message (13 percent inclusive CPU at HEAD, 480 child)
status: done
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-05-28
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 9a05a29c
landed-on: 2026-05-30
---

## Why
`update_near_pair_cache` is the #3 hot frame at HEAD: **13.27% inclusive CPU**
on a 60s seed-42 flamegraph. Ticket 431 Stage B already retired the O(N²)
distance sweep behind `CatMoved` — what's left is two unconditional
`BTreeMap::retain` calls in `src/systems/social.rs` (lines 64-66 and 88-92).
The first retain filters out pairs whose either endpoint is no longer "live"
(walks every pair every tick to check `live.contains(&a) && live.contains(&b)`);
the second drops moved-cat pairs before re-scanning. Both walk
`cache.pairs` in full every tick. Together they are most of the system's
inclusive CPU — and the eviction work they do only matters in two narrow
cases (a cat just died, or a cat just moved) that are already first-class
event signals.

## Scope
- Replace the live-set retain (line 64-66) with a `CatDied` Message reader:
  on each death, remove every pair containing that entity from `cache.pairs`.
- The moved-cat retain (line 88-92) is already gated on `!moved.is_empty()`
  but still walks the full map; refactor to iterate `moved` and remove its
  pairs directly (O(M × P_avg) instead of O(P_total)).
- Preserve `BTreeMap` iteration order and `normalize_pair` canonicalization
  — seed-42 byte-identity must hold.

## Out of scope
- The `passive_familiarity` and `SustainedCoPresence` consumers of
  `NearPairCache` — they read the cache, they don't write it.
- The 431-Stage-B `CatMoved`-driven incremental scan itself — only the
  eviction side changes.

## Current state
HEAD = 50f5fb77; HEAD flamegraph at `logs/flamegraphs/42-50f5fb77e342/`:
- `update_near_pair_cache` self 0.42% / inclusive 13.27%
- Child `<BTreeMap as ExtractIf>::next` (frame #4 in 05-23 profile, still
  dominant at HEAD) is the cost site

`CatDied` Message exists in the codebase already (used by other consumers);
this ticket adds a new reader inside `update_near_pair_cache`.

## Approach
1. Add `MessageReader<CatDied>` to the system's params.
2. Drain `CatDied` first; for each dead entity, walk only its own keys
   (`pair.0 == dead || pair.1 == dead`) and remove. Use `BTreeMap::range`
   on `(dead, Entity::MIN) ..= (dead, Entity::MAX)` for the `pair.0 == dead`
   half (O(log P + deg(dead))), and a full walk only for the `pair.1 == dead`
   half (BTreeMap can't range on the second coordinate without a secondary
   index — but the deg(dead) bound makes it cheap in practice).
3. Drop the unconditional live-retain. The moved-retain stays gated, but
   refactor to walk `moved` rather than `cache.pairs`.

**Determinism gate.** `#[cfg(debug_assertions)]` invariant assert: at the end
of the system, compute a fresh "live × live" pair set and panic if it
diverges from `cache.pairs.keys()`. Drop the assert before commit. Same 431
Stage B drift-detection precedent.

**Coupling with 485.** 485's lazy-eviction relies on `NearPairCache.pairs`
reflecting reality each tick. After 486 lands, `cache.pairs` only loses
entries on `CatDied` or `CatMoved` — i.e., the same events 485 could read
directly. If 485 lands first with approach (1) lazy-batch, 486 is independent;
if we ever pick 485 approach (2) (`NearPairDropped` emit), 486 is the place
to emit it.

## Verification
- `just flamegraph 42 60` + `samply_top.py --target update_near_pair_cache`
  — inclusive% should fall from 13.27% to under 5%.
- `just soak 42` + `just verdict logs/tuned-42-<sha>` — all hard survival
  gates + continuity canaries hold (this is behavior-preserving).
- The Stage B debug-only divergence assertion (already in
  `passive_familiarity`, lines 143-167) must continue to pass.

## Log
- 2026-05-28: opened from 480 flamegraph-bisect Phase 1. HEAD profile
  `42-50f5fb77e342` ranks `update_near_pair_cache` at 13.27% inclusive,
  with the retains dominating. Coupling with 485 noted.
- 2026-05-30: Implementation rode in on commit 9a05a29ca438 (feat: 487). The live-set diff against last_seen replaces the ticket-proposed CatDied reader — the cats query is permissive (admits wildlife/items via Without<Dead>, Without<Structure>) and those entities despawn directly rather than via Dead insertion. CatDied would leak entries and trip passive_familiarity's debug divergence guard; rationale documented at social.rs:71-78.
