---
id: 2026-04-22
title: "Phase 4b.4 — §4 `HasGarden` marker port"
status: done
cluster: null
landed-at: null
landed-on: 2026-04-22
---

# Phase 4b.4 — §4 `HasGarden` marker port

Second reference port of the Phase 4b.2 MVP pattern. Farm's outer
`if ctx.has_garden` gate retired; `FarmDse::new()` gains
`.require("HasGarden")`. Caller-side population in goap.rs /
disposition.rs reuses the existing `has_garden` computation with a
single appended `markers.set_colony("HasGarden", has_garden)` line.
Reinforces that per-marker porting is mechanical: three line
changes (population + `.require` + outer-gate retirement) +
optional test-fixture update.

Does not unblock Farming dormancy — the baseline's Farming = 0
traces to `TendCrops: no target` plan-failures (target-resolver
issue in GOAP), not an outer-eligibility issue.
