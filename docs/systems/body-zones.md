# Body Zones

## Purpose
Replaces the single health float with a body part model tracking injuries to specific anatomical regions. Each part has functional consequences when damaged — a wounded paw slows movement, a torn ear reduces threat detection, a throat wound bleeds toward death. Permanent injuries become part of a cat's identity ("one-eyed Bramble," "torn-ear Cedar"). Different attackers target different parts based on attack style. Phase 4 extension.

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Body parts per cat | 13 named parts | Significant anatomy without veterinary-level granularity |
| Condition states | 5 (Healthy → Bruised → Wounded → Mangled → Destroyed) | Clear progression with escalating consequences |
| tissue_damage range | 0.0 – 1.0 continuous | Smooth damage within condition tiers |
| Pain incapacitation threshold | total_pain > 0.9 | Can only Idle, Sleep, or accept treatment |

### Anatomical Model — 13 Body Parts

```
HEAD
├── Whiskers    — spatial awareness, hunting in low light/fog
├── Ears        — hearing, threat detection (treated as one part; torn ears are cosmetic scars)
├── Mouth/Jaw   — eating, bite attacks
├── Scruff      — grab point; kittens go limp when scruffed
└── Throat      — critical; bleeding if wounded, fatal if destroyed

TORSO
├── Flanks      — sides/ribs; armor for internal organs
└── Belly       — primordial pouch; natural armor during belly-kick fights; protects gut

LIMBS
├── Front Left Paw   — claws + grip; movement + climbing + combat
├── Front Right Paw  — claws + grip; movement + climbing + combat
├── Rear Left Paw    — landing/traction; movement + pounce
├── Rear Right Paw   — landing/traction; movement + pounce
└── Haunches         — rear body/hips; structural for rear movement + jumping

TAIL
└── Tail        — balance, expression, social signaling
```

### Condition Thresholds
| Condition | tissue_damage Range | Functional Effect |
|-----------|--------------------|--------------------|
| Healthy | 0.0 | None |
| Bruised | 0.01 – 0.25 | Cosmetic / pain only |
| Wounded | 0.26 – 0.60 | Partial function loss (see table below) |
| Mangled | 0.61 – 0.90 | Severe function loss |
| Destroyed | 0.91 – 1.0 | Complete loss; may be permanent |

### Functional Consequences by Part
| Part | Wounded | Mangled / Destroyed |
|------|---------|---------------------|
| Whiskers | -20% hunting in Fog/Night | Lost spatial sense; can't hunt in low visibility |
| Ears | -20% threat detection range | Deaf to distant threats; torn ear tip is permanent scar |
| Mouth/Jaw | -30% eating speed, -25% bite damage | Can't eat solid food unaided; no bite attack |
| Scruff | Pain only | Kittens can't be carried by this cat |
| Throat | Bleeding: health drain 0.02/tick | Fatal |
| Flanks | Pain; -15% defense vs torso hits | Internal damage possible; -30% defense |
| Belly | Reduced belly-kick defense | No protection during supine fighting; gut exposed |
| Paws (each) | -15% movement, -20% climbing/combat | Can't grip; severe movement penalty on that limb |
| Haunches | -30% movement speed, -40% jump/pounce | Rear legs non-functional; dragging movement only |
| Tail | -10% balance (movement jitter) | Permanent balance penalty; limited social expression |

### Natural Armor
| Protected Part | Armor Part | Requirement |
|----------------|-----------|-------------|
| Internal organs (implied) | Flanks | Flanks must be Wounded+ before internal damage applies |
| Gut (implied) | Belly pouch | Belly provides 0.3 damage reduction during supine fighting |
| Throat | — | Exposed; no natural armor (high-value target) |

### Combat Targeting Weights
| Attacker | Primary Targets (weight) | Secondary (weight) |
|----------|------------------------|-------------------|
| Cat (scratch) | Ears (0.25), Whiskers (0.2), Paws (0.2) | Flanks (0.2), Tail (0.15) |
| Cat (bite) | Throat (0.25), Scruff (0.2), Ears (0.15) | Paws (0.2), Tail (0.2) |
| Cat (bunny-kick) | Belly (0.4), Haunches (0.3) | Rear Paws (0.2), Tail (0.1) |
| Fox | Throat (0.35), Flanks (0.25), Haunches (0.15) | Paws (0.15), Ears (0.1) |
| Hawk | Ears (0.25), Whiskers (0.2), Scruff (0.2) | Flanks (0.2), Tail (0.15) |
| Snake | Front Paws (0.3), Whiskers (0.2), Mouth (0.15) | Rear Paws (0.2), Tail (0.15) |
| Shadow Fox | Same as Fox + corruption applied to wounded part |

### Healing Rates
| Part Category | Bruised→Healthy | Wounded→Bruised | Mangled→Wounded | Permanent? |
|---------------|----------------|-----------------|-----------------|------------|
| Soft tissue (ears, belly, scruff) | 30 ticks | 80 ticks | 200 ticks | Ear tips: permanent scar if Destroyed |
| Structural (haunches, paws) | 50 ticks | 150 ticks | 400 ticks | Haunches: permanent limp if Destroyed |
| Sensory (whiskers, mouth) | 40 ticks | 120 ticks | 300 ticks | Whiskers regrow; jaw permanent if Destroyed |
| Throat | 40 ticks | 100 ticks | N/A | Fatal before Mangled without treatment |
| Tail | 30 ticks | 80 ticks | 200 ticks | Permanent crook if Destroyed |

### Pain System
| Part | Pain Weight | Notes |
|------|------------|-------|
| Throat | 3.0 | Extremely painful; drives incapacitation |
| Haunches | 2.0 | Structural pain |
| Flanks | 1.5 | Rib pain |
| Paws (each) | 1.0 | Moderate |
| Ears, Whiskers, Tail | 0.5 | Low pain relative to function loss |
| Belly, Scruff | 0.8 | Moderate |
| Mouth/Jaw | 1.5 | Bone pain |

### Narrative Integration
Each body part has narrative templates for injury:
- Whiskers: "the fox's snap tore away {name}'s whiskers"
- Ears: "{name}'s right ear was shredded in the scuffle"
- Throat: "the fox's jaws closed around {name}'s throat"
- Paws: "{name} limps badly, favoring the wounded forepaw"
- Tail: "{name}'s tail hangs at an odd angle"
- Haunches: "{name} drags herself forward, haunches torn"

Permanent injuries become identity traits — shown in TUI inspect view and referenced in future narrative templates.

### Migration from Current System
| Current | New |
|---------|-----|
| `Health.current: f32` | Derived: `1.0 - (total_weighted_damage / max_weighted_damage)` |
| `Health.injuries: Vec<Injury>` | `Vec<BodyPartInjury>` with `part: BodyPart`, `condition: PartCondition`, `tissue_damage: f32` |
| `InjuryKind` (Minor/Moderate/Severe) | `PartCondition` (Healthy/Bruised/Wounded/Mangled/Destroyed) |
| `damage_to_injury()` in combat.rs | `damage_to_body_part()` — selects target via weighted random per attacker type |
| `is_incapacitated` check in ai.rs | Pain threshold check: `total_pain > 0.9` |

## Formulas
```
total_pain = sum(part.tissue_damage * part.pain_weight for part in cat.body_parts)

health_derived = 1.0 - (total_pain / max_possible_pain)

damage_to_body_part(attacker_type, damage_amount, tick):
    weights = targeting_weights[attacker_type]
    target = weighted_random(body_parts, weights)
    # Check natural armor
    if target.is_internal AND armor_part.condition < Wounded:
        target = armor_part  # Hit the armor instead
    target.tissue_damage += damage_amount
    target.condition = condition_from_damage(target.tissue_damage)
    return BodyPartInjury { part: target, condition, tissue_damage, tick }

healing(part, ticks_elapsed):
    rate = healing_rate[part.category][part.condition]
    part.tissue_damage -= (1.0 / rate)  # Heal toward lower condition tier
    if part.tissue_damage < condition_threshold(part.condition - 1):
        part.condition = part.condition - 1  # Upgrade condition
    if part.is_permanent_at(Destroyed) AND part.condition == Destroyed:
        part.permanent = true  # Never heals past Destroyed
```

## Tuning Notes
_Record observations and adjustments here during iteration._
