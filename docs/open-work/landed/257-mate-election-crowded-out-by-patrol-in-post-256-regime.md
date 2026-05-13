---
id: 257
title: Mate election crowded out by Patrol in post-256 regime
status: done
cluster: ai-substrate
added: 2026-05-10
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 10f65c471c34
landed-on: 2026-05-10
---

<!--
Bugfix-shape ticket. Use this template (rather than `_template.md`) when the
work is a fix to observed defective behavior. The "Bugfix discipline" section
of CLAUDE.md REQUIRES at least one structural-revision candidate per fix-shape
decision tree — the slots below force that to be drafted, named, and considered.
-->

## Why

The post-256 verification soak (`logs/tuned-42`, commit `12023b1c` dirty)
restored every continuity canary the post-252 collapse had broken
(`grooming · play · mentoring · courtship · mythic-texture` all green;
courtship 0 → 1609; ShadowFox deaths under the ≤10 hard gate at 3) but
left `MatingOccurred` in `never_fired_expected_positives`. The hard gate
`never_fired_expected_positives == 0` therefore fails.

The post-252 baseline ALSO had `MatingOccurred = 0` — this isn't a 256
regression; it's a pre-existing structural gap at the courtship → mating
boundary that 256 surfaces by restoring the upstream substrate. With
1609 courtships and 0 matings, the pairing layer is firing but the
transition to actual mating isn't.

Observed Patrol share = **59.84%** (vs 63.65% pre-256, vs healthy
baseline ~25-30%). Patrol still dominates the action pool, crowding out
Mate, Coordinate, and other tier-3+ behaviors. Auto-memory
`project_l3_patrol_absorption_cascade` warns: "Substrate axes need to
price the predator-exposure cost of what they elevate, not just the
cost of what they suppress." 256 priced the *path* (R4 overlay weights)
and the *target* (R3 ward sectors) but didn't reduce Patrol's L2
score globally — the `safety_deficit` Logistic still saturates at 1.0
whenever fox-scent is detected anywhere on the map.

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
- 2026-05-10: opened from 256's verification soak. Run dir
  `logs/tuned-42` (commit `12023b1c` dirty). 4 deaths
  (1× WildlifeCombat / 3× ShadowFoxAmbush — under 10 hard gate);
  continuity canaries all green; courtship 1609; matings 0;
  kittens_born 0. Pre-256 collapse soak
  (`tuned-42-post-252-fleeing-collapse`) had matings = 0 too,
  so this isn't a 256 regression — it's pre-existing structural
  blocked by 256's substrate work for separability. Layer-walk
  audit unwritten; Patrol's `safety_deficit` Logistic gate is
  the leading suspect for crowding out Mate.
- 2026-05-10: 2026-05-10: reframed. Layer-walk disproved the ticket's original "Patrol crowds Mate via global safety_deficit" framing (Patrol's safety_deficit is cat-local; Mate elects 0/7455). Actual defect: PairingActivity Commit B (bias readers) was never wired per pairing.rs:22-27's own deferral note, so paired interactions advance the bond ladder no faster than diffuse ones; the chain stalled at Friends. Implementation: pairing_bias_for() helper in src/components/pairing.rs; bias readers in resolve_socialize/resolve_groom_other/resolve_mentor_cat amplify fondness+familiarity deltas 1.5× when target == PairingActivity.partner; Feature::PairingBiasApplied emits per amplification and is now canary-gated; pairing.emission_threshold 0.25→0.20 so fresh-Friends pairs actually emit Pairing intentions; ChainStepReadContext SystemParam bundles new pairing_q to stay under Bevy's 16-param limit. Verification soak logs/tuned-42 (commit 05b662c7 dirty, Mocha focal): Partners/Mates narratives present for the first time (Mocha+Cedar+Bramble @ ticks 1247100/1249950); bonds_formed 19→21; positive_features_active 33→34; survival-canary delta vs pre-fix is neutral (same 4 deaths). MatingOccurred remains in never_fired_expected_positives — the chain now stalls one layer downstream at has_eligible_mate()'s breeding-floor AND-gate (hunger>0.6 ∧ energy>0.5 ∧ mood>0.2 ∧ Partners ∧ photoperiod). Opened follow-on 272 owning that gate; references 032 (in-progress).
