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

## Personality Gating (Phase Enhancement)

Extreme personality values don't just scale action scores — they gate behaviors entirely. A profoundly timid cat can't be made to fight; a deeply antisocial cat won't voluntarily socialize.

### Behavior Gates
| Gate | Trait Condition | Effect |
|------|----------------|--------|
| Too timid to fight | Boldness < 0.1 | Cannot select Fight action; always Flees from threats |
| Too shy to socialize | Sociability < 0.15 | Cannot voluntarily select Socialize; still responds if another cat initiates |
| Compulsive explorer | Curiosity > 0.9 | 20% chance per tick to ignore coordinator directive in favor of Explore |
| Stubborn refusal | Stubbornness > 0.85 | 30% chance to reject any coordinator directive outright |
| Reckless bravery | Boldness > 0.9 | Cannot select Flee; always Fights (even when outnumbered) |
| Compulsive helper | Compassion > 0.9 | Overrides current action to aid injured cat within 3 tiles |

### Personality Incompatibility
Cats with opposing extreme values generate automatic social friction when in proximity.

| Axis A | Axis B | Condition | Effect |
|--------|--------|-----------|--------|
| Tradition > 0.8 | Independence > 0.8 | Both cats within 3 tiles | Fondness -0.002/tick (passive friction) |
| Diligence > 0.8 | Playfulness > 0.8 | Both cats within 3 tiles | Fondness -0.001/tick |
| Loyalty > 0.8 | Independence > 0.8 | During coordinator directive | Loyal cat resents independent cat's non-compliance |
| Ambition > 0.8 | Ambition > 0.8 | Both eligible for coordinator | Rivalry: fondness -0.003/tick |

```
behavior_gate_check(cat, proposed_action):
    if proposed_action == Fight AND cat.boldness < 0.1:
        return Flee  # Override
    if proposed_action == Socialize AND cat.sociability < 0.15:
        return None  # Skip; score next action
    if proposed_action == Flee AND cat.boldness > 0.9:
        return Fight  # Override
    return proposed_action  # No gate triggered

incompatibility_check(cat_a, cat_b):
    friction = 0.0
    if cat_a.tradition > 0.8 AND cat_b.independence > 0.8:
        friction += 0.002
    if cat_a.diligence > 0.8 AND cat_b.playfulness > 0.8:
        friction += 0.001
    # Symmetric check
    fondness(cat_a, cat_b) -= friction
    fondness(cat_b, cat_a) -= friction
```

## Tuning Notes
_Record observations and adjustments here during iteration._
