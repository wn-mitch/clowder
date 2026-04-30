# Body Zones

## Purpose
Replaces the single health float with a body part model tracking injuries to specific anatomical regions. Each part has functional consequences when damaged — a wounded paw slows movement, a torn ear reduces threat detection, a throat wound bleeds toward death. Permanent injuries become part of a cat's identity ("one-eyed Bramble," "torn-ear Cedar"). Different attackers target different parts based on attack style. Phase 4 extension.

## Species Coverage

Body zones apply to **all combatant animals** in the simulation, not just cats. The model is tiered by entity complexity:

| Tier | Species | Parts | Condition Model | Rationale |
|------|---------|-------|-----------------|-----------|
| Full | Cat | 13 named parts | 5-tier (Healthy → Destroyed) | Colony protagonist; identity injuries + healing narrative |
| Medium | Fox, ShadowFox | 8 named parts | 5-tier | Full AI agents; persistent across session; cats fight back against them |
| Medium | Hawk | 8 named parts | 5-tier | Bilateral wing model required for grounding mechanic |
| Simplified | Snake | 3 named parts | 5-tier | Serpentine anatomy; head/body/tail captures all relevant function loss |
| Prey | Mouse, Rat, Rabbit, Fish, Bird | 3 zones | 3-tier (Healthy / Wounded / Dead) | Ephemeral entities; zones drive hunt difficulty and meat yield, not identity |

## Parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Body parts per cat | 13 named parts | Significant anatomy without veterinary-level granularity |
| Body parts per fox/hawk | 8 named parts | Medium complexity matching their AI depth |
| Body parts per snake | 3 named parts | Serpentine anatomy collapses to head/body/tail |
| Prey zones | 3 zones | Head / Body / Legs (or Wings / Fins per species) |
| Condition states (cats, predators) | 5 (Healthy → Bruised → Wounded → Mangled → Destroyed) | Clear progression with escalating consequences |
| Condition states (prey) | 3 (Healthy / Wounded / Dead) | Prey don't recover between encounters |
| tissue_damage range | 0.0 – 1.0 continuous | Smooth damage within condition tiers |
| Pain incapacitation threshold (cats) | total_pain > 0.9 | Can only Idle, Sleep, or accept treatment |
| Retreat threshold (predators) | total_pain > 0.7 | Predators retreat sooner than cats defend; tunable |

---

## Cat Anatomy — 13 Body Parts

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

### Cat Condition Thresholds
| Condition | tissue_damage Range | Functional Effect |
|-----------|--------------------|--------------------|
| Healthy | 0.0 | None |
| Bruised | 0.01 – 0.25 | Cosmetic / pain only |
| Wounded | 0.26 – 0.60 | Partial function loss (see table below) |
| Mangled | 0.61 – 0.90 | Severe function loss |
| Destroyed | 0.91 – 1.0 | Complete loss; may be permanent |

### Cat Functional Consequences by Part
| Part | Wounded | Mangled / Destroyed |
|------|---------|---------------------|
| Whiskers | -20% hunting in Fog/Night | Lost spatial sense; can't hunt in low visibility |
| Ears | -20% threat detection range | Deaf to distant threats; torn ear tip is permanent scar |
| Mouth/Jaw | -30% eating speed, -25% bite damage | Can't eat solid food unaided; no bite attack |
| Scruff | Pain only | This cat can't be carried (scruff too damaged to grip safely) |
| Throat | Bleeding: health drain 0.02/tick | Fatal |
| Flanks | Pain; -15% defense vs torso hits | Internal damage possible; -30% defense |
| Belly | Reduced belly-kick defense | No protection during supine fighting; gut exposed |
| Paws (each) | -15% movement, -20% climbing/combat | Can't grip; severe movement penalty on that limb |
| Haunches | -30% movement speed, -40% jump/pounce | Rear legs non-functional; dragging movement only |
| Tail | -10% balance (movement jitter) | Permanent balance penalty; limited social expression |

### Cat Natural Armor
| Protected Part | Armor Part | Requirement |
|----------------|-----------|-------------|
| Internal organs (implied) | Flanks | Flanks must be Wounded+ before internal damage applies |
| Gut (implied) | Belly pouch | Belly provides 0.3 damage reduction during supine fighting |
| Throat | — | Exposed; no natural armor (high-value target) |

### Cat Healing Rates
| Part Category | Bruised→Healthy | Wounded→Bruised | Mangled→Wounded | Permanent? |
|---------------|----------------|-----------------|-----------------|------------|
| Soft tissue (ears, belly, scruff) | 30 ticks | 80 ticks | 200 ticks | Ear tips: permanent scar if Destroyed |
| Structural (haunches, paws) | 50 ticks | 150 ticks | 400 ticks | Haunches: permanent limp if Destroyed |
| Sensory (whiskers, mouth) | 40 ticks | 120 ticks | 300 ticks | Whiskers regrow; jaw permanent if Destroyed |
| Throat | 40 ticks | 100 ticks | N/A | Fatal before Mangled without treatment |
| Tail | 30 ticks | 80 ticks | 200 ticks | Permanent crook if Destroyed |

### Cat Pain System
| Part | Pain Weight | Notes |
|------|------------|-------|
| Throat | 3.0 | Extremely painful; drives incapacitation |
| Haunches | 2.0 | Structural pain |
| Flanks | 1.5 | Rib pain |
| Paws (each) | 1.0 | Moderate |
| Ears, Whiskers, Tail | 0.5 | Low pain relative to function loss |
| Belly, Scruff | 0.8 | Moderate |
| Mouth/Jaw | 1.5 | Bone pain |

---

## Fox / ShadowFox Anatomy — 8 Body Parts

```
HEAD
├── Muzzle/Jaw  — primary bite weapon; eating
└── Ears        — acute hearing; prey detection; threat detection

THROAT          — critical; bleeding if wounded

TORSO
├── Flanks      — sides/ribs; structural armor
└── Belly       — soft underside; exposed when pinned

LIMBS (bilateral — each side collapsed to one zone)
├── Front Paws  — grip and grapple; movement
└── Haunches    — rear legs; pursuit speed, spring

TAIL            — balance, territory signaling
```

Fox wounds are **persistent across encounters** within a session. A fox that retreated with a wounded haunch returns slower next raid. ShadowFox carries the same anatomy with corruption applied to injured parts (see below).

### Fox Functional Consequences by Part
| Part | Wounded | Mangled / Destroyed |
|------|---------|---------------------|
| Muzzle/Jaw | -30% bite damage, -25% hunting yield | Can't bite; forced to flee rather than fight |
| Ears | -25% prey detection range | Near-deaf to approaching cats; ambush resistance lost |
| Throat | Bleeding: health drain 0.03/tick | Fatal without retreat |
| Flanks | -15% defense vs torso hits | Internal damage; -30% defense |
| Belly | Pain only | No defense if pinned (belly exposed during cat pile-on) |
| Front Paws | -20% grapple success, -15% movement | Can't grapple; significant movement penalty |
| Haunches | -35% pursuit speed, -40% spring/pounce | Can't pursue fleeing cats; drags rear |
| Tail | Balance disruption | No persistent penalty (not an identity-forming injury for foxes) |

### ShadowFox Extension
All fox zones apply. Additionally:
- A ShadowFox wound at Wounded+ leaves a **corruption patch** on adjacent tiles each tick.
- Destroying a ShadowFox part does not create a permanent scar; instead, the part reforms at Wounded condition after `shadow_fox_part_reform_ticks` (corruption cost to ecosystem).
- Shadow Fox banishment requires all parts at Wounded+ simultaneously (pain threshold + enough cats in posse).

### Fox Healing Rates
| Part Category | Bruised→Healthy | Wounded→Bruised | Mangled→Wounded |
|---------------|----------------|-----------------|-----------------|
| Soft (ears, belly) | 80 ticks | 250 ticks | 600 ticks |
| Structural (haunches, paws) | 120 ticks | 400 ticks | 900 ticks |
| Throat | 80 ticks | 200 ticks | N/A (retreat or die) |
| Muzzle/Jaw | 100 ticks | 300 ticks | 700 ticks |
| Tail | 60 ticks | 150 ticks | 350 ticks |

Foxes heal faster than cats when out of combat (den resting accelerates healing by 2×).

### Fox Pain System
| Part | Pain Weight |
|------|------------|
| Throat | 3.0 |
| Haunches | 2.0 |
| Flanks | 1.8 |
| Muzzle/Jaw | 1.5 |
| Front Paws | 1.0 |
| Ears, Belly, Tail | 0.6 |

---

## Hawk Anatomy — 8 Body Parts

```
HEAD
├── Beak        — tear and grip; also carried-prey killing bite
└── Eyes        — exceptional range; primary hunt detection sense

BODY
└── Breast/Keel — structural core; armor for vitals; primary body mass

WINGS (bilateral — tracked separately; grounding requires both Mangled+)
├── Left Wing
└── Right Wing

TALONS (bilateral — tracked separately; each is an independent weapon)
├── Left Talon
└── Right Talon

TAIL FEATHERS   — aerial maneuverability; dive-angle control
```

The bilateral wing/talon split is load-bearing: one Mangled wing impairs flight but does not ground the hawk. Both Mangled wings ground it (it cannot attack from altitude, becomes a ground-melee combatant). Individual talons matter for multi-target grabbing.

### Hawk Functional Consequences by Part
| Part | Wounded | Mangled / Destroyed |
|------|---------|---------------------|
| Beak | -25% carry-kill damage | Can't kill carried prey; forced to release |
| Eyes | -30% detection range | Near-blind at range; can't initiate dive from altitude |
| Breast/Keel | -15% defense | Vitals exposed; -35% defense |
| Left Wing | -20% flight speed, -15% altitude | Impaired aerial attack arc |
| Right Wing | -20% flight speed, -15% altitude | Impaired aerial attack arc |
| Both Wings (Mangled+) | — | Grounded: loses altitude attack, fights as ground predator |
| Left Talon | -40% talon grab damage (left) | Can't grab with left talon |
| Right Talon | -40% talon grab damage (right) | Can't grab with right talon |
| Both Talons (Mangled+) | — | Can't grab prey at all |
| Tail Feathers | -20% dive accuracy | Imprecise dive; wide miss radius |

### Hawk Healing Rates
| Part Category | Bruised→Healthy | Wounded→Bruised | Mangled→Wounded |
|---------------|----------------|-----------------|-----------------|
| Soft (beak, eyes, tail feathers) | 60 ticks | 180 ticks | 450 ticks |
| Structural (breast, wings, talons) | 100 ticks | 300 ticks | 750 ticks |

Hawks do not den-rest; healing occurs while roosting (perched on a landmark or map-edge tree).

### Hawk Pain System
| Part | Pain Weight |
|------|------------|
| Breast/Keel | 2.5 |
| Eyes | 1.5 |
| Wings (each) | 1.5 |
| Talons (each) | 1.0 |
| Beak | 1.2 |
| Tail Feathers | 0.3 |

---

## Snake Anatomy — 3 Body Parts

```
HEAD    — fangs, venom gland, pit organs (infrared), eyes
BODY    — upper coils; constriction and strike range; locomotion
TAIL    — rear grip; anchor for constriction hold
```

Snake anatomy is deliberately simple. The venom delivery system and sensory organs are co-located in the head — a Wounded head loses both. Constriction ability depends on body coil integrity.

### Snake Functional Consequences by Part
| Part | Wounded | Mangled / Destroyed |
|------|---------|---------------------|
| Head | -40% venom yield per bite, -30% pit-organ range | Venom delivery disabled; near-blind; bite = minor scratch only |
| Body | -25% movement speed, -30% constriction hold | Can't constrict; sluggish movement |
| Tail | -20% constriction stability | Constriction hold breaks on any movement by prey |

### Snake Healing Rates
| Part | Bruised→Healthy | Wounded→Bruised | Mangled→Wounded |
|------|----------------|-----------------|-----------------|
| Head | 40 ticks | 120 ticks | 300 ticks |
| Body | 50 ticks | 150 ticks | 400 ticks |
| Tail | 40 ticks | 100 ticks | 250 ticks |

### Snake Pain System
| Part | Pain Weight |
|------|------------|
| Head | 2.5 |
| Body | 1.5 |
| Tail | 0.8 |

---

## Prey Anatomy — 3-Zone Simplified Model

Prey animals (Mouse, Rat, Rabbit, Fish, Bird) use a simplified model. Their zones drive **hunt mechanics and meat yield**, not identity or long-term narrative. Condition is 3-tier: **Healthy / Wounded / Dead**.

```
HEAD    — vital zone; the killing catch; quick kill if targeted
BODY    — meat zone; wounds reduce food yield on carcass
LEGS    — movement zone; wounds reduce flee speed and escape success
         (Bird: Wings; Fish: Fins/Tail — functionally equivalent)
```

A Wounded prey entity is not removed from the sim immediately. It persists as a **wounded prey** that:
1. Moves slower (flee speed × `wounded_prey_flee_speed_multiplier`, default 0.6)
2. Has lower alertness ceiling (max alertness × 0.7 — distracted by injury)
3. Yields reduced food on kill (`body_wound_yield_penalty` applied to carcass if Body is Wounded)

Prey heal from Wounded → Healthy after `prey_wound_recovery_ticks` (default: 600 ticks — roughly half a sim-day). They do not receive treatment; recovery is passive.

### Prey Functional Consequences by Zone
| Zone | Wounded | Dead |
|------|---------|------|
| Head | No change (near-instant kill on Head hit) | Entity despawned; carcass spawned |
| Body | -15% food yield on carcass | Full carcass yield penalty applied |
| Legs/Wings/Fins | -40% flee speed, -30% escape roll | — |

### Prey Zone Combat Targeting (cat attacking prey)
| Attack Type | Primary Target | Secondary |
|-------------|---------------|-----------|
| Pounce / bite | Head (0.55) | Body (0.30), Legs (0.15) |
| Claw rake | Legs (0.50) | Body (0.35), Head (0.15) |
| Grab-and-bite | Head (0.70) | Body (0.30) |

A **Legs-wounded** prey that escapes leaves a scent trail with a `wounded` tag — future hunt DSE versions can prioritize wounded-prey cells.

### Wounded-Prey Follow-Up (implicit IAUS flow)

No new DSE work is required for cats to naturally prioritize wounded prey. The `hunt_target_dse` already reads `alertness` as a scored consideration — lower alertness yields a higher score. A Legs-wounded rabbit with alertness ceiling × 0.7 **automatically scores better than a healthy rabbit at the same distance**, without any explicit "prefer wounded prey" axis. The IAUS does the right thing for free.

The flee-speed penalty compounds this: the wounded rabbit is both easier to catch (lower alertness) and slower to escape (reduced flee speed). A cat that wounds but misses creates a near-guaranteed follow-up opportunity on the next approach.

Two levels of memory integration are possible for Phase 3:

| Level | Mechanism | Notes |
|-------|-----------|-------|
| Lightweight | Wounded-prey scent tag on the cell (ticket 062 integration) | Passive — any cat hunting in that cell gets the alertness-reduced read naturally |
| Rich | `WoundedPreyMemory` on the cat that made the wound | Explicit entity tracking; enables narrative ("Cedar circles back, remembering the limping rabbit"); more expensive |

---

## Combat Targeting Weights (all species)

### Predators → Cats
| Attacker | Primary Targets (weight) | Secondary (weight) |
|----------|------------------------|-------------------|
| Cat (scratch) | Ears (0.25), Whiskers (0.2), Paws (0.2) | Flanks (0.2), Tail (0.15) |
| Cat (bite) | Throat (0.25), Scruff (0.2), Ears (0.15) | Paws (0.2), Tail (0.2) |
| Cat (bunny-kick) | Belly (0.4), Haunches (0.3) | Rear Paws (0.2), Tail (0.1) |
| Fox | Throat (0.35), Flanks (0.25), Haunches (0.15) | Paws (0.15), Ears (0.1) |
| Hawk | Ears (0.25), Whiskers (0.2), Scruff (0.2) | Flanks (0.2), Tail (0.15) |
| Snake | Front Paws (0.3), Whiskers (0.2), Mouth (0.15) | Rear Paws (0.2), Tail (0.15) |
| Shadow Fox | Same as Fox + corruption applied to wounded part |

### Cats → Predators
| Defender | Cat Primary Targets (weight) | Cat Secondary (weight) | Notes |
|----------|------------------------------|------------------------|-------|
| Fox | Muzzle/Jaw (0.30), Haunches (0.25) | Flanks (0.25), Ears (0.20) | Disarm bite, slow pursuit |
| ShadowFox | Muzzle/Jaw (0.25), Flanks (0.30) | Haunches (0.25), Ears (0.20) | Flanks prioritized — corruption patch on flanks disrupts the largest zone |
| Hawk (grounded) | Left Talon (0.25), Right Talon (0.25) | Breast/Keel (0.25), Beak (0.25) | Disable talons first |
| Hawk (aerial) | Tail Feathers (0.40), Wings (0.40) | — (0.20) spread to remaining | Deflect dive, impair arc |
| Snake | Head (0.55), Body (0.30) | Tail (0.15) | Pin and bite the head |

### Zone Hit Difficulty

Not all targets are equally reachable. A throat lunge is hard to land; clipping an ear is not. Zone difficulty (0.0 = nearly impossible, 1.0 = trivial) acts as a cap on hit probability regardless of attacker skill — a masterful cat still can't reliably bite a tucked throat.

**Cat body zones (when being attacked):**
| Zone | Difficulty | Notes |
|------|-----------|-------|
| Throat | 0.25 | Small, instinctively tucked; the premium high-risk target |
| Belly | 0.35 baseline / 0.70 supine | Guarded upright; fully exposed in bunny-kick position |
| Scruff | 0.45 | Requires a grip before the bite lands |
| Mouth/Jaw | 0.55 | Counter-bite risk from defender raises effective difficulty |
| Front Paws | 0.60 | Moving target; varies with cat's combat activity |
| Rear Paws | 0.60 | Rear approach required |
| Haunches | 0.60 | Large but rear-facing; needs positioning |
| Whiskers | 0.65 | Face-level; exposed but requires close range |
| Flanks | 0.70 | Large torso target; easiest structural zone |
| Ears | 0.75 | Large, lateral, hard to protect |
| Tail | 0.75 | Easy to clip; low payoff |

**Predator zones (when cats attack):**
| Zone | Difficulty | Notes |
|------|-----------|-------|
| Snake Head | 0.30 | Fast-retracting, low profile — requires a pin |
| Fox Throat | 0.30 | Fox keeps chin tucked in active combat |
| Hawk Talons (aerial) | 0.20 | Nearly unreachable in flight |
| Hawk Talons (grounded) | 0.65 | Accessible once hawk can't dodge |
| Hawk Wings (aerial) | 0.50 | Large surface but fast-moving |
| Hawk Wings (grounded) | 0.75 | Pinned, large target |
| Fox Muzzle/Jaw | 0.40 | Forward-facing; active weapon makes it a risky grab |
| Fox Haunches | 0.60 | Large, rear; achievable with flanking |
| Fox Flanks | 0.65 | Large torso; easiest fox target |
| Snake Body | 0.55 | Continuous surface; hard to miss, hard to damage meaningfully |
| Snake Tail | 0.60 | Accessible from rear |

### Zone Adjacency Groups

When a hit drifts off the intended zone, it lands in the anatomically adjacent region — not a random part of the body. A lunge for the throat that goes slightly wrong clips the scruff or jaw, not the haunches. Drift is bounded by contact geometry.

| Group | Members | Typical drift destination |
|-------|---------|---------------------------|
| Head | Throat, Scruff, Ears, Whiskers, Mouth/Jaw | Within head group, weighted toward large zones (Ears, Jaw) |
| Torso | Flanks, Belly | Flanks ↔ Belly; both are reachable from a body hit |
| Rear | Haunches, Rear Paws, Tail | Within rear group |
| Front | Front Paws | Drifts to Head (whiskers) or Torso (flanks) |

Head group is the most consequential drift zone: a missed throat bite (0.25 difficulty) almost always drifts to ears, scruff, or jaw — all of which still deal useful damage.

### Attack Style Selection

The three cat attack styles (scratch, bite, bunny-kick) have distinct targeting weight distributions but `CurrentAction` today is just `Action::Fight` with no style state. The implementation needs a **per-tick weighted roll over styles** driven by personality.

There is no binary hit/miss. Damage always lands somewhere — the contest determines whether it lands on the intended zone or drifts. See the contested formula in Shared Formulas.

| Style | Personality Prior | Accuracy Bonus | Base Tissue Damage | Zone Drift Behavior |
|-------|------------------|---------------|-------------------|---------------------|
| Scratch | Default / baseline | +0.30 | 0.08 – 0.12 | Low drift; connects reliably with something nearby |
| Bite | High `temper` | +0.00 | 0.20 – 0.35 | Higher drift on poor contest; lands in head group but not necessarily the target zone |
| Bunny-kick | High `boldness` | +0.20 (belly/haunch only) | 0.15 – 0.25 | **Bilateral** — both parties take damage regardless of contest result; see below |

The style roll should happen once per combat tick, not per encounter — a cat cycles through instinctive attack patterns rather than choosing a strategy. Personality biases the prior but doesn't lock it.

**Bunny-kick is structurally different from the other two styles.** When a cat goes supine and locks its foreclaws into the opponent, both parties are grappling and clawing simultaneously. The contest result doesn't determine *whether* damage happens to each side — it determines the *ratio*: the winning cat deals more tissue damage to the intended zone; the losing cat still takes damage to its Belly from the opponent's claws. A bold cat going belly-up against a fox isn't gambling on a hit — it's committing to a mutual exchange where the upside is rear-claw damage to belly/haunches and the downside is a belly wound that was always going to happen to some degree.

---

## IAUS Integration

Three places where body zone state feeds back into the IAUS scoring layer. These are the load-bearing joints between the body zone data model and the decision system.

### 1. `threat_power` becomes dynamic

`fight_target_dse` scores threats via `WildAnimal.threat_power`, a flat constant today. Post-body-zones, effective threat power scales with key-part condition:

| Species | Key Part | Effect on `threat_power` |
|---------|----------|---------------------------|
| Fox | Muzzle/Jaw | Muzzle Mangled+ → threat_power × 0.5; Destroyed → × 0.2 |
| Hawk | Talons (both) | Both Mangled → threat_power × 0.4 |
| Snake | Head | Head Mangled → threat_power × 0.3 |

Because `fight_target_dse` uses a Quadratic curve on `threat_level_normalized`, a modest drop in threat_power produces a large drop in scoring (convex amplification works both ways). A fox with a Mangled muzzle will naturally draw fewer cats to engage it — not because of explicit memory, but because the IAUS perceives it as less urgent. Cats will still fight it if it approaches, but they won't mobilize a posse proactively.

This creates **persistent-wound payoff** without explicit inter-encounter memory: a colony that wounded a fox during its last raid will engage the damaged fox more casually on the next one.

### 2. `health_derived` feeds `combat_advantage_normalized`

The combat advantage formula is `skills.combat + self_health_fraction − target_threat_level`. Once `health_derived` replaces the raw `Health.current` float, an injured cat automatically sees its own combat advantage drop mid-fight — making the morale-flee check trigger sooner.

The compounding risk: a cat with Mangled haunches has both lower `health_derived` (pain) and lower movement speed. When its morale breaks it can't escape as fast. This is a genuine trap state that emerges from formula interactions with no new logic required.

**Breaking change to track:** When Phase 1 lands, `combat_advantage_normalized` in `fight_target.rs` must read `health_derived` (pain-based) rather than `Health.current`. That's a behavior change — injured cats will disengage sooner than today — and needs a soak comparison.

### 3. Zone-based early retreat

Today predators flee on flat HP threshold or outnumber count. Body zones add a third trigger: **key weapon destroyed**. When a predator's primary attack part reaches Mangled+, it fires a retreat check immediately regardless of HP:

```
key_weapon_broken_check(entity):
    if entity is Fox/ShadowFox AND Muzzle.condition >= Mangled:
        emit FleeMessage  # can't bite; nothing left to fight with
    if entity is Hawk AND Left Talon.condition >= Mangled
                      AND Right Talon.condition >= Mangled:
        if both Wings also >= Mangled: # grounded; can't flee upward
            switch to WildlifeAiState::Waiting (cornered ground fighter)
        else:
            emit FleeMessage
    if entity is Snake AND Head.condition >= Mangled:
        emit FleeMessage  # venom disabled; no threat vector left
```

This makes individual fights more volatile — they don't necessarily end when HP drains; they can end abruptly when the right part is hit. It also creates the grounded-hawk state transition: both wings Mangled means the hawk can't flee upward, it's trapped, and the cats have to finish it at ground range.

---

## Shared Formulas

```
total_pain(entity) = sum(part.tissue_damage * part.pain_weight for part in entity.body_parts)

health_derived(entity) = 1.0 - (total_pain / max_possible_pain)

# Zone-drift combat model.
# Damage always lands somewhere. The contest determines *where*, not whether.
combat_tick(attacker, defender, tick):

    # 1. Roll attack style (cat attackers only; predators use fixed style)
    style = weighted_style_roll(attacker.personality)  # Scratch / Bite / BunnyKick

    # 2. Intended target zone from style's targeting weight table
    intended = weighted_random(targeting_weights[style][defender_species])

    # 3. Contest result — continuous margin, not a binary
    attacker_score = attacker.combat_skill + style_accuracy_bonus[style] + jitter()
    defender_score = defender.base_defense + jitter()
    margin = attacker_score - defender_score  # positive = attacker winning

    # 4. Resolve landing zone
    #    precision = how reliably the attack reaches the intended zone.
    #    High zone difficulty + poor margin = high probability of drift.
    #    Drift always stays within the anatomical group (see Zone Adjacency Groups).
    precision = sigmoid(margin) * zone_hit_difficulty[intended]
    if random() < precision:
        landing = intended
    else:
        landing = weighted_random(adjacent_zones[intended])  # stays in same group

    # 5. Tissue damage — scales with margin; always at least a floor
    #    A dominant contest: near-full style_base_damage.
    #    A losing contest: glancing blow, maybe 20-30% of base.
    #    Never zero.
    damage_scale = sigmoid(margin)  # [0, 1]; 0.5 at parity
    raw_damage = style_base_damage[style] * damage_scale.max(min_glancing_factor)
    landing = armor_redirect(landing, defender)
    landing.tissue_damage += raw_damage
    landing.condition = condition_from_damage(landing.tissue_damage)
    emit BodyPartInjury { part: landing, condition, tissue_damage: raw_damage, tick }

    # 6. Bunny-kick: bilateral exchange
    #    The contest determines the damage ratio, not who gets hit.
    #    Both parties are always clawing.
    if style == BunnyKick:
        defender_damage = style_base_damage[BunnyKick] * (1.0 - damage_scale).max(min_glancing_factor)
        attacker.belly.tissue_damage += defender_damage  # attacker's belly always exposed
        attacker.belly.condition = condition_from_damage(attacker.belly.tissue_damage)
        emit BodyPartInjury { part: attacker.belly, tissue_damage: defender_damage, tick }

# Predator attacking cat uses the same model:
#   attacker_score = threat_power * key_part_modifier + jitter()
#   style fixed per species (Fox→Bite, Hawk→Talon, Snake→Strike)
#   defender_score = cat.combat_skill * health_derived_modifier + jitter()
#   zone drift stays within the same anatomical groups

armor_redirect(zone, defender):
    # Internal hit blocked by armor part if armor is still intact
    if zone.is_internal AND defender.armor_part(zone).condition < Wounded:
        return defender.armor_part(zone)
    return zone

healing(part, ticks_elapsed, entity_type):
    rate = healing_rate[entity_type][part.category][part.condition]
    part.tissue_damage -= (1.0 / rate)
    if part.tissue_damage < condition_threshold(part.condition - 1):
        part.condition = part.condition - 1
    if part.is_permanent_at(Destroyed) AND part.condition == Destroyed:
        part.permanent = true

predator_retreat_check(entity):
    if total_pain(entity) > retreat_threshold[entity.species]:
        emit FleeMessage
        set WildlifeAiState::Fleeing

prey_wound(zone, tissue_damage):
    # Prey 3-tier: Healthy → Wounded → Dead
    if zone == Head AND tissue_damage > 0.4:
        despawn(prey); spawn(carcass)  # kill on significant head hit
    elif tissue_damage > 0.3:
        prey.condition[zone] = Wounded
        apply_flee_speed_penalty / yield_penalty as appropriate
```

---

## Cat Narrative Integration
Each body part has narrative templates for injury:
- Whiskers: "the fox's snap tore away {name}'s whiskers"
- Ears: "{name}'s right ear was shredded in the scuffle"
- Throat: "the fox's jaws closed around {name}'s throat"
- Paws: "{name} limps badly, favoring the wounded forepaw"
- Tail: "{name}'s tail hangs at an odd angle"
- Haunches: "{name} drags herself forward, haunches torn"

Permanent injuries become identity traits — shown in TUI inspect view and referenced in future narrative templates.

Predator injuries do not generate identity narrative but do generate **encounter narrative**:
- Fox muzzle Wounded: "Cedar's claws found the fox's muzzle — it yelped and retreated"
- Hawk grounded (both wings Mangled): "the hawk dropped, wings broken, unable to rise"

---

## Migration from Current System

### Cat migration
| Current | New |
|---------|-----|
| `Health.current: f32` | Derived: `1.0 - (total_weighted_damage / max_weighted_damage)` |
| `Health.injuries: Vec<Injury>` | `Vec<BodyPartInjury>` with `part: BodyPart`, `condition: PartCondition`, `tissue_damage: f32` |
| `InjuryKind` (Minor/Moderate/Severe) | `PartCondition` (Healthy/Bruised/Wounded/Mangled/Destroyed) |
| `damage_to_injury()` in combat.rs | `damage_to_body_part()` — selects target via weighted random per attacker type |
| `is_incapacitated` check in ai.rs | Pain threshold check: `total_pain > 0.9` |

### Predator migration
| Current | New |
|---------|-----|
| `WildAnimal.defense: f32` | Derived: `1.0 - (total_pain / max_possible_pain)` |
| `WildAnimal.threat_power: f32` | Scales with key-part health: muzzle/talon/head condition modifies base `threat_power` |
| Flat damage in `resolve_combat` | `damage_to_body_part()` with predator-specific targeting weights |
| No retreat model | Pain-threshold retreat check emits `FleeMessage` |

### Prey migration
| Current | New |
|---------|-----|
| Prey despawned on kill | Head Wounded → kill; Legs/Body Wounded → wounded prey persists |
| No partial-capture state | `PreyState.wound_zones: [Option<WoundTier>; 3]` |
| Fixed flee speed | Legs wound applies `wounded_prey_flee_speed_multiplier` |

---

## Implementation Phases

| Phase | Scope | Depends On |
|-------|-------|------------|
| 1 — Cat zones | Full 13-part model for cats; migrate `Health` | Combat system (already landed) |
| 2 — Predator zones | Fox/ShadowFox (8 parts); Hawk (8 parts); Snake (3 parts) | Phase 1; ticket 025 (Hawk/Snake GOAP) for Hawks/Snakes to act on their own injuries |
| 3 — Prey zones | 3-zone prey model; wounded-prey persistence | Phase 1; ticket 062 (prey scent maps) to tag wounded-prey scent cells |

---

## Combat Stakes

Body zones add three qualitatively new categories of high-stakes combat outcome that don't exist in the current flat-HP model.

### Permanent injury risk

A cat whose haunches are Destroyed comes out of that fight permanently impaired — slower forever, reduced jumping, limited pounce. Every serious fight now has a tail risk beyond "health drops, heals in 400 ticks." Players will watch a fight differently when there's a chance Cedar comes out of it lame for life. The TUI inspect view and future narrative templates reference permanent injuries as identity markers, reinforcing that this cat's history is written on its body.

This also changes the calculus on sending a particular cat into a dangerous situation. A young cat in peak condition is a different resource than an elder with a Destroyed haunch.

### The grounded hawk

Both wings Mangled is a **discrete state transition** — the hawk drops, can no longer attack from altitude, and is trapped at ground range. This is recognizable to the player as a pivotal moment: "we have it now" or "we're fighting a cornered predator at melee range with no escape route."

The bilateral wing model exists specifically to create this transition. A one-wing-impaired hawk is still an aerial threat; a two-wing-impaired hawk is a fundamentally different creature. The player watches a hawk get hit once, wonders if the colony can hit the other wing before it recovers, and feels the stakes of that race.

### Cross-encounter wound accumulation

A fox that raids twice and gets its muzzle Wounded during the second encounter doesn't come back at full threat_power for the third. The colony is **slowly degrading a persistent predator** across multiple encounters without necessarily killing it — a very different narrative arc than a HP bar.

The inverse is equally interesting: a fox that never takes meaningful damage over many raids is genuinely more threatening because it hasn't been worn down. Individual predators develop divergent threat profiles based on their encounter history with that specific colony.

Neither of these outcomes requires player-controlled mid-fight choices. The stakes emerge from persistent body state and the IAUS signal flow described above — which fits the no-director design philosophy. The player watches, the drama comes from the system being honest.

---

## Tuning Notes
_Record observations and adjustments here during iteration._
