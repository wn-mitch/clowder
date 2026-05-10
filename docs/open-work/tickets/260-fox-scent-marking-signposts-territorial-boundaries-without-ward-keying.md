---
id: 260
title: Fox scent-marking signposts — territorial boundaries without ward keying
status: ready
cluster: null
added: 2026-05-10
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Foxes currently know the boundaries of cat territory by reading `WardCoverageMap` directly — substrate that exists for cat-side patrol routing, repurposed as a fox-side avoidance signal. This is mechanically convenient but ecologically dishonest: real predators don't perceive defensive infrastructure as a label, they perceive *territorial scent marks* and learn to route around them. Wards happen to *be* one such marking mechanism, but the perception path should be "fox sniffs scent → fox routes away," not "fox queries WardCoverageMap." The asymmetry leaves room for the substrate to model wildlife scent-marking too — foxes mark *their* territory, and other foxes (and cats) read those marks.

Surfaced 2026-05-10 by user during C3 spinout planning: "patrol also made me realize that it should have a ticket for handling scent marking behaviors so that foxes naturally know boundaries without having them key off of wards. We can make this a signpost graphic to distinguish."

## Scope

- **`FoxScentMap` extension** (already exists, `src/resources/fox_scent_map.rs:10–20`): foxes emit scent into the bucketed grid as they patrol; existing system handles decay. Already used as a *cat-side* path-cost penalty per ticket 223. This ticket extends consumers to include *fox-side* perception so foxes route around *other* fox territories.
- **Cat-side scent marking** (NEW): cats (especially coordinator / dominant cats) emit a `CatScentMap` (sibling resource, same bucketed shape). Wards' deterrent effect on foxes is partly mediated by the cats present at and around the warded site — the scent gradient *is* the perceptual signal, with wards adding a separate magic-perception layer.
- **Fox DSE re-wiring**: `src/ai/dses/fox_*.rs` (especially `fox_patrolling.rs`, `fox_raiding.rs`) read `CatScentMap` intensity instead of (or alongside) `WardCoverageMap`. Wards continue to deter foxes through the magic perception channel, but baseline territorial avoidance is scent-driven.
- **Signpost graphic**: render scent-marked tiles with a distinct visual signpost / spray-tile so the player can read territorial boundaries from the world without opening overlays.

## Out of scope

- Wildlife mental models of cat territories (that's C3 / ticket 258 substrate; this ticket is the *cue source*, not the *perception layer*).
- Snake / hawk territorial marking (different ecology — snakes use chemical-trail signaling, hawks use airspace+vocal; both deserve their own tickets if they prove load-bearing).
- Removing the `WardCoverageMap` entirely (it stays — wards provide a magic deterrence channel orthogonal to scent).
- Player-controlled scent marking or de-marking (deodorant items etc.).

## Current state

- `FoxScentMap` (`src/resources/fox_scent_map.rs:10–20`): bucketed scent grid (5-tile buckets, decay-per-tick), authored by fox patrol systems. Currently consumed by cat A* path-cost (ticket 223) and Patrol DSE route-cost overlay (256 R4).
- `WardCoverageMap` (`src/resources/ward_coverage_map.rs:43–53`): coverage intensity grid (0–1) replenished per-tick from live `Ward` entities. Currently doubles as fox-side avoidance signal — that conflation is the defect this ticket addresses.
- No `CatScentMap` exists yet.

## Approach

1. New `src/resources/cat_scent_map.rs` mirroring `FoxScentMap` shape (bucketed grid, decay).
2. Per-tick author from cat positions (every cat emits some scent; coordinators emit more; territorial cats emit more in their claimed zones).
3. Fox DSE updates: `fox_*.rs` consumers add a `CatScentMap` read for territorial-deterrence, reduce the direct `WardCoverageMap` read to magic-deterrence only.
4. Render: extend tilemap overlay (the existing F6/F7/F8 overlay toggles per CLAUDE.md Rendering section) with a scent-signpost visual.
5. Verify: foxes still avoid warded zones (now via two channels), patrol cascade signature (256 memory) doesn't recur, ShadowFoxAmbush canary stays ≤ 10.

## Verification

- Scenario microexperiment: spawn fox near a cluster of cats; verify fox routes around (not through). Repeat with no cats present + only wards; verify magic-deterrence channel still works.
- Soak: `just soak 42` + `just verdict` confirms no canary regression.
- Frame-diff: per-DSE drift on `fox_patrolling`, `fox_raiding` should show shifted scoring inputs but similar election distribution (substrate change, not balance change).

## Log

- 2026-05-10: opened as parking-lot idea surfaced during C3 spinout planning (ticket 258). Independent of the named cluster; addresses a substrate honesty defect (foxes "knowing" about wards rather than perceiving scent).
