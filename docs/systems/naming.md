# NamedLandmark Substrate — Event-Anchored Naming

## Purpose
Shared substrate for "colony produces a named entity that outlives its maker." A registry + event-proximity matcher + event-keyed name templates, used by every system that contributes to the **mythic-texture** continuity canary (`project-vision.md` §"Continuity canaries": ≥1 named event per sim year). Six stubs converge on this substrate:

- `paths.md` — named path segments / trails / roads.
- `crafting.md` Phase 3 — Named Objects (Mentorship Token, Heirloom Piece, Calling Wearable).
- `crafting.md` Phase 4 — Named Decorations (heritable rugs, tapestries, wall-hangings, scent censers, nesting inlays).
- `ruin-clearings.md` Phase 3 — Named-object drops from cleared ruins.
- `the-calling.md` — Named Wards / Named Remedies / Spirit Totems / Woven Talismans.
- `monuments.md` — Burial Mounds, Coming-of-Age Stones, Defender's Memorials, Pact Circles, Founding Stones.

Without a shared substrate each of those six rolls its own name generator, producing six slightly-different naming grammars and six independent event-proximity matchers. Building the substrate once and letting consumers register against it is the leverage move.

Score: **V=2 F=5 R=4 C=4 H=4 = 640** — "worthwhile" scaffolding per `systems-backlog-ranking.md`. V=2 because the substrate has no in-world effect until a consumer ships (same pattern as `slot-inventory.md`). V rises to effective-4 once one consumer registers, to effective-5 when three or more have shipped against it (cross-stub mythic-texture cascade). With six consumers now in the convergence set (five stubs plus this one), per-consumer leverage is near-maximum.

## Thesis alignment
- **Mythic texture is the thesis.** The named-event canary (`project-vision.md` §"Continuity canaries") asks the colony to produce ≥1 named event per sim year from live-sim sources. Named wards / calling outputs / paths / crafted heirlooms / decorations / ruin drops / monuments are six distinct mythic-texture vehicles; a shared substrate lets all six light the same canary with the same name quality.
- **Honest world, no director.** Names derive from events the colony generates, not from authored templates. "The Last Trace of Cedar" only exists because Cedar died on that segment. Silent on silent seasons.
- **Generational knowledge.** Named entities persist across cat generations. Kittens inherit a world whose vocabulary of places and objects was written by elders they may never have met.
- **Emergent complexity.** A named hearth rug from winter-3 influences kitten sleep locations in winter-7, which reinforces the hearth's place-anchor, which names the *next* rug differently. Chain reactions across decades.

## What lives in this substrate
Three components, all registry-backed so consumers compose:

### 1. NamedLandmark registry
A single resource (`Res<NamedLandmarkRegistry>`) keyed by `LandmarkId`. Stored fields:
| Field | Type | Purpose |
|---|---|---|
| `id` | `LandmarkId` | Stable unique handle |
| `name` | `String` | Generated name (e.g. "Silvermoon's Winter Rug") |
| `kind` | `LandmarkKind` enum | Path / CraftedObject / Decoration / Ward / Remedy / Totem / Talisman / RuinDrop / Monument |
| `anchor` | `LandmarkAnchor` | Tile position, entity, or both (see below) |
| `creator` | `Option<Entity>` | Cat credited with the landmark; `None` for emergent (paths) |
| `genesis_tick` | `u64` | When the landmark was named |
| `source_event` | `NarrativeEventRef` | Event that drove the naming |
| `name_template` | `NameTemplateId` | Which template produced the name (for narrative replay) |

`LandmarkAnchor` is an enum:
- `Tile(Position)` — path segments, decorations at fixed tiles.
- `Entity(Entity)` — crafted wearables, wards, carried objects.
- `Both { tile: Position, entity: Entity }` — placed decorations (rug-at-hearth).

The registry is additive — landmarks enter, they don't leave except on explicit destruction (cat dies wearing a Named Token, rug worn out, path fully decayed). Entries carry a `decayed` flag; destroyed entries aren't pruned, to preserve narrative continuity ("the old Thornheart Ward, now fallen").

### 2. Event-proximity matcher
A system (`match_naming_events`) that runs after `narrative.rs` emits events each tick. For every `NarrativeTier::Significant` event, it computes spatial proximity (via `event.location`) to landmark candidates:

- **Fresh landmarks** — landmarks whose `genesis_tick` is `None` but which have crossed a consumer-defined "ripe for naming" threshold (path crossed trail-wear threshold, crafted object left the loom, ward placed, etc.). Consumers call `registry.register_unnamed(LandmarkCandidate { ... })` when an entity becomes ripe; the matcher then watches for naming events in its radius.
- **Event-within-radius** — if an event lands within consumer-specified tiles of a candidate, and no cooldown applies, the matcher names the landmark using the event-kind → template mapping below.

### 3. Event-kind → name-template mapping
The canonical table (copy-pasted from `paths.md` §"Design constraint" and extended for non-path consumers):

| Event kind (`NarrativeTier::Significant`) | Path template | Object template | Decoration template | Ward/Remedy/Totem template |
|---|---|---|---|---|
| Cat death on/near anchor | "The Last Trace of {cat}" | "{cat}'s Final {object_type}" | "{cat}'s Parting Rug" | "{cat}'s Last Ward" |
| Banishment | "The Walk of Banishment" | "{cat}'s Exile {object}" | — | — |
| Calling-trance success | "{cat}'s Dreaming Path" | "{cat}'s {herb_motif} {object}" | — | "{cat}'s {herb_motif} Ward" |
| First-catch by kitten | "{kitten}'s First-Prey Trace" | "{kitten}'s First Token" | "{kitten}'s Cradle Rug" | — |
| Courtship-pair formation | "{cat-a} and {cat-b}'s Meeting Road" | "{cat-a}'s Courtship Gift to {cat-b}" | "{pair}'s Bonding Tapestry" | — |
| Shadow-fox ambush survived | "The Dark-Hour Passage" | — | "{cat}'s Ward-Rug" | "The Survivor's Ward" |
| Kitten reaches adulthood | "{cat}'s Coming-of-Age Walk" | "{cat}'s Majority Token" | — | — |
| Ruin cleared | "The {ruin-name} Road" | "{clearing-leader}'s {loot} of {ruin-name}" | — | — |

**Monument self-naming.** Monuments (see `monuments.md`) use a distinct naming path: the *raising action* emits a Significant-tier event whose subject metadata directly feeds a monument-kind-specific template (Burial Mound → "Where {cat} Sleeps"; Coming-of-Age Stone → "{season-year}'s Crossing"; Defender's Memorial → "{cat} Held the {compass-bearing} Edge"; Pact Circle → "Where {cat-a} and {cat-b} Bound"; Founding Stone → "This Colony Began With {founder(s)}"). Proximity radius = 0; the matcher treats monuments as self-naming at build completion rather than as proximity candidates. Other consumers retain the proximity model above.

The name-spam guardrail (≤6 named landmarks per sim-year, aggregated across all consumers) lives in the registry, not per-consumer. This is load-bearing — six consumers firing independently would produce 30+ names per sim-year without the shared ceiling. Monuments carry their own per-year cap (≤4 monuments per sim-year, see `monuments.md`) which nests inside the shared ceiling.

## Consumers and their registration contracts
Each consumer registers a `LandmarkKind` with:
- A **ripeness criterion** — when does an entity become a naming candidate? (Path: `wear > 0.6`. Rug: "fresh from loom." Ward: "placed.")
- A **proximity radius** — how close does an event need to land to name this candidate? (Path: 3 tiles. Placed decoration: 2 tiles. Wearable on a cat: 0 tiles — naming triggers on events the wearer is involved in.)
- A **cooldown / maximum-age** — how long after ripeness can the candidate still be named? (Path: 200 ticks. Rug: until worn out. Ward: until destroyed.)
- A **fallback** — if no event names the candidate within its window, does it stay unnamed forever, or fall back to a neutral generator? (Path: stays unnamed, that's fine; many trails never get named. Crafted object: falls back to creator-anchored neutral name "Silvermoon's Rug" without event flavor.)

This registration-over-hardcoding is the §5.6.9 discipline already used by `InfluenceMap` — treat this substrate the same way.

## Initial parameters
| Parameter | Initial Value | Rationale |
|-----------|---------------|-----------|
| Proximity radius (path) | 3 tiles | Event within 3 of fresh-trail midline names the path |
| Proximity radius (placed decoration) | 2 tiles | Tight binding to site |
| Proximity radius (carried object) | 0 tiles | Event must involve the wearer |
| Name-ripeness cooldown (path) | 200 ticks | Prevents same-tick event/creation naming races |
| Name-ripeness cooldown (crafted object) | 100 ticks | Workshop-fresh objects need a window to gather events |
| Max named landmarks per sim-year (all consumers) | 6 | Name-spam guardrail; shared across all registrations |
| Fallback name generator | Creator-anchored neutral | "Silvermoon's Rug" when no event qualifies |
| Decay flag vs. pruning | Flagged, never pruned | Preserves narrative continuity; memory cost is acceptable at the landmark volumes this substrate produces (<1k per colony-century) |

## Staging
- **Phase 1 — Registry + matcher + path consumer.** Ships with `paths.md` Phase 3 as first consumer. Required hypothesis: *event-proximity naming produces ≥1 named landmark per sim-year from live-sim sources on seed-42 `--duration 900`, from a budget ceiling of 6; no landmark naming races; fallback neutral generator is invoked <20% of the time.*
- **Phase 2 — Crafting Phase 3 consumer retrofit.** Mentorship Tokens, Heirloom Pieces, Calling Wearables register against the substrate instead of rolling their own names. Existing Calling name generator (`random_adjective() + random_material_word() + random_form()`) becomes the *fallback* for Calling creations with no qualifying event. Required hypothesis: *named-crafted-objects raise mythic-texture canary count independently of paths; total named-landmark count per sim-year stays ≤6 (budget holds).*
- **Phase 3 — Ruin-clearings + the-Calling + decorations + monuments retrofit.** Last four consumer wires. Required hypothesis: *six independent naming sources produce a steady 3–6 named landmarks per sim-year (not clustered); budget ceiling holds; no single consumer dominates the name distribution.*

## Scope discipline (load-bearing — keeps H=4)
1. **Name-spam guardrail is hard and shared.** ≤6 named landmarks per sim-year, counted across all consumers. A consumer that fires too often starves the others; that's the right feedback signal, not a bug.
2. **Fallback is always available.** A consumer that demands naming (e.g. Calling creations — every Named Ward has had a name historically) falls back to its pre-substrate generator when no event qualifies. No consumer blocks on the event-proximity matcher.
3. **Names are narrative, not mechanical.** A `NamedLandmark` carries no numeric modifiers. Effects (path speed-boost, rug warmth, ward strength) live on the *landmark's subsystem*, not on the registry. The registry is a vocabulary, not a stat sheet.
4. **The registry is additive.** Decayed landmarks are flagged, not pruned. The narrative history of the colony persists.
5. **No player-directed naming.** The event-proximity matcher decides, using the canonical template table. Letting the player override produces director-shape playthroughs.

## Required hypothesis per Balance Methodology
Per-phase hypotheses above. Umbrella:

> *A shared registry + event-proximity matcher + event-keyed template table produces colony-authored landmark names tied to the events that made them memorable.* ⇒ *On seed-42 `--duration 900` with all six consumers wired: 3–6 named landmarks per sim-year from live-sim sources, distributed across at least three different consumer kinds; mythic-texture canary passes; budget ceiling of 6 never exceeded; fallback rate <20%.*

## Canaries to ship in the same PR
1. **Named-landmark canary** (mythic-texture). ≥1 named landmark per sim-year from live-sim events.
2. **Name-spam canary.** ≤6 named landmarks per sim-year, aggregated across consumers.
3. **Consumer-diversity canary.** After all six consumers land, ≥3 distinct consumer kinds contribute names per sim-year. Detects single-consumer monopolization.
4. **Fallback-rate canary.** <20% of landmarks use the fallback neutral generator. High fallback rate means events are landing outside proximity windows — tuning signal.

## Shadowfox comparison
This is scaffolding, not a feature; shadowfox risk is minimal:
- **No feedback loop** — the substrate is write-once per landmark; no self-reinforcement.
- **No scoring interaction** — names don't change cat decisions.
- **No mortality surface** — worst failure mode is name-spam or silence, both detectable.
- **H=4 is honest** — one shared constant (name-spam ceiling) and per-consumer radius/cooldown tunables live in the consumers, not the substrate itself.

Main risk is the OSRS gravity well analogue: consumers slipping numeric fields onto `NamedLandmark` over time. Scope discipline #3 is the type-level guardrail. `NamedLandmark` has no generic `effects: Vec<Modifier>` field.

## Integration with existing systems
- **Consumes** `NarrativeTier::Significant` events from `src/systems/narrative.rs`. Treats them as naming candidates.
- **Produces** `NamedLandmark` entries that six consumer subsystems look up for display and narrative emission.
- **Interacts with** `src/systems/narrative.rs` — naming events emit their own `Nature` or `Significant`-tier narrative line ("The colony will remember this as {name}").
- **Does not touch** GOAP, scoring, or influence maps. Pure narrative substrate.

## Dependencies
- **No hard dependencies.** Can land standalone as Phase 1 with paths as first consumer.
- **Benefits from** — one consumer shipping in the same PR proves the registration contract. Paths is the canonical first consumer because path wear is naturally spatially-anchored (matching the event-proximity matcher's strongest shape).
- **Enables** mythic-texture canary pass for five other stubs (`paths.md`, `crafting.md` Phase 3 and Phase 4, `ruin-clearings.md` Phase 3, `the-calling.md` retrofit, `monuments.md`). Without this substrate each of those rolls its own generator.

## Open scope questions
1. **Where does the code live?** `src/systems/naming.rs` + `src/resources/named_landmark_registry.rs` is the natural home. Matcher runs after `narrative.rs` in the schedule. Confirm no ordering conflict with `system_activation` tracking.
2. **Does the substrate ship with paths (Phase 1) or with the first consumer that actually needs it?** Current lean: paths, because that's the first stub that hits the naming problem. But crafting Phase 3 is also a valid first consumer and will ship first given `crafting.md` rank.
3. **Monuments are the sixth consumer.** Resolved 2026-04-22: `monuments.md` is its own stub, self-naming at build-completion via kind-specific templates (Burial Mound → "Where {cat} Sleeps", etc.). See the "Monument self-naming" note in the event-kind template table above.
4. **Retrofit cost for the-Calling's existing compound-name generator.** The current generator works; the retrofit converts it into the naming substrate's fallback branch. Estimated <50 LOC but needs verification.

## Tuning Notes
_Record observations and adjustments here during iteration._
