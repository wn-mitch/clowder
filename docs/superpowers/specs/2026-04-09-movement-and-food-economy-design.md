# Movement Mechanics & Food Economy Overhaul

**Date:** 2026-04-09
**Problem:** Cats are 98% stationary per-tick. Hunting and foraging are abstract stand-and-wait timers. Food values are snack-sized, forcing constant hunting. Narrative fires repetitively without correlation to visible action.
**Goal:** Cats visibly hunt, forage, and roam. A single hunt is an event you can watch. Food sustains for days. Narrative tells a story, not a ticker tape.

---

## 1. Visible Hunting — Modeled on Real Cat Behavior

Cats are ambush predators. They don't patrol randomly — they detect prey by
scent carried on the wind, stalk carefully into position, and pounce from
close range. Personality shapes hunting style: patient cats wait for the
perfect angle, bold cats commit from further out, anxious cats spook prey.

### Wind & Scent System

**Wind resource** — `WindState` (new resource, like `WeatherState`):
- `direction: (f32, f32)` — normalized wind vector, rotates slowly (~0.01 rad/tick drift)
- `strength: f32` — 0.0 (calm) to 1.0 (gale). Affects scent carry distance.
- Shifts with weather: storms randomize direction, calm weather = steady.
- Updated by a `update_wind` system each tick (small drift + weather influence).

**Scent detection** — computed per cat during HuntPrey, not stored per-tile:
- For each prey entity, check if the cat is *downwind* of it (prey-to-cat vector aligns with wind direction).
- Scent range: `base_range * wind_strength * terrain_modifier`
  - `base_range`: 20 tiles
  - `wind_strength`: 0.3–1.0 (calm reduces range to 6 tiles)
  - `terrain_modifier`: 1.0 in open grass, 0.5 in light forest, 0.25 in dense forest (trees block scent)
- Downwind test: `dot(wind_dir, normalize(prey_pos - cat_pos)) > 0.3` — cat must be roughly downwind of prey.
- Result: cat knows *direction* of scent (upwind toward prey), not exact position.

### Hunt Phases

**Scent Search** — No scent detected. Cat moves upwind (into the wind) at 1 tile/tick, looking for a scent trail. This is purposeful movement — cats hunt into the wind to maximize detection. If wind is calm (strength < 0.2), wander using directional patrol. Search timeout: 60 ticks.

**Approach** — Scent detected, locked onto a specific prey entity. Cat moves 1 tile/tick in the upwind direction (opposite of `wind.direction`). Cat doesn't know the prey's exact tile — it moves generally upwind, and each tick the scent check re-confirms the prey is still detectable. Because the cat walks upwind and the prey is upwind, the cat converges naturally. If the prey moves or wind shifts and scent is lost, cat reverts to Search. Transition to Stalk when prey enters visual range (5 tiles manhattan).

**Stalk** — Prey visible at 2–5 tiles. Cat slows to 1 tile every 2 ticks (half speed, deliberate). Moves toward prey via `step_toward`. Cat stays low — prey doesn't spook yet at this range. Personality affects stalk:
- `patience > 0.7`: cat waits until distance ≤ 1 before pouncing (higher success)
- `patience < 0.3`: cat pounces at distance 2 (lower success, but faster)
- `anxiety > 0.7`: 15% chance per stalk tick of "spooked early" — cat makes a sudden move, prey bolts

**Pounce** — Cat commits from pounce range (1–2 tiles based on patience). Single-tick resolution:
- Success probability: `base * skill_mod * distance_mod`
  - `base`: 0.5
  - `skill_mod`: `0.5 + hunting_skill * 0.5` (range 0.5–1.0)
  - `distance_mod`: 1.0 at distance 1, 0.6 at distance 2
  - Total range: ~25% (unskilled, distance 2) to ~75% (skilled, distance 1)
- **On success**: Prey despawned. Item in inventory. Chain advances to return home.
- **On fail**: Prey state → `Fleeing`. Cat may give up (realistic — cats don't chase) or briefly pursue:
  - `boldness > 0.7`: chase for up to 5 ticks
  - Otherwise: step fails immediately (cat sits and watches prey escape)

### Parameters
| Parameter | Value |
|-----------|-------|
| Scent base range | 20 tiles |
| Scent in light forest | ×0.5 (10 tiles) |
| Scent in dense forest | ×0.25 (5 tiles) |
| Visual range (stalk trigger) | 5 tiles |
| Stalk speed | 1 tile per 2 ticks |
| Pounce range (patient cat) | 1 tile |
| Pounce range (impatient cat) | 2 tiles |
| Search timeout | 60 ticks |
| Brief chase (bold cats only) | 5 ticks |
| Pounce success (skilled, close) | ~75% |
| Pounce success (unskilled, far) | ~25% |

### Internal state
Extend `StepKind::HuntPrey` to carry persistent hunt state:
```
StepKind::HuntPrey {
    patrol_dir: (i32, i32),    // Current movement direction during search
}
```
Phase is implicit from step state: no `target_entity` = Search/Approach (scent-following), `target_entity` set + distance > 5 = Approach, distance 2–5 = Stalk, distance ≤ pounce range = Pounce.

Target entity is set on the step via `step.target_entity` when scent is first detected (locks onto a specific prey).

### Movement character
- **Search**: Purposeful upwind walk. 1 tile/tick. Cat looks like it's heading somewhere.
- **Approach**: Converging on scent. 1 tile/tick. Beeline-ish toward prey.
- **Stalk**: Slow, deliberate. 1 tile every 2 ticks. The visible slowdown is the tell.
- **Pounce**: Instant. One-tick resolution.
- **Post-fail**: Cat sits for 2–3 ticks (the "watching it go" moment), then step fails.

Cat moves **every tick** during Search and Approach. Every 2 ticks during Stalk. Never stands still without narrative reason. If direction is blocked, try perpendicular or reverse.

---

## 2. Food Values — A Hunt Is a Meal

One successful hunt should feed a cat for days. Current values are 3–7× too small.

| Item | Old | New | Satiety (ticks at 0.002 decay) | ~Sim days |
|------|-----|-----|-------------------------------|-----------|
| RawRat | 0.40 | 0.80 | 400 | 4 |
| RawMouse | 0.25 | 0.50 | 250 | 2.5 |
| RawFish | 0.35 | 0.70 | 350 | 3.5 |
| RawBird | 0.30 | 0.60 | 300 | 3 |
| Berries | 0.15 | 0.20 | 100 | 1 |
| Nuts | 0.15 | 0.20 | 100 | 1 |
| Roots | 0.15 | 0.20 | 100 | 1 |
| Mushroom | 0.15 | 0.20 | 100 | 1 |
| WildOnion | 0.15 | 0.20 | 100 | 1 |

### Expected food curve
- **Early colony** (tick 0–500): Stores empty, cats hunt aggressively. 8 cats × visible chases across the map.
- **Building phase** (tick 500–2000): Successful hunts fill stores. `food_fraction` rises. Hunt/Forage scores drop as stores fill (existing scoring formula).
- **Plateau** (tick 2000+): Stores at 60–80%. Cats shift to socializing, exploring, building. Occasional hunt when stores dip.
- **Disaster** (winter, predator incursion): Stores deplete → hunting ramps back up.

### Why this works
8 cats need ~8 meals per 250 ticks (one mouse-equivalent each). With 30 prey on the map breeding at current rates, that's sustainable. Excess kills accumulate in stores. The scoring system's `food_scarcity = (1.0 - food_fraction) * 0.5` naturally throttles hunting as stores fill.

---

## 3. Active Foraging

Foraging cat moves 1 tile per tick using directional patrol (not random walk).

- Pick a random direction, walk it, jitter 10% of the time
- Each tick, check current tile for forage yield
- Probabilistic find: `forage_yield * 0.25` chance per tick
  - Dense forest (0.5): 12.5% per tick → ~8 ticks avg
  - Light forest (0.3): 7.5% per tick → ~13 ticks avg
  - Grass (0.1): 2.5% per tick → ~40 ticks avg
- On find: item in inventory, chain advances to return-to-stores
- Search timeout: 40 ticks → step fails

Cat covers 8–40 tiles of ground per foraging trip. Visible movement across the landscape.

---

## 4. Narrative Pacing

**Principle: good storytellers don't repeat themselves.**

### Disposition-aware narration
- **Start**: Narrate when a cat enters a NEW disposition (different from previous). "Bramble heads out to hunt." Track `last_narrated_disposition: Option<DispositionKind>` per cat.
- **Events**: Narrate significant in-disposition moments:
  - Hunt: prey spotted, chase begins, catch, failed chase, returned to stores
  - Forage: found something, returned empty
  - Social: started grooming, formed bond
  - Rest: fell asleep (not every sleep cycle)
- **Suppress repeats**: If disposition is the same as last time, skip the start narration. Only narrate events within.
- **Existing rate-limiting**: Keep the 1-in-3 and 1-in-5 suppression for mundane events as a safety valve.

### Implementation
Add `last_narrated_disposition: Option<DispositionKind>` field to the `ActionHistory` component (already on every cat). The narrative system checks this before emitting disposition-start text.

For in-disposition events (prey spotted, catch, escape), emit from within the `HuntPrey`/`ForageItem` step handlers in `resolve_disposition_chains`, not from `generate_narrative`. This ties narration to actual game events rather than action completion timers.

---

## 5. Prey AI

Give `PreyAnimal` entities autonomous behavior so the map feels alive.

### Component addition
Add `PreyAiState` enum to `PreyAnimal` (or as a new component):
```
enum PreyAiState {
    Grazing { dx: i32, dy: i32 },  // Wander slowly in habitat
    Fleeing { from: Entity },       // Run from a specific cat
    Idle,                           // Standing still (default)
}
```

### Behavior
- **Idle → Grazing**: 5% chance per tick. Pick random direction.
- **Grazing**: Move 1 tile every 3 ticks (slow drift). 10% direction jitter. Reverse on blocked terrain. Stay within habitat tiles. Revert to Idle after 20 ticks.
- **Alert → Fleeing**: Prey detects cat at 2 tiles (visual) OR when a pounce fails. Transition to Fleeing.
- **Fleeing**: Move 1 tile per tick AWAY from the threat (opposite of `step_toward`). Flee for up to 15 ticks or until threat is >10 tiles away, then revert to Idle. Prey that reaches water (fish) or dense forest escapes more easily.

### System
New system `prey_ai` in `systems/prey.rs`, registered after `prey_population` and `prey_hunger`. Runs every tick for all `PreyAnimal` entities.

### Interaction with HuntPrey
- HuntPrey pounce failure sets prey state to `Fleeing { from: cat_entity }`
- Prey with `anxiety > 0.7` cat approaching in Stalk phase has 15% chance per tick of bolting (the cat spooked it)
- The prey AI system handles all flee movement
- HuntPrey checks adjacency each tick for pounce resolution
- Stalking cat does NOT trigger flee at distance 2-5 (cats are stealthy) — only the pounce or a spooked stalk triggers flight

---

## Files Modified

| File | Change |
|------|--------|
| `src/resources/wind.rs` | **New** — `WindState` resource (direction, strength) |
| `src/resources/mod.rs` | Register wind module |
| `src/systems/wind.rs` | **New** — `update_wind` system (drift + weather coupling) |
| `src/systems/mod.rs` | Register wind module |
| `src/systems/disposition.rs` | Rewrite HuntPrey (scent/stalk/pounce), rewrite ForageItem (patrol) |
| `src/components/items.rs` | Bump food_value() for all items |
| `src/components/prey.rs` | Add `PreyAiState` enum/component |
| `src/components/task_chain.rs` | Extend `StepKind::HuntPrey` with `patrol_dir` field |
| `src/systems/prey.rs` | Add `prey_ai` system |
| `src/systems/narrative.rs` | Disposition-aware narration, suppress repeats |
| `src/components/disposition.rs` | Add `last_narrated_disposition` to ActionHistory |
| `src/plugins/simulation.rs` | Register `prey_ai`, `update_wind` systems |
| `src/main.rs` | Register new systems in headless schedule |

## Verification

1. `cargo test` — all existing tests pass
2. Per-tick trace analysis:
   ```
   cargo run -- --headless --duration 10 --seed 42 --trace-positions 1 --event-log /tmp/trace.jsonl
   ```
   - Cats should be stationary < 30% of ticks (down from 98%)
   - Hunt action should show continuous movement (stalk/chase phases)
   - Forage action should show continuous movement (patrol pattern)
3. Food economy check:
   - `food_fraction` should rise from 0% to 50%+ within 2000 ticks
   - Hunt frequency should decrease as stores fill
4. Narrative check:
   - No repeated "heads out to hunt" for the same disposition
   - Chase events (spotted, caught, escaped) appear in log
   - Log entries should correlate with visible movement
