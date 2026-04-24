---
id: 002
title: Hunt-approach pipeline failures
status: ready
cluster: null
added: 2026-04-20
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Current state

**Why it matters:** 1,774 "lost prey during approach" failures in the
treatment soak vs. 9 "no scent found" search timeouts. Refines the
findability hypothesis: cats locate prey via scent fine, then lose it
during stalk/approach.

**Candidate levers:**
- Stalk speed (currently 1.0 tiles/tick, previously tuned up from 0.5)
- Approach speed (currently 3 tiles/tick)
- Prey detection-of-cat during approach phase (`try_detect_cat` in
  `src/systems/prey.rs`)
- Stall-out conditions — "stuck while stalking" fires 257–341× per soak,
  which is a separate failure mode from "lost"

**Catches-per-week trajectory** (seed-42, 17 weeks): week-0 boom (66),
weeks 1–3 settle (22/9/18), weeks 4+ oscillate 3–15. Not a flatline — the
local depletion → recovery cycle works. The issue is conversion: 1,981
Hunt plans created, ~11% convert to kills.
