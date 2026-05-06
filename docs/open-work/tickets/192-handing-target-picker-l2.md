---
id: 192
title: handing_target_dse — L2 multi-axis recipient picker (188 follow-on)
status: ready
cluster: ai-substrate
added: 2026-05-06
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 188 wave-closed the disposal-substrate migration with an
inline recipient-resolution fallback in
`goap.rs::HandoffItem`: when `target_entity.is_none()`, pick
the kitten with the lowest hunger satisfaction (tie-break by
proximity). This is the minimum viable shape — it works, but
it bypasses the L2 target-picker DSE pattern that
`groom_other_target.rs` and `socialize_target.rs` use for
their respective dispositions.

The L2 pattern is the right structural answer for mature
substrate: a multi-axis scorer over proximity + recipient-
hunger + fondness (+ optionally bond-weighted compassion or
coordinator-directed) lets the picker reflect richer policy
than nearest-hungry. Notably the inline form picks the same
target every tick regardless of fondness or relationship —
fine for kitten-feeding (which is universal-altruism shaped),
less fine if Handing extends to mate-feeding or ally-care in
later iterations.

## Direction

- Add `src/ai/dses/handing_target.rs` mirroring
  `groom_other_target.rs`. Considerations:
  - **Recipient-hunger** — `kitten.hunger` axis (low hunger
    satisfaction → high score).
  - **Proximity** — manhattan distance from acting cat,
    Composite{Logistic, Invert}.
  - **Fondness** — `Relationships::fondness(actor, recipient)`,
    Linear; gives kin/bonded-pair preference.
- Register in `populate_dse_registry`
  (`src/plugins/simulation.rs`).
- Hook into `goap.rs` Handing planning path the same way
  `resolve_groom_other_target` is called from `GroomOther` arm.
  Picker writes `target_entity`; if unregistered, the existing
  inline fallback in HandoffItem dispatch resolves nearest-
  hungry kitten as before.
- Validate via `just hypothesize` four-artifact loop: hypothesis
  on whether fondness-weighted picking shifts handoff cadence
  vs nearest-hungry. Concordance check against post-wave baseline.

## Out of scope

- Mate-feeding / ally-care extension (separate ticket once the
  picker substrate is in place).
- Coordinator-directed broadcast handoff.
- Removing the inline fallback in goap.rs HandoffItem — keep it
  as the unit-test / scenario seam (the picker may not fire in
  every scenario harness).

## Verification

- Unit tests on the new picker: hungry-kitten preference,
  proximity preference, fondness preference.
- Integration: `just hypothesize` loop on the prediction that
  parents preferentially feed their own kittens vs adopting
  random hungry kittens.
- Survival hard-gates pass.

## Log

- 2026-05-06: opened by 188's land-day follow-on. The wave-
  closeout shipped the simplest viable handoff substrate
  (colony marker + Logistic curve + inline fallback resolver);
  the L2 target-picker DSE that scales to richer recipient
  policy is split out here per CLAUDE.md antipattern-migration
  discipline.
