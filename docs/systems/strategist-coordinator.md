# Strategist coordinator (stub)

Status: **design note, not implemented**. Referenced from
`src/systems/coordination.rs::accumulate_build_pressure` via
`TODO(strategist-coordinator)`.

## Problem

The coordinator currently treats every building channel (Stores,
Kitchen, Workshop, Garden, Hearth, Watchtower) as an independent
pressure accumulator. Two dials encode urgency: `*_pressure_multiplier`
for steady accumulation and `no_*_pressure_multiplier` for
"this-one-is-missing" phase-unlock rushes.

That shape works for the first few structures, but it conflates two
fundamentally different categories:

1. **Phase unlocks** — buildings that, until they exist, *gate an
   entire behavior loop*. Stores gates storage (cats can't deposit
   anywhere else). Kitchen gates the Cook loop (scoring requires a
   functional Kitchen, so cats can't even *want* to cook until one
   exists). Hearth gates the gathering / social warmth loop. These are
   categorical — their presence/absence is a switch, not a dial.
2. **Incremental expansions** — additional copies of an already-present
   structure (second Stores when the first is full, second Garden to
   scale food output, etc.). These are linear — pressure rises
   proportional to how strained the existing capacity is.

The current additive-multiplier approach collapses these into one
channel. Tuning for phase unlocks (need hard pushes) breaks incremental
balance (colony over-invests in expansion), and vice versa.

## What Civilization AI does

Civ 5/6 AI flags a subset of technologies as *beeline targets* — picks
that unlock transformative mechanics (writing → libraries, bronze
working → phalanxes, pottery → granaries). The AI plans a shortest path
through prerequisites to the target and weights research along that
path dramatically higher than normal opportunity-cost scoring. Once the
beeline lands, the AI returns to steady-state tech-value scoring.

Two-layer planner:

- **Strategic layer**: "what are my priorities right now, and what
  prerequisite steps unlock them?" Produces a short priority queue of
  named strategic goals (e.g. "enable food surplus", "enable defense
  buffer", "unlock magic research").
- **Tactical layer**: "given the current priority, what's the next
  concrete action?" — picks the next tech / build / unit consistent
  with the strategic goal.

## Observation: parallel vs sequential builds

(Added after the iter-7 soak.) The coordinator originally spawned every
passing pressure channel as its own blueprint site — Kitchen + Storehouse
+ Workshop started within two sim-days of each other. With ~8 cats and
Build scoring being one of many competing actions, three parallel sites
each received ~3-4 Build plans over 29 sim-days — enough to start none
of them but not enough to finish any.

The fix landed as a hard `has_unfinished_site` guard: don't spawn a
second blueprint site while the first is in progress. This is the
simplest "strategic" move — finishing one thing before starting the
next — and it belongs to the same mental model as the Civ beeline idea:
*pick a target, commit to it, then pivot*.

The refinement the strategist layer should eventually handle is
**surplus-labor parallelism**: when the colony has more idle cats than
one site can absorb (roughly, when adjacent-cat count on the active
site saturates its per-tick construction bonus), *then* allow a second
site to start. The current version is conservative by design — one at
a time — until we measure real labor headroom.

## What we'd want in Clowder

Mirror the two-layer split:

- **Strategic layer** — build a dependency DAG of structures keyed by
  the behavior loop they unlock:
  - Stores → storage loop (deposit-and-retrieve)
  - Hearth → gathering loop (warmth amenity, social clustering)
  - Kitchen (requires Hearth) → Cook loop (raw → cooked food)
  - Workshop → crafting loop (herbs → remedies / durable wards)
  - Garden → farming loop (crop rotation, seasonal yield)
  - Watchtower → patrol loop (perimeter vigilance beyond cat sight)
  
  The coordinator selects one or two active strategic goals per phase
  and beelines prerequisites. Selection is driven by which behavior
  loops are *unavailable*: cats want to cook but can't → activate the
  Cook-loop goal, which requires Hearth then Kitchen.

- **Tactical layer** — the existing `BuildPressure` accumulator, but
  now filtered by the active strategic goal. Non-goal-aligned channels
  decay faster; goal-aligned channels accumulate at a boosted rate.

Decoupling "which loop do I want to unlock next" from "which specific
structure do I queue next" is the key. The current system fuses them,
which is why Kitchen tuning has been a moving target.

## Why not yet

Shipping this before the single-seed Cook loop is verified end-to-end
would be premature. Two things need to work first:

1. Cook actually fires in a seed-42 soak (currently failing — no
   Kitchen gets built in 15 min).
2. We have at least one other behavior loop whose activation sequencing
   we're willing to rewrite alongside (e.g. magic/ward infrastructure).

Picking the DAG up front without seeing a second phase-unlock in play
risks over-fitting to the Cook case. The numerical `no_kitchen_pressure_multiplier`
fix is the stopgap that buys time to see the pattern properly.

## Provisional sketch (not binding)

```rust
// src/systems/coordination/strategist.rs  (future file)

enum BehaviorLoop {
    Storage,
    Gathering,
    Cooking,
    Crafting,
    Farming,
    Patrol,
}

struct LoopDependency {
    loop_kind: BehaviorLoop,
    prerequisites: Vec<StructureType>,  // in build order
    unlock_signal: fn(&World) -> bool,  // e.g. "has_functional_kitchen"
}

struct StrategicGoal {
    target_loop: BehaviorLoop,
    next_structure: StructureType,
    priority: f32,
}

fn select_strategic_goals(
    demand: &UnmetDemand,
    structures_present: &StructureSet,
    deps: &[LoopDependency],
) -> Vec<StrategicGoal> {
    // Find loops whose unlock_signal == false AND unmet_demand > threshold.
    // For each, walk the dependency chain to the first missing prereq.
    // Rank by demand magnitude.
}

fn tactical_filter(
    pressure: &mut BuildPressure,
    goals: &[StrategicGoal],
    cc: &CoordinationConstants,
) {
    // For channels matching an active goal's next_structure: boost accumulation.
    // For channels not matching: decay faster.
}
```

This is sketch, not contract — the real design should happen with a
second phase-unlock case in hand.
