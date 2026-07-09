# 266 — prey AI: Bolt + ScatterGroup (first prey-side elections)

Plan step 25, landings 2–3 (after 516's hunt-axis revival so every hunt
gate here measures live yield/calm axes). Chained four-artifact streams:
516 `afcc14e2` → Bolt `c2730756` → Scatter (below).

## Landing 1 — Bolt (`c2730756`) — ACCEPTED

**Gate:** four-artifact vs `logs/tuned-42-afcc14e2` (post-516 accepted).
**Run:** `logs/tuned-42-c2730756` (seed 42, Simba focal, 900s). **Verdict:**
concern-band; survival PASS, continuity PASS, hard gates all pass
(Starvation 0, ShadowFoxAmbush 0 ≤ 10, never-fired 0). 517 hitch-integrity
scan clean (0 multi-type duplications, 0 snapshot cadence gaps; 14
duplicate ticks are the legitimate twin-entity class).

### Mechanism

Ground prey (mouse/rat/rabbit) holding a live *detected* threat (Alert ∪
Fleeing — the honest-perception gate stays `try_detect_cat`'s
stealth/tremor model) score the `prey_bolt` DSE every 8 ticks: threat
chase affordance (0.45) + implanted violence belief (0.20) + escape
viability (0.35). A win preempts the freeze timer (or upgrades a fleeing
prey) into `Bolting`: flight away from the threat's *predicted* position
(pos + vel × 4) — the anti-`pursue()` geometry; the arc emerges through
the integrator's acceleration-limited steer. Fleeing had to join the
alert set at implementation time: mouse `freeze_ticks == 1` means mice
spend their whole encounter in Fleeing and would never have been scored
on an 8-tick cadence.

### Predictions → observations

- **P1 (PreyBoltElected ≥ 1) — indirectly confirmed; instrument gap
  found.** Feature tallies never reach the footer/logdb, so the count
  was not directly measurable per soak. The behavioral print below is
  unambiguous, and the scenario pins the mechanism deterministically —
  but the Scatter landing adds `PreyBoltStarted` / `PreyScatterStarted`
  EventKinds so cadence becomes countable (`just q events
  --kind=PreyBoltStarted`). `expected_to_fire_per_soak` stays `false`
  until the direct count confirms (honest-lift discipline).
- **P2 (ground-species success drops 5–25% relative; aggregate ≥ 43%) —
  CONCORDANT, with an emergent reversal.** Mouse 86.1 → 69.6% (−19%
  rel), rabbit 87.2 → 75.6% (−13% rel), rat 79.0 → 72.1% (−9% rel) —
  squarely in the predicted band. Aggregate went UP 61.4 → 68.4%
  because the 516 fish over-attraction largely reverted: fish attempts
  337 → 142, success 16.9 → 47.2%. Mechanism chain: bolting ends
  encounters quickly and resets prey alertness on release, so ground
  prey read as *calm* again and out-rank fish in the yield/calm
  selection — 516's structural fish-calm distortion partially
  self-corrected once prey stopped marinating in chronic alertness.
  Total kills nearly flat (653 → 622); colony fed, zero starvation.
- **P3 hard gates — PASS** (all four continuity canaries ≥ 1; burial /
  mythic-texture zeros are the known informational pair, not in the
  gate set).
- **P4 (hunt scoring shape stable) — holds by construction** (no
  hunt_target axis changes; selection drift is input drift).
- **P5 (prey ecosystem breathes) — kills −5%,** no den-abandonment
  spike.
- **P6 (throughput) — pass band but contaminated:** −13.4% with heavy
  compiles running concurrently on the soak machine (the 517 scan shows
  no in-sim hitch; wall-clock ticks/sec is the only affected metric).
  Re-measured on the idle Scatter soak below.

### Downstream trajectory notes

Shadow-fox channels swung (sieges −63%, ward-avoidance −64%, ambushes
2 → 0) — trajectory divergence through prey-position/satiation coupling
(shadow-foxes eat prey), not a mechanism change; within gates. End-state
colony: kittens 7, peak 15, fulfillment 0.199 (the fulfillment number
keeps oscillating 0.20–0.27 across this family; band-calibration
watch-item, not a trend).

## Landing 2 — ScatterGroup — pending gate

Written before the gate soak; see predictions in
`266-scatter-predictions` (scratchpad) and results appended below.
