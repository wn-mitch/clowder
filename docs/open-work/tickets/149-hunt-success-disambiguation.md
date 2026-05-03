---
id: 149
title: Hunt-success disambiguation — instrument per-discrete-attempt outcomes
status: ready
cluster: null
added: 2026-05-02
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: [starvation-rebalance.md]
landed-at: null
landed-on: null
---

## Why

Ticket 032's §Scope item 4 audit surfaced an apparent gap: the sim's
per-Hunt-action success rate is **25.6%** (835 `PreyKilled` ÷ 3266 Hunt
actions on `logs/tuned-42-baseline-0783194`) — below the **30–50%**
real-cat-biology target the 032 ticket cites. But the existing event
vocabulary doesn't disambiguate two interpretations:

1. **Real ecology gap** — a higher fraction of Hunt actions end in `lost
   prey during approach` than real cats experience (i.e., the sim's
   prey-targeting is genuinely sub-optimal).
2. **Measurement artifact** — `Hunt` action and `EngagePrey: seeking
   another target` plan-failure (also 835 events) suggest within-Hunt
   retargeting may be conflated with discrete attempts. If "seeking
   another target" indicates within-Hunt retargeting (not a separate
   attempt), the actual per-discrete-attempt rate is ~34.4%, **inside
   the target band**.

The skill surface (`/logq events` filtered by kind) doesn't expose
enough granularity to distinguish the two. 032's audit closes
inconclusive and defers to this ticket.

This matters because hunt success rate is a load-bearing input to the
food economy: if the sim is genuinely 5+ percentage points lean, it
amplifies the survival-mode attractor 032 is trying to break, and any
graded-cliff tuning will be tuning around a hunting bug.

## Scope

- Add a `HuntAttempt` event family with start/outcome states (or
  equivalent instrumentation on the existing approach path) so a
  discrete attempt is countable and outcome-attributable.
- Suggested fields per attempt: `cat`, `start_tick`, `prey_species`,
  `start_distance`, `end_tick`, `outcome ∈ {killed, lost, retargeted,
  abandoned}`, `failure_reason`.
- Update the audit: rerun the 25.6% computation on a fresh post-instrumentation soak; report whether the rate is in/below the 30–50% real-cat band.

## Out of scope

- Tuning the hunt success rate itself. This ticket only *measures*. If
  measurement confirms a real gap, the tuning work opens as a follow-on.
- Changes to prey targeting, prey density, or carcass yield.
- New plan-failure reasons in the existing `EngagePrey` family — only
  *new* events, not refinement of existing ones.

## Approach

1. Identify the discrete-attempt boundary in `src/steps/hunt/` (or
   wherever `EngagePrey` step lives) — the natural attempt-start is
   approach-begin, attempt-end is the first of (kill, lost, retarget,
   abandon).
2. Emit `EventKind::HuntAttempt { ... }` at attempt-end. Augment
   `feature.rs` with a `HuntAttempted` Feature classified per the
   step-resolver contract (`expected_to_fire_per_soak()` ⇒ `true` —
   any healthy colony should attempt prey).
3. Add a `just q hunt-success <run-dir>` subtool (or an `--outcome`
   filter on the existing `events` subtool) so the audit becomes a
   one-line skill-surface query rather than a manual computation.
4. Re-run the 032 audit against a post-instrumentation soak and append
   the disambiguated result to `docs/balance/starvation-rebalance.md`.

## Verification

- New `HuntAttempt` events appear in `events.jsonl`; one per discrete
  attempt (verify by hand on a 30-tick smoke run).
- `just q hunt-success logs/tuned-42-<commit>` returns a
  per-discrete-attempt success rate.
- The audit re-run lands a definitive verdict: either "rate is in band,
  measurement artifact only" (close item 4 affirmatively) or "rate is
  below band by X%, open follow-on tuning ticket".

## Log

- 2026-05-02: Ticket opened. Surfaced by ticket 032's hunting-success
  audit (`docs/balance/starvation-rebalance.md` Iter 1). The ambiguity
  between Hunt-action count (3266) and discrete-attempt count makes the
  current 25.6% headline figure non-actionable — could be measurement
  artifact (true rate ≈34.4%, in band) or real ecology gap.
