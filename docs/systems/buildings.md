# Buildings

## Purpose
Persistent colony infrastructure that provides passive bonuses and enables new actions. Buildings degrade over time and require maintenance, creating ongoing resource and labor investment. Buildings are tile-composed from wall and roof tilesets, giving them visual presence proportional to their mechanical importance.

## Map Size
Default map: **120×90 tiles** (up from 80×60). A mature colony with full building set spans ~30–40 tiles wide, leaving >70% wilderness.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Condition decay — base rate | 0.001/tick | Slow enough that maintenance isn't constant burden |
| Condition decay — bad weather multiplier | 2× base rate | Storm/heavy rain accelerate wear |
| Effect threshold — reduced | < 0.5 condition | Below half, bonuses are halved |
| Effect threshold — non-functional | < 0.2 condition | Near-ruin provides no benefit |
| Effect radius origin | Building center tile | Not anchor (top-left); ensures range covers the full footprint |

### Structure Types
| Structure | Size | Materials | Primary Effects |
|-----------|------|-----------|----------------|
| Den | 3×3 | Wood 10, Stone 6 | Shelter (warmth recovery), safety bonus, sleep quality. Radius 4 from center. |
| Hearth | 4×3 | Stone 12, Wood 5 | Social gathering radius 5 from center, warmth modifier in cold. |
| Stores | 4×3 | Wood 10, Stone 5 | Food preservation (halves spoilage), capacity 50 items. |
| Workshop | 3×3 | Wood 7, Stone 4, Herbs 3 | Skill XP bonus for craftwork actions. |
| Garden | 6×5 | Wood 6 | Crop/herb yield passively each season. Fenced plot, no roof. |
| Watchtower | 2×3 | Wood 8, Stone 8 | 2× threat detection range. Tall structure, no roof. |
| Ward Post | 1×1 | Stone 2, Herbs 3 | Ward radius extension + ward decay reduction bonus. |
| Wall | 1×1 | Stone 3 | Movement barrier (blocks non-colony entities). |
| Gate | 2×1 | Wood 4, Stone 2 | Controllable passage. Animated open/close. |

## Formulas
```
condition(t+1) = condition(t) - decay_rate * weather_multiplier

weather_multiplier = 2.0 if weather in (HeavyRain, Storm) else 1.0

effect_strength =
    1.0   if condition >= 0.5
    0.5   if 0.2 <= condition < 0.5   (linearly scaled in range)
    0.0   if condition < 0.2
```

## Rendering — Tile-Composed Buildings

Buildings are composed from tileset pieces on the 16×16 pixel grid, rendered at TILE_SCALE (3×).

### Assets
| Tileset | File | Size | Purpose |
|---------|------|------|---------|
| Walls | `Tilesets/Building parts/Wooden_House_Walls_Tilset.png` | 80×48 | Corner, edge, and fill pieces for building perimeters |
| Roof | `Tilesets/Building parts/Wooden_House_Roof_Tilset.png` | 112×80 | Shingles, peaks, chimney |
| Door | `Tilesets/Building parts/door animation sprites.png` | 288×32 | Animated open/close (1-tile wide, 2-tile tall) |
| Fences | `Tilesets/Building parts/Fences.png` | 128×64 | Garden borders, Wall segments, Gate frames |
| Furniture | `Tilesets/Building parts/Basic_Furniture.png` | 144×96 | Decorative interior accents (16×16 items) |
| Chests | `Tilesets/Building parts/Chest.png` | 240×96 | Stores decoration, animated open/close |

### Composition layers
1. **Wall layer** (z = terrain + 1): Wall tileset pieces forming the building perimeter.
2. **Interior layer** (z = terrain + 2): Optional furniture/decoration sprites visible through open doors.
3. **Roof layer** (z above cat sprites): Roof tileset pieces extending 1–2 tiles above the ground footprint.

Roofs give visual height — a 3×3 Den renders as 3×3 walls with a peaked roof reaching ~5 tiles tall on screen. Garden and Watchtower have no roof. Wall/Gate use the fence tileset.

## Colony Expansion — Build Pressure

The coordinator decides when the colony needs *new* buildings through a slow-accumulating pressure system. This replaces instant threshold checks with a deliberative process shaped by the coordinator's personality.

### Signals
| Signal | Pressure channel | Detection |
|--------|-----------------|-----------|
| Stores at capacity for extended period | `storage` | StoredItems full for N consecutive checks |
| Cats sleeping outdoors (no Den in range) | `shelter` | Cat does Sleep action with no Den within radius |
| Low social satisfaction despite Hearth | `gathering` | Avg social need < 0.4 among cats near Hearth |
| No Workshop + skilled crafters | `workshop` | ≥2 cats with building skill > 0.5, no Workshop exists |
| Food scarcity + no Garden | `farming` | food_fraction < 0.3 persisting, no Garden exists |
| Wildlife breaching perimeter | `defense` | Hostile wildlife detected within colony radius |

### Coordinator Attentiveness
Derived from the coordinator's personality:
```
attentiveness = diligence * 0.5 + ambition * 0.3 + (1.0 - patience) * 0.2
```

- High diligence → notices problems sooner
- High ambition → expansion-minded, wants the colony to grow
- Low patience → acts on pressure sooner, doesn't wait for problems to resolve

### Pressure Accumulation
```
pressure(t+1) = pressure(t) + base_rate * attentiveness   if signal active
pressure(t+1) = pressure(t) * 0.95                         if signal inactive

action_threshold = 1.0 - attentiveness * 0.3
```

When `pressure > action_threshold`, the coordinator issues a Build directive with a blueprint for the needed structure type.

### Parameters
| Parameter | Value | Rationale |
|-----------|-------|-----------|
| Base accumulation rate | 0.01/eval | ~100 evaluations (~2000 ticks) of persistent signal to reach threshold for inattentive coordinator |
| Decay factor | 0.95 | Pressure halves in ~14 evaluations if signal stops |
| Action threshold range | 0.7–1.0 | Attentive coordinator acts at 0.7, inattentive at 1.0 |
| Evaluation interval | 20 ticks | Same cadence as existing assess_colony_needs |

### Narrative
When pressure crosses the threshold, emit a character-driven narrative line reflecting the coordinator's deliberation. The delay between problem appearing and the coordinator acting is where personality expresses itself.

## Tuning Notes
_2026-04-09: Increased building sizes from 2×2 to 3×3/4×3. Scaled material costs ~proportionally. Effect radii increased and changed to center-based measurement. Map increased to 120×90. Added build-pressure expansion system. All values need playtesting at new scale._
