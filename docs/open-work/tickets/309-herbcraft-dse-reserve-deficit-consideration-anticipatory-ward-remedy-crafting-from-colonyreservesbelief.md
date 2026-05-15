---
id: 309
title: Herbcraft DSE reserve-deficit consideration — anticipatory ward / remedy crafting from ColonyReservesBelief
status: ready
cluster: items-crafting
orchestration: substrate-sensitive
initiative: []
added: 2026-05-13
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

The Herbcraft DSE (and its planning surface — GatherHerb, PrepareRemedy, SetWard) currently fires reactively from acute signals: a wounded cat triggers ApplyRemedy, an immediate threat lifts SetWard, etc. There's no axis that says *"the colony's reserve is depleted, even though no acute crisis is present right now — go top up."* That's the gap ticket 260's soak exposed: the priestess kept choosing Guarding during sustained threat because Herbcraft only fired when she happened to *already* have thornbriar AND a placement target. With no reserve and no acute trigger, the chain stalled. 35 in-game days passed with no ward attempts and the colony lost 7 cats to ShadowFox ambush in an unwarded zone.

The fix is to add a new consideration on the Herbcraft DSE (and the relevant sub-actions in its plan template) that reads `ColonyReservesBelief` from ticket 308 and lifts the score when reserves are below target. The DSE then competes in the softmax against Guarding during quiet windows, pulling priestesses into GatherHerb / craft cycles *before* reserves fully empty.

This is the Desire-layer (per-tick DSE scoring) half of the anticipatory provisioning loop. Belief is in ticket 308; Intention layer (126 / 127) handles commitment retention so the priestess doesn't drop the herb-gathering plan the moment the reserves climb a sliver.

## Scope

- New consideration on `HerbcraftWardDse` (and / or the Herbalism plan-template scoring) reading `ColonyReservesBelief.thornbriar_count` against `colony_thornbriar_target` (new tuning constant).
- Curve: probably `Composite { Logistic(steepness=4, midpoint=0.5), Invert }` so the score is quiet when reserves are full, climbs as the gap grows, saturates when reserves are critical.
- Mirror axis on the remedy side reading `ColonyReservesBelief.remedy_herb_count`.
- Eligibility wiring with `HasWardHerbs` / `CanWard` markers from §4.3 of the substrate spec (currently designed-but-Absent).
- Tuning constants: `colony_thornbriar_target`, `colony_remedy_herb_target`, `herbcraft_reserve_consideration_weight`. Start with target counts low enough to validate behavior, then tune.
- Scenario: spawn priestess + low-thornbriar colony + ambient threat, assert priestess elects GatherHerb → SetWard cycle within N ticks even though no acute crisis is present.

## Out of scope

- The `ColonyReservesBelief` substrate itself (ticket 308 — blocker).
- Food-reserve equivalents on Cooking / Farming DSEs (similar pattern; natural follow-on).
- The §7.W fulfillment-scalar layer (separate concern).
- Role-aware crafting priorities — the long-term emergent town-roles vision. This ticket is the *general* anticipatory provisioning layer; role specialization sits above it.

## Current state

- Herbcraft / Herbalism DSE currently fires from acute triggers + spatial proximity to herb patches. Reactive only.
- `coordination.rs::compute_ward_placement` (1564) reads `cat_scent` for ward-site selection — 260's broadened authoring of CatScentMap shifted this from a peaked patrol-activity signal to a flat colony-density signal. That's a separate issue (mitigated by switching the consumer to `CatPatrolDeterrentMap` if desired, but out of scope here).
- §4.3 inventory markers (`HasWardHerbs`, `HasRemedyHerbs`, `CanWard`) designed, all Absent in code.

## Approach

1. Wait for 308 (belief substrate) to land.
2. Add the `HasWardHerbs` / `HasRemedyHerbs` / `CanWard` markers per §4.3 (substrate-stub rule: marker + reader + writer in same commit).
3. Add the reserve-deficit consideration to Herbcraft DSE; integrate into scoring.
4. Scenario microexperiment (anticipatory ward-craft under low reserve, no acute trigger).
5. Soak verification against `post-297-substrate-dormant` baseline: the 7-cat ambush wave from ticket 260's session should not recur — the priestess should rebuild the thornbriar reserve before depletion, wards should stay spread and replenished.

## Verification

- Scenario from §Approach.
- Soak survival gates: `Starvation == 0`, `ShadowFoxAmbush <= 10`, never-fired clean.
- The 260 regression marker: `kittens_born >= 1` on seed-42 (MatingOccurred fires; reproductive demographics restored).

## Log

- 2026-05-13: opened blocked-by 308. Hot context from 260's verification soak: priestess Guarding monopoly during 35k unwarded window, 7-cat ambush wave at (25-39, 20-23) following exhausted thornbriar reserve. This ticket is the Desire-layer half of the anticipatory-provisioning loop.
