---
id: 111
title: Retire 088 BodyDistressPromotion once kind-specific modifiers cover its surface
status: done
cluster: ai-substrate
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, distress-modifiers.md]
related-balance: [088-body-distress-modifier.md, 146-distress-substrate-inert.md]
landed-at: TBD
landed-on: 2026-05-02
---

## Resolution: closed without retirement

088 is retained. Ticket 146's investigation (see
`docs/open-work/landed/146-088-courtship-coverage-investigation.md`)
found:

- No structural coupling between 088 and any other substrate. The
  `BodyDistressed` marker is write-only and `body_distress_composite()`
  is read by no modifier.
- 088's effect on the courtship chain is incidental — a six-thousandths
  fondness nudge to one specific dyad (Mocha+Birch in seed-42) that
  pushes one fondness ceiling above the 0.30 courtship-drift gate.
  Without 088, that pair sits at 0.297 and the chain stalls.
- Removing 088 with per-axis lifts inert (`removal-bare`) preserves the
  rest of the colony's welfare metrics but collapses courtship.
- Restoring per-axis lift magnitudes to compensate for 088 produces
  a U-curve where mid-magnitude values are WORSE than either inert
  or full magnitudes, and full magnitudes cause colony extinction
  via the 107+110 Sleep double-stack.
- Retiring 088 makes the colony fragile in ways that aren't worth the
  cleanup hygiene win.

## Why (original)

088's `BodyDistressPromotion` is the original undifferentiated lift —
`body_distress_composite = max(deficits)` flattens
which-kind-of-distress into one scalar and lifts six self-care DSEs
uniformly. The kind-specific modifier program (047, 106, 107, 110)
replaces this surface with discriminating modifiers — each reading the
source axis directly with a curve and DSE-targeting that match the
actual phenomenon.

The original close-out plan was: once 047 + 106 + 107 + 110 ship,
retire 088. Verification soak invalidated this — 146 then investigated.

## Out of scope

- Retiring 087 `BodyDistressed` ZST marker. (Marker is write-only;
  could be retired as a separate cleanup follow-on.)

## Log

- 2026-05-01: Opened as cleanup follow-on from ticket 047. Blocked by
  the three remaining substrate-axis tickets that complete 088's
  replacement.
- 2026-05-02: **Unblocked.** 106 + 107 + 110 all landed inert by today.
  Note: 088 is still load-bearing in the canonical regime — retirement
  should be paired with a same-commit verification soak.
- 2026-05-02: **Verification soak failed** — `continuity_tallies.courtship`
  collapsed 999 → 0; `CourtshipInteraction` and `PairingIntentionEmitted`
  joined never-fired. Surfacing per-axis lifts at conservative
  magnitudes recovered nothing on courtship and over-corrected into
  Sleep loops. Status `ready` → `blocked` pending ticket 146
  investigation.
- 2026-05-02: **Closed without retirement** per ticket 146's
  investigation findings. 088 stays active. Status `blocked` → `done`.
