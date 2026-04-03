# Disease

## Purpose
Models wound infection, seasonal illness, and contagion. Untreated injuries fester, winter brings colds, spoiled food causes poisoning, and corruption exposure sickens. Treatment requires herbcraft skill, herbs, and rest — creating demand for medical infrastructure and healer specialization. Contagion creates quarantine tension: isolating the sick protects the colony but harms the patient's social needs. Phase 9.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Wound infection chance | 3% per tick for untreated wound (first 30 ticks) | Treatable window before infection sets in |
| Infection severity progression | +0.01/tick once infected | Slow enough for intervention; fatal if ignored |
| Fever energy drain multiplier | 2× base energy decay | Sick cats exhaust faster |
| Contagion radius | 2 tiles | Close contact required |
| Contagion chance per tick | 1% base (illness-dependent) | Low per-tick but compounds over time |
| Quarantine effectiveness | 0% transmission if isolated (no cats within 3 tiles) | Clear benefit to isolation |
| Treatment quality formula | healer_herbcraft × herb_potency | Skill matters |

### Disease Types
| Disease | Trigger | Contagious | Severity | Duration |
|---------|---------|-----------|----------|----------|
| Wound Infection | Untreated wound after 30 ticks | No | Progressive (0.01/tick health drain) | Until treated or fatal |
| Winter Cold | Winter season, 0.5% chance/tick per cat | Yes (2 tile radius, 1%/tick) | Mild (-0.1 mood, -20% action speed) | 100–200 ticks |
| Food Poisoning | Eating stale food (if Food Variety implemented) | No | Moderate (nausea, -0.2 mood, can't eat for 30 ticks) | 30–60 ticks |
| Corruption Sickness | Prolonged corruption exposure (tile > 0.5, 50+ ticks) | No | Progressive (mood drain, energy drain, eventual health drain) | Until removed from corrupted area + 50 ticks |

### Treatment
| Treatment | Requirement | Effect |
|-----------|------------|--------|
| Wound cleaning | Healer + any herb | Prevents infection if applied within 30 ticks of injury |
| Fever remedy | Healer + HealingMoss | Halts infection progression; healing begins |
| Cold remedy | Healer + Calmroot | Reduces duration by 50%; halves contagion chance |
| Corruption cleanse | Healer + Moonpetal + Dreamroot | Removes corruption sickness; reduces personal corruption by 0.1 |

### Medical Infrastructure
| Facility | Requirement | Bonus |
|----------|------------|-------|
| Sick Den | Den designated for treatment | +30% treatment effectiveness; patients rest faster |
| Healer role | Cat with herbcraft > 0.5 assigned via coordination | Dedicated treatment; won't abandon patient for other tasks |
| Herb stockpile | Stores building with herbs | Healer draws from stockpile without foraging mid-treatment |

## Formulas
```
infection_check(wound):
    if wound.ticks_untreated > 30 AND NOT wound.infected:
        if random() < 0.03: wound.infected = true

infection_progression(wound):
    if wound.infected:
        health.current -= 0.01 * (1.0 - treatment_quality)

contagion_check(sick_cat, nearby_cat):
    if distance <= 2 AND sick_cat.illness.contagious:
        base_chance = illness.contagion_rate
        if nearby_cat.is_sheltered: base_chance *= 0.5
        if random() < base_chance: nearby_cat.contract(illness)

treatment_quality = healer.herbcraft * herb.potency * facility_bonus
    facility_bonus = 1.3 if in Sick Den, else 1.0
```

## Tuning Notes
_Record observations and adjustments here during iteration._
