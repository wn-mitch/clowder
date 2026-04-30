---
id: 2026-04-20
title: Phase 3a–3d — AI substrate trunk + per-DSE ports + §9 faction model
status: done
cluster: null
landed-at: 03e9b23
landed-on: 2026-04-20
---

# Phase 3a–3d — AI substrate trunk + per-DSE ports + §9 faction model

Four sub-phases landing the full A1/A3/A4 substrate commitment from
cluster A #5. Per-sub-phase landings + seed-42 soak numbers live in
`docs/balance/substrate-phase-3.md` (783 lines, canonical detail);
this entry is the open-work cross-reference.

- **Phase 3a — L2 primitives + Dse trait + §4 marker catalog + §9
  faction model** (commits `03e9b23`, `01cb6e7`, `e02121f`, `1a50d30`).
    - `src/ai/curves.rs` — §2.1 `Curve` enum (Linear, Quadratic,
      Logistic, Logit, Piecewise, Polynomial, Composite), §2.2
      function-evaluated, §2.3 named anchors (`hangry`, `sleep_dep`,
      `loneliness`, `scarcity`, `flee_or_fight`, `inverted_need_penalty`,
      `fight_gating`).
    - `src/ai/considerations.rs` — §1.1 trait + §1.2 three flavors
      (`ScalarConsideration`, `SpatialConsideration`,
      `MarkerConsideration`).
    - `src/ai/composition.rs` — §3.1 modes (`CompensatedProduct`,
      `WeightedSum`, `Max`); §3.2 compensation factor
      (`DEFAULT_COMPENSATION_STRENGTH = 0.75`); §3.3.1 RtM / RtEO
      weight-mode enforcement at construction.
    - `src/ai/dse.rs` — `Dse` trait, `DseId`, `Intention` (Goal /
      Activity), `EligibilityFilter`, `EvalCtx`,
      `CommitmentStrategy` enum (Blind / SingleMinded / OpenMinded;
      Phase 3a commits the tag, §7 semantics lands later).
    - `src/components/markers/` — 49 ZST marker components covering
      the §4.3 catalog (Species / Role / LifeStage / State / Capability
      / Inventory / TargetExistence / Colony / SpawnImmutable
      categories).
    - `src/ai/faction.rs` — §9.1 biological 10×10 base matrix +
      §9.2 ECS-marker overlay resolver. Stub for Phase 3d's stance
      bindings.
- **Phase 3b — unified evaluator + modifier pipeline + Eat reference
  DSE** (commits `d9cf47e`, `afe22f5`).
    - `src/ai/eval.rs` — `DseRegistry`, `DseRegistryAppExt` (six
      registration methods per §L2.10.3 catalog), `ScoreModifier`
      trait + `ModifierPipeline`, `evaluate_single`,
      `evaluate_all_cat_dses`, `select_intention_softmax` stub (wired
      in 4a), `ScoredDse` output type. §3.4 Maslow pre-gate wired
      via `evaluate_single` closure accepting `Fn(u8) → f32` so
      `Needs::level_suppression` is preserved bit-for-bit.
    - `src/ai/dses/eat.rs` — reference port per §2.3's hangry anchor
      (`Logistic(8, 0.75)` on hunger); registered at all three
      mirror sites.
- **Phase 3c — peer-group per-DSE ports + Herbcraft/PracticeMagic
  sibling splits** (commits `91e6b56` through `60acb31`).
    - **3c.0** — `EvalInputs` bundle threaded through `score_actions`
      + `ctx_scalars` map centralizing the canonical scalar surface
      (semantic inversion bug-fix: `hunger` scalar = `1 − needs.hunger`).
    - **3c.1a/b** — Starvation-urgency peer group (cat + fox): Eat,
      Hunt, Forage, Cook, fox Hunting, Raiding.
    - **3c.2** — Fatal-threat peer group: Flee, Fight, Patrol, fox
      Fleeing, Avoiding, DenDefense.
    - **3c.3** — Rest-urgency peer group: Sleep, Idle, fox Resting.
    - **3c.4** — Social-urgency peer group: Socialize, Groom(other),
      Mentor, Caretake, Mate.
    - **3c.5+6+7** — Territory, Work, Exploration peer groups: cat
      Patrol, Build, Farm, Coordinate, Explore, Wander; fox Patrolling.
    - **3c.8** — fox Lifecycle + Feeding ports (Dispersing, Feeding).
    - **3c.last** — Herbcraft split (gather / prepare / ward) +
      PracticeMagic split (scry / durable_ward / cleanse /
      colony_cleanse / harvest / commune) per §L2.10.10's sibling-DSE
      resolution of the retiring `Max` composition.
    - Net result: all 21 cat DSEs + 9 fox dispositions resolved
      through the L2 evaluator's registry. `src/ai/dses/` holds the
      per-DSE factories (39 files at Phase 3c exit). **Note:
      "resolved through the registry" is not "every §2.3-assigned
      curve is in place."** Peer-group ports established DSE
      shapes + composition modes + the canonical anchors (`hangry`,
      `sleep_dep`, `scarcity`, `loneliness`, `flee_or_fight`,
      `fight_gating`, `inverted_need_penalty`, day-phase
      Piecewise). Five corruption-axis migrations in the
      Herbcraft/PracticeMagic sibling DSEs remain at
      `Curve::Linear` placeholders pending the §2.3-rows-4–6
      migration commits (see #5 Track B bullet). This is
      deliberate — axis-level curve migrations are the A1.3 "first
      measured curve shift" phase in the kickoff plan, downstream
      of the substrate trunk.
- **Phase 3d — §9.3 stance bindings + Fertility component +
  §7.M.7.2 phase transitions** (commits `c8bb1c6`, `562c575`).
    - Stance bindings on five target-taking DSEs (anticipatory for
      Phase 4b.3's TargetTakingDse foundation).
    - `Fertility` component with cycle-phase transitions driving
      the Mating aspiration's biological substrate (spec §7.M.7).

**§4 marker catalog foundation landed with Phase 3a (49 ZST components).**
Per-marker author-system rollout + lookup-snapshot wiring is a
separate track tracked in #14 and in #5's Track C bullet. Six
markers authored total: five colony-scoped (`HasStoredFood`,
`HasGarden`, `HasFunctionalKitchen`, `HasRawFoodInStores`,
`WardStrengthLow` — populated by `MarkerSnapshot` builders in
`systems/goap.rs` + `systems/disposition.rs`) plus one per-cat
(`Incapacitated` — authored by a dedicated
`src/systems/incapacitation.rs::update_incapacitation` system,
2026-04-23, first per-entity marker to use the `set_entity` API).
43 markers still unauthored — all life-stage /
capability / target-existence markers + most colony markers.

**Phase 3 exit deep-soak (seed 42, `--duration 900`, release,
commit `039c6fb`):** survival canaries held (Starvation = 8, three
Phase-3-exit regressions surfaced: MatingOccurred = 0, PracticeMagic
sub-modes 2/5, Farming = 0). The three regressions were resolved in
Phase 4a (softmax-over-Intentions + §3.5 modifier port) per the
substrate-phase-4.md thread; the MatingOccurred density + dormancy
gaps deferred per #14's balance-tuning-after-refactor commitment.

**Cluster A status update (post-Phase 4a):** the A1 IAUS refactor
(cluster A #5's TOP PRIORITY entry), A3 context-tag uniformity, and
A4 target-selection-as-inner-optimization are all substantially
landed via Phase 3a–4c — see cluster A preamble status line for
details.

---
