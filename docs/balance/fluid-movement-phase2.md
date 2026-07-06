# Fluid free-range movement — 140 Phase II landings

## Iteration 1 — step 6: integrator + TravelTo/PatrolTo/FleeTravel (2026-07-05)

### Hypothesis
Travel/patrol/flee moving via smoothed corridors + acceleration-limited
velocity (instead of per-tick tile hops) preserves colony function
while making motion continuous: same destinations, slightly different
timing (acceleration from rest costs ~2 ticks per leg; diagonal travel
is √2 slower by the Euclidean speed cap — the deliberate re-baseline).

### Predictions (pre-registered)
| # | Prediction | Band |
|---|---|---|
| P1 | Determinism: extended 1200-tick byte-gate green with the integrator live | hard (already observed in `just test` pre-soak) |
| P2 | Survival gates | Starvation == 0; ShadowFoxAmbush deaths ≤ 10 |
| P3 | Continuity canaries ≥1 each; rates within ±10-ish% of the post-506 baseline where trajectory permits | soft (trajectory divergence expected — seed-42 softmax perturbation is pre-carried by the plan) |
| P4 | kittens_born ≥ 1 (the 508 fertility floor holds under re-timed travel) | hard-ish; on miss, diagnose before landing |
| P5 | Travel-time shape: plan-failure rates for travel-family reasons (`GoalUnreachable`, stuck-gates, HoldUntilSafe timeout) do not blow up (> 2× baseline rate would indicate watchdogs not absorbing the one-tick arrival latency) | gate |
| P6 | Throughput within −15% of the post-506 lineage (integrator adds a per-mover pass; smoothing replaces dense-path pops) | verdict channel |

### Observation
Two soaks. **Soak 1 (`logs/tuned-42-2693ef2a`): FAIL** — ~1,750
`TravelTo(*): travel timeout` failures + 1 Starvation death. Root
cause: `waypoint_arrival_radius` 0.35 < max step 1.0 — the pop check
samples once per tick, so movers jumped the window and orbited missed
waypoints until watchdogs fired; food logistics stalled (472 Stores
timeouts) → starvation. Fixed: radius 0.6 (window diameter 1.2 >
max step — un-jumpable) + `travel_timeout_ticks` 200 → 300 (accel
ramp + √2 diagonals are the plan's deliberate slowdown).

**Soak 2 (`logs/tuned-42-e03bcb0f`): survival + continuity pass.**
- Travel timeouts **0**; zero deaths; kittens 3; welfare 0.52 (best
  of the 0.4.0 lineage); grooming/play/mentoring/courtship all
  healthy (mentoring 2275).
- ticks_per_sec **139.1 (+24.2% vs post-506 baseline)** — the
  integrator + sparse-waypoint travel is NET FASTER than per-tick
  dense-path popping (fewer A* recomputes, fewer path allocations).
- P1 determinism (1200 ticks) green pre-soak. P4 kittens ✓. P6 ✓.
- P5 watch items: `EngagePrey: stuck while stalking` 318 (unmigrated
  stalk arms — owned by steps 9-12's gait/pursue rework);
  `TravelTo(HerbPatch): no path and stuck` 172 — the 466-carried raw
  gate (≤100) is breached in raw count but the run is ~2× the
  gate-era tick length (rate-equivalent ≈ 83/60k-ticks); the RATE
  still grew +73%, tracked as a step-9-12 watch item alongside the
  herb-reachability question.

### Concordance
Core concordant after one fix iteration; the fix itself is the
soak-gate working as designed (a sub-step/pop-window interaction no
unit test modeled). Deliberate re-baseline components (slower legs,
faster wall-clock) land as predicted. Raw-count gates from the
1-tile/tick era are duration-confounded at the new tick rates —
prefer rate-normalized reads (footer-rate-arithmetic discipline).

## Iteration 2 — step 7 gate: ticket 511 kitten-starvation chain (2026-07-06)

Step 7's landing soak surfaced a kitten starvation that turned out to
be a five-defect chain, none of it step-7's code (deeper tick depth
exposed it): (1) no juvenile self-feeding substrate → R2b juvenile
Eat/BegForFood; (2) Resting excluded from Starvation urgency; (3)
urgency preemption blocked by equal-tier compare; (4) held commitment
never released on starvation → `starvation_override` fail sentinel;
(5) **the terminal lock** — early-graduate life-stage hole
(maturity 1.0 via Wean/Teach bumps removes `KittenDependency` while
the age band still reads Kitten → all sub-stage markers stripped →
`current_cat_life_stage` = None → empty DSE pool → empty-pool
Resting fallthrough, elections never record scores). Full chain in
ticket 511's log.

### Hypothesis
With the early-graduate fix (dep-less age-Kittens stay
`JuvenileKitten`), Duskkit-45 keeps a live DSE pool after maturity
1.0, self-feeds via the R2b substrate, and the step-7 soak passes all
hard gates.

### Predictions (pre-registered, soak on commit 36a4fc08)
| # | Prediction | Band |
|---|---|---|
| P1 | Starvation == 0; Duskkit-45 alive past tick 1297266 | hard gate |
| P2 | Duskkit-45 elects Eating/Foraging-family plans after maturity 1.0 (~tick 1288150); Resting share of its post-graduation plans < 50% | mechanism confirmation |
| P3 | KittenBegged / KittenFed continue firing; grooming · play · mentoring · courtship canaries ≥ 1 | hard gate |
| P4 | kittens_born ≥ 1; ShadowFoxAmbush ≤ 10; never-fired == 0 | hard gate |
| P5 | starvation_override PlanStepFailed rate ≈ 0 (the sentinel exists but the empty-pool relock it was masking is gone; occasional firings during genuine sleep-while-hungry are acceptable) | watch |
| P6 | Throughput within noise of soak 2 (139 tps); the fix is one match-arm | verdict channel |

### Observation (`logs/tuned-42-36a4fc08`, verdict `concern`)
- **P1 ✓ ZERO deaths run-wide.** Duskkit-45 alive and planning at
  tick 1316946+ (prior lock killed it at 1297266).
- **P2 ✓** Post-graduation plan mix (500 plans from 1288150):
  Foraging 185 · Grooming 141 · Socializing 70 · Exploring 69 ·
  Resting **6** (1.2%; was 399/400) · Eating 1 + the beg/feed cycle
  carried by KittenFed/KittenBegged features.
- **P3 ✓** grooming 2388 · play 168 · mentoring 1191 · courtship
  17630. **P4 ✓** survival canary pass (Starvation 0, ShadowFox ≤ 10,
  never-fired 0), kittens_born 2 (≥ 1). **P5 ✓** plan-failure reasons
  are all GoalUnreachable-family; no starvation_override in the
  footer tallies. **P6 ✓** 137 tps (+22.3% vs the 112-tps post-506
  baseline; soak-2 parity).
- Verdict `concern` = drift channels vs the pre-step-6 baseline
  (fulfillment +181.6%, kittens_born 4→2, structures −26.7%,
  founder dispersion cuddle-puddle windows — ticket 490's known
  signature). All pre-carried by the plan as trajectory divergence;
  re-baseline lands at step 13 (⚑).

### Concordance
Fully concordant — five stacked defects, and the gate held until the
last one (the life-stage hole) was fixed; the four earlier fixes were
each necessary (the R2b substrate is what the restored elections now
land in) but not sufficient. Methodological note for the bugfix doc:
"fix verified firing + outcome unchanged" is the signature of a
defect UPSTREAM of the fix's layer, not of a wrong fix — the layer
walk should have climbed from the commitment layer to the eligibility
layer one iteration sooner.
