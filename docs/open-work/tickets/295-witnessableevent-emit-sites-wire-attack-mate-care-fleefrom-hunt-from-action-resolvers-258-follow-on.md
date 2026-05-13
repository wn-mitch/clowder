---
id: 295
title: WitnessableEvent emit sites — wire Attack / Mate / Care / FleeFrom / Hunt from action resolvers (258 follow-on)
status: in-progress
cluster: C
added: 2026-05-11
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

258 landed the `WitnessableEvent` enum with 9 variants but only wired 2 emit sites (`Groom` from goap.rs grooming-completion site, `SelfPlanFailed` dual-emit). The other 6 variants (`Attack`, `Mate`, `Care`, `FleeFrom`, `Hunt`, `ConspecificStartle`, `AmbientShock`) exist as enum members but no resolver emits them — they're substrate stubs. Consumer tickets (263–270) need these variants firing to actually exercise belief facets in DSE considerations. Without emit-site wiring, `MentalModel<Cat>.affiliation_history` only updates from grooming (not mating or care), `MentalModel<Predator>.perceived_violence_capability` only updates from species-prior Implants (not from observed combat), and the door-slam-scenario substrate sleeps. This ticket wires the remaining emit sites, leaving conspecific-startle and ambient-shock to ticket 242 / future weather hooks.

## Scope

- **`WitnessableEvent::Attack`** — emit from `src/systems/combat.rs::resolve_combat` (line ~44). Cat-vs-cat aggression only (cat-vs-wildlife handled separately under 265 wildlife substrate). Carry `severity` from the resolved damage. Add `MessageWriter<WitnessableEvent>` to combat.rs's SystemParam bundle or bundle into a new `CombatNarrativeEmitter`.
- **`WitnessableEvent::Mate`** — emit from `src/steps/disposition/mate_with.rs` caller site in `src/systems/goap.rs` (around the Pregnant-or-conception-completion branch) and `src/systems/disposition.rs` (legacy path). Use NarrativeEmitter::witnessable (already extended in 258).
- **`WitnessableEvent::Care`** — emit from `src/steps/disposition/feed_kitten.rs` caller site at `src/systems/goap.rs` (FeedKitten witnessed-completion) and disposition.rs equivalent.
- **`WitnessableEvent::FleeFrom`** — emit from `src/steps/disposition/flee_travel.rs` caller site when the flee step Advances. Identify the threat from `CurrentAction.target_entity` or `NearestThreat` anchor.
- **`WitnessableEvent::Hunt { success }`** — emit from prey-killed flow at `src/systems/disposition.rs:3592` and `src/systems/goap.rs:7061` (current `PreyKilled` emit sites). `success=true` on kill; `success=false` on hunt-step timeout or failed pounce.
- Each emit site uses the existing `NarrativeEmitter::witnessable` MessageWriter (extended in 258 for the goap.rs path). For combat.rs which doesn't currently use NarrativeEmitter, add a dedicated MessageWriter SystemParam.
- Update `belief_integrator` tests + add scenario tests under `src/scenarios/` for each new event class (one scenario per emit kind, asserting facet lift on a witness).

## Out of scope

- `WitnessableEvent::ConspecificStartle` — depends on `BodyCueStartled` marker from ticket 242 (body-cue substrate). Wired in 242's exit criteria.
- `WitnessableEvent::AmbientShock` — needs a weather/world-event hook (thunderclap, door-slam-equivalent). Either bundled into 242's door-slam scenario or a separate weather-cue ticket.
- `WitnessableEvent::SelfPlanFailed` — already wired in 258.
- `WitnessableEvent::Groom` — already wired in 258.

## Current state

258 landed 2026-05-11 (commit `c3bce3500e6e`). The integrator's `apply_observation` already has match arms for all 9 variants — it just isn't seeing them at runtime. From `src/systems/belief_integrator.rs`:

| Variant | Integrator wired? | Emit site landed? |
|---|---|---|
| `Attack` | yes | **no** ← this ticket |
| `Groom` | yes | yes (258) |
| `Mate` | yes | **no** ← this ticket |
| `Care` | yes | **no** ← this ticket |
| `FleeFrom` | yes | **no** ← this ticket |
| `Hunt` | yes | **no** ← this ticket |
| `ConspecificStartle` | yes | no (deferred to 242) |
| `AmbientShock` | yes | no (deferred to weather hook) |
| `SelfPlanFailed` | yes | yes (258 dual-emit) |

## Approach

Five small commits, one per variant. Each follows the same pattern (precedent: 258's `Groom` emit at `goap.rs:4799`):

1. Find the resolver caller's `record_if_witnessed(.., Feature::X)` line — that's the witnessed-Advance branch.
2. Add `narr.witnessable.write(WitnessableEvent::Y { ... })` immediately after, gated on `outcome.witness.is_some()` (or equivalent success flag for prey-killed / combat).
3. Pull `position`, `actor`, and `target` from the surrounding scope (all are in scope at every current resolver site — confirmed in 258 plumbing audit).
4. For combat.rs: combat doesn't currently use `NarrativeEmitter` — either extend its SystemParam set to include a new `MessageWriter<WitnessableEvent>`, or bundle into an existing PlanResources-like SystemParam to stay under Bevy's 16-param limit. Default: add a one-field `WitnessableEmitter<'w>` SystemParam and use it.

Each commit gets its own null-drift soak before the next — confirms the emit alone (without consumers) is byte-deterministic. Null drift is the expected outcome of every commit in this ticket since no consumer reads the new substrate yet.

## Verification

- `just check` clean after each commit.
- `cargo test belief_integrator` — extend tests to cover each newly-wired emit.
- `just soak 42` + `just verdict` after each emit — null drift, survival + continuity canaries hold.
- New scenarios under `src/scenarios/`: `belief_witnessed_attack`, `belief_witnessed_mate`, `belief_witnessed_care`, `belief_witnessed_flee`, `belief_witnessed_hunt` — each preloads cats in a scenario triggering the action, then asserts the witness's relevant facet lifted by EMA.
- `events.jsonl` trace inspection via `just q trace` — confirm each variant fires in the wild with the documented frequency.

## Log

- 2026-05-11: opened as 258 follow-on. Substrate enum is alive; this ticket lights up the remaining 5 emit paths so consumer tickets (263–270) have real facet motion to score against.
