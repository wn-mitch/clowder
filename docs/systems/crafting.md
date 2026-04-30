# Crafted Items & Recipes

## Purpose
General-purpose material economy. Cats gather raw materials, transport them to crafting stations, and combine them by recipe into output items. Generalizes the existing narrow patterns — remedy prep from herbs at the Workshop (`src/components/task_chain.rs:93`, "10 ticks at workshop, 15 without") and ward-setting from thornbriar (`src/systems/magic.rs`) — into a unified recipe + station + craft-action substrate. The recipe catalog is **§5-first** (preservation, play, grooming, courtship, mentorship, generational knowledge) and extends into **place-making** — crafted decorations anchored to colony sites (rugs, lamps, censers, wall-hangings, heritable ceremonial markers) that shape the environment every cat shares rather than buffing the cat who holds them. Combat gear is a recipe cluster — bone-tip spears, hide bracers, slings, flint blades — drawn from the Bone & Shell, Hide & Pelt, Fiber & Weaving, and Stonecraft disciplines; items carry ecological properties (material, weapon class, noise profile, durability tier) that action resolvers read, not generic modifier fields.

Score: **V=5 F=4 R=3 C=3 H=3 = 540** — "worthwhile; plan carefully" per `systems-backlog-ranking.md`, promoted from 288 on 2026-04-22 when the decoration / place-making phase (Phase 4) was added. Load-bearing for `slot-inventory.md` (first producer of wearables), `ruin-clearings.md` (loot routes to crafting materials, not finished gear), and `naming.md` (Phase 3 and Phase 4 are two of the six NamedLandmark consumers). Among the split-out features from the 2026-04-22 triage, crafting is the anchor; ship it first.

## Design constraints: §5-first catalog + place-not-cat discipline

Every recipe targets at least one continuity canary or §5 sideways axis (grooming, play, courtship, burial, preservation, generational knowledge — `project-vision.md` §5). Recipes that don't justify themselves under §5 don't ship. The OSRS gravity well is avoided by construction:

1. **Items are not stat sticks.** The `CraftedItem` type carries narrative/identity fields (name, origin, creator, material, weapon class) and **no generic numeric modifier fields**. Items have real mechanical effects — a grooming brush improves grooming output; hide bracers reduce damage taken; a bone-tip spear extends hunt reach — but those effects live on the *action resolver*, keyed to item identity and ecological properties, not on `attack_bonus` / `armor_rating` floats bolted to the item. A cat should never feel like they are comparing `+3` vs `+5`; they should feel like they are choosing between a spear that pierces well but snaps, and bracers that are quiet and durable but offer no reach. The equipment is a characterization system: a cat carrying bone knives and a woven cloak reads differently from one carrying hide-plated bracers and a flint-tipped spear, and those reads connect to the sim's existing noise, stealth, and hunting systems.
2. **Decorations are place-anchored, not cat-anchored.** A rug warms the hearth tile; a lamp illuminates a room; a tapestry marks a wall. The cat that *placed* the decoration gets no personal bonus. Every colony member benefits from the decoration while occupying the site; nobody carries it as personal inventory. Carried-crafted-objects (tokens, gifts, talismans) stay narrative-only per rule 1.

Drift from *either* constraint is a thesis-breaking change and re-triggers ranking (F→2, H→2, composite score falls to ~96).

## Material tiers

### Cat-native materials
Reed, bone, fur, feather, shell, rendered fat, berry / clay / ash pigment, sinew, raw hide, herb, flint, fieldstone — gathered from the environment or as prey byproduct.

### Found and traded metal
Cats do not smelt, forge, or cast. Metal enters the economy as:

- **`ScavengedMetal`** — nails, wire, pins, clasps, rings recovered from ruins or human-adjacent sites. Each piece arrives as a tagged object with provenance (ruin name, depth, finder). Scarce by design.
- **`TradedMetal`** — goods acquired from metalworking species (forge-culture neighbors; undesigned as of 2026-04-22, likely badger-analogues or similar). Arrives via trade caravans or diplomatic events.

Metal is precious and storied — a cat knows where each piece came from. Found-metal enters recipes as a material input for Adornment & Setting and for construction fasteners; no discipline *produces* it. A tiara is cat-made adornment work; the wire in it came from somebody else's forge. Metal's scarcity and provenance is itself a story generator: a traded iron clasp from a badger-smith two generations back is already lore.

## Crafting disciplines

Disciplines are the named schools of cat craft, each mapping to one or more `aspirations.rs` mastery arcs (Phase 5 gating) and defining a family of stations and material inputs.

**Combat gear is a recipe cluster, not a discipline.** Warrior's kit items draw from whichever discipline supplies the relevant material. This means a spear is a Bone & Shell Craft recipe, bracers are a Hide & Pelt Work recipe, and a sling is a Fiber & Weaving recipe — combat purpose doesn't define a separate production tree.

| Discipline | Mastery arc | Core inputs | Representative outputs |
|---|---|---|---|
| **Fiber & Weaving** | `WeavingMastery` | reed, grass, fine fur, sinew | rope, baskets, mats, nets, slings, cloth, woven cloaks |
| **Bone & Shell Craft** | `BoneShapingMastery` | bone, shell, tooth, claw | needles, combs, toggles, bone-tip pierce-weapons, light armor plates, ornaments |
| **Hide & Pelt Work** | `HideworkMastery` | raw hide, sinew, fat, bark-tannin | cured leather, bracers, quivers, pouches, armor wraps |
| **Herbalism & Remedy** | *(generalizes `remedy_prep`)* | herbs, rendered fat, roots | poultices, salves, ward-herbs, preservation aids |
| **Pigment & Mark** | `PigmentMastery` | berry, clay, ash, charcoal | dyes for textiles, ceremonial pigments, ink |
| **Stonecraft & Cairn** | `CairnMastery` | flint, fieldstone, grinding stone | knapped blades, weights, grinding stones, cairns, shrine-stones |
| **Preservation & Foodcraft** | *(base colony skill; no mastery arc)* | raw food + fuel/herbs | dried fish, smoked meat, preserved organ |
| **Adornment & Setting** | *(draws on Bone, Shell, Pigment arcs; `ScavengedMetal` / `TradedMetal` eligible)* | shell, fine bone, stone, wire, found-metal | tiaras, collars, ceremonial markers — cats set and assemble; the metal arrived from elsewhere |

## Phase 1 — Food preservation (ships first)
Targets the starvation canary directly (winter buffer calories) and the preservation axis of §5.

| Recipe | Inputs | Station | Time | Output |
|--------|--------|---------|------|--------|
| Dried Fish | 1 Raw Fish | Drying Rack | ~3 days of Clear weather | 1 Dried Fish (doesn't spoil; 0.7× fresh hunger restore) |
| Smoked Meat | 1 Raw Meat + 1 Fuel | Smoking Rack | ~1 day tending | 1 Smoked Meat (doesn't spoil; 0.8× fresh hunger restore) |
| Preserved Organ | 1 Organ + 1 herb | Drying Rack | ~2 days | 1 Preserved Organ (retains mood bonus of fresh organ) |

Stations: Drying Rack (sun-powered, weather-sensitive); Smoking Rack (requires Fuel and an attending cat for tending cycles).

## Phase 2 — §5 behavioral tools
Targets the ecological-variety canary (grooming, play, courtship firing ≥1× per soak).

| Recipe | Inputs | Station | Output |
|--------|--------|---------|--------|
| Grooming Brush | Twig + Bristle (prey shedding) | Workshop | Grooming Brush — grooming resolver reads brush presence and improves output; the cat is not buffed, the action is |
| Play Bundle | Fiber + Feather | Workshop | Play Bundle — target object for Play action; kittens gain higher play-need satisfaction; social-learning tag |
| Courtship Gift | Polished Stone / Feather / Flower | Workshop | Gift Object — carried during Mating chain as expressive prop; fondness resolver reads gift presence |

Effects live on the action resolver keyed to item identity, not on modifier fields on the item type.

## Phase 2b — Warrior's kit
Targets hunt-success and survival canaries. Requires Hide & Pelt Work infrastructure (Tanning Frame station, which extends the Drying Rack). Items carry ecological properties — material, weapon class, noise profile, durability tier — that the hunt, combat, and movement resolvers read. No item in this cluster carries a generic numeric modifier field.

**Material properties that resolvers read:**

- **Bone** — good pierce (holds a point along the grain), brittle under lateral load (snap risk on failed strikes or slashing contact). Breaking is a mechanical event: a snapped bone-tip spear mid-hunt is a story. Bone weapons are light and silent.
- **Flint / knapped stone** — better cutting edge than bone, chips rather than snaps. Closer to slash-capable, but still disposable. Stone is heavier than bone.
- **Cured hide** — quiet, flexible, absorbs blunt impact; no snap risk. Does not offer pierce resistance. Durable with maintenance.
- **Fiber (woven reed, sinew)** — light, silent, no armor value. Slings provide reach; cloaks reduce visual signature.
- **`ScavengedMetal` / `TradedMetal`** — durable, holds a slash edge, does not snap. But metal-on-metal or metal-on-stone is loud; the noise resolver applies a significant detection penalty. A cat in traded-iron gear cannot hunt silently. This is the primary reason cats want metal but can't simply replace all their kit with it.

| Recipe | Discipline | Station | Properties | Output |
|--------|------------|---------|------------|--------|
| Bone-Tip Spear | Bone & Shell Craft | Workshop | pierce-class, silent, fragile | Hunt-strike resolver applies extended reach; snap risk on failed strike |
| Bone Stiletto | Bone & Shell Craft | Workshop | pierce-class, silent, fragile, concealable | Close-range pierce; light enough to carry alongside other kit |
| Flint Blade | Stonecraft | (open ground; no station) | slash-capable, silent, chips | Combat and hunt resolvers key on blade presence; edge degrades faster than metal |
| Hide Bracers | Hide & Pelt Work | Tanning Frame | blunt-absorb, silent, durable | `take_damage` resolver reduces blunt damage while worn; no pierce resistance |
| Hide-Plated Wrap | Hide & Pelt Work | Tanning Frame | blunt-absorb, pierce-partial, silent, durable | Heavier than bracers; more coverage; still silent |
| Sling | Fiber & Weaving | Workshop | ranged, silent, no melee value | Ranged-attack resolver; fieldstone is ammunition, not a crafted item |
| Woven Reed Cloak | Fiber & Weaving | Workshop | noise-silent, visual-mask, no armor | Movement resolver reduces visual detection radius; no combat value |
| Tooth-Notched Club | Bone & Shell Craft | Workshop | blunt-class, silent, fragile | Impact resolver applies blunt damage; teeth chip on hard targets |

**On traded metal:** a `ScavengedMetal` or `TradedMetal` blade or fastener-reinforced weapon can be produced by the Adornment & Setting discipline once the colony has access to found metal. These are rare, named, inherited — not routine kit. The noise penalty means a cat wearing full traded-iron gear is choosing protection over stealth, which is a meaningful trade-off with real hunting consequences.

## Phase 3 — Identity, mentorship & adornment
Targets the generational-continuity and mythic-texture canaries. First producer of wearables for `slot-inventory.md`. Adornment & Setting enters here as a discipline producing identity objects rather than functional gear.

| Recipe | Inputs | Station | Output |
|--------|--------|---------|--------|
| Mentorship Token | Elder fur + Kitten's-first-catch trophy | Workshop | Named Token ("Cedar's First Catch") — wearable on slot-inventory Collar slot; narrative hook only |
| Heirloom Piece | Fine fiber + Named-object fragment | Workshop | Artisan-signed crafted item with inheritance hook |
| Calling Wearable | Outputs of The Calling trance | Workshop or Fairy-Ring | Wearable routing of existing Calling Named Objects |
| Shell Collar | Shell × 3 + sinew lacing | Workshop | Identity wearable; no mechanical effect; social-reading signal for other cats |
| Bone-and-Wire Tiara | Fine bone + `ScavengedMetal` wire + shell | Workshop | Identity wearable; the cat shaped and set it; the wire came from elsewhere; naming-eligible |
| Stone-Set Pin | Polished stone + `ScavengedMetal` pin | Workshop | Identity wearable; Adornment & Setting discipline's simplest found-metal recipe |

Phase 3 is the integration point with `the-calling.md` (Calling Named Objects gain a wearable slot) and the trigger for `slot-inventory.md` to ship (first wearable producer).

**NamedLandmark substrate.** Phase 3 Named Objects are one of six convergent consumers of the shared naming substrate (registry + event-proximity matcher + event-keyed name templates) documented in `naming.md`. Event-driven naming — e.g. a Mentorship Token named from the kitten's first-catch event rather than a random generator — is the mythic-texture lever; implement against the shared matcher, not a per-stub name generator. Consumers: `paths.md`, crafting Phase 3 (this section), crafting Phase 4 (decorations below), `ruin-clearings.md` Phase 3, `the-calling.md`, `monuments.md`.

## Phase 4 — Domestic refinement (folk-craft tier)
Place-anchored decorations that shape the environment every cat shares. Targets the **preservation**, **generational knowledge**, and **mythic texture** axes of §5 simultaneously — heritable objects that outlive their makers, named via the `naming.md` substrate when Significant-tier events land near them.

| Recipe | Inputs | Station | Output (all place-anchored) |
|--------|--------|---------|--------------------|
| Reed Mat / Woven Rug | Fiber + Reed + Fine fur | Workshop | Placed at a tile: raises tile-warmth (sleep-quality, kitten-cradle bias); heritable across generations; eligible for naming via `naming.md` |
| Tallow Lamp | Rendered prey fat + Woven wick | Workshop | Placed at a tile: illuminates 3-tile radius at night; reduces night-fear in that area; requires periodic refuel (an attending-cat chain similar to Smoking Rack tending) |
| Scent Censer | Herb bundle (seasonal) + Ceramic-substitute vessel | Workshop | Placed at a tile: emits a colony-claimed scent in a radius; modulates `fox_scent_map` (repellent) and `prey_scent_map` (slight mask). Content herbs determine effect profile |
| Carved Comb | Bone or claw-shaped wood | Workshop | Placed at a tile as a grooming-station fixture: improves grooming-action output at that tile (the action is buffed, not the cat) |
| Wall-Hanging | Fiber + pigment (berry / clay) | Workshop | Placed at a wall: colony-memory marker; naming-eligible on any Significant event near it; visually distinguishes sub-colony identity |
| Nesting Inlay | Shell + Stone + Fine fiber | Workshop | Placed into a nesting alcove: permanent upgrade of that alcove's preservation-weight and sleep quality; highly heritable |

All Phase 4 items are `CraftedDecoration` entries — placed at a tile, not carried on a cat. No numeric modifier fields on the item; effects live on the tile (warmth, scent, illumination) or the action resolver (grooming quality).

## Phase 5 — Elevated cat-craft (collective, multi-season)
Long-horizon tier: objects cats "work up to" as the colony matures. **Explicit not-DF guardrail:** no individual-cat artifact obsession; no season-long solo trances (that remains `the-calling.md`'s niche). Phase 5 production is collective (multi-cat) or cumulative (multi-season), never individual-rare-strike.

Phase 5 items are gated by three conditions, *all* required:
1. **Colony-age gating.** The colony must have persisted continuously for ≥3 sim-years. Materials accrete across seasons; prior to that the substrate isn't available in quantity.
2. **Material-scarcity gating.** Inputs include resources that only come from deep exploration (`exploration_map.rs`), cleared ruins (`ruin-clearings.md`), or cross-season storage (intact organ-caches, cured sinew, seasoned herbs).
3. **Skill gating via `aspirations.rs`.** At least one cat in the colony must have advanced on a relevant mastery arc — a new set of arcs co-introduced with this phase: `WeavingMastery`, `BoneShapingMastery`, `HideworkMastery`, `PigmentMastery`, `CairnMastery`. Mastery is a prerequisite for *availability* of the recipe, not a per-cast bonus; the cat who crafts doesn't have to be the mastered cat (so the arcs remain collective enablers, not personal-obsession triggers).

| Recipe | Inputs | Station | Output (multi-cat or multi-season) |
|--------|--------|---------|-------------------------------------|
| Generational Tapestry | Fiber × seasons + Pigment × seasons + contributions from ≥3 cats | Workshop (placed at a wall for seasons during accumulation) | A tapestry whose weave records the season(s) and contributing cats; naming via `naming.md` draws from the aggregate of events that happened while it was being woven |
| Shrine-Cairn | Stone × 10+ + a consecrating event (named event at a site) | In-situ (stones moved to a sacred tile, no station) | Heaped-stone ritual marker; scent-claimed; weather-durable; visible across tiles. Overlaps with `monuments.md` — small shrine-cairns live here, larger memorial cairns cross-reference to `monuments.md` |
| Bone-Lattice Lantern | Fine bones × 20 + tallow refuel + `BoneShapingMastery` | Workshop | Elevated Phase 4 lamp with 5-tile illumination radius; requires less refuel; naming-eligible on hearth-events |
| Pigment-Deepened Textile | Phase 4 wall-hanging or rug + pigment × ≥3 seasons | Workshop | Upgrade applied to an existing Phase 4 textile; each seasonal dyeing deepens a visual marker readable as colony-age |
| Multi-Cat Nesting Alcove | Stone × 20 + Shell × 10 + Fine fiber × many + ≥2 cats contributing over ≥2 seasons | In-situ | Upgraded communal sleeping site; named after a coming-of-age or courtship event that happened in/near it |
| Kitten-Cradle Basket | Fine fiber + contribution from kitten's extended family | Workshop | Lineage-bound cradle — naming is parented by the kitten's line, persists as inheritance when the kitten ages out |

### Phase 5 safeguards against DF-drift
- **No individual artifact compulsion.** Phase 5 is never driven by a mood-strike on a single cat. That mechanism belongs to `the-calling.md`; crafting doesn't replicate it.
- **No artisan hierarchy in-sim.** Mastery via `aspirations.rs` is a latent colony property, not a visible rank. A cat with `WeavingMastery` isn't addressed differently by other cats; they're just a cat who has practiced enough for the colony to unlock certain recipes.
- **Collective contribution is mechanical, not flavor.** A Generational Tapestry doesn't *suggest* ≥3 contributors — the recipe *requires* contributions from ≥3 distinct cats across ≥2 seasons, recorded in the crafted object's history. Enforced in code, not prose.

## Integration with existing systems
- **Generalizes** existing `remedy_prep` (one-off in `magic.rs`) and `ward_setting` (thornbriar → Ward) as recipes in the unified catalog. Do not leave parallel code paths after Phase 1 lands.
- **Produces inputs for** `slot-inventory.md` Phase 3 (wearables) and feeds into existing `needs::eat_from_inventory` (preserved food).
- **Consumes outputs of** `ruin-clearings.md` (crafting materials from cleared ruins). `ScavengedMetal` is a primary loot type from cleared ruins.
- **Hooks into** `the-calling.md` for Named Object wearables (Phase 3) and for the not-DF discipline boundary (Phase 5).
- **Place-anchors into** `environmental-quality.md` (folded into the A-cluster refactor): Phase 4 decorations are the primary producers of tile-level environmental quality. Phase 4 ships with a minimal `TileAmenities` interface even if `environmental-quality.md` hasn't landed; the refactor reads it when ready.
- **Reads skill state from** `aspirations.rs` for Phase 5 gating. Introduces five new mastery arcs (`WeavingMastery`, `BoneShapingMastery`, `HideworkMastery`, `PigmentMastery`, `CairnMastery`) that live in `aspirations.rs`, not here.
- **Registers with** `naming.md` as a consumer kind (`LandmarkKind::CraftedObject` for Phase 3; `LandmarkKind::Decoration` for Phase 4 and most of Phase 5). Shrine-cairns cross-register with `monuments.md`.
- **Cross-linked with** `monuments.md` — shrine-cairns are the small-scale subset of monuments; larger civic / memorial monuments live in their own stub.
- **Feeds the noise resolver** via Phase 2b equipment properties. Metal gear found via `ScavengedMetal` / `TradedMetal` routes carries a loud noise profile; the hunting and stealth resolvers read this to apply detection penalties.

## Scope exclusions
- **No stat-stick items.** No generic `attack_bonus`, `armor_rating`, `damage_reduction`, or `speed_modifier` fields on `CraftedItem`. Combat gear (spears, bracers, slings, blades) is included as Phase 2b; all effects live on action resolvers (hunt, combat, movement, noise) keyed to item identity and ecological properties.
- **No metalworking discipline.** Cats do not smelt, forge, or cast. `ScavengedMetal` and `TradedMetal` are obtainable material inputs from ruins and trade; neither is producible by any cat discipline. Adornment & Setting works *with* found metal; it does not produce it. Metalworking belongs to other sapient species (undesigned as of 2026-04-22).
- **Skill-via-aspirations is Phase 5's gating mechanism — the new mastery arcs live in `aspirations.rs`, not here.** This is a carve-out from the original "no craft-skill mastery arcs inside this stub" rule: arcs are defined in aspirations-land and *read* by crafting for recipe availability. The original rule's intent (crafting doesn't own skill substrate) is preserved.
- No player-directed crafting queue. Cats decide what to craft via scoring — same as every other action.
- No individual-cat artifact compulsion (Strange Moods analogue). `the-calling.md` owns that mechanism.

## Required hypothesis per phase (per Balance Methodology)
- **Phase 1:** *Preservation-enabled colonies accumulate winter buffer calories ⇒ `deaths_by_cause.Starvation` on seed-42 `--duration 900` remains 0 while season-3 food-stockpile median rises ~2×; mortality distribution shifts from late-winter to non-seasonal causes.*
- **Phase 2:** *§5 tools entering the inventory raises ecological-variety canary firings ⇒ grooming, play, and courtship action counts each rise ≥1× per soak (from currently-zero or near-zero on seed 42).*
- **Phase 2b:** *Warrior's kit in the item pool changes hunt-success distribution without increasing starvation ⇒ on seed-42 `--duration 900`, hunt-success rate rises ≥1.1× for equipped cats vs. unequipped; `deaths_by_cause.Starvation` remains 0; bone-weapon snap events appear in the log ≥1× per soak confirming durability mechanics fire.*
- **Phase 3:** *Named objects entering the inventory raises mythic-texture canary ⇒ named-event count per sim year rises by ≥1 independent of Calling trigger rate.*
- **Phase 4:** *Place-anchored decorations raise tile-quality at colony-center sites ⇒ on seed-42 `--duration 900`, hearth-tile kitten-sleep count rises ≥1.5× vs. decoration-disabled control; mythic-texture count rises ≥1 additional named landmark per sim-year from decoration-origin events; `Starvation = 0` canary holds (no displacement of food-effort onto decoration-gathering).*
- **Phase 5:** *Colony-age + scarcity + skill gating produces a visible maturation arc ⇒ on a `--duration 1800` (30-min) deep-soak, a seed-42 colony that has crossed year-3 unlocks ≥1 Phase 5 recipe and produces ≥1 Phase 5 artefact; generational-continuity canary holds (kittens-to-adult count unchanged); no Phase 5 artefact is produced before year-3 on any controlled seed (gating holds).*

## Dependencies
- Benefits from A1 IAUS refactor (cleaner scoring integration) but does not hard-block on it.
- Phase 1 is independent.
- Phase 2b depends on Tanning Frame station (new) and benefits from noise-resolver existing; can stub the noise penalty if the resolver isn't ready.
- Phase 3 soft-depends on `slot-inventory.md` existing (otherwise wearables have nowhere to go).
- Phase 3 and Phase 4 soft-depend on `naming.md` for named outputs; both can ship with a neutral-fallback name generator if `naming.md` hasn't landed.
- Phase 4 soft-depends on `environmental-quality.md` (A-cluster refactor); can ship with a minimal `TileAmenities` interface if the refactor hasn't landed.
- **Phase 5 hard-depends on** `aspirations.rs` skill arcs (`WeavingMastery`, `BoneShapingMastery`, `HideworkMastery`, `PigmentMastery`, `CairnMastery`) being defined and readable. These arcs ship in the same PR as Phase 5 or as a precursor PR in `aspirations.rs`.
- Phase 5 soft-depends on `ruin-clearings.md` and `exploration_map.rs` for scarcity-gated inputs; both exist today (exploration map) or are in-flight (ruin-clearings).
- Phase 5 cross-references `monuments.md` for shrine-cairn scope boundary.
- `ScavengedMetal` as a material type soft-depends on `ruin-clearings.md` providing it as a loot class; can be stubbed as a zero-probability drop until that system lands.

## Tuning Notes
_Record observations and adjustments here during iteration._
