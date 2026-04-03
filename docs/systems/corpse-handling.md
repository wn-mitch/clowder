# Corpse Handling

## Purpose
Dead colony members persist as entities rather than despawning instantly. Unburied corpses create recurring grief for nearby cats, attract scavengers, and reduce environmental comfort. Burial at a designated cairn resolves grief and creates a positive "honored the dead" moment. High-spirituality cats are particularly affected by unburied kin. Phase 1 extension.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Corpse persistence | Until buried or 200 ticks (natural decay) | Long enough to matter; not permanent |
| Grief radius | 3 tiles | Cats passing nearby are affected |
| Grief mood modifier | -0.15 per tick within radius | Persistent low-grade misery; compounds |
| Spirituality grief amplifier | grief × (1.0 + spirituality × 0.5) | Spiritual cats feel it more deeply |
| Fondness grief amplifier | grief × (1.0 + fondness_with_dead × 0.3) | Close friends grieve harder |
| Burial action duration | 15 ticks | Meaningful time investment |
| Honored-the-dead modifier | +0.15 mood for 50 ticks | Reward for performing burial |
| Scavenger attraction radius | 5 tiles from corpse | Draws wildlife toward colony |
| Scavenger spawn bonus | +20% wildlife spawn chance near corpse | Creates secondary threat from neglect |

### Corpse States
| State | Duration | Visual | Effect |
|-------|----------|--------|--------|
| Fresh | 0–50 ticks | Cat body on tile | Grief modifier for nearby cats |
| Decaying | 50–150 ticks | Faded marker | Grief + comfort penalty (-0.3) on tile + scavenger attraction |
| Remains | 150–200 ticks | Bone marker | Reduced grief (-0.05); comfort penalty persists |
| Gone | 200+ ticks | Removed | Natural decay complete; no further effects |
| Buried | After burial action | Cairn marker | Grief resolved; positive memory for burial participants |

### Burial
| Aspect | Detail |
|--------|--------|
| Who can bury | Any living cat (no skill requirement) |
| Where | Adjacent to corpse; creates a Cairn terrain marker at that position |
| Duration | 15 ticks of dedicated action |
| Prerequisites | Cat must not be in mental break, combat, or fleeing |
| Coordinator interaction | Coordinator may issue burial directive if no cat has voluntarily started |
| Narrative | Significant tier: "{name} gently lays {dead_name} to rest beneath a cairn of stones." |

### Vigil Behavior
| Behavior | Trigger | Effect |
|----------|---------|--------|
| Vigil sitting | Cat with fondness > 0.5 toward dead cat, within 5 tiles | Cat moves to corpse tile and sits for 10–20 ticks; satisfies no needs but generates memory |
| Vigil mood | During vigil | Mood modifier -0.1 (sadness) but +0.05 acceptance need recovery |
| Vigil narrative | On vigil start | "{name} sits quietly beside {dead_name}, tail curled around her paws." |

## Formulas
```
grief_modifier(cat, corpse):
    base = -0.15
    distance_factor = 1.0 / max(distance(cat, corpse), 1.0)
    fondness_factor = 1.0 + fondness(cat, corpse.original_entity) * 0.3
    spirituality_factor = 1.0 + cat.personality.spirituality * 0.5
    return base * distance_factor * fondness_factor * spirituality_factor

scavenger_check(corpse):
    if corpse.state in (Decaying, Remains):
        if random() < 0.002:  # per tick
            spawn_wildlife(near=corpse.position, radius=5)

burial_complete(burier, corpse):
    remove_corpse(corpse)
    place_cairn(corpse.position)
    burier.mood.add_modifier("honored_the_dead", +0.15, 50)
    for cat in nearby_cats(corpse.position, radius=5):
        cat.mood.add_modifier("burial_witnessed", +0.05, 30)
    log.push(Significant, "{burier} lays {dead} to rest.")
```

## Tuning Notes
_Record observations and adjustments here during iteration._
