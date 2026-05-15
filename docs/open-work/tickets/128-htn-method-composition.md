---
id: 128
title: HTN method composition — epic
status: in-progress
cluster: ai-substrate
orchestration: coherent-block
verdict-anchor: true
block: htn-method-composition
initiative: [smarter-cats, htn-method-composition]
added: 2026-05-02
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, htn-methods.md, strategist-coordinator.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Cluster-C C4 deliverable. C1 (126 BDI intention substrate, landed
2026-05-08), C2 (127 joint-intention substrate, landed 2026-05-11),
C3 (258 mental models, landed 2026-05-11) all in. 128 is the
hierarchical planning layer that sits *above* the L2 Intention
layer 126 committed: methods decompose `Intention::Goal` into an
ordered sub-goal sequence, the `HeldGoalStack` Component carries
the cat's cursor through that sequence, and a registry-walked
inspection surface renders the full aspirational landscape from
one substrate.

Promoted from a single ticket to an epic per the 2026-05-14
design session: HTN method composition is "an order of magnitude
larger in scope than 126" (per the original 128 body) and needs
the same coverage-map dashboard treatment 060 uses for the
substrate refactor program. This file is read-only over its child
tickets — it doesn't own work, it owns *visibility*. Each
shippable unit lives in its own ticket; this file is the program-
level dashboard that answers "what's left in the HTN layer?" in
one read.

The user's load-bearing design constraints, gathered across the
design session:

1. **All multi-tick cat aspirations route through this layer
   with a single inspection surface.** Existing aspirational
   substrate (`Aspirations`, `JointIntention`, `Pregnant`,
   `KittenDependency`, `FatedLove`) either gets a method that
   mirrors it OR contributes to a method's applicability.
2. **Every dormant method has a glue ticket.** Methods that
   depend on future substrate (slot-inventory, crafting recipes,
   grief-vigil actions) register as
   `ApplicableWhen::PendingSubstrate { blocker }` where `blocker`
   names an **open** ticket that wires the method to Live.
   Enforcement script verifies both directions.
3. **The L1→L2 emission picker** closes the loop §7.7 left open:
   aspirations get a per-milestone `emits[]` table that names
   which Goal labels advance each milestone, plus a domain-
   affinity fallback for graceful degradation.

## Scope

This epic tracks every shippable unit of HTN method composition.
The unit of work is the child ticket; the unit of visibility is
this file. The dashboard updates when child tickets change
status, not on its own cadence.

The **full architectural design** lives in
[`docs/systems/htn-methods.md`](../../systems/htn-methods.md) —
the systems spec authored alongside this epic. This ticket body
is deliberately thin on architecture; it's the dashboard.

## Design summary

Three-layer commitment, mirroring §7.M Mating's worked example:

| Layer | Component / system | Strategy | New under 128? |
|---|---|---|---|
| L1 Aspiration | `Aspirations` + `aspirations.rs` (existing) | `OpenMinded` (§7.7) | No — gains per-milestone `emits[]` table |
| L2 Method frame | `HeldGoalStack` + `MethodRegistry` | inherits from method | **Yes** |
| L2 Intention (leaf) | `HeldIntention` (126) | per `Intention.strategy` | No |
| L3 Plan | `GoapPlan` + `src/ai/planner/` | `Blind` (replans) | No |

**HTN methods are not a new layer** — they're the decomposition
mechanism for L2 `Intention::Goal`. The 126 author site
(`goap.rs:568-635::evaluate_and_plan`) gains an enrichment gate:
when adoption happens, if the goal label has a registered method,
push a `GoalFrame` and adopt the first sub-goal's leaf as
`HeldIntention`. Else fall through to 126's existing direct
adoption.

Master-spec alignment: §7.M three-layer architecture, §7.7
aspiration emission, §L2.10.4 Intention vocabulary, §4.7
substrate-vs-search-state (`HeldGoalStack` classifies cleanly as
substrate; no hybrid), §11.5 registry-walked trace.

Literature alignment: SHOP2 (Nau 2003) compound-task /
decomposition / operator vocabulary; F.E.A.R.-style total-order
HTN (Humphreys, Game AI Pro vol. 1) for Phase-1 simplifications
(ordered task lists, first-applicable-precondition method
selection, no softmax over methods).

Full design: [`docs/systems/htn-methods.md`](../../systems/htn-methods.md).

## Phase coverage map

Extended with a Parallelism column so engineers can find
concurrent windows without rereading the spec:

| Phase | Spec section | State | Owner ticket(s) | Parallelism |
|---|---|---|---|---|
| Batch A — Infrastructure | §A-D | 🔄 in flight | [319](319-method-registry-populate-no-stub-enforcement.md) [320](320-heldgoalstack-component-l2-evaluator-integration.md) [321](321-aspirations-milestones-emits-table-l1-l2-picker.md) [322](322-action-enum-stubs-for-dormant-methods.md) | #319 ∥ #322; then #320 ∥ #321 |
| Batch B — Tier 1 methods | §G | 🔄 blocked on Batch A | [323](323-courtship-method-mirror-jointintention-stages.md) [324](324-gestation-method-mirror-pregnant-stages.md) [325](325-aspiration-milestone-wrapper-hunting.md) [326](326-aspiration-milestone-wrapper-social.md) | 4-way after Batch A |
| Batch C — Tier 1 chains | §G | 🔄 blocked on #319 + #321 | [327](327-aspiration-milestone-wrapper-combat.md) [328](328-aspiration-milestone-wrapper-herbcraft.md) [329](329-aspiration-milestone-wrapper-exploration.md) [330](330-aspiration-milestone-wrapper-building.md) [331](331-aspiration-milestone-wrapper-leadership.md) | 5-way; overlaps Batch B (8-9-way peak) |
| Batch D — Tier 2 glue | §G | 🔄 mixed; 332/333 substrate landed, dispatch in #357 | [332](../landed/332-grief-vigil-action-vocabulary.md) [333](../landed/333-kitten-rearing-action-vocabulary.md) [334](334-stealth-cloak-crafting-recipe-wearitem-resolver.md) [357](357-htn-driven-action-dispatch-dse-goapactionkind-plan-template-for-mourn-at-grave-and-rear-kitten-primitives.md) | 332+333 landed verdict-skipped; 357 unlocks chosen_action override |
| Batch E — Cross-cutting surface | §C / §H / inspection | 🔄 mixed | [335](335-coordinator-directives-as-htn-method-seeds-057-integration.md) [336](336-just-inspect-renders-goal-stack-aspiration-set.md) [337](337-l3commitment-trace-method-stack-field.md) [338](338-l1aspiration-trace-record-emit-walk.md) [339](339-catsnapshot-goal-stack-active-aspirations-fields.md) [340](340-port-mating-l3-chain-onto-htn-method.md) [341](341-retarget-057-blocked-by-from-126-to-128.md) | #336-#339 4-way; #335 / #340 own deps; #341 anytime |
| Parked / future | §H open Qs | 💤 parked | [342](342-phase-2-unordered-method-bodies-partial-order.md) [343](343-emit-cooldown-discipline-per-emit-cooldown-ticks.md) | — |

## Critical path

Deepest serial chain is 4 hops:
**#319 → #320 → #323 → #340** (infrastructure → stack-wiring →
courtship-method → Mating L3 port). Everything else has shorter
critical paths from infrastructure.

**13-way peak parallelism** after Batch A completes (Batches B
+ C + #336-#339 from E all in flight).

Unusually parallel for an ai-substrate epic — most children are
catalogue authoring (method definitions, emits tables, trace
fields, inspection-rendering panels) rather than substrate
stacking. 060 and 093 had deeper sequential chains because their
work *was* substrate stacking.

## Open child tickets — full roster

| Ticket | Status | Spec home | One-line scope |
|---|---|---|---|
| [319](319-method-registry-populate-no-stub-enforcement.md) | ready | §A | `populate_method_registry` + `ApplicableWhen` + `check_method_registry.sh` + `just methods --pending` |
| [320](320-heldgoalstack-component-l2-evaluator-integration.md) | blocked-by 319 | §B / §C | `HeldGoalStack` Component + L2 evaluator integration at `goap.rs:568-635` |
| [321](321-aspirations-milestones-emits-table-l1-l2-picker.md) | blocked-by 319 | §H | Extend `Milestone` with `emits[]`; per-tick picker; L1Aspiration trace record opens |
| [322](322-action-enum-stubs-for-dormant-methods.md) | ready | §G | `Action::WearItem` / `Craft` / `PetitionCoordinator` enum stubs + placeholder resolvers |
| [323](323-courtship-method-mirror-jointintention-stages.md) | blocked-by 319,320 | §G Tier 1 | Mirror 127 `JointIntention.stage` advance — 4 sub-goals |
| [324](324-gestation-method-mirror-pregnant-stages.md) | blocked-by 319,320 | §G Tier 1 | Mirror `Pregnant.stage` (Early → Mid → Late) |
| [325](325-aspiration-milestone-wrapper-hunting.md) | blocked-by 319,321 | §G Tier 1 | Wrap Hunting chain milestones with `emits[]` tables; Live registration |
| [326](326-aspiration-milestone-wrapper-social.md) | blocked-by 319,321 | §G Tier 1 | Wrap Social chain milestones; Live registration |
| [327](327-aspiration-milestone-wrapper-combat.md) | blocked-by 319,321 | §G | Wrap Combat chain; flip from PendingSubstrate → Live |
| [328](328-aspiration-milestone-wrapper-herbcraft.md) | blocked-by 319,321 | §G | Wrap Herbcraft chain; flip to Live |
| [329](329-aspiration-milestone-wrapper-exploration.md) | blocked-by 319,321 | §G | Wrap Exploration chain; flip to Live |
| [330](330-aspiration-milestone-wrapper-building.md) | blocked-by 319,321 | §G | Wrap Building chain; flip to Live |
| [331](331-aspiration-milestone-wrapper-leadership.md) | blocked-by 319,321 | §G | Wrap Leadership chain; flip to Live |
| [332](../landed/332-grief-vigil-action-vocabulary.md) | landed 2026-05-15 | §G Tier 2 | Vigil / GriefSit / ReleaseGrief primitives + Mourning Component; method flipped to Live (dispatch in #357) |
| [333](../landed/333-kitten-rearing-action-vocabulary.md) | landed 2026-05-15 | §G Tier 2 | Wean / Teach / Release witness-typed resolvers; method flipped to Live (dispatch in #357) |
| [334](334-stealth-cloak-crafting-recipe-wearitem-resolver.md) | blocked-by 320,322 | §G Tier 2 | Stealth-cloak crafting recipe + WearItem resolver wiring; flips `acquire_stealth_via_*` to Live |
| [357](357-htn-driven-action-dispatch-dse-goapactionkind-plan-template-for-mourn-at-grave-and-rear-kitten-primitives.md) | ready | §G Tier 2 dispatch | HTN-driven action dispatch (DSE / GoapActionKind / plan template) for `mourn_at_grave` and `rear_kitten` primitives — closes the dispatch gap surfaced in #332/#333 layer-walks |
| [335](335-coordinator-directives-as-htn-method-seeds-057-integration.md) | blocked-by 320 | §C | Coordinator directives as HTN method seeds; 057 integration |
| [336](336-just-inspect-renders-goal-stack-aspiration-set.md) | blocked-by 320,321 | inspection invariant | `just inspect` renders aspiration set + goal stack + recent method history |
| [337](337-l3commitment-trace-method-stack-field.md) | blocked-by 320 | §11.5 / trace | L3Commitment trace gains `method_stack` field (registry-walked) |
| [338](338-l1aspiration-trace-record-emit-walk.md) | blocked-by 321 | §H / §11.5 | New L1Aspiration trace record (emit-walk per active aspiration) |
| [339](339-catsnapshot-goal-stack-active-aspirations-fields.md) | blocked-by 320,321 | trace | `CatSnapshot.goal_stack` + `active_aspirations` fields in `events.jsonl` |
| [340](340-port-mating-l3-chain-onto-htn-method.md) | blocked-by 323 | §G worked example | Port `disposition.rs:1873-1919` mating chain onto `mate_with_goal` method |
| [341](341-retarget-057-blocked-by-from-126-to-128.md) | ready | process | One-line frontmatter edit on 057 — retarget `blocked-by: [126]` → `blocked-by: [128]` |
| [342](342-phase-2-unordered-method-bodies-partial-order.md) | parked | §H future | `:unordered` method bodies + partial-order decomposition (SHOP2 keyword) |
| [343](343-emit-cooldown-discipline-per-emit-cooldown-ticks.md) | parked | §H open Q | Per-emit `cooldown_ticks` to prevent picker thrash on serial method failures |

## Adjacent epics

- **060** — `tickets/060-ai-substrate-refactor-epic.md` — parent
  epic. Cluster C substrate row points at 128 as an active
  sub-epic (parallel to how 127's follow-ons are listed under
  it).
- **007** — `landed/007-cluster-c-deliberation-layer.md` —
  cluster mandate; C4 entry directs to 128.

## Glue-ticket invariant

Every dormant method registered as
`ApplicableWhen::PendingSubstrate { blocker }` has a corresponding
**open** ticket in `docs/open-work/tickets/` carrying a
`wires-method: [<method-id>...]` frontmatter field. The
enforcement script `scripts/check_method_registry.sh` (landing in
#319) verifies both directions:
- Dormant method without glue ticket → CI failure.
- Glue ticket without matching method-id → CI failure.

Per CLAUDE.md addendum landed alongside this epic: "All multi-
tick aspirations are HTN methods" + "Every dormant method has a
glue ticket." Without this discipline, design intent for arcs
the sim could express rots — methods describe natural narrative
trees that never sprout because nobody trips over the design
intent in their work surface.

## Preparation reading

Read in this order:

1. [`docs/systems/htn-methods.md`](../../systems/htn-methods.md)
   — primary design spec.
2. `docs/systems/ai-substrate-refactor.md` §7.M (3861-4143) —
   three-layer mating worked example.
3. `docs/systems/ai-substrate-refactor.md` §7.7 (4652-4863) —
   aspiration-level commitment, the L1 layer methods compose
   with.
4. `docs/systems/ai-substrate-refactor.md` §L2.10.4-§L2.10.7
   (5951-6223) — Intention vocabulary.
5. `docs/systems/ai-substrate-refactor.md` §4.7 (2509-2742) —
   substrate-vs-search-state classifier.
6. `docs/systems/ai-substrate-refactor.md` §11.5 (6561-6576) —
   registry-walk trace invariant.
7. [`docs/open-work/landed/126-bdi-intention-substrate.md`](../landed/126-bdi-intention-substrate.md)
   — BDI substrate (the leaf layer methods build on).
8. [`docs/open-work/landed/127-joint-intention-substrate.md`](../landed/127-joint-intention-substrate.md)
   — joint-intention substrate (the Courtship method mirrors).
9. `docs/reading-list.md` §C4 — SHOP2 (Nau 2003) + Game AI Pro
   Humphreys references.

## Log

- 2026-05-02: opened as 126 follow-on per CLAUDE.md
  antipattern-migration rule.
- 2026-05-14: promoted to epic. Spec authored at
  `docs/systems/htn-methods.md`. 25 children opened
  (#319-#343). 060's Cluster C row updated to flag 128 as an
  active sub-epic. CLAUDE.md gained "All multi-tick aspirations
  are HTN methods" + "Every dormant method has a glue ticket"
  rules.
