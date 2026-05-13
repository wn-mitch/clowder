---
id: 304
title: WitnessableEvent::Attack emit — gated on cat-vs-cat aggression substrate
status: ready
cluster: belief-perception
initiative: [full-sensory-perception]
added: 2026-05-12
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

295 wired four of the five in-scope `WitnessableEvent` emit paths (Mate / Care / FleeFrom / Hunt) but had to defer `Attack`: ticket 258's `belief_integrator` already has the matching arm (updates actor's `perceived_violence_capability` + `recency_of_threat_cue`, target's `perceived_injury_level`, and the location's `recency_of_threat_cue`), but no resolver in `src/` ever emits an `Attack` event because **cat-vs-cat aggression doesn't exist yet**. `src/systems/combat.rs::resolve_combat` is exclusively cat-vs-wildlife (the rustdoc at line 36 explicitly says so); there is no `resolve_fight_cat`, no `Disposition::AttackCat`, no plan template that targets another cat with hostile intent. The `Attack` variant therefore sleeps as a substrate stub — integrator reader without a writer, the exact antipattern `scripts/check_substrate_stubs.sh` polices for markers.

This ticket holds the intent durably until cat-vs-cat aggression substrate lands. It is **gated on a prerequisite that doesn't yet have its own ticket** — see §Out of scope.

## Scope

- Emit `WitnessableEvent::Attack { actor, target, position, severity, tick }` at the moment cat-vs-cat combat damage is applied.
- Follow the 295 emit pattern: parallel `narr.witnessable.write(...)` immediately after the resolver's damage-application step, gated on whether real damage occurred (severity > 0).
- Carry `severity: f32` from the resolved damage amount (the integrator uses it as the observed-value for `perceived_violence_capability` and `perceived_injury_level` updates).
- Add a scenario under `src/scenarios/`: `belief_witnessed_attack` — two adult cats with antagonistic relationship + adjacent positions + a witness within range; asserts witnesses' beliefs lift the actor's `perceived_violence_capability` and the target's `perceived_injury_level`.

## Out of scope

- **Cat-vs-cat aggression substrate itself**. Designing the disposition, the plan template, the resolver, and the balance work for when cats attack each other (vs. fleeing / posturing / displacement) is a much larger surface than 295's emit-wiring scope. That work needs its own ticket — likely sitting in Cluster C alongside 267 (Conflict-low DSEs: Threaten / Posture / Hiss escalation rungs) and 268 (perimeter / territory antagonism). Once cat-vs-cat aggression has a real resolver, 304 wires the emit on top in one commit.
- **Cat-vs-wildlife combat witnessing**. The `Attack` variant is for cat-cat. Cat→wildlife combat is handled separately under 265 (wildlife substrate), which has its own `WitnessableEvent` flow design.
- **Predator→cat attack witnessing**. Predator-side aggression has its own emit shape — likely a `PredatorAttack` variant or composing through the `Hunt { success: ... }` path. Out of scope here.

## Current state

`src/messages/witnessable_event.rs:28–35`:

```rust
Attack {
    actor: Entity,
    target: Entity,
    position: Position,
    severity: f32,
    tick: u64,
}
```

`src/systems/belief_integrator.rs:167–217` — the match arm is fully implemented (updates 3 different facet paths). Reader exists. No writer.

`src/systems/combat.rs` — cat-vs-wildlife only. Function signature has 14 SystemParams, one shy of Bevy's 16-param ceiling; if cat-vs-cat damage logic ends up extending this function, the new emit needs to bundle params into a `#[derive(SystemParam)]` struct or split into a sibling function with its own param bag.

## Approach

When cat-vs-cat aggression lands (likely as a Cluster C ticket gated on Conflict-low DSEs, the antagonism marker family, and the relationship-side damage application):

1. Find the resolver call site where cat-cat damage is applied. Capture: attacker `Entity`, defender `Entity`, attacker `Position`, `damage: f32`.
2. Add the emit immediately after damage is applied, gated on `damage > 0.0` (or whatever fail-mode the resolver carries — e.g. a missed swipe should not emit).
3. Pattern matches `record_hunt_attempt`'s centralization in goap.rs (295 precedent) — if multiple cat-vs-cat aggression sites land, centralize through a single helper.

```rust
narr.witnessable.write(
    crate::messages::witnessable_event::WitnessableEvent::Attack {
        actor: attacker_entity,
        target: defender_entity,
        position: attacker_pos,
        severity: damage,
        tick: time.tick,
    },
);
```

## Verification

- `just check` clean (substrate-stub linter requires this; once the writer lands, the `Attack` variant graduates from "integrator-only" to fully wired).
- New scenario `belief_witnessed_attack` passes; witness's `CatBeliefs.models[actor].perceived_violence_capability.value > 0`, `perceived_injury_level.value > 0` on the target's model, `LocationBeliefs.models[bucket(pos)].recency_of_threat_cue.value > 0`.
- Integration test in `src/systems/belief_integrator.rs::tests` covers the Attack arm (parallel to the 4 emit-coverage tests added in 295).
- `just soak 42` + `just verdict` after wiring — expect null drift (the integrator updates beliefs, but no DSE reads `perceived_violence_capability` / `perceived_injury_level` yet until consumer tickets land).

## Log

- 2026-05-12: opened as the deferred Attack arm of 295. Gated on cat-vs-cat aggression substrate, which has no ticket yet — this entry holds the intent until that substrate lands. Mirrors 295's deferrals of `ConspecificStartle` (242) and `AmbientShock` (weather hook) — both also waiting on prerequisite substrate.
