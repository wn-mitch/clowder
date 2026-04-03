# Activity Cascading

## Purpose
Models social contagion in work and play — cats nearby doing the same thing make an action more attractive and more effective. Creates emergent coordinated behavior without explicit task assignment.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Proximity radius | 5 tiles | Close enough to visually cluster; far enough to include adjacent work sites |
| Cascading utility bonus per nearby cat | +0.15 | Meaningful boost; 3 cats nearby nearly doubles the base social weight |

### Complementary Action Pairs
| Action A | Action B | Cascade Type |
|----------|----------|-------------|
| Hunt | Hunt | Group hunt |
| Build | Build | Build crew |
| Socialize | Socialize | Gathering |

### Group Effect Bonuses
| Effect | Formula |
|--------|---------|
| Group hunt success | base_success + 0.1 per additional hunter |
| Build crew speed | 1.0 + 0.3 per additional builder |
| Gathering (social need) | Social need satisfaction rate scaled by participant count |

## Formulas
```
nearby_same_action_count = count(cats within 5 tiles doing same action)

cascading_utility_bonus = nearby_same_action_count * 0.15

action_score(action) += cascading_utility_bonus  [applied in utility AI context step]

group_hunt_success = base_hunt_success + 0.1 * (hunter_count - 1)

build_speed = 1.0 + 0.3 * (builder_count - 1)
```

## Tuning Notes
_Record observations and adjustments here during iteration._
