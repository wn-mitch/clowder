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
