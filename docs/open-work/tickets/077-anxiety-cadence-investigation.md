---
id: 077
title: Anxiety-interrupt cadence root-cause investigation
status: ready
cluster: planning-substrate
added: 2026-04-29
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Audit gap #7 (important severity). The 70% drop in `anxiety_interrupt_total` between the seed-42 clean run (24,874) and the failed run (7,469) is unexplained — the existing `disposition.rs:231` anxiety-fire site has no per-cat per-tick rate limit, so it should fire whenever the precondition (critical health / hunger / energy) holds.

Hypotheses:
- **(a)** Plan-replan churn at ~4 ticks/plan races the per-tick anxiety check — the cat is preempted by replan before anxiety has a window to fire.
- **(b)** Urgency clearing at step boundaries (`goap.rs:2414` `urgencies.needs.clear()`) races critical-need detection.
- **(c)** The drop is a *symptom* of the stuck-loop, not a separate bug. After 073/074/076 land, anxiety counts return to band on a re-run; document and close.

Independent of 072 — investigation only; may produce no code change.

## Scope

- Investigate `src/systems/disposition.rs:231` (anxiety-fire site) and `src/systems/goap.rs:2414` (urgency clearing).
- Verify hypotheses (a) and (b) against the failed-run `events.jsonl` (`logs/tuned-42-027b-active-failed/`). Cross-check against post-073/074/076 soak.
- If hypothesis (c) — a symptom of the stuck-loop — document and close with no code change.
- If a real bug surfaces:
  - The fix likely lives in `plan_substrate::lifecycle::record_step_failure` (the natural home for "we should have fired anxiety here but didn't"), or in adjusting the anxiety-fire precondition to be sticky-while-holding rather than edge-triggered.
  - Implement and unit-test.

## Out of scope

- Adding a per-cat anxiety rate limit (the absence of one is currently fine; the hypothesis is the opposite — that anxiety is firing *less* than expected, not too much).
- Rewriting the urgency-accumulation path beyond the specific bug if found.

## Approach

Investigation steps:

1. **Inspect the failed run.** `just q events logs/tuned-42-027b-active-failed/ --filter Feature::AnxietyInterrupt` — bucket fires per 100K-tick window. Compare against `logs/tuned-42/` over the same windows.
2. **Cat-timeline drill.** Pull cat-timelines for Mocha / Nettle / Lark in the last 100K ticks of the failed run. Look for ticks where critical-need preconditions held (hunger > threshold) but no `AnxietyInterrupt` fired in the next N ticks. Tabulate the gap.
3. **Code path trace.** Walk the precondition check at `disposition.rs:231` — what gates the fire? Is it a one-shot edge trigger or sticky-while-holding? Cross-reference with `goap.rs:2414`.
4. **Hypothesis verdict.** Decide which of (a) / (b) / (c) the data supports.
5. **Course of action:**
   - (c) → append findings to ticket Log; mark `status: done` with no code change.
   - (a) or (b) → ship a fix (likely in `plan_substrate::lifecycle::record_step_failure` or the anxiety precondition); add unit tests; soak verdict.

Files (only if a fix lands):

- `src/systems/plan_substrate/lifecycle.rs::record_step_failure` — extend with anxiety-fire trigger if applicable.
- `src/systems/disposition.rs:231` — adjust anxiety-fire precondition if applicable.

## Verification

- Investigation document appended to ticket Log explaining the cause.
- Either:
  - A fix lands with unit tests + `just soak 42 && just verdict logs/tuned-42-077` clean; or
  - Documented as a symptom of the stuck-loop (closed by 073/074/076), closed with no-op.

## Log

- 2026-04-29: Opened under sub-epic 071.
