# Environmental Quality

## Purpose
Creates ambient, always-on mood pressure from a cat's physical surroundings. Unlike event-driven mood modifiers, environmental quality is a persistent background force — a well-maintained colony slowly lifts mood, while squalor grinds it down. This gives players a reason to invest in infrastructure beyond direct mechanical bonuses. Phase 3 extension.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Comfort rolling window | 20 ticks | Long enough to smooth jitter; short enough to feel responsive |
| Max positive modifier | +0.3 mood | Strong enough to matter; not dominant over events |
| Max negative modifier | -0.3 mood | Symmetric with positive |
| Overcrowding radius | 2 tiles | Cats in close quarters feel cramped |
| Overcrowding threshold | 4+ cats within radius | Small colony norm; above this feels crowded |
| Corpse comfort penalty | -0.4 per corpse within 3 tiles | Strong aversion; drives burial behavior |
| Corruption comfort penalty | -0.2 × tile corruption level | Scales with severity |

### Terrain Base Comfort
| Terrain | Base Comfort |
|---------|-------------|
| Fairy Ring | +0.3 |
| Light Forest | +0.1 |
| Dense Forest | +0.05 |
| Grass | 0.0 |
| Sand | -0.05 |
| Mud | -0.15 |
| Rock | -0.1 |
| Water | 0.0 (not usually occupied) |

### Building Comfort Contribution
| Building | Comfort Bonus | Radius | Condition Scaling |
|----------|--------------|--------|-------------------|
| Den | +0.2 | 2 tiles | Linear with condition |
| Hearth | +0.25 | 3 tiles | Linear with condition |
| Stores | +0.05 | 1 tile | Linear with condition |
| Workshop | +0.1 | 1 tile | Linear with condition |
| Garden | +0.15 | 2 tiles | Linear with condition |
| Ward Post | +0.05 | 1 tile | — |
| Wall | 0.0 | — | — |

### Negative Modifiers
| Source | Penalty | Range |
|--------|---------|-------|
| Unburied corpse | -0.4 | 3 tiles |
| Corrupted tile (>0.3) | -0.2 × corruption | On tile |
| Overcrowding | -0.05 per cat above threshold | 2 tiles |
| Mud | -0.15 | On tile |
| Snow (deep, >0.5) | -0.05 | On tile |

## Formulas
```
tile_comfort(x, y) =
    terrain_base(tile.terrain)
    + sum(nearby_building_bonuses * building.condition)
    - sum(nearby_negative_penalties)

cat_comfort_average(cat) = rolling_mean(tile_comfort(cat.position), last 20 ticks)

mood_modifier = clamp(cat_comfort_average * 0.5, -0.3, +0.3)

personality_scaling:
    modifier *= (1.0 + warmth * 0.3)      # warm cats care more about comfort
    modifier *= (1.0 - independence * 0.2)  # independent cats care less
```

## Tuning Notes
_Record observations and adjustments here during iteration._
