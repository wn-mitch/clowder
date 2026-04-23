# Happy Paths (Usage-worn Trails)

## Purpose
Cats concentrate movement between high-utility destinations (sleep spots, food stashes, water, the hearth). Repeated traversal compresses terrain; worn tiles offer a small speed boost and reinforce preference, so paths emerge from usage rather than authored placement. Worn enough, a path becomes a **civilizational marker** — the colony writing its own behavioral history into the world as physical grain. Prey learn to avoid the highest-wear tiles (ecology of fear extended to traffic, not just threat). When a Significant-tier event lands within N tiles of a fresh trail, the path takes its name from the event rather than a random generator — giving the colony's territory a spatial-narrative vocabulary that grows with its history.

Score: **V=4 F=5 R=3 C=4 H=2 = 480** — "worthwhile; plan carefully" per `systems-backlog-ranking.md`. The `NamedLandmark` substrate (resolved 2026-04-22 as its own precursor stub, `naming.md`, score 640) is shared with five other consumers; paths is the canonical first consumer because path wear is spatially-anchored.

## Thesis alignment
- **Honest world, no director.** Paths are pure consequence. No placement logic, no authored hub-and-spoke, no difficulty scaling. The world shows what the cats did.
- **Ecology with metaphysical weight.** Terrain carries memory. A trail is a physical record of where the colony chose to be. Over generations, kitten-born cats inherit paths they didn't make — the same way kittens inherit social knowledge.
- **Sideways-broadening (§5).** Directly advances **preservation** and **generational knowledge** — the colony's behavior leaves an artefact that outlives individual cats. Event-driven naming advances **mythic texture** by turning routine geography into named ground.
- **Emergent complexity.** Two-way coupling with prey creates a genuine chain reaction: cats wear paths → prey avoid paths → hunt geography shifts off paths → new wear patterns form → named hunt routes emerge from kill events along them. This is the DF-beer-cats-puke-depression shape, not a polish feature.

## Substrate reuse: stamps an existing influence map
Path wear is additive to the `InfluenceMap` scaffolding in `src/systems/influence_map.rs` — per §5.6.9 the registry is `(Channel, Faction)`-keyed so "a 14th map (pheromone, fire-danger, sacred-site draw)" is a registration, not a schema change. The stamping pipeline is already proven by `ExplorationMap`, `CatPresenceMap`, `prey_scent_map`, `fox_scent_map`. Wear is a new `Channel::Civilizational × Faction::Colony` (or equivalent) registration — not a new resource type. **This is the single largest cost-saver** and the primary reason C=4 rather than C=3.

## NamedLandmark substrate
Named path segments are one of six convergent consumers of the shared naming substrate documented in `naming.md` (registry + event-proximity matcher + event-keyed name templates). Path-specific registration contract: ripeness = `wear > 0.6` (trail threshold); proximity radius = 3 tiles; name-ripeness cooldown = 200 ticks; fallback = stays unnamed (many trails never qualify, that's fine). See `naming.md` for the full event-kind → template mapping table; path-specific templates are under the "Path template" column there.

## Open scope questions (paths-local)
1. **Anti-monopoly threshold.** What fraction of seasonal colony traversal on one tile trips the anti-monopoly canary? Placeholder 15% pending Phase 1 observation.
2. **Fox / prey path use.** Default excludes predators and prey from path speed-boost (kept colony-cultural). Revisit only if an honest ecological reason appears (e.g. hawks perching near high-traffic tiles for opportunism).

## Initial parameters
| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Wear accrual per cat-step | +0.01 | Small; trail formation takes hundreds of crossings |
| Wear decay per tick (unused tile) | −0.0005 | Seasonal regeneration on unused routes |
| Speed boost threshold (minor worn) | wear > 0.3 | Visible effect without runaway reinforcement |
| Speed boost factor (minor worn) | 1.25× (every 4th step skips cost) | Modest enough to not dominate pathfinding |
| Trail threshold (rendered as trail) | wear > 0.6 | Visible distinct tile variant |
| Road threshold | wear > 0.85 | Rare; generational |
| Prey avoidance radius from trail tiles | 2 tiles | Bounded; doesn't evict prey globally |
| Prey avoidance strength | linear in `wear − 0.3` | Proportional, no hard cutoff |
| Naming event-proximity radius | 3 tiles | Event within 3 of fresh-trail midline names the path |
| Naming trail-age minimum | 200 ticks since crossing threshold | Prevents same-tick event/creation naming races |
| Max named segments per sim-year | 6 | Name-spam guardrail |

## Staging
- **Phase 1 — Wear field + render.** Register the civilizational channel against `InfluenceMap`; increment per cat-step; decay per tick; render overlay on the tilemap (reuses existing overlay render path). No speed boost, no prey avoidance, no naming. Validates accrual/decay balance in isolation. Required hypothesis: *wear concentrates on high-utility routes ⇒ on seed-42 `--duration 900`, 5–15% of tiles show non-zero wear; <3% reach trail threshold; wear distribution is long-tailed (top-5% of tiles hold >50% of total wear).*
- **Phase 2 — Pathfinding weight + prey avoidance.** Modify `step_toward` / `find_path` in `src/ai/pathfinding.rs` to accept a cost map; extend prey movement scoring to subtract near-trail tile desirability. Required hypothesis: *speed boost reinforces preference without monopolizing ⇒ path formation metrics from Phase 1 remain in range; prey density within 2 tiles of trail tiles drops 20–40% vs. equivalent unworn terrain; `Starvation = 0` and `ShadowFoxAmbush ≤ 5` canaries still hold.*
- **Phase 3 — Event-driven naming + NamedLandmark substrate.** Ships the shared substrate (registry + event-proximity matcher + event-kind name templates). First consumer: path segments. Follow-on consumers refactor onto it in their own PRs (`crafting.md` Phase 3, `ruin-clearings.md` Phase 3, `the-calling.md`). Required hypothesis: *named-path events raise mythic-texture canary ⇒ ≥1 named path per sim-year, independent of Calling trigger rate, without exceeding 6 named segments per sim-year.*

## Scope discipline (load-bearing — keeps H=2 instead of H=1)
Violating any of these re-triggers ranking (the self-reinforcement loop wants to run away, and these are the brakes):

1. **Wear decays.** No permanent tiles absent ongoing use. A road that falls out of rotation becomes a trail, then unworn, then wild again. Permanence is a director-shape failure mode.
2. **Speed boost is modest and bounded.** 1.25× max, never stacking beyond that. Scaling speed with wear without a ceiling turns every colony into a highway monopoly.
3. **Prey avoidance is proportional, not binary.** No "prey will not enter a trail tile" rule. Continuous fall-off in a 2-tile band.
4. **Name-spam guardrail is hard.** Max 6 named segments per sim-year. Cooldowns on re-naming existing paths. A world where every tile has a name is as silent as one where no tile does.
5. **Paths don't gate hunt scoring.** Cats may prefer paths, but a hunt that requires off-path movement must still be reachable. Speed-boost is additive, not multiplicative-with-penalty on off-path.

## Required hypothesis per Balance Methodology
Phase-specific hypotheses are listed under **Staging** above. Overall umbrella:

> *Cats concentrate movement between high-utility destinations; repeated traversal compresses terrain into speed-boosted trails; prey respond to traffic density; and Significant-tier events within N tiles of a fresh trail derive its name.* ⇒ *After 3 sim-years on seed 42 `--duration 900`: wear forms a long-tailed distribution (top-5% of tiles hold >50% of wear); 5–15% of tiles non-zero-wear; <3% reach trail threshold; prey density within 2 tiles of trail tiles drops 20–40% vs. equivalent unworn terrain; ≥1 named path per sim-year without exceeding 6 named segments per sim-year; `Starvation = 0` and `ShadowFoxAmbush ≤ 5` canaries hold.*

## Canaries to ship in the same PR (before Phase 3 merges)
1. **Path-formation canary.** ≥1 trail segment of length ≥6 tiles persists between day 90 and day 180 on seed 42. Detects under-formation (no paths emerging).
2. **Anti-monopoly canary.** No single path tile's wear exceeds 15% of total-colony seasonal traversal. Detects runaway self-reinforcement (single-highway monopoly).
3. **Named-landmark canary** (mythic-texture extension). ≥1 named path per sim-year from live-sim events, independent of Calling. Detects substrate dormancy.
4. **Name-spam canary.** ≤6 named path segments per sim-year. Detects matcher over-firing.

All four are failure modes not legible to existing canaries; a silent-mythic-register bug or a single-highway bug won't surface on `Starvation = 0`.

## Shadowfox comparison
Two shared risk structures with shadowfoxes — self-reinforcing feedback loop, new input to prey fear — but decisive differences that keep H=2 not H=1:

- **No mortality-spike failure mode.** Path misbehavior is aesthetic (monopoly or silence), not lethal. No new `deaths_by_cause` category.
- **Continuous, not Poisson.** Path wear accumulates per step, no rare-cascade variance. Shadowfoxes' Poisson-ambush shape is what makes them canary-hard to measure.
- **Canaries are formation-quality, not survival.** Easier to observe, easier to diff A/B without deep-soak runs.
- **Substrate reuse is deep.** Influence-map stamping + NamedLandmark reuse means the ongoing tax is concentrated in tuning 4 constants (accrual, decay, prey-avoid, name-spam), not maintaining a bespoke subsystem.

The self-reinforcement loop remains the axis to watch — it has the same runaway shape that made shadowfoxes expensive. Scope disciplines 1, 2, and 5 above are the brakes.

## Integration with existing systems
- **Stamps** `InfluenceMap` via a new channel × faction registration (`src/systems/influence_map.rs`).
- **Extends** `src/ai/pathfinding.rs::step_toward` and `find_path` to accept a cost map; callers opt-in by passing the wear field. Default behavior unchanged for non-cat agents (fox stalk paths, prey movement) unless explicitly wired.
- **Produces** a `NamedLandmark` registry that `crafting.md` Phase 3, `ruin-clearings.md` Phase 3, and `the-calling.md` register against for their named entities.
- **Extends** prey movement scoring in `src/systems/prey.rs` with a near-trail avoidance term mirroring `prey_corruption_avoidance`.
- **Reads** `NarrativeTier::Significant` events from `src/systems/narrative.rs` for event-proximity naming.
- **Rendering** adds a wear-level overlay to the tilemap (reuses F6/F7/F8 overlay pattern).

## Dependencies
- **Soft-gated on tilemap rendering stability.** Feature is only visible if wear renders; the existing `Sprite`-based tilemap work should be stable before Phase 1 ships.
- **No A1 IAUS hard-gate.** Pathfinding weight integration is below the GOAP layer.
- **Enables mythic-texture consumers.** `crafting.md` Phase 3, `ruin-clearings.md` Phase 3, and `the-calling.md`'s Named Objects all benefit if the `NamedLandmark` substrate ships here (or as a precursor PR).
- **Interacts with** `prey.rs` (new fear input) and `wildlife.rs` (fox pathing: decide whether foxes benefit from path speed-boost — likely *no*, to keep predator stealth asymmetric).

## Scope exclusions
- **No authored roads at colony founding.** Paths are pure emergence. A spawn-time hearth→water path is a director-shape trap.
- **No player-directed path designation.** Cats decide where to walk; paths follow.
- **No combat gear named landmark.** Name templates target §5 axes (preservation, courtship, generational) plus the bounded list above. "Battle of {cat}'s Road" is out-of-scope unless future re-triage adds it.
- **No fox / prey-side path use.** Paths are a colony-cultural artifact; predators don't benefit from colony-made infrastructure. (Revisit if an honest ecological reason appears — e.g., hawks perching near high-traffic tiles for opportunism.)

## Tuning Notes
_Record observations and adjustments here during iteration._
