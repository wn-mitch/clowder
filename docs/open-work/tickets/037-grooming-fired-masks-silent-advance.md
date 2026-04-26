---
id: 037
title: GroomingFired event masks silent-advance — continuity canary lies when target picker returns None
status: ready
cluster: null
added: 2026-04-26
parked: null
blocked-by: []
supersedes: []
related-systems: [social.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

In the seed-42 soak at `a879f43`, **`Feature::GroomedOther` fired 0 times while `GroomingFired` events fired 71 times**. The continuity canary `grooming = 71` passes; the Activation canary `GroomedOther = 0` fails. They disagree because the event log re-creates the silent-advance shape that `StepOutcome<W>` was specifically designed to prevent.

`src/systems/goap.rs:2772-2788`:

```rust
outcome.record_if_witnessed(narr.activation, Feature::GroomedOther);   // gated on witness
...
if matches!(outcome.result, StepResult::Advance) {
    log.push(EventKind::GroomingFired { cat, target: ... });           // unconditional on Advance
}
```

`resolve_groom_other` in `src/steps/disposition/groom_other.rs:102-112`:

- `witnessed_with(Advance, GroomOutcome { ... })` when `target_entity == Some(_)` at duration end.
- `unwitnessed(Advance)` when `target_entity == None` at duration end.

The relationship/fondness mutations and the `colony_map.absorb` exchange (lines 75-92) only run inside `if let Some(target) = target_entity { ... }`. The actor's `fulfillment.social_warmth` boost (lines 99-100) runs *unconditionally on completion*. So a targetless groom completes with: zero relationship mods, zero hunting-prior exchange, zero fondness change — but the actor's own social_warmth axis still ticks up, and `GroomingFired` still logs.

**The continuity canary `grooming = 71` is currently counting plan-completion ticks, not actual grooming events.** The 71 GroomingFired emissions in this soak are 71 cats that decided to groom, completed the duration timer, and walked away without anyone to groom. No social interaction occurred.

## Two questions converge

1. **Why does `resolve_groom_other_target` always return `None`?** `src/ai/dses/groom_other_target.rs:171-208` filters cat candidates by `cat_pos.manhattan_distance(other_pos) <= GROOM_OTHER_TARGET_RANGE`. If the range constant is too tight relative to typical inter-cat distance in the 8-cat colony on the canonical map, the candidate list is empty for every actor. Verify by reading `GROOM_OTHER_TARGET_RANGE` against typical pairwise distances from `CatSnapshot` events.
2. **Should `GroomingFired` be witness-gated, or should the unwitnessed path Fail instead of Advance?** Either change makes the continuity canary honest:
   - **Option A (cheap):** move `EventKind::GroomingFired` inside `if outcome.witness.is_some()`, so it only logs when grooming actually happened. `grooming` continuity tally then reflects real grooms.
   - **Option B (deeper):** change `resolve_groom_other` to return `unwitnessed(Fail)` (or `unwitnessed(Continue)` until target appears) when target_entity is None at duration end. The plan drops, the cat re-decides; better than completing a no-op action.

## Suspect cohort

- §6.5 target-taking DSE port for groom_other (Phase 4c.6) wired the picker but may have shipped with a too-tight range constant — earlier dispositions used `find_social_target` (fondness-only) which had different range semantics.
- The §7.W warmth refactor split the fulfillment axis off from `needs.temperature`; that refactor moved the social_warmth boost out of the `if Some(target)` block, which means a targetless groom now has *some* effect (the actor's own warmth) where it previously was a complete no-op. This may have masked the symptom: the cat "feels socially warmer" without anyone present.

## Investigation steps

1. Print `GROOM_OTHER_TARGET_RANGE` and the median pairwise cat distance over a 60s seed-42 probe. If range < median distance, balance the range.
2. Run a focal-cat trace (`just soak-trace 42 Mocha`) and grep the L2 records for `groom_other_target` candidates lists. Are candidates being filtered, or is the candidate list empty before scoring?
3. Decide A vs B above based on what the focal trace shows.

## Concordance prediction

- If the picker is range-starved: bumping range so ≥1 candidate is typically in range ⇒ targets resolve, witness becomes `Some`, `Feature::GroomedOther` fires repeatedly per soak (≥10× expected at the current 71-Advance cadence).
- If A: `GroomingFired` event count drops from 71 to whatever the real-grooms count is (likely near 0 until the picker is fixed), continuity canary `grooming` correctly fails until then.

## Non-goals

- Tuning `groom_other_duration`, `groom_other_fondness_per_tick`, or any of the per-tick social/fondness/familiarity mod constants. The witness gap is the load-bearing failure; balance the post-fix behavior in a follow-on.
