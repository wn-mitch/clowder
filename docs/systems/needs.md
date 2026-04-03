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

## Food Variety (Phase 1 Extension)

FoodStores tracks food by type rather than a single number.

### Food Types
| Type | Source | Spoilage Rate | Mood on Repeat |
|------|--------|--------------|----------------|
| Prey | Hunting | 0.003/tick | "same old mouse" -0.05 after 3 consecutive |
| Foraged Plants | Foraging | 0.002/tick | "bland greens" -0.03 after 3 consecutive |
| Fish | Fishing (near water) | 0.004/tick (spoils fast) | "tired of fish" -0.04 after 3 consecutive |

### Variety Bonus
| Condition | Modifier |
|-----------|---------|
| Ate 2+ different types in last 100 ticks | +0.05 mood ("well-fed variety") |
| Ate 3 types in last 100 ticks | +0.1 mood ("excellent diet") |
| Ate same type 3+ times consecutively | -0.03 to -0.05 mood ("bland diet") |

### Freshness
| Freshness | Age (ticks since produced) | Effect |
|-----------|--------------------------|--------|
| Fresh | 0–50 | No modifier |
| Stale | 50–150 | -0.02 mood when eaten |
| Spoiled | 150+ | -0.05 mood; food poisoning risk (if Disease system active) |

## Territory Need (Phase Enhancement)

Cats are territorial. A new need axis at Level 2 (Safety).

### Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Territory establishment | Cat sleeps in same 2×2 area 5+ times | Preferred spot emerges from behavior |
| Displacement penalty | -0.1 mood when preferred spot occupied by another | Mild but persistent |
| Territorial friction | -0.01 fondness/tick with the occupier | Creates interpersonal tension |
| Independence scaling | Penalty × (1.0 + independence × 0.5) | Independent cats care more about territory |
| No-territory baseline | No penalty if cat hasn't established preference yet | New/young cats aren't territorial |

```
territory_check(cat):
    preferred = most_frequent_sleep_location(cat, last 500 ticks)
    if preferred.visit_count >= 5:
        cat.territory = preferred
    if cat.territory AND cat.territory.occupied_by(other_cat):
        cat.mood.add_modifier("displaced", -0.1 * (1.0 + independence * 0.5), 10)
        fondness(cat, other_cat) -= 0.01
```

## Tuning Notes
_Record observations and adjustments here during iteration._
