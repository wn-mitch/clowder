---
id: 200
title: Land PlannerZone::Midden — building-singleton zone for Trashing
status: ready
cluster: process-discipline
added: 2026-05-06
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Surfaced by 195's stub-comment lint. `Trashing`'s plan template currently
routes through `PlannerZone::Wilds` as a placeholder — the comment at
`src/ai/planner/actions.rs:240-245` flags this as a stub awaiting
`PlannerZone::Midden`. The Midden building exists as a colony singleton
(see `markers::HasMidden`), so the resolver could pin to the actual
Midden tile rather than the open Wilds. Today the lint allowlist points
at this ticket; landing it removes the allowlist entry.

## Scope

- Add `PlannerZone::Midden` to `PlannerZone` in `src/ai/planner/mod.rs`.
- Wire the zone resolver (planner zone → ECS target lookup) to find the
  colony's Midden building tile via `HasMidden` / the building registry.
- Update `trashing_actions` plan template at `actions.rs:246-257` to
  route through `PlannerZone::Midden` instead of `PlannerZone::Wilds`.
- Drop the `STUB(trashing): ...` comment + allowlist entry once the lint
  validates the new wiring.

## Out of scope

- Adding a Midden-spawn flow (the colony already gets one Midden via
  the founding sequence). Midden-multiplicity is a separate concern.
- Changing trashing's eligibility filter — `require(HasMidden)` already
  gates the disposition; this ticket only changes the *route*.

## Current state

195's lint flags the trashing comment as a stub-with-lifted-curve. The
substrate_stubs.allowlist entry for it cites this ticket; landing here
unblocks the lint promise.

## Approach

Walk the PlannerZone enum (`mod.rs:50-79`) for the existing
`MaterialPile` / `CarcassPile` precedents — those resolve OnGround
items by query. `Midden` resolves a singleton building entity instead;
the closer precedent is whatever zone resolves `Stores` (which is
also a singleton). Pattern-match on that.

## Verification

- `just check` clean after dropping the trashing allowlist entry.
- `just scenario disposal_election --focal <cat>` shows Trashing routing
  through the Midden tile rather than Wilds.
- No soak required — the route change is internal to the plan template.

## Log

- 2026-05-06: opened from 195's closeout. Blocks-by 195 because the
  stub-lint must be in place before this ticket's "drop the allowlist
  entry" verification step makes sense.
