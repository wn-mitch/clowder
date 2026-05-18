---
id: 272
title: MatingOccurred still gated by has_eligible_mate() breeding-floor AND-gate (post-257 follow-on)
status: done
cluster: ai-substrate
initiative: []
added: 2026-05-10
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 2e3666e327db
landed-on: 2026-05-10
---

<!--
Bugfix-shape ticket. Use this template (rather than `_template.md`) when the
work is a fix to observed defective behavior. The "Bugfix discipline" section
of CLAUDE.md REQUIRES at least one structural-revision candidate per fix-shape
decision tree — the slots below force that to be drafted, named, and considered.
-->

## Why

The post-257 verification soak (`logs/tuned-42`, commit `05b662c7` dirty,
Mocha focal) confirmed that 257's Pairing Commit B substrate fix landed
end-to-end: **Partners + Mates bonds form for the first time** (Mocha and
Cedar at tick 1247100 → 1249950; Mocha and Bramble same window),
`bonds_formed: 21` vs pre-fix 19, `Feature::PairingBiasApplied` fires
(`positive_features_active 33 → 34`). **But `MatingOccurred` remains in
`never_fired_expected_positives`** — the hard gate
`never_fired_expected_positives == 0` still fails. `just q actions
logs/tuned-42` shows `Mate` action electing 0 of 7263 CatSnapshot rows;
Mocha's focal L2 trace in the 1247000–1248000 tick window (immediately
after Partners promotion) shows **`mate` DSE with `eligibility_fails =
37 / 37`** — `HasEligibleMate` marker is never inserted despite Mocha
holding a Mates bond.

The blocker is one layer downstream of 257's substrate work: the
**compound AND-gate inside `has_eligible_mate()` (`src/ai/mating.rs:172-225`)**
that requires `season_fertility > 0 ∧ self.mating_need <
mating_interest_threshold ∧ is_fertile(self) ∧ is_sated_and_happy(self)
∧ is_fertile(other) ∧ is_sated_and_happy(other) ∧
orientation_compatible ∧ at_least_one_conception_viable ∧ bond ∈
{Partners, Mates}`. 257 lifted the bond-ladder gate (the last clause);
the remaining clauses — particularly the `is_sated_and_happy` floors
(`breeding_hunger_floor = 0.6`, `breeding_energy_floor = 0.5`,
`breeding_mood_floor = 0.2`) — are now the gating layer.

`breeding_hunger_floor`'s doc-comment names this exactly: *"colony-wide
reproduction collapse traces partly to this floor: at `0.6`, the
AND-gate of (hunger > 0.6 ∧ energy > 0.5 ∧ mood > 0.2 ∧ partners-bond
∧ photoperiod) is rarely satisfied because the colony lives in
survival mode. Treatment override `0.4` for the 032 hypothesize
sweep."* That treatment was never permanently landed; ticket 032
("Starvation rebalance — align with IRL cat biology, interesting not
cutthroat") remains `in-progress` and owns this work, but 272 is the
ticket that wakes 032 up against the post-257 substrate so the hard
gate can finally close.

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
- 2026-05-10: 2026-05-10: landed lower of breeding_hunger_floor 0.6 → 0.4. Verified seed-42 soak-trace Mocha 900s (commit 3444d2d9 dirty): never_fired_expected_positives=[] (was [MatingOccurred]); kittens_born=2; courtship 1609 → 2251; bonds_formed 21 → 26; positive_features_active 34 → 38. Mocha (pre-272 ShadowFox death tick 1303610) survives in new trajectory. Wildlife/ShadowFox death set stable (Calcifer/Heron/Bramble died at identical ticks pre/post). Downstream: Dawnkit-28 starves tick 1321484 — kitten-survival pipeline is next bottleneck (187 ready, post-272 reproduction logged there). 032 Item 3 closes; items 1, 2, 5 remain.
