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

## NamedLandmark substrate
Named Wards / Named Remedies / Spirit Totems / Woven Talismans are one of six convergent consumers of the shared naming substrate documented in `naming.md` (registry + event-proximity matcher + event-keyed name templates). Consumers: `paths.md`, `crafting.md` Phase 3, `crafting.md` Phase 4, `ruin-clearings.md` Phase 3, `the-calling.md` (this file), `monuments.md`.

The current compound-name generator (`random_adjective() + random_material_word(herbs) + random_form()`) works standalone but produces generic names ("Moonwhisper Thornheart") that don't reference the cat or the event that produced them. Under the shared substrate, a successful Calling becomes a `Significant`-tier event that the matcher consumes — producing "Brackenstar's Dreaming Ward" (cat-anchored) rather than "Moonwhisper Talisman" (generator-anchored). Cat-anchored naming strengthens the mythic-texture canary signal without changing the underlying success rules above. The legacy generator stays as the fallback branch of the matcher for Callings whose Significant-event proximity window contains no qualifying event.

## Relation to axis-capture

The Calling is the canonical existing instance of the general
axis-capture primitive defined in `docs/systems/ai-substrate-refactor.md`
§7.W — not a standalone Phase-6 mechanic. Every property this doc
specifies maps onto §7.W's vocabulary:

| Calling property (this doc) | Axis-capture vocabulary (§7.W) |
|---|---|
| Trigger conditions (affinity + mood + spirit ≥ thresholds) | Externally-seeded axis activation |
| Trance phase 1 Compulsion — "stops current action" | Captured axis abruptly wins scoring |
| Trance phase 3 Creation — "refuses all interaction" | Captured axis wins every tick; other axes active-but-losing (§7.W.2) |
| Herb gathering requirements + 100-tick timeout | `Blind` commitment on means (§7.1) |
| Success → Named Object | Positive resolution of a bounded capture window |
| Failure → corruption spike / Shaken | Pathological resolution of the same mechanism |
| "Touched" narrative identity post-success | Persistent identity modifier as capture residue |
| `Shaken` 2000-tick cooldown | Recovery window from pathological resolution |

Seeing the Calling as one *content-variant* of axis-capture — rather
than a bespoke system — has two consequences worth flagging:

**Dark Callings fall out of the mechanism.** A Calling-shaped gate
with corruption-tainted trigger conditions (high corruption + low
mood + some affinity threshold) produces a compulsion to create a
Named Curse or a shadow-pact object. Same trance mechanics, inverse
valence. The existing corruption-spike failure mode already
prefigures this shape. Implementation is Phase-6+ and explicitly
out of scope for the current spec; noted here so the mechanism owner
knows the capacity exists without requiring a new subsystem.

**Other captures share the architecture.** The axis-capture primitive
is not only for sanctioned / mythic content. Ordinary social bonds
are healthy axis-captures (see `docs/systems/warmth-split.md` —
`social_warmth` as a fulfillment-layer axis). Pathological captures
— addiction-analogues, sadist-play escalation, spite cycles — are
the same mechanism with sensitization (§7.W.1) and narrow-source
capture. The Calling sits at the "sanctioned, time-limited,
bivalent" corner of the capture space; design work on the rest of
the space can reuse everything this doc specifies.

**Architectural context:** see refactor doc §7.W (axis-capture and
the warring self), particularly §7.W.4(a) for the mapping table and
§7.W.5 for dark-Calling acknowledgment.

## Tuning Notes
_Record observations and adjustments here during iteration._
