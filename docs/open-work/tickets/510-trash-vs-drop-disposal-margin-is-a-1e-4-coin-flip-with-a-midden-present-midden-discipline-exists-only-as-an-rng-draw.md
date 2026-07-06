---
id: 510
title: Trash vs Drop disposal margin is a 1e-4 coin flip with a Midden present — midden discipline exists only as an RNG draw
status: ready
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-07-05
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
The disposal_election_trashing scenario (231's midden-discipline gate)
scores Trash 1.0088 vs Drop 1.0089 at its canonical setup (Midden
present, inventory full, seed 13) — softmax 46.35% vs 46.40%. The
"Trash wins" assertion only ever held by RNG draw; ticket 400 already
seed-shopped it (42→13) after one schedule-edge perturbation, and the
140 step-7 landing re-rolled it again (0 of 12 sampled seeds drew
Trash at tick 1). The 140 landing rewrote the test to the honest
current contract (score parity within epsilon); this ticket owns
making the margin DECISIVE so midden discipline is substrate, not
luck.

## Scope
- Give the Trash/Drop pair a real margin when a Midden exists: either
  a midden-presence consideration on Trash (accessibility-scaled) or a
  ground-litter penalty on Drop when a Midden is reachable — pillar 3
  (orthogonal axes), pillar 2 (substrate lever, not scenario dice).
- Restore disposal_election_trashing to a winner-draw assertion once
  the margin clears ~0.05.

## Out of scope
- Broader disposal-family retuning (Discard interplay stays as-is
  unless the new axis forces it).

## Verification
Scenario winner-draw green across ≥5 seeds; soak verdict pass with
no litter-rate regression (OverflowToGround / midden deposit rates).

## Log
- 2026-07-05: opened from the 140 step-7 gates; near-tie evidence in
  docs/balance/fluid-movement-phase2.md iteration 2 (to be appended).
