---
id: 357
title: HTN-driven action dispatch — DSE / GoapActionKind / plan template for mourn_at_grave and rear_kitten primitives
status: done
cluster: ai-substrate
orchestration: coherent-block
block: htn-method-composition
initiative: [smarter-cats, htn-method-composition]
added: 2026-05-15
parked: null
blocked-by: []
supersedes: []
related-systems: [htn-methods.md, ai-substrate-refactor.md]
related-balance: []
wires-method: [mourn_at_grave, rear_kitten]
landed-at: pending
landed-on: 2026-05-15
---

## Why

#332 and #333 landed the action vocabulary + substrate +
method-flip for `mourn_at_grave` and `rear_kitten`, but the
layer-walk surfaced a dispatch gap that both tickets named in
their landing Logs: the HTN substrate (`HeldGoalStack`) is
informational-only today. No system reads `frames[N].sub_goal_index`
to override `chosen_action` in the L2 evaluator — the cat does
whatever the per-tick DSE softmax picks, regardless of the method
frame on the stack. So even with both methods registered Live,
the primitive Actions (`Vigil` / `GriefSit` / `ReleaseGrief` /
`Wean` / `Teach` / `Release`) are never selected, and their
witness-typed resolvers never fire. This ticket closes that loop
for both methods in a single pass — they share the same gap and
the same shape of resolution.

## Scope

Per Action across the six primitives (`Vigil` / `GriefSit` /
`ReleaseGrief` / `Wean` / `Teach` / `Release`):

- New target-taking DSE under `src/ai/dses/`. For Vigil /
  GriefSit / ReleaseGrief: a Grave-target picker (§6.3 shape,
  parallels `caretake_target.rs`) keyed to
  `Mourning.deceased_name == Grave.deceased_name`. For Wean /
  Teach / Release: a dependent-kitten picker scored by
  `KittenDependency.maturity` (Wean targets pre-wean threshold,
  Teach targets post-wean / pre-teach, Release targets fully
  matured).
- `GoapActionKind::*` variant per primitive in
  `src/ai/planner/mod.rs`.
- Single-step plan template per primitive in
  `src/ai/planner/actions.rs` with the appropriate
  `StatePredicate` preconditions and `StateEffect` mutations.
- Resolver dispatch arm per primitive in `src/systems/goap.rs`
  (the existing `GoapActionKind::Cook => resolve_cook(...)`
  pattern).
- Refine the placeholder `applicable_when` predicate on
  `rear_kitten` from `cat_is_alive` to the precise reverse-lookup
  (any `KittenDependency.mother == Some(self)`). Implementation
  uses the dependent-kitten picker's lookup helper — the same
  reverse-lookup is needed for both the picker and
  `applicable_when`. The picker runs in an exclusive system
  (`&mut World`); `applicable_when` runs from `&World` and needs
  a sibling lookup path — author either a per-tick
  `KittenMotherIndex` resource updated by an exclusive system OR
  a `MotherOfDependent` marker Component maintained alongside
  `KittenDependency`. Pick at implementation time per the
  substrate-vs-search-state classifier.
- Emission path for `process_grief` and `kitten_reared`. Neither
  is an aspiration chain milestone today, so the existing
  `aspiration_picker` doesn't reach them. Two options surfaced
  during the #332/#333 layer-walk: (a) add a parallel
  reactive-emission system that walks `Mourning`-bearing cats
  and queens-with-dependents and writes `AspirationEmissions`
  rows directly, (b) extend the picker contract to consume a new
  `ReactiveEmit { applicable_when, label }` registry alongside
  aspiration milestones. (a) is simpler and additive; (b) is
  more uniform with the existing §H model. Pick at implementation
  time; the design memo lives in the ticket Log when settled.
- Promote `Feature::*` `expected_to_fire_per_soak()` to `true`
  for those that *should* fire in a healthy soak (`VigilHeld`,
  `KittenWeaned`, etc.) once the cutover soak observes them
  firing reliably. The Mourning-insertion path is still the
  §7.7.b debt (out of scope here; see below) — without it, only
  the rear_kitten primitives' Features can credibly promote
  immediately, since `KittenDependency` is already authored on
  birth.

## Out of scope

- **§7.7.b grief-event-emission debt** — the insertion path for
  `Mourning` when a colony-mate dies. Tracked under 060 Phase 6b
  as an independent epic. Without it, the `mourn_at_grave` chain
  is structurally Live but has no triggering event. This ticket
  ships the dispatch *machinery*; the §7.7.b debt ships the
  triggering substrate.
- Balance tuning for vigil / grief-sit / wean / teach / release
  durations and yields (balance-thread work; deferred until the
  AI substrate refactor stabilizes per CLAUDE.md).
- Father / partner involvement in rearing (#333's §Out of scope).
- Per-kitten grief emission on death (#333's §Out of scope).

## Current state

#332 landed 2026-05-15 (substrate: `Mourning` Component,
`Action::ReleaseGrief`, `TargetHint::Grave`, witness-typed Vigil
/ GriefSit / ReleaseGrief resolvers; method flipped to Live
gated on `Has<Mourning>`). #333 landed 2026-05-15 (substrate:
`Action::{Wean,Teach,Release}` resolvers upgraded to witness
types; `TargetHint::DependentKitten`; method flipped to Live
gated on `cat_is_alive` placeholder pending reverse-lookup
authoring here). Six new `Feature::*` variants registered with
`expected_to_fire_per_soak() => false` pending this dispatch
follow-on.

Verdict is anchored at #128 (`verdict-anchor: true`); this
ticket lands as a coherent-block intermediate (verdict-skipped)
per the orchestration discipline.

## Approach

Per htn-methods.md §G Tier-2 and the dispatch-gap analysis in
#332's and #333's landing Logs. The DSE / GoapActionKind / plan
template / dispatch trio mirrors the `Action::Bury` pattern
(§Pattern B single-action plan template, mirrors Mentor /
GroomOther shape). The reactive-emission path is the load-bearing
design decision — either (a) sidecar system or (b) extending the
picker contract — settle it in a §Approach update once the
implementation surfaces the trade-offs.

## Verification

- `cargo check --all-targets` passes.
- `just check` passes.
- `just soak-trace 42 <queen-with-kitten>` shows the `rear_kitten`
  method frame on the stack; Feature counts for `KittenWeaned`
  non-zero (rear_kitten can fire immediately on existing
  KittenDependency cats; mourn_at_grave still needs the §7.7.b
  emission ticket to fire).
- `just verdict logs/tuned-42` shows no regression on
  generational-continuity canaries.

## Log

- 2026-05-15: opened as the consolidated dispatch follow-on for
  #332 + #333. Both parents named this ticket's gap in their
  landing Logs; this ticket inherits their named structural
  candidates.
- 2026-05-15 (planning session, plan at
  `~/.claude/plans/work-357-purrfect-flask.md`):
  Planning surfaced that §Current state was inaccurate — #332 / #333
  shipped only paperwork; the substrate work (`Mourning` Component,
  `Action::ReleaseGrief`, `TargetHint::{Grave, DependentKitten}`,
  witness-typed Vigil / GriefSit / Wean / Teach / Release resolvers,
  Live-method flips) was orphaned twice by `scripts/session_done.sh`
  forgetting unpushed bookmarks. Forensic root cause: see #362.
  Substrate recovered via `jj restore --from 00aa3636` and landed at
  `4c211d5b` ("fix: 332/333 substrate recovery (orphaned during
  initial polecat landing — see 362)"). Tickets #362 (workflow bug
  fix) and #363 (track-enforcement gap follow-on) opened + #362
  landed alongside. Substrate state now matches the §Current state
  claim in this ticket.
- 2026-05-15: layered `rear_kitten` gate refinement on top — gate
  flipped from `cat_is_alive` placeholder to `has_dependent_kitten`,
  using the existing `Parent` marker (`src/components/markers.rs:620`,
  authored by `update_parent_markers` in `growth.rs:294`). Plan's
  D3 (`MotherOfDependent` marker) RETIRED in favor of reusing
  `Parent` — same predicate, single marker, mother-only filtering
  moves to the dependent-kitten target picker per #333 §Out of scope.
- **NEXT** (continuation session): D1 dispatch closure (adoption +
  advance hooks in `evaluate_and_plan` + `resolve_goap_plans`), D2
  reactive emission (`ReactiveEmit` registry in
  `aspiration_picker.rs`), D5 two consolidated target pickers
  (`grave_target.rs` + `dependent_kitten_target.rs`), three new
  `GoapActionKind` variants + plan templates + dispatch arms for
  Wean / Teach / Release (mourn arc deferred — its writer is the
  out-of-scope §7.7.b emission), D6 Feature promotion (flip
  `KittenWeaned`/`Taught`/`Released` to `true`). Plan file
  `~/.claude/plans/work-357-purrfect-flask.md` carries the full
  file-by-file list.
