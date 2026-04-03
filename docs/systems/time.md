# Time

## Purpose
Drives day/night cycles, seasonal progression, and simulation speed controls. Time of day modulates action weights (crepuscular hunting bonus), season drives weather and resource availability.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Ticks per day phase | 25 | 4 phases × 25 = 100 ticks/day |
| Ticks per day | 100 | Clean round number; easy to reason about |
| Ticks per season | 2000 | 20 in-game days per season |
| Ticks per year | 8000 | 4 seasons × 2000 |
| Speed — 1x | 1 tick/update | Real-time observation mode |
| Speed — 5x | 5 ticks/update | Fast-forward for routine periods |
| Speed — 20x | 20 ticks/update | Very fast; skip to events |
| Target render rate | 30 fps | Smooth enough for terminal TUI |
| Crepuscular hunting bonus | +0.2 to hunting action weight | Cats are dawn/dusk hunters; meaningful but not dominant |

### Day Phases
| Phase | Tick Range (within day) | Notes |
|-------|------------------------|-------|
| Dawn | 0–24 | Crepuscular bonus active |
| Day | 25–49 | Standard activity period |
| Dusk | 50–74 | Crepuscular bonus active |
| Night | 75–99 | Reduced activity; sleep weight increased |

### Seasonal Index
| Season | Year Tick Range |
|--------|----------------|
| Spring | 0–1999 |
| Summer | 2000–3999 |
| Autumn | 4000–5999 |
| Winter | 6000–7999 |

## Formulas
```
day_phase = (tick % 100) / 25  →  0=Dawn, 1=Day, 2=Dusk, 3=Night

season = (tick % 8000) / 2000  →  0=Spring, 1=Summer, 2=Autumn, 3=Winter

crepuscular_bonus = 0.2 if day_phase in (Dawn, Dusk) else 0.0
  applied as context_modifier to hunting action score
```

## Tuning Notes
_Record observations and adjustments here during iteration._
