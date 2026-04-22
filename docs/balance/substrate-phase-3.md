# AI Substrate Refactor — Phase 3 (L2 core: curves + composition + markers + faction)

**Status:** Phases 3a + 3b landed — reference DSE live, evaluator
registered as a Bevy Resource. Phase 3c (wholesale fan-out of the
remaining 29 DSEs + sibling splits) is the next wave.

| Sub-phase | Scope | Status |
|---|---|---|
| 3a — scaffolding | Types, curves, composition, markers, faction | **landed** |
| 3b.1 — evaluator plumbing | `DseRegistry` resource + `evaluate_single` + modifier pipeline + `DseRegistryAppExt` | **landed** |
| 3b.2 — Eat reference port | `EatDse` registered in plugin + headless | **landed** |
| 3c — fan-out port | 29 remaining DSEs + sibling splits + Mate L3; deletes `score_actions` inline blocks | pending |
| 3d — faction matrix + roster gap-fill | Authoring systems for §4.6 markers | pending |

## Thesis

§1–§4, §9, §L2.10 of `docs/systems/ai-substrate-refactor.md` replace
`src/ai/scoring.rs`'s 21 hand-authored action blocks and
`src/ai/fox_scoring.rs`'s parallel 9 fox dispositions with a single
unified evaluator. The end-state is 30 registered DSEs (plus sibling
splits — Herbcraft × 3, PracticeMagic × 6) reading through:

- **§2** named response-curve primitives (Logistic, Quadratic,
  Piecewise, …) — the substrate of the "hangry is a threshold not a
  ramp" correction.
- **§3** three composition modes (CompensatedProduct / WeightedSum /
  Max-retiring) with compensation factor (§3.2) and RtM/RtEO
  weight-mode invariants (§3.3.1).
- **§4** ECS marker components as first-class eligibility filters.
- **§9** faction stance matrix + overlay resolver for target-taking
  DSE filter bindings.

Phase 3 is *the* critical phase per the refactor plan: "whole L2
substrate lands as one unit so each DSE reaches the new evaluator
with its proper curve + composition mode at the same time — no
interim state where a DSE has been switched over but still uses
flat-preference WeightedSum."

## Hypothesis

> Replacing linear-always response curves with named-curve
> primitives + compensated product composition + the post-scoring
> modifier pipeline will produce directional shifts in DSE firing
> frequency that **surface higher-Maslow behaviors** (Mating,
> Crafting, PracticeMagic sub-modes, Farming, Build, Mentor) that
> today's substrate starves. Survival canaries hold; continuity
> canaries strengthen.

## Predicted drift direction (seed-42 `--duration 900` release soak)

Per the refactor plan's Phase 3 exit criteria. Final targets are
the refactor-level gate — Phase 3 must show they're *reachable*,
Phases 4–6 close any gap.

| Metric | Baseline | Phase 3 prediction |
|---|---|---|
| `Farming` fires | 0 | ≥1 (zero-to-nonzero transition — substrate dormancy confirmed as cause) |
| PracticeMagic sub-modes (Scry, DurableWard, Cleanse, ColonyCleanse, Harvest, Commune) firing at ≥1× | ~0 sub-modes | ≥3 sub-modes |
| Mating Intentions adopted | ~0 | ≥1 per colony per season when fertility windows open |
| Crafting Intentions adopted + held via `SingleMinded` commitment | sparse | recipes progress to completion |
| Mentor, Build frequency | low | rises above baseline |
| Starvation canary | 0 | 0 (hard gate) |
| ShadowFoxAmbush canary | ≤5 | ≤5 (hard gate) |
| Wipeout canary (day 180 on seed 42) | survives | survives (hard gate) |

Per-DSE frame-diff against the pre-substrate baseline
(`logs/baseline-pre-substrate-refactor/`) must show drift *in the
predicted direction* and *within rough magnitude* (four-artifact
rule). Wrong-direction drift is a rejection; magnitude > 2× off
triggers second-order investigation before acceptance.

## Per-DSE hypothesis table

The table below commits one row per DSE — to be filled in at each
port's landing commit. Seeded from §2.3 + §3.1.1 assignment tables;
predictions are the sub-agent's job per the refactor plan's Phase 3c
fan-out strategy.

### Cat DSEs — Tier 1

| DSE | Composition | Curve(s) | Prediction | Landing commit |
|---|---|---|---|---|
| Eat | CP | Logistic(8, 0.75) hunger | Starvation unchanged; firing threshold sharper at 0.75 midpoint | 3b |
| Sleep | WS | Logistic(10, 0.7) energy + Piecewise day-phase + Linear injury | Sleep firing rises in the 0.3–0.5 band; falls in the 0.6–0.9 band | 3c |
| Hunt | WS | Logistic(8, 0.75) hunger + Quadratic(2) scarcity + Linear boldness + Spatial prey | Hunt responsiveness to prey-proximity rises; bold-cat-on-full-stomach path opens | 3c |
| Forage | WS | Hunger + scarcity + Linear diligence | — | 3c |
| Groom (self) | CP (sibling) | Logistic(7, 0.6) thermal + Logistic(5, 0.6) affection | Retires Max-composed parent | 3c |
| Flee | CP | Logistic(10, threshold) safety + inverted Linear boldness | Fewer spurious flees in marginal-threat scenarios; sharper response near threshold | 3c |

### Cat DSEs — Tier 2

| DSE | Composition | Curve(s) | Prediction | Landing commit |
|---|---|---|---|---|
| Fight | WS | Linear boldness + combat + Piecewise health + Piecewise safety + saturating ally_count | Group-courage signal stronger; low-boldness cat with allies fights instead of fleeing | 3c |
| Patrol | CP | Linear boldness + Logistic(6, threshold) safety-deficit | — | 3c |
| Build | WS | Linear diligence + Piecewise site + Piecewise repair | Build frequency rises on respect-low cats via Pride modifier | 3c |
| Farm | CP | Quadratic(2) scarcity + Linear diligence | **First-ever fire** on seed 42 | 3c |
| Socialize | WS | Logistic(5, 0.6) social + Linear sociability + inverted Logistic phys_sat + Linear temper + Linear playfulness + Logistic(8, 0.1) corruption | — | 3c |

### Cat DSEs — Tier 2–5

| DSE | Composition | Curve(s) | Prediction | Landing commit |
|---|---|---|---|---|
| Explore | CP | Linear curiosity + Linear unexplored | Re-evaluated post-3c for the Explore-saturation sub-task from `open-work.md #1 sub-2` | 3c |
| Wander | WS | Linear curiosity + Linear base + Linear playfulness | — | 3c |
| Cook | WS | Linear base + Quadratic(2) scarcity + Linear diligence | — | 3c |
| Coordinate | WS | Linear diligence + saturating directive_count + Linear ambition | — | 3c |
| Mentor | WS | Linear warmth + Linear diligence + Linear ambition | Frequency rises per open-work #3 hypothesis | 3c |
| Caretake | WS | Linear urgency + Linear compassion + Piecewise parent | — | 3c |
| Idle | WS | Linear base + Linear incuriosity + Linear playfulness, floor-clamp | — | 3c |

### Herbcraft / PracticeMagic sibling DSEs (§L2.10.10)

| Sibling DSE | Intention shape | Prediction | Landing commit |
|---|---|---|---|
| `herbs_in_inventory` | Goal | Fires when herbs scarce + gather viable | 3c |
| `remedy_applied` | Goal + target-taking | Fires when allies injured | 3c |
| `ward_placed` | Goal + target-taking | Fires when wards weak + thornbriar available | 3c |
| `scry` | Activity — Calling integration point | Fires on ≥1× soak | 3c |
| `durable_ward` | Goal + target-taking | Fires on corruption spike | 3c |
| `cleanse` | Goal + target-taking | Fires on adjacent corruption | 3c |
| `colony_cleanse` | Goal | Fires on territory-wide corruption | 3c |
| `harvest` | Goal + target-taking | Fires when carcasses present + herbcraft skill | 3c |
| `commune` | Activity — special-terrain gate | Fires on special-terrain step | 3c |

### Mating (three-layer §7.M)

| Layer | DSE | Landing |
|---|---|---|
| L1 | `reproduce_aspiration_dse` | Phase 5 (aspiration catalog) |
| L2 | `pairing_activity_dse` | Phase 4 (target selection adds proximity bias) |
| L3 | `mate_with_goal_dse` + `Fertility` component | 3c |

### Fox DSEs

9 fox dispositions (Hunting, Raiding, Resting, Fleeing, Patrolling,
Avoiding, Feeding, DenDefense, Dispersing) port one-to-one per §3.1.1
+ §2.3 fox tables. Predictions committed at 3c landing.

## Canaries under this phase

### Hard gates (must pass)

- `Starvation` deaths = 0 on seed-42 `--duration 900`.
- `ShadowFoxAmbush` deaths ≤ 5.
- No wipeout before day 180.
- `just ci` green.

### Soft gates (continuity — must strengthen, not regress)

- Grooming, play, mentoring, burial, courtship each fire ≥ 1× per soak.
- Mythic texture: ≥ 1 named event per sim year.
- Generational continuity: ≥ 1 kitten reaches adulthood.

### Novel Phase 3 gates

- **Farming fires ≥ 1×** — the zero-to-nonzero transition is the
  load-bearing signal that substrate dormancy (not missing system)
  was the cause. If Farming still fires 0×, the refactor hypothesis
  is refuted on this axis and Phase 3 re-iterates.
- **At least 3/5 PracticeMagic sub-modes fire** — sibling-DSE split
  working.
- **Mating Intentions adopted via `SingleMinded`** — Intention
  framework integrated with commitment layer.

## Acceptance gate

Phase 3 exits when all of:

1. 21 cat DSEs + 9 fox DSEs + 9 Herbcraft/PracticeMagic siblings
   registered through the unified evaluator; `scoring.rs` +
   `fox_scoring.rs` action blocks deleted.
2. Hard gates pass (survival canaries hold).
3. Continuity canaries strengthen (improvement, not non-regression).
4. Positive-exit motion: Farming ≥1×, ≥3/5 PracticeMagic sub-modes,
   Mating and Crafting above baseline.
5. Per-DSE frame-diff matches hypothesis-table direction.
6. §9 faction matrix loaded; 5 DSE-filter bindings resolved correctly.
7. `Fertility` component emits phase transitions consistent with §7.M.7.2.

## Phases 3a + 3b deliverables landed

| Commit | Scope |
|---|---|
| `03e9b23` | L2 primitives — `curves.rs` (7 primitives + named anchors), `composition.rs` (3 modes + compensation), `considerations.rs` (Consideration trait + 3 flavors). 40 tests. |
| `01cb6e7` | `Dse` trait + `Intention` enum (Goal / Activity + CommitmentStrategy tag) + `Termination` + `EvalCtx` skeleton + `EligibilityFilter`; `FactionStance` stub + `StanceRequirement`. 7 tests. |
| `e02121f` | §4 marker catalog — 49 new ZST components across 11 categories (LifeStage, State, Capability, Inventory, TargetExistence, Colony, Reproduction, Fox-specific, §9.2 faction overlays). 10 queryability tests. |
| `1a50d30` | §9 faction model — flattened `FactionSpecies`, `FactionRelations` resource with 100-cell §9.1 matrix, `StanceOverlays` + `resolve_stance` most-negative-wins resolver, 4 `StanceRequirement` factory helpers. 26 tests. |
| `d9cf47e` | Phase 3b.1 evaluator — `DseRegistry` + `ModifierPipeline` + `ScoreModifier` trait + 6-method `DseRegistryAppExt`. `evaluate_single` runs eligibility → consideration scoring → composition → Maslow pre-gate → modifier pipeline → Intention emit. 9 tests. |
| `afe22f5` | Phase 3b.2 `EatDse` reference port — single hunger consideration through `Logistic(8, 0.75)`, CP composition, Maslow tier 1, Goal Intention with SingleMinded commitment. Registered in both `SimulationPlugin::build` and headless `build_new_world` + `setup_world` save-load path. 10 tests. |

**Total Phases 3a + 3b test coverage:** 102 unit tests on new primitives.

## Phase 3b ↔ 3c boundary

Phase 3b.2 registers `EatDse` but does **not** yet replace
`score_actions`'s inline Eat arithmetic. The refactor plan's "action
blocks delete as each new DSE lands" wave is Phase 3c — 3b.2
establishes the pattern; 3c's fan-out port replaces all formulas
together so `ScoringContext`'s 9 test-site constructors update once
rather than incrementally.

No behavior drift lands in production yet — `EatDse`'s score is
computed on demand but not consumed. Drift lands with Phase 3c's
wholesale port when the inline blocks delete.

## Phase 3a → 3b boundary (landed)

The refactor plan originally scoped the registration builder + unified
`evaluate()` + post-scoring modifier pipeline as Phase 3a deliverables.
Landing those without a consumer (no DSE registers yet) would force
speculative design decisions about `EvalCtx` shape, marker-query
dispatch, and modifier pass ordering that can only be validated against
a concrete port.

**Decision (landed):** the evaluator + registration builder (commit
`d9cf47e`) and the Eat reference port (commit `afe22f5`) ship as
Phase 3b, immediately after Phase 3a's type foundation. This
respected the refactor plan's discipline while adjusting the sub-
commit decomposition so each commit has a clean test boundary.

## Observation

*(to be filled in after Phase 3d lands)*

## Concordance

*(to be filled in after final phase-exit soak)*

## Cross-refs

- `docs/systems/refactor-plan.md` Phase 3 structure.
- `docs/systems/ai-substrate-refactor.md` §1 considerations,
  §2 curves, §3 composition + modifiers, §4 markers, §9 faction,
  §L2.10 DSE catalog + registration.
- `docs/balance/substrate-phase-1.md` — instrumentation scaffold.
- `docs/balance/substrate-phase-2.md` — L1 influence-map substrate.
- `docs/balance/substrate-refactor-baseline.md` — pre-refactor
  baseline soak, frame-diff target.
