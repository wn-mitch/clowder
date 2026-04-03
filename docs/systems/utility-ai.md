# Utility AI

## Purpose
Selects the best action for each cat each tick by scoring every candidate action with a composite formula. Replaces hand-coded FSMs with emergent behavior driven by need states, personality, and context.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Jitter range | ±0.05 | Breaks ties and prevents lockstep group behavior |
| Base weight — physiological actions | 2.0 | Ensures survival actions outcompete others when needs are urgent |
| Personality modifier scale | trait_value * 0.5 | Same scale as need system; consistent modifier range |
| Need urgency formula | 1.0 - need_value | Linear; 0 urgency when need is full, 1.0 when empty |

## Formulas
```
score(action) =
    sum_over_needs(need_urgency(n) * weight(n, action) * personality_modifier(action) * suppression(n) * context_modifier)
    + jitter

jitter = uniform(-0.05, 0.05)

need_urgency(n) = 1.0 - need_value(n)

personality_modifier(action) = 1.0 + relevant_trait * 0.5

suppression(n) = smoothstep value of the need level below n
                 (1.0 for physiological needs — never suppressed)

context_modifier = product of applicable situational multipliers
  (e.g., weather, time of day, proximity bonuses from activity cascading)
```

Action selection: the action with the highest score is chosen. Ties are broken by jitter.

## Tuning Notes
_Record observations and adjustments here during iteration._
