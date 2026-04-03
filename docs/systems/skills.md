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

## Tuning Notes
_Record observations and adjustments here during iteration._
