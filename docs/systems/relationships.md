# Relationships

## Purpose
Tracks pairwise state between every cat dyad. Drives social action selection, coordinator dynamics, narrative generation, and romantic/mate bonds.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Fondness range | -1.0 to 1.0 | Symmetric valence; negative values enable rivalry/hostility |
| Familiarity range | 0.0 to 1.0 | One-directional accumulation; never decreases passively |
| Romantic range | 0.0 to 1.0 | Separate from fondness; prevents conflation of love types |
| Positive interaction fondness delta | +0.02 per interaction | Small increments require sustained positive history |
| Negative interaction fondness delta | -0.05 per interaction | Negative events weighted 2.5x positive; losses sting more |
| Familiarity growth rate | +0.01 per tick in proximity | Proximity defined as same or adjacent tile/location |
| Value compatibility bonus | +0.01 per aligned value per interaction | Up to +0.05 per interaction if all 5 values align |

### Bond Thresholds
| Bond Type | Conditions |
|-----------|-----------|
| Friends | fondness > 0.3 AND familiarity > 0.4 |
| Partners | romantic > 0.5 AND fondness > 0.6 |
| Mates | romantic > 0.7 AND fondness > 0.7 |

### Romantic Progression Prerequisites
| Prerequisite | Value |
|-------------|-------|
| Orientation compatibility | Must match (resolved via identity system) |
| Fondness threshold | > 0.4 |
| Familiarity threshold | > 0.5 |

## Formulas
```
fondness(t+1) = fondness(t) + interaction_delta + value_compatibility_bonus

value_compatibility_bonus = count(aligned_values) * 0.01

romantic_growth = triggered by high fondness + familiarity meeting prereqs;
  grows via romantic interactions similar to fondness deltas

familiarity(t+1) = min(1.0, familiarity(t) + 0.01)  [when in proximity]
```

## Tuning Notes
_Record observations and adjustments here during iteration._
