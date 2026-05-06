---
id: 177
title: Wire Trash/Handoff/PickUp resolvers into GOAP dispatch (176 follow-on)
status: done
cluster: ai-substrate
added: 2026-05-05
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 9b39e2f5
landed-on: 2026-05-06
---

## Why

Ticket 176 stage 3 (`e7f333af`) wired `Action::Drop` into the
`resolve_disposition_action_kind` dispatch but left the three
sibling resolvers (`Action::Trash`, `Action::Handoff`,
`Action::PickUp`) as Fail-stubs. The resolvers exist as `pub`
functions under `src/steps/disposition/{trash.rs, handoff.rs,
pick_up.rs}`; what's missing is the dispatch-side query plumbing
that lets each resolver reach the queries it needs:

- **Trash** — needs a `Query<(&Structure, &mut StoredItems, &Position)>`
  to address the target Midden building. The dispatch surface
  doesn't currently thread this query through.
- **Handoff** — needs a cat-pair query split (`Query<&mut
  Inventory>` over the actor cat AND the recipient cat
  simultaneously). Bevy's borrow-checker forbids the simple
  `query.get_mut(actor); query.get_mut(target)` pattern; the
  resolver's signature is correct but the dispatch call site
  must use the same split-by-marker pattern that
  `groom_other` uses for the actor-vs-target split.
- **PickUp** — needs a `Query<&Item>` to look up the target
  ground-item entity's kind / modifiers.

Disposal DSEs ship default-zero (stage 3) so these arms are
unreachable at runtime today. Wiring them is the structural
prerequisite for any balance-tuning ticket that lifts the DSE
weights — without these, balance-tuning would surface the
stub-Fail messages as real plan failures.

## Direction

- Extend `resolve_disposition_action_kind`'s SystemParam bundle
  in `src/systems/goap.rs` with the three new queries.
- Replace the stage-3 Fail stubs (around `goap.rs:4646`) with
  calls to `resolve_trash_at_midden`, `resolve_handoff`, and
  `resolve_pick_up_from_ground`. Each routes Feature emission
  through `record_if_witnessed` per the step-resolver contract.
- For Handoff specifically: use the same actor/target query-split
  pattern as `groom_other`'s deferred `GroomOutcome`. The
  recipient inventory mutation may need a post-loop pass to
  satisfy Bevy's parallel-query disjointness rules.
- Add per-resolver wiring tests (the resolvers themselves are
  unit-tested already; this ticket adds dispatch-level tests).

## Out of scope

- Balance-tuning the DSE weights from default-zero — that's a
  separate ticket once the dispatch is wired.
- Handoff target-picking (which cat receives the item) — that's
  another follow-on; for stage-1 wiring the handoff target can be
  threaded as `target_entity` from the disposition layer.

## Verification

- `just check`, `just test` green.
- New unit tests prove each resolver fires when reached at the
  dispatch level (forced by manually inserting a Disposition with
  the relevant target_entity).
- Disposal DSEs still ship default-zero, so the post-fix soak
  shape is unchanged from the current 176-stage-5 main.

## Log

- 2026-05-05: opened by ticket 176's closeout. Stage 3 wired Drop
  but deferred the three siblings; this ticket finishes the
  dispatch wiring so balance-tuning is possible.
- 2026-05-06: landed at `9b39e2f5`. Trash uses a `midden_entities`
  snapshot (extending `stores_query` would conflict B0001 with
  `BuildingResolverParams::buildings`); the resolver's signature
  simplifies from `&mut Query<…>` to `&mut StoredItems + Position`.
  PickUp passes the existing `items_query` directly — the
  `Without<BuildMaterialItem>` filter Fails build-material targets
  at the resolver, which is the correct semantics. Handoff queues a
  `HandoffPending` accumulator entry from the dispatch arm and
  drains in a post-loop pass via `cats.get_many_mut::<2>` (mirrors
  `groom_other`'s deferred-mutation precedent). 3 dispatch-level
  integration tests added under `src/scenarios/disposal_dispatch.rs`.
  Soak verification: disposal Features stay at 0 across the entire
  seed-42 deep-soak (DSE default-zero invariant intact, dispatch
  arms never reached at runtime). Unblocks 178; opens 188 for
  the parked Handoff target-picking scope.
