---
id: 357
title: HTN-driven action dispatch — DSE / GoapActionKind / plan template for mourn_at_grave and rear_kitten primitives
status: ready
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
landed-at: null
landed-on: null
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
