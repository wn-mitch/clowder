# Mental Breaks

## Purpose
Mood thresholds that trigger loss-of-control behavioral episodes when a cat's emotional state deteriorates beyond coping. Creates cascading crises: a break damages relationships or property, which worsens mood for witnesses, which triggers further breaks — the "tantrum spiral." High mood triggers positive inspirations. Phase 3 extension.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Stressed threshold | valence < -0.5 | Early warning; minor scoring penalties only |
| Distressed threshold | valence < -0.7 | Chance of minor break per tick |
| Breaking threshold | valence < -0.9 | Chance of major break per tick |
| Minor break chance | 2% per tick while distressed | Frequent enough to matter; not guaranteed |
| Major break chance | 5% per tick while breaking | High stakes when mood is this low |
| Inspiration threshold | valence > 0.7 | Mirror of distressed; rewards good colony management |
| Inspiration chance | 1% per tick while inspired | Rarer than breaks — good times are quieter |
| Break duration — minor | 20–40 ticks | Long enough to disrupt, short enough to recover from |
| Break duration — major | 40–80 ticks | Serious disruption; colony must cope |

### Minor Break Types
| Break | Behavior | Personality Gate |
|-------|----------|-----------------|
| Sulking | Refuses all actions except Idle/Sleep; seeks isolation (moves away from cats) | Default for low-boldness cats |
| Yowling | Mood contagion amplified 3×; all cats within 4 tiles get -0.1 mood modifier | High temper OR high anxiety |
| Hiding | Retreats to nearest Den; won't emerge until break ends | Low boldness, high anxiety |

### Major Break Types
| Break | Behavior | Personality Gate |
|-------|----------|-----------------|
| Hissing fit | Fondness drops -0.15 with all cats within 3 tiles; triggers social memory | High temper |
| Food gorging | Eats 3× normal from stores; blocks other cats from eating during gorge | Low diligence |
| Territorial spraying | Tiles within 2 get comfort penalty (-0.2) for 50 ticks; narrative event | High independence OR high pride |
| Feral episode | Attacks nearest entity (cat or wildlife); uses combat system | High temper + high boldness |

### Inspiration Types
| Inspiration | Effect | Duration | Personality Gate |
|-------------|--------|----------|-----------------|
| Inspired Hunt | +0.3 hunting skill bonus | 50 ticks | High boldness OR high curiosity |
| Inspired Craft | Next build/remedy/ward action produces superior result | Until next relevant action | High diligence OR high ambition |
| Social Butterfly | Fondness gains doubled in all interactions | 50 ticks | High sociability OR high warmth |

## Formulas
```
break_check(mood):
    if mood.valence < -0.9:
        if random() < 0.05: trigger_major_break(personality)
    elif mood.valence < -0.7:
        if random() < 0.02: trigger_minor_break(personality)

break_type_selection(personality, severity):
    weights = base_weights[severity]
    for (break_type, gate_traits) in break_types:
        weight *= (1.0 + relevant_trait * 0.5)
    return weighted_random(break_types, weights)

inspiration_check(mood):
    if mood.valence > 0.7:
        if random() < 0.01: trigger_inspiration(personality)

# Cascade: witnesses of a break get mood penalty
witness_penalty = -0.15 (minor break) or -0.25 (major break)
witness_radius = 4 tiles
```

## Tuning Notes
_Record observations and adjustments here during iteration._
