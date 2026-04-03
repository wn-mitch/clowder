# Buildings

## Purpose
Persistent colony infrastructure that provides passive bonuses and enables new actions. Buildings degrade over time and require maintenance, creating ongoing resource and labor investment.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Condition decay — base rate | 0.001/tick | Slow enough that maintenance isn't constant burden |
| Condition decay — bad weather multiplier | 2× base rate | Storm/heavy rain accelerate wear |
| Effect threshold — reduced | < 0.5 condition | Below half, bonuses are halved |
| Effect threshold — non-functional | < 0.2 condition | Near-ruin provides no benefit |

### Structure Types
| Structure | Size | Materials | Primary Effects |
|-----------|------|-----------|----------------|
| Den | 2×2 | Wood | Shelter (warmth need recovery), safety bonus, sleep quality |
| Hearth | 2×2 | Stone + Wood | Social gathering radius 3 tiles, warmth modifier |
| Stores | 2×2 | Wood | Food preservation (slows food decay), capacity increase |
| Workshop | 2×2 | Wood + Stone | Skill XP bonus for craftwork actions |
| Garden | 3×3 | Labor only | Crop/herb yield passively each season |
| Watchtower | 1×1 | Wood + Stone | 2× threat detection range |
| Ward Post | 1×1 | Stone + Herbs | Ward radius extension + ward decay reduction bonus |
| Wall | 1×1 | Stone | Movement barrier (blocks non-colony entities) |

## Formulas
```
condition(t+1) = condition(t) - decay_rate * weather_multiplier

weather_multiplier = 2.0 if weather in (HeavyRain, Storm) else 1.0

effect_strength =
    1.0   if condition >= 0.5
    0.5   if 0.2 <= condition < 0.5   (linearly scaled in range)
    0.0   if condition < 0.2
```

## Tuning Notes
_Record observations and adjustments here during iteration._
