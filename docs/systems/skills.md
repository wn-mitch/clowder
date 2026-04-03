# Skills

## Purpose
Tracks competence in 6 domains. Skill level directly improves action outcomes (hunt success rate, herb potency, building speed, etc.) and grows through use with diminishing returns.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Starting value — base range | 0.05–0.20 | All cats start with some minimal competence in every skill |
| Starting value — aptitude skill | 0.20–0.40 | One randomly selected skill gets a head start per cat |
| Growth rate formula | 1.0 / (1.0 + total_skill_points) | Diminishing returns; prevents runaway specialists early |

### Skill-to-Action Mappings
| Skill | Actions Improved | Notes |
|-------|-----------------|-------|
| Hunting | Hunt (prey catch rate) | Modified by boldness, time of day, weather |
| Foraging | Forage (herb/food yield) | Modified by weather, map tile type |
| Herbcraft | Brew poultice, Brew tonic (potency) | Requires herbcraft skill + herbs in stores |
| Building | Construct structure, Repair structure (speed) | Modified by diligence, crew bonus |
| Combat | Attack (damage), Defense (damage reduction) | Modified by boldness, group bonus |
| Magic | Cast (success rate, effect magnitude) | Modified by affinity; low skill + high affinity → misfires |

## Formulas
```
growth(skill) = base_rate * (1.0 / (1.0 + sum_of_all_skill_values))

action_success_rate = base_rate + skill_value * skill_weight
  (skill_weight varies by action; typically 0.3–0.6)
```

## Skill Decay (Phase Enhancement)

Skills rust when unused, creating specialization pressure. A cat who stops hunting gradually loses the edge.

### Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Decay rate (unused) | 0.001/tick | ~10× slower than typical growth rate; gradual |
| Decay grace period | 200 ticks since last use | Skills don't immediately rust |
| Rust label threshold | Decay exceeds 20% of peak value | Visible indicator in TUI |
| Minimum floor | 0.05 | Skills never decay below baseline competence |
| Decay resistance from personality | Diligence reduces decay by up to 30% | Diligent cats maintain skills better |

### Rust States
| State | Condition | TUI Display | Effect |
|-------|-----------|-------------|--------|
| Sharp | No decay accumulated | Normal | Full skill value |
| Rusty | Decay > 20% of peak | "(rusty)" suffix | Skill functions at actual (decayed) value |
| Very Rusty | Decay > 50% of peak | "(very rusty)" suffix | Skill functions at decayed value; action scoring penalty -0.1 |

```
skill_decay(cat, skill):
    ticks_unused = current_tick - skill.last_used_tick
    if ticks_unused > 200:
        decay = 0.001 * (1.0 - cat.personality.diligence * 0.3)
        skill.value = max(skill.value - decay, 0.05)

rust_state(skill):
    if skill.peak_value == 0.0: return Sharp
    ratio = (skill.peak_value - skill.value) / skill.peak_value
    if ratio > 0.5: return VeryRusty
    if ratio > 0.2: return Rusty
    return Sharp
```

## Tuning Notes
_Record observations and adjustments here during iteration._
