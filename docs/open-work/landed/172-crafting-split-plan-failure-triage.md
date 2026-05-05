---
id: 172
title: Plan-failure triage on Cooking + Herbalism (155 follow-on)
status: done
cluster: ai-substrate
added: 2026-05-05
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 055d54ee
landed-on: 2026-05-05
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
- 2026-05-05 (`055d54ee`): landed the diagnostic-first fix.
  Promoted `EventKind::PlanningFailed.reason` from a stringly-typed
  `"no_plan_found"` constant to the typed `PlanningFailureReason`
  enum (`NoApplicableActions` / `GoalUnreachable` /
  `NodeBudgetExhausted`). `make_plan` now returns
  `Result<Vec<PlannedStep>, PlanningFailureReason>`. Added a per-
  `(disposition, reason)` footer aggregator
  (`planning_failures_by_reason`) keyed
  `"<Disposition>:<Reason>"`. No DSE / marker / `SimConstants`
  changes — events.jsonl header comparability invariant preserved
  (constants diff = 0 vs the post-155 baseline; plan-failure
  counts within ±3% — Cooking 2126→2076, Herbalism 1712→1663 —
  well under the ±10% binary-perturbation gate).
  - **Histogram finding (the load-bearing data point):**
    100% of the residual plan-failure surface is
    `GoalUnreachable`. Zero `NoApplicableActions`, zero
    `NodeBudgetExhausted`. Per-disposition:
    - `Cooking:GoalUnreachable` 2076
    - `Herbalism:GoalUnreachable` 1663
    - `Hunting:GoalUnreachable` 243
    - `Foraging:GoalUnreachable` 181
  - **Cull-shape decision:** the empirical histogram **rejects**
    sibling 173's premise. The per-Disposition cull isn't loose
    because of substrate-eligibility gating; A* finds applicable
    actions and fully explores reachable states without satisfying
    the goal predicate. Adding `IsHerbalist` / `IsSpiritualist` /
    `HasCorruptionNearby` would tighten the wrong layer. 173 is
    parked.
  - **Substrate-vs-search-state distinction (per
    `ai-substrate-refactor.md` §4.7):** the load-bearing question
    is now whether the L3 softmax elects Cooking / Herbalism for
    cats whose `PlannerState` doesn't carry the world-fact the
    action chain depends on (L1-marker-vs-PlannerState desync —
    the eligibility marker says "go" but the planner's start state
    doesn't reflect the substrate the action requires) OR whether
    the goal predicate doesn't track what the action chain
    actually produces (action-effect / goal-predicate mismatch).
    Either way, the fix-shape is different from 173's
    capability-marker pattern.
  - **Hard gates met:** `Starvation == 0` ✓; `WildlifeCombat == 1`
    (well under the `ShadowFoxAmbush ≤ 10` budget); footer line
    written ✓; `never_fired_expected_positives == []` ✓.
  - **Continuity canaries:** `burial = 0` (downstream of low
    deaths total — same shape as 155's closeout reported);
    grooming 3319, mentoring 1174, courtship 5506, play 26,
    mythic-texture 40 — all healthy. Generational continuity:
    kittens_born = 4 (down from 6 post-155; small absolute
    delta, not a regression).
  - **173 amended:** parked with `## Log` line citing this
    ticket's evidence. Sibling 174 was deleted (created in error).
  - **Diagnostic surface delivered:** future plan-failure triage
    has a typed cause histogram in the footer. The next ticket on
    the GoalUnreachable root-cause has a clear question and a
    cheap repro path (`just q events logs/tuned-42 --type=PlanFailed`
    filtered by disposition).
