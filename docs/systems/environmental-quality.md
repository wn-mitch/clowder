# Environmental Quality

## Purpose
Creates ambient, always-on mood pressure from a cat's physical surroundings.
Unlike event-driven mood modifiers, environmental quality is a persistent
background force — a well-maintained colony slowly lifts mood, while squalor
grinds it down. This gives players a reason to invest in infrastructure beyond
direct mechanical bonuses.

**Implementation ticket:** [`docs/open-work/tickets/101-environmental-quality-influence-maps.md`](../open-work/tickets/101-environmental-quality-influence-maps.md)

## Architecture

Five tile-resolution influence maps, each a flat `Vec<f32>`. Sources stamp
influence outward with linear radial falloff; cats sample their position each
tick as `EvalInput` scalars that thread through the IAUS like any other
consideration axis.

| Map | Sources | Personality scaling |
|-----|---------|---------------------|
| Comfort | terrain ease, building proximity, weather | `warmth` / `(1 − independence)` |
| Cleanliness | corpses, mud, dirty buildings | `anxiety` |
| Beauty | fairy rings, gardens, standing stones, deep pools | `spirituality` |
| Mystery | `Tile.mystery` radiated outward | `curiosity` |
| Corruption | `Tile.corruption` radiated outward | — (magic system owns response) |

Maps are rebuilt every tick by `update_env_quality_maps` (registered after
`decay_building_condition`). The sweep is a single pass: clear → terrain loop
over `TileMap` → buildings query → unburied-dead query → weather overlay →
clamp to `[−1.0, 1.0]`. Sources change slowly (building decay, dead entities,
weather phase shifts), so a tighter cadence would buy little; matching the
precedent of every existing influence-map writer keeps the substrate visible
to soak-trace verification on every recorded tick. Resolution is true tile
(120×90 grid, `bucket_size = 1`) so the 1–3 tile stamping radii produce
meaningful spatial gradients rather than step functions.

The `Feature::EnvironmentalComfortPositive` / `Negative` canaries are emitted
by a companion system `emit_env_quality_features` that runs immediately after
the sweep. It mirrors the modifier's combine math, samples each living cat's
tile, and records the feature when at least one cat clears
`feature_emit_threshold`. The companion exists because `ScoreModifier::apply`
is pure (no `SystemActivation` access) — see the 101 plan for the design
rationale.

Corruption's map is spatial perception infrastructure — cats sense the gradient
before stepping on a hot tile. The magic system's behavioral response (health
drain, mood penalty, erratic action) is unchanged.

## EvalInput Scalars

`"local_comfort"`, `"local_cleanliness"`, `"local_beauty"`, `"local_mystery"`,
`"local_corruption"` — resolved in `ctx_scalars` by sampling the map at the
cat's position. Any DSE or modifier can reference these as consideration axes
without additional plumbing.

## Modifier Formula

`EnvironmentalQualityModifier` registers in `default_modifier_pipeline` after
`ThermalDistress`. It applies a per-cat additive shift to a curated set of
*stay-and-engage* DSEs (Sleep / Idle / GroomSelf / GroomOther / Socialize) —
the modifier targets a subset of DSEs because softmax is shift-invariant on a
global lift, so an "ambient quality" shift only changes behaviour if it lifts
some DSEs relative to others. Movement DSEs (Wander, Explore, Patrol, Forage,
Hunt, Flee, Hide) are unaffected at first land; future tickets can broaden
the affected DSE set if soak data warrants it. The combine math reads the
four mood-relevant maps with personality scaling:

```
comfort_contrib     = local_comfort     × (1.0 + warmth × 0.3) × (1.0 − independence × 0.2)
cleanliness_contrib = local_cleanliness × (1.0 + anxiety × 0.4)
beauty_contrib      = local_beauty      × (1.0 + spirituality × 0.4)
mystery_contrib     = local_mystery     × (1.0 + curiosity × 0.4)

combined = clamp(sum × combination_weight, −0.3, +0.3)
```

All factors are `EnvironmentalQualityConstants` knobs in `SimConstants`.

## Source Values (Initial)

### Terrain → Comfort
| Terrain | Comfort |
|---------|---------|
| FairyRing | +0.3 |
| LightForest | +0.1 |
| DenseForest | +0.05 |
| Grass | 0.0 |
| Sand | −0.05 |
| Rock | −0.1 |
| Mud | −0.15 |

### Terrain → Beauty
| Terrain | Beauty | Radius |
|---------|--------|--------|
| FairyRing | +0.4 | 3 tiles |
| StandingStone | +0.25 | 2 tiles |
| Garden | +0.20 | 2 tiles |
| DeepPool | +0.15 | 2 tiles |
| AncientRuin | −0.10 | on-tile |

High corruption suppresses beauty: `−tile.corruption × 0.2` applied during
the terrain sweep.

### Building → Comfort
| Building | Peak bonus | Radius | Scales with |
|----------|-----------|--------|-------------|
| Hearth | +0.25 | 3 tiles | `condition` |
| Den | +0.20 | 2 tiles | `condition` |
| Garden | +0.15 | 2 tiles | `condition` |
| Workshop | +0.10 | 1 tile | `condition` |
| Stores | +0.05 | 1 tile | `condition` |
| WardPost | +0.05 | 1 tile | — |

### Cleanliness Sources
| Source | Penalty | Radius |
|--------|---------|--------|
| Unburied corpse | −0.4 | 3 tiles |
| Dirty building (`cleanliness < threshold`) | `−(1 − cleanliness)` scaled | building radius |
| Mud terrain | −0.15 | on-tile |

## Future Extensions

- **DSE location preference** — cats choose *where* to sleep, groom, or linger
  based on map values. `get(x, y)` at arbitrary positions is supported from
  day one; DSE wiring is a separate ticket.
- **Coordinator axes** — low colony-average beauty → motivate garden
  construction; high average filth → escalate burial priority.
- **Monument contributions** — beauty source when ticket 021 lands.
- **`CorruptionLandmarks` retirement** — centroid derivable from the corruption
  influence map; retire as a follow-on.
- **Snow-depth per tile** — promote from weather global overlay to per-tile
  stamp when `Tile.snow_depth` exists.

## Tuning Notes
_Record observations and adjustments here during iteration._
