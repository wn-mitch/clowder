---
id: 101
title: Environmental quality — five influence maps for ambient spatial pressure
status: ready
cluster: null
added: 2026-05-01
parked: null
blocked-by: [100]
supersedes: []
related-systems: [environmental-quality.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Cats currently have no sense of *where* they are beyond the tile they occupy.
A well-tended hearth and a patch of mud two tiles away feel identical; a fairy
ring adjacent to the den radiates nothing; a corrupted tile just outside
sensing range is invisible until a cat steps on it. The world is rich in
spatial signal that never reaches the decision layer.

Environmental quality fixes this with five tile-resolution influence maps —
comfort, cleanliness, beauty, mystery, and corruption — each a flat
`Vec<f32>` rebuilt on a cadence. Sources stamp influence outward with radial
falloff; cats sample their position as `EvalInput` scalars and those scalars
thread through the IAUS like any other consideration axis. The same maps are
ambient infrastructure for future DSE location-preference (where a cat chooses
to sleep, groom, or linger) and coordinator axes (low colony-average beauty
motivating garden construction) without rearchitecting.

The five axes each bind to a distinct personality trait:

| Map | Sources | Personality |
|-----|---------|-------------|
| Comfort | terrain ease, building proximity, weather | `warmth` / `(1 − independence)` |
| Cleanliness | corpses, mud, dirty buildings | `anxiety` |
| Beauty | fairy rings, gardens, standing stones, deep pools | `spirituality` |
| Mystery | `Tile.mystery` (fairy rings, ruins, standing stones) | `curiosity` |
| Corruption | `Tile.corruption` (hot tiles radiated outward) | — (magic system owns response) |

Corruption's influence map is spatial perception infrastructure: cats sense
approaching corruption before standing on it. The behavioral response (health
drain, mood penalty, erratic action) stays in the magic system unchanged.

## Scope

- **Five map resources** in `src/resources/env_quality.rs`:
  `ComfortMap`, `CleanlinessMap`, `BeautyMap`, `MysteryMap`,
  `CorruptionInfluenceMap`. Each is a flat tile-resolution `Vec<f32>`
  following the `CarcassScentMap` struct shape (`marks`, `width`, `height`,
  `get(x, y)`, `clear()`). Add a shared `stamp(cx, cy, peak, radius)`
  helper using linear falloff: `peak × (1 − dist / radius).max(0.0)`, applied
  additively and clamped to `[−1.0, 1.0]` per cell. Export from
  `src/resources/mod.rs`.

- **Update system** `update_env_quality_maps` in `src/systems/env_quality.rs`.
  Runs `run_if(on_cadence)` — cadence controlled by a new
  `EnvironmentalQualityConstants::update_interval` knob. Single sweep:
  1. Clear all five maps.
  2. Sweep `TileMap` once — stamp terrain contributions to comfort, beauty,
     mystery (from `Tile.mystery`), and corruption (from `Tile.corruption`).
  3. Sweep building entities — stamp comfort (scaled by `structure.condition`)
     and cleanliness (scaled by `structure.cleanliness`) at each building's
     position and radius.
  4. Sweep `Dead` entities without `Buried` marker — stamp cleanliness penalty.
  5. Apply `Weather::comfort_modifier()` as a flat additive offset across every
     comfort cell.
  6. Clamp all cells to `[−1.0, 1.0]`.

- **Source definitions per map:**

  *Comfort* — terrain base values (FairyRing +0.3, LightForest +0.1,
  DenseForest +0.05, Sand −0.05, Mud −0.15, Rock −0.1, all others 0.0);
  building bonuses radiating at condition-scaled peak (Den +0.2 / 2 tiles,
  Hearth +0.25 / 3 tiles, Stores +0.05 / 1 tile, Workshop +0.10 / 1 tile,
  Garden +0.15 / 2 tiles, WardPost +0.05 / 1 tile); global weather overlay
  applied after stamping.

  *Cleanliness* — negative stamps from unburied `Dead` entities (−0.4 peak,
  3-tile radius); negative from buildings where `structure.cleanliness <
  dirty_threshold` (magnitude scales with `1 − cleanliness`); Mud terrain
  (−0.15, on-tile only, stamped in the terrain sweep).

  *Beauty* — positive from FairyRing (+0.4 / 3 tiles), Garden (+0.2 / 2 tiles),
  StandingStone (+0.25 / 2 tiles), DeepPool (+0.15 / 2 tiles); positive from
  well-conditioned Den and Hearth (aesthetic upkeep: `condition × 0.1` / 1
  tile); negative from AncientRuin (−0.1, on-tile, blighted landscape overlay).
  High corruption tiles suppress beauty: subtract `tile.corruption × 0.2`
  during the terrain sweep.

  *Mystery* — read from `Tile.mystery`; stamp outward with a short falloff
  (default 2 tiles) so adjacent tiles feel the resonance. Sources are
  already seeded at world-gen: FairyRing 0.7–1.0, StandingStone 0.6–0.9,
  DeepPool 0.4–0.7, AncientRuin 0.3–0.6 (alongside corruption — ruins are
  both corrupted and mysterious).

  *Corruption* — read from `Tile.corruption`; stamp outward (default 3-tile
  radius) so cats perceive the gradient before crossing the threshold. The
  stamping makes the existing `corruption_tile_effects` point-sample
  redundant for spatial awareness; that system's mood penalties remain
  active and are not retired here.

- **Five `EvalInput` scalars** — `"local_comfort"`, `"local_cleanliness"`,
  `"local_beauty"`, `"local_mystery"`, `"local_corruption"`. Add the five
  map resources to `EvalInputs` (or `ColonyContext`); resolve in `ctx_scalars`
  by calling `map.get(inputs.position.x, inputs.position.y)`.

- **`EnvironmentalQualityModifier`** in the modifier pipeline
  (`src/ai/eval.rs`). Reads the four mood-relevant maps (comfort, cleanliness,
  beauty, mystery — not corruption). Combines with personality scaling:

  ```
  comfort_contrib    = local_comfort    × (1.0 + warmth × 0.3) × (1.0 − independence × 0.2)
  cleanliness_contrib = local_cleanliness × (1.0 + anxiety × 0.4)
  beauty_contrib     = local_beauty     × (1.0 + spirituality × 0.4)
  mystery_contrib    = local_mystery    × (1.0 + curiosity × 0.4)

  combined = clamp((comfort_contrib + cleanliness_contrib + beauty_contrib + mystery_contrib)
                   × combination_weight,
                   −0.3, +0.3)
  ```

  All scaling factors and clamp bounds are `EnvironmentalQualityConstants`
  knobs, not inline magic numbers.

- **Feature emission** — `Feature::EnvironmentalComfortPositive` /
  `Feature::EnvironmentalComfortNegative` fired when the combined modifier
  crosses a constants-controlled threshold. Classify
  `EnvironmentalComfortPositive` as `expected_to_fire_per_soak() => true`
  (seed-42 colony has a hearth and gardens; the modifier should go positive
  for cats near them). Classify `EnvironmentalComfortNegative` as `false`
  (canary enrollment deferred until a scenario reliably produces a negative-
  dominant environment in seed 42).

- **`EnvironmentalQualityConstants`** sub-struct added to `SimConstants`.
  Fields: `update_interval` (cadence), per-map source radii and peak values
  (matching the defaults enumerated above), `falloff: linear` (shape not
  tunable at launch — promote to a knob if terrain-specific falloffs are
  needed), the four personality scaling factors, `combination_weight`,
  modifier clamp bounds. Serialized into the `events.jsonl` header with the
  rest of `SimConstants`.

- Update `docs/systems/environmental-quality.md` to reflect the
  influence-map architecture and the locked five-axis design.

- Register `update_env_quality_maps` in `SimulationPlugin::build()` — runs
  after `decay_building_condition` and dead-entity systems so cleanliness
  and building-condition reads are current. Regenerate
  `docs/wiki/systems.md` (`just wiki`) in the same commit.

## Out of scope

- **DSE location preference** — cats choosing *where* to sleep, groom, or
  linger based on map values. The map must support cheap `get(x, y)` at
  arbitrary positions from day one (it will). Wiring into target-taking DSEs
  is a separate ticket with a larger surface area.
- **Coordinator axes** — low colony-average beauty motivating garden
  construction; high average filth escalating burial priority. Follow-on to
  coordinator work (tickets 057, 081).
- **`CorruptionLandmarks` retirement** — the corruption influence map makes
  the centroid-only `CorruptionLandmarks` resource redundant (centroid is
  derivable from the map). Retire as a follow-on once the corruption map is
  proven stable.
- **`apply_building_effects` dirty-discomfort retirement** — currently
  applies a temperature drain for dirty buildings. The cleanliness map
  targets mood, not temperature; they can coexist. Reconcile at
  implementation time if they produce redundant signals.
- Monument contributions (ticket 021 not yet landed; add as a beauty source
  when 021 lands).
- Snow-depth per tile (`Tile` has no `snow_depth` field). Handle via the
  `Weather::comfort_modifier()` global overlay for now; promote to a per-tile
  stamp when a snow-depth field exists.
- Rendering debug overlays for the five maps (F6/F7/F8 pattern).
- Generic `InfluenceMap<T>` abstraction — ticket 100 plans
  `src/systems/influence_map.rs` as shared infrastructure. If 100 lands
  first, align env-quality maps to that type instead of the per-struct
  pattern described here. If 100 is still in flight, proceed with
  per-struct and harmonize later.

## Approach

Follow the `CarcassScentMap` pattern in `src/resources/`:

1. **Map structs.** Each is a `Vec<f32>` at tile resolution. Shared logic
   (`stamp`, `get`, `clear`) can live on a private helper or be duplicated
   per map — decide at implementation time based on whether ticket 100's
   `InfluenceMap` type is available. Register all five in `setup.rs` and
   `setup_world_exclusive`.

2. **Update system sweep order.** Clear → terrain sweep (one double-loop over
   `TileMap`) → building sweep (one entity query) → dead-entity sweep (one
   entity query) → weather overlay. Single update per cadence tick; avoid
   per-tick recomputes.

3. **`EvalInputs` wiring.** Add the five map resources as reference fields on
   `EvalInputs` alongside `corruption_landmarks` and `exploration_map`. In
   `ctx_scalars`, sample `map.get(inputs.position.x, inputs.position.y)` for
   each and insert under the five scalar names. DSEs written against these
   names work immediately with no further plumbing.

4. **Modifier registration.** Add `EnvironmentalQualityModifier` to
   `default_modifier_pipeline` in `src/ai/eval.rs`. Give it a
   `feature_on_apply` once ticket 099's trait extension lands (if 099
   precedes this); otherwise emit features at the call site via the existing
   name-match pattern.

## Verification

- Unit tests in `env_quality.rs`:
  - `stamp` — peak value at source tile, linear falloff to zero at radius
    edge, no contribution beyond radius, additive when two sources overlap.
  - `get` out-of-bounds returns 0.0.
  - Weather overlay — comfort cells shift uniformly by `comfort_modifier()`.
- Unit tests for `EnvironmentalQualityModifier`:
  - High-warmth cat gets larger comfort contribution than a neutral cat at the
    same map value.
  - High-anxiety cat gets larger cleanliness penalty.
  - High-curiosity cat gets larger mystery contribution.
  - Combined modifier is clamped to `[−0.3, +0.3]`.
- `just soak 42 && just verdict` — all hard gates hold, no canary regression.
- `Feature::EnvironmentalComfortPositive` fires at least once (colony has
  hearth and gardens; cats near them should resolve a positive combined
  modifier).
- `just soak-trace 42 <name>` — `"local_comfort"` and `"local_mystery"`
  scalars appear in L1 records with non-zero values on ticks the cat is near
  known sources.

## Log

- 2026-05-01: Opened. Converted from the `docs/systems/environmental-quality.md`
  stub. Architecture shifted from per-cat rolling window to five influence
  maps after design discussion: maps thread through IAUS as `EvalInput`
  scalars, enabling future DSE location-preference and coordinator axes
  without rearchitecting. Corruption is a 5th map (spatial perception,
  not a new behavior axis — magic system owns the response). Personality
  table locked: comfort→warmth/(1−independence), cleanliness→anxiety,
  beauty→spirituality, mystery→curiosity, corruption→none.
