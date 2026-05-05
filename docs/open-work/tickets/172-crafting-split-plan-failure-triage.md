---
id: 172
title: Plan-failure triage on Cooking + Herbalism (155 follow-on)
status: ready
cluster: ai-substrate
added: 2026-05-05
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 155 split `DispositionKind::Crafting` into Herbalism /
Witchcraft / Cooking and retired `CraftingHint`. The post-155 seed-42
soak (`logs/tuned-42/`, commit `e1646089`) shows:

- Crafting (pre-155): 9,075 plan failures
- Herbalism (post-155): 1,712 plan failures
- Witchcraft (post-155): 0 plan failures
- Cooking (post-155): 2,126 plan failures
- **Total: 3,838 (58% reduction)** — but Herbalism and Cooking each
  exceed the ticket-155 target of "no single disposition above
  ~1,000."

This is structural visibility — the per-disposition counts were
hidden inside the Crafting blob pre-155 and are now exposed
individually. The 58% reduction proves the substrate split works;
the residual ~3,800 is the *post-cull* plan-failure surface that
needs its own triage.

## Investigation hooks

Both surfaces benefit from a layer-walk audit per CLAUDE.md "Bugfix
discipline":

- **Cooking 2,126**: the chain is `[RetrieveRawFood, Cook,
  DepositCookedFood]`. Likely failure modes — no raw food in stores,
  no functional kitchen reachable, planner can't path Wilds → Stores
  → Kitchen → Stores. `just q deaths logs/tuned-42` and `just q
  events logs/tuned-42 --type=PlanFailed` filtered to Cooking will
  surface the dominant reason.
- **Herbalism 1,712**: three sub-actions (Gather / Remedy / Ward).
  Likely failure modes — no herbs nearby (gather), no remedy patient
  in range (remedy), no thornbriar inventory (ward). The split is
  expected to surface which sub-mode dominates the failures, which
  the pre-155 hint-tournament couldn't.

## Recommended approach

`just inspect <focal-cat>` on a Herbalism-leaning cat and a
Cooking-leaning cat in the post-155 soak; pair with a focal trace
(`just soak-trace 42 <cat>`) for L1/L2/L3 visibility. The structural
fix is likely either (a) tighter L1 marker eligibility (the
`IsHerbalist` / `IsSpiritualist` capability markers deferred from
ticket 155) or (b) per-sub-action precondition tightening so the
planner doesn't spawn plans for impossible substrate states.

## Out of scope

- Balance iteration on cooked-food nutrition or ward strength —
  substrate must stabilize first.
- New continuity canaries — handle in ticket 174 (post-155 cascade).

## Log

- 2026-05-05: opened by ticket 155's closeout. Per-disposition plan
  failures are now visible; the 9,075 → 3,838 reduction landed but
  Cooking + Herbalism each exceed 1,000.
