---
id: 2026-04-23
title: "§4.3 `Incapacitated` marker author — Track C prereq for §13.1 rows 1–3"
status: done
cluster: ai-substrate
landed-at: null
landed-on: 2026-04-23
---

# §4.3 `Incapacitated` marker author — Track C prereq for §13.1 rows 1–3

First per-cat (`set_entity`) marker to land in the §4 catalog;
prior ports were all colony-scoped (`set_colony`). Author-system
track only — the DSE consumer cutover
(`.forbid("Incapacitated")`) is intentionally deferred to the
§13.1 retirement commit per the gate table in #13.1.

**New `src/systems/incapacitation.rs::update_incapacitation`
system.** Tick system reading the same predicate
`ScoringContext.is_incapacitated` reads today (severe unhealed
injury on the cat's Health component); inserts the `Incapacitated`
ZST marker on matching cats, removes it on recovery. Registered
at all three mirror sites per CLAUDE.md's headless-mirror rule:
`SimulationPlugin::build` + both `build_schedule` paths in
`main.rs`.

**`MarkerSnapshot::set_entity("Incapacitated", cat, is_incap)`**
populated in both parallel builders (`systems/goap.rs` +
`systems/disposition.rs`) inside the existing per-cat iteration.
First consumer of the `set_entity` API that was present in
`MarkerSnapshot` since Phase 4b.2 but unused outside tests.

**Spec updates** (`docs/systems/ai-substrate-refactor.md`):

- §4.3 `Incapacitated` row: status `Absent → Author ✓; DSE consumer
  pending §13.1`; Insert column points at
  `tick:systems::incapacitation::update_incapacitation`.
- §4.6 authoring-system roster: `Incapacitated` moved out of
  `needs.rs` into dedicated `src/systems/incapacitation.rs` module.

**Tests.** Unit tests cover the predicate (severe-unhealed →
marker insert; recovery → marker remove) and the tick-system
idempotence (re-running on a steady state produces no churn).
Integration check: post-MarkerSnapshot-build,
`snapshot.has("Incapacitated", cat)` reports `true` for downed
cats and `false` for healthy ones.

**Deliberately NOT in this commit** (per kickoff prompt's
non-goals):

- No `.forbid("Incapacitated")` on any DSE. The inline
  `if ctx.is_incapacitated` branch at `scoring.rs:574–598` still
  runs; the marker is observable but not yet consumed by
  eligibility filters. That cutover lands with §13.1 rows 1–3 as
  a single behavior-change commit.
- No deletion of `ScoringContext.is_incapacitated`; other
  consumers read it.

**Verification.** `just check` + `just test` green. Seed-42 soak:
survival canaries hold within noise band. Marker is observable on
live cats but unconsumed — the commit is additive by design.

**Gate-table impact** (see #13 item 13.1's 6-row gate table):
Rows 1–3 flip Track C from `✗` to `½` (author ✓, cutover ✗);
Rows 4–6 unchanged (Track B axis migrations outstanding).
Overall `gate ✗` on every row remains — §13.1 cannot land as a
single commit until Track B axis migrations + the Track C
cutover commit both ship.

**Specification cross-ref:** `docs/systems/ai-substrate-refactor.md`
§4.3 (marker catalog), §4.6 (authoring-system roster).

---
