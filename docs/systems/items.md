# Items

## Purpose
Lightweight item system appropriate for cats. Items provide environmental comfort, morale, personality-driven hoarding behavior, and the foundation for trade and substance systems. Not a full crafting economy — cats don't forge swords. They collect shiny things, trophies, nesting materials, and herbs. Phase 12a.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Inventory capacity | 5 slots (expanding existing herb pouch) | Small; cats can't carry much |
| Item quality range | 0.0 – 1.0 | Normalized; derived from finder/crafter skill |
| Hoarding trigger | Pride > 0.6 OR Ambition > 0.7 | Personality-driven collecting behavior |
| Comfort bonus per item in building | +0.02 × quality | Small per-item; rewards accumulation |
| Item decay rate | Varies by type | Organic items decay; stones don't |

### Item Types
| Category | Examples | Source | Decay | Use |
|----------|---------|--------|-------|-----|
| Trophies | Mouse skull, feather, fox tooth | Hunting, combat kills | Slow (0.001/tick) | Morale (+0.02 mood when in cat's nest), comfort bonus in buildings |
| Nesting Materials | Moss, soft leaves, dried grass | Foraging | Medium (0.003/tick) | Den comfort bonus; required for quality nests |
| Herbs | HealingMoss, Moonpetal, etc. | Existing system | Existing rates | Medicine, wards, remedies |
| Curiosities | Shiny pebble, glass shard, colorful shell | Random find while exploring/foraging | None (inorganic) | Mood bonus to owner; trade value; hoarding target |
| Substances | Catnip, Valerian, Corrupted variants | Growing on specific terrain | Medium (0.005/tick) | Euphoria/calming effects (see substances.md) |

### Item Quality
| Quality Tier | Range | Source |
|-------------|-------|--------|
| Poor | 0.0 – 0.2 | Unskilled finder; damaged |
| Common | 0.2 – 0.5 | Average skill |
| Fine | 0.5 – 0.8 | Skilled finder/crafter |
| Exceptional | 0.8 – 1.0 | Master skill; rare |

### Hoarding Behavior
| Personality | Behavior |
|------------|----------|
| High Pride | Collects trophies; displays near sleeping spot |
| High Ambition | Seeks rare/high-quality items; competes for the best |
| High Curiosity | Collects curiosities; explores more to find them |
| High Independence | Defends personal hoard; mood penalty if items taken |
| Low Pride/Ambition | Doesn't hoard; shares freely |

## Formulas
```
item_quality(finder_or_crafter):
    relevant_skill = action_skill_mapping[action]
    quality = (relevant_skill / 5.0) + uniform(-0.1, 0.1)
    return clamp(quality, 0.0, 1.0)

comfort_from_items(building):
    return sum(item.quality * 0.02 for item in building.placed_items)

hoarding_score(cat, item):
    base = 0.0
    if cat.pride > 0.6: base += item_is_trophy * 0.3
    if cat.ambition > 0.7: base += item.quality * 0.4
    if cat.curiosity > 0.7: base += item_is_curiosity * 0.3
    return base

item_decay(item):
    item.condition -= item.decay_rate
    if item.condition <= 0.0: remove(item)
```

## Tuning Notes
_Record observations and adjustments here during iteration._
