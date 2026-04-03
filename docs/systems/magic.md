# Magic

## Purpose
Models folk/hedge/trained magic as a rare, risky, and socially significant capability. Corruption is an environmental and personal hazard. Misfires create narrative tension. Herb-based effects are the accessible tier.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Affinity distribution — low tier | 80% of cats get 0.0–0.2 | Magic is rare; most cats have negligible affinity |
| Affinity distribution — mid tier | 15% of cats get 0.3–0.6 | Hedge magic practitioners |
| Affinity distribution — high tier | 5% of cats get 0.7–1.0 | Trained/gifted; colony-defining individuals |
| Misfire threshold | skill < affinity * 0.8 | Misfires when underskilled relative to power |
| Misfire probability | max(0, (1.0 - magic_skill / affinity) * 0.5) | Scales with skill gap; 0 when skilled enough |
| Corruption spread rate | 0.001/tick to adjacent tiles | Slow enough to be manageable; fast enough to require action |
| Ward strength at creation | 1.0 | Full potency on placement |
| Ward decay — basic | 0.005/tick | Needs regular renewal |
| Ward decay — durable | 0.001/tick | Longer-lasting; requires more skill/materials |

### Herb Effect Values
| Preparation | Effect | Magnitude |
|-------------|--------|-----------|
| Healing poultice | Health restoration | +0.3 health |
| Energy tonic | Energy restoration | +0.2 energy |
| Mood tonic | Mood valence boost | +0.3 mood |

### Personal Corruption Effects
| Effect | Formula |
|--------|---------|
| Mood instability | valence jitter scaled by corruption level |
| Social penalty | -0.1 * corruption_level applied to fondness deltas |

## Formulas
```
misfire_probability = max(0.0, (1.0 - magic_skill / affinity) * 0.5)
  (only evaluated when magic_skill < affinity * 0.8)

corruption(tile, t+1) = corruption(tile, t) + 0.001
  (for each adjacent corrupted tile per tick)

ward_strength(t+1) = ward_strength(t) - decay_rate

mood_jitter_from_corruption = uniform(-corruption, corruption)

fondness_delta_modifier = fondness_delta * (1.0 - 0.1 * corruption_level)
```

## Tuning Notes
_Record observations and adjustments here during iteration._
