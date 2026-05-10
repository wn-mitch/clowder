---
id: 261
title: ActionAffordances substrate — per-action success scalars + ActionKind enum + heuristic estimators
status: blocked
cluster: C
added: 2026-05-10
parked: null
blocked-by: [258]
supersedes: [141]
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

C3's belief substrate (ticket 258) tells a cat *what the target is like*. It does not tell the cat *whether my action against this target will succeed*. The L3 patrol-absorption cascade (memory `project_l3_patrol_absorption_cascade`) shows why this matters: substrate axes that elevate Patrol can't price predator-exposure-likelihood-of-success, so Patrol wins L3 bandwidth and exposes cats to ShadowFoxes. Adding a per-action success affordance — `(perceiver, target, action_kind) → success_scalar` — gives every target-taking DSE a price for "would this action even work" without each DSE re-deriving it from primitives.

Generalizes ticket 103 (escape_viability — done) and supersedes ticket 141 (combat_winnability — ready) once consumers migrate. Both were single-action perception scalars; this ticket lifts them into a uniform affordance substrate so ~22 action kinds across 5 groups are priced through one read API.

Sibling to ticket 258 (C3 spinout). 258 is BeliefsAboutTarget; this is ActionAffordances. Both are L1-shaped substrate; cross-cutting consumers in sibling tickets read both.

## Scope

- **`ActionAffordances` resource** wrapping `HashMap<(Entity, Entity, ActionKind), f32>`. Per `(perceiver, target, action_kind)` per tick within sensing range, computes a success scalar in `[0, 1]`.
- **`ActionKind` enum** with ~22 variants across 5 groups (entity-target only; zone/location affordances stay in existing influence maps):

  **Predation** (per-species subset): `Stalk`, `Chase` (universal predators); `Pounce` (Cat); `Dive` (Hawk); `Strike` (Snake); `Ambush` (ShadowFox).
  **Threat-response** (universal): `Flee`, `Fight`, `Freeze`, `Fawn`.
  **Conflict-low** (no DSE today; substrate populates for future consumers): `Threaten`, `Posture`, `Hiss`.
  **Social** (cat-cat mostly): `Socialize`, `GroomOther`, `Mate`, `Mentor`, `Care`, `FeedKitten`.
  **Prey-side** (no AI today; substrate populates for future consumers): `Bolt`, `ScatterGroup`.

- **`affordance_writer` system** computes affordance scalars per tick. Per-action heuristic estimators:

| ActionKind | Heuristic input |
|---|---|
| `Stalk` | cover availability + my stealth + target alertness (`perceived_intent_clarity`) |
| `Chase` | my speed vs target speed + RouteCostField + cover-density penalty |
| `Pounce` | distance + cover + `perceived_intent_clarity` |
| `Dive` | aerial cover absence + target speed + my approach angle |
| `Strike` | striking range + target speed + my coil-readiness |
| `Ambush` | concealment quality at my position + target situational awareness |
| `Flee` | RouteCostField + cover distance + my_speed vs target_speed |
| `Fight` | my HP/combat profile + `perceived_violence_capability` |
| `Freeze` | cover availability + `perceived_intent_clarity` |
| `Fawn` | proximity + `perceived_hostility` + recent groom history |
| `Threaten` | proximity + my size/condition + `perceived_violence_capability` ratio |
| `Posture` | proximity + my condition + audience presence |
| `Hiss` | proximity + my distress level + audience presence |
| `Socialize` | proximity + `affiliation_history` + `perceived_hostility` |
| `GroomOther` | proximity + bond + `perceived_hostility` |
| `Mate` | fertility + bond + `affiliation_history` + perceived receptivity |
| `Mentor` | apprentice age + bond + my knowledge + perceived receptivity |
| `Care` | `perceived_injury_level` + bond |
| `FeedKitten` | kitten hunger + my food + bond |
| `Bolt` | RouteCostField + cover distance + my_speed vs target_speed |
| `ScatterGroup` | herd density + cover distribution + my position in herd |

- **Per-action tunables in `SimConstants`**: each estimator's input weights, ceiling/floor, and a per-action `min_eligibility_threshold` below which the affordance is gated to zero.
- **Read API**: per-DSE consideration shape (per `src/ai/dses/socialize_target.rs` precedent).

## Out of scope

- **DSE consumer wiring** — sibling tickets per cluster (256-cluster, social, wildlife, Freeze, Fawn, EngageThreat, prey-side, conflict-low). This ticket lands the substrate; consumers wire it.
- **Belief facets** — sibling ticket 258. ActionAffordances *consumes* facet reads from MentalModels but doesn't author them.
- **Plan-aware (GOAP-derived) affordance estimates** — explicitly rejected (see Considered and rejected below).
- **Zone/location affordances** — Patrol-zone, Cook-station, Tend-building, GoTo-location stay in existing influence maps (RouteCostField, FoxScentMap, WardCoverageMap). NOT folded into this resource.

### Considered and rejected

- **Plan-aware (GOAP-derived) action-success estimates** — re-running GOAP per `(perceiver, target, action_kind)` per tick is prohibitive AND defeats the substrate purpose (substrate is supposed to give DSEs cheap reads, not gate them on planning). Cheap heuristics + influence-map reuse is the explicit alternative.
- **Folding zone/location affordances into this substrate** — existing influence maps already serve; conflating costs more than it earns.

## Current state

- Blocked-by 258 (C3 spinout). Belief facets must exist for affordance heuristics to read `perceived_violence_capability`, `perceived_intent_clarity`, etc.
- Existing influence maps usable for affordance computation: `RouteCostField` (per-cat path-cost field, ticket 228), `FoxScentMap` (territory scent grid), `WardCoverageMap` (ward coverage intensity).
- Existing per-action perception scalars to migrate / supersede:
  - ticket 103 `escape_viability` (done) → consumers re-target to `Affordance(Flee, ...)` reads.
  - ticket 141 `combat_winnability` (ready) → **supersedes**. 141's worked composition (dps-balance + ttk + ally-factor) is the load-bearing implementation of `Affordance(Fight, perceiver, target)`. The work in 141 isn't lost — it becomes the heuristic estimator for `Fight` in this substrate. 141 may close as superseded once consumers migrate, OR land first as the implementation work for the Fight estimator and close on land.
- Adjacent flee-substrate work to coordinate with (not supersede):
  - ticket 230 (`Carve DispositionKind::Fleeing + substrate-aware flee picker`) — 230 carves the disposition; this substrate adds the affordance reads. Land in either order.
  - ticket 254 (`PickFleeTarget witness contract`) — 254 ensures Flee target selection is substrate-aware; the new `Affordance(Flee)` axis composes naturally with 254's witness contract.
  - ticket 100 (`Tremor map, Action::Stalk, and personality-driven hunt approach`) — 100 introduces Stalk as an Action; this substrate's `Affordance(Stalk)` axis is the natural consumer.

## Approach

1. Land `ActionKind` enum (all ~22 variants) + `ActionAffordances` resource skeleton.
2. Land `affordance_writer` system with v1 heuristic estimators wired for **all** kinds (the substrate is honest day one — every variant is computable, even if no DSE consumes it yet).
3. Land per-action `SimConstants` tunables.
4. Write read-API helpers under `src/ai/dses/` for the consideration-shape pattern (so consumer tickets are mechanical wiring).
5. Update tickets 103 and 141 — 103 transitions its readers to `Affordance(Flee)`; 141 lands as `Affordance(Fight)` directly inside this substrate (rather than as a parallel scalar) and may close as superseded.

## Verification

### Scenario microexperiments (≤ 3s, under `src/scenarios/`)

- `affordance_flee_high_cover` — cat in dense cover, predator approaching; verify `Affordance(Flee, cat, predator)` is high.
- `affordance_flee_open_ground` — same predator distance, open terrain; verify Flee-affordance materially lower.
- `affordance_dive_hawk` — hawk above prey; verify Dive-affordance high; verify Pounce-affordance is zero (wrong species — hawks can't pounce).
- `affordance_chase_prey` — fox chasing rabbit; verify Chase-affordance scales with speed differential.
- `affordance_fight_capability_match` — two cats; verify Fight-affordance scales inversely with `perceived_violence_capability` differential.
- `affordance_supersedes_legacy_scalars` — scenarios from ticket 103 (`escape_viability`) reproduce identical scoring behavior when consumers migrate to `Affordance(Flee)`.

### Soak gates

After substrate scaffolds (no consumers wired), `just soak 42` + `just verdict` should show **null behavioral drift** (substrate present, unconsumed → no per-DSE score change). Each consumer ticket then earns its own four-artifact methodology drift check per CLAUDE.md.

### Trace inspection

`just q trace <run-dir> <cat> <tick>` confirms `Affordance(*)` reads appear in L2 trace per consumer DSE with named heuristic-input contributions ("Affordance(Flee, target=fox_5) = 0.7 from cover_distance=2, speed_ratio=1.4, route_cost=12").

## Log

- 2026-05-10: opened sibling-to-258. Generalizes 103 (done) and supersedes 141 (ready) once consumers migrate. Session plan: `~/.claude/plans/after-working-256-i-dreamy-fiddle.md`.
