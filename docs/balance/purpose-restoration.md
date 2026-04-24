# Purpose restoration — iteration 1 (colony-positive action hooks)

**Status:** landed alongside `mastery-restoration.md` iteration 2,
`acceptance-restoration.md` iteration 2 (deferred), and
`respect-restoration.md` iteration 1.

## Context

The seed-42 v2 deep-soak (`logs/tuned-42-v2/`) showed colony-mean
purpose at **0.009 mean / 91.0% zero**. Static analysis confirmed the
mechanism: `src/systems/needs.rs:186` drains purpose every tick, and
the only non-aspiration restorers were `resolve_survey` (purpose gain
tied to discovery value) and `aspirations.rs:569,610` (`+0.03`/`+0.10`
on milestone events). Both restorers fire at cadences that don't
offset the `purpose_base_drain` across typical colony ecology, so
purpose sinks and stays pinned.

This is structurally the same shape as the iter-1 acceptance problem
(rare-event restorers → pinned to 0 → welfare cascade drag), except
for purpose there was **no prior iteration** on file.

## Hypothesis

> Purpose models the **felt sense of contributing to the colony** —
> the self-actualization axis at the top of the Maslow hierarchy.
> Every action a cat takes that advances colony welfare should
> produce a small purpose pulse: depositing food into stores, placing
> a ward, completing a build step, finishing a coordinator directive.
> These are already the actions that *are* colony-positive; wiring
> per-action purpose gains makes the need match the ecological
> behavior rather than sinking to 0 regardless of colony activity.

Distinct from `respect` (which is esteem-cluster and scales with
social visibility — see `respect-restoration.md`): purpose is about
the cat's *own* felt contribution, not whether anyone saw it. A cat
placing a ward alone at the edge of corrupted territory is making the
colony safer; that's purpose-positive regardless of witnesses.

## Prediction

| Metric | Direction | Rough magnitude |
|---|---|---|
| Colony-averaged purpose | ↑ from 0.009 | 0.3–0.5 band |
| Purpose `=0%` over snapshots | ↓ from 91.0% | < 30% |
| Welfare composite | ↑ slightly | self-actualization tier recovers |
| Survival canaries | unchanged | hard gates |

## What landed

New constants in `DispositionConstants`:
- `purpose_per_colony_action = 0.005` — generic baseline.
- `purpose_per_deposit = 0.02` — tangible asset to colony pool.
- `purpose_per_ward_set = 0.03` — significant defensive contribution.
- `purpose_per_directive_completed = 0.04` — explicit coordination.
- `purpose_per_build_tick = 0.0003` — high-cadence during construction.

Hooks applied at dispatch sites (no resolver signature widening —
`&mut Needs` is already in scope at `dispatch_step_action` in
`goap.rs` and `dispatch_chain_step` in `disposition.rs`):

| site | hook |
|---|---|
| `goap.rs` `SetWard` Advance (both branches) | `+purpose_per_ward_set` |
| `goap.rs` `CleanseCorruption` Advance | `+purpose_per_colony_action` |
| `goap.rs` `Construct` Advance | `+purpose_per_colony_action` |
| `goap.rs` `TendCrops` Advance | `+purpose_per_colony_action` |
| `goap.rs` `Cook` witnessed-Advance (real flip only) | `+purpose_per_colony_action` |
| `disposition.rs` `DepositAtStores` Advance + !rejected + !no_store | `+purpose_per_deposit` |
| `disposition.rs` `DeliverDirective` witnessed-Advance with target | `+purpose_per_directive_completed` |

Note: `purpose_per_build_tick` constant added but not yet applied —
the construct step is per-event rather than per-tick, so the
per-tick variant is a follow-on for `dispatch_step_action::Construct`
if iteration 2 needs higher cadence.

## Observation

Pending — to be filled in after the post-commit seed-42 deep-soak.

## Concordance

Pending. Document direction match + magnitude band per CLAUDE.md
balance methodology.

## Related work

- `docs/balance/acceptance-restoration.md` — sibling upper-tier need.
- `docs/balance/mastery-restoration.md` — sibling esteem need.
- `docs/balance/respect-restoration.md` — sibling esteem need
  (visible-accomplishment side).
- `docs/systems/colony_score.rs` — self-actualization-tier suppression
  cascade amplifying pinned-at-0 purpose into welfare drag.
