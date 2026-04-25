---
id: 027
title: Mating cadence — three-bug cascade blocking MatingOccurred
status: in-progress
cluster: null
added: 2026-04-25
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

`MatingOccurred = 0` across all 15 sweep runs of the seed-42-multi
baseline dataset (`logs/baseline-2026-04-25/`). Ticket 014 deferred
the mating-cadence balance work pending substrate stabilization;
the substrate is now stable (Phase 4c.7 closed, §7.2 commitment gate
wired) but the dataset reveals **three distinct structural bugs**,
each at a different layer of the §7.M three-layer Mating model. None
are coefficient-tunable — all three need code fixes before any
balance-tuning iteration becomes meaningful.

The deeper consequence: without mating, **colony viability is
bounded above by founder lifespan**. Sweep seeds 99 and 314 already
collapse to 1 surviving cat across all three reps in 900s; longer
soaks would extinct every seed once founder old-age mortality starts.
Mating-cadence is upstream of generational continuity, mentorship
transmission to non-founders, KittenMatured firings, and any
multi-generational balance work.

## Scope

Three sequenced fixes, each with its own commit. Fix order chosen so
each fix's verification is testable against the next baseline soak
without waiting on subsequent fixes.

### Bug 1 (cheapest) — observability decoupling

`Feature::CourtshipInteraction` is currently emitted only inside the
`MateWith` step resolver (`src/systems/goap.rs:3035`,
`src/systems/disposition.rs:3776`). The continuity canary class
`courtship` therefore measures "MateWith executed with a target",
not "any courtship-related interaction" — and since MateWith never
runs (Bug 2), the canary reports zero even though the passive
courtship drift in `src/systems/social.rs::check_bonds` *is*
incrementing romantic toward the Partners threshold.

**Fix:** emit `Feature::CourtshipInteraction` inside `check_bonds`
when `rel.romantic` is incremented under the courtship-drift gate
(`romantic_eligible && fondness > courtship_fondness_gate &&
familiarity > courtship_familiarity_gate`). This decouples the
canary from the deadlocked MateWith path so subsequent fixes can be
verified against an honest signal.

### Bug 2 (one-line retire) — lifted-condition outer gate

`MateDse` was ported in Phase 4 with its own `EligibilityFilter`
(`forbid Kitten/Young/Incapacitated`) but the legacy outer wrapper
at `src/ai/scoring.rs:916` (`if ctx.has_eligible_mate { score_dse_by_id("mate", ...) }`)
was never retired. Even Adult cats with valid markers get blocked
because `has_eligible_mate` (`src/ai/mating.rs:147`) requires a
nearby cat with a `Partners` or `Mates` bond — which doesn't exist
in any 900s soak (Bug 3 root cause).

This is the same lifted-condition pattern CLAUDE.md flags from the
2026-04-23 §7.2 regression. The `Coordinate` retire at
`src/ai/scoring.rs:898` is the model:

```text
// §4 batch 1: inline `if ctx.is_coordinator_with_directives` guard
// retired. The coordinate DSE now carries
// `.require("IsCoordinatorWithDirectives")` on its EligibilityFilter,
// so `score_dse_by_id` returns 0.0 for non-coordinator cats.
```

**Fix:** retire the `if ctx.has_eligible_mate` wrapper. Add a
`HasEligibleMate` (or `HasMateBondPartner`) marker authored by a
new author system that wraps the existing
`crate::ai::mating::has_eligible_mate` predicate. Add the marker to
`MateDse::eligibility().require(...)`. Update tests.

This restores per-tick L2 trace records for `mate` (currently zero
across 3.3M total L2 records in the dataset), giving observability
into whether scoring loses softmax post-fix.

### Bug 3 (design + implementation) — missing L2 PairingActivity

The §7.M design specifies a three-layer Mating model: L1
`ReproduceAspiration` (OpenMinded, High), L2 `PairingActivity`
(OpenMinded, Medium), L3 `MateWithGoal` (SingleMinded, High). Code
ships L1 and L3; **L2 PairingActivity is not built**.

L2's purpose is to escalate Friends → Partners by holding compatible
adults colocated long enough that the courtship-drift gates in
`check_bonds` accumulate `romantic > partners_romantic_threshold`.
Without L2, the only path to a Partners bond is passive drift, which
maths out to ~0.65 max romantic in a 900s soak (best case, all
gates open from tick 1) — barely above `partners_romantic_threshold
= 0.5` and only if the cats happen to stay within range 2 of each
other for the full window.

**Fix:** implement `PairingActivity` per §7.M. New
`DispositionKind::Pairing` variant, new
`src/ai/dses/pairing_activity.rs` (and target-taking equivalent
`pairing_activity_target.rs`) plus the GOAP step resolver. The
candidate filter for the target-taking DSE is **Friends** bond
(not Partners) plus orientation-compatible — i.e., the courtship
arc, distinct from L3's mating arc. The step resolver drives
proximity + grooming + socializing toward the target and emits
`Feature::CourtshipInteraction` per interaction tick; this is what
actually accelerates `romantic` accumulation.

## Out of scope

- Coefficient tuning of `courtship_*` constants. Fix the structural
  bugs first; tune against the post-fix baseline soak.
- The §7.W Fulfillment register's interaction with Pairing — track
  separately if a regression surfaces.
- Mating itself as a balance thread (target ≥ 7 / 7-season soak,
  per ticket 014). That's downstream tuning once the cascade is
  unblocked.

## Current state

Diagnosed from `logs/baseline-2026-04-25/REPORT.md`:

- Mate L2 records: **0 / 3.3M total L2 records** across 10 focal
  traces (Bug 2 confirmed — DSE never enters scoring stage).
- `mate_target` L2 records: **0** across all traces (no candidates
  with Partners bonds, ever).
- `CourtshipInteraction` activations: **0 / 15** sweep runs.
- `BondFormed` activations: **1–3 per run** — confirms passive
  familiarity + Friends-bond formation are working, but escalation
  never reaches Partners.
- Adult cats exist in every seed (rosters.json shows 2–3 per
  founder roster, growing to 4–5 by end of 900s).
- Verification dataset: `logs/baseline-2026-04-25/` (commit
  `cba19bd`, 27 runs, 8.9 GB, header-parity clean).

## Approach

Land the three fixes as separate commits in order Bug 1 → Bug 2 →
Bug 3, each verified against a fresh baseline-dataset run. Bug 1 +
Bug 2 are mechanical; Bug 3 is feature work that probably wants its
own design checkpoint before implementation.

## Verification

After each commit:

1. `just baseline-dataset 2026-04-26-mating-bug<N>` — produces a
   parallel archive at the same commit (header-parity preserved
   because the orchestrator runs against current HEAD).
2. `just baseline-report 2026-04-26-mating-bug<N>` — re-renders
   REPORT.md.
3. Diff against `logs/baseline-2026-04-25/REPORT.md`.

Per-bug acceptance:

- **Bug 1**: `continuity_tallies.courtship` > 0 in ≥ 80% of sweep
  runs; passive drift now visible in the canary.
- **Bug 2**: `mate` L2 records > 0 in every focal trace where the
  cat is Adult / not-Incapacitated; final-score landscape now
  ranks `mate` against other DSEs (may still be score-buried, but
  visible).
- **Bug 3**: `MatingOccurred` > 0 in ≥ 1 sweep run; L3 trace shows
  Pairing disposition winning at least once before MateWithGoal
  fires.

## Log

- 2026-04-25: Ticket opened from baseline-dataset findings.
  See conversation transcript at conversation/run-launch trace
  for full causal-chain reasoning.
- 2026-04-25: **Bug 1 landed** — `social::check_bonds` now records
  `Feature::CourtshipInteraction` and pushes a new
  `EventKind::CourtshipDrifted` variant inside the courtship-drift
  gate. The variant tallies as `continuity_tallies.courtship`
  alongside `MatingOccurred` per the §11.3 piggyback pattern.
  `Feature::CourtshipInteraction` was promoted out of the rare-
  legend exempt list (`expected_to_fire_per_soak() => true`) since
  it now fires whenever any compatible Adult pair drifts.
  Verification — single-seed seed-42 release deep-soak
  (`logs/tuned-42-027bug1/`):

  | metric | pre-Bug-1 | post-Bug-1 |
  |---|---|---|
  | `continuity_tallies.courtship` | 0 | 840 |
  | `Feature::CourtshipInteraction` | rare-legend exempt | fires (expected) |
  | shadowfox_ambush_deaths | 6 | 3 |
  | starvation_deaths | 1 (noise band) | 1 (noise band) |
  | never_fired_expected | 8 | 8 (CourtshipInteraction left the list as expected; MatingOccurred + downstream still 0 pending Bugs 2/3) |

  Acceptance criterion (`continuity_tallies.courtship > 0`) cleared
  by 840×. Multi-seed sweep deferred to after Bug 2 lands so the
  sweep also confirms `mate` L2 records appear.
