# AI Substrate Refactor — Implementation Plan

> **Status:** plan of record. Post-planning-session (2026-04-21); draft
> framing retired. Changes from the draft are logged under
> "Decisions log" below.

## Context

The spec at `docs/systems/ai-substrate-refactor.md` (~6,450 lines, second-pass
stable as of 2026-04-21) defines the end-state for Clowder's AI substrate: a
three-layer architecture replacing today's hand-authored `src/ai/scoring.rs`
(2,817 lines), its parallel `src/ai/fox_scoring.rs` (479 lines), and ~8 other
scoring sites scattered across coordination, aspirations, disposition,
GOAP, narrative, and mate-selection code.

**Three architectural layers:**

- **L1 (Perception)** — §5 influence maps with species × role × injury ×
  environment attenuation. Generalizes today's de-facto scent IAM in
  `wind.rs` / `sensing.rs` into a reusable `InfluenceMap` abstraction
  serving 13 named maps (currently 0 Built, 5 Partial, 8 Absent).
- **L2 (Deliberation)** — §1–§6 + §L2.10. A `Dse` trait with response
  curves (§2), multi-consideration composition (§3), context-tag markers
  (§4), target-selection as inner optimization (§6), and the Intention
  output shape (§L2.10.4). 45 DSEs register through a unified surface
  replacing 10+ scattered scoring sites.
- **L3 (Commitment + Variation)** — §7 + §8. `CommitmentStrategy` enum
  (Blind / SingleMinded / OpenMinded) with drop-trigger gates, persistence
  bonus, softmax-over-Intentions (T = 0.15), and §7.W's `Fulfillment`
  scalar — a new register that unifies The Calling, the warmth split, and
  compulsion-shaped axis-capture into one primitive.

**Spec status (post-2026-04-21 pass):** §1–§9, §L2.10, §11, §12 all closed
in-scope. §7 closed. §7.M mating showcase + §7.M.7 fertility spec committed.
§7.W axis-capture / fulfillment primitive committed. §8 synthesis committed
(T = 0.15 default, softmax-over-Intentions, retention-with-bonus ordering).
The spec **explicitly excludes** phasing, agent fan-out, execution sequence,
and per-phase verification protocols — this plan fills that gap.

**Carried forward from §12 scope boundary:**
- No formal Belief layer. Memory stays disconnected from scoring.
- Per-cat beliefs about other cats, false beliefs, gossip-driven belief
  propagation — all deferred to the ToT epic (separate future effort).
- Pairwise social affinity stays where it is (§5.5 deferral).

**Calendar framing:** phases are cut around **invariant gates** (baseline
archive, shadow-mode parity, modifier layer, target layer, L3 with
fulfillment, cleanup), not calendar weeks. Each phase is a landable unit
whenever its gate clears; no phase assumes calendar separation from the
next. Pre-flight gates are same-session work, not weeks of prep.

**Parallel presenter-layer tracks (not blocking the substrate):**
- `open-work.md` #10 — post-death biographies (Claude API presenter, rank
  1024). Strict-presenter contract: reads finalized `events.jsonl`, writes
  `logs/biographies/*.md`, sim never reads back. Landable anytime.
- `open-work.md` #11 — cat-conversation rendering (Haiku over C3). Gated
  on A1 + A3 + C3 + #10; earliest landing is after Phase 5. Flagged so
  Intention output shape (§L2.10.4) stays presenter-readable per §0.3.

Acceptance is not "new code compiles" and not "behavior matches old
scoring bitwise." The whole point is that **behavior diverges in a
predicted direction**. The current sim's linear-always response curves
produce the flat-preference problem: every axis contributes equally
everywhere, context can't discriminate sharply enough to lift higher-
Maslow actions above survival-tier actions when survival is adequate but
not urgent. That's why higher-Maslow dispositions are dormant today; the
curve library + compensated product + commitment layer is the predicted
fix.

## Load-bearing refactor hypothesis

> Replacing linear-always response curves with named-curve primitives +
> compensated composition + the post-scoring modifier stack + commitment
> strategies + the §7.W fulfillment register will **surface higher-Maslow
> behaviors that are currently dormant or near-dormant** (Mating,
> Crafting, PracticeMagic sub-modes, Farming, Build, Mentor, Aspire)
> without regressing survival canaries, *and* produce legible
> warring-self and axis-capture narratives (The Calling, sadist-play,
> compulsion shapes) as free consequences of the unification.

Dormancy today is a substrate symptom — flat-preference linear curves and
compensation-less composition never give these DSEs enough headroom to
beat the always-warm survival-tier actions. Axis-capture phenomena today
don't exist because the substrate has no fulfillment register to sensitize
on.

## Decisions log

Changes from the pre-planning draft (2026-04-21 planning session,
verified against HEAD state):

- **Pre-flight gate count: 6 (was 7).** The draft's gate 5
  (retired-constants burn) moved to **Phase 3c**, landing in the same
  commit as the Logistic-curve DSEs that subsume those constants.
  Rationale: verification confirmed the eight §2.3 retired fields are
  actively read today —
  - `incapacitated_eat_urgency_{scale,offset}`,
    `incapacitated_sleep_urgency_{scale,offset}`,
    `incapacitated_idle_score` feed the `if ctx.is_incapacitated`
    early-return block at `src/ai/scoring.rs:182–201`.
  - `ward_corruption_emergency_bonus` (sim_constants.rs:1127 + :1267),
    `cleanse_corruption_emergency_bonus` (:1129 + :1268),
    `corruption_sensed_response_bonus` (:1139 + :1274) are live
    corruption-response axes.
  Burning them pre-flight would move the baseline soak away from
  current-tip behavior and defeat its purpose as the frame-diff
  reference. §13.1's "dangerous before Logistic curves" contract
  supersedes the original gate-5 framing. The enumeration-ledger row
  (below) reflects the new landing phase.
- **Pre-flight gate 2 (activation-1): park, not fix.** Founder
  `start_tick` / end-of-life-spawn regression handled as a standalone
  balance iteration post-Phase-7. Baseline soak inherits the
  founder-age wipeout tendency with that inheritance flagged
  explicitly in `docs/balance/substrate-refactor-baseline.md`. Every
  frame-diff run against the baseline must account for that noise
  until a re-baseline lands.
- **Pre-flight gate 4 (warmth-split Phase 2): two commits.** Field
  rename (`needs.warmth` → `needs.temperature`) ships separately from
  the 46-identifier constants rename in `sim_constants.rs`. Bisect
  granularity — if a field-rename bug slips past compile, the
  constants commit stays clean.
- **Pre-flight gate 5 wording (was gate 6 in draft):**
  `fox_softmax_temperature` is a *new* field added to
  `ScoringConstants` at value 0.15, not a constant that already
  exists. Draft phrasing implied otherwise.
- **Draft arithmetic error:** the draft said "seven fields" for §2.3
  retired constants then listed eight. The correct count is eight;
  the gate is no longer pre-flight, so the count now only matters at
  Phase 3c.

## Dormancy baseline (documented pre-refactor)

- **Farming — never fires** on seed-42 `--duration 900`. Zero is the
  baseline.
- **Mating — gate-starved** at 0% of snapshots under seed-42 wider-range
  treatment (per `docs/balance/social-target-range.report.md`, commit
  `290a5d9`). `has_eligible_mate:111` hard-gates the DSE on binary partner
  presence; when no `Partners+` bond exists, Mate is not a candidate.
- **Crafting — rarely fires.** Recipe state exists, DSE exists, but linear
  composition starves it.
- **PracticeMagic sub-modes (Scry, DurableWard, Cleanse, Commune,
  Harvest) — rarely fire.** The parent-action `max()`-selection collapses
  variance; §L2.10.4's sibling-DSE split under the Intention framing is
  what breaks this open.
- **The Calling — implementable but not yet live.** Design landed
  (`docs/systems/the-calling.md`); canonical axis-capture instance; awaits
  Phase 6's `Fulfillment` register.
- **Axis-capture narratives** (compulsion, sadist-play, devotion-as-
  mastery, Dark Callings) — not expressible; no fulfillment register,
  no warring-self telemetry.

## Positive exit criteria (refactor-level, not per-phase)

Final seed-42 `--duration 900` soak must show all of:

1. **Farming fires ≥1×** — zero-to-nonzero transition demonstrating
   substrate dormancy was the cause, not an absent system.
2. **Mating fires meaningfully** — courtship arcs complete; `Pairing`
   Activity sustains across multi-season bonds; `MateWithGoal` fires ≥3×;
   ≥2 surviving kittens per starter colony. Target counts fixed in
   `docs/balance/substrate-phase-N.md` at each phase entry.
3. **Crafting fires meaningfully** — recipes progress to completion;
   `Crafting` Intentions adopted and held via §7 SingleMinded commitment.
4. **PracticeMagic sub-modes fire diversely** — Scry, DurableWard,
   Cleanse, Commune, Harvest each ≥1× per soak. §L2.10.4's sibling-DSE
   split is the mechanism.
5. **Build + Mentor + Aspire frequency rises** vs. baseline.
6. **The Calling fires** — ≥1 successful Named-Object creation per
   sim year; Dark Calling capacity exists but triggers only on corruption
   paths.
7. **Warring-self signal observable** — `CatSnapshot.last_scores` shows
   ≥1 documented compulsion-shape (narrow winning axis + active losing
   counter-axis + mood valence drop) per soak. Narrative emitter can
   bind to it.
8. **Survival canaries hold** (hard gate).
9. **Continuity canaries strengthen** — grooming, play, mentoring,
   burial, courtship, mythic texture all fire ≥1× per soak.

Targets for #2 are set in Phase 1's baseline archive; targets for #6, #7
are set at Phase 6 entry once Fulfillment register lands.

## Acceptance, per phase and overall

1. **Survival canaries hold** — hard gate, unchanged. `Starvation = 0`,
   `ShadowFoxAmbush ≤ 5`, no wipeout on seed-42 `--duration 900`.
   Measured via `just check-canaries`.
2. **Continuity canaries strengthen, not regress** — grooming, play,
   mentoring, burial, courtship, mythic texture all fire ≥1× per soak.
   Phases 3–4 are expected to *close* some of these gaps.
3. **Directional concordance with the refactor hypothesis** — per-DSE
   frame-diff against pre-refactor baseline shows drift *in the predicted
   direction* and *within rough magnitude* (four-artifact rule:
   hypothesis / prediction / observation / concordance). Drift in the
   wrong direction is a rejection; drift > 2× predicted magnitude is an
   investigation before acceptance.
4. **Unblocks §10 feature queue** — at the end, Disease, Mental Breaks,
   Environmental Quality, Sensory, Recreation, Sleep, fox/hawk/
   shadowfox AI parity, Trade & Visitors, Organized Raids, Substances
   are all *addable* without further substrate work.

Frame-diff is a **signal, not a gate**. It tells us *which* consideration
/ curve / modifier drove the shift. The gate is the canary pair (hard)
+ directional concordance (soft, documented per phase).

## Pre-flight gates — clear these before Phase 1

Six gates. The refactor's baseline must be stable before the first
line of L2 code lands, or every A/B comparison will be noisy.

> Retired-constants burn (was gate 5 in the draft) moved to **Phase 3c**,
> landing with the Logistic curves that subsume them. See Decisions
> log above for rationale.

1. **Park `docs/open-work.md` follow-on #1 (Explore dominance).**
   Both sub-1 (social-target-range iter 3) and sub-2 (Explore
   saturation curve) are verified unresolved but outside the
   refactor's blast radius. Park both with pointer notes; sub-1 is
   superseded by Phase 4 target-selection and sub-2 is re-evaluated
   post-Phase-3c when Explore runs through the unified evaluator.
2. **Park `activation-1` (founder-age regression)** with a Blocked-by
   note. Root cause: `start_tick = 60 × ticks_per_season` spawns
   founders near end-of-life, 15/15 colonies wipe before day 180.
   Resolution is a standalone balance iteration post-refactor;
   baseline soak (gate 6 below) inherits the wipeout tendency with
   that inheritance flagged in
   `docs/balance/substrate-refactor-baseline.md`.
3. **Fix the 3 failing integration tests** (`cats_eat_when_hungry`,
   `simulation_is_deterministic`, `simulation_runs_1000_ticks_without_panic`).
   Red tests are toxic to a refactor of this size. Verified root
   cause: `ColonyCenter` resource is inserted by `build_new_world`
   (`src/main.rs:1034`) but not by `tests/integration.rs::setup_world`;
   `accumulate_build_pressure` (coordination.rs:202) and
   `spawn_construction_sites` (coordination.rs:1067) both require it.
   Fix: one-line `world.insert_resource(ColonyCenter(colony_site))`
   in `setup_world` after the existing `colony_site` computation.
4. **Land `needs.warmth` → `temperature` rename (Phase 2 of
   `docs/systems/warmth-split.md`).** Two commits for bisect
   granularity: commit A renames the `Needs.warmth` field across 30
   call sites in 11 files; commit B renames the 46 `*_warmth_*`
   identifiers in `src/resources/sim_constants.rs`. Verification:
   byte-identical seed-42 `sim_config` + `constants` header blocks
   vs. a pre-rename soak on the same commit. Phase 3 of the warmth
   split (`social_warmth` as fulfillment axis) lands later in
   Phase 6 alongside the §7.W Fulfillment register — not here.
5. **Commit new `fox_softmax_temperature: 0.15` field to
   `ScoringConstants`.** Field does not exist today; this adds it.
   §8.5 calls for fox convergence onto softmax. Matches the existing
   `action_softmax_temperature` (0.15) + `disposition_softmax_temperature`
   (0.15) at `sim_constants.rs:1244–1245`. Unused at commit time;
   Phase 3c retires `fox_scoring.rs:103` per-score jitter and wires
   this constant in.
6. **Archive a baseline soak** at the commit that clears gates 1–5.
   `just soak 42` → `logs/baseline-pre-substrate-refactor/`. Keep
   `events.jsonl`, `narrative.jsonl`, and `header.constants` versioned
   in `docs/balance/substrate-refactor-baseline.md`. This is the diff
   target for every phase. Pre-refactor dormancy counts (Farming 0,
   Mating ~0, Crafting sparse, PracticeMagic sub-modes sparse, The
   Calling absent) captured here. Baseline doc also records the
   founder-age-wipeout inheritance from gate 2's park decision.

## Phase structure

Seven phases. Phases 1–5 are L1+L2 substrate; Phase 6 is L3 + §7.W
fulfillment; Phase 7 is cleanup + §10 feature-queue handoff.

**Each phase ships under the same discipline:**

- A phase-kickoff doc at `docs/balance/substrate-phase-N.md` stating
  hypothesis, predicted drift direction, and canaries it must not break.
- A phase-exit doc in the same file recording observed drift and
  concordance call.
- If a phase's concordance fails, it does **not** roll forward; the phase
  re-iterates until it ties out.
- Phases land via solo-to-main; commits push direct to `main` per CLAUDE.md.
  Feature branches (`wnmitch/substrate-N-...`) are optional staging for
  work where review-by-revert is costly (Phases 3, 5, 6 are obvious
  candidates).

### Phase 1 — §11 instrumentation scaffold

**Why first:** §11 is how every subsequent phase proves concordance.
Building the new substrate without layer traces is the "change-and-see"
loop CLAUDE.md forbids.

**Deliverables:**
- `FocalTraceTarget` resource (§11.5 gate), inserted only by the
  headless runner. `--focal-cat NAME` flag plumbed through `main.rs`.
- `logs/trace-<focal>.jsonl` sidecar with shared header matching
  `events.jsonl` (commit_hash, sim_config, constants — §11.4
  joinability invariant).
- Trace-emitter systems registered at each layer:
  - **L1 emitter** — one at L1-sample-read time (§11.3 L1 record).
    Starts as a shim over today's `wind.rs` / `sensing.rs`; gets
    rewritten in Phase 2 when `InfluenceMap` lands.
  - **L2 emitter** — on every focal-cat DSE evaluation (§11.3 L2
    record). At Phase 1 entry there's no `Dse` trait yet; the L2
    emitter walks today's `ScoringResult` + `ScoringContext` and
    produces the record shape. Rewritten in Phase 3 when the trait
    lands.
  - **L3 emitter** — single-tick selection record (§11.3 L3 record).
    Wraps today's `select_disposition_softmax`.
- **Top-N losing-axis schema slot** (§7.W.6) — the L2 record format
  reserves space for top-N (suggested N=3) losing scores per tick, even
  though the axis-capture primitive doesn't land until Phase 6.
  Commit `290a5d9` already extends `CatSnapshot.last_scores` to log all
  gate-open action scores; Phase 1 wires this into the trace sidecar.
- **Continuity-canary telemetry** (CLAUDE.md flags these as "not yet
  wired into `logs/events.jsonl` — that's a follow-on plan"). Phase 1
  is that follow-on. Emit events for: grooming fires, play fires,
  mentoring fires, burial fires, courtship fires, mythic-texture events
  (Calling fired, banishment, visitor arrival, named object crafted).
  Footer tallies by class. `just check-continuity` exits non-zero on
  any continuity canary at 0.
- **Apophenia continuity-canary telemetry** (§8.6) — pairwise
  behavioral distance across N sampled cats at tick T; same-cat
  behavioral autocorrelation across K-day windows. Commits the schema
  slots; numeric N, K, and thresholds are Phase 6 calibration.
- `scripts/replay_frame.py --tick N --cat NAME` — pivot on `(tick,
  cat)`, reconstruct full frame top-to-bottom.
- `scripts/frame_diff.py <baseline-trace> <new-trace> --hypothesis PATH`
  — per-tick, per-cat, per-DSE score delta with hypothesis overlay;
  emits `concordance: ok | drift` report.
- New `just` recipes:
  - `just soak-trace SEED` — soak + emit focal trace.
  - `just frame-diff BASELINE NEW --hypothesis PATH` — pairwise
    frame-diff with hypothesis overlay.
  - `just check-continuity LOGFILE` — exits non-zero on canary=0.
  - `just autoloop SEED` — full gate (see Verification loop).

**Agent-team assignment:**
- **Main session** owns: `FocalTraceTarget` design, L2 record format
  decisions (what fields go in, especially the top-N losing-axis slot
  for §7.W.6 forward-compat), `replay_frame.py` pivot semantics.
- **Scripting sub-agent** — writes `frame_diff.py` with hypothesis
  overlay parsing.
- **Balance-log sub-agent** — drafts `docs/balance/substrate-phase-1.md`
  kickoff + exit from the hypothesis table.

**Acceptance:** soak at current tip, emit baseline trace, roundtrip
through `replay_frame.py`, confirm it reconstructs the same ranked-DSE
list as `CatSnapshot.last_scores`. If it doesn't, the emitter is wrong
and L1 is not yet trustworthy.

**Enumeration landing:** §11.3 (three record formats), §11.5 (focal-cat
gate), §11.4 (joinability invariant). §7.W.6 top-N losing-axis schema
slot reserved.

**Critical files:**
- `src/main.rs` — add `--focal-cat NAME` flag, `FocalTraceTarget`
  insertion. Remember manual-mirror rule vs. `build_schedule`.
- `src/resources/trace_log.rs` (new)
- `src/systems/trace_emit.rs` (new)
- `src/plugins/simulation.rs` + `src/main.rs::build_schedule` (manual
  mirror)
- `scripts/replay_frame.py`, `scripts/frame_diff.py` (new)
- `docs/balance/substrate-phase-1.md` (new)

### Phase 2 — L1 influence-map generalization (§5)

**Delivers:** B1 ("generalize influence maps") of open-work Cluster B.

**Strategy:** name what already exists. Today's `wind.rs` + `sensing.rs`
is a de-facto scent IAM (§Current state). Extract the pattern into a
reusable `InfluenceMap`; migrate `scent` as the reference port; add the
4 other Partial maps from §5.6.3. The 8 Absent maps stay Absent — they
land with the features that need them.

**Deliverables:**
- `InfluenceMap` abstraction per §5.1 (base maps, templates, working
  maps); §5.3 decay; §5.4 obstacle-aware propagation.
- Channel registry per §5.2 + §5.6.2 (extensibility contract from §5.6.9).
- `SpeciesSensitivity × RoleModifier × InjuryDeficit × EnvMultiplier`
  attenuation pipeline (§5.6.6). Species-sensitivity matrix already at
  `src/resources/sim_constants.rs:2605–2696` (40 cells committed) —
  wire it, don't re-author.
- L1 trace records now reflect real map reads (supersedes Phase 1 shim).
- **Migration of 5 Partial maps** (§5.6.3):
  1. `scent-proximity` (reference port — tick-for-tick parity required)
  2. `corruption level`
  3. `fox-scent threat-proximity`
  4. `exploration state`
  5. `congregation (cat-density)`
- **8 Absent maps deferred** to §10 feature-queue consumers:
  - ward coverage/strength (Environmental Quality / Herbcraft)
  - prey-location (Fox AI parity / Hunt)
  - carcass-location (Herbcraft Harvest)
  - food-location/herb-location/construction/garden-location (colony AI)
  - kitten-urgency (Caretake)

**Agent-team assignment:**
- **Main session** owns: `InfluenceMap` trait design, scent port
  (tick-for-tick parity is the hardest invariant in the whole refactor).
- **Map-builder sub-agents** (×4, parallel once scent lands) — one per
  Partial map (corruption, fox-scent, exploration, congregation). Each
  ports the ad-hoc accumulator to `InfluenceMap` following the scent
  reference.
- **Soak-runner sub-agent** — runs `just autoloop` on each sub-port
  landing; reports gate status.
- **Balance-log sub-agent** — drafts phase-2 balance doc.

**Acceptance:**
- Scent behavior identical tick-for-tick under seed 42 (bitwise or ≤ε
  floating-point drift, measured via frame-diff on scent L1 records).
  This is the hardest invariant-preservation step and justifies per-tick
  comparison.
- Corruption / fox-scent / exploration / congregation maps produce
  per-tick records matching today's ad-hoc accumulators within ≤ε.

**Enumeration landing:** §5.1–§5.4, §5.6.2 (4 channels), §5.6.3 (13 maps
— 5 Partial ported, 8 Absent deferred with owner tags), §5.6.4
(propagation modes), §5.6.5 (decay per map), §5.6.6.1 (species×channel
40 cells — wired from existing constants), §5.6.6.2 (role×channel —
wired but identity today; active when §4.3 role markers land in Phase
3a), §5.6.6.3 (injury×channel — wired but identity; active when body-zone
component lands, out of scope), §5.6.6.4 (environment×channel — wired
but identity; activation is Phase 2 balance work), §5.6.9 (extensibility
contract).

**Critical files:**
- `src/systems/influence_map.rs` (new — owns abstraction)
- `src/systems/wind.rs`, `src/systems/sensing.rs` — rewrite as thin
  adapters over `InfluenceMap`
- `src/resources/sim_constants.rs:2605–2696` (species-sensitivity matrix,
  reuse)
- `docs/balance/substrate-phase-2.md` (new)

### Phase 3 — L2 core: trait, curves, composition, modifiers, markers, Maslow pre-gate, faction (§1–§4, §9, §L2.10)

**The critical phase.** Whole L2 substrate lands as one unit so each DSE
reaches the new evaluator with its proper curve + composition mode at the
same time — no interim state where a DSE has been switched over but still
uses flat-preference WeightedSum. This is what breaks Mating / Crafting /
Magic / Farming dormancy open.

No shadow-mode. `scoring.rs` / `fox_scoring.rs` action blocks delete as
each new DSE lands. Behavior drift is the goal.

Sub-structured 3a → 3b → 3c → 3d. All four land before phase exit.

#### 3a. Scaffolding (no DSE yet consumes it)

**Deliverables:**

- **`Dse` trait** (§L2.10.2):
  ```rust
  trait Dse {
      fn id(&self) -> DseId;
      fn considerations(&self) -> &[Consideration];
      fn composition(&self) -> CompositionMode;
      fn eligibility(&self) -> EligibilityFilter;
      fn commitment_strategy(&self) -> CommitmentStrategy;  // Phase 3 default; semantics Phase 6
      fn emit(&self, score: f32, ctx: &EvalCtx) -> Intention;
  }
  ```
- **`Intention` enum** with `Goal | Activity` variants (§L2.10.4–§L2.10.5).
  `Intention` carries a `CommitmentStrategy` tag from day one (§7.1 +
  §L2.10.4); reading logic lands in Phase 6.
  Phase-3 defaults: `SingleMinded` for Goal, `OpenMinded` for Activity.
- **`Termination` enum** (§L2.10.4, 3 variants):
  ```rust
  enum Termination {
      Ticks(u32),
      UntilCondition(fn(&World, Entity) -> bool),
      UntilInterrupt,
  }
  ```
- **`EvalCtx`** (§4.2 marker-catalog reads) — replaces `ScoringContext` +
  `FoxScoringContext` as the evaluator's input-shape; pulls 27+9 booleans
  from §4.3 markers and keeps 19+5 scalars sampled per §4.5.
- **`EligibilityFilter`** — pre-scoring gate; reads §4.3 markers +
  §9.3 faction stance.
- **Registration builder** (§L2.10.3, 5 methods):
  - `app.add_dse` (18 registrations)
  - `app.add_target_taking_dse` (13 registrations)
  - `app.add_fox_dse` (9 registrations)
  - `app.add_aspiration_dse` (2 registrations — Phase 5)
  - `app.add_coordinator_dse` (2 registrations — Phase 5)
  - `app.add_narrative_dse` (1 registration — Phase 5)
- **Response-curve library** (§2.1, 7 primitives): `Linear`,
  `Quadratic`, `Logistic`, `Logit`, `Piecewise`, `Polynomial`,
  `Composite`. Function-evaluated per §2.2 (no LUT yet; §2.2 allows the
  LUT optimization later).
- **Composition modes** (§3.1, 3 modes): `CompensatedProduct`,
  `WeightedSum`, `Max`. Compensation factor per §3.2. The 3 `Max`
  assignments are retiring (Herbcraft, PracticeMagic parents) — sibling
  DSEs in 3c dissolve them.
- **Weight rationalization** (§3.3) — RtM/RtEO labels; §3.3.2 8
  absolute-anchor peer groups committed as cross-DSE magnitude contracts:
  1. Starvation-urgency anchor (6 DSEs: Eat, Hunt, Forage, Cook,
     Fox_hunting, Fox_raiding)
  2. Fatal-threat anchor (6: Flee, Fight, Patrol, Fox_fleeing,
     Fox_avoiding, Fox_den_defense)
  3. Rest-urgency anchor (3: Sleep, Idle, Fox_resting)
  4. Social-urgency anchor (5: Socialize, Groom(other), Mentor,
     Caretake, Mate)
  5. Territory-urgency anchor (2: Fox_patrolling, Patrol)
  6. Work-urgency anchor (3: Build, Farm, Coordinate)
  7. Exploration-urgency anchor (2: Explore, Wander)
  8. Lifecycle-override anchor (1: Fox_dispersing)
- **Maslow pre-gate wrapper** (§3.4) — wraps `level_suppression` from
  `src/components/physical.rs:249–263`; no new path.
- **Post-scoring modifier pipeline** (§3.5.1 / §3.5.2, 7 modifiers as
  passes):
  1. Pride bonus (Hunt, Fight, Patrol, Build, Coordinate)
  2. Independence solo boost (Explore, Wander, Hunt)
  3. Independence group penalty (Socialize, Coordinate, Mentor)
  4. Patience commitment bonus (disposition-constituent DSEs — subsumed
     by §7.4 persistence bonus in Phase 6; kept as modifier in Phase 3
     for parity)
  5. Tradition location bonus — **bug fix inline** (§3.5.3): was
     unfiltered loop applying to all actions; filter to per-action
     allowlist.
  6. Fox-territory suppression — **bug fix inline**: was producing
     Flee-boost side effect via score += suppression * 0.5; retire that
     path. Flee earns its score through its own curve, not as modifier
     side effect.
  7. Corruption-territory suppression (Explore, Wander, Idle)
- Dead `has_active_disposition` boolean field on `ScoringContext` line
  101 — **deleted** (§3.5.3).
- **Context-tag marker components** per §4.3 with scalar carve-out per
  §4.5:
  - ~44 ECS markers across 11 categories: Species (6), Role (3),
    LifeStage (4), State (7), Capability (4), Inventory (7),
    TargetExistence (10), Colony (3), SpawnImmutable (2), Reproduction
    (2 — `Fertility { phase }`, `Parent`), Fox-specific (8).
  - 36 boolean ScoringContext fields → ECS markers (27 cat + 9 fox).
  - 24 scalars stay sampled (19 cat + 5 fox).
  - Per-marker authoring-system roster (§4.6): map each marker to the
    system that inserts/removes it (e.g., `social.rs::check_bonds` owns
    `Visitor`; `combat.rs::check_incapacitation` owns `Incapacitated`).
    Gap-fill where no owner exists today.
- **§9 faction model scaffolding:**
  - `FactionStance` enum (6 variants: Same, Ally, Neutral, Prey,
    Predator, Enemy).
  - `FactionRelations` resource with the 10×10 biological base matrix
    (100 directed stance cells committed — see §9.1).
  - 4 ECS-overlay markers (§9.2): `Visitor`, `HostileVisitor`,
    `Banished`, `BefriendedAlly`. Resolution order:
    most-negative-wins (`Banished` ≻ `HostileVisitor` ≻ `Visitor` ≻
    base ≻ `BefriendedAlly`).
  - Stance-resolution helper reads base matrix, applies overlay per
    resolution order.
  - 5 DSE-filter bindings (§9.3) ready for consumers in 3c + Phase 4.
- **§12 belief proxies** named as interfaces (§12.3):
  `achievement_believed`, `achievable_believed`, `still_goal`.
  Implementations are per-DSE in 3c; Phase 6 consumes them in the
  drop-trigger gate.

**Agent-team assignment (3a):**
- **Main session only** — 3a is all scaffolding trait design and
  architecture calls. Sub-agents can't make these well.

#### 3b. Reference migration — `Eat` end-to-end

**Deliverables:**
- Port `Eat` as reference DSE (clean goal shape, no target-selection, no
  sub-modes). `WeightedSum` composition, `Logistic(8, 0.75)` on hunger.
  Shakes out the trait design and the §11 trace emitter's L2 record
  format against a real DSE before scaling. `scoring.rs::score_eat`
  deletes on land.

**Agent-team assignment (3b):**
- **Main session** owns the Eat port — this is the template every
  fan-out sub-agent mimics in 3c.

**Per-DSE hypothesis:** `Logistic(8, 0.75)` on hunger replaces linear
`(1 - hunger) * urgency_scale`. Prediction: Eat firing threshold shifts
sharper around hunger=0.75 midpoint; canary Starvation holds at 0.

#### 3c. Fan-out port of remaining DSEs

**Deliverables:**
- Port 20 remaining cat DSEs + 9 fox DSEs through the unified evaluator
  with §2.3 curves + §3.1.1 composition modes + §3.3.1 weight-expression
  modes + §3.5.2 modifier-applicability from day one. `scoring.rs` +
  `fox_scoring.rs` action blocks delete as each lands.
- **Herbcraft + PracticeMagic sibling-DSE split** (§L2.10.4,
  §L2.10.10):
  - **Herbcraft siblings (3):** `herbs_in_inventory` (Goal),
    `remedy_applied` (Goal + target-taking), `ward_placed` (Goal +
    target-taking).
  - **PracticeMagic siblings (6):** `scry` (Activity — Calling
    integration point), `durable_ward` (Goal + target-taking),
    `cleanse` (Goal + target-taking), `colony_cleanse` (Goal),
    `harvest` (Goal + target-taking), `commune` (Activity — special-
    terrain gate).
  - Parent `max()` dissolves. Each sub-mode is its own sibling goal- or
    activity-shaped DSE sharing eligibility. **This is the mechanism
    that breaks PracticeMagic sub-mode dormancy open.**
- **§7.M Layer-3 `MateWithGoal` lands here.** Replaces today's
  `DispositionKind::Mating` + `build_mating_chain`
  (`disposition.rs:1873–1919`). `Goal(mating_event_completed)` with
  `SingleMinded` commitment. Firing gates: L1 active, L2 active with
  specific partner (bond ≥ Partners), both satiated, both in seasonal
  fertile window (via `Fertility.phase`), partner in §5.6.3 sensory
  range. Existing 4-step chain (`MoveTo → Socialize → GroomOther →
  MateWith`) preserved; just re-parented into §L2.10.4 Intention
  framework.
- **§7.M.7 `Fertility` component** — new ECS component:
  ```rust
  #[derive(Component)]
  struct Fertility {
      phase: FertilityPhase,
      phase_ticks_remaining: u32,
      cycle_count: u32,
  }

  enum FertilityPhase {
      Proestrus,   // pre-receptive
      Estrus,      // receptive — peak window
      Diestrus,    // post-receptive
      Anestrus,    // non-receptive, seasonal (tom cats year-round here during Winter)
      Postpartum,  // nursing interval
  }
  ```
  Phase transition function per §7.M.7.2, cycle parameters per §7.M.7.3,
  reproductive roles (queen / tom) per §7.M.7.4. Signal mapping for
  Mate consideration per §7.M.7.5. Authoring system: new
  `src/systems/fertility.rs` per §7.M.7.7.
- **Target-taking DSEs** register with a placeholder single-target
  resolver matching today's behavior; full §6 treatment in Phase 4.
  This bounds Phase 3's scope.
- **Faction filter binding** (§9.3) activates: `SocializeDse`, `AttackDse`,
  `FleeDse`, `HuntDse`, `FoxRaidDse` each declare accepted stance set;
  `EligibilityFilter` reads stance via `FactionRelations` + overlay.

**Per-DSE hypothesis table** — drafted in
`docs/balance/substrate-phase-3.md` before 3c starts. Critical rows:

| DSE | Composition | Curve | Prediction |
|---|---|---|---|
| **Mate (L3)** | CompensatedProduct | Logistic(20, 0.5) partner-proximity | Gate-starvation resolved by L1/L2 context (Phase 4, 5); L3 firing count rises from ~0 to ≥3 per seed-42 soak. |
| **Crafting** | CompensatedProduct | Logistic(recipe_progress) | Recipes progress to completion; Crafting Intentions adopted and held. |
| **PracticeMagic sub-modes** | Sibling-DSE split + per-sub-mode composition | Varies per sibling | All 5 fire ≥1× per soak (Scry, DurableWard, Cleanse, Commune, Harvest). |
| **Farming** | CompensatedProduct | Quadratic(2) food-scarcity | First-ever fire. |
| **Build** | WeightedSum (unchanged) | Piecewise(site/repair bonuses) | Frequency rises; Pride bonus re-engages on respect-low cats. |
| **Mentor** | WeightedSum | Logistic(8, 0.4) skill-gap | Frequency rises per open-work #3's hypothesis (`mentor_warmth_diligence_scale` raised). |
| **§3.5.3 modifier bugs** | — | — | Tradition per-action allowlist; Fox-suppression Flee-boost retired; dead `has_active_disposition` deleted. |

**Agent-team assignment (3c):**
- **Main session** owns: Mating L3 port (interacts with Fertility
  component — new territory); Herbcraft + PracticeMagic sibling-DSE
  split (new semantic territory, not just a port); Faction filter
  wiring (cross-DSE invariant).
- **DSE-porter sub-agents** (×N, one per DSE, parallelizable):
  - Cat DSEs (17 remaining after Eat + Mate + Herbcraft + PracticeMagic
    families): Sleep, Hunt, Forage, Groom(self), Flee, Fight, Patrol,
    Build, Farm, Socialize, Groom(other), Explore, Wander, Cook,
    Coordinate, Mentor, Caretake, Idle, Apply Remedy (Apply Remedy is
    target-taking; placeholder resolver in 3c, full in Phase 4).
  - Fox DSEs (all 9): Fox_hunting, Fox_raiding, Fox_resting,
    Fox_fleeing, Fox_patrolling, Fox_avoiding, Fox_feeding,
    Fox_den_defense, Fox_dispersing.
  - Each DSE-porter follows the Eat reference; self-verifies via
    shadow-mode score-equality assertion against old scoring path (the
    assertion fails intentionally where curves differ — the hypothesis
    table declares expected directional drift).
- **Curve-assigner sub-agents** (×N, one per §2.3 row, parallel with
  DSE-porters) — assigns primitive + parameters for each consideration;
  updates tests; cross-checks §3.3.2 absolute-anchor peer groups.
- **Soak-runner sub-agent** — runs `just autoloop` on each DSE-port
  landing; reports gate status, per-DSE frame-diff, and hypothesis-table
  direction match.
- **Balance-log sub-agent** — maintains per-DSE table in
  `docs/balance/substrate-phase-3.md`.

**Spawning rule:** sub-agents do the *mechanical port of an
already-decided shape*. They do not make architectural calls, negotiate
spec ambiguities, or pick commitment strategies. Those stay with main.

#### 3d. Faction matrix + §4.6 authoring-system roster gap-fill

**Deliverables:**
- Faction base matrix (100 stance cells) committed in
  `sim_constants.rs` as const table following the §9.1 row list.
- §4.6 authoring-system roster — for any §4.3 marker without a current
  owner system, commit a new authoring system or extend an existing one.
  Examples:
  - `Fertility { phase }` → new `src/systems/fertility.rs`.
  - `Apprentice` / `Mentor` → extend `growth.rs`.
  - `Parent` → extend `pregnancy.rs`.
  - `IsParentOfHungryKitten` → derive from `Parent` + kitten needs via
    run-if gated system.
  - `Visitor` / `HostileVisitor` / `BefriendedAlly` — **deferred** to
    Trade & Visitors feature (§10 row); committed as empty-owner
    markers until that epic lands.
  - `Banished` — extend `combat.rs`'s existing `pending_banishments`
    path (today shadowfox-only) to cat-on-cat.

**Agent-team assignment (3d):**
- **Main session** owns faction-matrix commit (invariant-sensitive).
- **Marker-authoring sub-agents** (×N, one per missing owner system):
  ports each marker to its authoring system. Self-verifies via
  integration test asserting marker insertion on trigger event.

**Acceptance (Phase 3 overall):**
- 21 cat DSEs + 9 fox DSEs + 9 sibling DSEs registered through unified
  evaluator; `scoring.rs` + `fox_scoring.rs` action blocks deleted.
  Target-taking DSEs use placeholder resolvers (replaced Phase 4).
- Survival canaries hold.
- Continuity canaries **strengthen** (improvement required, not just
  non-regression).
- Positive-exit motion: Farming fires ≥1× (zero-to-nonzero), at least
  3 of 5 PracticeMagic sub-modes fire, Mating and Crafting frequency
  rise above baseline. Final targets are the refactor-level gate —
  Phase 3 must show they're *reachable*, Phases 4–6 close any gap.
- Per-DSE frame-diff matches hypothesis-table direction; wrong-direction
  drift investigated before phase exit.
- Faction matrix loaded and resolved correctly for all 5 DSE-filter
  bindings.
- `Fertility` component emits per-tick phase transitions on a seed-42
  soak that are consistent with §7.M.7.2.

**Enumeration landing (Phase 3):**
- §1.1–§1.3 (consideration trait + three flavors + normalization)
- §2.1 (7 curve primitives), §2.3 (assignment table — all 30 DSE rows)
- §3.1 (3 composition modes), §3.1.1 (per-DSE assignment — all 30
  rows), §3.2 (compensation factor), §3.3 (weight rationalization),
  §3.3.1 (per-DSE weight-expression), §3.3.2 (8 absolute-anchor peer
  groups)
- §3.4 (Maslow pre-gate)
- §3.5.1 (7 modifiers), §3.5.2 (applicability matrix), §3.5.3 (bug
  fixes: Tradition, Fox-suppression, dead `has_active_disposition`)
- §4.1–§4.6 (tag categories, catalog schema, ~44 marker catalog,
  ScoringContext crosswalk, scalar carve-out, authoring-system roster)
- §7.M.1 (L3 MateWithGoal only — L2, L1 in Phases 4, 5)
- §7.M.7 (full Fertility state spec)
- §9.0–§9.3 (faction base matrix 10×10, overlay markers, resolution
  order, DSE filter binding)
- §L2.10.2 (Dse trait), §L2.10.3 (registration catalog — 45 entries,
  though target-taking DSEs use placeholder resolvers; aspiration +
  coordinator entries defer to Phase 5), §L2.10.4 (Intention =
  Goal/Activity + CommitmentStrategy tag + 3 termination types),
  §L2.10.5 (Goal/Activity split), §L2.10.10 (Herbcraft/PracticeMagic
  sibling-DSE curve specs — 9 siblings)
- §12.3 belief proxies as named interfaces (implementations per-DSE)

**Critical files:**
- `src/ai/dse.rs` (new — trait, Intention, Termination, EvalCtx,
  EligibilityFilter)
- `src/ai/curves.rs` (new — §2.1 primitives)
- `src/ai/composition.rs` (new — §3.1 three modes + §3.2)
- `src/ai/eval.rs` (new — unified `evaluate(...)` + Maslow pre-gate +
  modifier pipeline)
- `src/ai/markers.rs` (new — §4.3 context-tag catalog + §4.5 carve-out)
- `src/ai/faction.rs` (new — §9 stance resolution)
- `src/ai/dses/*.rs` (new — one file per DSE)
- `src/components/fertility.rs` (new — §7.M.7)
- `src/systems/fertility.rs` (new — §7.M.7.7 authoring)
- `src/ai/scoring.rs`, `src/ai/fox_scoring.rs` — shrink to empty as 3c
  progresses; deleted at phase exit.
- `src/resources/sim_constants.rs` — 100-cell faction base matrix
  added.
- `docs/balance/substrate-phase-3.md` (new)

### Phase 4 — L2 target selection (§6, §7.M.2, §L2.10.7)

**Fixes the resolver-divergence bug** (§6.2). Today `disposition.rs` ranks
by fondness+novelty, `goap.rs` by fondness only. One `TargetTakingDse`
owns target-quality; both paths consume its result.

**Deliverables:**
- **`TargetTakingDse` trait** (§6.3):
  ```rust
  trait TargetTakingDse {
      fn eligibility(&self, target: Entity) -> bool;
      fn per_target_considerations(&self) -> &[TargetConsideration];
      fn aggregation(&self) -> AggregationMode;
  }
  ```
- **Personal-interest template** (§6.4) — 9 target-taking DSEs with per-row
  max range, distance curve, and eligibility. The 9 rows:
  1. Socialize (range 8, Quadratic(2))
  2. Mate (range 1, Logistic(20, 0.5))
  3. Mentor (range 8, Quadratic(2))
  4. Groom(other) (range 1–2, Logistic(15, 1))
  5. Hunt (species-dependent range, Quadratic(2))
  6. Fight (range 2–3, Logistic(10, 2))
  7. Apply Remedy (range 15, Quadratic(1.5))
  8. Build (range 20, Linear(-1/20, 1))
  9. Caretake (range 12, Quadratic(1.5))
- **§6.5 per-target considerations** (9 bundles × 4 considerations = 36
  rows). Each bundle committed in spec; port all weights + curves.
- **§6.6 aggregation modes** (3): `Best` default, `SumTopN(n)` for
  Fight (top-3 threats), `WeightedAverage` registered as alternative.
- **§L2.10.7 `SpatialConsideration`** — candidate (a) resolution. Lands
  inside each target-taking DSE as an attenuation channel separate from
  curve-based distance scoring. Consumed by Phase 6's drop-trigger
  `achievable_believed` proxy (second channel is `replan_count` hard-fail
  at `src/components/goap_plan.rs:103`, already present).
- **Elastic failure** (§0.2): target that vanishes re-ranks smoothly,
  never hard-gates the DSE.
- **§7.M Layer-2 `PairingActivity`** lands here as an Activity
  Intention (`Intention::Activity(Pairing, UntilCondition(...))`). Scope:
  multi-season ambient, active once `Partners+`-tier bond exists. Biases
  proximity-to-partner, grooming, nest-sharing, shared-travel,
  co-hunting via weight modifiers on existing DSEs (no new mechanics,
  per §7.M.1 "character expression" clause). Termination conditions:
  partner dies/leaves, bond drops below Partners, out-of-season,
  L1 aspiration drops. Persistence tier: Medium (mirrors Socializing in
  §7.4). Strategy: OpenMinded.
- **§7.M.2 post-consequence cascade** scaffolding — partner-death drop
  path wires into grief-event emitter (§7.7.b target, extended in
  Phase 6).

**Agent-team assignment:**
- **Main session** owns: `TargetTakingDse` trait, `SpatialConsideration`
  design, Mate/Mentor resolver-divergence reconciliation (§6.2 bug
  fix), §7.M.2 post-consequence cascade design.
- **Target-port sub-agents** (×9, one per target-taking DSE, parallel):
  each ports a target-taking DSE's per-target considerations from spec
  §6.5 rows.
- **Consideration-assigner sub-agents** (parallel with target-port):
  for each per-target consideration, commit curve + parameters + test.
- **Soak-runner sub-agent** — runs `just autoloop`.
- **Balance-log sub-agent** — phase-4 balance doc.

**Acceptance:**
- 9 target-taking DSEs pass per-row predictions — novelty weight applied
  universally lifts Mentor + Socialize; distance-aware target quality
  via `SpatialConsideration` smooths Hunt / Fight / ApplyRemedy.
- Survival canaries hold.
- Continuity canaries (grooming, play, mentoring, courtship) **improve**
  — this phase is the most direct fix for resolver divergence +
  target-existence collapse that suppressed them. "Not regressing" is
  too weak; the phase fails if they don't strengthen.
- `PairingActivity` Intention fires and sustains across multi-season bonds
  on seed-42. Phase 6 lands L1 (`ReproduceAspiration`); without it,
  PairingActivity fires for bonded pairs only, not partner-seeking
  behavior yet.

**Enumeration landing (Phase 4):**
- §6.1 (anti-pattern inventory), §6.2 (resolver-divergence bug fix),
  §6.3 (TargetTakingDse), §6.4 (personal-interest template — 9 DSEs),
  §6.5 (per-target considerations — 36 rows across 9 bundles),
  §6.6 (3 aggregation modes)
- §7.M.1 (L2 PairingActivity), §7.M.2 (post-consequence cascade
  scaffolding), §7.M.6 (relationship-embedded dispositions —
  resolver parity for Groom, Socialize, Fight-over-partner), §7.M.4
  (L2 belief proxies grounded)
- §L2.10.7 (SpatialConsideration + replan_count — spatially-sensitive
  DSE roster of 22 cat + 9 fox closed)

**Critical files:**
- `src/ai/target_taking.rs` (new — trait + aggregation)
- `src/ai/dses/*.rs` — target-taking DSEs get resolver wiring
- `src/systems/disposition.rs`, `src/systems/goap.rs` — resolver
  divergence removed; both call into `TargetTakingDse`
- `docs/balance/substrate-phase-4.md` (new)

### Phase 5 — L2 scattered-scoring unification (§L2.10.1)

**Picks up the 10+ scattered-scoring sites** (§L2.10.1 audit). Each
island becomes a DSE registration.

**Deliverables (one sub-phase per site):**

- **5a. Coordinator election + directive priority.**
  - `Coordinate` DSE with per-role eligibility. Subsumes
    `coordination.rs:88–107` (coordinator election, every 100 ticks).
  - `Directive(*)` DSE set emitting `Intention::Goal`. Subsumes
    `coordination.rs:321–503`, `:483` (corruption), `:495` (carcass),
    `:924` (cook), `:948` (construct), `:986–987` (build). Directive
    priority scores become DSE considerations; old priority queue
    retires.
  - Lands §7.3's coordinator-directive row (enumeration debt from §7
    handoff): `SingleMinded` with coordinator-cancel override.
- **5b. Aspiration domain affinity → `Aspire(*)` DSE set** (one per
  domain). Subsumes `aspirations.rs:49–96` (domain affinity scoring).
  - **§7.M Layer-1 `ReproduceAspiration`** lands here as an `Aspire`
    DSE. Scope: lifetime arc. Strategy: `OpenMinded`. Persistence tier:
    High. Drop events: life-stage → Elder, sustained injury below
    reproductive-viability threshold, bereavement of Partners-tier
    partner (§7.7.b grief cascade), hard-logical aspiration conflict
    (§7.7.1). Emits per-tick short-horizon Intentions at L2 and L3
    (scaffolded in Phase 3c + Phase 4; now driven by L1 aspiration).
  - Other Aspire DSEs: `AspireMastery(domain)` per domain (Hunting,
    Combat, Social, Herbcraft, Exploration, Building, Leadership).
- **5c. Mate target selection** — merged into Phase 4's `Mate`
  TargetTakingDse. Retires `disposition.rs:1881–1907` linear
  per-candidate scoring.
- **5d. Caretake target selection** — merged into Phase 4's `Caretake`
  TargetTakingDse. Retires `disposition.rs:1925–1943` + `feed_kitten.rs`
  boolean gate + kinship partial.
- **5e. Fox DSE parity** — `fox_scoring.rs` already deleted in Phase 3c;
  this sub-phase verifies fox DSEs run through the shared evaluator and
  registers any remaining parallel-Maslow leftovers. §L2.10
  species-extensibility payoff.
- **5f. Narrative template selection** — *stays separate.* Not action
  selection, just weighted-random selection of lines at
  `narrative_templates.rs:616–649`. Noted so a future reader doesn't
  assume it was missed. §L2.10.3 registers it via `add_narrative_dse`
  for catalog completeness only.

**Agent-team assignment:**
- **Main session** owns: Coordinate DSE + Directive DSE set design
  (architectural — coordinator priority + per-role gating is a
  semantic boundary the sub-agents can't set); `ReproduceAspiration`
  (L1 Mating — interacts with fertility + grief + conflict).
- **Scattered-port sub-agents** (×6, one per sub-phase 5a–5f, parallel
  once main session delivers 5a + 5b designs): each sub-agent owns one
  site's mechanical port.
- **Soak-runner sub-agent** — runs `just autoloop` per sub-phase
  landing.
- **Balance-log sub-agent** — phase-5 balance doc.

**Acceptance:**
- All scattered-scoring sites consumed; no islands remain.
- `scoring.rs` at this point is typically empty (modifiers moved into
  `src/ai/eval.rs` in Phase 3; all action blocks retired in 3c).
- Each site's characteristic metric drifts in its sub-phase's predicted
  direction; wrong-direction drift investigated before sub-phase lands.
- Survival canaries hold; continuity canaries hold or improve.
- `ReproduceAspiration` fires for reproductive cats; drives partner-
  seeking via Socialize / Wander weight biases. Mating gate-starvation
  fully resolved once Phase 6 commitment layer lands.

**Enumeration landing (Phase 5):**
- §L2.10.1 (10+ scattered sites — all consumed)
- §L2.10.3 aspiration + coordinator + narrative registrations (5
  remaining entries from the 45-row catalog)
- §7.M.1 (L1 ReproduceAspiration — completes the 3-layer architecture)
- §7.M.6 (relationship-embedded dispositions finalized across all
  three layers)

**Critical files:**
- `src/ai/dses/coordinate.rs`, `src/ai/dses/directive_*.rs` (new)
- `src/ai/dses/aspire_*.rs` (new)
- `src/systems/coordination.rs` — priority-queue path retires; builder
  remains for directive emission
- `src/systems/aspirations.rs` — scoring retires; chain-selection
  survives as input to Aspire DSEs
- `docs/balance/substrate-phase-5.md` (new)

### Phase 6 — L3 commitment, persistence, softmax, §7.W fulfillment

**The second critical phase.** With §7 closed and §8 closed, Phase 6 is
pure implementation. No spec authoring.

#### 6a. Commitment layer (§7.1–§7.6)

**Deliverables:**
- **`CommitmentStrategy` semantics** (§7.1) — tag slot added in Phase 3a;
  Phase 6 adds the behavior:
  - `Blind` — drop only on achieved (Resting, Guarding).
  - `SingleMinded` — drop on achieved OR unachievable (Hunting,
    Foraging, Coordinating, Building, Farming, Crafting, Caretaking,
    MateWithGoal).
  - `OpenMinded` — drop on achieved OR no longer desired (Socializing,
    Exploring, Mating-L1, PairingActivity).
- **Drop-trigger reconsideration gate** (§7.2) runs each tick *after*
  `check_anxiety_interrupts` (`src/systems/disposition.rs:93`).
  Three belief proxies (§12.3):
  - `achievement_believed` — goal-state predicate against current
    percepts.
  - `achievable_believed` — two-channel: `SpatialConsideration` retention
    threshold (Phase 4) + `GoapPlan::replan_count` hard-fail
    (`goap_plan.rs:103`, present).
  - `still_goal` — DSE re-score against current context.
  - AI8 cap on every Intention (`max_persistence_ticks`).
- **Per-DispositionKind strategy table** (§7.3) — 12 rows from the spec:

  | Disposition | Strategy |
  |---|---|
  | Resting | Blind |
  | Guarding | Blind |
  | Hunting | SingleMinded |
  | Foraging | SingleMinded |
  | Coordinating | SingleMinded |
  | Building | SingleMinded |
  | Farming | SingleMinded |
  | Crafting | SingleMinded |
  | Caretaking | SingleMinded |
  | Mating L1 (ReproduceAspiration) | OpenMinded |
  | Mating L2 (PairingActivity) | OpenMinded |
  | Mating L3 (MateWithGoal) | SingleMinded |
  | Socializing | OpenMinded |
  | Exploring | OpenMinded |
  | Coordinator-directive | SingleMinded w/ coordinator-cancel override (committed in Phase 5a) |

- **Persistence bonus** (§7.4) — `base * logistic(completion_fraction,
  midpoint, steepness)`. `completion_fraction` shape-specific: goal
  path-cost fraction, activity tick fraction, chain step fraction. `base`
  per-DispositionKind (12 values — enumeration debt; numeric tuning is
  balance work post-implementation, not Phase 6 entry).
  Subsumes `src/ai/scoring.rs:695–704` patience bonus. Patience
  personality trait survives as per-cat multiplier on `base`, not as
  separate additive bonus. Delete old block.
- **Maslow interrupt pipeline** (§7.5) — documents existing
  `check_anxiety_interrupts` placement; no new path. Event-driven
  preemption bypasses §7.2 gate AND §7.4 bonus; replacement Intention
  installs with `Blind` commitment. Interrupt events: CriticalHealth,
  Starvation, Exhaustion, ThreatDetected, CriticalSafety (5 events from
  `disposition.rs:180–253`).
- **Monitoring cadence** (§7.6) — per-tick polling + event-driven
  interrupts; both already in place. Documented, no new code.

#### 6b. Aspiration-level commitment (§7.7)

**Deliverables:**
- `OpenMinded` default at aspiration layer.
- Event-driven reconsideration (*not* per-tick) on five classes:
  1. **§7.7.a Life-stage transitions** — growth.rs emitter extensions.
  2. **§7.7.b Grief cascade** — death.rs emitter; relationship-classified
     grief (enumeration debt from open-work #13.2, Mating dependency).
  3. **§7.7.c Prophetic visions (fate events)** — fate.rs vocabulary
     expansion (enumeration debt from open-work #13.3).
  4. **§7.7.d Sustained mood-valence drift** (N-season threshold) —
     mood.rs drift-threshold detection (enumeration debt from
     open-work #13.4).
  5. **§7.7.e Skill-mastery plateau or achievement** — growth.rs +
     aspirations.rs.
- `ConfidenceConsideration` with `Logistic`/`Exponential` decay over
  projected-duration, applied inside aspiration-emitted DSEs.
- `AspirationSet` concurrency with adoption-time compatibility check
  (§7.7.1). Four conflict classes: hard-logical, hard-identity (both
  rejected at adoption), soft-resource, soft-emotional (both allowed).
  Compatibility matrix default *compatible*; only genuine contradictions
  listed. Matrix itself remains enumeration debt (blocked on
  aspiration-catalog stabilization; open-work #13.5).

#### 6c. Softmax variation (§8)

**Deliverables:**
- Softmax-over-Intentions (§L2.10.6, §8.1, §8.2) replacing today's
  softmax-over-dispositions. `select_disposition_softmax` renames to
  `select_intention_softmax`. `select_action_softmax` (off hot path)
  retires entirely (§8.2).
- Temperature `T = 0.15` default (§8.3). Matches existing
  `action_softmax_temperature` + `disposition_softmax_temperature` in
  `ScoringConstants`. Not personality-scaled (§8.3 — character shows
  through scoring, not softmax).
- Ordering with §7.4 persistence bonus (§8.4): softmax runs over fresh
  candidate pool; incumbent Intention stays in pool (never excluded);
  if softmax draws incumbent, no-op; if softmax draws challenger,
  challenger score compared against `current_score + persistence_bonus`;
  preemption fires only on strict-greater.
- Maslow interrupts (§7.5) bypass softmax entirely; event-driven
  preemption installs `Blind`-committed replacement.
- `fox_softmax_temperature: 0.15` committed to `ScoringConstants` in
  pre-flight gate 5; fox `select_best_disposition` retires; fox
  per-score jitter at `fox_scoring.rs:103` retires.
- **Apophenia continuity canary** (§8.6) operationalized — pairwise
  behavioral distance + same-cat behavioral autocorrelation measured
  against N, K, and threshold values tuned in this sub-phase. Wired
  into §11 telemetry (schema slots reserved in Phase 1).

#### 6d. §7.W Fulfillment register + axis-capture primitive

**Deliverables:**
- **`Fulfillment` ECS component** — per-cat scalar register, new:
  ```rust
  #[derive(Component)]
  struct Fulfillment {
      axes: HashMap<AxisId, AxisState>,
      aggregate_decay_rate: f32,
  }

  struct AxisState {
      value: f32,                // current fulfillment inflow
      sensitization: f32,        // per-axis growth multiplier on successful use
      tolerance: f32,            // per-unit yield drop with repetition
      last_fire_tick: u32,
      source_diversity_weight: f32,  // decay-slowing contribution from other axes
      can_sensitize: bool,       // corruption-tainted axes enable; ordinary don't
  }
  ```
- **Per-axis dynamics** (§7.W.1):
  - Decay modulated by source-diversity: narrow-source cats decay faster.
  - Sensitization (opt-in per axis — corruption-tainted axes on,
    ordinary axes off): axis weight grows with successful use.
  - Tolerance: per-unit yield drops with repetition.
  - Specific curve shapes, coefficients, and ordering between
    sensitization and tolerance are numeric-tuning balance work (not
    substrate spec).
- **Warring-self signal** (§7.W.2): losing axes stay active; their
  pull persists; fulfillment deficit accumulates. Wires into
  `src/systems/mood.rs` valence-drop pathway without architectural
  change. Compulsion signature mechanically visible: narrow winning
  axis + active losing counter-axis + mood valence drop.
- **Top-N losing-axis logging** (§7.W.6) — `CatSnapshot.last_scores`
  extended (commit `290a5d9` did this; Phase 1 wired schema slot).
  Phase 6 populates the top-N entries with axis identity + value +
  deficit so narrative templates can bind to "axis X winning while
  axis Y losing above deficit-threshold."
- **The Calling as canonical instance** (§7.W.4(a)) — `docs/systems/
  the-calling.md` design mapped onto the primitive. Trigger conditions
  (magic affinity + mood + spirituality) become axis-activation
  conditions; trance becomes captured-axis-wins-every-tick; herb
  timeout becomes `Blind` commitment on means; 40–60-tick creation
  phase becomes bounded capture window; success/failure bivalence
  becomes axis resolution; "Touched" becomes persistent identity
  modifier as capture residue.
- **Warmth-split Phase 3 (`social_warmth` as fulfillment axis)**
  (§7.W.4(b), `docs/systems/warmth-split.md`) — lands here. Extends
  the mechanical rename from pre-flight gate 4. Modifies
  `src/steps/disposition/groom_other.rs:47` to feed
  `social_warmth` fulfillment axis (not temperature need). Adds
  isolation-driven decay. UI: second bar in cat inspect panel.
- **Dark Callings capacity** (§7.W.5) — flagged as Phase 6+ content;
  primitive supports it. Corruption-tainted trigger conditions produce
  compulsion to create destructive Named Objects; same trance
  mechanics, inverse valence. No new mechanism needed.
- **Non-goals enforced** (§7.W.7):
  - No meta-cognition / preference-over-preferences store (§7.W.3 —
    active-but-losing axis *is* the second-order preference).
  - No moral-valence labels on axes (framework morally silent; story
    emerges from content).
  - Fulfillment sits *above* Maslow; doesn't override Maslow
    interrupts (§3.4 + §7.5).
  - No active-avoidance-of-captured-axis primitive.

**Agent-team assignment:**
- **Main session only** for Phase 6 entry. Commitment layer + softmax
  ordering + Fulfillment register are architectural; sub-agents can't
  make these calls.
- Once Fulfillment scaffolding lands, **axis-wiring sub-agents** (×N,
  one per axis: each of the 12 dispositions + mastery axes + The
  Calling axis + social-warmth axis) parallelize mechanical wiring.
- **Balance-log sub-agent** — phase-6 balance doc.

**Acceptance:**
- **Sleep via `Blind` on Resting** — circadian commitment becomes
  representable; §10 Sleep stub moves Aspirational → Partial/Built.
- **The Calling fires** — ≥1 successful Named Object per sim year on
  seed-42. §10 Calling stub moves Aspirational → Built.
- **Mid-life crisis demonstrable** — forced grief event in scripted
  scenario redirects cat's aspiration under §7.7.b event-driven
  reconsideration. Apophenia-fuel acceptance per §7.7's crosswalk to
  §0.3.
- **Warring-self signal observable** — ≥1 documented
  narrow-winning-axis + active-losing-counter-axis + valence-drop
  trio per seed-42 soak; narrative emitter can bind.
- **Continuity canary** — same focal cat shows coherent Intentions
  across day-spans in §11 trace (§0.3 long-term relevance leg).
  Flipper failure mode §7 was built to prevent is absent in trace.
- Canaries hold; softmax temperature tuned via §11 aggregate
  distribution.
- Apophenia canary: pairwise behavioral distance stays in target band;
  same-cat autocorrelation across K-day windows reads as coherent
  character.
- Mating positive-exit criterion met: courtship arcs complete, ≥3
  matings per soak, ≥2 surviving kittens per starter colony (once
  Fertility × L1 × L2 × L3 all online).

**Enumeration landing (Phase 6):**
- §7.1 (CommitmentStrategy 3 variants), §7.2 (drop-trigger gate + 3
  belief proxies), §7.3 (12 disposition strategy rows), §7.4
  (persistence bonus + 12 `base` values as enumeration debt opened),
  §7.5 (5 Maslow interrupt events), §7.6 (monitoring cadence doc)
- §7.7 (aspiration-level commitment), §7.7.a–e (5 reconsideration
  event classes), §7.7.1 (AspirationSet concurrency — compatibility
  matrix deferred)
- §7.M.3–§7.M.5 (gate-starvation resolution + 3-layer belief-proxy
  wire-up + cascades)
- §7.W.0–§7.W.8 (axis-capture primitive, Fulfillment scalar,
  warring-self, second-order preference collapse, worked examples,
  free consequences, telemetry, non-goals)
- §8.1 (softmax algorithm), §8.2 (scope), §8.3 (T=0.15), §8.4
  (ordering with §7.4), §8.5 (fox convergence), §8.6 (apophenia
  continuity canary operationalized), §8.7 (residuals resolved),
  §8.8 (out of scope documented)
- §12.3 belief proxies (implementations consumed)
- **Enumeration debts this phase opens or hands off:**
  - §7.3 coordinator-directive row — **handed off** to Phase 5a (landed
    with Coordinate DSE).
  - §7.4 per-DispositionKind `base` magnitudes (12 values) — **closed
    via balance iteration** post-Phase 6 code landing.
  - §7.5 Maslow-interrupt event catalog — **closed** by cross-check of
    `check_anxiety_interrupts`.
  - §7.7 reconsideration event-class exhaustiveness — **closed** by
    cross-check of `growth.rs`, `death.rs`, `fate.rs`, `mood.rs`,
    `narrative.rs` emitters against the five committed classes.
  - §7.7.1 compatibility matrix — **deferred** (blocked on
    aspiration-catalog stabilization — outside refactor scope).
  - §L2.10.7 spatially-sensitive DSE roster — **closed** in Phase 4.

**Critical files:**
- `src/ai/commitment.rs` (new — CommitmentStrategy semantics +
  drop-trigger gate)
- `src/ai/persistence.rs` (new — §7.4 persistence bonus)
- `src/ai/softmax.rs` (new — §8 Intention-layer softmax)
- `src/components/fulfillment.rs` (new — §7.W register)
- `src/systems/fulfillment.rs` (new — §7.W dynamics: decay,
  sensitization, tolerance, source-diversity)
- `src/systems/disposition.rs` — drop-trigger gate wired after
  `check_anxiety_interrupts`
- `src/systems/aspirations.rs` — §7.7 event-driven reconsideration
- `src/systems/mood.rs` — valence-drop wire for warring-self signal
- `src/steps/disposition/groom_other.rs:47` — warmth-split Phase 3:
  feed `social_warmth` fulfillment axis
- `src/resources/sim_constants.rs` — `fox_softmax_temperature` (landed
  in pre-flight gate 5); persistence-bonus `base` values (12
  disposition rows); §2.3 retired-constant deletion (landed in
  Phase 3c with Logistic curves)
- `docs/balance/substrate-phase-6.md` (new)

### Phase 7 — Cleanup & handoff

**Deliverables:**
- `ScoringContext` deleted; `FoxScoringContext` deleted.
- Retired `ScoringConstants` fields deleted (per §2.3 retired-constants
  list — already burned in Phase 3c with the Logistic curves that
  subsumed them; verify nothing re-landed).
- `docs/systems/*.md` status updates (§10 unblock map):
  - Environmental Quality: Aspirational → Built (§2 curves + §5 IAM
    substrate)
  - Sensory: Aspirational → Built (§5.2 + §5.6.6 attenuation pipeline)
  - Body Zones (perception slice): Aspirational → Partial (substrate
    wired; anatomical details pending dedicated epic)
  - Mental Breaks: Aspirational → Partial (§7.W axis-capture; content
    pending)
  - Recreation & Grooming: Aspirational → Built (§4 target-taking +
    §7.W social_warmth)
  - Disease: Aspirational → Partial (§2 curves + §4 markers; content
    pending)
  - Sleep That Makes Sense: Aspirational → Partial (§7 Blind
    commitment; circadian content pending)
  - The Calling: Aspirational → Built (§7.W canonical instance)
  - Fox/Hawk/Shadowfox AI parity: Aspirational → Built (§L2.10
    species-extensibility + Phase 3c fan-out)
  - Strategist Coordinator: Aspirational → Partial (§L2.10.3 Coordinate
    + Directive DSEs; HTN layer pending)
- `docs/wiki/systems.md` regenerated via `scripts/generate_wiki.py`.
- Handoff epics opened in `docs/open-work.md` for Disease, Recreation,
  Mental Breaks, Sleep-That-Makes-Sense, Substances, Trade & Visitors,
  Organized Raids (§0.4 filter applies to each design pass).
- `docs/systems/ai-substrate-refactor.md` reference-list pruned —
  external-author attributions removed from the spec body; replaced
  with intrinsic design descriptions. Cross-refs to
  `docs/reference/bdi-rao-georgeff.md` +
  `docs/reference/behavioral-math-*.md` removed; those files deleted
  (git status shows them already staged for deletion).
- `open-work.md` #13 spec follow-ons reconciled — items 13.1 (retired
  constants), 13.2 (grief), 13.3 (fate events), 13.4 (mood drift), 13.5
  (aspiration compat matrix), 13.6 (coordinator-directive strategy row)
  all either landed or handed to their owner systems' open work.
- Post-death biographies (#10) rank-1024 presenter unblocked to land
  anytime — substrate is done; `logs/events.jsonl` `cat_id`
  denormalization audit gated on this.

**Agent-team assignment:**
- **Main session** owns §10 unblock-map status updates (judgment calls).
- **Wiki-regen sub-agent** runs `scripts/generate_wiki.py`, reports
  drift.
- **Spec-cleanup sub-agent** prunes external-author references from
  `ai-substrate-refactor.md` (mechanical search-and-replace; no
  architectural decisions).

**Acceptance:**
- §10 unblock map: all named features `addable` without further
  substrate work.
- Spec is clean of external-author attributions.
- `docs/wiki/systems.md` regenerated and committed.
- `open-work.md` #13 closed.

**Enumeration landing (Phase 7):**
- §10.1 (feature-design filter from §0.4 — documented in each unblocked
  system's status update).
- Retired constants (§2.3) — verified burned.

## Verification loop — "testing roundtrips without handholding"

Mechanism that lets each phase iterate without constant babysitting.
Three layers: a **gate** that exits non-zero on regression, a **signal**
that tells us *what* drifted and by how much, and a **hypothesis
ledger** that accumulates.

### The gate — `just autoloop SEED`

Lands in Phase 1. One recipe, two modes:

**Phase 2 mode (invariant-preserving refactor):** frame-diff is a *gate*
— bitwise or ≤ε drift required because Phase 2 reshapes L1 without
changing math.

**Phase 3+ mode (behavior-changing refactor):** frame-diff is a *signal*
— per-DSE direction + magnitude of drift; phase-hypothesis doc validates.
Gate is now:
1. `just check-canaries` exits 0 (survival canaries — hard).
2. Continuity-canary check exits 0 (grooming / play / mentoring /
   burial / courtship / mythic texture fire ≥1×; apophenia pairwise-
   distance + autocorrelation in target band — Phases 3/4 require
   *improvement*; Phases 5/6/7 require non-regression).
3. Per-DSE directional check: no DSE drifts against its
   hypothesis-table row without being documented.

```
just soak SEED
  → logs/tuned-SEED/{events,narrative,trace-focal}.jsonl
just check-canaries logs/tuned-SEED/events.jsonl        # survival
just check-continuity logs/tuned-SEED/events.jsonl      # new in Phase 1
just frame-diff \
  logs/baseline-pre-substrate-refactor/trace-focal.jsonl \
  logs/tuned-SEED/trace-focal.jsonl \
  --hypothesis docs/balance/substrate-phase-N.md        # reads predictions
just diff-constants logs/baseline-pre-substrate-refactor/events.jsonl \
                    logs/tuned-SEED/events.jsonl        # existing
```

Any hard-gate step non-zero → gate fails. Directional mismatches surface
in `frame-diff`'s report and are resolved by updating the hypothesis
(prediction was wrong) or investigating the DSE (code is).

### The signal — per-DSE, per-consideration, per-modifier Δ

§11's `frame-diff` emits a ranked list of the largest per-(cat, tick,
DSE, consideration) deltas. That list IS the signal. Top of the list
tells which consideration or modifier is responsible — not "scoring
changed," but "`Logistic(8, 0.75)` on `hunger` shifted `Eat.score` by
+0.08 at tick 4821 for Simba."

§11's "Curvature at every layer" design applied operationally: a
regression is always localizable to a layer, a primitive, and a cat.

**Extension for Phase 6+ (§7.W):** frame-diff also emits top-N
losing-axis deltas, so warring-self narratives have a machine-readable
provenance.

### The hypothesis ledger — `docs/balance/substrate-phase-N.md`

Every phase opens one. Structure matches today's iteration logs
(`eat-inventory-threshold.report.md` reference pattern):

- **Thesis** — spec section realized.
- **Hypothesis** — predicted drift direction per characteristic metric.
- **Canaries under this phase** — any tighter thresholds beyond the
  global four.
- **Observation** — after-soak metrics table.
- **Concordance** — per-metric accept/reject.
- **Landed commits** — hashes realizing the phase.

CLAUDE.md's four-artifact rule is already the project standard; this
plan's contribution is *mechanizing the measurement side* via
`frame-diff`.

### Multi-seed validation — end-of-phase only

`just soak 42` per-iteration. At phase exit, also run `just soak 99 &&
just soak 7 && just soak 2025 && just soak 314` (CLAUDE.md canonical
sweep). Any seed showing >2× seed-42 drift is investigated before
phase-exit.

## Agent fan-out strategy

This refactor has a natural grid structure — DSE × consideration × map ×
species × axis. Once Phase-3 scaffolding lands, most of the remaining
work is "port action N to DSE shape following the Eat reference." That
parallelizes.

### What main session always owns

- Phase sequencing, pre-flight gates, baseline archiving.
- Scaffolding phases (1, 3a, 6 entry). These require judgment calls
  sub-agents can't make well.
- Cross-cutting designs: §7.M three-layer Mating (L3 Phase 3c, L2 Phase
  4, L1 Phase 5b); §7.W Fulfillment register (Phase 6d); §9 faction
  matrix + overlay (Phase 3d).
- Phase-exit concordance calls.
- Spec-level ambiguity resolution when a sub-agent surfaces one
  (sub-agents escalate, not guess).

### Sub-agent role catalog

Each role is spawned per work item once the phase's scaffolding is in
place. Roles are parallelizable within a phase's fan-out window; each
sub-agent's output goes through a phase-exit concordance check.

| Role | Phase(s) | What it does | Parallelism bound |
|---|---|---|---|
| **Scripting sub-agent** | 1 | Writes `replay_frame.py`, `frame_diff.py` | 1 (serial) |
| **Map-builder sub-agent** | 2 | Ports one §5.6.3 Partial map to `InfluenceMap` | 4 (scent reference ports first; other 4 parallel) |
| **DSE-porter sub-agent** | 3c | Ports one action block from `scoring.rs` / `fox_scoring.rs` to `src/ai/dses/N.rs` | 15+ (after Eat reference lands; one per DSE) |
| **Curve-assigner sub-agent** | 3c | Commits curve primitive + parameters for one §2.3 row; updates tests | 15+ (parallel with DSE-porter) |
| **Marker-authoring sub-agent** | 3a, 3d | Ports one §4.3 marker to its owner system; gap-fills missing authors | 10+ (parallel) |
| **Target-port sub-agent** | 4 | Ports one target-taking DSE's per-target considerations from §6.5 rows | 9 (one per target-taking DSE) |
| **Consideration-assigner sub-agent** | 4 | Commits curve + parameters + test for one per-target consideration | 36 (one per §6.5 row) |
| **Scattered-port sub-agent** | 5a–5f | Ports one scattered-scoring site to its DSE | 6 (sequential within 5a + 5b designs; others parallel) |
| **Axis-wiring sub-agent** | 6d | Wires one fulfillment axis into `Fulfillment` register | 15+ (one per disposition + mastery domain + Calling + social-warmth) |
| **Soak-runner sub-agent** | all | Runs `just autoloop`, reports gate status | 1 (per-iteration) |
| **Balance-log sub-agent** | all | Drafts phase-kickoff + phase-exit in `docs/balance/substrate-phase-N.md` | 1 per phase |
| **Wiki-regen sub-agent** | 7 | Runs `scripts/generate_wiki.py`, reports drift | 1 |
| **Spec-cleanup sub-agent** | 7 | Prunes external-author references from spec | 1 |

### Spawning rules

- Sub-agents do the *mechanical port of an already-decided shape*.
- They do NOT make architectural calls, negotiate spec ambiguities, or
  pick commitment strategies.
- A sub-agent that discovers ambiguity escalates to main session with
  a 1-paragraph summary; main resolves, sub-agent proceeds.
- Each sub-agent self-verifies:
  - DSE-porter: shadow-mode score-equality assertion against old path
    (intentionally fails where hypothesis predicts drift).
  - Curve-assigner: updated tests + §3.3.2 peer-group cross-check.
  - Map-builder: tick-for-tick parity test on the ported map.
  - Target-port: §6.5 row-coverage test.
  - Scattered-port: integration test exercising the old call site
    routes through the new DSE.
  - Marker-authoring: insertion-on-trigger integration test.
  - Axis-wiring: `Fulfillment` accumulation test on a scripted cat.

### Bounding parallelism

Sub-agents operate on `worktree`-isolated copies when their work touches
the same files (e.g., `src/ai/dses/` is per-file; safe parallel). When
sub-agents must edit the same file (e.g., `src/resources/sim_constants.rs`
during faction matrix + retired-constants burn + curve parameters), they
run sequentially or hand off via main.

## Risk management

**R1. Drift in the wrong direction, masked by canaries holding.**
Canaries are coarse (starvation, shadowfox, wipeout); a refactor can
satisfy all three while regressing continuity canaries or tilting
higher-Maslow activity the wrong way. Mitigation: per-phase hypothesis
table scored against per-DSE frame-diff. Wrong-direction drift
investigated before phase exit even if survival canaries pass.
Continuity canaries are elevated from "info" to "gate" specifically for
Phases 3 and 4, where they must *strengthen*.

**R2. Multi-phase spec drift.** Spec may evolve mid-refactor (e.g., Phase
3 discovers a §3 ambiguity). Mitigation: spec changes go into a
`spec-revision/` branch, merged deliberately between phases. Mid-phase
spec edits are a re-plan trigger, not a shortcut.

**R3. Bevy 0.18 Messages/Events boundary.** Adding trace emitters and
new messages happens in both `SimulationPlugin::build()` and headless
`build_schedule` per CLAUDE.md ECS Rules. Mitigation: lint script greps
for `add_message` / `register_message` asymmetry in `just ci`.

**R4. Instrumentation overhead crashing 60 TPS.** §11.2's
focal-cat-replay strategy mitigates in design; measure in Phase 1's
acceptance to confirm.

**R5. §7.W axis-capture introduces runaway feedback loops.** Fulfillment
sensitization is positive-feedback; a badly-tuned sensitization curve
can produce infinitely-growing axis weight. Mitigation: hard cap on
`AxisState.value`; source-diversity decay acts as negative-feedback
counter-force; soak with `corruption_axis` forced on to exercise
worst-case. This is balance-thread work post-Phase-6 landing, not
pre-landing.

**R6. §7.M Fertility component interacts with existing `pregnancy.rs`.**
Fertility introduces a 5-phase lifecycle on top of pregnancy's binary
pregnant/not. Mitigation: `Fertility.phase = Postpartum` is the bridge
— `pregnancy.rs::give_birth` transitions the queen cat to Postpartum;
Postpartum transitions to Proestrus after the nursing interval. Spec
§7.M.7.1 documents the lifecycle; implementation follows.

**R7. Agent fan-out producing inconsistent DSE shapes.** Sub-agents
could port DSEs with subtly different trait shapes. Mitigation: Eat
reference (Phase 3b) is the template; each DSE-porter must pass a shape
conformance test; main-session concordance check reviews the reference
invariant before phase exit.

## Committed decisions

- **Plan-cost feedback shape** (§L2.10.7). Resolved in spec: candidate
  (a) `SpatialConsideration` + `replan_count` hard-fail second channel.
  Phase 4 is pure implementation.
- **Softmax scope + temperature** (§8). Resolved in spec:
  softmax-over-Intentions, T = 0.15 default, incumbent retained with
  bonus (§8.4 ordering). Phase 6 is pure implementation.
- **Mating architecture** (§7.M). Resolved in spec: three-layer nested
  Intention. Phases 3c / 4 / 5b implement L3 / L2 / L1 respectively.
- **Fertility state** (§7.M.7). Resolved in spec: 5-phase lifecycle,
  reproductive roles, cycle parameters. Phase 3c implements.
- **Faction model** (§9). Resolved in spec: 10×10 biological matrix + 4
  overlay markers + 5 DSE filter rows + most-negative-wins resolution.
  Phase 3d lands matrix; Phases 3c + 4 wire filters.
- **§7.W axis-capture** (§7.W). Resolved in spec: Fulfillment scalar,
  per-axis sensitization/tolerance/source-diversity, warring-self
  signal, Calling-as-canonical-instance, no meta-cognition primitive.
  Phase 6d implements.
- **Landing style.** Commit-per-DSE direct to `main` per CLAUDE.md
  solo-to-main. Each DSE port is its own commit; phase-kickoff and
  phase-exit balance docs are their own commits. Maximum bisect
  granularity — any regressed DSE revertable via `jj undo`. Log noise
  is accepted as the cost.
- **Pre-flight scope.** All gates clear before Phase 1, as one
  pre-flight session (6 items listed in Pre-flight section above;
  the draft's gate 5 moved to Phase 3c per Decisions log).

## Enumeration ledger — cross-reference

Every spec enumeration → its landing phase. A reader should be able to
verify at a glance that nothing dropped.

| Spec enumeration | Count | Landing phase |
|---|---|---|
| §2.1 curve primitives | 7 | Phase 3a |
| §2.3 curve assignment table | 30+ rows | Phase 3a (library) + 3c (per-DSE) |
| §2.3 retired constants | 8 fields | Phase 3c (burned with Logistic-curve DSEs that subsume them; see Decisions log) |
| §3.1 composition modes | 3 | Phase 3a |
| §3.1.1 per-DSE composition assignment | 30 DSEs | Phase 3c |
| §3.2 compensation factor | 1 formula | Phase 3a |
| §3.3.1 per-DSE weight-expression mode | 30 DSEs | Phase 3a (labels) + 3c (per-DSE) |
| §3.3.2 absolute-anchor peer groups | 8 | Phase 3a |
| §3.4 Maslow pre-gate | 1 wrapper | Phase 3a |
| §3.5.1 post-scoring modifiers | 7 | Phase 3a |
| §3.5.2 per-DSE applicability matrix | 21×7 | Phase 3a |
| §3.5.3 modifier bug fixes | 3 (Tradition, Fox-Flee, dead field) | Phase 3a |
| §4.1 tag categories | 11 | Phase 3a |
| §4.3 ECS marker catalog | ~44 markers | Phase 3a (catalog) + 3d (authoring gap-fill) |
| §4.4 ScoringContext crosswalk | 27+9 booleans → markers | Phase 3a |
| §4.5 scalar carve-out | 19+5 scalars | Phase 3a |
| §4.6 authoring-system roster | ~44 owner rows | Phase 3d |
| §5.6.2 sensory channels | 4 | Phase 2 |
| §5.6.3 influence maps | 13 (5 Partial ported, 8 Absent deferred) | Phase 2 |
| §5.6.4 propagation modes per channel | 4 | Phase 2 |
| §5.6.5 decay model per map | 13 rows | Phase 2 |
| §5.6.6.1 species × channel sensitivity | 10×4 = 40 cells | Phase 2 (wired from existing constants) |
| §5.6.6.2 role × channel modifier | 11×4 = 44 cells | Phase 2 (wired, identity today) |
| §5.6.6.3 injury × channel deficit | 13×4 = 52 cells | Phase 2 (wired, identity; active on body-zones epic) |
| §5.6.6.4 environment × channel multiplier | ~19×4 = 76 cells | Phase 2 (wired, identity; activation is balance work) |
| §5.6.9 extensibility contract | 1 | Phase 2 |
| §6.3 TargetTakingDse trait | 1 | Phase 4 |
| §6.4 personal-interest template | 9 DSEs | Phase 4 |
| §6.5 per-target considerations | 9 bundles × 4 = 36 rows | Phase 4 |
| §6.6 aggregation modes | 3 | Phase 4 |
| §7.1 CommitmentStrategy enum | 3 variants | Phase 3a (slot) + Phase 6a (semantics) |
| §7.2 drop-trigger gate | 1 gate + 3 proxies | Phase 6a |
| §7.3 per-DispositionKind strategy table | 12 rows | Phase 6a (11 rows) + Phase 5a (coordinator-directive row) |
| §7.4 persistence bonus | 1 formula + 12 `base` values | Phase 6a (formula) + balance iteration (12 values) |
| §7.5 Maslow interrupt events | 5 | Phase 6a (documented; existing code) |
| §7.6 monitoring cadence | 1 | Phase 6a (documented) |
| §7.7 aspiration reconsideration classes | 5 (a–e) | Phase 6b |
| §7.7.1 compatibility matrix | Deferred | Phase 6b (scaffolding) + aspiration-catalog stabilization (matrix) |
| §7.M three-layer Mating | L1/L2/L3 | L3 Phase 3c; L2 Phase 4; L1 Phase 5b |
| §7.M.7 Fertility state | 5 phases + lifecycle + roles | Phase 3c |
| §7.W axis-capture primitive | Fulfillment + 3 dynamics + warring-self + 2 worked examples + 4 free consequences | Phase 6d |
| §8.1 softmax algorithm | 1 | Phase 6c |
| §8.2 scope | 1 (softmax-over-Intentions) | Phase 6c |
| §8.3 temperature | T = 0.15 | Phase 6c (also pre-flight gate 5 — new `fox_softmax_temperature` field) |
| §8.4 order with §7 momentum | 1 | Phase 6c |
| §8.5 fox convergence | 1 | Phase 6c (constant landed pre-flight; code in Phase 3c retires old path) |
| §8.6 apophenia continuity canary | 1 | Phase 1 (schema) + Phase 6c (operationalized) |
| §9.1 biological base matrix | 10×10 = 100 cells | Phase 3d |
| §9.2 ECS-marker overlay | 4 markers + resolution order | Phase 3d |
| §9.3 DSE filter binding | 5 rows | Phase 3c |
| §L2.10.2 Dse trait | 1 | Phase 3a |
| §L2.10.3 DSE registration catalog | 45 entries across 5 methods | Phase 3a (builder) + Phase 3c (action DSEs 21 cat + 9 fox + 9 sibling = 39) + Phase 5 (aspiration 2 + coordinator 2 + narrative 1 + target-taking 9) |
| §L2.10.4 Intention | Goal/Activity + CommitmentStrategy tag | Phase 3a |
| §L2.10.5 Termination types | 3 variants | Phase 3a |
| §L2.10.6 softmax-over-Intentions | 1 | Phase 6c |
| §L2.10.7 SpatialConsideration + replan_count | 1 | Phase 4 |
| §L2.10.10 Herbcraft/PracticeMagic sibling-DSE curve specs | 9 siblings | Phase 3c |
| §10 feature unblock map | 10+ system rows | Phase 7 |
| §11.3 three record formats | L1/L2/L3 | Phase 1 |
| §11.4 joinability invariant | 1 header contract | Phase 1 |
| §11.5 focal-cat gate | 1 | Phase 1 |
| §12.3 belief proxies | 3 | Phase 3a (named interfaces) + Phase 6a (drop-trigger gate consumers) |

## Critical files — landing order

(Not a file list, a landing order.)

1. `scripts/replay_frame.py`, `scripts/frame_diff.py` — new (Phase 1)
2. `src/resources/trace_log.rs`, `src/systems/trace_emit.rs` — new
   (Phase 1)
3. `src/main.rs`, `src/plugins/simulation.rs` — additive (Phase 1)
4. `src/systems/influence_map.rs` — new (Phase 2)
5. `src/ai/dse.rs`, `src/ai/curves.rs`, `src/ai/eval.rs`,
   `src/ai/markers.rs`, `src/ai/composition.rs`, `src/ai/faction.rs` —
   new (Phase 3a)
6. `src/components/fertility.rs`, `src/systems/fertility.rs` — new
   (Phase 3c)
7. `src/ai/dses/*.rs` — new (Phase 3c; target refinement Phase 4;
   scattered sites Phase 5)
8. `src/ai/scoring.rs` — deleted over Phase 3c as DSEs port; residue
   deleted Phase 7
9. `src/ai/fox_scoring.rs` — deleted Phase 3c
10. `src/ai/target_taking.rs` — new (Phase 4)
11. `src/ai/commitment.rs`, `src/ai/persistence.rs`, `src/ai/softmax.rs`
    — new (Phase 6a / 6c)
12. `src/components/fulfillment.rs`, `src/systems/fulfillment.rs` — new
    (Phase 6d)
13. `src/resources/sim_constants.rs` — warmth→temperature rename
    (pre-flight gate 4); `fox_softmax_temperature` added
    (pre-flight gate 5); faction matrix added Phase 3d; retired
    §2.3 constants burned in Phase 3c; persistence-bonus `base`
    values Phase 6a.

## Verification summary

End-to-end test after each phase, via `just autoloop 42`:

1. Build passes (`just check`).
2. `cargo test` passes (all 3 pre-existing failures fixed in
   pre-flight).
3. 15-min seed-42 soak completes without wipeout.
4. `just check-canaries logs/tuned-42/events.jsonl` exits 0.
5. `just check-continuity logs/tuned-42/events.jsonl` exits 0
   (Phase 1+).
6. `just frame-diff` emits ≤ ±10% drift on characteristic metrics OR
   drift is documented in `docs/balance/substrate-phase-N.md` with the
   four-artifact rule satisfied.
7. `just diff-constants` against prior phase's artifact: zero diff on
   out-of-scope constants, expected diff on in-scope constants.

Multi-seed sweep (99 / 7 / 2025 / 314) at phase exit.

Reference: CLAUDE.md "Simulation Verification" block; spec §11.
