---
id: 094
title: Eat-vs-Forage IAUS imbalance — colony hauls food but doesn't consume it
status: ready
cluster: substrate-over-override
added: 2026-04-30
parked: null
blocked-by: [091]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

091 fixed the founder starvation cascade (8 simultaneous → 1 isolated death) by removing `enforce_survival_floor` and the Forage/Hunt Carrying-veto. Producer chains now fire: 57,954 Foraging plans / 30,757 Hunting plans / 88,901 TravelTo(Stores) / 57,954 DepositFood across the seed-42 deep-soak. Cats deposit food into Stores constantly.

**They almost never eat from Stores.** Across 1.2M ticks × 8 cats, only **200** plans contained `EatAtStores`. The footer's `never_fired_expected_positives` still lists `FoodEaten`. The hunger trajectory is the smoking gun:

| Cat | hunger at t=1.2M | hunger at run end | min hunger |
|---|---|---|---|
| Birch | 0.90 | 0.76 | 0.49 |
| Calcifer | 0.99 | 0.70 | 0.46 |
| Ivy | 0.85 | 0.58 | 0.48 |
| Lark | 0.82 | **0.20** | 0.20 |
| Mallow | 0.88 | 0.67 | 0.40 |
| Mocha | 0.93 | **0.38** | 0.38 |
| Nettle | 0.79 | **0.00 (dead)** | 0.00 |
| Simba | 0.96 | 0.64 | 0.46 |

(Hunger semantics: `needs.hunger += food_value` in `eat_at_stores.rs:85` — high = fed, low = hungry.) Every cat is dropping. Lark went 0.82 → 0.20 over the soak window. Nettle starved at tick 1,304,677 after losing the most ground.

A sample of Lark's `last_scores` at hunger 0.20:
```
[("Hunt", 0.85), ("Forage", 0.61), ("Groom", 0.55), ...]
```

**Eat doesn't appear in her top scores.** A cat at hunger 0.20 — well below where Eat-DSE's hangry curve should saturate near 1.0 — is electing Hunt instead. The `last_scores` are also frozen across snapshots (same exact values across many CatSnapshots), confirming Lark is mid-plan and never re-electing.

## Hypothesis space

Two natural-lever gaps. Both are substrate-over-override-shaped (`093` thread): the right fix is an IAUS adjustment, not a clamp / interrupt / planner gate.

### H1 — Eat doesn't dominate Forage at the stores

When a cat is at `Zone::Stores` with `HasStoredFood=true` and hungry, Eat should win the IAUS contest. It doesn't.

Suspects:
- **Eat DSE composition strength.** Eat's `Logistic(8, 0.5)` hangry curve produces ~0.5 at urgency=0.5; Forage's hunger axis may produce a stronger signal at the same urgency.
- **Stores-proximity multiplier missing.** Eat has a stores-distance spatial axis (`considerations.rs:73`), but its weight may be too low to flip the contest when cat is *at* stores. Could be a `Logistic(slope=high, midpoint=low)` so it spikes near zero distance.
- **No satiation-pressure modifier.** Forage's hunger axis stays high at 0.20 hunger because the cat is still hungry — but ecologically once the cat is at the food, Forage's "go acquire more" axis should decay. A `colony_food_well_stocked` modifier on Forage could fix this.

### H2 — Long Forage plans don't preempt for Eat

A foraging cat at hunger 0.20 should *break* the trip and Eat now. Lark's frozen `last_scores` show she goes thousands of ticks without re-electing. Investigate:
- Commitment-strength tuning for Foraging — should the score margin to break be lower for severe hunger?
- Could 087's `body_distress_composite` modifier on the Eat axis be amplified to cross the preemption threshold mid-plan?
- Or: the Resting plan resolver could insert an Eat detour when planning enters Stores zone.

H1 is the higher-leverage fix; H2 may resolve as a byproduct.

## Reproduction

```
# Logs preserved in 091 investigation:
ls logs/tuned-42                            # post-091 final soak (food-loop broken)
ls logs/tuned-42-091-h1h4-telemetry-only    # post-H1, hack still in
ls logs/tuned-42-pre-087                    # baseline (works)
```

Confirmation:
```
just q events logs/tuned-42 --type=PlanCreated | jq -c 'select(.steps | contains(["EatAtStores"]))' | wc -l
# 200 — the smoking gun

jq -c 'select(.type == "CatSnapshot")' logs/tuned-42/events.jsonl | jq -c '{cat, h: .needs.hunger, top3: .last_scores[0:3]}' | tail -8
# Eat absent from top3 even at hunger 0.20
```

## Diagnostic queries

```
just soak-trace 42 Lark                     # focal trace for the most-affected survivor
just q trace logs/tuned-42 Lark --layer=L2  # Eat DSE consideration scores
just q trace logs/tuned-42 Lark --layer=L3  # ranked + chosen
```

The L2 trace will show whether Eat's score is genuinely below Forage at the stores, and which consideration axis is producing the gap. The L3 trace will show how many ticks elapse between the cat entering `Zone::Stores` and the next disposition election.

## Out of scope

- 091 follow-ups not in this ticket: Crafting Carrying-veto, residual Nettle late-death, continuity regressions (mating/courtship/burial). Open separately as needed.
- Structural marker/StatePredicate unification (092) — orthogonal; lands independently.

## Log

- 2026-04-30: Opened. Surfaced by user observation while reviewing 091's verification soak ("why is FoodEaten not firing if cats clearly don't starve?"). Cascade is fixed; food *loop* is broken in a different way. Next-priority follow-up to 091 on the substrate-over-override thread.
