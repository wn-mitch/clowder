---
id: 013
title: Spec-follow-on debts from AI substrate refactor
status: in-progress
cluster: null
added: 2026-04-21
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

**Why it matters:** The `docs/systems/ai-substrate-refactor.md`
spec committed its architectural decisions but carries six
spec-follow-on hooks whose resolution lives in *other* systems
(`src/systems/death.rs`, `fate.rs`, `mood.rs`, `coordination.rs`,
`aspirations.rs`) or in code (retired-constants cleanup under
§2.3). On 2026-04-21 the refactor's Enumeration Debt ledger was
pruned to spec-scope only; these six items moved here so (a) they
don't get lost from the refactor ledger as that doc narrows to
its own scope, and (b) their respective system owners can pick
them up in the PRs that touch each system.

Each item's substrate-side contract is *already committed* in
`ai-substrate-refactor.md`; what remains is target-system
implementation or enumeration work.

- **13.1 Retired scoring constants + incapacitated branch cleanup.**
  Spec: §2.3 "Retired constants" subsection. Delete the five
  `incapacitated_*` fields + the `if ctx.is_incapacitated`
  early-return block at `src/ai/scoring.rs:574–598` (renumbered
  from the spec's cited `181–201` by subsequent scoring.rs
  growth), plus `ward_corruption_emergency_bonus`,
  `cleanse_corruption_emergency_bonus`, and
  `corruption_sensed_response_bonus` from `SimConstants`.
  **Gated:** lands in the same PR that introduces the Logistic
  curves that replace them. Not before. Behavior-preserving once
  the curves are in; dangerous before.

  **Gate status (2026-04-23):** LANDED in two commits via a
  three-way parallel fan-out. Rows 1–3 (Incapacitated pathway)
  and rows 4–6 (corruption-emergency-bonus pathway) turned out
  to have disjoint file-sets and shipped as separate landings
  rather than the single commit the original kickoff envisioned.
  All six rows now at `gate ✓`:

  | Row | Constant(s) | Replaces | Prerequisite track | State |
  |---|---|---|---|---|
  | 1 | `incapacitated_eat_urgency_{scale,offset}` | `Eat.hunger = Logistic(8, 0.75)` + `.forbid("Incapacitated")` on non-Eat/Sleep/Idle | Track B: curve ✓. Track C: author ✓ + `.forbid` cutover ✓ (rows 1–3 landing). | **B ✓ / C ✓ — gate ✓** |
  | 2 | `incapacitated_sleep_urgency_{scale,offset}` | `Sleep.energy = Logistic(10, 0.7)` + `.forbid("Incapacitated")` | Track B: curve ✓. Track C: author ✓ + `.forbid` cutover ✓. | **B ✓ / C ✓ — gate ✓** |
  | 3 | `incapacitated_idle_score` | Idle's canonical axes + `.forbid("Incapacitated")` filtering non-eligible DSEs | Track C: author ✓ + `.forbid` cutover ✓; inline `is_incapacitated` branch at `scoring.rs:574–598` retired. | **C ✓ — gate ✓** |
  | 4 | `ward_corruption_emergency_bonus` | `Logistic(8, 0.1)` on `territory_max_corruption` axis in `herbcraft_gather` / `herbcraft_ward` / `practice_magic::durable_ward` | Track B: axes added + curve installed (rows 4–6 landing). | **B ✓ — gate ✓** |
  | 5 | `cleanse_corruption_emergency_bonus` | `Logistic(8, threshold)` on `practice_magic::cleanse.tile_corruption` + `Logistic(6, 0.3)` on `practice_magic::colony_cleanse.territory_max_corruption` | Track B: both axes curve-swapped from `linear()`. | **B ✓ — gate ✓** |
  | 6 | `corruption_sensed_response_bonus` | `Logistic(8, 0.1)` on `practice_magic::durable_ward.nearby_corruption_level` | Track B: axis added + curve installed. | **B ✓ — gate ✓** |

  Rows 1–3 landed as one `refactor:` commit covering the
  Incapacitated pathway: `.forbid("Incapacitated")` on every
  non-Eat/Sleep/Idle cat DSE + every fox DSE, inline branch
  deletion at `scoring.rs:574–598`, 5 `incapacitated_*` constant
  deletions. Rows 4–6 landed as a separate `refactor:` commit
  covering the corruption-emergency-bonus pathway: 5 axis
  migrations (3 added, 2 swapped) + 3 modifier impl deletions
  from `src/ai/modifier.rs` (`WardCorruptionEmergency`,
  `CleanseEmergency`, `SensedRotBoost`) + their pipeline
  registration + 3 `*_corruption_*_bonus` constant deletions.
  See both landing entries in the Landed section for soak
  footers + four-artifact acceptance notes.

- **13.2 Death-event relationship-classified grief emission
  (§7.7.b).** `src/systems/death.rs` today emits only
  generic-proximity grief + FatedLove/Rival removal. §7.7
  aspirations need a richer event — candidate shape is
  `CatDied { cause, deceased, survivors_by_relationship }` (or
  equivalent) — so §7.7.b reconsideration events can filter
  per-relationship (grief-for-mate vs. grief-for-mentor vs.
  grief-for-kin). **Gated:** requires formal relationship
  modeling beyond the current three-tier `BondType`, which is
  Talk-of-the-Town-adjacent work (see cluster C #7, sub-task C3
  — Subjective knowledge / belief distortion).

- **13.3 Fate event-vocabulary expansion (§7.7.c).**
  `src/systems/fate.rs` today emits only `FatedLove` / `FatedRival`.
  Aspirations that should respond to the Calling, destiny
  modifiers, or fated-pair convergence need those events to
  exist. **Gated:** on the Calling subsystem design per
  `docs/systems/the-calling.md` — itself rank 3 in
  `docs/systems-backlog-ranking.md`. Cross-cutting debt; lands
  alongside the Calling implementation, not standalone.

- **13.4 Mood drift-threshold detection layer (§7.7.d).**
  `src/systems/mood.rs` valence today has no hysteresis or
  sustain-duration detection. §7.7.d aspirations need "valence
  below X for N seasons AND misalignment with active-arc
  expected-mood" to fire mood-driven aspiration reconsideration.
  Design-heavy — its own small balance thread. **Gated:** on
  per-arc expected-valence targets, which land with the
  aspiration-catalog work in 13.5 below.

- **13.5 Aspiration compatibility matrix (§7.7.1).** The four
  conflict classes (hard-logical / hard-identity / soft-resource
  / soft-emotional) are committed in the spec; the specific
  hard-logical + hard-identity pair list is enumeration work
  against the stabilized aspiration catalog. **Gated:** lands in
  the PR that enumerates aspirations themselves (aspirations
  catalog isn't currently a tracked entry in this file — add
  one if prioritized). Also unblocks 13.4.

- **13.6 Coordinator-directive Intention strategy row (§7.3).**
  The §7.3 footer note commits `SingleMinded` with a
  coordinator-cancel override; the full row contents land with
  the coordinator DSE. **Cross-ref:** #1 sub-3 above — the C4
  strategist-coordinator task board. When C4 is picked up, this
  row gets its final commit and the ledger-level pointer in
  `ai-substrate-refactor.md` resolves.

- **13.7 Tradition unfiltered-loop fix (§3.5.3 item 1)
  [2026-04-23].** The `Tradition` modifier in `src/ai/modifier.rs`
  is a faithful port of the retiring inline block — it applies the
  caller-pre-computed `tradition_location_bonus` to **every** DSE
  rather than filtering by the action whose history matched this
  tile. Spec §3.5.3 item 1 calls this out as a bug with two
  candidate fixes:
  - **(a) Structural fix** — caller pre-computes a
    `HashMap<Action, f32>` keyed by the matched action; the
    modifier reads a per-DSE-id scalar and adds only on hits.
  - **(b) Semantic fix** — declare Tradition *is* a flat
    tile-familiarity bonus (not action-specific); update §2.3's
    Tradition row in the spec.

  Resolving this is a behavior change under CLAUDE.md's Balance
  Methodology — requires a hypothesis + prediction + measured A/B
  + concordance before landing. Today the caller sets
  `tradition_location_bonus = 0.0` in production (`goap.rs:900`),
  so the unfiltered-loop is a no-op in live soaks and the fix is
  not time-critical. **Gated:** on a balance thread choosing
  between (a) and (b); not a prerequisite for any cluster-A or
  cluster-B work.

**Dependency graph:**

- 13.1 gated on cluster A (#5 — A1 IAUS refactor).
- 13.2 gated on C3 (#7 — belief modeling).
- 13.3 gated on the Calling subsystem
  (`docs/systems/the-calling.md`; no current open-work entry —
  add one if prioritized ahead of 13.3).
- 13.4 gated on 13.5 (needs per-arc valence targets).
- 13.5 gates 13.4; stands on its own given the aspiration catalog.
- 13.6 gated on C4 (#1 sub-3).
- 13.7 gated on a balance thread; caller-side tradition bonus is
  `0.0` in production, so the unfiltered-loop port is currently a
  dormant no-op.

**Memory write-back on landing:** commit per-subtask memories as
each lands so the next cross-thread session has a local record
of what the substrate's follow-on contract was and how the
system owner satisfied it. Tag pattern: `substrate-follow-on`,
`{subsystem-name}`, `ai-substrate-refactor`.
