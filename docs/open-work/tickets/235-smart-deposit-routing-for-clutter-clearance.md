---
id: 235
title: Smart deposit routing for clutter clearance
status: ready
cluster: items-crafting
orchestration: substrate-sensitive
initiative: []
added: 2026-05-08
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

Ticket 231 landed `HasFreeSlot` + DropItem-as-prefix dual-branch
composition: cats with full inventory now compose `[DropItem,
PickUpItemFromGround]` automatically when they elect a pickup-class
disposition. The runtime resolver picks the lowest-priority slot via
`drop_priority` (curio < material < herb/food, with goal-aware
state modifiers).

**The narrative weakness.** A cat clogged with herbs who wants to
hunt drops the herb on the ground at the cat's current position.
Ideally — per the user's design intent in 231's scoping conversation
— the cat would route through the herb stash, deposit the herb
usefully, and then hunt: `[TravelTo(HerbStash), DepositHerb,
TravelTo(HuntingGround), Hunt]` rather than `[DropItem,
TravelTo(HuntingGround), Hunt]` with a herb left in the dirt.

**Required substrate expansion** (per 231's narrowing decision):
- Per-class inventory-content markers (`HasMaterialsInInventory`,
  `HasCuriosInInventory` — extending the existing
  `HasHerbsInInventory`).
- Colony-destination perception markers (`HasHerbStashAccessible`,
  `HasMaterialPileAccessible`).
- Class-specific `Deposit*` actions in pickup-class plan templates,
  competing with the bare `DropItem` on cost — A* prefers the
  routed-deposit when a stash is reachable.
- Curio-specific sink: curios have no destination today; either
  retire them as droppable-anywhere (the v1 behavior under 231) or
  introduce a `Cache` building (out of scope here, see ticket 16).

**Hard gate.** None today — 231's resolver-level `drop_priority`
ensures cats prefer dropping curios over hard-earned items. This
ticket is narrative-quality work, not a survival fix. Soak verdict
gate: post-ship, ground-item distribution should show herbs landing
near the herb stash rather than at random cat positions.

Blocked-by 231 because the substrate hooks (`HasFreeSlotThisPlan`,
DropItem-as-prefix composition) only exist post-231.

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
