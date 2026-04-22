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
> `.claude/skills/rank-sim-idea/SKILL.md`). The top of the backlog is
> now **Post-death biographies (1024)** — a presenter-layer entry
> added 2026-04-21, see #10 below — followed by Recreation & Grooming
> (900) and The Calling (675). Sleep Phase 1 and Environmental Quality
> were folded into the A-cluster refactor (see
> `docs/systems/ai-substrate-refactor.md` §10), which is where the
> former cheap wins actually get built.

### 1. Explore dominance over targeted leisure

> **Parked 2026-04-21** for AI substrate refactor (see
> `docs/systems/refactor-plan.md` pre-flight gate 1). Both sub-tasks
> verified unresolved but outside the refactor's blast radius.
> - **Sub-1 (social-target-range iter 3)** — superseded by refactor
>   Phase 4 target-selection (§6 `TargetTakingDse` replaces
>   `has_social_target` existence gate with target-quality scoring);
>   the pair-stickiness alternative named in iter-2's report becomes
>   a natural per-target consideration there.
> - **Sub-2 (Explore saturation curve)** — re-evaluated post-Phase-3c
>   once Explore runs through the unified evaluator with a proper §2.1
>   response curve. The sharp-decay-past-0.7 shape becomes a Logistic
>   consideration on `unexplored_fraction_nearby` rather than a bespoke
>   patch to today's linear multiplier.
> - **Sub-3 (strategist-coordinator)** — unchanged; still C4 in the
>   deliberation cluster, gated on cluster A.
> - **Resume when:** refactor reaches Phase 4 entry (sub-1) / Phase 3c
>   exit (sub-2).

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

### 10. Post-death biographies via Claude API (presenter) [2026-04-21]

**Why it matters:** Lights the **mythic-texture** continuity canary
(≥1 named event per sim year, currently zero from live-sim sources)
plus §5 **preservation** and **generational knowledge**. On `CatDied`
(or post-hoc over `logs/events.jsonl`), extract the cat's lifelog,
feed it to a prebuilt Claude API skill, emit prose into
`logs/biographies/<cat>.md`. The closest Clowder gets to DF's legend
mode.

**Architectural contract (load-bearing for the score):** LLM runs as
a **strict presenter** — reads finalized sim artifacts only, writes
sidecar files the sim never reads back. The `CLAUDE.md` "No LLMs"
rule defends authorial intent (sim behavior auditable back to math
the user wrote); presenter-only discipline is compatible with that
rule because the presenter contributes nothing to the `ground-truth →
math → outcome` chain. Audit test for the contract: `rm -rf
logs/biographies && just soak 42` produces byte-identical
`events.jsonl` + verification-tier `narrative.jsonl`. Assert this in
CI.

**Cross-reference:** `docs/systems-backlog-ranking.md` rank 1 —
V=4/F=4/R=4/C=4/H=4 → **1024** (cheap win; do first). Lands the
presenter-layer infrastructure (per-cat event indexing, Claude API
client, sidecar routing, CI audit test) that #11 below reuses.

**Open design choices:**
- Live-on-death vs. post-hoc log-processing tool. Post-hoc is
  strictly easier; live-on-death couples the sim binary to an
  external service.
- Sidecar directory vs. `narrative.jsonl` tier. **Strongly prefer
  sidecar** — keeping biographies out of verification-tier files
  preserves the byte-identical-across-matching-headers property that
  balance soaks rely on.
- Which lifelog events feed the prompt (cost and prose quality are
  both sensitive — more isn't better).

**Soft prerequisites:** audit whether every lifecycle-relevant event
in `logs/events.jsonl` carries a `cat_id` (spawns, significant
interactions, deaths); denormalize where missing.

**Memory write-back on landing:** commit an
`ongoing-tax-biographies` pattern memory per the skill's schema so
the next external-service triage has a prior to query.

---

### 11. Cat-conversation rendering via Haiku (presenter over C3) [2026-04-21]

**Why it matters:** Once C3 (§7 above) ships deterministic
facet-exchange records per Ryan, Mateas, Wardrip-Fruin 2016
*"Characters who speak their minds"* (AIIDE), Haiku renders the
prose of those exchanges into `logs/conversations/<tick>.md`. Belief
math stays in C3; LLM output never feeds back into sim state.

**Architectural contract:** same strict-presenter contract as #10.
C3 decides *what* beliefs got exchanged; the LLM only renders the
dialogue those exchanges would have produced.

**Cross-reference:** `docs/systems-backlog-ranking.md` rank 7 —
V=4/F=3/R=3/C=2/H=3 → **216** (earn the slot, after C3). Under the
**original in-loop framing** (LLM drives conversation → conversation
drives belief → belief drives scoring) the score is **4** —
shadowfox-worse, defer. The 216 only holds under strict presenter
discipline.

**Required hypothesis + prediction** (80–300 bucket per `CLAUDE.md`
Balance Methodology): *Adding presenter-rendered conversation prose
over C3's deterministic facet exchanges will not measurably alter
any canary (sim behavior is unchanged) but will measurably increase
time-to-comprehension when reading a seed-42 soak's social events.*
Null-direction sim prediction is unusual but correct here — this is
a rendering change, not a balance change.

**Dependencies:** gated on **A1** + **A3** + **C3** (above §§5 and
§7) and on **#10** landing first (reuses presenter-layer
infrastructure). Three-deep dependency chain; no rush.

**Risk surface to watch:** the soft aesthetic tax that LLM prose and
sim math can diverge — narratively-satisfying LLM prose subtly
drowning out the math's quieter truths. H=3 priced this in; vigilance
is the mitigation.

### 12. Warmth split — temperature need vs social-warmth fulfillment axis [2026-04-21]

**Status:** phase 1 (design) committed; phases 2–4 pending.

**Why it matters:** `needs.warmth` currently conflates physiological
body-heat (hearth/den/sleep/self-groom) with affective closeness
(grooming another cat,
`src/steps/disposition/groom_other.rs:47`). A cat near a hearth is
immune to loneliness at the needs level. The warring-self dynamic
of `docs/systems/ai-substrate-refactor.md` §7.W.2 requires a cat to
be able to be physically warm and socially starving at the same
time — otherwise the losing-axis narrative signal is drowned out by
shelter.

**Design captured at:** `docs/systems/warmth-split.md` (phase 1).
Cross-linked from `ai-substrate-refactor.md` §7.W.4(b).

**Phase 2 — mechanical rename.** Rename `needs.warmth` →
`needs.temperature` and all `*_warmth_*` constants across ~30 call
sites enumerated in the design doc. No behavior change. Verify
with `just check`, `just test`, and byte-identical
`sim_config`/`constants` header on seed 42 vs pre-rename baseline.
Safe; a single commit.

**Phase 3 — `social_warmth` implementation.** Gated on §7.W
Fulfillment component/resource landing. Adds `social_warmth` as a
fulfillment axis; modifies `groom_other.rs:47` to feed both parties'
`social_warmth` instead of the groomer's temperature; adds
isolation-driven decay; adds UI inspect second bar. Small expected
balance impact.

**Phase 4 — balance-thread retune.** New
`docs/balance/warmth-split.md` iteration log. Hypothesis: removing
social-grooming from temperature-inflow reduces well-bonded cats'
temperature refill by ~10–20%; without compensating drain-rate
reduction, cold-stress rises 1.5–3× on seed 42. Full four-artifact
acceptance per CLAUDE.md balance methodology. Starvation and
cold-death canaries must remain 0.

**Dependencies:** phase 2 is independent and can land any time.
Phase 3 is gated on §7.W (Fulfillment component) landing. Phase 4
is gated on phase 3.

### 14. Phase 4 follow-ons — target-taking registration + markers + mate-gender + Mating/PracticeMagic magnitude [2026-04-22]

**Why it matters:** Phase 4a landed three of the five Phase 4
deliverables (softmax-over-Intentions, §3.5 modifier pipeline port of
Herbcraft/PracticeMagic emergency bonuses, Adult-window retune). The
seed-42 `--duration 900` re-soak clears every survival canary and
reverses the three Phase-3-exit regressions, but two spec-committed
Phase 4 deliverables + three balance gaps still stand.

Phase 4a landing entry lives in the Landed section below; the
remaining work is itemised here.

**Still outstanding (spec-committed, Phase 4 scope):**

- **`add_target_taking_dse` + per-target considerations (§6.3,
  §6.5).** The §9.3 stance bindings shipped in `c8bb1c6` are
  declarative — their runtime-filtering consumption waits on this
  registration method. `src/ai/dse.rs` needs a `TargetTakingDse`
  trait; `eval.rs` gets a `add_target_taking_dse` method on
  `DseRegistryAppExt`.
- **§4 marker-eligibility authoring systems for roster gap-fill.**
  Per spec §4.6, this is a ~50-marker catalog across 13 author
  files plus the `src/ai/capabilities.rs` new-file. Scope for the
  landing PR:
    1. Wire `has_marker` from its current `|_, _| false` stub
       (`scoring.rs:435`, `score_dse_by_id`) to a real ECS-query
       backed closure. Canonical pattern: a `#[derive(SystemParam)]`
       bundle carrying per-marker `Query<With<MarkerN>>` rows, with
       a `fn has(&self, key: &str, entity: Entity) -> bool` that
       dispatches on the key. Every new marker in the catalog
       requires a query row here; the dispatch grows by one arm.
    2. Introduce a `ColonyState` singleton entity for colony-scoped
       markers (`HasStoredFood`, `HasFunctionalKitchen`,
       `WardStrengthLow`, `ThornbriarAvailable`, …).
    3. Author per-tick systems for each marker per §4.6 author-file
       assignments. `Changed<T>` filters where predicates read
       changing parent components; full-scan where predicates read
       position-adjacent state.
    4. Cut over each DSE's outer eligibility gate in `score_actions`
       to an `EligibilityFilter::require(marker)` row. Retire the
       inline `if ctx.flag { … }` block as its marker lands.
  **Nuance uncovered during Phase 4b investigation:** marker
  authoring alone does **not** unblock the Cleanse / Harvest /
  Commune dormancies. `magic_cleanse` requires the cat to be
  standing on a corrupted tile; `magic_harvest` requires a carcass
  within range; `magic_commune` requires fairy-ring / standing-stone
  adjacency. These gates reflect physical colocation, not authoring
  absence — porting them to markers cleans up the evaluator's hot
  path but doesn't change the underlying navigate-to-tile problem.
  Real unblock needs either (a) GOAP plan-shape changes that route
  cats TO corrupted tiles when they carry intent to cleanse, or
  (b) the §6.3 `TargetTakingDse` path where "target = corrupted
  tile" is a first-class candidate the evaluator scores distance
  to. Track as its own follow-on once §4 markers land.
- ~~**§7.M.7.4 `resolve_mate_with` gender fix.**~~ Landed as Phase
  4b.1 — see Landed section below.

**Balance gaps on the Phase 4a re-soak** (seed 42, `--duration 900`,
commit TBD on landing):

- **MatingOccurred = 1, target ≥ 1 per colony per season (45 per
  soak).** Phase 4a recovered the zero but fell short of the density
  target by ~45×. BondFormed 16 → 28 shows the social fabric
  strengthened; mating-act scoring is still the bottleneck. Levers
  from §7.M.7.8: raise per-mating conception roll (if needed for
  Generational-continuity canary), reduce cycle period, or tune
  `mating_interest_threshold`. Needs a separate balance iteration.
- **PracticeMagic sub-mode count 2 / 5** (Scry + DurableWard fire;
  Cleanse, ColonyCleanse, Harvest, Commune dormant). These gates
  reflect *physical colocation* (corrupted tile under the cat,
  carcass in sensor range, fairy ring under the cat), not modifier
  shape or marker authoring. Real unblock is plan-shape work — a
  cat with intent to cleanse must navigate to known corruption
  first. Routes through either §4 + GOAP-prep-step work, or §6.3
  `TargetTakingDse` where the target-candidate set includes
  corrupted tiles as spatial candidates.
- **Farming = 0** (stayed dormant). Baseline log shows
  `TendCrops: no target` plan-failures = 83 — crop entities exist
  but the resolver isn't matching them to the chain. Same target-
  availability shape as the cleanse/harvest dormancies. Tracks
  with whichever of §4 markers or target-taking DSE lands the
  target-resolver plumbing.
- **Dependency graph (balance gaps):** `markers_authoring` +
  `target-taking DSE` land in either order but together are the
  prerequisite for the dormancy unblock. The
  `mating_density` lever (Fertility-cycle pacing) is independent
  and tunable without either.

**Dependency graph (spec-scope work):**
- `add_target_taking_dse` and `markers_authoring` are orthogonal
  refactors — either can land first. Both are session-scale
  multi-hour pieces on their own. Shipping either partially is
  high-risk because `has_marker` wiring and `EligibilityFilter`
  consumption both need to land in lockstep.
- Neither directly unblocks the balance gaps — those route through
  downstream GOAP / target-resolver work once the foundations
  land.

**Re-open condition for Phase 3 hypothesis:** Phase 4a cleared the
survival canaries (Starvation 8 → 0, ShadowFoxAmbush 0) and moved
MatingOccurred off zero. The Phase 3 hypothesis in
`docs/balance/substrate-phase-3.md` is not being re-opened — the
three substrate mechanisms are validated. The remaining gaps route
through this entry's outstanding items.

---

### 13. Spec-follow-on debts from AI substrate refactor [2026-04-21]

**Why it matters:** The `docs/systems/ai-substrate-refactor.md`
spec committed its architectural decisions but carries six
spec-follow-on hooks whose resolution lives in *other* systems
(`src/systems/death.rs`, `fate.rs`, `mood.rs`, `coordination.rs`,
`aspirations.rs`) or in code (retired-constants cleanup under
§2.3). On 2026-04-21 the refactor's Enumeration Debt ledger was
pruned to spec-scope only; these six items moved here so (a) they
don't get lost from the refactor ledger as that doc narrows to
its own scope, and (b) their respective system owners can pick
them up in the PRs that touch each system.

Each item's substrate-side contract is *already committed* in
`ai-substrate-refactor.md`; what remains is target-system
implementation or enumeration work.

- **13.1 Retired scoring constants + incapacitated branch cleanup.**
  Spec: §2.3 "Retired constants" subsection. Delete the five
  `incapacitated_*` fields + the `if ctx.is_incapacitated`
  early-return block at `src/ai/scoring.rs:181–201`, plus
  `ward_corruption_emergency_bonus`,
  `cleanse_corruption_emergency_bonus`, and
  `corruption_sensed_response_bonus` from `SimConstants`.
  **Gated:** lands in the same PR that introduces the Logistic
  curves that replace them — cluster A entry #5 (A1 IAUS
  refactor). Not before. Behavior-preserving once the curves are
  in; dangerous before.

- **13.2 Death-event relationship-classified grief emission
  (§7.7.b).** `src/systems/death.rs` today emits only
  generic-proximity grief + FatedLove/Rival removal. §7.7
  aspirations need a richer event — candidate shape is
  `CatDied { cause, deceased, survivors_by_relationship }` (or
  equivalent) — so §7.7.b reconsideration events can filter
  per-relationship (grief-for-mate vs. grief-for-mentor vs.
  grief-for-kin). **Gated:** requires formal relationship
  modeling beyond the current three-tier `BondType`, which is
  Talk-of-the-Town-adjacent work (see cluster C #7, sub-task C3
  — Subjective knowledge / belief distortion).

- **13.3 Fate event-vocabulary expansion (§7.7.c).**
  `src/systems/fate.rs` today emits only `FatedLove` / `FatedRival`.
  Aspirations that should respond to the Calling, destiny
  modifiers, or fated-pair convergence need those events to
  exist. **Gated:** on the Calling subsystem design per
  `docs/systems/the-calling.md` — itself rank 3 in
  `docs/systems-backlog-ranking.md`. Cross-cutting debt; lands
  alongside the Calling implementation, not standalone.

- **13.4 Mood drift-threshold detection layer (§7.7.d).**
  `src/systems/mood.rs` valence today has no hysteresis or
  sustain-duration detection. §7.7.d aspirations need "valence
  below X for N seasons AND misalignment with active-arc
  expected-mood" to fire mood-driven aspiration reconsideration.
  Design-heavy — its own small balance thread. **Gated:** on
  per-arc expected-valence targets, which land with the
  aspiration-catalog work in 13.5 below.

- **13.5 Aspiration compatibility matrix (§7.7.1).** The four
  conflict classes (hard-logical / hard-identity / soft-resource
  / soft-emotional) are committed in the spec; the specific
  hard-logical + hard-identity pair list is enumeration work
  against the stabilized aspiration catalog. **Gated:** lands in
  the PR that enumerates aspirations themselves (aspirations
  catalog isn't currently a tracked entry in this file — add
  one if prioritized). Also unblocks 13.4.

- **13.6 Coordinator-directive Intention strategy row (§7.3).**
  The §7.3 footer note commits `SingleMinded` with a
  coordinator-cancel override; the full row contents land with
  the coordinator DSE. **Cross-ref:** #1 sub-3 above — the C4
  strategist-coordinator task board. When C4 is picked up, this
  row gets its final commit and the ledger-level pointer in
  `ai-substrate-refactor.md` resolves.

**Dependency graph:**

- 13.1 gated on cluster A (#5 — A1 IAUS refactor).
- 13.2 gated on C3 (#7 — belief modeling).
- 13.3 gated on the Calling subsystem
  (`docs/systems/the-calling.md`; no current open-work entry —
  add one if prioritized ahead of 13.3).
- 13.4 gated on 13.5 (needs per-arc valence targets).
- 13.5 gates 13.4; stands on its own given the aspiration catalog.
- 13.6 gated on C4 (#1 sub-3).

**Memory write-back on landing:** commit per-subtask memories as
each lands so the next cross-thread session has a local record
of what the substrate's follow-on contract was and how the
system owner satisfied it. Tag pattern: `substrate-follow-on`,
`{subsystem-name}`, `ai-substrate-refactor`.

---

## Landed

### Phase 4b.1 — §7.M.7.4 `resolve_mate_with` gender fix (2026-04-22)

Spec §7.M.7.4 committed that `Pregnant` must land on the
gestation-capable partner, not the initiator. Today's code did the
opposite — a Tom initiator paired with a Queen produced a pregnant
Tom. Shipped:

- `Gender::can_gestate` — Queens and Nonbinaries gestate; Toms
  don't.
- `resolve_mate_with` now takes both genders, returns
  `Some((gestator, litter_size))`. Tom×Tom returns `None` (mating
  need clears so the step advances; no `Pregnant` insert, no
  `MatingOccurred` event). Ties resolve to the initiator per spec.
- Both callers (`systems/disposition.rs`, `systems/goap.rs`)
  snapshot gender alongside the existing grooming snapshot and
  insert `Pregnant` on the returned gestator. `Pregnant::partner`
  carries the other mate.

Six new unit tests cover the four gender permutations, pre-
threshold continuation, and hunger-driven litter-size bump.

### Phase 4a — softmax-over-Intentions + §3.5 modifier port + Adult-window retune (2026-04-22)

Three Phase 4 deliverables landed together on the
`docs/balance/substrate-phase-4.md` balance thread. Each addresses one
of the three Phase 3 exit-soak regressions that prompted open-work #14:

- **§L2.10.6 softmax-over-Intentions** (`src/ai/eval.rs`,
  `src/ai/scoring.rs`, `src/ai/fox_scoring.rs`, `src/systems/goap.rs`,
  `src/systems/disposition.rs`, `src/systems/fox_goap.rs`). Replaced
  the `aggregate_to_dispositions → select_disposition_softmax`
  two-step and the fox-side argmax with direct softmax over the flat
  Intention pool. New `select_intention_softmax` in `eval.rs` consumes
  `&[ScoredDse]` per §L2.10.6; bridge helper
  `select_disposition_via_intention_softmax` in `scoring.rs` operates
  on the legacy `(Action, f32)` pool and maps via
  `DispositionKind::from_action`. New
  `ScoringConstants::intention_softmax_temperature` (default 0.15).
- **§3.5 modifier-pipeline port** — new `src/ai/modifier.rs` with
  three `ScoreModifier` impls (`WardCorruptionEmergency`,
  `CleanseEmergency`, `SensedRotBoost`). `ScoreModifier::apply`
  extended to take a `fetch_scalar` closure so modifiers read
  trigger inputs through the same canonical scalar surface as DSE
  considerations. `ctx_scalars` gained `nearby_corruption_level`,
  `maslow_level_2_suppression`, `has_herbs_nearby`, `has_ward_herbs`,
  `thornbriar_available`. The three emergency-bonus additions at
  `scoring.rs:576–712` are retired; pipeline registered at all four
  mirror sites (`plugins/simulation.rs` + `main.rs` setup_world /
  run_new_game + test infra in scoring.rs).
- **Adult life-stage window retune** — `Age::stage` Adult upper
  bound 47 → 59, Elder 60+. Paired update: `DeathConstants::
  elder_entry_seasons` 48 → 60 and `FounderAgeConstants::
  elder_{min,max}_seasons` 48/50 → 60/62 to keep the stage /
  old-age-mortality coupling and founder-runway invariants intact.
  Marker doc comments updated; `age_stages_at_boundaries` test
  updated to the new thresholds.

**Concordance on seed-42 `--duration 900` re-soak (landed commit `c4552dc`, log `logs/phase4a-c4552dc/events.jsonl`):**

| Metric | Baseline (`562c575`) | Phase 4a | Direction |
|---|---|---|---|
| deaths_by_cause.Starvation | 8 | 0 | ✅ canary passes |
| MatingOccurred | 0 | 0 | flat (substrate gate opens but density is a follow-on tune — dirty-commit run hit 1 on seed noise) |
| BondFormed | 16 | 34 | +112% |
| ScryCompleted | 256 | 615 | +140% |
| WardPlaced | 89 | 264 | +197% |
| ward_avg_strength_final | 0.0 | 0.39 | wards persisted |
| Grooming (continuity) | 30 | 213 | +610% |
| KnowledgePromoted | 35 | 92 | +163% |

Canonical `scripts/check_canaries.sh` passes all four survival
canaries (Starvation == 0, ShadowFoxAmbush ≤ 5, footer written,
features_at_zero informational). Generational-continuity canary still
fails (0 kittens matured) but that tracks with the MatingOccurred
density gap, not the substrate mechanisms shipped.

**Remaining Phase 4 work** moved to open-work #14 (outstanding):
target-taking DSE registration, §4 marker-eligibility authoring
systems, §7.M.7.4 `resolve_mate_with` gender fix, and the
MatingOccurred density + Cleanse/Harvest/Commune/Farming dormancy
balance gaps unblocked by §4 marker authoring.

Balance thread: `docs/balance/substrate-phase-4.md`.

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
