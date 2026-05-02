---
id: 081
title: Coordination directive-failure demotion (colony-side stuck-loop guard)
status: done
cluster: planning-substrate
added: 2026-04-29
parked: 2026-04-29
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: acb30b9d
landed-on: 2026-05-02
---

## Why

Audit gap #10 (minor → important once 072–076 land, since the directive-dispatch layer is the colony-side analog of per-cat plan-stasis). When the coordinator issues "build a kitchen" but materials are depleted, cats Build → fail → replan → coordinator re-assigns the same directive on its next tick → cats Build again → fail. The same stuck-loop pattern at the colony-coordination layer.

The per-cat 073 cooldown handles the cat side (the cat won't re-target the same blocked build site). But the coordinator keeps issuing the directive to *new* cats, who hit the same wall. Without a colony-side memory of cross-cat failure, the coordinator perpetually mis-assigns labor.

Parent: ticket 071. Blocked by 072 (`plan_substrate::lifecycle::record_step_failure`) and 073 (`RecentTargetFailures` aggregate readable as a sensor).

## Substrate-over-override pattern

Part of the substrate-over-override thread (see [093](093-substrate-over-override-epic.md)).

**Hack shape**: coordinator re-issues the same directive every tick even after repeated cross-cat failures (Build → fail → reassign → Build → fail). No colony-side memory of cross-cat failure means the coordinator perpetually mis-assigns labor. Currently *no* override exists in code — but adding one (this ticket's demotion logic) without substrate framing would just be a coordinator-side hack patched on top of a substrate-side bug.

**IAUS lever**: this ticket *is* the substrate-side fix. `DirectiveFailureLedger` is **colony-level failure memory as a first-class IAUS axis**, expressed as a Modifier in §3.5.1's pipeline that demotes via `RecentTargetFailures` aggregate. No out-of-band override; the coordinator's score economy reads the ledger like any other consideration.

**Sequencing**: blocked-by 072 (`plan_substrate::lifecycle::record_step_failure`) and 073 (`RecentTargetFailures` aggregate readable as a sensor). Cleanest unpark when the colony-side directive-loop pattern is observed in soak after the rest of [071](071-planning-substrate-hardening.md) lands.

**Canonical exemplar**: 087 (CriticalHealth interrupt → `pain_level` + `body_distress_composite` axes, landed at fc4e1ab).

## Scope

- Read aggregate `RecentTargetFailures` per directive in `coordination::accumulate_build_pressure` (`src/systems/coordination.rs:788–862`) and the dispatch site at `dispatch_urgent_directives` (line ~862). Compute the cross-cat failure count for the directive's action+target pair.
- When the aggregate failure count exceeds `directive_failure_threshold`, **demote** the directive: reduce its priority for `directive_failure_demotion_ticks`. Cats stop being preferentially assigned to it; alternative directives surface.
- After the demotion window, restore default priority (the directive can be retried; if conditions still block, it demotes again).
- New `DirectiveFailureLedger` resource — tracks `(DirectiveId) → (failure_count, last_demote_tick)`. Pruned on directive completion or coordinator restart. (Or extend an existing coordinator resource if one fits the shape.)
- New `CoordinationConstants` knobs: `directive_failure_threshold: u32` (default 5 cross-cat failures), `directive_failure_demotion_ticks: u64` (default ~4000 ≈ 1 sim-hour).
- New `Feature::DirectiveDemoted` (Neutral) fires when the demotion triggers.

## Out of scope

- Adding a shared "we tried this and it didn't work" narrative event for the colony — that's narrative-layer concern, not coordination-layer.
- Auto-resolution of the underlying blocker (e.g., automatically dispatching a forage directive when materials are depleted) — that's higher-level coordinator policy; this ticket only adds demotion.
- Tuning the threshold / window — pick conservative defaults; tune via post-landing soak.

## Approach

Files:

- `src/systems/coordination.rs` — extend `accumulate_build_pressure` (line ~788) and `dispatch_urgent_directives` (line ~862) to consult the ledger; add demotion logic. The priority reduction multiplier is a constant (e.g., ×0.1) similar to 073's cooldown shape.
- `src/resources/coordination.rs` (or wherever directive state lives — find via grep on `Coordinator` / `Directive`) — add `DirectiveFailureLedger` resource. `HashMap<DirectiveId, (u32 failure_count, u64 last_demote_tick)>`.
- `src/resources/sim_constants.rs::CoordinationConstants` — add `directive_failure_threshold: u32` and `directive_failure_demotion_ticks: u64`.
- `src/resources/system_activation.rs` — `Feature::DirectiveDemoted` (Neutral); exempt from `expected_to_fire_per_soak()`.
- `src/systems/plan_substrate/lifecycle.rs::record_step_failure` — on Build/Harvest/etc. failure with a directive-tagged target, increment the ledger. The hook is via the cat's plan carrying a `directive_id` field if it does today; otherwise via the directive resource's mapping of `(action, target) → directive_id`.

## Verification

- `just check && just test` green.
- Unit test: the ledger increments on directive-tagged failure, and the directive's priority drops by the configured factor once the threshold is hit.
- Unit test: the ledger expires entries past `last_demote_tick + directive_failure_demotion_ticks`.
- Synthetic-world integration test: 5 cats each fail Build on the same materials-depleted directive → the 6th cat is assigned a different directive; after the demotion window expires, the directive becomes assignable again.
- `just soak 42 && just verdict logs/tuned-42-081` — hard gates pass.

## Log

- 2026-04-29: Opened under sub-epic 071.
- 2026-04-29: Parked. Sub-epic 071 explicitly notes "Tickets 080 and 081 are important hardening but not blocking for 027b reactivation — the seed-42 stuck-loop pattern doesn't involve resource contention or coordination drift specifically. They strengthen the substrate against future failure modes." This ticket touches load-bearing `coordination.rs`; cleanest to land via a focused review pass after Wave 4's 027b soak validates the rest of the substrate. Unpark when the colony-side directive-loop pattern is observed in a soak (or as scheduled cleanup once Wave 4 has stabilized).
- 2026-05-02: **Retired without implementation.** Two reasons compose. **(a) No soak evidence of the pattern.** The most recent canonical seed-42 deep-soak (`logs/tuned-42-baseline-0783194`, commit `9945e59`, full 1.34M-tick run) shows 34,645 `DirectiveIssued` events split across `SetWard` 11860 / `Patrol` 11540 / `Cleanse` 6334 / `Fight` 4366 / `Herbcraft` 392 / `Hunt` 68 / `Forage` 60 / `HarvestCarcass` 25 — **zero `Build` or `Construct` directives**. The motivating "kitchen-build with depleted materials" scenario isn't reaching dispatch in seed-42 at all; `Construct` shows up exactly once in `PlanStepFailed` over the whole run. No `(kind, target)` directive pair recurs ≥ 3 times in 34,645 issuances, and directives don't currently emit a stable target identifier in the event log — so cross-cat directive cycling isn't even observable yet, let alone occurring. The high-failure actions (`EngagePrey` 4590, `ForageItem` 1846) are cat-layer step failures already covered by 073's `RecentTargetFailures` cooldown at the substrate layer. **(b) Internal framing tension.** The Why section frames the fix as "a Modifier in §3.5.1's pipeline" with 087 as exemplar (substrate-over-override doctrine). The Approach section describes coordinator-side ledger bookkeeping inside `dispatch_urgent_directives`. The coordinator has no IAUS / Modifier pipeline today and §3.5.1 is silent on coordinator-layer scoring — implementing the Why would mean inventing the first coordinator-layer Considerations pipeline (substantial out-of-scope work), and implementing the Approach is override-shaped under 093's lens. Neither is a clean substrate fix for a problem that hasn't appeared. **Re-open contract:** if a future seed surfaces directive-loop pathology, open a fresh ticket framed around the *specific* substrate gap that surfaces (most likely "directive identity in the event log" + a coordinator-layer Considerations slot, or a `RecentTargetFailures`-equivalent at the coordinator scope) rather than reviving this one. Note: 126 (`bdi-intention-substrate`) and 130 (`trust-weighted-coordinator-momentum`) reference 081 in their bodies as a consumer of 126's `IntentionSource` provenance and as a parallel demotion signal alongside 130's trust calibration; that link dissolves with this retirement. 130's frontmatter `blocked-by` drops 081 in the same commit.
