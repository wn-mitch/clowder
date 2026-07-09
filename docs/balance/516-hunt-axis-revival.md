# 516 — hunt-axis revival (target-DSE scalar routing fix)

**Commit:** `afcc14e2` · **Gate:** four-artifact vs `logs/tuned-42-cf3d55ae`
(post-310 accepted / promoted baseline) · **Run:** `logs/tuned-42-afcc14e2`
(seed 42, Simba focal, 900s) · **Verdict:** concern-band, ACCEPTED — survival
PASS, continuity PASS, throughput PASS (−0.1%); drift channels examined below.
Plan step 25 landing 1 of the 266 arc (routing fix lands before prey AI so
both 266 gates measure live hunt axes).

## Mechanism

`score_target_consideration` routed unprefixed scalar names to a self-scoped
fetcher every production resolver stubbed to `0.0`. Since the 4c.7 port, hunt
target selection ranked on pursuit-cost + cooldown only — `prey_yield`,
`prey_calm`, `prey_alertness_tolerance` (≈ 0.56 of the WeightedSum) read 0.0
for every candidate. Fix: the `fetch_self` channel is deleted; all scalars
route through the target-scoped fetcher. Unit tests converted to
expected-winner-first tied-position (the dead-axis coincidence class is
extinct); fight's `ally_proximity` also revived (argmax-neutral — uniform
across candidates; `aggregated_score` unconsumed, noted in ticket log).

## Predictions → observations

Written before the soak (`scratchpad/516-predictions.md`).

- **P1 kill composition shifts toward yield ranking — PARTIAL, mechanism
  confirmed at the selection layer.** Rabbit kill share 17.8% → 26.2% (the
  clean yield-over-mouse signature; rabbit 0.8125 vs mouse 0.625 at equal
  distance). Rat: attempts 572 → 305 (−47%) with success 54.9% → 79.0%
  (+24 pts) — the calm axis dropping futile attempts on alert rats; rat kill
  *share* fell (42.1% → 36.9%) contra the naive share prediction because
  efficiency, not appetite, is what the axis changes. Mouse share flat
  (abundance-dominated).
- **P2 hunt success no-crash — CONCORDANT (band mis-anchored, recorded
  honestly).** Aggregate 62.27% → 61.37%. The prediction's [70, 90] band was
  anchored to step 13's 82.8%, a pre-310 denominator era; the intent (stable,
  no crash, Starvation == 0) holds exactly.
- **P3 hunt scoring shape change — CONFIRMED** (definitional; new axes carry
  ≈ 0.56 of weight; focal rankings differ).
- **P4 fight inert — CONCORDANT.** No fight-selection change possible
  (uniform axis); FoxConfrontation deaths 0 → 1 is family-noise (present in
  the step-21 gate family too).
- **P5 hard gates — PASS.** Starvation 0, ShadowFoxAmbush 1 ≤ 10, never-fired
  0, all four continuity canaries ≥ 1.
- **P6 KnowledgePromoted cadence — unchanged at 0 per ~108k-tick window** in
  both streams; consistent with step 24's prey-competition finding.
  Mechanism stays scenario-gated.

## Emergent finding → named watch-item for the band calibration

**Fish over-attraction via structural calm.** Fish never alert
(`Stationary`, `freeze_ticks == 0`, alertness never accumulates), so
`prey_calm` is 1.0 for every fish, compounding with the second-highest yield
(0.875). Result: fish attempts 205 → 337 (+64%) while fish success collapsed
42.9% → 16.9% (90% of losses `lost_during_approach`) — cats over-commit to
long fish approaches the 467-B vantage gate lets through. Candidate knobs
when the step-25 band calibration lands (after 266, per plan): an honest
fish-side wariness proxy (water-startle → alertness), a per-species calm
floor, or tightening the 467 vantage/reachability election gate. NOT tuned
here — the 266 prey-AI landings will move these numbers again; calibrate once
against the final substrate.

## Trajectory-drift disposition

Verdict drift channels (kittens +300%, fulfillment −42.6%) are
checkpoint-timing artifacts on the diverged trajectory; end-state footers:
kittens 5 → 6, fulfillment 0.198 → 0.265 (+34%), peak population 12 → 13,
colony_score aggregate 4195 → 4503. Colony end-state mildly improved —
consistent with the mechanism (fewer wasted hunts on alert rats → more food).
Ward-channel drift (sieges +27%, avoided-ward +37%) tracks the trajectory
divergence family, not a ward-mechanism change (no ward code touched).
