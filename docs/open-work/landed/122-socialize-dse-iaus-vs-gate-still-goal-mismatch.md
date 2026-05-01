---
id: 122
title: Socialize IAUS scoring elects plans the OpenMinded gate drops on the same tick
status: done
cluster: substrate-over-override
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: b47af48f
landed-on: 2026-05-01
---

## Why

The §7.2 commitment gate's `still_goal` proxy for Socializing is
`needs.social < social_satiation_threshold (0.85)`
(`src/ai/commitment.rs#L307-L315`). The IAUS-side `Socialize` DSE elects
plans on sociability/playfulness/loneliness considerations that don't
include the same satiation predicate. When `social ≥ 0.85` (well-bonded
cats — founders at `start_tick`, or any cat just after a Socialize
completion) the producer side keeps electing the disposition and the
gate drops every plan on the same tick it was created. In the seed-42
diagnosis window for ticket 121, 588 of 3585 `PlanCreated` events in
1500 ticks are `Socializing` plans that died before any step fired.

This is a substrate-over-override violation: the IAUS layer and the
commitment gate disagree about whether the cat wants the goal. The gate
wins, but only after a planning round-trip per tick.

## Scope

- Re-confirm with focal-cat trace that Socializing plans land and drop on
  the same tick when `social ≥ social_satiation_threshold` (and that this
  still happens after ticket 121's fixes land).
- Add a satiation consideration to the `Socialize` DSE so the producer
  side mirrors the gate. Likely a Linear or Logistic curve over
  `social` need with the same midpoint as `social_satiation_threshold`.
- Verify the change does not over-correct (Socialize should still elect
  for low-social cats; this is just a scoring axis, not an eligibility
  filter).

## Out of scope

- Re-tuning `social_satiation_threshold` itself.
- Doing the same for any other DSE; if the same pattern shows up in
  Explore (which has its own gate proxy on `unexplored_fraction_nearby`)
  open that as a separate ticket — the Socialize fix should not be
  generalized speculatively.

## Current state

Ready to start once ticket 121 lands. Blocked because 121's `Wilds`-fix
may shift the cat-density / social-need profile enough to re-prioritize
this ticket.

## Approach

1. Locate the `Socialize` DSE factory (`src/ai/dses/socialize.rs`).
2. Add a `ScalarConsideration` on the social-need axis whose curve
   evaluates near zero when `social ≥ social_satiation_threshold` and
   near 1 when `social` is low. The CompensatedProduct composition will
   then gate Socialize on satiation the same way the commitment gate
   does.
3. Existing tests in `src/ai/commitment.rs` (`gate_drops_open_minded_socializing_on_satiation`)
   document the threshold; add an IAUS-side test that the DSE scores
   near zero at the same threshold.

## Verification

- `cargo nextest run --features all` for new DSE-scoring tests.
- 15-min `just soak 42` shows `CommitmentDropOpenMinded` with the
  `Socializing` disposition fraction dropping noticeably; cats with
  high social still socialize occasionally (the score doesn't have to
  be zero — just well below other elected dispositions when sated).
- Continuity canary: `Socialized` count per soak doesn't collapse.

## Log

- 2026-05-01: Carved out of ticket 121 §Approach #2.
- 2026-05-01: **Unblocked.** 121 landed (anchor-substrate alignment for
  `PlannerZone::Wilds`); the post-fix soak's cold-start window did not
  shift (first `BuildingConstructed` still at tick 1_201_490, identical
  PlanCreated count of 3588 in `[1_200_000, 1_201_500)`). Confirms 121
  alone was insufficient — the Socialize gate-mismatch carveout
  (588 of 3585 cold-start plans elected and immediately dropped per
  ticket 121's diagnosis) is load-bearing for the symptom. Tagged
  `cluster: substrate-over-override` since the IAUS/L3-gate
  dual-language is the same shape the epic catalogs.
- 2026-05-01: **Implemented.** New `SOCIAL_SATIATION_INPUT` axis on
  `SocializeDse` reading raw `social` need; curve is
  `Composite { Logistic(steepness=8, midpoint=0.85), Invert }` so
  `social=0 → ~1.0`, `social=0.85 → ~0.5`, `social=1.0 → ~0.0`.
  Composition stays `WeightedSum` (NOT `CompensatedProduct` — the
  existing `phys_satisfaction` axis at midpoint 0.3 would also become
  a multiplicative gate, locking well-fed cats out of socializing,
  which the §Out-of-scope caution forbids). New axis weight 0.30 with
  the existing 7 weights renormalized ×0.70; at full satiation
  (signal ≈ 0) the weighted-sum drops by 30% from pre-fix baseline.
  Five new tests cover axis presence, weight balance, curve
  monotonicity, and threshold midpoint. 1695 lib tests pass; `just
  check` clean (step-resolver + time-units + iaus-coherence).
  Soak verification deferred to the user's commit / push step.
