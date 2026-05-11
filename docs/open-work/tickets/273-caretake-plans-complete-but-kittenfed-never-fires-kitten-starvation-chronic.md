---
id: 273
title: Caretake plans complete but KittenFed never fires — kitten starvation chronic
status: ready
cluster: null
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

Every recent seed-42 deep-soak ends with exactly one kitten born and that
kitten starving to death over ~21 sim-days while the colony's `Feature::
KittenFed` event count stays at **0**. Surfaced during 127-Commit-C
verification (`logs/tuned-42`, commit `e60159bc`): Mocha gave birth to
Maplekit-92 at tick 1278320, the kitten received zero `KittenFed` events
across its 20898-tick lifespan, and starved at tick 1299298. The exact
same failure mode appears in the pre-127 baseline (`logs/tuned-42-pre-127`,
commit `3444d2d9`): Dawnkit-28 born to Mocha, never fed, starved at tick
1321484. Hard-gate violation: `deaths_by_cause.Starvation == 1` (must be
0). The continuity canary stays green because deaths/burial were demoted
from the canary set in ticket 250 — but kitten starvation is a separate
welfare line item that's been chronically broken without canary coverage.

`Feature::KittenFed` is cascade-exempt from `expected_to_fire_per_soak()`
(per `system_activation.rs:884-887`, the cascade-from-trunk demotion that
keeps one root-cause failure from multiplying into N canary entries). The
trunk it cascades from is `MatingOccurred` — and in current soaks Mating
*does* fire (≥1 birth per soak). So the cascade-exemption no longer
masks: the chain `MatingOccurred → KittenBorn → KittenFed` breaks
between birth and feeding, but `KittenFed`'s status as cascade-exempt
hides it from the never-fired-canary surface.

## Hot context

- Run dir: `logs/tuned-42` (127-Commit-C, commit `e60159bc`, headless
  release 15-min).
- Footer gate violation: `deaths_by_cause.Starvation: 1`, `kittens_born: 1`
  in 127-C and `2` in pre-127 (the kitten that's born starves either way).
- Caretake disposition plans **do** get created (3 in 127-C, 9 in pre-127)
  with the canonical `[TravelTo(Stores), RetrieveFoodForKitten, FeedKitten]`
  shape. None complete — no `KittenFed` events, no `PlanStepFailed`
  records for these three cats during the kitten lifespan window.
- Pre-127 had **816** `PlanStepFailed: HandoffItem` events with reason
  `"no kittens in colony"` — that's the **Handing** disposition (not
  Caretake) failing to find a recipient when a kitten existed. 127-C had
  17 — fewer attempts, not better resolution. Suggests the recipient-
  filter sees the kitten inconsistently.
- Kitten-care substrate is intact (verified to compile + author):
  `KittenCryMap` (Hearing channel), `CaretakeDse` (reads
  `kitten_cry_perceived`), `KittenCryCaretakeLift` modifier, alloparenting
  compassion-weighted Reframe A in `scoring.rs:407-700`, `FeedKitten`
  step in `goap_plan.rs:366`, `RetrieveAnyFoodFromStores` plan-template
  preceding `FeedKitten`. No witness/emission obviously broken on read.

Open-time signals captured here so the next session doesn't re-discover.
Remove this section once layer-walk rows are promoted.

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

## Log
- YYYY-MM-DD: opened.
