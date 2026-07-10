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

## Landing 2 — ScatterGroup (`7aad3c49`) — ACCEPTED (P2 reframed)

**Gate:** four-artifact vs `logs/tuned-42-c2730756` (accepted Bolt stream).
**Run:** `logs/tuned-42-7aad3c49` (seed 42, Simba focal, 900s, idle
machine). **Verdict:** concern-band; survival PASS, continuity PASS, hard
gates pass (Starvation 0, ShadowFoxAmbush 2 ≤ 10, never-fired 0). 517
hitch scan clean (132 duplicate ticks all single-type — herd members
bolting from a shared tile, the legitimate twin-entity class; 0 snapshot
gaps).

### Predictions → observations

- **P1 (direct counts) — CONFIRMED: 432 `PreyBoltStarted` + 475
  `PreyScatterStarted` per 900s.** The new EventKinds close the Bolt
  gate's instrument gap on their first run. Scatter volume far exceeded
  the "O(tens)" guess — prey cluster around dens, so the census gate is
  satisfied more often than not. `expected_to_fire_per_soak` for both
  Features can lift to `true` next commit (seed-42 observation
  requirement met with three orders of magnitude of margin).
- **P2 (±10% aggregate) — REFUTED-THEN-REFRAMED, honest emergence.**
  Aggregate 68.4 → 75.9% (+10.9% rel); every species rose (mouse 75.0%,
  rat 76.8%, rabbit 83.9% on 354 attempts — the herd species; fish
  59.0%, continuing the 516-distortion recovery). The divergence
  geometry itself works: chase timeouts became the top ground-prey
  failure (the pursuing cat's `pursue()` lock does break, as designed).
  But a flushed herd scatters across a field holding EIGHT hunting
  cats — members fleeing their own threat cross other hunters' paths
  and get picked up. This is the real reason cooperative predators
  flush herds; the sim discovered it from geometry. Not a sign defect:
  the ±34° parity rotation always keeps a positive away-component from
  the elected threat (`scattering_herd_diverges` pins it).
- **P3 hard gates — PASS.**
- **P4 shape stable — holds by construction.**
- **P5 throughput — 114 tps on the idle box** (family band 112–120):
  confirms the Bolt run's 104 was compile contention and prices the
  census + two-candidate election within the alert-set envelope.
- **P6 hitch — clean.**

KnowledgePromoted watch-item (step 24 → 25): still 0 per ~108k-tick
window across all three step-25 streams (516 / Bolt / Scatter). The
chain-rare demotion continues to hold; the false-belief scenario remains
the mechanism gate. No action.

### Hand-off to the band calibration

Post-266 hunt success sits at **75.9%** against the 30–50% biology band
— HIGHER than pre-prey-AI, because both emergent effects (fish-calm
reversal, herd flushing into the pack) favor the cats. The calibration
now has real instruments (bolt/scatter cadence via events; per-species
success via `just q hunt-success`) and honest substrate on both sides.
Escape-side knobs (`prey_bolt_election_threshold`,
`prey_scatter_election_threshold`, `detection_base_chance`) are the
pillar-2-preferred levers before any cat-side nerf.

## Band-calibration iteration 1 (`9306c110`) — FAIL, REVERTED, PARKED

**Knobs:** bolt/scatter thresholds 0.45 → 0.38, `detection_base_chance`
0.10 → 0.14 (uniform escape-side family). **Run:**
`logs/tuned-42-9306c110` vs the accepted Scatter stream. **Verdict:
FAIL** — `Starvation == 0` hard gate (1 starvation death).

### Both prediction families refuted

- **The knobs are inert.** Escape cadence 917 vs 907 elections/900s
  (predicted 1,300–2,300); hunt success 77.5% vs 75.9% (predicted a
  10–25-pt drop); kills 735 vs 714. The election score distribution is
  bimodal — committed chases score ≈ 0.65+, uncommitted ≈ 0.25 — so
  nothing lives between 0.38 and 0.45; and detection is not the binding
  constraint on escape volume. Success is decided DOWNSTREAM of the
  elections: chase kinematics (`sprint_speed_mult` 3.0 vs ground-prey
  flee cap 1.0 — chases are geometrically certain; mouse hit 94.4% this
  run with six chase timeouts total) and the pounce strike window
  (`pounce_awareness_idle` 0.95). Knob values reverted; the refutation
  is recorded on the constants' doc-comments.
- **The starvation is NOT a food undershoot.** Kill volume was the
  highest of the four step-25 streams; nourishment 0.692. Finchkit-18
  (a kitten) starved ~43k ticks after its likely caretaker Calcifer
  died of injury, churning 1,993 plans / 1,714 failures (1,629
  `SelfGroom: starvation_override`) while the colony ate. This is
  ticket 156's explicitly-unresolved "orphan-care path" corner (its
  named follow-on, 158, was repurposed for GroomedOther) → **ticket 529
  opened** with the layer-walk plan.

### Disposition

The calibration is **parked behind 529** (`ticket 530`, blocked-by):
the orphan pathology fails the hard gate on trajectories independent of
the knobs, making every calibration soak a gate lottery — the
park-behind-the-blocker rule. The remaining honest levers are
design-shaped and per-species (rabbit locomotion, strike-window
reaction, fish wariness — full analysis in 530), which is the plan's
own "per-PreyKind only if uniform fails" fallback. 0.4.0 ships at the
honest above-band 75.9%; step 25 closes on the accepted Scatter stream
(`7aad3c49`).
