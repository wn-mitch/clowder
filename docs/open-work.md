# Open work

> **What this is:** the cross-thread index of open work. New sessions should
> consult this, `docs/wiki/systems.md`, and `docs/balance/*.md` before starting
> fresh. See `CLAUDE.md` §"Long-horizon coordination" for the request-time
> checklist and maintenance rules.

Living backlog of known-but-not-scoped work. Each entry is a pointer, not a
plan — the plan is written when the work is picked up.

---

## Pre-existing issues (not from this session)

### Test harness drift

**Status:** pre-existing.

`cargo test` fails three integration tests with a Bevy "Resource does not
exist" panic:
- `cats_eat_when_hungry`
- `simulation_is_deterministic`
- `simulation_runs_1000_ticks_without_panic`

Reverting the 2026-04-19 balance change does not fix them — a system was
added to `build_schedule()` (in `src/main.rs` or `SimulationPlugin::build()`)
whose required Resource isn't inserted in `tests/integration.rs::setup_world`.

**`just check` (cargo check + clippy) passes green.** Only `cargo test` is
broken.

**To pick up:** enable a debug feature (or patch a local build of bevy_ecs)
to surface the actual system name and missing-Resource type, then add the
insertion to `setup_world`.

---

## Follow-on plans surfaced but not scoped

> **Cross-reference:** [`docs/systems-backlog-ranking.md`](systems-backlog-ranking.md)
> prices every unimplemented-system stub on the V×F×R×C×H rubric (see
> `.claude/skills/rank-sim-idea/SKILL.md`). The top of the standalone
> backlog is now Recreation & Grooming (900) and The Calling (675);
> Sleep Phase 1 and Environmental Quality were folded into the
> A-cluster refactor (see `docs/systems/ai-substrate-refactor.md` §10),
> which is where the former cheap wins actually get built.

### 1. Explore dominance over targeted leisure

**Why it matters:** Explore claims 44–47% of all action-time in a seed-42
soak. Groom sits at 0.4–0.5%, Mentor / Caretake / Cook at exactly 0. The
user's "narrative leisure isn't happening" observation is real but it's a
target-availability problem, not a survival-lock problem.

**Root cause:** Explore has the loosest gate (just `unexplored_nearby > 0`).
Other leisure actions require specific targets (`has_social_target`,
apprentice, kitten, Kitchen, mate) that aren't consistently present.
Choosing Explore moves cats toward unexplored periphery → away from other
cats → `has_social_target` turns false → Explore wins again. Dispersion
feedback loop.

**Three directions agreed in the 2026-04-19 session** (ordered by blast
radius):

1. **Broaden `social_target_range`** (`src/resources/sim_constants.rs:1672`)
   from 10 → ~20–30 Manhattan tiles. Current 10 is combat-adjacent range,
   not cat-socializing range. In a 120×90 map with 8 cats, 10 is too
   tight for clustered-at-infrastructure moments to register.
   - **Iter 1 (range=25) REJECTED** — 2026-04-19. Mating (−67%), Kittens
     (−75%), bonds (−44%) regressed.
   - **Iter 2 DIAGNOSTIC (instrumented)** — 2026-04-20. Full score
     distributions (commit `290a5d9`) reframe the mechanism: Mate is
     gate-starved (0% of snapshots), never competed with Socialize in the
     scoring layer. The true regression is **bond attenuation** — wider
     range spreads Socialize interactions across more partners; each pair
     builds fondness/familiarity slower; Partners/Mates bond progression
     stalls; `has_eligible_mate` never opens. Treatment had 0 matings and
     0 kittens vs baseline 4/5.
   - **Sub-task 1 fundamentally compromised** — lowering/raising
     `social_target_range` can't fix the dispersion loop without bond
     attenuation. See `docs/balance/social-target-range.report.md` §
     Proposed iteration 3 for alternatives: (a) pair-stickiness in
     social-target selection, (b) pursue sub-task 2 (Explore saturation)
     which doesn't touch social dynamics.
2. **Saturation curve on Explore's weight.** Real cats don't explore
   indefinitely — past a local familiarity threshold it becomes
   indistinguishable from Wander. Current formula multiplies by
   `unexplored_nearby` linearly; at 50% locally explored, Explore still
   scores 0.5× its raw weight (enough to beat Wander's 0.08 floor).
   Target: sharp decay once local exploration fraction crosses ~0.7.
   Touch points: `src/ai/scoring.rs:302–309` and the radius/threshold
   args to `ExplorationMap::unexplored_fraction_nearby`.
3. **Strategist coordinator task board**
   (`docs/systems/strategist-coordinator.md`). The structural fix: a
   two-layer planner (strategic goal → tactical action) that gives cats
   a colony-level task board to align behavior against. Explore becomes
   "I have no better goal" rather than "I have no target." The doc itself
   gates this on the Cook loop firing end-to-end first — which is partly
   unblocked by the eat-threshold balance change above.

   **Cross-reference:** this is **C4** in the deliberation-layer cluster
   (see #7 below). It sits above BDI intentions (C1), social practices
   (C2), and belief modeling (C3) — HTN-style hierarchical planning. The
   existing `docs/systems/strategist-coordinator.md` design stub remains
   the primary design document; the cluster context adds the
   architectural framing for when it gets picked up.

**Ordering:** (1) and (2) are small scoring-layer tunes with seed-42
A/B verification. (3) is real engineering and wants its own design pass.
Do them in order; (1) and (2) should make the strategist's value visible
before it's scoped.

### 2. Hunt-approach pipeline failures

**Why it matters:** 1,774 "lost prey during approach" failures in the
treatment soak vs. 9 "no scent found" search timeouts. Refines the
findability hypothesis: cats locate prey via scent fine, then lose it
during stalk/approach.

**Candidate levers:**
- Stalk speed (currently 1.0 tiles/tick, previously tuned up from 0.5)
- Approach speed (currently 3 tiles/tick)
- Prey detection-of-cat during approach phase (`try_detect_cat` in
  `src/systems/prey.rs`)
- Stall-out conditions — "stuck while stalking" fires 257–341× per soak,
  which is a separate failure mode from "lost"

**Catches-per-week trajectory** (seed-42, 17 weeks): week-0 boom (66),
weeks 1–3 settle (22/9/18), weeks 4+ oscillate 3–15. Not a flatline — the
local depletion → recovery cycle works. The issue is conversion: 1,981
Hunt plans created, ~11% convert to kills.

### 3. Mentor score magnitude (from iter-2 diagnostic, 2026-04-20)

**Why it matters:** "Mentoring fires ≥1× per soak" is a continuity
canary currently failing. The iter-2 diagnostic for social_target_range
(commit `290a5d9`) showed Mentor's gate opens 43.7% of baseline
snapshots — gate availability is **not** the blocker. The blocker is
raw score magnitude: Mentor mean score 0.126 vs Sleep 0.802, Eat 0.725,
Hunt 0.669. Mentor cannot win scoring even when its gate is open.

**Touch point:** `src/ai/scoring.rs:597–605` + constants
`mentor_warmth_diligence_scale: 0.5` and `mentor_ambition_bonus: 0.1` in
`src/resources/sim_constants.rs`. For comparison
`socialize_sociability_scale = 2.0` — Mentor is 4× smaller in scale
despite stricter gates.

**Hypothesis:** Raising `mentor_warmth_diligence_scale` to ~1.5–2.0 lifts
Mentor score into competitive range, producing ≥1 Mentor firing per
seed-42 soak (continuity canary). Secondary effect: the already-consumed
apprentice-skill-growth path at `src/systems/goap.rs:2672–2743` becomes
load-bearing for the first time, so skill progression for low-skill cats
accelerates. Orthogonal to social_target_range work.

**Bounds/risks:** Mentor competes in the utility layer with Socialize;
over-tuning could re-trigger the iter-1 mating regression via a
different pathway. Measure MatingOccurred / KittenBorn as mandatory
canaries on any Mentor tuning.

### 4. Magic hard-gated at scoring

**`src/ai/scoring.rs:483`** — `PracticeMagic` only scored if
`ctx.magic_affinity > 0.3 && ctx.magic_skill > 0.2`. ~60% of cats fall
below the affinity threshold and never see magic as a scoring option.

Contradicts `docs/systems/project-vision.md`'s framing of magic as an
ecological phenomenon — a kitten wandering into a FairyRing should feel
the pull whether or not it has "magic training." The misfire system
(`check_misfire`, `src/systems/magic.rs:919–940`) is the intended risk
gate for unskilled attempts; the scoring-level gate makes it unreachable.

Also touches `src/systems/disposition.rs:1675–1676, 1717–1718, 1748`
(redundant downstream gates that become dead once the scoring gate eases).

---

### 5. Scoring substrate refactor cluster [A — FOUNDATIONAL]

**Why this is a cluster:** entries A1–A4 are the structural refactor of
`src/ai/scoring.rs` from hand-authored per-action linear formulas to an
Infinite-Axis-Utility-System–shaped architecture (Mark 2009). A1 is the
foundational change; A2 is the buy-vs-build investigation; A3 and A4 are
natural companions that should be bundled with A1 to avoid re-churning
the scoring layer twice.

**Gating:** all of cluster B (influence maps), cluster C (deliberation),
and cluster E (world-gen history) assume A1 is done — they add new
axes or read shared slow-state that the current additive-composition
scoring layer can't cleanly consume.

**See also:** `docs/reading-list.md` cross-refs A1–A4 to Dave Mark's
GDC talks, *Behavioral Mathematics for Game AI*, and Game AI Pro IAUS
chapters. Plan reasoning in
`/Users/will.mitchell/.claude/plans/this-project-has-grown-jolly-wilkes.md`.

#### A1. IAUS refactor — response curves + multiplicative composition [TOP PRIORITY]

**Why it matters:** Linear response curves misrepresent biological
response to stimuli. Real cats don't get 1.5× as motivated to eat when
hunger goes from 0.6 to 0.9 — they get ~5×. "Hangry" is a threshold
phenomenon best modeled by a logistic curve with inflection near
0.7–0.8. Same for sleep deprivation (panic threshold), fear
(flee-or-fight switchover), loneliness (acute onset), cold exposure.
Current `scoring.rs` bakes linearity into every axis via `_scale`
constants, which forces either "over-reactive in normal range" or
"sluggish at critical" — can't achieve both with linear math. Curves
are the thing that lets the system react realistically across the full
range of stimulus intensity.

**Current state:** `src/ai/scoring.rs:177–660` is an axis-based utility
system with always-linear response curves. `Needs::level_suppression`
in `src/components/physical.rs:249` is the one non-linear element
(Maslow-ordered multiplicative gate). Inputs are assembled in
`ScoringContext` (`src/ai/scoring.rs:27–110`) — already a
"pre-evaluated considerations" bag in Mark's sense; the missing piece
is the consideration abstraction itself.

**IAUS supplies three things this codebase doesn't have:**
1. **Response curves per axis** — linear, polynomial, logistic, logit,
   piecewise, with shape parameters (slope, exponent, h-shift, v-shift)
   rather than a single `_scale` constant.
2. **Multiplicative composition across *all* axes** (not just Maslow),
   with a compensation factor so multi-axis actions aren't penalized
   for thoroughness. Any axis ≈ 0 ⇒ action ≈ 0.
3. **Named, reusable considerations** — e.g. a single `HungerUrgency`
   consideration used in Eat, Sleep, Hunt, Forage rather than the
   `(1.0 - ctx.needs.hunger) * X` pattern rewritten four times.

**Pros:**
- Per-axis decomposition makes the `last_scores` instrumentation (see
  `docs/balance/scoring-layer-second-order.md` framing #2) natively
  diagnostic — every score traces to labeled, curved axes.
- Adding axes (`bond_strength_with_target`, `strategist_goal_match`,
  influence-map lookups) becomes a one-line addition. Prerequisite for
  most other clusters.
- Multiplicative composition prevents "one high axis dominates"
  pathologies; pairs naturally with context-tag gating (A3).

**Cons / risks:**
- ~700 lines of `scoring.rs` + `sim_constants.rs` reshape (constants
  become curve shape params + axis IDs).
- High regression risk: every formula needs to preserve output (or at
  least action *ordering* in common states). A/B against seed-42 deep-
  soak with tight tolerances; golden-master snapshots pre-refactor.
- Maslow `level_suppression` is a genuinely non-trivial element to
  preserve — IAUS doesn't natively model hierarchies, so keep Maslow
  as a separate pre-gate above the axis-multiplication layer, or lift
  it into a dedicated "hierarchical axis" concept.

**Touch points:**
- `src/ai/scoring.rs:177–660` — all per-action formulas
- `src/components/physical.rs:249` — `Needs::level_suppression`
- `src/resources/sim_constants.rs` — `ScoringConstants` reshape

**Preparation reading:**
- *Already watched (prompted this thread):* "Winding Road Ahead:
  Designing Utility AI with Curvature" (Dave Mark, GDC 2018,
  <https://www.youtube.com/watch?v=TCf1GdRrerw>); "Building a Better
  Centaur: AI at Massive Scale" (Dave Mark, GDC 2015,
  <https://archive.org/details/GDC2015Mark>)
- Dave Mark, *Behavioral Mathematics for Game AI* (Cengage 2009) —
  canonical IAUS text; ch. 9–12 cover response curves and
  multi-consideration composition
- "Embracing the Dark Art of Mathematical Modeling in AI" (Dave Mark,
  GDC 2013, on GDC Vault) — deeper curve treatment than Winding Road
- *Game AI Pro* IAUS chapters (Mark's chapters in vols. 1 and 2, free
  PDFs at <http://www.gameaipro.com/>) — canonical curve primitives
  and formulas
- Ian Millington, *AI for Games* (3rd ed.) ch. on decision-making /
  utility — reference for invariant preservation during refactor

**Exit criterion:** seed-42 deep-soak with refactored scoring produces
identical canary results; per-axis diagnostic output lands in
`logs/events.jsonl`; at least one previously-linear axis (Hunger) is
shipped with a logistic curve and the effect is measured (predicted:
Starvation ticks more aggressively near critical, idles more gently in
normal range).

**Dependency callouts:**
- A2 runs first — may supply the substrate
- A3 and A4 are natural companions; bundle with A1
- B1, C1, C2, C3, E1 are **gated on A1**

---

#### A2. Investigate big-brain as IAUS migration vehicle

**Why it matters:** Before committing to an in-house IAUS refactor
(A1), verify whether `zkat/big-brain` (Bevy utility AI crate) supplies
enough of the needed machinery to serve as a substrate or partial
migration target.

**Current state:** Not evaluated. Known points: big-brain provides
`Scorer`/`Action`/`Picker` abstractions and composition primitives
(`WinningScorer`, `ProductOfScorers`, `MeasuredScorer`), but it's not
full IAUS out of the box — no canonical curve library, no compensation
factor, no target-scoring primitive. Bevy version compatibility needs
checking (last known: Bevy 0.16; we're on 0.18).

**Touch points:**
- `src/ai/scoring.rs` — current Scorer equivalent
- `src/systems/goap.rs` — planner layer big-brain doesn't have

**Preparation reading:**
- big-brain README + crates.io page:
  <https://github.com/zkat/big-brain>
- big-brain API docs: <https://docs.rs/big-brain/latest/big_brain/>
- big-brain examples directory — the `thirst` and `farming_sim`
  examples show idiomatic Scorer/Action composition
- Bevy 0.18 migration notes (check big-brain's CHANGELOG and Bevy's
  migration guide for 0.16 → 0.18 deltas)

**Exit criterion:** decision memo — adopt / borrow / ignore. If
"borrow," list concrete primitives worth reimplementing (e.g. the
`ProductOfScorers` compensation logic). If "adopt," confirm Bevy 0.18
compatibility and sketch migration order.

---

#### A3. Context-tag uniformity refactor

**Why it matters:** Context tags (Mark) are a uniform way to filter
which decisions and targets are eligible. Clowder already uses the
pattern, but inconsistently: some tags are Bevy ECS components
(`Coordinator`, `Adult`, `Injured`, `Pregnant` — queryable,
first-class), some are booleans in `ScoringContext` (`has_social_target`,
`can_hunt`, `on_corrupted_tile`), some are inline `if` expressions in
scoring. Three different dialects for the same pattern.

Bevy ECS is *natively* a declarative entity-tagging system —
components *are* tags. Committing fully to component-as-tag aligns
Clowder with Mark's context-tag model **and** with idiomatic Bevy
simultaneously. These aren't two refactors; they're the same refactor.

**Current state:**
- `ScoringContext.has_threat_nearby` / `has_mentoring_target` / etc.
  could become ECS-side marker components set by spatial-query systems
  and then read by the scoring system via `Query<With<ThreatNearby>>`.
- Action entry guards (`if ctx.can_hunt { ... }`) become filter
  predicates on declarative tag sets.

**Touch points:**
- `src/ai/scoring.rs` ScoringContext struct + every action block's
  entry guard
- `src/components/` — new marker components where warranted
- Systems that currently populate `ScoringContext` booleans — convert
  to systems that insert/remove marker components

**Preparation reading:**
- Dave Mark, "Architecture Tricks: Managing Behaviors in Time, Space,
  and Depth" (GDC 2012, with Kevin Dill, GDC Vault) — where Mark
  formalizes context tags as filters for DSE relevance
- "Embracing the Dark Art of Mathematical Modeling in AI" (GDC 2013)
  — context-tag coverage alongside curves
- Bevy official docs: "Components, Bundles, and Tags" + marker
  component patterns
- *Game AI Pro* chapters on tag-based reasoner architecture

**Exit criterion:** `ScoringContext` shrinks to scalar state only;
boolean flags replaced by ECS-side markers; at least one action
migrates to a pure-tag-filter entry guard as proof-of-pattern.

**Dependency:** best done *concurrently with* A1 so `scoring.rs` is
touched once, not twice.

---

#### A4. Target selection as inner optimization

**Why it matters:** Mark's framework treats target-taking actions
(Socialize, Mate, Mentor, Caretake, potentially Attack) as double
scoring: iterate candidate targets, score each, pick the best, use
that best score as the action's score. Clowder uses `has_X_target:
bool` — existence, not quality. The iter-1 `social_target_range`
regression is a direct symptom: widening the range added strangers
without picking high-bonded partners first. Wider net, thinner bonds,
broken Mate supply chain.

**Current state:** Target existence is precomputed; quality isn't
scored. Fixing this is probably the single highest-leverage
consideration-framework change — it directly addresses
`docs/balance/scoring-layer-second-order.md` without needing BDI or
Versu layers.

**Touch points:**
- `src/ai/scoring.rs` Socialize (~262), Mate (~611), Mentor (~597),
  Caretake (~650) blocks
- Whatever currently computes `has_social_target` and
  `has_mentoring_target` — those become target-ranking routines

**Preparation reading:**
- *Already watched:* "Building a Better Centaur" (GDC 2015) —
  explicit target-scoring treatment; re-watch the 20–35 min mark if
  details are foggy
- *Game AI Pro* IAUS chapters — the "target-taking actions" section
  gives the canonical double-scoring algorithm in pseudocode
- Dave Mark, *Behavioral Mathematics for Game AI* ch. 13 (target
  selection) — longest-form treatment; covers tie-breaking and
  filtering by tag before scoring

**Exit criterion:** best-bonded partner is measurably preferred for
Socialize/Mate/Mentor; iter-1 `social_target_range` can be re-attempted
without the bond-attenuation regression.

**Dependency:** natural companion to A1 (new "target quality" axis
reads best-target score); bundle with A1 to avoid churning Socialize
twice.

#### A5. Substrate instrumentation — Curvature-at-every-layer traces

**Why it matters:** After A1–A4 land, the substrate is a 3-layer
Forrester stock-and-flow system (L1 influence maps → L2 DSEs → L3
selection). Today's `CatSnapshot.last_scores` captures only L2's
*output*; L1 samples, per-consideration contributions, post-modifier
deltas, and L3 softmax probabilities are all invisible. `CLAUDE.md`'s
Balance Methodology (hypothesis → prediction → observation →
concordance) collapses to "change it and see what happens" without
instrumentation that exposes input distributions and per-layer
transforms. A1–A4 ship much harder to verify without A5.

**Design:** focal-cat replay — one designated cat emits full
layer-by-layer records every tick to a sidecar
`logs/trace-<focal>.jsonl`; all other cats retain today's snapshot
cadence. Records are joinable on `(tick, cat)` so
`scripts/replay_frame.py --tick N --cat NAME` can reconstruct a full
decision frame top-to-bottom. Default focal cat is user preference
(Simba on seed 42); `--focal-cat NAME` overrides. Headless-only
emission via a `FocalTraceTarget` resource that the interactive build
doesn't insert.

**Touch points:**
- `src/resources/event_log.rs` — sidecar emitter or new EventKind
  variants
- `src/ai/scoring.rs` (and A1's L2 replacement) — per-consideration
  + per-modifier emission hook behind `FocalTraceTarget`
- `src/systems/sensing.rs` + B1's influence-map module — lazy L1
  sample emission (only when an L2 consideration reads the map)
- `src/systems/goap.rs` + selection sites — L3 selection emission
- `src/main.rs` — `--focal-cat` flag plumbing, `FocalTraceTarget`
  resource insertion in the headless runner only
- `scripts/replay_frame.py` — new Python tool for frame decomposition

**Specification:** `docs/systems/ai-substrate-refactor.md` §11.

**Exit criterion:** `just soak 42 --focal-cat Simba` produces
`logs/trace-Simba.jsonl` whose line-1 header matches `events.jsonl`'s
`commit_hash`; `scripts/replay_frame.py --tick N --cat Simba`
reconstructs L1+L2+L3 for one tick; before/after a known-good curve
change (e.g. `Eat.hunger` Logistic midpoint 0.75 → 0.65) the replay
shows matching `consideration.input` distributions, diverging
`consideration.score` distributions, and a concordant `final_score`
shift.

**Dependency:** lands *with* A1–A4, not after. Delaying makes the
refactor harder to verify under `CLAUDE.md`'s balance rule. Bundled
into cluster A as A5.

---

### 6. Shared spatial slow-state cluster [B]

**Why this is a cluster:** scoring layer currently lacks uniform access
to shared, spatially-varying state. The scent map in `wind.rs` +
`sensing.rs` is a one-off; corruption, wards, predator danger, prey
opportunity, and social attraction are each implemented differently
or recomputed per query. Influence maps (Mark, Dahlberg) are the
canonical form of this abstraction.

#### B1. Generalize influence maps

**Why it matters:** Influence maps are the spatial form of
"consideration inputs shared across actions" — exactly what
`docs/balance/scoring-layer-second-order.md` framing #1 identifies as
missing. Generalizing to a uniform system would (a) standardize how
spatial considerations are consumed by scoring, (b) give
pair-stickiness a natural home (social attraction field pulls bonded
cats together), (c) give the strategist-coordinator a spatial
substrate.

**Current state:** One-off implementations per concern; no unified
abstraction. `wind.rs` + `sensing.rs` handles scent only.

**Touch points:**
- `src/systems/wind.rs` + `sensing.rs` — existing scent map to
  generalize
- `src/systems/magic.rs` — corruption field, ward field
- `src/systems/prey.rs` — prey density (already sort of an influence
  map)
- Possibly new: `src/systems/influence_maps.rs`

**Preparation reading:**
- **"Modular Tactical Influence Maps"** — Dave Mark, *Game AI Pro 2*
  ch. 30, free PDF at
  <http://www.gameaipro.com/GameAIPro2/GameAIPro2_Chapter30_Modular_Tactical_Influence_Maps.pdf>
  — THE definitive written reference; read first
- "Lay of the Land: Smarter AI Through Influence Maps" (Dave Mark,
  GDC 2014, GDC Vault) — the original pure-influence-maps talk
- "Spatial Knowledge Representation through Modular Scalable
  Influence Maps" (Dave Mark, GDC 2018, GDC Vault) — most recent
  full treatment, best on implementation details
- *Already watched:* "Building a Better Centaur" (GDC 2015) — fusion
  of utility AI + influence maps at scale; the architectural move
  this task implements
- Nick Mercer Unity reference implementation:
  <https://github.com/NickMercer/InfluenceMap>

**Exit criterion:** at least two distinct layers (scent + corruption,
or scent + social attraction) share one abstraction; scoring layer
reads influence-map values as native axis inputs (gated on A1).

**Dependency:** gated on A1 for clean consumption by scoring; can
proceed in parallel with cluster C.

---

### 7. Deliberation-layer cluster [C]

**Why this is a cluster:** C1–C4 all sit *above* the per-tick scoring
layer and add forms of persistence, commitment, and structure that
scoring alone cannot produce. Each addresses a specific gap named in
`docs/balance/scoring-layer-second-order.md`. All are gated on A1
because they add new scoring axes or consume slow-state the current
additive layer can't cleanly express.

#### C1. BDI-style intention persistence

**Why it matters:** `docs/balance/scoring-layer-second-order.md` framing
#1 is a direct restatement of the BDI (Belief-Desire-Intention) thesis
(Rao & Georgeff 1991): *intentions are commitments that persist across
deliberation cycles*. Clowder's GOAP has *partial* intention persistence
(once a plan is picked, steps execute sequentially), but at the scoring
layer there is no commitment — each tick re-scores from scratch.
Result: flipper behavior near equal scores, and social/skill
accumulation supply chains that can't survive the per-tick churn.

**Current state:** Scoring is stateless per tick. GOAP adds per-plan
continuity but not per-goal continuity (a goal is re-deliberated every
time a plan completes or fails).

**Proposed approach:** add a lightweight intention layer between
scoring and GOAP — a per-cat `CurrentIntention` component carrying
a goal + commitment strength + expiry, scored with a momentum bonus
during deliberation. New plans inherit the intention if valid; new
deliberations only override if the alternative's margin exceeds the
commitment strength.

**Touch points:**
- New: `src/components/intention.rs` (or similar)
- `src/systems/goap.rs` — read intentions when picking plans
- `src/ai/scoring.rs` — momentum bonus as an axis (needs A1 for clean
  addition)

**Preparation reading:**
- Rao & Georgeff (1991), "Modeling Rational Agents within a
  BDI-Architecture" — KR 1991; canonical paper; Google Scholar PDF
  widely mirrored. Short, formal, readable.
- Michael Wooldridge, *An Introduction to MultiAgent Systems* (2nd
  ed., Wiley 2009) ch. 4 "Practical Reasoning Agents" — textbook BDI
  treatment with examples
- Jeff Orkin, "Three States and a Plan: The AI of F.E.A.R." (GDC
  2006) — free at <https://alumni.media.mit.edu/~jorkin/goap.html>;
  commitment and plan persistence in practical game AI
- `docs/balance/scoring-layer-second-order.md` (in repo) — framing #1
  is the BDI thesis in Clowder terms; re-read before starting

**Exit criterion:** seed-42 deep-soak shows reduced plan-preemption
rate without increased starvation or other canary degradation.

**Dependency:** gated on A1 (momentum is a scoring axis); pairs well
with A4 (target commitment and intention commitment interact).

---

#### C2. Versu-style social practices

**Why it matters:** Evans & Short's Versu system models social
interactions as *practices* — multi-agent coordinated behaviors with
shared state (courtship, gossip, quarrel, greeting). A practice has
roles, stages, and invariants; agents *enter into* a practice
together, then its stages drive their actions until it completes.
Clowder's current social model is one-sided: one cat scores Socialize,
picks a target, emits an interaction, partner reacts. Practices would
let courtship, mentoring, and play be durable multi-stage structures
rather than per-tick score winners.

**Current state:** Social interactions are per-tick single-cat
decisions; partner cats react but don't co-commit. Bond/relationship
state accumulates but practice-level structure doesn't exist.

**Touch points:**
- `src/systems/social.rs` — currently one-sided interactions
- `src/systems/pregnancy.rs` (courtship should become a practice)
- New: `src/systems/practices.rs` or similar
- Relationship state in `src/components/social.rs`

**Preparation reading:**
- Richard Evans & Emily Short, "Versu — A Simulationist Storytelling
  System" (IEEE TCIAIG, 2014) — canonical Versu paper; defines
  practices, roles, stages, invariants
- Richard Evans, "The Sims 3" / "Imagination Engines" (GDC 2011,
  with Emily Short, GDC Vault) — shorter on-ramp; predecessor ideas
  at production scale
- Emily Short's two-part review of Ryan's dissertation
  (<https://emshort.blog/2019/05/21/curating-simulated-storyworlds-james-ryan/>)
  — Short was Evans' co-author; connects ToT practices to Versu
- James Ryan, *Curating Simulated Storyworlds* (UCSC 2018) ch. 5 on
  ToT — gossip and relationship practices; eScholarship PDF at
  <https://escholarship.org/uc/item/1340j5h2>
- James Ryan et al., Talk of the Town FDG 2015 paper (via
  <https://www.jamesryan.world/publications>) — dense, practical

**Exit criterion:** at least one practice (courtship is the natural
target — addresses the Mate supply-chain problem) implemented as a
two-agent multi-stage structure; partners co-commit rather than each
independently scoring Mate.

**Dependency:** gated on A1 (practices inform scoring axes); C1
(intentions are a natural substrate for practice participation);
potentially simpler post-A4.

---

#### C3. Subjective knowledge / belief distortion

**Why it matters:** `src/systems/colony_knowledge.rs` models knowledge
as **democratic consensus** — memories held by ≥`promotion_threshold`
cats get promoted to `ColonyKnowledge.entries`; below that, they're
per-cat memories that decay. The model is elegant but structurally
prevents a whole class of emergent narrative: *the colony wrongly
believes X because one cat saw something misleading and panic
propagated faster than ground truth corrected*.

Ryan et al. (*Game AI Pro 3* ch. 37, 2017) give the canonical
alternative — a full architecture where per-character belief is
first-class, with explicit mechanisms for origination, propagation,
reinforcement, deterioration, and termination. The chapter is specific
enough to be a blueprint; this entry adopts its vocabulary.

**Current state:** `colony_knowledge.rs` tracks aggregated memories
only; `Memory.events` is assumed faithful to the ground-truth event;
no source attribution, no mutation, no candidate-belief tracking.

**Proposed architecture (scaled to cats, not ToT's 300–500 humans):**

*Ontological structure.* Each cat maintains an ontology of linked
mental models: one per other cat it knows of, one per notable location
(den, feeding spot, fox territory, fairy ring), one per non-cat entity
that matters (specific foxes/hawks). Models link to each other —
"Silverpaw's known den" points to a Location model. Keeps storage lean
and avoids cross-model inconsistency.

*Mental model facets.* Each model is a list of **belief facets** with
type + value + evidence + strength + accuracy flag. Clowder facet
types (vastly narrower than ToT's 24 human attributes):
- For cat mental models: **lineage** (parents/offspring, links to
  other cat models), **status** (alive/dead/banished), **role**
  (coordinator/healer/hunter), **bond** (affinity toward this cat),
  **last_seen_location**, **reputation** (recent aggressive/grooming
  events)
- For location mental models: **last_threat** (fox at this tile three
  days ago), **last_opportunity** (prey plentiful last visit),
  **affective_tag** (safe/fearful/sacred), **owner** (for dens — links
  to cat model)
- For predator mental models: **last_seen_location**, **last_seen_tick**,
  **scent_signature**, **known_victims** (links to cat models)

*Evidence typology* (Clowder-subset of ToT's eleven — skipping
Reflection (trivial) and Lie (cats don't linguistically lie)):
- **Observation** — direct witness. Origination for most beliefs.
- **Transference** — new entity reminds the cat of an old one (shared
  scent, territory, coat color) → beliefs copy. *This is how cats
  generalize fear of one fox to all foxes.*
- **Confabulation** — probabilistic invention weighted by the colony's
  distribution. "Where do foxes live?" confabulated = most-commonly-
  reported fox territory.
- **Implant** (at end of world-gen / E1 boundary) — starting cats
  know their parents' world because their parents told them. Handled
  in E1's implantation phase.
- **Declaration** (behavioral, not linguistic) — every time a cat
  *acts on* a belief (flees from a tile, approaches a preferred den),
  the belief is reinforced. Corollary: panic behavior can
  self-reinforce into conviction even if original evidence was weak.
- **Mutation** — probabilistic drift per tick scan; affected by the
  cat's `memory` personality attribute (inherited from parent) and
  the facet's salience.
- **Forgetting** — belief terminates when strength hits zero.

*Evidence metadata* — every piece carries: **source** (which cat told
me, if any), **location**, **tick**, **strength**. Source + location +
tick are exactly what's needed to surface "Whisker told me at the old
den three days ago" as a citable narrative line.

*Salience computation* — probability of observing / propagating /
forgetting weighted by:
- Character salience: kin > bonded > coordinator > stranger
- Attribute salience: `last_threat` ≫ `coat_color`; `lineage` ≫
  `last_visited`
- Existing belief strength (weak beliefs more likely to deteriorate)

*Belief revision with candidate tracking.* First evidence adopts.
Contradicting evidence weaker than current → adopt as *candidate*
belief, track separately; further reinforcing evidence strengthens the
candidate until/unless it exceeds the currently-held belief, at which
point they swap. Enables *belief oscillation* (the cat isn't sure yet)
and *slow conversion* (Whisker eventually accepts the fox moved dens,
but not on first encounter).

*Candidate narrative outputs* (free byproducts):
- "Whisker no longer believes the old den is safe" (candidate won)
- "The colony has forgotten Silverpaw" (all belief facets terminated)
- "Ember wrongly believes the fox returned last night" (ground-truth
  divergence available for diagnostic assertion)

**Touch points:**
- `src/systems/colony_knowledge.rs` — promotion becomes
  "high-agreement-across-mental-models" rather than simple carrier
  count; ColonyKnowledge may be derived rather than primary
- `src/components/mental.rs` `Memory` + `MemoryEntry` — becomes a
  collection of mental models, each a list of belief facets
- `src/systems/social.rs` — conversation-style knowledge exchange
  during co-location (salience-weighted topic selection, per-facet
  exchange probability)
- `src/systems/sensing.rs` — observation-as-evidence pipeline
- New: `src/resources/ground_truth_log.rs` (or reuse
  `logs/events.jsonl`) for accuracy assertions and divergence
  diagnostics

**Preparation reading:**
- **Ryan, Summerville, Mateas, Wardrip-Fruin (2017). "Simulating
  Character Knowledge Phenomena in Talk of the Town."** *Game AI
  Pro 3* ch. 37 (free PDF at
  <https://www.gameaipro.com/GameAIPro3/GameAIPro3_Chapter37_Simulating_Character_Knowledge_Phenomena_in_Talk_of_the_Town.pdf>)
  — the definitive treatment. Read § 37.3 front-to-back; § 37.3.5
  (evidence typology) and § 37.3.9 (belief revision) are load-bearing.
- **Ryan, Summerville, Mateas, Wardrip-Fruin (2015). "Toward
  characters who observe, tell, misremember, and lie."** Proc. 2nd
  Workshop on Experimental AI in Games, Nov 2015, UC Santa Cruz —
  earlier, denser version
- **Ryan, Mateas, Wardrip-Fruin (2016). "Characters who speak their
  minds: Dialogue generation in Talk of the Town."** AIIDE 2016 —
  how mental-model facets feed dialogue/narrative generation
- **James Ryan, *Curating Simulated Storyworlds*** (UCSC 2018) ch. 4
  — long-form treatment; covers Hennepin successor system
- **Shi Johnson-Bey, *Neighborly*** (ECS Python, archived 2026-04-07,
  <https://github.com/ShiJbey/neighborly>) — concrete ECS sketch of
  a ToT descendant; stable reference
- **Damián Isla, "Third Eye Crime: Building a stealth game around
  occupancy maps"** (AIIDE 2013) — cited in § 37.3; simpler cousin
  useful for calibration

**Exit criterion:** three demonstrable phenomena:
1. *Deliberate false belief:* plant a ground-truth-inconsistent
   observation in one cat, propagate via gossip, observe the false
   belief spreading with measurable divergence duration.
2. *Candidate revision:* construct a scenario where a cat holds a
   stale belief, expose them to weak counter-evidence twice, verify
   candidate tracking works and eventually flips.
3. *Transference:* introduce a second fox that shares features with a
   historically-known fox, verify the cat transfers fear and/or
   territory beliefs.

Add `belief_divergence_duration` and `belief_candidates_per_cat` as
diagnostic lines in `logs/events.jsonl`.

**Dependency:**
- Gated on **A1** (belief-strength becomes a scoring axis — fear
  scales with belief strength, not just raw danger).
- Gated on **A3** (mental models are entities with components — the
  context-tag refactor is a prerequisite for clean per-cat belief
  storage).
- Pairs with **C2** (gossip is a practice; co-location exchange is a
  practice).
- **Architecturally intertwined with E1** — world-gen runs without
  knowledge phenomena (too expensive), then knowledge-implantation at
  the E1 → runtime boundary seeds each cat's mental models (see
  § 37.3.10 for ToT's implantation procedure).

---

#### C4. Strategist-coordinator task board

Existing entry: **this file, `#1 sub-3`** and design doc
`docs/systems/strategist-coordinator.md`. **Recontextualize under this
cluster** — it's the HTN-style hierarchical planning layer, sitting
above BDI intentions (C1), practices (C2), and belief modeling (C3).
Not duplicated here; see sub-task 3 of #1.

**Preparation reading** (for when the existing entry gets picked up):
- Dana Nau et al., "SHOP2: An HTN Planning System" (JAIR 2003) —
  canonical HTN reference; free PDF via Google Scholar
- Kallmann & Thalmann on hierarchical planning in game characters —
  shorter, more applied
- *Game AI Pro* chapters on hierarchical task networks and
  goal-oriented architectures — free at <http://www.gameaipro.com/>
- `docs/systems/strategist-coordinator.md` (in-repo) — the existing
  design stub

---

### 8. Formalization / verification cluster [D]

**Why this is a cluster:** D1–D3 are each half-day investigations that
formalize names for patterns Clowder likely already has. The payoff
is *vocabulary-as-engineering-leverage*: once `weather.rs` is labeled
as a Markov process, "add a rare unseasonal-warm-spell" becomes "add
a state + transition probabilities," not "figure out where in
`weather.rs` to add an if-else." Low urgency; no code changes expected
unless verification surfaces a bug.

#### D1. Verify / label corruption spread as cellular automaton

Does `src/systems/magic.rs` corruption use local-rule propagation
(classic CA) or global scalars? If CA, label it as such in the
system's `docs/systems/*.md` stub. If not, consider whether
reaction-diffusion PDE or CA rules would produce better-looking
spread patterns.

**Preparation reading (shared with D2/D3):**
- Stephen Wolfram, *A New Kind of Science* ch. 2–3 (skim) — free
  online at <https://www.wolframscience.com/nks/> — CA classification
- Epstein & Axtell, *Growing Artificial Societies* — Sugarscape shows
  CA-style spread inside agent-based models; closest to Clowder's
  use case
- NetLogo CA model library (<http://ccl.northwestern.edu/netlogo/>) —
  runnable reference implementations of forest-fire and diffusion CAs,
  directly analogous to corruption spread

#### D2. Verify / label mood dynamics as Markov process

Does `src/systems/mood.rs` implement explicit transition probabilities
between mood states? If yes, label as Markov. If transitions are
deterministic cascades, note the distinction.

**Preparation reading:**
- Any introductory probability textbook chapter on Markov chains
  (Grinstead & Snell, *Introduction to Probability* ch. 11, free
  Dartmouth PDF at <https://math.dartmouth.edu/~prob/prob/prob.pdf>)
- Marsella & Gratch, "Computationally modeling human emotion" (CACM
  2014) — depth on affect dynamics; probably overkill

#### D3. Verify / label weather transitions as Markov process

Probably already obvious; confirm in `docs/systems/` stubs.

**Preparation reading:** same as D2.

**Exit criterion for cluster D:** `docs/systems/*.md` stubs carry the
formal pattern name where applicable.

---

### 9. World-generation richness cluster [E]

**Why this is a cluster:** Clowder currently starts every game at t=0
with fresh cats — no lineage, named past, seeded `ColonyKnowledge`, or
historical bonds. Emergent narrative is therefore a pure
forward-product. Talk of the Town and Dwarf Fortress both fix this
with the same architectural move: run the sim loop itself for
generations before the player arrives.

#### E1. Pre-simulation history via same-loop fast-forward

**Why it matters:** ToT (*Game AI Pro 3* § 37.2.2) runs 140 sim-years
(1839–1979) using utility-based action selection (Mark 2009 — the
same substrate A1 refactors toward) and produces 300–500 NPCs with
full lineages, residences, daily routines, work networks, and
asymmetric unidirectional affinities as the *output* of that
fast-forward. There is no separate procedural-history algorithm;
history is what the runtime loop produces when you run it longer than
the player sees.

This is architecturally cheap: no new procedural system. It is
architecturally profound: the resulting past has the same causal
density as the present, because it *was* the present, a few generations
ago.

**Clowder analogue:** fast-forward ~3–5 cat generations (~15–30
sim-years) before the first player-visible tick. Output state at t=0:
- Starting cats have known parents, grandparents, great-grandparents
  referenced by name/lineage even when only the living cats are
  active entities
- `ColonyKnowledge` pre-seeded with named events produced during
  fast-forward ("the Long Winter of year 12," "the Fox that took
  Silverpaw")
- Asymmetric bonds already exist between starting cats because their
  ancestors' relationships propagated
- Starting cats carry implanted mental models (see C3) covering kin,
  territory, known predators, and colony history
- `fate.rs` carries prophecies/visions rooted in generated history
- `narrative.rs` templates can reference historical figures and events
  from tick 1

**Two-phase architecture (following § 37.2.2 + § 37.3.10):**

*Phase 1: fast-forward without knowledge phenomena.* Run the sim loop
for ~15–30 sim-years with all C3 belief machinery disabled. Cats still
interact, practices still run (C2), relationships still form, cats
still die and give birth, but no mental-model tracking, no belief
mutation, no gossip as knowledge propagation. Output: ground-truth
event log, lineage tree, surviving cat roster with relationships,
dens/territory/named features.

*Phase 2: knowledge implantation at the boundary.* Once fast-forward
terminates, run the implantation procedure: for each surviving cat,
populate mental models over their kin, closest bonds, territory, and
a probabilistically-selected subset of other entities weighted by
salience (§ 37.3.10, Listing 37.1 gives the pseudocode). All implanted
beliefs are accurate at t=0; divergence emerges once runtime knowledge
phenomena activate.

*Phase 3: runtime with knowledge.* From t=0 onward, full C3 machinery
is active: observation, transference, confabulation, mutation,
declaration-reinforcement, belief-revision, forgetting.

Phase 1 is cheap (sim loop minus rendering, maybe coarser ticks).
Phase 2 is a one-time bulk-insert. Phase 3 is the runtime system and
pays the ongoing cost.

**Current state:** `src/main.rs` spawn phase creates cats at t=0 with
fresh stats; `Memory.events` is empty; `ColonyKnowledge.entries` is
empty; no lineage depth.

**Touch points:**
- `src/main.rs` spawn phase — new pre-sim fast-forward phase
- `build_schedule()` in `src/main.rs` + `SimulationPlugin::build()` in
  `src/plugins/simulation.rs` — both need a "history-gen" mode that
  can run the sim loop without rendering, narrative emission (or at a
  tier filter), or per-tick diagnostic capture
- `src/resources/colony_knowledge.rs` — persist state across the
  fast-forward → runtime transition
- `src/components/mental.rs` — support inherited memories referencing
  ancestors
- `src/systems/social.rs` — asymmetric affinities survive the
  transition
- `src/systems/fate.rs` — seed prophecies from generated history
- `src/systems/narrative.rs` — templates reference historical state

**Performance sub-concern:** current headless runs ~15 min wall per
~10 sim-days. Fast-forward of 15 sim-years at that throughput ≈ 8
hours wall — too slow. A history-gen mode needs 10–100× throughput,
achievable by (a) skipping rendering + detailed sensing, (b) filtering
narrative emission to Significant tier only, (c) disabling per-axis
diagnostic capture, (d) possibly coarser tick resolution during
fast-forward. Benchmark and profile before scoping.

**Preparation reading:**
- **Ryan et al. *Game AI Pro 3* ch. 37 § 37.2.2 "World Generation"**
  — the 140-year loop architecture; "world-gen is the sim loop, run
  longer"
- **§ 37.3.10 "Knowledge Implantation"** (same chapter, Listing 37.1)
  — the bridge procedure from fast-forward to runtime knowledge
- **Ryan, Mateas, Wardrip-Fruin (2016). "A simple method for evolving
  large character social networks."** Proc. 5th Workshop on Social
  Believability in Games — "Ryan 16c"; the *how* to § 37.2.2's *what*
- **Tarn Adams (2015). "Simulation principles from Dwarf Fortress."**
  *Game AI Pro 2* pp. 519–522, CRC Press — DF's stance on
  world-generation-as-simulation; same intellectual tradition
- Tarn Adams + Tanya Short, GDC 2016 "Procedural Narrative and
  Mythology" — DF's version at larger scale
- James Ryan, *Curating Simulated Storyworlds* ch. 4–5 — long-form
  treatment
- Neighborly `neighborly/simulation/` — concrete ECS implementation
  of fast-forward over a ToT-like model
  <https://github.com/ShiJbey/neighborly>

**Exit criterion:** at the first player-visible tick, `ColonyKnowledge`
is non-empty, starting cats have ≥2-generation lineage referenceable
by name, ≥1 seeded historical event appears in a narrative line in
the first sim-week, and the "named events per sim year" continuity
canary passes from t=0 forward without relying on live-sim events.

**Dependency:**
- **Gated on A1 (IAUS refactor).** Running 15+ sim-years of
  linear-additive scoring bakes scoring pathologies into "canonical
  history" and produces an incoherent past.
- **Gated on performance work.** History-gen mode must run fast
  enough to be tolerable at build time; spec before implementation.
- Pairs with **C2 (Versu practices)** — courtship and mentoring
  practices during fast-forward produce the relationship graph at
  t=0.
- Pairs with **C3 (subjective knowledge)** — generated history is
  ground truth; per-cat beliefs are subjective views. Different
  starting cats know different versions of the generated past.
- Pairs with **C4 (strategist-coordinator)** — leadership patterns
  during fast-forward produce the current coordinator *and* the
  dynastic backstory explaining why they lead.

---

## Landed

### v0.2.0 release — `aca13acf` (2026-04-19)

The `chore: release v0.2.0` commit bundled in-flight threads that had been
staged as "uncommitted" in earlier revisions of this document. Kept here
rather than deleted because the archived baselines and report pointers
remain useful for retros.

- **Balance: `eat_from_inventory_threshold: 0.05 → 0.4`** — seed-42 15-min
  soak: starvation 2→1, below-0.3 hunger 1.06%→0.50%, stores mean 85%→92%,
  leisure action-time +18%, colony survives +2 sim-weeks. Report at
  `docs/balance/eat-inventory-threshold.report.md`. Baselines:
  `logs/tuned-42-archive-apr17/`, `logs/tuned-42-baseline-eat-threshold/`,
  `logs/tuned-42/`. Pre-existing: `check_canaries.sh` still fails on
  `Starvation == 0` (now 1, was 2).
- **Docs reframe** — CLAUDE.md opening rewrite + Systems inventory +
  continuity canaries + `src/main.rs:346` line reference correction;
  `docs/systems/project-vision.md` new (thesis, influences, design
  corollaries); this file introduced.

### Mentor snapshot "never applied" — obsolete (no commit, 2026-04-19)

Prior follow-on item claimed `resolve_mentor_cat` produces a snapshot that
is never consumed. Verified false: the snapshot IS drained in the live
GOAP path at `src/systems/goap.rs:2672–2743` (biggest teachable skill gap
gets `growth_rate * apprentice_skill_growth_multiplier` added to the
apprentice's `Skills`). The `disposition.rs:3157` consumer is in
`resolve_disposition_chains`, which is not registered in either
`SimulationPlugin::build()` or `build_schedule()` — dead code.

Mentor *does* teach when it fires. Mentor firing 0× in the seed-42 soak
is a target-availability problem, already covered by follow-on #1.

---

## Conventions

- When an item here becomes a plan, write the plan and leave a pointer in
  the entry (don't delete it until the plan lands).
- When an item lands, move the entry to the "Landed" section above with
  the commit hash, or just delete it if trivial.
- New entries go at the end of the relevant section, dated inline if the
  context is time-sensitive.
