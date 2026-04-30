---
id: 091
title: Post-087 seed-42 plan-execution collapse — Eat picks 62% but FoodEaten never witnesses; no Forage/Hunt; founder starvation cascade
status: ready
cluster: null
added: 2026-04-30
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Surfaced by the 087 verification soak (`logs/tuned-42` at commit `fc4e1ab8`, vs. baseline `logs/tuned-42-pre-087/` at `e838bb7`). Substrate-and-DSE-adoption landed test-green (`just check` + `cargo test --lib` 1640/1640 pass) but the canonical seed-42 deep-soak shows a colony-action collapse in the logged tail-window (ticks 1.2M → 1.21M, the only ticks `just soak` writes events for). Reads as either a balance issue from the Sleep/Flee axis additions or a DSE→GOAP plan-execution disconnect — *not* a strict sim regression.

## Observation

| Metric | Baseline (e838bb7) | Post-087 (fc4e1ab8) |
|---|---|---|
| Cats | 8 founders | 8 founders (same set: Birch, Calcifer, Ivy, Lark, Mallow, Mocha, Nettle, Simba) |
| Wildlife at tick 1.2M | mice 80 / rats 55 / rabbits 45 / fish 35 / birds 30 | identical |
| Final tick | 1,314,346 | 1,211,999 |
| `interrupts_by_reason` | `{CriticalHealth: 19382}` | `{}` |
| Deaths (in window) | 3 ShadowFoxAmbush | 8 Starvation |
| Death cluster timing | spread | evenly-spaced cascade ticks 1209999 → 1211999 (~285 ticks apart) |
| Stores (end of run) | `current 50 / capacity 50` (full) | `current 0 / capacity 50` (empty) |
| Plans/tick | ~2.92 | ~0.72 (4× fewer) |
| `PlanInterrupted` total | 19,382 | 0 |
| `PlanStepFailed` total | 3,373 | 1 |

**Action distribution (% of CatSnapshot rows):**

| Action | Baseline | Post-087 |
|---|---|---|
| Forage | 19.1% | **0%** |
| Coordinate | 16.6% | 5.0% |
| Eat | 15.8% | **62.4%** |
| Sleep | 15.3% | 1.1% |
| PracticeMagic | 10.4% | 7.7% |
| Hunt | 4.9% | **0%** |
| Groom | 3.7% | 0% |
| Patrol | 3.4% | 0% |
| Socialize | 2.3% | 3.2% |
| Wander | 2.0% | 0.3% |
| Cook | 2.0% | 0% |
| Fight | 1.8% | 0% |
| Build | 1.2% | 13.0% |
| Flee | 1.0% | 0% |
| Herbcraft | 0.5% | 7.2% |

**Smoking gun**: `Eat` is the most-frequent action (62%) yet `FoodEaten` is in `never_fired_expected_positives`. The action records but the resolver never witnesses completion. `eat_at_stores` is the only emission site (`src/systems/disposition.rs:3771` and `src/systems/goap.rs:3043`), so cats are picking Eat plans against an empty Stores and the resolver short-circuits without recording.

## Hypothesis space (user-flagged: balance vs DSE→plan)

1. **Plan-execution disconnect.** Cats elect `Eat` against empty Stores; `eat_at_stores` precondition fails silently and the plan re-fires next tick. With 0 PlanStepFailed and 0 PlanInterrupted, no telemetry surfaces the loop. *Test:* trace one starver and check whether the eat_at_stores resolver is reached vs short-circuiting at a precondition gate; check whether `food_available: false` should have prevented Eat from being elected at all.

2. **Producer-side suppression.** Forage and Hunt drop to 0% — the producer side of the food economy is silent. Why didn't they fire? Eat scoring should be high (food_scarcity ≈ 1.0 with empty stores) but Forage/Hunt should also be high under the same signal. *Test:* score-sample on a hungry cat with empty stores and a prey/forageable in range — does Forage/Hunt actually win, or does Eat dominate via the Maslow-tier-1-trumps logic?

3. **Sleep reweight side-effect.** Sleep dropped from 15% → 1%. Cats might be staying tired, low-energy, and their score landscape skews. *Test:* revert just the 0.10/0.90 reweight on Sleep (keep `pain_level` axis but at weight 0 or remove it); re-soak.

4. **Flee CP gating effect.** Flee added a 4th CP axis (`health_deficit`, Linear floor 0.6); CP geometric mean now over 4 axes. Flee dropped to 0%. The 4-axis CP suppresses Flee scores enough that other actions always win. *Test:* check the score-distribution on a healthy cat near a threat — does Flee still beat Patrol/Wander?

5. **Substrate side-channel.** Some unintended interaction in Chain 2a (the new `author_self_markers` registration position) shifts marker-snapshot timing for downstream consumers. *Test:* move `author_self_markers` to before `update_injury_marker` or after the whole batch; re-soak and compare.

User judgment: the regression reads as #1 / #2 (DSE→plan or balance), not substrate. Investigation owned by user.

## Reproduction

```
mv logs/tuned-42 logs/tuned-42-fc4e1ab8
just soak 42
just q run-summary logs/tuned-42
```

Both runs preserved:
- `logs/tuned-42-pre-087/` — baseline, e838bb7
- `logs/tuned-42/` — post-087, fc4e1ab8 (until next soak run; rename if you want to keep)

## Diagnostic queries to start with

```
just soak-trace 42 Nettle           # focal trace on first starver
just q cat-timeline logs/tuned-42 Nettle
just q anomalies logs/tuned-42      # 14 canary failures recorded
```

The focal trace will reveal which DSE Nettle elected at each tick before starvation, and the L1/L2/L3 score landscape that produced it.

## Out of scope

- Reverting 087's substrate or DSE adoption — keep both in place; the substrate is the architectural value, the regression is a balance-or-execution issue downstream.
- Opening a follow-on for the deferred Rest DSE (087's `Implementation deviation #1`) — separate scope, not on the critical path here.

## Log

- 2026-04-30: Opened. Surfaced by 087's seed-42 verification soak. User flagged framing as balance / DSE→GOAP plan-execution issue, not a strict sim regression. Investigation ownership: user.
