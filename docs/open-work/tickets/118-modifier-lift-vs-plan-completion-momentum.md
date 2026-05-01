---
id: 118
title: Modifier lifts gated by plan-completion momentum — substrate raises score but cat completes mid-plan first
status: ready
cluster: ai-substrate
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [047-acute-health-adrenaline.md]
landed-at: null
landed-on: null
---

## Why

Discovered during ticket 047 Phase 2 verification (focal-trace at logs/tuned-42 trace-Mallow.jsonl). The new `AcuteHealthAdrenalineFlee` modifier successfully raises Sleep's L2 final_score from a baseline mean of 0.30 to 0.87 (lift +0.50 stacking with 088). Sleep wins the disposition softmax in **99.3% of injured-window L3 ticks**.

But Sleep is the **chosen** action only **1.4%** of those ticks (93 of 6839). The cat is mid-plan executing Hunt / Forage / Patrol / Fight steps; those plans complete naturally before Sleep can be selected. By the time the next softmax fires, Sleep wins again, but the cat has done another action of in-flight Hunt/Forage in the interim. The cycle repeats — substrate works at the *scoring* layer but commitment-momentum / plan-completion gates the *behavioral* expression.

Trace confirms `commitment_strength = 0.0` on all sampled records, so it isn't the §075 CommitmentTenure modifier holding things — it's the L3Commitment branch firing `achieved` then `dropped` per natural plan-step completion, with the cat re-engaging the same DSE class because Hunt/Forage scores haven't dropped.

## Why this matters

Substrate-over-override (epic 093) requires substrate to actually drive behavior, not just rank highest in the scoring layer. Today, modifiers that raise a DSE score above its peers can still fail to express behaviorally if the cat is mid-plan in a different DSE. This is a **systemic substrate gap** affecting every modifier shipping under tickets 047 / 088 / 094 / 106 / 107 / 108 / 110 / 111.

Ticket 047's Phase 4 (retire CriticalHealth interrupt) was deferred specifically because of this gap. The substrate raises Sleep but the legacy interrupt's force-Flee path is doing 64% of the actual life-saving (163 of ~250 substrate-driven escape actions per 7000-tick injury window). Removing the interrupt without fixing this gap would lose the force-Flee path's contribution.

## Design space

Three plausible mechanisms, decide at unblock:

1. **Score-margin preemption.** When the next softmax-elected DSE outscores the in-flight DSE by more than a configurable margin (e.g. 0.30), preempt the in-flight plan immediately rather than waiting for natural completion. Hooks into the L3Commitment gate at the existing preemption code path. Risk: too aggressive a margin churns plans; too conservative a margin doesn't fire.
2. **Per-modifier preemption flag.** Add an opt-in modifier trait method `preempts_in_flight() -> bool`. Acute-class modifiers (lurch shape, large magnitude) opt in; pressure-class modifiers (graded ramp) don't. The substrate-class doctrine in `docs/systems/distress-modifiers.md` (ticket 113) maps cleanly to this flag.
3. **Plan-step duration cap under high-modifier-lift.** Cap the remaining ticks of an in-flight plan when the new top DSE outscores it by margin. Less disruptive than full preemption.

(2) is the most substrate-correct — the modifier itself declares whether its lifts demand behavioral expression vs scoring presence. Defer choice to implementation.

## Scope

- Add the chosen mechanism per the design space above.
- Trace integration: emit a new L3 record type or field naming the preemption (so `clowder-focal-cat` reports it cleanly).
- Unit + integration tests covering preemption boundaries.
- Hypothesize spec re-run for ticket 047's metrics with this fix active — expect Sleep chosen-rate to rise from 1.4% to closer to 50%+ during injury, and CriticalHealth interrupts to drop sharply (because Sleep routes to Resting which is exempt from the interrupt at goap.rs:496).

## Verification

- Focal-trace soak post-fix: Mallow's chosen-action distribution during injury window shifts toward Sleep (>30% target) without losing safety canaries.
- `just verdict` post-fix: survival canaries hold; CriticalHealth interrupt count drops vs the 047 post-Phase-3 baseline.

## Out of scope

- Interrupt retirement (ticket 115 — opens after this lands; that's the actual Phase 4 of 047's original arc).
- Per-DSE plan-step-duration tuning (separate balance work).

## Log

- 2026-05-01: Opened from ticket 047 Phase 2 trace analysis. The AcuteHealthAdrenalineFlee modifier worked as designed at the scoring layer but its behavioral expression was gated by plan-completion momentum. Substrate-over-override discipline requires fixing this before retiring the legacy CriticalHealth interrupt (deferred to ticket 115).
