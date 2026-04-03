# Weather — Living Climate System

## Purpose
A continuous, physically-grounded climate simulation that the cats experience as a real world. Temperature is a continuous variable driven by season, time of day, and weather state. Precipitation accumulates and changes the map. Cats sense barometric pressure shifts and react instinctively to approaching storms. Wind direction affects scent-based hunting and threat detection. Extreme events — cold snaps, blizzards, droughts — create colony-level crises. Shelter is the difference between life and death.

## Current Implementation
8 weather types (Clear, Overcast, LightRain, HeavyRain, Snow, Fog, Wind, Storm) cycling via seasonal probability tables. Affects warmth drain and building decay. `movement_multiplier()` and `comfort_modifier()` are defined but **not wired up**.

---

## 1. Temperature — Continuous Variable

Replace the implicit "cold weather = bad" boolean with per-tick ambient temperature.

### Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Temperature unit | "paw-degrees" (arbitrary scale) | Avoids false precision of Celsius/Fahrenheit |
| Cat comfort range | 5 – 25 | Cats comfortable in mild conditions |
| Cold stress threshold | < 5 | Warmth need decays faster below this |
| Hypothermia threshold | < -2 | Health drain begins |
| Heat stress threshold | > 25 | Energy decays faster above this |
| Heatstroke threshold | > 35 | Health drain begins |

### Seasonal Baselines
| Season | Min | Max | Midpoint |
|--------|-----|-----|----------|
| Spring | 8 | 18 | 13 |
| Summer | 15 | 30 | 22 |
| Autumn | 5 | 15 | 10 |
| Winter | -5 | 8 | 1.5 |

Baseline interpolates sinusoidally within each season (coldest at season start, warmest at midpoint, cooling toward end — except summer peaks at midpoint).

### Diurnal Cycle
| Day Phase | Temp Modifier |
|-----------|--------------|
| Dawn | -3 |
| Day | +2 |
| Dusk | -1 |
| Night | -4 |

### Weather Modifiers
| Weather | Temp Modifier | Notes |
|---------|--------------|-------|
| Clear | +2 | Radiative heating (day) / cooling (night) amplified |
| Overcast | +1 | Cloud insulation moderates |
| LightRain | -1 | Evaporative cooling |
| HeavyRain | -3 | Significant cooling |
| Snow | -5 | Cold precipitation |
| Fog | 0 | Moisture traps heat near ground |
| Wind | -2 | Wind chill |
| Storm | -4 | Cold + wind + rain |

### Per-Tile Temperature Modifiers
| Source | Modifier | Notes |
|--------|----------|-------|
| Inside building (Den/Stores/Workshop) | +5 | Insulation |
| Near functional Hearth (≤3 tiles) | +3 | Active warmth source |
| DenseForest canopy | +2 | Wind shelter + insulation |
| Water tiles | Pull toward 12 | Moderating effect; reduces extremes |
| Rock tiles (summer daytime) | +3 | Thermal mass radiates stored heat |
| Rock tiles (night) | -1 | Cold stone |

### Formulas
```
ambient_temp = seasonal_baseline(season, day_of_season)
             + diurnal_modifier(day_phase)
             + weather_modifier(current_weather)
             + uniform(-1.0, 1.0)  # natural jitter

effective_temp(cat) = ambient_temp + tile_modifier(cat.position)

warmth_drain(cat):
    if effective_temp < 5:
        excess_cold = 5.0 - effective_temp
        drain = 0.001 + excess_cold * 0.001  # proportional to cold severity
    else:
        drain = 0.001  # base drain

    if effective_temp < -2:
        health_drain = 0.005 * ((-2.0 - effective_temp) / 10.0)  # hypothermia

energy_drain(cat):
    if effective_temp > 25:
        excess_heat = effective_temp - 25.0
        drain = 0.002 + excess_heat * 0.0005
    if effective_temp > 35:
        health_drain = 0.003  # heatstroke
```

---

## 2. Barometric Pressure & Storm Sensing

Cats detect dropping pressure through their inner ears (confirmed by 2025 vestibular neuron study).

### Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Pressure trend states | Rising, Stable, Falling | Simple tri-state |
| Pre-storm detection window | 10 ticks before Storm/HeavyRain transition | Cats sense it before it happens |
| Storm-sense shelter bonus | +0.2 to shelter-seeking action scores | Drives instinctive hiding |
| Restlessness modifier (anxious cats) | -0.05 mood | Anxiety amplifies pressure discomfort |
| Post-storm relief modifier | +0.1 mood for 5 ticks | Brief euphoria when pressure stabilizes |

### Cat Behaviors
| Behavior | Trigger | Mechanic |
|----------|---------|---------|
| Ear-washing | Pressure falling | Grooming action probability 2×; narrative: "{name} washes behind her ears — rain is coming" |
| Shelter-seeking | Pressure falling AND boldness < 0.5 | +0.2 to all shelter/Den-seeking action scores |
| Ignoring signs | Pressure falling AND boldness > 0.8 | No behavioral change; narrative: "{name} ignores the darkening sky" |
| Post-storm stretch | Pressure rising after storm | +0.1 mood; narrative: "{name} stretches in the clearing air" |

---

## 3. Wind Direction & Scent

### Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Wind directions | 8 (N/NE/E/SE/S/SW/W/NW) | Cardinal + intercardinal |
| Direction change | On weather transition | Wind shifts with weather |
| Upwind hunting penalty | -30% detection range | Prey smells hunter approaching |
| Downwind hunting bonus | +30% detection range | Hunter smells prey first |
| Upwind threat detection penalty | -3 tile threat range | Predator approaches from downwind undetected |
| Downwind threat detection bonus | +5 tile threat range | Wind carries predator scent to cats |

### Wind Speed by Weather
| Weather | Speed | Scent Effect |
|---------|-------|-------------|
| Clear | Calm | No directional modifier |
| Overcast | Light | ±10% detection |
| LightRain | Light | ±10% + rain washes scent trails |
| HeavyRain | Moderate | Rain dominates; scent hunting -40% |
| Snow | Light-Moderate | Snow muffles; scent -20% |
| Fog | Calm | Scent trapped: +20% at ≤3 tiles, -50% beyond |
| Wind | Strong | Full ±30% directional modifier |
| Storm | Gale | Scent useless; hunting -60% |

### Shelter Interaction
Buildings and walls on the windward side provide extra warmth bonus. Leeward side of dense forest counts as sheltered from wind chill.

---

## 4. Precipitation Accumulation & Terrain Effects

Weather changes the map.

### Snow Accumulation
| Parameter | Value | Notes |
|-----------|-------|-------|
| Accumulation rate | +0.01/tick during Snow weather | On outdoor tiles only (not under building/dense canopy) |
| Movement penalty | +20% cost at depth > 0.3; +50% at > 0.6 | Deeper snow = harder travel |
| Kitten impassable | Depth > 0.9 | Small bodies can't push through deep snow |
| Foraging penalty | Effectiveness × (1.0 - snow_depth) | Snow covers food sources |
| Track visibility | Cats and wildlife leave tracks on snowy tiles | +20% hunting accuracy from tracks; predators track cats too |
| Melt rate | -0.005/tick when temp > 5 sustained | Creates puddles on low terrain |
| First snowfall | Annual narrative event | Kittens get +mood from novelty (playfulness-gated) |

### Rain & Mud
| Parameter | Value | Notes |
|-----------|-------|-------|
| Mud creation | HeavyRain for 30+ ticks on Grass/Dirt | Terrain becomes Mud |
| Mud movement penalty | +30% cost | Cats hate mud |
| Mud grooming penalty | -0.03/tick grooming state | Mud on paws |
| Mud foraging bonus | +10% (worms/insects surface) | Silver lining |
| Mud drying | Clear/Wind for 30+ ticks | Reverts to base terrain |
| Puddle formation | Low tiles after heavy rain | Water source; attracts wildlife; freezes in winter |

### Freezing
| Parameter | Value | Notes |
|-----------|-------|-------|
| Freeze threshold | Temp < 0 for 10+ sustained ticks | Water tiles → Ice |
| Ice properties | Passable but slippery (movement jitter) | Fall risk for injured/elder cats |
| Lost water source | Frozen water can't be drunk | Forces alternatives |
| Thaw threshold | Temp > 3 for sustained ticks | Ice → Water; narrative: "the ice cracks and groans" |

### Drought
| Parameter | Value | Notes |
|-----------|-------|-------|
| Drought trigger | 50+ ticks of Clear weather in Summer | Soil dries |
| Crop growth penalty | -50% | Farms suffer |
| Herb spawning | Suspended during drought | No new herbs |
| Foraging yield | -40% | Dry earth yields less |
| Fire risk | Lightning (Storm) can ignite dry grass tiles | Fire spreads to adjacent dry tiles; cats flee; buildings can burn |

---

## 5. Extreme Weather Events

Rare multi-day events that create colony-level challenges.

| Event | Trigger | Duration | Effects |
|-------|---------|----------|---------|
| Cold Snap | Winter, 5% chance per season | 200–400 ticks | Temp -10 from baseline; hypothermia risk outdoors; water freezes; 2× snow accumulation |
| Heat Wave | Summer, 5% chance per season | 200–400 ticks | Temp +10; heatstroke risk; crops wilt (-50%); food spoils 2× faster |
| Blizzard | Winter Storm escalation | 50–100 ticks | Temp -8; visibility near-zero; 0.2× movement; outdoor cats risk death; 3× building decay |
| Thunderstorm | Summer/Autumn Storm escalation | 30–60 ticks | Lightning strikes random tile: ignites dry grass, terrifies cats (safety spike), can damage watchtower |
| Fog Bank | Autumn/Spring Fog escalation | 80–150 ticks | Visibility 3 tiles; pathfinding fails beyond 3 tiles (cats wander lost); predators undetected |
| First Frost | Late Autumn, annual | 1 tick | All crops die; herbs wither; narrative milestone; colony mood depends on food stores |
| Thaw | Early Spring, annual | 20–30 ticks | Ice melts; snow recedes; first herbs spawn; colony mood boost |

---

## 6. Cat-Specific Weather Behaviors

Real feline behaviors translated to simulation mechanics.

| Behavior | Real Basis | Mechanic |
|----------|-----------|---------|
| Ear-washing before rain | Inner ear pressure sensitivity | Grooming probability 2× when pressure falling |
| Sleeping more in winter | Energy conservation, melatonin | Energy decay +30% in winter; Sleep scores higher |
| Sunbathing | Warmth-seeking | Clear + Day → rock/open tiles get "sun patch" marker; cats seek them |
| Huddling | Social thermoregulation | Temp < 3 + within 1 tile of another cat: warmth drain halved; fondness +0.002/tick |
| Rain aversion | Cats dislike wet fur | In rain: grooming -0.1, mood -0.05, comfort -0.1; shelter-seeking unless bold |
| Fireside sitting | Heat-seeking to excess | Cold weather: cats crowd Hearth; adjacent too long → singed-fur narrative |
| Back-to-fire posture | Folklore prediction | Cat facing away from Hearth → narrative hint of approaching cold (flavor only) |

---

## 7. Shelter Effectiveness

Activates the existing `shelter_value` per terrain (currently defined but unused in `map.rs`).

### Shelter Values
| Terrain | Shelter | Notes |
|---------|---------|-------|
| Den / Stores / Workshop | 0.8 – 1.0 | Full protection |
| DenseForest | 0.6 | Canopy blocks most rain/snow |
| LightForest / Watchtower | 0.3 | Partial cover |
| Open (Grass / Rock / Sand) | 0.0 | Fully exposed |

### Shelter Effects
```
effective_temp(cat) = ambient_temp + shelter_value * 5.0  # up to +5 in full shelter

precipitation_exposure = 1.0 - shelter_value
    # Controls: grooming state loss from rain, snow accumulation underneath

wind_chill_reduction = shelter_value
    # 1.0 shelter = no wind chill applied
```

In a blizzard at -10, a cat in a Den (shelter 1.0) feels -10 + 5 = -5. A cat in the open feels -10. The difference between discomfort and death.

---

## 8. System Integration

| System | Current | Expanded |
|--------|---------|----------|
| Needs (warmth) | Weather enum → fixed drain | Continuous temp → proportional drain |
| Needs (energy) | No weather effect | Heat stress → faster energy drain |
| Buildings (decay) | Multiplier 1.0/1.5/2.0 | + snow load, moisture damage, lightning fire |
| AI (scoring) | movement_multiplier unused | Temperature drives shelter-seeking; rain drives grooming; storm drives hiding |
| Combat | None | Wind affects ambush; snow slows pursuit; fog limits detection |
| Farming | None | Snow cover, drought, frost kills crops, rain aids growth |
| Narrative | Template matching | + extreme events, seasonal milestones, cat weather-sensing |
| Magic | None | Storm amplifies corruption spread; lightning hits ward posts |
| Coordination | None | Coordinator issues weather-prep directives (stockpile before winter, repair before storm) |
| Social | None | Huddling; fireside gathering; shared storm anxiety |

### Migration Path
1. `WeatherState` gains: `temperature: f32`, `wind_direction: Direction`, `pressure_trend: PressureTrend`
2. `Tile` gains: `snow_depth: f32`, `moisture: f32`, `frozen: bool`
3. Wire up existing `movement_multiplier()` and `comfort_modifier()` (defined/tested, never called)
4. Warmth drain in `needs.rs` switches from weather enum match to temperature-proportional formula
5. Building decay adds snow load and moisture factors
6. `Weather` enum stays as the visible weather type; temperature is derived from season + weather + day phase

## Tuning Notes
_Record observations and adjustments here during iteration._
