# Respect restoration — iterations 1 & 2

**Status:** iter 1 landed alongside `mastery-restoration.md`
iteration 2, `acceptance-restoration.md` iteration 2 (deferred),
and `purpose-restoration.md` iteration 1. Iter 2 (magnitude
correction) landed 2026-04-25.

---

## Iteration 1 — witness-multiplier on chain completion

### Context

The seed-42 v2 deep-soak (`logs/tuned-42-v2/`) showed colony-mean
respect at **0.287 mean / 67.1% zero**. This is the third upper-tier
need (alongside acceptance and purpose) that flatlines despite the
existing `respect_for_disposition` chain-completion grants
(`src/systems/disposition.rs:1461`):

| disposition | constant | typical magnitude |
|---|---|---|
| Hunting / Foraging / Building / Coordinating / Guarding / Socializing | `respect_gain_*` | 0.005–0.05 per chain completion |

These hooks fire on every successful chain completion, but the
magnitudes are sized against per-chain cadence — a cat that completes
3–4 chains per sim-day at 0.02 each gains ~0.06–0.08 respect/day,
against a `respect_base_drain` that exceeds that on most days. The
result is the steady-state mean of 0.287 — alive, but a third of cats
sit at zero at any given snapshot.

### Hypothesis

> Respect models the **felt sense of being seen** — the social
> visibility component of esteem. The existing chain-completion
> grants treat respect as a private reward (the cat that did the
> work earns it regardless of whether anyone witnessed). But esteem-
> tier needs are about *social standing*, not solitary accomplishment.
> A respect bump that scales with the number of nearby witnesses at
> the moment of completion ties the need to its actual social
> mechanism: cats earn respect from being seen completing tasks, not
> from completing them in a vacuum.

This is symmetric with the existing `acceptance` design — acceptance
is the receiver side of care, respect is the visible side of
accomplishment. Both are esteem-cluster needs that depend on other
cats being present.

### Prediction

| Metric | Direction | Rough magnitude |
|---|---|---|
| Colony-averaged respect | ↑ from 0.287 | 0.5–0.7 band |
| Respect `=0%` over snapshots | ↓ from 67.1% | < 20% |
| Welfare composite | ↑ slightly | esteem term recovers |
| Survival canaries | unchanged | hard gates |

The expected mechanism: a cat completing a chain at the hearth (with
3–4 other cats nearby) earns `0.005 × 3 = 0.015` on top of the
disposition baseline — comparable to the baseline itself, doubling
the per-completion magnitude for socially-visible work. Cats working
in isolation (out hunting, surveying remote tiles) get only the
baseline, preserving the asymmetry between visible and private
accomplishment.

### What landed

- New constants in `DispositionConstants`:
  - `respect_per_witness = 0.005` — added per nearby cat.
  - `respect_witness_radius = 5` — Manhattan tiles.
  - `respect_witness_cap = 4` — diminishing returns above 4 witnesses.
- New helper in `src/systems/disposition.rs`:
  `count_witnesses_within_radius(actor, actor_pos, positions, radius, cap)`
  (now `pub`). Pure function; unit-tested (`respect_witness_tests` module).
- Witness-multiplier applied at the plan-completion site in
  `resolve_goap_plans` (`src/systems/goap.rs:~1812`), alongside the
  existing `respect_for_disposition` baseline. Uses
  `snaps.cat_positions`, which was already being built for other
  purposes — no new snapshot field required.

#### Relocation note (2026-04-24 post-landing fix)

The original iter-1 landing wrote the witness-multiplier into
`resolve_disposition_chains` (both chain-completion arms). That
function is registered only in test schedules (`tests/integration.rs`,
`tests/mentor.rs`); production schedules (`src/plugins/simulation.rs`,
`src/main.rs`) register `resolve_task_chains` instead, which does not
read `&mut Needs`. The iter-1 writes never ran in soaks — batch 2
(seed 42, 900s) showed respect mean = 0.291, essentially unchanged
from the pre-iter-1 baseline of 0.287. Diagnosis in
`docs/open-work/landed/2026-04.md` under the relocation entry.

**Fix**: the witness-multiplier moved to `resolve_goap_plans`'s
plan-completion block (`src/systems/goap.rs:~1812`), right next to
the existing `respect_for_disposition` baseline write, so both fire
from the same live schedule. The code blocks that used to live in
`resolve_disposition_chains` are now replaced by pointer comments;
the helper function and its unit tests remain in
`src/systems/disposition.rs` unchanged. Hypothesis and prediction
bands unchanged — measurement re-runs against the relocated code.

### Observation

Post-relocation seed-42 15-min soak (`logs/tuned-42-iter2/`, commit
`da51270`):

| Metric | Baseline (pre-iter-1) | Iter-1 observed (0.005) | Predicted band |
|---|---|---|---|
| Respect mean | 0.287 | **0.998** | 0.5–0.7 |
| Respect =0% | 67.1% | **0.0%** | < 20% |

Cross-seed confirmation on the Thistle seed
(`logs/tuned-18301685438630318625-iter2/`): mean ≈ 0.997, =0% ≈ 0%.
Pinned at the `.min(1.0)` saturation ceiling on both seeds — no
seed-conditional dynamics differentiate runs. Survival canaries hold
(starvation = 0, ambush = 0).

### Concordance

**Direction:** match (mean ↑, zero% ↓ in the predicted directions).
**Magnitude:** **reject — 3.5× off** at the band centre. Saturation at
the `.min(1.0)` ceiling is the failure mode: the per-completion gain at
0.005 × ~3 witnesses (≈ 0.015) exceeds the drain accumulated between
completions, so respect pegs at 1.0 and the system loses the dynamics
needed to sit mid-band.

Per CLAUDE.md balance methodology: direction-match-with-magnitude-3.5×-
off requires an iter-2 magnitude correction. See **Iteration 2** below.

## Iteration 2 — magnitude correction

### Hypothesis

> The iter-1 saturation is a magnitude-only failure: per-witness gain
> too large relative to drain. Reducing `respect_per_witness`
> sufficiently — to where per-completion gain at the typical witness
> count falls below the drain accumulated between completions — should
> drop equilibrium below `.min(1.0)` and let the system settle into the
> 0.5–0.7 prediction band.

This keeps the iter-1 *mechanism* (witness multiplier on plan
completion) intact. Only the magnitude is in scope; relocation site,
`respect_witness_radius = 5`, `respect_witness_cap = 4`, and the
`count_witnesses_within_radius` helper are out of scope.

### Prediction

Same targets as iter 1: respect mean in 0.5–0.7, =0% < 20%, survival
canaries unchanged. Initial guess: 3.3× cut (0.005 → 0.0015), based on
the 3.5× magnitude rejection.

### What landed

`default_respect_per_witness`: **0.005 → 0.0001** (50× cut).

Bisection through the saturation cliff was steeper than predicted. The
iter-1 magnitude rejection (3.5×) suggested a 3.3× cut would suffice,
but the response curve near `.min(1.0)` is highly nonlinear because the
disposition baselines (`respect_gain_*` = 0.01–0.15 per chain) plus any
nonzero witness contribution stay above drain — the ceiling clips the
gain rate, so cuts < 16× look identical at the colony-mean level. Three
batches:

| Batch | `respect_per_witness` | Mean | sd | =0% |
|---|---|---|---|---|
| Iter 1 (saturated) | 0.005 | 0.998 | — | 0% |
| 2-batch1 | 0.0015 | 0.996 | 0.032 | 0% |
| 2-batch2 | 0.0003 | 0.982 | 0.060 | 0% |
| **2-batch3 (landed)** | **0.0001** | **0.566** | **0.463** | **18.3%** |

The cliff edge sits between 0.0003 and 0.0001; under it, equilibrium
falls below saturation and the colony spreads naturally.

### Observation

Seed-42 15-min soak at `respect_per_witness = 0.0001`
(`logs/tuned-42-iter2-batch3/`):

| Metric | Iter-1 | Iter-2 (landed) | Predicted band |
|---|---|---|---|
| Respect mean | 0.998 | **0.566** | 0.5–0.7 ✓ |
| Respect =0% | 0.0% | **18.3%** | < 20% ✓ |
| Respect sd | — | **0.463** | wide ✓ |
| Survival canaries | hold | hold | required ✓ |

The colony-mean lands at 0.566 — squarely in band. zero% (18.3%) sits
just under the 20% target.

**Per-cat distribution is bimodal-by-role**, which matches iter-1's
design intent (*"Cats working in isolation … get only the baseline,
preserving the asymmetry between visible and private accomplishment"*):

| Cat | Action register | Mean respect | =0% |
|---|---|---|---|
| Birch / Mocha / Nettle / Ivy | colony-centre / herbcraft | ~0.98 | 0% |
| Calcifer / Lark | mid-isolated | 0.17–0.19 | 35–38% |
| Mallow / Simba | hunter-coded, solo | 0.08–0.12 | 30–45% |

Social-centre cats still saturate at 1.0, so the asymmetry is real but
the social-side ceiling is sharp. A future iteration could soften
`.min(1.0)` (logistic-style growth, or proportional-to-`(1 - respect)`
scaling) to compress the bimodal toward a tighter mid-band cluster
without losing the role asymmetry.

### Concordance

**Direction:** match (mean from 0.998 → 0.566, =0% from 0% → 18.3%, both
toward the band).
**Magnitude:** mean lands inside the strict 0.5–0.7 band. =0% under 20%
target. **Accept.**

The handoff's predicted 3.3× cut would have shipped a still-saturated
soak — the response near `.min(1.0)` is much steeper than the iter-1
magnitude-rejection ratio implied. 50× cut was needed; the cliff lives
between 0.0003 and 0.0001.

### Survival canaries (seed-42)

- Starvation = 0 ✓
- ShadowFoxAmbush = 0 ✓
- Footer written ✓
- never-fired-expected positives = 10 (unchanged from iter-1; pre-
  existing, deferred per `docs/balance/acceptance-restoration.md` iter-2
  deferral)
- Continuity tallies: grooming = 33 (>0); play, mentoring, burial,
  courtship, mythic-texture all 0 (pre-existing, deferred — same
  passive-`social`-saturation root cause)

### Deferred / iter 3 candidates

- **Bimodal compression.** Saturation at 1.0 for colony-centre cats is
  cosmetically unsatisfying even though it matches the role asymmetry.
  An iter-3 candidate is replacing the additive-with-clamp accumulation
  with a logistic update (`respect += (1 - respect) × witnesses ×
  per_witness`), which gives smooth saturation rather than a hard
  ceiling. Would require structural code change in `goap.rs:~1825` and
  `disposition.rs:~2682,2797` baseline writes.
- **Cross-seed concordance.** Thistle-seed (18301685438630318625) re-run
  at 0.0001 deferred — only seed-42 measured for this iter. Symmetry
  check on a follow-up if the iter-2 mean shows seed-conditional drift.

## Related work

- `docs/balance/acceptance-restoration.md` — sibling esteem-cluster
  receiver-side need.
- `docs/balance/mastery-restoration.md` — sibling esteem-cluster
  competence-felt need.
- `docs/balance/purpose-restoration.md` — sibling self-actualization
  need.
- `docs/systems/colony_score.rs` — esteem-tier suppression cascade
  that amplifies pinned-at-0 needs into welfare drag.
