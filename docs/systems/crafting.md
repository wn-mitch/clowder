# Crafted Items & Recipes

## Purpose
General-purpose material economy. Cats gather raw materials, transport them to crafting stations, and combine them by recipe into output items. Generalizes the existing narrow patterns — remedy prep from herbs at the Workshop (`src/components/task_chain.rs:93`, "10 ticks at workshop, 15 without") and ward-setting from thornbriar (`src/systems/magic.rs`) — into a unified recipe + station + craft-action substrate. The recipe catalog is **§5-first** (preservation, play, grooming, courtship, mentorship, generational knowledge) and extends into **place-making** — crafted decorations anchored to colony sites (rugs, lamps, censers, wall-hangings, heritable ceremonial markers) that shape the environment every cat shares rather than buffing the cat who holds them. Combat gear is deferred or excluded.

Score: **V=5 F=4 R=3 C=3 H=3 = 540** — "worthwhile; plan carefully" per `systems-backlog-ranking.md`, promoted from 288 on 2026-04-22 when the decoration / place-making phase (Phase 4) was added. Load-bearing for `slot-inventory.md` (first producer of wearables), `ruin-clearings.md` (loot routes to crafting materials, not finished gear), and `naming.md` (Phase 3 and Phase 4 are two of the six NamedLandmark consumers). Among the split-out features from the 2026-04-22 triage, crafting is the anchor; ship it first.

## Design constraint: §5-first catalog + place-not-cat discipline
Every recipe targets at least one continuity canary or §5 sideways axis (grooming, play, courtship, burial, preservation, generational knowledge — `project-vision.md` §5). Recipes that don't justify themselves under §5 don't ship. The OSRS gravity well is avoided by construction by two rules:

1. **No numeric capability modifiers on the cat.** The `CraftedItem` type carries narrative/identity fields (name, origin, creator) and **no numeric modifier fields**. "Bonus" effects (brush → grooming-rate, rug → sleep-quality, lamp → night-visibility) live on the *action resolver* or the *environment* — the cat using a brush isn't buffed, the *grooming action* improves its output; the cat sleeping on a rug isn't buffed, the *hearth tile* is warmer.
2. **Decorations are place-anchored, not cat-anchored.** A rug warms the hearth tile; a lamp illuminates a room; a tapestry marks a wall. The cat that *placed* the decoration gets no personal bonus. Every colony member benefits from the decoration while occupying the site; nobody carries it as personal inventory. Carried-crafted-objects (tokens, gifts, talismans) stay narrative-only per rule 1.

Drift from *either* constraint is a thesis-breaking change and re-triggers ranking (F→2, H→2, composite score falls to ~96).

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
| Grooming Brush | Twig + Bristle (prey shedding) | Workshop | Grooming Brush (consumed by self-grooming action for +grooming-rate bonus, no stat modifier on the cat) |
| Play Bundle | Fiber + Feather | Workshop | Play Bundle (target object for Play action; kittens gain +0.1 play-need satisfaction; social-learning tag) |
| Courtship Gift | Polished Stone / Feather / Flower | Workshop | Gift Object (carried during Mating chain as expressive prop; +0.05 fondness gain in recipient) |

The "bonus" fields above live on the *action resolver*, not the item type — a brush doesn't buff the cat; using a brush improves the action's output. Type guardrail unchanged.

## Phase 3 — Identity & mentorship objects
Targets the generational-continuity and mythic-texture canaries. First producer of wearables for `slot-inventory.md`.

| Recipe | Inputs | Station | Output |
|--------|--------|---------|--------|
| Mentorship Token | Elder fur + Kitten's-first-catch trophy | Workshop | Named Token ("Cedar's First Catch") — wearable on slot-inventory Collar slot; carries narrative hook only |
| Heirloom Piece | Fine fiber + Named-object fragment | Workshop | Artisan-signed crafted item with inheritance hook |
| Calling Wearable | Outputs of The Calling trance | Workshop or Fairy-Ring | Wearable routing of existing Calling Named Objects |

Phase 3 is the integration point with `the-calling.md` (Calling Named Objects gain a wearable slot) and the trigger for `slot-inventory.md` to ship (first wearable producer).

**NamedLandmark substrate.** Phase 3 Named Objects are one of six convergent consumers of the shared naming substrate (registry + event-proximity matcher + event-keyed name templates) documented in `naming.md`. Event-driven naming — e.g. a Mentorship Token named from the kitten's first-catch event rather than a random generator — is the mythic-texture lever; implement against the shared matcher, not a per-stub name generator. Consumers: `paths.md`, crafting Phase 3 (this section), crafting Phase 4 (decorations below), `ruin-clearings.md` Phase 3, `the-calling.md`, `monuments.md`.

## Phase 4 — Domestic refinement (folk-craft tier)
Place-anchored decorations that shape the environment every cat shares. Targets the **preservation**, **generational knowledge**, and **mythic texture** axes of §5 simultaneously — heritable objects that outlive their makers, named via the `naming.md` substrate when Significant-tier events land near them.

| Recipe | Inputs | Station | Output (all place-anchored) |
|--------|--------|---------|--------------------|
| Reed Mat / Woven Rug | Fiber + Reed + Fine fur | Workshop | Placed at a tile: raises tile-warmth (sleep-quality, kitten-cradle bias); heritable across generations; eligible for naming via `naming.md` |
| Tallow Lamp | Rendered prey fat + Woven wick | Workshop | Placed at a tile: illuminates 3-tile radius at night; reduces night-fear in that area; requires periodic refuel (an attending-cat chain similar to Smoking Rack tending) |
| Scent Censer | Herb bundle (seasonal) + Ceramic-substitute vessel | Workshop | Placed at a tile: emits a colony-claimed scent in a radius; modulates `fox_scent_map` (repellent) and `prey_scent_map` (slight mask). Content herbs determine effect profile |
| Carved Comb | Bone or claw-shaped wood | Workshop | Placed at a tile as a grooming-station fixture: improves grooming-action output at that tile (the action is buffed, not the cat — preserves type guardrail) |
| Wall-Hanging | Fiber + pigment (berry / clay) | Workshop | Placed at a wall: colony-memory marker; naming-eligible on any Significant event near it; visually distinguishes sub-colony identity |
| Nesting Inlay | Shell + Stone + Fine fiber | Workshop | Placed into a nesting alcove: permanent upgrade of that alcove's preservation-weight and sleep quality; highly heritable |

All Phase 4 items are `CraftedDecoration` entries — placed at a tile, not carried on a cat. No numeric modifier fields on the item; effects live on the tile (warmth, scent, illumination) or the action resolver (grooming quality).

## Phase 5 — Elevated cat-craft (collective, multi-season)
Long-horizon tier: objects cats "work up to" as the colony matures. **Explicit not-DF guardrail:** no individual-cat artifact obsession; no season-long solo trances (that remains `the-calling.md`'s niche). Phase 5 production is collective (multi-cat) or cumulative (multi-season), never individual-rare-strike.

Phase 5 items are gated by three conditions, *all* required:
1. **Colony-age gating.** The colony must have persisted continuously for ≥3 sim-years. Materials accrete across seasons; prior to that the substrate isn't available in quantity.
2. **Material-scarcity gating.** Inputs include resources that only come from deep exploration (`exploration_map.rs`), cleared ruins (`ruin-clearings.md`), or cross-season storage (intact organ-caches, cured sinew, seasoned herbs).
3. **Skill gating via `aspirations.rs`.** At least one cat in the colony must have advanced on a relevant mastery arc — a new set of arcs co-introduced with this phase: `WeavingMastery`, `BoneShapingMastery`, `PigmentMastery`, `CairnMastery`. Mastery is a prerequisite for *availability* of the recipe, not a per-cast bonus; the cat who crafts doesn't have to be the mastered cat (so the arcs remain collective enablers, not personal-obsession triggers).

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
- **Consumes outputs of** `ruin-clearings.md` (crafting materials from cleared ruins).
- **Hooks into** `the-calling.md` for Named Object wearables (Phase 3) and for the not-DF discipline boundary (Phase 5).
- **Place-anchors into** `environmental-quality.md` (folded into the A-cluster refactor): Phase 4 decorations are the primary producers of tile-level environmental quality. Phase 4 ships with a minimal `TileAmenities` interface even if `environmental-quality.md` hasn't landed; the refactor reads it when ready.
- **Reads skill state from** `aspirations.rs` for Phase 5 gating. Introduces four new mastery arcs (`WeavingMastery`, `BoneShapingMastery`, `PigmentMastery`, `CairnMastery`) that live in `aspirations.rs`, not here.
- **Registers with** `naming.md` as a consumer kind (`LandmarkKind::CraftedObject` for Phase 3; `LandmarkKind::Decoration` for Phase 4 and most of Phase 5). Shrine-cairns cross-register with `monuments.md`.
- **Cross-linked with** `monuments.md` — shrine-cairns are the small-scale subset of monuments; larger civic / memorial monuments live in their own stub.

## Scope exclusions
- No combat gear. No `HuntingBonusItem`, no `DamageReductionMod`. Defer to post-Phase-5 re-triage if ever proposed.
- **Skill-via-aspirations is Phase 5's gating mechanism — the new mastery arcs live in `aspirations.rs`, not here.** This is a carve-out from the original "no craft-skill mastery arcs inside this stub" rule: arcs are defined in aspirations-land and *read* by crafting for recipe availability. The original rule's intent (crafting doesn't own skill substrate) is preserved.
- No player-directed crafting queue. Cats decide what to craft via scoring — same as every other action.
- No individual-cat artifact compulsion (Strange Moods analogue). `the-calling.md` owns that mechanism.

## Required hypothesis per phase (per Balance Methodology)
- **Phase 1:** *Preservation-enabled colonies accumulate winter buffer calories ⇒ `deaths_by_cause.Starvation` on seed-42 `--duration 900` remains 0 while season-3 food-stockpile median rises ~2×; mortality distribution shifts from late-winter to non-seasonal causes.*
- **Phase 2:** *§5 tools entering the inventory raises ecological-variety canary firings ⇒ grooming, play, and courtship action counts each rise ≥1× per soak (from currently-zero or near-zero on seed 42).*
- **Phase 3:** *Named objects entering the inventory raises mythic-texture canary ⇒ named-event count per sim year rises by ≥1 independent of Calling trigger rate.*
- **Phase 4:** *Place-anchored decorations raise tile-quality at colony-center sites ⇒ on seed-42 `--duration 900`, hearth-tile kitten-sleep count rises ≥1.5× vs. decoration-disabled control; mythic-texture count rises ≥1 additional named landmark per sim-year from decoration-origin events; `Starvation = 0` canary holds (no displacement of food-effort onto decoration-gathering).*
- **Phase 5:** *Colony-age + scarcity + skill gating produces a visible maturation arc ⇒ on a `--duration 1800` (30-min) deep-soak, a seed-42 colony that has crossed year-3 unlocks ≥1 Phase 5 recipe and produces ≥1 Phase 5 artefact; generational-continuity canary holds (kittens-to-adult count unchanged); no Phase 5 artefact is produced before year-3 on any controlled seed (gating holds).*

## Dependencies
- Benefits from A1 IAUS refactor (cleaner scoring integration) but does not hard-block on it.
- Phase 1 is independent.
- Phase 3 soft-depends on `slot-inventory.md` existing (otherwise wearables have nowhere to go).
- Phase 3 and Phase 4 soft-depend on `naming.md` for named outputs; both can ship with a neutral-fallback name generator if `naming.md` hasn't landed.
- Phase 4 soft-depends on `environmental-quality.md` (A-cluster refactor); can ship with a minimal `TileAmenities` interface if the refactor hasn't landed.
- **Phase 5 hard-depends on** `aspirations.rs` skill arcs (`WeavingMastery`, `BoneShapingMastery`, `PigmentMastery`, `CairnMastery`) being defined and readable. These arcs ship in the same PR as Phase 5 or as a precursor PR in `aspirations.rs`.
- Phase 5 soft-depends on `ruin-clearings.md` and `exploration_map.rs` for scarcity-gated inputs; both exist today (exploration map) or are in-flight (ruin-clearings).
- Phase 5 cross-references `monuments.md` for shrine-cairn scope boundary.

## Tuning Notes
_Record observations and adjustments here during iteration._
