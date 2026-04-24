---
id: 025
title: Hawk and snake GOAP planner domains
status: in-progress
cluster: null
added: 2026-04-24
parked: null
blocked-by: []
supersedes: []
related-systems: [wildlife]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Hawks and snakes use hardcoded state machines in `wildlife_ai` (Circling / Waiting). Foxes have a full GOAP system with Maslow-structured dispositions. Giving hawks and snakes their own GOAP domains creates species-distinct behavior rather than a shared state machine, and establishes the pattern for future wildlife.

## Current state

**Landed (Phase 1 — decision substrate):**
- `src/ai/hawk_planner/` — `HawkDomain` implementing `GoapDomain` with 4 zones (Sky, HuntingGround, Perch, MapEdge), 5 action types, 4 dispositions (Hunting, Soaring, Fleeing, Resting), action/goal/predicate/effect tables. 3 planner tests.
- `src/ai/snake_planner/` — `SnakeDomain` implementing `GoapDomain` with 4 zones (Cover, HuntingGround, BaskingSpot, MapEdge), 5 action types, 4 dispositions (Ambushing, Foraging, Basking, Fleeing). 3 planner tests.
- `src/ai/hawk_scoring.rs` — `HawkNeeds` (1-level Maslow), `HawkPersonality` (boldness, patience), `HawkScoringContext`, softmax disposition selection. 3 scoring tests.
- `src/ai/snake_scoring.rs` — `SnakeNeeds` (2-level Maslow: survival + thermoregulation), `SnakePersonality` (aggression, patience), `SnakeScoringContext`, softmax selection. 3 scoring tests.
- `src/ai/dses/hawk_*.rs` — 3 DSE factories (hunting, fleeing, resting) with 2 axes each. 17 tests.
- `src/ai/dses/snake_*.rs` — 4 DSE factories (ambushing, foraging, fleeing, basking) with 1-2 axes each. 13 tests.
- `src/ai/eval.rs` — `DseRegistry` extended with `hawk_dses` / `snake_dses` slots + lookup methods + `add_hawk_dse` / `add_snake_dse` extension methods.
- 53 new tests total, all passing.

**Remaining (Phase 2 — runtime wiring):**
- ECS components: `HawkState`, `SnakeState` (lifecycle, hunger), `HawkAiPhase`, `SnakeAiPhase` in `src/components/wildlife.rs`
- GOAP systems: `hawk_evaluate_and_plan`, `hawk_resolve_goap_plans` in `src/systems/hawk_goap.rs`; same for snake
- Step resolvers: `src/steps/hawk/mod.rs`, `src/steps/snake/mod.rs`
- Registration: add systems + DSEs to `SimulationPlugin::build` and both `build_schedule` paths
- Replace hardcoded hawk/snake AI in `wildlife_ai` with GOAP-driven behavior
- Attach `HawkState`/`SnakeState` at spawn in `spawn_initial_dens` / edge-spawner
- SimConstants: hawk/snake-specific tuning knobs
