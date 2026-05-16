---
id: 016
title: Crafting — items, recipes, stations (epic dashboard)
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
| 1a | Crafting substrate — Recipe / Station / CraftAction; generalize remedy_prep + ward_setting | ready | [365](365-crafting-substrate-recipe-station-craftaction-generalize-remedy-prep-and-ward-setting-016-phase-1a.md) | — |
| 1b | Preservation recipes + Drying Rack + Smoking Rack | blocked | [367](367-phase-1-preservation-recipes-dried-fish-smoked-meat-preserved-organ-drying-rack-and-smoking-rack-stations-016-phase-1b.md) | 365 |
| 2 | §5 behavioral tools (Grooming Brush, Play Bundle, Courtship Gift) | blocked | [368](368-phase-2-behavioral-tools-grooming-brush-play-bundle-courtship-gift-016-phase-2.md) | 365 |
| 2b | Warrior's kit (8 items, Tanning Frame, material-property substrate) | blocked | [369](369-phase-2b-warriors-kit-8-items-tanning-frame-station-material-property-substrate-for-huntcombatnoise-resolvers-016-phase-2b.md) | 365 |
| 3 | Identity, mentorship, adornment (first wearable producer) | blocked | [370](370-phase-3-identity-mentorship-adornment-mentorship-token-heirloom-piece-shell-collar-bone-and-wire-tiara-stone-set-pin-016-phase-3-first-wearable-producer.md) | 368, 017 |
| 4 | Domestic refinement — place-anchored decorations | blocked | [371](371-phase-4-domestic-refinement-place-anchored-decorations-reed-mat-tallow-lamp-scent-censer-carved-comb-wall-hanging-nesting-inlay-016-phase-4.md) | 370 |
| 5-prereq | Aspirations mastery arcs (Weaving, BoneShaping, Hidework, Pigment, Cairn) | ready | [366](366-aspirations-mastery-arcs-weaving-boneshaping-hidework-pigment-cairn-016-phase-5-precursor.md) | — |
| 5 | Elevated cat-craft (collective / multi-season, triple-gated) | blocked | [372](372-phase-5-elevated-cat-craft-generational-tapestry-shrine-cairn-bone-lattice-lantern-pigment-deepened-textile-multi-cat-nesting-alcove-kitten-cradle-basket-016-phase-5-triple-gated.md) | 371, 366 |

## Related existing tickets

- **173** (parked) — `crafting-split-capability-markers`. Un-park or close decision happens in the first commit of 365's session (capability markers may fall out of the unified Recipe substrate naturally).
- **309** (blocked-by 308) — Herbcraft DSE reserve-deficit consideration / anticipatory crafting. Phase 1.5 extension; remains gated on 308 (ColonyReservesBelief).
- **334** (blocked-by 365, 17) — Stealth-cloak crafting recipe + WearItem resolver. 128 epic Tier-2 glue; scope may shrink to pure HTN-method-flip once 369 lands the WearItem resolver. Decide during 369 session.

## Design constraints

Load-bearing — drift re-triggers ranking (F→2, H→2, score → ~96). Quick anchors (see [`docs/systems/crafting.md`](../../systems/crafting.md) §Design constraints for the full statement):

- §5-first catalog. Combat gear (spears, bracers, blades, slings) included as Phase 2b recipe cluster.
- Items are characterization, not commodity — effects live on action resolvers keyed to item identity and ecological properties.
- Decorations are place-anchored, not cat-anchored (Phase 4+).
- Cat-native materials palette: reed, bone, fur, feather, shell, fat, pigment, hide, sinew, flint, fieldstone. Metal arrives via `ScavengedMetal` / `TradedMetal` — never produced by a cat discipline.
- Phase 5 not-DF guardrail: collective (multi-cat) or cumulative (multi-season), never individual-rare-strike. `the-calling.md` owns individual mood-strike craft.
- Generalize `remedy_prep` and `ward_setting` into the unified catalog in Phase 1. No parallel code paths.

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

Pick up next: Phase 1a — substrate refactor that generalizes `remedy_prep` + `ward_setting`. Phase 5 prereq (366) can run in parallel; mastery arcs are independent of crafting code.

## Log

- 2026-04-22: opened with V=5 F=4 R=3 C=3 H=3 = 540 score; Phase 4 added on promotion from 288 → 540. Phase 5 added (collective / multi-season tier with not-DF guardrail).
- 2026-05-16: promoted to epic dashboard (128-style — read-only over child tickets). Opened 8 phase children (365 Phase 1a substrate, 366 Phase 5 prereq, 367 Phase 1b recipes, 368 Phase 2 tools, 369 Phase 2b warrior's kit, 370 Phase 3 identity, 371 Phase 4 decorations, 372 Phase 5). Status flipped `ready → in-progress`. 334 (stealth-cloak) re-blocked on 365 + 017.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-08 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- · **  1** (in-progress, —, score 0.87) — Explore dominance over targeted leisure
- ✓ landed **176** (done, ai-substrate, score 0.86 (cross-cluster)) — cats need real inventory reasoning — trash, build-more-stores, satiation-aware…
- · ** 21** (blocked, —, score 0.86) — Monuments — civic & memorial structures

<!-- linkages:end -->
