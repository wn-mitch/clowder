---
id: 201
title: CriticalHealth interrupt drives "hunt-to-starvation" plan churn — concrete reproducer for ticket 119
status: dropped
cluster: ai-substrate
added: 2026-05-06
parked: null
blocked-by: [119]
supersedes: []
related-systems: [ai-substrate-refactor.md, needs.md]
related-balance: [047-acute-health-adrenaline.md]
landed-at: b861150c
landed-on: 2026-05-06
---

## Why

Surfaced by the post-ticket-193 canonical seed-42 soak (`logs/tuned-42`,
commit `6273c669`). The 193 fix closed the
`PickingUp:GoalUnreachable` cascade (3302 → 0); the colony now survives
2.3× longer (24,145 → 55,671 ticks) and nourishment recovers from
0.22 to 0.92 (back above the 0.82 healthy baseline). But two adult
founders — **Cedar** (tick 1,240,147) and **Heron** (tick 1,246,625)
— starve under a different mechanism that was previously masked by
193's earlier collapse.

The pathology is the **CriticalHealth interrupt treadmill** that
ticket 119 already documents structurally. The post-193 run
provides the first clean production reproducer because the colony
finally lives long enough for slow-onset starvation to surface
behind the (now closed) plan-creation cascade.

**Diagnostic data (Cedar):**

| Tick | hunger | health | current_action |
|---:|---:|---:|---|
| 1,230,000 | 0.995 | 0.277 | Forage |
| 1,239,000 | 0.095 | 0.097 | **Hunt** |
| 1,239,100 | 0.085 | 0.097 | **Hunt** |
| 1,239,200 | 0.075 | 0.097 | **Hunt** |
| 1,240,147 | 0.000 | — | **Death (Starvation)** |

Health pinned at ≈0.097 (right at `CriticalHealth` threshold) for
the entire ~10kt decay window. Action distribution in the final
6kt: 52% Hunt, 35% Forage, 8% PickUp, 4% Wander, 2% Sleep —
**0% Eat**. Cat created **9,760 plans / 10,000 ticks (1.04 ticks
per plan)**, of which 8,711 were `PlanInterrupted`. Heron shows
the identical pattern at a different tick offset.

**Colony-wide signal:** `interrupts_by_reason.CriticalHealth =
29,089` (29kt of cumulative interrupts across an 8-cat colony in
55kt of run time). Sole non-trivial interrupt reason. This is the
treadmill ticket 119 promises to retire.

The mechanism: cat below CriticalHealth threshold → interrupt
fires every tick the cat tries to commit to a recovery action
→ replan churns at ~1 tick/plan → no plan reaches the consume
step → hunger drains linearly → starvation. `HasStoredFood = false`
(stores empty during the famine window) gates `EatAtStores`
out, so the only food path is Hunt → DepositPrey → Cook → Eat,
which the treadmill prevents from ever completing.

## Why this is 201 (not just a 119 log line)

119's narrative is the structural argument; this ticket is the
**verification target**. Once 119 lands, this scenario MUST stop
producing the 1.04-ticks/plan churn signature — that's the
concrete pass criterion for 119's retirement landing cleanly.
Logging it as a separate ticket (instead of folding into 119's
`## Log`) keeps the verification artifact stable across 119's
implementation iterations: when 119 lands, this one closes
trivially with a pointer to the post-119 soak's footer drop in
`interrupts_by_reason.CriticalHealth`.

## Pass criteria when 119 lands

Re-run `just soak 42` post-119 and verify in
`logs/tuned-42/events.jsonl` footer:

- `interrupts_by_reason.CriticalHealth = 0` (true zero per 119's
  own gate).
- Cedar / Heron equivalent founders complete a full Hunt → Cook
  → Eat cycle without 9000+ plan-create cadence in any 10kt
  window. (Use `just q cat-timeline logs/tuned-42 <name> --tick-range=...
  --summarize` — auto-flags plan-churn under 5 ticks/plan.)
- `deaths_by_cause.Starvation` ≤ 1 in the canonical 15-min soak
  (matches the pre-193-collapse `pre-184` baseline; 0 is the
  hard target but seed-42 has known endemic starvation noise).

## Out of scope

- The structural fix itself (lives in 119; 119 in turn blocks on
  118's modifier-lift-vs-plan-completion-momentum gap).
- The mating-loop / kittens-born=0 issue (separate; 188 / 192
  follow-ons).
- The post-193 `Guarding:GoalUnreachable=810` spike (separate
  investigation — wards/patrol substrate, possibly a new ticket).
- Tuning the CriticalHealth threshold itself (parameter-level
  workaround — 119's structural retirement supersedes it).

## Log

- 2026-05-18: Recovered from git history during Linear migration archaeology — added in two empty-message draft commits (b861150c, a03823d6) that never reached main; treating as abandoned draft. Restored to preserve Linear ID alignment.
- 2026-05-06: opened. Post-193 soak surfaced the 119 pattern as
  a concrete production reproducer (Cedar & Heron starvation
  with 1.04 ticks/plan churn, hunger 0.99 → 0 over 10kt with
  0% Eat actions, action distribution dominated by Hunt). 193's
  PickingUp cascade had previously masked this by killing cats
  via injury before slow-onset starvation could surface. Blocked
  on 119 (which itself blocks on 118).
