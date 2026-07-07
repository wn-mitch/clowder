---
id: 514
title: mentor churn: mentor_target elects Incapacitated mentees the step-entry alive-gate then rejects — 1111 MentorCat plan failures per soak on incapacitation-heavy trajectories
status: ready
cluster: planning-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-07-07
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: [fluid-movement-phase2]
landed-at: null
landed-on: null
---

## Why
Election and execution disagree on an eligibility axis, and the
disagreement burns a replan per tick of disagreement. The
`mentor_target` DSE's eligibility is the shared
`require_alive_and_unreserved_filter()` (alive + unreserved ONLY —
`src/ai/dses/mentor_target.rs:161`), so an `Incapacitated` cat is a
first-class mentee candidate; but the generic step-entry alive-gate
(`goap.rs:6005-6027` → `validate_target_for_step`) rejects
Incapacitated targets, failing the plan before the resolver runs.
When a trajectory holds a long incapacitation window, mentors churn:
elect → author plan → step-entry reject → replan → re-elect the same
mentee. `plan_failures_by_reason["MentorCat: target invalid at step
entry: Incapacitated"]`: promoted baseline (3e4f7caf) = 2; step-11
gate run (44d3ecfb) = 0; step-12 iter-4 (b26f5407) = **434**; 467
gate run (861c9fe5) = **1111** (694× baseline rate — tripped the
verdict plan-failure canary). Predates 467; magnitude tracks how much
predator-contact injury the trajectory produces, and Phase II's
hunting-economy lift makes such trajectories the norm.

Same defect shape as 467 (fish elected that execution can't reach)
and the same fix precedent as the actor side: the self-state
`MentorDse` already `.forbid(Incapacitated)`s the MENTOR
(`src/ai/dses/mentor.rs:80`); the mentee side never got the matching
forbid.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Target election | `src/ai/dses/mentor_target.rs:161` | eligibility = `require_alive_and_unreserved_filter()` — no Incapacitated forbid on the mentee | `[verified-correct]` (read this session) |
| Actor election | `src/ai/dses/mentor.rs:80` | MentorDse `.forbid(markers::Incapacitated::KEY)` on the actor — the precedent shape | `[verified-correct]` |
| Step-entry gate | `src/systems/goap.rs:6005-6027` | generic alive-gate rejects Incapacitated targets for all steps except Bury/FeedKitten carve-outs | `[verified-correct]` |
| Churn accounting | footer `plan_failures_by_reason` | 434 (b26f5407) → 1111 (861c9fe5) vs 2 baseline; canary band `high-rate-ratio` | `[verified-observed]` |
| Cooldown layer | 073 `RecentTargetFailures` | failures DO feed the target cooldown, so the churn rotates/re-elects on cooldown expiry rather than hard-locking — masks, doesn't fix (same masking observed on 467 fish) | `[suspect]` — confirm re-election cadence from a trace before trusting |

## Fix candidates

**Parameter-level:** none apply — no knob expresses "mentee must be
conscious".

**Structural options:**
- R1 (**target-side forbid, recommended**) — add
  `.forbid_target(markers::Incapacitated::KEY)` (or the
  EligibilityFilter equivalent) to `mentor_target`'s eligibility so
  election agrees with the step-entry gate. Mirrors mentor.rs:80.
- R2 (**sister-defect audit, same commit**) — the shared filter is
  used by 9 target DSEs; the cat-targeting ones need a per-DSE call:
  `mate_target` (should forbid — courting an unconscious cat is
  wrong), `fight_target` (already stance/combat-gated; verify),
  `caretake_target` / `apply_remedy_target` /
  `dependent_kitten_target` (care actions — these plausibly SHOULD
  accept Incapacitated targets; if their step-entry gate rejects
  what their election accepts, they need the FeedKitten-style
  carve-out at `goap.rs:6014` instead of a forbid). Enumerate each
  with its intended semantics; don't blanket-forbid at the shared
  filter (hunt/build/herbcraft targets can't be Incapacitated, and
  care DSEs legitimately want them).
- R3 (**compile-time contract**) — a test asserting every
  cat-targeting TargetTakingDse either forbids Incapacitated or its
  GoapActionKind has a step-entry carve-out; prevents the next
  election/execution split (compile-time-contracts convention).

## Recommended direction
R1 + R2 in one commit (the audit is the fix for the class), R3 as the
regression fence. Small blast radius: eligibility filters compose at
the IAUS layer and surface in the L2 trace.

## Out of scope
- Why the trajectory holds long incapacitation windows (injury/
  recovery balance — Phase V predator-prey territory).
- The 073 cooldown's masking behavior in general.

## Verification
Seed-42 soak: `plan_failures_by_reason["MentorCat: target invalid at
step entry: Incapacitated"]` back to ≤ baseline-noise (≤ ~5);
MentoringSession continuity canary still ≥ 1; scenario-level: a
mentor + incapacitated-mentee preset must not elect MentorCat toward
the incapacitated candidate (structural verification per the
chain-rare-events feedback memory).

## Log
- 2026-07-07: opened from the 467 gate-run verdict (`tuned-42-861c9fe5`,
  plan-failure canary 694× baseline). Layer walk done in-session;
  R1+R2 recommended. Pre-existing: 434 occurrences already in the
  step-12 iter-4 run with zero 467 code aboard.
