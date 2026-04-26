# AI Substrate Refactor — Design Specification

> **Status:** Second-pass draft (2026-04-21, §L2.10 closure pass).
> §5 (influence maps) and L2 sections §1 (Considerations), §2
> (Response curves), §3 (Composition), §6 (Target selection), and
> §L2.10 (DSE catalog & Intention output) are well-formed against
> Mark's *Behavioral Mathematics for Game AI* chapters 12–14
> (`docs/reference/behavioral-math-ch{12,13,14}-*.md`) and Mark's
> "Modular Tactical Influence Maps" (*Game AI Pro 2* ch. 30,
> `docs/reference/modular_tactical_influence_maps.md`). §7
> (commitment and persistence) is committed against Rao &
> Georgeff 1991 (`docs/reference/bdi-rao-georgeff.md`) and Mark
> ch 15 (`docs/reference/behavioral-math-ch15-changing-decisions.md`).
> **This pass closes the §L2.10 enumeration cluster:** §L2.10.3
> (full 45-row DSE registration catalog across 5 registration
> methods) and §L2.10.10 (9-row Herbcraft / PracticeMagic sibling-
> DSE curve specs — first use of §L2.10.5's `Goal | Activity`
> split below the parent-DSE layer). **§7.M Mating closed as the
> canonical three-layer aspiration → activity → goal showcase**
> (`ReproduceAspiration` / `PairingActivity` / `MateWithGoal`);
> cascades land in §7.3 strategy table, §7.4 persistence-tier
> table + completion-fraction footer, §L2.10.7 Mate-row note, and
> §6.5.2 per-target considerations — the first case that exercises
> §7.7 nested Intentions end-to-end against a load-bearing
> ecological function (generational continuity). Earlier passes
> closed §7.4 (persistence-bonus tier table), §7.5 (Maslow-
> interrupt catalog), and §L2.10.7 (30-row spatial-DSE roster),
> and split §7.7 reconsideration-events into current-state vs.
> emission-debt. §L2.10.7 is committed to candidate (a) per
> ch 14 §"Which Dude to Kill?" §12 (beliefs, percepts, memory)
> names the scope boundary between belief proxies and the
> deferred Talk-of-the-Town layer. §8 (variation in choice)
> remains a **stub pending synthesis** of ch 16. §0 (four design
> principles) added 2026-04-20 from Sylvester's RimWorld GDC 2017
> talk — cross-cuts all of §1–§L2.10; §0.4 is a scope-discipline
> filter on future mechanic proposals and points forward to §10
> feature-design work. §11 (instrumentation) added 2026-04-20 —
> per-layer Curvature-style traces, focal-cat replay, sidecar
> JSONL; lands alongside the refactor so `CLAUDE.md`'s balance
> rule can predict-and-verify rather than change-and-see.
> **Table of contents** (added 2026-04-21) sits above the
> Enumeration Debt ledger for navigation through the doc's 5k+
> lines. **Ledger pruned 2026-04-21 to spec-scope only:** seven
> items that close in other systems (§7.7.b/c/d event-vocabulary
> debts, §7.3 coordinator row, §7.7.1 aspiration-pair list, the
> retired-constants code cleanup, the `needs.warmth` split) now
> live in `docs/open-work.md` entry #13. The three in-scope holes
> surfaced by the §7.M / §L2.10.3 closures were all burned down in
> the 2026-04-21 pass: §9 faction-model enumeration (10×10 stance
> matrix + marker overlay, §9.0–§9.3), §4.3 marker-catalog
> completeness (Reproduction block for Fertility / Apprentice /
> Parent), and §7.M.7 fertility state specification (five-variant
> phase enum + `fertility.rs` state-machine + Gender↔role canon +
> §6.5.2 / §7.M.4 wire-ups). No in-scope holes remain.
>
> **This is a design specification, not an implementation plan.** Phasing,
> execution sequence, agent-team fan-out, and per-phase verification
> protocols are explicitly out of scope. Implementation scoping begins
> only once this spec stabilizes — trying to phase the work before the
> end-state is well-specified is exactly how this thread ended up
> needing a re-plan.

## Table of contents

**Front matter**
- [Enumeration Debt (TODO)](#enumeration-debt-todo)
- [What this document is for](#what-this-document-is-for)
- [Motivation](#motivation)
- [Current state](#current-state)
- [Architectural vision](#architectural-vision)

**§0 Design principles**
- [§0.1 The simulation is the director is the player](#01-the-simulation-is-the-director-is-the-player)
- [§0.2 Elastic failure](#02-elastic-failure)
- [§0.3 Apophenia has two legs: abstracted feedback and long-term relevance](#03-apophenia-has-two-legs-abstracted-feedback-and-long-term-relevance)
- [§0.4 Mechanics must express character, not just apply modifiers](#04-mechanics-must-express-character-not-just-apply-modifiers)
- [§0.5 Cross-refs](#05-cross-refs)

**§1 Considerations — the scoring atom**
- [§1.1 Trait shape](#11-trait-shape)
- [§1.2 Three flavors of consideration](#12-three-flavors-of-consideration)
- [§1.3 Resolved open questions](#13-resolved-open-questions)
- [§1.4 Size and shape guidance](#14-size-and-shape-guidance)

**§2 Response curve primitives**
- [§2.1 Curve primitive enum](#21-curve-primitive-enum)
- [§2.2 LUT backing — start function-evaluated](#22-lut-backing--start-function-evaluated)
- [§2.3 Curve-shape assignment table](#23-curve-shape-assignment-table)
- [§2.4 Cross-refs](#24-cross-refs)

**§3 Multi-consideration composition**
- [§3.1 Three composition modes](#31-three-composition-modes)
- [§3.1.1 Per-DSE composition mode assignment](#311-per-dse-composition-mode-assignment)
- [§3.2 The compensation factor](#32-the-compensation-factor)
- [§3.3 Weight rationalization](#33-weight-rationalization)
- [§3.4 Maslow as a hierarchical pre-gate (keep)](#34-maslow-as-a-hierarchical-pre-gate-keep)
- [§3.5 Post-scoring modifiers as a distinct layer](#35-post-scoring-modifiers-as-a-distinct-layer)
- [§3.6 Granularity (ch 13 pain-scale discipline)](#36-granularity-ch-13-pain-scale-discipline)
- [§3.7 Cross-refs](#37-cross-refs)

**§4 Context tags — ECS markers as eligibility filters**
- [§4.1 Tag categories](#41-tag-categories)
- [§4.2 Catalog schema](#42-catalog-schema)
- [§4.3 Marker catalog](#43-marker-catalog)
- [§4.4 Crosswalk: ScoringContext → markers](#44-crosswalk-scoringcontext--markers)
- [§4.5 Scalar carve-out](#45-scalar-carve-out)
- [§4.6 Authoring-system roster](#46-authoring-system-roster)

**§5 Influence-map substrate**
- [§5.1 Base maps, templates, working maps](#51-base-maps-templates-working-maps)
- [§5.2 Sensory channels (Clowder-specific extension)](#52-sensory-channels-clowder-specific-extension)
- [§5.3 Decay](#53-decay)
- [§5.4 Obstacle-aware propagation](#54-obstacle-aware-propagation)
- [§5.5 Social influence (deferred to ToT phase)](#55-social-influence-deferred-to-tot-phase)
- [§5.6 L1 context enumeration](#56-l1-context-enumeration)

**§6 Target selection as inner optimization**
- [§6.1 Anti-pattern inventory — worse than previously documented](#61-anti-pattern-inventory--worse-than-previously-documented)
- [§6.2 Silent divergence — GOAP vs. disposition resolver](#62-silent-divergence--goap-vs-disposition-resolver)
- [§6.3 `TargetTakingDse` shape](#63-targettakingdse-shape)
- [§6.4 Personal-interest template formalized](#64-personal-interest-template-formalized)
- [§6.5 Per-target consideration sets](#65-per-target-consideration-sets)
- [§6.6 Aggregation choices](#66-aggregation-choices)
- [§6.7 Cross-refs](#67-cross-refs)

**§7 Decision persistence and momentum**
- [§7.1 Commitment-strategy enum](#71-commitment-strategy-enum)
- [§7.2 Drop-trigger reconsideration gate](#72-drop-trigger-reconsideration-gate)
- [§7.3 Per-Intention-class strategy assignment](#73-per-intention-class-strategy-assignment)
- [§7.4 Persistence bonus (ch 15 Finish Him!)](#74-persistence-bonus-ch-15-finish-him)
- [§7.M Mating — canonical three-layer aspiration showcase](#7m-mating--canonical-three-layer-aspiration-showcase)
- [§7.5 Maslow interrupt interaction](#75-maslow-interrupt-interaction)
- [§7.6 Monitoring cadence](#76-monitoring-cadence)
- [§7.7 Aspiration-level commitment (separate layer from §7.1–§7.6)](#77-aspiration-level-commitment-separate-layer-from-7177176)
- [§7.W Axis-capture and the warring self](#7w-axis-capture-and-the-warring-self)
- [§7.8 Residuals — open questions from the original stub, resolved inline](#78-residuals--open-questions-from-the-original-stub-resolved-inline)
- [§7.9 Cross-refs](#79-cross-refs)

**§8 Variation in choice**
- [§8.1 Algorithm — softmax-over-all candidates](#81-algorithm--softmax-over-all-candidates)
- [§8.2 Scope — softmax-over-Intentions](#82-scope--softmax-over-intentions)
- [§8.3 Temperature — commit T = 0.15 default](#83-temperature--commit-t--015-default)
- [§8.4 Order with §7 momentum — softmax first, persistence-bonus gating second](#84-order-with-7-momentum--softmax-first-persistence-bonus-gating-second)
- [§8.5 Species variants — converge foxes onto softmax](#85-species-variants--converge-foxes-onto-softmax)
- [§8.6 Apophenia calibration as continuity canary](#86-apophenia-calibration-as-continuity-canary)
- [§8.7 Residuals — open questions from the original stub, resolved inline](#87-residuals--open-questions-from-the-original-stub-resolved-inline)
- [§8.8 Out of scope for this spec](#88-out-of-scope-for-this-spec)
- [§8.9 Cross-refs](#89-cross-refs)

**§9 Faction model**
- [§9.0 Vocabulary reconciliation with §5.6.6.1](#90-vocabulary-reconciliation-with-5661)
- [§9.1 Biological base matrix (10 × 10)](#91-biological-base-matrix-10--10)
- [§9.2 ECS-marker overlay (colony / visitor / banished / befriended)](#92-ecs-marker-overlay-colony--visitor--banished--befriended)
- [§9.3 DSE filter binding](#93-dse-filter-binding)

**§L2.10 DSE catalog & single invocation surface**
- [§L2.10.1 Current landscape — scoring is scattered](#l2101-current-landscape--scoring-is-scattered)
- [§L2.10.2 Unified evaluation surface](#l2102-unified-evaluation-surface)
- [§L2.10.3 DSE registration](#l2103-dse-registration)
- [§L2.10.4 DSE output: Intention, not Action](#l2104-dse-output-intention-not-action)
- [§L2.10.5 Intention = `Goal | Activity` is Clowder-specific](#l2105-intention--goal--activity-is-clowder-specific)
- [§L2.10.6 Softmax-over-Intentions is the right variation scope](#l2106-softmax-over-intentions-is-the-right-variation-scope)
- [§L2.10.7 Plan-cost feedback — resolved via Mark ch 14](#l2107-plan-cost-feedback--resolved-via-mark-ch-14)
- [§L2.10.8 Dependencies on §7 and §8](#l2108-dependencies-on-7-and-8)
- [§L2.10.9 Cross-refs](#l2109-cross-refs)
- [§L2.10.10 Herbcraft / PracticeMagic sibling-DSE curve specs](#l21010-herbcraft--practicemagic-sibling-dse-curve-specs)

**§10 Baseline-feature unblock map**
- [§10.1 Feature-design filter from §0.4](#101-feature-design-filter-from-04)

**§11 Instrumentation and observability**
- [§11.1 Design principle: Curvature at every layer](#111-design-principle-curvature-at-every-layer)
- [§11.2 Sampling strategy: focal-cat replay](#112-sampling-strategy-focal-cat-replay)
- [§11.3 Record format — sidecar JSONL](#113-record-format--sidecar-jsonl)
- [§11.4 Joinability — the load-bearing invariant](#114-joinability--the-load-bearing-invariant)
- [§11.5 Scope rules and defensive structuring](#115-scope-rules-and-defensive-structuring)
- [§11.6 Out of scope (flagged for follow-on, not for this refactor)](#116-out-of-scope-flagged-for-follow-on-not-for-this-refactor)
- [§11.7 Cross-refs](#117-cross-refs)

**§12 Beliefs, percepts, and memory — scope boundary**
- [§12.1 The three states Clowder maintains today](#121-the-three-states-clowder-maintains-today)
- [§12.2 What a Rao & Georgeff Belief would be](#122-what-a-rao--georgeff-belief-would-be)
- [§12.3 The belief proxies §7.2 consumes](#123-the-belief-proxies-72-consumes)
- [§12.4 Why this is sufficient for L3](#124-why-this-is-sufficient-for-l3)
- [§12.5 Cross-refs](#125-cross-refs)

**Back matter**
- [A2 — big-brain evaluation](#a2--big-brain-evaluation)
- [Reading list](#reading-list)
- [Key insights accumulated](#key-insights-accumulated)
- [What's explicitly out of scope](#whats-explicitly-out-of-scope)

> TOC lists `##` top-level and `###` numbered-section headings. Deeper
> (`####` / `#####` — e.g., §5.6.6.1 species × channel, §7.7.a
> life-stage, §3.3.1 weight-mode rows) are intentionally omitted to
> keep this navigable at a glance; use `Grep "^#####? "` for the
> deepest level when needed. §L2.10's subsections are included because
> that parent is the doc's single largest section and its sub-numbering
> is load-bearing for cross-references.

---

## Enumeration Debt (TODO)

This refactor's design principle: **every major decision is enumerated
for every instance it applies to**, not illustrated with examples and
deferred. Each open item below is a section of *this spec* that
currently shows a pattern + some rows and names further enumeration
as future work within this doc. Burn them down as sessions allow;
check the box in the commit that lands the enumeration.
Already-enumerated decisions (e.g. §2.1 curve primitive enum, §3.1
composition mode enum) are not on the list.

**Scope boundary.** This ledger tracks enumeration debt that closes
*in this spec*. Spec-follow-on hooks that close in other systems
(`src/systems/death.rs`, `fate.rs`, `mood.rs`, `coordination.rs`,
aspirations) or in code (retired constants cleanup, `needs.warmth`
split) live in `docs/open-work.md` entry #13. See the "Tracked
elsewhere" footer below for the move-out list.

**Full-enumeration debt — every instance committed, not sampled:**

- [x] **§2.3** — Curve-shape assignment per DSE consideration (all 21
      cat DSEs in `src/ai/scoring.rs` + 9 fox dispositions in
      `src/ai/fox_scoring.rs`).
- [x] **§3.1** — Composition mode declared per DSE
      (`CompensatedProduct` / `WeightedSum` / `Max` named per DSE, not
      a summary count).
- [x] **§3.3** — Weight-expression mode declared per DSE: §3.3.1
      commits RtM (11 CP DSEs) / RtEO (16 WS DSEs) / deferred (3 `Max`-
      retiring) with axis counts and rationale; §3.3.2 enumerates
      eight absolute-anchor peer groups (starvation, threat, rest,
      social, territory, work, exploration, lifecycle).
- [x] **§3.5** — Post-scoring modifier applicability matrix: §3.5.1
      is the per-modifier catalog (6 modifiers + Independence's
      dual-direction expansion = 7 rows) with trigger condition,
      transform shape, DSE list, and `scoring.rs` source lines; §3.5.2
      is the DSE × modifier cross matrix (21 cat DSEs × 7 modifier
      columns); §3.5.3 captures three discoveries (Tradition
      unfiltered-loop bug, Fox-suppression's `Flee`-boost side effect,
      dead `has_active_disposition` field).
- [x] **§4** — ECS marker vocabulary catalog: §4.3 enumerates every
      species / role / life-stage / state / capability / inventory /
      target-existence / colony / spawn-immutable / fox-specific
      marker with predicate, insert system, remove system, query
      pattern, current-code status, and source field. §4.4 crosswalks
      the 27 + 9 `ScoringContext` / `FoxScoringContext` booleans
      against the catalog; §4.5 carves out the 19 + 5 scalars that
      stay sampled.
- [x] **§5.6.3** — Influence-map inventory: all 13 maps enumerated
      with per-row grid representation, update cadence, propagation
      mode (→ §5.6.4), decay factor (→ §5.6.5), current backing, and
      status. 0 Built, 5 Partial (#1 scent, #2 corruption, #4 fox-scent,
      #11 exploration, #12 congregation), 8 Absent.
- [x] **§5.6.5** — Decay factor committed per map: all 13 from §5.6.3
      plus 2 infrastructure rows (Wind gusts, Territory markers). Three
      `0.0` re-stamp rows, six `1.0` entity-lifecycle rows, four
      fading-persistence rows (0.85 / 0.90 / 0.95 / 0.99 / 0.999).
- [x] **§5.6.6** — Attenuation pipeline split into four sub-matrices:
      §5.6.6.1 (10 species × 4 channels, all Built from
      `sim_constants.rs:2605–2696`), §5.6.6.2 (11 role/life-stage rows ×
      4 channels, all identity — Absent), §5.6.6.3 (13 body-zones × 4
      channels with channel-feeder flags — Absent, coefficients TBD with
      body-zones build), §5.6.6.4 (8 weather + 4 phase + 7 terrain
      buckets × 4 channels = 76 env cells, all identity — Partial).
- [x] **§6.4** — Personal-interest template completeness: 9 target-
      taking DSEs, rows ordered by §6.1 severity (Critical 1–4, Partial
      5–9). Each row carries `Backs ScoringContext (field:line) +
      Resolver today (file:line) + Max range + Curve shape + Note`.
      Sibling DSEs under Herbcraft/PracticeMagic remain deferred to
      §L2.10 (see separate debt item below).
- [x] **§6.5** — Per-target considerations split into §6.5.1–§6.5.9
      (one per target-taking DSE). Each consideration declares
      `(signal source, curve primitive, weight, rationale)` — 36
      consideration rows total. Weights are first-pass commits;
      balance iterations refine.

**Synthesis debt — Mark ch 15 / ch 16 reading:**

- [x] **§7** — Decision persistence and momentum. Closed by BDI
      (Rao & Georgeff 1991) + Mark ch 15 synthesis. Strategy
      vocabulary (§7.1), drop triggers (§7.2), per-DispositionKind
      strategy table (§7.3), persistence bonus (§7.4), Maslow
      preemption (§7.5), monitoring cadence (§7.6), aspiration
      commitment layer (§7.7), aspiration concurrency via
      goal-consistency (§7.7.1), residual resolutions inline (§7.8).
      Numeric tuning (Logistic `midpoint`/`steepness`, `base`
      magnitudes) follows implementation, not spec.
- [x] **§8** — Variation in choice. Closed by Mark ch 16 synthesis.
      Algorithm committed (softmax-over-all, §8.1), scope committed
      (softmax-over-Intentions, §8.2 inheriting §L2.10.6),
      temperature band + default `T = 0.15` committed (§8.3), order
      with §7.4 momentum resolved (softmax first, bonus second;
      incumbent retained, §8.4), fox argmax+jitter converges onto
      softmax (§8.5), apophenia calibration scoped as §11
      continuity-canary work (§8.6). Numeric refinement is
      balance-thread work per line 24–29.

**Design-decision debt — pick one candidate:**

- [x] **§L2.10.7** — Plan-cost feedback. Closed in favor of
      candidate (a) `SpatialConsideration` with response curves,
      citing Mark ch 14 §"Which Dude to Kill?" The `replan_count`
      hard-fail channel supplements (a) for §7.2's
      `achievable_believed` drop trigger.

**Closed by the 2026-04-21 enumeration pass:**

- [x] **§7.4** — Per-DispositionKind persistence-bonus tier.
      14-row table committed (11 baseline tiers + 3 Mating-layer
      tiers from §7.M: L1 High, L2 Medium, L3 High). Tiers are
      categorical (Low / Medium / High / Indefinite); numeric
      magnitudes remain balance-thread work per line 24–29.
- [x] **§7.5** — Maslow-interrupt event catalog. 5-row table
      committed (CriticalHealth / Starvation / Exhaustion /
      ThreatDetected / CriticalSafety) with per-row exemption list,
      boldness-scaled threshold note, and Hunt-action carve-out
      note. Sourced from `src/systems/disposition.rs:180–253`.
- [x] **§L2.10.7** — Spatially-sensitive DSE roster. 22-row cat
      table (Groom splits self/other) + 9-row fox table committed
      with (Today, Target landmark, Curve primitive, Rationale)
      columns. Audit finding noted: no DSE currently uses continuous
      distance-to-landmark scoring; roster is fully aspirational.
- [x] **§L2.10.3** — DSE registration catalog. 45-row table
      (6 blocks: Tier 1 cat / Tier 2 cat / Tier 2–5 cat / Mating
      three-layer / Herbcraft–PracticeMagic siblings / fox /
      scattered-site absorbents) with constructor, method,
      subsumed source site(s), composition mode, Intention shape,
      notes. Extends the API sketch to five registration methods
      (`add_dse` / `add_target_taking_dse` / `add_fox_dse` /
      `add_coordinator_dse` / `add_aspiration_dse` /
      `add_narrative_dse`) and names target-ranking unification as
      the mechanism that dissolves §6.2's silent-divergence bug.
- [x] **§L2.10.10** — Herbcraft / PracticeMagic sibling-DSE curve
      specs. 9 siblings (3 Herbcraft + 6 PracticeMagic) committed
      with Intention shape, composition mode, and per-consideration
      curve specs. `scry` and `commune` flagged Activity-shaped per
      §L2.10.5; the other 7 Goal-shaped. All axis curves cite their
      §2.3 anchor row.
- [x] **§7.M** — Mating disposition, resolved as **canonical
      three-layer aspiration → activity → goal showcase**, not
      deferred to a separate stub. L1 `ReproduceAspiration`
      (OpenMinded, High tier), L2 `PairingActivity`
      (OpenMinded, Medium tier), L3 `MateWithGoal`
      (SingleMinded, High tier). Cascades updated in §7.3 strategy
      table, §7.4 persistence-tier table + completion-fraction
      footer, §L2.10.7 Mate row note, §6.5.2 per-target
      considerations. Motivation: Rao & Georgeff AI4 nested-intention
      semantics applied end-to-end against the iter-2-observed
      gate-starved Mate regression (0% snapshots / 0 matings in
      `docs/balance/social-target-range.report.md`).
- [x] **§9** — Faction model enumeration. §9.0 reconciles the
      `Species` vocabulary with §5.6.6.1's 10-row species set
      (Cat / Fox / Hawk / Snake / ShadowFox / Mouse / Rat / Rabbit /
      Fish / Bird) backed by the code-level `SensorySpecies` union.
      §9.1 commits a 10 × 10 directed-pair base matrix (100 / 100
      stance cells) with 9 footnotes covering asymmetries (Snake ≠
      Cat, Hawk-vs-Cat kitten carve-out, Fox × ShadowFox =
      conservative Neutral pending a lore-hostility code path,
      aquatic carve-out for Fish, rat-on-mouse). §9.2 adds a
      four-marker overlay (`Visitor`, `HostileVisitor`, `Banished`,
      `BefriendedAlly`) with committed most-negative-wins resolution
      order, scoped against the §12 ToT belief boundary. §9.3 binds
      five target-taking DSEs (`SocializeDse`, `AttackDse`,
      `FleeDse`, `HuntDse`, `FoxRaidDse`) to their accepted stance
      sets. §7.M.7 fertility remains the sole open "Holes identified"
      item.

**Holes identified 2026-04-21 — to be filled in a future pass of this
spec:**

- [x] **§7.M.7** — Fertility state specification. Resolved
      2026-04-21: `Fertility { phase: FertilityPhase,
      cycle_offset, post_partum_remaining_ticks }` lifecycle
      committed. Gender↔role canon (§7.M.7.4): Queens gestate,
      Toms sire, Nonbinaries do both. Toms never carry
      `Fertility`; Queens and NBs carry it while Adult and
      not-Pregnant. Insert on Young→Adult transition (`growth.rs`,
      gender-gated to skip Toms); remove on Adult→Elder or
      `MateConceived`. Phase expanded from 4 → 5 variants —
      dedicated `Postpartum` added (§7.M.7.3, scoring-equivalent
      to Anestrus but narratively distinguishable). Phase evolves
      as a pure function of `(tick + cycle_offset, season,
      post_partum)` evaluated by a new `src/systems/fertility.rs`
      at 100-tick cadence. Winter → Anestrus for all carrying
      cats. §7.M.7.5 phase→scalar mapping drives §6.5.2 Logistic
      (with a Tom-target fallback for missing markers); §7.M.7.6
      asymmetric hard gate ("at-least-one gestator in
      non-`{Anestrus, Postpartum}` phase") + soft geometric-mean
      Logistic drive §7.M.4 L2/L3 belief proxies. Implementing PR
      also corrects `resolve_mate_with` to land `Pregnant` on the
      gestator rather than the action-runner. New
      `FertilityConstants` block (or flat `fertility_*` fields) on
      `SimConstants`; existing `mating_fertility_*` season
      multipliers retained as secondary environmental modulation.
      Verisimilitude hypothesis: mating events ↓30–55%, kittens
      stable, bond evolution unchanged, events cluster in
      ~2000-tick Estrus windows per gestator; `Pregnant`-on-Tom
      count drops to zero.
- [x] **§4.3** — Marker catalog completeness for `Fertility` /
      `Apprentice` / `Parent`. Closed 2026-04-21. A new Reproduction
      subcategory lands between SpawnImmutable and Fox-specific with
      two rows — `Fertility { phase }` (Absent, shape committed,
      lifecycle pending §7.M.7) and `Parent` (Absent, active-not-
      lifetime with `growth.rs::update_parent_markers` authoring
      system). Apprentice's §6.5.3 "pending §4" flag was stale —
      reconciled by pointing the row at the existing §4.3 Role block
      entry (Partial — `Apprentice` derived from `skills.rs::Training`).
      §6.5.2's fertility-window row now
      cites the new Reproduction block; §6.5.6's kinship row reads
      `target.KittenDependency` directly (the directed-pair predicate
      is canonical on the child; `Parent` is the self-side query-
      optimization counterpart, not the pair predicate's source).
      Block prose commits active-parenthood semantics, the `Parent`-
      vs-grief ordering contract (survivors_by_relationship in
      `CatDied` payload is canonical, not `With<Parent>` post-death),
      and the §7.M.7 deferral for fertility lifecycle. §5.6.8 summary
      count bumped 42 → 44 Absent; §4.6 authoring-system roster
      extended (growth.rs gains `Parent`, new TBD-per-§7.M.7 bullet
      for `Fertility`).

**Tracked in `docs/open-work.md` entry #13, not here (follow-on,
code, or external-system work):**

These items were previously carried in this ledger but close in
other systems or in code, not in this spec. Each item's
substrate-side contract is committed above; what remains is the
target-system work.

- **Retired scoring constants + incapacitated branch cleanup** —
  code change. §2.3's "Retired constants" subsection names the
  fields; the deletion lands in the A1 IAUS-refactor PR (open-work
  cluster A, #5) — not before the Logistic curves that replace them.
- **`needs.warmth` split** → `needs.temperature` + a
  `social_warmth` fulfillment axis. Design committed in
  `docs/systems/warmth-split.md` and referenced from §7.W.4(b);
  tracked as open-work entry #12 with three landing phases.
- **§7.7.b grief emission** — `src/systems/death.rs` vocabulary
  expansion for `CatDied { cause, deceased,
  survivors_by_relationship }`. ToT-adjacent; gated on formal
  relationship modeling beyond the current three-tier `BondType`.
- **§7.7.c fate event-vocabulary expansion** — `src/systems/fate.rs`
  today emits only FatedLove / FatedRival. Blocked on the Calling
  subsystem design per `docs/systems/the-calling.md`.
- **§7.7.d mood drift-threshold detection** —
  `src/systems/mood.rs` hysteresis + sustain-duration detection
  layer. Gated on per-arc valence targets, which land with the
  aspiration catalog.
- **§7.7.1 aspiration-compatibility pair list** — the four
  conflict classes are committed here; the specific
  hard-logical / hard-identity pair list lands in the PR that
  enumerates aspirations themselves.
- **§7.3 coordinator-directive Intention strategy row** — commits
  to `SingleMinded` with a coordinator-cancel override here; the
  full specification lands with the coordinator DSE (open-work
  #1 sub-3 / cluster C4).

**Explicitly not on this list — open-set by design, not enumeration
debt:**

- §5.6.2 sensory channels — contract is "accept new channel as
  registration, not refactor"; not a fixed set.
- §5.6.9 extensibility constraints — specifies invariants, not
  instances.
- §5.6.10 pre-build checklist — live checklist; items are added and
  removed as implementation progresses.
- §10 baseline-feature unblock map — phasing index that grows with the
  feature backlog.

## What this document is for

This is the durable end-state design for Clowder's AI substrate. It
names the architectural pieces that replace the current hand-authored
per-action scoring layer in `src/ai/scoring.rs` and that unblock the
twelve Aspirational system stubs in `docs/systems/*.md`.

A fresh session should read, in order: this file's §Motivation and
§Current state; then skim §Architectural vision; then jump to whichever
numbered section (§1–§10) is relevant to the task at hand.
Cross-references to source files carry line numbers; cross-references
to Mark's book carry chapter numbers so a reader can go to the
extracted reference markdown under `docs/reference/`.

Related indexes:
- `docs/open-work.md` — tactical queue; all cluster-A/B/C/E entries
  fold into one epic pointing at this doc once the spec stabilizes.
- `docs/wiki/systems.md` — auto-generated status per design stub;
  mirrors which stubs move Aspirational → Partial → Built as the
  capabilities below land.
- `docs/balance/*.md` — balance-change iteration logs; individual
  considerations may spawn their own iteration threads.

## Motivation

The `docs/systems/*.md` stubs are not aspirational wishlist — they are
the intended baseline simulation. Of 18 documented systems, 4 are
Built (Collective Memory, Corpse Handling, Magic, Weather), 2 are
Partial (Body Zones, World Generation), and 12 are Aspirational
(Disease, Environmental Quality, Mental Breaks, Sensory modulation,
Recreation, Substances, Sleep That Makes Sense, The Calling, Trade &
Visitors, Organized Raids, Strategist-Coordinator, plus Body Zones /
World Gen promotion). Each Aspirational stub is blocked on a
capability this refactor adds.

Designing this substrate is therefore not "architecture cleanup." It
is "finish the baseline game by adding the capabilities everyone has
been waiting on." §10 maps each capability to the stubs it unblocks.

## Current state

Facts captured 2026-04-20 (seed-42 soak at commit `039c6fb`):

- **`src/ai/scoring.rs`** — 2,817 lines. Contains 23 action blocks
  (lines 177–758) plus 5 post-scoring modifiers (lines 664–750: pride,
  independence, patience, tradition, fox-territory-suppression,
  corruption-suppression). Each action is hand-authored with an
  always-linear response applied to its inputs. Action composition
  is additive with one `Needs::level_suppression` multiplier.
- **`ScoringContext`** (`src/ai/scoring.rs:27–144`) — 27 boolean
  eligibility gates + 19 scalar floats + 4 counts + 6 refs/enums.
  The boolean:scalar ratio (27:19) tells the story: eligibility is
  expressed imperatively ("can this action fire?") rather than
  declaratively ("what markers characterize this entity?").
- **`ScoringConstants`** (`src/resources/sim_constants.rs:939–1140`) —
  57 flat `_scale` / `_threshold` / `_weight` fields. `SimConstants`
  as a whole is 196 tuning knobs. Curve shape is implicit in scalar
  multipliers; there is no named curve primitive.
- **`Needs::level_suppression`** (`src/components/physical.rs:249`) —
  a Maslow-ordered multiplicative cascade where higher tiers gate on
  all lower tiers' satisfaction. This is already non-linear
  composition and generalizes cleanly as a hierarchical pre-gate
  above the axis-multiplication layer.
- **`src/systems/wind.rs` + `src/systems/sensing.rs`** — a de-facto
  proximity influence map: locus at a scent source, radiating with
  decay, accumulating across multiple emitters. The pattern exists,
  is not recognized as an instance of the general abstraction, and is
  not reusable for other spatial slow-state.
- Post-scoring modifiers handle fox territory (multiplicative
  suppression), corruption (multiplicative suppression), ally count
  (additive Fight bonus), carcass detection (boolean gate + count),
  and social fading across map distance. All ad-hoc, none sharing
  infrastructure.

Five existing ad-hoc non-linear elements (Sleep day-phase additive,
Fight conditional health/safety factors, Herbcraft/PracticeMagic max-
selection, fox/corruption multiplicative suppression) show that
pieces of IAUS-like math *already exist* inside `scoring.rs` — just
expressed inconsistently. This substrate makes them uniform.

## Architectural vision

The substrate has three layers, from bottom up:

**L1 — Shared slow-state.** Influence maps per sensory channel × per
faction, plus declarative ECS marker components for categorical state.
Agents read this state through per-species sensory filters, not
directly. This layer is the *input* to the deliberation process.

**L2 — Decision Score Evaluators (DSEs).** Each action is a DSE: a
bundle of (considerations, context filter, scoring rule). A
consideration takes a scalar OR positional input, passes it through
a named response curve, and emits a `[0, 1]` score. Considerations
compose multiplicatively across axes with Mark's compensation factor;
Maslow's pre-gate wraps the whole composition.

**L3 — Commitment and selection.** Intention/momentum carries
commitment forward across ticks (per ch 15, stubbed). Softmax
variation selects from top-scoring DSEs (per ch 16, already partial
in Clowder).

The design is deliberately *not* a new engine. Most of L1 replaces or
generalizes code that already exists. L2 replaces `scoring.rs`'s
per-action blocks. L3 adds a thin commitment layer above the current
GOAP planner.

---

## §0 Design principles

Four framing principles that cut across every section below. Sourced
from Tynan Sylvester's GDC 2017 "RimWorld: Contrarian, Ridiculous, and
Brilliant," sharpened through design conversation to fit Clowder's
zero-agency player. These aren't new systems — they're the rubric
every decision in §1–§L2.10 is measurable against, and the filter
every feature this substrate unblocks (§10) has to pass.

### §0.1 The simulation is the director is the player

Sylvester's "this isn't a game, it's a story generator" framing lands
more strongly on Clowder than on RimWorld itself, and the reason is
structural, not stylistic.

In conventional game design, three roles are distinct:
- **Player** — an external actor making choices inside the simulation.
- **Director** (RimWorld's storyteller) — a pacing layer calibrating
  event frequency and challenge against *player skill*.
- **Simulation** — the substrate running actors and events.

Clowder collapses these into one. The cats *are* the actors — each is a
BDI agent (§L2.10) authoring its own arc inside the sim. The ecology —
seasons, predator-prey oscillation, corruption cycles, weather — *is*
the director, producing the pressure curve natively because it doesn't
need to know what "skill" to calibrate against; it's just the world
cycling. Both actors and director are internal to the simulation. The
human sits outside the loop, a pure observer pattern-matching on
output.

This is what `CLAUDE.md`'s "honest world, no director" block is
claiming, and it's not a stylistic choice. A RimWorld-style director
exists to keep the player on their mastery edge. Clowder has no player
skill to calibrate against, so the role is vacant — not rejected, but
without a target function. Trying to re-introduce one would mean
inventing a modulator with no actor to modulate for. Future readers
tempted by Sylvester-envy to re-open this question: the answer is the
ecology already is the director; there is nothing left for a second
one to do.

North-star consequence for every design decision in this substrate:
*does this produce sim state that reads as narrative to a passive
observer?* This is strictly weaker than §10's "unblocks a stub" test —
stubs-unblocked is a proxy for narrative surface, not a substitute for
it. A capability can unblock a stub and still fail the narrative test
if the stub ships as a character-inert stat-buff (§0.4).

### §0.2 Elastic failure

A failure mode that propagates consequence without terminating the
generative arc. Sylvester's example was RimWorld colonists breaking
under stress in different shapes (catatonic wander, food binge, insult
spiral) — each a recovery-possible consequence, not a colony-ending
event. Clowder's version: a hunt misses (hunger compounds into
condition loss into possible starvation next season), a ward collapses
(corruption creeps into fox incursion into territory shrink), a bond
partner dies (grief cascades into secondary decompensation), a GOAP
plan fails (Intention effective score degrades, another DSE takes the
cat forward).

The substrate must preserve elasticity at four layers:

- **Composition (§3.2).** The compensation factor softens low-axis
  scores rather than zeroing them. This is elastic failure applied to
  composition: a single underperforming axis is a *weakening signal*,
  not a *kill signal*. Pure multiplicative composition would be
  brittle.
- **Commitment (§7).** Rao & Georgeff's open-minded commitment
  strategy is elastic (release on target invalidation); blind
  commitment is brittle (hold until explicit termination). Synthesis
  work against ch 15 picks per Intention class, not globally.
- **Target selection (§6).** A target that vanishes (moved out of
  range, died, deprioritized) re-ranks smoothly into the next-best
  candidate. The `TargetTakingDse`'s `Best` aggregation preserves this
  naturally; the anti-pattern `has_X_target: bool` collapse (§6.1) is
  also an elastic-failure bug — it hard-gates the whole DSE on
  existence rather than letting target quality degrade gracefully.
- **Slow-state decay (§5.3).** Corruption, wind, ward strength decay
  on a curve rather than toggling. Already elastic; §0.2 names the
  principle.

Anti-goals to watch for during implementation and balance review:
single-axis score = 0 propagating to whole DSE, commitment holding
past target invalidation, binary transitions in slow-state (a ward
"fails" by flipping off rather than eroding), hard-rejection of a
scored Intention when GOAP can't immediately plan it (see §L2.10.7).

### §0.3 Apophenia has two legs: abstracted feedback and long-term relevance

Apophenia is the observer inferring pattern and intentionality from
procedural output. It's load-bearing for Clowder specifically because,
per §0.1, there's no external storyteller cleaning up narrative
threads — the observer is the sole source of interpretation, and the
substrate has to leave them *space* to do the work.

Two decomposed requirements on the substrate:

**1. Abstracted feedback.** The sim presents *what happened*, not
*why*. The observer does the causal attribution. A narrative system
that says "Whiskers is sad because her bondmate died" kills apophenia
— the observer has nothing left to infer, and the line reads like a
pop-up. A system that shows Whiskers disengaging from grooming,
sleeping in a different spot, and declining food for three days, with
a narrator that only says *"Whiskers hasn't groomed in three days,"*
invites pattern-recognition and owns the meaning when the observer
constructs it.

Clowder's existing tiered narrative emission (Micro / Action /
Significant / Danger / Nature — `src/resources/narrative_templates.rs`)
should stay close to "X happened" and resist drifting to "X happened
*because Y*". That corpus is an ongoing editorial discipline, not a
one-time spec.

**2. Long-term relevance.** Patterns are temporal. Observations only
compound into narrative if per-cat state persists and influences
future state — an injury that lingers and shifts behavior, a bond that
deepens over seasons, a skill that accumulates into preference, an
aspiration that carries across a lifetime. A sim where every tick is
fresh has no apophenia surface even if moment-to-moment behavior is
rich.

Substrate consequences, grouped by leg:

- *For abstracted feedback*, the substrate emits legible primitives —
  Intentions (§L2.10.4), named considerations (§1), per-consideration
  contribution rows in `logs/events.jsonl` — and declines to emit
  pre-interpreted summaries. "She wants to mother that kitten" is a
  hook the observer carries forward; a scored action label isn't.
- *For long-term relevance*, state carries. Momentum (§7) extends
  commitment across ticks so arcs form. Per-cat memory
  (`src/systems/memory.rs`), aspirations (`src/systems/aspirations.rs`),
  bonds and grief cascades, persistent injury, and skill accumulation
  are the substrate's long-horizon surface. They are what make a
  pattern noticed on day 40 feel like it *started* on day 12.
  Apophenia requires these to be present and *visible through
  behavior*, not just logged.

Calibration of §8 softmax temperature lives across both legs: too cold
reads as inert (no variation to abstract over), too hot reads as
random (no relevance across time). Target feel is "a cat that
surprises but stays in character across weeks."

Meta-point: because §0.1 collapses the director role into the
simulation itself, there is no external storyteller cleaning up
narrative threads. The substrate carries the full legibility budget
alone — through abstracted presentation and long-horizon state
continuity — and the observer's pattern-matching does the rest.

### §0.4 Mechanics must express character, not just apply modifiers

Sylvester's design discipline in the same talk: reject design spaces
whose interactions with a character don't add to that character's
emotional arc. Applied to Clowder: every mechanic must answer *"what
does this say about who this cat is?"* A stat-buff-style mechanic
("wear item → +5 to stat") is apophenia-inert (§0.3) — it gives the
observer nothing to attribute character meaning to. A character-
expressive mechanic makes the same mechanical contribution *while
saying something about the cat who has it*.

Worked example: armor. *Armor-as-buff* ("wear helmet → +defense")
fails the filter — the helmet tells the observer nothing about the
wearer. *Armor-as-class-expression* ("the scout wears nothing,
warriors wear heavy, ward-bearers wear decorated light armor — armor
is a visible signifier of the path this cat has chosen") passes —
mechanical contribution is similar, narrative surface differs
completely.

Clowder is already well-aligned by existing design ethos: personality
traits shape scoring preferences, not just efficiency; magic affinity
gates who a cat can *become*, not just what buffs they get; skills
change behavioral preferences, not just roll modifiers; aspirations
are lifetime character arcs. §0.4 names the principle so it becomes an
evaluable filter on *future* mechanic proposals, especially as §10's
feature queue lands.

Filter question for every future mechanic: **"Would the sim tell a
different story about this cat if this mechanic's value were
different?"** If yes (armor-as-class, skill-as-behavioral-preference,
aspiration-as-lifetime-arc), the mechanic belongs. If no (stat-buff
consumables, numeric upgrades that don't shift behavioral disposition,
level gates with no expressive content), it doesn't — even if it would
work mechanically. This principle feeds §0.3 directly:
character-expressive mechanics *give the observer new axes of
attribution*; character-inert mechanics consume legibility budget
without returning apophenia.

### §0.5 Cross-refs

- Tynan Sylvester, "RimWorld: Contrarian, Ridiculous, and Brilliant" —
  GDC 2017. Source of all four principles, not transcribed.
- `docs/systems/project-vision.md` — "honest world, no director" is
  §0.1 stated in design-principle form. §0.1 supplies the
  structural argument behind it.
- Rao & Georgeff (1991) — BDI architecture; §0.2's commitment
  vocabulary (blind / single-minded / open-minded) comes from the
  paper's commitment-strategies section.

---

## §1 Considerations — the scoring atom

A consideration is a named function `input → [0, 1]` that takes a
scalar, positional, or marker input and returns a normalized score.
Each DSE composes some number of considerations (see §3) to produce
one score for its candidate Intention (§L2.10).

### §1.1 Trait shape

```rust
pub trait Consideration {
    /// Evaluate this consideration for a cat in a context.
    /// Returns a strict [0, 1] score — see §1.3 on normalization.
    fn score(&self, cat: Entity, ctx: &ConsiderationCtx) -> f32;

    /// Name for per-axis diagnostic logging (see logs/events.jsonl).
    fn name(&self) -> &'static str;
}
```

`ConsiderationCtx` carries three kinds of access, enough to implement
all three consideration flavors below:
- **Scalar state refs** — needs (`&Needs`), personality (`&Personality`),
  skills, health, inventory aggregates. Borrowed, never cloned.
- **Influence-map sampler** — `fn sample_map(channel, pos) -> f32`, the
  L1 surface from §5. Required because spatial considerations are
  first-class (below).
- **ECS world access for marker queries** — so a consideration can ask
  "does this cat have `Injured`?" without requiring the caller to have
  pre-queried it. Replaces the 27 boolean fields in today's
  `ScoringContext` (see §4).

### §1.2 Three flavors of consideration

Mark ch 13 §"Weighting a Single Criterion" splits inputs into *concrete
numbers* (counts, distances, damage) and *abstract ratings* (satisfaction,
desire). Clowder adds a third flavor, *spatial*, because L1 is
influence-map-based.

| Flavor | Input | Example | Mark-cite |
|---|---|---|---|
| `ScalarConsideration` | One scalar (`f32`) from ctx | `(1 - hunger)` through a Logistic curve for `Eat` | Ch 13 §"Concrete Numbers" / §"Abstract Ratings" |
| `SpatialConsideration` | One position + one channel, sampled via L1 | `distance_to(target) → Quadratic(exp=2)` for `Socialize`; `fox_scent_at(my_pos) → Logit` for threat avoidance | IAM ch 30 §"Personal-Interest Template" (generalized; ch 13 doesn't cover) |
| `MarkerConsideration` | One ECS marker query | `With<HasFunctionalKitchen>` as a 0/1 gate for `Cook` | Ch 14 §"Context Tags" (Clowder §4 restates in ECS terms) |

The personal-interest template from IAM ch 30 is *not* a new primitive —
it's a `SpatialConsideration` parameterized by `(center = self.pos, curve
= Quadratic(…))`. Any DSE, including target-taking DSEs (§6), composes one
or more of these alongside `ScalarConsideration`s. This is why the trait
must accept positional inputs from the start; L2 bolted-on spatial
support later would force re-shaping every DSE.

### §1.3 Resolved open questions

Three questions from the prior stub, now answered against ch 12/13:

- **Strict `[0, 1]` normalization?** Yes. Ch 13 §"Normalizing" makes
  this the whole-chapter thesis: shared scale is what enables
  composition (§3), layered weighting models, and cross-DSE
  comparability. Considerations that want to *contribute more* should
  express that via their DSE's weight (§3), not by emitting
  out-of-range scores.
- **Canonical trait shape from Mark's code?** Mark's code (ch 12
  `sBUCKET`, ch 14 weapon-damage/accuracy formulas) uses concrete
  structs, not a Rust-style trait — but the separation of (input →
  curve → score) is consistent across every example. Our
  `Consideration` trait is the Rust-idiomatic version of that pattern;
  the curve lives inside the consideration (§2), the input source is
  the consideration's job to fetch, and the output is always a
  normalized score.
- **Drifting input distributions at runtime?** Handled at the
  curve/normalization layer (§2), not the consideration. If `hunger`'s
  real-world range drifts from `[0, 1]` to `[0.3, 0.9]` due to balance
  changes, rewire the curve's anchor per ch 13 §"Weights Relative to a
  Maximum" (pick a new normalizing constant) or §"Relative to Each
  Other" (rescale against the current max). The consideration itself
  doesn't change.

### §1.4 Size and shape guidance

A consideration is cheap — one function call, possibly one curve
lookup. An action's DSE should compose **4–8 considerations** typical,
hard ceiling ~10. The current `Eat` block has 3 inputs (on the low
end); `PracticeMagic` with 6 sub-modes × 4–6 axes = 24–36 effective
considerations at the action level (way over). Post-refactor,
`PracticeMagic`'s sub-modes become sibling DSEs (§L2.10) — each with
4–8 considerations, instead of one mega-action with 36.

Ch 13 §"Granularity / Accuracy / Too Many" is the governing principle:
enough precision to differentiate meaningfully, no more. If two
considerations always move together, collapse them.

---

## §2 Response curve primitives

Linear response curves misrepresent biological response to stimuli.
Real cats don't get 1.5× as motivated to eat when hunger goes from
0.6 to 0.9 — they get ~5×. *Hangry* is a threshold phenomenon best
modeled by a logistic curve. The same is true for sleep deprivation
(panic threshold), fear (flee-or-fight switchover), loneliness (acute
onset), and cold exposure (curve, not step).

All 21 of today's DSE blocks in `src/ai/scoring.rs` use linear math
except five ad-hoc non-linearities (Sleep day-phase additive, Fight
conditional health/safety suppression, Herbcraft/PracticeMagic
max-selection, fox/corruption multiplicative suppression). §2 replaces
the ad-hoc with named curve primitives so the shape of each
consideration's response is declarative, not buried in arithmetic.

### §2.1 Curve primitive enum

```rust
pub enum Curve {
    Linear { slope: f32, intercept: f32 },
    Quadratic { exponent: f32, divisor: f32, shift: f32 },
    Logistic { steepness: f32, midpoint: f32 },
    Logit { slope: f32, inflection: f32 },
    Piecewise { knots: Vec<(f32, f32)> },
    Polynomial { exponent: u8, divisor: f32 }, // exponent 1..=4
    Composite { inner: Box<Curve>, post: PostOp }, // clamp, invert, etc.
}
```

| Primitive | Shape params | Used for | Mark-cite |
|---|---|---|---|
| `Linear` | slope, intercept | Trivial mappings; default when no better-fitted curve is known | Ch 12 §"Simple 1-to-1 Mappings" |
| `Quadratic` | exponent, divisor, shift | Influence-map falloff; damage/accuracy vs. distance | Ch 14 §"Choose Your Weapon" (worked example); IAM ch 30 §30.3 |
| `Logistic` | steepness, midpoint | Threshold urgency: hangry, panic, flee-or-fight | Ch 10 S-curve; ch 12 implicit via piecewise |
| `Logit` | slope, inflection | Inverse urgency: satisfaction, calm, decay of alertness | Ch 10 inverse-S |
| `Piecewise` | knot points | Hand-crafted curves with inflection(s); day-phase behavioural modulation | Ch 12 §"Hand-Crafted Response Curves" |
| `Polynomial` | exponent (1–4), divisor | IAM threat/proximity templates | IAM ch 30 template library |
| `Composite` | inner + post-op | Clamp, invert-to-`(1 - x)`, apply min-floor | Ch 12 §"Adjusting Data" (runtime tweaks) |

### §2.2 LUT backing — start function-evaluated

Ch 12 is *titled* "Response Curves," and ~80% of the chapter is LUT
machinery: `sBUCKET` struct, `AddBucket` / `RebuildEdges`, binary-
search retrieval, and `FillVector` to bake a formula into a table. The
rationale is (a) hand-tweakability of specific x-values, (b)
selection-from-distribution (bucket semantics), and (c) runtime
weight-adjustment via `RebuildEdges`.

For Clowder's per-tick scoring, direct function evaluation is cheap.
Back-of-envelope: 8 cats × 23 actions × 5 avg considerations = 920
samples/tick. Each primitive evaluates in <100 ns (even `Logistic`,
which is one `exp()`), so total curve-sampling cost is <100 µs/tick
— negligible against a 16 ms budget at 60 TPS.

**Recommendation:**
- Start function-evaluated. `Curve::evaluate(x: f32) -> f32` is the
  canonical API; LUT is an implementation detail.
- Keep LUT as an optional `LutBacked<C> { curve: C, table: Vec<f32> }`
  wrapper for three cases:
  1. **Hand-crafted curves where the curve *is* the data** (ch 12
     §"Hand-Crafted") — no formula to evaluate, so the LUT is the
     ground truth.
  2. **Hot-path curves where profiling shows a win** — don't
     pre-optimize; measure first.
  3. **Runtime-adjustable curves** (ch 12 §"Dynamic Response Curves" +
     §"Adjusting Data") — if balance tuning or designer tools want to
     tweak one x-value without re-deriving a formula, LUT storage is
     the natural fit.
- API should support both transparently: `Curve::evaluate` works for
  both enum variants and `LutBacked` wrappers.

### §2.3 Curve-shape assignment table

This table commits a curve primitive for every consideration axis in
every current scoring site — 21 cat DSEs in `src/ai/scoring.rs` and 9
fox dispositions in `src/ai/fox_scoring.rs`. One row per consideration
axis, since curve shape is a property of the axis, not the DSE. Rows
are grouped by agent then by Maslow tier.

**Anchoring rules** — two DSEs sharing an axis share a shape. Anchors
are the rows every derived row cites; they're called out in the
rationale column so drift between sister DSEs is caught at review:

| Axis semantics | Default curve | Anchor row |
|---|---|---|
| Threshold urgency (hangry, panic, flee-or-fight, sleep-dep) | `Logistic(steepness, midpoint=need_threshold)` | `Eat.hunger = Logistic(8, 0.75)` — steepness tuned relative to this |
| Marginal utility of scarcity (food, prey, herbs) | `Quadratic(exponent=2)` | Ch 13 §"Soldiers, Sailors, Airmen" soldier curve |
| Inverted-need penalty (satisfaction → penalty contribution) | `Composite { inner: Logistic, post: Invert }` | `Socialize.phys_satisfaction = Composite{ Logistic(5, 0.3), Invert }` |
| Personality / skill scalar (boldness, diligence, compassion, herbcraft_skill, …) | `Linear` | Personality and mastery are already bounded `[0, 1]` preference coefficients; a curve on top obscures tuning. Upgrade only when profiling shows a behavior miss. |
| Diurnal phase | `Piecewise(4 knots: dawn/day/dusk/night)` | `Sleep.day_phase` — shape reused across cat & fox with different knot values |
| Piecewise threshold (health / safety gating combat) | `Piecewise(3–4 knots)` | `Fight.health = Piecewise([(0,0), (0.3,0.2), (0.5,1.0), (1.0,1.0)])` |
| Saturating count (directives, allies, carcasses, cats-nearby) | `Composite { inner: Linear, post: Clamp(max=cap) }` | Implicit in `.min(cap)` today; make the clamp a primitive |
| Spatial presence (`X_nearby`, `has_X_within_range`, distance-to-nearest-X as a score contribution) | `SpatialConsideration(center=self.pos, map=<X-location map from §5.6.3>, curve=Quadratic(exponent=2))` — IAM personal-interest template | §6.4's `Hunt` row is the anchor. Distinct from eligibility gates: a spatial axis contributes *continuously* to the score; a gate is a threshold projection of the same map used for eligibility only. |
| Bool eligibility gate (`has_X`, `is_Y`, `X && !Y`, map-sample-above-threshold) | Not a curve — ECS marker filter at evaluator level (§4), possibly produced by projecting an influence map through a threshold | Rows note `context-tag` rather than a primitive. If a bool is used to *add to the score* (not gate the DSE), it's a spatial axis, not a tag. |

Table legend: **Today** = current math in `scoring.rs` / `fox_scoring.rs`; **Proposed** = `Curve` primitive from §2.1; **Rationale** = one-line citation of Mark's chapter or Clowder-specific ecology.

#### Cat DSEs — Tier 1 (physiological / survival)

| DSE | Consideration | Today | Proposed | Rationale |
|---|---|---|---|---|
| `Eat` | hunger | `(1 - hunger)` linear | `Logistic(steepness=8, midpoint=0.75)` | **Hangry anchor.** Threshold, not ramp (ch 13 pain-scale analogy). |
| `Sleep` | energy deficit | `(1 - energy) * sleep_urgency_scale` linear | `Logistic(steepness=10, midpoint=0.7)` | Sleep-dep is **steeper** than hangry — micro-sleeps are involuntary once energy drops past ~30%. |
| `Sleep` | day_phase offset | enum → additive constant | `Piecewise([(dawn, sleep_dawn_bonus), (day, sleep_day_bonus), (dusk, sleep_dusk_bonus), (night, sleep_night_bonus)])` | **Diurnal-phase anchor.** Today's enum→constant is a discrete piecewise masquerading as additive math; make it declarative. Cat is nocturnal-leaning (Night knot heaviest). |
| `Sleep` | injury rest bonus | `(1 - health) * injury_rest_bonus` conditional | `Linear(slope=injury_rest_bonus)` gated on `health < 1.0` | Recovery urgency is monotone in injury severity. |
| `Hunt` | hunger | `(1 - hunger)` in compensated product | `Logistic(steepness=8, midpoint=0.75)` | Reuse `Eat.hunger` anchor — one hunger shape across Tier-1 food-seeking DSEs. |
| `Hunt` | food-scarcity (`1 - food_fraction`) | linear × scale | `Quadratic(exponent=2)` | **Scarcity anchor.** Marginal utility of food rises sharply near scarcity (ch 13 soldier curve). |
| `Hunt` | boldness | `personality.boldness` | `Linear` | Personality coefficient. |
| `Hunt` | prey proximity | bool `prey_nearby` + additive `hunt_prey_bonus` | `SpatialConsideration(center=self.pos, map=Prey-location [§5.6.3], curve=Quadratic(exponent=2))` per §6.4's personal-interest template. Under §6, Hunt is a target-taking DSE — the bool is absorbed into per-candidate distance aggregation (`Best` over prey entities) and retires. | Today's bool is a lossy projection of the Prey-location influence map listed in §5.6.3 (row #5). The score contribution is continuous in distance, not binary. Cross-species sensory attenuation applies via §5.6.6. |
| `Forage` | hunger | linear in compensated product | `Logistic(steepness=8, midpoint=0.75)` | Reuse hangry anchor. |
| `Forage` | food-scarcity | linear × scale | `Quadratic(exponent=2)` | Reuse scarcity anchor. |
| `Forage` | diligence | linear | `Linear` | Personality coefficient. |
| `Groom` (self) | thermal deficit (post-split `needs.thermal`) | part of `(1 - warmth) * self_groom_warmth_scale` today | `Logistic(steepness=7, midpoint=0.6)` | Body-temperature threshold. Drained by weather/season (`needs.rs:52–87`), hearth-restored (`hearth_warmth_bonus_cold`). Gentler than hangry (7 vs 8) — cats thermoregulate passively. Self-grooming fluffs fur → minor thermal contribution. |
| `Groom` (self) | social-warmth deficit (post-split `needs.affection`) | part of the same `needs.warmth` field today — conflated | `Logistic(steepness=5, midpoint=0.6)` — reuse loneliness anchor | Affectionate-contact threshold. Self-grooming partially substitutes for allogrooming when no social target is available. Sibling axis to `Socialize.social`; shared shape because both are slow-building social-need axes. **Blocked on substrate fix — see "Split `needs.warmth` …" TODO.** |
| `Flee` | safety deficit (`1 - safety`) | linear + threshold early-return | `Logistic(steepness=10, midpoint=flee_safety_threshold)` | **Flee-or-fight anchor.** Canonical logistic example; today's early-return is a crude step. Steepest logistic in the catalog; shared with `DenDefense.cub_safety`. |
| `Flee` | boldness (inverted) | `(1 - boldness)` linear | `Composite { inner: Linear, post: Invert }` on `boldness` | Bold cats flee less. |
| `Flee` | threat_nearby | bool gate | Context-tag filter (§4) | Eligibility gate. |

> **Conflation flag — `needs.warmth` serves two behavioral axes today.**
> The field at `src/systems/needs.rs` drains from cold weather/season
> (thermal signal — `needs.rs:52–87`, `hearth_warmth_bonus_cold`) *and*
> is restored by allogrooming (social signal — `groom_other_warmth_gain`;
> co-occurring with the "social warmth" mood modifier at
> `mood.rs:158–192`). These are distinct needs that peak
> simultaneously in winter — a cat alone by a hearth satisfies thermal
> but not social; a cat huddled with a bonded cat satisfies both;
> today's single field can't distinguish them. The refactor splits
> `needs.warmth` into `needs.thermal` + `needs.affection`; the two
> `Groom (self)` rows above are spec'd against the split. Tracked as
> **"Split `needs.warmth` …"** in the Enumeration Debt list.

#### Cat DSEs — Tier 2 (safety / territory)

| DSE | Consideration | Today | Proposed | Rationale |
|---|---|---|---|---|
| `Fight` | boldness | linear | `Linear` | Personality coefficient. |
| `Fight` | combat_effective | linear, input already `[0, 1]` | `Linear` | Already a composite index upstream. |
| `Fight` | health-suppression | `if health < threshold { health / threshold } else { 1.0 }` | `Piecewise([(0, 0), (0.3, 0.2), (0.5, 1.0), (1.0, 1.0)])` | **Piecewise-threshold anchor.** Keep shape; name it so tuning edits one knot, not two constants. |
| `Fight` | safety-suppression | same piecewise-linear as health | `Piecewise([(0, 0), (0.3, 0.2), (0.5, 1.0), (1.0, 1.0)])` | Parallel to `Fight.health` — cat already in danger shouldn't double down. |
| `Fight` | ally count | `allies * fight_ally_bonus_per_cat` | `Composite { inner: Linear(slope=fight_ally_bonus_per_cat), post: Clamp(max=cap) }` | **Saturating-count anchor.** First ally is huge; fifth adds less. |
| `Fight` | threat_nearby + allies_fighting_threat ≥ min | compound bool gate | Context-tag filter (§4) | Eligibility gate. |
| `Patrol` | safety deficit | `(1 - safety)` linear, gated | `Logistic(steepness=6, midpoint=patrol_safety_threshold)` | Softer than `Flee` (6 vs 10) — Patrol is proactive, operates *above* Flee's threshold. |
| `Patrol` | boldness | linear | `Linear` | Personality coefficient. |
| `Build` | diligence | linear | `Linear` | Personality coefficient. |
| `Build` | site_bonus | `if has_site { bonus } else { 0 }` | `Piecewise([(0, 0), (1, build_site_bonus)])` | Binary-presence bonus as named primitive. |
| `Build` | repair_bonus | `if has_damaged_building { bonus } else { 0 }` | `Piecewise([(0, 0), (1, build_repair_bonus)])` | Same pattern as site_bonus. |
| `Farm` | food-scarcity | linear × scale | `Quadratic(exponent=2)` | Reuse scarcity anchor. |
| `Farm` | diligence | linear | `Linear` | Personality coefficient. |
| `Socialize` | social need (`1 - social`) | linear | `Logistic(steepness=5, midpoint=0.6)` | **Loneliness anchor.** Gentler than hangry (5 vs 8) — social deficit builds over days, not hours. |
| `Socialize` | phys_satisfaction (drives temper penalty) | `temper * (1 - phys_sat)` bilinear | `Composite { inner: Logistic(steepness=5, midpoint=0.3), post: Invert }` on `phys_sat`, multiplied by `temper` in composition | **Inverted-need-penalty anchor.** Bilinear interactions live in composition (§3); curve is per-axis. |
| `Socialize` | sociability | linear | `Linear` | Personality coefficient. |
| `Socialize` | temper | linear | `Linear` | Personality coefficient (enters via composition). |
| `Socialize` | playfulness bonus | additive linear | `Linear` | Additive bonus. |
| `Socialize` | tile_corruption bonus | conditional additive, gate at 0.1 | `Logistic(steepness=8, midpoint=0.1)` × `corruption_social_bonus` weight | Threshold gate absorbed into the curve; no separate `> 0.1` check. Weight stays as the axis magnitude. |
| `Groom` (other) | social deficit | reuse `(1 - social)` | `Logistic(steepness=5, midpoint=0.6)` | Reuse loneliness anchor. |
| `Groom` (other) | warmth (personality) | linear | `Linear` | Personality coefficient. |
| `Groom` (other) | temper penalty | `temper * (1 - phys_sat)` | Reuse `Composite { Logistic(5, 0.3), Invert }` | Reuse inverted-need-penalty anchor. |
| `Explore` | curiosity | linear | `Linear` | Personality coefficient. |
| `Explore` | unexplored_nearby | linear | `Linear` | Already a bounded coverage fraction. |
| `Wander` | curiosity | linear | `Linear` | Personality coefficient. |
| `Wander` | base constant | `wander_base` | `Linear(intercept=wander_base)` | "Always available" sentinel as linear intercept. |
| `Wander` | playfulness bonus | additive linear | `Linear` | Additive bonus. |
| `Cook` | food-scarcity | linear × scale | `Quadratic(exponent=2)` | Reuse scarcity anchor. |
| `Cook` | diligence | linear | `Linear` | Personality coefficient. |
| `Cook` | hunger > cook_gate, has_functional_kitchen | bool gates | Context-tag filter (§4) | Eligibility gates. |

#### Cat DSEs — Tier 2–5 (craft / leadership / reproduction / care / idle)

> `Herbcraft` and `PracticeMagic` use `Max`-composition today over 3 and
> 6 sub-modes respectively. §L2.10 resolves this by splitting each into
> sibling goal-shaped DSEs. This table enumerates the sub-mode axes at
> their current granularity; the **Herbcraft / PracticeMagic sibling-DSE
> curve specs** TODO picks up the rest once §L2.10 names the final
> children.

| DSE | Consideration | Today | Proposed | Rationale |
|---|---|---|---|---|
| `Herbcraft.gather` | spirituality | linear | `Linear` | Personality coefficient. |
| `Herbcraft.gather` | herbcraft_skill | `skill_offset + skill` linear | `Linear(intercept=herbcraft_gather_skill_offset)` | Mastery is linear in `[0, 1]`; offset preserved. |
| `Herbcraft.gather` | corruption emergency bonus | compound conditional flat bonus | **Retired** — replaced by `Logistic(steepness=8, midpoint=0.1)` on `territory_max_corruption` at the sibling-DSE level. | The flat emergency bonus was a workaround for the linear gather score being too small to compete with Hunt when corruption appears. A proper Logistic on the corruption axis produces the surge naturally; the bonus constant retires. |
| `Herbcraft.prepare` | compassion | linear | `Linear` | Personality coefficient. |
| `Herbcraft.prepare` | colony_injury_count (saturating) | `(count * scale).min(cap)` | `Composite { inner: Linear(slope=herbcraft_prepare_injury_scale), post: Clamp(max=herbcraft_prepare_injury_cap) }` | Reuse saturating-count anchor. |
| `Herbcraft.ward` | spirituality | linear | `Linear` | Personality coefficient. |
| `Herbcraft.ward` | corruption emergency bonus | compound conditional flat bonus | **Retired** — `Logistic(steepness=8, midpoint=0.1)` on `territory_max_corruption` absorbs the emergency surge at the axis level. | Same workaround pattern as `Herbcraft.gather.emergency`. |
| `Herbcraft.ward` | siege bonus | conditional additive | `Piecewise([(0, 0), (1, herbcraft_ward_siege_bonus)])` | Binary-presence bonus — legitimate compound-condition surge (wards actively under attack), not a curve-shape workaround. |
| `PracticeMagic.scry` | curiosity × spirituality × magic_skill | linear product | `Linear` per axis (product lives in composition §3) | Personality × skill. |
| `PracticeMagic.durable_ward` | spirituality × magic_skill | linear product | `Linear` per axis | Personality × skill. |
| `PracticeMagic.durable_ward` | ward_emergency bonus | compound conditional flat bonus | **Retired** — folded into the Logistic on `territory_max_corruption`. | Same workaround pattern. |
| `PracticeMagic.durable_ward` | nearby_corruption_level | `corruption_sensed_response_bonus * level` with `> 0.1` gate | `Logistic(steepness=8, midpoint=0.1)` × axis weight | Threshold-check + linear-scale pair collapses into one Logistic. `corruption_sensed_response_bonus` retires; axis weight lives in the composition layer (§3.3). |
| `PracticeMagic.cleanse` | tile_corruption | linear × skill | `Logistic(steepness=8, midpoint=magic_cleanse_corruption_threshold)` | Threshold-gated cleansing — corrupted tile is a "now" problem, not a ramp. |
| `PracticeMagic.colony_cleanse` | territory_max_corruption | linear × scale | `Logistic(steepness=6, midpoint=0.3)` | Softer than tile cleanse — territory-wide corruption drives earlier but less sharp response. |
| `PracticeMagic.harvest` | carcass_count (saturating) | `.min(3)` | `Composite { inner: Linear, post: Clamp(max=3) }` | Reuse saturating-count anchor. |
| `PracticeMagic.harvest` | herbcraft_skill | `skill + 0.1` | `Linear(intercept=0.1)` | Tiny offset preserved. |
| `PracticeMagic.commune` | on_special_terrain | bool gate | Context-tag filter (§4) | Eligibility gate. |
| `Coordinate` | diligence | linear | `Linear` | Personality coefficient. |
| `Coordinate` | pending_directive_count | `count * directive_scale` linear | `Composite { inner: Linear(slope=coordinate_directive_scale), post: Clamp(max=cap) }` | Reuse saturating-count anchor — one vs. ten directives shouldn't produce a 10× score. |
| `Coordinate` | ambition bonus | linear | `Linear` | Personality coefficient. |
| `Coordinate` | is_coordinator_with_directives | bool gate | Context-tag filter (§4) | Eligibility gate. |
| `Mentor` | warmth × diligence | bilinear | `Linear` per axis | Personality coefficients (bilinear lives in composition §3). |
| `Mentor` | ambition bonus | linear | `Linear` | Personality coefficient. |
| `Mentor` | has_mentoring_target | bool gate | Context-tag filter (§4) | Eligibility gate. |
| `Mate` | mating need deficit (`1 - mating`) | linear | `Logistic(steepness=6, midpoint=0.6)` | Reproductive urgency threshold — seasonal receptivity + cumulative need produce an inflection, not a linear rise. |
| `Mate` | warmth | linear | `Linear` | Personality coefficient. |
| `Mate` | has_eligible_mate | bool gate | Context-tag filter (§4) | Eligibility gate. |
| `Caretake` | hungry_kitten_urgency | linear | `Linear` | Already composed `[0, 1]` urgency from pregnancy system; curve lives upstream. |
| `Caretake` | compassion | linear | `Linear` | Personality coefficient. |
| `Caretake` | parent bonus | conditional additive | `Piecewise([(0, 0), (1, caretake_parent_bonus)])` | Binary-presence bonus. |
| `Caretake` | hungry_kitten_urgency > 0 | implicit gate | Context-tag filter (§4) | Eligibility gate. |
| `Idle` | idle_base | constant | `Linear(intercept=idle_base)` | "Always available" sentinel. |
| `Idle` | incuriosity (`1 - curiosity`) | linear | `Linear` | Personality coefficient, inverted. |
| `Idle` | playfulness penalty | linear subtraction | `Linear` (negative slope) | Subtractive bonus stays linear. |
| `Idle` | floor clamp | `.max(idle_minimum_floor)` | `Composite { inner: Linear, post: Clamp(min=idle_minimum_floor) }` | Named floor primitive. |

#### Incapacitated override — **retired**

`scoring.rs:181–201` routes incapacitated cats through a separate scoring branch with five bespoke constants (`incapacitated_eat_urgency_scale/offset`, `incapacitated_sleep_urgency_scale/offset`, `incapacitated_idle_score`). The branch exists because *linear* Eat/Sleep urgency didn't climb fast enough to dominate other DSEs when energy and mobility both crashed — a duplicate scoring path was added to paper over the shape problem.

Under the curve refactor, the branch retires entirely. The `Incapacitated` ECS marker (§4) filters out every DSE a downed cat can't execute; the remaining DSEs (`Eat`, `Sleep`, `Idle`) produce correct behavior on their canonical axes because `Logistic(8, 0.75)` on `Eat.hunger` and `Logistic(10, 0.7)` on `Sleep.energy` already spike hard enough to dominate without a scale multiplier. Listed in the **Retired constants** subsection below.

#### Post-scoring modifiers (§3.5 layer — cross-referenced here for completeness)

Modifiers apply *after* composition, not per-consideration. Curve-shape column below covers the trigger-side curve only; full per-DSE applicability matrix lives in **§3.5.2**.

| Modifier | Axis | Today | Proposed | Applies to |
|---|---|---|---|---|
| Fox-scent suppression | `fox_scent` above threshold | `(fox_scent - threshold) / (1 - threshold)` linear ramp + multiplicative damp | `Logit(slope=6, inflection=fox_scent_threshold)` | Hunt, Explore, Forage, Patrol, Wander (**+ additive boost on `Flee`** — see §3.5.3) |
| Corruption suppression | `corruption` above threshold | conditional multiplicative damp | `Logit(slope=6, inflection=corruption_threshold)` — share fox-scent shape | Explore, Wander, Idle |
| Pride bonus | `respect` below threshold | conditional additive × `personality.pride` | `Piecewise([(0, pride_bonus), (pride_respect_threshold, 0)])` × `Linear` on `pride` | Hunt, Fight, Patrol, Build, Coordinate |
| Independence (solo boost) | always active | additive × `personality.independence × independence_solo_bonus` | `Linear` on `independence`, additive | Explore, Wander, Hunt |
| Independence (group penalty) | always active | subtractive × `personality.independence × independence_group_penalty`, clamped to ≥ 0 | `Linear` on `independence`, subtractive with `Clamp(min=0)` | Socialize, Coordinate, Mentor |
| Patience commitment bonus | `active_disposition.is_some()` | additive × `personality.patience × patience_commitment_bonus` on each constituent action | `Linear` on `patience`, additive — applied to `DispositionKind::constituent_actions()` | Dynamic (see §3.5.2 matrix's Patience column + §3.5.3 for the disposition → actions mapping) |
| Tradition location bonus | `tradition_location_bonus > 0.0` (caller-computed as `personality.tradition × 0.1`) | additive flat value applied to every scored action | `Linear` on `tradition`, additive — **filter missing, see §3.5.3 (1)** | All DSEs today (bug); future: per-DSE history match |

#### Fox dispositions (`src/ai/fox_scoring.rs`)

Truncated 3-level Maslow (Survival / Territory / Offspring). Personality fields differ from cats: `boldness`, `cunning`, `territoriality`, `protectiveness`.

| Disposition | Consideration | Today | Proposed | Rationale |
|---|---|---|---|---|
| `Hunting` | hunger deficit | `(1 - hunger)` linear | `Logistic(steepness=8, midpoint=0.75)` | Reuse cross-species hangry anchor. |
| `Hunting` | prey proximity | bool `prey_nearby` + additive `+0.3` | Same as cat `Hunt.prey_proximity` — `SpatialConsideration` sampling the shared Prey-location map (§5.6.3) with `Quadratic(exponent=2)` falloff. Fox scent-sensitivity applies via §5.6.6 attenuation. | Shared map across species; species differences expressed via `species.sensitivity(channel)`, not separate maps or curves. |
| `Hunting` | local_prey_belief | `belief * 0.2` | `Linear(slope=0.2)` on a memory-decayed sample of the Prey-location map (§5.6.3) persisted in the fox's prey-memory component | Curve stays `Linear` because the belief scalar is already the attenuated + decayed map projection. Distinct from `prey_proximity` (current-frame sample) — belief is *remembered* presence, letting a hungry fox return to recently-productive ground. |
| `Hunting` | day_phase | enum → additive | `Piecewise([(dawn, fox_hunt_dawn_bonus), (day, fox_hunt_day_bonus), (dusk, fox_hunt_dusk_bonus), (night, fox_hunt_night_bonus)])` | Reuse `Sleep.day_phase` shape; fox is crepuscular (Dusk/Night-peaked), cat is nocturnal. |
| `Hunting` | boldness with floor | `boldness.max(0.3)` | `Composite { inner: Linear, post: Clamp(min=0.3) }` | Floor prevents timid foxes from starving. |
| `Raiding` | hunger deficit | linear | `Logistic(steepness=8, midpoint=0.75)` | Reuse hangry anchor. |
| `Raiding` | cunning | linear | `Linear` | Personality coefficient. |
| `Raiding` | store_visible && !store_guarded | compound bool gate | Context-tag filter (§4) | Eligibility gate. |
| `Resting` | hunger-as-comfort | `hunger * health_fraction` bilinear | `Linear` per axis (bilinear in composition §3) | Well-fed + healthy produces comfort. |
| `Resting` | health_fraction | linear | `Linear` | Already a bounded fraction. |
| `Resting` | day_phase | enum → additive | `Piecewise([(dawn, fox_rest_dawn_bonus), (day, fox_rest_day_bonus), (dusk, fox_rest_dusk_bonus), (night, fox_rest_night_bonus)])` | Reuse day_phase anchor; diurnal rest — knot values peaked at Day, inverse of Hunting. |
| `Resting` | hunger > 0.5, has_den | bool gates | Context-tag filter (§4) | Eligibility gates. |
| `Fleeing` | health deficit | `(1 - health_fraction)` linear | `Logistic(steepness=8, midpoint=0.5)` | Injury panic threshold — inflection at the current hardcoded `< 0.5` gate. |
| `Fleeing` | cats_nearby bonus | `+0.5 if cats >= 2` | `Piecewise([(0, 0), (1, 0), (2, 0.5), (N, 0.5)])` | Step function at 2+ cats. |
| `Fleeing` | boldness (damped invert) | `(1 - boldness * 0.5)` | `Composite { inner: Linear(slope=0.5), post: Invert }` | Timid foxes flee more. |
| `Patrolling` | territory_scent deficit | `(1 - scent)` linear | `Logistic(steepness=5, midpoint=0.5)` | Scent-marking urgency rises as marks fade; gentler than hangry (5 vs 8) — foxes don't panic about territory. |
| `Patrolling` | time-since-patrol, normalized | `(ticks / 2000).min(1.0)` | `Composite { inner: Linear(divisor=2000), post: Clamp(max=1.0) }` | Reuse saturating-count anchor, time-variant. |
| `Patrolling` | day_phase | enum → additive | `Piecewise([(dawn, fox_patrol_dawn_bonus), (day, fox_patrol_day_bonus), (dusk, fox_patrol_dusk_bonus), (night, fox_patrol_night_bonus)])` | Reuse day_phase anchor; Patrol knots distinct from Hunt. |
| `Patrolling` | territoriality | linear | `Linear` | Personality coefficient. |
| `Patrolling` | has_den | bool gate | Context-tag filter (§4) | Eligibility gate. |
| `Avoiding` | cats_nearby urgency | `cats_nearby as f32` | `Composite { inner: Linear, post: Clamp(max=cap) }` | Reuse saturating-count anchor. |
| `Avoiding` | boldness (damped invert, stronger) | `(1 - boldness * 0.8)` | `Composite { inner: Linear(slope=0.8), post: Invert }` | Damped more heavily than `Fleeing` — Avoiding tolerates more boldness before disengaging. |
| `Avoiding` | hunger > 0.3, health > 0.5, cats ≥ 1 | bool gates | Context-tag filter (§4) | Eligibility gates. |
| `Feeding` | cub_satiation deficit | `(1 - cub_satiation)` linear | `Logistic(steepness=7, midpoint=0.6)` | Cub-hunger threshold — gentler than adult hangry (7 vs 8) because adults buffer the gap. |
| `Feeding` | protectiveness | linear | `Linear` | Personality coefficient. |
| `Feeding` | has_cubs && cubs_hungry | bool gates | Context-tag filter (§4) | Eligibility gates. |
| `DenDefense` | cub_safety deficit | `(1 - cub_safety)` linear | `Logistic(steepness=10, midpoint=0.5)` | Flee-or-fight analog — reuse `Flee.safety` anchor (steepest logistic in catalog). |
| `DenDefense` | protectiveness | linear | `Linear` | Personality coefficient. |
| `DenDefense` | cat_threatening_den && has_cubs | bool gates | Context-tag filter (§4) | Eligibility gates. |
| `Dispersing` | juvenile lifecycle override | hardcoded `2.0` | `Linear(intercept=2.0)` — lifecycle-gated, not a scored axis | Dispersal is a lifecycle-stage instinct. Including for completeness so the catalog has no gaps. |

#### Pattern summary

**Every non-`Linear` curve replaces either an explicit threshold check,
a hand-piecewise constant table, a bilinear interaction, or a `.min()`
/ `.max()` clamp.** Making these declarative is what unblocks balance
tuning without re-reading `scoring.rs`.

Three curve shapes dominate the upgrade:

- **`Logistic`** for physiological thresholds — hunger, sleep-dep,
  warmth, safety, cub-safety, territory-scent, reproductive need.
  Steepness calibrates how cliff-like the threshold is (steep=10 for
  panic; steep=5 for slow-building social / territorial pressure).
- **`Quadratic(exp=2)`** for food-scarcity — marginal utility of the
  next food unit rises sharply as stores empty.
- **`Piecewise`** for diurnal phase, piecewise health/safety gating,
  and step-function presence bonuses — anywhere today's code does
  `match phase { ... }` or `if flag { bonus } else { 0 }`.

`Linear` remains the default for personality and skill scalars: they're
already bounded preference/mastery coefficients, and a curve on top
would obscure tuning.

**Cross-axis anchor check** — derived rows cite their anchor, so drift
gets caught at review:

- **Hangry anchor** (`Eat.hunger = Logistic(8, 0.75)`): reused by
  `Hunt.hunger`, `Forage.hunger`, `Eat (incap.)`, fox `Hunting.hunger`,
  fox `Raiding.hunger`.
- **Sleep-dep anchor** (`Sleep.energy = Logistic(10, 0.7)`): reused by
  `Sleep (incap.)`.
- **Loneliness anchor** (`Socialize.social = Logistic(5, 0.6)`): reused
  by `Groom (other).social`.
- **Inverted-need-penalty anchor**
  (`Composite { Logistic(5, 0.3), Invert }` on `phys_satisfaction`):
  reused by `Groom (other)` temper penalty.
- **Piecewise-threshold anchor**
  (`Fight.health = Piecewise([(0, 0), (0.3, 0.2), (0.5, 1.0), (1.0, 1.0)])`):
  reused by `Fight.safety`.
- **Flee-or-fight anchor** (`Flee.safety = Logistic(10, threshold)`):
  reused by fox `DenDefense.cub_safety`.
- **Diurnal-phase anchor** (`Sleep.day_phase` 4-knot piecewise): shape
  reused by fox `Hunting.day_phase`, fox `Resting.day_phase`, fox
  `Patrolling.day_phase` with species-specific knot values.
- **Scarcity anchor** (`Hunt.food_scarcity = Quadratic(exp=2)`): reused
  by `Forage.food_scarcity`, `Farm.food_scarcity`, `Cook.food_scarcity`.
- **Saturating-count anchor**
  (`Composite { Linear, Clamp(max) }`): reused by `Fight.ally_count`,
  `Coordinate.pending_directive_count`, `PracticeMagic.harvest.carcass_count`,
  `Patrolling.time_since_patrol`, `Avoiding.cats_nearby`.

#### Constants retired by the curve refactor

The following `SimConstants` fields exist today as workarounds for the
limitations of linear scoring math. Each is made obsolete by a proper
curve primitive in the rows above — not a behavior change, a shape
change. Delete when the §2.3 curves land.

| Retired constant(s) | Current role | Replaced by |
|---|---|---|
| `incapacitated_eat_urgency_scale`, `incapacitated_eat_urgency_offset` | Boost Eat urgency for incapacitated cats so it dominates | `Logistic(8, 0.75)` on `Eat.hunger` spikes hard on its own; the `Incapacitated` ECS marker (§4) filters ineligible DSEs |
| `incapacitated_sleep_urgency_scale`, `incapacitated_sleep_urgency_offset` | Boost Sleep urgency for incapacitated cats | Same — `Logistic(10, 0.7)` on `Sleep.energy` + `Incapacitated` filter |
| `incapacitated_idle_score` | Fallback sentinel for incapacitated branch | Idle's canonical axes already produce a fallback; branch disappears |
| `ward_corruption_emergency_bonus` | Flat bonus added to ward score when corruption appears | `Logistic(steepness=8, midpoint=0.1)` on `territory_max_corruption` at `Herbcraft.ward` / `PracticeMagic.durable_ward` |
| `cleanse_corruption_emergency_bonus` | Flat bonus added to cleanse scores when corruption appears | Same — absorbed into Logistic on the corruption axis for `PracticeMagic.cleanse` and `.colony_cleanse` |
| `corruption_sensed_response_bonus` | Linear scale on `nearby_corruption_level` with a `> 0.1` gate | Single `Logistic(steepness=8, midpoint=0.1)` collapses the gate + scale into one primitive |

Unifying shape: **each retired constant was a flat additive bonus gated
by a compound threshold, used to overcome the fact that the underlying
axis was being scored linearly.** Replacing linear with Logistic makes
the axis climb steeply past its threshold on its own, eliminating the
need for an emergency-bonus layer. This is exactly ch 13
§"Compartmentalized Confidence" applied to retire workaround layers:
when each axis is shaped correctly, bolt-on compensators become noise.

Not retired (legitimate constants that keep their role):
- `fight_health_suppression_threshold`, `fight_safety_suppression_threshold` — these move into the `Piecewise` knot positions for `Fight.health` / `Fight.safety`, not workarounds.
- `herbcraft_ward_siege_bonus` — a genuine compound-condition surge (wards actively under attack is a distinct event), not paper over a shape problem.
- `injury_rest_bonus` on `Sleep` — legitimate second axis (injury separate from energy), keeps its Linear slope.
- `flee_safety_threshold`, `patrol_safety_threshold`, `cook_hunger_gate`, etc. — migrate into Logistic midpoints or eligibility gates; the constants survive in a different role.
- `idle_minimum_floor` — Idle's floor stays as the `Clamp(min)` parameter.
- `boldness.max(0.3)` floor in fox Hunting — design choice (even timid foxes must hunt when starving), not a workaround.
- Fox `Dispersing` hardcoded `2.0` — lifecycle override, not a curve-replaceable shape problem.

### §2.4 Cross-refs

- Ch 12 §"Constructing Response Curves" — why LUT-shaped thinking
  matters even when we function-evaluate.
- Ch 12 §"Converting Functions to Response Curves" — the
  formula→LUT pipeline, applicable if we opt a specific curve into
  `LutBacked`.
- Ch 12 §"Hand-Crafted Response Curves" — Piecewise is the Rust
  encoding of this pattern.
- Ch 12 §"Dynamic Response Curves" + §"Adjusting Data" — runtime
  tuning; relevant for designer tools, not hot-path.
- Ch 14 §"Identifying Factors" + §"Choose Your Weapon" — the
  worked Quadratic example (weapon damage/accuracy vs. distance) is
  the most direct model for spatial `Quadratic` falloff in Clowder.

**Cross-ref:** `docs/reference/behavioral-math-ch12-response-curves.md`

---

## §3 Multi-consideration composition

A DSE with N considerations must reduce them to one `[0, 1]` score.
Ch 13 gives three legitimate composition shapes; Clowder needs all
three, plus a Maslow pre-gate wrapper and a post-scoring modifier
layer. Forcing one composition across all DSEs would regress a
meaningful fraction of today's 21 actions.

### §3.1 Three composition modes

```rust
pub enum Composition {
    CompensatedProduct,
    WeightedSum { weights: Vec<f32> },
    Max,
}
```

| Mode | Formula | When to use | Count across 30 DSEs (21 cat + 9 fox) |
|---|---|---|---|
| `CompensatedProduct` | `score = Π c; compensation = score^(1/n)` (see §3.2) | Every axis is a *true gate* — a zero on any one means the action is definitionally wrong. `Flee` (no threat ⇒ nothing to flee from; fully bold ⇒ standing not fleeing); `Mate` (no drive ⇒ no action; no warmth ⇒ non-consensual). See §3.1.1 for the full roster. | **11** (§3.1.1) |
| `WeightedSum` | `score = (Σ wᵢ · cᵢ) / Σ wᵢ` | Axes are *trade-off drivers* — a single strong axis can motivate the action even if others are zero. `Sleep`'s night-phase drives rest for well-rested cats; `Forage`'s hunger drives starving lazy cats; `Hunt`'s prey-proximity drives bold cats on a full stomach. See §3.1.1 for the full roster. | **16** (§3.1.1) |
| `Max` | `score = max(sub_scores)` | Sub-mode competition under a shared eligibility filter. **All three instances retire under §L2.10's sibling-DSE split** — `Max` is not a live mode in the end-state. | **3, all retiring** (Groom, Herbcraft, PracticeMagic) |

This answers the prior stub's open question #3 ("are there DSEs where
additive composition is correct?") — **yes, 16 of 30**. Don't force
multiplicative on everything. Ch 13 §"Weighted Sums" treats additive
composition as first-class, not a fallback.

### §3.1.1 Per-DSE composition mode assignment

This table commits a composition mode for every DSE in the current
scoring surface — 21 cat DSEs in `src/ai/scoring.rs` and 9 fox
dispositions in `src/ai/fox_scoring.rs`. Rows grouped by Maslow tier
to match §2.3's layout.

**Classification is by *design intent*, not by today's arithmetic.**
This is a refactor spec: today's `scoring.rs` is input, not authority.
Where design intent disagrees with the current math, the row's Note
cell names the implied restructure. Three tests pick the mode:

1. *Would a zero on any single axis make this action semantically
   wrong?* Yes on every axis ⇒ `CompensatedProduct`. No on at least
   one ⇒ `WeightedSum`.
2. *Is there a meaningful "base rate" where one axis alone can drive
   the action?* Yes ⇒ `WeightedSum` (Sleep's night-phase, Wander's
   base rate, Forage's hunger).
3. *Are the axes cooperating (each supplies necessary information) or
   competing (any one is enough)?* Cooperating ⇒ CP. Competing ⇒ WS.

Personality scalars with a floor (`boldness.max(0.3)`) or damped
inverts (`1 - boldness * 0.8`) are not gates — they're bounded
modulators. Treat as WS-compatible axes, not as CP gates.

#### Cat DSEs — Tier 1 (physiological / survival)

| DSE | Today's shape (file:lines) | L2 mode | Axes | Note (why this mode) |
|---|---|---|---|---|
| `Eat` | `(1-hunger) × scale × sup` (`scoring.rs:203–208`) | `CompensatedProduct` | hunger | n=1 today; kept CP (not WS) so future axes (`food_available`, `digestion_gate`) compose with gating semantics — a cat with no food available should not score Eat. |
| `Sleep` | `(1-energy) × scale × sup + day_phase + injury_bonus` (`scoring.rs:210–233`) | `WeightedSum` | energy_deficit, day_phase, injury_rest | Night-phase alone drives well-rested cats to sleep; injury alone drives rest at moderate energy. Design-intent comment at `scoring.rs:212–214`: *"Additive (not multiplicative) so Sleep remains available as a pressure-release valve at low energy even during feeding peaks."* |
| `Hunt` | `((1-hunger) + scarcity) × boldness × scale × sup + prey_bonus` (`scoring.rs:235–249`) | `WeightedSum` | hunger, food_scarcity, boldness, prey_proximity | Bold cat spotting prey ⇒ Hunt even on full stomach; starving timid cat ⇒ Hunt out of need. No single axis is a gate. Prey axis becomes a `SpatialConsideration` under §6. |
| `Forage` | `((1-hunger) + scarcity) × diligence × scale × sup` (`scoring.rs:251–259`) | `WeightedSum` | hunger, food_scarcity, diligence | A starving lazy cat should still forage (desperation); a diligent cat should still forage when colony stores are low. Design intent disagrees with a strict CP read of today's math — implementation PR restructures to flat weighted sum. |
| `Groom` | `max(self_groom, other_groom)` (`scoring.rs:283–300`) | `Max` **retiring (§L2.10)** | — | Splits into sibling DSEs `Groom(self)` + `Groom(other)`; each sibling becomes CP. Sibling-DSE composition specs are the separate TODO at Enumeration Debt line 71–74. |
| `Flee` | `(1-safety) × (1-boldness) × scale × sup` (`scoring.rs:320–327`) | `CompensatedProduct` | safety_deficit, boldness_inverse | Both axes gate: a fully-brave cat never flees (bold cats stand/fight); full safety has nothing to flee from. |

#### Cat DSEs — Tier 2 (safety / territory)

| DSE | Today's shape (file:lines) | L2 mode | Axes | Note (why this mode) |
|---|---|---|---|---|
| `Fight` | `boldness × combat × sup × health_piece × safety_piece + group_bonus` (`scoring.rs:329–353`) | `WeightedSum` | boldness, combat_eff, health, safety, ally_count | Group bonus expresses *herd courage* — a cat surrounded by allies engages even at low boldness. A pure product would suppress this; WS preserves the social-dynamics signal. |
| `Patrol` | `boldness × scale × (1-safety) × sup` (`scoring.rs:355–362`) | `CompensatedProduct` | boldness, safety_deficit | Timid cats flee (not patrol); full-safety has nothing to patrol. Both gate. |
| `Build` | `diligence × scale × sup + site_bonus + repair_bonus` (`scoring.rs:364–388`) | `WeightedSum` | diligence, site_presence, repair_presence | Site presence drives even low-diligence cats (*"there's literally a half-built wall here"*); repair need drives build independently. |
| `Farm` | `(1-food_frac) × diligence × scale × sup` (`scoring.rs:390–401`) | `CompensatedProduct` | scarcity, diligence | Comment at `scoring.rs:391–394` names Farm as scarcity-response — both gate; no design intent for a "base-rate" maintenance-farm today. |
| `Socialize` | `(1-social) × sociability × scale × sup − temper × (1-phys_sat) + playfulness_bonus + corruption_bonus` (`scoring.rs:261–281`) | `WeightedSum` | social_deficit, sociability, temper, phys_satisfaction, playfulness, corruption | Loneliness, playfulness, and corruption-push-back each drive independently. Bilinear `temper × (1-phys_sat)` subtracts (per §2.3 line 673, bilinear lives in composition) but doesn't gate. |

#### Cat DSEs — Tier 2–5 (craft / leadership / reproduction / care / idle)

| DSE | Today's shape (file:lines) | L2 mode | Axes | Note (why this mode) |
|---|---|---|---|---|
| `Explore` | `curiosity × scale × sup × unexplored` (`scoring.rs:302–309`) | `CompensatedProduct` | curiosity, unexplored_nearby | Both gate: no curiosity ⇒ no drive to explore; nothing unexplored ⇒ nothing to explore. |
| `Wander` | `curiosity × scale × sup + base + playfulness_bonus` (`scoring.rs:311–318`) | `WeightedSum` | curiosity, base_rate, playfulness | Base rate keeps Wander available for zero-curiosity cats; playfulness adds independently. Named in §3.1's summary as the canonical WS example. |
| `Cook` | `(cook_base + scarcity) × sup` (`scoring.rs:618–639`) | `WeightedSum` | base_rate, scarcity, diligence | Base rate and scarcity urgency trade off — cooking is ongoing activity plus scarcity response, not strictly gated on either. |
| `Herbcraft` | `max(gather, prepare, ward)` (`scoring.rs:403–479`) | `Max` **retiring (§L2.10)** | — | 3 sub-modes → sibling DSEs (`gather` / `prepare` / `ward`). Per-sibling composition specs are the separate TODO at Enumeration Debt line 71–74. |
| `PracticeMagic` | `max(scry, durable_ward, cleanse, colony_cleanse, harvest, commune)` (`scoring.rs:481–583`) | `Max` **retiring (§L2.10)** | — | 6 sub-modes → sibling DSEs. Per-sibling composition specs are the separate TODO at Enumeration Debt line 71–74. |
| `Coordinate` | `diligence × directive_count × scale + ambition × bonus × sup` (`scoring.rs:585–595`) | `WeightedSum` | diligence, directive_count, ambition | Ambition bonus drives coordinator work even at low directive count; directive urgency drives low-diligence coordinators. |
| `Mentor` | `warmth × diligence × scale × sup + ambition × bonus` (`scoring.rs:597–605`) | `WeightedSum` | warmth, diligence, ambition | Design-intent call: ambitious-but-cold cats *do* mentor (for status/respect, not affection) — a real cat social dynamic. WS preserves this; CP would silence it. |
| `Mate` | `(1-mating) × warmth × scale × sup` (`scoring.rs:607–616`) | `CompensatedProduct` | mating_deficit, warmth | Both gate: no drive ⇒ no action; no warmth toward partner ⇒ the action would not be a valid Mate. |
| `Caretake` | `(urgency × compassion × scale × sup) + parent_bonus` (`scoring.rs:641–654`) | `WeightedSum` | kitten_urgency, compassion, is_parent | Parent bonus drives even low-compassion parents (bloodline override); compassion drives non-parents facing hungry kittens. |
| `Idle` | `(base + (1-curiosity) × scale − playfulness × scale).max(floor)` (`scoring.rs:656–662`) | `WeightedSum` | base_rate, incuriosity, playfulness | Base rate + incuriosity additive; floor is a post-composition `Clamp(min)` (§2.3 saturating-count pattern). |

**Incapacitated branch** (`scoring.rs:181–201`) is retired under §2.3
and the §4 eligibility-filter pattern; no table row. See Enumeration
Debt line 75–83.

#### Fox dispositions — Level 1 (survival)

| Disposition | Today's shape (file:lines) | L2 mode | Axes | Note (why this mode) |
|---|---|---|---|---|
| `Hunting` | `(hunger + prey + belief + phase) × boldness.max(0.3) × sup` (`fox_scoring.rs:131–150`) | `WeightedSum` | hunger, prey_proximity, prey_belief, day_phase, boldness | Four additive urgency drivers; `boldness.max(0.3)` makes boldness a modulator, not a gate — *starvation overrides timidity*. |
| `Raiding` | `hunger × cunning × scale × sup` (`fox_scoring.rs:152–159`) | `CompensatedProduct` | hunger, cunning | Both gate: raiding requires cleverness; no hunger ⇒ no reason to risk colony contact. |
| `Resting` | `((hunger × health_frac) × 0.6 + phase_bonus) × sup` (`fox_scoring.rs:161–177`) | `WeightedSum` | hunger, health_fraction, day_phase | Day-phase drives rest even when comfort (bilinear hunger × health) is low — diurnal foxes rest by day regardless of comfort state. |
| `Fleeing` | `((1-health_frac) + cats_nearby_bonus) × (1-boldness×0.8) × sup` (`fox_scoring.rs:179–186`) | `WeightedSum` | health_deficit, cats_nearby, boldness | Health-deficit and cat-proximity are additive trade-off drivers; damped boldness inverse is a modulator, not a gate. |

#### Fox dispositions — Level 2 (territory)

| Disposition | Today's shape (file:lines) | L2 mode | Axes | Note (why this mode) |
|---|---|---|---|---|
| `Patrolling` | `(scent + time_since + phase) × territoriality × sup` (`fox_scoring.rs:193–210`) | `WeightedSum` | scent_deficit, time_since_patrol, day_phase, territoriality | Three additive urgency drivers; design intent is that a *mostly*-territorial fox with faded scent still patrols. Flattens today's nested mult-over-add — implementation PR restructures. |
| `Avoiding` | `cats_nearby × (1-boldness×0.8) × sup` (`fox_scoring.rs:212–222`) | `CompensatedProduct` | cats_nearby, boldness_inverse | Both gate (damped-boldness as the gate): no cats ⇒ nothing to avoid; max boldness ⇒ never avoids. |

#### Fox dispositions — Level 3 (offspring)

| Disposition | Today's shape (file:lines) | L2 mode | Axes | Note (why this mode) |
|---|---|---|---|---|
| `Feeding` | `(1-cub_sat) × protect × scale × sup` (`fox_scoring.rs:229–236`) | `CompensatedProduct` | cub_satiation_deficit, protectiveness | Both gate: fed cubs ⇒ no action; no protectiveness ⇒ vixen doesn't provision. |
| `DenDefense` | `(1-cub_safety) × protect × scale × sup` (`fox_scoring.rs:238–245`) | `CompensatedProduct` | cub_safety_deficit, protectiveness | Both gate. Reuses `Flee.safety` steepness=10 anchor from §2.3 — same flee-or-fight threshold shape. |
| `Dispersing` | hardcoded `2.0 + jitter` (`fox_scoring.rs:106–123`) | `CompensatedProduct` | lifecycle_intercept | n=1 lifecycle override (`Linear(intercept=2.0)`); juvenile-dispersal lifecycle marker is the eligibility filter (§4). |

#### Classification totals

- **`CompensatedProduct`: 11** — cat: Eat, Flee, Patrol, Farm, Explore,
  Mate (6); fox: Raiding, Avoiding, Feeding, DenDefense, Dispersing (5).
- **`WeightedSum`: 16** — cat: Sleep, Hunt, Forage, Fight, Build,
  Socialize, Wander, Cook, Coordinate, Mentor, Caretake, Idle (12);
  fox: Hunting, Resting, Fleeing, Patrolling (4).
- **`Max` (retiring)**: 3 — cat: Groom, Herbcraft, PracticeMagic. All
  three dissolve into sibling DSEs under §L2.10.4.

#### Implementation-PR implications

Three classifications disagree with today's arithmetic. Each is a
design-intent commitment; the implementation PR restructures the math
to match:

- **`Forage`** — today's `((1-hunger) + scarcity) × diligence` nests
  additive inside multiplicative. A strict CP read zeroes when
  `diligence=0` (starving lazy cat stops foraging), which is wrong.
  Design intent: WS. Implementation flattens to three weighted axes
  (`hunger`, `food_scarcity`, `diligence`).
- **`Caretake`** — today's `(urgency × compassion × scale × sup) +
  parent_bonus` is outer-additive already; WS re-expression makes the
  "parent bonus drives regardless of compassion" semantic explicit.
- **Fox `Patrolling`** — today's `(scent + time + phase) ×
  territoriality` nests additive inside multiplicative. A strict CP
  read silences non-territorial foxes even when scent marks have
  faded, which contradicts design intent. WS flattens to four
  weighted axes.

Per CLAUDE.md's balance methodology, each restructure is a behavior-
change candidate: the implementation PR must land a hypothesis +
A/B result for any characteristic-metric drift > ±10%. Expected
drift is small (the current math already approximates the intended
behavior via arithmetic coincidence in the nominal case); canaries
gate acceptance.

### §3.2 The compensation factor

Mark's compensation factor compensates for the fact that a pure
product over N axes punishes actions with *more* considerations: if
each consideration averages 0.7, a 3-axis product is 0.34, a 6-axis
product is 0.12 — yet the 6-axis action isn't twice as bad a fit, it's
just more thoroughly measured. Ch 13 addresses this indirectly via
weighted means; `big-brain`'s `ProductOfScorers` implements one
canonical formula.

The form Clowder will use (expected, final tweak at implementation
time):

```
raw_product   = Π cᵢ               // pure multiplicative
compensated   = raw_product^(1/n)  // geometric mean
final_score   = lerp(raw_product, compensated, compensation_strength)
```

`compensation_strength` ∈ `[0, 1]` — 0 reproduces pure product, 1
gives geometric mean. Default 0.75 mirrors big-brain's observable
behavior. Any consideration at ≈ 0 still zeroes the score (soft gate
preserved).

The compensation factor is **elastic failure applied to composition**
(§0.2): a single low axis softens the score rather than zeroing it.
Pure-product composition would be brittle — one bad axis kills the
whole DSE even when the action is still the right call. Naming this
here so future tuning doesn't read the `0.75` as an ergonomic default
and push it back toward `0`.

### §3.3 Weight rationalization

Today's 57 `ScoringConstants` are absolute weights with arbitrary
magnitudes: `hunt_prey_bonus=0.2`, `sleep_night_bonus=1.2`,
`fight_ally_bonus_per_cat=0.15`. Ch 13 §"Absolute vs Relative Weights"
identifies three weight expressions; Clowder's L2 picks one *per DSE*
based on composition mode:

- **Relative to max** (ch 13 §"Weights Relative to a Maximum") — each
  weight expressed as a fraction of a declared max. Best for
  `CompensatedProduct` — all axes live in `[0, 1]` natively, and
  "how much does this axis contribute at its max?" is a meaningful
  tuning question.
- **Relative to each other** (ch 13 §"Weights Relative to Each Other")
  — weights sum to 1.0 within a `WeightedSum` DSE. Best where axes
  trade off: Sleep's phase-offset ratios, Hunt's `food_scarcity +
  hunger` split.
- **Absolute anchored** (ch 13 §"Absolute Weights") — pick a semantic
  anchor ("starvation-level urgency = 1.0") and express every DSE's
  max score against that. Best for cross-DSE comparability: Hunt and
  Forage should reach similar magnitudes when hunger is equal and
  terrain differs.

A DSE declares its weight-expression mode; implementation validates
at plugin-load (weights sum to 1.0 for `WeightedSum`, etc.).

#### §3.3.1 Per-DSE weight-expression mode assignment

The composition mode picked in §3.1.1 determines the weight-expression
mode mechanically:

- `CompensatedProduct` → **Relative-to-max** (RtM). Every axis is a gate
  valued in `[0, 1]`; each weight is a per-axis max-contribution
  coefficient. Implementation requires `weights.iter().all(|w| (0.0..=1.0).contains(w))`.
- `WeightedSum` → **Relative-to-each-other** (RtEO). Weights are trade-
  off shares that sum to 1.0. Implementation requires
  `(weights.iter().sum::<f32>() - 1.0).abs() < epsilon`.
- `Max` (retiring) → the sibling-DSE split resolves the weight-expression
  question downstream; each sibling re-emerges with its own RtM/RtEO
  declaration. See Enumeration-Debt line 71–74.

The table below commits this assignment for every DSE in §3.1.1.
Absolute-anchored constraints are a separate, cross-DSE concern —
enumerated in §3.3.2.

##### Cat DSEs — Tier 1 (physiological / survival)

| DSE | Composition (§3.1.1) | Weight mode | Axis count | Notes |
|---|---|---|---|---|
| `Eat` | CompensatedProduct | RtM | 1 today (→ 2–3 at L2) | Single-axis today; RtM is trivial but locks the contract when `food_available` + `digestion_gate` join. |
| `Sleep` | WeightedSum | RtEO | 3 (`energy_deficit`, `day_phase`, `injury_rest`) | Weights sum to 1.0. Phase-offset ratios (`night : day : dawn : dusk`) express the diurnal tradeoff. |
| `Hunt` | WeightedSum | RtEO | 4 (`hunger`, `food_scarcity`, `boldness`, `prey_proximity`) | Food-scarcity / hunger split is the canonical RtEO example (ch 13 §"Weights Relative to Each Other"). Absolute-anchor peer of Forage / Cook — see §3.3.2. |
| `Forage` | WeightedSum | RtEO | 3 (`hunger`, `food_scarcity`, `diligence`) | Peer of Hunt. |
| `Groom` | Max (retiring) | — | — | Sibling DSEs declare their own mode. |
| `Flee` | CompensatedProduct | RtM | 2 (`safety_deficit`, `boldness_inverse`) | Both axes gate; RtM. Absolute-anchor peer of DenDefense (fox) — shared flee-or-fight logistic shape. |

##### Cat DSEs — Tier 2 (safety / territory)

| DSE | Composition (§3.1.1) | Weight mode | Axis count | Notes |
|---|---|---|---|---|
| `Fight` | WeightedSum | RtEO | 5 (`boldness`, `combat_eff`, `health`, `safety`, `ally_count`) | Highest axis count in the catalog; RtEO enables the group-courage signal (a low-boldness cat can still be pulled in by ally count). Absolute-anchor peer of Hunt / Patrol. |
| `Patrol` | CompensatedProduct | RtM | 2 (`boldness`, `safety_deficit`) | Both gate. |
| `Build` | WeightedSum | RtEO | 3 (`diligence`, `site_presence`, `repair_presence`) | |
| `Farm` | CompensatedProduct | RtM | 2 (`scarcity`, `diligence`) | |
| `Socialize` | WeightedSum | RtEO | 6 (`social_deficit`, `sociability`, `temper`, `phys_satisfaction`, `playfulness`, `corruption`) | High-n RtEO; §3.2 compensation factor (geometric-mean lerp) is *not* involved (RtEO's normalization handles multi-axis bias). |

##### Cat DSEs — Tier 2–5 (craft / leadership / reproduction / care / idle)

| DSE | Composition (§3.1.1) | Weight mode | Axis count | Notes |
|---|---|---|---|---|
| `Explore` | CompensatedProduct | RtM | 2 (`curiosity`, `unexplored_nearby`) | |
| `Wander` | WeightedSum | RtEO | 3 (`curiosity`, `base_rate`, `playfulness`) | `base_rate` as an RtEO axis is the canonical "keep available at zero drive" pattern. |
| `Cook` | WeightedSum | RtEO | 3 (`base_rate`, `scarcity`, `diligence`) | Absolute-anchor peer of Hunt / Forage. |
| `Herbcraft` | Max (retiring) | — | — | Sibling DSEs (gather / prepare / ward) declare their own mode. |
| `PracticeMagic` | Max (retiring) | — | — | Sibling DSEs (scry / durable_ward / cleanse / colony_cleanse / harvest / commune) declare their own mode. |
| `Coordinate` | WeightedSum | RtEO | 3 (`diligence`, `directive_count`, `ambition`) | |
| `Mentor` | WeightedSum | RtEO | 3 (`warmth`, `diligence`, `ambition`) | RtEO preserves the "ambitious-but-cold cat mentors for status" signal — RtM would silence it (see §3.1.1 note). |
| `Mate` | CompensatedProduct | RtM | 2 (`mating_deficit`, `warmth`) | Both gate. |
| `Caretake` | WeightedSum | RtEO | 3 (`kitten_urgency`, `compassion`, `is_parent`) | `is_parent` is a 0/1 axis with a non-trivial RtEO weight — encodes the bloodline-override signal numerically. |
| `Idle` | WeightedSum | RtEO | 3 (`base_rate`, `incuriosity`, `playfulness`) | Floor is a post-composition `Clamp(min)` (§2.3), not an axis. |

##### Fox dispositions — Level 1 (survival)

| Disposition | Composition (§3.1.1) | Weight mode | Axis count | Notes |
|---|---|---|---|---|
| `Hunting` | WeightedSum | RtEO | 5 (`hunger`, `prey_proximity`, `prey_belief`, `day_phase`, `boldness`) | `boldness` is modulator (`max(0.3)` floor), not a gate — §3.1.1 note. Absolute-anchor peer of cat `Hunt` through the shared Prey-location map. |
| `Raiding` | CompensatedProduct | RtM | 2 (`hunger`, `cunning`) | Both gate. |
| `Resting` | WeightedSum | RtEO | 3 (`hunger`, `health_fraction`, `day_phase`) | Diurnal rest even when comfort is low — RtEO preserves the day-phase independent drive. |
| `Fleeing` | WeightedSum | RtEO | 3 (`health_deficit`, `cats_nearby`, `boldness`) | `boldness` damp (`(1 - boldness × 0.5)`) is a modulator; RtEO composes with the two additive urgency drivers. |

##### Fox dispositions — Level 2 (territory)

| Disposition | Composition (§3.1.1) | Weight mode | Axis count | Notes |
|---|---|---|---|---|
| `Patrolling` | WeightedSum | RtEO | 4 (`scent_deficit`, `time_since_patrol`, `day_phase`, `territoriality`) | |
| `Avoiding` | CompensatedProduct | RtM | 2 (`cats_nearby`, `boldness_inverse`) | Damped-boldness is the gate (`(1 - boldness × 0.8)`). |

##### Fox dispositions — Level 3 (offspring)

| Disposition | Composition (§3.1.1) | Weight mode | Axis count | Notes |
|---|---|---|---|---|
| `Feeding` | CompensatedProduct | RtM | 2 (`cub_satiation_deficit`, `protectiveness`) | |
| `DenDefense` | CompensatedProduct | RtM | 2 (`cub_safety_deficit`, `protectiveness`) | Absolute-anchor peer of cat `Flee` — shared flee-or-fight shape. |
| `Dispersing` | CompensatedProduct | RtM | 1 (`lifecycle_intercept`) | Lifecycle override; RtM is degenerate but locks the contract. |

##### Totals

- **Relative-to-max (RtM):** 11 DSEs (all CP). Cat: Eat, Flee, Patrol,
  Farm, Explore, Mate. Fox: Raiding, Avoiding, Feeding, DenDefense,
  Dispersing.
- **Relative-to-each-other (RtEO):** 16 DSEs (all WS). Cat: Sleep, Hunt,
  Forage, Fight, Build, Socialize, Wander, Cook, Coordinate, Mentor,
  Caretake, Idle. Fox: Hunting, Resting, Fleeing, Patrolling.
- **Deferred (sibling-DSE):** 3 `Max`-retiring DSEs (Groom, Herbcraft,
  PracticeMagic). Enumeration-Debt line 71–74.

#### §3.3.2 Absolute-anchor peer groups

Absolute anchoring is **not a per-DSE declaration**; it is a per-peer-
group constraint that binds each peer group's max-output magnitude to a
common anchor. Without it, a starving cat's Hunt score and the same
cat's Forage score could be arbitrarily different even at equal hunger
— and the planner's top-choice would be a formatting accident, not a
considered decision.

Each peer group declares a single semantic anchor. Every DSE in the
group is tuned so its peak output (at full activation of every axis)
maps onto that anchor. Within a peer group, cross-DSE switching is
meaningful (a full-stomach bold cat sees Hunt and Patrol at *peer*
intensity); across groups, comparison is undefined by design (Mate and
Fight are not peers).

| Anchor | Peer group | Basis |
|---|---|---|
| **Starvation urgency** = 1.0 | Eat, Hunt, Forage, Cook, (fox) Hunting, Raiding | All four (+2 fox) channel the same physiological drive. A peer-locked anchor means the planner's food-acquisition choice is driven by axis context, not magnitude mismatch. |
| **Fatal threat** = 1.0 | Flee, Fight, Patrol, (fox) Fleeing, Avoiding, DenDefense | Flee, Fight, and Patrol are three responses to the same underlying danger signal; matching peaks lets safety context (boldness × ally_count × health) pick the response instead of a magnitude bias. |
| **Rest urgency** = 1.0 | Sleep, Idle, (fox) Resting | Two cat rest-family DSEs plus fox Resting; `Idle` is a low-floor fallback and caps below Sleep's peak. |
| **Social urgency** = 1.0 | Socialize, Groom(other), Mentor, Caretake, Mate | Social-family DSEs. Bond strength + relationship tags (future ToT layer) will split this group further. |
| **Territory urgency** = 1.0 | (fox) Patrolling, (cat) Patrol | Cross-species peer — marks the territorial drive as shared, despite different mechanisms. |
| **Work urgency** = 1.0 | Build, Farm, Coordinate | Colony-maintenance peers; diligence-driven. |
| **Exploration urgency** = 1.0 | Explore, Wander | Low-priority discovery family. `Wander` caps below `Explore` (Wander is a base-rate fallback when nothing unexplored is nearby). |
| **Lifecycle override** = 2.0 | (fox) Dispersing | Single-member group. The `2.0` intercept intentionally exceeds every other fox disposition's 1.0 peak so Dispersing cannot be outvoted when its eligibility filter fires. |

Anchors are *design commitments*, not empirical measurements — they
inform tuning-PR acceptance (is Hunt's peak within ±10% of Forage's at
equal hunger?), not a runtime assertion. Absolute-anchoring is tested
at plugin-load as a warning, not an error: a peer group whose DSEs
diverge by > 2× on the characteristic test case is a bug, but
lock-stepping them is a tuning expectation, not a compile-time
invariant.

**Cross-ref:** §3.1.1 (composition mode per DSE), §2.3 (curve shapes
per consideration), §L2.10.6 (softmax temperature is downstream of
anchor agreement — without comparable magnitudes, the softmax's
"fairness" argument collapses).

### §3.4 Maslow as a hierarchical pre-gate (keep)

Maslow is a separate layer on top of the DSE layer — not folded into
axis composition. This is ch 13 §"Layered Weighting Models" applied:

```
raw_score    = composition_mode.reduce(considerations)
gated_score  = maslow_suppression(dse.tier) * raw_score
```

`Needs::level_suppression` already implements this hierarchically:
Level 1 (physiological survival) always fires at full strength;
Level 5 (self-actualization) gates on all four lower tiers being
satisfied. **This is Clowder-specific, not from Mark** — BDI-style
Maslow wrapping isn't in *Behavioral Mathematics*, but it composes
cleanly with the IAUS layer. Don't refactor it.

### §3.5 Post-scoring modifiers as a distinct layer

Pride, independence, patience, tradition, fox-territory suppression,
and corruption-territory suppression each modify already-composed
scores. Ch 13 §"Layered Weighting Models / Propagation of Change"
treats these as filter stages. Clowder's L2 structures each as a
`ScoreModifier`:

```rust
pub trait ScoreModifier {
    fn apply(&self, dse_id: DseId, score: f32, ctx: &Ctx) -> f32;
    fn name(&self) -> &'static str;
}
```

Applied in order after base composition. Each modifier owns its
triggering condition (fox-scent threshold, active disposition, etc.)
and its transform (additive bonus, multiplicative damp, etc.).

**Why distinct:** this separation is exactly ch 13 §"Compartmentalized
Confidence" — changes to fox-suppression's threshold don't require
re-tuning base Hunt scores, because the layers compose independently.
Today's scoring.rs mixes these into the per-DSE blocks (5 modifiers
inlined per action); L2 factors them out.

#### §3.5.1 Modifier catalog

All six modifiers live in `compose_action_scores()` at
`src/ai/scoring.rs:666–750` — already post-composition, but inlined into
one function rather than registered as `ScoreModifier` instances. The
catalog below names each modifier's trigger, transform, DSE applicability,
and citation. Status reflects today's code: every row is **Built** as
imperative code; the refactor promotes them to `ScoreModifier` trait
objects registered in plugin order.

| Modifier | Trigger condition | Transform shape | Applies to DSEs | Status | Source |
|---|---|---|---|---|---|
| **Pride bonus** | `ctx.respect < s.pride_respect_threshold` (default 0.5) | `score += personality.pride × s.pride_bonus` — additive, personality-scaled. Default bonus 0.1 → `[0.0, 0.1]` range added. | Hunt, Fight, Patrol, Build, Coordinate | Built | `scoring.rs:666–677` |
| **Independence (solo boost)** | Always active (no threshold gate). | `score += personality.independence × s.independence_solo_bonus` — additive, personality-scaled. Default bonus 0.1. | Explore, Wander, Hunt | Built | `scoring.rs:679–686` |
| **Independence (group penalty)** | Always active. | `score = (score − personality.independence × s.independence_group_penalty).max(0.0)` — subtractive, clamped to ≥ 0. Default penalty 0.1. | Socialize, Coordinate, Mentor | Built | `scoring.rs:687–693` |
| **Patience commitment bonus** | `ctx.active_disposition.is_some()`. | `score += personality.patience × s.patience_commitment_bonus` — additive, personality-scaled, applied to the *constituent actions* of the active disposition (via `DispositionKind::constituent_actions()`). Default bonus 0.15. | Dynamic — any DSE that appears in the active disposition's constituent list (see §3.5.3 for the disposition → actions map) | Built | `scoring.rs:695–704` |
| **Tradition location bonus** | `ctx.tradition_location_bonus > 0.0` (caller pre-computes as `personality.tradition × 0.1` when the cat's action at this tile was previously successful; otherwise 0). | `score += ctx.tradition_location_bonus` — additive flat value. | **All DSEs (unfiltered loop — see §3.5.3 discovery)** | Built, **buggy** — filter missing | `scoring.rs:706–714` |
| **Fox-territory suppression** | `ctx.fox_scent_level > s.fox_scent_suppression_threshold` (default 0.3). | `score *= (1.0 − suppression).max(0.0)` where `suppression = ((fox_scent − threshold) / (1 − threshold)) × s.fox_scent_suppression_scale` (scale 0.8). Multiplicative damp. | Hunt, Explore, Forage, Patrol, Wander — **plus** `Flee += suppression × 0.5` (additive boost) | Built | `scoring.rs:716–737` |
| **Corruption-territory suppression** | `ctx.tile_corruption > s.corruption_suppression_threshold` (default 0.3). | Same shape as Fox-suppression (multiplicative damp); constants `corruption_suppression_threshold` + `corruption_suppression_scale` (0.6). No Flee-boost secondary effect. | Explore, Wander, Idle | Built | `scoring.rs:739–750` |

**Constants referenced:** `pride_respect_threshold`, `pride_bonus`,
`independence_solo_bonus`, `independence_group_penalty`,
`patience_commitment_bonus`, `fox_scent_suppression_threshold`,
`fox_scent_suppression_scale`, `corruption_suppression_threshold`,
`corruption_suppression_scale` — all live in
`src/resources/sim_constants.rs::ScoringConstants`. **Tradition has no
scale constant** in `ScoringConstants` — the `× 0.1` lives in the
caller. Refactor candidate: promote to `tradition_location_scale`.

#### §3.5.2 Per-DSE applicability matrix

Each cell shows whether a modifier touches the DSE. `✓` = applies;
`(+)` = additive only; `(×)` = multiplicative only; `(±)` = can go
either direction (Independence); `*` = dynamic via Patience's
disposition gate; `—` = no interaction. Patience's column names the
**dispositions** under which the modifier activates for that DSE (a
DSE can be reached by multiple dispositions; listed disjunctively).

| DSE | Pride | Ind. (solo +) | Ind. (group −) | Patience | Tradition | Fox-sup. | Corr.-sup. |
|---|---|---|---|---|---|---|---|
| `Eat` | — | — | — | \* Resting | ✓ | — | — |
| `Sleep` | — | — | — | \* Resting | ✓ | — | — |
| `Hunt` | ✓ (+) | ✓ (+) | — | \* Hunting | ✓ | ✓ (×) | — |
| `Forage` | — | — | — | \* Foraging | ✓ | ✓ (×) | — |
| `Groom` | — | — | — | \* Resting ∨ Socializing | ✓ | — | — |
| `Flee` | — | — | — | — | ✓ | ✓ (**+ boost**) | — |
| `Fight` | ✓ (+) | — | — | \* Guarding | ✓ | — | — |
| `Patrol` | ✓ (+) | — | — | \* Guarding | ✓ | ✓ (×) | — |
| `Build` | ✓ (+) | — | — | \* Building | ✓ | — | — |
| `Farm` | — | — | — | \* Farming | ✓ | — | — |
| `Socialize` | — | — | ✓ (−) | \* Socializing | ✓ | — | — |
| `Explore` | — | ✓ (+) | — | \* Exploring | ✓ | ✓ (×) | ✓ (×) |
| `Wander` | — | ✓ (+) | — | \* Exploring | ✓ | ✓ (×) | ✓ (×) |
| `Cook` | — | — | — | \* Crafting | ✓ | — | — |
| `Herbcraft` | — | — | — | \* Crafting | ✓ | — | — |
| `PracticeMagic` | — | — | — | \* Crafting | ✓ | — | — |
| `Coordinate` | ✓ (+) | — | ✓ (−) | \* Coordinating | ✓ | — | — |
| `Mentor` | — | — | ✓ (−) | \* Socializing | ✓ | — | — |
| `Mate` | — | — | — | \* Mating | ✓ | — | — |
| `Caretake` | — | — | — | \* Caretaking | ✓ | — | — |
| `Idle` | — | — | — | — | ✓ | — | ✓ (×) |

**Fox dispositions** (`fox_scoring.rs`) — none of the six cat modifiers
apply. Fox scoring has its own personality axes (`boldness`, `cunning`,
`territoriality`, `protectiveness`) expressed as per-consideration
scalars (see §2.3 Fox table); no post-scoring modifier layer is
enumerated for foxes in Phase 1. If L2 introduces fox-side modifiers
(e.g., juvenile-territorial-hesitation), append a separate §3.5.2.1
matrix.

**Matrix observations:**

- **Every cat DSE except `Flee` and `Idle` is touched by Patience** when
  the matching disposition is active — Patience is the widest-reaching
  modifier after Tradition.
- **Tradition touches every DSE** (see §3.5.3 for why this is a bug).
- **Pride and Independence are narrow** — 5 and 6 DSEs respectively. They
  are the modifiers where "modifier applies to DSE X" is a design
  commitment, not a consequence of other infrastructure.
- **Fox-suppression and Corruption-suppression overlap on
  Explore/Wander only** — Hunt is suppressed by fox-scent but not by
  corruption; Idle is suppressed by corruption but not by fox-scent.
  The asymmetry is intentional (fox-scent means prey fled + threat
  near; corruption means metaphysical malaise), but worth re-verifying
  during implementation — both thresholds default to 0.3 and both
  trigger often on the same seed-42 runs.

#### §3.5.3 Discoveries and open issues from enumeration

The per-modifier inventory surfaced three discrepancies that belong on
the implementation-PR docket but are not part of §3.5's closure.
Captured here so they route to `docs/open-work.md` rather than being
re-discovered later.

1. **Tradition is applied to every DSE unconditionally**
   (`scoring.rs:706–714`). The field `tradition_location_bonus` is
   pre-computed by the caller only when the cat's *current* action
   matches a previously successful action at this tile — but the loop
   that consumes it iterates over `scores.iter_mut()` without
   filtering by action. The effect today is muted because the caller
   sets the value to 0.0 in production (`goap.rs:900`), so the loop
   is a no-op. Two fixes available:
   - **(a) Structural fix:** caller pre-computes a `HashMap<Action, f32>`
     keyed by action; the modifier loops over the map and adds only to
     matched DSEs. Requires caller rework.
   - **(b) Semantic fix:** declare that Tradition *is* a flat bonus
     applied to every DSE at the tile where the cat has any history,
     not action-specific. Weaker signal, cheap implementation.
   Picking (a) preserves the spec's "previously successful action at
   this tile" framing; picking (b) rewrites §2.3's Tradition row. Flag
   to resolve during the L2 modifier-refactor PR.
2. **Fox-suppression boosts `Flee`** (`scoring.rs:732–735`) as an
   additive side effect: `flee_score += suppression × 0.5`. This
   mechanism is semantically sensible (bad territory → leave) but was
   invisible in §2.3's original matrix row. §3.5.1 now names it
   explicitly. No fix needed; just enumeration.
3. **`has_active_disposition` field is dead** (`ScoringContext.102`,
   hardcoded `false` in `goap.rs:898`) — already called out in the §4.4
   crosswalk. Relevant here because Patience's trigger reads
   `active_disposition: Option<DispositionKind>` at L104 instead
   (line 697). L104 is live; L102 is safe to delete.

Resolution of (1) is a behavior change under CLAUDE.md's balance
methodology — file a hypothesis + A/B result before landing. (2) and
(3) are documentation/cleanup and land without gating.

### §3.6 Granularity (ch 13 pain-scale discipline)

Today's `f32` scoring gives 2²³ ≈ 8M discrete levels. Ch 13
§"Accuracy / Too Many / Not Enough" argues: **pick granularity to
match what differentiation is actually meaningful.** A `Socialize`
score of 0.627 vs. 0.629 is noise; 0.62 vs. 0.68 is a decision.

Proposal: keep `f32` internally (composition math needs it), but
document that **tuning constants round to 2 significant figures**.
Our differentiation isn't finer than that anyway, and tighter
precision hides the actual behavioral signal in formatting noise —
when a balance iteration ships `sleep_night_bonus: 1.203948`, that's
a Rust-default formatting accident, not a considered value.

### §3.7 Cross-refs

- Ch 13 §"Weighted Sums" — composition shapes.
- Ch 13 §"Layered Weighting Models / Constructing a Layer / You Can't
  Always Eat What You Want" — Maslow pre-gate + post-scoring modifiers
  as layered filters.
- Ch 13 §"Propagation of Change" — how changes at each layer flow
  through the final score; informs A/B strategy during balance
  iteration.
- Ch 13 §"Compartmentalized Confidence" — why §3.5's modifier-as-
  distinct-layer matters for tuning discipline.

**Cross-ref:** `docs/reference/behavioral-math-ch13-factor-weighting.md`

---

## §4 Context tags — ECS markers as eligibility filters

Mark's "context tags" are filters that determine whether a DSE is
eligible to score at all. They are categorical, not scored:
`InCombat`, `HasWeapon`, `InjuredRight`, etc. The DSE evaluator skips
ineligible DSEs entirely, avoiding the cost of computing a score that
can't win.

**Clowder's collapse: three vocabularies, one pattern.** Context tags
in Mark's framework, ECS marker components in Bevy, and the boolean
eligibility fields in our current `ScoringContext` (27 of them, plus 9
in `FoxScoringContext`) are the same concept in three vocabularies.
All three collapse into ECS marker components inserted/removed by
per-tick systems.

DSE eligibility becomes a Bevy `Query<With<MarkerA>, Without<MarkerB>>`
filter — a first-class ECS operation, not a per-tick `if` statement.
The context-tag refactor and the pure-Bevy-idiom refactor are the
same refactor.

### §4.1 Tag categories

- **Species** — spawn-immutable: `Cat`, `Fox`, `Hawk`, `Snake`,
  `ShadowFox`, `Prey`.
- **Role** — set by role-resolution systems: `Coordinator`, `Mentor`,
  `Apprentice`.
- **LifeStage** — tick-maintained from `Age`: `Kitten`, `Young`,
  `Adult`, `Elder`.
- **State** — per-tick insert/remove by spatial or status systems:
  `Injured`, `Pregnant`, `InCombat`, `Incapacitated`, `OnCorruptedTile`,
  `OnSpecialTerrain`, `HasThreatNearby`, `Dead`, …
- **Capability** — derived per-tick from tag combinations:
  `CanHunt`, `CanForage`, `CanWard`, `CanCook`.
- **Inventory** — per-tick from item/colony state: `HasRemedyHerbs`,
  `HasFunctionalKitchen`, …
- **TargetExistence** — per-tick from spatial queries against
  colony/world: `HasSocialTarget`, `HasConstructionSite`, …
  Cross-ref §6 — these gate target-taking DSEs before per-target
  scoring runs.
- **Colony** — colony-scoped state attached to a colony-singleton
  entity, not duplicated per cat: `WardStrengthLow`, `WardsUnderSiege`,
  `HasFunctionalKitchen`, `ThornbriarAvailable`, …

**Relationship tags** (pairwise, asymmetric) — `BondedWith(Entity)`,
`SharesTerritoryWith(Entity)` — do not live here. They are part of
the ToT-style belief layer (out of scope for this substrate; see
Phase-5-equivalent work deferred to that thread).

**Cross-ref:** `docs/reference/behavioral-math-ch14-modeling-decisions.md`
(for DSE vocabulary; ch 14 is the integrated chapter).

### §4.2 Catalog schema

Each catalog row in §4.3 commits to eight columns. Status codes:
`Built` — component + author both exist. `Partial` — predicate is
computed imperatively in consumer code (typically inside the
`ScoringContext`-builder in `goap.rs`), but no marker component
exists yet. `Absent` — neither component nor predicate computation
exists.

| Column | Meaning |
|---|---|
| **Marker** | ECS component name (PascalCase, zero-sized unless a Data note appears). |
| **Category** | Species / Role / LifeStage / State / Capability / Inventory / TargetExistence / Colony / SpawnImmutable. |
| **Predicate** | Boolean condition the marker encodes, in terms of other components/resources. |
| **Insert** | `spawn` = inserted at entity creation. `tick:<file>::<fn>` = maintained by a per-tick system. `event:<MessageName>` = inserted reactively on a message. `—` = no author exists yet (status = Absent). |
| **Remove** | Same format. `—` if SpawnImmutable. |
| **Query** | Canonical consumer form: `Q<X, With<M>>`, `Q<X, Without<M>>`, or `Q<X, (With<A>, With<B>)>`. |
| **Status** | `Built` / `Partial` / `Absent`. |
| **Source** | Where the predicate lives today: `ScoringContext.<field>:<line>`, `FoxScoringContext.<field>:<line>`, `goap.rs:<line>`, `derived`, `spawn`, or `new`. |

### §4.3 Marker catalog

The vocabulary is **open**, not closed — see §5.6.9 for the extensibility
contract that governs additions. The rows below enumerate current
coverage; adding a marker later is writing one tick-system, not
refactoring consumers.

#### Species (spawn-immutable)

| Marker | Predicate | Insert | Remove | Query | Status | Source |
|---|---|---|---|---|---|---|
| `Cat` | Entity is a cat. Today the component is named `Species` (`identity.rs:17`); rename proposed for consistency with the other species markers. | `spawn`: `src/plugins/setup.rs` cat-spawn path | — | `Q<_, With<Cat>>` | Built (as `Species`) | `spawn` |
| `Fox` | Entity is a fox. Today carried as `WildAnimal { species: WildlifeKind::Fox }` — **not a marker**. Catalog proposes promoting to ZST so `Q<_, With<Fox>>` is disjoint from `Q<_, With<Cat>>` without projecting the enum. | `spawn`: `src/systems/wildlife.rs` | — | `Q<_, With<Fox>>` | Partial | `WildAnimal.species` |
| `Hawk` | As `Fox`. | `spawn`: `wildlife.rs` | — | `Q<_, With<Hawk>>` | Partial | `WildAnimal.species` |
| `Snake` | As `Fox`. | `spawn`: `wildlife.rs` | — | `Q<_, With<Snake>>` | Partial | `WildAnimal.species` |
| `ShadowFox` | Entity is a corruption-spawned shadow-fox. Currently a `WildlifeKind` variant. | `spawn`: `src/systems/magic.rs` shadowfox-spawn | — | `Q<_, With<ShadowFox>>` | Partial | `WildAnimal.species` |
| `Prey` | Entity is a prey animal. Today a `PreyAnimal` ZST (`prey.rs:130`); rename for consistency if desired. | `spawn`: `src/systems/prey.rs` | — | `Q<_, With<Prey>>` | Built (as `PreyAnimal`) | `spawn` |

Species markers are the one category where a query-disjointness argument
already motivates the change outside the §L2.10 DSE work. Per CLAUDE.md
ECS Rules: *when splitting `Query<&mut Component>` into separate
data/marker patterns, add `With<Marker>` to restore disjointness for
paired `Without<Marker>` filters in other queries.*

#### Role

| Marker | Predicate | Insert | Remove | Query | Status | Source |
|---|---|---|---|---|---|---|
| `Coordinator` | Cat holds the colony's coordinator role (elected by social weight). | `tick:coordination.rs::eval_coordinator_role` (~100-tick cadence) | same | `Q<_, With<Coordinator>>` | Built | `coordination.rs:14` |
| `Mentor` | Cat is the mentor side of a `Training { mentor, apprentice }` relationship. | `tick:aspirations.rs::update_training_markers` (proposed — today the predicate lives inside scoring via `has_mentoring_target` lookup) | same | `Q<_, With<Mentor>>` | Partial | `skills.rs::Training` |
| `Apprentice` | Cat is the apprentice side of a `Training` relationship. | same as `Mentor` | same | `Q<_, With<Apprentice>>` | Partial | `skills.rs::Training` |

#### LifeStage

Today `LifeStage` is a derived method: `Age::stage(current_tick,
ticks_per_season)` called from every consumer (`identity.rs:47`).
Promoting to markers means a single tick-system maintains exactly one
of the four markers per cat; consumers become `Q<_, With<Adult>>` etc.,
and the `Age::stage()` hot-call disappears. `KittenDependency`
(`kitten.rs:11`) stays a *data* component — it carries mother / father
/ maturity — and cross-refs the `IsParentOfHungryKitten` marker below.

| Marker | Predicate | Insert | Remove | Query | Status | Source |
|---|---|---|---|---|---|---|
| `Kitten` | `Age::stage() == Kitten` (0–3 seasons old). | `tick:growth.rs::update_life_stage_markers` (new — one of four markers mutually exclusive per cat) | same | `Q<_, With<Kitten>>` | Absent | `derived: Age::stage()` |
| `Young` | `Age::stage() == Young` (4–11 seasons). | same | same | `Q<_, With<Young>>` | Absent | `derived: Age::stage()` |
| `Adult` | `Age::stage() == Adult` (12–47 seasons). | same | same | `Q<_, With<Adult>>` | Absent | `derived: Age::stage()` |
| `Elder` | `Age::stage() == Elder` (48+ seasons). | same | same | `Q<_, With<Elder>>` | Absent | `derived: Age::stage()` |

#### State

| Marker | Predicate | Insert | Remove | Query | Status | Source |
|---|---|---|---|---|---|---|
| `Incapacitated` | `health.injuries.iter().any(\|i\| i.kind == Severe && !i.healed)`. Early-return in scoring still present; retires with §13.1 rows 1–3. | `tick:systems::incapacitation::update_incapacitation` (landed 2026-04-23) — Chain 2, before the GOAP scoring pipeline | same | `Q<_, With<Incapacitated>>` (emergency DSEs) or `Q<_, Without<Incapacitated>>` (non-emergency) | Author ✓; DSE consumer pending §13.1 | `ScoringContext.is_incapacitated:45` / `goap.rs:767` |
| `Injured` | `health.current < 1.0` OR any unhealed injury. Weaker than `Incapacitated`. | `tick:needs.rs::update_injury_marker` (new) | same | `Q<_, With<Injured>>` | Absent | `goap.rs:627` |
| `InCombat` | Cat's active plan is in a Fight step, or a hostile is attacking an adjacent cat. | `tick:combat.rs::update_combat_marker` (new) | same | `Q<_, With<InCombat>>` | Absent | action-level today; no component |
| `Pregnant` | Cat is gestating. Data: `Pregnant { conceived_tick, partner, litter_size, … }`. | `event:MateConceived`: `pregnancy.rs` | `event:KittenBorn`: `pregnancy.rs` | `Q<_, With<Pregnant>>` | Built | `pregnancy.rs:17` |
| `Dead` | Cat has died (prior to despawn grace). Data: `Dead { tick, cause }`. | `tick:death.rs::check_death_conditions` | — (despawn terminates entity) | `Q<_, With<Dead>>` | Built | `death.rs:72` |
| `OnCorruptedTile` | `map.get(pos).corruption > corrupted_tile_threshold`. | `tick:magic.rs::update_corrupted_tile_markers` (new — one insert/remove per cat per tick) | same | `Q<_, With<OnCorruptedTile>>` | Absent | `ScoringContext.on_corrupted_tile:76` / `goap.rs:808` |
| `OnSpecialTerrain` | Tile under cat is `Terrain::FairyRing \| Terrain::StandingStone`. | `tick:sensing.rs::update_terrain_markers` (new) | same | `Q<_, With<OnSpecialTerrain>>` | Absent | `ScoringContext.on_special_terrain:84` / `goap.rs:814` |
| `HasThreatNearby` | ≥1 wildlife hostile within species-attenuated detection range. | `tick:sensing.rs::update_threat_proximity_markers` (new) | same | `Q<_, With<HasThreatNearby>>` | Absent | `ScoringContext.has_threat_nearby:37` |

#### Capability

All derived per-tick from parent tags; all Absent today. A single
fan-out system (`src/ai/capabilities.rs`, new) reads the parent
markers it depends on and maintains all four capability markers
per cat.

| Marker | Predicate | Insert | Remove | Query | Status | Source |
|---|---|---|---|---|---|---|
| `CanHunt` | `With<Cat> + With<Adult> + Without<Injured> + Without<InCombat> + has_nearby_tile(hunt_terrain)`. | `tick:ai/capabilities.rs::update_capability_markers` (new) | same | `Q<_, With<CanHunt>>` | Absent | `ScoringContext.can_hunt:32` / `goap.rs:734` |
| `CanForage` | `With<Cat> + With<Adult> + Without<Injured> + has_nearby_tile(forage_terrain)`. | same | same | `Q<_, With<CanForage>>` | Absent | `ScoringContext.can_forage:33` / `goap.rs:737` |
| `CanWard` | `With<Cat> + With<Adult> + Without<Injured> + With<HasWardHerbs> + magic_skill >= ward_skill_floor`. | same | same | `Q<_, With<CanWard>>` | Absent | implicit in magic-scoring gates today |
| `CanCook` | `With<Cat> + With<Adult> + Without<Injured> + (ColonyState has HasFunctionalKitchen + HasRawFoodInStores)`. | same | same | `Q<_, With<CanCook>>` | Absent | implicit in cooking gates today |

#### Inventory

Per-cat inventory markers maintained via `Changed<Inventory>` — cheap
because the query only visits cats whose inventory mutated this tick.

| Marker | Predicate | Insert | Remove | Query | Status | Source |
|---|---|---|---|---|---|---|
| `HasHerbsInInventory` | `inventory.slots.iter().any(\|s\| matches!(s, ItemSlot::Herb(_)))`. | `tick:items.rs::update_inventory_markers` (new, `Changed<Inventory>` filter) | same | `Q<_, With<HasHerbsInInventory>>` | Absent | `ScoringContext.has_herbs_in_inventory:64` |
| `HasRemedyHerbs` | `inventory.has_remedy_herb()`. | same | same | `Q<_, With<HasRemedyHerbs>>` | Absent | `ScoringContext.has_remedy_herbs:66` |
| `HasWardHerbs` | `inventory.has_ward_herb()` (Thornbriar). | same | same | `Q<_, With<HasWardHerbs>>` | Absent | `ScoringContext.has_ward_herbs:68` |

Colony-scoped inventory markers attach to a single `ColonyState`
entity (or to the current `Coordinator` if the colony-singleton
pattern isn't yet in place):

| Marker | Predicate | Insert | Remove | Query | Status | Source |
|---|---|---|---|---|---|---|
| `HasFunctionalKitchen` | Any `Kitchen` structure with `site.is_none() && effectiveness() > 0.0`. | `tick:buildings.rs::update_colony_building_markers` (new) | same | `Q<With<ColonyState>, With<HasFunctionalKitchen>>` | Absent | `ScoringContext.has_functional_kitchen:141` / `goap.rs:556` |
| `HasRawFoodInStores` | `Stores` carries ≥1 raw-food item. | same | same | `Q<With<ColonyState>, With<HasRawFoodInStores>>` | Absent | `ScoringContext.has_raw_food_in_stores:143` |
| `HasStoredFood` | `Stores` carries ≥1 food item (raw or cooked). Gates `Eat`. | same | same | `Q<With<ColonyState>, With<HasStoredFood>>` | Absent | `ScoringContext.food_available:31` |
| `ThornbriarAvailable` | ≥1 harvestable Thornbriar exists in the world. | `tick:magic.rs::update_herb_availability_markers` (new) | same | `Q<With<ColonyState>, With<ThornbriarAvailable>>` | Absent | `ScoringContext.thornbriar_available:70` |

#### TargetExistence

Target-existence markers gate target-taking DSEs (§6) — they answer
"is there anything worth scoring targets against?" before per-target
scoring runs. Shared broad-phase with `sensing.rs`: a single spatial
tick fans out multiple markers per cat.

| Marker | Predicate | Insert | Remove | Query | Status | Source |
|---|---|---|---|---|---|---|
| `HasSocialTarget` | ≥1 visible cat within socialize range. | `tick:sensing.rs::update_target_existence_markers` (new) | same | `Q<_, With<HasSocialTarget>>` | Absent | `ScoringContext.has_social_target:35` |
| `HasHerbsNearby` | ≥1 `Harvestable` herb within gather range and visibility. | same | same | `Q<_, With<HasHerbsNearby>>` | Absent | `ScoringContext.has_herbs_nearby:62` |
| `PreyNearby` | ≥1 prey within hunt detection range (species-attenuated). Shared with foxes via `With<Prey>` + distance. | same | same | `Q<_, With<PreyNearby>>` | Absent | `ScoringContext.prey_nearby:95` / `FoxScoringContext.prey_nearby:36` |
| `CarcassNearby` | ≥1 uncleansed/unharvested carcass within range. | same | same | `Q<_, With<CarcassNearby>>` | Absent | `ScoringContext.carcass_nearby:127` |
| `HasConstructionSite` | ≥1 reachable `ConstructionSite`. | `tick:buildings.rs::update_colony_building_markers` | same | `Q<_, With<HasConstructionSite>>` | Absent | `ScoringContext.has_construction_site:47` |
| `HasDamagedBuilding` | ≥1 `Structure` with condition < 0.4. | same | same | `Q<_, With<HasDamagedBuilding>>` | Absent | `ScoringContext.has_damaged_building:49` |
| `HasGarden` | ≥1 garden building exists. | same | same | `Q<_, With<HasGarden>>` | Absent | `ScoringContext.has_garden:51` |
| `HasMentoringTarget` | ≥1 other cat has a skill < 0.3 where this cat has the same skill > 0.6. Per-cat (relative predicate). | `tick:aspirations.rs::update_mentoring_markers` (new) | same | `Q<_, With<HasMentoringTarget>>` | Absent | `ScoringContext.has_mentoring_target:93` |
| `HasEligibleMate` | Orientation-compatible partner with Partners+ bond exists (`mating::has_eligible_mate()`). | `tick:mating.rs::update_mate_eligibility_markers` (new) | same | `Q<_, With<HasEligibleMate>>` | Absent | `ScoringContext.has_eligible_mate:111` |
| `IsParentOfHungryKitten` | Cat is on the parent side of a `KittenDependency` whose linked kitten's hunger exceeds threshold. | `tick:growth.rs::update_parent_hungry_kitten_markers` (new) | same | `Q<_, With<IsParentOfHungryKitten>>` | Absent | `ScoringContext.is_parent_of_hungry_kitten:115` |

#### Colony

Colony-scoped markers attach to a `ColonyState` singleton entity
(introduce it as part of this substrate build if not already present).
DSE queries joining cat + colony use `(cat_q, colony_q.single())`.

| Marker | Predicate | Insert | Remove | Query | Status | Source |
|---|---|---|---|---|---|---|
| `IsCoordinatorWithDirectives` | `With<Coordinator> + DirectiveQueue.len() > 0`. Per-coordinator-cat, not on `ColonyState`. | `tick:coordination.rs::update_directive_markers` (new) | same | `Q<_, (With<Coordinator>, With<IsCoordinatorWithDirectives>)>` | Absent | `ScoringContext.is_coordinator_with_directives:87` |
| `WardStrengthLow` | Colony ward coverage: no wards OR average strength < 0.3. | `tick:magic.rs::update_ward_coverage_markers` (new) | same | `Q<With<ColonyState>, With<WardStrengthLow>>` | Absent | `ScoringContext.ward_strength_low:74` |
| `WardsUnderSiege` | Any colony ward has `WildlifeAiState::EncirclingWard` adjacent. | `tick:magic.rs::update_ward_siege_marker` (new) | same | `Q<With<ColonyState>, With<WardsUnderSiege>>` | Absent | `ScoringContext.wards_under_siege:133` / `goap.rs:620` |

#### SpawnImmutable

| Marker | Predicate | Insert | Remove | Query | Status | Source |
|---|---|---|---|---|---|---|
| `FateAssigned` | Cat's fated bonds have been computed (fate roll complete). | `fate.rs:78::evaluate_fate_assignment` | — | `Q<_, With<FateAssigned>>` | Built | `fate.rs:49` |
| `AspirationsInitialized` | Cat's aspirations/prefs have been initialized. | `aspirations.rs:260,272::init_aspirations` | — | `Q<_, With<AspirationsInitialized>>` | Built | `aspirations.rs:139` |

**`MagicAffinity` is not a marker** — it's an `f32` scalar component
(`skills.rs`). Magic DSEs gate it via *thresholded response curves*
(§2), not boolean eligibility. Listed in §4.5 scalar carve-out.

#### Reproduction

Reproduction is its own category because both rows below carry
reproductive-lifecycle semantics that don't fit elsewhere in the
catalog. `Fertility` is not a transient condition (vs. §4.3 State:
`Injured`, `InCombat`) and not socially conferred (vs. §4.3 Role:
`Coordinator`, `Mentor`) — it's a cycled physiological state whose
phase feeds §6.5.2's Mate consideration. `Parent` likewise is
lifecycle-scoped (active, not lifetime — see below) and query-used
for aspiration routing, not for target-side kinship.

Three design commitments the rows below depend on:

- **`KittenDependency` remains the canonical data for parent-of-kitten
  lookup.** §6.5.6's target-side `target.Parent == self` check reads
  `KittenDependency { mother, father }` on the kitten directly;
  `Parent` is a self-side query-optimization marker for "is this cat
  a parent?", not a new source of truth for the pair predicate.
- **`Fertility` catalog row commits the shape; §7.M.7 commits the
  lifecycle.** §7.M.7 resolved (2026-04-21) as a phase-bearing
  component with `fertility.rs`-maintained pure-function phase
  transitions. The phase enum also expands from four to **five
  variants** — §7.M.7.3 adds a dedicated `Postpartum` phase for
  the nursing interval, enabling narrative templates and
  event-filter queries to cleanly distinguish environmental
  (winter) from biological (post-birth) suppression, even though
  scoring treats both identically (§7.M.7.5). Gender mapping is
  canon per §7.M.7.4: Queens gestate, Toms sire, Nonbinaries do
  both; only Queens and NBs carry `Fertility`.
- **`Parent` is active parenthood, not lifetime identity.** A cat
  loses the marker when their last dependent kitten either matures
  (`KittenDependency` dissolves at `maturity ≥ 1.0`) or dies.
  Consumers that want lifetime-kin semantics — future `is_kin()`,
  grief-of-parent queries, sibling detection — must not lean on
  `Parent` as a lifetime-identity proxy. That information lives in
  death-event payloads (§7.7.b's planned `CatDied { cause, deceased,
  survivors_by_relationship }` vocabulary expansion) and in future
  kinship-relation components, not in a sticky ZST. Rationale:
  making `Parent` sticky would be a one-off break from the catalog's
  lifecycle-scoped convention (compare `Pregnant`, `InCombat`,
  `Incapacitated` — all transient, all removed when the generating
  condition clears).

**Ordering hazard — `Parent` removal vs. grief emission.** If
`growth.rs::update_parent_markers` runs before any grief/death
consumer in the same tick, the surviving cat loses `Parent` before a
grief system could query "was this cat a parent of the deceased?" The
catalog contract is: consumers MUST NOT infer grief-parent status by
querying `With<Parent>` on survivors post-death. The canonical
parent-at-time-of-death channel is the `CatDied` event payload
(future §7.7.b `survivors_by_relationship` field). Implementation-side
schedule ordering between `update_parent_markers` and the §7.7.b
grief cascade is a follow-on when both land.

| Marker | Predicate | Insert | Remove | Query | Status | Source |
|---|---|---|---|---|---|---|
| `Fertility { phase: FertilityPhase, cycle_offset, post_partum_remaining_ticks }` | Cat is cycle-capable; `phase` ∈ {Proestrus, Estrus, Diestrus, Anestrus, Postpartum} (5 variants — Postpartum added by §7.M.7.3 for the nursing interval, scoring-equivalent to Anestrus per §7.M.7.5 but narratively distinguishable). Mutually exclusive with `With<Pregnant>`. Only Queens and Nonbinaries carry it (§7.M.7.4 canon — Toms are sire-only and never carry the marker). **Lifecycle per §7.M.7** — four-phase cycling plus a dedicated Postpartum interval, maintained by `fertility.rs` as a pure function of `(tick + cycle_offset, season, post_partum_remaining)`. Winter forces Anestrus on every Fertility-bearing cat. | `tick:growth.rs::update_life_stage_markers` (Adult entry, gender-gated to skip Toms; alongside Kitten/Young/Adult/Elder life-stage markers) + `tick:fertility.rs::handle_post_partum_reinsert` (on `KittenBorn`, with `phase = Postpartum` and `post_partum_remaining_ticks` set to `fertility_post_partum_recovery_ticks` per §7.M.7.3). Initial phase computed by §7.M.7.2 at insert tick. | `tick:growth.rs::update_life_stage_markers` (Adult→Elder transition — tightens today's-Elders-can-mate per §7.M.7.1) OR `tick:fertility.rs::handle_conception_remove` on `event:MateConceived` (atomic with `Pregnant` insert). Re-insert path per Insert cell. | `Q<_, With<Fertility>>` (eligibility — Toms excluded by construction) or `Q<&Fertility>` (phase-scoped scoring per §6.5.2 fertility-window curve) | Absent | `ScoringContext: —` (no current field; verisimilitude gap flagged at `mating.rs:102–114` season-granularity; §7.M.7.5 phase→scalar mapping supersedes) |
| `Parent` | **Active parenthood:** cat has ≥1 living entity with `KittenDependency.mother == self` or `KittenDependency.father == self`. Mutually compatible with `Pregnant`, `Mentor`, all life-stage markers ≥ Adult. See block prose for lifecycle-scoped rationale. | `tick:growth.rs::update_parent_markers` (new) — runs after `tick_kitten_growth`; single pass over `Query<&KittenDependency>` fans out markers onto referenced parent entities. | Same system removes `Parent` from any cat whose **last** dependent kitten matured (`maturity ≥ 1.0`) OR died this tick. Death-of-kitten path: listen for `CatDied` where deceased carried `KittenDependency`, then re-evaluate the named parents. | `Q<_, With<Parent>>` (aspiration-layer "is parent" filter — future `ProvideKittenAspiration` routing, post-§7.M-pattern kitten-care aspirations) | Absent | `ScoringContext: —` (only the self-side aggregate `is_parent_of_hungry_kitten:115` exists, which is strictly narrower: parent *and* kitten-hungry) |

#### Fox-specific

Fox-side tick systems (`fox_goap.rs`, `fox_spatial.rs`) maintain the
fox's Maslow-3 eligibility markers. `PreyNearby` is shared with the
cat catalog above — the same spatial tick produces it for both.

| Marker | Predicate | Insert | Remove | Query | Status | Source |
|---|---|---|---|---|---|---|
| `StoreVisible` | Colony store within raid range (fox PoV). | `tick:fox_spatial.rs::update_store_awareness_markers` (new) | same | `Q<With<Fox>, With<StoreVisible>>` | Absent | `FoxScoringContext.store_visible:40` |
| `StoreGuarded` | Visible store has cats within guard range. | same | same | `Q<With<Fox>, With<StoreGuarded>>` | Absent | `FoxScoringContext.store_guarded:42` |
| `CatThreateningDen` | Cat within 5 tiles of fox's den AND cubs present. | `tick:fox_spatial.rs::update_den_threat_markers` (new) | same | `Q<With<Fox>, With<CatThreateningDen>>` | Absent | `FoxScoringContext.cat_threatening_den:46` |
| `WardNearbyFox` | Ward within fox detection radius. Stubbed in `FoxScoringContext` today. | `tick:fox_spatial.rs::update_ward_detection_markers` (new) | same | `Q<With<Fox>, With<WardNearbyFox>>` | Absent | `FoxScoringContext.ward_nearby:48` (stub) |
| `HasCubs` | Fox has ≥1 cub at its den. | `event:CubsBorn` + on-despawn cleanup | same | `Q<With<Fox>, With<HasCubs>>` | Absent | `FoxScoringContext.has_cubs:56` |
| `CubsHungry` | `cub_satiation < 0.4`. | `tick:fox_goap.rs::update_cub_hunger_markers` (new) | same | `Q<With<Fox>, With<CubsHungry>>` | Absent | `FoxScoringContext.cubs_hungry:58` |
| `IsDispersingJuvenile` | `life_stage == Juvenile && home_den.is_none()`. | `tick:fox_goap.rs::update_juvenile_dispersal_markers` (new) | same | `Q<With<Fox>, With<IsDispersingJuvenile>>` | Absent | `FoxScoringContext.is_dispersing_juvenile:62` |
| `HasDen` | `FoxState.home_den.is_some()`. | `event:DenClaimed` / `event:DenLost` | same | `Q<With<Fox>, With<HasDen>>` | Absent | `FoxScoringContext.has_den:64` |

### §4.4 Crosswalk: ScoringContext → markers

Every boolean field in `ScoringContext` and `FoxScoringContext` has
exactly one of the following dispositions. Source lines refer to the
field declaration inside the struct; the disposition column either
points to the §4.3 row this field collapses into or names the reason
the field stays a runtime sample / dead field.

#### Cat `ScoringContext` (27 booleans)

| Field | Line | Disposition |
|---|---|---|
| `food_available` | 31 | → `HasStoredFood` (§4.3 Inventory — colony-scoped; broader than `HasRawFoodInStores`). |
| `can_hunt` | 32 | → `CanHunt` (§4.3 Capability). |
| `can_forage` | 33 | → `CanForage` (§4.3 Capability). |
| `has_social_target` | 35 | → `HasSocialTarget` (§4.3 TargetExistence). |
| `has_threat_nearby` | 37 | → `HasThreatNearby` (§4.3 State). |
| `is_incapacitated` | 45 | → `Incapacitated` (§4.3 State). |
| `has_construction_site` | 47 | → `HasConstructionSite` (§4.3 TargetExistence). |
| `has_damaged_building` | 49 | → `HasDamagedBuilding` (§4.3 TargetExistence). |
| `has_garden` | 51 | → `HasGarden` (§4.3 TargetExistence). |
| `has_herbs_nearby` | 62 | → `HasHerbsNearby` (§4.3 TargetExistence). |
| `has_herbs_in_inventory` | 64 | → `HasHerbsInInventory` (§4.3 Inventory). |
| `has_remedy_herbs` | 66 | → `HasRemedyHerbs` (§4.3 Inventory). |
| `has_ward_herbs` | 68 | → `HasWardHerbs` (§4.3 Inventory). |
| `thornbriar_available` | 70 | → `ThornbriarAvailable` (§4.3 Inventory — colony-scoped). |
| `ward_strength_low` | 74 | → `WardStrengthLow` (§4.3 Colony). |
| `on_corrupted_tile` | 76 | → `OnCorruptedTile` (§4.3 State). |
| `on_special_terrain` | 84 | → `OnSpecialTerrain` (§4.3 State). |
| `is_coordinator_with_directives` | 87 | → `IsCoordinatorWithDirectives` (§4.3 Colony). |
| `has_mentoring_target` | 93 | → `HasMentoringTarget` (§4.3 TargetExistence). |
| `prey_nearby` | 95 | → `PreyNearby` (§4.3 TargetExistence; shared with foxes). |
| `has_active_disposition` | 102 | **Dead field — flag for deletion.** Production value is hardcoded `false` (`goap.rs:898`) and never read in `score_actions`. Patience-commitment bonus reads `active_disposition: Option<DispositionKind>` at line 104 instead. Replacement is `Q<&Disposition>` — no marker needed. Add a `docs/open-work.md` entry to remove the field in a later PR. |
| `has_eligible_mate` | 111 | → `HasEligibleMate` (§4.3 TargetExistence). |
| `is_parent_of_hungry_kitten` | 115 | → `IsParentOfHungryKitten` (§4.3 TargetExistence). |
| `carcass_nearby` | 127 | → `CarcassNearby` (§4.3 TargetExistence). |
| `wards_under_siege` | 133 | → `WardsUnderSiege` (§4.3 Colony). |
| `has_functional_kitchen` | 141 | → `HasFunctionalKitchen` (§4.3 Inventory — colony-scoped). |
| `has_raw_food_in_stores` | 143 | → `HasRawFoodInStores` (§4.3 Inventory — colony-scoped). |

#### Fox `FoxScoringContext` (9 booleans)

| Field | Line | Disposition |
|---|---|---|
| `prey_nearby` | 36 | → `PreyNearby` (§4.3 TargetExistence — shared with cats). |
| `store_visible` | 40 | → `StoreVisible` (§4.3 Fox-specific). |
| `store_guarded` | 42 | → `StoreGuarded` (§4.3 Fox-specific). |
| `cat_threatening_den` | 46 | → `CatThreateningDen` (§4.3 Fox-specific). |
| `ward_nearby` | 48 | → `WardNearbyFox` (§4.3 Fox-specific). |
| `has_cubs` | 56 | → `HasCubs` (§4.3 Fox-specific). |
| `cubs_hungry` | 58 | → `CubsHungry` (§4.3 Fox-specific). |
| `is_dispersing_juvenile` | 62 | → `IsDispersingJuvenile` (§4.3 Fox-specific). |
| `has_den` | 64 | → `HasDen` (§4.3 Fox-specific). |

### §4.5 Scalar carve-out

These fields stay as sampled values read inside DSE consideration
evaluation. They are not markers because they scale or threshold a
score via response curves (§2), not gate eligibility. Listed here so
future sessions do not re-propose them as markers.

#### Cat `ScoringContext` scalars (19)

| Field | Type | Why scalar, not marker |
|---|---|---|
| `allies_fighting_threat` | `usize` | Count scales the Fight score; one-marker-per-count is not viable. |
| `combat_effective` | `f32` | Multiplicative modifier on Fight score. |
| `health` | `f32` | Feeds response curves for Rest, Fight, injured-self-groom. |
| `food_fraction` | `f32` | Scarcity curve for Hunt / Forage / Farm / Cook. |
| `magic_affinity` | `f32` | Spawn-immutable scalar; thresholded via curve (§2), not boolean. |
| `magic_skill` | `f32` | Scales magic-family DSEs; thresholded via curve. |
| `herbcraft_skill` | `f32` | As `magic_skill`. |
| `colony_injury_count` | `usize` | Scales Herbcraft-Prepare urgency. |
| `tile_corruption` | `f32` | Scales Cleanse urgency and Social suppression. |
| `nearby_corruption_level` | `f32` | Scales proactive Cleanse / SetWard bonus. |
| `pending_directive_count` | `usize` | Scales Coordinate urgency; presence gated by `IsCoordinatorWithDirectives`. |
| `phys_satisfaction` | `f32` | Temper modifier input; continuous. |
| `respect` | `f32` | Pride modifier input; continuous. |
| `tradition_location_bonus` | `f32` | Post-scoring additive bonus (§3.5). Pre-computed by caller; currently always 0.0 in production. |
| `hungry_kitten_urgency` | `f32` | Scales Caretake; presence gated by `IsParentOfHungryKitten`. |
| `unexplored_nearby` | `f32` | Scales Explore. |
| `fox_scent_level` | `f32` | Scales territorial suppression. |
| `nearby_carcass_count` | `usize` | Scales Magic-Harvest urgency; presence gated by `CarcassNearby`. |
| `territory_max_corruption` | `f32` | Scales colony-wide corruption response. |

Non-scalar, non-marker fields: `active_disposition:
Option<DispositionKind>` (a reference into the `Disposition` component
— replaceable by `Q<&Disposition>`), `day_phase: DayPhase` (enum read
as resource, not entity-attached).

#### Fox `FoxScoringContext` scalars (5)

| Field | Type | Why scalar, not marker |
|---|---|---|
| `local_prey_belief` | `f32` | Fox-hunting conviction curve (from `FoxHuntingBeliefs`). |
| `cats_nearby` | `usize` | Scales avoidance pressure. |
| `local_threat_level` | `f32` | Fox-threat-memory sample. |
| `local_exploration_coverage` | `f32` | Fox-exploration-map sample. |
| `ticks_since_patrol` | `u64` | Patrol-pressure accumulator. |

### §4.6 Authoring-system roster

Grouped by author file — answers *"when this file's tick runs, which
markers does it maintain?"* Useful when scoping a PR touching any one
marker cluster, to know who the downstream query consumers are.

- **`src/systems/needs.rs`** → `Injured`.
- **`src/systems/incapacitation.rs`** → `Incapacitated` (landed 2026-04-23; hosted in a dedicated module rather than `needs.rs`).
- **`src/systems/magic.rs`** → `OnCorruptedTile`, `WardStrengthLow`,
  `WardsUnderSiege`, `ThornbriarAvailable`.
- **`src/systems/sensing.rs`** → `OnSpecialTerrain`, `HasThreatNearby`,
  `HasSocialTarget`, `HasHerbsNearby`, `PreyNearby`, `CarcassNearby`.
- **`src/systems/combat.rs`** → `InCombat`.
- **`src/systems/coordination.rs`** → `Coordinator` (exists),
  `IsCoordinatorWithDirectives`.
- **`src/systems/buildings.rs`** → `HasFunctionalKitchen`,
  `HasRawFoodInStores`, `HasStoredFood`, `HasConstructionSite`,
  `HasDamagedBuilding`, `HasGarden`.
- **`src/systems/items.rs`** → `HasHerbsInInventory`, `HasRemedyHerbs`,
  `HasWardHerbs`.
- **`src/systems/growth.rs`** → `Kitten`, `Young`, `Adult`, `Elder`,
  `IsParentOfHungryKitten`, `Parent`.
- **`src/systems/mating.rs`** → `HasEligibleMate`.
- **`src/systems/aspirations.rs`** → `Mentor`, `Apprentice`,
  `HasMentoringTarget`.
- **`src/systems/death.rs`** → `Dead` (exists).
- **`src/systems/pregnancy.rs`** → `Pregnant` (exists).
- **`src/systems/fertility.rs`** *(new file)* → `Fertility`. Runs
  three systems per §7.M.7.7: `update_fertility_phase` (100-tick
  cadence; applies §7.M.7.2 transition function per Fertility-
  bearing cat — Toms excluded by construction per §7.M.7.4),
  `handle_post_partum_reinsert` (listens `KittenBorn`, re-inserts
  `Fertility` on the birthing mother with `phase = Postpartum` and
  `post_partum_remaining_ticks` set), `handle_conception_remove`
  (listens `MateConceived`, atomically removes `Fertility` as
  `Pregnant` lands via `pregnancy.rs`). Adult-entry insert and
  Elder-exit remove live in `growth.rs` per §4.3 LifeStage-block
  authoring convention; `fertility.rs` reacts to life-stage
  transitions rather than authoring them. The Adult-entry insert
  is gender-gated (Queens + NBs only).
- **`src/systems/fate.rs`** → `FateAssigned` (exists).
- **`src/ai/capabilities.rs`** *(new file)* → `CanHunt`, `CanForage`,
  `CanWard`, `CanCook`.
- **`src/systems/fox_spatial.rs`** → `StoreVisible`, `StoreGuarded`,
  `CatThreateningDen`, `WardNearbyFox`.
- **`src/systems/fox_goap.rs`** → `CubsHungry`, `IsDispersingJuvenile`.
- **`src/systems/wildlife.rs` / spawn path** → `Fox`, `Hawk`, `Snake`,
  `ShadowFox`, `HasDen`, `HasCubs`.
- **Spawn-time only:** `Cat` (→ `Species` today), `Prey` (→
  `PreyAnimal` today), `FateAssigned`, `AspirationsInitialized`.

**Cadence guidance:**
- **Per-cat per-tick:** capability (4), life-stage (1-of-4), state
  markers (`Incapacitated`, `Injured`, `InCombat`, `OnCorruptedTile`,
  `OnSpecialTerrain`, `HasThreatNearby`). Use `Changed<T>` filters
  where the predicate depends on changes in a parent component
  (`Inventory`, `Health`, `Position`) to skip inert cats.
- **Per-cat per-hundred-ticks:** role markers (`Mentor`, `Apprentice`),
  aligned with `coordination.rs` cadence.
- **Colony per-few-ticks:** colony-scoped markers on the `ColonyState`
  entity (`WardStrengthLow`, `WardsUnderSiege`, `HasFunctionalKitchen`,
  …).
- **Event-driven:** `Pregnant`, `Dead`, `HasCubs`, `HasDen` — clean
  state transitions that don't need per-tick polling.

**Cross-ref:** §1.1 (markers carry eligibility, not scores), §5.6.7
(pre-enumeration sketch — see §4.3 for the full catalog), §L2.10.3
(DSE registration consumes these markers via `Query<With<…>>`
signatures).

---

## §5 Influence-map substrate

This is the substantive section of the spec — Mark's IAM chapter
gives us the full architecture, and our existing scent system is
half an implementation already.

### §5.1 Base maps, templates, working maps

Per Mark ch 30:

- **Base maps** — persistent 2D grids storing one quantity (proximity,
  threat, corruption level, scent). One map per (sensory channel ×
  faction). Updated every few ticks (not every frame) by iterating
  registered emitters and *stamping* their influence via precomputed
  templates.
- **Templates** — precomputed normalized `[0, 1]` stamps sized to an
  agent's effective range. One per movement speed band, one per
  threat range. Stamped additively into the base map, scaled by
  per-agent magnitude. Eliminates repetitive distance + curve
  calculations.
- **Working maps** — ephemeral, small, agent-centered. Constructed
  at decision time by combining base maps via modular recipes
  (`add`, `multiply`, `inverse`, `normalize`). Queried for point
  values or area maxima.

Clowder's `src/systems/wind.rs` + `src/systems/sensing.rs` is already
a base map (scent proximity, decay with distance, accumulation
across multiple emitters). The refactor is recognize-and-generalize,
not build-from-scratch.

### §5.2 Sensory channels (Clowder-specific extension)

Mark's chapter treats all agents as reading the same map
identically. Clowder extends this: each base map is *authored* on a
sensory channel (sight / hearing / scent / tremor), and each agent
*reads* through a per-species × per-injury × environmental
attenuation:

```
perceived_value = map.sample(position, channel)
                  * agent.species.sensitivity(channel)
                  * (1.0 - agent.injury.deficit(channel))
                  * world.environment.multiplier(channel)
```

Consequences:
- Species variation is native. A fox reads scent at 1.3, a hawk at
  0.4, a cat at 1.0 — all from the same base map.
- `docs/systems/sensory.md`'s "environmental multipliers pinned at
  1.0" is resolved: rain reduces scent reads, fog reduces sight
  reads, etc.
- Body Zones (`docs/systems/body-zones.md`) connects to the
  substrate naturally: a damaged nose reduces scent reads; a
  punctured ear reduces hearing reads. Body Zones can promote
  Partial → Built on the back of this layer.
- The "line of sight" problem Mark's chapter raises becomes
  "line of channel" — each channel carries its own propagation mode
  (scent follows wind, sight blocked by opaque obstacles, tremor
  blocked by water, hearing blocked by distance only).

### §5.3 Decay

Mark's chapter zeros base maps each update and re-stamps fresh.
This is correct for scent (driven by current emitters + wind) but
wrong for corruption (which has momentum) and wards (which decay
slowly). Clowder adds a per-map `decay_per_update: f32` parameter:
- `0.0` = full re-stamp (scent)
- `0.95` = slow persistence (wind gusts)
- `0.99` = corruption (emitters add; natural decay removes)
- `1.0` = indestructible until explicitly decayed (wards,
  territory markers)

### §5.4 Obstacle-aware propagation

Mark's chapter recommends precomputed Dijkstra distances for
pursuit-threat maps where obstacles matter. Clowder inherits this
for some channels but not all:
- **Scent**: obstacle-free (scent drifts around fences); linear
  distance + wind vector.
- **Sight**: obstacle-blocked by opaque geometry (buildings, dense
  foliage); LOS raycasting.
- **Tremor**: obstacle-blocked by water; Manhattan distance with
  water mask.
- **Pursuit proximity** (threat maps): Dijkstra path distance.

### §5.5 Social influence (deferred to ToT phase)

Pairwise social affinity between cats is NOT modelled as a base map
here. It is pairwise and asymmetric (cat A's fondness for cat B ≠
B's fondness for A; there are 8*7 = 56 pairs to track for an
8-cat colony, growing with pop²). This belongs to the Ryan et al.
Talk-of-the-Town belief-model phase, not this substrate.

What *does* live here: congregation maps (where are cats gathering),
not relationship maps (whom do I want to be with).

**Cross-ref:** `docs/reference/modular_tactical_influence_maps.md`
(canonical IAM reference).

### §5.6 L1 context enumeration

#### §5.6.1 Purpose

This subsection enumerates every L1 piece the preceding §5.1–§5.5 sections
describe, with status flags against current code. A reader can scan it cold
to confirm, before any L2 build begins, which parts of the sensory / spatial
surface are Built, Partial, or Absent. §5.1–§5.5 give the architecture;
§5.6 is the checklist.

#### §5.6.2 Sensory channels

| Channel | Base-range model | Current propagation | End-state propagation | Status |
|---|---|---|---|---|
| Sight | per-species range × acuity × falloff | LoS via Bresenham (`sensing.rs:394–431`) | Obstacle-blocked via opaque geometry | **Built** |
| Hearing | per-species range × acuity | Distance-only (`sensing.rs:300–305`) | Distance-only (matches spec) | **Built** |
| Scent | per-species range × acuity × wind-directional flag | Wind-aware + terrain-modulated (`sensing.rs:525–563`) | Wind vector + obstacle diffusion (not bresenham-blocked) | **Partial** — diffusion-around-obstacles missing |
| Tremor | per-species range × action-multiplier | `terrain.tremor_transmission()` modulation (`sensing.rs:312–317`) | Explicit water-mask blocking | **Partial** — water masking absent |

The four channels above cover Clowder's *currently understood* sensory
needs. They are **not a closed set**. If L2 design surfaces a need for a
new channel (e.g., a pheromone / scent-mark channel distinct from scent
itself, or an acoustic-signature channel for cat-to-cat calls), the L1
substrate must accept it as a registration, not as a refactor. §5.6.9
defines the defensive-structuring requirements that keep the channel set
open.

#### §5.6.3 Base maps required

Every influence map the refactor needs, derived from the spatial-fact
`ScoringContext` fields + target-existence-collapse booleans + existing
half-baked maps. Table schema mirrors §4.3 marker-catalog density:
`Map | Channel × faction | Backs ScoringContext (field:line) | Grid rep |
Update cadence | Propagation (→ §5.6.4) | Decay (→ §5.6.5) | Current
backing | Status`.

**Grid-representation vocabulary** used in the catalog:
- *flat `Vec<f32>` per-tile* — ExplorationMap shape, 1:1 with `TileMap`.
- *per-tile field on `TileMap`* — corruption shape; lives on the tile
  struct, not a separate resource.
- *bucketed overlay* — FoxScentMap / CatPresenceMap shape, coarser than
  tile-grain.
- *sparse per-emitter* — on-demand pairwise proximity (today's scent
  pattern in `wind.rs` + `sensing.rs`).
- *on-demand ECS iteration* — no persistent grid; query recomputes
  every scoring pass. This is the "absent" default.

**Propagation shorthand** (full per-channel details in §5.6.4):
`LoS-raycast`, `distance-only`, `wind+terrain`, `Manhattan+water-mask`,
`emitter-stamp-falloff`, `weighted-flood-fill`.

Each row's ScoringContext line number is the field declaration in
`src/ai/scoring.rs`'s `ScoringContext` struct (lines 27–144 today; spot-
check the field name if the line has drifted).

| # | Map | Channel × faction | Backs ScoringContext (field:line) | Grid rep | Update cadence | Propagation (§5.6.4) | Decay (§5.6.5) | Current backing | Status |
|---|---|---|---|---|---|---|---|---|---|
| 1 | Scent-proximity (per-emitter species) | scent × per-species (mouse / rat / rabbit / fish / bird / fox / hawk / snake / shadow-fox) | `has_threat_nearby:37`, `fox_scent_level:124`, `prey_nearby:95`, `carcass_nearby:127`, `nearby_carcass_count:129` (scent-side target-existence) | sparse per-emitter today; persistent bucketed grid at end-state | every tick (scent is driven by current emitters + wind) | `wind+terrain` | `0.0` (full re-stamp) | `cat_smells_prey_windaware()` at `src/systems/sensing.rs:525–563` + `wind.rs` `WindState` resource | **Partial** — implicit per-query; no persistent grid |
| 2 | Corruption level | sight-independent spatial × neutral | `tile_corruption:78`, `nearby_corruption_level:82`, `on_corrupted_tile:76`, `territory_max_corruption:131` | per-tile field on `TileMap` (single `f32` per tile) | every `corruption_spread_interval` ticks (default 10, `sim_constants.rs::MagicConstants`) | `Manhattan+water-mask` at end-state; today 4-adjacent spread | `0.99` (momentum; emitters add, pushback subtracts) | `corruption_spread` at `src/systems/magic.rs:41–91` + `CorruptionPushback` messages | **Partial** — substrate exists; needs generalization to the shared influence-map API |
| 3 | Ward coverage / strength | sight-independent spatial × colony | `ward_strength_low:74`, `wards_under_siege:133` | none today; flat `Vec<f32>` per-tile (or bucketed) at end-state | event-driven (ward placement / cleansing / siege-tick decay) | `emitter-stamp-falloff` (radial around each ward) | `1.0` (player-intentional; siege pressure is the only decay path — `ward_decay` at `src/systems/magic.rs:100+`) | aggregated ad-hoc per ward query (no shared map) | **Absent** |
| 4 | Fox-scent threat-proximity | scent × fox-faction | `has_threat_nearby:37`, `fox_scent_level:124` | bucketed overlay (`src/resources/fox_scent_map.rs:10–20`: `grid_w × grid_h`) | every tick (fox movement deposits; global `decay_all()` per tick) | `emitter-stamp-falloff` (wind-independent today; should inherit scent's `wind+terrain` at end-state) | `0.90` (fast-fading — older deposits prevent stale hotspots) | `FoxScentMap::deposit` / `decay_all` / `highest_nearby` in `src/resources/fox_scent_map.rs` | **Partial** — grid exists but isn't consumed via a uniform substrate API; wind modulation absent |
| 5 | Prey-location | sight × per-prey-species (mouse / rat / rabbit / fish / bird) | `prey_nearby:95`, Hunt target-taking (via GOAP) | none today; one bucketed overlay per prey-kind at end-state | every tick during active hunts; otherwise every N ticks | `LoS-raycast` (cats sight-hunt) + `distance-only` for hearing fallback | `0.0` (prey move — re-stamp per update) | ECS iteration in GOAP hunt-target selection (no persistent grid) | **Absent** |
| 6 | Carcass-location | scent × neutral | `carcass_nearby:127`, `nearby_carcass_count:129` | none today; bucketed overlay at end-state | event-driven on carcass spawn / cleanup | `emitter-stamp-falloff` using scent propagation | `0.95` (carcasses persist for days; decay via rat consumption / sim-aging) | ECS iteration per query | **Absent** |
| 7 | Food-location (colony stores) | sight × colony | `food_available:31`, `has_raw_food_in_stores:143`, `has_functional_kitchen:141` | none today; sparse per-entity (stores/kitchen positions) at end-state | event-driven (stores placed / destroyed; restock cycle) | `LoS-raycast` (stores are placed structures) | `1.0` (static until consumed / destroyed) | per-query ECS iteration over `Stores` / `Kitchen` entities | **Absent** |
| 8 | Herb-location | sight × neutral | `has_herbs_nearby:62`, `thornbriar_available:70` | none today; flat `Vec<f32>` per-tile (herb density, keyed by herb kind) at end-state | every tick during growth phase; event-driven on harvest | `LoS-raycast` (plants are visible) | `1.0` (static until harvested; regrow restamps) | per-query ECS iteration over `Herb` + `Harvestable` | **Absent** |
| 9 | Construction / damaged-building | sight × colony | `has_construction_site:47`, `has_damaged_building:49` | none today; sparse per-entity (sites by position) at end-state | event-driven (site placed / building damaged / repair complete) | `LoS-raycast` | `1.0` (entity lifecycle) | per-query ECS iteration over construction / building entities | **Absent** |
| 10 | Garden-location | sight × colony | `has_garden:51` | none today; sparse per-entity at end-state | event-driven (garden placed / destroyed) | `LoS-raycast` | `1.0` (entity lifecycle) | per-query ECS iteration | **Absent** |
| 11 | Exploration state | derived overlay × observer-specific | `unexplored_nearby:120` | flat `Vec<f32>` per-tile (`src/resources/exploration_map.rs:8–12`) | every tick per active explorer | `distance-only` within `explore_range` | `0.999` (near-permanent; slow fade models memory over seasons) | `ExplorationMap` resource + `unexplored_fraction_nearby()` helper | **Partial** — map exists and is read; decay + multi-observer attribution absent |
| 12 | Congregation (cat-density) | sight × colony | `has_social_target:35` (plus §5.5's "where are cats gathering") | bucketed overlay (`src/resources/cat_presence_map.rs:10–19`) | every tick per cat movement | `emitter-stamp-falloff` (short-range; matches social scoring range) | `0.85` (faster than scent; 30-second-old deposits aren't social affinity) | `CatPresenceMap::deposit` / `decay_all` / `highest_nearby` | **Partial** — grid exists, not yet consumed by the shared substrate API |
| 13 | Kitten-urgency (proximity-weighted) | sight × colony | `hungry_kitten_urgency:113`, `is_parent_of_hungry_kitten:115` (Caretake target-taking) | none today; bucketed overlay at end-state | every tick (kittens move + need state changes fast) | `emitter-stamp-falloff` (urgency as magnitude) | `0.0` (re-stamp per update — current urgency only) | per-query aggregate over kitten entities | **Absent** |

**13 maps, tally after enumeration:** 0 Built, 5 Partial (#1 scent, #2
corruption, #4 fox-scent, #11 exploration, #12 congregation), 8 Absent.
The three bucketed overlays (#4, #12) + full-tile `ExplorationMap` (#11)
were previously counted as Absent in §5.6.8; the enumeration surfaces
that they exist-as-data but aren't consumed via the uniform substrate
API. §5.6.8 status-summary row updates accordingly.

Pairwise social affinity (§5.5) is **not** in this catalog — it belongs
to the ToT belief layer. "Congregation" (#12, *where are cats
gathering*) is in scope here; "relationship" (whom does cat A want to
be near) is not.

**Explicitly open-set, not enumeration debt:** this table commits
today's 13 known maps. §5.6.9 #1 makes the storage registry
`(channel_id, faction_id)`-keyed, so adding a 14th map (e.g., a
pheromone-mark channel, a fire-danger map, a sacred-site draw) is a
registration, not a schema change.

#### §5.6.4 Propagation modes per channel

*Per-map propagation assignments live in §5.6.3's catalog — this section
describes the per-channel strategies each map picks from.*

| Channel | Obstacle handling | Distance model | Today | Gap |
|---|---|---|---|---|
| Sight | LoS raycasting (blocked by opaque geometry) | Linear / falloff | Bresenham in `sensing.rs:394–431` | Cat-side application pending; wildlife side uses it |
| Hearing | none (distance-only) | Linear / falloff | Distance check in `sensing.rs:300–305` | — |
| Scent | wind-vector + terrain modulation; does NOT obstacle-diffuse | Wind-strength-scaled range | `cat_smells_prey_windaware()` | No diffusion-around-barriers (§5.4: "scent drifts around fences") |
| Tremor | water-blocked via mask | Manhattan | `terrain.tremor_transmission()` multiplicative only | Explicit water mask absent |
| Pursuit proximity (threat maps) | Weighted shortest-path: water/wall impassable; dense forest / mud / garden / light forest slower; grass baseline | Cost-stamped flood-fill from a source cell | Per-terrain weights in `src/resources/map.rs::Terrain::movement_cost()` (Grass=1, LightForest/Mud/Garden=2, DenseForest=3, Water/Wall=u32::MAX). Single-source A* in `src/ai/pathfinding.rs:66+` uses those weights for point-to-point paths. | **Partial** — weight vocabulary and point-to-point A* both built; cost-stamped-to-all-cells flood-fill (the influence-map shape) is absent |

Shape distinction worth calling out: emitter-stamped maps (scent,
corruption, fox-scent) stamp *magnitude* from a source via a falloff
kernel. Pursuit-proximity maps stamp *traversal cost* from a source via a
weighted flood-fill. Both are influence maps; the computational shape
differs (convolution-style stamp vs. priority-queue expansion). Clowder
already has the weights and the per-path traversal; the absent piece is
the stamp-once-query-many substrate.

#### §5.6.5 Decay model per map

Per-map decay factors for every §5.6.3 row, plus two infrastructure
maps (Wind gusts, Territory markers) that aren't in the 13-map catalog
but share the decay parameter. Baseline commitments for L1 build; each
is a tunable knob, and per `CLAUDE.md` Balance Methodology, shipping a
non-baseline value requires a hypothesis + A/B observation.

| Map (§5.6.3 #) | Per-tick decay factor | Rationale | Current backing |
|---|---|---|---|
| Scent-proximity (#1) | `0.0` (full re-stamp) | Driven by current emitters + wind; no persistence between updates | On-demand model in `sensing.rs:525–563` — matches `0.0` shape |
| Corruption (#2) | `0.99` (slow persistence) | Corruption has momentum; decay pairs with `CorruptionPushback` emitters that subtract on positive colony events | `corruption_spread` at `magic.rs:41–91` has spread + pushback but **no per-tick decay today**; adding `0.99` is the L1-build delta |
| Ward coverage (#3) | `1.0` (indestructible until explicit decay) | Player-intentional infrastructure; ward siege decay is event-driven via shadow-fox encirclement, not per-tick | `ward_decay` at `magic.rs:100+` already event-drives per-ward decay; the map layer inherits `1.0` (no field-level decay) |
| Fox-scent (#4) | `0.90` (fast fade) | Moving emitter; older deposits must fade fast or stale hotspots form where foxes briefly passed days ago | `FoxScentMap::decay_all()` in `src/resources/fox_scent_map.rs` — verify today's multiplier against 0.90 when promoting to the shared substrate |
| Prey-location (#5) | `0.0` (re-stamp) | Prey move; every detection is a fresh read | Not implemented as a map today |
| Carcass-location (#6) | `0.95` (slow fade) | Carcasses persist for days but lose scent / draw rats; decay models natural consumption | Not implemented as a map today |
| Food-location / stores (#7) | `1.0` (entity lifecycle) | Static until consumed or destroyed; decay is event-driven on consumption | Not implemented as a map today |
| Herb-location (#8) | `1.0` (entity lifecycle) | Static until harvested; regrow re-stamps on seasonal cycle | Not implemented as a map today |
| Construction / damaged-building (#9) | `1.0` (entity lifecycle) | Entity lifecycle; decay event-driven on repair | Not implemented as a map today |
| Garden-location (#10) | `1.0` (entity lifecycle) | Entity lifecycle; decay on destruction only | Not implemented as a map today |
| Exploration state (#11) | `0.999` (near-permanent) | Near-permanent; slow fade models memory of "I knew this tile once" over sim-seasons. Key: `unexplored_fraction_nearby` should *rise* again slowly after long absence so Explore isn't permanently locked out of familiar territory | `ExplorationMap` at `src/resources/exploration_map.rs:8–12` has no decay today; adding `0.999` is an L1-build delta |
| Congregation / cat-density (#12) | `0.85` (fast fade) | Faster than scent; "where cats were 30 seconds ago" isn't social attraction — stale gatherings shouldn't pull future cats in | `CatPresenceMap::decay_all()` in `src/resources/cat_presence_map.rs` — verify against 0.85 |
| Kitten-urgency (#13) | `0.0` (re-stamp) | Kittens move and urgency is current-state; stale urgency is meaningless | Not implemented as a map today |
| Wind gusts (infrastructure, not in #1–#13) | `0.95` (slow persistence) | Wind direction has inertia; drift-toward-weather-target is already implicit in `wind.rs` `update_wind()` but isn't exposed as an influence-map decay | `wind.rs` `WindState` resource — today's target-drift arithmetic is equivalent to `0.95`-shaped persistence |
| Territory markers (infrastructure, future) | `1.0` (explicit lifecycle) | Stamped on place, removed on unclaim; no passive decay | Not yet built |

All values are L1 baseline commitments. Each non-identity decay needs
a per-tick-recompute pass in the substrate (§5.6.9 #5: decay is a
per-map parameter, not a per-map code path). The three `0.0` rows
(scent, prey, kitten-urgency) are free — the substrate re-stamps each
update anyway. The six `1.0` rows (wards, food, herbs, construction,
garden, territory markers) are also free — no per-tick recompute
unless an event fires.

#### §5.6.6 Attenuation pipeline

Per §5.2's formula:

```
perceived = base_map.sample(pos, channel)
          × agent.species.sensitivity(channel)
          × agent.role.modifier(channel)
          × (1.0 - agent.injury.deficit(channel))
          × world.environment.multiplier(channel)
```

The enumeration below fills in the value matrices for the four
multiplier layers: species (§5.6.6.1), role (§5.6.6.2), injury
(§5.6.6.3), environment (§5.6.6.4). The `base_map.sample()` layer is
the grid substrate itself and is covered by §5.6.3.

**Layer-level status (carried over from earlier draft):**

| Layer | Backing today | Status |
|---|---|---|
| `base_map.sample(pos, channel)` | On-demand computation in `sensing.rs::detect()`; no persistent grid (§5.6.3 #1–#13) | **Partial** — works functionally, needs grid substrate |
| `species.sensitivity(channel)` | `SensoryProfile` per `SensorySpecies` (10 species) at `sim_constants.rs:2605–2696`; see §5.6.6.1 | **Built** |
| `role.modifier(channel)` | `SensoryModifier` at `src/components/sensing.rs:96–120` with `.combine()`; no role-promotion logic populates it; see §5.6.6.2 | **Partial** — struct present, zero call sites |
| `injury.deficit(channel)` | no tie between `Health` / body-zone damage and perception; see §5.6.6.3 | **Absent** — `docs/systems/body-zones.md` describes intent; no code |
| `environment.multiplier(channel)` | `EnvCtx` wrapper with (weather × phase × terrain) composition in `sensing.rs:186–238`; all source fns return 1.0 (`weather.rs:94–111`, `time.rs:88–103`, `map.rs:198–201`); see §5.6.6.4 | **Partial** — structure present, values inert; canary test at `sensing.rs:929–983` asserts identity |

##### §5.6.6.1 Species × channel sensitivity (10 × 4)

Full matrix from `sim_constants.rs:2605–2696` (`SensoryConstants::default`).
Each cell is a `Channel { base_range, acuity, falloff }` struct today
(`sensing.rs:76–94`), not a single multiplier — the "sensitivity" the
§5.2 formula names is *derived* from the Channel at detect time via
`effective_range()` + `channel_confidence()` (`sensing.rs:253–319`).
Displayed format: `range / acuity / falloff`. `—` = channel `DISABLED`
(species has no sensitivity on that channel, e.g., hawks don't smell).

| Species | Sight | Hearing | Scent | Tremor | Scent directional? |
|---|---|---|---|---|---|
| Cat | 10.0 / 0.5 / Cliff | 8.0 / 0.5 / Cliff | 15.0 / 0.5 / Cliff | — | true |
| Fox | 8.0 / 0.5 / Cliff | 10.0 / 0.5 / Cliff | 12.0 / 0.5 / Cliff | 3.0 / 0.5 / Cliff | true |
| Hawk | 15.0 / 0.5 / Cliff | 5.0 / 0.5 / Cliff | — | — | false (irrelevant; no scent) |
| Snake | 1.0 / 0.5 / Cliff | 3.0 / 0.5 / Cliff | 8.0 / 0.5 / Cliff | 6.0 / 0.5 / Cliff | true |
| Shadow Fox | 8.0 / 0.5 / Cliff | 7.0 / 0.5 / Cliff | 10.0 / 0.5 / Cliff | 5.0 / 0.5 / Cliff | **false** (supernatural — ignores wind; `sim_constants.rs:2652`) |
| Mouse | 3.0 / 0.5 / Linear | 6.0 / 0.5 / Cliff | 5.0 / 0.5 / Cliff | 6.0 / 0.5 / Cliff | true |
| Rat | 5.0 / 0.5 / Linear | 7.0 / 0.5 / Cliff | 6.0 / 0.5 / Cliff | 7.0 / 0.5 / Cliff | true |
| Rabbit | 6.0 / 0.5 / Linear | 10.0 / 0.5 / Cliff | 4.0 / 0.5 / Cliff | 12.0 / 0.5 / Cliff | true |
| Fish | 3.0 / 0.5 / Linear | 5.0 / 0.5 / Cliff | 5.0 / 0.5 / Cliff | 6.0 / 0.5 / Cliff | false (water currents handled separately) |
| Bird | 10.0 / 0.5 / Linear | 5.0 / 0.5 / Cliff | 2.0 / 0.5 / Cliff | 2.0 / 0.5 / Cliff | true |

40 of 40 species × channel cells committed. Acuity uniformly 0.5 in
Phase 1; `Falloff::Cliff` is the default discipline and `Linear` is
reserved for the prey-detects-cat path (`sim_constants.rs:2655–2658`
comment). Prey species use Linear sight falloff so the legacy
`1 - dist/(alert_radius+1)` gradient survives.

**§5.6.9 #7–#8 tension (flag for L1 build).** Today's `SensoryProfile`
is a fixed 4-field struct (`sensing.rs:83–94`): `sight`, `hearing`,
`scent`, `tremor`. §5.6.9 #7 requires `SensoryChannel` to be an open
enum / newtype id, and §5.6.9 #8 requires species × channel to be a
matrix (or sparse map), not named fields per species. Adding a fifth
channel (e.g., a pheromone-mark channel distinct from scent) today
would edit this table *and* every `SensoryProfile` instantiation *and*
every `match` on the four names. Migrate the struct to
`channel_id → Channel` before the L1 substrate freezes — the
enumeration above is the value-shape this migration must preserve.

##### §5.6.6.2 Role × channel modifier (identity today)

`SensoryModifier` (`src/components/sensing.rs:96–120`) is a component
with 8 additive bonus fields (range + acuity per channel) and a
`.combine()` reducer. Today: **zero call sites write it** — no role
promotion, no injury penalty, no role-based variation. Full matrix
held at identity.

Rows below are every §4.3 marker that could plausibly carry a sensory
modifier at some future point (Coordinator, Healer, Hunter, Mentor,
Apprentice, Pregnant, Injured, Dead, plus life-stages Kitten/Adult/Elder
from `src/components/identity.rs:31–36`). Status column: **Absent**
for every row — not even wiring exists.

| Role marker | Sight mod | Hearing mod | Scent mod | Tremor mod | Intended shape | Status |
|---|---|---|---|---|---|---|
| Coordinator | 1.0 | 1.0 | 1.0 | 1.0 | would hear colony-wide farther if wired to range_bonus | Absent |
| Healer (future; not in §4.3 today) | 1.0 | 1.0 | 1.0 | 1.0 | would smell wound rot / herb potency more acutely | Absent |
| Hunter (capability; `can_hunt`) | 1.0 | 1.0 | 1.0 | 1.0 | would read prey scent sharper during hunt | Absent |
| Mentor (via `Training`) | 1.0 | 1.0 | 1.0 | 1.0 | no expected bonus; included for completeness | Absent |
| Apprentice (via `Training`) | 1.0 | 1.0 | 1.0 | 1.0 | possible *reduction* while learning, not bonus | Absent |
| Pregnant | 1.0 | 1.0 | 1.0 | 1.0 | possible scent acuity bump (pregnancy heightens smell) | Absent |
| Injured | 1.0 | 1.0 | 1.0 | 1.0 | modelled via §5.6.6.3 injury deficit, not role | Absent — cross-ref §5.6.6.3 |
| Dead | 0.0 | 0.0 | 0.0 | 0.0 | perception disabled; cross-ref existing `Dead` marker gating | Absent |
| Kitten (life-stage) | 1.0 | 1.0 | 1.0 | 1.0 | possible all-channel reduction (developing senses) | Absent |
| Adult (life-stage) | 1.0 | 1.0 | 1.0 | 1.0 | baseline; 1.0 is authoritative | Absent |
| Elder (life-stage) | 1.0 | 1.0 | 1.0 | 1.0 | possible sight/hearing reduction with age | Absent |

**Balance-hypothesis gate.** Filling any of these cells off 1.0 is a
balance-affecting change per `CLAUDE.md` Balance Methodology — do not
flip without an ecological claim + seed-42 A/B. This matrix is a menu
of *candidates* for future populated rows, not a commitment.

##### §5.6.6.3 Injury × channel deficit (13 × 4)

Body zones from `docs/systems/body-zones.md:14–36`. Doc names 13
parts; the table below enumerates which body zone feeds which sensory
channel so future body-zone work has a concrete attenuation target.
Cells: `✓` = zone feeds this channel (damage → deficit); `—` = no
expected relationship.

| Body zone | Sight | Hearing | Scent | Tremor | Intended deficit when Destroyed | Source (body-zones.md) |
|---|---|---|---|---|---|---|
| Whiskers | — | — | ✓ (close-range) | ✓ (substrate vibration) | Partial scent + tremor loss; body-zones.md:51 "lost spatial sense; can't hunt in low visibility" | :51 |
| Ears | — | ✓ (primary) | — | — | Deaf to distant threats; body-zones.md:52 "-20% threat detection range → deaf" | :52 |
| Mouth/Jaw | — | — | ✓ (Jacobson's / flehmen; weak secondary) | — | Weak scent reduction; primary impact is eat/bite (not sensory) | :53 |
| Scruff | — | — | — | — | No sensory channel | :54 |
| Throat | — | — | — | — | No sensory channel; critical for bleeding death | :55 |
| Flanks | — | — | — | — | No sensory channel; defensive armor | :56 |
| Belly | — | — | — | — | No sensory channel; defensive armor | :57 |
| Front Left Paw | — | — | — | ✓ (25%) | Substrate sensing; contributes 25% of tremor channel | :58 |
| Front Right Paw | — | — | — | ✓ (25%) | Substrate sensing; contributes 25% of tremor channel | :58 |
| Rear Left Paw | — | — | — | ✓ (25%) | Substrate sensing; contributes 25% of tremor channel | :58 |
| Rear Right Paw | — | — | — | ✓ (25%) | Substrate sensing; contributes 25% of tremor channel | :58 |
| Haunches | — | — | — | — | No sensory channel (movement loss only) | :59 |
| Tail | — | — | — | — | No sensory channel (balance + signalling only) | :60 |

**Status: every row Absent.** Neither `Health` (single struct at
`src/components/physical.rs:80–94`) nor any per-zone component exists
today. The body-zones.md design commits "functional consequences" but
not the deficit coefficients the §5.2 formula needs.

**Channel-feeder tally:**
- Sight: **0 zones feed it.** Body-zones.md has no **Eyes** entry —
  either folded into an unlisted "head" category or a doc gap. Surface
  to `docs/systems/body-zones.md` work; Sight-channel deficit has no
  zone in today's enumeration.
- Hearing: 1 zone (Ears).
- Scent: 2 zones (Whiskers close-range + Mouth/Jaw weak secondary).
- Tremor: 5 zones (Whiskers + 4 paws).

**Coefficient magnitudes TBD.** The enumeration commits *which* zones
feed *which* channels; the magnitude per zone (e.g., "destroyed ears =
1.0 hearing deficit vs. 0.5 deficit") lands with the body-zones build,
not here. A one-line commitment per zone is sufficient to unblock the
`injury.deficit(channel)` column — the substrate code can read `0.0
(healthy) → target_deficit (destroyed)` once body-zones writes per-zone
condition into components.

##### §5.6.6.4 Environment × channel multiplier (identity today)

Three tables, one per environment-axis source. Composition in
`sensing.rs:186–238`: `EnvCtx::from_environment(weather, phase, terrain)`
multiplies the three sources per channel. All source functions return
1.0 today — confirmed at:

- **Weather** — `src/resources/weather.rs:94–111` (`sight_multiplier`,
  `hearing_multiplier`, `scent_multiplier`, `tremor_multiplier` all
  return `1.0`).
- **Day phase** — `src/resources/time.rs:88–103` (same four multipliers,
  all return `1.0`).
- **Terrain** — `src/resources/map.rs:198–201`
  (`tremor_transmission()` returns `1.0`; sight / hearing / scent
  multipliers not yet defined on Terrain).

A canary test at `sensing.rs:929–983` asserts identity — flipping any
cell below off 1.0 will fail that canary until it's updated alongside.

**Weather × channel** (8 `Weather` variants from
`src/resources/weather.rs:12–21`):

| Weather | Sight | Hearing | Scent | Tremor | Intended shape (future) |
|---|---|---|---|---|---|
| Clear | 1.0 | 1.0 | 1.0 | 1.0 | identity baseline |
| Overcast | 1.0 | 1.0 | 1.0 | 1.0 | slight sight reduction (dim light) |
| LightRain | 1.0 | 1.0 | 1.0 | 1.0 | mild scent wash; mild tremor dampen |
| HeavyRain | 1.0 | 1.0 | 1.0 | 1.0 | strong scent wash; hearing dampened by rain noise |
| Snow | 1.0 | 1.0 | 1.0 | 1.0 | hearing reduction (muffled); scent reduction (cold) |
| Fog | 1.0 | 1.0 | 1.0 | 1.0 | **sight strongly reduced** (the primary fog behavior) |
| Wind | 1.0 | 1.0 | 1.0 | 1.0 | scent directional amplified (tail wind vs. head wind) |
| Storm | 1.0 | 1.0 | 1.0 | 1.0 | compounded Rain + Wind effects; tremor noise from thunder |

**Day phase × channel** (4 `DayPhase` variants from
`src/resources/time.rs:50–55`):

| Phase | Sight | Hearing | Scent | Tremor | Intended shape (future) |
|---|---|---|---|---|---|
| Dawn | 1.0 | 1.0 | 1.0 | 1.0 | sight ramping up; scent tracking peak (dew, still air) |
| Day | 1.0 | 1.0 | 1.0 | 1.0 | identity baseline |
| Dusk | 1.0 | 1.0 | 1.0 | 1.0 | sight reducing; hearing + scent compensate |
| Night | 1.0 | 1.0 | 1.0 | 1.0 | **sight strongly reduced** except for night-adapted species (cats) |

**Terrain × channel** (21 `Terrain` variants from
`src/resources/map.rs:9–35`). Grouped into 7 behavioral buckets to keep
the table tractable — per-variant cells can split when a balance
hypothesis surfaces a need:

| Terrain bucket (variants) | Sight | Hearing | Scent | Tremor | Intended shape (future) |
|---|---|---|---|---|---|
| Open (Grass, Sand) | 1.0 | 1.0 | 1.0 | 1.0 | identity baseline |
| Light vegetation (LightForest, Mud, Garden) | 1.0 | 1.0 | 1.0 | 1.0 | mild sight dampen; mild scent retention |
| Dense vegetation (DenseForest) | 1.0 | 1.0 | 1.0 | 1.0 | strong sight dampen; scent retention; hearing muffled |
| Rock / impassable solid (Rock, Wall) | 1.0 | 1.0 | 1.0 | 1.0 | sight blocked by Wall; Rock neutral |
| Water (Water, DeepPool) | 1.0 | 1.0 | 1.0 | 1.0 | **tremor blocked by water** (§5.4 spec commitment) |
| Settlement (Den, Hearth, Kitchen, Stores, Workshop, Watchtower, WardPost, Gate) | 1.0 | 1.0 | 1.0 | 1.0 | mostly identity; Den dampens scent (enclosed) |
| Special (FairyRing, StandingStone, AncientRuin) | 1.0 | 1.0 | 1.0 | 1.0 | mythic — scent/tremor may be non-identity for shadow-fox channel |

Total env × channel cells enumerated today: 8 weather × 4 +
4 phase × 4 + 7 terrain buckets × 4 = 76 cells, all 1.0. Populating
any cell requires a balance-hypothesis per `CLAUDE.md` Balance
Methodology — the "Intended shape" column is design intent, not a
commitment.

#### §5.6.7 ECS marker vocabulary

**See §4.3 for the full catalog.** This subsection was an 11-marker
pre-enumeration sketch during the first-pass draft; §4.3 now
enumerates the ~40 species / role / life-stage / state / capability /
inventory / target-existence / colony / spawn-immutable / fox-specific
markers with predicate, insert system, remove system, query pattern,
current-code status, and source field. §4.4 crosswalks the 27 + 9
`ScoringContext` / `FoxScoringContext` booleans against it; §4.5
carves out the 19 + 5 scalars that stay sampled. The extensibility
contract that keeps the marker set open to future additions lives in
§5.6.9.

#### §5.6.8 Status summary

| L1 piece | Built | Partial | Absent |
|---|---|---|---|
| Sensory channels | 2 (sight, hearing) | 2 (scent, tremor) | 0 |
| Base maps (§5.6.3) | 0 | 5 (#1 scent, #2 corruption, #4 fox-scent, #11 exploration, #12 congregation) | 8 (wards, prey, carcass, food, herb, construction, garden, kitten-urgency) |
| Propagation modes | 2 (sight LoS, hearing distance) | 3 (scent, tremor, pursuit-proximity) | 0 |
| Attenuation layers | 1 (species — §5.6.6.1) | 3 (base-map substrate, role §5.6.6.2, environment §5.6.6.4) | 1 (injury deficit — §5.6.6.3) |
| ECS markers | 7 (Species/PreyAnimal, Coordinator, Pregnant, Dead, FateAssigned, AspirationsInitialized) | 6 (Fox/Hawk/Snake/ShadowFox via `WildAnimal`, Mentor/Apprentice via `Training`) | 44 (LifeStage 4 + State 6 + Capability 4 + Inventory 7 + TargetExistence 10 + Colony 3 + Reproduction 2 + Fox-specific 8) |

#### §5.6.9 Extensibility constraints — L1 surface must stay open to L2 evolution

L2 design (future work) is *highly likely* to surface needs L1 didn't
anticipate — new channels, new maps, refactored propagation modes, new
markers, new attenuation layers. §5.6 is an inventory of what's needed
**as currently understood**; it is explicitly not a closed specification.
The L1 substrate must be structured so additions are registration-style
(add a row, plug in a strategy), not re-architecture.

Defensive-structuring requirements:

1. **Base-map storage is keyed by `(channel_id, faction_id)`, not named
   fields.** Adding a new map (new channel, new faction, or both)
   registers a new key; no hardcoded channel/faction set anywhere except
   the registry itself.
   - *Anti-goal:* `struct Maps { scent: Grid, corruption: Grid, fox_scent: Grid, ... }` — closed set, requires edits everywhere when a map is added.

2. **Propagation modes are pluggable strategies per channel.** Each
   channel registers its propagation function (LoS raycast, distance-only,
   wind-vector, weighted cost-stamp, etc.). Adding a channel with a novel
   propagation mode means adding one strategy, not branching existing code.
   - *Anti-goal:* a `match channel { Sight => raycast, Hearing => distance, … }` scattered across consumer code.

3. **Attenuation pipeline composes uniformly across channels.** The
   `sample × species.sens × role.mod × (1 - injury.deficit) × env.mul`
   formula runs for any channel. New channels get the pipeline by default,
   with 1.0 multipliers until sensitivity / modifier / deficit / environment
   tables are filled in.
   - *Anti-goal:* channel-specific attenuation code paths.

4. **ECS marker set is open.** The 11 listed markers (§5.6.7) are today's
   coverage. Adding a marker later is writing one insert/remove tick-system;
   consumer queries (`Query<With<MarkerX>>`) don't need refactor when new
   markers appear.

5. **Decay factor is a per-map parameter**, not a per-map code path. A new
   map registers its decay value via configuration; no `match` statement
   needs a new arm.

6. **Consideration-side query API is channel-agnostic.** Considerations
   sample `ctx.sample_map(channel, pos)`, not `ctx.fox_scent_at(pos)`. L2
   code decouples from the specific L1 channel set; a new map is consumed
   by passing a new `channel_id`, not by re-importing a new field.
   - *Anti-goal:* consideration code that names specific channels/maps
     statically.

7. **`SensoryChannel` is an open enum (or a newtype over an id), not a
   fixed 4-variant enum.** Today's `SensoryProfile` structure in
   `src/resources/sim_constants.rs:2614–2653` is shaped as a tuple of four
   named channel entries — that shape must be renegotiated into a
   `channel_id → channel_params` map before the substrate is built,
   otherwise adding a 5th channel later requires touching every species'
   profile definition.

8. **Species × channel sensitivity is a matrix (or sparse map), not a set
   of named fields per species.** Same principle as #7 applied to
   per-species sensitivity tables.

**What this means for §5.6 itself.** The tables throughout §5.6 enumerate
*current needs*, not *total possible needs*. §5.6.2's channels and §5.6.3's
base maps are "here are the channels / maps Clowder knows it needs today,"
not "here is the closed set." The extensibility constraints above are what
keep those tables safe to extend.

**What this means for L2 design.** When L2 surfaces a need for something
L1 doesn't yet provide, the response is: register it in the substrate
(new channel, new map, new marker, new attenuation column), not refactor
the substrate to accommodate it. If the response *requires* substrate
refactor, the constraints above have been violated — that's a bug in L1,
flagged back to this section.

#### §5.6.10 Pre-build checklist

A single checklist to run before any L1 build freezes. Each item is
derived from the tables above; the "meaning" column spells out what must
be answered in the build phase:

- [ ] **Known-channel coverage.** The 4 currently-identified channels
      (sight, hearing, scent, tremor) are registered with their propagation
      strategies and attenuation columns. *Not:* a claim that no further
      channels will ever be needed.
- [ ] **Extensibility-compliance review.** Walk §5.6.9's 8 requirements
      against the in-progress implementation before freeze. Any
      `match channel { … }` outside the channel-registry code, any
      `struct Maps { scent, corruption, … }` with fixed fields, any
      consideration code that names a specific channel — flag and refactor
      before freeze.
- [ ] **Base-map grid representation.** Choose grid data structure (flat
      `Vec<f32>` per map, chunked, sparse) and update cadence (every tick /
      every N ticks / event-driven). Storage keyed by `(channel_id,
      faction_id)`, not named fields (§5.6.9 #1).
- [ ] **Template precomputation policy.** Which decay shapes get
      precomputed LUTs, which evaluate live (see §2). Commit a per-map
      answer.
- [ ] **Scent generalization path.** How does `wind.rs + sensing.rs`'s
      on-demand model migrate to a persistent grid without regressing the
      wind-vector and terrain-modulation semantics? One map per
      scent-emitter-species or one aggregate?
- [ ] **Corruption map promotion.** How does `magic.rs`'s grid generalize
      into the shared substrate without losing its decay / spread
      semantics? Are the `corruption_spread` parameters preserved?
- [ ] **Attenuation pipeline wiring.** When do the role / injury /
      environment multipliers activate? Today they're identity; a
      balance-change hypothesis is needed before flipping any off 1.0
      (Balance Methodology in `CLAUDE.md`).
- [ ] **ECS marker tick-systems.** Each of the ~11 markers needs an
      insert/remove tick-system. Small, deterministic; enumerate so none
      are forgotten.
- [ ] **Per-map decay factor commit.** §5.6.5 provides target values; the
      substrate needs a per-map parameter (§5.6.9 #5).
- [ ] **Weighted pursuit-proximity map scope.** Does the initial L1 build
      include a cost-stamped-to-all-cells flood-fill substrate, or is
      on-demand A* sufficient? If included, §5.4's "Dijkstra" phrasing
      should be updated to "weighted shortest-path flood-fill" to match
      the cost model.
- [ ] **Body-Zone → channel deficit table.** `docs/systems/body-zones.md`
      names zones (whiskers, ears, nose, eyes, etc.); the L1 spec needs a
      concrete mapping (damaged nose → scent deficit coefficient, etc.)
      before injury attenuation can be built.

---

## §6 Target selection as inner optimization

Mark treats target-taking DSEs (attack X, socialize-with-Y, mate-
with-Z) as **double scoring**: iterate candidate targets, score each
one via a per-target DSE, select the best score as the action's
score. Ch 14 §"Which Dude to Kill?" is the worked example — combining
target-choice and weapon-choice into one decision so the interaction
("Bad Dude has a melee weakness") isn't missed.

Clowder's current pattern is the anti-pattern: `has_X_target: bool`
collapses "quality of best target" to "existence of any target." This
is the root cause of the iter-1 `social_target_range` regression
(widening the range admitted more strangers who ranked
indistinguishably from bonded partners, because the existence check
didn't see rank).

### §6.1 Anti-pattern inventory — worse than previously documented

Prior spec draft said "8 target-existence-collapse booleans." The
Phase-1 audit of today's `ScoringContext` (27-field struct at
`src/ai/scoring.rs:27–144`) found **13**, and graded them by severity:

**Critical (4)** — scoring is fully blind to best-target quality;
resolver does rich per-target ranking that scoring never saw:
- `has_social_target` — `Socialize` scores on need/personality only;
  resolver picks by `fondness × weight + (1 - familiarity) × novelty_weight`
- `has_eligible_mate` — `Mate` scores on need/warmth only; resolver
  picks by `romantic + fondness - distance × 0.05`
- `has_mentoring_target` — `Mentor` scores on
  warmth/diligence/ambition only; resolver picks by fondness + novelty,
  *ignoring* skill-gap magnitude (the whole point of mentorship)
- `has_social_target` (again, for `Groom` other-mode) — identical
  asymmetry to `Socialize`

Target-existence collapse is also an **apophenia failure** (§0.3). "The
cat chose the nearest social partner" isn't a readable story — the
observer has nothing to infer character from. "The cat chose her
bonded partner three rooms away over a nearby stranger" is a story,
because the choice reflects who she is. Forcing target-quality into
the scoring layer via `TargetTakingDse` (§6.3) is a legibility fix as
much as a correctness fix.

**Partial (4)** — scoring uses a count or scalar, resolver picks
nearest, losing quality signal:
- `prey_nearby` + `has_threat_nearby` — Hunt uses fixed `prey_bonus`;
  resolver picks `min_distance` prey regardless of yield
- `colony_injury_count` — ApplyRemedy scales by count capped at max;
  resolver picks nearest injured cat regardless of injury severity
- `has_construction_site` + `has_damaged_building` — Build uses fixed
  site/repair bonuses; resolver picks by build-progress or structural
  condition, quality invisible to scoring
- `hungry_kitten_urgency` (partial L2 already — scalar, not boolean) —
  Caretake uses max-of-kitten-urgencies, which is correct per Mark;
  resolver then navigates to *nearest Stores* rather than the kitten
  whose urgency drove the score

**Non-targeting (5)** — booleans that express inventory/site
availability, not targets. Collapse to ECS markers per §4, not §6:
- `food_available`, `can_hunt`, `can_forage`, `has_garden`,
  `has_functional_kitchen`, `has_raw_food_in_stores`, `has_herbs_nearby`,
  `has_herbs_in_inventory`, `has_remedy_herbs`, `has_ward_herbs`,
  `has_functional_kitchen` (appears in both severity buckets — one as
  target-ish, one as inventory, bad design smell on its own).

### §6.2 Silent divergence — GOAP vs. disposition resolver

The scoring-layer ignorance has a second-order cost: because the
resolver owns target quality, and the resolver code lives twice
(`disposition.rs:1329–1347` and `goap.rs:3788–3810`), they disagree.

- `disposition.rs:1329–1347` — social target chosen by `fondness ×
  fondness_social_weight + (1 - familiarity) × novelty_social_weight`
- `goap.rs:3788–3810` — social target chosen by `fondness` only;
  novelty weight ignored

Two plans for the same cat-action on the same tick can pick different
partners depending on which code path executes. **No single source of
truth for target quality** is the core problem. §6.3's `TargetTakingDse`
fixes this: one DSE owns the scoring, both execution paths consume
its result.

### §6.3 `TargetTakingDse` shape

Inner optimization per ch 13 §"Deciding on Dinner" and ch 14 §"Which
Dude to Kill?":

```rust
pub struct TargetTakingDse {
    pub id: DseId,
    pub candidate_query: fn(&World, Entity) -> Vec<Entity>,
    pub per_target_considerations: Vec<Box<dyn Consideration>>,
    pub composition: Composition,             // typically CompensatedProduct
    pub aggregation: TargetAggregation,
    pub intention: fn(Entity) -> Intention,   // see §L2.10
}

pub enum TargetAggregation {
    Best,                  // action score = max over candidates (default)
    SumTopN(usize),        // threat aggregation — sum top N scores
    WeightedAverage,       // rare; decaying contribution of ranked targets
}
```

The output is `(score, winning_target)` — the action's score is the
best candidate's score, and the winning target is carried forward to
the Intention that GOAP plans against (§L2.10). Both disposition.rs
and goap.rs consume the same DSE result; silent divergence (§6.2)
can't reappear.

### §6.4 Personal-interest template formalized

Mark's personal-interest template (IAM ch 30) — the falloff curve
centered on the evaluating agent's position — is a
`SpatialConsideration` with `(center = self.pos, curve = Quadratic or
Logistic)`. Every target-taking DSE in §6.5 declares one row below;
the same `SpatialConsideration` shape appears as the *distance*
consideration in each §6.5 sub-table, parameterized per this row.

Rows ordered by §6.1 severity class (Critical first, then Partial) so
the four scoring-blind DSEs (that today ignore target-quality
entirely) surface before the four scalar-only ones.

| # | Action | Backs ScoringContext (field:line) | Resolver today (file:line) | Max range (tiles) | Curve shape | Note (why this shape) |
|---|---|---|---|---|---|---|
| 1 | `Socialize` | `has_social_target:35` (bool) | `disposition.rs:1328–1347` + `goap.rs:3788–3810` | 8 | `Quadratic(exponent=2)` | Gentle convex falloff over colony range. §6.1 Critical: resolver today picks by `fondness × w + (1-familiarity) × w`; curve admits ranking over distance without nulling far partners. |
| 2 | `Mate` | `has_eligible_mate:111` (bool) | `disposition.rs:1873–1919` | 1 (adjacency) | `Logistic(steepness=20, midpoint=0.5)` | Near-step — mating is physically colocated. Partners/Mates bond is an ECS eligibility filter (§4), not a consideration. §6.1 Critical. |
| 3 | `Mentor` | `has_mentoring_target:93` (bool) | `disposition.rs:1352–1377` (sub-action of socializing chain) | 8 | `Quadratic(exponent=2)` | Matches `Socialize` reach — mentors find apprentices in the same colony cluster. §6.1 Critical: resolver today ignores skill-gap entirely; §6.5.3 installs it. |
| 4 | `Groom` (other) | `has_social_target:35` (bool, shared with `Socialize`) | `disposition.rs:1379–1385` (sub-action of socializing chain) | 1–2 | `Logistic(steepness=15, midpoint=1)` | Close physical range — allogrooming needs adjacency. §6.1 Critical: shared existence-bool with `Socialize`; sibling-DSE split under §L2.10 assigns distinct curves. |
| 5 | `Hunt` | `prey_nearby:95` (bool) + `hunt_prey_bonus` scalar (ScoringConstants, consumed at `scoring.rs:239`) | `disposition.rs:1172–1193` (chain skeleton); `HuntPrey` step handles target resolution internally via scent/sight | species-dependent (scent/sight range from §5.6.6.1 row `Cat`) | `Quadratic(exponent=2)` | Range bound is the cat's own sensory profile, not a fixed tile count — ties §6.4 to the sensory substrate. §6.1 Partial: resolver picks `min_distance` regardless of yield. |
| 6 | `Fight` | `has_threat_nearby:37` (bool) | `disposition.rs:1229–1283` (`build_guarding_chain` → `nearest_threat`) | 2–3 | `Logistic(steepness=10, midpoint=2)` | Steep threshold at engagement range — cats engage or flee, not linger at mid-range. §6.1 Partial: threat-count aggregates via `SumTopN` (§6.6), not `Best`. |
| 7 | `ApplyRemedy` | `colony_injury_count:72` (count) | `disposition.rs:1651–1662` (nearest injured, under `Herbcraft`/`PrepareRemedy` sub-mode) | 15 | `Quadratic(exponent=1.5)` | Long, gentle falloff — a healer walks across the colony for severe injury. §6.1 Partial: resolver picks nearest, ignores severity. Sibling-DSE under §L2.10 (child of `PracticeMagic` or `Herbcraft`). |
| 8 | `Build` | `has_construction_site:47` (bool) + `has_damaged_building:49` (bool) | `disposition.rs:1413–1437` (`build_building_chain` → priority=site first, tie-break by distance) | 20 | `Linear(slope=-1/20, intercept=1)` | Near-flat falloff — builders commit to sites across the colony, distance is a tiebreaker not a driver. §6.1 Partial: resolver picks by `(site_priority, min_distance)`; quality (progress, condition) invisible to scoring. |
| 9 | `Caretake` | `hungry_kitten_urgency:113` (scalar, already partial L2) | `disposition.rs:1925–1942` (chain picks nearest `Stores`; kitten resolved at `FeedKitten` step execution) | 12 | `Quadratic(exponent=1.5)` | Long reach for kittens — §6.1 Partial: max-of-kitten-urgencies is correct per Mark ch 13, but resolver then navigates to nearest Stores rather than the kitten whose urgency drove the score. L2 fix moves target commitment forward. |

**Sibling DSEs deferred to §L2.10.** Herbcraft's `gather` / `prepare` /
`ward` children and PracticeMagic's `scry` / `durable_ward` / `cleanse`
/ `colony_cleanse` / `harvest` / `commune` children will each own a
§6.4 row once §L2.10 lands the final sibling set (Enumeration Debt
line 95–98). `ApplyRemedy` is the one sibling already enumerated here
because it was surfaced by §6.1's inventory; the rest are blocked on
naming.

**Row-count invariant.** §6.4 and §6.5 must match row-for-row on the
target-taking DSE set — every §6.4 row has a §6.5.n sub-table, and
every §6.5.n sub-table has a §6.4 row. Drift between the two is an
enumeration-debt regression.

### §6.5 Per-target consideration sets

Each target-taking DSE from §6.4 declares a bundle of
per-target considerations below. Every consideration carries a
`(curve primitive, weight, signal source)` tuple — not just a label —
so the substrate can compose it via the §3.1.1 mode (default
`CompensatedProduct`; see §6.6 for non-default aggregations).

Columns:
- **Signal source** — the `Relationships`/`Health`/`Needs` field or
  resolver variable the consideration reads. Cite `file:line` where
  the signal is already computed; `—` where the signal is absent
  today and the L2 implementation surfaces it.
- **Curve** — primitive from §1 (Linear / Logistic / Quadratic /
  Cliff / Exponential) with parameter tuple.
- **Weight** — relative contribution within the per-DSE bundle.
  Weights sum to ~1.0 per bundle; composition normalizes. These are
  first-pass commits — balance iterations refine per the
  `CLAUDE.md` Balance Methodology.
- **Rationale** — why this curve/weight shape; flags Critical/Partial
  from §6.1 where the consideration closes a target-quality blind
  spot.

Sub-tables ordered identically to §6.4 (§6.1 severity: Critical 1–4,
then Partial 5–9). `distance (Spatial)` is the §6.4 personal-interest
template row reified as a bundle member; its curve parameters come
from §6.4, weight committed here.

#### §6.5.1 `Socialize`

| Consideration | Signal source | Curve | Weight | Rationale |
|---|---|---|---|---|
| distance (Spatial) | `self.pos` + target `Position` | Template §6.4 row #1 (`Quadratic(exponent=2)`, range=8) | 0.25 | Gentle falloff; far cats aren't zero-scored if bond is strong. |
| fondness | `Relationships::get(self, target).fondness` — `disposition.rs:1335` | `Linear(slope=1, intercept=0)` | 0.35 | Direct use — fondness *is* the affinity axis. Matches today's resolver `fondness_social_weight = 0.6` dominance. |
| novelty `(1 - familiarity)` | `Relationships::get(self, target).familiarity` — `disposition.rs:1337` | `Linear(slope=-1, intercept=1)` | 0.25 | Low-familiarity partners are interesting; high-familiarity are background. Matches `novelty_social_weight = 0.4`. |
| species-compat | `SensorySpecies` pair match — `—` (today implicit: social chain filters to cats only) | `Cliff(threshold=0.5)` | 0.15 | Edge-case capability for cross-species socializing under future visitors/trade; step for now, ranged later. |

#### §6.5.2 `Mate`

| Consideration | Signal source | Curve | Weight | Rationale |
|---|---|---|---|---|
| distance (Spatial) | `self.pos` + target `Position` — `disposition.rs:1900` | Template §6.4 row #2 (`Logistic(steepness=20, midpoint=0.5)`, range=1) | 0.15 | Near-step at adjacency — mating is physically colocated. |
| romantic | `Relationships::get(self, target).romantic` — `disposition.rs:1901` | `Linear(slope=1, intercept=0)` | 0.40 | Direct use; dominant axis per resolver's `romantic + fondness` sum. |
| fondness | `Relationships::get(self, target).fondness` — `disposition.rs:1901` | `Linear(slope=1, intercept=0)` | 0.25 | Second summand in resolver; breaks ties between equally-romantic partners by affection. |
| fertility-window | `target.Fertility { phase }` — §4.3 Reproduction block + §7.M.7 lifecycle; phase→scalar mapping per §7.M.7.5 (Tom-target fallback handles Toms who carry no marker). Absent pending the implementing PR. | `Logistic(steepness=10, midpoint=0.5)` | 0.20 | S-curve over cycle — proestrus ramps receptivity, diestrus suppresses, postpartum pins to 0 alongside anestrus. Closes a verisimilitude gap (today Mate fires any-time in Spring/Summer/Autumn per `mating.rs:102–114`). |

Partners|Mates bond is an ECS eligibility filter (§4), not a
consideration — resolver today filters at `disposition.rs:1893–1899`.

**Cross-ref: §7.M three-layer Mating architecture.** These per-target
considerations power the partner-selection scoring inside each of
§7.M's three layers at different cadences:
- **Layer 1 `ReproduceAspiration`** — runs this consideration set
  across *all reachable cats* when driving aspiration-emitted
  partner-seeking behavior; no bond-tier eligibility filter at this
  layer (the aspiration is what *grows* the bond).
- **Layer 2 `PairingActivity`** — runs against the *active partner
  only*; the consideration set acts as a belief-proxy `still_goal`
  check (romantic + fondness above retention threshold).
- **Layer 3 `MateWithGoal`** — inherits the Layer 2 partner; the
  set gates `achievable_believed` (partner in sensory range,
  fertility-window axis above threshold).

#### §6.5.3 `Mentor`

| Consideration | Signal source | Curve | Weight | Rationale |
|---|---|---|---|---|
| distance (Spatial) | `self.pos` + target `Position` | Template §6.4 row #3 (`Quadratic(exponent=2)`, range=8) | 0.20 | Matches `Socialize` — mentors find apprentices in the same colony cluster. |
| fondness | `Relationships::get(self, target).fondness` | `Linear(slope=1, intercept=0)` | 0.20 | Mentors gravitate to cats they like — realistic social dynamic. |
| skill-gap-magnitude | `self.skills[k] - target.skills[k]` per-skill, max over k — `disposition.rs:1361–1376` (computed as boolean today) | `Logistic(steepness=8, midpoint=0.4)` | 0.40 | **§6.1 Critical fix**: the whole point of mentorship. Gap-too-small (near peer) or gap-too-large (overwhelming) both suppress via S-curve's upper saturation; peak at moderate gap. Resolver today *ignores this entirely*. |
| apprentice-receptivity | `target.Apprentice` marker + `target.personality.ambition` — §4.3 Role block row `Apprentice` (Partial — component-derived from `skills.rs::Training { apprentice }`; insert system `aspirations.rs::update_training_markers` proposed) | `Linear(slope=1, intercept=0)` | 0.20 | Ambitious apprentices are receptive; the asymmetry (mentor's disposition × apprentice's receptivity) expresses mentorship as a two-sided transaction. |

#### §6.5.4 `Groom` (other)

| Consideration | Signal source | Curve | Weight | Rationale |
|---|---|---|---|---|
| distance (Spatial) | `self.pos` + target `Position` | Template §6.4 row #4 (`Logistic(steepness=15, midpoint=1)`, range=1–2) | 0.30 | Close physical range — allogrooming requires adjacency. |
| fondness | `Relationships::get(self, target).fondness` | `Linear(slope=1, intercept=0)` | 0.30 | Cats groom cats they like; matches warmth-threshold sub-action pick at `disposition.rs:1381`. |
| target-need-warmth | `target.needs.warmth` deficit (L1 `1 - warmth_level`) | `Quadratic(exponent=2)` | 0.30 | Convex: desperate-need amplifies outreach, mirrors `caretake`'s urgency axis. Requires `needs.warmth` split per Enumeration Debt line 108–117. |
| kinship | `Relationships::is_kin(self, target)` | `Cliff(threshold=kin=1.0, non-kin=0.5)` | 0.10 | Mild kin bias — mothers groom kittens, siblings groom siblings. |

#### §6.5.5 `Hunt`

| Consideration | Signal source | Curve | Weight | Rationale |
|---|---|---|---|---|
| distance (Spatial) | `self.pos` + prey `Position`, attenuated by §5.6.6.1 `Cat` sensory profile | Template §6.4 row #5 (`Quadratic(exponent=2)`, range=species-dependent) | 0.25 | Species-bounded — a cat hunts what it can sense, no further. |
| prey-species-yield | `PreyKind` calorie yield — `src/resources/prey.rs` yield constants | `Linear(slope=1, intercept=0, normalized)` | 0.25 | **§6.1 Partial fix**: larger prey (rabbit > mouse) preferred when equally accessible; resolver today picks `min_distance` regardless of yield. |
| prey-alertness | `Prey::alertness` (fear level) — `wildlife.rs` | `Linear(slope=-1, intercept=1)` | 0.20 | Unaware prey is easier — inverts the alertness axis. |
| pursuit-cost | Estimated chase path cost (plan-cost feedback per §L2.10.7) | `Logistic(steepness=10, midpoint=0.5, inverted)` | 0.30 | High-cost prey suppressed via S-curve cutoff. Blocks on §L2.10.7 (plan-cost feedback design). Until then, pursuit-cost proxies as `distance²`. |

#### §6.5.6 `Caretake`

| Consideration | Signal source | Curve | Weight | Rationale |
|---|---|---|---|---|
| distance (Spatial) | `self.pos` + kitten `Position` (via `KittenUrgencyMap` §5.6.3 row #13) | Template §6.4 row #9 (`Quadratic(exponent=1.5)`, range=12) | 0.20 | Long reach — parents cross the colony for a hungry kitten. |
| kitten-hunger | `target.needs.hunger` (kitten) — already surfaced as `hungry_kitten_urgency:88` scalar | `Quadratic(exponent=2)` | 0.40 | Convex amplification — near-starving kittens dominate. Mark ch 13 §"Deciding on Dinner" max-of-urgencies pattern is preserved per §6.6 `Best` aggregation. |
| kinship (parent of target) | `target.KittenDependency.mother == Some(self) \|\| target.KittenDependency.father == Some(self)` — §4.3 Reproduction block (kinship reads `KittenDependency` on the target directly; the self-side `Parent` marker is the query-optimization counterpart, not this consumer's source) | `Cliff(threshold=parent=1.0, non-parent=0.6)` | 0.25 | Parents preferentially feed their own kittens; non-parents still provision at lower weight (colony-raising pattern). |
| kitten-isolation | target has no sibling/parent within 3 tiles | `Linear(slope=1, intercept=0)` | 0.15 | Isolated kitten (wandered off, orphaned) gets priority — protects the edge case that motivates per-kitten targeting over per-store. |

#### §6.5.7 `ApplyRemedy`

| Consideration | Signal source | Curve | Weight | Rationale |
|---|---|---|---|---|
| distance (Spatial) | `self.pos` + patient `Position` — `disposition.rs:1653` | Template §6.4 row #7 (`Quadratic(exponent=1.5)`, range=15) | 0.15 | Healers cross the colony for severe injury. |
| injury-severity | `Health` deficit (today single-struct); future `BodyZone::Destroyed_count` when body-zones build lands | `Quadratic(exponent=2)` | 0.40 | **§6.1 Partial fix**: severe injuries triage higher. Resolver today picks nearest, ignores severity. |
| remedy-match | `Inventory::has_herb(remedy_kind_for_injury)` match — `disposition.rs:1628–1634` | `Cliff(threshold=match=1.0, mismatch=0.0)` | 0.30 | Hard gate — a HealingPoultice doesn't treat a mood-injury. Gating-via-Cliff (not eligibility filter) because remedy match is per-candidate, not a fixed species filter. |
| kinship | `Relationships::is_kin(self, target)` | `Linear(slope=0.5, intercept=0.5)` | 0.15 | Mild kin bias; healers treat the colony but kin gets a nudge. |

Under §L2.10, `ApplyRemedy` becomes a sibling DSE of `PracticeMagic`
or `Herbcraft`; this bundle migrates wholesale to the sibling spec.

#### §6.5.8 `Build`

| Consideration | Signal source | Curve | Weight | Rationale |
|---|---|---|---|---|
| distance (Spatial) | `self.pos` + site `Position` — `disposition.rs:1416,1420` | Template §6.4 row #8 (`Linear(slope=-1/20, intercept=1)`, range=20) | 0.20 | Near-flat falloff — site priority dominates distance. |
| site-type | `ConstructionSite` marker vs `Structure::needs_repair` — `disposition.rs:1418–1421` | `Cliff(threshold=ConstructionSite=1.0, RepairNeeded=0.6)` | 0.30 | Categorical: new builds > repairs by design intent (matches resolver's `priority = 0` for sites, `1` for repair). |
| progress-urgency | `ConstructionSite::progress_fraction` (close to complete = near-1) | `Quadratic(exponent=2)` | 0.30 | Convex: nearly-done sites pull builders to finish — "sunk-progress" effect. **§6.1 Partial fix** — invisible to scoring today. |
| structural-condition | `Structure::health_fraction` (low = needs urgent repair) | `Linear(slope=-1, intercept=1)` | 0.20 | Damaged structures surface linearly; pairs with site-type Cliff to span build and repair semantics. |

#### §6.5.9 `Fight`

| Consideration | Signal source | Curve | Weight | Rationale |
|---|---|---|---|---|
| distance (Spatial) | `self.pos` + threat `Position` — `disposition.rs:1241` | Template §6.4 row #6 (`Logistic(steepness=10, midpoint=2)`, range=2–3) | 0.25 | Steep at engagement range — engage or don't, no lingering. |
| threat-level | Wild-species combat rating × aggression — `src/resources/wildlife.rs` threat tables | `Quadratic(exponent=2)` | 0.30 | Convex amplification: a hawk is worth more attention than a snake at the same distance. |
| combat-advantage | `self.skills.combat + self.health_fraction − target.threat_level` | `Logistic(steepness=10, midpoint=0.5)` | 0.25 | S-curve tips past parity — cats don't half-commit. Shares steepness with `Flee`'s safety axis (§2.3 anchor). |
| ally-proximity | Count of ally cats within 4 tiles — bucketed via `CatPresenceMap` §5.6.3 row #12 | `Linear(slope=1, intercept=0, cap=3)` | 0.20 | Linear with cap: first 3 allies boost confidence linearly; more has diminishing returns. §3.1.1 names Fight's group-bonus as the WS motivator; this is its target-side reification. |

Fight also uses `SumTopN(3)` aggregation (§6.6) over threats, not
`Best` — the action score reflects *total* threat from the top-3
adversaries, so a surrounded cat engages even if no single threat is
maximal.

### §6.6 Aggregation choices

- **`Best` (default)** — action score = max-scoring target. Applies to
  all target-taking DSEs by default.
- **`SumTopN(n)`** — threat aggregation. Useful for `Fight`: total
  threat from N adversaries should drive the action, not max single
  threat. Ch 13 §"In the Game: Soldiers, Sailors, Airmen" marginal-
  utility pattern applies.
- **`WeightedAverage`** — rare. Considered for `Hunt` when multiple
  prey visible: instead of committing to "the best one," average
  top-3 scored. Probably worse than `Best` plus plan-cost awareness;
  queued as a design alternative, not a default.

### §6.7 Cross-refs

- Ch 13 §"Weighted Sums" + §"Deciding on Dinner" — target-taking as
  weighted sum over per-target considerations.
- Ch 14 §"Analyzing a Single Option" + §"Identifying Factors" +
  §"Which Dude to Kill?" — integrated target+weapon DSE, the most
  direct model for Clowder's per-target consideration bundles above.
- IAM ch 30 — Personal-Interest Template as the L1 shape of spatial
  considerations.

---

## §7 Decision persistence and momentum

Per-tick re-scoring from scratch causes "flipper" behavior near equal
scores and prevents any commitment supply chain (courtship, skill
progression, apprenticeship). Clowder's GOAP planner carries
intra-plan commitment, but at the scoring layer every tick is
independent.

This section synthesizes two reference sources to close the problem:
Rao & Georgeff (1991) — extracted at `docs/reference/bdi-rao-georgeff.md`
— supplies the **categorical commitment vocabulary** (which kind of
commitment is this Intention under?). Mark *Behavioral Mathematics*
ch 15 —
`docs/reference/behavioral-math-ch15-changing-decisions.md` — supplies
the **numeric momentum shape** (how heavy is the current commitment
when a challenger tries to preempt?). The two layers are orthogonal
and compose without contradiction: strategy decides *when* a drop is
permitted, persistence bonus decides *how hard* it is to be preempted
before the drop condition fires.

### §7.1 Commitment-strategy enum

The Rao & Georgeff vocabulary, committed verbatim:

```rust
pub enum CommitmentStrategy {
    /// Drop only on believed achievement (AI9a). AI8-capped.
    /// Zealot posture — use for critical-need resting + territorial
    /// guarding where mid-pursuit reconsideration is the failure mode.
    Blind,
    /// Drop on achievement OR believed unachievable (AI9b).
    /// Pragmatist posture — default for goal-shaped Intentions.
    /// Flipper-proof without being fanatical.
    SingleMinded,
    /// Drop on achievement OR no longer desired (AI9c).
    /// Desire-drift-sensitive — default for activity-shaped Intentions
    /// and for aspirations (§7.7), where the cat's own preference
    /// drift should be able to end the arc.
    OpenMinded,
}
```

The strategy tag lives on the `Intention` (per §L2.10.4), not on the
DSE. A DSE can emit Intentions with context-dependent strategies —
e.g., a `Patrol` DSE under critical-threat context could emit a
`Blind` Guarding Intention; under routine context, `SingleMinded`.

### §7.2 Drop-trigger reconsideration gate

Runs each tick **after** `check_anxiety_interrupts`
(`src/systems/disposition.rs:93`) — Maslow override stays supreme.
Three evaluations plus the AI8 cap; the strategy decides which
combination triggers the drop:

```
fn should_drop(intention, cat, ctx) -> bool {
    if intention.age >= intention.max_persistence_ticks { return true; }  // AI8

    let achieved     = achievement_believed(intention, cat, ctx);
    let unachievable = !achievable_believed(intention, cat, ctx);
    let dropped_goal = !still_goal(intention, cat, ctx);

    match intention.strategy {
        Blind        => achieved,
        SingleMinded => achieved || unachievable,
        OpenMinded   => achieved || dropped_goal,
    }
}
```

Signal definitions (each is a **belief proxy** per §12 — Clowder has
no formal beliefs):

- **`achievement_believed`** — goal-state predicate evaluated against
  current percepts. Percept ≈ belief for goal-achievement since the
  cat trusts what it senses right now. Examples: hunger level below
  threshold, carcass at stores, kitten's hunger sated, ward placed.
- **`achievable_believed`** — two-channel signal:
  1. DSE re-evaluated score above a retention threshold. Distance-
     to-target landmark contributes via the `SpatialConsideration`
     (§L2.10.7); as the target becomes inaccessible, the score
     attenuates smoothly. This is ch 14 §"Which Dude to Kill?"
     applied elastically per §0.2.
  2. `GoapPlan::replan_count < max_replans` — the existing
     (`src/components/goap_plan.rs:103`) hard-fail signal. If the
     planner cannot route a step chain after the capped number of
     retries, achievability is believed lost.
  Channel 1 is elastic (smooth degradation); channel 2 is the hard
  exit when elasticity isn't enough.
- **`still_goal`** — DSE re-score against current context. Below the
  retention threshold → the cat no longer goals this Intention. Only
  load-bearing under `OpenMinded`.
- **AI8 cap** — every Intention carries `max_persistence_ticks` at
  cat-scale (minutes of sim time, not ticks-per-second). Per Rao &
  Georgeff AI8, no Intention is held forever regardless of
  strategy.

### §7.3 Per-Intention-class strategy assignment

Maps against the 12 `DispositionKind` variants in
`src/components/disposition.rs:39–64`. Pattern: goals →
single-minded, activities → open-minded, critical needs → blind.
This is Rao & Georgeff's mixed-strategy example (page 14 —
"open-minded on ends, single-minded on means") applied at the
per-class level.

| Disposition   | Strategy     | Rationale |
|---------------|--------------|-----------|
| Resting       | Blind        | Physiological completion; Maslow gate handles preemption already. AI8 caps runaway sleeps. |
| Guarding      | Blind        | Territory defense shouldn't flinch mid-patrol. AI8 caps fixation. |
| Hunting       | SingleMinded | Drop when prey dead/fled or plan fails past replan cap. Flipper-proof without fanaticism. |
| Foraging      | SingleMinded | Drop when cache exhausted or trip target met. |
| Coordinating  | SingleMinded | Drop when directive queue drained or role removed. |
| Building      | SingleMinded | Drop when chain finishes or material source vanishes. |
| Farming       | SingleMinded | Drop when crop harvested or plot destroyed. |
| Crafting      | SingleMinded | Drop on recipe complete or ingredient unavailable. |
| Caretaking    | SingleMinded | Drop when kitten's need met or kitten dies. |
| Mating — L1 `ReproduceAspiration` | OpenMinded | Aspiration-layer default (§7.7). Grief, fate, mood drift, life-stage transitions should redirect a multi-year arc. See §7.M.1. |
| Mating — L2 `PairingActivity`     | OpenMinded | Activity-shaped (`UntilCondition`, §L2.10.5). Mirrors Socializing — partner invalidation, desire drift, season-out all drop naturally. See §7.M.1. |
| Mating — L3 `MateWithGoal`        | SingleMinded | Goal-shaped single event wrapping the existing 4-step chain. Drop on partner invalidation or `replan_count` cap. See §7.M.1. |
| Socializing   | OpenMinded   | Activity-shaped; drops on sated-sociability or lost interest. |
| Exploring     | OpenMinded   | Activity-shaped; curiosity drift drops it. |

Coordinator-issued directive Intentions (future, blocked on the
coordinator DSE design — see Enumeration Debt) default
`SingleMinded` with a coordinator-cancel override that functions as
an event-driven drop signal outside the normal gate.

**Aspiration-emitted Intentions inherit this table for their
short-horizon cadence** — if a "master hunter" aspiration emits a
Hunt Intention, that Intention is `SingleMinded` per-tick. The
aspiration itself is `OpenMinded` (§7.7). Two layers, two defaults,
no contradiction.

### §7.4 Persistence bonus (ch 15 Finish Him!)

Post-composition bonus applied to the currently-held Intention's
score during re-evaluation, sized by **task-completion fraction**
through a non-linear response curve. Prevents strobing via Mark's
bowling-ball-vs-billiard-ball metaphor: the challenger Intention
must beat `current_score + persistence_bonus` to preempt, not just
`current_score`.

```
persistence_bonus = base * logistic(completion_fraction, midpoint, steepness)
```

Uses the `Logistic` primitive from §2.1. Rationale from ch 15
§"Just Gimme a Minute, Boss!": a 99%-completed building should not
be abandoned for a slightly-better greenfield building, and the
increasing-marginal-utility shape of a logistic delivers that
behavior naturally. Ch 15 §"Finish Him!" applies the same logic to
low-HP targets: each remaining percent of damage is worth more than
the last.

**`base` is per-DispositionKind.** The table below commits one of
four categorical tiers (Low / Medium / High / Indefinite) per
variant, aligned with the existing `target_completions`
personality-scaling patterns in `src/components/disposition.rs:66–88`.
Specific numeric magnitudes are balance-thread work (candidate
bands: Low ≈ 0.05, Medium ≈ 0.10, High ≈ 0.20, Indefinite = N/A);
the tier commitment is what this enumeration closes.

| DispositionKind | Base tier     | Rationale |
|-----------------|---------------|-----------|
| Resting         | Indefinite    | Need-driven; `target_completions` returns `u32::MAX`. The Maslow gate ends Resting, not the persistence bonus — tier is moot. |
| Guarding        | High          | Commitment should hold through noise. `Blind` strategy + High tier prevents mid-patrol flinching; AI8 cap guarantees termination. |
| Hunting         | Medium        | Diligence-scaled `target_completions` (1 + `2·diligence`). Enough persistence to finish a chase; not so much that repeated prey-loss sticks past the `replan_count` hard-fail. |
| Foraging        | Medium        | Same shape as Hunting; diligence-scaled trip count. |
| Caretaking      | Medium        | Compassion-scaled (1 + `2·compassion`). A parent shouldn't abandon a hungry kitten for marginal score deltas. |
| Socializing     | Medium        | Sociability + playfulness scaled (1 + `2·sociability` + `playfulness`). Activity-shaped; `OpenMinded` strategy means desire drift still drops naturally, Medium persistence keeps conversations from strobing. |
| Exploring       | Medium-High   | Curiosity-scaled with the highest base (2 + `3·curiosity`). Exploration arcs are long; persistence should match — still `OpenMinded`, so curiosity drift can terminate. |
| Building        | High          | Chain-driven. The 99%-done-building case from ch 15 §"Just Gimme a Minute, Boss!" lives here — High tier prevents abandonment for marginal score deltas. |
| Farming         | High          | Same chain-driven Finish-Him logic as Building. |
| Crafting        | High          | Same reasoning; herbcraft and magic chains fragment under preemption otherwise. |
| Coordinating    | Medium        | Chain-driven but short; directive queues drain quickly. |
| Mating — L1 `ReproduceAspiration` | High | Aspiration-layer default for multi-year commitments. Event-driven drops (grief, injury, life-stage, §7.7.1 conflict), not per-tick. See §7.M.1. |
| Mating — L2 `PairingActivity`     | Medium | Activity mirror of Socializing (§7.4 row above) — `OpenMinded` strategy + Medium tier keeps the pair stable without fanaticism. See §7.M.1. |
| Mating — L3 `MateWithGoal`        | High | Finish-Him logic. Courtship chain near-completion shouldn't restart over marginal deltas; `SingleMinded` drops on partner invalidation. See §7.M.1. |

**Patience personality trait as tier multiplier.** Today, patience
adds flat trips to `target_completions`
(`disposition.rs:87`, `scoring.rs:695–704`). Post-refactor,
patience applies as a **per-cat multiplier on the tier-derived
`base` magnitude** — a patient cat's High tier is effectively
higher than an impatient cat's High tier. This subsumes the
existing patience-bonus code block; the flat-per-action bonus
deletes when §7.4 lands in implementation.

**`completion_fraction` is Intention-shape-specific:**
- Goal Intentions (most rows above): `1 - (remaining_cost / initial_cost)`.
  Cost here is GOAP path cost as estimated at Intention adoption.
- Activity Intentions (`Socializing`, `Exploring`): `elapsed_ticks / termination.ticks`.
- Chain-driven (`Building`, `Farming`, `Crafting`, `Coordinating`):
  `chain.steps_completed / chain.total_steps`.
- `Mating`: split per §7.M three-layer resolution.
  - `ReproduceAspiration` (L1): `elapsed_seasons /
    reproductive_window_seasons`. Aspiration-layer completion
    fraction rises over the cat's reproductive window, giving
    late-arc elders Finish-Him-scaled persistence on seeing the
    arc through.
  - `PairingActivity` (L2): `elapsed_ticks /
    pairing_termination.ticks`. Even though L2 is
    `OpenMinded`, the `UntilCondition` termination provides a
    meaningful completion fraction (time-in-activity relative to
    the partner-loss / season-close horizon).
  - `MateWithGoal` (L3): `chain.steps_completed / 4`. The 4
    chain steps (MoveTo → Socialize → GroomOther → MateWith,
    `disposition.rs:1873–1919`) remain the execution unit;
    Finish-Him applies in its canonical shape.

### §7.M Mating — canonical three-layer aspiration showcase

Mating is **the worked example** for this substrate's long-horizon
BDI architecture. Two framings drive the placement:

1. **Ecological load-bearing.** Without reproduction, the colony has
   no kittens, no generational continuity, no aspiration-inheritance
   narrative, and in the failure case no future at all. The
   iter-2 instrumentation session
   (`docs/balance/social-target-range.report.md`, commit `290a5d9`)
   caught the current `Mating` disposition gate-starved at **0% of
   snapshots** under a wider social-target-range treatment — 0
   matings and 0 kittens in the canonical deep-soak. The substrate
   has to express reproductive commitment in a shape that survives
   partner death, season-out, and aspiration conflict elastically,
   not as a brittle "do I have an eligible partner right now" gate.
2. **Multi-timescale BDI.** Reproduction naturally spans three
   timescales — the lifetime arc of wanting to raise offspring, the
   multi-season rhythm of courting a partner, and the single
   completed mating event. The substrate already names both layers
   (§7.7 aspirations, §L2.10.5 Goal/Activity Intentions, §L2.10.4
   strategy-on-Intention); mating is the first case that exercises
   three nested layers end-to-end. Rao & Georgeff AI4 explicitly
   allows `INTEND(INTEND(φ))` (`docs/reference/bdi-rao-georgeff.md`
   §3 CI4), and this is where it earns its keep in the design.

A Phase-1 audit of the current code surface (committed 2026-04-20)
named three tensions in today's `DispositionKind::Mating`: a hybrid
single-event-wrapping-a-4-step-chain shape
(`disposition.rs:1873–1919`), ambient pair-bond state that already
exists independently (`relationships.rs:14–17`, evolved by
`social.rs:100–175`), and no post-mating consequence graph. All
three dissolve under a three-layer nested-Intention design.

#### §7.M.1 Three-layer architecture

**Layer 1 — `ReproduceAspiration`** (lives in `aspirations.rs`
alongside Mastery / Territory arcs).

- **Scope.** Lifetime arc. A cat in its reproductive window adopts
  a `Reproduce` aspiration; the arc terminates at elder life-stage,
  on sustained injury below reproductive viability, on chosen
  celibacy (personality interaction), or on
  aspiration-compatibility conflict (§7.7.1).
- **Strategy.** `OpenMinded` — §7.7 default for aspirations. Grief,
  fate events, life-stage transitions, and mood drift *should* be
  able to redirect a multi-year arc. That's character (§0.4), not a
  bug.
- **Persistence tier.** **High.** Aspiration-layer multi-year
  commitments don't flip on marginal score deltas; they drop on
  event-driven reconsideration (§7.7.a life-stage, §7.7.b grief,
  §7.7.c fate, §7.7.d mood drift, §7.7.1 aspiration conflict).
- **Drop events.**
  - Life-stage transition into Elder (§7.7.a) — reproductive window
    closed; arc terminates cleanly.
  - Sustained injury below reproductive-viability threshold —
    body-zone integrity tracked by the pending Body Zones work;
    integrated here as a percept-backed drop condition.
  - Bereavement of a long-term `Mates`-tier partner (§7.7.b grief
    cascade) — personality-weighted redirect: seek new partner,
    grief-celibacy drop, or care-pivot into surviving offspring.
  - Hard-logical aspiration conflict per §7.7.1 (e.g., warrior-
    mastery arc consuming all attention) — pair list committed
    when §7.7.1 stabilizes.
- **Emits.** Per-tick short-horizon Intentions at Layers 2 and 3.
  When no Layer 2 bond exists, the aspiration biases partner-
  seeking behavior through `Socialize` / `Wander` target-selection
  weights (§6.5.1, §6.5.2) — the aspiration is *always active* for
  reproductive cats, even when no specific candidate is in range.

**Layer 2 — `PairingActivity`**: a new
`Intention::Activity(Pairing, UntilCondition(...))` (§L2.10.5).

- **Scope.** Multi-season, ambient once a `Partners+`-tier bond
  exists (`relationships.rs:14` — Friends → Partners → Mates).
  Biases proximity-to-partner, grooming-other, nest-sharing,
  shared-travel, co-hunting — the "courting couple" behavioral
  cluster as a sustained activity, not a discrete event.
- **Strategy.** `OpenMinded` (§L2.10.5 Activity correlation —
  `UntilCondition` pairs with desire-drift sensitivity). Partner
  invalidation, desire drift, and season-out-of-fertile all drop
  naturally without needing a hard gate.
- **Persistence tier.** **Medium** (§7.4 row — mirrors Socializing).
  Activity persistence keeps the pair from strobing between
  "courting" and "independent" every tick, without tipping over
  into the Building / Farming / Crafting "finish the chain" band.
- **Termination conditions** (Activity completion signals for
  §12.3 belief proxies):
  - Partner dies or leaves the colony → Layer 2 ends, Layer 1
    enters §7.7.b grief reconsideration.
  - Bond drops below Partners tier (relationship decay at
    `social.rs:60`'s `check_bonds` already maintains this).
  - Cat transitions out-of-season (seasonal fertile-window
    predicate, from `weather.rs` + life-stage).
  - Layer 1 aspiration drops (cascade) → Pairing idles but does
    not break the bond; an elder cat or a mastery-pivoted cat can
    still hold `Mates`-tier bond, just stops *working toward*
    reproduction.
- **Character expression (§0.4).** *Which shape* courtship takes
  says who each cat is. The Pairing activity doesn't prescribe
  behavior — it biases the DSE weights of existing actions the
  cat already scores:
  - Playful cat initiates play-bouts and chase-games with partner
    (Wander + Socialize weighting).
  - Diligent cat provisions partner (Hunt targeting partner's
    food-preference / Cook directed at shared Stores).
  - Bold cat defends partner territory (Patrol + Fight weighting
    scoped to partner's occupied tiles).
  - Affectionate cat allogrooms constantly (Groom-other weight
    multiplier).
  No new mechanics are needed for any of these — they fall out of
  Layer 2 as a weight modifier across the existing DSE set. This
  is §0.4's "mechanics must express character" filter passing
  cleanly: the same Pairing Intention produces four different
  observable arcs for four different personalities.

**Layer 3 — `MateWithGoal`**: a new
`Intention::Goal(mating_event_completed)` (§L2.10.5).

- **Scope.** A single completed mating event. Replaces today's
  `DispositionKind::Mating` + `build_mating_chain`
  (`disposition.rs:1873–1919`).
- **Strategy.** `SingleMinded` — the classic goal-shaped default.
  Drop on partner invalidation (moved out of range, died,
  re-partnered) or `GoapPlan::replan_count ≥ max_replans`
  (`goap_plan.rs:103`).
- **Persistence tier.** **High** — Finish-Him logic per §7.4. A
  courtship sequence near-completion (partner in adjacent tile,
  allogrooming already underway) shouldn't restart over marginal
  score deltas elsewhere. Patience multiplier from §7.4 footer
  applies.
- **Firing conditions** (evaluator gates Layer 3 as a candidate
  Intention only when all hold):
  - Layer 1 `ReproduceAspiration` is active.
  - Layer 2 `PairingActivity` is active with a specific partner
    (bond ≥ `Partners`).
  - Both cats satiated (hunger + energy above `SimConstants`
    thresholds; sourced from `needs.rs`).
  - Both cats inside seasonal fertile window.
  - Partner in §5.6.3 sensory range via the sensing pipeline.
- **Executes.** The existing 4-step chain
  (`MoveTo → Socialize → GroomOther → MateWith`) — **behavior
  preserved**, just re-parented into the §L2.10.4 Intention
  framework. GOAP plans from the goal state naturally per Jeff
  Orkin (§L2.10.9 cross-ref); the chain-driven shape of the
  execution unit remains chain-driven, only the commitment layer
  changes.
- **Completes.** Successful `MateWith` step → `pregnancy.rs`
  (existing system) hooks in → post-consequence cascade begins.

The three layers compose cleanly because Rao & Georgeff's
commitment vocabulary was designed for exactly this kind of
nesting: `INTEND(INTEND(φ))` is AI4 (commitment carries through
nested intentions) and CI1 (strong realism at each layer
independently). L1's horizon is years; L2's is seasons; L3's is
ticks. Same vocabulary, three orders of magnitude.

#### §7.M.2 Post-consequence cascade — elastic failure at three timescales

§0.2 elastic failure applies at each layer's timescale:

- **Successful mating** → `pregnancy.rs` system takes over for the
  pregnant cat (existing). Layer 1 aspiration doesn't drop — a
  cat can have multiple litters in its reproductive window;
  aspiration persists until the window closes or grief/conflict
  drops it. Post-partum, a `RaiseOffspringAspiration` (nested
  inside or adjacent to `ReproduceAspiration` — §7.7.1 conflict
  class TBD) emits Caretake Intentions; the partner's aspiration
  shifts toward a provisioner role via personality-weighted pick
  (diligent → Hunt-biased; compassionate → Caretake-biased).
- **Partner death** → Layer 2 Pairing's `still_goal` proxy drops
  immediately (the partner-reference invalidates); Layer 3 never
  fires (firing condition unsatisfied). Layer 1 receives §7.7.b
  grief event — reconsideration is personality-weighted:
  high-romantic-attachment cats more likely to drop into
  grief-celibacy; socially-resilient cats more likely to seek a
  new partner after a mourning interval; parental cats more
  likely to care-pivot into surviving offspring (if any).
- **Seasonal fertile-window close** → Layer 2 idles (Activity
  persists but `still_goal` evaluates low on the fertile-window
  axis); Layer 3 stops firing as a candidate. Layer 1 persists.
  When the window reopens, Layers 2 and 3 resume without Layer 1
  ever having been disturbed — the three-layer shape is what
  gives cats multi-season seasonal patience naturally.
- **Aspiration conflict** (§7.7.1) — e.g., a mastery-arc
  identity-pair — drops Layer 1 on the incompatibility event.
  Layer 2 cascades to idle but doesn't force a bond downgrade;
  the cat can still hold `Mates`-tier bond, just stops working
  *toward* reproduction. Re-prioritized cats who never had
  kittens with a long-term partner are a legitimate observable
  character arc; the architecture allows it.

Every failure mode above is elastic (§0.2): failure propagates
consequence at the appropriate timescale rather than terminating
the arc abruptly. A bereaved cat doesn't hard-stop
relationship-seeking; it drifts through a grief interval. A
seasonal cat doesn't forget its partner; it suspends active
courtship. A conflicted cat doesn't break its bond; it
de-prioritizes reproductive expression within it.

#### §7.M.3 Why this solves the gate-starved Mate observation

Iter-2 instrumentation (`docs/balance/social-target-range.report.md`):
Mate scored 0% of snapshots under the range=25 treatment because
`has_eligible_mate` hard-gated the DSE on binary partner presence.
When no `Partners+`-tier bond existed, Mate was simply not a
candidate, and the cat's scoring layer had no signal that
partner-seeking was relevant. Under the three-layer design:

- **Layer 1 is always active for reproductive cats.** The
  aspiration never hard-gates on partner presence — its
  expression when no partner exists *is* partner-seeking behavior
  (Socialize toward cats with high romantic-potential via
  §6.5.1's fondness + novelty weights, Wander biased toward
  under-explored social space). The aspiration's score never
  drops to 0 on bond-tier absence; only its concrete
  intention-emissions shift.
- **Layer 2 fires once bond ≥ Partners.** The aspiration's
  partner-seeking behavior drives bond progression (fondness /
  romantic drift at `social.rs:100–175`). The feedback loop
  closes naturally: aspiration → proximity → social event →
  bond advance → Layer 2 activation.
- **Layer 3's strict firing conditions are fine.** Mating is a
  rare ecological event by design — both cats fertile, both
  satiated, both in proximity. Low firing frequency is correct.
  The bug was never Layer 3's gating; it was the absence of
  Layers 1 and 2 to drive cats toward the conditions that make
  Layer 3 fireable.

**Ecological viability canary (per `CLAUDE.md` Balance
Methodology).** Hypothesis: under the three-layer design,
seed-42 `--duration 900` produces ≥3 matings and ≥2 surviving
kittens from a typical starter colony. Falsifiable via A/B soak
diff when implementation lands. This becomes the operational
version of the CLAUDE.md §Canaries "Generational continuity"
continuity-canary, measured against the concrete substrate
resolution committed here.

#### §7.M.4 Belief proxies at each layer (§12.3 grounding)

§12.3 names three belief proxies (`achievement_believed`,
`achievable_believed`, `still_goal`) as Clowder's substitutes for
Rao & Georgeff's formal Belief store. Each layer of the Mating
architecture grounds all three:

| Layer | `achievement_believed` | `achievable_believed` | `still_goal` |
|---|---|---|---|
| L1 `ReproduceAspiration` | Aspiration horizon exhausted (reproductive window closed; already-raised-offspring count crosses personal threshold) | Life-stage still reproductive; body-zone reproductive integrity intact | Personality-weighted grief / celibacy / conflict events drop the goal |
| L2 `PairingActivity` | Not applicable for Activity-shaped Intentions (no terminal goal state) | Partner still present; partner in §5.6.6-attenuated sensory range; bond ≥ Partners | Romantic + fondness toward partner above retention threshold; `With<Fertility>` AND `Fertility.phase ∉ {Anestrus, Postpartum}` per §7.M.7.6 (Tom-sided cats at L2 drop on `season == Winter` only) |
| L3 `MateWithGoal` | `MateWith` step completed successfully (pregnancy chance rolled) | `replan_count < max_replans`; target partner still exists; **at least one partner carries `Fertility` with `phase ∉ {Anestrus, Postpartum}`** (hard gate per §7.M.7.6); per-pair §6.5.2 fertility-window soft gate ≥ `l3_fertility_firing_threshold` | Inherits from L2 (OpenMinded outer layer drops cascade down L3's SingleMinded inner) |

> Fertility predicates above expand per §7.M.7; gender-to-role
> mapping is canon per §7.M.7.4 (Queens and Nonbinaries carry
> `Fertility`, Toms do not); the L3 hard gate is **asymmetric**
> ("at-least-one gestator in non-`{Anestrus, Postpartum}` phase")
> rather than symmetric.

No new belief-proxy mechanism is added — §12.3's catalog is
sufficient as designed. The three layers are inline use of the
existing vocabulary, which is the load-bearing proof that
§L2.10 / §7 / §12 compose as intended: the hardest case the
substrate will face (multi-timescale nested commitment) closes
without extension.

#### §7.M.5 Cascades into §7.3, §7.4, §L2.10.7

The "TBD" cells in §7.3 and §7.4 are resolved in those tables
directly — see rows for `ReproduceAspiration`, `PairingActivity`,
`MateWithGoal`. Summary:

- §7.3 strategy: OpenMinded / OpenMinded / SingleMinded.
- §7.4 persistence tier: High / Medium / High.
- §7.4 completion_fraction: `elapsed_seasons /
  reproductive_window_seasons` / `elapsed_ticks /
  pairing_termination.ticks` / `chain.steps_completed / 4`.
- §L2.10.7 Mate row: `SpatialConsideration` curve applies to
  Layer 3 MateWith's travel-to-partner step and Layer 2
  Pairing's proximity-bias term. Layer 1's partner-seeking uses
  the broader §6.5.2 `Mate` target-selection consideration set
  (romantic + fondness + distance + fertility-window) against
  all reachable cats, not a narrow distance-to-known-partner
  projection.

#### §7.M.6 The other relationship-embedded dispositions

The Phase-1 audit flagged that similar tensions may apply to
`Caretaking`, `Socializing`, and `Mentor target-taking`. The
three-layer architecture *doesn't* automatically apply to them,
and the short answer for each is:

- **Caretaking** — already Activity-shaped in §L2.10.5's
  classification (per §6.5.6's kitten-targeting). `Socializing`
  and `Mentor` don't need a Layer-1 aspiration unless mastery-
  stewardship or care-mastery becomes an explicit aspirational
  arc (see the §10 "Substances / Mental Breaks / Recreation"
  feature-design work and §7.7's mastery-aspiration framing).
  When that happens, the three-layer shape ports directly.
- **Socializing** — Activity at Layer 2; no Layer 3 needed. A
  cat can have friendship aspirations (seed for Talk-of-the-
  Town), but they're an optional future layer above the
  already-existing Activity.
- **Mentor** — Activity at Layer 2 (sustained teaching
  relationship); Layer 3 would be individual `TeachSkill`
  events, but today's mentor chain conflates Layer 2 and Layer
  3 into one disposition. Splitting is deferred to the
  aspiration-cataloging work (§7.7.1).

In none of these cases is the ecological argument for inline
resolution as strong as it is for Mating. Mating's
ecological load-bearing (§7.M framing 1) is what pulled it into
the substrate spec; the siblings stay in their stub docs until
their own feature-design pass demands a similar resolution.

#### §7.M.7 Fertility state specification

§4.3's Reproduction block committed the `Fertility { phase:
FertilityPhase }` marker shape (four-variant phase enum, mutually
exclusive with `With<Pregnant>`, event-driven insert/remove) and
deferred the **lifecycle** to this sub-section. §7.M.7 closes the
deferral: when the marker enters and leaves a cat's archetype; how
`phase` evolves while the marker is present; how `Gender` maps to
reproductive role; how §7.M.4's belief proxies and §6.5.2's
fertility-window consideration consume the state; and an expansion
of the phase enum to **five variants** — a dedicated `Postpartum`
variant is added alongside the original four `{Proestrus, Estrus,
Diestrus, Anestrus}`, scoring-equivalent to `Anestrus` but
narratively and log-query distinguishable.

##### §7.M.7.1 Lifecycle (insert, remove, recover)

| Event | Action | Author system | Phase on insert |
|---|---|---|---|
| Young → Adult life-stage transition (Queen / Nonbinary only) | Insert `Fertility` | `tick:growth.rs::update_life_stage_markers` (same system that maintains `Kitten/Young/Adult/Elder` per §4.3 LifeStage block; gender-gated) | Computed by §7.M.7.2 at transition tick |
| Young → Adult life-stage transition (Tom) | No-op | — | — |
| Adult → Elder life-stage transition | Remove `Fertility` if present | `tick:growth.rs::update_life_stage_markers` | — |
| `event:MateConceived` | Remove `Fertility` atomically from the gestating partner; `Pregnant` replaces | `pregnancy.rs` (conception path, updated to select gestation-capable partner per §7.M.7.4) | — |
| `event:KittenBorn` | Re-insert `Fertility` on the birthing mother | `tick:fertility.rs::handle_post_partum_reinsert` | `Postpartum` (dedicated phase; auto-transitions to normal cycle after `fertility_post_partum_recovery_ticks` elapse) |
| Cat dies | Entity despawn removes marker implicitly | — | — |

The Adult→Elder removal is new substrate behavior. Today's
`has_eligible_mate` (`mating.rs:102–114`) permits Elder cats to
mate; the refactor tightens this per §7.M.1 L1 ("Life-stage
transition into Elder — reproductive window closed; arc terminates
cleanly"). This is the ai-substrate framing principle in action:
the old gate is a first-pass artifact, not a constraint. Balance
Methodology applies — the drift from "Elders mate" to "Elders
don't mate" is part of §7.M.7.8's hypothesis.

##### §7.M.7.2 Phase transition function

Phase is a **pure function** of three inputs, evaluated per-cat at
`fertility_update_interval_ticks` cadence by `fertility.rs`:

```
phase(cat) = phase_from(
    cycle_tick:  (current_tick + cat.fertility.cycle_offset)
                 % fertility_cycle_length_ticks,
    season:      time.season(&config),
    post_partum: cat.fertility.post_partum_remaining_ticks,
)
```

Evaluation order (first match wins):

1. `season == Winter` → `Anestrus`.
2. `post_partum > 0` → `Postpartum` (and `post_partum -= interval`).
3. `cycle_tick < proestrus_end` → `Proestrus`.
4. `cycle_tick < estrus_end` → `Estrus`.
5. otherwise → `Diestrus`.

Where `proestrus_end = cycle_length × proestrus_fraction` and
`estrus_end = cycle_length × (proestrus_fraction +
estrus_fraction)`. The `Diestrus` remainder is
`cycle_length × (1 - proestrus_fraction - estrus_fraction)`.

`Gender` is absent from the input vector because this function only
runs for cats that already carry `Fertility` — the marker's
presence *is* the gender gate (§7.M.7.4: Queens + Nonbinaries have
the marker, Toms do not). The function is gender-agnostic at this
layer; the gender filter lives in the §7.M.7.1 Adult-entry insert
path, not in the transition function.

The function is **pure** (same inputs → same phase) and
**deterministic** (all inputs are either tick-derived,
season-derived, spawn-immutable, or event-stamped). Two soaks with
matching seed and matching `SimConstants` produce byte-identical
fertility traces.

`post_partum_remaining_ticks` is the only mutable per-cat state
beyond `cycle_offset`. It lives on the `Fertility` component
alongside the phase field; it's initialized to
`fertility_post_partum_recovery_ticks` on `KittenBorn` re-insert
and decremented every update tick until zero, after which it stays
at zero and rule 2 above falls through to rule 3 onward.

##### §7.M.7.3 Cycle parameters (new `SimConstants` tunables)

Proposed initial values for a new `FertilityConstants` block on
`SimConstants` (implementing PR may flatten to `fertility_*` fields
to match the existing flat-field convention):

| Field | Type | Default | Meaning |
|---|---|---|---|
| `cycle_length_ticks` | `u32` | `10_000` | Full cycle = half a season at `ticks_per_season = 20_000`. Each non-winter season sees ~2 full cycles per gestating cat. |
| `proestrus_fraction` | `f32` | `0.15` | 1,500 ticks of rising receptivity per cycle. |
| `estrus_fraction` | `f32` | `0.20` | 2,000 ticks of peak receptivity per cycle. |
| `diestrus_fraction` | `f32` | `0.65` | Implied: `1.0 - proestrus - estrus`. 6,500 ticks refractory per cycle. Validated at `SimConstants::validate()`, not a free-floating field. |
| `post_partum_recovery_ticks` | `u32` | `5_000` | Quarter-season forced `Postpartum` phase post-birth. |
| `update_interval_ticks` | `u32` | `100` | Phase refresh cadence — matches needs-update cadence. Coarse enough to skip most ticks; fine enough not to jitter scoring. |
| `cycle_offset_seed_mix` | `u64` | `0x9E37_79B9_7F4A_7C15` | Golden-ratio constant mixed into entity-id to derive per-cat `cycle_offset`. Deterministic, reproducible across seeds. |
| `l3_fertility_firing_threshold` | `f32` | `0.15` | Soft-gate cut below which L3 `MateWithGoal` is not enumerated as a candidate Intention. See §7.M.7.6. |

**`FertilityPhase` enum expansion — 4 → 5 variants.** The prior
§4.3 commitment listed four phases `{Proestrus, Estrus, Diestrus,
Anestrus}`. §7.M.7 adds a dedicated fifth `Postpartum` variant
(nursing interval post-birth). `Postpartum` scores identically to
`Anestrus` in §7.M.7.5's receptivity mapping (both `0.0`), but is
narratively and for log-query purposes distinguishable —
environmental (winter) vs biological (post-birth) suppression are
different phenomena, and narrative templates / event filters
should be able to treat them as such.

The existing `mating_fertility_{spring,summer,autumn,winter}`
multipliers (`ScoringConstants`) are **retained**. Winter's `0.0`
is now belt-and-braces with the `Anestrus` phase gate;
Spring/Summer/Autumn's `1.0/0.55/0.20` remain as **secondary
environmental modulation** on the §6.5.2 Logistic output. The
phase gate enforces biology; the season multiplier tunes ecology
(Autumn still produces fewer matings than Spring even when a
gestator is in Estrus on both).

##### §7.M.7.4 Reproductive roles (canon)

Clowder canon maps `Gender` to reproductive capacity via a small
fixed table. **No separate biological-sex axis is introduced** —
the absence is load-bearing for the simulation's identity.

| Gender | Gestates | Sires | Cycles, carries `Fertility` | Typical phase trajectory |
|---|---|---|---|---|
| `Queen` | ✓ | — | ✓ | Proestrus → Estrus → Diestrus, repeating; Winter → Anestrus; post-birth → Postpartum |
| `Tom` | — | ✓ | — | no cycle; no marker; implicit "always available in-season" receptivity via §7.M.7.5 fallback |
| `Nonbinary` | ✓ | ✓ | ✓ | Same as Queen; eligible to be chosen as the gestator *or* the sirer of any pairing (magical-realism wildcard) |

Interpretation:

- **Toms do not carry `Fertility` at any life-stage.** Their
  reproductive activity is always-on in non-winter seasons
  (mirroring felid biology — toms respond to partner estrus rather
  than cycling themselves). `Q<_, With<Fertility>>` excludes every
  Tom by construction.
- **Queens and Nonbinaries cycle identically** under §7.M.7.2. No
  NB-specific rule; the cycling system doesn't distinguish them.
- **`resolve_mate_with` needs an update.** Today's assignment
  (`commands.entity(cat_entity).insert(Pregnant::new(...))` at
  `disposition.rs:3053` and `goap.rs:1861`) lands `Pregnant` on
  whoever ran the Mate action — which can be a Tom, producing a
  pregnant Tom. The implementing PR for §7.M.7 picks the
  gestation-capable partner (Queen or NB) as the `Pregnant` target.
  For Queen×Queen / Queen×NB / NB×NB pairs, today's
  initiator-gestates behavior is preserved (the cat who ran the
  action gets `Pregnant`). For Tom-included pairs, the non-Tom
  partner gestates. Tom×Tom pairs fail §7.M.7.6's hard gate and
  never reach the Mate step, but the assignment logic must be
  robust anyway — defensive fallback returns no-pregnancy.

The role table sits inside `fertility.rs` and the updated
`resolve_mate_with` as a two-line helper:

```rust
fn can_gestate(gender: Gender) -> bool { gender != Gender::Tom }
fn can_sire(gender: Gender) -> bool    { gender != Gender::Queen }
```

Future-Gender-variant extensibility (e.g., magical alterations) is
a one-line edit in two places — no substrate refactor.

##### §7.M.7.5 Signal mapping for §6.5.2 `fertility-window`

The per-target fertility-window consideration (§6.5.2 `Mate` row)
reads the target cat's Fertility state and maps to a receptivity
scalar before the Logistic curve. Two cases:

```
fertility_scalar_for(target, season) =
    if target has Fertility {
        match target.Fertility.phase {
            Anestrus   => 0.0,
            Postpartum => 0.0,
            Diestrus   => 0.1,
            Proestrus  => 0.5,
            Estrus     => 1.0,
        }
    } else {
        // Tom target per §7.M.7.4 — no cycle, implicit always-on
        // in non-winter; Anestrus-equivalent in winter.
        if season == Winter { 0.0 } else { 1.0 }
    }
```

The scalar is multiplied by the environmental factor
`mating_fertility_{season}` before entering the §2.3 Logistic:

```
signal = fertility_scalar_for(target, season)
       × scoring.season_fertility(season)
curve  = Logistic(steepness=10, midpoint=0.5)
axis   = curve.evaluate(signal) × 0.20   // weight per §6.5.2
```

With the Logistic midpoint at `0.5` and steepness `10`, Diestrus
scores `~0.02` (near-zero contribution), Proestrus scores `~0.5`
(inflection), Estrus scores `~0.99` (near-full contribution), and
Anestrus + Postpartum both pin to `0.0`. A non-winter Tom target
scores `~0.99` — behaviorally equivalent to "always available."
The multiplicative season factor preserves today's tuning hooks
without double-gating.

Anestrus and Postpartum share the `0.0` scoring but differ
narratively and for log analysis — Anestrus is environmental
(season-driven, colony-wide), Postpartum is biological (per-cat,
birth-driven). Narrative emission and `events.jsonl` filtering can
distinguish them; scoring treats them identically.

##### §7.M.7.6 §7.M.4 belief-proxy wire-up (hard + soft gates)

The §7.M.4 table cells that reference "seasonal fertile window" are
replaced with concrete `Fertility.phase` predicates:

| Layer | Cell | Wire-up |
|---|---|---|
| L2 `PairingActivity` | `still_goal` (gestating partner) | Was: "cat still inside fertile window." Now: if `With<Fertility>`, then `Fertility.phase ∉ {Anestrus, Postpartum}`; if no Fertility (Tom-sided cat at L2), then `season != Winter`. Drops at Winter onset and for the post-partum interval; resumes without touching L1. |
| L3 `MateWithGoal` | firing-condition **hard gate** | At least one partner in the pair is gestation-capable (Queen or NB) with `Fertility.phase ∈ {Proestrus, Estrus, Diestrus}` (i.e., `∉ {Anestrus, Postpartum}`). Formally: `∃ p ∈ pair : With<Fertility>(p) ∧ p.Fertility.phase ∉ {Anestrus, Postpartum}`. Tom×Tom fails unconditionally. Winter fails because every gestator is Anestrus. Elder pairs fail because Elder removes Fertility per §7.M.7.1. Post-birth pairs fail until the mother's `Postpartum` interval elapses. |
| L3 `MateWithGoal` | firing-condition **soft gate** | Geometric mean of per-partner §7.M.7.5 scalars: `firing_strength = sqrt(fertility_scalar_for(a, season) × fertility_scalar_for(b, season))`. Tom partners contribute their Tom-fallback scalar. Below `l3_fertility_firing_threshold` (§7.M.7.3 default `0.15`), L3 is not enumerated as a candidate Intention; above, enters the DSE score pool normally. |
| L3 `MateWithGoal` | `still_goal` | Inherits from L2 — Anestrus / Postpartum onset on the gestating side cascades L2 drop cascades L3 drop per §L2.10.5 OpenMinded-outer/SingleMinded-inner layering. |

The hard+soft split matches Mark's behavior-mathematics framing:
hard-exclude the impossible, soft-weight the improbable. A pair
with every gestator in `{Anestrus, Postpartum}` is biologically
incapable of conception (hard); a pair with a Diestrus gestator
is capable but low-probability (soft). Cats paired with a Diestrus
gestator who score high enough on the other axes (romantic +
fondness + proximity) can still attempt mating at rare moments;
biology's probabilistic realism is expressed through the Logistic,
not through a binary gate.

**Asymmetric gate, symmetric query.** The hard gate reads as "at
least one" rather than "both" because only the gestator's cycle
state limits conception; the sirer's availability is modeled
separately by the soft gate (via their Tom-fallback or own-cycle
scalar). This keeps L3 firing viable for any pair with a Queen/NB
in at-least-Proestrus, regardless of the partner's gender.

##### §7.M.7.7 Authoring system

Resolves §4.6's `<TBD per §7.M.7>` placeholder.

- **`src/systems/fertility.rs`** *(new file)* → `Fertility`.
  - `tick:fertility.rs::update_fertility_phase` —
    `update_interval_ticks` cadence; iterates `Q<&mut Fertility,
    With<Adult>>` (Toms are excluded by construction per §7.M.7.4);
    applies §7.M.7.2 transition function.
  - `tick:fertility.rs::handle_post_partum_reinsert` — listens for
    `KittenBorn` events; re-inserts `Fertility` on the birthing
    mother with `phase = Postpartum` and
    `post_partum_remaining_ticks = fertility_post_partum_recovery_ticks`.
    Postpartum auto-transitions back to cycle-based rules once the
    counter expires (§7.M.7.2 rule 2 falls through).
  - `tick:fertility.rs::handle_conception_remove` — listens for
    `MateConceived` events; removes `Fertility` from the gestating
    partner (atomic with `Pregnant` insert by `pregnancy.rs`).
  - Adult-entry insert and Elder-exit remove live in `growth.rs`
    per §4.3 LifeStage-block authoring convention — `fertility.rs`
    reacts to life-stage transitions but doesn't author the
    markers. The Adult-insert is **gender-gated**: `growth.rs`
    inserts `Fertility` only for cats with `gender != Tom`; Toms
    skip this path entirely.

Rejected alternatives:

- **Co-locate in `needs.rs`** — wrong concern. Needs are
  decay/satiate; fertility is cyclical.
- **Co-locate in `pregnancy.rs`** — tangles two state machines.
  Pregnancy already owns a gestation-stage machine; adding cycle
  management thickens an already-complex file.
- **Compute per-consumer inline** — no savings. Per-cat variation
  (`cycle_offset`) has to live somewhere spawn-immutable anyway;
  at that point the component exists, just pretending not to.
  Narrative emission of phase transitions requires per-tick change
  detection which is a system regardless. DRY fails.

##### §7.M.7.8 Verisimilitude hypothesis

Per CLAUDE.md Balance Methodology, this substrate change ships with
a testable ecological claim for the landing PR.

- **Ecological claim.** Real felines exhibit estrous cyclicity —
  receptive phases (proestrus + estrus) occupy roughly 30–35% of
  cycle ticks; the non-receptive refractory (diestrus) dominates
  the remainder. Gestating cats (Queens + Nonbinaries in Clowder
  canon) are seasonally polyestrous with a winter anestrus.
  Forcing mating to cluster in gestator-receptive phases should
  produce more narratively legible courtship–mating arcs (cats
  visibly "come into season" rather than firing any-tick-in-Spring)
  without reducing generational continuity.
- **Predicted direction + magnitude** (seed-42 `--duration 900`
  release soak, A/B against today's `has_eligible_mate`):
  - **Mating events per soak: ↓ 30–55%.** Hard gate ("at least one
    gestator in non-Anestrus / non-Postpartum phase") opens for
    ~65% of non-winter cycle ticks per gestating cat — Proestrus +
    Estrus + Diestrus together. Soft-gate Logistic clustering
    shifts events into Proestrus/Estrus (cumulative ~35% of cycle
    ticks). Elder cats no longer eligible. Post-partum gestators
    absent for ~5,000 ticks each. Tom×Tom pairs — previously
    permitted by the symmetric today-gate and producing
    anatomy-blind pregnancies — now fail the hard gate cleanly.
  - **Kittens per soak: stable to ↓ 15%.** If `pregnancy.rs`'s
    per-mating conception roll is fixed-probability, kittens scale
    with matings. If conception is raised as follow-on to preserve
    the Generational-continuity canary, kittens stay flat — see
    acceptance below.
  - **Bond progression (fondness / romantic / Partners-tier
    count): stable to slightly ↑.** Bond evolution (`social.rs:
    100–175`) is season-agnostic today and stays so; the phase
    gate only fires at the mating-act scoring layer. Near-miss
    "courting" arcs during Diestrus accumulate fondness normally.
  - **Mating events clustered into season-week bands per
    gestator.** Measurable via `events.jsonl` `MateWith`
    timestamps — under today's flat gate, events are uniform in
    non-winter ticks; under the phase gate, events bunch into
    ~2000-tick Estrus windows ~2×/non-winter-season per gestating
    cat, desynchronized across the colony via per-cat
    `cycle_offset`.
  - **`Pregnant` never lands on a Tom.** The prior code's
    anatomy-blind assignment is corrected; Tom entities should
    show zero `Pregnant` insert events in logs. Implementing PR
    should add a debug assertion on the insert site.
- **Canary.** Generational continuity (≥1 surviving kitten per
  seed-42 `--duration 900`) **must hold**. If it fails, the tuning
  levers are (a) raise `pregnancy.rs` per-mating conception
  probability, or (b) reduce `cycle_length_ticks` so cycles repeat
  more often. The lever is *not* reverting the substrate — the
  substrate shape is load-bearing for the three-layer BDI and the
  §6.5.2 consideration curve.
- **Acceptance.** Direction matches prediction on all five
  metrics; magnitude within 2× on mating-events-per-soak;
  Generational-continuity canary passes; Starvation canary at 0;
  ShadowFoxAmbush canary ≤ 5. A/B headless soaks with matching
  `commit_hash` on both logs before/after.

##### §7.M.7.9 Code-change implications (for the landing PR)

This sub-section commits spec shape, not code. The implementing PR
(bundles with open-work cluster A, entry #5 — A1 IAUS refactor)
makes these concrete changes; listed here so the substrate
contract is unambiguous:

- **`src/systems/fertility.rs`** — new file; three tick systems
  per §7.M.7.7.
- **`src/steps/disposition/mate_with.rs::resolve_mate_with`** —
  partner-selection fix per §7.M.7.4: return `Some((gestator,
  litter_size))` where `gestator = can_gestate(a) ? a : b` with
  initiator-preference on ties. For Tom×Tom, return `None`.
- **`src/systems/growth.rs::update_life_stage_markers`** —
  gender-gated `Fertility` insert on Young→Adult (skip Toms);
  unconditional `Fertility` remove on Adult→Elder.
- **`MateConceived` / `KittenBorn` event vocabulary** — if not
  already emitted, added to wire up `fertility.rs`'s event
  listeners.
- **`src/resources/sim_constants.rs`** — new
  `FertilityConstants` block (or flat `fertility_*` fields to
  match existing style) per §7.M.7.3.
- **`src/ai/mating.rs::has_eligible_mate`** — hard-gate amendment
  per §7.M.7.6 (at-least-one gestator in non-Anestrus /
  non-Postpartum phase). This may dissolve entirely in the A1
  IAUS refactor; the predicate content moves into the DSE
  definition regardless.

The landing PR's `logs/events.jsonl` header will carry a new
`commit_hash` *and* a different `constants` blob; the reference
seed-42 soak must be re-baselined, and the verisimilitude
hypothesis above evaluated against the new baseline.

### §7.5 Maslow interrupt interaction

Critical-need signals are **event-driven preemptions** per Mark
ch 15 §"Event-Driven Recalculations" and §"Interrupting with an
Event." They bypass the §7.2 reconsideration gate and the §7.4
persistence bonus entirely; the replacement Intention installs
with `Blind` commitment so it cannot itself be preempted by
normal scoring until its achievement condition fires.

This matches the existing `check_anxiety_interrupts` pipeline
(`src/systems/disposition.rs:93`) — **no new path is added**;
§7.5 just formally documents the placement and the exhaustive
interrupt catalog.

**Interrupt catalog.** Sourced from
`src/systems/disposition.rs:180–253` (`InterruptReason` enum +
`check_interrupt`). Five interrupts; three are flat-thresholded,
one is personality-scaled, one is a computed-urgency signal.
Exemptions are tracked per-DispositionKind at the category level.

| Interrupt | Trigger | Replacement behavior | Exempt dispositions | Source |
|---|---|---|---|---|
| **CriticalHealth** | `health.current / health.max < critical_health_threshold` | Re-evaluate (anxiety-driven drop; no specific replacement Intention) | *None* — fires universally, including for Guarding. A cat below the health threshold must re-evaluate regardless of role. | `disposition.rs:202` |
| **Starvation** | `needs.hunger < starvation_interrupt_threshold` | Re-evaluate | Resting, Hunting, Foraging — these *are* the solution path. | `disposition.rs:212` |
| **Exhaustion** | `needs.energy < exhaustion_interrupt_threshold` | Re-evaluate | Resting, Hunting, Foraging (same reason as Starvation). | `disposition.rs:215` |
| **ThreatDetected** | Sighted wildlife passing `cat_sees_threat_at`; `threat_urgency = 1 - (manhattan_dist / threat_urgency_divisor)` exceeds personality-scaled `flee_threshold_base + boldness · flee_threshold_boldness_scale` | `Flee` toward threat position, `Blind`-committed at install | Guarding — guards handle threats directly via the guard-threat detection range. | `disposition.rs:226` |
| **CriticalSafety** | `needs.safety < critical_safety_threshold` | Re-evaluate | *None* — Guards are no longer exempt once safety is critical (recent change, see `disposition.rs:245` comment). | `disposition.rs:248` |

**Boldness as interrupt modulator.** Bold cats have a higher
`flee_threshold`, so threat detection must reach higher urgency
before interrupting. This is the one personality-scaled interrupt;
the other four are flat thresholds. Character-expressive future
work (§0.4) can hook additional personality modulators here — e.g.,
wounded-pride cats could lower their CriticalSafety threshold so
they panic-retreat earlier.

**Hunt-action carve-out.** Separate from the per-disposition
exemption matrix, `src/systems/disposition.rs:135–138` protects
**an active `Action::Hunt` step** from interruption even if the
disposition is not category-exempt. This is a per-Action carve-out
(not a per-Disposition one); semantically it prevents mid-pounce
abandonment. The pipeline order remains:

1. Maslow / anxiety interrupts (event-driven, bypass all gates) —
   **except** Hunt-in-progress carve-out (line 135).
2. §7.2 reconsideration gate (per-tick, strategy-dependent drop
   check).
3. §L2.10.6 softmax-over-Intentions for the challenger candidate.
4. §7.4 persistence bonus applied to the current Intention's score.
5. Compare challenger vs. `current + persistence_bonus`; preempt if
   strictly greater.

### §7.6 Monitoring cadence

Per Mark ch 15 §"A Hybrid Approach": minimum-granularity polling
plus event-driven interrupts for critical signals. Clowder's
existing per-tick scoring cadence is the minimum-granularity layer;
anxiety-interrupt events are the event-driven layer. Both are
already in place.

Mark's ch 15 benchmark of "human reaction time ≈ 240 ms / 12 frames"
translates to Clowder's tick rate comfortably — the per-tick polling
produces decisions within a Mark-appropriate responsiveness band
without needing a separate reaction-time throttle. The persistence
bonus + commitment strategy + retention threshold combination
handles strobing at the scoring layer; timer-based throttling is
not needed at the cat scale.

### §7.7 Aspiration-level commitment (separate layer from §7.1–§7.6)

Aspirations (`src/systems/aspirations.rs`) are **long-horizon
Intentions that emit short-horizon Intentions.** Rao & Georgeff
explicitly allow nested `INTEND(INTEND(φ))`; the same
commitment-strategy vocabulary applies at both layers, but with
different defaults and different reconsideration cadence.

**Default strategy at the aspiration layer: `OpenMinded`.** At
multi-season granularity, you *want* cats to reconsider. A cat
stuck blind-committed to "become master hunter" through the death
of its mate, the birth of its first kitten, and a prophetic vision
telling it to tend herbs is not persevering — it's broken. The
aspiration-level `OpenMinded` `still_goal` drop trigger is what
makes midlife crises a first-class substrate feature rather than
an absence of bug.

**Aspiration reconsideration is event-driven, not timer-driven**
(ch 15 §"Event-Driven Recalculations"). Per-tick re-evaluation of
multi-year arcs *is* the strobing failure mode Mark warns about —
just at a different timescale. The reconsideration gate runs only
on the classes of "bowling balls" (ch 15 §"Building Decision
Momentum") heavy enough to redirect a multi-year arc.

> **Spec correction (2026-04-21):** a Phase-1 audit of the emitter
> systems surfaced that several events named in the pre-correction
> list do not exist in code. Specifically: death.rs has no per-
> relationship grief events (generic-proximity + fated-bond-removal
> only); fate.rs has only FatedLove / FatedRival (no Calling,
> destiny modifier, or fated-pair-convergence as separate events);
> mood.rs has no drift-threshold detection; aspirations.rs has
> stagnation-abandon logic but no distinct plateau / achievement
> signal. The enumeration below splits each class into **currently
> emitted** (wirable today) and **aspiration emission debt** (what
> the aspirations subsystem or the underlying emitter must add
> before that class's reconsideration can fire).

**The five reconsideration event classes:**

##### §7.7.a Life-stage transitions

- **Currently emitted.** `LifeStage` enum
  (`src/components/identity.rs:31–36`) has four variants
  (Kitten, Young, Adult, Elder). Three time-based transitions fire
  today via age-check logic in `src/systems/growth.rs`:
  Kitten→Young, Young→Adult, Adult→Elder. All three are valid
  reconsideration triggers today.
- **Aspiration emission debt.** None. Aspirations just need a hook
  into the growth system's transition moment to observe these.
  Note: the pre-correction plan listed "kitten → adult, adult →
  elder" — the actual chain has three transitions (via Young),
  not two.

##### §7.7.b Grief cascade

- **Currently emitted.** `src/systems/death.rs:124–185` emits two
  distinct classes of grief event:
  - *Generic-proximity grief* — any cat within
    `grief_detection_range` of a death receives a mood penalty.
    Not relationship-classified.
  - *Fated-bond removal* — FatedLove / FatedRival component
    stripped on partner's death, with a Danger-tier narrative line
    ("The stars dim…" / "The challenge will never be answered…").
  No per-relationship-class grief exists (no distinct "mate grief"
  vs. "kitten grief" vs. "mentor grief" path).
- **Aspiration emission debt.** death.rs needs to emit
  relationship-classified grief events for aspirations to
  meaningfully filter. A combat-mastery aspiration should redirect
  on mate/kitten/mentor death specifically, not on any nearby
  death. Candidate shape:
  `CatDied { cause, deceased, survivors_by_relationship: HashMap<RelationshipKind, Vec<Entity>> }`.
  This work is Talk-of-the-Town-adjacent (requires formal
  relationship modeling beyond the current three-tier BondType)
  and is named as Enumeration Debt §7.7.b.

##### §7.7.c Prophetic visions (fate events)

- **Currently emitted.** `src/systems/fate.rs:21–200` implements
  two fate events — FatedLove assignment (stars-mark narrative at
  line 131) and FatedRival assignment (lock-eyes narrative at
  line 189). The pre-correction plan's "Calling," "destiny
  modifier," and "fated-pair convergence" are not separate
  emission surfaces in fate.rs today.
- **Aspiration emission debt.** fate.rs needs an expanded event
  vocabulary for aspiration-redirecting fate beyond mate/rival.
  The Calling is named in `docs/systems/project-vision.md` and
  `docs/systems/the_calling.md` (Aspirational per
  `docs/wiki/systems.md`); wiring aspirations to respond to
  Calling events requires Calling itself to emit events first.
  Cross-cutting debt — tracked here as §7.7.c and duplicated
  against `the_calling.md`'s own scope.

##### §7.7.d Sustained mood-valence drift

- **Currently emitted.** `src/systems/mood.rs:14–80` computes a
  per-tick mood valence with decay-based modifiers (wounded pride
  on low respect at line 59; contentment on physiological
  satisfaction at line 70). The valence is a continuous scalar —
  **there is no threshold-crossing or sustain-duration
  detection**. Valence doesn't cross hysteresis bands; there's no
  "drift sustained for N seasons" signal.
- **Aspiration emission debt.** mood.rs needs a drift-detection
  layer — e.g., "valence has been below X for N seasons AND the
  cat's active aspiration's expected-mood-reward points the
  opposite direction." This is the most design-heavy of the three
  debts and should be its own balance-thread once the aspirations
  catalog has per-arc valence targets. Named as §7.7.d.

##### §7.7.e Skill-mastery plateau or achievement

- **Currently emitted.** `src/systems/aspirations.rs:378–411`
  already has an auto-abandon path on **stagnation + low
  alignment** — effectively an `OpenMinded` `still_goal` drop
  under the §7.7 framework. Partial coverage; semantically the
  existing trigger is "you haven't made progress and you don't
  care," not a dedicated plateau or achievement signal.
- **Aspiration emission debt.** aspirations.rs needs distinct
  *milestone* (arc-specific checkpoint reached) and *ceiling*
  (skill-cap hit without further progression possible) signals.
  Both differ semantically from the existing stagnation-abandon
  and from the achievement-satisfies-aspiration normal drop.

##### Summary

Three of the five classes have non-trivial emission debt before
aspiration reconsideration can fully fire: per-relationship grief
(§7.7.b), fate-event vocabulary expansion (§7.7.c), and mood-drift
threshold detection (§7.7.d). Life-stage transitions (§7.7.a) and
aspiration-stagnation (§7.7.e partial) are wirable today. The
aspirations follow-on epic consumes the wirable classes first and
drives the emission debts in parallel.

**Ch 15 Information Expiry at the sub-Intention layer.** The
aspiration emits short-horizon Intentions biased by its projected-
payoff-distance. A `ConfidenceConsideration` inside the
aspiration-emitted DSE applies a `Logistic` or `Exponential`
confidence-decay curve over projected-duration — Mark ch 15 Figure
15.2 is the reference shape. *This is a DSE-layer mechanism, not a
§7 commitment-layer mechanism.* Flagged here because aspirations
consume it, but it lives inside `aspirations.rs` DSE construction,
not in the commitment gate.

**Crosswalk to §0 principles:**

- *§0.3 (apophenia, long-term-relevance leg):* midlife crises are
  peak apophenia fuel — the observer reads the arc. "She pursued
  combat mastery, then her kitten died, now she tends the herb
  garden." This is the substrate earning §0.3's long-term-relevance
  leg by exposing coherent multi-year reversals rather than
  flattening cats into interchangeable action loops.
- *§0.4 (mechanics express character):* aspiration reconsideration
  *is* character expression. A cat that redirects after grief is a
  different cat than one that persists; the sim tells a different
  story about them. Passes the §0.4 filter cleanly.
- *§0.2 (elastic failure):* grief-driven aspiration change is the
  failure-propagates-into-consequences shape §0.2 wants. The loss
  doesn't terminate the arc — it redirects it.

#### §7.7.1 Concurrent aspirations and conflict-check

A cat holds an `AspirationSet` of zero-to-N concurrent aspirations.
There is **no fixed N**; the bound is **mutual consistency**,
directly from Rao & Georgeff's goal-consistency requirement (CI1/CI2
— goals must be consistent; paper §3.2.2, see
`docs/reference/bdi-rao-georgeff.md` §1). A new aspiration enters
the set iff it is consistent with every aspiration already held.

**Four conflict classes** (declared at aspiration authoring time,
not runtime-inferred):

| Class | Example | Resolution |
|---|---|---|
| Hard-logical — mutually exclusive end-states | "Achieve territorial dominance" vs. "Become pacifist mentor" | Rejected at adoption: cat cannot hold both. |
| Hard-identity — incompatible life-paths | "Solitary wanderer" vs. "Colony coordinator" | Rejected at adoption. |
| Soft-resource — compete for cat-hours but don't contradict | "Master hunter" + "Master gardener" | **Allowed.** Competition resolved at the emitted-Intention layer via normal softmax. |
| Soft-emotional — tension but not contradiction | "Raise many kittens" + "Master combat" | **Allowed.** Tension is feature not bug (§0.4 — character expression); drops via normal §7.7 reconsideration events if sustained mood-drift fires. |

Hard-logical and hard-identity conflicts are declared in a sparse
compatibility matrix alongside the aspirations table in
`src/systems/aspirations.rs`. Matrix default is *compatible*; only
genuinely-contradictory pairs need listing. The matrix itself is
Enumeration Debt (blocked on the aspiration catalog stabilizing —
see Enumeration Debt section).

**Runtime conflict-check at adoption:**

```rust
fn can_adopt(existing: &AspirationSet, candidate: &Aspiration) -> bool {
    !existing.iter().any(|a| COMPATIBILITY.conflicts(a, candidate))
}
```

No per-tick re-checking. Consistency holds by construction; the
only way the set becomes inconsistent is via aspiration drift
(§7.7 reconsideration events), and drift removes aspirations —
never adds incompatibles.

**Interaction with midlife crisis.** Reconsideration events operate
on a **single aspiration at a time**, not the whole set. Grief
over a kitten's death may drop the combat-mastery aspiration while
leaving the herb-tending aspiration intact. The slot freed up can
be filled by a new aspiration at the next life-stage transition or
under a prophetic-vision event. One arc redirecting doesn't cascade
the cat's whole identity.

**Crosswalk to existing code.** `src/systems/aspirations.rs`
currently uses domain-affinity scoring for aspiration *selection*
(per-zodiac + personality). The compatibility matrix is a layer
above that — affinity decides which aspirations *want* to be
adopted; compatibility decides which *can* be. No scoring change
needed, just an adoption gate.

### §7.W Axis-capture and the warring self

A second load-bearing pattern that §7's commitment vocabulary enables,
parallel to §7.M's three-layer mating showcase. Where §7.M exercises
the layering of Intentions across timescales, §7.W exercises the
*semantic framing* of the Desire layer: Clowder permits the Desire
set to contain genuinely conflicting pulls, and resolves the conflict
at the Intention layer rather than the Desire layer. This unlocks
**axis-capture** as a unified primitive for a class of phenomena that
neither classical BDI nor classical IAUS names: compulsion, addiction,
cruelty, fated pull, devotion, mastery, self-destructive spite. All
of them are the same mechanism with different content.

#### §7.W.0 Motivation — what neither framework natively handles

Rao & Georgeff give us the commitment vocabulary (§7.1). Mark gives
us the numeric composition machinery (§1–§3). Neither names a
construct for *maligned* motives — desires that capture an agent's
fulfillment pipeline while degrading its flourishing. Classical BDI's
deliberation cycle includes a filter step that collapses Desires to a
consistent Goal set; this is the mechanism that keeps a rational
agent from simultaneously pursuing incompatible aims. IAUS, dually,
is utility-as-number: axis values compete, highest wins, nothing in
the architecture labels any axis as pathological.

But the clinical reality of compulsion isn't "one desire won the
filter." It's **the same action-execution loop that satisfies the
compulsion continues to raise its future pull, even as the agent's
overall situation degrades.** A sign-flip on the feedback term,
essentially — ordinary drives are negative-feedback (satiation),
compulsion is positive-feedback (sensitization). Neither BDI nor
IAUS has a principled place to locate this sign flip, because
neither has a register that is explicitly amoral and *retrospective*.

Clowder's answer is to add exactly that register — the **fulfillment
scalar** — and keep the Desire layer honestly inconsistent. The rest
of §7.W unpacks the consequences.

#### §7.W.1 Fulfillment as amoral retrospective scalar

Rao & Georgeff themselves note that Desires may be inconsistent —
"desires can be inconsistent with each other (wanting incompatible
things is normal); goals are the consistent subset of desires the
agent has chosen to actively pursue"
(`docs/reference/bdi-rao-georgeff.md:20`). Classical BDI enforces
consistency at the Desire→Goal filter. Clowder weakens the filter: a
cat's Desires may remain inconsistent, and the competition happens at
the scoring layer (per-tick DSE competition, §1.2) with final
consistency only at the Intention layer (one enacted Intention per
action horizon — an architectural necessity, since a cat can only do
one thing at once).

On top of that non-filtering Desire layer, Clowder adds a retrospective
register:

**Fulfillment** — a scalar, filled by any axis that successfully
executed in the last window, regardless of valence. Spite-enforcement,
kitten-grooming, Calling-trance, mastery-hunt, sensitization-capture,
social-bond reinforcement all contribute. The framework itself is
morally silent; the story of whether a cat is flourishing emerges from
*which* axes are filling the bar and at what cost to the cat's other
needs.

Three per-axis dynamics drive the pathology surface:

- **Decay modulated by source-diversity.** A cat whose fulfillment
  inflow comes from many axes decays slower than a cat whose inflow is
  narrow. Diversity-as-health is an emergent property, not a coded
  flag. Compulsion produces narrow-source cats; mastery arcs produce
  slightly-less-narrow-source cats (the mastery axis dominates but
  social axes still contribute); well-integrated cats have the widest
  spread.
- **Sensitization.** Per-axis property (not global). An axis with
  sensitization enabled has its weight *grow* with successful use —
  the IAUS sign flip made explicit. Corruption-tainted axes sensitize;
  ordinary axes don't. This is the knob that produces runaway
  axis-capture; without it, any axis eventually saturates and the cat
  moves on.
- **Tolerance.** Per-unit fulfillment yield drops with repetition.
  Partner to sensitization — together they produce the "needs more to
  feel the same" signature. Cheap to implement, large narrative payoff.

Specific curve shapes, coefficients, and the ordering between
sensitization and tolerance are numeric-tuning balance-thread work
(doc's line 24–29 scope discipline), not substrate spec.

#### §7.W.2 Warring-self scoring — losing axes don't vanish

The critical consequence of weakening the Desire→Goal filter: when
one axis wins the tick, the losing axes *stay active*. Their pull
persists; their fulfillment deficit accumulates. The cat hasn't
decided that the losing desire doesn't matter — they just didn't act
on it this tick.

This lands in the existing mood cascade (`src/systems/mood.rs`)
without architectural change: unrequited fulfillment-deficit on a
still-active axis feeds the valence-drop pathway, which presents as
*tension* or *ambivalence*. A cat with a captured axis winning
repeatedly AND an active counter-axis starving is legibly torn — the
architecture produces the signal, the narrative emitter reads it.

The compulsion signature becomes mechanically visible: **narrow
winning axis + active losing counter-axis + mood valence drop**.
That's what distinguishes pathological capture from a cat who just
likes doing one thing a lot. The hobbyist isn't starving a
counter-axis; the addict is.

#### §7.W.3 Second-order preferences collapse into first-order conflict

Frankfurt-style second-order preferences ("I want to not want this")
are a rich topic in philosophical accounts of addiction and agency.
The natural question for a BDI-derived substrate is whether Clowder
needs to represent preferences-over-preferences as a distinct
mechanism.

It does not. **The active-but-losing axis is the second-order
preference.** The addict who wishes they didn't want heroin is
mechanically a cat with an active anti-heroin axis that keeps losing
to the pro-heroin axis. The architecture already contains the
structural element the philosophical framing demands; no meta-
cognition primitive is required.

This is an explicit non-goal: Clowder does not implement
preference-over-preferences as a separate store. The simpler
architecture carries the same narrative payload. Cats don't need to
*know* they're conflicted for the world to *show* them as conflicted —
the mood cascade carries the signal, and the narrative emitter
(§7.W.6 telemetry) can surface the winning/losing-axis pair directly.

#### §7.W.4 Worked examples

Two concrete instances of the axis-capture primitive, demonstrating
that the mechanism is general across valence.

##### §7.W.4(a) The Calling — externally-seeded, bivalent, time-limited

`docs/systems/the-calling.md` already specifies a rare creative
trance triggered by magic affinity + elevated mood + spirituality.
Re-read through the axis-capture vocabulary, the Calling *is* the
canonical existing instance of the primitive:

| Calling property | Axis-capture vocabulary |
|---|---|
| Trigger conditions (affinity + mood + spirit threshold) | Externally-seeded axis activation |
| "Refuses all interaction" during trance | Captured axis wins every tick; other axes active-but-losing |
| Specific herb requirements within timeout | `Blind` commitment on means (see §7.1) |
| 40–60 tick creation phase | Bounded capture window (not indefinite) |
| Success → Named Object; failure → corruption spike | Bivalent resolution (positive / pathological outcomes share one mechanism) |
| "Touched" identity post-success | Persistent identity modifier as capture residue |
| `Shaken` 2000-tick cooldown post-failure | Recovery window from a pathological resolution |

The Calling is not a separate Phase-6 mechanic waiting for its own
architecture; it is the first-implemented instance of a general
primitive. Seeing it this way changes the specification posture — new
axis-captures (see §7.W.5 below) don't need new systems, they need
new *content* plugged into the same mechanism.

##### §7.W.4(b) The warmth split — healthy social axis-capture

The `needs.warmth` axis currently in `src/systems/needs.rs` conflates
two distinct phenomena: physiological body-heat (drained by weather
and season, restored by hearth and den) and affective closeness
(restored by grooming other cats, `src/steps/disposition/groom_other.rs:47`).
The conflation means a cat near a hearth is immune to loneliness at
the needs level — hearth-warmth and social-warmth fill the same bar.

Under the axis-capture framing, the split is load-bearing: the
warring-self dynamic of §7.W.2 requires that a cat be able to be
physically warm and socially starving at the same time. Otherwise
the losing-axis signal the narrative layer depends on is drowned out
by the first shelter the cat finds.

The split, specified in full in `docs/systems/warmth-split.md`:

- `needs.temperature` — physiological, stays in Maslow L1 (§3.4),
  drained by weather/season, restored by hearth/den/sleep/self-groom.
- `social_warmth` — fulfillment-layer axis (§7.W.1), drained by
  isolation/bond-loss, restored by grooming-others, huddling,
  mating-partner proximity.

This is the healthy-capture case, and it's the one that demonstrates
the fulfillment primitive is not only for pathology. Ordinary social
bonds are *also* axis-captures — a cat whose fulfillment comes
predominantly from kin-grooming is doing exactly what the mechanism
names, and it produces a flourishing life, not a compulsive one.
The difference between the narc-cat and the devoted-mother is the
*content* of the axis and the *breadth* of other contributing axes,
not the mechanism.

#### §7.W.5 Free consequences of the unification

Four design affordances fall out of treating Calling / warmth /
compulsion / mastery as the same primitive with different content.
None require new systems; each is enumerated here so downstream work
can draw on them without re-litigating.

- **Dark Callings.** A Calling-shape gate with corruption-tainted
  trigger conditions produces a compulsion to create something
  destructive — a Named Curse, a shadow-pact object, a shadowfox
  lure. Same trance mechanics, inverse valence. The existing corruption
  spike failure mode (`the-calling.md:44–48`) is already the precursor
  shape. Implementing dark Callings is Phase-6+ content; flagging it
  here so the mechanism owners know the capacity exists.
- **Persistent identity modifiers generalize beyond "Touched."**
  Axis-capture residue accumulates on a cat's life trajectory.
  Successful Calling produces "Touched"; narrow pathological capture
  might produce "Hollow," "Marked," "Bitter"; sustained mastery
  capture produces "Devoted" or a domain-specific title. These are
  narrative-layer identity bits — no component change needed beyond
  the existing `Touched` slot generalizing to an enum or tag-set.
- **Sadist-play is not a new system.** A cat whose play axis has
  sensitized around prey-distress as the fulfillment source is just
  an axis-capture with particular content. No "sadism" system, no
  "cruelty" trait — the mechanism produces the archetype from
  sensitization + blind-on-means + narrow-source decay. The colony's
  reaction (fear, banishment, shunning, attempts at redirection) is
  the story the narrative system emits.
- **Every capture is legible.** Because the losing-axis signal feeds
  mood and telemetry (§7.W.6), the warring-self state is observable
  to the narrative emitter. "Whisker-mother trembled at the workshop
  doorway; her kitten cried in the den, but her paws moved of their
  own accord" is free output from the mechanism — the capturing axis
  and the starving counter-axis both have addresses.

#### §7.W.6 Telemetry — losing-axis observation

`CatSnapshot.last_scores` was extended in commit `290a5d9` to log
all gate-open action scores per tick, not just the winner. §7.W
relies on this: the narrative emitter reading only the winning
disposition per tick cannot produce warring-self lines, because the
counter-axis that makes the capture legible is the *losing* one.

The instrumentation work of §11 therefore needs to preserve top-N
losing-axis scores (suggested N=3) across snapshots, with a fixed
schema column so narrative templates can bind to "axis X winning
while axis Y losing above deficit-threshold." Not a new telemetry
stream — an extension of the existing one. Exact N and
deficit-threshold values are §11 instrumentation tuning.

#### §7.W.7 Non-goals

The following are deliberately *not* in §7.W's scope, and adding them
would constitute architectural drift:

- **Meta-cognition / preference-over-preferences as a separate store.**
  Resolved by §7.W.3 (collapse into first-order conflict).
- **Moral-valence labels on axes in the sim.** No "this axis is bad"
  flag. The framework is morally silent (§7.W.1); story emerges from
  content and colony reaction.
- **Fulfillment as an override of survival needs.** Maslow pre-gate
  (§3.4) remains supreme. Starving cats interrupt spite, interrupt
  Calling trances at critical-need thresholds, interrupt mastery
  pursuit for water. The `check_anxiety_interrupts` pipeline
  (`src/systems/disposition.rs:93`) is unchanged. Fulfillment sits
  *above* Maslow in priority order, not instead of it.
- **Active avoidance of captured axes as a first-class mechanism.**
  The cat doesn't "know" they're captured in any mechanical sense.
  Avoidance emerges when the mood cascade's valence drop crosses an
  interrupt threshold (§7.5 Maslow interrupt interaction), or when
  another high-score axis wins the scoring competition and displaces
  the capture. No "resist captured axis" primitive.

#### §7.W.8 Cross-refs

- §3.4 (Maslow pre-gate) — fulfillment sits above it; §7.W does not
  weaken Maslow override.
- §7.1–§7.3 — commitment strategies apply per-captured-axis.
  Compulsion signature = narrow winning axis + `Blind` commitment on
  means. Sadism-shape = `SingleMinded` on means, `OpenMinded` on
  ends (Rao & Georgeff mixed-strategy licensing,
  `docs/reference/bdi-rao-georgeff.md:88`).
- §7.7 aspirations — a long-horizon aspiration can *be* a captured
  axis over lifetime (healthy mastery) or become one by way of
  sensitization (pathological). The aspiration layer is orthogonal
  to capture valence.
- §7.M mating — ordinary reproductive commitment is an axis-capture
  in the healthy register, same mechanism.
- §11 instrumentation — top-N losing-axis score logging (§7.W.6).
- §12 belief proxies — "active-but-losing" is a scoring fact, not a
  belief, so no belief-layer extension needed.
- `docs/reference/bdi-rao-georgeff.md:20` — Rao & Georgeff on
  Desire inconsistency; §7.W.1 extends this by not materializing
  a separate consistent Goal set.
- `docs/reference/bdi-rao-georgeff.md:88` — Rao & Georgeff mixed
  commitment strategies; the per-axis commitment-strategy choice
  §7.W relies on.
- `docs/systems/the-calling.md` — canonical instance (§7.W.4(a)),
  cross-linked from the Calling doc's "Relation to axis-capture"
  section.
- `docs/systems/warmth-split.md` — healthy-capture worked example
  (§7.W.4(b)), cross-linked from the warmth-split doc's cross-refs.
- `src/systems/mood.rs` — valence-drop pathway that converts
  losing-axis deficit into legible tension (§7.W.2).
- `src/steps/disposition/groom_other.rs:47` — the groom-other bleed
  site where the warmth conflation lives today.

### §7.8 Residuals — open questions from the original stub, resolved inline

The four open `?` bullets from the pre-rewrite stub, with their
resolutions so the history is preserved:

- **"Does Mark treat momentum as a consideration (axis inside the
  product) or as a post-composition bonus?"**
  Resolution: **both, at different layers.** Post-composition
  **persistence bonus** lives at the §7 commitment layer (§7.4),
  applied to the active Intention's score during re-evaluation.
  **Task-progress marginal utility** lives inside individual DSEs
  as ordinary considerations (e.g., a Build DSE's "percent-complete"
  axis gets a Logistic response curve from §2.1). Conflating the
  two would force every DSE to reimplement the commitment-layer
  gate or the commitment layer to leak DSE-specific progress
  semantics.

- **"What's the canonical decay curve for commitment strength?
  (Likely not linear.)"**
  Resolution: non-linear, increasing-marginal-utility on
  task-completion fraction. **Logistic** primitive from §2.1 is the
  clean fit and matches ch 15 §"Just Gimme a Minute, Boss!" and
  §"Finish Him!" The specific `midpoint` and `steepness` are
  numeric-tuning balance-thread work, not substrate spec (per the
  doc's line 24–29 scope discipline). The `base` magnitude is
  per-DispositionKind and is Enumeration Debt — 12 values.

- **"How does momentum interact with Maslow override — hunger
  should still preempt a committed 'explore' action below the
  starvation threshold."**
  Resolution: Maslow interrupts are **event-driven preemption**
  (§7.5) that bypass the §7.2 gate and the §7.4 persistence bonus
  entirely. The replacement Intention installs with `Blind`
  commitment. Matches the existing `check_anxiety_interrupts`
  pipeline exactly — no new path needed.

- **"Commitment strategy is an elasticity × apophenia tradeoff."**
  Resolution: the tradeoff is resolved via the **per-Intention-class
  strategy table** (§7.3). `Blind` for critical needs where
  elasticity is a liability (§0.2 would rather hold through noise);
  `SingleMinded` for goals where both elasticity and arc-legibility
  matter; `OpenMinded` for activities and aspirations where
  character-drift is the long-term-relevance payoff (§0.3). The
  table is not a global setting — different Intention classes get
  different elasticity profiles. Aspirations (§7.7) apply the same
  framework at a second timescale.

### §7.9 Cross-refs

- `docs/reference/bdi-rao-georgeff.md` — Rao & Georgeff extract;
  commitment vocabulary (§7.1), drop triggers (§7.2), mixed
  strategies (§7.3, §7.W.8), nested intentions (§7.7), Desire
  inconsistency (§7.W.1).
- `docs/systems/the-calling.md` — canonical axis-capture instance
  (§7.W.4(a)).
- `docs/systems/warmth-split.md` — healthy-capture worked example
  (§7.W.4(b)).
- `docs/reference/behavioral-math-ch15-changing-decisions.md` —
  Mark ch 15; monitoring cadence (§7.6), event-driven interrupts
  (§7.5), persistence bonus shape (§7.4), information expiry
  (§7.7), decision momentum framing (§7.7 reconsideration events).
- §0.2 (elastic failure), §0.3 (apophenia), §0.4 (character
  expression) — principles §7 instantiates.
- §L2.10.4, §L2.10.5 — Intention output and Goal | Activity split
  that §7.1's strategy tag rides on.
- §L2.10.6 — softmax scope; the challenger-Intention selection that
  §7.4's persistence bonus biases.
- §L2.10.7 — plan-cost feedback; the `SpatialConsideration` route
  providing §7.2's `achievable_believed` signal (candidate (a)
  resolution per ch 14).
- §12 — belief proxy scope boundary for §7.2's signals.
- `src/components/disposition.rs:39–64` — DispositionKind variants
  §7.3 maps against.
- `src/components/goap_plan.rs:103` — `replan_count` hard-fail signal
  §7.2 consumes.
- `src/ai/scoring.rs:695–704` — patience-bonus code §7.4 subsumes.
- `src/systems/disposition.rs:93` — `check_anxiety_interrupts`
  pipeline §7.5 documents placement of.
- `src/systems/aspirations.rs`, `src/systems/growth.rs`,
  `src/systems/death.rs`, `src/systems/fate.rs`,
  `src/systems/mood.rs` — aspiration layer (§7.7) event emitters.

---

## §8 Variation in choice

Normative argmax collapses every cat with the same inputs onto the
same action — Mark's "Stepford bank" failure mode (ch 16 opening
vignette, extracted at
`docs/reference/behavioral-math-ch16-variation.md`). The substrate
needs variation because §0.1 collapses player / director / simulation
into one: the observer is the sole consumer of behavioral diversity,
and variation is what they pattern-match against. Too little variation
reads as inert; too much reads as random. §0.3's two legs — abstracted
feedback and long-term relevance — make both failures spec-level bugs,
not tuning regrets.

Ch 16 walks through three algorithm generations: random-from-top-N,
weighted-random-from-top-N, then weighted-random-from-all. Only the
third survives Mark's own critique of arbitrary cutoffs
(ch 16 §"Weighted Random from All Choices"). This section commits
Clowder's placement in that lineage and closes the four open
questions the pre-synthesis stub carried.

### §8.1 Algorithm — softmax-over-all candidates

Commit: Boltzmann softmax weighting over the full candidate Intention
pool, no cutoff. Weight for candidate `i`:

```
w_i = exp((score_i - max_score) / T)
```

Sampling draws one Intention proportional to `w_i`. This is the
topology ch 16 settles on at chapter close (all candidates included,
weighting biases toward high-score options, bad options are
arithmetically suppressed rather than rule-based excluded). The one
departure from Mark's literal treatment: exponential weighting
instead of his linear `TotalScore / ThisScore` form (ch 16
§"Automatic Scaling"). Rationale:

- **Single-knob sharpness.** Temperature `T` collapses Mark's
  per-step coefficient + response-curve + rescale machinery (ch 16
  §"Scores and Weights", §"Use the Right Tool for the Job") into one
  continuous parameter. `T → 0` is argmax; `T → ∞` is uniform;
  intermediate values interpolate smoothly. This is easier to tune
  and diagnose than picking a coefficient, a curve shape, and a
  normalization strategy separately.
- **Numerical stability from §1.3.** Considerations emit strictly
  `[0, 1]` per §1.3 normalization; DSE scores compose into the
  same range per §3. Subtracting `max_score` before `exp()` keeps
  the weights in `[0, 1]` regardless of `T`, avoiding the
  integer-overflow-and-rescale pitfall ch 16 walks through.
- **Same family, not a departure.** Linear and exponential
  weighting are both weighted-random-from-all; they differ only in
  how steeply probability rises with score. The substrate-level
  decision here is the *topology* (all candidates, no cutoff),
  which matches ch 16's close. The specific weight shape is
  defensible either way.

**Rejected alternatives:**
- *Top-N or top-N% cutoffs* (ch 16's §"Random from Top n Choices"
  and §"Weighted Random from Top n Choices") — arbitrary thresholds
  that ch 16 itself walks back. Weighted-random-from-all squeezes
  low-score options to near-zero probability without needing a
  rule-based cutoff.
- *Linear weighting (Mark's literal form)* — lacks a single
  sharpness knob; forces the tuner to choose between coefficient
  magnitude and score-curve manipulation, which the §1.3
  normalization makes unnecessary.
- *Per-DSE temperature* — no current behavioral motivation; Clowder's
  score scale is already uniform across DSEs. Flagged in §8.7 as a
  possible future extension if balance work surfaces a need.

### §8.2 Scope — softmax-over-Intentions

Softmax runs once per cat per deliberation tick over the candidate
Intention pool (§L2.10.4). Action selection inside a chosen
Intention (GOAP step sequence for `Goal` Intentions, activity-runner
tick execution for `Activity` Intentions per §L2.10.5) is
deterministic. **Stochastic intent, deterministic execution.**

This inherits §L2.10.6's scope decision verbatim; §8.1 is the formal
resolution that §L2.10.6 flagged as pending. Two consequences for
today's code:

- `select_disposition_softmax` (`src/ai/scoring.rs:1194–1231`, hot
  path via `disposition.rs:721` and `goap.rs:1019`) is the ancestor
  of the post-refactor `select_intention_softmax`. Same algorithm,
  renamed vocabulary.
- `select_action_softmax` (`src/ai/scoring.rs:1039–1076`, exists
  but off hot path) retires. The unified DSE surface (§L2.10.4)
  emits Intentions, not Actions; action-layer softmax has no
  remaining role.

### §8.3 Temperature — commit T = 0.15 default

Ch 16 declines to commit a numeric temperature — Mark's linear
weighting has no temperature parameter — so calibration is
Clowder-specific. Commit the current tuned value as the substrate
default, with the bands that motivate it:

| Temperature | Behavior | Failure mode |
|---|---|---|
| `T < 0.05` | Approaches argmax; primary wins ~100% of the time | Stepford bank (ch 16 opening) — §0.3 "inert" leg fails |
| `T ≈ 0.10–0.20` | Personality-primary ~45–60%, coherent secondary runners-up | Target band |
| `T ≈ 0.30–0.50` | Notable secondary behaviors; mercurial at day-scale | §0.3 "long-term relevance" leg degrades |
| `T > 1.0` | Approaches uniform random | Noise; personality and scoring are irrelevant |

**Default: `T = 0.15`.** Matches existing `action_softmax_temperature`
and `disposition_softmax_temperature` in `ScoringConstants`
(`src/resources/sim_constants.rs:1081–1082, 1244–1245`). This is the
empirically-surviving value on seed-42 `--duration 900` soaks under
current canaries; not a first-principles derivation. The substrate
pins the band and the default; numeric refinement is balance-thread
work per `CLAUDE.md` line 24–29.

**Not personality-scaled.** Temperature describes how decisive the
cat is *at the Intention-selection layer*. Personality already flows
into scoring through per-consideration weights (§1, §4 markers).
Layering a second personality-scaled variation source on top of
score-embedded personality would double-count, collapsing the
spec-level separation of "who the cat is" (scoring) from "how
decisive they are" (selection). Character differences show up
through score differences; softmax is a uniform observer-facing
variation layer.

### §8.4 Order with §7 momentum — softmax first, persistence-bonus gating second

Per §L2.10.6: softmax runs over the freshly-scored candidate pool
("what would I pick if starting fresh?"); §7.4's persistence bonus
applies to the currently-held Intention; the challenger must beat
`current_score + persistence_bonus` to preempt. The stub's
"excluded from sampling vs. retained with bonus" sub-question
resolves as follows:

- **The committed Intention stays in the candidate pool.** Softmax
  samples it alongside every other candidate.
- If the sample draws the current Intention, the preemption check
  is trivially a no-op (the challenger is the incumbent).
- If the sample draws a different Intention, the challenger's
  score is compared against `current_score + persistence_bonus`
  per §7.4. Preemption fires only on strict-greater.
- **Never exclude the incumbent.** Excluding would force preemption
  every time softmax landed elsewhere — exactly the strobing
  failure mode §7.4 exists to prevent. Retention with bonus is the
  separation Mark ch 15 and Rao & Georgeff were pointing at: the
  variation layer and the commitment layer are orthogonal.

**Maslow interrupts (§7.5) bypass softmax entirely.** Event-driven
preemption installs a `Blind`-committed replacement Intention
without running the softmax path. Softmax lives on the normal
per-tick deliberation path (pipeline step 3 in §7.5); interrupts
live on step 1 and skip past the whole stack.

### §8.5 Species variants — converge foxes onto softmax

Today's fox pipeline uses `select_best_disposition`
(`src/ai/fox_scoring.rs:252–258`) — a deterministic `max_by` over
scores with additive pre-scoring jitter applied at scoring time. In
ch 16 terms this is "argmax with observation noise," roughly the
primitive form Mark walks away from across the chapter. It predates
the cat softmax path and is a §8-of-*Key insight #8* artifact (the
simpler approach that was good enough when the fox disposition set
was small).

Commit: **foxes converge to softmax** with a species-specific
temperature constant. Reasons:

- **Substrate uniformity.** The unified DSE surface (§L2.10) assumes
  one selection path. A parallel argmax+jitter model forces every
  future species (hawks, snakes, shadowfoxes, visitors) to pick a
  side and inherits a species-count-sized decision surface the
  substrate doesn't need.
- **Elastic variation scales with disposition count.** The
  argmax+jitter form works when the top candidate is obvious;
  softmax's graded weighting extends cleanly as the fox disposition
  roster grows (e.g., post-§L2.10 DSE additions for hunting
  sub-modes, pack coordination).
- **Matches *Key insight #8*.** Today's fox code is evidence of
  what the old substrate permitted, not a normative spec that the
  refactor must preserve.

Implementation trails this spec (lands with §L2.10's broader
DSE-surface unification). Spec-level commit: add
`fox_softmax_temperature` to `ScoringConstants`, default `0.15`
matching cats; fox-specific tuning is balance-thread work if
divergence becomes warranted.

The jitter-range term currently applied at fox scoring time
(`fox_scoring.rs:103`, per-score) retires — softmax replaces its
role entirely. Keeping both would stack two variation sources on
one species.

### §8.6 Apophenia calibration as continuity canary

Temperature's behavioral band (§8.3) is ultimately calibrated
against §0.3's two legs, not an information-theoretic target:

- **Abstracted feedback leg.** A cat's behavior should read to an
  observer as "she surprised me there," not "she acted randomly."
  Softmax landing on a lower-scored Intention is a *surprise* when
  it's in-character (personality-plausible, context-appropriate);
  it's *noise* when it's out-of-character.
- **Long-term relevance leg.** Same cat watched across multi-day
  windows should read as coherent — variation at the moment-to-
  moment scale does not dissolve character at the day scale.

Qualitative bar — "a cat that surprises but stays in character
across weeks" — makes temperature calibration a **continuity canary**
in the `CLAUDE.md` sense. Sits alongside the existing Generational
Continuity, Ecological Variety, and Mythic Texture canaries; hard
gate when operationalized.

Operationalization (pairwise behavioral distance across N sampled
cats at tick T; same-cat behavioral autocorrelation across K-day
windows) is §11 instrumentation work, not §8 substrate spec. Flagged
here so the canary gets wired in with the rest of the per-cat replay
tooling rather than invented separately later.

### §8.7 Residuals — open questions from the original stub, resolved inline

The four `?` bullets from the pre-rewrite stub, with their
resolutions:

- **"Does Mark recommend softmax-over-all or weighted-random-from-top-N?"**
  Resolution: ch 16 recommends weighted-random-from-*all*; top-N is
  an intermediate form ch 16 walks back. Clowder's existing
  softmax-over-all matches the topology; exponential vs linear
  weight shape is the only difference and the §8.1 single-knob
  rationale makes exponential the cleaner fit. See §8.1.

- **"What's the right temperature range for behaviorally-realistic
  variation vs. randomness?"**
  Resolution: `T = 0.10–0.20` band, committing `0.15` as the
  substrate default matching existing constants. Numeric refinement
  is balance-thread work. See §8.3.

- **"How does softmax interact with momentum (§7)? Does committed
  action get excluded from sampling, or retained with bonus?"**
  Resolution: retained with bonus. Softmax runs first over the full
  candidate pool; §7.4's persistence bonus applies to the incumbent
  during the subsequent preemption check. Never exclude — excluding
  forces the strobing §7.4 is built to prevent. Maslow interrupts
  bypass softmax entirely. See §8.4.

- **"Temperature calibration is bounded by apophenia."**
  Resolution: operationalizes as a continuity canary per §0.3's
  two legs, alongside `CLAUDE.md`'s existing continuity canary
  suite. Instrumentation plan belongs in §11. See §8.6.

### §8.8 Out of scope for this spec

- **Score-level normal-distribution fuzzing** (ch 16's closing
  "Use the Right Tool for the Job" suggestion). Clowder's
  personality traits already live inside DSE scoring (§1, §4); a
  second variation source at the score layer would double-count
  against the softmax-layer variation. Parked as a balance-thread
  experiment if §8.6 canary failures need more variation headroom.
- **Per-Intention-class temperature.** Plausible future extension
  (e.g., `Mating`-class Intentions softmax with sharper `T` because
  the decision is higher-stakes; `Wander`-class with softer `T`
  because variation is the whole point). No current behavioral
  motivation; single-temperature default holds until a canary
  failure demands otherwise.
- **Stochastic execution** (softmax at the action or plan-step
  layer). Explicitly not the scope — §8.2 commits to stochastic
  intent, deterministic execution. Revisiting would reopen the
  §7.4 momentum interaction; the substrate treats execution as a
  commitment-obligation contract, not a re-rolled choice.

### §8.9 Cross-refs

- `docs/reference/behavioral-math-ch16-variation.md` — ch 16
  extract; §"Weighted Random from All Choices" closes §8.1,
  §"Reasons for Variation" anchors §8.0 framing.
- §0.1 (director collapse — why variation matters to the observer),
  §0.3 (apophenia — the calibration target for §8.3 and §8.6),
  §0.4 (character expression — bounds what "in-character surprise"
  means) — the principles §8 instantiates.
- §1.3 (strict `[0, 1]` normalization) — what keeps `exp()`
  numerically tame in §8.1.
- §L2.10.4 (Intention output) — the object softmax samples over.
- §L2.10.5 (Goal / Activity split) — execution stays deterministic
  inside either shape.
- §L2.10.6 (scope decision) — §8 is the formal resolution that
  §L2.10.6 flagged pending.
- §7.1 (commitment strategy), §7.2 (drop-trigger gate), §7.4
  (persistence bonus), §7.5 (Maslow interrupts) — the commitment
  layer that runs after softmax per §8.4's ordering.
- §11 (instrumentation) — apophenia continuity-canary tooling for
  §8.6.
- `src/ai/scoring.rs:1039–1076` (`select_action_softmax`,
  retiring), `:1194–1231` (`select_disposition_softmax`, renames to
  `select_intention_softmax` post-refactor) — implementations §8.1
  formalizes.
- `src/ai/fox_scoring.rs:252–258` (`select_best_disposition`), `:103`
  (per-score jitter, retiring) — the argmax-plus-jitter path §8.5
  converges onto softmax.
- `src/resources/sim_constants.rs:1081–1082, 1244–1245` — the two
  existing temperature constants §8.3 commits; a third
  (`fox_softmax_temperature`) lands with §8.5 implementation.
- `src/systems/disposition.rs:721`, `src/systems/goap.rs:1019` —
  hot-path softmax call sites that become Intention-layer call
  sites post-refactor.

---

## §9 Faction model

Faction relationships are declared once in a `FactionRelations`
resource and read by DSE eligibility filters:

```rust
pub enum FactionStance {
    Same,      // same species, same colony
    Ally,      // different species, aligned (e.g., a befriended fox)
    Neutral,
    Prey,      // hunting target
    Predator,  // flee target
    Enemy,     // combat target
}

pub struct FactionRelations(HashMap<(Species, Species), FactionStance>);
```

Asymmetry is free: `Cat → Fox = Predator` and `Fox → Cat = Prey`
coexist without contradiction. Per-cat perception (which *specific*
fox is known-as-dangerous vs. unknown-stranger) lives in the ToT
belief layer, not here.

### §9.0 Vocabulary reconciliation with §5.6.6.1

`Species` in `(Species, Species)` is the flattened 10-variant set
matching §5.6.6.1's row vocabulary: Cat, Fox, Hawk, Snake, ShadowFox,
Mouse, Rat, Rabbit, Fish, Bird. The code-side substrate reaches the
same value-shape through the existing nested enum union at
`src/components/sensing.rs:19` (`SensorySpecies = Cat | Wild(WildSpecies) |
Prey(PreyKind)`), with `WildSpecies` (`src/components/wildlife.rs:9`)
and `PreyKind` (`src/components/prey.rs:17`) covering the eight
non-cat species. Whether `FactionRelations` is keyed by a flattened
`FactionSpecies` newtype or by the nested enum itself is an
implementation detail of the L2 build; the spec commits only the
*value-shape*: 100 stance cells, one per directed species pair.

The two 10-species matrices (§5.6.6.1 sensory and §9.1 stance) must
stay vocabulary-aligned — adding a species means extending both.

### §9.1 Biological base matrix (10 × 10)

Base stance by directed species pair. Rows = observer species;
columns = target species. Diagonal is `Same` by convention.
Abbreviations: `Sm` = Same, `Al` = Ally, `N` = Neutral, `Py` = Prey
(hunt target), `Pd` = Predator (flee target), `E` = Enemy (combat
target).

|           | Cat  | Fox  | Hawk | Snake | ShFx | Mouse | Rat  | Rabbit | Fish | Bird |
|-----------|------|------|------|-------|------|-------|------|--------|------|------|
| **Cat**    | Sm¹  | Pd   | Pd²  | Pd³   | E    | Py    | Py   | Py     | Py   | Py   |
| **Fox**    | Py   | Sm   | N    | N     | N⁴   | Py    | Py   | Py     | N⁶   | Py   |
| **Hawk**   | Py²  | N    | Sm   | Py⁵   | N    | Py    | Py   | Py     | N⁶   | Py   |
| **Snake**  | N³   | N    | Pd   | Sm    | N    | Py    | Py   | Py     | N⁶   | Py   |
| **ShFx**   | E    | N⁴   | N    | N     | Sm⁷  | Py⁸   | Py⁸  | Py⁸    | N⁶   | Py⁸  |
| **Mouse**  | Pd   | Pd   | Pd   | Pd    | Pd⁸  | Sm    | Pd⁹  | N      | N    | N    |
| **Rat**    | Pd   | Pd   | Pd   | Pd    | Pd⁸  | Py⁹   | Sm   | N      | N    | N    |
| **Rabbit** | Pd   | Pd   | Pd   | Pd    | Pd⁸  | N     | N    | Sm     | N    | N    |
| **Fish**   | Pd   | N⁶   | N⁶   | N⁶    | N⁶   | N     | N    | N      | Sm   | N    |
| **Bird**   | Pd   | Pd   | Pd   | Pd    | Pd⁸  | N     | N    | N      | N    | Sm   |

100 / 100 directed species-pair stance cells committed.

Footnotes:

1. **Cat × Cat = Same** is the base; overlaid per §9.2 by `Visitor`,
   `HostileVisitor`, `Banished`, and `BefriendedAlly` markers. All
   cat-on-cat stance variation is expressed through the overlay, not
   through widening `Species`.
2. **Hawk → Cat = Prey** applies to kittens (carry-off) and injured
   adults; healthy-adult cats are functionally ignored. Size / life-
   stage scaling is carried by `AttackDse` target considerations
   (`boldness × target_size`), not by the base matrix — the row
   commits the ecological default and `FleeDse` eligibility filter.
3. **Cat → Snake = Predator / Snake → Cat = Neutral** is asymmetric.
   Cats respect snakes as dangerous (`FleeDse` triggers), snakes do
   not actively hunt cats (`wildlife.rs:280,390,666` — snakes pursue
   prey, not cats; coiling-near-cat narrative at `wildlife.rs:600–603`
   is defensive, not offensive). A cornered snake becomes a combat
   target via `AttackDse` using a `Threatened` eligibility marker, not
   a base-stance upgrade.
4. **Fox × ShadowFox = Neutral** on both rows. Lore (`docs/systems/
   magic.md`, `src/systems/magic.rs:501`) frames shadowfox as a
   corrupted double of a fox, which *suggests* Enemy; but no code path
   today enforces fox-vs-shadowfox hostility (both share
   `predator_hunt_range` and wildlife-morale logic at
   `wildlife.rs:667–730`). Commit Neutral as the conservative stance;
   upgrade to Enemy in a dedicated balance pass if/when a
   fox-rejects-corruption system lands.
5. **Hawk → Snake = Prey** reflects the ecological default (raptors
   take snakes). No DSE currently consumes this cell; committed for
   matrix completeness and future Hawk-AI extension.
6. **Aquatic carve-out.** Terrestrial predators (Fox, Hawk, Snake,
   ShFx) × Fish = Neutral — these species do not cross the water
   boundary in Clowder's map today. Only Cat × Fish = Prey
   (`Hunt` over water-edge). Reciprocal Fish → Cat = Predator
   (fish flee cats at the water edge) but Fish → {Fox, Hawk, Snake,
   ShFx} = Neutral for the same reason.
7. **ShadowFox × ShadowFox = Same** is a matrix-completeness
   convention; shadowfoxes are solitary and have no intra-species
   interaction in the sim. No DSE consumes this cell.
8. **ShadowFox × prey = Prey** (and reciprocally prey × ShadowFox =
   Predator) because shadowfoxes run the same predator-hunt code as
   regular foxes (`wildlife.rs:667` — `predator_hunt_range_shadow_fox`).
   The motive differs (corruption-spread rather than satiation — `fox
   satiation_ticks` gain is gated to non-shadowfox at
   `wildlife.rs:721–725` and `wildlife.rs:702–717` occasionally
   spawns a corrupting carcass instead of straight consumption), but
   the stance toward prey is functionally `Py`.
9. **Rat-mouse predation.** Adult rats opportunistically kill mice;
   Rat→Mouse = `Py`, Mouse→Rat = `Pd`. Committed per §0 enumeration
   discipline even though no DSE consumes this cell today. Flagged
   as a latent ecology hook for a future prey-on-prey build.

### §9.2 ECS-marker overlay (colony / visitor / banished / befriended)

The `(Species, Species) → FactionStance` map cannot, by itself,
express colony membership or per-entity social state — `Cat × Cat` is
a single key. Four ECS markers refine the base stance at DSE-filter
time. These live in the §4.3 vocabulary (same schema: Predicate /
Insert / Remove / Status / Source system); §9.2 is their stance-
refinement definer.

| Marker | Predicate | Insert / Remove | Effect on stance | Status | Source system |
|---|---|---|---|---|---|
| `Visitor` | Non-colony cat present on map — Wandering Loner, Trader, or Scout per `docs/systems/trade.md:19–22` | `trade.rs::arrive_visitor` / `trade.rs::depart_visitor` (new) | Observer-Cat × target-Cat: demote `Same` → `Neutral`. No effect on other pairs. | Absent | `docs/systems/trade.md` |
| `HostileVisitor` | Hostile-Loner variant (`trade.md:21`); attached on arrival or on stolen-from / drove-off-previously memory | as `Visitor` | Observer-Cat × target-Cat: demote `Same` → `Enemy`. | Absent | `docs/systems/trade.md` |
| `Banished` | Cat exiled from the colony via social or combat consequence (`colony_score.banishments:41` already counts the event; the marker does not yet persist on the exiled entity) | `social.rs` / `combat.rs` — today's `pending_banishments` pattern at `combat.rs:120,242,356–395` is shadowfox-only, extend to cat-on-cat | Observer-Cat × target-Cat: demote `Same` → `Enemy`. | Absent | `src/systems/combat.rs:117–395` + `src/systems/social.rs` (new predicate) |
| `BefriendedAlly` | Fox or prey-species target befriended through repeated non-hostile contact — subsumes the current §9 prose example "a befriended fox" | `social.rs::befriend_wildlife` (new) | Observer-Cat × target-Fox: upgrade `Predator` → `Ally`. Reciprocal target-Fox × observer-Cat: upgrade `Prey` → `Ally`. | Absent | `docs/systems/trade.md` befriending path |

**Resolution order.** Overlay markers apply *after* the base lookup;
if multiple markers coexist on a target, **most-negative wins**:
`Banished` ≻ `HostileVisitor` ≻ `Visitor` ≻ base ≻ `BefriendedAlly`.
This prevents "Banished + BefriendedAlly" from collapsing to `Ally`
and "Visitor + Banished" from collapsing to `Neutral`. The order is
committed here, not rediscovered per system, so the marker-insert
systems can land independently without drift.

**Scope boundary with §12 (beliefs).** §9.2 markers encode *facts
about the target entity* authored by colony-scale systems (arrival
event, banishment event, befriending event). Per-cat perceptual state
— "this *specific* fox is known-as-dangerous to cat X but not to cat
Y" — remains in the ToT belief layer per §12, not here. The practical
split: if the fact is uniform across all observers (a Banished cat is
Banished to everyone in the colony), it is a §9.2 marker. If the fact
is per-observer (cat X has witnessed this fox kill a clanmate; cat Y
has not), it is a §12 belief.

### §9.3 DSE filter binding

The stance matrix is testable against §L2.10.3's DSE registry —
each DSE that operates on a candidate entity declares the
`FactionStance` set it accepts as an eligibility filter. The
substrate rejects candidates whose stance is not in the accepted set
before scoring, so the matrix above is directly load-bearing for
target-taking DSE output.

| DSE | Required stance on candidate | Source §L2.10.3 row |
|---|---|---|
| `SocializeDse` | `Same` \| `Ally` | Tier-2 cat block |
| `AttackDse` | `Enemy` \| `Prey` | Tier-2 cat block |
| `FleeDse` | `Predator` | Tier-1 cat block |
| `HuntDse` | `Prey` | Tier-2 cat block |
| `FoxRaidDse` | `Prey` (with colony-adjacency `StoreVisible` marker refinement per §4.3) | Fox block |

Five filter rows cover the target-taking DSEs that read stance
today. A DSE not listed here (e.g., `EatDse`, `SleepDse`,
`HerbcraftDse`) does not gate on stance — its target universe is
defined by other §4.3 markers (`HasStoredFood`, terrain markers,
herb-availability markers).

---

## §L2.10 DSE catalog & single invocation surface

Prior sections address three of the four L2 problems: **structure**
(§1 Consideration trait), **response shape** (§2 curves), and
**composition** (§3). This section addresses the remaining two:
**discovery** — unifying the 10+ scattered scoring sites into a single
evaluation surface — and **DSE output shape** — deciding what a DSE
actually produces when it scores.

### §L2.10.1 Current landscape — scoring is scattered

The Phase-1 audit found scoring logic in at least 10 sites, most of
them not recognized as DSEs:

| Site | Shape | What it scores |
|---|---|---|
| `src/ai/scoring.rs` | 21 action blocks, canonical | Cat per-tick action scores |
| `src/ai/fox_scoring.rs` | Parallel 3-level Maslow | Fox dispositions; separate `FoxScoringContext`; no softmax |
| `src/systems/coordination.rs:88–107` | Hand-authored multiplicative formula, every 100 ticks | Coordinator election across cats |
| `coordination.rs:321–503` | Parallel priority queue | Directive generation with float priorities |
| `src/systems/aspirations.rs:49–96` | Domain affinity scoring | Aspiration-chain selection per zodiac + personality |
| `disposition.rs:1329–1347`, `goap.rs:3788–3810` | Duplicated per-target ranking | Social / mentor / mate / attack target selection (divergent, per §6.2) |
| `disposition.rs:1881–1907` | Linear per-candidate | Mate target selection |
| `disposition.rs:1925–1943` + `feed_kitten.rs` | Boolean gate + kinship | Caretake target selection (partial; §6.1) |
| `src/resources/narrative_templates.rs:616–649` | Weighted-random by specificity × weight | Narrative line selection |

Any L2 that unifies only the cat-action DSEs leaves the fox DSE,
coordinator election, directive priorities, target-resolvers, and
narrative selection as stranded utility islands. Each island is a
future divergence bug (§6.2 is one instance already).

### §L2.10.2 Unified evaluation surface

Every scoring site above expresses as a `Dse` or `TargetTakingDse`
(§6.3). One evaluation function consumes them all:

```rust
pub trait Dse {
    fn id(&self) -> DseId;
    fn eligibility(&self) -> &EligibilityFilter;  // ECS markers, §4
    fn score(&self, cat: Entity, ctx: &EvalCtx) -> f32;
    fn intention(&self, cat: Entity, ctx: &EvalCtx) -> Intention;
}

pub fn evaluate(
    dses: &[&dyn Dse],
    cat: Entity,
    ctx: &EvalCtx,
) -> Vec<(DseId, f32, Intention)>;
```

Selection strategy — argmax, softmax, weighted-random, top-N-sample —
is a **separate concern** from evaluation, per ch 13 §"Compartmentalized
Confidence." The evaluate function produces scored candidates;
selection is a downstream step with its own (tunable) temperature.

### §L2.10.3 DSE registration

DSEs are registered at plugin load, not hard-coded per-action. The
registration API is the single seam across which every scoring
surface in the substrate ships, and its method set is **open by
design** per §5.6.9 — a future species (hawks, snakes, prey, human
visitors, shadowfoxes promoted to first-class) slots in as a new
`add_*_dse` method without touching the evaluator.

```rust
app.add_dse(eat_dse())
   .add_dse(sleep_dse())
   .add_dse(hunt_dse())
   // ...
   .add_target_taking_dse(socialize_dse())
   .add_target_taking_dse(mate_dse())
   // ...
   .add_fox_dse(fox_patrol_dse())
   .add_fox_dse(fox_hunt_dse())
   // Scattered-site absorbents (§L2.10.1):
   .add_coordinator_dse(coordinator_election_dse())
   .add_coordinator_dse(directive_assessment_dse())
   .add_aspiration_dse(reproduce_aspiration_dse())
   .add_narrative_dse(narrative_template_selection_dse());
```

Five registration methods cover today's scoring surface:

- `add_dse(...)` — plain cat DSEs without per-candidate target
  ranking.
- `add_target_taking_dse(...)` — DSEs that resolve among candidate
  entities via §6.3 `TargetTakingDse`. The 9 target-taking DSEs from
  §6.3 / §6.4 register here; per-target considerations live in
  §6.5.
- `add_fox_dse(...)` — per-species fox registrations. Replaces
  today's parallel `src/ai/fox_scoring.rs` Maslow surface with a
  registration set sharing the evaluator.
- `add_coordinator_dse(...)` — coordinator-role DSEs (election,
  directive assessment, urgent dispatch). Today scattered across
  `src/systems/coordination.rs`; absorbed as first-class DSEs.
- `add_narrative_dse(...)` — narrative-line selection as a DSE
  consumer, per §L2.10.1's "narrative template specificity ×
  weight" surface at `narrative_templates.rs:616–649`.

The registration catalog below is **exhaustive** against commit
`333fd7b`. Line-number citations match source today; Explore-agent
verification confirmed no drift. Every row of §L2.10.1's
"stranded utility islands" table appears as at least one row
below; §L2.10.1 and §L2.10.3 form a one-to-one before/after pair.

#### Cat DSEs — Tier 1 (physiological / survival)

| Constructor | Method | Subsumes | Composition | Intention shape | Notes |
|---|---|---|---|---|---|
| `eat_dse()` | `add_dse` | `scoring.rs:203–208` | CompensatedProduct | `Goal(hunger < threshold)` | Need-driven; Maslow L1. |
| `sleep_dse()` | `add_dse` | `scoring.rs:210–233` | CompensatedProduct | `Goal(energy > threshold)` | Diurnal-phase piecewise + injury axis per §2.3. |
| `hunt_dse()` | `add_target_taking_dse` | `scoring.rs:235–249`; merges with `disposition.rs` prey-resolve path | CompensatedProduct | `Goal(prey_caught)` | §6.5.5 target set; §L2.10.7 Quadratic spatial curve. |
| `forage_dse()` | `add_dse` | `scoring.rs:251–259` | CompensatedProduct | `Goal(food_at_stores)` | Food-scarcity axis via §2.3 Quadratic. |
| `groom_self_dse()` | `add_dse` | `scoring.rs:283–300` (self branch) | WeightedSum | `Goal(thermal + affection deficit cleared)` | Sibling to `groom_other` (Max retires). Pending `needs.warmth` split. |
| `flee_dse()` | `add_dse` | `scoring.rs:320–327` | CompensatedProduct | `Goal(threat_distance > safe)` | Steepest Logistic (§2.3); event-driven interrupt path (§7.5). |

#### Cat DSEs — Tier 2 (safety / territory)

| Constructor | Method | Subsumes | Composition | Intention shape | Notes |
|---|---|---|---|---|---|
| `fight_dse()` | `add_target_taking_dse` | `scoring.rs:329–353`; merges with `disposition.rs` combat-resolve path | CompensatedProduct | `Goal(threat_incapacitated)` | §6.5.9 target set; piecewise health/safety gating. |
| `patrol_dse()` | `add_dse` | `scoring.rs:355–362` | CompensatedProduct | `Activity(Patrol, UntilCondition(scent_refreshed))` | Activity-shaped; territory-scent map consumer. |
| `build_dse()` | `add_target_taking_dse` | `scoring.rs:364–388`; merges with `disposition.rs` site-resolve path | WeightedSum | `Goal(structure_complete)` | §6.5.8 target set; chain-driven completion. |
| `farm_dse()` | `add_dse` | `scoring.rs:390–401` | WeightedSum | `Goal(crop_harvested)` | Chain-driven; Maslow L2 suppression on phys only. |
| `socialize_dse()` | `add_target_taking_dse` | `scoring.rs:261–281`; unifies divergent resolvers at `disposition.rs:1329–1347` and `goap.rs:3788–3810` | WeightedSum | `Activity(Socialize, UntilCondition(sated))` | §6.5.1 target set; **resolves §6.2 silent divergence** — one resolver now. |
| `groom_other_dse()` | `add_target_taking_dse` | `scoring.rs:283–300` (other branch) | WeightedSum | `Activity(Allogroom, Ticks(N))` | §6.5.4 target set; sibling to `groom_self` (Max retires). |
| `explore_dse()` | `add_dse` | `scoring.rs:302–309` | WeightedSum | `Activity(Explore, UntilInterrupt)` | Exploration-map consumer; §7.4 Medium-High persistence. |
| `wander_dse()` | `add_dse` | `scoring.rs:311–318` | WeightedSum | `Activity(Wander, UntilInterrupt)` | "Always available" sentinel with curiosity + playfulness. |
| `cook_dse()` | `add_dse` | `scoring.rs:618–639` | WeightedSum | `Goal(food_cooked_at_kitchen)` | Chain-driven: move to kitchen, process raw → cooked. |

#### Cat DSEs — Tier 2–5 (craft / leadership / reproduction / care / idle)

| Constructor | Method | Subsumes | Composition | Intention shape | Notes |
|---|---|---|---|---|---|
| `coordinate_dse()` | `add_dse` (coordinator-role eligibility filter) | `scoring.rs:585–595` | WeightedSum | `Activity(Coordinate, UntilInterrupt)` | Per-role registration — non-coordinators don't register. |
| `mentor_dse()` | `add_target_taking_dse` | `scoring.rs:597–605`; merges with `disposition.rs:1361–1376` apprentice-targeting | WeightedSum | `Activity(Mentor, UntilCondition(skill_gap_closed))` | §6.5.3 target set; skill-gap-magnitude is the **§6.1 critical fix**. |
| `caretake_dse()` | `add_target_taking_dse` | `scoring.rs:641–654`; merges with `disposition.rs:1925–1943` + `feed_kitten.rs` | WeightedSum | `Goal(kitten_hunger_sated)` | §6.5.6 target set; Quadratic spatial curve (ch 14 "Which Dude"). |
| `idle_dse()` | `add_dse` | `scoring.rs:656–662` | WeightedSum | `Activity(Idle, UntilInterrupt)` | Always-available fallback; floor-clamp curve. |
| `apply_remedy_dse()` | `add_target_taking_dse` | `disposition.rs` remedy-target path | WeightedSum | `Goal(injury_healed)` | §6.5.7 target set; today chain lives in `ai/planner/actions.rs:221–262`. |

**Incapacitated-retiring.** `scoring.rs:181–201` (incapacitated
override) **does not** register a DSE — the branch retires per §2.3.
The `Incapacitated` ECS marker (§4) filters ineligible DSEs; surviving
candidates (Eat, Sleep, Idle) produce correct behavior on their own
curves.

#### Mating (three-layer — §7.M)

| Constructor | Method | Subsumes | Composition | Intention shape | Notes |
|---|---|---|---|---|---|
| `reproduce_aspiration_dse()` | `add_aspiration_dse` | New — no single source site today | WeightedSum | `Aspiration(Reproduce, OpenMinded, ...)` | Layer 1 per §7.M.1. Emits L2 + L3. |
| `pairing_activity_dse()` | `add_dse` (gated on `Partners+` bond marker, §4) | Absorbs ambient pair-bond drift in `social.rs:100–175` as an explicit activity | WeightedSum | `Activity(Pairing, UntilCondition(partner_lost_or_out_of_season))` | Layer 2 per §7.M.1. |
| `mate_with_goal_dse()` | `add_target_taking_dse` | `scoring.rs:607–616` + `disposition.rs:1873–1919` mating chain | CompensatedProduct | `Goal(mating_event_completed)` | Layer 3 per §7.M.1. §6.5.2 target set for partner selection. |

#### Herbcraft / PracticeMagic sibling DSEs (§L2.10.10)

| Constructor | Method | Subsumes | Composition | Intention shape | Notes |
|---|---|---|---|---|---|
| `herbs_in_inventory_dse()` | `add_dse` | `scoring.rs:420–428` | WeightedSum | `Goal(herbs_in_inventory > threshold)` | Herbcraft sub-mode; per §L2.10.10. |
| `remedy_applied_dse()` | `add_target_taking_dse` | `scoring.rs:429–437` | WeightedSum | `Goal(injury_healed)` | Herbcraft sub-mode; per §L2.10.10. |
| `ward_placed_dse()` | `add_target_taking_dse` | `scoring.rs:451–464` | WeightedSum | `Goal(ward_at_tile)` | Herbcraft sub-mode; per §L2.10.10. |
| `scry_dse()` | `add_dse` | `scoring.rs:485–488` | CompensatedProduct | `Activity(Scry, UntilCondition(vision_received))` | PracticeMagic sub-mode; Calling integration point. |
| `durable_ward_dse()` | `add_target_taking_dse` | `scoring.rs:512–522` | WeightedSum | `Goal(durable_ward_at_tile)` | PracticeMagic sub-mode. |
| `cleanse_dse()` | `add_target_taking_dse` | `scoring.rs:523–533` | CompensatedProduct | `Goal(tile_corruption == 0)` | PracticeMagic sub-mode. |
| `colony_cleanse_dse()` | `add_dse` | `scoring.rs:535–541` | CompensatedProduct | `Goal(territory_max_corruption < threshold)` | PracticeMagic sub-mode. |
| `harvest_dse()` | `add_target_taking_dse` | `scoring.rs:543–551` | WeightedSum | `Goal(harvested_from_carcass)` | PracticeMagic sub-mode. |
| `commune_dse()` | `add_dse` | `scoring.rs:552–559` | CompensatedProduct | `Activity(Commune, UntilInterrupt)` | PracticeMagic sub-mode; special-terrain gate. |

Retires the parent `herbcraft_dse` / `practice_magic_dse` `Max`-composed
surface entirely — sibling registration is the mechanism by which the
§L2.10.4 Intention framing dissolves the old parent-action bundling.

#### Fox DSEs (`fox_scoring.rs`, 9 dispositions)

| Constructor | Method | Subsumes | Composition | Intention shape | Notes |
|---|---|---|---|---|---|
| `fox_hunting_dse()` | `add_fox_dse` | `fox_scoring.rs:134–150` | CompensatedProduct | `Goal(prey_caught)` | L1 survival; shares prey-location map with cat Hunt (§5.6.3 row #5). |
| `fox_raiding_dse()` | `add_fox_dse` | `fox_scoring.rs:152–159` | CompensatedProduct | `Goal(food_from_stores)` | L1 survival; store-visibility eligibility. |
| `fox_resting_dse()` | `add_fox_dse` | `fox_scoring.rs:161–177` | WeightedSum | `Activity(Rest, UntilCondition(energy_restored))` | L1 survival; den-based comfort. |
| `fox_fleeing_dse()` | `add_fox_dse` | `fox_scoring.rs:179–186` | CompensatedProduct | `Goal(threat_distance > safe)` | L1 survival; shares Flee anchor with cat. |
| `fox_patrolling_dse()` | `add_fox_dse` | `fox_scoring.rs:193–210` | WeightedSum | `Activity(Patrol, UntilCondition(scent_refreshed))` | L2 territory; ticks-since-patrol saturation. |
| `fox_avoiding_dse()` | `add_fox_dse` | `fox_scoring.rs:212–222` | WeightedSum | `Activity(Avoid, UntilCondition(cats_cleared))` | L2 territory; pre-threat avoidance. |
| `fox_feeding_dse()` | `add_fox_dse` | `fox_scoring.rs:229–236` | CompensatedProduct | `Goal(cubs_satiated)` | L3 offspring; protectiveness-scaled. |
| `fox_den_defense_dse()` | `add_fox_dse` | `fox_scoring.rs:238–245` | CompensatedProduct | `Goal(threat_cleared_from_den)` | L3 offspring; shares Flee anchor. |
| `fox_dispersing_dse()` | `add_fox_dse` (juvenile life-stage eligibility) | `fox_scoring.rs:106–124` | — | `Goal(reached_new_territory)` | Lifecycle override; juveniles only. |

Fox DSEs pointedly have **no** `add_target_taking_dse` entries today
— §L2.10.7's audit found all 9 use binary range gates or
aggregate-proximity scalars rather than per-candidate ranking. When
the refactor lands, fox DSEs can opt into `add_target_taking_fox_dse`
by row without per-species code changes.

#### Scattered-site absorbents (§L2.10.1 → §L2.10.3)

| Constructor | Method | Subsumes | Composition | Intention shape | Notes |
|---|---|---|---|---|---|
| `coordinator_election_dse()` | `add_coordinator_dse` | `coordination.rs:60–160` (`evaluate_coordinators`, every 100 ticks) | CompensatedProduct | `Goal(coordinator_role_assigned)` | Social-weight × diligence × sociability × ambition formula; runs on a slower cadence than per-tick DSEs. |
| `directive_assessment_dse()` | `add_coordinator_dse` | `coordination.rs:179–543` (`assess_colony_needs`, every 20 ticks) | WeightedSum | `Goal(directive_queue_filled)` | 7 branches (Food, Threat, Building, Injury, Posse, Ward, Corruption); priorities merge into a single queue. Urgent dispatch at `559–700+` becomes an event-driven emission, not a separate DSE. |
| `reproduce_aspiration_dse()` | `add_aspiration_dse` | See §7.M row above (this line is aspiration-layer for the Mating showcase). | WeightedSum | `Aspiration(Reproduce, OpenMinded, ...)` | First concrete user of `add_aspiration_dse`; framework ports to other aspirations. |
| `aspiration_chain_dse()` | `add_aspiration_dse` | `aspirations.rs:49–96` (`score_chain`) | WeightedSum | `Aspiration(<domain>, OpenMinded, ...)` | Domain-affinity scoring (zodiac × personality × experience + jitter). One registration per domain; domain list lives in §7.7.1. |
| `narrative_template_selection_dse()` | `add_narrative_dse` | `narrative_templates.rs:616–649` (`TemplateSet::select`) | WeightedSum | — (narrative output, no Intention) | Template specificity × template weight; filtered by context. Not a goal-directed DSE — the registration surface reuses the evaluation infrastructure for the selection math. |

**Target-ranking unification.** Today, `disposition.rs:1329–1347`
and `goap.rs:3788–3810` run divergent per-target scoring (fondness +
novelty vs. fondness alone — §6.2 silent divergence). Neither
appears as its own row above because **both are absorbed into the
`add_target_taking_dse` rows** for `socialize_dse`, `mate_with_goal_dse`,
`mentor_dse`, and `caretake_dse` via §6.3. The divergence is fixed at
registration: one resolver, one source of truth.

#### Catalog summary

| Method | Rows | Notes |
|---|---|---|
| `add_dse` | 18 | Plain cat DSEs (Tier 1–5) + sibling singletons + cook/idle/wander sentinels. |
| `add_target_taking_dse` | 13 | 9 headline target-taking DSEs (§6.3) + 4 herb/magic target-taking siblings. |
| `add_aspiration_dse` | 2 | Reproduce + domain-affinity scaffold; one registration per future aspiration domain. |
| `add_coordinator_dse` | 2 | Election + directive-assessment; urgent dispatch is event-driven. |
| `add_fox_dse` | 9 | All 9 fox dispositions; no target-taking entries today. |
| `add_narrative_dse` | 1 | Template selection; future expansion for scripted-narrative DSEs. |
| **Total** | **45** | Versus today's 23+ scattered sites; one registration surface, one evaluator. |

The registration API is open-set (§5.6.9). Adding a new species,
role, or aspiration domain is one registration; adding a new
registration method (e.g., `add_prey_dse`, `add_visitor_dse`) is
one method on the app-extension trait. Neither touches the
evaluator, the Intention vocabulary, or downstream commitment /
softmax layers.

### §L2.10.4 DSE output: Intention, not Action

The bigger framing question: **what does a DSE's result represent?**

Today: DSE produces an `Action` enum variant. `Disposition` (a
post-scoring grouping) aggregates actions into categorical labels; GOAP
then back-infers a step chain from the disposition label. Herbcraft
and PracticeMagic bundle sub-modes under one parent action because
otherwise each sub-mode would compete independently with everything
else and the "crafty" behavioral arc would fragment.

Proposed L2: DSE produces an **Intention**. This collapses multiple
design problems simultaneously — companion-action bundling, sub-mode
max-selection, resolver/scoring divergence — into a single cleaner
layer.

```rust
pub enum Intention {
    /// Reach a goal state; GOAP plans the step chain.
    Goal(GoalState),
    /// Sustain an activity for a duration or until a condition.
    Activity(ActivityKind, Termination),
}

pub enum Termination {
    Ticks(u32),
    UntilCondition(fn(&World, Entity) -> bool),
    UntilInterrupt,  // e.g. Idle — preempted by anything scoring higher
}
```

Every emitted `Intention` additionally carries a
`CommitmentStrategy` tag (§7.1; Rao & Georgeff commitment vocabulary
per `docs/reference/bdi-rao-georgeff.md` §3) —
`Blind | SingleMinded | OpenMinded` — that determines when the
reconsideration gate drops it. The strategy tag rides on the
Intention, not on the DSE, so a single DSE can emit
context-dependent strategies (e.g., `Patrol` emits a `Blind`
Guarding Intention under high-threat context, `SingleMinded` under
routine).

**Why this collapses problems:**

- **Companion-action bundling dissolves.** `Caretake` as
  `Intention::Goal(kitten.hunger < threshold)` lets GOAP plan
  walk→pickup→deliver from its action library naturally — that's what
  GOAP is for (Jeff Orkin, GDC 2006). No more "the DSE emitted an
  Action label, we need to infer the rest." Mark ch 14 §"More or
  Less?" is resolved via the Intention vocabulary, not via decision-
  composition.
- **Sub-mode max-selection dissolves.** `Herbcraft` today does
  `max(GatherHerbs, PrepareRemedy, SetWard)` as a parent action.
  Under Intention-emitting DSEs, each sub-mode becomes a sibling
  goal-shaped DSE (`herbs_in_inventory`, `remedy_applied`,
  `ward_placed`) with shared eligibility. No parent `max()`; the
  evaluator + softmax pick directly among siblings. Same for
  `PracticeMagic`'s 6 sub-modes.
- **Silent GOAP/Disposition resolver divergence (§6.2) dissolves.**
  One `TargetTakingDse` owns target-quality; its winning target is
  carried into the `Intention::Goal`. Both disposition.rs and goap.rs
  consume the same Intention.
- **Intention-over-Action serves apophenia's abstracted-feedback leg**
  (§0.3). The observable story unit is "she wants to mother that
  kitten" — a hook the observer carries forward and attributes causally
  — not "Caretake scored 0.82." Emitting Intentions upgrades the sim's
  narrative surface from scored labels to inferable wants.

### §L2.10.5 Intention = `Goal | Activity` is Clowder-specific

Classical BDI (Rao & Georgeff 1991 —
`docs/reference/bdi-rao-georgeff.md`) assumes every
Intention reduces to a goal state that expands to a plan. That fits
`Caretake`, `Hunt`, `Herbcraft` sub-modes, `Build`, `Mate`, `ApplyRemedy`
— each has a clean goal.

But Clowder has **sustained activities** with no threshold end
condition: `Socialize` (interact for a while), `Wander` (walk
aimlessly), `Idle` (do nothing until preempted), `Patrol` (move around
the territory). These don't reduce to `state_delta < threshold`;
they're "do this for a while." Classical BDI would force them into
implicit goal form ("maintain bond-with-X above Y for N ticks"); the
Clowder L2 call is to treat them as first-class `Activity`
intentions with explicit termination conditions.

**This is a Clowder-specific design call**, explicitly flagged so
future readers don't assume pure-BDI semantics. The cost is two
execution paths (GOAP for `Goal`, activity runner for `Activity`).
The benefit is that activity-shaped DSEs stop pretending to be goals
— today's `Socialize` resolver already runs a scripted-duration
interaction, the code just doesn't admit that's what it is.

**Strategy-shape correlation.** `Activity` Intentions with
`Termination::UntilCondition` or `UntilInterrupt` almost always
pair with `OpenMinded` commitment (§7.3) — the activity's drop
trigger *is* a desire-drift condition (sated-sociability, curiosity
exhausted) rather than a goal-state predicate. `Activity` with
`Termination::Ticks` pairs with `SingleMinded` by default (the
tick budget is the achievability bound). This is not a hard rule —
the strategy tag rides on the Intention per §L2.10.4 — but the
correlation is strong enough that §7.3's OpenMinded rows all live
in the `Activity` half of the Intention enum.

### §L2.10.6 Softmax-over-Intentions is the right variation scope

Today's softmax runs over dispositions (`select_disposition_softmax`,
temperature 0.15). `select_action_softmax` exists but is never in the
hot path. Given the Intention framing, the natural scope is
**softmax over Intentions** — stochastic *intent*, deterministic
*execution*. This matches §8's variation goal and keeps §7's momentum
attachment (commitment is to the Intention, not to individual plan
steps).

Formal resolution lives in §8 (closed 2026-04-21 against Mark ch 16).
This section named the scope commitment so §8 could inherit rather
than re-litigate it — see §8.2.

**Order with §7.4's persistence bonus.** Softmax picks the challenger
Intention from the freshly-scored candidate pool (the "what would I
pick if starting fresh?" question). §7.4's persistence bonus is then
applied to the *currently-held* Intention's score, and the challenger
must beat `current_score + persistence_bonus` to preempt. Softmax
runs first; persistence-bonus gating runs second. This order matters:
variation lives at "decide what to want," not at "decide whether to
abandon my current commitment." The commitment-layer gate is
deterministic given the score inputs.

### §L2.10.7 Plan-cost feedback — resolved via Mark ch 14

Emitting Intentions doesn't make scoring cost-aware. If `Caretake`
scores 0.9 but the kitten is 50 tiles away while food is 2 tiles
away, utility was blind to cost — GOAP will plan the long trip on an
inflated score. Mark ch 14 §"Which Dude to Kill?" folds distance
into the decision explicitly; Clowder needs the same.

**Chosen: candidate (a) — `SpatialConsideration` with response curves.**
Ch 14 (`docs/reference/behavioral-math-ch14-modeling-decisions.md`,
§"Which Dude to Kill?" through §"Scoring the Option") folds distance
into scoring via response curves — weapon accuracy falloff over
range (Figure 14.6), urgency-vs-detonator-range via a parabolic
curve (Figure 14.8) — without invoking a pathfinder at scoring time.
Mark's agent does not ask "can I path there?" mid-score; it asks
"what's the shaped-by-distance score?" and lets the score itself
encode reachability implicitly through its curve shape.

Clowder matches this pattern. Each spatially-sensitive DSE — the
target-taking DSEs catalogued in §6.3 + §6.4, plus the non-target
DSEs whose scoring is landmark-anchored (Forage-at-cache, Rest-at-
hearth, etc.) — carries a `SpatialConsideration` with a curve
primitive from §2.1. Appropriate curves:

- `Quadratic` or `Power` for "closer is better, falling off sharply"
  (hunt, defend-territory, urgent-threat-response).
- `Logistic` for "close enough is close enough, then falls off"
  (routine errands, non-urgent socializing).
- `Linear` as a fallback when the distance semantics don't warrant
  a non-linear shape (e.g., exploration where the distance curve
  *is* the incentive gradient).

The full per-DSE roster of `SpatialConsideration` curve assignments
is Enumeration Debt — see the doc's Enumeration Debt section.

**Why candidate (b) is rejected.** Pathfinder-in-the-loop creates
the chicken-and-egg that prior drafts of this section flagged: the
scoring layer asks GOAP "what would this cost?" while GOAP is
trying to decide whether this Intention is plannable. It's also
expensive (pathfinder invocations per-DSE-per-tick-per-cat) and
brittle under world-state change without a cache-and-invalidate
layer that itself becomes a correctness surface. Mark ch 14 does
not advocate this shape; §0.2 elastic failure prefers the smooth
score-degradation that (a) naturally provides.

**The `replan_count` hard-fail exit.** Repeated
`GoapPlan::replan_count ≥ max_replans`
(`src/components/goap_plan.rs:103`) remains the hard-fail signal for
§7.2's `achievable_believed` ⇒ false path under `SingleMinded`
commitment. This is a GOAP-layer signal, not a scoring-layer
signal, and it fires only when an Intention is genuinely
unplannable (impassable geometry, destroyed target, resource
vanished mid-plan). The two-channel structure — elastic score
attenuation via (a), plus hard-fail via `replan_count` — is what
§7.2 consumes; see §7.2 for how they compose.

**Elastic-failure preservation (§0.2).** Candidate (a) degrades
smoothly as landmarks become less reachable (obstacle density
grows, path distance increases). The `replan_count` hard-fail
fires only when elasticity has run out — the score has already
attenuated low but the cat is still committed under strategy
rules. Both channels match §0.2's "consequence-rich failure that
propagates, never arc-terminating failure" principle.

**Spatially-sensitive DSE roster.** Full enumeration of the 21
current cat DSEs (`src/ai/scoring.rs`) and 9 fox dispositions
(`src/ai/fox_scoring.rs`). Each row commits the post-refactor
shape: target landmark for the `SpatialConsideration` and the curve
primitive from §2.1 that fits it. Numeric tuning (curve
midpoint/steepness) is balance-thread work per line 24–29.

> **Audit finding.** No cat or fox DSE currently uses continuous
> distance-to-landmark scoring. All 13/21 cat DSEs with spatial
> inputs and 6/9 fox dispositions with spatial inputs use binary
> range gates or aggregate-proximity scalars (`unexplored_nearby`,
> `tile_corruption`, `nearby_corruption_level`, `local_prey_belief`).
> The refactor changes every row's shape, not just adds curves —
> this roster is a full aspirational specification, not an audit of
> current behavior. Consistent with *Insight #8* (evidence, not
> specification): the binary-gate pattern reflects the old
> substrate's constraints, not the target state.

**Cat DSEs (22 rows — Groom splits into self and other post-refactor, per §L2.10.2's per-species registration surface; the other 21 map 1:1 to `scoring.rs` action blocks):**

| DSE | Today | Target landmark | Curve | Rationale |
|---|---|---|---|---|
| Eat | binary `food_available` | Stores / Kitchen building | `Logistic` | Close-enough-is-close-enough; distant food viable but discounted. |
| Sleep | N/A | Own Den / sleeping spot | `Power` | Strong preference for own den; sharp fall-off from it. |
| Hunt | binary `prey_nearby` | Prey entity position | `Quadratic` | Ch14 weapon-accuracy shape — closer prey is disproportionately better. |
| Forage | binary `can_forage` | Nearest forageable tile cluster | `Logistic` | Routine errand; sharp fall-off outside a reasonable radius. |
| Socialize | binary `has_social_target` | Social partner position | `Logistic` | Routine social visibility; near partners saturated, far ones discounted. |
| Groom (self) | N/A | N/A (self) | N/A — not spatial | Self-directed, no landmark. |
| Groom (other) | binary `has_social_target` | Other-cat position | `Quadratic` | Intimate act; must be adjacent — sharp distance penalty. |
| Explore | aggregate `unexplored_nearby` | Unexplored frontier | `Linear` | Distance *is* the incentive gradient; linear shape preserves gradient-following. |
| Wander | N/A | N/A | N/A — not spatial | Curiosity baseline; no target. |
| Flee | binary `has_threat_nearby` | Threat position (inverted) | `Power` | Inverse-distance-from-threat; closer threat is sharply more urgent. |
| Fight | binary `has_threat_nearby` + `allies_fighting_threat` | Threat + ally cluster | `Quadratic` | Range + ally-proximity factor; commitment rises sharply when allies engaged nearby. |
| Patrol | `needs.safety` (not spatial today) | Territory perimeter | `Linear` | Walking-the-beat pattern; even spacing along perimeter. |
| Build | binary `has_construction_site` | Site position | `Logistic` | Commute-to-work pattern. |
| Farm | binary `has_garden` | Garden tile | `Logistic` | Same commute shape as Build. |
| Herbcraft | binary `has_herbs_nearby` / `has_remedy_herbs` / `thornbriar_available` | Herb patch / ward placement tile | `Logistic` | Herb commute; emergency-corruption boost handled by scalar, not spatial. |
| PracticeMagic | aggregate `tile_corruption` / `nearby_corruption_level` | Corrupted tile cluster | `Power` | Corruption urgency rises sharply near epicenter per `magic.rs` scent-like spread (§5.6). |
| Coordinate | N/A | Coordinator's perch / meeting tile | `Logistic` | Weakly spatial — coordinator works from location; distant cats discounted for participation. |
| Mentor | binary `has_mentoring_target` | Mentee position | `Quadratic` | Requires sustained proximity; sharp fall-off. |
| Mate | binary `has_eligible_mate` | Mate position | `Logistic` | Courtship commute pattern. **Note:** Mating now resolves as the three-layer aspiration → activity → goal showcase in §7.M. This row's `Logistic` spatial curve applies specifically to **Layer 3** `MateWithGoal`'s travel-to-partner step and to **Layer 2** `PairingActivity`'s proximity-bias term. **Layer 1** `ReproduceAspiration`'s partner-seeking is not landmark-anchored — it drives the broader §6.5.2 `Mate` target-selection consideration set (romantic + fondness + distance + fertility-window) across all reachable cats, not a narrow distance-to-known-partner curve. |
| Cook | binary `has_functional_kitchen` + `has_raw_food_in_stores` | Kitchen building | `Logistic` | Travel-to-kitchen commute. |
| Caretake | scalar `hungry_kitten_urgency` | Kitten position | `Quadratic` | Urgency × proximity; distant hungry kitten vs. near healthy one is exactly the ch14 §"Which Dude" shape. |
| Idle | N/A | N/A | N/A — not spatial | Fallback; no target. |

**Fox dispositions (9 rows):**

| Disposition | Today | Target landmark | Curve | Rationale |
|---|---|---|---|---|
| Hunting | binary `prey_nearby` + `local_prey_belief` scalar | Prey-belief cluster centroid | `Quadratic` | Belief-grid provides soft location; same ch14 shape as cat Hunt. |
| Feeding | binary `has_cubs` + `cubs_hungry` | Den position | `Power` | Return-to-den is highly localized. |
| Patrolling | N/A | Territory perimeter | `Linear` | Even spacing along scent perimeter. |
| Raiding | binary `store_visible` + `store_guarded` | Colony store | `Logistic` | Commute-to-target with guard-deterrent handled as a separate scalar. |
| DenDefense | binary `cat_threatening_den` + `has_cubs` | Den position | `Power` | Inverse-distance-from-den; sharper than Flee because cubs anchor commitment. |
| Resting | binary `has_den` | Den position | `Power` | Home-base pull; sharp fall-off. |
| Dispersing | N/A (lifecycle) | Map edge / nearest unclaimed territory | `Linear` | Gradient-following from parent territory outward. |
| Fleeing | `needs.health_fraction` / `cats_nearby` count | Nearest map edge | `Power` | Same inverse-distance-from-threat shape as cat Flee. |
| Avoiding | `cats_nearby` count | Cat cluster centroid (inverted) | `Power` | Inverse-distance-from-cats; sharper than Flee because Avoiding is pre-threat. |

**Non-spatial rows.** Five cat DSEs (Groom-self, Wander, Idle,
Cook, Coordinate-minimal) and one fox disposition (Patrolling is
weakly-spatial via perimeter; Dispersing is lifecycle) declare
`not spatial` or gradient-based rather than landmark-anchored.
This exhaustive declaration satisfies the doc's enumeration
principle — every DSE is addressed, even if addressed as "not
applicable."

**Cross-refs:**
- §6.3 `TargetTakingDse` — `SpatialConsideration` lives inside
  per-target scoring, not outside it.
- §6.4 personal-interest template — distance is already named as a
  row in the existing per-target consideration enumeration.
- §7.2 reconsideration gate — consumes both channels of the
  `achievable_believed` signal.
- §12.3 — names `achievable_believed` as the belief proxy grounded
  in Rao & Georgeff's strong-realism axiom (CI1), see
  `docs/reference/bdi-rao-georgeff.md` §2.
- §7.M — notes that the Mate row's curve applies to the
  travel-to-partner step only; Mating's overall substrate shape is
  under reconsideration.
- `docs/reference/behavioral-math-ch14-modeling-decisions.md`
  §"Which Dude to Kill?" — reference pattern; Figures 14.6–14.8.

### §L2.10.8 Dependencies on §7 and §8

- **§7 (commitment and persistence)** — resolved. The Rao & Georgeff
  commitment strategies (§7.1, `Blind / SingleMinded / OpenMinded`)
  tag Intentions; the Mark ch 15 persistence bonus (§7.4) gates
  preemption during re-evaluation. The two-layer aspiration-vs-
  Intention architecture (§7.7) uses the same vocabulary at
  different timescales. §7 is no longer a dependency *blocker* —
  the framework is specified and §L2.10.4's Intention output
  carries the strategy tag directly.
- **§8 (variation)** runs softmax over Intentions (§L2.10.6). Keeps
  micro-execution deterministic; variation lives at the
  decide-what-to-want layer, not the execute-what-I-chose layer.

### §L2.10.9 Cross-refs

- Mark ch 14 §"More or Less?" — whether to bundle actions into one
  decision (resolved here via Intention vocabulary).
- Rao & Georgeff (1991) — BDI architecture; `Intention` naming and
  commitment-strategy framework.
- Jeff Orkin, "Three States and a Plan: The AI of F.E.A.R." (GDC
  2006) — the GOAP template Clowder's planner descends from; the
  goal-shaped Intention framing here matches Orkin's goal-selection
  + planner split.

### §L2.10.10 Herbcraft / PracticeMagic sibling-DSE curve specs

§L2.10.4 named the sibling set that replaces the `Max`-composed
parent `Herbcraft` and `PracticeMagic` DSEs: three Herbcraft
siblings (`gather`, `prepare`, `ward`) and six PracticeMagic
siblings (`scry`, `durable_ward`, `cleanse`, `colony_cleanse`,
`harvest`, `commune`). Each sibling is a first-class DSE under
§L2.10.4 — its own eligibility filter, its own composition mode,
its own considerations — registered as a separate row per
§L2.10.3. The parent `Max` wrappers retire entirely.

This section commits curve specs per sibling. §2.3's curve-shape
table names shapes per parent consideration axis grouped under
`Herbcraft.*` and `PracticeMagic.*`; the view here is the
*sibling* frame — each sibling as a complete DSE with its full
consideration set. Rows cite the corresponding §2.3 anchor where
shapes reuse established primitives.

**Key decision recorded inline.** Two siblings (`scry`, `commune`)
are **Activity-shaped** per §L2.10.5; the other seven are
**Goal-shaped**. This is the first concrete application of the
`Goal | Activity` split *below* the top-level DSE layer —
§L2.10.5 named the split at parent-DSE granularity, and this
subsection demonstrates that the split ports cleanly to siblings
when their termination semantics diverge. `scry` is an
observation-until-vision activity (Calling integration point per
`docs/systems/the_calling.md`); `commune` is a presence-at-
special-terrain activity that terminates on interrupt rather
than a goal state.

**Herbcraft siblings (3 rows):**

| Sibling DSE | Today | Intention shape | Composition | Considerations → curves |
|---|---|---|---|---|
| `herbs_in_inventory` (today `Herbcraft.gather`) | `scoring.rs:420–428` | `Goal(inventory.herbs > per_cat_threshold)` | WeightedSum | `spirituality` → `Linear` (§2.3 personality anchor); `herbcraft_skill` → `Linear(intercept=herbcraft_gather_skill_offset)` (§2.3); `territory_max_corruption` → `Logistic(steepness=8, midpoint=0.1)` (§2.3 — retires `ward_corruption_emergency_bonus` / `cleanse_corruption_emergency_bonus` flat-bonus shape). Eligibility: `has_herbs_nearby` marker (§4). |
| `remedy_applied` (today `Herbcraft.prepare`) | `scoring.rs:429–437` | `Goal(target_cat.injury_level < threshold)` | WeightedSum | `compassion` → `Linear`; `herbcraft_skill` → `Linear(intercept=herbcraft_prepare_skill_offset)`; `colony_injury_count` → `Composite { Linear(slope=herbcraft_prepare_injury_scale), Clamp(max=herbcraft_prepare_injury_cap) }` (§2.3 saturating-count anchor). Eligibility: `has_remedy_herbs` marker + at least one injured colony cat. Target-taking (registers via `add_target_taking_dse`) — per-target consideration set is §6.5.7's `ApplyRemedy` template. |
| `ward_placed` (today `Herbcraft.ward`) | `scoring.rs:451–464` | `Goal(ward_at_tile && ward.strength > threshold)` | WeightedSum | `spirituality` → `Linear`; `herbcraft_skill` → `Linear`; `territory_max_corruption` → `Logistic(8, 0.1)` (§2.3); `ward_under_siege` → `Piecewise([(0, 0), (1, herbcraft_ward_siege_bonus)])` (§2.3 — keeps siege bonus as a named primitive). Eligibility: `thornbriar_available` marker; target tile resolved via §6.3 target-taking. |

**PracticeMagic siblings (6 rows):**

| Sibling DSE | Today | Intention shape | Composition | Considerations → curves |
|---|---|---|---|---|
| `scry` | `scoring.rs:485–488` | `Activity(Scry, UntilCondition(calling_vision_received))` | CompensatedProduct | `curiosity`, `spirituality`, `magic_skill` → all `Linear` (§2.3 personality + mastery anchors). Eligibility: `magic_affinity > gate` marker (per §4; today a hard gate at `scoring.rs:483` — the §L2.10.4 refactor softens this into a misfire filter rather than a DSE gate, per the 2026-04-19 follow-on note). No spatial input — introspective activity. |
| `durable_ward` | `scoring.rs:512–522` | `Goal(durable_ward_at_tile)` | WeightedSum | `spirituality` → `Linear`; `magic_skill` → `Linear`; `nearby_corruption_level` → `Logistic(8, 0.1)` (§2.3 — collapses the old `corruption_sensed_response_bonus` flat gate + scale into one primitive). Eligibility: `durable_ward_herbs_prepared` marker + target tile from §6.3. |
| `cleanse` | `scoring.rs:523–533` | `Goal(tile_corruption == 0)` | CompensatedProduct | `spirituality` → `Linear`; `magic_skill` → `Linear`; `tile_corruption` → `Logistic(steepness=8, midpoint=magic_cleanse_corruption_threshold)` (§2.3). Eligibility: `on_corrupted_tile` marker. Spatial target is the corrupted tile itself; target-taking registers via `add_target_taking_dse`. |
| `colony_cleanse` | `scoring.rs:535–541` | `Goal(territory_max_corruption < threshold)` | CompensatedProduct | `spirituality` → `Linear`; `magic_skill` → `Linear`; `territory_max_corruption` → `Logistic(steepness=6, midpoint=0.3)` (§2.3 — softer than per-tile cleanse because territory-wide response is proactive, not emergency). No tile-presence gate — motivation is global; execution picks the hottest tile at step-resolution time. |
| `harvest` | `scoring.rs:543–551` | `Goal(harvested_from_carcass)` | WeightedSum | `curiosity` → `Linear`; `herbcraft_skill` → `Linear(intercept=0.1)` (§2.3); `carcass_count` → `Composite { Linear, Clamp(max=3) }` (§2.3 saturating-count anchor). Eligibility: `carcass_nearby` marker; target-taking (carcass entity) via §6.3. |
| `commune` | `scoring.rs:552–559` | `Activity(Commune, UntilInterrupt)` | CompensatedProduct | `spirituality` → `Linear`; `magic_skill` → `Linear`. Eligibility: `on_special_terrain` marker. **Spatial consideration deferred** — the special-terrain influence map (§5.6.3 row #13) is currently Absent; once built, a `Power` curve landmark-anchored at the nearest special-terrain tile applies (§L2.10.7 pattern). Flagged explicitly so the gap is visible. |

**Retires on this subsection landing.** The `Herbcraft` and
`PracticeMagic` parent rows in §3.1.1 already carry the
"Max — retiring" tag; this subsection's sibling breakdown is what
makes the retirement concrete. Parent `Max` composition disappears;
each sibling carries its own composition mode (CP or WS per the
table above). Parent `Herbcraft` / `PracticeMagic` DSE registrations
(in §L2.10.3) do not exist — the nine sibling registrations replace
them entirely.

**Personality-coefficient anchors** across all nine rows reuse
§2.3's Linear default for bounded `[0, 1]` coefficients
(`spirituality`, `curiosity`, `compassion`, `herbcraft_skill`,
`magic_skill`). Only the non-coefficient axes (`territory_max_corruption`,
`nearby_corruption_level`, `tile_corruption`, `colony_injury_count`,
`carcass_count`, `ward_under_siege`) pick non-`Linear` primitives,
each citing its §2.3 anchor row.

**Cross-refs:**
- §L2.10.4 — the Intention framing that produces the sibling set
  (parent `Max` dissolves into sibling DSEs).
- §L2.10.5 — Goal | Activity classification; `scry` and `commune`
  are the first sub-parent uses of the Activity path.
- §L2.10.3 — the nine rows appear in the Herbcraft / PracticeMagic
  sibling block of the registration catalog; total per-subsystem
  row count there sums to 3 + 6 = 9.
- §2.3 — anchor curves (hangry, scarcity, inverted-need-penalty,
  saturating-count, piecewise-threshold) reused across sibling
  axes.
- §3.1.1 — composition-mode assignment sums to 4 CP + 5 WS for the
  nine siblings; parent `Max` rows retire.
- §6.5.7 `ApplyRemedy` target set — `remedy_applied` sibling uses
  the existing §6.5 template; no new per-target considerations
  needed.

---

## §10 Baseline-feature unblock map

The substrate's end-state unblocks these `docs/systems/*.md` stubs
(currently Aspirational or Partial per `docs/wiki/systems.md`):

| Substrate capability | Stubs unblocked | Stub status move |
|---|---|---|
| §2 Response curves + §3 composition | Disease, Mental Breaks, Recreation & Grooming, Sleep (curve part), Environmental Quality (mood-pressure curve) | Aspirational → Built |
| §5 Influence maps (general) | **Environmental Quality (ambient comfort as the canonical first non-scent layer)**; enables §5.2 sensory modulation + spatial coordination substrate for Raids/Strategist | Env Quality: Aspirational → Built |
| §5.2 Sensory channel attenuation | Sensory System (currently pinned at 1.0), Body Zones (perception effects) | Aspirational → Built, Partial → Built |
| §6 Target-as-inner-optimization | (retries `social_target_range` iter-1 regression; unblocks nothing standalone) | — |
| §L2.10 Unified DSE surface + Intention output | Fox AI parity with cat AI (shared evaluator, no parallel Maslow); species-extensibility for prey/hawks/snakes/shadowfox (register DSEs, don't re-implement scoring); Strategist-Coordinator (directive-priority becomes a first-class DSE set) | Aspirational → Built (Strategist pending ToT + this) |
| §7 Momentum | Sleep (circadian commitment), The Calling (trance commitment), partial Substances enablement | Aspirational → Built |
| §4 Context tags (faction + relationships) | Trade & Visitors (arrival via faction stance), Organized Raids (multi-agent coordinated behavior) | Aspirational → Built (requires ToT for pairwise) |

Three stubs — **Substances**, **The Calling**, **Strategist-Coordinator**
— need capabilities from multiple sections plus the ToT belief layer,
and thus become Built only after that follow-on work. They are
listed here for dependency clarity.

**World Generation** (currently Partial) promotion requires the
full substrate being stable enough to run in fast-forward mode —
out of scope for this spec.

### §10.1 Feature-design filter from §0.4

When designing the features this substrate unblocks, the §0.4 filter
applies to each: *would the sim tell a different story about this cat
if this feature's value were different?* Feature designs that pass the
filter deepen who each cat is; designs that don't pass consume
apophenia budget without returning character.

Worked examples against the §10 queue:

- **Disease** — passes as "disease changes *who a cat is* during the
  illness" (withdrawn, irritable, over-needy, clingy — shape depends
  on the cat's personality). Fails as "disease reduces constitution by
  N." The substrate makes both possible; §0.4 says pick the former.
- **Recreation & Grooming** — passes as "a cat's choice of play
  expresses their personality" (a solitary ambush-cat stalks
  thornbriar pods; a social cat initiates a group pounce; an elder
  watches from a sunbeam). Fails as generic "fun meter fills on play."
- **Mental Breaks** — passes naturally because Sylvester's own
  template is character-expressive breaks (catatonic / binge / insult
  spiral, each legible as character). Fails only if flattened to a
  single "stress-break" mechanic.
- **Substances** — the risk case. Passes as "which substance a cat
  chooses and how it changes them says who they are." Fails as "drink
  thornbriar tea → +5 courage for 2 days." Designers should expect to
  push back against buff-style framings here.
- **Sleep that makes sense** — passes when sleep pattern expresses the
  cat (a nervous cat sleeps shallowly, a bonded pair curl together,
  an elder naps through the day). Fails as "sleep restores stat."

None of the §10 capabilities automatically pass — each stub needs its
own character-expression framing at feature-design time. The substrate
enables both shapes; §0.4 is the filter that catches the wrong one.

---

## §11 Instrumentation and observability

Substrate layers L1 / L2 / L3 are each a Forrester stock-and-flow
system with a distinct input set, a distinct transform, and a distinct
output stock. Today's only cross-layer observational surface is
`CatSnapshot.last_scores` — the *output* of L2, with no visibility
into L1 samples, per-consideration contributions, post-modifier
deltas, or L3 selection probabilities. Against a 3-layer substrate
that adds 13 influence maps, ~30 DSEs × ~5 considerations each, 6+
modifiers, softmax variation, and momentum commitment, that surface
is too narrow to support balance work — a Logistic midpoint tweak
can't be predicted from current state, and a surprising decision
frame can't be replayed.

The instrumentation design here generalizes Dave Mark's **Curvature**
tool (input distribution + curve + output distribution overlaid for
one response curve) to every layer of the substrate.

### §11.1 Design principle: Curvature at every layer

Each layer has a natural input→transform→output triple that the
instrumentation mirrors one-to-one:

| Layer | Input (stock) | Transform (flow) | Output (stock) |
|---|---|---|---|
| L1 (§5) | Emitter positions × channel × faction | Propagation kernel + decay + attenuation pipeline (species × role × injury × env) | Per-cell map value; per-cat sampled value |
| L2 (§1–§3) | Per-consideration scalar / marker / spatial inputs | Curve (§2) → composition (§3) → Maslow pre-gate → modifier stack (§3.5) | Final DSE score per cat per tick |
| L3 (§7–§8, §L2.10) | Ranked DSE scores + current Intention + momentum | Softmax (§8) + commitment-margin test (§7) | Chosen Intention; GOAP plan |

"Curvature at a frame" means: for any (cat, tick) the full
decomposition — every L1 sample, every L2 consideration contribution,
the L3 pick and its probability mass — is joinable into one
reconstruction. Balance work then becomes the predict-from-transform
loop `CLAUDE.md`'s Balance Methodology already prescribes: hypothesize
a change, predict the effect from the current input distribution + the
new transform, A/B via soak, accept/reject on concordance.

### §11.2 Sampling strategy: focal-cat replay

Full-fidelity emission is infeasible at 60 TPS × N cats × ~30 DSEs ×
~5 considerations each. The chosen strategy is **focal-cat replay**:
one designated "debug-traced" cat emits full layer-by-layer records
every tick; all other cats retain today's `CatSnapshot` cadence.

Rationale:

- Matches how users form narrative attachment. When a specific cat
  surprises you on seed 42, you replay *her* decision frames, not the
  colony's. This is §0.3 apophenia working in reverse — the same
  attachment that makes the sim generative also localizes the
  diagnostic interest.
- Tractable: ~1/N of full-fidelity emission cost.
- Extensible: event-triggered records (emit a full frame only when
  selection changes or a rare Intention wins) and aggregate-
  distribution footers (per-DSE, per-consideration histograms across
  the whole soak) become cheap follow-ons once the per-frame format
  exists. They aren't load-bearing for the initial build.

Default focal cat is deterministic per seed (cat-name generation is
seeded); `--focal-cat NAME` on the headless runner overrides, with a
spawn-order-index fallback if the name isn't present.

### §11.3 Record format — sidecar JSONL

Traces write to `logs/trace-<focal>.jsonl`, a separate file from
`events.jsonl`. The sidecar's line-1 header mirrors events.jsonl's
(`commit_hash`, `sim_config`, `constants`) so the two diff-lock as a
pair — a trace from one run is comparable to another only when both
sidecar and events headers agree. Reason for sidecar over promoting
into events.jsonl: today's tooling (`just verdict`,
`scripts/check_canaries.sh`, `just sweep-stats`) assumes events.jsonl
is a colony-wide aggregate at a stable cadence. Promoting focal-only
records inline would bloat events.jsonl 10–20× and break those
scripts' cadence assumptions.

Record shapes (sketch; exact schema refined at implementation):

**L1 sample** — one record per (focal cat × map × sample), emitted
lazily as a side-effect of an L2 consideration that reads the map
(no every-tick × every-map emission):

```json
{"tick": 4821, "cat": "Simba", "layer": "L1",
 "map": "fox_scent", "faction": "fox", "channel": "scent",
 "pos": [14, 9], "base_sample": 0.42,
 "attenuation": {"species_sens": 1.0, "role_mod": 1.0,
                 "injury_deficit": 0.0, "env_mul": 1.0},
 "perceived": 0.42,
 "top_contributors": [{"emitter": "Fox#7", "pos": [17, 11],
                       "distance": 4, "contribution": 0.31}]}
```

`top_contributors` is load-bearing: §5.1 stamps templates additively,
so a high scent reading can have many emitters. Without the
breakdown, you see "scent is high" but not *which* fox drove it, and
the "should this ward have fired?" question is unanswerable.

**L2 DSE evaluation** — one record per (focal cat × eligible DSE ×
tick):

```json
{"tick": 4821, "cat": "Simba", "layer": "L2", "dse": "Hunt",
 "eligibility": {"markers_required": ["CanHunt", "¬Incapacitated"],
                 "passed": true},
 "considerations": [
   {"name": "hunger", "input": 0.82, "curve": "Logistic(8,0.75)",
    "score": 0.94, "weight": 0.35},
   {"name": "food_scarcity", "input": 0.55, "curve": "Quadratic(2)",
    "score": 0.30, "weight": 0.25},
   {"name": "prey_proximity", "input": 4, "curve": "Quadratic(2)",
    "score": 0.56, "weight": 0.20,
    "spatial": {"map": "prey_location", "best_target": "Mouse#42"}},
   {"name": "boldness", "input": 0.7, "curve": "Linear",
    "score": 0.7, "weight": 0.20}
 ],
 "composition": {"mode": "WeightedSum", "raw": 0.64},
 "maslow_pregate": 1.0,
 "modifiers": [
   {"name": "pride", "delta": 0.03},
   {"name": "fox_suppression", "multiplier": 1.0},
   {"name": "independence_solo", "delta": 0.05}
 ],
 "final_score": 0.72,
 "intention": {"kind": "Goal", "target": "Mouse#42",
               "goal_state": "prey_caught"}}
```

Each row is a joinable stock-flow decomposition: the `input` column
is a stock sample (from L1 for spatial considerations, from
Needs/Personality/Skills for scalar ones); the `score` column is the
post-curve projection; `raw` and `final_score` are the
composition/modifier flows.

**L3 selection** — one record per (focal cat × tick):

```json
{"tick": 4821, "cat": "Simba", "layer": "L3",
 "ranked": [["Hunt", 0.72], ["Eat", 0.68], ["Patrol", 0.41]],
 "softmax": {"temperature": 0.15, "probabilities": [0.58, 0.38, 0.02]},
 "momentum": {"active_intention": "Hunt", "commitment_strength": 0.6,
              "margin_threshold": 0.09, "preempted": false},
 "chosen": "Hunt",
 "intention": {"kind": "Goal", "target": "Mouse#42"},
 "goap_plan": ["MoveToTile(15,10)", "StalkPrey(Mouse#42)",
               "PouncePrey(Mouse#42)"]}
```

L3 closes the loop: what the cat saw (L1) → what she wanted (L2) →
how she planned to get it (L3 + GOAP).

### §11.4 Joinability — the load-bearing invariant

Every record carries `(tick, cat, layer, primitive_id)`. A Python
replay tool (`scripts/replay_frame.py --tick N --cat NAME`) pivots
on `(tick, cat)` to reconstruct a full decision frame top-to-bottom.
Aggregate views fall out of the same data: histogram
`consideration.input` across all ticks → the cat's `hunger`
distribution over the soak; diff against a post-change run →
predicted-vs-observed shift.

This is the instrumentation property that lets §2/§3 balance changes
ship under `CLAUDE.md`'s four-artifact rule (hypothesis, prediction,
observation, concordance). Without joinability, predictions can only
operate on outputs (`final_score`), not on inputs or transforms —
and the prediction loop degenerates to "change it and see what
happens."

### §11.5 Scope rules and defensive structuring

Alignment to §5.6.9's L1 extensibility contract: the trace emitter
must never hardcode a channel / map / DSE / consideration list. It
walks the registries at runtime. Adding a new L1 map, a new DSE, or
a new consideration results in a new record shape automatically; the
trace format is a passive reflection of the registries, not a
parallel enumeration. Violating this (adding a named
`fox_scent_trace` method) would regenerate the §5.6.9 anti-goal
inside the instrumentation layer.

Headless-only emission: a `FocalTraceTarget` resource is inserted by
the headless runner, absent in `SimulationPlugin`. Trace systems gate
on `run_if(resource_exists::<FocalTraceTarget>)`. No interactive code
path sees the trace emitter.

### §11.6 Out of scope (flagged for follow-on, not for this refactor)

- GUI frame-scrubber in the interactive build — replay is post-hoc
  JSONL for now.
- Multi-focal-cat traces — combat correlation with two cats in the
  same frame. Second-pass extension.
- Event-triggered records (full frame emitted only on selection
  change or rare-Intention win) — cheap follow-on once the
  per-frame format is wired.
- Aggregate distribution footer (per-DSE × per-consideration
  histograms across the soak) — same; low-cost follow-on.

### §11.7 Cross-refs

- `CLAUDE.md` Balance Methodology — the four-artifact rule
  (hypothesis, prediction, observation, concordance) that §11's
  joinability exists to serve.
- §2.3 curve-shape assignment table — each row is a prediction the
  instrumentation must be able to verify post-implementation.
- §3.3.2 absolute-anchor peer groups — traces must expose enough
  per-DSE magnitude data to validate peer-anchor drift < 2×.
- §5.6.9 L1 extensibility constraints — the trace emitter inherits
  these; it must be registry-walking, not name-hardcoded.
- Dave Mark, Curvature tool (GDC AI Summit demos) — the model this
  section generalizes from L2 response curves to every substrate
  layer.

---

## §12 Beliefs, percepts, and memory — scope boundary

Rao & Georgeff's BDI architecture
(`docs/reference/bdi-rao-georgeff.md`) treats **Belief** as a
first-class mental store with formal semantics (belief-accessible
worlds; possible-worlds set consistent with what the agent holds as
true). Clowder does not maintain formal beliefs as a data
structure, and this refactor explicitly does not add one.

This section names the three states Clowder *does* maintain, the
**belief proxies** §7.2's reconsideration gate consumes in place of
formal beliefs, and the scope boundary separating what lives in
this substrate from what lives in the deferred Talk-of-the-Town
belief layer.

### §12.1 The three states Clowder maintains today

- **Percept** — this-frame sensing. `ScoringContext`
  (`src/ai/scoring.rs:27–144`) booleans and scalars, fed by
  `src/systems/sensing.rs`'s four-channel sensory model (sight,
  hearing, scent, tremor). Ephemeral, not retained across frames.
  What the cat senses *right now*.
- **Memory** — `Memory` component
  (`src/components/mental.rs:79`). Rolling buffer of `MemoryEntry`
  (max 20 entries) with per-entry decay (firsthand −0.001/tick,
  secondhand −0.002/tick). Retained across frames but **disconnected
  from scoring today** — memory feeds narrative and colony-knowledge
  promotion, not decision-making. Talk-of-the-Town integration work
  would change this; this refactor does not.
- **Colony knowledge** — per-colony episodic knowledge
  (`src/systems/colony_knowledge.rs:22`). Promoted from individual
  memories when carrier count crosses threshold. Decays over time.
  Used for narrative emission and social transmission — not for
  decision-making.

### §12.2 What a Rao & Georgeff Belief would be

A consistent set of propositions the cat holds as true about the
world, queried by the reconsideration gate to determine whether a
goal is achievable (strong realism, CI1) and whether achievement
has occurred (drop triggers under all three strategies).

Clowder does not have this. Adding it is the Talk-of-the-Town work
(Ryan, Summerville, Mateas, Wardrip-Fruin, "Simulating Character
Knowledge Phenomena in Talk of the Town," *Game AI Pro 3* ch. 37 —
queued in the Reading list), which is a separate epic with its own
design pass and is explicitly outside this substrate's scope (see
§5.5 and the final "What's explicitly out of scope" section).

### §12.3 The belief proxies §7.2 consumes

In lieu of formal beliefs, §7.2's reconsideration gate uses three
proxy signals, each derived from state the substrate already
maintains:

1. **`achievement_believed`** ← current percepts evaluated against
   the Intention's goal predicate.
   Percept ≈ belief for goal-achievement because the cat trusts
   what it senses right now. Examples: hunger level below threshold
   (`Needs::hunger < threshold`), carcass at stores (spatial
   predicate), kitten's hunger sated (percepted kitten-state), ward
   placed (direct tile-state check).
2. **`achievable_believed`** ← two-channel signal, both load-
   bearing per §L2.10.7:
   - (a) DSE re-evaluated score above a retention threshold.
     Spatial reachability contributes via the
     `SpatialConsideration` response curve; as the target becomes
     less reachable (obstacles, distance), the score attenuates
     smoothly and eventually crosses under the retention threshold.
   - (b) `GoapPlan::replan_count < max_replans`
     (`src/components/goap_plan.rs:103`). Hard-fail signal when the
     planner cannot route a step chain after the capped number of
     retries.
   Channel (a) is elastic (smooth degradation per §0.2); channel
   (b) is the hard exit when elasticity is insufficient.
3. **`still_goal`** ← DSE re-evaluation against current context
   above the retention threshold. Below threshold → the cat no
   longer goals this Intention. Only load-bearing under `OpenMinded`
   commitment (§7.3).

### §12.4 Why this is sufficient for L3

Strong realism (Rao & Georgeff CI1) requires "a cat cannot intend
what it doesn't believe achievable." The `achievable_believed`
proxy — (a) + (b) together — covers this for every goal-shaped
Intention: if no plan routes, the `replan_count` cap fires; if the
target becomes unreachable via smooth degradation, the
`SpatialConsideration` attenuation fires; either way the
reconsideration gate drops the Intention under `SingleMinded`.

Activity-shaped Intentions (§L2.10.5) don't need CI1 at all — they
have no goal state to be unachievable. Achievement is termination
(by ticks, condition, or interrupt), not a world-predicate. The
belief-proxy architecture is therefore complete for both Intention
shapes.

What the proxy architecture **cannot** cover:

- **Agent beliefs about other agents.** "Does cat A believe cat B
  will cooperate?" requires a per-relationship belief store.
- **Candidate revision / belief about past events.** "Does cat A
  believe cat B betrayed cat C?" requires evidence typology and
  revision rules.
- **False beliefs.** "Cat A believes mate is alive when mate is
  actually dead" requires distinguishing cat-A's belief store from
  world-ground-truth.
- **Gossip-driven belief propagation.** Related to colony knowledge
  but semantically richer — colony knowledge is "fact X is known
  to the colony"; belief propagation is "cat A's belief about fact
  X shifted because of what cat B told them."

All four are Talk-of-the-Town concerns and deferred to that epic.
The refactor's §5.5 already stakes this position for pairwise
social affinity specifically; §12 restates it for the general
belief-layer question.

### §12.5 Cross-refs

- `docs/reference/bdi-rao-georgeff.md` §1, §2, §9 — BDI formalism
  and crosswalk that motivates this boundary.
- §5.5 — pairwise social affinity deferred to ToT.
- §7.2 — consumer of the three belief proxies.
- §L2.10.7 — plan-cost feedback resolution that provides channel
  (a) of `achievable_believed`.
- `docs/systems/project-vision.md` — magic and social weight are
  ecological phenomena; the belief layer's absence here doesn't
  deny that weight, it just defers its structured representation
  to the ToT epic.

---

## A2 — big-brain evaluation

The `zkat/big-brain` Rust crate provides Bevy utility-AI primitives
(`Scorer`, `Action`, `Picker`, `ProductOfScorers`, `MeasuredScorer`).
Considered as a substrate in place of an in-house implementation.

**Decision: rejected.** Reasoning:

- big-brain is pinned to Bevy 0.16. Clowder is on Bevy 0.18. An open
  PR at codeberg#124 proposes Bevy 0.18 support but is stale as of
  2026-04-20. Adopting a crate stuck on an older Bevy would invert
  our upgrade risk — we'd be gated on an unmaintained upstream.
- big-brain does not ship Mark's response-curve primitives (no
  canonical curve library, no named curve shapes). We'd be
  reimplementing §2 regardless.
- big-brain's `ProductOfScorers` is a useful reference for the
  compensation-factor semantics (§3), but we can borrow the pattern
  without taking a dependency.
- big-brain has no target-as-inner-optimization primitive (§6). We'd
  be building it.
- big-brain doesn't address influence maps (§5) at all.

**What we keep from big-brain:** the semantic model of Scorer /
Action / Picker as separable concerns, borrowed into our own trait
design. `ProductOfScorers` compensation semantics informs §3.

---

## Reading list

Consolidated reading for this substrate, with current status:

**Already extracted (in-repo):**
- `docs/reference/modular_tactical_influence_maps.md` — Mark, *Game
  AI Pro 2* ch. 30 (canonical IAM reference, drives §5)
- `docs/reference/behavioral-math-ch12-response-curves.md` — drives §2
- `docs/reference/behavioral-math-ch13-factor-weighting.md` — drives §3
- `docs/reference/behavioral-math-ch14-modeling-decisions.md` — drives §1, §4, §6, §L2.10.7 (integrated chapter; §"Which Dude to Kill?" closes §L2.10.7 in favor of candidate (a))
- `docs/reference/behavioral-math-ch15-changing-decisions.md` — drives §7 (persistence bonus, event-driven interrupts, information expiry, decision-momentum framing)
- `docs/reference/behavioral-math-ch16-variation.md` — drives §8
- `docs/reference/bdi-rao-georgeff.md` — Rao & Georgeff (1991), "Modeling Rational Agents within a BDI-Architecture," KR 1991. Gameplay-first summary of the BDI formalism. Drives §L2.10.4 Intention framing, §L2.10.5 Goal|Activity split, §L2.10.7 strong-realism grounding, §7 commitment strategies, §7.7 nested-intention aspiration layer, §7.7.1 goal-consistency concurrency bound, §12 belief-proxy boundary. Original PDF remains at `docs/reference/rao91a.pdf`.

**Watched (user recall, transcript unavailable):**
- Dave Mark, "Building a Better Centaur: AI at Massive Scale" —
  GDC 2015. Archive.org video only; GDC Vault auth-walled. Fuses
  IAUS + influence maps at MMO scale. *The talk that prompted this
  substrate work.*
- Jeff Orkin, "Three States and a Plan: The AI of F.E.A.R." — GDC
  2006. Canonical GOAP talk; the goal-shaped Intention framing in
  §L2.10 matches Orkin's goal-selection + planner split.
- Dave Mark, "Winding Road Ahead: Designing Utility AI with Curvature"
  — GDC 2018. Deeper curve treatment than the book.
- Dave Mark, "Spatial Knowledge Representation through Modular
  Scalable Influence Maps" — GDC 2018. Most recent full treatment.
- Tynan Sylvester, "RimWorld: Contrarian, Ridiculous, and Brilliant"
  — GDC 2017. Source of §0's four design principles (story-generator
  framing, elastic failure, apophenia, character-expressive mechanics).
  Not transcribed.

**Queued (not yet in repo, for post-substrate phases):**
- Ryan, Summerville, Mateas, Wardrip-Fruin (2017). "Simulating
  Character Knowledge Phenomena in Talk of the Town." *Game AI Pro
  3* ch. 37. (Belief modeling; unblocks Phase 5–style work.)
  <https://www.gameaipro.com/GameAIPro3/>
- Evans & Short (2014). "Versu — A Simulationist Storytelling
  System." *IEEE TCIAIG*. (Multi-agent practices; BDI-adjacent
  applied game-dev reference.)
- Tarn Adams (2015). "Simulation Principles from Dwarf Fortress."
  *Game AI Pro 2*. (World-generation-as-simulation.)

## Key insights accumulated

Non-obvious synthesis points that should survive re-reads of this
doc:

1. **The refactor is finishing the baseline game, not architecture
   cleanup.** Every capability here unblocks at least one `docs/systems/*.md`
   stub (§10). If a design decision doesn't unblock a stub, question
   it.
2. **Sensory-channel attenuation on stamp *read*** is a
   Clowder-specific IAM extension that Mark's chapter doesn't cover
   (§5.2). Resolves `sensory.md`'s "multipliers pinned at 1.0" and
   connects Body Zones to the substrate in one move.
3. **Personal-interest template is a spatial consideration**, not a
   separate primitive (§1, §6). The trait in §1 must accept positional
   inputs from the start; bolting it on later means re-shaping.
4. **ECS markers = Mark context tags = DSE filters**, three
   vocabularies for one concept (§4). The A3-style context-tag
   refactor and the pure-Bevy-idiom refactor are the same refactor,
   not two.
5. **Pairwise social affinity belongs to the ToT belief layer**, not
   to influence maps (§5.5). Treating it as a base map would explode
   storage and still wouldn't capture asymmetry. This is explicit
   scope discipline for this substrate.
6. **`Needs::level_suppression` generalizes cleanly as a
   hierarchical pre-gate** above the IAUS layer (§3). Maslow is not
   folded into the axis product — it wraps the product.
7. **big-brain is out** on Bevy-version grounds (A2 note). In-house
   IAUS, borrowing `ProductOfScorers` semantics as reference.
8. **Today's `src/ai/scoring.rs` is evidence, not specification.** The
   current action boundaries, 57 tuning constants, 5 post-scoring
   modifiers, ScoringContext field layout, 5 ad-hoc non-linearities, and
   21-action set are all evidence of what was tried under the old
   architecture — tuning patches against observed behavior given the old
   substrate's constraints, not first-principles design against the
   right substrate. The refactor is free to reshape, collapse, split,
   dissolve, or replace any piece. The only real constraints are the
   semantic goals (the behavioral arcs from the 21 current actions + 12
   aspirational stubs in §10) and the behavioral baseline (canary metrics
   per `CLAUDE.md`'s Balance Methodology). When L2 design encounters a
   shape from today's scoring, ask *"does the new substrate make this
   unnecessary?"* before *"what primitive reproduces it?"* Applies equally
   to the L1 enumeration in §5.6 — those tables are current needs, not a
   closed contract (§5.6.9).
9. **Target-existence collapse is worse than prior spec said** (§6.1).
   Thirteen target-ish booleans in `ScoringContext`, four of them
   critical (`Socialize`, `Mate`, `Mentor`, `Groom`). The GOAP/
   Disposition resolvers do rich per-target ranking that the scoring
   layer is blind to, and — worse — the two resolvers *disagree*
   (`disposition.rs` uses fondness + novelty, `goap.rs` uses fondness
   only). One `TargetTakingDse` (§6.3) owns target quality; both
   execution paths consume the same result. Fixes a class of bugs the
   prior spec didn't flag.
10. **Composition isn't one formula.** Compensated product, weighted
    sum, and max all have legitimate use cases (§3.1) per Mark ch 13
    §"Weighted Sums" and the existing 21-DSE catalog. Forcing
    multiplicative everywhere would regress ~8 of 21 actions. The
    prior spec's "pseudocode" implied single-formula; real answer is
    per-DSE selection from three named modes.
11. **DSE output is an Intention, not an Action** (§L2.10.4). This is
    the framing that collapses multiple problems at once:
    companion-action bundling (resolver back-infers step chains from
    disposition labels), sub-mode max-selection (Herbcraft / Magic
    parent actions with internal `max()`), and GOAP/Disposition
    resolver divergence (§6.2) all dissolve under Intention-emitting
    DSEs. Dispositions stay as Clowder's BDI Intention layer (Rao &
    Georgeff), GOAP plans from `Intention::Goal`, and softmax-over-
    Intentions gives variation in intent, determinism in execution.
12. **`Intention = Goal | Activity` is a Clowder-specific split**
    (§L2.10.5). Classical BDI assumes every Intention reduces to a
    goal state. Clowder has sustained-activity DSEs (`Socialize`,
    `Wander`, `Idle`, `Patrol`) that don't — "do this for a while"
    doesn't want a threshold goal. Two execution paths (GOAP for
    `Goal`, activity runner for `Activity`) is the cost; honest
    shape-matching is the benefit. Flagged explicitly so future
    readers don't assume pure-BDI.
13. **Plan-cost feedback folds into scoring via `SpatialConsideration`,
    not a pathfinder handshake** (§L2.10.7). Mark ch 14 §"Which Dude
    to Kill?" treats distance as a scoring input with a response
    curve (Figures 14.6–14.8), not as a separate cost-query channel.
    Clowder matches: each spatially-sensitive DSE carries a
    `SpatialConsideration` with a `Quadratic` / `Power` / `Logistic`
    curve; elastic score-attenuation (§0.2) handles reachability
    degradation; `GoapPlan::replan_count ≥ max_replans` remains the
    hard-fail exit for genuine unplannability. Candidate (b)
    pathfinder-in-the-loop is rejected — chicken-and-egg under §7
    commitment strategies, expensive, and §0.2-hostile.
14. **The simulation is the director is the player** (§0.1). In
    conventional game design, player / director / simulation are three
    distinct roles; Clowder collapses all three into the simulation
    itself. Cats are the actors, ecology is the director, the human
    observes from outside. A RimWorld-style director is structurally
    absent — not rejected by preference — because the player-skill
    target it calibrates against is absent. The AI substrate is
    therefore *the product*, not a system supporting a product.
15. **Elastic failure is a substrate-level constraint** (§0.2). The
    compensation factor (§3.2), open-minded commitment (§7), target
    re-ranking on loss (§6), slow-state decay (§5.3), and the
    GOAP-can't-plan path in §L2.10.7 are all instances of one
    principle: consequence-rich failure that propagates, never
    arc-terminating failure. Today's spec had each mechanism
    independently motivated; §0.2 names them as one family so future
    design choices don't quietly re-introduce brittle shapes.
16. **Apophenia has two legs** (§0.3). *Abstracted feedback* — the sim
    presents "what happened," not "why"; the observer does the
    attribution. *Long-term relevance* — patterns form only if state
    persists and compounds across time (memory, aspirations, bonds,
    injury, skill). The substrate's job is to provide *space* for
    observer pattern-matching: legible primitives (Intentions, named
    considerations, event logs) on one side, persistent long-horizon
    state on the other. A storyteller that narrates causation kills
    apophenia because the inference is gone.
17. **Mechanics must express character, not just apply modifiers**
    (§0.4). Filter for every future mechanic proposal: "would the sim
    tell a different story about this cat if this value were
    different?" Armor-as-class-expression passes; armor-as-stat-buff
    doesn't. Catches character-inert designs (consumables, numeric
    upgrades, expressive-content-free level gates) before they consume
    apophenia budget. Clowder's existing ethos (personality shapes
    scoring, magic affinity gates becoming, skills shift behavioral
    preferences, aspirations are lifetime arcs) is already aligned;
    §0.4 names the filter so future work preserves that alignment —
    especially for the §10 queue (Disease, Recreation, Substances,
    Mental Breaks), where stat-buff framings are the path of least
    resistance.
18. **Strong realism via plannability + score attenuation gives us
    CI1 without formal beliefs** (§7.2, §12, §L2.10.7). Rao &
    Georgeff's "an agent cannot intend what it doesn't believe
    achievable" is the one BDI axiom with direct gameplay teeth. We
    satisfy it with two existing signals: `SpatialConsideration` score
    attenuation (elastic per §0.2) plus `GoapPlan::replan_count`
    hard-fail (the brittleness backstop). No Belief data structure,
    no pathfinder-in-the-loop (that's the rejected §L2.10.7
    candidate (b)), no chicken-and-egg. The full Talk-of-the-Town
    belief layer remains deferred (§12) — L3 commitment does not
    require it.
19. **Persistence bonus lives at the commitment layer; task-progress
    marginal utility lives in the DSE** (§7.4, §7.8). Both mechanisms
    come from Mark ch 15 — bowling-ball decision momentum and the
    Finish Him! / Just Gimme a Minute framing — but they homo-
    morphically live in two different places. The commitment-layer
    bonus is generic (Logistic on task-completion-fraction, applied
    once during re-evaluation to the active Intention). The DSE-layer
    marginal utility is instance-specific (Build's "percent-complete"
    axis; Farm's "crop maturity" axis). Conflating them produces
    either commitment-layer code that leaks DSE internals or per-DSE
    code that duplicates the commitment gate.
20. **Midlife crisis is a nested-Intention OpenMinded drop fired by
    event** (§7.7). Aspirations are a separate commitment layer from
    per-tick Intentions. Per-tick defaults SingleMinded (flipper
    prevention); aspirations default OpenMinded because grief, fate,
    mood drift, and life-stage transitions *should* redirect
    multi-year arcs — that's character, not a bug. Same Rao &
    Georgeff vocabulary at a second timescale, with event-driven
    reconsideration (not per-tick — per-tick reconsideration of
    multi-year arcs *is* the strobing failure at a different
    timescale). Connects directly to §0.3's long-term-relevance leg
    of apophenia and §0.4's character-expression filter.
21. **Concurrent-aspiration cap is consistency, not a number**
    (§7.7.1). A cat holds as many aspirations as remain mutually
    consistent — Rao & Georgeff's goal-consistency axiom (CI1/CI2)
    applied directly. Hard-logical and hard-identity pairs are
    declared incompatible in a sparse authoring-time matrix;
    soft-resource and soft-emotional tensions are allowed and often
    desirable (character expression). Reconsideration events drop
    aspirations one at a time, so grief over a kitten's death can
    redirect combat-mastery while leaving herb-tending intact. One
    arc redirecting doesn't cascade the cat's whole identity.
22. **Softmax-over-all is Mark ch 16's closing recommendation in
    exponential-weight clothing** (§8.1). Ch 16's progression goes
    argmax → random-top-n → weighted-random-top-n →
    weighted-random-from-all; only the last survives Mark's own
    critique of arbitrary cutoffs. Clowder's existing
    softmax-over-all matches that topology — the departure is
    exponential weighting instead of Mark's linear
    `TotalScore / ThisScore`, justified by §1.3's strict `[0, 1]`
    score normalization (keeps `exp()` stable) and by softmax's
    single-knob temperature collapsing Mark's per-step coefficient
    + curve + rescale machinery into one continuous parameter. The
    decision to keep softmax is a topology match, not a departure
    — and the temperature bound is two-sided, apophenia-anchored:
    too cold reads as Stepford, too hot dissolves character at the
    day scale (§0.3's two legs, §8.3, §8.6). This also names the
    fox argmax-plus-jitter path as a *Key insight #8* artifact
    converging onto the unified softmax under §8.5.

---

## What's explicitly out of scope

This spec does not contain:
- **Implementation phasing.** No Phase 1 / Phase 2 / etc. numbering.
- **Agent-team fan-out.** No parallelization map.
- **Execution sequence.** No "do this, then this, then this" list.
- **Per-phase verification protocols.** Canary mapping, baseline
  archiving, A/B expectations — all deferred to the implementation
  plan that follows this spec.
- **Feature specs for unblocked stubs.** Disease, Mental Breaks,
  Sleep-That-Makes-Sense, etc. each need their own design pass
  against this substrate. §10 lists what each unblocks; none of
  them are specified here.
- **The ToT belief layer.** Explicitly deferred. Pairwise social
  perception, mental models, evidence typology, candidate
  revision — next epic, not this one.

The framing correction behind these exclusions: *design the output,
not the project.* Phasing before the end-state is well-specified is
exactly how this thread needed a re-plan.
