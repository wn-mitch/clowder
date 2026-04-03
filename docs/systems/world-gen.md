# World Generation

## Purpose
Procedurally generates the map tile grid using layered Perlin/simplex noise for elevation and moisture. Places the colony site and special locations with minimum spacing rules to ensure playable and interesting layouts.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Map size | 80×60 tiles | Large enough for multiple locations; fits a terminal at reasonable zoom |
| Noise scale | 0.05 | Produces features at a 20-tile wavelength; large regions, not noise |
| Elevation threshold — water | < -0.3 | ~15% of map water at standard distribution |
| Elevation threshold — rock | > 0.6 | ~20% rocky impassable terrain |
| Moisture threshold — forest | > 0.3 | Wet areas become forest |
| Moisture threshold — dry (mud/sand) | < -0.2 | Dry lowlands become mud or sand |

### Colony Site Criteria
| Criterion | Value |
|-----------|-------|
| Center tile | Must be passable |
| Passable area | > 80 passable tiles within 11×11 area (121 tiles) |
| Water proximity | Water source within 10 tiles |

### Special Locations
| Type | Minimum Spacing from Others | Notes |
|------|-----------------------------|-------|
| Fairy ring | 15 tiles from other specials | Magic affinity/narrative site |
| Standing stone | 15 tiles from other specials | Magic/ritual site |
| Deep pool | 15 tiles from other specials | Resource and narrative site |
| Ancient ruin | 15 tiles from other specials | Exploration/lore site |
| Corruption sources | > 30 tiles from colony | Prevents instant corruption threat at start |

## Formulas
```
elevation(x, y) = perlin(x * 0.05, y * 0.05)  [normalized -1 to 1]
moisture(x, y)  = perlin((x + offset) * 0.05, (y + offset) * 0.05)

tile_type:
  elevation < -0.3             → Water
  elevation > 0.6              → Rock
  moisture > 0.3               → Forest
  moisture < -0.2              → Mud / Sand
  otherwise                    → Grass / Open

colony_valid(cx, cy) =
    passable(cx, cy)
    AND count(passable tiles in rect(cx-5, cy-5, 11, 11)) > 80
    AND exists water tile within manhattan_distance(10)

special_location_valid(x, y, placed) =
    distance(x, y, other) >= 15 for all other in placed
    AND (not corruption_source OR distance(x, y, colony) > 30)
```

## Tuning Notes
_Record observations and adjustments here during iteration._
