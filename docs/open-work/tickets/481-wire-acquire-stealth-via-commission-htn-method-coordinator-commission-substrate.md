---
id: 481
title: Wire acquire_stealth_via_commission HTN method (coordinator-commission substrate)
status: blocked
cluster: items-crafting
orchestration: coherent-block
block: htn-method-composition
initiative: [smarter-cats, world-richness, htn-method-composition]
added: 2026-05-27
parked: null
blocked-by: [381]
wires-method: [acquire_stealth_via_commission]
supersedes: []
related-systems: [htn-methods.md, crafting.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why
334 landed the self-craft half of the stealth-cloak acquisition arc
(`acquire_stealth_via_self_craft` flipped Live: craft the woven reed cloak,
then don it). It could not flip the *commission* sibling
(`acquire_stealth_via_commission`) because that path's substrate does not
exist: `resolve_petition_coordinator` is a Fail-stub, `Action::PetitionCoordinator`
has no `htn_primitive_actions` arm (it would panic on dispatch), and the
method's `Goal("ordered_item_ready")` sub-goal has no satisfying substrate —
a coordinator cat cannot yet accept and fulfil a crafting order placed by
another cat. 334 re-pointed the method's `PendingSubstrate` blocker from "334"
to this ticket so the method-registry lint stays honest. This ticket holds the
commission wiring until the trader/coordinator-commission substrate (381)
lands the order-fulfilment loop it depends on.

## Scope
- `resolve_petition_coordinator` real resolver (replaces the #322 Fail-stub):
  a cat petitions an in-range coordinator to commission the woven reed cloak.
- `Action::PetitionCoordinator` HTN-primitive wiring (`htn_primitive_actions`
  arm + `GoapActionKind` + dispatch + frame-advance recognition), mirroring
  334's `Action::WearItem` wiring.
- The `ordered_item_ready` completion substrate: the coordinator-side order
  queue + fulfilment that makes the method's middle sub-goal satisfiable
  (depends on 381's trader/exchange foundation).
- Flip `acquire_stealth_via_commission` from `PendingSubstrate { blocker: "481" }`
  to `ApplicableWhen::Live`, with the full sub-goal chain
  `[petition(PetitionCoordinator), Goal("ordered_item_ready"), retrieve(Navigate), don(WearItem)]`.

## Out of scope
- The self-craft path (landed in 334).
- Generic trader/visitor-cat substrate (381).
- Other commissionable items (this ticket lands only the stealth-cloak
  commission exemplar).

## Current state
Opened 2026-05-27 as a 334 follow-on. Blocked on 381 (trader-substrate
foundation, parked) for the order-fulfilment loop. `acquire_stealth_via_commission`
remains registered `PendingSubstrate { blocker: "481" }` in
`src/ai/methods/acquire_stealth.rs`; the don leaf (`WearItem`) and its resolver
already exist (landed in 334) and will be reused unchanged.

## Approach
Per `docs/systems/htn-methods.md` §Worked example (commission branch). Mirror
334's `WearItem` wiring shape for `PetitionCoordinator`. The `Goal("ordered_item_ready")`
sub-goal recurses into the coordinator-commission substrate 381 provides;
decide the recursion seam (method-registry entry vs goal-advance interception)
when 381's order-queue shape is known, reusing the precedent 463 set for
HaveItem decomposition.

## Verification
- `cargo check --all-targets` + `just check` (method-registry lint confirms the
  method is Live and the `wires-method` back-reference is consistent).
- `just soak-trace 42 <focal>` on a cat with a Hunting aspiration lacking
  stealth gear AND an in-range coordinator: L2 picks
  `acquire_stealth_via_commission`, the petition leg fires, the order completes,
  retrieve + don advance, frame pops.
- `just verdict logs/tuned-42`: no regression on hunt / craft / coordinate canaries.

## Log
- 2026-05-27: opened as a 334 follow-on. 334's narrowing decision (commission
  cannot go Live: PetitionCoordinator stub + no `ordered_item_ready` substrate +
  381 parked) moved this method's wiring here. `wires-method:
  [acquire_stealth_via_commission]` carries the method-registry back-reference;
  the method's `blocker` re-points to 481 in the same commit that lands 334.
