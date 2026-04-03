# Weather

## Purpose
Creates environmental pressure that shifts action viability, mood, and resource yields. Seasonal probability weights ensure appropriate climate feel across the year.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Duration per weather state | 30–80 ticks | Long enough to matter; short enough to feel dynamic |

### Weather States
| State | Movement Multiplier | Comfort Modifier | Hunting Success Modifier |
|-------|-------------------|-----------------|------------------------|
| Clear | 1.0 | 0 | 0 |
| Overcast | 1.0 | -0.05 | +0.05 (reduced visibility helps cats) |
| LightRain | 0.9 | -0.1 | -0.1 |
| HeavyRain | 0.75 | -0.2 | -0.2 |
| Snow | 0.6 | -0.15 | -0.15 |
| Fog | 0.85 | -0.05 | -0.15 (prey harder to find too) |
| Wind | 0.9 | -0.1 | -0.1 |
| Storm | 0.4 | -0.3 | -0.3 |

### Seasonal Transition Weights
Weights are relative probabilities for next-state selection. Higher values in a season mean that state is more likely to follow any state in that season.

| State | Spring | Summer | Autumn | Winter |
|-------|--------|--------|--------|--------|
| Clear | 3 | 5 | 2 | 1 |
| Overcast | 3 | 2 | 4 | 3 |
| LightRain | 4 | 2 | 3 | 2 |
| HeavyRain | 2 | 1 | 3 | 1 |
| Snow | 0 | 0 | 1 | 5 |
| Fog | 1 | 1 | 2 | 2 |
| Wind | 2 | 2 | 3 | 3 |
| Storm | 1 | 1 | 2 | 1 |

_Note: Exact weights from the plan's `WeatherState::next_weather` function — verify against spec at `~/.claude/plans/structured-napping-candle.md` during implementation._

## Formulas
```
next_weather = weighted_random(states, weights[current_season])

duration = uniform(30, 80) ticks

effective_movement_speed = base_speed * movement_multiplier

comfort_modifier added to mood as a transient modifier
```

## Tuning Notes
_Record observations and adjustments here during iteration._
