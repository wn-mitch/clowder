---
id: 289
title: EngageThreat retry cooldown after morale_break — suppress immediate Guarding re-election under active Fight directive
status: ready
cluster: combat-threat
orchestration: substrate-sensitive
initiative: []
added: 2026-05-11
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

<!--
Bugfix-shape ticket. Use this template (rather than `_template.md`) when the
work is a fix to observed defective behavior. The "Bugfix discipline" section
of CLAUDE.md REQUIRES at least one structural-revision candidate per fix-shape
decision tree — the slots below force that to be drafted, named, and considered.
-->

## Why

In the 288 verification soak (`logs/tuned-42`, post-288, seed 42),
Cedar took a ShadowFox ambush hit at tick 1235692, the EngageThreat
resolver fired `Fail("morale_break")`, and 288's new dispatcher branch
correctly released her Guarding commitment. **But on the very next
tick (1235693) L3 re-elected Guarding** — the Fight directive
(priority 1.0) was still active, so `ActiveDirectiveLift` pushed
Action::Fight back to the top of the softmax pool. A fresh
`[EngageThreat]` plan was built and the resolver fired `morale_break`
*again* on the same tick. Cedar oscillated through this pattern across
1235692-1235694 before the threat target despawned and the legacy
in-Guarding replan (target-invalid path) kicked her into
`[TravelTo(PatrolZone), Survey]`. She survived the run, so the
behavior didn't cost a death — but the **9 morale_break events** in
that single soak (across Cedar + Simba + others) are all rapid-cycle
thrashes, and Simba (tick 1227785, ambush at (22, 8)) ultimately died
68 ticks after her own morale_break. This is the thrash the 288 plan
explicitly named under `## Risks`:

> Cedar re-elects Guarding within 1–3 ticks, walks back to
> EngageThreat, morale_breaks again, abandons again. This is a thrash
> (spammy activation counter) not a death cycle (no replan inside
> Guarding means the cat doesn't keep marching deeper into ambush
> positions).

The hard-gate violated is observational, not survival: the
`CommitmentDropMoraleBreak` counter fires 1179× in a single soak (per
event scan over `logs/tuned-42/events.jsonl`) where the substrate
intent is "a release is an event, not a per-tick pulse." The
follow-on per 288's recommendation is to suppress immediate
re-election when EngageThreat just morale-broke — either by writing
EngageThreat onto `RecentTargetFailures` (or `plan.failed_actions`)
for a cooldown window, by attenuating the directive lift on
recently-broken cats, or by branching the L3 election to filter
EngageThreat candidates whose state still triggers morale_break.

## Hot context (auto-prefilled from /ticket-from-session; remove once picked up)
<!-- Failing run dir, footer gate violations, commit hash, recent edits, and
     any conflicting signals. Preserves open-time signal so a fresh session
     doesn't re-discover. Section is optional — present only when the ticket
     was opened via `/ticket-from-session`. Delete this whole section once
     the layer-walk rows have been promoted to [verified-*] and the fix
     direction is settled. -->

## Current architecture (layer-walk audit)

Walk every layer of the AI pipeline relevant to the defect. Tag each
load-bearing fact `[verified-correct]` (you read the code or a recent run
and it matches the assumption), `[suspect]` (you haven't verified, or it
looks wrong), or `[needs-promote]` (auto-prefilled by `/ticket-from-session`
from a hypothesis the Plan agent couldn't promote — the next session
promotes via a fresh query before any candidate that depends on the row).
A row tagged `[suspect]` or `[needs-promote]` MUST be addressed by at
least one of the fix candidates below.

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | `src/ai/markers/...` |  | `[verified-correct]` / `[suspect]` |
| L2 DSE scores | `src/ai/dses/...` |  |  |
| L3 softmax | `src/ai/scoring.rs` |  |  |
| Action→Disposition mapping | `src/components/disposition.rs::from_action` / `constituent_actions` |  |  |
| Plan template | `src/ai/planner/...` (or `goap_plan.rs`) |  |  |
| Completion proxy | `src/components/commitment.rs` |  |  |
| Resolver | `src/steps/...` |  |  |

## Fix candidates

**Parameter-level options** (resolver patch, predicate flip, scoring tweak,
marker threshold, etc.):
- R1 — …
- R2 — …

**Structural options** (at least one MUST be drafted, even if it doesn't win):
- R<N> (**split**) — give the action its own `DispositionKind` / DSE / Marker
  variant. Name the new variant and what moves into it.
- R<N+1> (**extend**) — keep the umbrella, branch the plan template /
  completion proxy on entry conditions so the umbrella varies by trigger.
- R<N+2> (**rebind**) — change the Action → Disposition mapping without
  inventing a new variant.
- R<N+3> (**retire**) — delete the variant if the layer-walk showed no
  load-bearing job. (Often N/A; include only if applicable.)

## Recommended direction
Which candidate (or combination) ships, and why the structural candidate did
or did not win. If a parameter-level option wins, briefly note why the
structural alternative was rejected — that's the audit trail.

## Out of scope
- What this ticket explicitly does NOT cover. Spin out follow-on tickets here.

## Verification
Hard-gate / canary the fix should restore. Soak seed + verdict expected.
Focal-cat replay (`just soak-trace <seed> <cat>`) if the defect was
narrative-bound to one cat.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- · **270** (ready, combat-threat, score 0.88) — EngageThreat split from Patrol DSE (256 R6 follow-on with Belief + ActionAfford…
- · **249** (parked, ai-substrate, score 0.86 (cross-cluster)) — Extend DispositionFailureCooldown coverage to Resting/Guarding/PickingUp et al.…
- ✓ landed **247** (done, ai-substrate, score 0.86 (cross-cluster)) — Diagnose IntentionMomentum + floor-removal PickUp-lock cliff

<!-- linkages:end -->
## Log
- YYYY-MM-DD: opened.
