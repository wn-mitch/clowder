# Combat

## Purpose
Resolves conflict between cats and threats (foxes, shadow-foxes, hawks, snakes). Probabilistic outcomes driven by skill, boldness, and group size. Injuries persist and require recovery time, creating lasting consequences.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Jitter range | implied ±0.05 (consistent with utility AI) | Prevents deterministic outcomes |
| Group bonus per ally | +0.2 to attack score | Meaningful but not overwhelming; a lone cat vs 3 is still risky |

### Cat Attack Formula Components
| Component | Formula |
|-----------|---------|
| Base attack score | combat_skill * boldness |
| Group contribution | base * (1 + 0.2 * ally_count) |
| Final score | base * group_modifier + jitter |

### Threat Statistics
| Threat | Attack Power | Defense | Morale Break Threshold | Special |
|--------|-------------|---------|----------------------|---------|
| Fox | 0.4 | 0.3 | 0.3 health remaining | — |
| Shadow-fox | 0.6 | 0.4 | 0.5 health remaining | +0.1 corruption to all fighters per hit |
| Hawk | 0.3 | 0.2 | Flees at 0.5 health | Aerial; may require special handling |
| Snake | 0.5 | 0.1 | — | Ambush bonus (first strike advantage) |

### Injury Thresholds
| Severity | Health Threshold | Recovery Time |
|----------|----------------|--------------|
| Minor | health < 0.7 | 50 ticks |
| Moderate | health < 0.4 | 200 ticks |
| Severe | health < 0.2 | 500+ ticks |
| Fatal | health = 0.0 | N/A — death |

## Formulas
```
cat_attack = combat_skill * boldness * (1.0 + 0.2 * ally_count) + jitter

hit_lands = cat_attack > threat_defense

damage_dealt = cat_attack - threat_defense  (if hit_lands)

threat_attack_roll = threat_power + jitter
damage_to_cat = threat_attack_roll - cat_defense_rating

morale_check: threat flees/retreats if health <= morale_threshold
```

## Tuning Notes
_Record observations and adjustments here during iteration._
