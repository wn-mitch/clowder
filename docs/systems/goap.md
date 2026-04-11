# GOAP (Goal-Oriented Action Planning)

## Purpose
Replaces the hand-built `TaskChain` / chain-builder system with a declarative action planner. Utility scoring still selects *what* a cat wants to do (`DispositionKind`). GOAP plans *how* to achieve it — sequencing movement, resource handling, and execution steps automatically. Gives us replanning on failure (current system aborts) and a single place to define action preconditions/effects instead of per-disposition chain-builder functions.

## Architecture

### What Changes
| Layer | Before | After |
|-------|--------|-------|
| Goal selection | Utility scoring → Disposition component | Utility scoring → `GoapPlan` (Disposition component removed) |
| Plan generation | `build_hunting_chain()` etc. (12 hand-built functions) | GOAP planner generates plan from action definitions |
| Plan representation | `TaskChain` (linear step list) | `GoapPlan` (ordered action list with replanning) |
| Commitment tracking | `Disposition` component (sustained intent, completion counts) | `GoapPlan` absorbs commitment, completion, personality-scaled targets |
| Step execution | `resolve_disposition_chains` (1100-line monolith) | Single executor system with match dispatch to helper functions |
| Failure handling | `AbortChain` (discard plan, idle) | Replan from current state |

### What Stays
- Utility scoring (`scoring.rs`) — all bonus layers, softmax, personality modifiers
- `DispositionKind` enum — used for scoring aggregation, narrative labels, anxiety exemptions, respect gain (the `Disposition` *component* is removed; `DispositionKind` survives as a label on `GoapPlan`)
- Anxiety interrupts — strip `GoapPlan` on critical needs, force re-evaluation
- Coordinator directives — bias utility scores, not planning
- All game logic inside actions (scent tracking, combat, A\* pathfinding, narrative emission)
- Existing step resolver functions in `src/steps/disposition/`, `src/steps/building/`, `src/steps/magic/` — reused by the new executor
- `generate_narrative` system and `TemplateRegistry` — action-completion narrative is unchanged
- Mid-action event narrative (`emit_event_narrative` with event tags) — moves to GOAP executor, same templates

### No Traits / Generics
Actions are enum-variant data, not polymorphic trait objects. The planner uses `GoapActionDef` structs (precondition/effect tables for A\* search). The executor uses `GoapActionKind` enum dispatch to call step resolver helpers. Rationale:
- The planner is pure A\* over hashable state — it evaluates predicates, not dispatches behavior. Struct fields, not trait methods.
- Each action's execution needs different ECS state (hunt needs prey queries, forage needs terrain, social needs relationships). A unified trait method would require threading the entire world through a single parameter type.
- Bevy's pattern is enum dispatch (`Action`, `StepKind`, `DispositionKind`). The one trait in the codebase (`PreyProfile`) flattens to a component at spawn time — no runtime dispatch.

## Planning State

Compact, hashable state for A\* search. Constructed from ECS queries on demand — not stored persistently.

| Field | Type | Description |
|-------|------|-------------|
| `zone` | enum | Abstract location: Stores, HuntingGround, ForagingGround, Farm, ConstructionSite, HerbPatch, RestingSpot, SocialTarget, Wilds |
| `carrying` | enum | Nothing, Prey, ForagedFood, BuildMaterials, Herbs, Remedy |
| `trips_done` | u32 | Deposit/interaction cycles completed this disposition |
| `hunger_ok` | bool | Hunger above comfort threshold |
| `energy_ok` | bool | Energy above comfort threshold |
| `warmth_ok` | bool | Warmth above comfort threshold |
| `interaction_done` | bool | Social/mentor/mate interaction completed |
| `construction_done` | bool | Build contribution completed |
| `prey_found` | bool | Prey detected by SearchPrey, consumed by EngagePrey |
| `farm_tended` | bool | Farm tend/harvest cycle completed |

Zone is resolved to a concrete position at execution time, not planning time. The planner thinks "go to Stores"; the executor finds the nearest store via spatial query.

## GOAP Actions

Each action has preconditions (state predicates), effects (state mutations), and a cost.

### Movement Actions
| Action | Precondition | Effect | Cost |
|--------|-------------|--------|------|
| `TravelTo(zone)` | zone != target | zone = target | distance-weighted |

One parameterized action template, instantiated per reachable zone. Cost derived from estimated tile distance so the planner prefers nearby targets.

### Hunting
| Action | Precondition | Effect | Cost |
|--------|-------------|--------|------|
| `SearchPrey` | zone = HuntingGround, carrying = Nothing | prey_found = true | 3 |
| `EngagePrey` | prey_found = true | carrying = Prey, prey_found = false | 2 |
| `DepositPrey` | zone = Stores, carrying = Prey | carrying = Nothing, trips_done += 1 | 1 |

Goal: `trips_done >= target` (personality-scaled, stored in `GoapPlan`).

Planner output for one trip: `[TravelTo(HuntingGround), SearchPrey, EngagePrey, TravelTo(Stores), DepositPrey]`. On trip completion, executor checks `trips_done < target` and re-invokes planner from current state for the next trip (see Per-Trip Planning).

`SearchPrey` encapsulates scent detection, visual detection, and belief-driven movement. It's a GOAP action because failed search is a meaningful replanning trigger — the planner can switch to foraging if the hunting ground is empty.

`EngagePrey` encapsulates Stalk → Chase → Pounce as internal phases in `resolve_engage_prey()`. These phases are reactive to prey position and awareness — the planner can't model them. If the pounce fails, the executor triggers replanning (which may produce another SearchPrey).

### Foraging
| Action | Precondition | Effect | Cost |
|--------|-------------|--------|------|
| `ForageItem` | zone = ForagingGround, carrying = Nothing | carrying = ForagedFood | 3 |
| `DepositFood` | zone = Stores, carrying = ForagedFood | carrying = Nothing, trips_done += 1 | 1 |

Goal: `trips_done >= target`.

### Resting
| Action | Precondition | Effect | Cost |
|--------|-------------|--------|------|
| `EatAtStores` | zone = Stores | hunger_ok = true | 2 |
| `Sleep` | zone = RestingSpot | energy_ok = true | 2 |
| `SelfGroom` | — | warmth_ok = true | 1 |

Goal: `hunger_ok && energy_ok && warmth_ok`. Planner sequences eat/sleep/groom in whatever order is cheapest from current state.

### Guarding
| Action | Precondition | Effect | Cost |
|--------|-------------|--------|------|
| `PatrolArea` | zone = PatrolZone | trips_done += 1 | 2 |
| `EngageThreat` | zone = PatrolZone, threat_nearby = true | trips_done += 1 | 3 |
| `Survey` | zone = PatrolZone | trips_done += 1 | 1 |

Goal: `trips_done >= target`.

### Socializing
| Action | Precondition | Effect | Cost |
|--------|-------------|--------|------|
| `SocializeWith` | zone = SocialTarget | interaction_done = true, trips_done += 1 | 2 |
| `GroomOther` | zone = SocialTarget | interaction_done = true, trips_done += 1 | 2 |
| `MentorCat` | zone = SocialTarget | interaction_done = true, trips_done += 1 | 3 |

Goal: `trips_done >= target`.

### Building
| Action | Precondition | Effect | Cost |
|--------|-------------|--------|------|
| `GatherMaterials` | zone = Wilds, carrying = Nothing | carrying = BuildMaterials | 2 |
| `DeliverMaterials` | zone = ConstructionSite, carrying = BuildMaterials | carrying = Nothing | 1 |
| `Construct` | zone = ConstructionSite | construction_done = true | 3 |

Goal: `construction_done = true`.

### Farming
| Action | Precondition | Effect | Cost |
|--------|-------------|--------|------|
| `TendCrops` | zone = Farm | farm_tended = true | 2 |
| `HarvestCrops` | zone = Farm, farm_tended = true | trips_done += 1 | 2 |

Goal: `trips_done >= 1`.

### Crafting (Herbcraft / Magic)
Sub-mode selected by CraftingHint from scoring. Each sub-mode has its own action set. Pattern follows the same precondition/effect model — details deferred to implementation since these are the most complex and least stable dispositions.

### Mating / Caretaking / Coordinating / Exploring
Same pattern: TravelTo(target) + domain-specific action. Goal is interaction_done or trips_done.

## Action Granularity

An action belongs at the GOAP planner level when the planner can meaningfully choose to do it OR something else. It belongs as an executor helper when it's a reactive phase that always happens as part of a larger arc.

| Concern | GOAP Action (planner-visible) | Executor Helper (planner-invisible) |
|---------|------------------------------|-------------------------------------|
| State | Changes planner state (zone, carrying, trips_done, prey_found) | Operates on runtime game state (prey position, awareness, terrain rolls) |
| Choice | Planner could choose an alternative | Always happens as part of a larger action |
| Preconditions | Expressible in abstract `PlannerState` | Depend on dynamic entity positions, AI state |
| Examples | TravelTo, SearchPrey, EngagePrey, DepositPrey, Sleep, EatAtStores | Stalk/Chase/Pounce (inside EngagePrey), scent detection (inside SearchPrey), tile-by-tile movement (inside TravelTo), den detection |

`EngagePrey` is a GOAP action whose internal phases (Stalk → Chase → Pounce) are executor helpers in `resolve_engage_prey()`. The planner knows EngagePrey turns `prey_found` into `carrying = Prey`. The executor handles the procedural hunting loop.

`ForageItem` stays as a single GOAP action — foraging is a directional patrol with per-tile yield rolls, no distinct search/collect phases worth splitting until foraging depletes environmental resources.

## Planner Algorithm

A\* search over `PlannerState` nodes. Edges are GOAP actions. Heuristic: count of unsatisfied goal predicates (admissible, fast).

```
fn make_plan(start: PlannerState, actions: &[GoapAction], goal: &GoalState) -> Option<Vec<PlannedStep>>
```

Pure function — no Bevy dependency. Testable with unit tests against known scenarios.

Search is bounded: max depth = 12 steps, max nodes expanded = 1000. Plans are short (typically 3-6 actions for one trip). If no plan found, cat re-enters utility evaluation next tick.

## Replanning

### On Failure
1. Snapshot current `PlannerState` from ECS (zone, carrying, trips_done, etc.)
2. Re-invoke planner with same goal (from `GoapPlan.kind`)
3. If plan found: replace steps in `GoapPlan`, continue
4. If no plan found: remove `GoapPlan`, cat re-evaluates next tick

Failure triggers: target despawned, path blocked after N retries, prey search timed out (SearchPrey), pounce failed (EngagePrey), resource exhausted (no harvestable tiles), build site completed by another cat.

### On Trip Completion (Per-Trip Planning)
When a plan completes successfully, the executor checks `trips_done < target_trips`:
1. If more trips needed: snapshot `PlannerState`, re-invoke planner for the next trip, replace steps in `GoapPlan`
2. If target met: plan is fully complete, remove `GoapPlan`, emit `PlanNarrative::Completed`

This means plans are always short (3-6 actions) — the planner plans one trip at a time. Each trip replans from fresh state, so the cat adapts to changes: if stores filled after trip 1, trip 2 might go to a different stores. If hunger dropped, the planner might insert `EatAtStores` before the next hunt.

The planner's goal for multi-trip dispositions is `trips_done >= trips_done + 1` (one trip increment), not `trips_done >= target` (all trips). The executor handles the trip loop.

## Integration with ECS

### Components
| Component | Role |
|-----------|------|
| `GoapPlan` | Replaces both `TaskChain` and `Disposition` — ordered list of `PlannedStep` with current step index, `DispositionKind` label, commitment persistence (adopted tick, replan count), personality-scaled target trips, `CraftingHint` |
| `CurrentAction` | Unchanged — tracks what the cat is doing this tick |
| `PlanNarrative` (message) | Emitted by executor on plan lifecycle events: `Adopted`, `Completed`, `Replanned`, `Abandoned`. Carries `entity`, `DispositionKind`, `PlanEvent`, and `completions` count. |

`PlannerState` is NOT a component — computed on demand from queries. `Disposition` component is removed; all its fields (`kind`, `target_trips`, `adopted_tick`, `crafting_hint`) move into `GoapPlan`.

### Systems (schedule order)
1. `check_anxiety_interrupts` — strips `GoapPlan` on critical needs, forces re-evaluation
2. `evaluate_and_plan` — **merged system** replacing both `evaluate_dispositions` and `disposition_to_chain`. Scores utilities → selects `DispositionKind` via softmax → invokes GOAP planner → inserts `GoapPlan`. Cats without a `GoapPlan` get scored; cats with one keep it (commitment). Emits `PlanNarrative::Adopted` on new plan creation.
3. `resolve_goap_plans` — **single executor system** replacing `resolve_disposition_chains`. Ticks the current step; on completion, advances to next step. On failure, snapshots `PlannerState` and replans. Dispatches to existing step resolver helper functions via match on `GoapActionKind` (not per-action Bevy systems — avoids borrow checker conflicts with overlapping mutable queries). Emits `PlanNarrative::Completed`, `Replanned`, or `Abandoned` at plan lifecycle transitions. Mid-action event narrative (`emit_event_narrative` with tags like `"catch"`, `"miss"`, `"scent"`) is emitted inline during step execution, same as today.
4. `emit_plan_narrative` — reads `PlanNarrative` messages, builds `TemplateContext` with the disposition's primary action + plan event tag, calls `emit_event_narrative`. Runs after `resolve_goap_plans`.

### Module Layout
```
src/ai/
    planner/
        mod.rs       — PlannerState, GoapAction, Plan, make_plan()
        actions.rs   — action definitions (preconditions, effects, costs)
        goals.rs     — goal state builders per DispositionKind
        state.rs     — PlannerState construction from ECS queries
    scoring.rs       — unchanged
    pathfinding.rs   — unchanged
```

## Implementation Sequence

1. Build planner core (`src/ai/planner/`). Pure Rust, no Bevy. Unit-tested against hand-constructed scenarios.
2. Define actions and goals for all 12 dispositions in declarative tables.
3. Add `GoapPlan` component (`src/components/goap_plan.rs`) absorbing fields from `Disposition` + `TaskChain`.
4. Build merged `evaluate_and_plan` system — scores utilities → selects goal → invokes planner → inserts `GoapPlan`.
5. Build `resolve_goap_plans` — single executor dispatching to existing step resolver helpers via match.
6. Wire replanning on failure.
7. Add `PlanNarrative` message, `PlanEvent` enum, and `emit_plan_narrative` system. Add `.ron` templates for plan-level events.
8. Remove `Disposition` component, 12 chain builders, `TaskChain`, old schedule systems. Update UI queries.
9. `just score-track` + `just score-diff` to verify no behavioral regression.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Max plan depth | 12 | Per-trip plans are 3-6 steps; 12 gives headroom for resting (eat + sleep + groom) |
| Max nodes expanded | 1000 | Plans are small state spaces; guards against pathological cases |
| Replan retry limit | 3 per disposition | Prevents infinite replan loops; drops disposition after 3 failures |
| TravelTo base cost | 1 | Scaled by estimated distance; keeps movement cheap but distance-aware |

## Narrative Integration

Three narrative emission paths under GOAP. The first two are unchanged from the current system; the third is new.

### Path 1: Action-Completion Narrative (unchanged)

`generate_narrative` fires when `ticks_remaining == 1` on `CurrentAction`. Selects templates from `TemplateRegistry` keyed on the `Action` enum + personality/mood/weather/terrain context. `CurrentAction` still exists — the GOAP executor sets it when starting each action. No changes needed.

### Path 2: Mid-Action Event Narrative (moved, not changed)

`emit_event_narrative` calls with event tags move from `resolve_disposition_chains` to `resolve_goap_plans`. Same `TemplateContext` + `VariableContext`, same `.ron` template files. The executor dispatches to existing step resolver helpers which emit these at the same points as today.

Existing event tags preserved:
| Tag | Source Action | Trigger |
|-----|-------------|---------|
| `"catch"` | EngagePrey | Prey killed |
| `"miss"` | EngagePrey | Pounce failed |
| `"scent"` | SearchPrey | Scent trail detected |
| `"raid"` | EngagePrey | Den raided |
| `"find"` | ForageItem | Forageable found |

### Path 3: Plan-Level Narrative (new)

Structural events emitted as `PlanNarrative` messages at plan lifecycle transitions. Handled by `emit_plan_narrative` system using the existing template engine with new event tags.

| Plan Event | Event Tag | When | Example |
|------------|-----------|------|---------|
| `Adopted` | `"plan_adopted"` | New plan created by `evaluate_and_plan` | "Bramble heads out to hunt." |
| `Completed` | `"plan_complete"` | All steps succeed, disposition target met | "Bramble returns from a productive hunt." |
| `Replanned` | `"plan_replanned"` | Action fails, planner re-sequences | "Bramble adjusts course after losing the scent." |
| `Abandoned` | `"plan_abandoned"` | Replan fails or retry limit hit | "Bramble gives up the hunt." |

```
#[derive(Message)]
pub struct PlanNarrative {
    pub entity: Entity,
    pub kind: DispositionKind,
    pub event: PlanEvent,
    pub completions: u32,
}

pub enum PlanEvent {
    Adopted,
    Completed,
    Replanned,
    Abandoned,
}
```

`emit_plan_narrative` maps `kind` → primary `Action` (via `DispositionKind::constituent_actions()[0]`), builds a `TemplateContext` with the plan event tag, and calls `emit_event_narrative`. New `.ron` template files (e.g. `assets/narrative/plan_events.ron`) provide disposition-specific text, with personality/weather/season conditioning available via the existing template matching system.

### Narrative Dedup

`ActionHistory.last_narrated_disposition` currently suppresses repeated "heads out to hunt" messages. Under GOAP, `PlanNarrative::Adopted` replaces this — the executor sends the message once per plan creation. `emit_plan_narrative` handles dedup by only narrating `Adopted` when the disposition kind differs from the cat's previous plan (tracked on `GoapPlan.kind`).

## Tuning Notes
_Record observations and adjustments here during iteration._
