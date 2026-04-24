# Respect restoration — iteration 1 (witness-multiplier on chain completion)

**Status:** landed alongside `mastery-restoration.md` iteration 2,
`acceptance-restoration.md` iteration 2 (deferred), and
`purpose-restoration.md` iteration 1.

## Context

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

## Hypothesis

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

## Prediction

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

## What landed

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

### Relocation note (2026-04-24 post-landing fix)

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

## Observation

Pending — to be filled in after the post-commit seed-42 deep-soak.

## Concordance

Pending. Document direction match + magnitude band per CLAUDE.md
balance methodology.

## Related work

- `docs/balance/acceptance-restoration.md` — sibling esteem-cluster
  receiver-side need.
- `docs/balance/mastery-restoration.md` — sibling esteem-cluster
  competence-felt need.
- `docs/balance/purpose-restoration.md` — sibling self-actualization
  need.
- `docs/systems/colony_score.rs` — esteem-tier suppression cascade
  that amplifies pinned-at-0 needs into welfare drag.
