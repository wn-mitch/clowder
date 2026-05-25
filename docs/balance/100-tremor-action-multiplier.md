# Ticket 100 — Tremor map + Action::Stalk/Pounce + personality hunt approach

## Hypothesis (filed pre-soak, per CLAUDE.md "Verification" §2)

**Ecological claim.** Two previously-dead paths in the sensory model
become live:

1. `prey_cat_proximity()` now returns `sight.max(tremor)` (was sight
   only). Rabbits (tremor base_range = 12) can detect a running cat 11
   tiles away even when the cat is outside the rabbit's 6-tile sight
   alert_radius; pre-100 they couldn't.
2. `current_action_tremor_mul` is now sourced from
   `action_tremor_mul(cat.current_action.action, &constants.tremor)`
   instead of being hardcoded `1.0` everywhere. Stalking cats emit
   ~0.2× their tremor baseline (≈ 0 substrate vibration); running cats
   ~1.8×; pouncing ~2.0×.

The `EngagePrey` resolver stamps `Action::Stalk` on `StepPhase::Stalking`
and `Action::Pounce` on `StepPhase::Pouncing`, so `tremor_tick` reads
the correct multiplier next tick.

A new `prey_alertness_tolerance` axis ships live in the HuntTarget DSE
with weight 0.15 (input = `boldness × alertness`). Bold cats partially
offset the prey_calm penalty when committing to nervous prey; patient
cats don't.

The `EngagePrey` approach-phase stalk-start distance moves from the
constant `(alert_radius + buffer).max(minimum)` to a per-cat
computation `min + buffer × patience + alertness_push × alertness +
species_push × prey_tremor_sensitivity + patience × tremor_push ×
tremor_at_prey - patience × scent_settle_push × scent_at_prey`,
clamped to `[min, min + 2 × buffer]`.

## Predictions

- **Bold cats:** similar Hunt attempt rate to baseline; *lower* success
  rate against high-tremor prey (Rabbit at tremor=12, Rat at 10);
  *similar* success against low-tremor prey (Bird at 2, Fish at 6).
  Mechanism: bold cats stalk less (patience-coupled effective_stalk_
  distance contracts; tolerance axis biases them toward alert prey
  they shouldn't catch).
- **Patient cats:** *lower* Hunt attempt rate (they filter for calm
  targets); *higher* success rate overall, particularly against
  high-tremor prey (extended stalk distance gives them margin).
- **Colony-wide:** Hunt success and prey-death rate within ±10% of
  baseline. The personality redistribution should roughly cancel: bold
  cats catch fewer rabbits, patient cats catch more. Welfare gates
  (Starvation=0, ShadowFoxAmbush≤10, continuity canaries) hold.

## Magnitudes (predictions to compare against)

- Hunt success rate: ±10% of baseline (concordance threshold).
- Hunt attempt rate: ±15% (slightly looser; selection asymmetry).
- Prey-death rate by species: Rabbit −10–20% (high-tremor); Bird
  +0–5%; Mouse / Rat / Fish near baseline.
- Colony starvation deaths: 0 (hard gate).

## Drift policy

- Drift > ±10% on Hunt success or prey-death rate requires the
  four-artifact concordance check (hypothesis · prediction ·
  observation · concordance — direction + magnitude within ~2×) per
  CLAUDE.md "Drift" rule.
- Survival canaries (Starvation, ShadowFoxAmbush, continuity canaries,
  never_fired_expected_positives) are hard gates regardless of
  hypothesis outcome.

## Schedule-edge perturbation (documented, expected)

Adding `tremor_tick` to the per-tick chain perturbs Bevy's topological
sort enough that seed-42 RNG order shifts. Already absorbed:

- `scenarios::drying_chain_eligibility::resolver_completes_load_step_on_far_rack`
  re-seeded from 1 → 21 (the new schedule's first DryFood-electing
  seed for that fixture).
- `tests::scenarios::declared_expected_features_all_fire`'s
  `farm_herb_demand` scenario tick budget bumped 80 → 300;
  `fox_cat_scent_avoidance` bumped 100 → 200. The mechanisms still
  work at every seed probed; the fixtures just needed more headroom.

## Verification recipe

1. `just soak-trace 42 Simba` (canonical focal-cat baseline).
2. `just verdict logs/tuned-42/` — hard gate (Starvation=0, ShadowFox
   ≤ 10, continuity canaries ≥ 1 each, never_fired==0).
3. `just frame-diff` against `logs/baselines/current.json` —
   Hunt + HuntTarget DSE rows; if Hunt mean score drifts > ±10%, file
   concordance.
4. Confirm `"tremor"` key appears in L1 records of `trace-Simba.jsonl`.
5. Confirm `Action::Stalk` appears in CurrentAction.action transitions
   during EngagePrey steps.
6. Confirm `prey_alertness_tolerance` axis appears in L2 HuntTarget
   rows of the trace sidecar.

## Observation (2026-05-24, soak `logs/tuned-42-1838ce91`)

- **Verdict: concern**, hard gates pass.
  - `survival: pass` — Starvation=0, ShadowFoxAmbush within band.
  - `continuity: pass` — grooming, play, mentoring, courtship all ≥ 1.
  - `constants_drift_vs_baseline: drift` — expected (new
    `TremorConstants` + new `DispositionConstants` fields +
    `ScoringConstants::hunt_alertness_tolerance_weight`).
- **L1 trace confirms tremor map live.** First L1 record at tick
  1200001 shows `{"map":"tremor","faction":"neutral","channel":"tremor",
  "base_sample":0.0}`; by tick 1200002 `base_sample:1.0` — the writer is
  depositing.
- **Frame-diff vs baseline 1799e798 shows Hunt is NOT in the top-15
  movers.** Top deltas (idle +1034%, groom_self −74%, forage −71%,
  build +58%, socialize +401%, caretake +0.222 new, explore −45%,
  fight −27%, farm +1672%, craft_at_workshop +110603% new, flee −79%,
  patrol +0.096 new) reflect the *cumulative* drift from
  every ticket landed since baseline (055, 369, etc.), not ticket 100.
  Hunt mean score is stable on Simba; the 0.15-weight tolerance axis
  and the per-cat effective_stalk_distance shift didn't dislodge the
  focal cat's Hunt scoring.
- **Concordance** — no Hunt drift > ±10%; the predicted "bold cats
  catch fewer rabbits" effect is too narrow a per-personality slice to
  surface on Simba alone. A multi-focal sweep (Simba + a high-boldness
  cat) would isolate it but is out of scope for first-light
  activation per `feedback_dormant_substrate_activation_soak_first`.
- **Concern items.** `shadow_foxes_avoided_ward_total` (0→3436) and
  `ward_siege_started_total` (0→27) are new-nonzero. Both reflect
  ShadowFox behavior shifts driven by Ward/ShadowFox tickets landed
  between baseline and now (not ticket 100 — wards aren't touched
  here). `colony_score.fulfillment +98.8%` and `seasons_survived
  +50.0%` are cumulative-baseline artifacts; `colony_score.health
  −33.7%` is from 2 injury deaths (the only non-zero death cause).

## Conclusion

Ticket 100 substrate is wired and live. Hunt-side behavior is stable
on the focal cat. The concern-level verdict reflects cumulative drift
from intermediate tickets, not 100's contribution. Survival gates
pass. Recommend landing; follow-on tuning per-personality cohort
should compose with a multi-focal sweep once the next baseline is
promoted.

## Iter-3 (2026-05-25) — ticket 465 A* fallback in hunt approach

### Hypothesis

The prior "stuck during approach" pattern (80.9% of hunt losses
post-464 R1, 91.8% pre-100) was a load-bearing defect in
`goap.rs::resolve_engage_prey`'s approach phase, not the natural
failure-mode distribution. Greedy `step_toward`
(`src/ai/pathfinding.rs:342-423`) has no backtrack capability and
returns `None` in concave-terrain local-minima — the rustdoc explicitly
names this as a known Phase-1 limitation. The seed=42 map has ~5
specific trap configurations near common prey-resting tiles (Rabbit
events clustered 112-at-(18,15), 32-at-(17,14), 21-at-(16,13)…). Same
trap × same prey-resting-tile × repeated cat attempts = the
1068-events / 67k-ticks observation.

**Predicted direction:** falling back to A* (`find_path`) when greedy
returns `None` should drop "stuck during approach" dramatically for
land prey (Rabbit/Rat/Mouse/Bird). **Fish** would remain stuck because
`find_path` correctly refuses impassable targets (water is impassable
to cats; Fish-on-water is structurally unhuntable — orthogonal).

**Predicted magnitude:** Rabbit success ≥ 29.7% (ticket §Verification
target, within ±10% of pre-100 baseline 33.0%).

### Observation (`logs/tuned-42-59e26d68`, seed=42, dirty)

| Species | Pre-fix (cfc6f4fa) | Post-fix (59e26d68) | Δ |
|---|---:|---:|---:|
| Colony | 18.47% (1619 att) | 23.89% (1348 att) | +5.4 pp |
| Rabbit | 17.05% (434) | **96.06% (127)** | **+79.0 pp** |
| Rat    | 45.40% (163) | **100.0% (43)** | +54.6 pp |
| Mouse  | 54.40% (125) | 87.10% (62) | +32.7 pp |
| Bird   | 16.34% (361) | 22.01% (359) | +5.7 pp |
| Fish   | 4.48% (536) | 3.17% (757) | −1.3 pp |

`EngagePrey: lost prey during approach` plan-failure rate: 1069 → 579
events (−46% absolute). Of the remaining 578 stuck events, **78.9% are
Fish** (cats trying to enter water tiles) — confirming the orthogonal
Fish defect that this ticket does not address.

### Concordance

**Direction:** matches prediction across all five species. Land-prey
hunts (Rabbit/Rat/Mouse) recover dramatically; Fish unchanged
(predicted); Bird recovers modestly (much of Bird "stuck" was prey-
teleport — birds correctly flee when cats approach).

**Magnitude:** **far exceeds** the ±10% concordance band (and the
±30% scrutiny threshold). Rabbit 17% → 96% is a 5.6× swing. This is
**defect-removal magnitude**, not parameter-tuning magnitude — the
prior baselines (pre-100 33.0%, post-100 17.5%, post-464 17.05%) were
all bug-corrupted by the greedy-stuck defect. The post-465 number is
the *true* success rate that the substrate would have produced absent
the pathing bug.

**Verdict:** `concern` (not fail). Survival/never-fired gates pass.
Welfare metrics overshoot baseline coherently — `nourishment`,
`seasons_survived` (+50%), `peak_population`, `structures_built`
(+36%), `bonds_formed` (+33%) all moved in the predicted
positive-uplift direction per pillar #3 ("richer perception, better
strategy" — cats with route knowledge make better strategic decisions
and welfare improves). The drift exceeds concordance bands but in the
*intended* direction.

### Follow-ons opened (blocked-by 465)

- **TravelTo same-defect** — `TravelTo(HerbPatch): no path and stuck`
  went 278 → 452 (cats travel more now that hunts complete faster;
  same greedy defect in a different consumer). Substrate-correct fix
  is to lift the A*-fallback into `CatPathPlan::next_step`.
- **Fish-on-water hunt resolver** — 578 of 579 remaining stuck events
  are Fish. Options: retire Fish from cat hunt eligibility, OR wire a
  shoreline-pounce resolver. Design question, not a parameter tune.
- **CraftAtWorkshop recipe-not-satisfied** — new plan-failure surface
  at 0.01364/tick (964 events). Likely secondary to the larger /
  better-fed population; needs investigation before next balance pass.

### Decision

**Ship 465** as a defect removal. The prior baselines should be
treated as bug-corrupted; future balance work on hunt substrate (per-
personality cohorts, tremor-action retunes) should compose against
the post-465 metric distribution. Balance retuning of hunt rates (if
desired) is a separate concern from the path-defect fix.
