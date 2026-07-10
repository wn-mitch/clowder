---
id: 532
title: Tick-time budget scheduler — priority-tiered per-tick compute allocation, discretionary passes defer under load (atlas Economic page, behavior-changing — needs balance framing)
status: blocked
cluster: ai-substrate
initiative: []
added: 2026-07-09
parked: null
blocked-by: [527, 528]
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
The atlas Economic page of the `/ideate` perf pass frames tick-time as a fixed
budget and per-tick systems as spenders competing for it — the [480](480-sim-per-tick-throughput-regression-63-p90-decline-197-72-tickssec-over-five-weeks-flamegraph-bisect-reclaim.md)
regression is "deficit spending nobody metered." The proposed lever, borrowed
from congestion-pricing / priority lanes: a **priority-tiered scheduler** where
safety-critical passes (starvation, predator flee) get a guaranteed CPU
allocation and discretionary passes (familiarity, beliefs, joint-intention,
prey-detection) draw on leftover budget and **defer when the tick runs over**.
Under load the colony keeps making the decisions that matter and lets the
low-stakes ones slip a tick.

## The tension that must be resolved before this is built (read first)
**This ticket has no behavior-preserving form.** That is the load-bearing finding
and the reason it is opened `blocked` rather than `ready`:

- A budget that only ever defers work that is *already stale* is exactly
  [528](528-entropy-proportional-compute-one-state-delta-skip-gate-all-hot-per-tick-passes-consult-generalizing-505-at-rest-skip-480-child.md)
  (the skip-gate) with extra machinery — deferring stale work changes nothing, so
  the budget adds no value over 528. It collapses into its prerequisite.
- A budget that delivers *new* value must defer **real, non-stale** discretionary
  work under load — which means a cat's familiarity/beliefs/prey-scan updates a
  tick (or more) late. That is a genuine sim-outcome change: it breaks the
  byte-identical `_footer` determinism gate that 527/528 pass, and it is a
  **balance change**, not a perf refactor.
- Reordering can't rescue it: this sim's per-tick semantics are load-bearing (a
  pass on tick N vs N+1 sees different state), and within-tick order is already
  fixed by the Bevy schedule (moving it is the schedule-edge-perturbation trap,
  memory `learning_bevy_schedule_edge_perturbation`). There is no free lunch that
  packs the same work differently without changing what state each pass observes.

So the honest classification: **this is a behavior-changing design proposal
(adaptive fidelity under compute pressure), not a behavior-preserving perf
ticket.** It therefore does NOT verify against the byte-identical footer gate; it
verifies against the four-artifact balance methodology (hypothesis · prediction ·
observation · concordance) via `just hypothesize`, and it should be triaged
through `/rank-sim-idea` (the V×F×R×C×H rubric) before any code — unlike 527/528,
which are refactors that rubric refuses. It may be the right call that this ticket
**should not be built at all** if the bounded-staleness policy can't be shown to
preserve colony health; that decision is a legitimate outcome of the triage.

## Scope (only if the triage above says proceed)
- A **priority-tier annotation** on per-tick passes: `Guaranteed` (never deferred —
  survival gates) vs `Discretionary` (deferrable). The tier list is explicit and
  reviewed, not heuristic.
- A **per-tick budget accounting** (reads 527's fence instrumentation) and a
  **deterministic deferral policy**: when over budget, defer discretionary passes
  in a fixed, seed-stable order; each deferred cat/pass runs on the next tick it
  fits. Deferral order MUST be deterministic (BTreeMap/stable-sort, the 431 trap).
- A **bounded-staleness guarantee**: a discretionary pass may be deferred at most
  K ticks (a `SimConstants` knob), so no cat's model goes arbitrarily stale.
- A **balance report** proving colony health (survival gates + continuity canaries)
  holds across the deferral policy, at several budget-pressure levels.

## Out of scope
- **Safety-tier deferral** — survival-critical passes are never on the discretionary
  side of the budget. Non-negotiable.
- **Approximation / LOD** (coarse computation for the work that *does* run) — a
  different atlas page (Mythic), and pillar-conflicted per 528's note.
- **The monitoring fence** (527) and **skip-gate** (528) themselves — prerequisites,
  not deliverables here.

## Current state
Opened 2026-07-09 from the `/ideate` atlas draw (Economic page), `blocked` behind
527 (needs the tick-time budget instrument to allocate against) and 528 (needs the
skip-gate so the budget only ever defers genuinely-discretionary work, not work a
cheaper skip would have eliminated). Priced #2 of the atlas candidates — real value
but medium determinism/behavior risk. **No design or code; the tension above is
unresolved and gates everything.**

## Approach
Deferred until unblocked. First step when picked up is the triage, not code:
1. Run `/rank-sim-idea` on the behavior-changing framing; if it scores below the
   ship bar, close as `dropped` with the rationale (that is a valid outcome).
2. If it proceeds, design the specific bounded-staleness policy and write it as a
   balance hypothesis (`docs/systems/*.md` stub + `just hypothesize` spec).
3. Only then implement, verifying via the four-artifact methodology — NOT the
   byte-identical footer gate (behavior *will* change by design; the question is
   whether it changes colony health, which the balance report answers).

## Verification
- `/rank-sim-idea` triage recorded in the Log before any code.
- `just hypothesize <spec>` — the bounded-staleness policy's predicted effect on
  characteristic metrics is stated up front and the observed sweep concords.
- `just verdict` across budget-pressure levels — survival + continuity canaries
  hold; `Guaranteed`-tier passes demonstrably never defer.
- `just check && just test`.

## Log
- 2026-07-09: opened from `/ideate` (atlas Economic page), blocked behind 527+528.
  Central finding recorded: no behavior-preserving form exists — the ticket is a
  behavior-changing design proposal that must clear `/rank-sim-idea` + the
  four-artifact balance methodology, and may legitimately end up `dropped`. Opened
  as a decision record precisely so the tension is captured, not rediscovered.
