---
id: 188
title: Handoff target-picker — pick the recipient cat at L2
status: done
cluster: ai-substrate
added: 2026-05-06
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-06
---

## Why

Ticket 177 wired the Handoff dispatch arm but explicitly parked
recipient-picking — for stage-1 wiring, the handoff target is
threaded as `target_entity` from whatever produces the
`DispositionKind::Handing`, which today is **nothing** (the
Handing DSE ships default-zero, no L2 target picker exists).

Once 178 lifts disposal DSE weights and Handing actually fires,
the dispatch arm will Fail with `"handoff: no recipient on
disposition"` on every attempt — there's no system populating
`target_entity` for Handing dispositions. The L2 target DSE
needs to pick a recipient (proximity? hunger gradient?
fondness? leader-coordinator?) and thread the choice into
`Disposition::target_entity` the way `groom_other_target` and
`socialize_target` already do for their respective dispositions.

Parked rather than ready because:
1. 178 must land first — without disposal DSE weights, this
   ticket can't be exercised at runtime.
2. The "right" recipient policy is a balance question, not a
   wiring question. Defer to balance work after 178 surfaces
   the actual handoff-rate signal.

## Direction (sketch)

- Add a `handing_target_dse` mirroring
  `src/ai/dses/groom_other_target.rs` — scores candidate
  recipient cats by proximity + hunger + fondness or some
  combination grounded in observable substrate.
- Wire it into `populate_dse_registry` in
  `src/plugins/simulation.rs`.
- Hook the picker into the Handing disposition planning path
  the same way `resolve_groom_other_target` is called from
  `goap.rs` around the `GroomOther` arm.

## Out of scope

- The recipient-picking policy itself (balance, not structural).
- Multi-recipient broadcast / coordinator-directed handoff.

## Resolution

Wave-closeout step 3 of 3. Substrate landed via the simpler
**inline-fallback + colony-marker** shape (the L2 target-picker
DSE is deferred to balance follow-on 192).

**What shipped:**

- `HasHandoffRecipient` re-classified from per-cat to
  **colony-scoped** (markers.rs doc + authoring site). Authored
  by `update_colony_building_markers` (`src/systems/buildings.rs`)
  on the simple predicate "≥1 living `Kitten` exists in the
  colony" — adults give to kittens. Threaded through
  `colony_state_query` in both `goap.rs` and `disposition.rs`
  into `MarkerSnapshot` via `set_colony`.
- `HandingDse` curve lifted from `Linear { slope: 0.0,
  intercept: 0.0 }` to the same Logistic on `inventory_excess`
  that 178's Discarding/Trashing use (steepness/midpoint from
  `disposal_inventory_excess_*` constants). Symmetry across the
  three sibling DSEs: a cat with stuffed inventory has parallel
  disposal options keyed by which colony substrate is set
  (Midden → Trashing, ChronicallyFull → Discarding, Kitten →
  Handing). Eligibility: `forbid(Incapacitated) ∧
  require(HasHandoffRecipient)`.
- `goap.rs::HandoffItem` dispatch arm gains an inline
  recipient-resolution fallback mirroring
  `TrashItemAtMidden`'s nearest-Midden pattern: when
  `target_entity.is_none()`, pick the kitten with the lowest
  hunger satisfaction (tie-break by manhattan distance from the
  acting cat). Uses `snaps.kitten_snapshot` already populated
  per-tick for the FeedKitten resolver.
- Allowlist entry `HasHandoffRecipient 188` retired in
  `scripts/substrate_stubs.allowlist`. The wave is now lint-
  clean across all three landings.
- `handing_dse(_)` signature changed to take `&ScoringConstants`
  (mirrors `discarding_dse` / `trashing_dse`). Single call site
  in `populate_dse_registry` updated.
- New unit tests on `handing.rs`:
  `handing_curve_lifts_with_inventory_excess` (curve shape),
  `handing_eligibility_requires_handoff_recipient`.

**Scope narrowing — `handing_target_dse` deferred:**

The original "Direction" called for an L2 target-picker DSE
mirroring `groom_other_target.rs` / `socialize_target.rs` to
score multi-axis recipient-suitability (proximity + hunger +
fondness). That shape is structurally the right answer for the
mature substrate, but it's ~150 lines of code + plumbing for
a feature whose minimum viable form is one nearest-hungry-
kitten lookup. The wave's purpose is to stop the substrate
jostling (per ticket 189's reframe); shipping the inline form
clears the allowlist and lets future regression triage measure
against firing substrate. The L2 picker becomes follow-on 192.

## Land-day follow-on

- **192** — `handing_target_dse` (L2 target picker over
  proximity + hunger + fondness + bond-weighted axes). Mirror
  `groom_other_target.rs`. Hook into goap.rs Handing arm before
  the inline fallback (so picker wins when registered, fallback
  catches the unregistered case for tests / scenarios).

## Log

- 2026-05-06: opened by 177's closeout. 177 explicitly parked
  this scope ("for stage-1 wiring the handoff target can be
  threaded as `target_entity` from the disposition layer";
  no such layer exists today). Becomes load-bearing as soon as
  178 lifts the Handing DSE above default-zero.
- 2026-05-06: unparked. Ticket 178 landed; the Handing DSE
  eligibility filter now requires `HasHandoffRecipient`
  (allowlisted in `scripts/substrate_stubs.allowlist`). This
  ticket lands the marker writer alongside the curve lift.
  Removing the allowlist entry is a same-commit step on land
  day.
- 2026-05-06: landed (wave-closeout step 3 of 3). Substrate
  shipped via inline-fallback + colony-marker shape; L2 target
  picker deferred to ticket 192. Wave-closeout complete; the
  disposal-substrate migration is now firing-load-bearing
  rather than phantom-load-bearing on the schedule edge. Plan
  at `~/.claude/plans/i-just-finished-a-compiled-hanrahan.md`.
