---
id: PE-003
title: Substrate stub catalogue — orphan markers + invalid consideration string-name references
status: in-progress
cluster: ai-substrate
added: 2026-05-05
parked: null
priority: medium
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why this exists

Ticket 158 cost a full-soak round-trip because `IsParentOfHungryKitten`
had been spec'd in `docs/systems/ai-substrate-refactor.md` §4.3 with author
function `tick:growth.rs::update_parent_hungry_kitten_markers (new)` for
~1 year — but the author function was never written and nothing read the
marker. The `(new)` annotation in the spec was the only signal the marker
was unimplemented; substrate-over-override discipline depends on substrate
being *actually wired*, not just declared.

Ticket 160 added `scripts/check_substrate_stubs.sh` — a grep-based lint
wired into `just check` that runs two audits:

- **Audit 1** — every marker declared in `src/components/markers.rs` must
  have at least one reader (`Has<>` / `With<>` / `KEY` reference) AND at
  least one writer (`.insert(X)` / `.remove::<X>()` / `MarkerSnapshot::set_entity` /
  `MarkerSnapshot::set_colony` / `toggle()` helper). Markers missing
  either side are classified `fully-orphan` (neither), `read-only`
  (read but never written), or `write-only` (written but never read).
- **Audit 2** — `MarkerConsideration::new(_, "<NAME>", _)` references a
  marker by string literal. Each such name must match a real marker
  enumerated by Audit 1. Catches typos and rename-without-update drift.
  (Forward-looking as of 2026-05-05: there are zero production callsites
  for `MarkerConsideration::new` today; the type is referenced only by
  unit tests. The audit exists so the moment a production caller adopts
  it, drift is caught.)

This file is the index of every orphan currently allowlisted in
`scripts/substrate_stubs.allowlist`. Each entry names the ticket that
will wire it. When a follow-on ticket lands, the corresponding allowlist
entry drops and the catalogue entry moves to a "## Resolved" section
(or the catalogue file is deleted entirely once the list empties).

## Active orphans

### `ColonyState` — fully-orphan

- **Type:** `pub struct ColonyState;` (no `KEY` constant)
- **Declaration:** `src/components/markers.rs:300`
- **Spec reference:** `docs/systems/ai-substrate-refactor.md` §4.3 lines
  ~1985–1995 (colony-singleton promotion); deferred-comment in
  `src/systems/goap.rs:913`; target query shape documented in
  `src/ai/scoring.rs:89-94`.
- **Failure mode:** never inserted, never queried. Substrate-refactor
  Phase 4b.2 promotes colony-scoped markers (`HasFunctionalKitchen`,
  `HasRawFoodInStores`, …) onto a `ColonyState` singleton entity; that
  promotion is unimplemented and the marker has no live use.
- **Wiring ticket:** [168](../tickets/168-colony-state-singleton-wiring.md)

### `HasConstructionSite` — fully-orphan

- **Type:** `pub struct HasConstructionSite;` (no `KEY` constant)
- **Declaration:** `src/components/markers.rs:381`
- **Spec reference:** `docs/systems/ai-substrate-refactor.md` §4.3 line
  1976. Author function: `tick:buildings.rs::update_colony_building_markers`.
- **Failure mode:** never authored, never queried, no `KEY` constant
  to lookup. The string-name `"HasConstructionSite"` appears at
  `src/ai/considerations.rs:417`, but that's inside a unit test for
  `MarkerConsideration::score()` — `MarkerConsideration::new` itself
  has no production callsites (see Audit 2 note above).
- **Wiring ticket:** [169](../tickets/169-author-construction-and-damaged-building-markers.md)

### `HasDamagedBuilding` — fully-orphan

- **Type:** `pub struct HasDamagedBuilding;` (no `KEY` constant)
- **Declaration:** `src/components/markers.rs:384`
- **Spec reference:** `docs/systems/ai-substrate-refactor.md` §4.3 line
  1977. Same author function as `HasConstructionSite`.
- **Failure mode:** never authored, never queried; only doc-comment
  references in `src/systems/buildings.rs:390` and
  `src/resources/sim_constants.rs:4445`.
- **Wiring ticket:** [169](../tickets/169-author-construction-and-damaged-building-markers.md)
  (single ticket — same author fn as `HasConstructionSite`)

### `HideEligible` — read-only

- **Type:** `pub struct HideEligible;` with `KEY = "HideEligible"`
- **Declaration:** `src/components/markers.rs:230`
- **Spec reference:** `src/ai/dses/hide.rs:8-19` (Phase-2 predicate
  spec); ticket 104 landed Phase 1 dormancy contract.
- **Reader:** `src/ai/dses/hide.rs:94` —
  `eligibility: EligibilityFilter::new().require(markers::HideEligible::KEY)`.
- **Failure mode:** the Hide DSE gates on this marker, but no system
  authors it — the DSE is dormant in production (score-bit-identical
  to baseline). 105's adrenaline-freeze modifier is similarly gated.
- **Wiring ticket:** [170](../tickets/170-hide-eligible-authoring-system.md)

## How to remove an entry

When a follow-on ticket lands wiring a marker:

1. Land the marker's author/reader code in the same commit as the
   ticket's `status: done` flip.
2. Drop the marker's line from `scripts/substrate_stubs.allowlist`.
3. Run `just check` — the lint should pass with one fewer
   allowlisted entry.
4. Move the catalogue entry from "## Active orphans" to a
   "## Resolved" section at the bottom (or delete the catalogue file
   entirely if no orphans remain).
5. The change to `markers.rs` and `substrate_stubs.allowlist` lands
   in the same commit as the wiring code, per CLAUDE.md
   §"Substrate stubs are forbidden".

## Sibling stub-trackers

- `dead-features-in-activation-tracker.md` (PE-002) — covers
  `Feature::*` enum dead variants (`FoxDenEstablished`, `FoxDenDefense`,
  `CombatResolved`). Different category — orphan *features* in the
  activation tracker, not orphan *markers*. The two catalogues are
  conceptually similar but their detection logic differs (Feature
  variants check `record_if_witnessed` call sites; markers check
  reader+writer pairs).

## Out of scope

Per ticket 160 §"Out of scope":

- Coverage for non-marker substrate (orphan Components / Resources /
  Messages / plan templates). Each category has different reader
  semantics; defer per-category extensions to follow-on tickets once
  the marker shape proves out.
- Implementing the unwired substrate items themselves — that's
  tickets 168 / 169 / 170.
