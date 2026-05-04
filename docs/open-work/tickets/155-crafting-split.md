---
id: 155
title: Split Crafting into Herbalism / Witchcraft / Cooking — retire CraftingHint
status: ready
cluster: ai-substrate
added: 2026-05-03
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
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
