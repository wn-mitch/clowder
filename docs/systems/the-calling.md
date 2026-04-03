# The Calling

## Purpose
A rare, high-stakes creative trance inspired by Dwarf Fortress's Strange Mood system. High-affinity cats in positive emotional states occasionally experience an overwhelming creative/spiritual compulsion to create something extraordinary. Success produces a Named Object — a colony treasure with mechanical benefits and narrative significance. Failure risks corruption and psychological damage. Phase 6 extension.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Trigger: magic affinity minimum | > 0.5 | Only magically-inclined cats experience this |
| Trigger: mood minimum | valence > 0.6 | Must be in a genuinely good state |
| Trigger: spirituality minimum | > 0.5 | Spiritual cats are conduits for the calling |
| Trigger: chance per tick | 0.05% (1 in 2000) | Very rare; ~1 per season in a colony of 8 |
| Trance duration | 40–60 ticks | Substantial commitment; colony must cope without them |
| Material requirements | 2–3 specific herbs (randomly selected) | Creates urgency to gather |
| Material gathering timeout | 100 ticks | Fail if herbs not found in time |
| Success threshold | magic_skill ≥ affinity × 0.6 | Achievable for practiced cats; risky for raw talent |

### Trance Phases
| Phase | Duration | Behavior |
|-------|----------|----------|
| 1. Compulsion | 5 ticks | Cat stops current action; narratively "stares into the distance, trembling" |
| 2. Gathering | Up to 100 ticks | Cat seeks 2–3 specific herb types; will cross entire map |
| 3. Creation | 40–60 ticks | Cat retreats to Workshop/WardPost/FairyRing; refuses all interaction |
| 4. Resolution | 1 tick | Success or failure resolved |

### Success Outcomes
| Object Type | Creation Condition | Colony Effect |
|------------|-------------------|---------------|
| Named Ward | Herbs include ward-related (Thornbriar, Dreamroot) | Ward with 3× normal strength and named identity |
| Named Remedy | Herbs include healing-related (HealingMoss, Calmroot) | Remedy with 2× potency; single use but legendary |
| Spirit Totem | Herbs include Moonpetal | Placeable object: +0.1 mood to all cats within 4 tiles permanently |
| Woven Talisman | Any herb combination | Carried by creator: +0.05 corruption resistance; small mood bonus |

### Named Object Properties
| Property | Value |
|----------|-------|
| Name generation | Compound name: adjective + material + form (e.g., "Moonwhisper," "Thornheart") |
| Creator attribution | "{cat_name}'s {object_name}" |
| Narrative entry | Significant tier; recorded in colony knowledge permanently |
| Destruction consequence | Creator gets -0.5 mood for 100 ticks; colony gets -0.2 mood |

### Failure Outcomes
| Outcome | Trigger | Effect |
|---------|---------|--------|
| Corruption spike | magic_skill < affinity × 0.4 | +0.2 personal corruption; mood -0.4 for 50 ticks |
| Shaken | magic_skill between 0.4× and 0.6× affinity | Mood -0.3 for 30 ticks; "Shaken" modifier prevents another Calling for 2000 ticks |
| Materials expired | Gathering phase timeout | Mild disappointment; mood -0.1; herbs consumed anyway |

### Post-Calling Effects on Creator
| Outcome | Effect |
|---------|--------|
| Success | +0.5 mood for 100 ticks; magic_skill gains 0.5; spirituality shifts +0.05 permanently; becomes "Touched" (narrative identity) |
| Failure | Mood penalty as above; corruption risk; cooldown before next Calling |

## Formulas
```
calling_trigger_check(cat):
    if cat.magic_affinity > 0.5
       AND cat.mood.valence > 0.6
       AND cat.personality.spirituality > 0.5
       AND NOT cat.has_modifier("Shaken")
       AND NOT cat.in_calling:
        if random() < 0.0005: begin_calling(cat)

required_herbs = random_sample(all_herb_kinds, count=random(2, 3))

success_check(cat, herbs_gathered):
    if all required herbs gathered within timeout:
        if cat.skills.magic >= cat.magic_affinity * 0.6:
            return Success(determine_object_type(herbs))
        elif cat.skills.magic >= cat.magic_affinity * 0.4:
            return Failure(Shaken)
        else:
            return Failure(CorruptionSpike)
    else:
        return Failure(MaterialsExpired)

object_name = random_adjective() + random_material_word(herbs) + random_form()
```

## Tuning Notes
_Record observations and adjustments here during iteration._
