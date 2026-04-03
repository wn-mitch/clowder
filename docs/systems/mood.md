# Mood

## Purpose
A real-time emotional state layered on top of baseline personality. Affects narrative output, social contagion, and action weights. Distinct from needs (which are slow) — mood swings faster and is socially transmitted.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Mood valence range | -1.0 to 1.0 | Full bipolar scale |
| Baseline valence | 0.2 | Slightly positive default; pessimism requires cause |
| Contagion radius | 3 tiles | Close enough to matter; far enough to model group atmosphere |
| Modifier decay rate | 10% strength lost per tick | Modifiers fade; prevents permanent mood lock from single events |
| Optimism baseline shift | +optimism * 0.4 | Trait directly offsets baseline toward positive |
| Anxiety negative amplifier | modifier * (1.0 + anxiety * 0.5) | Anxious cats feel bad events more strongly |

### Contagion Weight Formula Components
| Component | Formula |
|-----------|---------|
| Proximity weight | 1.0 / distance (tiles) |
| Fondness weight | fondness scaled 0.0–1.0 (fondness + 1.0) / 2.0 |
| Intensity weight | abs(source_mood_valence) |

## Formulas
```
effective_baseline = 0.2 + optimism * 0.4

mood_valence = clamp(effective_baseline + sum(active_modifiers), -1.0, 1.0)

modifier_strength(t+1) = modifier_strength(t) * 0.90

contagion_influence = source_mood_valence
                      * (1.0 / distance)
                      * ((fondness + 1.0) / 2.0)
                      * abs(source_mood_valence)

negative_modifier_effective = modifier * (1.0 + anxiety * 0.5)
```

## Tuning Notes
_Record observations and adjustments here during iteration._
