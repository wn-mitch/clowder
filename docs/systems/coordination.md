# Coordination

## Purpose
Models emergent leadership without explicit hierarchy. High-social-weight cats become informal coordinators whose directives influence others' utility scores — but compliance is voluntary and personality-dependent.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Coordinator selection count | Top 1–2 cats | Small enough to matter; large enough for redundancy |
| Re-evaluation interval | Every 50 ticks | Stable enough to feel real; frequent enough to shift with events |
| Directive utility bonus | coordinator_social_weight * fondness_with_coordinator * 1.5 | Friendship amplifies influence significantly |
| Significant event weight | 0.5 per event | Events in shared memory add social capital |
| Compliance modifier — diligence | target's diligence * fondness | Diligent cats follow those they trust |
| Competing directive resolution | Follow higher-fondness coordinator | Simple, interpretable tiebreak |

### Social Weight Formula Components
| Component | Formula |
|-----------|---------|
| Positive fondness sum | sum(max(0, fondness) for each relationship) |
| Familiarity contribution | avg(familiarity) * colony_size |
| Significant events | count(significant_shared_events) * 0.5 |

## Formulas
```
social_weight(cat) =
    sum(max(0, fondness[cat][other]) for other in colony)
    + avg(familiarity[cat]) * colony_size
    + significant_events_count * 0.5

coordinator_score(cat) = social_weight(cat) * diligence(cat) * sociability(cat)

directive_utility_bonus = coordinator_social_weight * fondness(target, coordinator) * 1.5

compliance_modifier = diligence(target) * fondness(target, coordinator)
```

## Tuning Notes
_Record observations and adjustments here during iteration._
