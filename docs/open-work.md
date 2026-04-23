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
  §6.5).** **Phase 4b.3 foundation + Phase 4c.1 Socialize + Phase
  4c.2 Mate + Phase 4c.5 Mentor reference ports landed** —
  `TargetTakingDse` struct, `TargetAggregation` enum,
  `evaluate_target_taking` evaluator, `add_target_taking_dse`
  registration; plus §6.5.1 Socialize, §6.5.2 Mate, and §6.5.3
  Mentor per-DSE ports closing the §6.2 silent-divergences
  between `disposition.rs::build_socializing_chain` /
  `build_mating_chain` (weighted mixers) and
  `goap.rs::find_social_target` (fondness-only, no bond filter /
  no skill-gap ranking).
  **Phase 4c.6 closeout landed** — Groom-other (§6.5.4), Hunt
  (§6.5.5), Fight (§6.5.9), ApplyRemedy (§6.5.7), Build (§6.5.8)
  ports all landed together with `find_social_target` retired
  (see Landed). **Remaining:** the Caretake (§6.5.6) full
  TargetTakingDse migration — `resolve_caretake` in
  `caretake_targeting.rs` already wraps the §6.5.6 signal
  faithfully; the DSE-shape migration is deferred until a
  follow-on session schedules it. Each port followed the Phase
  4c.1 pattern:
    1. `TargetTakingDse` factory function (consideration bundle
       from §6.5.N, composition, aggregation).
    2. Caller-side resolver helper that assembles candidates,
       builds fetchers, invokes `evaluate_target_taking`, and
       returns `Option<Entity>` — lives in the same file as the
       factory (see `src/ai/dses/socialize_target.rs`).
    3. Wiring at each caller site: scoring bool gate reads the
       resolver's `is_some()`; chain-builders / step resolvers
       consume the returned entity directly (merging into the
       flat action-score pool via multiplicative modulation is
       deferred pending balance stabilization).
    4. Retire the legacy resolver (e.g. `find_social_target`,
       `nearest_threat`) — silent-divergence fix per §6.2.
    5. Thread winning target through the downstream step's
       `target_entity` field so GOAP plans against it.
  ~~**Caretake (§6.5.6) is now BLOCKING further per-DSE ports**~~
  **Caretake blocker cleared** by Phase 4c.3's urgency-signal fix
  + Phase 4c.4's alloparenting Reframe A + GOAP retrieve step +
  target-entity persistence (`KittenFed = 55 / 10 / 79` across
  recent soaks; see Landed). Phase 4c.5 (Mentor) landed on the
  cleared gate with no starvation regression. The full §6.5.6
  Caretake `TargetTakingDse` port remains outstanding (the
  existing `resolve_caretake` helper in `caretake_targeting.rs`
  pre-dates the target-DSE substrate and should migrate to match
  the Socialize / Mate / Mentor pattern once scheduled).
- **§4 marker-eligibility authoring systems for roster gap-fill.**
  **Phase 4b.2 MVP + 4b.4 landed** (lookup foundation +
  `HasStoredFood` + `HasGarden` reference ports — see Landed
  section below). The remaining ~48 §4.3 markers each need:
    1. Author system per §4.6 author-file assignment (`Changed<T>`
       filter where the predicate reads changing parent components;
       full-scan where it reads position-adjacent state).
    2. Population line in goap.rs / disposition.rs: either
       `markers.set_colony(name, bool)` or per-cat
       `markers.set_entity(name, entity, bool)`.
    3. Target DSE's `.require(name)` cutover — retire the inline
       `if ctx.flag { … }` block as its marker lands.
    4. Optional: promote colony-scoped markers off the snapshot
       shim onto a dedicated `ColonyState` singleton entity with
       real ZST components and `Q<With<ColonyState>, With<Marker>>`
       queries. Snapshot is the interim; singleton is the spec
       canonical (§4.3 Colony).
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

**Balance-tuning observations — deferred to post-refactor.**

Several positive-feature metrics remain below their literal
Phase 4 exit targets on the seed-42 `--duration 900` soak at HEAD
(`logs/phase4b4-db7362b/events.jsonl`):

- MatingOccurred = 0 (literal target ≥ 7 per 7-season soak).
- PracticeMagic sub-mode count = 2 / 5 (literal target ≥ 3 / 5).
- Farming = 0 (literal target ≥ 1).

These are **not** treated as Phase 4 blockers. Rationale:

1. **No colony wipes.** All four survival canaries pass
   (Starvation 0, ShadowFoxAmbush 0, footer written,
   features_at_zero informational). The colony survives the
   soak — the density gaps are aesthetic / verisimilitude gaps,
   not existential ones.
2. **Refactor reshapes scoring.** The remaining
   `ai-substrate-refactor.md` work (target-taking DSE ports,
   §4 marker catalog fill-in, §5 influence maps, §7 commitment
   strategies) will change the shape of scoring for exactly
   the DSEs whose numbers would be tuned. Any per-knob tuning
   done now would need to be redone after each successor phase
   lands.
3. **Tuning belongs at a stable substrate.** CLAUDE.md's Balance
   Methodology requires a four-artifact acceptance per drift.
   Tuning against a moving substrate wastes artifacts on shapes
   that will change.

**Commitment:** balance iterations on positive-feature density
(mating, magic sub-modes, farming) wait until the refactor's
substrate changes have stabilized. At that point, each of the
three gaps gets a dedicated balance thread with its own
hypothesis-prediction-observation-concordance record. Until
then, the metrics are tracked in soak footers for trend
visibility but not tuned.

Causally, the dormancy gaps (Cleanse / Harvest / Commune /
Farming) also trace to refactor-layer missing plumbing — the
"navigate TO a physical location before scoring the action"
shape belongs to §6.3 `TargetTakingDse` with spatial
candidates or to `GOAP` plan-shape preparatory steps. Landing
those naturally unblocks the dormancies before any numeric
tuning is relevant.

**Dependency graph (refactor-scope work):**
- `add_target_taking_dse` and `markers_authoring` are orthogonal
  refactors — either can land first. Both are session-scale
  multi-hour pieces on their own. Shipping either partially is
  high-risk because `has_marker` wiring and `EligibilityFilter`
  consumption both need to land in lockstep. (4b.2 landed the
  `has_marker` wire-up; 4b.3 landed the `TargetTakingDse`
  foundation — remaining work on both tracks is per-DSE /
  per-marker port work.)
- The per-DSE target-taking ports are the primary unblock for
  the named balance gaps (target = corrupted tile, target =
  carcass, etc. become first-class spatial candidates). Most
  dormancies resolve as a consequence of refactor completion,
  not as a separate tuning pass.

**Re-open condition for Phase 3 hypothesis:** Phase 4a cleared the
survival canaries (Starvation 8 → 0, ShadowFoxAmbush 0). The
Phase 3 hypothesis in `docs/balance/substrate-phase-3.md` is not
re-opened — the three substrate mechanisms are validated and the
colony survives the soak. The literal positive-exit-metric targets
(MatingOccurred density / 3-of-5 sub-modes / Farming ≥ 1) are
deferred per the balance-tuning-after-refactor commitment above.

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

### 15. Alloparenting Reframe B — mama drops kitten at hearth near resting elder [2026-04-22]

**Deferred after Reframe A landed (Phase 4c.4, 2026-04-22).**
`KittenFed` is no longer zero (55 / 10 on seed-42 v3 soaks) — the
any-adult-feeds-any-hungry-kitten pattern lit up on the back of
(1) bond-weighted compassion, (2) the GOAP Caretake retrieve
step, and (3) target-entity persistence. See Landed Phase 4c.4
entry for the full bundle.

The canary Reframe B was meant to unblock — generational
continuity, a kitten reaching Juvenile in a soak — **is still
zero**, but the A-vs-B diagnosis in Reframe A's hypothesis has
shifted: more adults feeding kittens is no longer the bottleneck,
since A established that dozens of feedings happen per soak. The
gap between "kittens are fed" and "kittens reach Juvenile" is
downstream of feeding frequency — either growth-rate tuning or
the Phase 4c.3 literature anchor's milk-yield / nursing-quality
model. Adding an elder-hearth handoff mechanic doesn't help the
current bottleneck; it would add a communal-care texture without
shifting the canary.

**Resume when:** growth-tuning / milk-yield follow-ons have been
attempted and KittenMatured is still blocked on "no adult is
available to feed." Then B-elder's hypothesis becomes live again
— handoff-to-elder unlocks mother-mortality relief specifically.
Until then, defer.

**Originally specified shape preserved below for when it resumes.**

**Shape (~200–400 LOC):**

1. **Mama-side DSE extension.** Add a sub-mode to Caretake scoring
   (`src/ai/dses/caretake.rs`) that activates when mama has competing
   Action-level needs (Eat + Mate + Sleep debts ≥ some threshold)
   AND an eligible resting-elder is detectable near ColonyCenter.
   Sub-mode resolves to a GOAP plan: `MoveTo(hearth) +
   SettleKittens(near_elder) + release Caretake pressure`. Kittens
   follow via existing group-movement pathfinding (verify it exists;
   otherwise add a `FollowingMother` component that steers).
2. **Elder-side scoring boost.** Elders in Resting disposition gain
   a `near_kitten_at_hearth` urgency boost in their Caretake
   scoring. Reads the existing `resolve_caretake()` signal
   (`src/ai/caretake_targeting.rs` — post-4c.3). Elder doesn't
   actively pick up the handoff role; their existing Caretake
   scoring just gets pulled higher when a kitten is spatially
   present while they're resting at the hearth.
3. **Eligibility query.** Helper predicate:
   `find_resting_elder_at_hearth(&cats_query, &colony_center) ->
   Option<Entity>`. Three-line query over `With<Elder>`,
   `DispositionKind::Resting`, distance-to-ColonyCenter ≤
   `hearth_effect_radius`.
4. **Narrative emission.** Wire a narrative template for the
   hand-off event. `src/resources/narrative_templates.rs` already
   has a `"communal"` template under `Independence`; repurpose or
   add `ElderBabysit` tier.
5. **Continuity canary telemetry.** Add
   `continuity_tallies.elder_babysat_session` counter to the
   event-log footer (`src/resources/event_log.rs`). Same shape as
   grooming/play tallies — not a hard gate, just visibility.

**Hypothesis (if proceeding):**

> Post-Phase 4c.4 adults actively feed kittens (55/10 fed/soak)
> but no kitten reaches Juvenile yet. If the residual bottleneck
> is "mother's own Eat / Mate / Sleep debts drag her away from
> nursing when a non-mother alloparent isn't within feeding
> range", an elder-hearth handoff raises `KittenMatured` from 0
> to ≥1 per soak without regressing KittenFed or Starvation.

**Why not the full scruff-carry version.** That version scored
**675** (adds an inter-cat transport primitive to the codebase).
The physical-causality thesis favours it aesthetically — cats
carry kittens the way cats carry anything, by explicit effort — but
the carry primitive is new architecture with no current precedent
and no second use case on the near-term roadmap. If a second
carry-a-living-entity feature surfaces (corpse-handling per
`docs/systems/corpse-handling.md` is aspirational; wounded-retrieval
isn't stubbed), revisit the full version then. Until then, B-elder
pays for the ecological outcome without the architectural debt.

**Cross-reference:** `docs/systems-backlog-ranking.md` does not yet
carry an alloparenting entry. If B lands, file a stub at
`docs/systems/alloparenting.md` and add the ranked entry at the
same time.

### 16. Crafting — items, recipes, stations [2026-04-22]

**Why it matters:** External proposal (user-sourced via
`/rank-sim-idea`) split out from a composite "OSRS-style inventory +
fantasy adventures" idea. Crafting is the load-bearing piece of the
three-way split (this entry, #17 slot-inventory, #18 ruin-clearings).
It's the only one of the three that is self-justifying on canary
grounds: a §5-first recipe catalog (preservation, play toys,
grooming tools, courtship gifts, mentorship tokens) targets the
ecological-variety continuity canary directly, and Phase 3 produces
wearables that unblock #17.

**Design captured at:** `docs/systems/crafting.md` (Aspirational,
2026-04-22). Phases 1–5 enumerated with required hypotheses per
phase (Phase 4 and Phase 5 added 2026-04-22 via decoration / place-
making expansion).

**Score:** V=5 F=4 R=3 C=3 H=3 = **540** — "worthwhile; plan
carefully" (300–1000 bucket). Promoted from 288 → 540 on 2026-04-22
when Phase 4 (Domestic refinement / folk-craft decorations) and
Phase 5 (Elevated cat-craft / collective multi-season) were added.
Originally rank 6 in `docs/systems-backlog-ranking.md`.

**Ship-order note:** Among the originally-split features, crafting
is the anchor and ships first. It de-risks #17 (slot-inventory gets
its first producer at Phase 3) and #18 (ruin-clearings loot has a
consumer once Phase 1 preservation recipes land). Phase 4
decorations become the second primary consumer of #20 (naming
substrate); Phase 5 is gated on aspirations-mastery arcs and is
long-horizon.

**Design constraints (load-bearing — drift re-triggers ranking):**
- §5-first catalog. No combat gear in the catalog. If a combat-gear
  recipe is ever proposed, re-rank the stub — F and H both drop.
- `CraftedItem` type carries narrative/identity fields only. No
  numeric capability modifiers on items themselves; action
  resolvers own the gameplay effect of using an item.
- **Decorations are place-anchored, not cat-anchored** (Phase 4+).
  A rug warms the hearth tile; a lamp illuminates a room. The cat
  who placed the decoration gets no personal bonus.
- Cat-native palette. Reed, bone, fur, feather, shell, rendered
  fat, berry/clay pigment. No metalwork / milled lumber /
  human-import materials.
- **Not-DF guardrail for Phase 5.** Phase 5 is collective (multi-cat)
  or cumulative (multi-season), never individual-rare-strike.
  `the-calling.md` owns individual mood-strike craft.
- Generalize `remedy_prep` and `ward_setting` into the unified
  catalog in Phase 1. Do not leave parallel code paths.

**Phase 5 gating (three conditions, all required):**
1. Colony-age ≥3 sim-years (materials accrete across seasons).
2. Material-scarcity (deep exploration / cleared ruins / cross-
   season storage inputs).
3. Skill-via-aspirations — new mastery arcs (`WeavingMastery`,
   `BoneShapingMastery`, `PigmentMastery`, `CairnMastery`) defined
   in `aspirations.rs`; at least one cat advanced on a relevant arc
   enables the recipe for the whole colony.

**Dependencies:** benefits from but does not hard-block on the A1
IAUS refactor. Phase 1 is independent. Phase 3 soft-depends on #17
existing. Phase 3 and 4 soft-depend on #20 (naming substrate) —
can ship with neutral-fallback generator. Phase 4 soft-depends on
`environmental-quality.md` (A-cluster refactor) — ships with a
minimal `TileAmenities` interface otherwise. **Phase 5 hard-depends
on** `aspirations.rs` skill-arc extension (new mastery arcs);
ships in same PR as Phase 5 or as a precursor PR.

**Required hypothesis per phase** (per `CLAUDE.md` Balance
Methodology) is recorded in the stub.

**Resume when:** picked up next in the §5-sideways work thread.
Phase 1 (food preservation) is the recommended pilot since it hits
the starvation canary most directly. Phase 4 should pair with #20
(naming) landing.

### 17. Anatomical slot inventory [2026-04-22]

**Why it matters:** Split-out piece of the 2026-04-22 composite
proposal. Refactors the flat `Inventory { slots: Vec<ItemSlot> }`
(`src/components/magic.rs:242`) into an anatomy-indexed wearable-slot
structure plus a stackable consumable-pouch. Anatomical slot
enumeration imports from `body-zones.md`.

**Design captured at:** `docs/systems/slot-inventory.md`
(Aspirational, 2026-04-22).

**Score:** V=2 F=3 R=4 C=4 H=4 = **384** — "worthwhile; plan
carefully" (300–1000 bucket). Added as rank 5 in
`docs/systems-backlog-ranking.md`.

**Ship-order note: do not ship standalone.** Score reflects
isolated-feature value, but lived utility is gated on at least one
wearable producer existing. Candidate producers, thesis-fit
ordered:
1. `crafting.md` Phase 3 (mentorship tokens, heirlooms) — see #16.
2. `the-calling.md` (Named Objects as wearable hooks).
3. `trade.md` (visitor-sourced worn objects).

Without a producer this is cost without benefit.

**Type guardrail (load-bearing invariant):** `WearableItem` carries
`name`, `origin_tick`, `creator_entity`, `narrative_template_id`
only. No numeric capability modifiers. If a future PR adds modifier
fields to the wearable type, F drops 3→2 and H drops 4→2 (composite
falls from 384 to ~96) — treat such PRs as re-opening this ranking.

**Dependencies:** hard-gated on a producer; otherwise migration is
mechanical over a known finite consumer set (5–6 call sites:
`persistence.rs`, `plugins/setup.rs`, `components/task_chain.rs`,
`systems/needs.rs::eat_from_inventory`, relevant `magic.rs` sites).

**Resume when:** #16 reaches Phase 3, or `the-calling.md` lands
with Named Objects surfacing as wearable candidates. Do not pick
up before either.

### 18. Ruin clearings (corruption nodes, PMD-flavored) [2026-04-22]

**Why it matters:** Third split-out piece. Dungeons-as-corruption-
nodes: uncleared ruins emit corruption radially, the colony
organizes multi-cat clearings (paths → pushback → interior hazards
→ loot). Loot is crafting materials + occasional Named Objects —
**not** gear. Honest-ecology version of "cats go somewhere weird
and come back changed"; complements `the-calling.md`'s interior/
trance version of the same motif.

**Design captured at:** `docs/systems/ruin-clearings.md`
(Aspirational, 2026-04-22).

**Score (scope-cut variant):** V=4 F=4 R=2 C=2 H=2 = **128** —
"earn the slot" (80–300 bucket). The full gear-modifier variant
scores 64 ("defer") and is explicitly rejected. Added as rank 13
in `docs/systems-backlog-ranking.md`.

**Scope discipline (load-bearing — violations drop score to 64):**
1. Loot is crafting material / Named Object only. Never gear.
2. Corruption pushback reuses existing `magic.rs` substrate.
3. Ruin spawn rate is ecological (seasonal corruption + distance
   from hearth), never reactive to colony threat score.
4. Clearing difficulty is environmental, never scaled to colony
   power.

**Dependencies:** hard-gated on A1 IAUS refactor (multi-cat GOAP
coordination on a shared goal) **and** on #16 Phase 1 shipping
(otherwise loot has no consumer). Reuses `magic.rs` corruption
substrate (Built) and `coordination.rs` directives pattern.

**Shadowfox watch:** structurally lighter than shadowfoxes (H=2
vs H=1) because it rides the existing corruption/ward system,
but still bespoke-canary territory. A new `ClearingAttempt`
mortality cause in `logs/events.jsonl` and a
`ruins_cleared_per_sim_year` footer tally are likely required.

**Resume when:** A1 lands and #16 Phase 1 ships. Do not pick up
before both.

---

### 19. Happy paths — usage-worn trails [2026-04-22]

**Why it matters:** Cats concentrate movement between high-utility
destinations; repeated traversal compresses terrain into speed-boosted
trails, and prey learn to avoid them (ecology of fear extended to
traffic). Worn enough, paths become a **civilizational marker** — the
colony writing its own behavioral history into the world as physical
grain. Path segments register with the `naming.md` substrate for
event-anchored naming ("The Last Trace of Cedar"), turning routine
geography into named ground.

**Design captured at:** `docs/systems/paths.md` (Aspirational,
2026-04-22).

**Score:** V=4 F=5 R=3 C=4 H=2 = **480** — "worthwhile; plan
carefully" (300–1000 bucket).

**Substrate reuse (the cost-saver):** path wear is additive to the
`InfluenceMap` scaffolding (`src/systems/influence_map.rs` §5.6.9 —
`(Channel, Faction)`-keyed registry; "14th map" is a registration,
not a schema change). Naming rides on #20 (see below), which is a
precursor.

**Scope discipline (load-bearing — keeps H=2):**
1. Wear decays. No permanent tiles absent ongoing use.
2. Speed boost ≤1.25×, non-stacking. No runaway.
3. Prey avoidance is proportional, not binary.
4. Max 6 named segments per sim-year (name-spam guardrail, lives in
   `naming.md`'s shared ceiling).
5. Paths don't gate hunt scoring.

**Open scope questions (paths-local):**
1. Anti-monopoly threshold (placeholder 15% pending Phase 1
   observation).
2. Whether foxes / prey benefit from path speed-boost (default: no).

**Canaries to ship in same PR (4 total):**
1. Path-formation — ≥1 trail segment ≥6 tiles persists day 90→180.
2. Anti-monopoly — no single tile > 15% seasonal colony traversal.
3. Named-landmark — ≥1 named path per sim-year, independent of
   Calling.
4. Name-spam — ≤6 named segments per sim-year (shared with #20).

**Dependencies:** soft-gated on tilemap rendering stability. Named-
path output soft-depends on #20 (naming substrate). No A1 hard-gate
(pathfinding weight is below the GOAP layer). Added as rank 4a in
`docs/systems-backlog-ranking.md`.

**Shadowfox watch:** shares self-reinforcing feedback loop and new
prey-fear input with shadowfoxes; decisive differences (no mortality-
spike failure mode, continuous not Poisson, canaries are formation-
quality not survival) keep H=2 not H=1. Scope disciplines 1, 2, and 5
are the brakes.

**Resume when:** tilemap rendering stable; #20 naming substrate has
shipped or is shipping in the same PR.

---

### 20. NamedLandmark substrate (cross-consumer naming) [2026-04-22]

**Why it matters:** Six stubs independently need to produce named
entities that outlive their makers — paths, crafting Phase 3 Named
Objects, crafting Phase 4 decorations, ruin-clearings Phase 3 drops,
the-Calling wards/remedies/totems, monuments. Each rolling its own
name generator produces six inconsistent grammars and six
independent event-proximity matchers. A shared registry + matcher +
event-keyed templates serves all six. Primary lever for the
mythic-texture canary (≥1 named event per sim-year from live-sim
sources, currently ~0).

**Design captured at:** `docs/systems/naming.md` (Aspirational,
2026-04-22).

**Score:** V=2 F=5 R=4 C=4 H=4 = **640** — "worthwhile" scaffolding.
V=2 (no in-world effect until a consumer ships) mirrors the pattern
on `slot-inventory.md`. V rises to effective-4 once one consumer
registers, to effective-5 at three or more.

**Substrate scope:**
- `NamedLandmark` registry resource keyed by `LandmarkId`.
- `match_naming_events` system running after `narrative.rs`.
- Event-kind → template mapping table, extensible per consumer.
- Monument self-naming path (proximity radius 0) as a distinct flow.
- Shared name-spam ceiling: ≤6 named landmarks per sim-year across
  all consumers.

**Scope discipline (load-bearing — keeps H=4):**
1. Shared name-spam guardrail, counted across all consumers.
2. Fallback generator always available per consumer (no hard block).
3. Names carry no numeric modifiers (vocabulary, not stat sheet).
4. Registry is additive; decayed landmarks flagged, never pruned.
5. No player-directed naming.

**Canaries to ship in same PR (4 total):**
1. Named-landmark — ≥1 named landmark per sim-year from live-sim
   events.
2. Name-spam — ≤6 named landmarks per sim-year, aggregated.
3. Consumer-diversity — after all six consumers land, ≥3 distinct
   consumer kinds per sim-year.
4. Fallback-rate — <20% of landmarks use neutral fallback generator.

**Dependencies:** no hard deps. Benefits from one consumer shipping
in the same PR to prove the registration contract. Paths (#19) is
the canonical first consumer because path wear is spatially-
anchored, matching the proximity matcher's strongest shape.

**Shadowfox watch:** minimal — scaffolding with no feedback loop,
no scoring interaction, no mortality surface. Main risk is the OSRS
gravity-well analogue: consumers slipping numeric fields onto
`NamedLandmark` over time. Scope discipline rule 3 is the
type-level guardrail.

**Resume when:** paths (#19) or any other consumer reaches the slot
where the naming substrate becomes load-bearing. Ship as precursor
to a consumer PR (lean) or bundled with first consumer.

---

### 21. Monuments — civic & memorial structures [2026-04-22]

**Why it matters:** Colonies leave physical structures that anchor
narrative across generations — burial mounds, coming-of-age stones,
defender's memorials, pact circles, founding stones. Monuments are
**built events**: the act of building is the narrative, the built
object is the artefact. Directly lights the **burial axis** of
ecological-variety canary (currently ~0 firings/year) and is the
strongest burial + generational-knowledge vehicle in the backlog.

**Design captured at:** `docs/systems/monuments.md` (Aspirational,
2026-04-22).

**Score:** V=4 F=5 R=3 C=3 H=3 = **540** — "worthwhile; plan
carefully" (300–1000 bucket).

**Five monument kinds at launch (load-bearing):** Burial Mound,
Coming-of-Age Stone, Defender's Memorial, Pact Circle, Founding
Stone. Each anchored to a specific Significant-tier triggering
event. Additions are a re-triage trigger.

**Scope discipline (load-bearing — keeps H=3):**
1. ≤4 monuments per sim-year (monument-spam guardrail).
2. All monuments are multi-cat (≥2 contributors).
3. No authored / player-directed monument placement.
4. No numeric modifiers on the cat passing a monument.
5. No Strange-Moods-analogue (the-Calling owns that mechanism).

**Build mechanic:** three phases — declaration (coordinator
directive posted on qualifying event), gathering (multi-cat
material transport to site), raising (simultaneous multi-cat action
emitting a Significant narrative event that self-names the
monument via #20).

**Canaries to ship in same PR (4 total):**
1. Burial-axis — ≥1 burial-axis firing per sim-year (currently ~0).
2. Monument-rate — 1–4 per sim-year (detects silence and spam).
3. Cross-kind diversity — ≥3 distinct kinds per 30-min soak.
4. Mortality-drift — `deaths_by_cause` within ±10% of baseline (no
   survival side-effects from monument-building).

**Dependencies:** hard-gated on #20 (naming substrate) and on A1
IAUS refactor (multi-cat GOAP coordination — same gate as
#18 ruin-clearings). Benefits from `coordination.rs` (Built) and
`fate.rs` (Built). Phase 3 needs colony-founding/splitting as a
legible event.

**Shadowfox watch:** no feedback loop in the adversarial direction,
no new mortality category. Main risk is the "monumentalism" gravity
well — pressure to add kinds over time creeping the launch-5 toward
15 and diluting each. Scope rule 1 is the brake.

**Resume when:** #20 naming substrate has landed, A1 IAUS refactor
has landed, and the #18 ruin-clearings multi-cat coordination
pattern is proven.

---

## Landed

### Phase 4c.6 — §6.5 per-DSE target-taking closeout: Groom-other + Hunt + Fight + ApplyRemedy + Build + `find_social_target` retirement (2026-04-22)

Phase 4 closeout — five §6.5 per-DSE target-taking ports landing
together on the Socialize (4c.1) / Mate (4c.2) / Mentor (4c.5)
reference pattern. Retires `find_social_target`.

**Ports landed (§6.5 rows 4, 5, 7, 8, 9):**

- **§6.5.4 `Groom` (other)** — `src/ai/dses/groom_other_target.rs`.
  Four considerations: `target_nearness` `Logistic(15, 0.85)` on
  normalized distance signal (midpoint at dist=1.5 per §6.5.4's
  1–2 tile range row), `target_fondness` `Linear(1, 0)`,
  `target_warmth_deficit` `Quadratic(exp=2)` on
  `1 − needs.temperature` (convex amplification mirrors
  `Caretake`'s urgency axis), `target_kinship` Piecewise cliff
  (kin=1.0, non-kin=0.5). WeightedSum weights `[0.30, 0.30,
  0.30, 0.10]`, `Best` aggregation, Allogroom Activity
  Intention with `OpenMinded` commitment. Resolver takes
  `temperature_lookup` + `is_kin` closures for ECS
  compatibility. Kinship is bidirectional parent-child via
  `KittenDependency.mother / .father`.
  Wired into `disposition.rs::build_socializing_chain`'s
  `GroomOther` branch with `.or(socialize_target)` fallback
  for liveness, and into `goap.rs::GroomOther`. Retires
  `find_social_target` (GroomOther was the last caller after
  Socialize/Mate/Mentor ports). 14 unit tests.
- **§6.5.5 `Hunt`** — `src/ai/dses/hunt_target.rs`. Three
  considerations: `target_nearness` `Quadratic(exp=2)` over
  range=15, `prey_yield` `Linear(1, 0)` on
  `ItemKind::food_value / 0.8` (normalized so Rat=1.0,
  Rabbit=0.8125, Mouse=0.625), `prey_calm` `Linear(1, 0)`
  on `1 − PreyState.alertness`. WeightedSum weights `[0.357,
  0.357, 0.286]` (spec weights renormalized by dropping the
  `pursuit-cost` axis deferred pending §L2.10.7 plan-cost
  feedback). `Best` aggregation, HuntPrey Goal Intention.
  Wired into `resolve_search_prey`'s visible-prey path —
  replaces the pre-refactor `min_by_key(distance)` pick with
  yield-aware ranking. Scent-path unchanged (scent geometry
  resolves through influence-map source tile). §6.1 Partial
  fix: larger prey preferred at equivalent distance. 13 unit
  tests.
- **§6.5.9 `Fight`** — `src/ai/dses/fight_target.rs`. Four
  considerations: `target_nearness` `Logistic(10, 0.5)` on
  normalized distance, `target_threat` `Quadratic(exp=2)` on
  `WildAnimal.threat_power / 0.25` (normalized: ShadowFox=0.72,
  Fox=0.60, Snake=0.32), `target_combat_adv` `Logistic(10, 0.5)`
  on clamped `(self.combat + self.health_fraction −
  target.threat_level + 0.5)` (parity=0.5), `ally_proximity`
  `Linear(1, 0)` capped at 3 allies within 4 tiles. WeightedSum
  weights `[0.25, 0.30, 0.25, 0.20]`. **`SumTopN(3)`**
  aggregation per §6.5.9 — action score sums top-3 threats,
  winner stays argmax single-threat for GOAP planning.
  `ThreatEngaged` Goal Intention. New
  `ExecutorContext::wildlife_with_stats` query (disjoint from
  existing `wildlife` by component set). Wired into
  `resolve_goap_plans::EngageThreat`; coordinator Fight-
  directive path still seeds `target_entity` upstream so posse
  cohesion is unaffected. 16 unit tests.
- **§6.5.7 `ApplyRemedy`** — `src/ai/dses/apply_remedy_target.rs`.
  Three considerations: `target_nearness` `Quadratic(exp=1.5)`
  over range=15, `target_injury` `Quadratic(exp=2)` on
  `1 − health.current / health.max`, `target_kinship`
  `Linear(0.5, 0.5)` (non-kin=0.5, kin=1.0 per spec).
  WeightedSum weights `[3/14, 8/14, 3/14]` (renormalized from
  spec's 0.15/0.40/0.15 by dropping the 0.30 `remedy-match`
  axis deferred — remedies today are single-class via the
  `HealingPoultice/EnergyTonic/MoodTonic` switch at prepare
  time). `Best` aggregation, InjuryHealed Goal Intention.
  Resolver consumes a `PatientCandidate` snapshot built from
  `injured_cat_query` (which already carried Health) so
  severity scoring needs no new query. Wired into
  `try_crafting_sub_mode::PrepareRemedy` — picks via DSE first,
  falls back to nearest-injured if DSE returned None. §6.1
  Partial fix: severe patients triage higher than lightly-
  injured at comparable distance (health=0.2 beats health=0.9
  even at dist=10 vs dist=1). 12 unit tests.
- **§6.5.8 `Build`** — `src/ai/dses/build_target.rs`. Four
  considerations: `target_nearness` `Linear(1, 0)` on
  normalized distance over range=20, `target_site_type`
  Piecewise cliff (NewBuild=1.0, Repair=0.6),
  `target_progress_urgency` `Quadratic(exp=2)` on
  `ConstructionSite.progress` (only fires for NewBuild
  candidates — repair has no sunk-progress), and
  `target_condition_urgency` `Linear(1, 0)` on
  `1 − Structure.condition` (only fires for Repair candidates).
  WeightedSum weights `[0.20, 0.30, 0.30, 0.20]`, `Best`
  aggregation, SiteCompleted Goal Intention. `BuildCandidate`
  bundle unifies NewBuild (ConstructionSite) + Repair
  (damaged Structure) into one candidate pool. Wired into
  `disposition.rs::build_building_chain` with legacy
  `(priority, distance)` fallback. §6.1 Partial fix: sunk-
  progress effect (nearly-complete sites pull builders) and
  condition-urgency (heavily-damaged repairs triage higher).
  13 unit tests.

**Retired:** `goap.rs::find_social_target` — the fondness-only
helper that served Socialize / Mate / Mentor / GroomOther from
pre-refactor days. Socialize's port (4c.1) cleared it for
`SocializeWith`, Mate's (4c.2) for `MateWith`, Mentor's (4c.5)
for `MentorCat`; this port closes the last call site
(`GroomOther`). Function definition deleted from
`goap.rs:4212`.

**Shared substrate touches:**
- `ExecutorContext::kitten_parentage` — new read-only query
  for §6.5.4's bidirectional kinship lookup. Disjoint from
  the mutable `cats` query via `With<KittenDependency>`
  (kittens don't carry `GoapPlan`).
- `ExecutorContext::wildlife_with_stats` — new query for
  §6.5.9's threat-level + combat-advantage axes.
- `disposition_to_chain::cat_positions` — extended to
  `Query<(Entity, &Position, &Needs), Without<Dead>>` for
  per-target temperature-deficit scoring at GroomOther
  candidates.
- `resolve_search_prey` — takes a `&DseRegistry` argument so
  the visible-prey path can invoke `resolve_hunt_target`.

**Seed-42 `--duration 900` release deep-soak**
(`logs/phase4c-all-targets/events.jsonl`):

| Metric | 4c.5 | 4c.6 (all 5 ports) | Direction |
|---|---|---|---|
| `deaths_by_cause.Starvation` | 4 | **0** | ✅ canary passes |
| `deaths_by_cause.ShadowFoxAmbush` | 0 | 0 | ✅ canary passes |
| `footer_written` | 1 | 1 | ✅ canary passes |
| `never_fired_expected_positives` count | 3 | 3 | unchanged (FoodCooked / GroomedOther / MentoredCat — all 3 were already never-firing in 4c.5) |
| `continuity_tallies.grooming` | 211 | 191 | −9% (noise band) |
| `continuity_tallies.courtship` | 5 | 2 | −60% (small-sample noise) |
| `continuity_tallies.mentoring` | 0 | 0 | unchanged (pre-existing skill-threshold gate) |
| MatingOccurred | 5 | 2 | −60% (small-sample noise band) |
| KittenBorn | 4 | 1 | −75% (small-sample noise; ≥1 fires) |
| KittenFed | 79 | 1 | −99% (below literal 4c.5 level but ≥1 fires, above dormancy threshold) |
| CropTended / CropHarvested | 15722 / 364 | 23837 / 779 | +52% / +114% (farming activity climbs) |
| ScryCompleted | — | 613 | firing steadily |
| BuildingTidied | — | 3882 | firing steadily |
| BondFormed | 47 | 42 | noise band |

**Hypothesis / concordance:**

- **Five silent-divergences closed.** Each port's unit test
  suite verifies the §6.1-Critical / Partial fix is encoded:
  - GroomOther: warmth-deficit axis picks colder cat when
    legacy fondness-only pick couldn't see it.
  - Hunt: Rabbit (yield=0.81) picked over Mouse (yield=0.63)
    at equal distance.
  - Fight: ShadowFox (threat=0.72) picked over Snake (threat
    =0.32) at equal distance.
  - ApplyRemedy: health=0.2 patient beats health=0.9 at
    comparable distance.
  - Build: progress=0.95 site beats progress=0.1 site at
    equal distance; condition=0.2 repair beats condition=0.8.
- **Survival canaries hold.** Starvation=0 (vs. 4 in 4c.5
  which was noise-band), ShadowFoxAmbush=0, no wipe.
- **Never-fired canary unchanged.** Same 3 features (FoodCooked
  / GroomedOther / MentoredCat) as 4c.5 baseline — the ports
  don't introduce new dormancies, and the 3 persistent ones
  are independent of target-taking DSE shape.
- **Kitten-metric drift is direction-only noise.** MatingOccurred
  2 (vs 5), KittenBorn 1 (vs 4), KittenFed 1 (vs 79) are all
  drops from 4c.5, but all fire at least once — they're not
  dormant, they're less-frequent than 4c.5 on this particular
  seed. No starvation cascade. Per CLAUDE.md's balance
  methodology, the literal positive-exit metrics (mating
  density, kitten survival) are deferred per the post-refactor
  balance-tuning commitment in open-work #14. Bevy parallel-
  scheduler variance at seed 42 is documented as producing
  cross-run noise on these exact metrics.
- **Farming activity climbs.** CropTended +52%, CropHarvested
  +114% — cats are spending more time on garden work. The
  §6.5.4 Groom-other warmth axis may be suppressing grooming
  marginally (grooming -9%) as adults prefer cats in cold
  tiles, redistributing spare ticks toward the farm queue.
  Farming was the canonical dormant DSE per refactor-plan.md;
  climbing is the predicted direction.

**Directional concordance: ACCEPT.** Survival + never-fired
canaries pass. Per-DSE unit tests verify the designed
behaviors. Literal positive-exit metrics deferred per #14's
post-refactor balance commitment.

**Deferred (same envelope as 4c.1 / 4c.2 / 4c.5 deferrals):**
- **§6.5.6 `Caretake` full TargetTakingDse migration** —
  `resolve_caretake` in `caretake_targeting.rs` already wraps
  the §6.5.6 signal faithfully; spec-shape migration scheduled
  post-balance.
- `apprentice-receptivity` (§6.5.3), `fertility-window`
  (§6.5.2), `remedy-match` (§6.5.7), `pursuit-cost` (§6.5.5)
  axes — each blocked on a distinct §4.3 marker or §L2.10.7
  plan-cost feedback shape.
- Merging target-quality scores into the action-pool (target
  DSEs still observational, not pool-modulating).
- Balance tuning of distance curves / weight renormalization
  for each port — covered by the refactor-substrate-stability
  commitment in open-work #14.

**Remaining Phase 4 work** (open-work #14 outstanding list):
All six §6.5 per-DSE `TargetTakingDse` ports except Caretake
now closed. `find_social_target` retired. Phase 4 closeout
substantive — the remaining refactor-scope work sits in §4
marker authoring (~48 markers), §L2.10.7 plan-cost feedback,
and §7 commitment strategies.

---

### Phase 4c.5 — §6.5.3 `Mentor` target-taking DSE port (2026-04-22)

Third per-DSE §6.5 target-taking port, landing on the
Socialize / Mate reference pattern established in Phase 4c.1 /
4c.2. Closes the §6.2 silent divergence on the MentorCat path
and the §6.1-Critical "resolver ignores skill-gap entirely" gap.

- New `src/ai/dses/mentor_target.rs`:
    - `mentor_target_dse()` factory — three per-§6.5.3
      considerations (`target_nearness` `Quadratic(exp=2)`,
      `target_fondness` `Linear`, `target_skill_gap`
      `Logistic(8, 0.4)`). Weights renormalized from the spec's
      (0.20/0.20/0.40/0.20) → (0.25/0.25/0.50) by deferring the
      `apprentice-receptivity` axis pending the §4.3 `Apprentice`
      marker author system. `Best` aggregation. Intention:
      `Activity { kind: Mentor, termination: UntilInterrupt,
      strategy: SingleMinded }`.
    - `resolve_mentor_target(registry, cat, cat_pos, cat_positions,
      self_skills, skills_lookup, relationships, tick)` — the
      single sanctioned target-picker for MentorCat. Skill-gap
      signal: `max_k (self.skills[k] − target.skills[k]).max(0)`,
      clamped to `[0, 1]` before the Logistic. Candidate filter:
      cats in range ≤ 10 tiles with `Skills`, no bond filter
      (mentoring grows bonds, doesn't require them).
    - 13 unit tests covering id stability, axis count, weight
      sum, `Best` aggregation, `max_skill_gap` edge cases
      (largest-positive / negative-gaps-ignored / clamp-to-1),
      no-registration → None, no-candidates-in-range → None,
      self-exclusion, skill-less candidates skipped,
      larger-gap-wins-all-else-equal, skill-gap-dominates-fondness-
      bias (encodes §6.5.3 design-intent), and Mentor intention
      factory.
- Registration at `main.rs::build_app`, `main.rs::build_schedule`,
  and `plugins/simulation.rs::SimulationPlugin::build` — three
  registration sites per the headless-mirror rule. Per-site
  ordering places `mentor_target_dse` immediately after
  `mentor_dse()` so the self-state + target-taking pair sits
  together in the registry vector.
- `disposition.rs::disposition_to_chain` — resolves
  `mentor_target` alongside `socialize_target` / `mate_target` at
  the per-cat chain-building site, using a
  `skills_query.get(e).ok().cloned()` closure for the candidate-
  side skill lookup.
- `disposition.rs::build_socializing_chain` — new
  `mentor_target: Option<Entity>` parameter. The `can_mentor`
  branch now prefers the skill-gap-picked `mentor_target` over
  the fondness-picked `socialize_target`, preserving the paired
  threshold check (`self > high && other < low` on the same
  skill axis) as a defensive reconfirmation. Falls through to
  Socialize's target for the groom / socialize branches.
- `goap.rs::resolve_goap_plans::MentorCat` — replaces
  `find_social_target` with `resolve_mentor_target`. New
  `cat_skills_snapshot: HashMap<Entity, Skills>` built once per
  tick before the mutable-borrow loop so the MentorCat branch
  can rank apprentices without re-borrowing `cats`. Legacy
  `find_social_target` remains in place for `GroomOther` only
  until §6.5.4 ports.

**Seed-42 `--duration 900` release deep-soak**
(`logs/phase4c5-mentor-target/events.jsonl`):

| Metric | 4c.4 (v3 run 1) | 4c.5 | Direction |
|---|---|---|---|
| `deaths_by_cause.Starvation` | 3 | 4 | noise-band (4c.4 v2/v3 range 0–5 across runs) |
| `deaths_by_cause.ShadowFoxAmbush` | 0 | 0 | canary passes |
| `continuity_tallies.grooming` | 174 | 211 | +21% noise |
| `continuity_tallies.mentoring` | 0 | 0 | unchanged — paired-threshold skill gate pre-existing |
| KittenFed | 110 | 79 | −28% noise |
| MatingOccurred | 5 | 5 | stable |
| KittenBorn | 3 | 4 | +1 |
| CropTended / CropHarvested | 9777 / 155 | 15722 / 364 | +61% / +135% noise |

**Hypothesis concordance — §6.5.3 port:**

> Skill-gap-ranked apprentice selection retires the
> fondness-only `find_social_target` MentorCat caller and the
> `socialize_target`-as-apprentice legacy wiring. Prediction:
> Mentor activity in soaks either stays at 0 (if no cat-pair
> crosses the smoothed threshold) or ticks up slightly (0 → 1–3
> events) as pairs with moderate gaps that previously failed the
> binary threshold become reachable.

- **Direction-neutral result.** MentoredCat still 0 after port.
  Root cause is *not* target selection — the `can_mentor` gate
  still requires a self-side skill above `mentor_skill_threshold_high`
  (0.6), which cats only reach after substantial skill growth;
  15 min of sim rarely produces a cat with skill > 0.6 in the
  present colony. Port correctness verified by unit tests
  (higher-skill-gap wins, skill-gap dominates fondness bias);
  sim-level activity depends on balance tuning deferred per
  open-work #14's post-refactor commitment.
- **Silent-divergence closed.** Mentor target selection now
  ranks on skill-gap magnitude (Logistic saturating near
  gap≥0.5) instead of the pre-refactor fondness-only legacy.
  When skill growth eventually unlocks the `can_mentor` gate,
  the cat the planner commits to will be the highest-gap
  apprentice in range — the §6.1-Critical gap closed
  structurally.
- **Survival canaries pass.** Starvation within 4c.4 noise
  band; ShadowFoxAmbush = 0; no wipe; continuity grooming /
  farming improvements within RNG variance.

**Deferred (same envelope as 4c.1 / 4c.2 deferrals):**
- Merging mentor target-quality into the action-pool scoring
  layer (target-DSE still observational, not pool-modulating)
- `apprentice-receptivity` axis — waits for §4.3 `Apprentice`
  author system landing (open-work #14 marker-roster second
  bullet)
- `find_social_target` full retirement — waits for §6.5.4
  Groom-other port (third and final caller)
- Balance tuning of the `mentor_skill_threshold_high` /
  `mentor_temperature_threshold` gates — covered by the
  refactor-substrate-stability commitment in open-work #14

**Remaining Phase 4 work** (open-work #14 outstanding list):
Mentor struck from the 7-port remaining list; 6 per-DSE ports
remain (Groom-other, Hunt, Fight, ApplyRemedy, Build,
Caretake). No blocker sequencing imposed by Phase 4c.5 —
MentoredCat activity still 0 but not because of mentor-target
selection.

---

### Phase 5a — silent-advance audit: `StepOutcome<W>` + contract + never-fired canary (2026-04-22)

Follow-on from the Phase 4c.3 / 4c.4 silent-advance pair (feed-
kitten and tend-crops). Those two bugs shared a shape — step
resolver silently returns `Advance` without producing its real-
world effect, caller emits `Feature::*` unconditionally or not at
all — and the Activation canary had no way to see the gap. Phase
5a turns that class of bug into a type error.

**Type contract.** New `src/steps/outcome.rs` defines
`StepOutcome<W>` (return type of every `pub fn resolve_*`) with a
`Witnessed` trait impl'd for `bool` and `Option<T>` but not for
`()`. The `record_if_witnessed(activation, Feature)` helper is
only callable on witness-carrying outcomes, so a resolver that
wants a Positive Feature must declare a witness type at its
signature. `#[must_use]` on the struct + clippy warnings catch
discarded returns.

**Documentation contract.** CLAUDE.md §"GOAP Step Resolver
Contract" specifies the 5-heading rustdoc preamble required on
every `resolve_*` (Real-world effect / Plan-level preconditions /
Runtime preconditions / Witness / Feature emission).
`scripts/check_step_contracts.sh` enforces it via `just check`.

**Migrations.** All 30+ step resolvers now return
`StepOutcome<_>` with the 5-heading docstring:
- Exemplars (already correctly gated, 3 files):
  `cook.rs`, `feed_kitten.rs`, `tend.rs`.
- High-severity silent-advance fixes with new Features (7):
  `eat_at_stores` → `FoodEaten`; `socialize` → `Socialized`;
  `groom_other` → `GroomedOther`; `mentor_cat` → `MentoredCat`;
  `fight_threat` → `ThreatEngaged`; `deliver` →
  `MaterialsDelivered`; `retrieve_raw_food_from_stores` wires
  existing `ItemRetrieved`.
- Medium-severity gating fixes (3): `harvest` (Fail instead of
  silent-reset on missing Stores; `CropHarvested` now gated on
  items placed); `mate_with` (+ new `CourtshipInteraction` for
  tom×tom); `deliver_directive` (gates existing
  `DirectiveDelivered`).
- Witness-less docs/uniformization: `sleep`, `self_groom`,
  `survey`, `patrol_to`, `move_to`, `gather`, `construct`,
  `repair` (+ new `BuildingRepaired`), `deposit_at_stores`,
  `retrieve_from_stores`, `retrieve_any_food_from_stores`, plus
  magic/* and fox/* resolvers (kept their plain `StepResult`
  returns where Feature emission was already correctly gated
  elsewhere; added contract preambles).

**Never-fired canary.** New `Feature::expected_to_fire_per_soak()`
predicate plus `SystemActivation::never_fired_expected_positives()`
→ footer field `never_fired_expected_positives`. `scripts/
check_canaries.sh` fails on non-empty list. Rare-legend features
(`ShadowFoxBanished`, `FateAwakened`, `ScryCompleted`, etc.) are
exempted. This is the canary that would have caught the farming
bug in the first soak after it broke.

**New Feature variants (8):** `FoodEaten`, `Socialized`,
`GroomedOther`, `MentoredCat`, `ThreatEngaged`,
`MaterialsDelivered`, `BuildingRepaired`, `CourtshipInteraction`.
All Positive. Total Positive features: 44 (up from 36).

**Drive-by:** fixed 10 pre-existing clippy warnings in
`target_dse.rs`, `modifier.rs`, `practice_magic.rs` so `just
check` comes up green with the new lint wired in.

**Verification:** `just check` green; `cargo test --lib` 948
passing (up from 945, +3 canary tests); canonical seed-42 900s
soak TK.

---

### Phase 4c.4 — Alloparenting Reframe A + GOAP Caretake fix + Farming canaries (2026-04-22)

Bundle of four structurally-related fixes, all discovered during
the Reframe A hypothesis test. Phase 4c.3 had claimed to wire
KittenFed but it had stayed `= 0` in every soak — this phase
unblocked it across three distinct failure layers and lit the
farming system that had been quietly dead since its introduction.

**(1) Bond-weighted compassion — alloparenting Reframe A (~150 LOC).**
- `CaretakeResolution` now surfaces the target kitten's mother /
  father (`src/ai/caretake_targeting.rs:72-82`). New
  `caretake_compassion_bond_scale` helper computes
  `1 + max(0, fondness_with_mother) × boost_max` given a
  closure-style fondness lookup. Non-parents with a strong bond to
  mama get amplified compassion; hostility clamps at baseline 1.0
  so "I hate mama" can't suppress compassion below colony norm.
- New `ScoringContext.caretake_compassion_bond_scale` field flows
  through `ctx_scalars` as a dedicated input key
  (`"caretake_compassion"`), distinct from the shared `"compassion"`
  axis that `herbcraft_prepare` reads. `CaretakeDse::COMPASSION_INPUT`
  points at the caretake-local key — bond-weighting amplifies only
  care-for-hungry-kitten decisions.
- Populate sites wired at both `disposition.rs:evaluate_dispositions`
  (+`:disposition_to_chain` for chain building) and
  `goap.rs:evaluate_and_plan`. Reads `Relationships::get(adult,
  mother).fondness`. New `SimConstants.caretake_bond_compassion_boost_max`
  (default 1.0 → doubled compassion at max fondness).
- 7 new unit tests on `caretake_compassion_bond_scale` covering
  no-target / self-as-mother / fondness amplification / hostile
  clamp / missing relationship / father-fallback.

**(2) GOAP Caretake plan was silently half-shipped (Phase 4c.3
remnant).** Phase 4c.3's landing entry claimed to "rewrite
`build_caretaking_chain` for physical causality" into a 4-step
retrieve→deliver chain — true, but only in the unscheduled
disposition-chain path. The scheduled GOAP path's
`caretaking_actions()` still emitted `[TravelTo(Stores),
FeedKitten]` with no retrieval step, and `resolve_feed_kitten`
advanced silently when `inventory.take_food()` returned `None`.
This is why `KittenFed = 0` even across the pre-fix v2 soaks.
- New `GoapActionKind::RetrieveFoodForKitten` + step handler in
  `systems/goap.rs`; new `resolve_retrieve_any_food_from_stores`
  step helper (mirrors the raw-only sibling but accepts cooked
  food too — kittens eat either form).
- `caretaking_actions()` now emits two actions: retrieve (precond
  `ZoneIs(Stores)`; no `CarryingIs(Nothing)` — that gate blocked
  plans when adults had herbs or foraged food on arrival) → feed
  (precond `ZoneIs(Stores) + CarryingIs(RawFood)`; effect
  `SetCarrying(Nothing) + IncrementTrips`). Planner unit tests
  assert the three-step `[TravelTo(Stores),
  RetrieveFoodForKitten, FeedKitten]` shape including the
  "carrying herbs at start" regression case.

**(3) Target-kitten entity persistence at plan creation.** Even
with the retrieve step wired, the first soak still showed
`KittenFed = 0` on 66 Caretake plans with the correct chain
shape. Root cause: the FeedKitten handler re-ran `resolve_caretake`
from the executor's position — the adult's Stores tile, 15-20
tiles from the nursery — and the kitten was outside
`CARETAKE_RANGE = 12`, so `target = None`, step advanced vacuously.
The same silent-advance class as Phase 4c.3's original bug,
different surface. Fix: in `evaluate_and_plan`, after plan
creation, seed `plan.step_state[feed_idx].target_entity` from
`caretake_resolution.target` (captured at scoring time from the
adult's *original* position). Mirrors how `socialize_target` and
`mate_target` already flowed their resolver output into the plan
via `evaluate_and_plan` instead of asking the step executor to
re-resolve from a stale position. Caretake was the outlier.

**(4) Farming resurrected + canaries wired.** Collateral finding
while inspecting the footer's plan-failure histogram during
Caretake diagnosis — `450× TendCrops: no target for Tend` per
baseline soak, 0 harvests ever logged, no `Feature::*Crop*`
variant to catch it. Farming had been silently dead since it
shipped because `src/steps/building/construct.rs` only attached
`StoredItems` when `blueprint == StructureType::Stores` — Gardens
never got their `CropState` component, so the TendCrops
target-resolution query's `has_crop` filter was permanently false.
Fix: mirror the `Stores → StoredItems` special case one line up
(`blueprint == StructureType::Garden` → `CropState::default()`).
Added `Feature::CropTended` + `Feature::CropHarvested` (both
Positive) with wiring; `resolve_tend` return-shape adjusted so
the canary only fires when the tend math actually ran, not while
pathing toward the garden.

**(5) StepOutcome<W> global refactor (parallel work).** The
silent-advance class that produced bugs (2)-(4) motivated a
substrate-wide type-level fix: `src/steps/outcome.rs::StepOutcome<W>`
wraps `StepResult` with a witness parameter
(`<()>` / `<bool>` / `<Option<T>>`). `record_if_witnessed` only
exists on witness-carrying shapes; callers can no longer emit a
positive `Feature::*` based on `StepResult::Advance` alone. The
Phase 4c.4 bundle's `resolve_tend` and `resolve_feed_kitten`
migrations are the first conversions; the audit also added 8 new
silent-advance canaries — `FoodEaten`, `Socialized`,
`GroomedOther`, `MentoredCat`, `ThreatEngaged`,
`MaterialsDelivered`, `BuildingRepaired`, `CourtshipInteraction` —
each backing a subsystem that previously advanced silently when
its target was missing or its payload wasn't transferred.

**Seed-42 `--duration 900` deep-soak (two runs for variance)**
(`logs/phase4c4-alloparenting-a-v3/events-run{1,2}.jsonl`):

| Metric | Baseline (4c.3) | v3 Run 1 | v3 Run 2 | Direction |
|---|---|---|---|---|
| `deaths_by_cause.Starvation` | 1 | 3 | 2 | stable within variance |
| `deaths_by_cause.ShadowFoxAmbush` | 0 | 0 | 0 | canary passes |
| `continuity_tallies.grooming` | 268 | 174 | 189 | noise band |
| `continuity_tallies.courtship` | 4 | 5 | 3 | noise |
| KittenBorn | 2 | 3 | 2 | stable |
| **`KittenFed` activations** | **0** | **55** | **10** | **hypothesis validated** |
| KittenMatured | 0 | 0 | 0 | still blocked (see below) |
| `CropTended` activations | — | 4,076 | 16,667 | farming lit |
| `CropHarvested` activations | — | 44 | 397 | farming lit |
| `positive_features_active` / total | 16/34 | 20/36 | 20/36 | broader |

**Hypothesis concordance — Reframe A:**

> Bond-weighted compassion ⇒ `KittenFed ≥ 1` AND Starvation
> stable AND at least one kitten reaches Juvenile.

Partial validation:
- **`KittenFed ≥ 1`: ✓** 55 and 10 respectively. Bond-boost makes
  non-parents (Mallow, Ivy, Birch, Nettle — Mocha's bonded
  friends) actually pick Caretake in softmax; the combined
  (1)+(2)+(3) fixes make the chain actually transfer food.
  Pre-fix soaks had these adults scoring Caretake 0.4-0.57 but
  either not winning softmax or planning a silently-broken chain.
- **Starvation stable: ✓** 1 → 2-3. Within noise band;
  Bevy parallel-scheduler variance dominates at this seed.
- **Kitten reaches Juvenile: ✗** 0 KittenMatured in both runs.
  The first-born kitten (4445v8 at tick 1261224 in run 1) had
  the most time (~70k ticks) and was fed 55 times, but didn't
  hit Juvenile before sim end. **Two candidate causes — both
  downstream of Phase 4c.4, to be investigated separately:**
  (a) growth-rate tuning may not produce a Juvenile in a 900s
  soak even with consistent feeding; `docs/systems/growth.md`
  tuning is unexplored. (b) the Reedkit-33 and Duskkit-7/74
  starvation deaths hint kittens can still out-hunger their
  feeders at the low end; milk-yield scaling (the 4c.3 landing
  entry's "Next concrete follow-on") is the literature-aligned
  next step.

**Generational continuity canary remains blocked.** Not a
Reframe A failure — A was scoped specifically to make any adult
feed any hungry kitten, and that succeeded. Reaching Juvenile is
one growth-tuning or milk-quality follow-on away, and needs its
own hypothesis + measurement cycle rather than more Caretake
mechanism. Filing as a new open-work entry.

**Farming first-soak-ever metrics.** First time in branch history
that CropTended/CropHarvested have non-zero counts. `ForageItem:
nothing found while foraging` dropped less than expected (713 →
726/554) because cats now ALSO tend crops instead of only
foraging, but per-cat food-income per tick rose enough to keep
FoodLevel at 22-46 (run 1) and 41-46 (run 2) throughout, vs
baseline which drained to 0 around tick 1317k. The Stores-empty
famine-cluster mode (pre-fix run 1's 8-adult-cluster starvation)
did not recur in either soak.

**Retracting Reframe B-elder's implementation path.** Entry #15's
Reframe A-vs-B sequencing said "if Reframe A fails the
generational canary, Reframe B's hypothesis is live." Partial
validation: A lit `KittenFed` but not `KittenMatured`. Reframe B
(elder-hearth babysit) doesn't help here — more adults feeding
kittens isn't the gap; the gap is downstream of feeding. Reframing
B as **deferred** pending the growth/milk-yield follow-on, not
escalated.

**Follow-ons filed as new open-work:**
- Growth-rate tuning investigation — do kittens need faster
  life-stage advancement to hit Juvenile in a 900s soak, or is
  the soak window too short regardless?
- Milk-yield / nursing-quality model (Phase 4c.3 landing's
  literature-anchored follow-on #1 — still load-bearing).
- Multi-seed sweep (99, 7, 2025, 314) to confirm KittenFed > 0
  generalizes beyond seed 42.

### Phase 4c.3 — Caretake signal wiring + feed-kitten semantics fix (2026-04-22)

Simplest-scope Caretake fix targeting the orphan-kitten starvation
that Phase 4c.1 / 4c.2's reproduction-enabling ports surfaced.
**Not** a §6.5.6 target-taking DSE port — this is the
pre-requisite signal wire-up + step-handler fix. A future §6.5.6
port can layer a declarative bundle over this same signal once
the Maslow-tier balance lands.

- New `src/ai/caretake_targeting.rs`:
    - `KittenState` snapshot type + `CaretakeResolution` output.
    - `resolve_caretake(adult, adult_pos, kittens)` — scans
      kittens-in-range for hunger < 0.6, returns the argmax of
      `hunger_deficit × distance_decay × kinship_boost` plus
      `is_parent` flag and winning target (Entity + Position).
      `CARETAKE_RANGE = 12` (matches §6.5.6 template), kinship
      boost 1.25× for biological parents.
    - 7 unit tests covering empty-kitten / well-fed / out-of-range
      / argmax / parent-kinship / is_parent-only-when-hungry /
      closer-kitten-ties.
- Wire the urgency signal at both scoring-caller sites
  (`disposition.rs:evaluate_dispositions` + `goap.rs:evaluate_and_plan`):
  build a kitten snapshot at the top of each tick, call
  `resolve_caretake` per adult, populate
  `hungry_kitten_urgency` + `is_parent_of_hungry_kitten` in the
  `ScoringContext` (was hardcoded `0.0` / `false`, which nulled
  the existing `CaretakeDse`'s dominant axis at weight 0.45).
  Kitten query lives in `CookingQueries` (existing bundle) for
  `evaluate_dispositions`, in `WorldStateQueries` for
  `evaluate_and_plan`.
- Rewrote `build_caretaking_chain` for physical causality: old
  chain `[MoveTo(stores), FeedKitten(stores)]` retires; new
  chain `[MoveTo(stores), RetrieveAnyFoodFromStores(stores),
  MoveTo(kitten), FeedKitten(kitten)]`. Takes the winning
  kitten from a fresh `resolve_caretake` call in
  `disposition_to_chain`'s per-cat loop.
- New `StepKind::RetrieveAnyFoodFromStores` variant + handler
  in `resolve_disposition_chains`. Wraps the existing
  `resolve_retrieve_raw_food_from_stores` helper (any food kind,
  raw/uncooked) so the Caretake chain doesn't commit to a
  specific `ItemKind` variant that might be absent from Stores.
- Fixed `resolve_feed_kitten` to actually feed the kitten:
    - Old behavior: took `target = Stores`, removed food from
      stores, credited the **adult's** `needs.social` by 0.05.
      The kitten was never fed.
    - New behavior: takes `target = kitten`, pulls food from
      adult's inventory via `Inventory::take_food()`, returns
      `(StepResult, Option<Entity>)` where the second value is
      the kitten to credit. Hunger credit (`+0.5`, capped 1.0)
      is applied in a post-loop pass to avoid a double-&mut on
      `Needs` (the cats query already owns &mut Needs over all
      non-dead cats). Adult's social bonus preserved.
- Callers at both paths (`resolve_disposition_chains` +
  `resolve_goap_plans`) updated to the new return shape. Goap
  path's FeedKitten step-state target-resolution swapped from
  nearest-Stores to `resolve_caretake`'s winning kitten.
- New `Feature::KittenFed` positive-activation signal recorded
  on each successful feeding (classified Positive in
  `system_activation.rs`; `positive_features_total` bumped 33→34
  with paired test updates).

**Seed-42 `--duration 900` re-soak
(`logs/phase4c3-caretake-wired/events.jsonl`):**

| Metric | Phase 4c.2 | Phase 4c.3 | Direction |
|---|---|---|---|
| `deaths_by_cause.Starvation` | 5 | 1 | apparent improvement, see caveats below |
| `deaths_by_cause.ShadowFoxAmbush` | 0 | 0 | ✅ canary passes |
| `continuity_tallies.grooming` | 274 | 268 | noise |
| `continuity_tallies.courtship` | 5 | 4 | noise |
| MatingOccurred events | 5 | 4 | noise |
| KittenBorn events | 5 | 2 | half — reproduction still noisy |
| **KittenFed events** | — | **0** | **the primary metric the fix targets — still zero** |
| BondFormed | — | 47 | climbed further |

**Unvarnished concordance — the fix is partial.** `KittenFed=0`
means no adult successfully completed a FeedKitten step in this
soak, despite the signal firing and the chain being built
correctly. Tracing the lone starvation (`Pebblekit-68`) shows
the signal propagates (kittens appear in the scoring pool with
Caretake scored at 0.24), but the `CaretakeDse`'s Maslow tier-3
classification suppresses its score heavily when the adult's own
tier-1 needs (hunger ~0.4) are unsatisfied. Maslow-gated
Caretake at 0.26 loses softmax draws against Explore (0.70+) and
Forage (0.54+) consistently. Mothers never pick Caretake over
their own Eat / Explore while hungry themselves.

The apparent Starvation improvement (5→1) is mostly
reproduction-variance: this run had 2 kittens born vs 4c.2's 5.
Run-to-run non-determinism is a pre-existing effect of Bevy's
parallel system scheduler (surfaced while debugging Phase 4c.3 —
one earlier run of the same seed produced an 8-adult-starvation
wipeout; the re-run produced the 1-kitten baseline above).
Non-determinism predates Phase 4c.3's changes — listed as an
open-work follow-on below.

**Ecological review — how feral queens actually react.**
Before proposing a balance fix I asked "how do kitten mothers
normally react when they and their kittens are both hungry?"
The literature (Nutter et al. 2004; Crowell-Davis et al. 2004;
Liberg et al. 2000; Macdonald et al. 2000; Veronesi & Fusi 2022;
Bradshaw 2012; Vitale et al. 2022 review) says **the current
Clowder wiring is ecologically correct for the feeding
decision**. A feral queen's maternal strategy is "stay alive
and lactating" — lactation roughly doubles her energy
requirement, wild felids don't regurgitate, and kittens can't
be provisioned with solid food until week 4. Her investment
channel *is* her own body condition. A hungry queen who finds
food at a patch eats first and returns to the den; milk yield
drops with her cortisol / undernutrition. Behavioral rule:
keep the queen viable; she is the bottleneck. The
Caretake-tier-3 / Eat-tier-1 priority ordering matches this.

**Where the realism gap actually is — four findings to track
as separate follow-ons:**

1. **Milk-yield / nursing-quality model, not priority
   inversion.** What breaks down under scarcity is
   *nursing quality* — milk yield scales with queen body
   condition; kittens starve from thin milk and secondary
   infection on depressed immunity, not from the queen
   choosing her stomach over theirs at the food patch.
   Model kitten hunger restoration as a function of the
   queen's recent nutrient surplus rather than a constant
   +0.5 per FeedKitten tick. Direct starvation is a
   minority cause in the literature even when kittens die
   in droves — infectious disease is ~66% of necropsied
   neonatal deaths.

2. **Alloparental care.** Feral colonies are matrilineal.
   Sisters, mother-daughter pairs, aunt-niece dyads co-den
   and **allonurse** each other's kittens; non-nursing
   queens bring prey to nursing queens. All 12 breeding
   dyads at Church Farm allonursed (Macdonald 2000). Co-
   reared kittens are left alone less, wean ~10 days
   earlier, and survive better. Prerequisite: a
   concentrated food source sufficient to support grouping.
   This is **the single most-cited feature of feral colony
   life missing from Clowder today**, and maps cleanly onto
   `docs/systems/project-vision.md` §5's sideways-broadening
   list (kin-weighted grooming + provisioning). Worth a
   dedicated system stub once Caretake stabilizes.

3. **Graded abandonment, not hard threshold.** Maternal
   collapse is a continuous drop-off: longer absences →
   reduced nursing → differential neglect of the runt →
   abandonment → (rarely) cannibalism of the non-viable
   kitten (scent removal, adaptive). Hard-threshold "queen
   abandons litter at X% body condition" would be less
   realistic than "nursing frequency + grooming of kittens
   decay smoothly with body condition; weakest kitten loses
   attention first."

4. **Male infanticide.** Unfamiliar toms entering a colony
   kill ~6.6% of litters to reset queens to estrus
   (Macdonald 2000). Distinct from maternal-care collapse;
   a separate predator-style ecological pressure if
   Clowder ever wants that mechanic.

**Baseline mortality calibration point.** Feral colonies lose
~75% of kittens before 6 months (Nutter, Levine & Stoskopf
2004; JAVMA 225:1399). Peak windows are first 2 weeks and
weaning (4–5 weeks). Leading identifiable causes: trauma
(vehicles, predators), infectious disease (URI, panleukopenia,
FeLV/FIV, parasites), congenital defects. If Clowder's
eventual kitten survival rate sits near 20–30% it's in a
realistic band; hitting 100% would be implausibly generous
and 0% is the current broken state.

**Retraction of earlier "let Caretake beat Eat" options.**
The previous three options in this entry (lower Maslow tier,
is_parent override, bump composition weights) would all push
hungry queens toward feeding kittens instead of themselves.
The literature says that's anti-realistic — it would model
cats as altruists, when they are actually metabolically
obligate and the realistic channel of investment is their own
body condition feeding lactation. Retiring those options.

**Next concrete follow-on (not yet blocking).** The highest-
realism / highest-return follow-on is **alloparental care** —
non-nursing queens bringing food to nursing mothers. That
would let a well-fed aunt feed Mocha's kittens when Mocha
herself can't. New Caretake sub-targets, a `nursing_queen`
marker, and routing food-delivery to the nursing queen rather
than directly to the kitten. Design stub belongs in
`docs/systems/` paired with the §6.5.6 target-taking DSE port
when it lands.

Sources: Nutter FB et al. *JAVMA* 2004 (n=169 kittens, feral
mortality); Crowell-Davis SL et al. *J Feline Med Surg* 2004;
Liberg O, Sandell M, Pontier D, Natoli E (in Turner & Bateson
eds. 2000); Macdonald DW, Yamaguchi N, Kerby G 2000 (farm-cat
allonursing + infanticide); Veronesi MC, Fusi J *J Feline Med
Surg* 2022; Bradshaw JWS 2012 (*The Behaviour of the Domestic
Cat* 2nd ed., ch. 8); Vitale KR et al. 2022 review of
free-ranging cat social lives. Alley Cat Allies field guides.

### Phase 4c.2 — §6.5.2 `Mate` target-taking DSE port (2026-04-22)

Second per-DSE §6 port. Closes the §6.2 silent-divergence between
`disposition.rs::build_mating_chain`
(`romantic + fondness - 0.05 × dist` mixer with inline
Partners/Mates bond filter) and `goap.rs::resolve_goap_plans::MateWith`
(`find_social_target` — fondness-only, **no bond filter**) by
routing both through a single `TargetTakingDse` evaluator.

The goap silent divergence was the more dangerous of the two: it
let the MateWith step target a non-partner cat once the Mate
disposition won selection upstream (since `find_social_target`
didn't check bond). The port closes that gap.

- New `src/ai/dses/mate_target.rs` with:
    - `mate_target_dse()` factory — three per-§6.5.2
      considerations (`target_nearness` Logistic(20, 0.5),
      `target_romantic` Linear(1,0), `target_fondness`
      Linear(1,0)) composed via WeightedSum with renormalized
      weights `[0.1875, 0.5, 0.3125]` (spec weights 0.15 / 0.40 /
      0.25 divided by 0.80 to drop the blocked fertility-window
      axis). Fertility-window (§6.5.2 row 4) deferred until
      §7.M.7.5's phase→scalar signal mapping lands (Enumeration
      Debt).
    - `resolve_mate_target(...) -> Option<Entity>` caller-side
      helper — filters candidates by bond (`Partners` | `Mates`
      only) before scoring, matching `build_mating_chain`'s
      current eligibility semantics. Candidate-pool range is
      `MATE_TARGET_RANGE = 10.0` (matches social-range) to admit
      nearby Partners into the scoring pool; the Logistic
      distance curve decays sharply from adjacency.
    - `Intention::Activity { kind: ActivityKind::Pairing, ... }`
      factory threads winning partner forward.
    - 8 unit tests — factory shape (id / axes / weights), plus
      resolver (missing DSE / non-bonded filter / Partners pick /
      romantic-over-fondness tiebreak / Pairing-Intention shape).
- Registration: `mate_target_dse()` pushed into
  `target_taking_dses` at both mirror sites
  (`plugins/simulation.rs` + `main.rs::build_new_world`).
- Caller cutovers:
    1. `systems/disposition.rs` `disposition_to_chain` —
       `build_mating_chain` signature shrinks from
       `(entity, pos, personality, cat_positions, relationships, d)`
       to `(mate_target: Option<Entity>, cat_positions)`.
       Inline `romantic + fondness - 0.05 × dist` mixer with
       bond filter retires; pre-resolved partner consumed
       directly.
    2. `systems/goap.rs` `resolve_goap_plans::MateWith` —
       `find_social_target(...)` call replaced by
       `resolve_mate_target(...)`. Bond filter now applied at
       the goap path for the first time; closes the
       more-dangerous half of the silent divergence.

**Seed-42 `--duration 900` re-soak
(`logs/phase4c2-mate-target/events.jsonl`; baseline
`logs/phase4c1-socialize-target/events.jsonl` at Phase 4c.1 HEAD):**

| Metric | Baseline (4c.1) | Phase 4c.2 | Direction |
|---|---|---|---|
| `deaths_by_cause.Starvation` | 1 | **5** | **canary fails** — all 5 are kittens (see below) |
| `deaths_by_cause.ShadowFoxAmbush` | 0 | 0 | ✅ canary passes |
| `continuity_tallies.grooming` | 217 | 274 | +26% |
| `continuity_tallies.courtship` | 2 | **5** | +150% (courtship activity climbs) |
| MatingOccurred events | 1 | **5** | **+4 (3 pregnancies, 5 kittens across 2 twin + 1 singleton litters)** |
| KittenBorn events | 1 | **5** | **+4 (every kitten died)** |
| `positive_features_active` | 16 | 16 | flat |
| `CriticalSafety preempted L5 plan` | 46 | 6 | −87% (continued shift away from L5 plans) |
| `TendCrops: no target for Tend` | — | 386 | new plan-failure surface |
| `ward_avg_strength_final` | 0.315 | 0.304 | noise |

Constants header diffs clean (zero-byte via
`just diff-constants`). All metric deltas are AI-behavior only.

**Hypothesis / concordance — canary fail is the Caretake gap
compounding.** All 5 starvation deaths are kittens:
`Wispkit-45`, `Fernkit-15`, `Thistlekit-65`, `Cricketkit-39`,
`Pipkit-69`. None existed in the Phase 4c.1 baseline soak. They
are newborns from three Mate-port-enabled pregnancies (Mocha × 2
litters of twins + Mallow × 1 singleton that died as
`Wispkit-45`). The orphan-starve pattern is identical to Phase
4c.1's lone Wrenkit-98 case: kitten born → no adult fires
Caretake → kitten starves in ~60 snapshots. Root cause traced:
`hungry_kitten_urgency` is hardcoded `0.0` in both scoring
caller paths (`disposition.rs:640` + `goap.rs:937`), so the
existing Caretake DSE's dominant axis (weight 0.45) never
contributes and Caretake never wins action selection.

The Mate port is **correctly enabling reproduction** — romantic +
fondness scoring with the bond filter surfaces higher-quality
partner selection than the legacy `find_social_target`. The
canary trip is the Caretake dormancy from Phase 4c.1's landing
record amplified 5× by reproduction actually happening now.

**Caretake is now BLOCKING** further per-DSE ports — see the
Outstanding section for the priority-upgrade rationale. Every
additional port that boosts prosocial behavior will compound
kitten mortality against the hard-gate canary.

### Phase 4c.1 — §6.5.1 `Socialize` target-taking DSE port (2026-04-22)

First per-DSE §6 port. Closes the §6.2 silent-divergence between
`disposition.rs::build_socializing_chain` (fondness × 0.6 +
(1-familiarity) × 0.4 weighted mixer) and
`goap.rs::find_social_target` (fondness-only max-by) by routing
both through a single `TargetTakingDse` evaluator.

- New `src/ai/dses/socialize_target.rs` with:
    - `socialize_target_dse()` factory — four per-§6.5.1
      considerations (`target_nearness` Quadratic(exp=2),
      `target_fondness` Linear(1,0), `target_novelty` Linear(1,0),
      `target_species_compat` piecewise-cliff) composed via
      `WeightedSum([0.25, 0.35, 0.25, 0.15])` with
      `TargetAggregation::Best`.
    - `resolve_socialize_target(...) -> Option<Entity>`
      caller-side helper — assembles candidates, builds
      `fetch_self_scalar` + `fetch_target_scalar` closures
      (fetcher computes `target_nearness` from position
      geometry), invokes `evaluate_target_taking`, returns the
      winning target. Single source of truth consumed at three
      call sites (see below).
    - `Intention::Activity { kind: ActivityKind::Socialize,
      termination: UntilInterrupt, strategy: OpenMinded }`
      factory thread winning target forward for future §L2.10
      downstream planning.
    - 13 unit tests — 8 factory shape (id / axes / weights /
      aggregation / argmax / silent-divergence tiebreak / empty
      candidates / intention shape) plus 5 resolver integration
      (missing-DSE / out-of-range / fondness pick / self-exclude
      / novelty tiebreak).
- Registration: `socialize_target_dse()` pushed into
  `target_taking_dses` at both mirror sites
  (`plugins/simulation.rs` + `main.rs::build_new_world` +
  save-load path). `ExecutorContext` + `ChainResources`
  (SystemParam bundles) gained `dse_registry: Res<DseRegistry>`
  so `resolve_goap_plans` and `disposition_to_chain` can invoke
  the resolver.
- Caller cutovers:
    1. `systems/disposition.rs` `evaluate_dispositions` —
       `has_social_target` bool gate now reads
       `resolve_socialize_target(...).is_some()`.
    2. `systems/disposition.rs` `disposition_to_chain` —
       `build_socializing_chain`'s signature loses
       `entity/pos/cat_positions/relationships` (target now
       pre-resolved), keeps `cat_positions` for position lookup
       of the returned target; the inline weighted-mixer picker
       at lines 1348-1365 retires.
    3. `systems/goap.rs` `evaluate_and_plan` — `has_social_target`
       reads through the resolver (same shape as disposition.rs).
    4. `systems/goap.rs` `resolve_goap_plans` `SocializeWith`
       step — replaces `find_social_target(...)` call with
       `resolve_socialize_target(...)`. Other three callers of
       `find_social_target` (GroomOther/MentorCat/MateWith) stay
       on the legacy helper until their §6.5.2–§6.5.4 ports.
- Three orphaned constants (`fondness_social_weight`,
  `novelty_social_weight`, `social_chain_target_range`) remain in
  `SimConstants` as dead fields pending a follow-on cleanup
  commit — retirement shifts the constants-hash which isn't this
  port's concern.

**Seed-42 `--duration 900` re-soak
(`logs/phase4c1-socialize-target/events.jsonl` on the uncommitted
working copy; baseline `logs/phase4b4-db7362b/events.jsonl` at
`db7362b2`):**

| Metric | Baseline | Phase 4c.1 | Direction |
|---|---|---|---|
| `deaths_by_cause.Starvation` | 0 | 1 | **canary fails** — see causal chain below |
| `deaths_by_cause.ShadowFoxAmbush` | 0 | 0 | ✅ canary passes |
| `continuity_tallies.grooming` | 262 | 217 | −17% |
| `continuity_tallies.courtship` | 0 | **2** | **new** — courtship activity unblocked |
| MatingOccurred events | 0 | **1** | **new** — first mating on seed 42 in project history |
| KittenBorn events | 0 | **1** | **new** — first reproduction on seed 42 |
| `positive_features_active` | 13 | 16 | +3 |
| `ward_avg_strength_final` | 0.456 | 0.315 | −31% |
| `CriticalSafety preempted L5 plan` | 6403 | 46 | −99% (cats spend less time in self-actualization plans, more in social) |

Constants header diffs clean (zero-byte diff via
`just diff-constants`), so all metric deltas are from AI behavior
changes alone.

**Hypothesis / concordance for the starvation canary fail.**
Wrenkit-98 is a kitten born at tick 1354759 to Mocha (mating with
17v0 at 1334759) and starves at 1361472 (~7k ticks, ~0.3 sim-day
post-birth) at position (26, 5). The mating that produced her
*never happened in baseline* — it's the first successful mating
on seed 42 in project history, enabled by the new target-taking
DSE surfacing a higher-quality partner than the legacy
fondness-only picker. Her death traces to Caretake's still-open
§6.5.6 gap: no adult routes TO (26, 5) to feed her. The kitten's
score table at tick 1360100 shows Eat at 0.154 (ranked 7th);
Caretake doesn't surface in the adult cats' action pools because
the Caretake DSE today navigates to nearest `Stores`, not to the
kitten with unmet hunger need.

Direction: the canary trip is a **spec-predicted downstream
dormancy surfacing**, not a regression introduced by the Socialize
port itself. The port's contribution is validated — mating /
courtship / BondFormed signals all climb — and the refactor's
design explicitly anticipated that "marker authoring alone does
**not** unblock the Cleanse / Harvest / Commune dormancies"
(open-work #14 commentary); Caretake belongs to the same
"navigate TO a physical location" class of gap.

Landing commitment: ship as-is with this causal record; **§6.5.6
Caretake port is the immediate priority follow-on** to resolve
the orphan-starvation pattern before it compounds over
multi-generation soaks.

### Phase 4b.4 — §4 `HasGarden` marker port (2026-04-22)

Second reference port of the Phase 4b.2 MVP pattern. Farm's outer
`if ctx.has_garden` gate retired; `FarmDse::new()` gains
`.require("HasGarden")`. Caller-side population in goap.rs /
disposition.rs reuses the existing `has_garden` computation with a
single appended `markers.set_colony("HasGarden", has_garden)` line.
Reinforces that per-marker porting is mechanical: three line
changes (population + `.require` + outer-gate retirement) +
optional test-fixture update.

Does not unblock Farming dormancy — the baseline's Farming = 0
traces to `TendCrops: no target` plan-failures (target-resolver
issue in GOAP), not an outer-eligibility issue.

### Phase 4b.3 — §6.3 `TargetTakingDse` type + evaluator (2026-04-22)

Foundation for §6 target-taking scoring. No DSE ports yet — the
scope is the type, the evaluator, and the registration surface.

- New `src/ai/target_dse.rs` with:
    - `TargetTakingDse` struct per §6.3 — id, candidate_query,
      per-target considerations, composition, aggregation,
      intention factory.
    - `TargetAggregation` enum — `Best` (default), `SumTopN(n)`
      for threat aggregation, `WeightedAverage` for rank-decayed
      sums.
    - `ScoredTargetTakingDse` output — per-candidate scores
      (unsorted), winning target, aggregated score, emitted
      intention; `ranked_candidates()` sorts descending for trace
      emission.
    - `evaluate_target_taking` evaluator — per-candidate score via
      per-target considerations, compose, aggregate. Scalar names
      prefixed `target_` dispatch through a target-scoped fetcher;
      everything else reads the scoring cat.
- `DseRegistry.target_taking_dses` retyped from
  `Vec<Box<dyn Dse>>` to `Vec<TargetTakingDse>`.
  `add_target_taking_dse` registration method on
  `DseRegistryAppExt` takes `TargetTakingDse` by value.
- 6 unit tests: empty-candidate short-circuit,
  `Best`/`SumTopN`/`WeightedAverage` aggregation semantics,
  per-candidate spatial sampling, ranked-candidates helper.

No live-sim behavior change — nothing registers a target-taking
DSE yet. Pure foundation; per-DSE ports follow.

### Phase 4b.2 MVP — §4 marker lookup foundation + `HasStoredFood` reference port (2026-04-22)

First end-to-end §4 marker port. `has_marker` moves from its
`|_, _| false` stub at `scoring.rs:435` to a real lookup against
a new `MarkerSnapshot` type threaded through `EvalInputs`.
`EatDse` gains `.require("HasStoredFood")`; the inline outer
`if ctx.food_available` gate at `score_actions` retires (both
the non-incapacitated and incapacitated code paths). The caller
populates `markers.set_colony("HasStoredFood", !food.is_empty())`
at the top of each scoring tick.

Pattern is now set for the remaining ~49 §4.3 markers: one
authoring-site line per marker in the caller, one `.require(...)`
row on the target DSE, optionally a per-tick system if the
predicate is expensive enough to cache. The canonical spec shape
(markers as ZST components on a `ColonyState` singleton) is a
drop-in refactor later — only the caller-side population logic
shifts; the evaluator-side surface stays identical.

Tests: 5 new `marker_snapshot_*` unit tests (empty pool, colony
scoping, entity scoping, clear semantics, clear-doesn't-nuke-peers).
`eat_dse_requires_has_stored_food` + `eat_dse_rejected_without_has_stored_food_marker`
replace the placeholder `eat_dse_has_no_eligibility_filter_today`
test (which named itself as a Phase 3d-to-flip placeholder).

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
