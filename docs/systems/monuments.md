# Monuments — Civic & Memorial Structures

## Purpose
Colonies leave physical structures that anchor narrative across generations — not decoration (`crafting.md` Phase 4), not transient ritual markers (small shrine-cairns in `crafting.md` Phase 5), but deliberate *civic* or *memorial* construction meant to persist as landmarks the next generation inherits without question. A burial mound over an honored elder's remains; a coming-of-age stone raised by a generation of kittens reaching adulthood together; an ambush-memorial at the edge of territory where a cat died defending kin; a pact-circle where a fated-pair bond was named. Monuments are **built events**: the *act of building* is the narrative, the *built object* is the artefact, and both count toward mythic texture.

Monuments differ from `crafting.md` Phase 5 in three ways:
1. **Scale and permanence.** Monuments are tile-scale structures that persist until explicit destruction (ruin collapse, deliberate tear-down). They are not consumable, not reclaimable for material, not heritable-as-owned — they belong to the site and the colony.
2. **Build requires a triggering event.** Unlike a Phase 5 tapestry that accumulates passively across seasons, a monument is *raised in response to* a Significant-tier narrative event (death, coming-of-age, pact, banishment, founding). No event, no monument. The event is the monument's soul; the physical object is the body.
3. **Build is a coordinated colony action.** Multi-cat work-pressure (`coordination.rs`) directs construction; cats opt in to contribute labor the way they opt into hunting or foraging. A monument is never one cat's project.

Score: **V=4 F=5 R=3 C=3 H=3 = 540** — "worthwhile; plan carefully" per `systems-backlog-ranking.md`. V=4 because monuments double-light the **mythic-texture** and **generational knowledge** canaries and add a burial-axis firing (all three are currently under-fired); V=5 is in play if telemetry confirms burial/coming-of-age events currently fire zero monuments-worth of visible persistence. F=5 because the thesis in one line: honest ecology writing itself into the landscape with metaphysical weight. R=3 because the mechanism touches coordination, naming, and environmental-quality — bounded but multi-axis. C=3 because the coordination-directive + tile-structure + naming-registration + rendering stack adds ~800 LOC but rides on three existing substrates. H=3 because monuments are additive-permanent (failure mode is monument-spam or monument-silence, both detectable) with no self-reinforcement loop.

## Thesis alignment
- **Honest world, no director.** Monuments are raised in response to events the world generated. No authored monument placement, no tutorial structures, no colony-founding "you get a monument free." First monument requires a first qualifying event.
- **Ecology with metaphysical weight.** A burial mound is simultaneously a physical feature of the terrain (cats navigate around it, new births sometimes happen on its slope) and a narrative anchor ("where Ashfur sleeps"). The two are not separable.
- **Generational knowledge.** Cats born after a monument was raised inherit it as given — they did not experience its triggering event directly but the monument transmits the event's weight into colony-shared knowledge (`colony_knowledge.rs`).
- **Sideways-broadening (§5).** Directly advances **burial** (memorial monuments over remains), **preservation** (physical structures outliving their makers for generations), and **generational knowledge** (second- and third-generation cats inheriting monument-anchored narrative). The burial axis currently fires near-zero in live sim; monuments are its strongest vehicle.
- **Emergent complexity.** A territory edge with a defender's memorial becomes a site future defenders are more likely to approach for patrol (courage bonus near the memorial), which attracts future ambushes there, which produces more monuments, which reshapes the colony's relationship to that edge over decades. The DF-beer-cats-puke-depression shape.

## Monument kinds — triggering event → structure
Each monument kind is anchored to a Significant-tier event and a site. The kinds are intentionally narrow at launch; additions are a re-triage trigger so monument-spam doesn't creep in.

| Kind | Triggering event | Site choice | Structure | Narrative anchor |
|---|---|---|---|---|
| Burial Mound | Cat death, honored (not banished, not shadow-fox kill in the wilds) | Cat's death tile if reachable, else nearest hearth-adjacent burial site | Raised earth + placed stones; 1–2 tiles | "Where {cat} sleeps" |
| Coming-of-Age Stone | Multiple kittens reach adulthood in the same season (≥2) | A tile in the colony-center visible from hearth | Standing stone with carved-claw marks; 1 tile | "{season-year}'s Crossing" |
| Defender's Memorial | Cat dies in territorial defense (combat, non-shadow-fox ambush) at the edge | Death tile, edge-of-territory | Cairn + placed feathers/bones; 1 tile | "{cat} Held the {compass-bearing} Edge" |
| Pact Circle | Fated-pair bond named, or generational-mentorship explicit transfer | Site of the bond-naming event | Ring of stones at a flat tile; 2–3 tiles | "Where {cat-a} and {cat-b} Bound" or "Where {mentor} Passed the {skill} to {apprentice}" |
| Founding Stone | First winter survived by a newly-founded colony, or a colony-splitting event's new-colony side surviving its first winter | Hearth-tile of the founding / new colony | Large flat stone with placed pigment-marks; 1 tile | "This Colony Began With {founder(s)}" |

Monument kinds *not* on this list don't build. Monument-spam is the failure mode; a world cluttered with monuments reads as cheap, the way a world with every path named reads as cheap. Five kinds is load-bearing at launch.

## Build mechanic
A monument is built in three phases, all multi-cat:

1. **Declaration.** A qualifying event fires → `MonumentDeclaration` message. A coordinator-directive (`coordination.rs`) is posted proposing a build. The declaration names the site, the kind, and the eligible contributor set (cats present at the event, or — for post-event monuments like Coming-of-Age Stones — cats who were part of the cohort).
2. **Gathering.** Contributing cats gather materials (stones, earth, pigment, feathers, bones per kind) and transport them to the site. This is a GOAP chain (`BuildMonument` task) scored similarly to `BuildWard` but open to more cats simultaneously.
3. **Raising.** Once materials are present, a raising action — multi-cat, performed together — places the structure on the site. The raising emits a **Significant**-tier narrative event ("The colony raises {kind} for {subject}") which is itself naming-eligible via `naming.md` (so a Burial Mound for Ashfur becomes "Where Ashfur Sleeps" rather than a generic name).

The multi-cat requirement is load-bearing for the not-DF discipline: a monument is never one cat's project. A minimum of 2 contributors is enforced; many events will qualify more (all adult cats for a Coming-of-Age Stone, the defender's kin-circle for a Memorial).

## Initial parameters
| Parameter | Initial Value | Rationale |
|-----------|---------------|-----------|
| Declaration window after event | 100 ticks | Event energy fades; build must start within this window or the declaration expires silently |
| Minimum contributors | 2 | Enforces multi-cat; prevents solo monument-building |
| Build-pressure priority | Equal to Ward-building | Monuments aren't survival-urgent; they're significance-urgent |
| Gather radius for materials | 20 tiles around site | Bounded foraging; long-reach gathering is a sign the site was poorly chosen |
| Raising action duration | 50 ticks | Substantial; colony pauses other optional work but not survival work |
| Max monuments per sim-year | 4 | Monument-spam guardrail; enforced as a declaration-rate limit, not a hard block on qualifying events |
| Monument decay | None absent explicit destruction | Permanence is the point |
| Cross-generational narrative weight | 0.8× the originating event's weight | Future cats reference the monument with high-but-discounted weight vs. first-hand witnesses |

## Staging
- **Phase 1 — Burial Mounds only.** Smallest-scope monument (single-kind, well-bounded triggering event). Validates the declaration → gather → raise pipeline and the naming-event emission. Required hypothesis: *cats dying on-colony trigger burial-mound construction by nearby kin ⇒ on seed-42 `--duration 900`, ≥1 burial mound per sim-year from non-wild deaths; `burial` axis of ecological-variety canary rises from ~0 to ≥1 fire per sim-year; `deaths_by_cause` distribution unchanged (no mortality-shift side effect).*
- **Phase 2 — Coming-of-Age Stones + Defender's Memorials.** Two more kinds; adds cross-edge spatial behavior. Required hypothesis: *qualifying cohort-adulthoods and edge-deaths produce Coming-of-Age Stones and Defender's Memorials at measurable rates; generational-continuity canary rises as monuments-per-colony-lifespan metric increases across sim-years.*
- **Phase 3 — Pact Circles + Founding Stones.** Completes the five-kind launch set. Requires fated-pair resolution from `fate.rs` (Built) and colony-founding/splitting to be a legible event (Partial — check `coordination.rs`). Required hypothesis: *with all five monument kinds live, monument-per-sim-year distribution is 1–3 average, never exceeds the 4-per-year cap, and contributes ≥2 additional named landmarks per sim-year via `naming.md`.*

## Scope discipline (load-bearing — keeps H=3)
1. **Monument-spam guardrail is hard.** ≤4 monuments per sim-year across all kinds. Guardrail enforced at the declaration layer, not the build layer (if a 5th event qualifies in a sim-year, it's silently dropped — not queued for next year).
2. **All monuments are multi-cat.** No solo monument-building. A declaration that cannot recruit ≥2 contributors within the declaration window expires silently.
3. **No authored / player-directed monument placement.** Monuments are entirely event-driven; player cannot command construction or select sites.
4. **No numeric modifiers on the cat passing a monument.** A cat visiting Ashfur's grave may experience a mood nudge (grief recall, meditative pause — handled by existing `mood.rs` / `memory.rs`), but the monument itself carries no stat-buff field. Keeps the OSRS-gravity-well type guardrail.
5. **No Strange-Moods-analogue triggering.** Construction is rational and opt-in (coordinated colony work), not a mood-strike on a single cat. `the-calling.md` owns rare-individual-mood-strike craft; monuments are deliberately distinct.

Drift from any of these re-triggers ranking.

## NamedLandmark substrate
Monuments are the sixth convergent consumer of the shared naming substrate documented in `naming.md`. Registration contract:
- **Ripeness criterion:** raising action completes → monument is ripe for naming.
- **Proximity radius:** 0 tiles — the raising event *is* the naming event, so no search radius needed. The declaration's subject (`{cat}` for burial, `{cat-a}` and `{cat-b}` for pact, etc.) feeds directly into the name template.
- **Cooldown:** none; every raised monument names immediately.
- **Fallback:** kind-anchored neutral ("A Burial Mound") if the declaration's subject metadata is corrupted — shouldn't happen in normal flow.

Full consumer list (six total): `paths.md`, `crafting.md` Phase 3, `crafting.md` Phase 4, `ruin-clearings.md` Phase 3, `the-calling.md`, `monuments.md` (this file).

## Required hypothesis per Balance Methodology
Per-phase hypotheses above. Umbrella:

> *Qualifying Significant-tier events trigger multi-cat monument construction; completed monuments anchor generational narrative and raise mythic-texture plus burial-axis firing rates without introducing mortality or survival drift.* ⇒ *On seed-42 `--duration 1800` (30-min deep-soak) with all five monument kinds live: 1–3 monuments raised per sim-year; monument-per-year ceiling of 4 never exceeded; burial axis fires ≥1 per sim-year (up from ~0); mythic-texture count gains ≥2 named landmarks per sim-year; `deaths_by_cause` distribution within ±10% of pre-monument baseline (no mortality side effects); generational-continuity canary holds.*

## Canaries to ship in the same PR
1. **Burial-axis canary** (ecological-variety extension). Burial-axis firings per sim-year ≥1 from live sim events, up from baseline ~0. Detects burial-mound silence.
2. **Monument-rate canary.** 1–4 monuments raised per sim-year on seed-42 `--duration 1800`. Detects both silence (<1) and spam (>4).
3. **Cross-kind diversity canary.** After all five kinds live, at least three distinct monument kinds are represented per `--duration 1800` soak. Detects single-kind monopolization (e.g. burials fire fine but Coming-of-Age Stones never build).
4. **Mortality-drift canary.** `deaths_by_cause` distribution stays within ±10% of pre-monument baseline. Detects monument-building interfering with survival work (kin cats starving while gathering stones for a burial mound is a failure, not a feature).

## Shadowfox comparison
Structurally lighter than shadowfoxes:
- **No feedback loop in the adversarial direction.** Monuments don't spawn threats. The "defender's memorial attracts future ambushes there" interaction is honest ecology (patrol density raises ambush opportunity) but runs through existing `wildlife.rs` ambush mechanics, not a new loop.
- **No new mortality category.** Monuments aren't a death-cause.
- **Canaries are formation-quality + survival-drift, not new-mortality-type.** Easier to observe.
- **H=3 is honest** — monument-spam / silence / diversity / mortality-drift are four new metrics, but all four sit within existing telemetry shapes (event-log counts, `deaths_by_cause` comparison).

Main risk is the "monumentalism" gravity well — over-time, the pressure to add more monument kinds creeps the launch-set of 5 toward 15, diluting each kind's meaning. Scope discipline rule 1 (hard cap at 5 kinds at launch; additions are a re-triage trigger) is the brake.

## Integration with existing systems
- **Declares via** `coordination.rs` build-pressure directives (same pattern as Ward construction).
- **Builds via** a new `BuildMonument` GOAP task chain (`task_chains.rs` / `disposition.rs`), mirroring `BuildWard` but open to multiple contributors simultaneously.
- **Registers with** `naming.md` as `LandmarkKind::Monument`.
- **Reads / produces** `NarrativeTier::Significant` events (reads the triggering event; emits the raising event).
- **Persists in** the tile map as terrain features (similar to existing Ward entities); influences `pathfinding.rs` in a minor way (monuments are not obstacles but may bias routing at the margin — TBD in Phase 1 observation).
- **Contributes to** `colony_knowledge.rs` — new colony members know about extant monuments without needing to have witnessed their events.
- **Cross-references** `crafting.md` Phase 5 shrine-cairns. Small shrine-cairns stay in crafting; larger civic/memorial monuments live here. The boundary: shrine-cairns are raised passively at sacred sites; monuments require a triggering narrative event.

## Dependencies
- **Hard-gated on `naming.md`.** Without the shared substrate, monuments would need to duplicate name generation — inconsistent with the five-other-consumer convergence.
- **Hard-gated on A1 IAUS refactor** for multi-cat GOAP coordination on a shared goal (same gate as `ruin-clearings.md`).
- **Benefits from** `coordination.rs` build-pressure directives (Built).
- **Benefits from** `fate.rs` (Built) for Pact Circle triggering events.
- **Phase 3 needs** colony-founding / colony-splitting as a legible event, which may require work in `coordination.rs` or a new message — check at Phase 3 entry.
- **Soft-depends on** `environmental-quality.md` for the monument-tile ambient effect (visiting a grave nudges mood); can ship with a minimal stub interface.

## Open scope questions
1. **Destruction mechanism.** Can monuments be destroyed? A burial mound scattered by a fox is unlikely; a memorial at a lost edge if the colony retreats feels plausible. My lean: no destruction at launch. Add only if retrospective play surfaces a need.
2. **Prospective / pre-event monuments?** A cat elder *asks* for a burial mound at a chosen site before dying — a foreshadowing mechanic. Cute but director-shaped. Cut at launch.
3. **Monument visiting as a leisure action.** Should there be a `VisitMonument` action, or does existing proximity-based mood/memory handling suffice? My lean: rely on existing handling; only add a dedicated action if observation shows monuments are mechanically inert after raising.
4. **Cross-colony monuments.** If colony-splitting is a thing, do daughter colonies inherit the parent's monuments? My lean: yes, the physical tile stays; but a new Founding Stone at the daughter's hearth is its own Phase 3 monument.

## Tuning Notes
_Record observations and adjustments here during iteration._
