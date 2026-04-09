# Personality Stat Integration Design

## Context

The personality system defines 18 axes across 3 layers (core drives, temperament,
values). Eight of these stats are partially or fully unused in gameplay mechanics:
**temper**, **stubbornness**, **playfulness**, **patience**, **tradition**,
**pride**, **ambition**, and **independence**. Some have partial wiring (ambition
scales respect need decay; tradition/pride/independence feed value compatibility;
independence reduces directive weight). This design wires all eight into active
mechanics with chain-reaction cascades.

**Design philosophy:** emergent complexity through interacting systems. Each stat
touches 2-3 systems so interactions compound into Dwarf Fortress-style cascades.
Build on the existing `docs/systems/personality.md` spec.

## Architecture: Two Layers

### Layer 1 — Continuous Modifiers

Each stat gets multiplier hooks in `scoring.rs`, `mood.rs`, `needs.rs`,
`combat.rs`, and/or `social.rs`. These run every tick and create the everyday
texture of personality. No event needed — this is ongoing behavioral bias.

### Layer 2 — Observable Events

When personality threshold + situational trigger align, systems emit Bevy events.
Events are things another cat could witness or a narrator could describe — "Bramble
snapped at Clover" is an event; hunger ticking down is not.

Events follow the CLAUDE.md convention: past-tense verbs, defined in a central
module (`src/events/personality.rs`).

```rust
// src/events/personality.rs
TemperFlared { cat: Entity, target: Option<Entity> }
DirectiveRefused { cat: Entity, coordinator: Entity }
PlayInitiated { cat: Entity }
PrideCrisis { cat: Entity }
LeadershipChallenge { challenger: Entity, coordinator: Entity }
TraditionBroken { cat: Entity, location: TilePos }
WentSolo { cat: Entity }
```

Downstream systems (mood, social, narrative, AI) read events and react
independently. The emitter does not know who is listening.

---

## Stat Designs

### Temper

**Continuous modifiers:**

- **Scoring** (`scoring.rs`): When physiological satisfaction < 0.5, temper adds a
  negative modifier to Socialize and Groom(other):
  `-(temper * 0.3 * (1.0 - phys_satisfaction))`.
  High temper + hungry = socializing more likely to go badly.

- **Combat** (`combat.rs`): Temper adds to cat attack damage:
  `cat_damage *= 1.0 + temper * 0.15`. Also modifies morale — replace the
  `boldness * 0.3` morale component with `boldness * 0.2 + temper * 0.1`
  (preserves most of boldness's contribution while adding temper).

- **Mood** (`mood.rs`): Temper amplifies negative modifiers when needs are unmet:
  `amplified_negative *= 1.0 + temper * 0.3 * (1.0 - phys_satisfaction)`.
  When fed and safe, temper is dormant.

**Event — `TemperFlared`:**

- Trigger: each tick, if `phys_satisfaction < 0.4 AND mood.valence < -0.3`,
  roll `temper * 0.08` chance.
- Cascade (The Tantrum Spiral):
  1. Fondness penalty (-0.05) toward nearest cat within 3 tiles
  2. Target gets MoodModifier(-0.2, 20 ticks, "snapped at by {name}")
  3. Mood contagion spreads negative mood to neighbors
  4. If neighbors have high temper, recursive flare chance
  5. Narrative: "{name} hisses at {target} for no reason anyone can name."

---

### Patience

**Continuous modifiers:**

- **Disposition** (`disposition.rs`): Patience adds to target_completions for all
  disposition kinds: `base_target + (patience * 1.0).round() as u32`. Stacks with
  the existing per-kind personality scaling.

- **Scoring** (`scoring.rs`): When a cat has an active disposition, actions within
  that disposition get a commitment bonus: `+patience * 0.15`.

- **Mood** (`mood.rs`): When a positive mood modifier is added to a patient cat,
  extend its duration: `ticks_remaining += (patience * ticks_remaining as f32 *
  0.3).round() as u64`. At patience=1.0, +30% duration on positive moods.
  Applied at modifier creation, not per-tick. Negative modifiers are unaffected —
  patience does not prolong suffering.

- **Needs** (`needs.rs`): Patience reduces purpose need decay:
  `purpose_drain *= 1.0 - patience * 0.3`. Patient cats do not feel purposeless as
  quickly.

**No observable event.** Patience is a modulator, not a trigger. Its drama comes
from its absence (impatient cats switching dispositions constantly).

**Interaction with temper:** Patience does NOT suppress temper flare probability
(independent additive axes). A patient high-temper cat has the same flare chance as
an impatient high-temper cat. The difference: the patient cat stays in its
disposition longer between flares.

---

### Stubbornness

**Continuous modifiers:**

- **Directive compliance** (`ai.rs`/`scoring.rs`): Add to existing directive bonus
  formula: `* (1.0 - stubbornness * 0.4)`. Stacks with independence reduction.
  Independence = principled autonomy. Stubbornness = resistance to changing course.

- **Disposition switching**: When AI evaluates a new disposition while one is
  active, competing dispositions get: `score * (1.0 - stubbornness * 0.3)`.
  Stubborn cats stay the course. Independent of patience (patience adds commitment
  bonus to current actions; stubbornness penalizes alternatives).

- **Social influence** (`mood.rs`): Stubbornness reduces mood contagion received:
  `influence *= 1.0 - stubbornness * 0.2`. Stubborn cats are emotionally
  thick-skinned.

**Event — `DirectiveRefused`:**

- Trigger: when a directive targets a cat with stubbornness > 0.7, roll
  `(stubbornness - 0.5) * 0.6` chance of outright rejection (0% at 0.5, 30% at
  1.0, matching personality.md spec for stubbornness > 0.85).
- Cascade (The Stubborn Standoff):
  1. Coordinator gets MoodModifier(-0.15, 20 ticks, "directive ignored by {name}")
  2. Loyal cats within 5 tiles (fondness > 0.3 toward coordinator) get
     MoodModifier(-0.08, 15 ticks, "saw {name} ignore the coordinator")
  3. Coordinator fondness toward refuser: -0.03. Loyal bystanders: -0.01.
  4. Cat stays in current disposition (the refusal is the point).
  5. Narrative: "The coordinator calls for {name} to join the patrol. {name}
     flicks an ear and goes back to foraging."

**Behavior gate:** Stubbornness > 0.85 → 30% directive rejection chance (naturally
produced by the formula above).

---

### Playfulness

**Continuous modifiers:**

- **Scoring** (`scoring.rs`): Playfulness boosts Socialize (`+playfulness * 0.3`)
  and Wander (`+playfulness * 0.2`). Idle penalty: `-playfulness * 0.05`.

- **Social quality** (`actions.rs`): During Socialize, playfulness adds a positive
  fondness delta: `fondness_delta += initiator.playfulness * 0.005`.

- **Disposition** (`disposition.rs`): Socializing target_completions:
  `1 + (sociability * 2.0 + playfulness * 1.0).round() as u32`.

- **Mood** (`mood.rs`): Successful Socialize mood bonus amplified by playfulness:
  `base_bonus * (1.0 + playfulness * 0.3)`.

**Event — `PlayInitiated`:**

- Trigger: during Socialize, if playfulness > 0.6 AND mood.valence > 0.0, roll
  `playfulness * 0.1` chance.
- Cascade (The Play Epidemic):
  1. All cats within 4 tiles: MoodModifier(+0.1, 15 ticks, "watched play nearby")
  2. Cats within 4 tiles: +0.12 to Socialize score (stronger than normal +0.08
     cascading bonus)
  3. If a second cat joins and has playfulness > 0.6, recursive PlayInitiated check
  4. Participating cats enter "chase" sub-behavior — alternate following each
     other's positions across tiles, creating visible movement on the map
  5. Play self-limits: social need satisfies at +0.05/tick while playing
  6. Narrative: "A game breaks out near the old oak. {name} bats a pinecone toward
     {other_name}." / "{name} and {other_name} tear across the meadow, trading the
     lead."

**Incompatibility:** Diligence > 0.8 + Playfulness > 0.8 within 3 tiles → fondness
-0.001/tick.

---

### Pride

**Already wired:** Value compatibility axis in `social.rs`.

**New continuous modifiers:**

- **Needs** (`needs.rs`): When respect < 0.4, pride amplifies respect decay:
  `respect_drain *= 1.0 + pride * 0.8 * (1.0 - respect / 0.4)`. At respect=0,
  pride=1.0: 1.8x drain rate. Matches personality.md spec.

- **Scoring** (`scoring.rs`): When respect < 0.5, pride boosts status-granting
  actions (Hunt, Fight, Patrol, Build, Coordinate): `+pride * 0.1`.

- **Mood** (`mood.rs`): When respect < 0.3, per-tick modifier:
  `MoodModifier(-(pride * 0.15), 1 tick, "wounded pride")`.

**Event — `PrideCrisis`:**

- Trigger: respect < 0.2 AND pride > 0.6. Emit once per 100-tick cooldown.
- Cascade (The Pride Spiral):
  1. Temporary +0.25 to Hunt/Fight/Patrol/Coordinate for 50 ticks
  2. If near another cat with ambition > 0.6: fondness -0.04 (rivalry under
     status pressure)
  3. If desperate status-seeking fails (lost fight, failed hunt), respect drops
     further → recursive crisis
  4. Narrative: "{name}'s tail lashes. Nobody seems to notice what {name} has done
     for this colony."

---

### Ambition

**Already wired:** Scales respect need decay, maps to Leadership aspiration domain.

**New continuous modifiers:**

- **Scoring** (`scoring.rs`): Ambition boosts Coordinate even for non-coordinators:
  `+ambition * 0.2 * level_suppression(4)`. Also boosts Mentor:
  `+ambition * 0.1`.

- **Coordinator election** (`coordination.rs`): Add ambition as a factor in
  coordinator election scoring alongside social_weight, diligence, sociability.

**Event — `LeadershipChallenge`:**

- Trigger: ambitious cat (ambition > 0.7) has higher respect + mastery than
  current coordinator AND fondness toward coordinator < 0.2. Roll
  `ambition * 0.05` per tick.
- Cascade:
  1. Cats loyal to coordinator: negative mood modifier. Cats with low loyalty:
     slight positive modifier (change is exciting).
  2. Challenger gets strong commitment bonus to Coordinate actions.
  3. Narrative: "{name} begins speaking over the coordinator in meetings, voice
     carrying further each day."

**Interaction with pride:** Pride x ambition is the status-obsession axis. High both
→ natural leader or natural tyrant.

---

### Tradition

**Already wired:** Value compatibility axis in `social.rs`.

**New continuous modifiers:**

- **Scoring** (`scoring.rs`): Location preference bonus. When a cat performs an
  action at a tile where it previously succeeded at the same action:
  `+tradition * 0.1`. Requires a per-cat map of `(TilePos, Action) →
  success_count`, capped at ~20 entries. Creates "favorite spots."

- **Disposition** (`disposition.rs`): Traditional cats resist novel dispositions
  they have not held in the last 200 ticks: penalty of `tradition * 0.15`.

- **Needs** (`needs.rs`): Familiar territory (within 5 tiles of most-frequented
  area) gives safety recovery: `safety += tradition * 0.002/tick`. Unfamiliar
  territory drains safety: `safety -= tradition * 0.001/tick`.

**Event — `TraditionBroken`:**

- Trigger: coordinator directive forces traditional cat (tradition > 0.7) to move
  >10 tiles from most-frequented area.
- Cascade:
  1. MoodModifier(-0.15, 30 ticks, "uprooted from familiar ground")
  2. Safety -0.05
  3. If tradition > 0.85, add +0.1 to effective stubbornness for directive
     refusal check (possible DirectiveRefused cascade)
  4. Narrative: "{name} lingers at the edge of the old meadow, looking back."

**Incompatibility:** Tradition > 0.8 + Independence > 0.8 within 3 tiles → fondness
-0.002/tick.

---

### Independence

**Already wired:** Directive weight reduction in `ai.rs`, value compatibility in
`social.rs`.

**New continuous modifiers:**

- **Scoring** (`scoring.rs`): Solo action bonus (Explore, Wander, Hunt):
  `+independence * 0.1`. Group action penalty (Socialize, Coordinate, Mentor):
  `-(independence * 0.1)`.

- **Needs** (`needs.rs`): Purpose need decay modulated:
  `purpose_drain *= 1.0 + independence * 0.4`. Independent cats feel purposeless
  faster — they need to find their own meaning.

- **Disposition** (`disposition.rs`): Penalty for switching to Coordinating or
  Socializing dispositions: `score -= independence * 0.2`.

**Event — `WentSolo`:**

- Trigger: during action selection, if cascading activity bonus was applied to at
  least one action (nearby cats doing the same thing) but the cat
  (independence > 0.7) selected a non-cascading action instead.
- Cascade:
  1. Loyal cats nearby: fondness toward loner -0.01
  2. Independent cat: MoodModifier(+0.1, 10 ticks, "freedom")
  3. Narrative: "The others head for the berry thicket together. {name} turns the
     other way."

**Incompatibility:** Loyalty > 0.8 + Independence > 0.8 during directives → loyal
cat resents independent cat's noncompliance (one-directional).

---

## Personality Incompatibility System

New system `personality_friction` runs each tick. Checks proximity and applies
fondness deltas for trait clashes.

| Cat A | Cat B | Condition | Fondness/tick |
|-------|-------|-----------|---------------|
| Tradition > 0.8 | Independence > 0.8 | Within 3 tiles | -0.002 (symmetric) |
| Diligence > 0.8 | Playfulness > 0.8 | Within 3 tiles | -0.001 (symmetric) |
| Loyalty > 0.8 | Independence > 0.8 | During active directive | -0.002 (loyal → independent only) |
| Ambition > 0.8 | Ambition > 0.8 | Both coordinator-eligible | -0.003 (symmetric) |

---

## Behavior Gates

Hard overrides checked after scoring, before action execution. Implemented as a
`behavior_gate_check()` function.

| Gate | Condition | Effect |
|------|-----------|--------|
| Too timid to fight | boldness < 0.1 | Fight → Flee |
| Too shy to socialize | sociability < 0.15 | Socialize → skip |
| Compulsive explorer | curiosity > 0.9 | 20% chance/tick override directive → Explore |
| Stubborn refusal | stubbornness > 0.85 | 30% directive rejection (via DirectiveRefused) |
| Reckless bravery | boldness > 0.9 | Flee → Fight |
| Compulsive helper | compassion > 0.9 | Override action to aid injured cat within 3 tiles |

---

## Data Requirements

### New Components/Fields

- **Location memory** (for tradition): per-cat map of `(TilePos, Action) →
  success_count`, capped at 20 entries. Stored as a new field on an existing
  component (Memory or a new LocationPreferences component).

- **PrideCrisis cooldown**: per-cat `last_pride_crisis_tick: Option<u64>` to
  enforce 100-tick cooldown.

- **Familiar territory**: per-cat tracking of most-frequented tiles. Could be
  derived from location memory or tracked as a running average position.

### New Systems

- `personality_friction` — incompatibility fondness deltas
- `behavior_gate_check` — post-scoring action overrides
- `personality_event_emitters` — checks conditions and emits TemperFlared,
  DirectiveRefused, PlayInitiated, PrideCrisis, LeadershipChallenge,
  TraditionBroken, WentSolo
- `personality_event_handlers` — reads events and applies downstream effects
  (mood modifiers, fondness changes, narrative entries)

---

## Verification

- **Unit tests per stat**: each continuous modifier has a test that creates two
  cats (high vs low trait value) and asserts the expected scoring/mood/need
  difference.
- **Cascade integration tests**: for each named cascade, set up the triggering
  conditions and assert that downstream effects occur (mood modifier applied,
  fondness changed, narrative entry logged).
- **Behavior gate tests**: assert that extreme-trait cats get action overrides.
- **Incompatibility tests**: place two incompatible cats adjacent and assert
  fondness decay after N ticks.
- **Visual verification**: run the simulation and observe the narrative log for
  cascade events firing. Inspect panel should show personality-driven behavior
  patterns.

---

## Files to Modify

| File | Changes |
|------|---------|
| `src/events/personality.rs` | **New.** Event definitions. |
| `src/ai/scoring.rs` | Temper, patience, playfulness, pride, ambition, tradition, independence modifiers |
| `src/systems/mood.rs` | Temper amplification, patience decay slowdown, playfulness social bonus |
| `src/systems/combat.rs` | Temper damage/morale modifiers |
| `src/systems/needs.rs` | Patience purpose drain, pride respect drain, independence purpose drain, tradition safety modifiers |
| `src/systems/social.rs` | Playfulness fondness delta, personality_friction system |
| `src/systems/ai.rs` | Stubbornness directive reduction, disposition switching penalties |
| `src/components/disposition.rs` | Patience/playfulness/tradition/independence target completion and switching modifiers |
| `src/systems/coordination.rs` | Ambition in coordinator election |
| `src/lib.rs` | Register new systems and events |
| `docs/systems/personality.md` | Update with implemented mechanics |
| `tests/` | New integration tests for cascades |
