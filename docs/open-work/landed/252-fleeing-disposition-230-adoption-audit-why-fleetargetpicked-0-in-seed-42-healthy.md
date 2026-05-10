---
id: 252
title: Fleeing disposition (230) adoption audit — why FleeTargetPicked = 0 in seed-42 healthy
status: done
cluster: ai-substrate
added: 2026-05-09
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-10
---

## Why

Ticket 230 (landed) carved `DispositionKind::Fleeing` out of the
legacy `check_anxiety_interrupts::ThreatDetected` arm — the closing
move on the §4.7 substrate-over-override migration thread. The
substrate-aware Fleeing disposition has a real plan template
(`[PickFleeTarget, Flee, HoldUntilSafe]`), reads the per-cat
`RouteCostField` (boldness-scaled fox-scent + corruption overlays)
to pick the lowest-cost passable tile within `flee_distance`, and
composes with the modifier preempt guard instead of firing every
tick. It's the load-bearing example of substrate-aware retreat.

249's verification audit (2026-05-09) discovered that
**`FleeTargetPicked` cumulative = 0 in every seed-42 healthy soak in
`logs/tuned-42-*`** — the substrate-aware Fleeing disposition
**never adopts** in the canonical healthy run. `FleeRecovered`
likewise = 0. The Fleeing path is dead code on seed-42's healthy
trajectory.

This ticket determines whether that's:

1. **Intended** — per the `AcuteHealthAdrenalineFlee` modifier
   doc (047): *"Flee is filtered from the disposition softmax"*,
   so Flee should never win L3 in healthy contexts; the lift's
   landing target is Sleep ("the in-pool partner"), not Flee
   itself. The Fleeing disposition activates only under genuine
   threat-driven contexts that don't appear in the seed-42 healthy
   profile (e.g., ShadowFox attack waves on collapsed runs).
2. **A regression** — Fleeing should be adopting in some seed-42
   contexts (e.g., wildlife combat encounters, fox-scent overlays
   driving low-bold cats away) but isn't, due to a wiring gap, a
   scoring axis weight that's wrong, or interaction with another
   modifier / commitment substrate.

Either outcome warrants documentation: if intended, document it in
spec §4.7 / §6 so future contributors don't re-investigate; if
regression, fix.

## Scope

- **Phase A — historical spread.** Survey `FleeTargetPicked` and
  `FleeRecovered` cumulative counts across every `logs/tuned-42-*`
  archive (and selected non-42 seeds if available) — confirm or
  refute the "0 in every healthy soak" finding.
- **Phase B — collapsed-run engagement.** Check whether Fleeing
  *does* adopt on collapsed soaks (e.g.,
  `logs/tuned-42-post-246-floor-removed-collapsed`,
  `logs/tuned-42-post-248-boundary-zero-collapsed`) — if so, the
  disposition is wired correctly and just doesn't fire on healthy
  trajectories.
- **Phase C — scenario microexperiment.** Construct a scenario
  (e.g., low-bold cat + nearby ShadowFox + open terrain + healthy
  HP) where Fleeing should plausibly win L3. If it doesn't, walk
  the L1 markers → L2 DSE scores → L3 softmax to find the gap.
- **Phase D — write up.** Either: (a) document Fleeing as
  collapsed-run-only behavior in spec §4.7 (intended), or (b)
  open a fix ticket for the wiring gap (regression).

## Out of scope

- **Re-shaping Fleeing's plan template / consideration set.** This
  ticket audits adoption, not internals.
- **Retiring `AcuteHealthAdrenalineFlee`.** That's ticket 251.
  This ticket may *inform* whether 251's retirement should also
  re-examine why Flee can't win L3 (the disposition-softmax
  filter), but doesn't re-do that work.
- **Fox-aware route cost field changes.** 228 (RouteCostField) is
  the substrate Fleeing reads from; questions about its accuracy
  are separate.

## Current state

- 230 landed at sha — see `landed/230-...md`. Plan template +
  target picker wired; `FleeTargetPicked` is a registered Feature
  with `expected_to_fire_per_soak()` returning **false** (rare
  enough not to canary-trigger, per
  `src/resources/system_activation.rs`).
- 249's verification audit cataloged `FleeTargetPicked = 0` and
  `FleeRecovered = 0` across the post-247 baseline + the post-249
  attempted soak. Both are explicitly counted in the
  `SystemActivation` rolling totals.
- The `AcuteHealthAdrenalineFlee` modifier (047) lifts Flee score
  but per its own doc, *"Flee is filtered from the disposition
  softmax"* — this is the candidate explanation for why Flee never
  wins L3 in healthy contexts.

## Approach

1. **Phase A — quick query.** `just q` over every healthy soak
   archive: extract `FleeTargetPicked` from the last
   `SystemActivation`. Tabulate. Confirm 0 across the board.
2. **Phase B — collapsed-run check.** Same query against
   collapsed-soak archives. If non-zero in some, the disposition
   does adopt under threat-cascade contexts.
3. **Phase C — scenario.** Build a `flee_substrate_engagement`
   scenario in `src/scenarios/` (low-bold cat + adjacent
   ShadowFox + healthy HP + open passable terrain). Run with
   `just scenario`; expect Fleeing to win L3 within ≤5 ticks. If
   it does, the path works and the seed-42 healthy non-adoption
   is a profile artifact. If it doesn't, walk the layers:
   - L1: `HasThreatNearby` set?
   - L2: `Flee` DSE score competitive with `Hunt` / `Sleep`?
   - L3: dispositions softmax filter excludes `Fleeing`?
4. **Phase D — write up.** Document the verdict in spec §4.7 (or
   fix-ticket if regression).

## Verification

- Phase A complete: tabulated `FleeTargetPicked` across all
  `logs/tuned-42-*` archives.
- Phase B + C complete: collapsed-run + scenario data establish
  whether the path is wired correctly.
- Phase D output: a paragraph in spec §4.7 (or §6 — depending on
  where Fleeing's adoption profile is best documented) naming the
  observed-vs-expected adoption rate, OR a fix ticket open with
  scope.

## Log

- 2026-05-09: opened from 249's audit. The 0-adoption finding
  surfaced when investigating the modifier-preempt regression;
  `FleeTargetPicked = 0` is independent of 249's gate (true in
  baseline too). User flagged: *"are cats not fleeing using the
  fox aware path?"* — answer: no, they're not, regardless of
  249's fix. This ticket determines whether that's intended.
- 2026-05-10: 2026-05-10: filter lift landed; verification soak tuned-42-post-252-fleeing-collapse FAILED (kittens_born 4→0; courtship 0; never_fired_expected_positives = MatingOccurred/CourtshipInteraction/PairingIntentionEmitted; HoldUntilSafe step-timeouts 71). Root: surfaced PickFleeTarget witness-contract bug. User chose to land regression and split fix. Follow-ons: 254 (picker witness fix) blocks-on 252; 255 (108 calibration audit) blocks-on 252.
