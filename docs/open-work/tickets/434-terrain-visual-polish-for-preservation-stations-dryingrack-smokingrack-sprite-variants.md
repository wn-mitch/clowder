---
id: 434
title: Terrain visual polish for preservation stations (DryingRack + SmokingRack sprite variants)
status: ready
cluster: rendering
orchestration: substrate-sensitive
initiative: [world-richness]
added: 2026-05-21
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
Ticket 367 (Phase 1b preservation) shipped Drying Rack and Smoking Rack as
constructible stations but reused `Terrain::Workshop` (drying) and
`Terrain::Hearth` (smoking) for the stage-1 visual — mirroring the Midden
precedent in `src/components/building.rs:97-104`. Stage-1 reuse avoided
churning the autotile palette + sprite atlas pipeline during the substrate
ship. The result is correct mechanically (the stations work; cats build
them, load them, output items spawn) but visually muddy: a drying rack is
indistinguishable from a workshop, and a smoking rack from a hearth, when a
player surveys their colony.

## Scope
- Add `Terrain::DryingRack` + `Terrain::SmokingRack` variants in `src/components/physical.rs`.
- Update `StructureType::DryingRack::terrain()` and `SmokingRack::terrain()` in `src/components/building.rs` to return the new variants (currently `Terrain::Workshop` / `Terrain::Hearth`).
- Wire the autotile / palette / sprite atlas entries for the two new terrains.
- Update `docs/wiki/systems.md` (via `just wiki`) if any system status shifts.

## Out of scope
- Other preservation-station visuals (Tanning Frame is tracked under 369 Phase 2b).
- Animated state (loaded vs idle, mid-cycle smoking visuals) — open as a separate ticket if the mechanic surfaces in playtesting.

## Current state
Substrate landed in 367 Commits 1-7 (2026-05-21). Visual reuse is intentional and documented; this ticket is the antipattern-migration follow-on per CLAUDE.md "Substrate-over-override discipline" applied to the visual layer.

## Approach
Find the autotile + atlas precedent in an existing pair (e.g. Workshop or Hearth themselves) and mirror the entries. The Midden precedent — `Terrain::Wilds` reuse for stage 1 — is the architectural sibling; if Midden gained its own terrain later (check `git log`), follow that template.

## Verification
- Visual diff via `just run` or screenshot fixtures.
- `just wiki` regenerates clean.
- Existing tests still pass; new sprite atlas entries don't break headless render path.

## Log
- 2026-05-21: opened as 367 antipattern-migration follow-on.
