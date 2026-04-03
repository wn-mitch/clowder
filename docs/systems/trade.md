# Trade & Visitors

## Purpose
Introduces external contact with the colony — wandering loners who might join, traders who barter goods, and scouts who share knowledge. Breaks the isolation of a single colony and creates seeds for future multi-colony play. Colony reputation determines the frequency and disposition of visitors. Phase 12c.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Visitor check interval | Every 500 ticks (~5 days) | Infrequent enough to be notable |
| Base visitor chance | 5% per check | Rare; scales with reputation |
| Reputation range | 0.0 – 1.0 | Normalized |
| Recruitment fondness threshold | 0.6 | Must genuinely befriend a loner to recruit |
| Trader visit duration | 50–100 ticks | Long enough to interact; they don't stay forever |
| Loner stay duration | 200–400 ticks | Longer; gives time to build relationship |

### Visitor Types
| Type | Frequency Weight | Behavior | Prerequisites |
|------|-----------------|----------|---------------|
| Wandering Loner | 3 | Arrives at map edge; explores colony; can be befriended and recruited | Reputation > 0.2 |
| Trader | 2 | Arrives with items; seeks Hearth/Stores; offers barter | Reputation > 0.4 |
| Hostile Loner | 1 | Arrives at map edge; may steal food and leave; can be driven off or befriended | Always possible |
| Scout | 1 | Arrives from "another colony"; shares memories; observes and leaves | Reputation > 0.6 |

### Colony Reputation
| Factor | Contribution | Notes |
|--------|-------------|-------|
| Food surplus | (current - capacity × 0.5) / capacity × 0.3 | Well-fed colonies attract visitors |
| Building quality | avg(building.condition) × 0.2 | Maintained infrastructure signals stability |
| Safety | (1.0 - recent_death_count × 0.1) × 0.2 | Deaths reduce reputation |
| Population | colony_size / 20 × 0.15 | Larger colonies are more notable |
| Trade history | successful_trades × 0.05 × 0.15 | Good trade reputation compounds |

### Recruitment
| Step | Mechanic |
|------|---------|
| 1. Loner arrives | Random personality generated; appears at map edge |
| 2. Social interaction | Colony cats can Socialize with loner; builds fondness/familiarity |
| 3. Fondness threshold | When fondness > 0.6 AND familiarity > 0.4, loner considers joining |
| 4. Colony assessment | Loner evaluates: food stores, building quality, safety. Poor colony → loner leaves |
| 5. Integration | Loner joins colony; full cat entity with relationships initialized at low familiarity |

### Trade Mechanics
| Step | Mechanic |
|------|---------|
| 1. Trader arrives | Carries 3–5 random items (herbs, curiosities, nesting materials) |
| 2. Barter | Colony offers items/food; trader evaluates by item quality + type preference |
| 3. Exchange | If offer value ≥ 80% of requested value, trade succeeds |
| 4. Mood/reputation | Successful trade: +0.05 mood for participating cat, +0.05 reputation |
| 5. Departure | Trader leaves after duration expires; may return next visit cycle |

## Formulas
```
reputation = clamp(
    food_factor + building_factor + safety_factor + population_factor + trade_factor,
    0.0, 1.0
)

visitor_chance = base_chance * (1.0 + reputation * 2.0)
    # At max reputation (1.0): 15% chance per check

visitor_type = weighted_random(types, weights)
    # Modified by reputation thresholds — low rep only gets loners/hostiles

recruitment_check(loner, colony):
    fondness = max_fondness_with_any_colony_cat
    if fondness > 0.6 AND familiarity > 0.4:
        colony_appeal = (food_ratio + avg_building_condition + safety_score) / 3.0
        if colony_appeal > 0.4: loner.joins_colony()
```

## Tuning Notes
_Record observations and adjustments here during iteration._
