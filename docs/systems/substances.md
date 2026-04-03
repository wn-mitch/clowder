# Substances

## Purpose
Models catnip and valerian as naturally-occurring substances with euphoric or calming effects, tolerance buildup, dependence, and withdrawal. Creates personality-driven addiction dynamics where playful, low-diligence cats are vulnerable while diligent cats resist. Corrupted variants act as "hard drugs" with stronger effects and faster addiction. Phase 6 / Phase 12b.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Catnip spawn terrains | LightForest, FairyRing | Thematic; findable but not everywhere |
| Catnip spawn density | 1 per 200 eligible tiles | Rare enough to create competition |
| Valerian spawn terrains | Grass near Water | Different niche than catnip |
| Valerian spawn density | 1 per 300 eligible tiles | Rarer than catnip |
| Safe use interval | 200 ticks between uses | Below this, tolerance builds |
| Tolerance buildup rate | +0.1 per use within safe interval | Gradual; ~5 uses to full tolerance |
| Dependence threshold | tolerance > 0.5 | Halfway to full tolerance triggers craving |
| Withdrawal mood penalty | -0.3 to -0.5 (scales with dependence) | Severe enough to drive seeking behavior |
| Withdrawal duration | 300 ticks after last use | Long enough to be painful |

### Substance Effects
| Substance | Immediate Effect | Duration | Side Effects |
|-----------|-----------------|----------|-------------|
| Catnip | Euphoria (+0.4 mood), silly behavior (random movement, pouncing at nothing) | 20 ticks | Energy +0.1 |
| Catnip (tolerant) | Reduced euphoria (+0.2 mood), same silly behavior | 15 ticks | Energy +0.05 |
| Corrupted Catnip | Strong euphoria (+0.6 mood), erratic behavior | 30 ticks | +0.05 corruption per use |
| Valerian | Calm (+0.2 mood), anxiety suppressed | 30 ticks | Energy -0.1 (lethargy) |
| Corrupted Valerian | Deep calm (+0.4 mood), near-catatonia | 50 ticks | +0.03 corruption, -0.2 energy |

### Personality Susceptibility
| Trait Combination | Effect |
|-------------------|--------|
| Playfulness > 0.7 AND Diligence < 0.3 | 2× likelihood to seek catnip when available |
| Diligence > 0.7 | 0.5× likelihood; resists even when craving |
| Anxiety > 0.7 | 2× likelihood to seek valerian |
| Stubbornness > 0.7 | Withdrawal is milder (stubbornness as resilience) |
| Independence > 0.7 | Ignores coordinator attempts to restrict access |

## Formulas
```
tolerance(t+1) =
    if used_this_tick AND ticks_since_last_use < 200:
        tolerance(t) + 0.1
    else:
        (tolerance(t) - 0.005).max(0.0)   # slow natural recovery

is_dependent = tolerance > 0.5

craving_urgency = if is_dependent: (1.0 - tolerance_satisfied) * dependence_level

withdrawal_mood = if is_dependent AND ticks_since_last_use > 50:
    -0.3 * dependence_level * (1.0 - stubbornness * 0.3)

euphoria_effectiveness = base_effect * (1.0 - tolerance * 0.5)

seek_probability = base * playfulness * (1.0 - diligence) * craving_urgency
```

## Tuning Notes
_Record observations and adjustments here during iteration._
