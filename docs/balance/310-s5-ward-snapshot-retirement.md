# 310 S5 — ward-snapshot retirement: four-artifact record

Ticket: `docs/open-work/tickets/310-*.md` (S5, final stage; release-plan
step 23). Commit: cf3d55ae. Baseline: `tuned-42-910a1cb7` (S4 accepted
artifact). Gate: `tuned-42-cf3d55ae` (900s) — verdict **concern**
(survival/continuity PASS, ShadowFoxAmbush deaths 2 ≤ 10, never-fired
clean, throughput +2.4%).

## Hypothesis

`predator_stalk_cats` still made its ward decisions from the pre-260
`ward_positions × shadow_fox_ward_repel_multiplier` snapshot — the ×3
inflated radius (~27 tiles vs ~9 actual coverage) that ticket 310's Why
section names as the safety blanket masking the absence of predator AI.
With S1–S4's substrate landed (satiation gates on all predation
entries, kill-site memory, ward-filtered selections, deliberate DSE
hunts), the hack retires second (pillar 2): the in-ward flee decision
and the stalk-cancel read `WardCoverageMap` at the actual tile against
`shadow_fox_ward_avoid_threshold` — one substrate channel for every
ward decision, matching the hunt-pool filter and wildlife_ai's 260
reads. Ward entities remain geometry-only flee anchors. Also retired:
dead `ambush_cooldown` writes (readerless since S4 retired the roll).
Descoped: `ShadowFoxBeliefs.last_ward_encounter` — no honest reader
exists (retreat-geometry was the candidate; retreat election is
dormant); 518 or pack coordination may motivate it.

## Prediction → observation

1. Ambushes spread, not waves (the ticket's own verification line) →
   **confirmed**: 11 ambushes at inter-ambush gaps 4,918–20,773 ticks
   (one 378-tick pair — a two-hit engagement, not a train).
2. Hard gates → held (deaths 2; continuity pass).
3. Foxes operating the 9–27-tile band could raise proximity pressure →
   the opposite: haunting fell to 152 (S4 artifact: 261), no siege
   flag, and the colony reads BETTER than the S4 baseline (fulfillment
   +80.1%, welfare +8.2%, the health −20.8% flag gone). Reading: the
   inflated radius was pinning foxes into a narrow annulus around the
   colony's ward line; with real coverage geometry they disperse.

## Verdict

S5 **accepted** at cf3d55ae. Ticket 310 complete: the shadow-fox is a
goal-directed predator — satiation-gated hunts elected by hunger, night
and concealment; kill-site memory steering it to fresh ground; a den it
carries kills home to; every ward decision on the substrate channel;
and the pre-AI pinball (5%/tick roll, ×3 repel blanket, cooldown-only
cadence) fully retired. Predation posture (engagement ~5–6× the pinball
era) is the named step-24/25 item.
