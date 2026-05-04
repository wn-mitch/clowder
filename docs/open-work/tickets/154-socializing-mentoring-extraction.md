---
id: 154
title: Extract Mentoring from Socializing — split DispositionKind so MentoredCat fires
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

Per ticket 152's audit verdict on the Socializing cluster, `Action::Mentor`
maps to `DispositionKind::Socializing` alongside `Action::Socialize` and
`Action::Groom`. The plan template `[SocializeWith (2), GroomOther (2),
MentorCat (3)]` shares a single `TripsAtLeast(N+1)` completion goal, so
the GOAP planner picks the cheapest applicable sibling and the
`Action::Mentor` L3 pick is effectively discarded.

**Evidence (`logs/032-soak-treatment/`, seed 42, header
`commit_hash_short=883e9f3` post-150 + post-032 cliff):**

- `current_action` distribution from `just q actions` (10,017 CatSnapshot
  rows, 8 cats): **Mentor is entirely absent**. Socialize 0.73%
  (73 samples) and Groom 0.71% (71 samples) both fire; Mentor never does.
- `never_fired_expected_positives` includes `MentoredCat` and
  `GroomedOther` (and the new entries `CourtshipInteraction`,
  `PairingIntentionEmitted` though those belong to Mating).
- `continuity_tallies.mentoring = 0` — the mentoring continuity canary
  is dark for the entire 8-season soak.

The defect-shape is L3 cost-asymmetry: the L3 softmax pool contains
Mentor, but once `from_action` collapses Mentor → Socializing, the
planner only sees the DispositionKind and runs whichever step has the
lowest cost. MentorCat (cost 3) loses to SocializeWith (2) or
GroomOther (2) every time, and the trip-count goal satisfies on the
cheaper step.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | `src/ai/markers/...` | unverified — does Mentor have a marker distinct from Socialize? | `[suspect]` |
| L2 DSE scores | `src/ai/dses/...` | unverified — is there a separate mentor-DSE driving Mentor's score? | `[suspect]` |
| L3 softmax | `src/ai/scoring.rs::select_disposition_via_intention_softmax_with_trace` (line 1815) | Mentor IS in the action pool; chosen-Action recorded for trace, then collapsed via `from_action` | `[verified-correct]` |
| Action→Disposition mapping | `src/components/disposition.rs::from_action:123` | `Socialize \| Mentor → Socializing` collapses the cost asymmetry | `[suspect]` (the defect site) |
| Plan template | `src/ai/planner/actions.rs::socializing_actions` | `[SocializeWith (2), GroomOther (2), MentorCat (3)]` | `[verified-correct]` |
| Completion proxy | `src/ai/planner/goals.rs:60–67` | `TripsAtLeast(N+1)` — any sibling step satisfies the goal | `[suspect]` |
| Resolver | `src/steps/...` | `MentorCat` step exists but is unreachable in practice | `[verified-correct]` |

## Fix candidates

**Parameter-level options:**

- R1 — **lower MentorCat cost from 3 to 2** to tie SocializeWith /
  GroomOther on cost. Doesn't fix the underlying collapse: trip-count
  goal still satisfies on whichever step the planner's tie-breaking
  picks first; the L3 Mentor pick still vanishes. Param tweak that
  doesn't address the structural defect.
- R2 — **separate completion proxy for Mentor** within Socializing
  (e.g., gate the trip on `MentorshipInteractionDone`). Couples
  goal-shape to constituent identity inside one Disposition, which is
  exactly the substrate-over-search-state violation the architecture
  warns against (see CLAUDE.md substrate-refactor §4.7).

**Structural options:**

- R3 (**split**) — extract `DispositionKind::Mentoring` with constituent
  `[Mentor]`. Plan template `[MentorCat]`. Completion proxy
  `[InteractionDone(true)]` (matches Mating, Coordinating). Maslow
  tier 3. Update `from_action` so `Action::Mentor → Mentoring`,
  `Action::Socialize → Socializing`, `Action::Groom → None` (caller
  decides self-vs-other, unchanged). `constituent_actions` becomes:
  Socializing → `[Socialize, Groom]`; Mentoring → `[Mentor]`.
- R4 (**extend**) — keep Socializing as the umbrella but branch the
  plan template / completion proxy on the L3-picked Action (thread the
  Action into the planner instead of dropping it at `goap.rs:1599`).
  Generalizes to other clusters but adds a parallel `Action`-arg
  pipeline that the audit explicitly identified as the worse path —
  Crafting's `CraftingHint` is exactly this pattern and is itself a
  defect (see ticket 155).
- R5 (**rebind**) — re-map `Action::Mentor → Caretaking` (Maslow 3,
  exists, single-constituent today). Risk: Caretaking's plan template
  is `[Retrieve + Feed]`, not skill-transfer-shaped; the rebind would
  silently change Caretake's behavior or require Caretake to grow
  branching logic.

## Recommended direction

**R3 (split).** Direct match to ticket 150 R5a's precedent: a single
`DispositionKind` extracted to give the long-form action its own plan
template + completion proxy + Maslow tier (already correct at 3).
Mentor becomes substrate, not search-state. Avoids the
`CraftingHint`-style band-aid that R4 would introduce. R5 is rejected
as a category mistake: mentoring a peer is structurally different from
caring for a kitten.

`GroomedOther` is also never-fired, but it shares
`Action::Groom`-as-self with Resting (per `from_action:124`'s
self-vs-other split). Whether GroomedOther's never-firing is a
Mentor-class issue (cost-asymmetry crowd-out by SocializeWith) or a
caller-side issue (the self-vs-other resolver picks self too often) is a
sub-question for this ticket's investigation step. If GroomedOther
needs structural treatment, draft R3' alongside R3 in the
implementation plan.

## Verification

- **Hard gate:** after the split, `MentoredCat` must move off
  `never_fired_expected_positives` in a clean seed-42 deep-soak (`just
  soak 42` → `just verdict`). The `mentoring` continuity canary should
  reach ≥1.
- **Drift check:** `socializing` action distribution should not collapse
  — i.e., Socialize + Groom-other should still fire at roughly their
  pre-split rates (~0.73% + ~some-fraction-of-0.71%); only Mentor's rate
  should rise from 0%.
- **Focal-cat replay:** `just soak-trace 42 <focal>` should show
  `MentoringDSE` (or whatever the new DSE name is) winning at L2 for
  cats with high warmth + sociability + maturity. Compare against
  pre-split focal trace via `just frame-diff`.

## Out of scope

- **GroomedOther structural treatment** unless the ticket's
  investigation pass shows it's the same defect-shape. If it's a
  caller-side issue, open a separate ticket.
- **Maslow re-tiering**: Mentoring stays at 3 (matches Socializing).
- **Coordinator-driven mentor directives** ("send Patch to mentor
  Bracken") — that's a directive-substrate question, not part of the
  base extraction.

## Log

- 2026-05-03: opened by ticket 152's audit verdict on the Socializing
  cluster. See `docs/open-work/landed/152-...md` for the layer-walk and
  evidence trail.
