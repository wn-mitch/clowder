---
id: 016
title: Crafting — items, recipes, stations (epic dashboard)
epic: true
status: in-progress
cluster: items-crafting
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-04-22
parked: null
blocked-by: []
supersedes: []
related-systems: [crafting.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

§5-first crafting anchor. External proposal (user-sourced via `/rank-sim-idea`) split out from a composite "OSRS-style inventory + fantasy adventures" idea on 2026-04-22. The §5-first recipe catalog (preservation, play toys, grooming tools, courtship gifts, mentorship tokens) targets the ecological-variety continuity canary; Phase 3 produces wearables that unblock #17.

Promoted to an epic dashboard 2026-05-16. This ticket is now read-only over its child tickets — it doesn't own work, it owns *visibility*. Each shippable unit lives in its own ticket; this file is the dashboard that answers "what's left in the crafting layer?" in one read. Pattern mirrors 128 (HTN method composition).

Full architectural design lives in [`docs/systems/crafting.md`](../../systems/crafting.md). This body is deliberately thin on architecture — it's the dashboard.

## Phase coverage map

| Phase | Title | State | Ticket | Blocked by |
|---|---|---|---|---|
| 1a | Crafting substrate — Recipe / Station / CraftAction; generalize remedy_prep + ward_setting | done | [365](../landed/365-crafting-substrate-recipe-station-craftaction-generalize-remedy-prep-and-ward-setting-016-phase-1a.md) | — |
| 1b | Preservation recipes + Drying Rack + Smoking Rack | done | [367](../landed/367-phase-1-preservation-recipes-dried-fish-smoked-meat-preserved-organ-drying-rack-and-smoking-rack-stations-016-phase-1b.md) | — |
| 2 | §5 behavioral tools (Grooming Brush, Play Bundle, Courtship Gift) | ready | [368](368-phase-2-behavioral-tools-grooming-brush-play-bundle-courtship-gift-016-phase-2.md) | — |
| 2b | Warrior's kit (8 items, Tanning Frame, material-property substrate) | ready | [369](369-phase-2b-warriors-kit-8-items-tanning-frame-station-material-property-substrate-for-huntcombatnoise-resolvers-016-phase-2b.md) | — |
| 3 | Identity, mentorship, adornment (first wearable producer) | blocked | [370](370-phase-3-identity-mentorship-adornment-mentorship-token-heirloom-piece-shell-collar-bone-and-wire-tiara-stone-set-pin-016-phase-3-first-wearable-producer.md) | 368, 17 |
| 4 | Domestic refinement — place-anchored decorations | blocked | [371](371-phase-4-domestic-refinement-place-anchored-decorations-reed-mat-tallow-lamp-scent-censer-carved-comb-wall-hanging-nesting-inlay-016-phase-4.md) | 370 |
| 5-prereq | Aspirations mastery arcs (Weaving, BoneShaping, Hidework, Pigment, Cairn) | done | [366](../landed/366-aspirations-mastery-arcs-weaving-boneshaping-hidework-pigment-cairn-016-phase-5-precursor.md) | — |
| 5 | Elevated cat-craft (collective / multi-season, triple-gated) | blocked | [372](372-phase-5-elevated-cat-craft-generational-tapestry-shrine-cairn-bone-lattice-lantern-pigment-deepened-textile-multi-cat-nesting-alcove-kitten-cradle-basket-016-phase-5-triple-gated.md) | 371 |

## Related existing tickets

- **173** (parked) — `crafting-split-capability-markers`. Un-park or close decision happens in the first commit of 365's session (capability markers may fall out of the unified Recipe substrate naturally).
- **309** (blocked-by 308) — Herbcraft DSE reserve-deficit consideration / anticipatory crafting. Phase 1.5 extension; remains gated on 308 (ColonyReservesBelief).
- **334** (blocked-by [17]) — Stealth-cloak crafting recipe + WearItem resolver. 128 epic Tier-2 glue; scope may shrink to pure HTN-method-flip once 369 lands the WearItem resolver. Decide during 369 session.

## Design constraints

Load-bearing — drift toward random/decoupled stat-stick fields or RNG affix rolls re-triggers ranking (F→2, H→2, score → ~96); identity/material-grounded effect-data composed by the aggregation layer is the sanctioned shape (doctrine corrected in 476). Quick anchors (see [`docs/systems/crafting.md`](../../systems/crafting.md) §Design constraints for the full statement):

- §5-first catalog. Combat gear (spears, bracers, blades, slings) included as Phase 2b recipe cluster.
- Items are characterization, not commodity, but they have bite — real mechanical effects keyed to item identity/material (+ quality), composed by the uniform modifier-aggregation layer (ticket 477) and applied in resolvers, never as random stat-sticks.
- Decorations are place-anchored, not cat-anchored (Phase 4+).
- Cat-native materials palette: reed, bone, fur, feather, shell, fat, pigment, hide, sinew, flint, fieldstone. Metal arrives via `ScavengedMetal` / `TradedMetal` — never produced by a cat discipline.
- Phase 5 not-DF guardrail: collective (multi-cat) or cumulative (multi-season), never individual-rare-strike. `the-calling.md` owns individual mood-strike craft.
- Generalize `remedy_prep` and `ward_setting` into the unified catalog in Phase 1. No parallel code paths.

## Lessons from 367 first-light (2026-05-21)

Three substrate-stub-class gaps surfaced when 367 (Phase 1b preservation) ran
its first soak. None were caught by `just check` or `cargo test --lib`;
all surfaced as either silent canary false-negatives or as dormant-substrate
"compiled and tested but never fires" outcomes. Every Phase ≥1b ticket
inherits these:

1. **Hand-maintained iteration lists are substrate, not metadata.** When
   adding new enum variants (`Feature`, `ItemKind`, `DispositionKind`,
   `Action`, etc.), enroll them in **every** hand-maintained iteration
   constant, not just the exhaustive `match` arms. The compiler catches
   missing match arms; it does **not** catch a `pub const ALL: &[Foo] = &[...]`
   omission. The 367 case: 6 new `Feature` variants got writers,
   `category()` arms, `feature_name()` arms, and `expected_to_fire_per_soak()`
   arms — but the introducer missed `Feature::ALL` at
   `src/resources/system_activation.rs:619`. The `SystemActivation` snapshot
   emitter iterates `Feature::ALL`, so the new variants were excluded from
   every per-tick activation record, **and** the never-fired canary's
   `never_fired_expected_positives` returned `[]` as a false negative
   (because the canary also iterates `ALL`). Same iteration-list class
   exists for `DispositionKind::ALL` (`src/components/disposition.rs:443`),
   `Feature::ALL`, and likely others.

2. **Substrate-completeness ≠ election-completeness.** A new buildable
   `StructureType` is *not* shippable when the construct.rs arm + state
   Components + recipe registry entries + DSEs all land — it is shippable
   when the colony **elects** to build it. The election layer is
   `BuildPressure` (`src/components/coordination.rs:144-189`): one f32
   channel per electable structure type, accumulated in
   `assess_colony_needs` against a colony-state signal, decayed when the
   signal is absent, reset on construction completion. 367 Commits 1-6
   wired the full preservation substrate end-to-end and passed every
   check; first-light revealed **zero racks ever constructed** because
   `BuildPressure` had no `drying_rack` / `smoking_rack` channel. Fixed in
   367 Commit 8. Applies wherever a ticket adds a new `StructureType`
   variant (369 Tanning Frame is the immediate next case).

3. **First-light soak is mandatory verification.** `just check` +
   `cargo test --lib` verify mechanism; `just soak-trace` + `just verdict`
   verify activation. The substrate-stub-catalog scripts
   (`check_substrate_stubs.sh`, `check_marker_snapshot_wiring.sh`, etc.)
   catch *some* substrate gaps but **not** hand-maintained iteration
   lists and **not** election gaps. Per the
   `feedback_dormant_substrate_activation_soak_first` memory: every
   substrate-activation ticket runs ≥1 first-light soak before landing,
   and the soak is the gate, not `just check`.

A fourth lesson is **decorative-vs-load-bearing wiring** (367 Commit 4b
plumbed `ItemSlot.quality` through `pick_up` → load resolvers → output
items, but `food_value()` still reads `ItemKind` only — so quality
propagates correctly but has zero behavioral effect). Substrate that
*passes through* a value isn't *consuming* it; consumer-side wiring
(read sites in `food_value`, `take_damage`, `noise_signature`, etc.) is
a separate substrate step. 369 (material-property reads) is the canonical
home for this discipline; cross-reference it from any future ticket
that adds an item-borne property.

## Phase 5 gating

Three conditions, all required:
1. Colony-age ≥3 sim-years.
2. Material-scarcity (deep exploration / cleared ruins / cross-season storage inputs).
3. Skill-via-aspirations — mastery arcs from 366 (`WeavingMastery`, `BoneShapingMastery`, `HideworkMastery`, `PigmentMastery`, `CairnMastery`); ≥1 cat advanced on a relevant arc unlocks the recipe for the whole colony.

## Ship-order note

Crafting is the anchor of the 2026-04-22 three-way split (this entry, #17 slot-inventory, #18 ruin-clearings) and ships first. De-risks 017 at Phase 3 (first wearable producer) and 018 at Phase 1 (preservation recipes consume cleared-ruin food). Phase 4 decorations become the second primary consumer of #20 (naming substrate). Phase 5 is long-horizon and gated on aspirations-mastery arcs.

## Score

V=5 F=4 R=3 C=3 H=3 = **540** — "worthwhile; plan carefully" (300–1000 bucket). Promoted from 288 → 540 on 2026-04-22 when Phase 4 (Domestic refinement / folk-craft decorations) and Phase 5 (Elevated cat-craft / collective multi-season) were added. Originally rank 6 in `docs/systems-backlog-ranking.md`.

## Resume when

Phase 1a (365) and 1b (367) are done. Pick up next: Phase 2 (368, behavioral tools) and Phase 2b (369, warrior's kit) are both ready and can run in parallel.

## Log

- 2026-04-22: opened with V=5 F=4 R=3 C=3 H=3 = 540 score; Phase 4 added on promotion from 288 → 540. Phase 5 added (collective / multi-season tier with not-DF guardrail).
- 2026-05-16: promoted to epic dashboard (128-style — read-only over child tickets). Opened 8 phase children (365 Phase 1a substrate, 366 Phase 5 prereq, 367 Phase 1b recipes, 368 Phase 2 tools, 369 Phase 2b warrior's kit, 370 Phase 3 identity, 371 Phase 4 decorations, 372 Phase 5). Status flipped `ready → in-progress`. 334 (stealth-cloak) re-blocked on 365 + 017.
- 2026-05-18: 366 Phase 5 prereq landed (9d41d3ebc5cf). Five mastery arcs registered + Skills/SkillKind axes + Recipe.skill_gate field + RecipeRegistry::is_phase5_unlocked predicate. Adoption deferred to 372 (Kinship-pattern skip preserves seed-42 determinism). 372 unblocked; ready to start.
- 2026-05-18: 365 Phase 1a landed (4ecfaf0cd49b). Crafting substrate — Recipe / Station / CraftAction; remedy_prep + ward_setting generalized into unified catalog. 368, 369 unblocked.
- 2026-05-21: 367 Phase 1b landed (d8f49157). Preservation recipes (dried fish, smoked meat, preserved organ) + Drying Rack + Smoking Rack. First-light lessons logged above. 368 and 369 both ready.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- · **  1** (in-progress, ai-substrate, score 0.89 (cross-cluster)) — Explore dominance over targeted leisure
- · ** 41** (ready, items-crafting, score 0.87) — Founding wagon-dismantling haul — balance the early-game cost so cats don't sta…
- ✓ landed **328** (done, ai-substrate, score 0.87 (cross-cluster)) — Herbcraft aspiration_milestone_wrapper + emits tables

<!-- linkages:end -->
