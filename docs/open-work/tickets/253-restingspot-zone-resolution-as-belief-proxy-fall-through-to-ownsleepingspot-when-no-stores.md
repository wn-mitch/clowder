---
id: 253
title: RestingSpot zone resolution as belief proxy — fall through to OwnSleepingSpot when no Stores
status: ready
cluster: ai-substrate
orchestration: substrate-sensitive
added: 2026-05-09
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

`RestingSpot` zone resolution at `src/systems/goap.rs:7766-7771`
currently picks the Manhattan-nearest position from
`stores_positions.iter().min_by_key(...).map(...)` — which yields
`None` when no Stores building exists in the colony. When the
zone resolves to `None`, the Resting plan template's
`ZoneIs(RestingSpot)` precondition fails, the Sleep step is
unreachable, and the disposition's plan completes with
`GoalUnreachable`. This is the cliff 247 diagnosed: in a no-Stores
world, cats elect Resting → plan fails → re-elect Resting next
tick → repeat. R4 (247) gated the cliff at the trigger-3 / IAUS
preempt layer (`intention_preempt_strength_regime_boundary`); 249
attempted a §4.3 TargetExistence-marker fix at the L1 / DSE-eligibility
layer and rolled back when that broke the
`AcuteHealthAdrenalineFlee` modifier's Sleep-lift landing target.

This ticket fixes the cliff at the **right layer**: zone resolution.

The Sleep DSE already has two memory-driven anchors that are
independent of Stores:

- `LandmarkAnchor::OwnSleepingSpot` — per-cat memory-derived
  sleep-spot location, populated by Sleep memory traces.
- `LandmarkAnchor::OwnSafeRestSpot` — per-cat memory-derived,
  threat-suppressed safe-rest spot via
  `interoception::own_safe_rest_spot` (ticket 089).

A cat with a positive-weight memory-based sleep spot SHOULD be
able to plan Resting and travel to that spot — even without a
colony Stores building. Today's zone resolver doesn't read those
anchors; it only looks at `stores_positions`.

The substrate-correct fix is for `RestingSpot` resolution to:

1. First, try `stores_positions` (current behavior — colony-scale
   den).
2. If empty, fall through to per-cat `OwnSleepingSpot` (memory-
   derived den).
3. If empty, fall through to `OwnSafeRestSpot` (threat-suppressed
   safe spot).
4. If empty, return `None` (no plannable rest target — let
   `replan_count` cap fire and §7.2 drop the intention).

This makes the zone honestly belief-proxy-shaped: the cat's
*belief* about where it can rest is the cumulative output of
percept (Stores in colony) + memory (places I've slept before /
felt safe). When the cat genuinely has no plannable rest target,
the planner backs off cleanly via `replan_count` cap rather than
hammering — which is the §12.3 channel (b) `achievable_believed`
proxy doing its job.

## Scope

1. **Re-shape `RestingSpot` zone resolution** at
   `src/systems/goap.rs:7766-7771` (or wherever the zone resolver
   currently lives — verify the line number after
   `BdiInteractions` refactor) to fall through to per-cat memory
   anchors when `stores_positions` is empty.
2. **Verify `replan_count` cap fires correctly** when *no*
   plannable rest target exists (no Stores AND no memory). The
   §12.3 channel (b) hard-fail path should produce a clean §7.2
   drop, not a tight loop.
3. **Add a regression scenario** under `src/scenarios/` —
   `resting_with_memory_no_stores`: a cat with a positive-weight
   `OwnSleepingSpot` memory but no colony Stores. Expected:
   Sleep wins L3, plans `[TravelTo(memory_spot), Sleep]`,
   travels, sleeps successfully.
4. **Document the layered zone resolution** in spec §4.7
   (substrate-vs-search-state) — the resolver IS substrate (it
   composes percept + memory into a belief), and zone resolution
   is the substrate-correct cliff fix layer.

## Out of scope

- **Authoring `OwnSleepingSpot` from outside Sleep memory.** That
  axis is already populated by Sleep memory traces; this ticket
  reads it, doesn't change its author.
- **Changing the cooldown's behavior.** When all three anchors
  are empty AND the cat keeps electing Resting, that's exactly
  the categorical-aspirational gap the cooldown was designed
  for — but extending its match arms isn't this ticket's scope
  (per 249's reframe doctrine; see modifier rustdoc §"Substrate
  posture").
- **Retiring `AcuteHealthAdrenalineFlee`.** That's ticket 251.
  Once the zone resolver is honest about memory-based anchors,
  the cliff fix doesn't need DSE-eligibility gating, so 251's
  retirement is structurally simpler — but the two tickets are
  independent.

## Current state

- 247 (landed) gated the cliff at the trigger-3 / IAUS preempt
  layer via `intention_preempt_strength_regime_boundary`. That
  works for the seed-42 case but is a tuning-constant lever, not
  the substrate-correct fix.
- 249 (closed without landing) attempted a §4.3 TargetExistence-
  marker fix at the DSE-eligibility layer and broke the
  `AcuteHealthAdrenalineFlee` modifier's Sleep-lift landing
  target (11× modifier-preempt regression).
- The §4.7 substrate-vs-search-state classifier:
  zone-resolution-with-memory IS substrate (it doesn't change
  during a single replan — it changes when memory or
  stores_positions change), so this fix sits cleanly in the
  substrate column.
- Per-cat memory anchors already exist and are read by the
  Sleep DSE's `SpatialConsideration` axes (`sleep_spot_distance`
  via `OwnSleepingSpot`, `safe_rest_distance` via
  `OwnSafeRestSpot`). The DSE *already believes* in memory-based
  spots; only the zone resolver doesn't.

## Approach

1. **Phase A — verify the zone resolver location.** Read
   `goap.rs:7766-7771` (or its current line range) and confirm
   the `stores_positions.iter().min_by_key(...).map(...)` shape.
   Identify the resolver function name + signature.
2. **Phase B — extend the resolver.** Add the fall-through chain:
   stores → `OwnSleepingSpot` → `OwnSafeRestSpot` → None. Each
   anchor read goes through the existing
   `interoception::own_safe_rest_spot` /
   `interoception::own_sleeping_spot` helpers (ticket 089) — no
   new author site needed. Returns the closest of the available
   anchors (Manhattan-nearest, matching today's stores_positions
   behavior).
3. **Phase C — scenario.** New `resting_with_memory_no_stores`
   scenario: 1 cat at (10, 10), positive-weight `OwnSleepingSpot`
   memory at (15, 15), no Stores building, no threats. Expected:
   Sleep wins L3, plans `[TravelTo((15,15)), Sleep]`, executes
   cleanly.
4. **Phase D — verification soak.** `just soak-trace 42 Mallow` +
   `just verdict logs/tuned-42 --baseline <post-247>`. Required:
   (a) Hard gates pass; (b) modifier-preempt rate stays at
   post-247 baseline (~347/10kt) — no regression like 249's;
   (c) Sleep events per 10kt within ±10% of baseline; (d) the
   no-Stores cliff scenario from 247 (with
   `intention_preempt_strength_regime_boundary = 0.0`) does NOT
   re-collapse — the cat plans to a memory spot when no Stores.

## Verification

- The new scenario `resting_with_memory_no_stores` passes:
  cat plans Resting and reaches the memory anchor.
- `intention_preempt_strength_regime_boundary = 0.0` cliff-replay
  scenario does NOT re-collapse: cats with memory anchors plan
  successfully; cats without memory hit `replan_count` cap
  cleanly via §7.2 drop.
- Modifier-preempt rate stays at post-247 baseline (~347/10kt;
  no regression — the `AcuteHealthAdrenalineFlee` Sleep-lift
  landing target is preserved because Sleep stays eligible).
- Hard gates + continuity canaries hold.
- Frame-diff per-DSE drift on Sleep within concordance band.
- `just check` clean.

## Log

- 2026-05-09: opened from 249's audit. 249 attempted to gate
  the cliff at the L1 / DSE-eligibility layer and broke the
  AcuteHealthAdrenalineFlee Sleep-lift landing target. The
  substrate-correct cliff fix layer is **zone resolution** —
  the resolver IS the substrate composition of percept (Stores)
  + memory (OwnSleepingSpot / OwnSafeRestSpot) into a "where can
  I rest" belief. Sleep DSE already reads memory anchors via
  spatial axes; only the zone resolver lags.
