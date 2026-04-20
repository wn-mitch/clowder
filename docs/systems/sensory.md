# Sensory System — Detection, Channels, and Profiles

## Status (Phase 1 — structural scaffolding)

Types, function, and per-species profile table are in place. Every spawnable
observer carries a `SensorySpecies` tag and a `SensorySignature` component.
Environmental modulation is **wired but inert**: `Weather`, `DayPhase`, and
`Terrain` all return identity multipliers (1.0) from their sensory methods.
No call sites are migrated yet — `detect()` is a defined function that nothing
calls from production code.

The `env_from_environment_is_identity_in_phase_1` unit test is a canary: it
iterates every (weather × phase × terrain) combination and asserts all
multipliers stay at 1.0. It fails the moment Phase 5b activation begins
without its verisimilitude hypothesis paperwork.

## The four channels

Every sensing decision in the sim flows through one function:

```rust
pub fn detect(observer: ObserverCtx, target: TargetCtx, env: EnvCtx) -> SensoryResult
```

`SensoryResult` carries a confidence value on `[0.0, 1.0]` for each of four
channels:

| Channel | Models | Modulated by |
|---|---|---|
| **Sight** | line-of-sight vision | weather (fog/storm), time-of-day (night), DenseForest occlusion |
| **Hearing** | airborne sound | weather (heavy rain/storm/wind masking) |
| **Scent** | olfactory detection | wind direction (downwind required), weather (rain damping), terrain |
| **Tremor** | substrate vibration | terrain transmission (stone > dirt > grass > water), *target's current action* |

Tremor is the atypical one: unlike the other three (where a target's
detectability is static), tremor signature depends on what the target is
*doing*. A motionless cat emits ~0; a stalking cat emits ~0.2×; a walking cat
emits 1.0×; a running cat emits ~1.8×; a fighting cat ~1.5×. This is what
gives the existing stalk/pounce mechanic real ecological weight: stalking hides
the cat from a rabbit's tremor sense, even though the rabbit can still hear
and see it.

**Touch is explicitly not a channel.** Contact-distance proprioception stays
modeled as proximity effects (`passive_familiarity_range`, combat adjacency,
etc.) — those are AOE, not perception.

## Per-species profiles

| Species | Sight | Hearing | Scent | Tremor | Notes |
|---|---|---|---|---|---|
| Cat | 10 | 8 | 15 | disabled | sight/hearing hunter |
| Fox | 8 | 10 | 12 | 3 | ears + nose dominant |
| Hawk | 15 | 5 | — | — | pure raptor vision |
| Snake | 1 | 3 | 8 | 6 | scent + vibration |
| Shadowfox | 8 | 7 | 10 | 5 | corrupted; wind-independent scent |
| Mouse | 3 | 6 | 5 | 6 | substrate-sensitive |
| Rat | 5 | 7 | 6 | 7 | substrate-sensitive |
| Rabbit | 6 | 10 | 4 | 12 | thumping fame = tremor-dominant |
| Fish | 3 | 5 | 5 | 6 | lateral line maps to tremor |
| Bird | 10 | 5 | 2 | 2 | sight-dominant |

All values are Phase 1 defaults in `src/resources/sim_constants.rs`
(`SensoryConstants`). They serialize into the constants-hash header, so any
change invalidates the headless diff baseline.

## Per-role modifiers

Role differences (Guard, Hunter, Scout) attach as a separate
`SensoryModifier` component with additive bonuses. The `combine()` method
lets multiple modifiers stack. Phase 1 doesn't attach any modifiers —
role promotion logic will wire them in when roles start using them.

## What's in scope vs. out of scope

**In scope:** per-agent sensing across cats, wildlife predators
(fox/hawk/snake/shadowfox), and prey (mouse/rat/rabbit/fish/bird).
Migrating ~20 `*_detection_range` constants across 7 systems into unified
profile lookups.

**Out of scope — coordinator-level sensing stays binary.** These are
aggregations of colony-wide awareness, not individual perception:
`posse_alarm_range`, `threat_proximity_range`, `colony_breach_range`,
`preemptive_patrol_scent_radius`, `wildlife_breach_range`. Modeling "what
the colony collectively knows" is a separate future feature (posse gossip
propagation).

**Out of scope — proximity effects stay binary.**
`passive_familiarity_range`, `bond_proximity_range`, `hearth_effect_radius`,
`tradition_familiar_distance` — these are area-of-effect, not perception.

## Migration discipline

The refactor is phased over multiple sessions. Each phase is a separately
merging PR.

1. **Phase 1 (done in part):** scaffolding — types, function, profiles,
   signatures at spawn, `SENSING_TRACE` tool. No behavior change.
2. **Phase 2:** shadowfox threat awareness (`threat_awareness_range` →
   `detect()`). First real migration; load-bearing defensive sensor.
3. **Phase 3:** cat hunt / forage sensors. Absorbs the existing
   wind-modulated scent system at `disposition.rs:1867`.
4. **Phase 4:** social / mortality / prey sensors. Prey
   `try_detect_cat()` becomes a thin wrapper over `detect()`.
5. **Phase 5a:** wildlife predators + line-of-sight introduction
   (occluders, Bresenham walk).
6. **Phase 5b:** semantic activation — Weather/DayPhase/Terrain multipliers
   go live. Balance pass with verisimilitude hypotheses per the Balance
   Methodology rule in `CLAUDE.md`.

**Phases 1–5a are structural.** Environmental multipliers pinned at 1.0.
Each migration must produce a byte-identical `SENSING_TRACE` diff against
the pre-migration baseline at seed 42.

**Phase 5b is semantic.** Multipliers go live one at a time, each activation
shipping as a testable hypothesis (ecological fact → predicted metric shift)
with concordance check.

## The SENSING_TRACE tool

When `SENSING_TRACE=1` is set in the environment, every `detect()` call
emits a JSONL record to `logs/sensing-trace.jsonl` (override path with
`SENSING_TRACE_PATH`). The record shape is:

```json
{"tick":142,"o":[12,8],"os":"cat","t":[15,10],"r":[1.0,0.0,0.0,0.0]}
```

`o` is observer position, `os` is observer species, `t` is target
position, `r` is `[sight, hearing, scent, tremor]` confidence.

Identity uses positions, not Entity IDs, because Entity IDs aren't stable
across runs. Two seed-42 runs with the same sim state produce the same
positions at the same ticks.

The tool is off by default with zero runtime cost (OnceLock keeps the
sink at None). It's migration infrastructure: Phase 2+ PRs produce
pre-migration baseline traces, run the migration, and diff the
post-migration trace — zero diff is the merge gate.

## Tuning

All profile values live in `SimConstants::sensory` and serialize into the
`logs/events.jsonl` header. The workflow for a tuning change:

1. Edit the profile field in `src/resources/sim_constants.rs`.
2. `just soak 42` produces a new headless run.
3. `just diff-constants` confirms the header reflects the change.
4. `just check-canaries` validates colony survival.
5. If drift in characteristic metrics exceeds ±10%, the change falls under
   the Balance Methodology rule in `CLAUDE.md` — write a verisimilitude
   hypothesis and run the concordance check before merging.
