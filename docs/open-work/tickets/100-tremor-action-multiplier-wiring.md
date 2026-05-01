---
id: 100
title: Tremor map, Action::Stalk, and personality-driven hunt approach
status: ready
cluster: null
added: 2026-05-01
parked: null
blocked-by: [062]
supersedes: []
related-systems: [sensory.md, ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Two dead paths in the sensory model prevent tremor from influencing
any production behavior:

1. `prey_cat_proximity()` returns `.sight` only. Rabbits (tremor=12,
   their primary defensive sense) detect cats the same way birds do
   (tremor=2).
2. `current_action_tremor_mul` is hardcoded to `1.0` everywhere. A
   stalking cat and a sprinting cat are indistinguishable to prey.

But the structural fix opens a richer opportunity. Tremor should be an
influence map — fast-decaying (1–3 ticks vs. scent's ~1 day) but the
same pattern. A running cat deposits heavily; a stalking cat deposits
nearly zero. Once the map exists and `Action::Stalk` is a first-class
enum variant, the cat's own personality can govern how it reads the
environment to choose its approach.

The IAUS framing: don't branch on "bold → charge, patient → stalk."
Instead, express stalk vs. beeline as utility outputs from axes already
in the system — `boldness` and `patience` on `Personality`, prey
`alertness` on `PreyState`, and ambient environment reads from the
`TremorMap` and per-species scent maps (ticket 062). The behavior
emerges from the decision levers rather than being explicitly programmed.

Blocked by ticket 062: the per-species scent maps are a cat-side
opportunity-quality input (scent strength at prey location = how long
prey has been settled there), so 062 must land first.

## Approach

Two existing decision points carry the personality × environment
interaction naturally.

**Decision point 1 — `HuntTarget` DSE (target selection).**
The alertness axis already penalizes alert prey. Its weight becomes
personality-modulated:

```
alertness_weight = base_alertness_weight × (1 − boldness × boldness_alertness_discount)
```

Bold cats care less about how nervous their target is and will
occasionally commit to alert prey. Patient cats filter heavily for
calm targets. The approach style then follows the *selected* target:
a cat that chose an alert rabbit has little margin to preserve and may
as well charge; a cat that waited for a calm, settled rabbit has reason
to stalk. No explicit approach-style branch needed — it falls out of
target selection.

**Decision point 2 — `EngagePrey` approach phase (behavior parameter).**
`stalk_start_buffer` and `stalk_start_minimum` are currently global
constants. Make the effective value a continuous per-cat computation:

```
effective_stalk_distance = stalk_minimum
    + stalk_buffer × patience
    + alertness_push × prey.alertness
    + species_push × prey_tremor_sensitivity
```

This is a behavior *parameter*, not a behavior *selection* — no new
GOAP actions required. Bold cats barely stalk (low `patience`,
low `alertness_push`). Patient cats stalk from far out. Critically,
even a bold cat gets pushed upward by a high `prey.alertness` reading:
the ecological reality overrides the reckless instinct at the extremes.

**The influence maps as cat-side opportunity reads.**
The maps are bidirectional. Cats use them not just to find prey but
to assess whether *now* is a good time to commit to an approach:

- `TremorMap` at the prey's position: low reading → prey is resting,
  minimal self-vibration, this is the moment. High reading → prey
  is already moving, potentially alert.
- Per-species scent strength at prey's position (from ticket 062):
  strong accumulated scent → prey has been settled there for a while,
  a stalk opportunity. Thin scent → prey arrived recently, may bolt
  unpredictably.

Patient cats consult both before committing. Bold cats skip the read
and go on the direct prey detection signal alone — which is expressed
naturally by the lower weight those axes carry in the bold-cat
effective-stalk-distance computation.

**Emergent ecological niching.**
Over many hunts: bold cats repeatedly alert rabbits (tremor=12) and
fail; they succeed more often on birds (tremor=2) and fish. Patient
cats succeed broadly but particularly against high-tremor prey. Species
preferences emerge from hunt success rates without any explicit
tradition logic — purely from the axis weights and the ecology.

## Scope

1. **`Action::Stalk` and `Action::Pounce` in `src/ai/mod.rs`** —
   add both variants to the `Action` enum. Update
   `GoapActionKind::to_action()` so `EngagePrey` still maps to
   `Action::Hunt` at plan-adoption time, but the `EngagePrey` resolver
   in `goap.rs` / `disposition.rs` sets `current_action.action =
   Action::Stalk` when entering `StepPhase::Stalking` and
   `Action::Pounce` when entering `StepPhase::Pouncing`. This is the
   load-bearing prerequisite: without it, `tremor_tick` cannot
   differentiate stalking from charging and the tremor suppression
   benefit is dead.

2. **`src/resources/tremor_map.rs`** — new `TremorMap` resource. Same
   bucketed-grid shape as `PreyScentMap` (120×90, bucket_size=3).
   Methods: `new`, `default_map`, `deposit`, `decay_all`,
   `highest_nearby`, `get`. Add `TremorConstants` to `SimConstants`:
   `deposit_per_tick`, `decay_per_tick` (default: empties a full
   bucket in 1–3 ticks), `detect_threshold`, and the action-multiplier
   table (see item 3).

3. **Action-multiplier table in `TremorConstants`** —
   `action_tremor_stalk`, `action_tremor_idle`, `action_tremor_walk`,
   `action_tremor_run`, `action_tremor_fight`, `action_tremor_pounce`.
   Initial values: stalk≈0.2, idle/resting/sleeping=0.0, walk=1.0,
   run≈1.8, fight≈1.5, pounce≈2.0 (explosive spring). Add
   `action_tremor_mul(action: Action, c: &TremorConstants) -> f32`.

4. **`InfluenceMap` impl in `src/systems/influence_map.rs`** —
   `name: "tremor"`, `channel: ChannelKind::Tremor`,
   `faction: Faction::Neutral`.

5. **`tremor_tick` writer system** — iterates all entities with
   `(&Position, &SensorySignature, &CurrentAction)`. Deposit:
   `signature.tremor_baseline × action_tremor_mul(action) ×
   deposit_per_tick`. Decay first each tick. Register in
   `SimulationPlugin::build()` alongside `prey_scent_tick`.

6. **Resource registration** — `setup.rs` inserts
   `TremorMap::default_map()`. Export from `src/resources/mod.rs`.

7. **Prey consumer cutover in `src/systems/prey.rs`** —
   `try_detect_cat` samples `tremor_map.highest_nearby` at the prey's
   position before the entity-iteration sight loop. Signal above
   `TremorConstants::detect_threshold` → enter `PreyAiState::Alert`.
   Point-to-point sight loop continues for entity identification.

8. **`prey_cat_proximity()` fix in `src/systems/sensing.rs`** —
   return `result.sight.max(result.tremor)` instead of `.sight` only.

9. **`HuntTarget` DSE alertness axis — boldness modulation** in
   `src/ai/dses/hunt_target.rs`. The alertness consideration weight
   becomes `base_alertness_weight × (1 − boldness ×
   c.boldness_alertness_discount)` where
   `boldness_alertness_discount` is a new `ScoringConstants` field
   (default ≈ 0.4, meaning a fully bold cat discounts the alertness
   penalty by 40%). Bold cats will occasionally select alert prey;
   patient cats filter for calm prey. No new axes — modulate the
   existing weight.

10. **`EngagePrey` approach phase — `effective_stalk_distance`** in
    `src/systems/goap.rs` (and the disposition.rs path). When
    transitioning from `StepPhase::Approaching`, replace the
    constant `stalk_start_buffer` / `stalk_start_minimum` comparison
    with a per-cat computation:

    ```
    effective_stalk_distance = c.stalk_start_minimum
        + c.stalk_start_buffer × personality.patience
        + c.alertness_push    × prey_state.alertness
        + c.species_push      × prey_tremor_sensitivity(prey_kind, &constants.sensory)
    ```

    where `prey_tremor_sensitivity` normalises the prey species'
    tremor `base_range` against the maximum (Rabbit=12). New
    `DispositionConstants` fields: `alertness_push` (default≈3.0),
    `species_push` (default≈2.0). The existing `stalk_start_buffer`
    and `stalk_start_minimum` remain as the personality-neutral base
    values.

11. **Opportunity-quality reads in `EngagePrey` approach phase** —
    before committing to the approach, the cat samples two ambient
    signals at the prey's last known position:
    - `TremorMap::get(prey_pos)` — high value means prey is actively
      moving; feeds into an `opportunity_quality` scalar that scales
      `effective_stalk_distance` upward (more caution when prey is
      restless).
    - Per-species scent strength from `PreyScentMaps::for_kind(prey_kind)
      .get(prey_pos)` (requires ticket 062) — high value means prey
      has been settled; scales `effective_stalk_distance` downward
      (prey is comfortable, now is the moment).
    - These reads only apply when `personality.patience > threshold`
      (a new `DispositionConstants` field, default≈0.4) — bold cats
      skip the ambient read entirely, expressed by the axis weight
      dropping to zero below the threshold.

12. **Trace emitter** — add `tremor_map: Option<Res<TremorMap>>` to
    `emit_focal_trace` in `src/systems/trace_emit.rs` and wire through
    `emit_l1_for_map`.

## Out of scope

- Water-mask blocking of tremor (`Terrain::tremor_transmission()` →
  Phase 5b).
- Weather / day-phase tremor multipliers (all return 1.0 → Phase 5b).
- Per-species tremor maps — tremor is aggregate substrate vibration;
  prey cannot discriminate cat vibration from fox vibration.
- Explicit species preferences baked into cat personality — let those
  emerge from hunt success rates, not direct assignment.
- Tremor-based hunting for cats (vole-hunting mechanic — different
  ticket if ever).

## Verification

- Unit tests in `tremor_map.rs`: deposit + decay, `highest_nearby`,
  `action_tremor_mul` for each `Action` variant including `Stalk` and
  `Pounce`.
- Existing `sensing.rs` tests `rabbit_feels_tremor_from_running_cat`
  and `stalking_cat_hides_tremor_from_rabbit` must still pass.
- New `sensing.rs` test: stalking cat at tremor range (beyond sight)
  is undetected by rabbit; running cat at same distance is detected.
- Unit test on `HuntTarget` DSE: bold cat (boldness=0.9) scores an
  alert prey candidate higher than a neutral cat does at equal
  distance and yield; patient cat (patience=0.9) scores the same
  candidate lower.
- Unit test on `effective_stalk_distance`: bold+impatient cat
  produces a value near `stalk_start_minimum`; patient cat produces a
  value near `stalk_start_minimum + stalk_start_buffer`.
- `just soak-trace 42 Simba` — `"tremor"` key in L1 records;
  `Action::Stalk` visible in trace during EngagePrey steps.
- `just inspect <bold-cat-name>` and `just inspect <patient-cat-name>`
  from a soak run — confirm different effective stalk distances and
  different prey-alertness selection patterns in decision history.
- **Hypothesis (file before soaking):** _"Stalk/pounce now
  ecologically live and personality-modulated → bold cats: similar
  hunt attempt rate, lower success rate on high-tremor prey (rabbit,
  rat); patient cats: lower attempt rate, higher success rate
  overall. Net: colony-wide starvation unchanged; per-cat hunt
  efficiency varies by personality axis."_ Drift > ±10% on Hunt
  success or prey death rate requires concordance check.
- `just frame-diff` — check Hunt DSE score distribution split by
  personality bucket (bold vs. patient) if the log supports it;
  otherwise check aggregate Hunt success against baseline.

## Log

- 2026-05-01: opened from conversation on ticket 062. `prey_cat_proximity()`
  confirmed sight-only; `current_action_tremor_mul` confirmed hardcoded
  1.0 at all call sites. Scope expanded to full `TremorMap` influence
  map. `Action::Stalk` identified as load-bearing prerequisite —
  without it `tremor_tick` cannot differentiate stalking from charging.
  Personality-driven approach logic added: `HuntTarget` DSE boldness
  modulation on alertness axis weight (item 9) and per-cat
  `effective_stalk_distance` from patience + environmental reads
  (items 10–11). Design principle: behavior emerges from IAUS axis
  weights, not explicit stalk-or-charge branches. Blocked on 062 for
  the per-species scent opportunity-quality read (item 11).
