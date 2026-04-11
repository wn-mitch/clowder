# Reproduction, Kittens & Grooming Design

**Date:** 2026-04-11
**Status:** Draft

## Context

Generational play is core to Clowder's vision — memory inheritance, cultural accumulation, recipe transmission across generations. None of that works without kittens. This spec adds reproduction as a Maslow need, pregnancy as a physical arc, kittens as dependent entities that grow into full colony members, and the behavioral systems that make parenting an emergent colony-wide activity.

Alongside reproduction, this spec introduces grooming as a persistent physical condition that feeds into esteem and romantic attraction. Grooming ties directly into mating (attractiveness), parenting (grooming kittens), and the depression spirals that emerge from neglected self-care.

The design follows the lifecycle-layered approach: each phase hooks into existing patterns (Needs, Disposition, TaskChain, AI scoring) rather than introducing new architectural concepts.

## 1. Grooming Condition

### New Component: `GroomingCondition`

```rust
#[derive(Component)]
pub struct GroomingCondition(pub f32); // 0.0 (matted/filthy) → 1.0 (pristine)
```

A physical property — not a Maslow need. It decays passively and is restored by grooming actions. Other systems read it to modulate social and esteem outcomes.

### Decay

Rate: `0.00003` per tick. A fully groomed cat (1.0) reaches unkempt territory (~0.3) in about 23,000 ticks (~1 season). Personality `pride` accelerates awareness but not actual decay — instead it affects how much low grooming hurts the pride need (see below).

### Restoration

- **Self-groom** (existing action): +0.15 per groom session
- **Groom-other**: target gets +0.12 per session (slightly less than self — another cat can't groom you as well as you can yourself)
- Capped at 1.0

### Effects on Other Systems

**Pride (L4 Esteem):**
The existing `respect` need gains an additional decay penalty when grooming is low:
```
pride_grooming_penalty = (1.0 - grooming) * pride * 0.00005
```
High-pride cats suffer more from being unkempt. A slovenly cat with low pride doesn't care. This amplifies the depression spiral: depressed cat → stops grooming → pride drops → esteem suppresses purpose → deeper depression.

**Respect from Others (L4 Esteem):**
During social interactions, fondness growth is modulated by the target's grooming state:
```
fondness_delta *= 0.7 + grooming * 0.3
```
A pristine cat (1.0) gets full fondness growth. A filthy cat (0.0) gets only 70%. Not punitive enough to create outcasts, but enough to create a soft social advantage for well-groomed cats.

**Romantic Attraction (L3 Belonging):**
Romantic score growth during social interactions is more sensitive to grooming:
```
romantic_delta *= 0.5 + grooming * 0.5
```
A filthy cat (0.0) earns romantic points at half rate. This directly gates mating: cats that don't groom struggle to reach Partners threshold, making grooming a prerequisite for reproduction without explicit checks.

**Kitten Grooming Bonus:**
Grooming a kitten (via Caretaking disposition) restores the kitten's grooming condition AND gives the groomer a social need boost (+0.05). Caretakers get social satisfaction from tending kittens, creating a soft incentive loop.

### Initialization

All founding cats spawn at 0.8 grooming (slightly below pristine — they've been busy settling). Kittens spawn at 1.0 (freshly groomed by mother at birth).

### Files Modified

- New component `src/components/grooming.rs` — `GroomingCondition`
- `src/components/mod.rs` — re-export
- `src/systems/needs.rs` — grooming decay, pride penalty from low grooming
- `src/systems/social.rs` — modulate fondness/romantic deltas by grooming
- `src/systems/disposition.rs` — update groom step resolution to restore grooming condition
- `src/plugins/setup.rs` — initialize grooming on cat spawn

## 2. Mating Need

### Placement

New `mating: f32` field on `Needs` at Level 3 (Belonging), alongside `social` and `acceptance`.

**Not averaged into `belonging_satisfaction()`.** The mating need is a scoring input for the Mate action only — it does not suppress Level 4/5 needs. This avoids destabilizing the existing suppression hierarchy.

### Decay

Rate: `0.00008 * (1.0 + warmth * 0.5)` per tick. Personality `warmth` (from the 8 core drives) governs how quickly the urge builds.

### Gating Conditions

The need only decays (activates) when ALL of:

1. Cat is `Adult` or `Elder` life stage
2. Season is `Spring` or `Summer`
3. Cat orientation is not `Asexual`
4. Cat is not currently pregnant (`Pregnant` component absent)

When gated off, the need stays at 1.0 (fully satisfied). Asexual cats have a permanent 1.0 — the need never activates.

### Satisfaction

Successful mating resets the need to 1.0. During pregnancy, decay is suppressed (gating condition 4). The need resumes decaying the next fertile season after birth.

### Files Modified

- `src/components/physical.rs` — add `mating` field to `Needs`, update `Default`, update `staggered()`
- `src/systems/needs.rs` — add mating decay with gating logic

## 3. Mating Behavior

### New Action: `Mate`

Added to the `Action` enum in `src/ai/mod.rs`.

### Scoring

```
base = (1.0 - mating_need) * warmth * 1.5 * level_suppression(3)
```

Prerequisites for score > 0:
- Mating need < 1.0 (need is active)
- Cat is `Adult` or `Elder`
- Season is `Spring` or `Summer`
- At least one eligible partner exists: orientation-compatible, `Partners` or `Mates` bond, not pregnant, `Adult` or `Elder`

Existing `apply_fated_bonuses()` extended to boost `Mate` for fated loves.

### New DispositionKind: `Mating`

Target completions: 1 (single mating event per disposition adoption).

### Mating TaskChain

1. **`MoveTo`** — approach partner
2. **`Socialize`** — courtship interaction (romantic +0.05, fondness +0.03)
3. **`GroomOther`** — mutual grooming
4. **`MateWith`** — new `StepKind`. On completion:
   - Carrying cat receives `Pregnant` component
   - Both cats: mating need → 1.0, mood +0.2 for 30 ticks, romantic +0.1
   - Bond may upgrade to `Mates` if thresholds are met from the romantic boost

### Who Carries

- If exactly one partner is a Queen → they carry
- Same-sex pairs or Nonbinary pairs → one randomly selected
- This keeps reproduction available to all orientation-compatible pairs

### Files Modified

- `src/ai/mod.rs` — add `Mate` variant to `Action`
- `src/ai/scoring.rs` — add scoring logic, extend `apply_fated_bonuses()`
- `src/components/disposition.rs` — add `Mating` to `DispositionKind`
- `src/components/task_chain.rs` — add `MateWith` to `StepKind`
- `src/systems/disposition.rs` — add `build_mating_chain()`, handle in `disposition_to_chain()`, `resolve_disposition_chains()`, `aggregate_to_dispositions()`

## 4. Pregnancy Arc

### New Component: `Pregnant`

```rust
#[derive(Component)]
pub struct Pregnant {
    pub conceived_tick: u64,
    pub partner: Entity,
    pub litter_size: u8,
    pub stage: GestationStage,
    pub nutrition_sum: f32,
    pub nutrition_samples: u32,
}

pub enum GestationStage { Early, Mid, Late }
```

`nutrition_sum` / `nutrition_samples` track average queen hunger during pregnancy — this determines kitten starting health at birth.

### Gestation Duration

1 season (`ticks_per_season`, currently 20,000 ticks). Trimester boundaries at 33% and 66%.

### Physical Effects by Stage

| Stage | Hunger drain | Energy drain | Movement | Other |
|-------|-------------|-------------|----------|-------|
| Early (0–33%) | Normal | Normal | Normal | Mating need frozen at 1.0 |
| Mid (33–66%) | +25% | Normal | Normal | Mood +0.1 ("expectant") |
| Late (66–100%) | +50% | +25% | -25% speed | Nesting drive: seeks safe position near colony center |

### Litter Size

Determined at conception: base 1, +1 if queen health > 0.8, +1 if queen hunger > 0.7. Range: 1–3.

### No Miscarriage

Kittens are always born. Poor queen nutrition during pregnancy produces frailer kittens (lower starting health) rather than pregnancy loss — the consequences surface post-birth through kitten vulnerability.

### Birth

When `current_tick - conceived_tick >= ticks_per_season`:
1. Remove `Pregnant` component
2. Spawn kitten entities at queen's position (see Section 5)
3. Emit `KittenBorn` events

### Kitten Mood Aura

Kittens provide a persistent mood bonus to nearby adults (within 5 Manhattan distance) that scales inversely with maturity:

```
bonus = 0.15 * (1.0 - maturity)
```

At birth (maturity 0.0) this is the full +0.15; by independence (maturity 1.0) it's gone. Applied as a renewable mood modifier refreshed each tick a cat is near a kitten — not a one-time event. Multiple kittens stack diminishingly: `total = bonus_1 + bonus_2 * 0.5 + bonus_3 * 0.25`.

This means kittens are a continuous source of colony happiness. Adults gravitate toward them not just for caretaking but because proximity feels good. A colony that loses its kittens feels the absence.

### Files Modified

- `src/components/physical.rs` (or new file `src/components/pregnancy.rs`) — `Pregnant`, `GestationStage`
- New system `src/systems/pregnancy.rs` — tick gestation, apply physical effects, trigger birth
- `src/systems/needs.rs` — apply hunger/energy drain modifiers when `Pregnant` is present

## 5. Kitten Spawning & Growth

### Kitten Entity

Spawned at birth with:
- `Age::new(current_tick)`
- `KittenDependency { mother, father, maturity: 0.0 }`
- Personality from genetic inheritance (Section 7)
- Appearance inherited with variation (Section 7)
- Health: `0.7 + queen_avg_nutrition * 0.3` (well-fed queen → 1.0; starving queen → 0.7)
- Needs: hunger 0.5 (immediate feeding pressure), energy 0.8, all others default
- All standard cat components (Species, Name, Gender, Orientation, etc.)

### New Component: `KittenDependency`

```rust
#[derive(Component)]
pub struct KittenDependency {
    pub mother: Entity,
    pub father: Entity,
    pub maturity: f32,
}
```

### Maturity Progression

Advances linearly from 0.0 → 1.0 over the Kitten life stage (4 seasons):
```
rate = 1.0 / (4.0 * ticks_per_season as f32)
```

### Capability Milestones

| Maturity | Label | Capabilities |
|----------|-------|-------------|
| 0.00–0.24 | Newborn | Fed by adults only. No autonomous actions. Cries when hungry (social ping for caretakers). |
| 0.25 | Weaned | Can eat from stores at 50% efficiency. |
| 0.50 | Mobile | Full movement speed. Can Socialize, Wander (play). Still no hunt/forage/build. |
| 0.75 | Apprentice | Can forage at 75% efficiency. Can be mentored. |
| 1.00 | Independent | Full capabilities. `KittenDependency` removed. |

### Kitten AI

`score_actions()` gates actions behind maturity thresholds. Before 0.25, the kitten's behavior is:
- Cry when hunger < 0.3 (emits a signal that nearby adults detect)
- Sleep when energy < 0.3
- Follow nearest bonded adult (mother/father, or any adult with fondness > 0.2)

The action set expands at each milestone. This is implemented as early-returns or zero-scores in the scoring function based on the `KittenDependency.maturity` value.

### Orphan Handling

If a parent dies and is despawned, the `KittenDependency` entity reference becomes stale. The growth system must handle this gracefully: check `world.get_entity(mother)` / `world.get_entity(father)` before accessing parent data. Orphan kittens continue maturing normally — they just lose the parent bond bonus in caretaking scoring and must rely on colony compassion for feeding.

### Files Modified

- New component in `src/components/` — `KittenDependency`
- New system `src/systems/growth.rs` — tick maturity, remove `KittenDependency` at 1.0
- `src/ai/scoring.rs` — gate actions behind maturity thresholds
- `src/plugins/setup.rs` — kitten spawning logic (called from pregnancy birth system)

## 6. Caretaking Behavior

### New Action: `Caretake`

Added to `Action` enum.

### Scoring

```
base = hungry_kitten_nearby * compassion * 1.8 * level_suppression(3)
```

- `hungry_kitten_nearby`: `1.0 - min_kitten_hunger` where `min_kitten_hunger` is the lowest hunger value among kittens within 8 Manhattan distance that have hunger < 0.4. Zero if no such kittens exist.
- Parents get +0.5 bonus for their own kittens.
- Any bonded adult can caretake — not limited to parents.

### New DispositionKind: `Caretaking`

Target completions: personality-scaled. `1 + (compassion * 2.0).round()` — compassionate cats do more rounds of feeding.

### Caretaking TaskChain

1. **`MoveTo`** — approach hungriest nearby kitten
2. **`FeedKitten`** — new `StepKind`. Duration: 10 ticks. Kitten hunger +0.3. Consumes food from stores (same as Eat cost).
3. **`GroomOther`** — groom the kitten (fondness boost, kitten warmth need)

### Emergent Dynamics

- Colony naturally rallies to feed kittens through compassion scoring
- Orphan kittens (parents dead) rely on colony compassion — low-compassion colonies let orphans starve
- A hungry colony can't afford to feed kittens (L3 suppression) — population self-regulates
- Caretaking competes with hunting/foraging for adult labor — too many kittens starves everyone

### Files Modified

- `src/ai/mod.rs` — add `Caretake` variant
- `src/ai/scoring.rs` — add scoring logic
- `src/components/disposition.rs` — add `Caretaking` to `DispositionKind`
- `src/components/task_chain.rs` — add `FeedKitten` to `StepKind`
- `src/systems/disposition.rs` — add `build_caretaking_chain()`, handle in resolution systems

## 7. Genetic Inheritance

### Personality

For each of the 18 personality axes:
```rust
let parent_avg = (mother_value + father_value) / 2.0;
let mutation = rng.gen_range(-0.1..=0.1);
let child_value = (parent_avg + mutation).clamp(0.0, 1.0);
```

Creates recognizable family traits with drift. Bold-hunter lineages produce mostly bold kittens, with occasional outliers.

### Appearance

- `fur_color`: 50/50 from either parent, 10% chance of random mutation
- `pattern`: same logic
- `eye_color`: same logic
- `distinguishing_marks`: union of both parents' marks, each 50% pass-through, 5% chance of new random mark

### Gender & Orientation

- Gender: random (equal Tom/Queen probability, 5% Nonbinary)
- Orientation: random (same distribution as founding cats — not inherited)

### Files Modified

- New function in `src/plugins/setup.rs` (or utility module) — `generate_kitten()` taking parents as input

## 8. Population Control

Three interlocking mechanisms prevent runaway growth:

### Seasonal Fertility

Mating need only decays during Spring and Summer. Autumn and Winter are infertile — the need stays at 1.0.

### Economic Pressure

Mating is L3 — requires `level_suppression(3) > 0`, meaning physiological (L1) and safety (L2) needs must be met. A hungry or unsafe colony won't mate.

### Gestation Cooldown

1-season gestation means max 1 litter/year per queen. Conceived in Spring → born in Summer. Conceived in Summer → born in Autumn (kittens face winter — risky).

### Combined Effect

With 1–3 kittens per litter, max 1 litter/year per queen, only during prosperity:
- Colony of 8 (4 queens): 4–12 kittens/year theoretical max
- Realized growth lower due to kitten mortality from hunger pressure
- Carrying capacity feedback: more cats → prey depletion → hunger → L3 suppressed → no mating

## 9. Events & Colony Score

### New EventKinds

- `MatingOccurred { cat_a: String, cat_b: String }`
- `PregnancyBegan { queen: String, partner: String, litter_size: u8 }`
- `KittenBorn { mother: String, father: String, kitten: String, litter_id: u64 }`
- `KittenMatured { kitten: String, new_stage: String }`

### ColonyScore

- `kittens_born` — already exists, increment on birth
- Add `kittens_surviving: u64` — living cats that were born in-sim (not founding members)

### Narrative Integration

- Birth announcements: tier 2 significant events
- Nearby cats observe birth: mood boost, memory entry of type `Birth`
- First-born kitten of the colony: tier 3 landmark event
- Parentage tracked on `KittenDependency` for future lineage/family tree features

## 10. Files Summary

### New Files

| File | Purpose |
|------|---------|
| `src/components/grooming.rs` | `GroomingCondition` component |
| `src/components/pregnancy.rs` | `Pregnant`, `GestationStage` components |
| `src/components/kitten.rs` | `KittenDependency` component |
| `src/systems/pregnancy.rs` | Gestation ticking, physical effects, birth trigger |
| `src/systems/growth.rs` | Kitten maturity progression, capability unlocks |

### Modified Files

| File | Changes |
|------|---------|
| `src/components/physical.rs` | Add `mating` field to `Needs` |
| `src/systems/social.rs` | Modulate fondness/romantic deltas by grooming condition |
| `src/components/mod.rs` | Re-export new component modules |
| `src/components/disposition.rs` | Add `Mating`, `Caretaking` to `DispositionKind` |
| `src/components/task_chain.rs` | Add `MateWith`, `FeedKitten` to `StepKind` |
| `src/ai/mod.rs` | Add `Mate`, `Caretake` to `Action` |
| `src/ai/scoring.rs` | Scoring for Mate and Caretake, maturity gating |
| `src/systems/needs.rs` | Mating need decay with gating, pregnancy hunger/energy modifiers |
| `src/systems/disposition.rs` | `build_mating_chain()`, `build_caretaking_chain()`, resolution logic |
| `src/systems/task_chains.rs` | Handle `MateWith`, `FeedKitten` step kinds |
| `src/systems/mod.rs` | Register new systems |
| `src/systems/death.rs` | Handle kitten death (parent grief, KittenDependency cleanup) |
| `src/systems/social.rs` | Parent-kitten relationship initialization |
| `src/plugins/setup.rs` | `generate_kitten()` function for birth spawning |
| `src/plugins/simulation.rs` | Register pregnancy and growth systems in schedule |
| `src/resources/event_log.rs` | New event kinds |
| `src/resources/colony_score.rs` | Add `kittens_surviving` |
| `src/main.rs` | Mirror new systems in headless schedule |

## 11. Verification

### Unit Tests

- Grooming condition decay rate and restoration from groom actions
- Grooming effect on fondness/romantic delta multipliers
- Pride penalty from low grooming scales with personality pride
- Mating need decay gating (season, life stage, orientation, pregnancy)
- Litter size calculation from queen stats
- Genetic inheritance: personality blend + mutation stays in [0.0, 1.0]
- Maturity progression rate matches expected timeline
- Capability gating at each maturity threshold

### Integration Tests

- Full mating arc: two partnered adults in spring → mating disposition → pregnancy → birth → kitten entities exist
- Caretaking: hungry kitten → nearby adult adopts Caretaking disposition → feeds kitten
- Population control: headless run with seed 42 — verify population doesn't explode beyond prey carrying capacity
- Kitten growth: verify kitten reaches independence after 4 seasons
- Economy interaction: starving colony → no mating activity

### Headless Simulation

Before/after comparison per CLAUDE.md verification protocol:
```
cargo run -- --headless --duration 60 --seed 42
```
- ColonyScore welfare axes should not regress
- `kittens_born` > 0 in spring/summer runs
- `deaths_starvation` should not spike (kittens shouldn't crash the food economy)
- Multi-seed validation: seeds 42, 99, 7, 2025, 314
