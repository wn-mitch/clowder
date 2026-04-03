# Recreation & Grooming

## Purpose
Models the need for varied leisure activity and physical self-maintenance. Cats that do the same thing repeatedly get bored; cats that never play get cranky. Grooming state tracks physical upkeep — well-groomed cats are socially favored, while matted cats suffer mood and health penalties. Both systems are deeply cat-thematic and create personality-driven behavioral variety. Phase 11.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Recreation need decay rate | 0.001/tick | Same tier as social need; builds slowly |
| Recreation need Maslow level | 3 (Belonging) | Alongside social; suppressed when survival is threatened |
| Variety bonus threshold | 3+ distinct types in last 200 ticks | Rewards behavioral diversity |
| Variety mood bonus | +0.1 ("stimulated") | Noticeable but not dominant |
| Repetition penalty | -50% recreation gain per consecutive same-type | Diminishing returns from monotony |
| Grooming state decay rate | 0.002/tick base | Slow; cats stay clean for a while |
| Well-groomed threshold | > 0.7 | Visible positive effects |
| Unkempt threshold | < 0.3 | Visible negative effects |

### Leisure Activities
| Activity | Recreation Gain | Need Satisfied | Personality Affinity | Side Effects |
|----------|----------------|----------------|---------------------|-------------|
| Play-hunting | 0.15 | Recreation + (tiny combat XP) | Playfulness, Boldness | Energy -0.02 |
| Sunbathing | 0.10 | Recreation + Warmth | Patience | Requires Clear + Day + warm tile |
| Climbing | 0.12 | Recreation + Mastery | Curiosity, Boldness | Requires Rock/Watchtower terrain |
| Bird-watching | 0.08 | Recreation + (threat detection) | Curiosity, Patience | Passive scouting bonus while active |
| Self-grooming | 0.05 | Recreation + Grooming state | All (universal need) | Grooming state +0.1 |
| Exploring | 0.10 | Recreation + Mastery | Curiosity | Already exists as action; add recreation |

### Grooming State Effects
| State | Range | Effects |
|-------|-------|---------|
| Pristine | 0.9 – 1.0 | +0.05 mood, +0.02 fondness gain rate (others find cat pleasant) |
| Well-groomed | 0.7 – 0.9 | +0.02 mood |
| Normal | 0.3 – 0.7 | No modifiers |
| Unkempt | 0.1 – 0.3 | -0.05 mood, -0.02 fondness gain rate, skin irritation risk |
| Matted | 0.0 – 0.1 | -0.1 mood, -0.05 fondness, infection risk (if Disease system active) |

### Grooming State Modifiers
| Event | Effect on Grooming State |
|-------|-------------------------|
| Self-grooming action | +0.1 per action completion |
| Social grooming (received) | +0.15 per action completion |
| Rain exposure | -0.02/tick while in rain |
| Mud tile | -0.03/tick while on mud |
| Combat | -0.1 per fight participated in |
| Swimming/water | -0.05 per crossing |

## Formulas
```
recreation_gain(activity, cat):
    base = activity.recreation_value
    if activity.type == cat.last_recreation_type:
        base *= 0.5  # repetition penalty
    if activity.personality_affinity matches cat personality > 0.6:
        base *= 1.3  # personality bonus
    return base

variety_check(cat, window=200):
    distinct_types = count_unique(cat.recreation_history[last 200 ticks])
    if distinct_types >= 3:
        apply_mood_modifier("stimulated", +0.1, 50 ticks)

grooming_decay(cat, weather, terrain):
    base = 0.002
    if weather in (LightRain, HeavyRain): base += 0.02
    if terrain == Mud: base += 0.03
    cat.grooming_state -= base
    cat.grooming_state = cat.grooming_state.clamp(0.0, 1.0)
```

## Tuning Notes
_Record observations and adjustments here during iteration._
