# Organized Raids

## Purpose
Escalates threats beyond lone predators to coordinated assaults that test colony defenses. Predator packs, rat swarms, and corruption incursions require group response — walls, watchtowers, coordinated combat, and advance preparation. Raid frequency scales with colony success, creating a natural difficulty curve. Phase 4 extension.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Raid check interval | Every 1000 ticks (~10 days) | Infrequent enough to prepare between raids |
| Base raid chance | 3% per check | Rare early; compounds with colony growth |
| Colony threat score | size × 0.3 + food_ratio × 0.2 + building_count × 0.1 | Larger, richer colonies attract more attention |
| Minimum colony size for raids | 5 cats | Very small colonies don't trigger organized assault |
| Season modifier | Winter: 2× chance; Summer: 0.5× chance | Desperate predators in winter |
| Watchtower early warning | +10 ticks advance notice | Detected on approach; time to prepare |
| Wall defensive bonus | Attackers must path around or breach | Walls funnel attackers to gates |

### Raid Types
| Raid | Composition | Trigger Weight | Behavior | Target |
|------|------------|---------------|----------|--------|
| Predator Pack | 3–5 foxes moving as group | 3 (common) | Coordinated: approach from one direction, attack together | Stores (food), then kittens/elders |
| Rat Swarm | 8–12 rats (low individual threat) | 2 | Flood: scatter across map toward food | Food stores directly; bypass defenses individually |
| Corruption Incursion | 2–4 shadow foxes | 1 (rare) | Emerge from most-corrupted map zone | Cats nearest corruption; spread corruption as they move |
| Hawk Flock | 3–4 hawks | 1 (rare, spring/summer only) | Aerial: ignore walls, target open-ground cats | Kittens, small/injured cats in open terrain |

### Raid Scaling
| Colony Threat Score | Raid Intensity |
|--------------------|---------------|
| 0 – 3 | No raids (too small to notice) |
| 3 – 6 | Predator packs (3 foxes) or rat swarms (8 rats) |
| 6 – 10 | Larger packs (5 foxes) or mixed raids; corruption incursion possible |
| 10+ | All raid types; larger compositions; simultaneous raids possible |

### Defensive Systems Integration
| Defense | Effect Against Raids |
|---------|---------------------|
| Wall | Blocks ground movement; raiders must path around or through gates |
| Gate (closed) | Blocks all entry; open gates are bypassed |
| Watchtower | Early detection (+10 ticks warning); cats on tower get +0.2 threat detection |
| Ward Post | Shadow foxes avoid tiles within ward radius; corruption incursion redirected |
| Coordinator | Issues Fight/Patrol directives automatically when raid detected |
| Group combat | Existing ally bonus (0.2 per ally) makes coordinated defense effective |

### Raid Resolution
| Outcome | Condition | Colony Effect |
|---------|-----------|---------------|
| Repelled | All raiders killed or fled | +0.2 mood for fighters; +0.1 colony-wide; victory memory |
| Partial breach | Some food stolen or cats injured | Food stores reduced; injuries; -0.1 mood colony-wide |
| Overrun | Colony cannot mount defense | Significant casualties; food loss; -0.3 mood; possible cat deaths |

## Formulas
```
raid_check(colony):
    threat_score = colony.size * 0.3
                 + food_stores.ratio * 0.2
                 + building_count * 0.1
    if threat_score < 3.0: return  # too small

    season_mod = match season { Winter => 2.0, Autumn => 1.5, _ => 1.0 }
    chance = 0.03 * season_mod * (threat_score / 5.0)
    if random() < chance: spawn_raid(threat_score)

raid_composition(threat_score, raid_type):
    base_count = raid_type.min_count
    scaling = (threat_score - 3.0) / 7.0  # 0.0 at score 3, 1.0 at score 10
    count = base_count + floor(scaling * raid_type.scaling_range)
    return min(count, raid_type.max_count)

early_warning(colony):
    if any_watchtower_functional:
        alert_tick = raid_spawn_tick - 10
        coordinator.issue_directive(Fight, priority=1.0)
```

## Tuning Notes
_Record observations and adjustments here during iteration._
