# 310 S3 — ShadowFoxBeliefs ambush memory: four-artifact record

Ticket: `docs/open-work/tickets/310-*.md` (S3; release-plan step 23).
Commit: aa365199. Baseline: `tuned-42-571815fd` (S2 accepted artifact).
Gate artifact: `tuned-42-aa365199` (900s) — verdict **concern**
(survival/continuity PASS, never-fired clean, throughput +20% pass,
zero colony_score drift flags).

## Hypothesis

Predators don't hunt fished-out ponds. S3 adds `ShadowFoxBeliefs` — a
bespoke place-memory component with the MentalModel migration path
documented in its rustdoc (place-memories, not agent-models; folds into
a 258-style store only if shadow-foxes ever need to model cats as
agents). `den_position` migrates in from `ShadowFoxDrives` (S2);
`last_kill_site` + `last_kill_tick` are written on a landed ambush. The
kill-site consideration excludes cats within
`shadow_fox_kill_site_avoid_radius` (6.0) of the remembered site for
`shadow_fox_kill_site_memory_ticks` (20,000 — outlasting satiation
suppression, so returning hunger hunts elsewhere) at **all three**
target selections: legacy-roll pool, hunger election, and the
active-stalk retarget. `Feature::ShadowFoxKillSiteAvoided` names the
choice whenever memory (not geometry) reshaped it. The ticket's
`last_ward_encounter` field ships with S5 (its reader — substrate-stubs
discipline).

## Defect caught by the scenario en route

The active-stalk retarget (`predator_stalk_cats` Stalking arm) snapped
the target to the nearest cat every tick, unfiltered: a hunger election
toward clean ground snapped back to the fished cluster one tick later.
The retarget IS a target selection; it now respects the same filter
(empty filtered pool → hold the committed target). The bare-schedule
unit tests could not see this (no `predator_stalk_cats` in the
schedule); the full-App `shadowfox_kill_site_avoidance` scenario did —
the same lesson as S1's gate: election-layer claims need the full
pipeline before they're claims.

## Interpretation call (ticket ambiguity)

The ticket's verification line "second ShadowFox should NOT re-stalk at
the same site immediately (per-entity memory)" is read with per-entity
semantics per the ticket's own design text ("Each ShadowFox should
retain a per-entity last successful hunt site"): the killer avoids its
own site; a different fox holds no such memory. Colony-shared ambush
knowledge is the cats' side of the substrate (294 → per-cat
LocationBeliefs), not the predator's.

## Prediction → observation

1. Hard gates hold; knife-family flips expected (+2 constants) →
   **held**, and this family flagged nothing at all — the fulfillment
   −13..−16% streak breaks at four families (watch-item retained).
2. Soak-level near-null: kill-site avoidance needs kill + re-hunt near
   the same ground within 20k ticks — compound-rare at the ~1–2/900s
   ambush rate → **held** (2 ambushes, each with paired retreat; no
   avoidance events). `ShadowFoxKillSiteAvoided` ships
   expected_to_fire=false; the scenario hosts the assertion.

## Verdict

S3 **accepted** at aa365199. Rolled to S4: the beliefs component is the
natural read surface for DSE considerations (den distance, kill-site
freshness as scoring axes rather than hard filters).
