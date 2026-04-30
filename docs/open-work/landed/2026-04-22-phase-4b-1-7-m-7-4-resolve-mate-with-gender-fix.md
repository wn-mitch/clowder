---
id: 2026-04-22
title: "Phase 4b.1 — §7.M.7.4 `resolve_mate_with` gender fix"
status: done
cluster: null
landed-at: null
landed-on: 2026-04-22
---

# Phase 4b.1 — §7.M.7.4 `resolve_mate_with` gender fix

Spec §7.M.7.4 committed that `Pregnant` must land on the
gestation-capable partner, not the initiator. Today's code did the
opposite — a Tom initiator paired with a Queen produced a pregnant
Tom. Shipped:

- `Gender::can_gestate` — Queens and Nonbinaries gestate; Toms
  don't.
- `resolve_mate_with` now takes both genders, returns
  `Some((gestator, litter_size))`. Tom×Tom returns `None` (mating
  need clears so the step advances; no `Pregnant` insert, no
  `MatingOccurred` event). Ties resolve to the initiator per spec.
- Both callers (`systems/disposition.rs`, `systems/goap.rs`)
  snapshot gender alongside the existing grooming snapshot and
  insert `Pregnant` on the returned gestator. `Pregnant::partner`
  carries the other mate.

Six new unit tests cover the four gender permutations, pre-
threshold continuation, and hunger-driven litter-size bump.
