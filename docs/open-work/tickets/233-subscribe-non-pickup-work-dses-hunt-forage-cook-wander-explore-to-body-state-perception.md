---
id: 233
title: Subscribe non-pickup work DSEs (Hunt Forage Cook Wander Explore) to body-state perception
status: ready
cluster: ai-substrate
orchestration: substrate-sensitive
added: 2026-05-08
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

The post-230 soak dying-arc analysis (`logs/tuned-42`, commit `ffb2b69b`,
focal Calcifer) revealed a substrate stub of the same shape on every
work-class DSE: external pressure (hunger urgency, food scarcity,
curiosity) is fully read, but the cat's own body state (`pain_level`,
`body_distress_composite`, `health_deficit`) is read **zero times** by
Hunt, Forage, Cook, Wander, Explore, HerbcraftGather. Sleep reads four
body-state axes; Flee reads two; pickup-class DSEs read zero (covered
by ticket 231 R3b). This ticket covers the food-production / exploration
half — same gap, different DSEs.

Cedar's dying arc at tick 1251600–1251700 (HP=0.64, safety crashing
from 0.44 → 0.31) showed Forage winning the L3 softmax twice in a row.
Forage's L2 score was 0.83–0.86 — driven by hunger urgency (0.34) +
food scarcity (high in season 3) + diligence (linear scaling). None of
those axes asked "is this cat fit to forage right now?" A wounded cat
foraging at the edge of fox-scent territory is the substrate failing
to describe its own viability — exactly the same shape as 231's PickUp
gap, applied to the food-production side.

Per CLAUDE.md and the substrate-over-override directive reaffirmed in
230's session, the fix is not a filter or gate. The fix is to add a
Consideration to each work-class DSE that reads body-state perception
already published in `ctx_scalars`, so the DSE's L2 score honestly
reflects "this is a poor task choice when the cat is in distress." The
exemplar pattern is `src/ai/dses/sleep.rs:considerations` — Sleep
reads four body-state axes plus two spatial axes; the work DSEs need
the same body-state subscription with inverted curve direction
(Sleep rises on body distress; work damps).

## Scope

DSEs that need body-state Considerations added (one Consideration each;
weight calibrated to preserve healthy-cat scoring within ε):

- `src/ai/dses/hunt.rs::HuntDse`
- `src/ai/dses/forage.rs::ForageDse`
- `src/ai/dses/cook.rs::CookDse`
- `src/ai/dses/wander.rs::WanderDse`
- `src/ai/dses/explore.rs::ExploreDse`
- `src/ai/dses/herbcraft_gather.rs::HerbcraftGatherDse`
- (defer to scope review): `src/ai/dses/farm.rs::FarmDse` —
  food-production but heavily marker-gated already; verify whether
  the body-state axis materially shifts current behavior before
  including.

For each DSE:

- Add a `Consideration::Scalar` reading `body_distress_composite` (or
  `pain_level` / `health_deficit` — pick the composite that best
  captures "this is a bad task right now"; recommendation:
  `body_distress_composite` for non-combat work, `health_deficit` for
  combat-adjacent like Hunt, but verify per-DSE).
- Curve shape: an inverted/damping `Linear` or `Logistic` so high
  distress drives the axis toward 0, low distress toward 1.
- Composition weight calibrated so:
  - Healthy cat (body_distress_composite ≈ 0): score within ε of
    pre-fix.
  - Cat at HP=0.4 with active threat: score materially below
    Sleep / Flee / Eat (target: > 30% gap to the survival-tier winner).

Per-DSE calibration will require sensitivity sweep — `just hypothesize`
with a body-state-axis-weight spec.

## Out of scope

- **Pickup-class DSEs** (PickUp / RetrieveRawFood / GatherHerb /
  RetrieveFoodForKitten) — covered by ticket 231 R3b. Same shape, same
  fix; lands in 231's session.
- **Combat DSE (Fight)** — Fight has its own body-state semantics
  (cornered-cat ferocity per ticket 102's gating); the AcuteHealth gate
  already inverts Flee/Fight under low escape_viability. Verify Fight
  isn't already correct before touching; if it IS the same gap, fold
  into this ticket; if not, leave alone.
- **Disposal DSEs** (Discarding / Trashing / Handing) — these are
  Maslow-tier-1 inventory-clearing and might WANT to fire under body
  distress (clearing inventory to make room for safer items). Verify
  per-DSE; default-include unless the analysis shows otherwise.
- **Magic / Witchcraft DSEs** (MagicScry / MagicCleanse / etc.) —
  separate scope; spiritual-work might score differently under body
  distress than colony-feeding work. Open as a sibling if the gap
  applies.
- **Personality coupling on the body-state axis** (anxious cats damp
  more steeply; bold cats damp less) — composable as a follow-on.

## Current state

- `body_distress_composite` is published in `ScoringContext` and
  surfaces in `ctx_scalars` (see `src/ai/scoring.rs:209+, 577+`).
  The substrate side of the gap is closed.
- Sleep, Flee consume body-state scalars (Sleep: four axes; Flee:
  two). The asymmetry is the structural defect.
- Ticket 231 R3b covers the pickup-class DSEs. This ticket covers the
  food-production / exploration half. Both compose with ticket 232's
  body-state-coupled softmax temperature.

## Approach

1. Per DSE: add the Consideration, calibrate the weight, run the
   verification sweep before locking.
2. Land DSE-by-DSE rather than as a single batch — each DSE has its
   own balance surface and the soak-side regression risk is lower
   with smaller commits. Consider a `subspec` per DSE under
   `docs/balance/233-*.md`.
3. Mirror 231 R3b's curve calibration approach: pick the body-state
   scalar, pick the curve, weight to preserve healthy-cat parity,
   verify dying-cat behavior change.

## Verification

- **Healthy-cat parity (per DSE):** scenario with all needs > 0.7, no
  threat, no injury — assert per-DSE L2 score matches pre-fix within ε.
- **Dying-cat behavior shift (per DSE):** scenario with HP=0.4 + active
  threat — assert per-DSE L2 score drops materially (> 30% gap from
  Sleep / Flee / Eat scores).
- **Soak drift:** seed-42 `just soak-trace 42 Calcifer` post-fix; the
  dying-arc decision points (Cedar tick 1251600 Forage, etc.) should
  pivot to tier-1 dispositions; modifier_preemption count drops
  further alongside 230 + 231.
- **Per-DSE balance doc:** `docs/balance/233-<dse>-body-state-axis.md`
  documenting the four-artifact methodology (hypothesis · prediction ·
  observation · concordance) for each DSE's calibration.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed ** 87** (done, ai-substrate, score 0.90) — Interoceptive perception substrate
- ✓ landed ** 88** (done, ai-substrate, score 0.88) — Body-distress Modifier — uniform self-care promotion under §L2.10 Modifier subs…
- ✓ landed **189** (done, ai-substrate, score 0.88) — Post-178 food_available regression — layer-walk diagnosis

<!-- linkages:end -->
## Log

- 2026-05-08: opened from post-230 soak dying-arc analysis. Cedar at
  HP=0.64, safety dropping, picked Forage twice in a row (L2 score
  0.83–0.86) while Sleep was scoring 1.05–1.08. Forage's L2
  Considerations don't include any body-state axis — same shape as
  231's PickUp gap on the food-production half. Per substrate-over-
  override discipline, fix is to add Considerations rather than gate
  or filter.
