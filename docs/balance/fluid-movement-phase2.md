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

## Iteration 3 — step 8: perception metric pivot, 494 inverted (2026-07-06)

### Hypothesis
With locomotion now isotropic (Euclidean speed clamp), pivoting
`distance_to` to world-space Euclidean re-aligns perception with
actual travel time. Diagonal candidates read √2 farther than the
Chebyshev era — nearest-X picks shift at former Chebyshev ties, range
gates tighten at diagonals, and the colony re-equilibrates without
survival damage.

### Changes
- `distance_to` → `self.0.distance(other.0)` (world-space, sub-tile
  visible); `euclidean_distance` → same metric (was tile-quantized),
  survives as the radial intent-marker; `tile_distance_squared` →
  `dx²+dy²` over tiles (pre-494 body; i32/Ord-composable nearest-pick).
- Chebyshev call-site audit (33 sites): all direct callers are
  adjacency/strike/reach reads — stay.
- `distance_to`-with-small-constant audit: 10 tactical sites converted
  to `chebyshev_distance`/tile-equality (shadowfox ambush-adjacency,
  gate-hold `== 1.0` [exact float equality — would never fire under a
  continuous metric], patrol/move_to same-tile arrival, 5 building/
  remedy `> 1.0` approach gates [Euclidean would forbid diagonal work
  positions and fight the separation force]). Radial keeps: workshop
  bonus zone, fox den zones, flee arrival tolerance, building auras.
- NearPairCache admission + debug parity stay **tile-quantized**
  Euclidean (`tile_distance_squared`): `CatMoved` fires on tile
  crossings only, so a continuous admission metric would desync the
  event-driven cache (caught by the 431 parity panic in `just test`).
- Hawk `step_flying` arrival stays Chebyshev (signum tile-stepper
  until step 10 retires it).

### Predictions (pre-registered)
| # | Prediction | Band |
|---|---|---|
| P1 | 1200-tick determinism byte-gate green | hard (observed pre-soak in `just test`) |
| P2 | Starvation == 0; ShadowFoxAmbush ≤ 10; never-fired == 0 | hard gate |
| P3 | Continuity canaries ≥ 1 each; trajectory divergence expected (nearest-pick ties flip → softmax RNG stream shifts) | hard gate / soft rates |
| P4 | kittens_born ≥ 1 | hard-ish; diagnose before landing on miss |
| P5 | GoalUnreachable-family plan-failure rates within ~2× of iteration-2 run (range gates tighten at diagonals; watch Guarding 102 / Hunting 83 / PickingUp 81 / Foraging 50 / Herbalism 21 raw counts, rate-normalized) | gate |
| P6 | Throughput parity with iteration 2 (137 tps) — metric swap is arithmetic-neutral (Euclidean sqrt vs abs/max; tile_distance_squared unchanged cost) | verdict channel |

### Observation (`logs/tuned-42-fdaf4152`, verdict `concern`)
- **P1–P4 ✓**: determinism green; ZERO deaths run-wide; canaries
  grooming 3739 · play 150 · mentoring 1513 · courtship 14721;
  kittens_born ≥ 1. **P5 ✓**: GoalUnreachable counts *below*
  iteration 2 (93/43/37/24/22 vs 102/83/81/50/21).
- **Significant-band zeros needing explanation**: wards_placed 41→18,
  ward_siege 167→0, shadow_foxes_avoided_ward 2120→0, and zero
  ambush-lunges (19 in iteration 2).
- **Drill-down (WildlifePositions + drive-entry events)**: both
  step-8 shadowfoxes spawned co-located inside the southern
  corruption patch (~(27,67), first snapshot 1209000) and cycled
  Reconstituting↔Seeding there all run (226 entries; **zero**
  Haunting/Tending — no dread targets: the run's only southern cat
  excursion ended ~1k ticks before the first fox existed). Iteration
  2's second fox spawned at (116,20) on the colony flank — its patrol
  produced the 2120 ward-avoidances. Spawn placement is
  rejection-sampled against `distance_to(colony)`, so the metric
  pivot changed RNG consumption → different spawn sites → a
  discretely different fox story. The avoidance/siege code paths
  (WardCoverageMap threshold + drives radius) are metric-audited and
  intact; the ward-placement halving is demand-side (no fox pressure
  → less corruption near colony → fewer wards) — one causal chain,
  not four regressions.
- **Cross-seed structural check**: seed-43 soak on the same binary
  (`logs/tuned-43-fdaf4152`) to confirm the fox→ward channel fires
  under the Euclidean metric when spawn geometry cooperates.
- **P6 watch**: 116.9 tps (+4.4% vs post-506 baseline, −15% vs
  iteration 2's 137). Suspect trajectory (regular-fox activity 132→621
  position-snapshots; EngagePrey failure volume up) rather than metric
  arithmetic; the step-13 perf gate (⚑ flamegraph + re-baseline) owns
  the verdict.

### Concordance
Concordant with one drill-down. The hard gates and the pre-registered
drift shape (nearest-pick shifts, trajectory divergence) all landed as
predicted; the unpredicted part was the *severity* of one trajectory
branch — fox spawn sites moved, and with only 2 shadowfox spawns the
seed-42 fox story collapsed to "never met a cat." The cross-seed
structural check (`logs/tuned-43-fdaf4152`) confirms the channel:
**1334 ward-avoidances, 68 sieges, 1 ambush death, mythic-texture 6**
under the same binary. Lesson recorded: for subsystems driven by 1–3
long-lived actors (shadowfoxes), a single-seed zero is spawn-geometry
luck until a cross-seed run says otherwise — drill the actor's
position history before treating a channel zero as a code regression.
Seed-43 also flagged `ItemSourcedFromDenRaid` never-fired — cross-seed
expectation noise on a chain-rare event (the landing gate is seed-42,
which was clean); no action.

## Iteration 4 — step 9: fox/hawk/snake desire migration (2026-07-06)

### Changes
- Fox `step_toward` → `desire_toward`: cached **smoothed** corridor
  (string-pulled under the cat-patrol deterrent overlay — same overlay
  feeds the smoothing cost ceiling, so pruning never shortcuts through
  a patrol the router paid to avoid) + waypoint-pop seek at
  `fox_max_speed`.
- Hawk `step_flying` (signum tile-stepper) → `desire_flight`
  straight-line seek; hawks get `Flying` at the spawn author point
  (`on_wild_animal_added` — which now also authors
  `Velocity`/`DesiredVelocity` for ALL wildlife, since the species
  dispatchers require `DesiredVelocity` and a missing insert silently
  drops the animal from its own resolver query).
- Snake `step_slithering` → `desire_slither`: ticket-138 tick-skip
  `try_spend_step` gate retired — continuous 0.5 speed via the
  integrator cap, same average speed, no stutter.
- Integrator wires the previously-inert `hawk_max_accel` via the
  `Flying` branch (airborne movers turn harder; step-10 burst birds
  inherit).
- `wildlife_ai` (shadowfox legacy) untouched — step 11. It excludes
  the three GOAP species (`Without<FoxState/HawkState/SnakeState>`),
  so no double-move.

### Hypothesis (gate: hypothesize)
Same destinations, continuous motion. Hawk diagonal legs slow ~√2
(Euclidean cap vs king-move tile hops) and hawks pay an accel ramp;
snakes stop stuttering but average the same speed; foxes gain smoothed
corridors. Predictions: HawkDiveLanded / SnakeStruckPrey / snake-
inflicted injuries within ±10-ish% rate of the step-8 run where
trajectory permits; smoke tests green; survival + continuity gates
hold; zero fox/hawk/snake resolver-family timeout spikes (watchdogs
absorb the accel ramp; soar watchdog 200 ticks ≫ map-diagonal at 1.0).

### Observation — soak 1 (`logs/tuned-42-dc11ac39`): FOX EXTINCTION
Hard gates pass (zero cat deaths, canaries green), hawk/snake rates
in band (HawkDiveLanded 1048 vs 1273, SnakeStruckPrey 318 vs 415,
SnakeAmbushed up), but **both foxes dead by tick 1214400** (baseline:
alive past 1305000). FoxHuntedPrey 19→4, FoxAvoidedCat 5262→35;
Hunting replans on an exact 201-tick cadence (travel-timeout loop).
Position drill: both foxes fled to (0,5)/(0,6) — **Water tiles** (NW
lake, confirmed by seed-42 terrain probe) — and starved there.

Root cause pair:
1. **Entry** — the legacy `fox_movement` phase-mirror mover (a second
   Position writer that double-drove every GOAP travel step) has a
   `Fleeing` arm with an in-bounds check but NO terrain check;
   cat-injured foxes (`fox_ai_decision` hurt-flee heads for a map
   corner) marched straight into the lake.
2. **No self-rescue** — pre-140 `step_toward` teleported onto the
   first A* waypoint (always passable), silently rescuing stranded
   movers; the integrator correctly refuses to cross impassable
   terrain, so `passable()` failed on every sub-step INCLUDING
   within-tile moves — a mover standing on impassable terrain was
   frozen until starvation.

Fixes: `fox_movement` RETIRED (every moving fox phase is authored by
the GOAP dispatcher's phase mirror — juvenile dispersal included via
the Dispersing disposition — so the mover was pure double-drive plus
the lethal flee arm); integrator gains an **anti-strand hatch** (a
mover on an impassable tile accepts sub-steps unconditionally,
bounds-clamped, until it stands on legal ground — unit-tested both
directions: walks out, cannot walk back in). Note for step 11:
`wildlife_ai`'s shadowfox Fleeing arm has the same unchecked-terrain
shape but shadowfoxes remain direct writers until step 11, so they
can still walk out; close the hole when migrating.

### Observation — soak 2 (`logs/tuned-42-f7ddfeda`): PASS
- Foxes alive at every population sample (2/2 run-long, FoxDied 0,
  FoxHuntedPrey 22 vs 19 baseline — feeding restored); ZERO deaths
  run-wide; canaries green; never-fired clean; 123.8 tps.
- Same-tick (1229900) rate check vs step-8: HawkDiveLanded 1250 vs
  1273 (−1.8% ✓); SnakeAmbushed 291 vs 215; **SnakeStruckPrey 233 vs
  415 (−44%, out of the ±10% band)** — the snake's every-other-tick
  king-move hop averaged ~0.7 Euclidean-equivalent tiles/tick on
  diagonals; the honest 0.5 cap + accel ramp is slower inside the
  30-tick strike watchdog. This is the same verisimilitude-carried
  diagonal re-baseline as cats/hawks, and the burst-strike answer is
  step 12's sprint gait (`sprint_speed_mult` on pounce-class
  resolvers — snake Strike included). Accepted with that ownership;
  SnakeStruckPrey still fires 233× (no canary risk).
- Trajectory-divergence note: retiring `fox_movement` removed
  per-fox-per-tick RNG jitter draws, shifting the whole SimRng
  stream — shadow_fox_spawn 2→10 (ambush deaths still 0),
  FoxAvoidedCat 5262→1314, ward channels reshuffled
  (avoidance 262 via the drives flee arm, sieges 0 this trajectory;
  channel-alive proof stands on tuned-43-fdaf4152).

### Concordance
Concordant after one fix iteration, and the fix is the release's
thesis working as intended: the integrator enforcing passability
honestly EXPOSED a pre-existing lethal hole (terrain-unchecked flee
writes) that the old teleporting stepper had been silently papering
over. Two structural lessons: (1) when migrating a mover to
desire-based motion, grep for EVERY other Position writer on that
entity class first — the second writer is both a double-move and a
potential strand-injector; (2) "the old code self-rescued by
accident" is a failure mode of honest physics — pair every
passability-enforcement change with an explicit strand-escape story.
