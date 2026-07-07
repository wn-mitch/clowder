---
id: 513
title: shadowfox colony-pressure channel fading under fluid movement: drives softmax self-anchors on remote corruption (Reconstituting-Seeding loop), scent shell repels at fine granularity, ward/siege/haunting interactions zero on both gate seeds
status: ready
cluster: wildlife
orchestration: substrate-sensitive
initiative: []
added: 2026-07-06
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
Across the Phase II gate soaks the shadowfox-vs-colony conflict
channel has gone quiet: `shadow_foxes_avoided_ward_total` 2142 (post-506
baseline) → 0 on seed-42 from step 8 onward, and seed-43 — which still
showed 1334 avoidances + 68 sieges + 27 hauntings on the step-8 binary
(`tuned-43-fdaf4152`) — dropped to zero everything except
Reconstituting/Seeding on the step-11 binary (`tuned-43-a961e2b0`).
Wards placed halve in response (42 → 13–28: no fox pressure → less
corruption near colony → less ward demand). Hard gates still pass
(zero deaths, canaries green) — this is texture/pressure fading, not
survival damage. mythic-texture canary has been 0 on seed-42
throughout.

## Evidence table
| Run | Binary | avoided_ward (footer) | AvoidedCatScent | Haunting entries | wards placed |
|---|---|---|---|---|---|
| tuned-42-3e4f7caf (baseline) | pre-Phase-II | 2142 | — | — | 42 |
| tuned-42-fdaf4152 | step 8 | 0 | 16 | 45 | 18 |
| tuned-42-44d3ecfb | step 10 | 0 | 118 | 0 | 27 |
| tuned-42-a961e2b0 | step 11 | 0 | 8554 | 0 | 24 |
| tuned-43-fdaf4152 | step 8 | 1334 | 1165 | 27 | 7 |
| tuned-43-a961e2b0 | step 11 | 0 | 0 | 0 | 15 |

## Mechanism (layer-walked in-session, all [verified-*])
1. **Self-anchoring drives loop** — a shadowfox deposits corruption on
   every tile it crosses; `shadowfox_motivation_tick`'s Reconstituting
   target is "highest-corruption tile within scan radius 12" — i.e.
   usually its own trail. Coherence/entropy pressures stay fed by the
   fox's own deposits, so the softmax cycles Reconstituting↔Seeding in
   a remote corruption field indefinitely (46–70 entries per run).
   Dread (the cat-directed drive) requires a cat within 12 tiles —
   never true when camped 60 tiles out. Pre-existing shape, but fluid
   movement + spawn trajectories now park BOTH foxes there on BOTH
   seeds.
2. **Scent shell as hard barrier** — the patrol arm reverses heading
   whenever the tile ahead crosses `cat_scent_avoidance_threshold`.
   Post-step-11 the fox re-triggers this at integrator granularity
   (8554 reversals on seed-42 — it paces the shell), and Stalking (the
   only state that ignores scent) is reachable only via dread/siege,
   which require the proximity the shell prevents. The colony's scent
   halo is now effectively impenetrable to patrol-state shadowfoxes.

## Fix candidates (Phase V scope — steps 22–25 living-world work)
- R1 (**drives rebalance**) — decay/discount self-deposited corruption
  in the Reconstituting scan (a fox shouldn't be nourished by its own
  trail), and/or hunger-analog pressure that grows while coherence is
  sated, pushing exploration toward the colony frontier.
- R2 (**scent shell porosity**) — probabilistic/threshold-graded
  penetration instead of hard reversal (bold or desperate shadowfoxes
  probe deeper), composing at the modifier layer.
- R3 (**310 seam**) — when wildlife perception/belief wiring lands
  (Phase IV/V), shadowfox target acquisition should read the belief
  substrate rather than raw proximity scans, giving dread a memory
  that survives distance.

## Out of scope
- Any Phase II landing gate — hard gates pass on both seeds; this is
  the pressure-economy regression tracked so Phase V can restore it
  deliberately (it owns ShadowFoxAmbush/mythic-texture rates).

## Verification
Seed-42 AND seed-43 soaks: `shadow_foxes_avoided_ward_total > 0`,
`ward_siege_started_total > 0`, ≥1 Haunting entry per run, and
mythic-texture canary ≥ 1 — restoring the pre-Phase-II conflict
texture without breaching `ShadowFoxAmbush ≤ 10`.

## Log
- 2026-07-06: opened from the step-11 gate cross-seed check. Evidence
  chain + mechanism layer-walk complete; parked for Phase V (steps
  22–25) per plan; cross-linked from
  docs/balance/fluid-movement-phase2.md Iteration 6.
