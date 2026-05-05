---
id: 160
title: Substrate stub catalogue + lint — fail the build on orphan markers
status: done
cluster: ai-substrate
added: 2026-05-04
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 37348f21
landed-on: 2026-05-05
---

## Why

Ticket 158 was misdiagnosed for one round-trip because the
`IsParentOfHungryKitten` marker had been spec'd in
`docs/systems/ai-substrate-refactor.md` §4.3 with author function
`tick:growth.rs::update_parent_hungry_kitten_markers (new)` for ~1
year — but the author function was never written and nothing read the
marker. Grep confirms zero readers across `src/`. The marker sat as
orphan substrate for that entire window.

The `(new)` annotation in the §4.3 spec is the only signal that an
item is unimplemented, and the substrate-doc has no automated check
that diffs the spec against actual src/. So unimplemented substrate
accumulates silently. CLAUDE.md's "substrate-over-override" doctrine
explicitly depends on substrate being *actually wired*, not just
declared.

The current partial inventory:

- **`docs/open-work/pre-existing/dead-features-in-activation-tracker.md`**
  — covers `Feature::*` enum dead variants only (3 features:
  `FoxDenEstablished`, `FoxDenDefense`, `CombatResolved`).
- **`docs/systems/ai-substrate-refactor.md` §4.3 marker table**
  (lines ~1900-2200) — *spec* catalogue of ~19 markers. Author
  string carries `(new)` for unimplemented; no automated audit.
- **No cross-type catalogue** for unread Components / orphan
  Markers / unfired Features / orphan Resources / undefined plan
  templates / dangling `MessageWriter` registrations.

## Scope

Two deliverables:

1. **`scripts/check_substrate_stubs.sh`** — mirrors
   `scripts/check_step_contracts.sh` (referenced in CLAUDE.md and
   wired into `just check`). For every type declared in
   `src/components/markers.rs` with `#[derive(Component)]`, grep
   `src/` for readers. A marker has zero readers iff:
   - No `Has<markers::X>` in any Query<>.
   - No `MarkerSnapshot.has(X::KEY, …)` call.
   - No `Q<_, With<X>>` filter clause.
   - No `EligibilityFilter::require(X::KEY)`.
   - No `commands.entity(_).insert(X)` outside the marker module
     itself (write-only-then-orphan also counts as a stub).

   Fail the build (`exit 1`) when any marker is orphan and not
   explicitly excluded by an allowlist of "known orphans pending
   work" with their corresponding ticket id.

   Add to `just check` after the existing step-resolver +
   time-units lints.

2. **`docs/open-work/pre-existing/substrate-stub-catalogue.md`** —
   single index file listing all current orphan substrate items,
   replacing the per-category `dead-features-in-activation-tracker.md`.
   Each entry: type / location / spec ref / status / "ticket to
   wire it". Initial population by running the new lint and
   capturing the failure list.

## Out of scope

- Implementing the unwired substrate items themselves. Each gets
  its own ticket once catalogued. 160 only catalogues and
  fails-fast on new occurrences.
- Coverage for non-marker substrate (Components, Resources,
  Messages, plan-template stubs). Those are conceptually similar
  but each has different "what counts as a reader" semantics —
  defer to follow-on tickets per category once the marker shape
  proves out.

## Verification

1. **Run the new lint locally** before this ticket lands. The list
   of orphan markers populates the initial catalogue.
2. **Negative test**: temporarily add a test marker without any
   reader in a branch, run `just check`, confirm it fails with a
   specific error pointing at the marker name + file location.
3. **Allowlist round-trip**: take one orphan from the initial list
   (e.g., `IsParentOfHungryKitten` would have been here pre-158),
   add it to the allowlist with its ticket id, confirm `just check`
   passes again.
4. **CLAUDE.md update**: add a paragraph under the "Conventions" or
   "Bugfix discipline" section pointing at the catalogue and
   stating that any new marker MUST land with at least one reader
   in the same commit, or with an allowlist entry naming the
   ticket that wires it.

## Log

- 2026-05-04: opened in the same commit that lands ticket 158.
  Case-in-point is `IsParentOfHungryKitten` — the marker that 158's
  fix wires up. Without this lint, the next dead marker might cost
  another full-soak round-trip to detect.
- 2026-05-05: landed. Initial catalogue captures 4 orphans:
  `ColonyState` (fully-orphan, ticket 168), `HasConstructionSite` +
  `HasDamagedBuilding` (fully-orphan, ticket 169), `HideEligible`
  (read-only, ticket 170). Three follow-on tickets opened in same
  commit per CLAUDE.md "antipattern follow-ups are non-optional"
  rule. Lint passes `just check` with all 4 allowlisted; verified
  via 7-test suite (allowlist round-trip, synthetic LintCanary,
  IsParentOfHungryKitten regression-safety, Audit 2 production-code
  injection). Discovery: `MarkerConsideration::new` has zero
  production callsites today — Audit 2 is forward-looking until a
  caller adopts it.
