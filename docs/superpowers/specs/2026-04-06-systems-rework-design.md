# Clowder Systems Rework — Design Spec

## Vision

Clowder is a **digital aquarium** — a deep, autonomous civilization simulation. The observer watches a cat colony unfold, discovering complexity rather than managing it. There is no player making strategic decisions. Cats figure things out themselves through utility AI, personality-driven behavior, and emergent social dynamics.

**The Dwarf Fortress comparison is apt for simulation depth, not for player challenge.** Every system should be internally complex enough that the observer notices new things the longer they watch. The joy is discovery: "wait, when did they start making fish hooks?" "that kitten learned to cook from the elder who learned it from the founder."

**Creative touchstones:**
- Redwall — communal feasts, anthropomorphized but still animal, goodness attracting envious outsiders
- Mausritter — small creatures making tools and building communities in a big world
- Dwarf Fortress — emergent complexity from interacting systems, layered depth
- Warriors — clan culture, generational knowledge, territorial dynamics

**Key design principles:**
1. **The economy is real.** Every meal was once alive. Hunting kills a real prey entity. Food stores are actual items in a building, not a float. Overhunting depletes prey populations.
2. **Behavior is emergent.** Quarantine happens because sick cats seek isolation and healthy cats avoid them. Work crews form because activity cascading makes group work attractive. No one assigns tasks — the utility AI, personality, and social dynamics produce coordination.
3. **Knowledge is cultural.** The first generation knows nothing. Recipes, techniques, territorial knowledge, and danger memories are discovered, taught, and inherited across generations. Three generations in, the colony has accumulated wisdom no single cat invented.
4. **The ecosystem is alive.** Rats, hawks, foxes, and fish have their own needs-based AI. Prey populations respond to predation. The food web self-regulates. Remove cats and rats take over. Remove rats and hawks range elsewhere.
5. **Chain reactions are the point.** When independent systems interact in unexpected ways, that's not a bug — it's the joy of the simulation. Design for emergent chain reactions.

---

## Foundation Systems

These must be built or reworked first. Everything else depends on them.

### Items & Material Chain

**Replaces:** Current `docs/systems/items.md` (lightweight trophy pouch)
**Scope:** The physical backbone of the entire economy.

Items are the universal representation of physical objects in the simulation. Every tangible thing — prey carcasses, herbs, tools, food, trophies, Named Objects — is an `Item` entity with a kind, quality, and condition.

**Item categories:**

| Category | Examples | Source | Decay Rate | Notes |
|----------|---------|--------|------------|-------|
| Raw Prey | Rat carcass, fresh fish, mouse, bird | Hunting, fishing | Fast (0.01/tick) | Must be eaten or preserved quickly |
| Foraged | Berries, nuts, roots, wild onion, mushrooms, moss, dried grass, feathers | Foraging | Medium (0.005/tick) | Seasonal availability |
| Herbs | HealingMoss, Moonpetal, Calmroot, Dreamroot, Thornbriar | Existing herb system | Existing rates | Dual-use: medicine AND cooking ingredient |
| Substances | Catnip, Valerian, Corrupted variants | Specific terrain | Medium (0.005/tick) | See substances system |
| Curiosities | Shiny pebble, glass shard, colorful shell | Random find while exploring/foraging | None (inorganic) | Hoarding targets; trade value |
| Prepared Food | Dried meat, smoked fish, herb-cured game, sushi, feast dish | Cooking | Slow (0.002/tick) | Higher hunger satisfaction + mood bonus |
| Tools | Sharpened stone, braided vine pouch, fish hook, bark scoop, woven screen | Crafting | Very slow (0.001/tick) | Action efficiency bonuses |
| Comfort Items | Lined nest, plush bedding, mounted trophy, flower crown | Crafting | Slow (0.002/tick) | Building/personal comfort bonuses |
| Named Objects | Spirit Totem, Named Ward, Woven Talisman | The Calling | None (legendary) | Colony-wide mechanical effects |

**Item properties:**

```
Item {
    kind: ItemKind,          // Enum of all item types
    quality: f32,            // 0.0–1.0; derived from harvester/crafter skill
    condition: f32,          // 1.0 → 0.0; decays over time; 0.0 = destroyed
    decay_rate: f32,         // Per-tick condition loss
    name: Option<String>,    // Only Named Objects have names
    creator: Option<Entity>, // Who made/found this
}
```

**Quality tiers:**

| Tier | Range | Source |
|------|-------|--------|
| Poor | 0.0–0.2 | Unskilled finder; failed craft |
| Common | 0.2–0.5 | Average skill |
| Fine | 0.5–0.8 | Skilled finder/crafter |
| Exceptional | 0.8–1.0 | Master skill; rare |

**Item spatial model — three locations an item can be:**

| Location | Representation | Example |
|----------|---------------|---------|
| In a cat's inventory | `Carried(Entity)` — item has no `Position`; moves with the cat | Cat carrying a fish home |
| On the ground | `Position` component on the item entity | Dropped fish, displayed trophy, herbs growing |
| In a building | `StoredIn(Entity)` — item is inside a building entity | Food in Stores, bedding in Den, trophy on wall |

Items on the ground are visible on the tile at detail zoom. Items in buildings are visible when inspecting the building. Items in inventory are visible when inspecting the cat. A cat picking up or dropping an item transitions between these states.

**Inventory:**
- Each cat has a 5-slot inventory (expanding existing herb pouch)
- Tools: braided vine pouch adds +2 slots (7 total)
- Buildings have item storage capacity (Stores building holds colony food/materials)

**Rendering note:** The current map view is too zoomed out for items and detailed behavior to be observable. The TUI needs two zoom levels:
- **Overview:** Full map, 1 char per tile. Territory, clusters, predator movements. Strategy-map feel.
- **Village view:** ~20x20 tile viewport, scrollable. Individual cats with labels, items on ground, building contents, cooking at hearth, prey animals. This is the aquarium view.
- **Follow-cam:** Lock viewport to a specific cat. Watch their full day unfold.

TUI zoom redesign is a separate design session from this systems spec, but item spatial existence is a prerequisite — items must have map presence before they can be rendered.

**Hoarding behavior** (personality-driven):

| Trait | Hoarding Behavior |
|-------|-------------------|
| High Pride | Collects trophies; displays near sleeping spot |
| High Ambition | Seeks rare/high-quality items; competes for the best |
| High Curiosity | Collects curiosities; explores more to find them |
| High Independence | Defends personal hoard; mood penalty if items taken |
| Low Pride/Ambition | Doesn't hoard; shares freely |

**Migration from current system:**

| Current | New |
|---------|-----|
| `FoodStores` resource (float) | Item entities stored in Stores building |
| `Inventory` in magic.rs (`Vec<HerbKind>`) | `Vec<Item>` with generic item kinds |
| Abstract food from hunting | Hunt kills prey entity → produces raw prey item |
| Abstract food from foraging | Forage produces foraged item entities |

**Key files to modify:** `src/components/magic.rs` (refactor Inventory), `src/resources/food.rs` (replace FoodStores), `src/systems/actions.rs` (hunt/forage produce items), `src/ai/scoring.rs` (hoarding behavior).

---

### Ecosystem & Wildlife Rework

**Replaces:** Current wildlife system (spawn-and-attack) + `docs/systems/raids.md` (escalating difficulty)
**Scope:** Real prey populations, needs-based wildlife AI, food web.

Every creature in the world has its own needs-based utility AI. Prey species are real entities that breed, are hunted, and can be depleted. Predator species have territorial behavior and respond to prey density. There are no "raids" — there's an ecosystem, and sometimes competing species pressure the colony.

**Species:**

#### Prey Species

| Species | Habitat | Breed Rate | Food Source | Population Cap | Notes |
|---------|---------|-----------|-------------|---------------|-------|
| Mouse | Grass, Light Forest | 0.003/tick when fed | Seeds (grass tiles) | 30 per map | Primary hunting target; abundant |
| Rat | Near structures, Dark | 0.005/tick when fed | Any food source incl. cat stores | 50 per map | Breeds fast; attracted to stored food |
| Fish | Water tiles | 0.002/tick | Implicit (water ecosystem) | 20 per water cluster | Caught at water edges; seasonal peak in spring/summer |
| Bird | Trees, Open sky | 0.001/tick | Implicit (insects) | 15 per map | Feathers as crafting material; hunted by hawks too |

#### Predator Species

| Species | Needs | Actions | Population | Behavior |
|---------|-------|---------|-----------|----------|
| Fox | Hunger, Safety, Territory | Patrol, Hunt (lone/weak targets), Probe (assess group size), Ambush, Flee | 1–2 per map, territorial | Winter hunger makes them bolder (safety weight decreases). Avoids groups of 3+ cats. |
| Hawk | Hunger, Nesting (seasonal) | Hunt (smallest exposed creature), Roost, Nest (spring), Dive (ignores walls) | 1–3 per map, based on terrain | Targets rats > kittens > small cats. Aerial — ignores walls/gates. |
| Shadow Fox | Corruption-seeking | Same as fox + corruption spread | Spawned by corruption threshold | Supernatural exception; drawn to corrupted tiles; ward posts repel |
| Snake | Hunger, Shelter | Ambush (waits in terrain), Strike, Flee | 1–3, warm season only | Lurks in grass/rocks; surprise attacks on paws/whiskers |

**Needs-based AI for wildlife:**

Each non-cat species gets a simplified utility AI:

```
wildlife_evaluate(animal):
    scores = {}
    for action in species.available_actions:
        scores[action] = base_weight(action)
            * need_urgency(animal, action.satisfies)
            * safety_check(animal, action)
            * environmental_modifier(animal, action)
    return highest_scoring(scores)
```

- **Hunger need:** Drives hunting/foraging. As hunger increases, animals take more risks.
- **Safety need:** Drives fleeing, avoiding threats. Decreases weight when hunger is critical (desperate animals are bold).
- **Territory need:** Predators patrol their territory. Overlapping territories cause conflict.
- **Nesting need:** Seasonal for hawks; rats nest near food. Breeding triggers when food > threshold and nest exists.

**Prey population dynamics:**

The population cap is a carrying capacity, not a hard ceiling. As population approaches the cap, competition for food intensifies — breeding slows, starvation increases, and animals range farther (becoming more visible to predators). Hitting the cap is an ecological event, not a silent gate.

```
breed_check(species, population, food_availability):
    density_pressure = 1.0 - (population / species.cap)  // 1.0 at empty, 0.0 at cap
    if density_pressure <= 0.0:
        log(Micro, "The {species} have overrun their territory — they fight over scraps.")
        // No breeding; existing animals starve faster (food_availability effectively halved)
        return
    if density_pressure < 0.2:
        log(Micro, "The {species} are growing restless — too many mouths, not enough food.")
    breed_chance = species.breed_rate * food_availability * density_pressure
    if random() < breed_chance: spawn_offspring(species)

death_check(prey):
    // Natural death from starvation
    if prey.hunger > 0.9: prey.health -= 0.01/tick
    // Overcrowding stress: animals near cap forage less efficiently
    if population / species.cap > 0.8:
        prey.hunger += 0.001/tick  // Extra hunger from competition
    // Predation handled by hunt actions from cats, hawks, foxes
```

**Food web interactions:**
- Cats hunt mice, rats, fish, birds, snakes
- Hawks hunt mice, rats, fish, birds, kittens (if exposed)
- Foxes hunt mice, rats, birds, lone cats
- Rats eat stored food, foraged materials, compete with cats for foraging
- Snakes eat mice

**Testable ecosystem properties:**
- Rats-only: exponential growth until food runs out, then die-off
- Rats + hawks: hawks regulate rat population, stable oscillation
- Rats + cats: cats hunt rats, food supply = rat population, both stabilize
- Full ecosystem: complex multi-species equilibrium with seasonal shifts
- Remove cats: rats boom, hawks feast, foxes move in

**Ecological incursions (replacing raids):**

There is no raid system. What looks like a raid is an emergent property of the ecosystem:
- **"Rat swarm"**: Rat population near stores grew unchecked. Rats are hungry and bold.
- **"Predator pack"**: Winter hunger drives foxes closer. Multiple foxes converge on same prey-rich area (the colony).
- **"Hawk flock in spring"**: Hawks nesting season. Multiple hawks active, hunting small prey. Kittens exposed in open terrain are targets.
- **"Corruption incursion"**: Corruption threshold spawns shadow foxes. Supernatural, not ecological.

Triggers are seasonal and situational:
- Winter: prey scarce → predators range farther → more fox encounters
- Spring: hawks nesting → more aerial hunts; kittens born → vulnerable targets
- Summer: abundance → lower pressure from predators; rat breeding peaks
- Autumn: animals stockpiling → competition for foraged resources

Defensive response is emergent: coordinator issues Fight directives when threats detected. Cats with high boldness respond first. Walls and gates shape movement.

**Key files to modify:** `src/components/wildlife.rs` (needs-based AI, prey components), `src/systems/wildlife.rs` (population dynamics, species AI), `src/systems/actions.rs` (hunting targets real entities), `src/ai/scoring.rs` (threat assessment from ecosystem).

---

### Crafting & Cooking

**New system.** No existing doc.
**Scope:** Recipe-based item transformation with cultural knowledge transmission.

Crafting transforms raw items into refined items. Cooking is the most visible crafting category — the Redwall food fantasy realized through anthropomorphized cats.

**Crafting categories:**

#### Cooking (new skill)

| Tier | Examples | Ingredients | Hunger Mult | Mood Bonus | Skill Req |
|------|---------|------------|-------------|-----------|-----------|
| Raw | Fresh fish, raw mouse | Single prey item | 1.0× | None | None |
| Simple | Dried meat, smoked fish | Prey + hearth proximity | 1.5× | None | > 0.2 |
| Prepared | Herb-crusted fish, berry-glazed game | Prey + herb/foraged | 2.0× | +0.05 | > 0.4 |
| Elaborate | Fish sushi, roasted game with herbs and berries | 3+ ingredients | 2.5× | +0.1, social bonus if shared | > 0.6 |
| Feast | The Great Catch, Midsummer Spread | 5+ ingredients, feeds colony | 3.0× per cat | +0.15 colony-wide, memory created | > 0.8 |

**Feast events:** High-skill cook with enough ingredients prepares a feast at the hearth. The feast is a single large prepared food item placed at the hearth. All cats within 5 tiles receive the hunger and mood benefits simultaneously — they gather and eat together. Satisfies social needs, creates shared positive memories, boosts fondness between all participants. A named feast ("Bramble's Midsummer Catch") enters colony lore — passed down through memory inheritance.

**Seasonal cooking:**
- Spring: fresh herbs, young greens, light fish
- Summer: berry-heavy, elaborate feasts (abundance)
- Autumn: nut-heavy, preserving for winter
- Winter: hearty dishes from dried stores, warming meals at the hearth

#### Nest-Weaving & Comfort

| Recipe | Inputs | Output | Effect |
|--------|--------|--------|--------|
| Lined nest | Moss + dried grass | Comfort item | Den comfort +0.1 |
| Plush bedding | Feathers + soft leaves | Comfort item | Den comfort +0.15, energy recovery bonus |
| Wind screen | Woven grass | Comfort item | Outdoor comfort bonus in radius |

#### Tool-Making

| Recipe | Inputs | Output | Effect |
|--------|--------|--------|--------|
| Digging tool | Sharpened stone | Tool | Build speed +20% |
| Carry pouch | Braided vine | Tool | Inventory +2 slots; loses 0.05 condition per use (item stored/retrieved) |
| Fish hook | Thorns + vine | Tool | Fishing success +30% |
| Water scoop | Curved bark | Tool | Can carry water to sick/injured cats |

#### Trophy & Decoration

| Recipe | Inputs | Output | Effect |
|--------|--------|--------|--------|
| Mounted trophy | Cleaned bone/tooth | Comfort item | Building morale +0.03 |
| Curiosity display | Arranged curiosities | Comfort item | Morale bonus in radius |
| Flower crown | Woven flowers | Worn item | Social fondness gain +0.05 |

#### Herbcraft (existing, integrated into item system)

Existing recipes from magic system. Herbs are now items. Remedies and wards produced are items. The Calling produces legendary-tier items.

**Crafting mechanics:**

```
craft(crafter, recipe, inputs):
    if NOT crafter.knows_recipe(recipe): return Err(UnknownRecipe)
    if NOT has_required_inputs(crafter, recipe): return Err(MissingInputs)

    time = recipe.base_time / (1.0 + crafter.skill * 0.5)
    // Crafter works for `time` ticks at crafting station

    quality = crafter.skill * 0.6
            + avg(input.quality for input in inputs) * 0.3
            + personality_bonus(crafter, recipe.category) * 0.1

    consume(inputs)
    produce(Item { kind: recipe.output, quality, creator: crafter })
```

**Recipe learning — cultural evolution:**

Cats don't start knowing all recipes. Knowledge is acquired and transmitted:

| Learning Method | Mechanism | Notes |
|-----------------|-----------|-------|
| Observation | Kitten watches adult craft within 3 tiles | Learns recipe as secondhand knowledge |
| Mentoring | Adult explicitly teaches during Mentor action | Faster; requires both cats to be idle |
| Experimentation | Cat with Curiosity > 0.6 tries combining items | Success = new recipe learned; Failure = inputs consumed, nothing produced |
| Memory inheritance | Elder tells stories during Socialize; kitten absorbs recipe memory | Personality-filtered: absorption chance = 0.3 × trait_affinity(recipe.category, kitten.personality). A bold kitten has high affinity for hunting recipes; a warm kitten for cooking. Unmatched recipes have base 0.1 chance. |
| Trade/visitors | Visitor from distant territory knows recipes the colony doesn't | Social interaction can transmit recipe knowledge |

**Recipe discovery as cultural evolution:** The first generation eats raw prey and sleeps on bare ground. A curious cat experiments with drying meat near the hearth. She teaches her kittens. One kitten discovers herb-cured fish. Three generations later, the colony has a cuisine tradition no single cat invented. The observer discovers this depth over time.

**Personality drives crafting:**

| Trait | Crafting Preference |
|-------|-------------------|
| High Warmth | Cooking for others; nest-weaving |
| High Pride | Trophy-making; elaborate food presentation |
| High Curiosity | Experimentation (recipe discovery) |
| High Diligence | Food preservation; consistent quality |
| High Spirituality | Herbcraft; ward-crafting |
| High Independence | Tool-making (self-reliance) |
| High Boldness | Strong flavors; unusual game ("will cook snake") |

**Cooking smells:** Active cooking at a hearth creates a temporary positive comfort modifier (+0.1) in a 3-tile radius. The colony smelling dinner being prepared is ambient aquarium life.

**Key files:** New `src/components/crafting.rs`, new `src/systems/crafting.rs`, `src/ai/scoring.rs` (crafting action scoring), `src/ai/mod.rs` (Cook/Craft action variants).

---

## Environmental & Physical Systems

### Body Zones

**Status:** Existing doc is thorough and fits the aquarium vision.
**Changes:** None. The 13-part anatomy, combat targeting, permanent identity traits, and narrative integration are exactly right.
**Doc:** `docs/systems/body-zones.md` — implement as written.

Key design points:
- 13 named body parts with functional consequences
- 5 condition states: Healthy → Bruised → Wounded → Mangled → Destroyed
- Permanent injuries become identity traits ("one-eyed Bramble")
- Pain system with weighted parts drives incapacitation
- Attacker-specific targeting weights per species
- Migration: `Health.current` becomes derived from weighted pain; `damage_to_injury()` becomes `damage_to_body_part()`

**Key files:** `src/components/physical.rs`, `src/systems/combat.rs`, `src/systems/death.rs`, `src/ai/scoring.rs`.

---

### Environmental Quality

**Status:** Existing doc is mechanically solid. Minor reframe.
**Changes:**
- Remove "gives players a reason to invest" language — cats seek comfort autonomously
- Add comfort-seeking to utility AI: cats with high warmth weight comfort when choosing where to idle/socialize/sleep
- Add cooking smell modifier: active cooking at hearth → +0.1 comfort in 3-tile radius
- Cats self-organize into cozy clusters around high-comfort areas

**Doc:** `docs/systems/environmental-quality.md` — implement with above additions.

Key design points:
- Tile-level comfort from terrain, buildings, corpses, corruption
- Rolling 20-tick average feeds persistent mood modifier
- Personality scaling: warmth amplifies, independence dampens
- Building proximity and condition contribute to comfort
- Overcrowding penalty at 4+ cats within 2 tiles

**Key files:** New `src/systems/environment.rs`, `src/resources/map.rs`, `src/systems/buildings.rs`.

---

### Corpse Handling

**Status:** Existing doc fits aquarium vision well.
**Changes:** Remove "player incentive" language. Otherwise implement as written.

Key design points:
- Corpse decay lifecycle: Fresh → Decaying → Remains → Gone (200 ticks total)
- Grief radius (3 tiles) scales with fondness and spirituality
- Vigil behavior: close friends sit quietly beside the dead
- Burial at cairns resolves grief, creates positive memory
- Scavenger attraction: decaying corpses draw wildlife (ecosystem integration)
- Coordinator may issue burial directive if no cat volunteers

**Doc:** `docs/systems/corpse-handling.md` — implement as written.

**Key files:** `src/systems/death.rs`, `src/components/physical.rs`, `src/ai/scoring.rs`.

---

## Behavioral & Social Systems

### Recreation & Grooming

**Status:** Existing doc fits aquarium vision perfectly.
**Changes:** None. Core aquarium content — watching cats sunbathe, play-hunt, groom each other.

Key design points:
- Recreation need at Maslow level 3; variety bonus rewards behavioral diversity
- 6 leisure activities with personality affinity (play-hunting, sunbathing, climbing, bird-watching, self-grooming, exploring)
- Grooming state (0.0–1.0) decays from weather/combat; maintained by self/social grooming
- Well-groomed cats get fondness bonuses; matted cats get mood/health penalties
- Grooming state feeds into disease (infection risk when matted)

**Doc:** `docs/systems/recreation.md` — implement as written.

**Key files:** `src/ai/mod.rs`, `src/components/mental.rs`, `src/ai/scoring.rs`, `src/systems/needs.rs`.

---

### Substances

**Status:** Existing doc fits aquarium vision.
**Changes:** Substances are now items in the material chain (catnip, valerian are `ItemKind::Substance`). Otherwise unchanged.

Key design points:
- Catnip (euphoria + silly behavior) and valerian (calm + lethargy)
- Tolerance buildup, dependence, withdrawal
- Personality-gated susceptibility: playful + low-diligence = vulnerable; diligent = resistant
- Corrupted variants: stronger effects + corruption accumulation
- Withdrawal mood penalty compounds with other mood sources

**Doc:** `docs/systems/substances.md` — implement with item integration.

**Key files:** New `src/components/substances.rs`, `src/ai/scoring.rs`, `src/systems/mood.rs`.

---

### Emotional States (renamed from Mental Breaks)

**Status:** Mechanics are sound. Framing needs shift from crisis-management to behavioral observation.
**Changes:**
- Rename from "mental breaks" to "emotional states" or "behavioral episodes"
- Remove "colony must cope" framing — the colony copes naturally through emergent AI response
- Cascade mechanics stay exactly as designed (witness mood penalties, personality-gated behaviors, inspirations)
- These are observable behavioral patterns, not game crises

Key design points:
- Mood thresholds trigger behavioral episodes: minor (sulking, yowling, hiding) at valence < -0.7; major (hissing, food gorging, spraying, feral) at < -0.9
- Personality-gated episode selection (temper → hissing; anxiety → hiding; etc.)
- Witness contagion: nearby cats get mood penalty → potential cascade
- Positive inspirations at valence > 0.7 (inspired hunt, inspired craft, social butterfly)
- Duration: minor 20–40 ticks, major 40–80 ticks

**Doc:** `docs/systems/mental-breaks.md` — implement with rename and framing shift.

**Key files:** `src/components/mental.rs`, new `src/systems/emotional_states.rs`, `src/systems/mood.rs`, `src/ai/scoring.rs`.

---

### Activity Cascading

**Status:** Tiny, perfect for aquarium. No changes.

Key design points:
- +0.15 utility bonus per nearby cat (within 5 tiles) doing the same action
- Group hunt success: +0.1 per additional hunter
- Build crew speed: +0.3 per additional builder
- Social gathering: satisfaction scales with participants
- Emergent coordination without task assignment

**Doc:** `docs/systems/activity-cascading.md` — implement as written.

**Key files:** `src/ai/scoring.rs`, `src/systems/actions.rs`.

---

## Health & Medicine

### Disease

**Status:** Mechanics are sound. Reframe quarantine as emergent.
**Changes:**
- Quarantine is not directed — sick cats seek warm isolation (comfort/energy needs shift); healthy cats avoid sick cats (mild mood penalty from proximity). The effect is quarantine without anyone deciding to quarantine.
- Healer role through coordination stays — coordinator recognizes "someone with herbcraft should tend to the sick cat." This is emergent colony organization, not player assignment.
- Treatment now uses items (herbs from inventory, not abstract)

Key design points:
- 4 disease types: wound infection, winter cold (contagious), food poisoning, corruption sickness
- Infection from untreated wounds after 30 ticks (3% per tick chance)
- Contagion: 2-tile radius, 1% per tick for contagious illness
- Treatment: healer + specific herb items + facility bonus from Sick Den
- Treatment quality = healer.herbcraft * herb.potency * facility_bonus
- Grooming state from recreation feeds infection risk (matted cats more vulnerable)

**Doc:** `docs/systems/disease.md` — implement with emergent quarantine and item integration.

**Key files:** New `src/components/disease.rs`, new `src/systems/disease.rs`, `src/systems/combat.rs`.

---

## Advanced Systems

### The Calling

**Status:** Fits aquarium vision perfectly. Minor item integration.
**Changes:** Named Objects are now items in the material chain. A Spirit Totem is an item that gets placed. A Named Ward is an item that gets installed.

Key design points:
- Rare trigger: 0.05% per tick for cats with magic_affinity > 0.5, mood > 0.6, spirituality > 0.5
- 4-phase trance: Compulsion → Gathering → Creation → Resolution
- Success produces Named Objects with colony-wide effects (Spirit Totem, Named Ward, Named Remedy, Woven Talisman)
- Failure risks corruption spike or psychological "Shaken" state
- Named Objects have compound names ("Moonwhisper," "Thornheart") and creator attribution
- Legendary tier of the crafting chain

**Doc:** `docs/systems/the-calling.md` — implement with item integration.

**Key files:** New `src/systems/calling.rs`, `src/components/magic.rs`.

---

### Reproduction & Memory Inheritance

**Status:** Mechanics solid. Adding memory inheritance and reframing.
**Changes:**
- Add memory inheritance: elders share memories with kittens during Socialize
- Add recipe inheritance: crafting/cooking knowledge transmitted through same channel
- Personality-filtered absorption: bold kittens absorb fighting stories, warm kittens absorb cooking knowledge
- Cultural accumulation: colony knowledge persists across generations
- Remove "forcing long-term planning" framing — resource pressure emerges naturally from more mouths to feed

Key design points:
- Mating requires Mates bond + both healthy + food items in Stores building > 60% of storage capacity
- Conception: 5% per season when prerequisites met
- Pregnancy: 1 season (~2000 ticks); queen has increased food need, reduced speed, shelter-seeking
- Litter: 1–3 kittens (weighted: 1=40%, 2=40%, 3=20%)
- Kitten development: Newborn (0–1 season, milk-dependent) → Kitten (1–3, follows adults, plays) → Young (4–11, full actions, faster skill growth)
- Trait inheritance: personality = average of parents ± 0.15 per axis
- Kitten death: most severe mood event (-0.8 for parents, -0.4 colony)
- Memory inheritance: elders share stories → kittens absorb filtered by personality → cultural accumulation across generations
- Recipe inheritance: cooking/crafting knowledge passes through the story-time channel
- Kitten learning from observation: watching adults gives tiny skill XP + recipe knowledge

**Doc:** `docs/systems/reproduction.md` — implement with memory inheritance additions.

**Key files:** `src/components/identity.rs`, new `src/systems/reproduction.rs`, `src/ai/scoring.rs`, `src/systems/social.rs`.

---

## External World

### Trade & Visitors

**Status:** Solid foundation. Expanding for material chain and recipe exchange.
**Changes:**
- Trade is fully item-based: traders carry actual items, colony offers items in return
- Visitor gifts: loners sometimes bring items as social gestures
- Reputation from cooking: feasts and well-fed cats boost reputation ("word travels about the cuisine")
- Recipe exchange: visitors from distant territories may know recipes the colony hasn't discovered; social interaction transmits knowledge
- Scouts share ecological knowledge: prey locations, herb locations, danger areas → feeds memory system

Key design points:
- Visitor types: Wandering Loner, Trader, Hostile Loner, Scout
- Colony reputation: derived from food surplus, building quality, safety, population, trade history, feast events
- Recruitment: befriend loner (fondness > 0.6, familiarity > 0.4) → loner evaluates colony → joins or leaves
- Barter: trader carries 3–5 items; colony offers items; exchange if offer value >= 80% of requested
- Visitor check every 500 ticks; base 5% chance scaled by reputation

**Doc:** `docs/systems/trade.md` — implement with expansions.

**Key files:** New `src/systems/trade.rs`, new `src/components/visitors.rs`, `src/resources/relationships.rs`.

---

## Implementation Order

The material chain revelation reshapes the order. Items and ecosystem are foundational — everything else builds on real physical objects and real prey populations.

### Phase 1: The Material Chain
**Systems:** Items & Material Chain, Ecosystem & Wildlife Rework
**Theme:** Make the economy real. Prey are entities. Hunting produces items. Food stores are items in buildings.
**Scope:** This is the biggest rework — it touches hunting, food, wildlife, and inventory. But everything downstream depends on it.

### Phase 2: Crafting & Cooking
**Systems:** Crafting & Cooking (new)
**Theme:** Transform raw materials into refined goods. Introduce cooking as a skill. First feast event.
**Depends on:** Phase 1 (items, raw materials from ecosystem)

### Phase 3: Body & Environment
**Systems:** Body Zones, Environmental Quality, Corpse Handling
**Theme:** Physical depth — anatomical injuries, ambient comfort, grief and burial.
**Depends on:** Phase 1 (corpse handling integrates with ecosystem; env quality reads item-based food stores)

### Phase 4: Behavioral Depth
**Systems:** Recreation & Grooming, Substances, Emotional States, Activity Cascading
**Theme:** Observable behavior — leisure, addiction, emotional cascades, group dynamics.
**Depends on:** Phase 3 (grooming feeds disease; substances are items; emotional states compound with pain from body zones)

### Phase 5: Health & Generational
**Systems:** Disease, Reproduction & Memory Inheritance
**Theme:** Health chains close; generational play begins. Recipe inheritance creates cultural evolution.
**Depends on:** Phase 3 (body zones for wound infection), Phase 4 (grooming for infection risk), Phase 2 (recipe inheritance)

### Phase 6: The Outside World
**Systems:** Trade & Visitors, The Calling
**Theme:** External contact and legendary creation. Visitors bring new recipes. The Calling produces colony treasures.
**Depends on:** Phase 1 (items for trade), Phase 2 (recipes for exchange), Phase 5 (population for reputation)

### Verification per phase:
1. `just test` — all existing tests pass, new tests cover the phase's systems
2. `just run` — observe new systems interacting in simulation
3. `just check` — no clippy warnings, clean compile
4. Narrative log shows new event types
5. Chain reactions between phase systems and existing systems are visible

### End-to-end verification:
- Run simulation 5000+ ticks; observe multi-species ecosystem dynamics
- Run rats-only test: verify exponential growth
- Run full ecosystem without cats: verify food web dynamics
- Observe at least one feast event and verify memory creation
- Observe recipe discovery and cross-generational transmission
- Verify hunting depletes actual prey populations
- Verify cooking smells, grooming state, and comfort-seeking as visible ambient behaviors
