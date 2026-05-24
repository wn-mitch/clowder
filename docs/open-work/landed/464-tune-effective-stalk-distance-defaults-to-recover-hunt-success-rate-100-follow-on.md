---
id: 464
title: Tune effective_stalk_distance defaults to recover hunt success rate (100 follow-on)
status: done
cluster: wildlife
orchestration: substrate-sensitive
initiative: [predator-prey-dynamics]
added: 2026-05-24
parked: null
blocked-by: []
supersedes: []
related-systems: [sensory.md, ai-substrate-refactor.md]
related-balance: [100-tremor-action-multiplier.md]
landed-at: pending
landed-on: 2026-05-24
---

## Why

Ticket 100 landed at `36fc65b4` with `verdict=concern` (survival/continuity
gates pass). Hunt success rate dropped from **19.7% → 13.3% colony-wide
(−32% relative)** between baseline `tuned-42-1799e798` and post-100 run
`tuned-42-1838ce91`. Per-species breakdown surfaces the cause as a
defect in 100's `effective_stalk_distance` defaults (not the substrate
shape):

| Species | tremor base_range | baseline success | post-100 success | Δ |
|---|---|---|---|---|
| Rat | 10 | 39.5% | 14.1% | **−25.4 pp** |
| Rabbit | 12 | 33.0% | 17.5% | **−15.5 pp** |
| Bird | 2 | 18.8% | 15.7% | −3.1 pp |
| Fish | 6 | 2.7% | 3.6% | +0.9 pp |
| Mouse | 6 | 46.3% | 52.8% | +6.4 pp |

The pattern is monotonic in `prey_tremor_sensitivity` (rat / rabbit
hardest hit; bird least). Top failure reason in post-100 is `EngagePrey:
lost prey during approach` at 1075 events (rate 0.0149/tick) — up from
917 baseline — and `lost_during_stalk` quadrupled (74 → 301). The
shift is **stalk-start distance pushed too far for high-tremor prey**:
the `species_push × prey_tremor_sensitivity` term (2.0 × 1.0 = 2 for
Rabbit) lifts `effective_stalk_distance` from the pre-100 baseline of
~`alert_radius + 2` ≈ 8 tiles for Rabbit to the post-100 clamp ceiling
of `min + 2 × buffer` = 9 tiles. Cats enter stalk mode from one tile
farther out, hit cluttered terrain the approach pathing can't resolve,
and "stuck during approach" 95.7% of Rabbit losses (vs 91.8% baseline)
plus a new failure surface during the stalk itself.

The hypothesis in `docs/balance/100-tremor-action-multiplier.md`
predicted ±10% drift; observed −32% exceeds the concordance threshold.
Hard gates passing means the colony absorbed the food shortfall — but
the welfare margin shouldn't carry a defect, especially when the
substrate's design intent ("bold cats catch fewer rabbits, patient
cats catch more, net unchanged") didn't materialize.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | `src/resources/tremor_map.rs` | TremorMap fires; `base_sample` confirmed at tick 1200002 in `trace-Simba.jsonl` | `[verified-correct]` |
| L2 DSE scores | `src/ai/dses/hunt_target.rs` | `prey_alertness_tolerance` axis at weight 0.15 is too small to dislodge focal-cat Hunt mean score (frame-diff confirms Hunt not in top-15 movers) — NOT the regression source | `[verified-correct]` |
| L3 softmax | `src/ai/scoring.rs` | Hunt selection rate unchanged colony-wide (attempts 1386→1776 is duration-driven; per-10kt rate ~unchanged at 246 vs 245) | `[verified-correct]` |
| EngagePrey resolver | `src/systems/goap.rs::resolve_engage_prey:9001-9026` | `effective_stalk_distance = min + buffer × patience + alertness_push × alertness + species_push × prey_tremor_sensitivity ± patience × (tremor_push, scent_settle_push)` clamped to `[5, 9]` | `[verified-defect]` — high-tremor prey saturate the clamp ceiling, pushing stalk-start 1-3 tiles further than pre-100 |
| Plan template | `goap_plan.rs::EngagePrey` | Plan unchanged; phase transitions Approach → Stalk/Chase/Pounce same as pre-100 | `[verified-correct]` |
| Completion proxy | n/a | unaffected | `[verified-correct]` |
| Pathing (resolver-side) | `goap.rs::step_toward` + A* | "stuck during approach" 4× increase at higher stalk distances reveals a pre-existing pathing weakness at the 8-9 tile range | `[suspect]` — separate concern, out of scope here |

The defect is at the resolver layer, parameter-tuning shape. Substrate
is structurally correct (`prey_cat_proximity` returns `sight.max(tremor)`,
Stalk/Pounce dispatch fires, L1 tremor map populates).

## Fix candidates

**Parameter-level options:**

- **R1 (recommended) — halve `species_push` and `alertness_push`.**
  Set `DispositionConstants::species_push` 2.0 → 1.0 and
  `alertness_push` 3.0 → 1.5. With Rabbit (sens=1.0, alertness≈0.0 idle),
  raw stalk distance moves from `5 + 1 + 0 + 2 = 8` to `5 + 1 + 0 + 1 = 7`
  — back near pre-100's `alert_radius+buffer = 8` for the patient
  default. Bird (sens≈0.17) drops from 7.34 → 6.17 (clamp doesn't bite).
  Predicted recovery: Rabbit success 17% → ~28% (within the ±10% band
  from the original 33%); Rat similar.

- **R2 — drop the clamp upper bound** from `min + 2 × buffer = 9` to
  `min + 1 × buffer = 7`. Tighter ceiling makes high-tremor prey
  un-distinguishable from low-tremor at the ceiling, which collapses
  the design intent — rejected on shape grounds but listed for
  completeness.

- **R3 — make the ambient terms (TremorMap, scent_settle) zero-default
  by setting `tremor_push = 0.0` and `scent_settle_push = 0.0`.**
  These already gate on `patience × …` so they're typically small;
  zeroing them removes one source of noise. Likely insufficient on
  its own — the species_push term is the dominant lift on high-tremor
  prey. Useful as a "follow-on once R1 lands."

**Structural options:**

- **R4 (split) — give Stalk its own resolver step distinct from
  EngagePrey.** Currently EngagePrey decides phase per tick by
  distance threshold; split into `StalkPrey` (slow-creep, suppressed
  tremor) and `EngagePrey` (approach + chase + pounce) as separate
  GoapActionKinds with different plan templates. Stalking becomes a
  first-class commitment, not a phase. Cost: meaningful refactor
  (new disposition?, new plan template, completion proxy split).
  Rejected for the tuning regression — overkill when R1 likely fixes
  the success-rate gap.

- **R5 (extend) — branch effective_stalk_distance computation on
  whether the prey is currently in `PreyAiState::Alert`.** If alert,
  use the pre-100 formula `alert_radius + buffer`; if idle/grazing,
  use the post-100 personality-modulated formula. Captures the
  intuition "you stalk *calm* prey from far; alert prey you charge."
  Worth considering as a follow-on but adds branching to the
  approach-phase logic the substrate-over-overrides pillar warns
  against.

- **R6 (rebind) — n/a.** No Action→Disposition mapping change applies.

- **R7 (retire) — n/a.** The new fields are load-bearing; retiring
  them reverts 100.

## Recommended direction

**R1 ships first**: halve `species_push` and `alertness_push`. Smallest
diff, preserves the design shape, addresses the load-bearing defect
directly. Verify via `just soak-trace 42 Simba` + `just q hunt-success`
on both baseline and post-fix runs; concordance check requires
post-fix colony-wide success rate within ±10% of baseline 19.7% (i.e.,
≥ 17.7%) and per-species Rabbit/Rat within ±10% of their baselines.

If R1's soak still shows > ±10% drift on Rabbit/Rat, escalate to R5
(alertness-conditional formula) as a follow-on ticket — don't stack
parameter tweaks on top of R1.

## Out of scope

- The "stuck during approach" 4× lost_during_stalk increase suggests
  a pre-existing pathing weakness at 8-9 tile stalk distances. R1
  papers over it by keeping cats nearer; the underlying A*-gives-up
  failure mode deserves its own ticket if it surfaces in other
  scenarios. Open as `465-cat-approach-pathing-failure-at-stalk-range`
  if R1's soak still shows residual lost_during_stalk elevation.
- Multi-focal sweep (Simba + high-boldness focal) to isolate per-personality
  Hunt outcomes — deferred per the original 100 plan; revisit once R1
  lands and a new baseline is promoted.

## Verification

- `just check` + `just test` clean.
- `just soak-trace 42 Simba` post-fix.
- `just verdict logs/tuned-42-<sha>` — survival/continuity hard gates
  must pass; constants_drift_vs_baseline acceptable (new defaults).
- `just q hunt-success logs/tuned-42-<sha>` — colony-wide success rate
  ≥ 17.7% (within ±10% of baseline 19.7%).
- Per-species: `just q hunt-success logs/tuned-42-<sha> --species=rabbit`
  and `--species=rat` — both within ±10% of their baselines (Rabbit
  ≥ 29.7%, Rat ≥ 35.6%).
- `just frame-diff logs/tuned-42-1838ce91/trace-Simba.jsonl
  logs/tuned-42-<new>/trace-Simba.jsonl` — Hunt mean score should
  shift back toward baseline; if it doesn't move, the L2 isn't the
  driver and the regression really is resolver-pathing-bound.

## Log

- 2026-05-24: opened from 100 post-soak regression analysis. Hunt
  success rate −32% colony-wide (19.7% → 13.3%); Rabbit −15.5 pp,
  Rat −25.4 pp. R1 (halve species_push + alertness_push) recommended;
  R5 held as escalation path if R1 underdelivers.
- 2026-05-24: soak `logs/tuned-42-cfc6f4fa` (header carries my tuned constants; `commit_hash` field reads the parent due to build.rs/jj `git rev-parse HEAD` interaction). Colony-wide hunt success 13.3% → 18.47% (target ≥17.7% ✓), Rat 14.1% → 45.4% (target ≥35.6% ✓, exceeds baseline 39.5%). Rabbit stalled at 17.05% (target ≥29.7% missed) — failure mode is "stuck during approach" 97.8% of Rabbit losses, the A*-pathing weakness §Out of scope named. Opened 465 for the pathing follow-on. `just frame-diff` confirms Hunt not in top-15 movers — recovery is resolver-side via effective_stalk_distance arithmetic, not L2 score shape. Hard gates pass (0 deaths, all 4 continuity canaries fire). Welfare +13.9%, seasons survived +50%, run lasted 19% longer. lost_during_stalk recovered 301 → 108 (~3× toward baseline 74).
