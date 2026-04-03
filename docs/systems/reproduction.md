# Reproduction

## Purpose
Models mating, pregnancy, birth, and kitten-rearing as a population growth mechanic that creates resource pressure, parental behavior overrides, and the colony's most emotionally devastating potential loss. Kittens are vulnerable dependents that can't contribute labor, forcing long-term planning and creating tension between growth and survival. Phase 10.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Mating prerequisites | Mates bond + both healthy + food > 60% capacity | Colony must be stable before reproduction |
| Conception chance | 5% per season when prerequisites met | Not guaranteed; multiple seasons may pass |
| Pregnancy duration | 1 season (~2000 ticks) | Long enough to plan; short enough to feel eventful |
| Litter size | 1–3 kittens (weighted: 1=40%, 2=40%, 3=20%) | Small litters keep population manageable |
| Milk dependency duration | 500 ticks (~2.5 days) | Kittens need mother nearby for early life |
| Kitten vulnerability period | Until Young life stage (season 4) | Extended dependency creates sustained tension |
| Parental override priority | Overrides all actions except Flee when kitten threatened | Parents will sacrifice food/sleep for kitten safety |
| Kitten death mood penalty | -0.8 for parents, -0.4 for colony | Most severe mood event in the game |

### Pregnancy Effects on Queen
| Effect | Value | Notes |
|--------|-------|-------|
| Food need decay multiplier | 1.5× | Eating for more than one |
| Movement speed | 0.7× base | Slower in late pregnancy |
| Safety need weight | 2× | Strongly shelter-seeking |
| Combat ability | Disabled in final 25% of pregnancy | Won't fight; flees instead |
| Colony mood | +0.1 for all cats | Anticipation bonus |

### Kitten Traits
| Trait | Source | Notes |
|-------|--------|-------|
| Personality | Average of parents ± random variation (±0.15 per axis) | Heritable but not deterministic |
| Skills | All start at 0.0 | Blank slate |
| Appearance | Random blend of parent attributes | Fur color, eye color, pattern |
| Magic Affinity | Average of parents × random(0.5, 1.5) | Can exceed parents or be lower |

### Kitten Development
| Age (seasons) | Stage | Capabilities |
|---------------|-------|-------------|
| 0–1 | Newborn | Milk-dependent; no actions; stays near mother; can be carried (scruff) |
| 1–3 | Kitten | Solid food; play-hunting (tiny skill XP); follows adults; vulnerable to all threats |
| 4–11 | Young | Full action set unlocked; faster skill growth (1.5× base rate) |

## Formulas
```
conception_check(mate_a, mate_b, food_stores):
    if bond(mate_a, mate_b) == Mates
       AND mate_a.health.current > 0.7
       AND mate_b.health.current > 0.7
       AND food_stores.current / food_stores.capacity > 0.6:
        if random() < 0.05: begin_pregnancy(queen)

kitten_personality(parent_a, parent_b):
    for each axis:
        base = (parent_a.trait + parent_b.trait) / 2.0
        variation = uniform(-0.15, 0.15)
        kitten.trait = clamp(base + variation, 0.0, 1.0)

parental_override(parent, kitten):
    if kitten.safety_threatened:
        parent.current_action = Flee(toward_kitten) OR Fight(threat_near_kitten)
        # Overrides all other action scoring

kitten_learning(kitten, nearby_adult):
    if distance(kitten, adult) <= 2 AND adult.current_action.is_skilled():
        skill = adult.current_action.relevant_skill()
        kitten.skills[skill] += 0.001  # Tiny XP from observation
```

## Tuning Notes
_Record observations and adjustments here during iteration._
