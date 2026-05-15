---
id: 259
title: L1 to L3 activation visualization in log viewer
status: ready
cluster: tooling-diagnostics-ui
orchestration: substrate-sensitive
initiative: []
added: 2026-05-10
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

The substrate-refactor's L1 (markers / influence maps) → L2 (DSE consideration scoring) → L3 (softmax + plan election) is a layered activation pipeline that conceptually resembles a neural network. Today, drilling into a single tick's decision requires composing `just q trace` + `just q cat-timeline` + manual cross-referencing — the layered structure is implicit. A visual activation graph (click a tick, see which markers fire, which DSE axes activate, which Action wins L3) would shorten triage from minutes to seconds and make the substrate's behavior legible to non-author future-me sessions.

User mention: surfaced 2026-05-10 alongside the C3 spinout work as a parking-lot devex idea. Not load-bearing for any in-flight ticket, but earns its place per CLAUDE.md antipattern-migration discipline.

## Scope

- **Frame view**: click a `(cat, tick)` and see all L1 markers/maps that lit up (which the focal cat's sensing system read), all L2 DSE scores (per-axis with weights), and the L3 softmax distribution + winning Action.
- **Activation styling**: markers/axes with non-zero contribution rendered in a heatmap; zero-weight axes greyed out so the actual signal flow is obvious at a glance.
- **Source contributions**: hover an L2 axis, see which L1 markers fed it; hover an L3 entry, see the DSE scoring breakdown.
- **Tick navigation**: prev/next tick + jump-to-tick + jump-to-action-change; the activation graph re-renders.
- **Source data**: reads `events.jsonl` + `trace-<cat>.jsonl` sidecar (the L2 trace already records per-axis values; this ticket builds the *visualization*, not new logging).

## Out of scope

- New diagnostic logging (the L2 trace is already complete enough; if it's not, that's a separate fix per `project_l2_l3_disconnect_observation` memory and ticket 163).
- Cross-run / cross-cat comparison views (this is single-cat single-tick first; comparison is a future ticket).
- Real-time visualization during a running soak (post-hoc on saved logs only at v1).
- Editing / write-back into the log (read-only).

## Current state

- L2 trace already records per-axis values per `(cat, tick)` per DSE in the trace sidecar — no new logging needed.
- 9 bonus modifier layers in `goap.rs::evaluate_and_plan` mutate per-Action scores after L2 emit and aren't recorded (ticket 163 tracks rectification — memory `project_l2_l3_disconnect_observation`). v1 of this viz will show the L2 trace honestly + a "post-L2 modifier delta" row that displays the sum of bonus contributions even if the breakdown isn't visible. Full fidelity blocks on 163 landing.
- Existing log-viewer entry point: `just narrative-editor` (Writer's Toolkit). Reuse harness OR build dedicated `just activation-viewer` — design choice during implementation.

## Approach

1. Reuse the existing log-viewer harness if the substrate fits; otherwise scaffold a tiny web view that loads `events.jsonl` + `trace-*.jsonl` and renders a layered graph per `(cat, tick)`.
2. v1 = static activation graph for one focal cat one tick.
3. v2 = tick navigation + DSE breakdown hover.
4. v3 = jump-to-action-change.

If 163 (L2-vs-pool divergence rectification) lands first, the post-L2 modifier delta row becomes a per-modifier breakdown; if not, v1 shows the aggregate delta with a note pointing at 163.

## Verification

- Manual: load a known-interesting tick (e.g. an ambush-leading-to-Patrol-cascade tick from a soak) and verify the activation graph clearly shows the L1 markers / L2 axes / L3 winner.
- Cross-check: the rendered L3 winner matches `just q trace`'s reported winning Action for that `(cat, tick)`.
- Documentation: README or skill doc explains how to invoke + how to interpret the rendering.

## Log

- 2026-05-10: opened as parking-lot devex idea surfaced during C3 spinout planning (ticket 258). Independent of the named cluster but tracked here per CLAUDE.md antipattern-migration discipline.
