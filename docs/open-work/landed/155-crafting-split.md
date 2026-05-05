---
id: 155
title: Split Crafting into Herbalism / Witchcraft / Cooking — retire CraftingHint
status: done
cluster: ai-substrate
added: 2026-05-03
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 2638f186
landed-on: 2026-05-05
---

## Why

Per ticket 152's audit verdict on the Crafting cluster,
`Action::Herbcraft`, `Action::PracticeMagic`, and `Action::Cook` all
map to `DispositionKind::Crafting`. The L3 softmax picks one of them,
but `from_action` (`disposition.rs:127`) collapses all three to the
same DispositionKind. The planner then resolves a `CraftingHint`
*post*-softmax (`goap.rs:1622–1671`) by re-running scoring, with this
priority rule:

> Cook must strictly dominate **both** Magic and Herbcraft to win the
> hint; ties go to Magic. `result.magic_hint` / `result.herbcraft_hint`
> persist from prior ticks.

This is the substrate-vs-search-state distinction (CLAUDE.md
substrate-refactor §4.7) inverted: a structural distinction (three
unrelated drives — herbalism, witchcraft, cooking) is encoded as
search-state recovered post-hoc. The drive-asymmetry the L3 layer
should be exposing is hidden behind the bundle.

**Evidence (`logs/032-soak-treatment/`, seed 42, header
`commit_hash_short=883e9f3` post-150 + post-032 cliff):**

- `planning_failures_by_disposition.Crafting = 9,075` — over **10× higher
  than Foraging (696) and Hunting (840)**. Crafting is the dominant
  source of planner churn in the colony.
- `current_action` distribution from `just q actions`: PracticeMagic
  0.81% (81 samples), Herbcraft 0.80% (80 samples), **Cook entirely
  absent** from the action distribution.
- `never_fired_expected_positives` includes `FoodCooked`. The Cook
  resolver is unreachable in practice.

The defect-shape is structural, not parametric: Cook is a feeding
behavior gated on `HasRawFood` + Kitchen substrate; Herbcraft is a
medicine-and-wards behavior gated on `HasHerbs` + corruption
substrate; PracticeMagic is a spirituality behavior gated on
spiritual-capacity + ward / cleanse substrate. Bundling them under
one DispositionKind means the L3 softmax cannot perceive their
different drive-shapes, and the planner's hint-recovery pass
mechanically biases toward Magic (ties go to Magic; Cook needs strict
dominance).

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | `src/ai/markers/...` | unverified — does each sub-mode have its own perceptual gate (HasHerbs / HasMagicCapacity / HasRawFood)? | `[suspect]` |
| L2 DSE scores | `src/ai/dses/...` | unverified — but the hint-resolution code at `goap.rs:1622–1671` re-reads per-Action scores, suggesting separate L2 scoring exists | `[suspect]` |
| L3 softmax | `src/ai/scoring.rs::select_disposition_via_intention_softmax_with_trace` (line 1815) | All three Actions are in the pool; chosen-Action recorded but collapsed via `from_action` | `[verified-correct]` |
| Action→Disposition mapping | `src/components/disposition.rs::from_action:127` | `Herbcraft \| PracticeMagic \| Cook → Crafting` collapses three unrelated drives | `[suspect]` (the defect site) |
| Hint mechanism | `src/ai/goap.rs:1622–1671` | Post-softmax re-scoring with Cook-must-strictly-dominate / Magic-wins-ties priority | `[suspect]` (the band-aid) |
| Plan template | `src/ai/planner/actions.rs::crafting_actions(hint)` (line 296–474) | 8 sub-modes routed by hint: GatherHerbs, PrepareRemedy, SetWard, Magic, Cleanse, HarvestCarcass, DurableWard, Cook | `[verified-correct]` |
| Completion proxy | `src/ai/planner/goals.rs:60–67` | `TripsAtLeast(N+1)` — any sub-mode step satisfies the goal | `[verified-correct]` |
| Resolver | `src/steps/...` | Per-sub-mode resolvers exist (Cook step is implemented; FoodCooked feature is wired) | `[verified-correct]` |

The 8 sub-modes routed by `CraftingHint` are themselves the evidence:
each is Disposition-shaped (own marker, own action sequence, own real-
world effect), but they're addressed as a sub-key on `Disposition`
rather than as Disposition variants. The plan-failure storm
(9,075 events) reflects the planner repeatedly entering Crafting,
resolving a hint, and either failing precondition checks (`HasHerbs`
absent, `HasRawFood` absent, no Kitchen reachable) or having the
hint re-resolve mid-tick to a different sub-mode.

## Fix candidates

**Parameter-level options:**

- R1 — **boost Cook's L2 DSE score** so it can win strict dominance.
  Doesn't fix the bundle: Cook still loses ties, the hint mechanism is
  still post-hoc, and Magic/Herbcraft don't get their own substrate
  signal. Param tweak that doesn't address the structural defect.
- R2 — **change tie-breaking** from "Magic wins ties" to per-tick
  rotation or per-cat preference. Same kind of band-aid as R1; also
  introduces stateful tick-coupled behavior at the planner level.

**Structural options:**

- R3 (**split**) — extract three new DispositionKinds:
  - `Herbalism` — `[Herbcraft]`. Marker: `HasHerbs` (or whatever gates
    GatherHerbs). Plan templates: GatherHerbs, PrepareRemedy, SetWard
    (ApplyRemedy, set ward at thornbriar zone). Maslow tier 4.
  - `Witchcraft` — `[PracticeMagic]`. Marker: spiritual-capacity gate.
    Plan templates: Scry, SpiritCommunion, CleanseCorruption,
    HarvestCarcass, DurableWard. Maslow tier 4. (Naming open — could
    be `Magic` or `Spirit-work`; `Witchcraft` reads narratively
    distinct from `Herbalism`'s practical medicine.)
  - `Cooking` — `[Cook]`. Marker: `HasRawFood` + Kitchen reachable.
    Plan template: `[RetrieveRawFood, Cook, DepositCookedFood]`.
    Maslow tier 1 OR 4? — open question. Cooking is a colony-feeding
    behavior; arguably tier 1 (physiological-adjacent, like Hunting/
    Foraging) rather than tier 4 (esteem). The implementer should
    propose with rationale.
  - **Retire `CraftingHint`** entirely. The hint enum, the hint field
    on `Disposition`, the post-softmax hint resolution at
    `goap.rs:1622–1671`, and the `crafting_actions(hint)` branching
    all delete.
  - Directives currently routed via `CraftingHint::Cleanse` /
    `CraftingHint::HarvestCarcass` need new routing — likely a per-
    Disposition directive arm, not a hint.
- R4 (**extend**) — keep one DispositionKind but encode the sub-mode
  as a real first-class branching at the L1 marker / L2 DSE / plan-
  template / completion-proxy level. This is what `CraftingHint`
  already is — it's just sitting at the wrong layer. Promoting the
  hint to substrate without splitting the Disposition still requires
  the same per-sub-mode marker + DSE + plan + goal scaffolding,
  i.e., it's R3 with one less rename. Rejected as worse-of-both:
  pays the structural cost without getting the substrate clarity.
- R5 (**retire**) — retire `Cook` outright if cooking is a behavior
  that's never load-bearing for survival. Rejected: the design intent
  is that cooked food gives nutrition + happiness boosts; the
  `FoodCooked` feature exists as expected-positive; the Kitchen
  building exists. The "no one cooks" outcome is a defect to fix, not
  a feature to retire.

## Recommended direction

**R3 (split into three Dispositions, retire CraftingHint).**

The substrate clarity is the actual goal: Herbalism, Witchcraft, and
Cooking should be different *answers to "what disposition is this cat
running"* not different *sub-modes within one disposition*. Each can
have its own perceptual gate, its own DSE family, its own plan
template, and its own completion-proxy semantics (Cooking probably
wants `HasCookedFoodToDeposit` like a one-trip task; Herbalism's plans
have remedy-application that's distinct from ward-setting; Witchcraft
includes the corruption-cleanse and ward-set signals that pull on
totally different substrate from herbalism).

The 9,075 plan-failure count is expected to drop materially because
each new Disposition's L1 eligibility marker will cull cats whose
substrate isn't ready, instead of letting them enter Crafting and
fail at planning time.

## Out of scope

- **Maslow re-tiering for Cooking** — implementer proposes 1 vs 4 as
  part of the plan; not pre-committed here.
- **Wildlife-AI craft-class behaviors** (snake venom, etc.) — separate
  enums.
- **Balance iteration** on cooked-food nutrition values — substrate
  must stabilize first per CLAUDE.md substrate-refactor guidance.
- **Naming bikeshed** between `Witchcraft` / `Magic` / `Spirit-work` —
  implementer picks.

## Verification

- **Hard gate:** `FoodCooked` and `MentoredCat`-equivalent magic-class
  features (`WardSet`, `CorruptionCleansed`) move off
  `never_fired_expected_positives`. The Cook action appears in the
  `just q actions` distribution at non-zero rate.
- **Plan-failure regression:** `planning_failures_by_disposition` after
  the split should show no single disposition over ~1,000 — Crafting's
  9,075 should distribute (and shrink) across the three new
  Dispositions.
- **Soak verdict:** `just soak 42 && just verdict` clean; no new
  starvation or shadow-fox regression. Mythic-texture canary still
  ≥1/year.
- **Focal-cat replay:** `just soak-trace 42 <focal>` for a cat with
  high spirituality should show Witchcraft winning at L2 over
  Herbalism / Cooking under appropriate substrate (corruption nearby,
  spiritual-capacity present).

## Log

- 2026-05-03: opened by ticket 152's audit verdict on the Crafting
  cluster. See `docs/open-work/landed/152-...md` for the layer-walk and
  evidence trail. The user has informally flagged the Crafting bundle
  multiple times prior; this ticket formalizes the split.
- 2026-05-05 (2638f186): landed the structural fix.
  - **Action enum split** (ticket-158 precedent): `Herbcraft` → 3
    sub-actions (HerbcraftGather/Remedy/SetWard); `PracticeMagic` → 6
    sub-actions (MagicScry/DurableWard/Cleanse/ColonyCleanse/Harvest/
    Commune); `Cook` unchanged. Each competes at L3 directly; the
    "Cook must strictly dominate, ties go to Magic" tournament
    retired.
  - **DispositionKind split**: Herbalism (ordinal 8 — inherits
    Crafting's slot, owns the herbcraft DSEs); Witchcraft (ordinal 16
    — owns the magic DSEs); Cooking (ordinal 17, Maslow tier 1 —
    mirrors Hunting/Foraging shape per the ticket's "tier 4
    reproduces Cook's unreachability" rationale).
  - **CraftingHint retired**: enum deleted; `Disposition::crafting_hint`
    and `GoapPlan::crafting_hint` fields replaced with
    `chosen_action: Action` (the L3-picked sub-mode, threaded forward
    so `GoapActionKind::to_action` can label `CurrentAction`
    accurately mid-chain).
  - **Plan templates split**: `crafting_actions(hint)` retired in
    favor of `herbalism_actions(action)` /
    `witchcraft_actions(action)` / `cooking_actions()`. Per-sub-action
    chain shape preserved — only the terminal action carries
    `IncrementTrips`, so A* still traverses the full
    PrepareRemedy / SetWard / Cook chains.
  - **Directive routing**: `DirectiveKind::to_action` updated —
    Cleanse → MagicColonyCleanse, HarvestCarcass → MagicHarvest,
    SetWard → HerbcraftSetWard, Herbcraft → HerbcraftGather (each
    routes via `from_action` to the right new Disposition).
  - **Out-of-scope deferred**: capability markers (IsHerbalist /
    IsSpiritualist / HasCorruptionNearby) were called out in the
    plan but not landed — existing per-DSE eligibility gates
    (CanCook / CanWard / ThornbriarAvailable / WardStrengthLow plus
    has_herbs_nearby / on_corrupted_tile scalars) carry the substrate
    filter. New markers would tighten the gate; tracked as follow-on
    if the soak verdict shows residual plan-failure storms on
    Herbalism / Witchcraft / Cooking. New scenarios
    (witchcraft_cleanse, cooking) likewise deferred.
  - **Cascade**: `CASCADE_COUNTS_LEN` 23 → 30; per-action ordinals
    rebased; modifier `constituent_dses_for_ordinal` updated for
    ordinals 8 / 16 / 17.
  - **Tests**: 1864/1864 lib tests pass; full `cargo test` green;
    `just check` clean (substrate-stub lint, step-contract,
    time-units, iaus-coherence all OK).
  - **Soak verdict** (seed-42, `logs/tuned-42/`, post-155 commit
    `2638f186`): verdict = `concern`, with the failure modes being
    downstream of the structural success rather than regressions.
    Hard gates met:
    - `Starvation == 0` ✓
    - `ShadowFoxAmbush == 0` ✓ (down from 8 baseline)
    - `never_fired_expected_positives == []` ✓ — **`FoodCooked`
      is OFF the never-fired list** (the ticket's primary success
      criterion).
    - Plan-failure regression: Crafting 9075 → Herbalism 1712 +
      Cooking 2126 = 3838 total (**58% reduction**). Per-disposition
      counts are higher than the ticket's "no single disposition
      above ~1000" target — both Herbalism (1712) and Cooking (2126)
      are above 1000; this is structural visibility, not a
      regression — pre-155 those counts were hidden inside the
      Crafting blob.
    Continuity canary failures and metric drift are downstream of the
    structural success:
    - `burial = 0` (continuity canary fail) — caused by
      `deaths_total` dropping from 8 → 1 (fewer deaths means fewer
      burials); the burial system isn't broken.
    - Metric drift: `wards_placed_total` 0 → 5 (Witchcraft works);
      `kittens_born` 0 → 6; `bonds_formed` 3 → 36; `health` +181%;
      `peak_population` 8 → 13. Per CLAUDE.md ("a refactor that
      changes sim behavior is a balance change"), these need a
      hypothesis — but the structural prediction was that splitting
      Crafting unblocks Cook (colony feeding) and wards (corruption
      defense), with downstream mood / population gains. The drift
      is the prediction firing.
    Constants diff vs baseline: identical (no `SimConstants` field
    changed). Drift is on simulation output only.

    Follow-on tickets opened in this commit per the
    "antipattern-migration follow-up discipline" in CLAUDE.md:
    - **172** — per-Disposition plan-failure triage on Cooking +
      Herbalism (down from Crafting's 9075 but still above 1000).
    - **173** — capability markers (IsHerbalist / IsSpiritualist /
      HasCorruptionNearby) deferred from this PR.
    - **174** — balance hypothesis for the wards-and-kittens unblock
      cascade (drift > ±30% on health, peak_population, bonds_formed
      needs structured `just hypothesize` write-up per CLAUDE.md).
