# Shadow Fox Ward Crisis

## Problem

In 15-minute headless sims, all 8 cats die from `ShadowFoxAmbush` injuries around sim day 106-108. Food stores remain at 47-50/50 the entire time — this is not a starvation problem. Injury source tagging (added 2026-04-14) confirms 100% of deaths are shadow fox ambushes.

## Root Cause

No wards exist on the map. Wards are never pre-spawned — they only appear when a cat with herbcraft skill places one via the Crafting disposition's SetWard sub-mode. The GOAP migration (2026-04-14) replaced the old TaskChain executor with a new GOAP executor, but the crafting/magic action dispatch uses a 20-tick duration fallback instead of delegating to the real step resolvers. The fallback completes the GOAP step without actually calling `resolve_set_ward`, so no Ward entity is ever spawned.

Even before GOAP, ward placement depended on a cat:
1. Having `herbcraft_skill > 0` and `has_ward_herbs = true` and `ward_strength_low = true`
2. Winning the Crafting disposition via softmax
3. The CraftingHint selecting SetWard over GatherHerbs/PrepareRemedy
4. The chain builder finding a herb with Thornbriar in inventory
5. The magic resolver (`resolve_set_ward`) running to completion

This is a fragile chain. If any link breaks — no herbs spawned nearby, no cat with herbcraft skill, scoring doesn't favor Crafting — wards never appear and shadow foxes roam freely.

## Shadow Fox Mechanics

`src/systems/wildlife.rs:617-771` — `predator_stalk_cats()`:

- Only affects `WildSpecies::ShadowFox` entities
- Shadow foxes respect wards: if inside a ward's repel radius (strength * `shadow_fox_ward_repel_multiplier`), they flee away. If stalking a cat inside a ward radius, they cancel the stalk.
- Without wards: shadow foxes detect cats within `base_detection_range`, begin stalking (5% chance/tick), close to 1 tile, then ambush — dealing `threat_power` raw damage + `apply_injury` (tagged `ShadowFoxAmbush`)
- Shadow foxes spawn via `ShadowFoxSpawn` system when corruption is present — 2 spawned in the test runs
- The ambush system is independent of cat behavior — it doesn't matter if a cat is resting, hunting, or fighting

## Ward Mechanics

`src/components/magic.rs:334-365`:
- `WardKind::Thornward` — herb-based, strength 1.0, decay 0.005/tick (~200 tick lifespan)
- `WardKind::DurableWard` — magic-based, strength 1.0, decay 0.001/tick (~1000 tick lifespan), can misfire

`src/systems/magic.rs:88-112` — `ward_decay()`:
- Every tick, ward.strength -= decay_rate (half speed on WardPost terrain tiles)
- Despawned when strength <= 0

`src/steps/magic/set_ward.rs` — `resolve_set_ward()`:
- Thornward: consumes Thornbriar herb from inventory, spawns Ward entity at cat position
- DurableWard: misfire check, spawns Ward entity (or inverted ward on misfire)
- Both grant herbcraft/magic skill growth

## Crafting Pipeline (What Needs to Work)

The full chain from "cat decides to ward" to "Ward entity exists on map":

### Scoring (`src/ai/scoring.rs:363-381`)
```
Herbcraft score = spirituality * (skill_offset + herbcraft_skill) * ward_scale * level_suppression(4)
```
Preconditions: `has_ward_herbs && ward_strength_low`
- `has_ward_herbs`: inventory contains a ward-eligible herb (Thornbriar)
- `ward_strength_low`: average ward strength across all wards < threshold, OR no wards exist (always true when no wards)

### GOAP Planner (`src/ai/planner/actions.rs`)
CraftingHint::SetWard produces:
```
SetWard: preconditions=[CarryingIs(Herbs)], effects=[CarryingIs(Nothing), IncrementTrips]
```
CraftingHint::GatherHerbs produces:
```
GatherHerb: preconditions=[ZoneIs(HerbPatch), CarryingIs(Nothing)], effects=[CarryingIs(Herbs), IncrementTrips]
```

A typical ward plan: `TravelTo(HerbPatch) → GatherHerb → SetWard`

### GOAP Executor (`src/systems/goap.rs`)
**Currently broken.** The `GatherHerb` and `SetWard` action kinds hit the fallback branch:
```rust
GoapActionKind::GatherHerb | ... | GoapActionKind::SetWard | ... => {
    if ticks >= 20 { StepResult::Advance } else { StepResult::Continue }
}
```
This completes after 20 ticks without calling `resolve_gather_herb` or `resolve_set_ward`.

### Step Resolvers (working, just not called)
- `src/steps/magic/gather_herb.rs` — `resolve_gather_herb()`: finds herb entity, adds to inventory, despawns herb
- `src/steps/magic/set_ward.rs` — `resolve_set_ward()`: consumes herb, spawns Ward entity with Position
- `src/steps/magic/prepare_remedy.rs` — `resolve_prepare_remedy()`: consumes herb, advances
- `src/steps/magic/apply_remedy.rs` — `resolve_apply_remedy()`: paths to patient, applies remedy effect

## What Needs to Happen

### 1. Wire crafting step resolvers into GOAP executor

Replace the 20-tick fallback for `GatherHerb`, `PrepareRemedy`, `ApplyRemedy`, `SetWard`, `Scry`, `SpiritCommunion` with actual calls to the existing step resolver functions in `src/steps/magic/`.

Each resolver takes explicit parameters (ticks, inventory, skills, etc.) and returns `StepResult`. The GOAP executor already has access to all needed data via its system params. The pattern matches the existing hunt/forage/socialize dispatch — just needs the same treatment for magic actions.

Key considerations:
- `resolve_gather_herb` needs a `Query<(Entity, &Herb, &Position), With<Harvestable>>` — currently removed from executor params due to query conflicts. Will need a snapshot approach (pre-collect herb entities) or a ParamSet.
- `resolve_set_ward` needs `&mut Commands` to spawn the Ward entity — already available.
- `resolve_set_ward` also needs `MagicAffinity`, `Corruption`, `Mood`, `Health` components — `Mood` and `Health` are in the cats query, but `MagicAffinity` and `Corruption` are NOT currently in the cats query tuple. They'll need to be added or queried separately.

### 2. Wire building step resolvers too

Same pattern for `Construct`, `TendCrops`, `HarvestCrops`, `GatherMaterials`, `DeliverMaterials`. These also use the 20-tick fallback. The building resolvers in `src/steps/building/` are working — just need dispatch.

The Construct resolver (`src/steps/building/resolve_construct`) needs a mutable `Query<(Entity, &mut Structure, ...)>` which conflicts with other queries. May need the same pre-collect snapshot approach used for building_query elsewhere.

### 3. Ensure herb spawning works

Herbs spawn seasonally via `herb_growth` in the simulation chain. Verify that herbs are actually spawning and that `Harvestable` components are being added. The scoring check `has_ward_herbs` requires Thornbriar specifically — confirm Thornbriar spawns in the herb pool.

### 4. Consider emergency ward seeding

Even with fully working crafting, the ward pipeline is fragile (depends on herbs spawning, cat skill, scoring selection). Consider:
- Spawning 1-2 initial thornwards near the colony stores at world generation
- Lowering the Crafting/SetWard scoring threshold so it triggers earlier
- Making the coordinator system prioritize ward placement when `ward_strength_low` is true

## Files

| File | Role |
|------|------|
| `src/systems/goap.rs` | GOAP executor — fallback branch needs real resolver calls |
| `src/steps/magic/set_ward.rs` | Ward placement resolver (working) |
| `src/steps/magic/gather_herb.rs` | Herb gathering resolver (working) |
| `src/steps/magic/prepare_remedy.rs` | Remedy prep resolver (working) |
| `src/steps/magic/apply_remedy.rs` | Remedy application resolver (working) |
| `src/steps/magic/scry.rs` | Scrying resolver (working) |
| `src/steps/magic/spirit_communion.rs` | Spirit communion resolver (working) |
| `src/steps/building/construct.rs` | Building resolver (working) |
| `src/steps/building/tend.rs` | Farming resolver (working) |
| `src/steps/building/harvest.rs` | Harvest resolver (working) |
| `src/ai/planner/actions.rs` | Crafting action definitions for planner |
| `src/ai/scoring.rs:363-381` | Herbcraft/ward scoring |
| `src/systems/wildlife.rs:617-771` | Shadow fox stalk/ambush with ward avoidance |
| `src/systems/magic.rs:88-112` | Ward decay system |
| `src/components/magic.rs:334-365` | Ward/WardKind types |

## Diagnostic Data

From 15-minute headless sim (seed 42):
```
tick  106261  Ash           ShadowFoxAmbush
tick  106604  Willow        ShadowFoxAmbush
tick  106617  Basil         ShadowFoxAmbush
tick  106663  Briar         ShadowFoxAmbush
tick  106881  Hazel         ShadowFoxAmbush
tick  107264  Calcifer      ShadowFoxAmbush
tick  107366  Mocha         ShadowFoxAmbush
tick  107896  Simba         ShadowFoxAmbush
```
Food stores: 47-50/50 entire run. Ward count: 0 entire run. Shadow fox count: 2 (spawned via ShadowFoxSpawn). AnxietyInterrupt count escalated from 245 to 4000+ as injured cats oscillated between Resting plans and health-based interrupts.
