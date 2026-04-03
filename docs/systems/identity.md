# Identity

## Purpose
Defines each cat's fixed attributes: name, gender, orientation, appearance, and age stage. These inform relationship compatibility, narrative pronoun selection, and life-stage behavior modifiers.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Gender — Tom | 50% | Approximate real-world cat colony distribution |
| Gender — Queen | 45% | Slightly fewer to model typical colony dynamics |
| Gender — Nonbinary | 5% | Present but rare by default; configurable |
| Orientation — Straight | 75% | Default majority |
| Orientation — Gay | 10% | Configurable per world |
| Orientation — Bisexual | 10% | Configurable per world |
| Orientation — Asexual | 5% | Configurable per world |

### Name Pool (30 names)
Bramble, Thistle, Cedar, Moss, Fern, Ash, Reed, Clover, Wren, Hazel, Rowan, Sage, Ivy, Birch, Flint, Nettle, Sorrel, Briar, Ember, Willow, Thorn, Juniper, Lark, Pebble, Lichen, Mallow, Basil, Tansy, Finch, Heron

### Age Stages
| Stage | Season Range | Notes |
|-------|-------------|-------|
| Kitten | 0–3 seasons | Dependent; limited action set |
| Young | 4–11 seasons | Growing skills; full action set unlocked |
| Adult | 12–47 seasons | Peak capability; prime breeding age |
| Elder | 48+ seasons | Reduced physical stats; social/knowledge bonuses |

### Appearance Pools
| Attribute | Notes |
|-----------|-------|
| Fur color | To be defined during implementation (e.g., black, white, ginger, tabby, tortoiseshell, grey) |
| Eye color | To be defined during implementation (e.g., amber, green, blue, yellow, hazel) |
| Pattern | To be defined during implementation (e.g., solid, tabby, bicolor, pointed, spotted) |

## Formulas
```
gender = weighted_random([Tom, Queen, Nonbinary], [0.50, 0.45, 0.05])

orientation = weighted_random([Straight, Gay, Bisexual, Asexual], [0.75, 0.10, 0.10, 0.05])

name = random_choice(NAME_POOL)

age_stage(seasons):
    0–3   → Kitten
    4–11  → Young
    12–47 → Adult
    48+   → Elder

romantic_compatible(a, b) =
    (a.orientation == Straight AND genders differ)
    OR (a.orientation == Gay AND genders match)
    OR (a.orientation == Bisexual)
    OR (a.orientation == Asexual AND b.orientation == Asexual)
    [symmetric check required]
```

_Note: Gender/orientation distributions are configurable at world-gen time._

## Tuning Notes
_Record observations and adjustments here during iteration._
