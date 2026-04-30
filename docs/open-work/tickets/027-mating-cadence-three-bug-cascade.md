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

## Substrate-over-override pattern

Part of the substrate-over-override thread (see [093](093-substrate-over-override-epic.md)). This ticket tracks three mating-cadence bugs that each fit the pattern.

**Hack shape**:
- **Bug 1** — observability hack: `Feature::CourtshipInteraction` emitted only inside MateWith, masking passive-drift courtship. Continuity canary lies.
- **Bug 2** — lifted-condition outer gate at `scoring.rs:916` (`if ctx.has_eligible_mate { … }`) bypasses the substrate's marker-based eligibility.
- **Bug 3** — missing L2 substrate layer (PairingActivity); compensated for via post-IAUS bias-pin at `socialize_target.rs:193`.

**IAUS lever**:
- Bug 1: emit the feature inside `check_bonds` (substrate-side observability).
- Bug 2: retire the wrapper, author `HasEligibleMate` marker, gate via `EligibilityFilter::require()`.
- Bug 3: implement L2 PairingActivity as a parallel persistent commitment layer (see [027b](027b-l2-pairing-activity.md)) — multi-layer choreography expressed natively in the substrate per spec §7.M.

**Sequencing**: Bugs 1 and 2 are landed. Bug 3 is parked under 027b, blocked-by [071](071-planning-substrate-hardening.md) (planning-substrate hardening). The bias-pin at `socialize_target.rs:193` is itself a hack — ticket 078 (under 071) is the substrate-side replacement (a `target_pairing_intention` Consideration that turns the pin into a curve-tunable axis).

**Canonical exemplar**: 087 (CriticalHealth interrupt → `pain_level` + `body_distress_composite` axes, landed at fc4e1ab).

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
- 2026-04-25: **Bug 2 landed** — `mating::update_mate_eligibility_markers`
  is now a real per-tick author of the `HasEligibleMate` ZST, wrapping
  the existing `has_eligible_mate` predicate. `MateDse.eligibility()`
  carries `.require(HasEligibleMate::KEY)`, retiring the lifted
  `if ctx.has_eligible_mate { ... }` wrapper at `scoring.rs:916`.
  `ScoringContext.has_eligible_mate` field deleted; inline computation
  + 8 test fixture rows + redundant `mating_fitness` / `current_season`
  let-bindings in `disposition.rs` and `goap.rs` removed. New
  `mate_eligibility` query in `MarkerQueries` (disposition) +
  sibling `mate_eligibility_q` in `goap.rs` populates the snapshot
  via `markers.set_entity(HasEligibleMate::KEY, entity, has)` per cat.

  Verification — survival (`logs/tuned-42-027bug2/`) + focal-trace
  (`logs/tuned-42-027bug2-trace/`):

  | metric | pre-Bug-2 baseline | post-Bug-2 |
  |---|---|---|
  | `mate` L2 records (Simba focal trace) | 0 / 3.3M | **34,949** |
  | mate L2 with `passed:true` | 0 | 0 (Simba never reaches Partners in 900s — Bug 3 territory) |
  | continuity_tallies.courtship | 840 | 1175 (Bug 1 still emitting) |
  | shadowfox_ambush_deaths | 6 | 4 |
  | starvation_deaths | 1 (noise) | 0 |
  | never_fired_expected | 8 | 8 (MatingOccurred + downstream still 0; pending Bug 3) |

  Acceptance criterion ("`mate` L2 records > 0 in every focal trace
  where the cat is Adult / not-Incapacitated") cleared. The DSE is
  now visible to the L2 trace pipeline; once Bug 3 (PairingActivity
  L2) drives cats to a Partners bond, the same trace machinery will
  show `passed: true` records and a real `final_score` landscape.

  4 new tests: 3 in `mating.rs` exercising the author system
  (insert on eligible pair / skip on no bond / remove when partner
  becomes Dead); 1 in `mate.rs` asserting the EligibilityFilter
  carries the `.require(HasEligibleMate::KEY)`. 1303 / 1303 lib tests
  green.
- 2026-04-26: **Bug 3 partial — target-picker bias attempt (NOT
  acceptance-clearing)**. The full L2 `PairingActivity` self-state
  DSE described in the ticket scope (with `DispositionKind::Pairing`,
  `pairing_activity.rs`, GOAP step resolver, and target-taking
  sibling) was scoped during exploration and judged a multi-commit
  feature with open design questions — see `commitment.rs:175–183`'s
  doc-comment ("L1/L2 strategies are carried inline on the
  Intention, not [in `DispositionKind`]") and the discovery that
  target-taking DSEs are *not* independent score competitors today
  (per `socialize_target.rs:14–19`'s "target-quality merging into
  the action pool is deferred"). A standalone `pairing_activity_target.rs`
  would have been inert.

  Pivoted to the smallest intervention that addresses the
  Mocha+Birch failure mode found in `tuned-42-027bug3-trace`: the
  pair reached `romantic = 1.0` but stayed Friends because the
  Partners promotion gate at `social.rs:146` requires
  `fondness > 0.6 ∧ familiarity > 0.5 ∧ romantic > 0.5` — and the
  passive courtship-drift loop only accumulates `romantic`. Two
  layered changes:

  1. **Bond-bias added to `socialize_target`** as a fifth
     consideration (`target_partner_bond`, Linear curve, weight
     0.20; existing four weights renormalized ×0.80 to keep the
     RtEO sum at 1.0). Graduated scalar: None=0.0, Friends=0.5,
     Partners/Mates=1.0 — keeps a paired cat oriented toward the
     deeper bond. Fetcher resolves via a new `bond_score(...)`
     helper. SocializeDse fires reliably (565 courtship events in
     the bug3-trace, 484 play, 151 grooming) so this rides on an
     existing high-frequency selection path; cats that choose to
     socialize now preferentially pick a Friends-bonded compatible
     peer, concentrating fondness/familiarity accumulation with the
     same partner. 4 new tests + retrofitted "five axes" length
     test.
  2. **Two constants tweaked** in `SocialConstants`. `partners_fondness_threshold`
     0.60 → 0.55 (direct response to Mocha+Birch — fondness was the
     wall, not romantic). `courtship_romantic_rate` 0.0025 → 0.0035
     (1.4×, inside the ±30% noise band — late-spawning pairs were
     timing out before reaching the romantic threshold).
     `mates_fondness_threshold = 0.7` left untouched as the deeper-
     affection ceiling.

  Verification — `just check` clean, `just test` 1308 / 1308 green
  (including `compatible_adults_reach_partners_bond_in_expected_time`
  which exercises the courtship timing). Single-seed deep-soak
  (`logs/tuned-42-027bug3-bias-5a5506e-dirty/`): **inconclusive**.
  Run landed in the unlucky tail of seed-42 scheduler-noise
  (`continuity_tallies.courtship = 0`, `BondFormed = 0`,
  `MatingOccurred = 0`) — the same noise CLAUDE.md flags as a
  re-run-of-same-commit phenomenon. Comparable bracket: bug2-trace
  had `courtship = 0`, bug3-trace had `courtship = 565` — same
  code, different runs. `Starvation = 0` (improvement vs baseline);
  `ShadowFoxAmbush = 7` (within the ≤10 cap). The single-run signal
  cannot distinguish a regression from a noise-tail draw; multi-seed
  sweep validation is required.

  **Bug 3 acceptance NOT cleared** — `MatingOccurred > 0` is the
  hard gate, and there is no run yet showing it. Recommended next
  step: `just baseline-dataset 2026-04-26-bug3-bias` (3-rep ×
  4-seed sweep) to average over the noise band, then
  `just sweep-stats … --vs logs/baseline-2026-04-25` for the
  Welch's-t / Cohen's d direction-of-drift check. Promote
  `logs/baseline-2026-04-25` (or the post-bias sweep, depending on
  which represents healthy state) to `logs/baselines/current.json`
  via `just promote-baseline` — the missing pointer is why
  `verdict` falls back to a 5-version-stale baseline today, drowning
  real signal in substrate-refactor drift noise.

  Status remains `in-progress`. Do not move to `landed` until the
  multi-seed sweep clears `MatingOccurred > 0`. The full L2
  `PairingActivity` DSE remains as a possible follow-up if the
  bias-only intervention is insufficient — open as ticket 027b
  rather than nesting a fourth bug here.
