---
id: 110
title: ThermalDistress modifier — substrate axis for thermal interrupts (and shelter-seeking)
status: ready
cluster: ai-substrate
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

`thermal_deficit` is already published in `ctx_scalars` but no modifier currently consumes it directly — it composes into `body_distress_composite` only. A kind-specific lift on Sleep (find shelter) and eventually Build (construct shelter) gives cold cats a behaviorally-distinct response from "generally distressed cats."

Lower priority than 106/107/108 because no current `InterruptReason::ThermalCritical` branch exists to retire; this is purely a perception-richness lever (the "shake the tree" pattern from ticket 047's design — richer cat understanding ⇒ more levers).

## Scope

- New `ThermalDistress` modifier reading `thermal_deficit`.
- Lifts Sleep (find a den / hearth; routes to warm tile) and Build (eventually — construct shelter; out of scope for v1).
- Constants: `thermal_distress_threshold`, `thermal_distress_sleep_lift`. Default 0.0 inert.
- Phase 3 hypothesize predicting cold-weather mortality drops.

## Out of scope

- The Build-lift (deferred — needs a "BuildShelter" disposition variant to make sense).
- Composing with weather forecast (separate spec).

## Log

- 2026-05-01: Opened as the fourth substrate-axis follow-on from ticket 047 — the lower-priority "more levers" application of the doctrine.
