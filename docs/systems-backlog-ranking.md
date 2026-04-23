# Systems backlog — ranked

> **What this is:** a one-time Tynan-style triage of the unimplemented
> system stubs in `docs/systems/`. Each stub is scored on five axes
> (V × F × R × C × H) and placed in a cost/value bucket. Produced by
> applying the `rank-sim-idea` skill retrospectively to the 2026-04-20
> backlog, then extended 2026-04-21 with two presenter-layer ideas
> (post-death biographies, cat-conversation rendering) that aren't
> system stubs but are subject to the same rubric. Further extended
> 2026-04-22 with three split-out features from a composite
> "OSRS-style inventory + fantasy adventures" proposal — crafting
> (`crafting.md`), slot inventory (`slot-inventory.md`), and ruin
> clearings (`ruin-clearings.md`) — each priced independently so the
> trim options are legible. Further extended later 2026-04-22 with
> happy paths (4a, `paths.md`, 480), the NamedLandmark naming
> substrate (4b, `naming.md`, 640 — shared scaffolding for six
> convergent consumers), and monuments (4c, `monuments.md`, 540 —
> civic/memorial structures); concurrently the crafting entry (#6)
> was promoted from 288 → 540 when Phase 4 (decorations / place-
> making) and Phase 5 (elevated cat-craft / collective multi-season)
> were added to the crafting stub.
>
> **What this is not:** the design docs themselves. Ranking is a
> price tag on each stub; the stubs remain the source of truth for
> design intent.

## Preamble

Tynan Sylvester's observation (from *Designing Games*): ideas are not
equal-cost. Shadowfoxes are Clowder's own proof — narratively a pillar
of the mythic-texture canary, cheap to scaffold, but levying a
permanent tax (dedicated canary `ShadowFoxAmbush ≤ 5` in `CLAUDE.md`,
defense pipeline `docs/systems/shadowfox_wards.md`, perpetual tuning
slot). A rubric that prices only implementation effort would greenlight
a second shadowfox-class idea and invite the same tax again. So the
rubric separates **implementation cost (C)** from **ongoing
simulation-health tax (H)** — cheap to build and expensive to live
with are independent failure modes.

## Rubric (brief)

Five axes, 1–5 each, multiplied (range 1–3125). Full definitions in
`.claude/skills/rank-sim-idea/SKILL.md`.

- **V — Value.** Canary coverage + §5 sideways alignment
  (`project-vision.md`). 5 = directly lights a currently-zero
  continuity canary.
- **F — Fit.** Thesis concordance: honest world, no director,
  ecology-with-metaphysical-weight, emergent complexity.
- **R — Risk** *(higher = safer)*. Probability it works on first ship
  without regressing canaries.
- **C — Implementation Cost** *(higher = cheaper)*. One-time LOC +
  coupling. 5 = ≲300 LOC, one file; 1 = multi-cluster, gated on A1.
- **H — Simulation-Health Tax** *(higher = lower tax)*. Ongoing tuning
  cycles, canaries forced, scoring-surface interaction. 1 =
  shadowfox-class; 5 = zero ongoing tax.

**Buckets**

- **>1000** — cheap win. Pick up next session.
- **300–1000** — worthwhile; plan carefully.
- **80–300** — expensive but valuable; earn the slot. Requires
  hypothesis + prediction per `CLAUDE.md` §"Balance Methodology".
- **<80** — defer unless a dependency forces the hand, or reframe
  to raise a low axis.

## Shadowfox calibration anchor

| Axis | Score | Reason |
|------|-------|--------|
| V | 5 | Mythic-texture canary pillar; fog-bound corruption-born predator is the thesis in one creature. |
| F | 5 | Ecology with metaphysical weight in one line. |
| R | 2 | Fear/ward/flee interaction with scoring destabilized mortality; required a bespoke canary to detect misbehavior. |
| C | 3 | Significant — ambush + corruption-spawn pipeline contributed meaningfully to `wildlife.rs`. |
| H | 1 | Dedicated canary, defense pipeline stub, perpetual tuning slot. Maximum ongoing tax. |

**Score: 5 × 5 × 2 × 3 × 1 = 150.** Shipped anyway because the narrative
V=5 earned it, but the lived cost matches the score. Every ranking
below reads against this anchor. Anything scoring `V=5, F=5, H≤2` is
asking to become the next shadowfox.

## Stub deleted (implemented, doc was stale)

- **`docs/systems/world-gen.md`** — verified shipped in
  `src/world_gen/{terrain,colony,special_tiles}.rs`. Design's SCALE,
  15-tile spacing, corruption-colony distance, colony validity
  criteria all live in code. Deleted as part of this exercise;
  `docs/wiki/systems.md` regenerated.

## Folded into the AI substrate refactor

Four stubs were initially ranked here but **their core mechanic lives
inside the A-cluster refactor** (`docs/systems/ai-substrate-refactor.md`),
not outside it. Scoring them as standalone features double-counts —
once at the refactor's cost, again as their own line item. They're
listed here as pointers only; their cost is the refactor's cost.

| Stub | Subsumed by | Note |
|------|-------------|------|
| **`sleep-that-makes-sense.md`** — Phase 1 (day-phase bonus) | §2 Response curves + §7 Momentum | Day-phase bonus is literally "add a logistic response curve to the sleep consideration" — the canonical IAUS test case. Phase 1's four `sleep_{dawn,day,dusk,night}_bonus` constants collapse into curve shape parameters. Phases 2–4 are separate downstream work (prey GOAP migration etc.) but hang off the same substrate. |
| **`environmental-quality.md`** — ambient mood pressure | §2 Response curves + §5 Influence-map substrate | **The canonical first non-scent influence-map layer.** Terrain comfort + building adjacency + squalor/corpse penalty is inherently spatial, works as a base map with slow decay (`decay_per_update: 0.9`), and is read by mood scoring through a simple response curve. Updated §10 of the refactor doc to list it here. |
| **`body-zones.md`** — perception-effects slice only | §5.2 Sensory channel attenuation | Damaged nose → reduced scent reads; punctured ear → reduced hearing reads. Already explicitly named in §5.2 of the refactor spec. **The anatomical-injury-replacing-flat-health part stays standalone** — that's a combat refactor, not a substrate change. See row below in the main ranking table. |
| **`sensory.md`** — Phases 2–5 | §5 Influence-map substrate + §5.2 Sensory channel attenuation | The whole "migrate ~20 call sites to unified `detect()` with environmental multipliers" program is the §5.2 work. Phase 1 (scaffolding) is already shipped; the refactor is where Phases 2–5 actually happen. |

Net effect on the ranking: **the two former cheap-win entries (Sleep
Phase 1 at 1875, Environmental Quality at 1280) are no longer standalone
line items.** The standalone backlog below has no >1000 entries,
which is itself a finding — *the cheap wins were structurally part of
the refactor all along*. This reinforces the refactor's priority
rather than competing with it.

## Ranking (standalone features, post-fold)

Column headers use the rubric shorthand: **V** Value · **F** Fit · **R**
Risk *(higher = safer)* · **C** Implementation Cost *(higher = cheaper)* ·
**H** Simulation-Health Tax *(higher = lower tax)*.

| Rank | System | V<br>Value | F<br>Fit | R<br>Risk | C<br>Cost | H<br>Health | Score | Bucket |
|------|--------|:----------:|:--------:|:---------:|:---------:|:-----------:|------:|--------|
|  1 | Post-death biographies (Claude API presenter) † | 4 | 4 | 4 | 4 | 4 | **1024** | Cheap win |
|  2 | Recreation & Grooming | 5 | 5 | 3 | 3 | 4 | 900 | Worthwhile |
|  3 | The Calling | 5 | 5 | 3 | 3 | 3 | 675 | Worthwhile |
|  4 | Body Zones — anatomical injury model *(perception slice folded → §5.2)* | 4 | 5 | 3 | 3 | 3 | 540 | Worthwhile |
|  5 | Anatomical slot inventory ‡ | 2 | 3 | 4 | 4 | 4 | 384 | Worthwhile |
|  6 | Crafting — items, recipes, stations ‡ | 4 | 4 | 3 | 2 | 3 | 288 | Earn slot |
|  7 | Raids | 4 | 5 | 3 | 2 | 2 | 240 | Earn slot |
|  8 | Log Analytics Dashboard | 1 | 3 | 5 | 3 | 5 | 225 | Earn slot |
|  9 | Cat-conversation rendering (Haiku presenter over C3) † | 4 | 3 | 3 | 2 | 3 | 216 | Earn slot |
| 10 | Mental Breaks | 4 | 4 | 2 | 3 | 2 | 192 | Earn slot |
| 11 | Strategist Coordinator | 3 | 3 | 3 | 2 | 3 | 162 | Earn slot |
| 12 | Trade & Visitors | 4 | 3 | 3 | 2 | 2 | 144 | Earn slot |
| 13 | Ruin clearings — corruption nodes ‡ | 4 | 4 | 2 | 2 | 2 | 128 | Earn slot |
| 14 | Disease | 3 | 4 | 2 | 2 | 2 | 96 | Earn slot |
| 15 | Substances | 2 | 3 | 2 | 2 | 1 | **24** | **Defer** |

† Presenter-layer ideas (2026-04-21 extension). Not system stubs —
LLM-rendering surfaces over finalized sim artifacts. Priced on the
same rubric; see the "Presenter layer" subsection below for the
discipline these scores assume.

‡ Split-out features from the 2026-04-22 `/rank-sim-idea` triage of
a composite "OSRS-style inventory + fantasy adventures" proposal.
Crafting (rank 6) is the anchor and ships first; slot-inventory
(rank 5) scores highest in the split but **does not ship standalone**
— it's gated on a wearable producer (crafting Phase 3, the Calling,
or trade); ruin-clearings (rank 13) is the scope-cut variant of the
dungeon idea. See their stubs for the constraints that hold the
scores in place.

One cheap win (>1000) now exists as a standalone presenter-layer
feature. Among the stub-level items the worthwhile band now holds
four entries (Recreation, Calling, Body Zones, Anatomical slot
inventory). Slot inventory's rank-5 placement reflects its standalone
score; its ship-order is bound to a wearable producer, not to the
raw number. Everything at rank 6 and below needs a hypothesis +
prediction per balance methodology before it gets a slot.

## Presenter layer

The two presenter-layer entries (ranks 1 and 7) share an architectural
contract that shapes their scores. The `CLAUDE.md` Design Principles
rule *"No behavior trees, no LLMs"* defends authorial intent: sim
behavior must be auditable back to math the user wrote. Per the
2026-04-21 clarification, the rule targets **LLMs inside the
`ground-truth → math → outcome` chain**, not LLMs in general. An LLM
that reads *finalized* sim artifacts and writes *sidecar* files the
sim never reads back is compatible with the rule. The contract:

1. **Input:** LLM reads only finalized artifacts (`logs/events.jsonl`,
   snapshots, C3 belief-facet records) — never live ECS state.
2. **Output:** LLM writes only to sidecars (`logs/biographies/`,
   `logs/conversations/`) — never to `logs/events.jsonl`, never to
   `logs/narrative.jsonl` (the verification-tier file).
3. **Audit:** `rm -rf` the sidecars, re-run `just soak 42`, diff
   `events.jsonl` + verification-tier `narrative.jsonl` against the
   pre-delete run. Byte-identical = presenter by construction. Put
   this assertion in CI.

Both ranks assume this discipline holds. The **R=3** on
cat-conversation rendering reflects the risk of the contract slipping
(LLM output feeds back into belief state via some future shortcut);
the **H=3** reflects a soft aesthetic tax — LLM prose and sim math
can diverge, and narratively-satisfying LLM prose can subtly drown out
the math's quieter truths. Biographies score R=4/H=4 because there's
no downstream math to destabilize (a dead cat stays dead).

## Cheap win (>1000)

### 1. Post-death biographies via Claude API *(1024)*

On `CatDied` (or post-hoc over `logs/events.jsonl`), extract the cat's
lifelog, feed it to a prebuilt Claude API skill, emit a biographical
prose render into `logs/biographies/<cat>.md`. Lights the
**mythic-texture** continuity canary (≥1 named event per sim year,
currently zero from live-sim sources) plus §5 **preservation** and
**generational knowledge** axes. C=4 because it's mostly Python-side
(API client, prompt, biography writer); Rust side is an audit of
`cat_id` denormalization across `logs/events.jsonl`.

Structural tells: none fire. No sim scoring read/write, no
probabilistic cascade, no feedback loop (the biography is terminal —
player reads it, sim doesn't). Ongoing tax is external-service
upkeep (prompt drift, API-version management, cost monitoring),
bounded and visible. Not shadowfox-class.

**Key design choices before code:** (a) live-on-death vs. post-hoc
log-processing tool (post-hoc is strictly easier); (b) sidecar vs.
narrative.jsonl tier — **strongly prefer sidecar** to preserve
verification-tier reproducibility; (c) which lifelog events feed the
prompt (more isn't better — cost + quality are both sensitive).

Landing this entry establishes the **presenter-layer infrastructure**
(per-cat event indexing, Claude API client, sidecar routing, CI
audit) that rank 9 (cat-conversation rendering) reuses when C3 ships.
Do this one first.

## Worthwhile (300–1000) — plan carefully

### 2. Recreation & Grooming *(900)*

The §5 headliner. Directly lights the **ecological variety** canary
(grooming, play, mentoring all fire zero today). Extends the Maslow
stack with a recreation need; adds leisure actions that score off
existing mood/fondness axes. H=4 because the feedback is one-way —
grooming affects mood, but mood doesn't gate grooming-seeking in a
runaway loop. The main risk is over-scoring play and starving the
colony; first soak gates this. Benefits from §2 response curves
being available (variety-bonus is a saturation curve) but doesn't
require them.

### 3. The Calling *(675)*

The **mythic-texture** canary pillar — ≥1 named event per sim year,
currently zero from live-sim sources. Rare trigger (0.05%/tick gated
on affinity + mood + spirituality), four-phase trance producing
persistent colony artifacts (Ward / Remedy / Totem / Talisman). F=5
because the thesis explicitly names Calling as ecological-magical-
realist *par excellence*. H=3 because the rare-event cascade is
bounded (one cat at a time, short duration) and uses existing magic +
corruption axes rather than introducing new ones. §7 momentum (from
the refactor) helps the trance commit cleanly, but the Calling is
primarily a new feature not substrate work.

### 4. Body Zones — anatomical injury model *(540)*

13-part anatomical injury model replacing flat `Health.current`. Scars
become identity; amputated tails reduce balance; torn ears reduce
hearing. Strong §5 alignment (burial weight, generational knowledge
via scarred elders). C=3 for the targeting + healing table refactor of
`combat.rs`. H=3 because pain feeds back into existing fear/energy
axes, but the feedback is local (injured cat acts injured) rather than
systemic. **The perception slice is already folded into §5.2
sensory-channel attenuation** (damaged nose → reduced scent reads);
the remaining work is the combat/health side.

### 4a. Happy paths — usage-worn trails + NamedLandmark substrate *(480)* [added 2026-04-22]

Cats concentrate movement between high-utility destinations; repeated
traversal compresses terrain into speed-boosted trails; prey learn to
avoid them; and Significant-tier events near a fresh trail derive its
name ("The Last Trace of {cat}") rather than a random generator.
Worn enough, paths become a **civilizational marker** — the colony
writing its own behavioral history into the world as physical grain.

V=4 because event-driven naming directly lights the mythic-texture
canary, and the feature advances the preservation / generational-
knowledge axes of §5 as a first-order physical artefact (not a
scoring tweak). F=5 because the thesis — ecology with metaphysical
weight, emergent complexity, honest-world-no-director — lands
one-to-one on "the world shows what the cats did." R=3 because path
weight integration touches `pathfinding.rs::step_toward` (every
agent's motion layer) and adds a new fear input to prey scoring; A/B
verification per Balance Methodology is required. C=4 because path
wear stamps an existing `InfluenceMap` (`src/systems/influence_map.rs`
§5.6.9 — `(Channel, Faction)`-keyed registry; "14th map" is a
registration, not a schema change) rather than introducing a new
resource; the expensive substrate is already scaffolded. H=2 because
the self-reinforcing feedback loop (wear→speed→preference→wear) and
the new prey-fear input force bespoke canaries (formation,
anti-monopoly, named-landmark, name-spam) and contribute tunables to
future hunt-balance threads — but decisive shadowfox-differentiators
(no mortality-spike failure mode, continuous not Poisson, canaries are
formation-quality not survival) keep H=2 not H=1.

**Leverage point — the NamedLandmark substrate** is a separate
precursor scaffolding (rank 4b, `naming.md`) shared with five other
consumers (`crafting.md` Phase 3, `crafting.md` Phase 4,
`ruin-clearings.md` Phase 3, `the-calling.md`, `monuments.md`).
Resolved 2026-04-22: substrate lives in its own stub as precursor.
Named-path output soft-depends on 4b landing; can ship with
neutral-fallback generator if needed.

**Shadowfox watch:** shares self-reinforcing feedback loop and new
prey-fear input with shadowfoxes; five scope disciplines (wear
decays; speed boost ≤1.25× non-stacking; prey avoidance is
proportional; ≤6 named segments per sim-year; paths don't gate hunt
scoring) are the brakes against running away. Violations re-trigger
ranking.

### 4b. NamedLandmark substrate — event-anchored naming *(640)* [added 2026-04-22]

Shared registry + event-proximity matcher + event-keyed name
templates. Six convergent consumers (`paths.md`, `crafting.md` Phase
3, `crafting.md` Phase 4, `ruin-clearings.md` Phase 3,
`the-calling.md`, `monuments.md`) all need to produce named entities
that outlive their makers; without a shared substrate each rolls its
own name generator. The canonical scaffolding pattern (V=2, ship-
order gated) applies — this mirrors `slot-inventory.md`'s structure.

V=2 because the substrate has no in-world effect until a consumer
ships; V rises to effective-4 once one consumer registers, to
effective-5 at three or more consumers (cross-stub mythic-texture
cascade). F=5 because the thesis — ≥1 named event per sim-year from
live-sim sources — is *exactly* what this substrate enables. R=4
because it's isolated scaffolding with no scoring interaction.
C=4 because the matcher + registry + template table + six consumer
registration contracts is ~500–800 LOC. H=4 because the shared
name-spam ceiling (≤6 named landmarks per sim-year across all
consumers) is a single constant to tune; per-consumer radius /
cooldown tunables live in the consumer stubs, not here.

**Ship-order discipline:** precursor to paths (4a) or to any other
consumer that reaches the naming slot first. Do not ship stand-
alone — V=2 pre-consumer. Paths is the canonical first consumer
because path wear is spatially-anchored (matches the proximity
matcher's strongest shape). Monument self-naming is a distinct
zero-proximity flow, documented in `naming.md`.

**Shadowfox watch:** minimal (no feedback loop, no scoring, no
mortality). Main risk is the OSRS gravity-well analogue — consumers
slipping numeric fields onto `NamedLandmark` over time. Type-level
guardrail (no generic `effects: Vec<Modifier>` field on the
registry type) is the brake. See `docs/systems/naming.md`.

### 4c. Monuments — civic & memorial structures *(540)* [added 2026-04-22]

Colonies raise physical structures that anchor narrative across
generations in response to Significant-tier events — burial mounds,
coming-of-age stones, defender's memorials, pact circles, founding
stones. Monuments are **built events**: the act of building is the
narrative, the built object is the artefact. Five kinds at launch
(load-bearing cap).

V=4 because monuments double-light the **burial axis** of the
ecological-variety canary (currently ~0 firings/year) and the
**mythic-texture** canary while advancing **generational knowledge**
— the strongest burial vehicle in the backlog. V=5 is in play once
telemetry confirms burial-axis zero-baseline. F=5 because the thesis
in one line: honest ecology writing itself into the landscape with
metaphysical weight. R=3 because the declaration → gather → raise
pipeline touches coordination, naming, and pathfinding-at-the-margin
— bounded but multi-axis. C=3 because the coordination-directive +
tile-structure + naming-registration + rendering stack adds ~800
LOC but rides on three existing substrates (`coordination.rs`,
`fate.rs`, `naming.md` once shipped). H=3 because monuments are
additive-permanent with no self-reinforcement loop (failure modes
are monument-spam and monument-silence, both detectable).

**Ship-order discipline:** hard-depends on 4b (naming substrate) and
A1 IAUS refactor (multi-cat GOAP coordination, same gate as #13
ruin-clearings). Phase 1 (Burial Mounds only) is the pilot — smallest
scope, well-bounded triggering event. Phase 3 adds Founding Stones
which need colony-founding to be a legible event (may require new
work in `coordination.rs`).

**Shadowfox watch:** no adversarial feedback loop, no new mortality
category. Main risk is the "monumentalism" gravity-well — pressure
to add kinds over time creeping the launch-5 toward 15 and diluting
each. Scope rule 1 (hard cap at 5 kinds at launch; additions are a
re-triage trigger) is the brake. See `docs/systems/monuments.md`.

### 5. Anatomical slot inventory ‡ *(384)*

Refactor of the flat `Inventory { slots: Vec<ItemSlot> }`
(`src/components/magic.rs:242`) into anatomy-indexed wearable slots
(drawn from `body-zones.md`'s 13-part enumeration) plus a stackable
consumable pouch; crafted bags (from `crafting.md`) expand pouch
capacity. V=2 because the feature scores well in isolation but has
no in-world effect until a wearable producer ships — it's
scaffolding. F=3 because the OSRS gravity well is real; the stub
holds F=3 via a type-level guardrail (no numeric modifier fields on
wearables). H=4 because the ongoing tax is bounded to one or two
capacity-related constants.

**Ship-order discipline:** do not ship standalone. Gated on at
least one wearable producer — crafting Phase 3 (rank 6),
the-calling.md, or trade.md. The score ranks it in the
"worthwhile" band but the stub explicitly defers shipping until
a producer exists. See `docs/systems/slot-inventory.md`.

**Shadowfox watch:** the only shadowfox risk is the OSRS-misbuild
trap. Type guardrail (no numeric fields on `WearableItem`) is the
primary mitigation; violating it re-opens the ranking (F→2, H→2,
composite falls to ~96).

## Earn the slot (80–300) — requires hypothesis per balance methodology

### 6. Crafting — items, recipes, stations ‡ *(540, promoted from 288 on 2026-04-22)*

> **Bucket move:** 2026-04-22 expansion of the stub to include
> Phase 4 (Domestic refinement / folk-craft decorations) and Phase 5
> (Elevated cat-craft / collective multi-season) promoted this entry
> from "earn the slot" (80–300) into "worthwhile; plan carefully"
> (300–1000). Numbering retained at 6 for stable cross-references;
> score 540 places it adjacent to body-zones (540) and monuments
> (540) despite appearing in the lower bucket section below. Reader
> note: the bucket headers are advisory; the score is the source of
> truth.

General-purpose material economy — recipes + stations + craft
actions — generalizing the existing narrow patterns (remedy prep at
Workshop, ward-setting from thornbriar) into a unified substrate.
**§5-first recipe catalog** is the load-bearing design constraint:
every recipe targets grooming / play / courtship / burial /
preservation / generational knowledge. Combat gear is excluded;
drift from the constraint re-opens the ranking (F→2, H→2, composite
falls to ~96).

The 2026-04-22 expansion added two new phases:
- **Phase 4 — Domestic refinement (folk-craft tier).** Place-anchored
  decorations: reed mats, tallow lamps, scent censers, carved combs,
  wall-hangings, nesting inlays. Effects live on the *tile*, not the
  cat — a rug warms the hearth, not the cat carrying the rug.
- **Phase 5 — Elevated cat-craft (collective, multi-season).**
  Generational Tapestries, Shrine-Cairns, Bone-Lattice Lanterns,
  Pigment-Deepened Textiles, Multi-Cat Nesting Alcoves, Kitten-
  Cradle Baskets. Gated by three conditions (all required): colony-
  age ≥3 sim-years, material-scarcity (ruins + exploration inputs),
  and skill-via-aspirations (new mastery arcs in `aspirations.rs`).
  Explicit not-DF guardrail: collective or cumulative, never
  individual-rare-strike. `the-calling.md` owns that niche.

V=5 (promoted from 4) because Phase 4 decorations are the strongest
mythic-texture vehicle in the backlog — heritable named objects that
literally outlive their makers, reinforced by generational use. F=4
holds under the catalog constraint + place-anchored discipline. R=3
unchanged. C=3 (promoted from 2) because Phase 4 adds ~400 LOC
(placed-object rendering, env-quality interaction, naming
registration for a new LandmarkKind) and Phase 5 adds skill-arc
integration into `aspirations.rs`. H=3 holds — scoring-axis coupling
is real but failure modes stay legible.

**Ship-order note:** crafting is the anchor of the 2026-04-22
three-way split. Ships **before** ranks 5 and 13 — it de-risks
slot-inventory (first producer of wearables in Phase 3) and
ruin-clearings (loot has a consumer once Phase 1 preservation
recipes land). Phased delivery with per-phase hypothesis:

- **Phase 1** — food preservation (drying/smoking racks).
  Strongest canary linkage (starvation + winter buffer).
- **Phase 2** — §5 behavioral tools (brush, play bundle, gift).
  Targets ecological-variety canary.
- **Phase 3** — identity & mentorship objects. Triggers
  slot-inventory (rank 5) to ship. Registers with 4b (naming
  substrate) as a consumer.
- **Phase 4** — place-anchored decorations. Second naming-substrate
  consumer; first tile-level environmental-quality producer. Soft-
  depends on 4b.
- **Phase 5** — elevated cat-craft. Hard-depends on `aspirations.rs`
  mastery-arc extension. Long-horizon.

See `docs/systems/crafting.md` and `docs/open-work.md` #16.

### 7. Raids *(240)*

Organized pack assaults (3–5 foxes, rat swarms, shadow-fox
incursions) scaled to colony threat score. V=4 for mythic texture +
burial; F=5 because raid-as-ecological-response is thesis-perfect. But
H=2 — three tells fired (reads/writes fear, probabilistic cascade,
feedback with building-pressure) and the design practically requires
a bespoke raid-death canary. **Shadowfox comparison:** structurally
similar to shadowfoxes (rare-event predators with fear-axis
interaction). Budget the tuning slot before picking up. Benefits
from §5 influence maps (raid pathing as an influence-map read) and
§4 faction tags.

### 8. Log Analytics Dashboard *(225)*

Tooling win. Zero ongoing sim tax (H=5), zero regression risk (R=5),
no canary moved (V=1). Completes `tools/narrative-editor/` with map
overlay, system activation diff, belief heatmap. The rubric places it
in "earn the slot" because V=1 caps the ceiling no matter how cheap
the H — a tool can't light a canary. Real value is force-multiplier
on *future* balance work; worth the slot if the next several
balance threads need better instrumentation.

### 9. Cat-conversation rendering via Haiku (presenter over C3) *(216)*

Once **C3** (subjective knowledge / ToT mental models,
`docs/open-work.md` §7) lands, C3 will emit deterministic
facet-exchange records whenever co-located cats exchange beliefs —
per Ryan, Mateas, Wardrip-Fruin 2016 *"Characters who speak their
minds"* (AIIDE 2016). This entry is a **Haiku-powered presenter** over
those records: take a facet-exchange event, render the prose of the
conversation the cats would have had, write to
`logs/conversations/<tick>.md`. Belief math stays in C3; LLM output
never feeds back.

Scoring assumes presenter discipline (see "Presenter layer"
subsection above). **Under the in-loop framing** (LLM drives
conversation → conversation drives belief → belief drives scoring)
the score is **4** — shadowfox-worse, defer. The 4 × 3 × 3 × 2 × 3 =
216 score only applies to the strict presenter variant.

**Dependencies:** gated on **A1** (IAUS refactor), **A3**
(context-tag uniformity), and **C3** — all in `docs/open-work.md`
§§5 and §7. Also gated on rank 1 (biographies) landing — reuses the
presenter-layer infrastructure (per-cat event indexing, Claude API
client, sidecar routing, CI audit). Without rank 1 the C score drops
a notch.

**Canary hit:** none directly. C3's canaries (gossip-as-practice,
belief candidate revision, transference) are lit by C3's math, not
by this presenter.

**Required hypothesis + prediction** (80–300 bucket per `CLAUDE.md`
Balance Methodology): *Adding presenter-rendered conversation prose
over C3's deterministic facet exchanges will not measurably alter
any canary (sim behavior is unchanged) but will measurably increase
time-to-comprehension when reading a seed-42 soak's social events.*
The sim metric prediction is a **null direction** — no shift — which
is unusual for a balance hypothesis but appropriate: this isn't a
balance change, it's a rendering change. Concordance criterion is
*null in canaries, positive in qualitative experience*.

### 10. Mental Breaks *(192)*

Mood-threshold crisis cascade (sulking → hissing fit → feral
episode) + inspiration mirror. V=4 because breaks create behavioral
cascades that fire the ecological-variety canary, but H=2 — breaks
feed back into mood, and witness-radius penalty creates a
colony-wide cascade surface. **Shadowfox comparison:** rare-event +
feedback loop + likely-needs-bespoke-canary = three tells. Not
lethal, but it will be a permanent tuning axis. Substantially cleaner
to design once §2 response curves are available (mood→break probability
is a classic logistic).

### 11. Strategist Coordinator *(162)*

The HTN-style two-layer planner above GOAP. V=3 as enabling
infrastructure (doesn't light a canary itself but unblocks the Cook
loop and the "Explore as 'no better goal'" fix per `docs/open-work.md`
#1 sub-3). F=3 because strategic goal selection can slide into
difficulty-dial framing if the goal weights are tuned reactively.
C=2 for 1.2k+ LOC of coordination rework. Gated on Kitchen loop
stability. Also benefits from §L2.10 (unified DSE surface) — strategic
directives become first-class DSEs.

### 12. Trade & Visitors *(144)*

Loners/traders/scouts/hostile visitors, reputation-driven spawn rate,
fondness-threshold recruitment. V=4 (mythic texture + courtship +
generational knowledge). F=3 because reputation scaling is the
closest thing in the backlog to a RimWorld-style director. H=2 for
feedback coupling (colony success → reputation → more visitors →
larger colony). Architecturally big — new entity type, new state,
new canary. §4 context tags (faction stance) is a prerequisite per
the refactor doc's §10.

### 13. Ruin clearings — corruption nodes ‡ *(128)*

PMD-flavored dungeons-as-corruption-nodes: uncleared ruins emit
corruption radially, the colony organizes multi-cat clearings
(paths → pushback → interior hazards), loot routes to crafting
materials + occasional Named Objects. **This is the scope-cut
variant.** The full gear-modifier variant scores 64 (defer) and is
explicitly rejected.

V=4 because it adds a new colony-scale coordinated-action mode and
contributes to mythic-texture via Named Object drops. F=4 because
dungeons-as-ecological-pressure is thesis-aligned — especially
under the 2026-04-22 clarification of `project-vision.md` §3 that
"no protagonist shield" means *named cats can die*, not *cats
aren't narrative leads* (the Stark framing). Cats can die in ruins.
R=2 because it's architecturally significant — new overworld entity
type, multi-cat GOAP coordination, real second-order mortality
effects. C=2 under the scope cut. H=2 because three of five
structural tells fire, but the dungeon-existence mechanic rides
the **existing** `magic.rs` corruption substrate rather than
inventing a new predator feedback loop — decisively lighter than
shadowfox (H=1).

**Scope discipline (violations drop score to 64):**
1. Loot is crafting material or Named Objects — never gear.
2. Corruption pushback reuses existing `magic.rs` substrate.
3. Spawn rate is ecological (seasonal corruption + distance from
   hearth), never reactive to colony threat-score.
4. Clearing difficulty is environmental, never scaled to colony
   power.

**Shadowfox comparison:** structurally lighter than shadowfoxes
despite being in the same ecological-threat family (F=4 vs 5, H=2
vs 1). Still bespoke-canary territory — a new `ClearingAttempt`
mortality cause and a `ruins_cleared_per_sim_year` footer tally
are likely required.

**Dependencies:** hard-gated on A1 IAUS refactor (multi-cat GOAP
coordination) and on **rank 6 (crafting) Phase 1** shipping
(otherwise loot has no consumer). See
`docs/systems/ruin-clearings.md` and `docs/open-work.md` #18.

### 14. Disease *(96)*

Wound infection, seasonal illness, contagion, quarantine. V=3
(supports §5 via healer role + generational knowledge but doesn't
directly light a zero canary). H=2 — three tells fired
(reads/writes fear + energy, probabilistic cascade, feedback with
coordination). **Shadowfox comparison: closest analogue in the
backlog.** Narratively rich, operationally expensive, needs a
bespoke epidemic canary. Score at 96 is above the defer threshold
but only barely; before picking up, reframe to drop the cross-
coupling with coordination (disease as ambient mood pressure rather
than coordinator-gated healer role) to pull H up to 3.

## Defer (<80)

### 15. Substances *(24)*

Catnip, valerian, corrupted variants; tolerance / dependence /
withdrawal / craving mechanics. V=2 (no canary hit; orthogonal
polish), F=3 (tension risk — addiction as difficulty valve), H=1
(three tells fired and the personality-addiction coupling creates a
permanent balance surface). Sub-shadowfox score. Reframe required
before reconsidering: strip the dependence/tolerance loop and make
substances a one-shot mood modifier (drop H to 3), or lean into
named-object-producing rituals (catnip ceremonies → +V via mythic
texture). As designed today: defer.

## Dependency graph

**Via the substrate refactor (folded items):**

- **Sleep Phase 1, Environmental Quality, Body Zones (perception),
  Sensory Phases 2–5** — all gated on the A-cluster refactor landing.
  Their individual stubs remain valid as design intent; their
  implementation happens inside the refactor.

**Standalone features:**

- **Recreation & Grooming** — independent. Benefits from §2 response
  curves but doesn't require them.
- **The Calling** — soft-dependent on spirituality being a personality
  dimension (check or add). Benefits from §7 momentum.
- **Body Zones (anatomical)** — independent; ships alongside any
  combat tuning.
- **Raids, Disease, Trade & Visitors** — all benefit from Strategist
  Coordinator landing first (gives them a place to hang strategic
  goals like "defend", "heal", "host").
- **Mental Breaks** — independent but compounds poorly with
  Substances if both ship; stagger them.
- **Crafting (rank 6)** — independent. Benefits from A1 IAUS refactor
  but does not hard-block. Anchor of the 2026-04-22 split; **ships
  before ranks 5 and 13**.
- **Anatomical slot inventory (rank 5)** — hard-gated on a wearable
  producer. Candidates in thesis-fit order: crafting Phase 3 (rank 6),
  the Calling (rank 3), trade & visitors (rank 12). Score reflects
  standalone value; ship-order is bound to a producer.
- **Ruin clearings (rank 13)** — hard-gated on A1 IAUS refactor
  **and** crafting Phase 1 (rank 6) shipping. Scope discipline in
  the stub is load-bearing — drift re-opens the ranking.
- **Anything touching scoring (most of the above)** — renegotiates
  C and H once the **refactor** lands. Revisit this ranking at that
  milestone.

## Revisit triggers

Regenerate this ranking when any of:

1. **A1 IAUS refactor lands** — response curves + multiplicative
   composition change C scores for every scoring-adjacent system.
   Mental Breaks, Disease, Recreation all pick up cleaner designs.
2. **A §5 influence-map layer ships.** Environmental Quality is the
   proposed first layer; once it's live, future spatial features
   (corruption-pushback, ward reach, raid pathing) have a template.
3. **Any system here ships.** Update the memory substrate per the
   skill's memory schema (`type=pattern`, tag `clowder ongoing-tax`)
   with observed vs. predicted H. Future rankings get sharper.

## Methodology note

Scores were produced by applying the rubric in
`.claude/skills/rank-sim-idea/SKILL.md`. H scoring used source (a)
**structural tells** for every row (the memory substrate is empty at
first triage; no priors to query). Source (c) **balance-doc grep**
informed Disease and Raids (both zero iterations in `docs/balance/`
despite being cascade-heavy designs — unpriced in lived balance work,
which raises uncertainty rather than lowering H).

**2026-04-21 extension.** Ranks 1 and 9 (the presenter-layer entries)
were added prospectively — triaged before any design doc or code
exists. Their scores are under the strict presenter contract
documented in the "Presenter layer" subsection above; relaxing that
contract collapses both scores (cat conversations in particular drop
from 216 to 4 if the LLM touches the belief-update loop). If the
presenter contract ever needs to be loosened, re-rank these rows
before proceeding.

**2026-04-22 extension.** Ranks 5, 6, and 13 (split-out features from
a composite "OSRS-style inventory + fantasy adventures" proposal)
were added after the proposal triaged poorly as a unit (V=1/F=1,
score ~2) but contained separable thesis-aligned pieces. Splitting
surfaces the actual trim options: **crafting (rank 6) is the
anchor**, slot-inventory (rank 5) is scaffolding gated on a
producer, ruin-clearings (rank 13) is the scope-cut dungeon. Each
score is held in place by a load-bearing constraint in its stub
(§5-first recipe catalog for crafting; no-numeric-fields type
guardrail for slot-inventory; four scope disciplines for
ruin-clearings). Violations of any constraint re-open the relevant
ranking.

The fold into the refactor was driven by a separate signal:
`docs/systems/ai-substrate-refactor.md` §10 already enumerates which
stubs the refactor subsumes versus merely unblocks. Items listed as
"Aspirational → Built" under a specific substrate section are
subsumed; items listed as "unblocked by" remain standalone.

## Related indexes

- `docs/systems/ai-substrate-refactor.md` — §10 is the canonical
  fold-vs-unblock map; this ranking defers to it.
- `docs/open-work.md` — tactical in-flight queue; many rankings here
  cross-reference entries there.
- `docs/wiki/systems.md` — Built/Partial/Aspirational status table
  (auto-generated; refresh via `scripts/generate_wiki.py`).
- `docs/systems/project-vision.md` — the thesis the V and F axes
  quote from (§5 sideways-broadening, continuity canaries).
