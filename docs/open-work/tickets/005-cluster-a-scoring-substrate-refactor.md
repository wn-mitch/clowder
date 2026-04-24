---
id: 005
title: Scoring substrate refactor cluster (Cluster A — Foundational)
status: in-progress
cluster: A
added: 2026-04-20
parked: null
blocked-by: []
supersedes: []
related-systems:
  - ai-substrate-refactor.md
  - refactor-plan.md
  - a1-iaus-core-kickoff.md
  - a1-2-focal-cat-replay-kickoff.md
  - a1-4-retired-constants-kickoff.md
  - phase-6a-commitment-gate-attempt.md
related-balance:
  - substrate-refactor-baseline.md
  - substrate-phase-1.md
  - substrate-phase-2.md
  - substrate-phase-3.md
  - substrate-phase-4.md
  - scoring-layer-second-order.md
landed-at: null
landed-on: null
---

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

**Status (2026-04-23, revised after audit):** A1 is a composite of
three structurally distinct tracks; track-level status does **not**
flatten to a single "landed / outstanding" bit. A2 is resolved. A3
and A4 each have sub-tracks in similar states to A1. A5 remains
outstanding. Call the tracks **A** (substrate), **B** (per-axis
curve migration), **C** (per-marker authoring); every cluster-A
entry slices across them.

- **Track A — Substrate infrastructure. LANDED.**
  `src/ai/{curves,considerations,composition,dse,eval,modifier}.rs`
  shipped in Phase 3a+3b: `Curve` enum (7 variants + 7 named
  anchors), `Consideration` (Scalar/Spatial/Marker), `Composition`
  (CP / WS / Max with RtM / RtEO enforcement + 0.75 compensation),
  `Dse` trait, `EligibilityFilter`, `DseRegistry` + 6-method
  registration, `ModifierPipeline`, `evaluate_single`,
  `select_intention_softmax`. The four-layer pipeline shape
  (eligibility → considerations → composition → Maslow pre-gate →
  modifier pipeline) is live. 30 cat+fox DSE factories exist under
  `src/ai/dses/`, all routed through `score_dse_by_id → evaluate_single`.
- **Track B — Per-axis curve migration. LANDED (§2.3 rows 1–6
  closed).** All §2.3-assigned curves are in place: `hangry()` on
  hunger peer-group, `sleep_dep()` on Sleep, `loneliness()` on
  social axes, `scarcity()` on food-scarcity axes,
  `flee_or_fight()` on safety axes, `fight_gating()` Piecewise on
  Fight health/safety, `piecewise(…)` on all day-phase axes,
  `inverted_need_penalty()` on phys-satisfaction axes,
  `Composite`/`ClampMin` on personality-floor axes,
  `Composite`/`ClampMax` on saturating-count axes, plus the five
  corruption-axis migrations (rows 4–6) landed with §13.1 rows 4–6
  on 2026-04-23: `Logistic(8, 0.1)` on
  `herbcraft_gather.territory_max_corruption` (axis added),
  `herbcraft_ward.territory_max_corruption` (axis added), and
  `practice_magic::durable_ward.nearby_corruption_level` (axis
  added); `Logistic(8, magic_cleanse_corruption_threshold)` swap
  on `practice_magic::cleanse.tile_corruption`; `Logistic(6, 0.3)`
  swap on `practice_magic::colony_cleanse.territory_max_corruption`.
- **Track C — Per-marker author systems. PARTIAL (~14%).**
  49 §4.3 marker ZSTs exist as components (Phase 3a). Six have
  per-tick author systems landed: the five colony-scoped authors
  (`HasStoredFood`, `HasGarden`, `HasFunctionalKitchen`,
  `HasRawFoodInStores`, `WardStrengthLow` — authored inline in
  `systems/goap.rs` + `systems/disposition.rs` parallel
  `MarkerSnapshot` builders) plus the first per-cat author,
  `Incapacitated` (`systems/incapacitation.rs::update_incapacitation`,
  plus per-cat `set_entity` in both `MarkerSnapshot` builders).
  DSE consumers are wired for all six authored markers;
  `Incapacitated`'s consumer cutover shipped with §13.1 rows 1–3
  on 2026-04-23 — `.forbid("Incapacitated")` now appears on every
  non-Eat/Sleep/Idle cat DSE and every fox DSE, and the inline
  `if ctx.is_incapacitated` branch at `scoring.rs:574–598` has
  retired. 43 remain fully unauthored — every LifeStage
  (`Kitten`/`Young`/`Adult`/`Elder`), every State
  (`InCombat`/`Injured`/`Pregnant`), every Capability
  (`CanHunt`/`CanForage`/`CanWard`/`CanCook`), every
  TargetExistence, and most Colony markers.

**Cluster-A entry reframing:**

- **A1** — Track A landed; Track B landed (§2.3 rows 1–6 all
  closed after 2026-04-23's five corruption-axis migrations);
  Track C ~14% (43 markers unauthored; `Incapacitated` author
  + consumer cutover landed, inline branch retired).
  The 21 cat + 9 fox DSE *shapes* are in the registry with
  their spec-committed curves; the remaining refactor-scope work
  for A1 is Track C marker authoring and a handful of deeper
  structural items (~~§7 commitment layer~~ — landed 2026-04-24;
  §L2.10.7 plan-cost feedback).
- **A2** — resolved as *build in-house*. `big-brain` was not
  adopted. The L2 substrate at `src/ai/eval.rs` is the outcome.
- **A3** — Track A landed (49 ZSTs + consumer substrate); A3's
  exit criterion ("at least one action migrates to a
  pure-tag-filter entry guard as proof-of-pattern") is met for
  the six authored markers with live consumers
  (`HasStoredFood`, `HasGarden`, `HasFunctionalKitchen`,
  `HasRawFoodInStores`, `WardStrengthLow`, `Incapacitated`).
  The remaining 43 per-marker ports are the bulk of A3's work,
  tracked in #14's "Still outstanding" list.
- **A4** — landed. §6.3 `TargetTakingDse` foundation + all nine
  §6.5 per-DSE target-taking ports shipped. `find_social_target`
  retired. A4 is the one cluster-A entry that truly slices cleanly
  across all three tracks.
- **A5** — landed. §11 focal-cat replay instrumentation shipped via
  Phase A1.2: at-source L2/L3 capture through `evaluate_single_with_trace`
  / `ModifierPipeline::apply_with_trace` /
  `select_disposition_via_intention_softmax_with_trace`. L2 records
  now carry real per-consideration + modifier data; L3 records carry
  real softmax probabilities + RNG roll. Replay-frame joinability
  (§11.4) no longer vacuous. See the Landed section for the full
  landing entry.

**Still outstanding refactor-scope work** (consolidated; each item
links to its gate chain rather than being called "ready to land"):

- ~~**§11 instrumentation (A5).**~~ **Landed — see Phase A1.2 entry
  in Landed section.**
- ~~**§3.5 remaining-modifier port.**~~ **Landed — all 10 §3.5
  modifiers ported.** Pride / Independence-solo / Independence-group
  / Patience / Tradition / Fox-suppression / Corruption-suppression
  each registered as a first-class `ScoreModifier` in
  `src/ai/modifier.rs`, mirroring the Phase 4.2
  `WardCorruptionEmergency` / `CleanseEmergency` / `SensedRotBoost`
  pattern. `default_modifier_pipeline` now hands out all 10 passes in
  retiring-inline-order. Trigger inputs flow through new scalar-
  surface keys (`respect`, `pride`, `independence`, `patience`,
  `tradition_location_bonus`, `fox_scent_level`,
  `active_disposition_ordinal`) in `ctx_scalars`; Patience's
  disposition→DSE membership lives in
  `constituent_dses_for_ordinal` keyed off the active-disposition
  ordinal. Inline `score_actions:666–750` block deleted; covered by
  23 new unit tests in `src/ai/modifier.rs` (31 total modifier
  tests). Tradition's unfiltered-loop bug (§3.5.3 item 1) is filed
  below as a separate follow-on — a behavior change requires the
  balance-methodology hypothesis + A/B, not a translation-scoped
  port.
- ~~**§2.3 corruption-axis migrations (Track B remainder).**~~
  **Landed — see §13.1 rows 4–6 entry in Landed section.** All
  five per-DSE axis additions / curve swaps in Herbcraft /
  PracticeMagic sibling DSEs shipped 2026-04-23.
- ~~**`Incapacitated` DSE consumer cutover (Track C).**~~
  **Landed — see §13.1 rows 1–3 entry in Landed section.**
  `.forbid("Incapacitated")` added to every non-Eat/Sleep/Idle
  cat DSE + every fox DSE; inline `is_incapacitated` branch at
  `scoring.rs:574–598` retired; 5 `incapacitated_*` constants
  deleted.
- ~~**§13.1 retired-constants cleanup.**~~ **Landed in two
  commits on 2026-04-23.** Rows 1–3 (Incapacitated pathway) +
  rows 4–6 (corruption-emergency-bonus pathway) shipped as
  separate refactor commits; all 8 retired constants + 3 retired
  modifier impls gone from the codebase. See both entries in the
  Landed section. The
  [`a1-4-retired-constants-kickoff.md`](../systems/a1-4-retired-constants-kickoff.md)
  doc captures the original single-commit framing; the split
  happened naturally once rows 1–3 and rows 4–6 turned out to
  have disjoint file-sets and a parallelizable fan-out landed
  both in one afternoon.
- ~~**§7 commitment strategies (§7.2 + §7.3).**~~ **Landed across
  sessions 1–4 (2026-04-23 → 2026-04-24).** Root cause was an LLVM
  optimization cliff: 4 cross-module function calls in the ~4,500-line
  `resolve_goap_plans` hot loop pushed LLVM past a release-mode
  optimization threshold. Fix: split into `resolve_goap_plans` (~797
  lines) + `dispatch_step_action` (~1,275 lines, `#[inline(never)]`),
  then wire the prologue gate into the smaller function. Full
  investigation in
  [`docs/systems/phase-6a-commitment-gate-attempt.md`](systems/phase-6a-commitment-gate-attempt.md).
  See Landed section for details.

  ~~Adjacent `find_nearest_tile` north-bias~~ — fixed (tiebreaker via
  `mix_hash` added to `goap.rs:4563`).
- **§L2.10.7 plan-cost feedback.** Blocks 4 §6.5 deferred axes
  (`apprentice-receptivity`, `fertility-window`, `remedy-match`,
  `pursuit-cost`).
- **§4 marker authoring rollout (~43 markers).** Life-stage,
  state (minus `Incapacitated`, landed 2026-04-23), capability,
  target-existence, and colony markers still unauthored. Each is
  one tick-system + `MarkerSnapshot` population + a `.require()`
  / `.forbid()` cutover on its consumer DSE.
- ~~`resolve_disposition_chains` split (LLVM cliff prevention).~~
  **Landed.** Extracted 875-line `match &step.kind` dispatch into
  `dispatch_chain_step` with `#[inline(never)]` + `ChainStepSnapshots` /
  `ChainStepAccumulators` structs, mirroring the goap `dispatch_step_action`
  pattern. Post-split sizes: caller ~448 lines, dispatch ~885 lines.

### A1. IAUS refactor — response curves + multiplicative composition [TOP PRIORITY]

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

### A2. Investigate big-brain as IAUS migration vehicle

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

### A3. Context-tag uniformity refactor

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

### A4. Target selection as inner optimization

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

### A5. Substrate instrumentation — Curvature-at-every-layer traces

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
