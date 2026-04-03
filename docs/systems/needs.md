# Needs

## Purpose
Models a Maslow-style hierarchy of cat needs across 5 levels and 10 axes. Higher-level needs are suppressed until lower-level needs are sufficiently met. Urgency drives action selection via the utility AI.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Hunger decay rate | 0.003/tick | Fastest-decaying physiological need; cats must eat frequently |
| Energy decay rate | 0.002/tick | Slightly slower; sleep needs build over hours |
| Warmth decay rate | 0.001/tick | Slow ambient loss; buildings and weather modulate heavily |
| Safety recovery rate | 0.005/tick | Recovers quickly once threat is gone to avoid action lock |
| Social decay rate | 0.001/tick | Slow; social needs build over time without interaction |
| Acceptance decay rate | 0.0005/tick | Esteem-layer; decays very slowly |
| Respect decay rate | 0.0003/tick | Slower than acceptance; tied to long-term reputation |
| Mastery decay rate | 0.0002/tick | Self-actualization layer; near-negligible passive decay |
| Purpose decay rate | 0.0001/tick | Slowest; existential need, nearly permanent once fulfilled |
| Suppression smoothstep lower edge | 0.15–0.20 | Below this, higher-level needs are fully suppressed |
| Suppression smoothstep upper edge | 0.60–0.70 | Above this, suppression is fully lifted |
| Personality scaling bonus | +0.5 at trait=1.0 | Trait values linearly scale need decay/recovery up to this bonus |

## Formulas
```
need_value(t+1) = need_value(t) - decay_rate * personality_multiplier

suppression(lower, upper, x) = smoothstep(lower, upper, x)
  where smoothstep(a, b, x) = clamp((x - a) / (b - a), 0, 1)^2 * (3 - 2 * clamp(...))

personality_multiplier = 1.0 + relevant_trait * 0.5

need_urgency = 1.0 - need_value
```

Higher-level needs are multiplied by the suppression factor of the level below:
```
effective_urgency(need_L) = urgency(need_L) * suppression_of(level_L-1)
```

## Tuning Notes
_Record observations and adjustments here during iteration._
