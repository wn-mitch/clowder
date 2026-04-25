---
id: 014
title: Phase 4 follow-ons — target-taking registration + markers + mate-gender + Mating/PracticeMagic magnitude
status: in-progress
cluster: null
added: 2026-04-22
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

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
  (see Landed).
  **Phase 4c.7 landed** — Caretake (§6.5.6) full TargetTakingDse
  port; `resolve_caretake` retired in favor of the spec-shape
  four-axis resolver. With 4c.7 the §6.5 per-DSE target-taking
  slate is **closed**. Each port followed the Phase 4c.1 pattern:
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
  cleared gate with no starvation regression.
  ~~The full §6.5.6 Caretake `TargetTakingDse` port remains
  outstanding~~ **Landed as Phase 4c.7** — the `resolve_caretake`
  plain helper retired in favor of `caretake_target_dse()` +
  `resolve_caretake_target` on the Socialize / Mate / Mentor
  pattern (`KittenFed = 27` on the landing soak, within the
  seed-42 noise envelope; see Landed).
- **§4 marker-eligibility authoring systems for roster gap-fill.**
  **Phase 4b.2 MVP + 4b.4 + 4b.5 landed** (lookup foundation +
  `HasStoredFood` + `HasGarden` + colony-scoped batch
  `HasFunctionalKitchen` / `HasRawFoodInStores` / `WardStrengthLow`
  — see Landed section below). **`Incapacitated` per-cat author
  landed 2026-04-23** (`systems::incapacitation::update_incapacitation`,
  first `set_entity` consumer) + **consumer cutover landed
  2026-04-23** via the §13.1 rows 1–3 commit:
  `.forbid("Incapacitated")` on every non-Eat/Sleep/Idle cat DSE
  + every fox DSE, inline `if ctx.is_incapacitated` branch at
  `scoring.rs:574–598` retired. **Marker string keys centralized
  2026-04-24** — `impl Marker { pub const KEY: &str }` on each
  marker component in `src/components/markers.rs`; all ~97 raw
  string call sites replaced with `Marker::KEY` constants (typo →
  compile error). **LifeStage markers authored 2026-04-24** —
  `growth.rs::update_life_stage_markers` maintains exactly one of
  {Kitten, Young, Adult, Elder} per living cat;
  `MarkerSnapshot` population wired in both scoring loops;
  `mate.rs` gates on `.forbid(Kitten::KEY).forbid(Young::KEY)`.
  Chain 2 split into 2a/2b sub-chains (Bevy 20-system tuple limit);
  `MarkerQueries` SystemParam bundle created for future batches.
  **Batch 1 landed 2026-04-24** — 3 per-cat author systems
  (`update_injury_marker`, `update_inventory_markers`,
  `update_directive_markers`) + colony-scoped shared helpers
  (`scan_colony_buildings`, `is_ward_strength_low`). 5 new KEY
  constants, `MarkerQueries::per_cat` extended, ScoringContext
  fields read from MarkerSnapshot, coordinate DSE
  `.require("IsCoordinatorWithDirectives")` cutover. 31 tests.
  `Injured` marker unblocks Capability markers (batch 2).
  **Batch 2 landed 2026-04-24** — `src/ai/capabilities.rs` authors
  4 capability markers (`CanHunt`, `CanForage`, `CanWard`, `CanCook`)
  with spec-intent life-stage rules: Young cats hunt (badly) and
  forage, Elders forage only, Kittens excluded from all, Injured
  excluded from all. DSE `.require()` cutover on Hunt, Forage,
  HerbcraftWard, Cook. Retired `can_hunt`, `can_forage`,
  `has_ward_herbs` from `ScoringContext` + inline gates. 23 tests.
  The remaining ~30 §4.3 markers each need:
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

**Balance-tuning observations — deferral status updated 2026-04-25.**

Several positive-feature metrics remain below their literal
Phase 4 exit targets. The 2026-04-25 multi-soak baseline dataset
(`logs/baseline-2026-04-25/`, 27 footer-complete runs at commit
`cba19bd`) supersedes the single-seed snapshot at
`logs/phase4b4-db7362b/events.jsonl` and resolves several deferral
predicates. Updated status per metric:

- ~~MatingOccurred = 0 (literal target ≥ 7 per 7-season soak).~~
  **No longer deferred.** Diagnosed across 15 sweep runs: the gap
  is structural, not coefficient-tunable. Three layered bugs
  (lifted-condition outer gate at `scoring.rs:916`, missing L2
  PairingActivity per §7.M, misnamed `CourtshipInteraction`
  canary). Active work tracked in
  [ticket 027](027-mating-cadence-three-bug-cascade.md).
  Substrate stability predicate satisfied per the §6.5 target-taking
  port closeout in Phase 4c.7 + §7.2 commitment-gate landing in
  Phase 6a — the "wait for substrate" justification no longer holds.
- PracticeMagic sub-mode count = 2 / 5 (literal target ≥ 3 / 5).
  **Status revision.** Baseline shows `CleanseCompleted` mean 215.7
  across 15/15 runs, `CarcassHarvested` mean 6.3 (12/15 non-zero),
  `SpiritCommunion` mean 0.6 (6/15 non-zero, dormant per §6.3
  spatial-target routing). Cleanse and Harvest are firing
  vigorously; only Commune remains dormant. The "2/5" framing
  needs updating against the new measurement.
- Farming = 0 (literal target ≥ 1).
  **Resolved.** `CropTended` mean 17,191.6 across 14/15 runs;
  `CropHarvested` mean 873.7 across 13/15 runs. The original
  measurement at `phase4b4-db7362b` predated marker authoring
  for `HasGarden` / `HasFunctionalKitchen`. Farming is no longer
  a deferred metric.

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

**Updated commitment (2026-04-25):** the substrate-stable predicate
is satisfied. Mating cadence has its own ticket (027) covering the
three structural bugs blocking it. Magic sub-mode density now
narrows to Commune-only, blocked by §6.3 spatial-target routing
(track separately if a magic balance thread opens). Farming is
resolved. The soak-footer trend tracking continues but is no longer
a "wait for substrate" item.

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
