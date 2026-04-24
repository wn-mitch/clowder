---
id: 003
title: Mentor score magnitude
status: ready
cluster: null
added: 2026-04-20
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Current state

(from iter-2 diagnostic, 2026-04-20)

**Why it matters:** "Mentoring fires ≥1× per soak" is a continuity
canary currently failing. The iter-2 diagnostic for social_target_range
(commit `290a5d9`) showed Mentor's gate opens 43.7% of baseline
snapshots — gate availability is **not** the blocker. The blocker is
raw score magnitude: Mentor mean score 0.126 vs Sleep 0.802, Eat 0.725,
Hunt 0.669. Mentor cannot win scoring even when its gate is open.

**Touch point:** `src/ai/scoring.rs:597–605` + constants
`mentor_warmth_diligence_scale: 0.5` and `mentor_ambition_bonus: 0.1` in
`src/resources/sim_constants.rs`. For comparison
`socialize_sociability_scale = 2.0` — Mentor is 4× smaller in scale
despite stricter gates.

**Hypothesis:** Raising `mentor_warmth_diligence_scale` to ~1.5–2.0 lifts
Mentor score into competitive range, producing ≥1 Mentor firing per
seed-42 soak (continuity canary). Secondary effect: the already-consumed
apprentice-skill-growth path at `src/systems/goap.rs:2672–2743` becomes
load-bearing for the first time, so skill progression for low-skill cats
accelerates. Orthogonal to social_target_range work.

**Bounds/risks:** Mentor competes in the utility layer with Socialize;
over-tuning could re-trigger the iter-1 mating regression via a
different pathway. Measure MatingOccurred / KittenBorn as mandatory
canaries on any Mentor tuning.
