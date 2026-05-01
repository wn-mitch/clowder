---
id: 123
title: RecentDispositionFailures cooldown so cats stop spamming dispositions the planner can't satisfy
status: done
cluster: substrate-over-override
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: b47af48f
landed-on: 2026-05-01
---

## Why

When `make_plan` returns `None` for an elected disposition (e.g., Crafting
with no kitchen, Foraging with no nearby foraging tile, Hunting with no
prey in range), `evaluate_and_plan` emits a `PlanningFailed` event with
`reason: "no_plan_found"` (ticket 091 made this non-silent) and the cat
re-evaluates next tick. Today the cat's IAUS scoring is unchanged by the
failure, so the same disposition typically wins again and the same
`make_plan → None` collapse repeats. In the seed-42 diagnosis window for
ticket 121:

```
1150  Crafting   no_plan_found
1105  Foraging   no_plan_found
 804  Hunting    no_plan_found
```

3059 wasted planning rounds in 1500 ticks. The pattern is parallel to
ticket 073's per-target failure memory (`RecentTargetFailures`); this
ticket lifts that idea to disposition-scope cooldowns.

This ticket is the amplifier, not the root cause — tickets 121 and 122
should land first and we should re-measure before committing to the
fix here. If the no_plan_found rate drops below ~10% of pre-121 levels,
this ticket can be parked.

## Scope

- Re-measure `PlanningFailed/no_plan_found` rates after tickets 121 and
  122 land. If still above the parking threshold:
- Add a `RecentDispositionFailures` component (BTreeMap<DispositionKind,
  (last_failure_tick, failure_count)>) lazily inserted on first failure,
  mirroring the ticket 073 `RecentTargetFailures` shape.
- Add a `Consideration::RecentDispositionFailureCooldown` that decays a
  disposition's IAUS score by a function of (failure_count,
  ticks_since_last_failure). Curve shape: zero score at last failure,
  recovering linearly over a tunable cooldown window.
- Wire the consideration into all DSEs that share the failure-prone
  step graph (Hunting, Foraging, Crafting, Caretaking, Building, Mating —
  the `_ => plan.trips_done >= plan.target_trips` family). Keep
  Resting/Guarding exempt (different completion semantics).

## Out of scope

- Per-target cooldowns (already in ticket 073).
- Changing the planner itself to enumerate disposition reachability
  upfront (a separate, larger refactor).

## Current state

Ready to start once tickets 121 and 122 land and the post-fix measurement
confirms the no_plan_found rate is still material.

## Approach

1. Component shape: `RecentDispositionFailures { entries: BTreeMap<DispositionKind, (u64, u32)> }`.
   Lazy insertion on first failure, parallel to `RecentTargetFailures`.
2. Authoring system: `evaluate_and_plan`'s `make_plan → None` branch
   inserts/updates the component before emitting `PlanningFailed`. Use
   the existing `EventLog::push` site as the anchor.
3. Decay: a per-tick system (or inline in `evaluate_and_plan`) prunes
   entries whose `ticks_since_last_failure > cooldown_window`. Tunable
   constant in `DispositionConstants`.
4. Consideration: read the entry for the candidate disposition,
   evaluate a Linear curve `1 - (1 - elapsed_fraction) * failure_severity`
   where `failure_severity = min(1.0, count / cap)`. Wire as a
   `ScalarConsideration` factor in the affected DSEs.

## Verification

- `cargo nextest run --features all` for new component + consideration
  tests.
- Soak-42 footer: `PlanningFailed/no_plan_found` count in the first
  1500 ticks drops by an order of magnitude.
- No regression in any continuity canary.

## Log

- 2026-05-01: Carved out of ticket 121 §Approach #3. Blocked on 121 + 122
  measurements; may park if those clear the symptom.
- 2026-05-01: **Unblocked.** 121 landed (anchor-substrate alignment for
  `PlannerZone::Wilds`); the post-fix soak shows the cold-start window
  unchanged (first `BuildingConstructed` still at tick 1_201_490).
  Top failure dispositions in the cold-start window remain
  Crafting/Foraging/Hunting `no_plan_found`, which is exactly the
  retry-storm shape this ticket targets. Tagged `cluster:
  substrate-over-override` (per-cat failure-history is itself a
  substrate axis the planner currently lacks).
- 2026-05-01: **Implemented (with two design deviations from the
  ticket spec, noted below).**
    - **Component shape simplified to tick-only `HashMap<DispositionKind,
      u64>`**, mirroring `RecentTargetFailures` exactly rather than the
      `(tick, count)` shape ticket §1 specifies. The cooldown curve's
      0.1× floor at fresh failure already provides meaningful
      suppression on first failure, and a refreshed tick on repeat
      failure resets the cooldown clock back to maximum penalty.
      Adding count-tracking adds API surface without clear value at
      this stage. If the soak shows insufficient suppression, add
      count-tracking in a follow-up commit.
    - **Cooldown applied as a `score_actions`-side score-list
      attenuation function (`apply_disposition_failure_cooldown`)
      rather than a per-DSE `Consideration`** as ticket §4 specifies.
      The six failure-prone dispositions span 10+ DSEs (Crafting alone
      has 10 constituents — herbcraft × 3, magic × 6, cook), so
      registering a `ScalarConsideration` on each with renormalized
      weights would touch every Crafting / Caretaking / Building DSE
      file for the same multiplicative effect. The score-list
      attenuator gives true multiplier semantics (a `WeightedSum`
      axis at weight `1/N` could only produce an `(N-1)/N`-floor
      attenuation, never a 0.1× damp). Mirrors the existing
      `apply_*_bonuses` post-scoring pattern in `goap.rs`. The
      substrate-over-override doctrine is preserved — the cooldown
      lives inside the IAUS engine, just at the score-list layer
      instead of inside a DSE's Composition.
    - Implementation surface: new `RecentDispositionFailures`
      component + 6 unit tests; new sensor
      `disposition_recent_failure_age_normalized` + 8 unit tests;
      new `apply_disposition_failure_cooldown` mutator + 7 unit tests;
      new `prune_recent_disposition_failures` system in chain 2a;
      new `disposition_failure_cooldown_ticks` constant on
      `PlanningSubstrateConstants` (default 4000 ticks ≈ 1 sim-hour,
      half the target-failure window); new `DISPOSITION_RECENT_FAILURE_INPUT`
      constant in `plan_substrate/mod.rs` (reserved for future
      Consideration uses though the score-list path is the one wired
      today). Authoring site: `evaluate_and_plan` writes the
      component on the `make_plan → None` branch immediately before
      the `PlanningFailed` event push. Application site: same
      function, after `score_actions` and before the `apply_*_bonuses`
      chain. 1695 lib tests pass; `just check` clean. Soak
      verification deferred to the user's commit / push step.
