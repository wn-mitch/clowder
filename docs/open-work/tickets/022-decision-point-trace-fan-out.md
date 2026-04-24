---
id: 022
title: §11 decision-point trace fan-out — remaining target-taking DSEs + deep-soak review
status: in-progress
cluster: null
added: 2026-04-23
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

**Status:** infrastructure landed this session (trace-log schema,
Feature split, commitment + plan-failure capture, ineligibility
capture, `socialize_target` hook wiring). Seven target-taking DSEs
still lack per-candidate ranking capture and the seed-42 focal soak
has not been reviewed beyond a "does it run, do the new rows appear"
sanity check.

**Remaining work:**

- Wire the `FocalTargetHook` (or the `target_ranking_from_scored`
  helper in `src/ai/target_dse.rs`) through the remaining seven
  target-taking DSE resolvers: `mate_target`, `mentor_target`,
  `groom_other_target`, `apply_remedy_target`, `build_target`,
  `caretake_target`, `hunt_target`, `fight_target`. Each needs:
  - `resolve_*_target` signature gains an `Option<FocalTargetHook>`
    trailing param (mirror `socialize_target.rs:152`).
  - Every caller in `goap.rs` / `disposition.rs` passes either
    `None` or a focal-hook built from `ec.focal_capture` +
    `ec_is_focal` + an `Entity → String` name lookup.
  - Test call sites (local `evaluate_target_taking` unit tests
    aren't affected; only `resolve_*_target` tests in each file).

- Review the seed-42 deep-soak focal trace that the wiring session
  produces. Specifically:
  - `jq 'select(.layer == "L3Commitment")' logs/tuned-42/trace-Simba.jsonl`
    should show `branch: "achieved"` rows for every disposition
    Simba completes (Resting, Hunting, Foraging, etc.) plus
    `branch: "unachievable"` rows on any replan-cap hit.
  - `jq 'select(.layer == "L3PlanFailure")' logs/tuned-42/trace-Simba.jsonl`
    should show `reason: "anxiety_interrupt"` rows if Simba takes
    a CriticalHealth hit.
  - `events.jsonl` footer: `Feature::CommitmentDropBlind` /
    `…SingleMinded` / `…OpenMinded` / `…ReplanCap` counters each
    should have non-zero counts on a 15-min seed-42 soak. A zero on
    any of them is a canary for either the capture not firing or
    the disposition mix not exercising that strategy.
  - Confirm no regression in existing survival canaries (Starvation
    == 0, ShadowFoxAmbush ≤ 5, footer-written, never-fired-expected
    == 0) or continuity canaries.

- The Phase 6a commitment gate landed (2026-04-24). The prologue
  gate's `should_drop_intention` call path does not emit focal-trace
  records (LLVM cliff — see
  `docs/systems/phase-6a-commitment-gate-attempt.md`). Focal-trace
  coverage remains at the existing `disposition_complete` and
  `max_replans_exceeded` cold-path sites. `retained: true` rows
  (plans the gate evaluated but kept) are still invisible by design
  — adding per-tick retained-trace capture to the prologue would
  re-trigger the optimization cliff.

**Why it matters:** without the remaining 7 hooks, a focal-cat
soak on a priestess, a mentor, a hunter etc. won't show per-target
rankings for their signature activities. A socialize-only hook
covers Simba but misses the §6.5 coverage matrix described in
CLAUDE.md ("multi-focal runs against different cats").

**Resume when:** next session that opens `trace-<focal>.jsonl` and
wants the `targets` block on a non-socialize L2 record, or any
session that re-attempts Phase 6a integration.
