# Guarding exit recipe — §7.2 commitment gate, iter 1

## Context

On the 2026-04-24 seed-69 soak (`logs/tuned-SEED=69/events.jsonl`,
commit `eaba846` — random seed 18301685438630318625, mis-parsed from
`SEED=69`), the colony wiped after 900s with:

- Starvation = 8 (canary fail).
- 425 "urgency CriticalSafety (level 2) preempted level 4 plan"
  interrupts, with Thistle alone accounting for 335 (79%).
- Patrol the #1 action by snapshot count (540 > Eat's 522).
- Zero wards placed, zero shadowfox spawns, 5/6 continuity
  classes at zero.

Batch 2 on the same commit, seed 42, 900s: 0 deaths, 544 wards
placed, 0 interrupts, Patrol the *bottom* action. The failure is
seed-conditional but reveals a structural gap in the §7.2 gate.

### Causal chain

1. Safety falls below `critical_safety_threshold` (default 0.2).
2. `src/systems/goap.rs:~515` emits `UrgencyKind::CriticalSafety`
   at Maslow level 2.
3. The Patrol DSE (`src/ai/dses/patrol.rs`) scores highest within
   the Guarding chain: `safety_deficit` via `Logistic(6, 0.8)` ≈ 1.0
   at low safety.
4. Guarding disposition itself is Maslow level 2
   (`src/components/disposition.rs:~169`).
5. The preempt gate at `src/systems/goap.rs:~1982` compares
   `urgent.maslow_level < current_maslow` — `2 < 2` is false.
   **CriticalSafety cannot abandon a running Guarding plan.** This
   is semantically correct: Guarding *is* the response to low
   safety, and preempting it with CriticalSafety would be Maslow
   thrash.
6. Guarding's commitment strategy is `Blind`
   (`src/ai/commitment.rs::strategy_for_disposition`). Under Blind,
   only `achievement_believed` drops the plan.
7. Before this iteration, `achievement_believed` for Guarding was
   `plan.trips_done >= plan.target_trips` — i.e., "complete N
   patrols", with no safety-state signal.
8. Patrol's per-tile safety gain is small
   (`patrol_per_tile_safety_gain = 0.0005`); arrival adds `0.005`.
   A cat whose safety dropped to 0.15 might rise to ~0.22 over a
   multi-trip patrol — back into the non-critical band, but the
   plan kept running to its trip target before dropping. By then
   other needs had decayed; cat re-evaluated → Patrol scored highest
   again → Guarding re-selected. Loop.

The loop denied Thistle-pattern cats meaningful re-evaluation
windows. Eat plans never took DSE selection; hunger crossed the
starvation threshold; 8 cats died in the final 9% of the run.

## Hypothesis

> Adding a safety-recovered arm to Guarding's `achievement_believed`
> recipe (OR'd with the existing trip-target arm) lets the §7.2
> commitment gate drop Guarding cleanly once a cat has run at least
> one patrol trip AND safety has climbed past the exit band
> (`critical_safety_threshold + guarding_exit_epsilon`). This breaks
> the Patrol loop without touching the preempt-ordering gate or
> Patrol DSE scoring.

Grounding: Guarding's semantic purpose per the CLAUDE.md Maslow model
is "restore safety", not "complete N patrols". The trip target is a
mechanism for giving Guarding a discrete completion when no safety
signal is available; once safety does recover, the discrete trip count
is a worse success signal than the continuous safety level.

## Prediction

| Metric | Direction | Rough magnitude |
|---|---|---|
| `deaths_by_cause.Starvation` on seed-69 900s soak | ↓ | 8 → 0 (hard canary) |
| `interrupts_by_reason["urgency CriticalSafety (level 2) preempted level 4 plan"]` | ↓ | 425 → < 100 (breaking the loop lets level-4 plans complete) |
| Patrol share of top-level actions (per snapshot count) | ↓ | 540 → < 100 (Guarding drops sooner, cat re-evaluates more freely) |
| Guarding disposition share on Thistle-equivalents | ↓ | ~37% → < 10% of snapshots |
| Survival canaries (other seeds — 42) | unchanged | hard gates |

## What landed

- New constant in `DispositionConstants`:
  `guarding_exit_epsilon = 0.15` — band above
  `critical_safety_threshold` (0.2) that marks safety "recovered".
  Serde-default so a post-landing soak can tune without migrations.
  Exit band with defaults: 0.35.
- Extended `achievement_believed` arm for `DispositionKind::Guarding`
  in `src/ai/commitment.rs::proxies_for_plan`:
  ```rust
  let trips_complete = plan.trips_done >= plan.target_trips;
  let safety_recovered = plan.trips_done >= 1
      && needs.safety >= d.critical_safety_threshold + d.guarding_exit_epsilon;
  trips_complete || safety_recovered
  ```
  The `trips_done >= 1` trip guard mirrors the Resting recipe's
  lifted-condition protection: a cat entering Guarding with ambient
  safety already above the exit band must not read as achieved
  before patrolling. Without the guard the plan fires and drops on
  the same tick it was built.
- Five new unit tests in `src/ai/commitment.rs::tests`:
  - `proxies_guarding_achievement_requires_trip_guard`
  - `proxies_guarding_unachieved_when_safety_below_exit_band`
  - `proxies_guarding_achieved_when_safety_above_exit_band_after_trip`
  - `proxies_guarding_achieved_on_legacy_trips_target`
  - `gate_drops_blind_guarding_when_safety_recovers_mid_plan`
- Existing `gate_retains_blind_guarding_under_planner_hard_fail` test
  updated to explicitly set low safety so the new arm doesn't
  accidentally fire under the "still replanning" case it tests.

## Observation

Pending — to be filled in after the post-landing seed-69 and seed-42
deep-soaks.

## Concordance

Pending. Per CLAUDE.md balance methodology: direction match mandatory;
magnitude > 2× off requires second-order investigation before
acceptance; survival canaries are hard gates regardless.

## Fallback (iter 2 if needed)

If the post-fix soak holds starvation at 0 but still shows Patrol
dominance or Thistle-pattern cats spending > 50% of snapshots in
Guarding, the next iteration adds a **Patrol DSE safety-upper-bound
gate**: the `patrol_safety_threshold` consideration in
`src/ai/dses/patrol.rs` gets a second curve that zeros Patrol above
a `patrol_exit_threshold` (e.g., 0.5). That closes the DSE-scoring
half of the loop to complement the commitment-gate half landed here.
Held until verification shows the commitment arm alone is
insufficient — design parsimony favors one mechanism at a time.

## Related work

- `docs/systems/ai-substrate-refactor.md` §7.2 — the commitment
  gate and its proxy recipe contract.
- `CLAUDE.md` §"§7.2 commitment gate — mental model" — the
  lifted-condition anti-regression guidance that shaped the trip
  guard.
- `docs/open-work/landed/2026-04.md` — Thistle seed-69 diagnosis
  and landing entry.
- `docs/balance/respect-restoration.md` — sibling iter-1 relocation
  landed in the same session.
