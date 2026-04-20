# Systems backlog — ranked

> **What this is:** a one-time Tynan-style triage of the unimplemented
> system stubs in `docs/systems/`. Each stub is scored on five axes
> (V × F × R × C × H) and placed in a cost/value bucket. Produced by
> applying the `rank-sim-idea` skill retrospectively to the 2026-04-20
> backlog.
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
|  1 | Recreation & Grooming | 5 | 5 | 3 | 3 | 4 | 900 | Worthwhile |
|  2 | The Calling | 5 | 5 | 3 | 3 | 3 | 675 | Worthwhile |
|  3 | Body Zones — anatomical injury model *(perception slice folded → §5.2)* | 4 | 5 | 3 | 3 | 3 | 540 | Worthwhile |
|  4 | Raids | 4 | 5 | 3 | 2 | 2 | 240 | Earn slot |
|  5 | Log Analytics Dashboard | 1 | 3 | 5 | 3 | 5 | 225 | Earn slot |
|  6 | Mental Breaks | 4 | 4 | 2 | 3 | 2 | 192 | Earn slot |
|  7 | Strategist Coordinator | 3 | 3 | 3 | 2 | 3 | 162 | Earn slot |
|  8 | Trade & Visitors | 4 | 3 | 3 | 2 | 2 | 144 | Earn slot |
|  9 | Disease | 3 | 4 | 2 | 2 | 2 | 96 | Earn slot |
| 10 | Substances | 2 | 3 | 2 | 2 | 1 | **24** | **Defer** |

No cheap wins (>1000) remain as standalone features. The top three
fall in the "worthwhile; plan carefully" band. Everything below rank 3
needs a hypothesis + prediction per balance methodology before it
gets a slot.

## Worthwhile (300–1000) — plan carefully

### 1. Recreation & Grooming *(900)*

The §5 headliner. Directly lights the **ecological variety** canary
(grooming, play, mentoring all fire zero today). Extends the Maslow
stack with a recreation need; adds leisure actions that score off
existing mood/fondness axes. H=4 because the feedback is one-way —
grooming affects mood, but mood doesn't gate grooming-seeking in a
runaway loop. The main risk is over-scoring play and starving the
colony; first soak gates this. Benefits from §2 response curves
being available (variety-bonus is a saturation curve) but doesn't
require them.

### 2. The Calling *(675)*

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

### 3. Body Zones — anatomical injury model *(540)*

13-part anatomical injury model replacing flat `Health.current`. Scars
become identity; amputated tails reduce balance; torn ears reduce
hearing. Strong §5 alignment (burial weight, generational knowledge
via scarred elders). C=3 for the targeting + healing table refactor of
`combat.rs`. H=3 because pain feeds back into existing fear/energy
axes, but the feedback is local (injured cat acts injured) rather than
systemic. **The perception slice is already folded into §5.2
sensory-channel attenuation** (damaged nose → reduced scent reads);
the remaining work is the combat/health side.

## Earn the slot (80–300) — requires hypothesis per balance methodology

### 4. Raids *(240)*

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

### 5. Log Analytics Dashboard *(225)*

Tooling win. Zero ongoing sim tax (H=5), zero regression risk (R=5),
no canary moved (V=1). Completes `tools/narrative-editor/` with map
overlay, system activation diff, belief heatmap. The rubric places it
in "earn the slot" because V=1 caps the ceiling no matter how cheap
the H — a tool can't light a canary. Real value is force-multiplier
on *future* balance work; worth the slot if the next several
balance threads need better instrumentation.

### 6. Mental Breaks *(192)*

Mood-threshold crisis cascade (sulking → hissing fit → feral
episode) + inspiration mirror. V=4 because breaks create behavioral
cascades that fire the ecological-variety canary, but H=2 — breaks
feed back into mood, and witness-radius penalty creates a
colony-wide cascade surface. **Shadowfox comparison:** rare-event +
feedback loop + likely-needs-bespoke-canary = three tells. Not
lethal, but it will be a permanent tuning axis. Substantially cleaner
to design once §2 response curves are available (mood→break probability
is a classic logistic).

### 7. Strategist Coordinator *(162)*

The HTN-style two-layer planner above GOAP. V=3 as enabling
infrastructure (doesn't light a canary itself but unblocks the Cook
loop and the "Explore as 'no better goal'" fix per `docs/open-work.md`
#1 sub-3). F=3 because strategic goal selection can slide into
difficulty-dial framing if the goal weights are tuned reactively.
C=2 for 1.2k+ LOC of coordination rework. Gated on Kitchen loop
stability. Also benefits from §L2.10 (unified DSE surface) — strategic
directives become first-class DSEs.

### 8. Trade & Visitors *(144)*

Loners/traders/scouts/hostile visitors, reputation-driven spawn rate,
fondness-threshold recruitment. V=4 (mythic texture + courtship +
generational knowledge). F=3 because reputation scaling is the
closest thing in the backlog to a RimWorld-style director. H=2 for
feedback coupling (colony success → reputation → more visitors →
larger colony). Architecturally big — new entity type, new state,
new canary. §4 context tags (faction stance) is a prerequisite per
the refactor doc's §10.

### 9. Disease *(96)*

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

### 10. Substances *(24)*

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
