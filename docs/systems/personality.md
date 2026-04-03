# Personality

## Purpose
Gives each cat a stable, heritable character that modulates needs, action weights, relationship dynamics, and narrative flavor. 18 axes across 3 layers prevent flat archetypes while keeping the system tractable.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| All axes range | 0.0–1.0 | Normalized for easy multiplication as modifiers |
| Generation method | Average of 2 uniform samples | Produces a bell curve centered near 0.5; avoids extreme cats being common |

### Layer 1 — Core Drives (8 traits)
| Trait | Mechanical Effect | Need Axes Scaled |
|-------|------------------|-----------------|
| Boldness | Increases combat action weight, reduces fear response | Safety (recovery faster) |
| Sociability | Increases weight of social actions; drives gathering attendance | Social, Acceptance |
| Curiosity | Increases weight of exploration and magic investigation | Mastery, Purpose |
| Diligence | Speeds skill growth; scales coordinator compliance | Mastery |
| Warmth | Boosts fondness deltas in interactions; increases social contagion | Acceptance, Social |
| Spirituality | Increases magic affinity expression; drives magic action weight | Purpose |
| Ambition | Increases weight of skill-building and leadership-seeking actions | Respect, Mastery |
| Patience | Reduces temper flares; lengthens planning horizon in utility scoring | Safety |

### Layer 2 — Temperament (5 traits)
| Trait | Mechanical Effect | Need Axes Scaled |
|-------|------------------|-----------------|
| Anxiety | Amplifies negative mood modifiers; lowers safety need threshold | Safety |
| Optimism | Shifts mood baseline upward; dampens negative event impact | All (mild) |
| Temper | Increases negative interaction probability when needs are unmet | Safety, Social |
| Stubbornness | Reduces compliance with coordinator directives | — |
| Playfulness | Increases weight of social and leisure actions | Social |

### Layer 3 — Values (5 traits)
| Trait | Mechanical Effect | Need Axes Scaled |
|-------|------------------|-----------------|
| Loyalty | Bonus fondness delta with colony members; compliance with coordinator | Acceptance |
| Tradition | Preference weight toward established locations and routines | Safety |
| Compassion | Increases probability of aid/healing actions toward injured cats | Acceptance |
| Pride | Reduces tolerance for low respect; amplifies respect decay at low values | Respect |
| Independence | Reduces weight of group/coordinator actions; favors solo utility | Purpose |

## Formulas
```
trait_value = (uniform(0,1) + uniform(0,1)) / 2.0

personality_modifier(trait, action_relevance) = 1.0 + trait * 0.5
  (when trait is positively relevant to action)

personality_modifier(trait, action_relevance) = 1.0 - trait * 0.5
  (when trait is negatively relevant, e.g. stubbornness vs. compliance)
```

## Tuning Notes
_Record observations and adjustments here during iteration._
