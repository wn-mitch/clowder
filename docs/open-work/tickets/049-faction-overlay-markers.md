---
id: 049
title: §9.2 faction overlay markers — Visitor / HostileVisitor / Banished / BefriendedAlly
status: ready
cluster: null
added: 2026-04-27
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, trade.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

The §4 marker catalog large-fill (ticket 014, landed 2026-04-27) closed
all §4.3 markers except the four §9.2 faction overlay ZSTs:
`Visitor` / `HostileVisitor` / `Banished` / `BefriendedAlly`. These
already exist in `src/components/markers.rs` but have no author systems
and no consumers in faction-stance resolution. They're a separate
cross-cutting cluster from the §4.3 trait/state/inventory markers
because they require faction-stance demotion infrastructure.

## Scope

Author systems + consumer wiring for the four faction overlay markers:

- **Visitor** (Wandering Loner / Trader / Scout per
  `docs/systems/trade.md`) — non-colony cat present on the map.
  Observer-Cat × target-Cat: demote `Same` → `Neutral`.
- **HostileVisitor** — hostile-loner variant. Observer-Cat × target-Cat:
  demote `Same` → `Enemy`.
- **Banished** — cat exiled from the colony. Observer-Cat × target-Cat:
  demote `Same` → `Enemy`. Today's `combat.rs::pending_banishments`
  path is shadowfox-only; extending to cat-on-cat is in scope here.
- **BefriendedAlly** — fox or prey-species target befriended through
  repeated non-hostile contact. Observer-Cat × target-Fox: upgrade
  `Predator` → `Ally`; reciprocal on fox: `Prey` → `Ally`.
  Authoring lives at `social.rs::befriend_wildlife`.

Each marker needs:
1. Author system per the marker rustdoc (or new author for cat-on-cat
   banishment / non-colony visitor spawn).
2. Snapshot population in both scoring loops (mirror the §4.3 marker
   pattern from ticket 014).
3. Consumer wiring in faction-stance resolution code (where
   `Faction::Same` / `Faction::Predator` etc. is decided per-pair).

## Out of scope

- Trade subsystem implementation. Visitor / HostileVisitor reference
  `docs/systems/trade.md` but trade infrastructure is its own track.
- Cross-species befriending mechanics (just-fired event flow); only
  the marker + stance-demotion is in scope here.

## Approach

Read `docs/systems/ai-substrate-refactor.md` §9.2 for the demotion
matrix. The implementing PR follows the §4.3 marker pattern set by
ticket 014: author per-tick, populate snapshot, retire any inline
faction-stance fallback paths.

Suggested chunking:
1. **Visitor + HostileVisitor**: needs a non-colony-cat spawn pathway
   (likely tied to trade subsystem); without that, both markers stay
   theoretical. Park until trade lands, OR define a Visitor spawn
   shim for testing.
2. **Banished** (cat-on-cat): combat.rs already has shadowfox banishment;
   extend to cat exile pipeline. Author the marker on banished cats.
3. **BefriendedAlly**: simplest — add author + befriending threshold to
   `social.rs`. Visit-counter or repeated-non-hostile-contact
   accumulator on the relationship.

## Verification

- Lib tests: each new author + consumer with insert/remove/edge-case
  coverage (~6 tests per marker).
- Soak verdict on canonical seed-42 deep soak: faction-stance
  demotion shouldn't change anything in a colony with no visitors,
  exiles, or befriended wildlife in the soak window. Behavior-neutral.
- If a soak surfaces non-trivial behavior changes, document the
  hypothesis per CLAUDE.md balance methodology.

## Log
- 2026-04-27: opened from ticket 014 closeout (§4 marker catalog
  large-fill). Faction overlay was explicitly out of scope for the
  §4.3 marker fill-in.
