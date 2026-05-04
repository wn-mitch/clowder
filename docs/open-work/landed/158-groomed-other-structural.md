---
id: 158
title: GroomedOther never-fired post-154 — split Action::Groom + extract DispositionKind::Grooming
status: done
cluster: ai-substrate
added: 2026-05-03
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: [mentoring-extraction.md]
landed-at: d1722a33
landed-on: 2026-05-04
---

## Why

Ticket 154's land-day soak (`logs/tuned-42`, seed 42, commit
`bb189bc`, 8 sim years) shows `GroomedOther` *still* in
`never_fired_expected_positives`. After 154's split, Socializing's
plan template is `[SocializeWith (cost 2), GroomOther (cost 2)]` —
equal cost, so the cost-asymmetry collapse 154 fixed for Mentor
shouldn't apply here. Yet GroomOther still doesn't fire on this seed.

154's parent ticket explicitly carved out GroomedOther's structural
treatment as conditional follow-on:

> **GroomedOther structural treatment** unless the ticket's
> investigation pass shows it's the same defect-shape. If it's a
> caller-side issue, open a separate ticket.

Post-154 soak confirms GroomedOther stays dark. This ticket holds
the structural investigation. Two candidate defect-shapes:

**Caller-side (self-vs-other resolver).** `Action::Groom` is the
self-vs-other split point at `from_action:124`: the L3 softmax picks
`Action::Groom`, then a separate resolver decides whether
`self_groom_won == true` (routes to `Resting`) or `false` (routes to
`Socializing`). If that resolver biases toward self, `GroomOther`
never reaches the planner. (The `groom_self_dse` and `groom_other_dse`
both register in `populate_dse_registry`, so the issue isn't DSE
absence — it's the choice between them.)

**Planner-side (deterministic tie-break).** With Socializing's
template now 2-action equal-cost, the GOAP planner's tie-break may
be deterministic (lexicographic? position-in-vec?) and consistently
pick `SocializeWith` over `GroomOther`. If so, `GroomOther` is
unreachable in practice even though the eligibility chain is healthy.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | none specific to GroomOther | `[verified-correct]` |
| L2 cat-DSE | `src/ai/dses/groom_other.rs` | scoring is healthy: 4-axis CompensatedProduct on `social_deficit`, `warmth`, `phys_satisfaction`, `social_warmth_deficit` | `[verified-correct]` |
| L2 target-DSE | `src/ai/dses/groom_other_target.rs` (or fallback to socialize_target) | unverified — does this produce a target on this seed? | `[suspect]` |
| L3 softmax | `src/ai/scoring.rs` | `Action::Groom` is in the pool; softmax-pickable | `[verified-correct]` |
| Self-vs-other split | wherever `self_groom_won` is decided (likely `scoring.rs:1696-1701`) | unverified — does the resolver pick self too often, hiding GroomOther from the Socializing branch? | `[suspect]` (defect candidate A) |
| Plan template | `src/ai/planner/actions.rs::socializing_actions` | `[SocializeWith (2), GroomOther (2)]` post-154 | `[verified-correct]` |
| GOAP planner tie-break | `src/ai/planner/mod.rs` (A* / Dijkstra search) | unverified — when two actions have equal cost and equal precondition match, which fires? Is it deterministic per seed? | `[suspect]` (defect candidate B) |

## Investigation step

1. **Self-vs-other audit.** Run `just q trace logs/tuned-42 <focal>
   --layer=L3` for a high-warmth focal cat (warmth > 0.6). Count
   ticks where `Action::Groom` was the L3 pick; for each, was
   `self_groom_won == true` or `false`? If always self, defect
   candidate A is confirmed.
2. **Planner tie-break audit.** Build a unit-test scenario where
   `socializing_actions()` is fed a state with both
   `SocializeWith` and `GroomOther` precondition-satisfied, and
   trace which step the planner picks. If it always picks
   `SocializeWith`, defect candidate B is confirmed.
3. **Cross-check Mentoring's outcome.** Mentoring's split
   *succeeded* (1614 firings), so the `Action::Mentor` → planner
   path is healthy. The contrast is informative: Mentor has its
   own DispositionKind (no equal-cost siblings), Mentor's L3 pick
   isn't a self-vs-other split. Diff the two paths.

## Fix candidates

**Parameter-level (rejected by 154's parent ticket spirit):**

- R1 — bias the self-vs-other resolver toward "other" with a
  random tie-break or warmth-weighted gate. Doesn't address the
  underlying coupling of two actions on one Action variant.
- R2 — bump `GroomOther`'s cost in the plan template *below*
  `SocializeWith`'s cost (e.g. 1 vs 2). Cheap-grooming fix; risks
  over-firing GroomOther over Socialize.

**Structural (the ticket's spirit per CLAUDE.md bugfix discipline):**

- R3 (**split**) — extract `Action::GroomOther` and `Action::GroomSelf`
  as distinct Action variants in `src/ai/mod.rs` (currently both
  ride `Action::Groom`). Each gets its own DSE → its own L3 softmax
  pick → its own `from_action` arm → cleaner substrate. Mirror 150
  R5a / 154's split shape.
- R4 (**rebind**) — keep `Action::Groom` as one variant but route
  the L3 pick to the appropriate disposition based on `self_groom_won`
  *at planner-state construction time* rather than at chain-build
  time. The L3 pick survives, but the routing decision moves earlier
  in the pipeline.
- R5 (**extend**) — keep one `Action::Groom` but give the GOAP
  planner a stochastic tie-break on equal-cost equal-precondition
  actions. Affects all dispositions, not just Socializing; broader
  risk.

## Recommended direction

**R3 (split Action::Groom into GroomOther + GroomSelf).** Mirrors
the substrate-vs-search-state discipline: the self-vs-other
distinction is load-bearing semantic info that's currently treated
as ephemeral search-state at the chain-build layer. Splitting it at
the Action-enum level means the L3 softmax pick directly carries
the semantics through to the planner, with no resolver in between.

R4 is a viable fallback if R3 turns out to ripple into too many
sites. R5 (stochastic tie-break) is rejected as a band-aid that
papers over the structural issue.

## Verification

- Post-fix `just soak 42` → `just verdict`: `GroomedOther` is **off**
  `never_fired_expected_positives`.
- `continuity_tallies.grooming` stays ≥ ~300 (within 2× of post-154
  baseline of 388; some shift expected as GroomOther starts firing
  for the first time).
- Mentoring undisturbed: `MentoredCat` stays off the never-fired
  list; `mentoring` tally ≥ ~1000.
- No regression on `Starvation == 0` (assuming 156 has landed first
  or this fix doesn't affect Caretaking).

## Out of scope

- **Kitten starvation localization** (ticket 156).
- **Burial canary** (ticket 157).
- **Self-grooming behavior changes** beyond what R3's split
  necessarily implies. The `groom_self_dse` keeps its current
  scoring shape under R3.

## Log

- 2026-05-03: opened by ticket 154's land-day verdict.
  154 §"Out of scope" allowed for this conditional follow-on if
  GroomedOther stayed dark post-split — confirmed dark, opening
  the structural ticket.
- 2026-05-04: investigation pass — both `[suspect]` audit-table rows
  promoted to `[verified-defect]`:
  - **Defect A (resolver bias)** — `let self_groom_won =
    self_groom_score >= other_groom_score;` lives at
    `src/systems/goap.rs:1666` AND duplicated identically at
    `src/systems/disposition.rs:1097`. The `>=` ties to self-groom
    on equality. Even when other > self, routing flows through a
    side-channel boolean into `select_disposition_via_intention_softmax_with_trace`
    — a substrate-vs-search-state confusion (the self-vs-other
    distinction is load-bearing semantic info, not ephemeral
    resolver state).
  - **Defect B (planner pre-pruning, not tie-break)** —
    `socializing_actions()` returns `[SocializeWith (2),
    GroomOther (2)]` with **identical effects**
    (`SetInteractionDone(true), IncrementTrips`). At
    `src/ai/planner/mod.rs:437`, once `SocializeWith` (vec[0])
    writes `best_g[next_state] = 2`, `GroomOther`'s `tentative_g
    (2) >= 2` triggers `continue` — GroomOther is **never even
    pushed to the open set**. This is correct A* over a degenerate
    plan template, not a tie-break bug.
  R3 alone (Action enum split) addresses A but not B. Recommended
  direction: **R3 + DispositionKind::Grooming extraction** —
  mirrors 154's Mentoring split exactly and 150 R5a's Eating
  precedent. Single-action `[GroomOther]` template makes
  equivalent-sibling pruning structurally impossible. Bonus: the
  duplicate resolver block becomes deletable.
- 2026-05-04: status flipped to in-progress; implementation begins.
- 2026-05-04: landed. R3 implemented as
  **Action enum split + `DispositionKind::Grooming` extraction**
  per the 154 / 150 R5a precedent. Bridge at
  `src/ai/scoring.rs::score_actions` no longer max-collapses the
  two DSE scores — emits `Action::GroomSelf` and `Action::GroomOther`
  as distinct L3 pool entries. Both duplicate `self_groom_won`
  resolver blocks (`src/systems/goap.rs:1657-1666` +
  `src/systems/disposition.rs:1088-1097`) deleted; the
  `self_groom_won` parameter on
  `select_disposition_via_intention_softmax_with_trace` retired;
  `Action::Groom` special-case in `aggregate_to_dispositions`
  retired; the orphaned `self_groom_temperature_scale` `SimConstants`
  field removed (header-shape change documented in
  `docs/balance/mentoring-extraction.md` Iter 2). New
  `src/scenarios/grooming_other.rs` triage scenario per ticket 162's
  harness wires this defect class into the bugfix-discipline tooling.
- 2026-05-04 verdict: `GroomedOther` **off**
  `never_fired_expected_positives` (only `FoodCooked` remains —
  separate kitchen-construction question); `continuity_tallies.grooming
  = 1279` (+156% vs pre-158 baseline 499); `MentoredCat` still firing
  (`mentoring = 165`). Verdict overall = `fail` on inherited
  post-154 cascade (Starvation = 3 kitten deaths at (41,22) and
  (24,19) → ticket 156's same-shape kitten-feeding gap;
  `burial = 0` → ticket 157). 158-specific structural success
  criterion met. Drift from pre-158 documented as Iter 2 in
  `docs/balance/mentoring-extraction.md` — the substrate split
  surfaced an attention-share regression (parent cats picking
  GroomOther 6.5× more often than Caretake while own kittens
  starve), called out as a follow-on observation under 156's
  cluster.
