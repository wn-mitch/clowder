# 310 S2 — shadow-fox den + post-ambush retreat: four-artifact record

Ticket: `docs/open-work/tickets/310-*.md` (S2; release-plan step 23).
Commit: 571815fd. Baseline: `tuned-42-1effd660` (S1 accepted artifact).
Gate artifact: `tuned-42-571815fd` (900s) — verdict **concern**
(survival/continuity PASS, never-fired clean, throughput −11.9% pass).

## Hypothesis

Post-ambush the shadow-fox resumed patrol at the kill site (cooldown +
S1 satiation gates only). S2 gives it a home: a `ShadowFoxDen` world
entity at the corruption-saturated manifestation origin (items-are-real
— a spatial anchor, not a home flag; reused within
`shadow_fox_den_reuse_radius` 8 so dens don't stack across spawn
cycles), `den_position` recorded on `ShadowFoxDrives`, and a
SingleMinded `Retreating` state entered on a landed ambush — held by
the motivation-tick guard (like Stalking/EncirclingWard), driven by
`steering::arrive` on the DesiredVelocity contract (140 step-11), and
released to Patrolling within `shadow_fox_retreat_arrival_radius` (1.5)
of the den. Den unknown (pre-S2 saves, bare spawns) falls back to the
legacy Patrolling reset.

## Prediction → observation

1. Hard gates hold; near-null soak drift beyond the +3-constants
   knife-family flip → **held** (survival/continuity PASS; drift flags
   are the family class, e.g. shelter +9.7% → −18.5% between adjacent
   families).
2. Retreats fire rarely at soak scale (S1-family ambush rate ~0–1 per
   900s), deterministic proof in the scenario → **exact**: one Ambush
   (tick 1,226,543, Calcifer) with `ShadowFoxRetreatEntered` paired at
   the same tick, den (26,61). The extended
   `shadowfox_hunger_hunt_cycle` covers the full loop: hunger election
   → stalk from beyond legacy sight → ambush ~tick 11 → Retreating →
   home leg → released ~tick 29.
3. `ShadowFoxRetreatEntered` ships expected_to_fire=false (scenario
   hosts the assertion; 1:1 sibling rule).

## Scenario race fixed en route

The S1 scenario started the fox `Patrolling {dx:1}`: the eastward drift
raced the first motivation cadence (~tick 15) into legacy detection
range (≤ 8) by ~tick 5, letting the 5%/tick roll win the Stalking entry
in some trajectory families — the S1 pass had been family luck. The fox
now starts `Waiting` (holds position; the legacy roll only fires from
Patrolling/Circling), making the hunger election the only possible
entry by construction.

## Watch-item sharpened

Fulfillment has read −13% to −16% vs baseline across four consecutive
trajectory families (S1 iterations 1–3 and S2). Individually each is
the knife-family drift class; the persistence of sign and magnitude
across families upgrades it from noise toward trend. Step 24's baseline
re-promote should check the fulfillment trajectory explicitly (it is
already on the rolled watch-item list).

## Verdict

S2 **accepted** at 571815fd. Rolled to S3/S4: den_position migrates
into `ShadowFoxBeliefs` (S3); rest/recovery at the den is S4's business.
