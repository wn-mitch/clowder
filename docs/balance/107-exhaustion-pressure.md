# ExhaustionPressure modifier — Phase 2 verdict: lift wins L2, blocked by plan-completion momentum (047 pattern on the energy axis); ship inert (2026-05-02)

Authored after the ticket-107 Phase 2 focal-trace soak. No Phase 3
hypothesize sweep was run — see the Decision section for why.

## Hypothesis

Adding an `ExhaustionPressure` pressure-modifier lift on Sleep + GroomSelf
above the 0.7 energy-deficit threshold redirects exhausted cats to rest
before the `Exhaustion` interrupt fires, reducing
`interrupts_by_reason.Exhaustion` in the same regime.

**Constants patch (proposed; never promoted from defaults):**

```json
{
  "scoring": {
    "exhaustion_pressure_threshold": 0.7,
    "exhaustion_pressure_sleep_lift": 0.40,
    "exhaustion_pressure_groom_lift": 0.10
  }
}
```

## Prediction

| Field | Value |
|---|---|
| Metric | `interrupts_by_reason.Exhaustion` |
| Direction | decrease |
| Rough magnitude band | ±40–90% |

## Observation

**Phase 3 hypothesize sweep was skipped.** The canonical baseline carries
`interrupts_by_reason.Exhaustion == 0` (same vestigial pattern as 106's
Starvation arm). Phase 2 substituted a focal-trace stress test:
`logs/tuned-trace-42-107-phase-2/`, 900s seed-42 release headless run
with focal cat Simba, the proposed lifts active, AND doubled energy
decay (`needs.energy_decay = 0.2`/day vs default 0.1/day).

CatSnapshot energy trajectory survey across all 8 cats:

| Cat | n snapshots | min energy | max deficit | in-window (deficit ≥0.7) | legacy threshold (energy ≤0.10) |
|---|---:|---:|---:|---:|---:|
| Simba | 693 | 0.293 | 0.707 | 7 (1.0%) | 0 |
| Birch | 864 | 0.278 | 0.722 | 11 (1.3%) | 0 |
| Calcifer | 159 | 0.300 | 0.700 | 1 (0.6%) | 0 |
| Ivy | 153 | 0.298 | 0.702 | 2 (1.3%) | 0 |
| Lark | 722 | 0.285 | 0.715 | 2 (0.3%) | 0 |
| Mallow | 283 | 0.262 | 0.738 | 11 (3.9%) | 0 |
| Mocha | 976 | 0.240 | 0.760 | 18 (1.8%) | 0 |
| **Nettle** | **973** | **0.000** | **1.000** | **79 (8.1%)** | **33 (3.4%)** |

Unlike 106, the modifier window IS reached — every cat enters it briefly,
and Nettle goes deep enough to reach the legacy interrupt threshold for 33
ticks.

Mechanical wiring verified at Simba's tick 1202395 (deficit just above
the 0.7 threshold): L2 row shows `exhaustion_pressure` modifier firing
on Sleep (delta = +0.0005) and GroomSelf (delta = +0.0001) — micro-deltas
because Simba is at the threshold edge. The gated-boost contract is
honored. At deeper deficits the lift would be larger: at deficit 1.0
(Nettle), full ramp = 1.0 × 0.40 = +0.40 on Sleep, mechanically
sufficient to win L2 against most competing DSEs.

## Concordance

**Verdict: lift wins L2 scoring, blocked by plan-completion momentum at
L3 expression.**

Diagnostic finding: Nettle at energy 0.0 (max deficit) spends 15+
consecutive snapshots (tick 1294600 onward) in `action=Forage`. The Sleep
DSE wins the L2 score contest within Foraging-disposition evaluations,
but GOAP commitment to the active Forage plan blocks the L3 transition
to Sleep. Same pattern as 047's verification soak on the health axis —
ticket 118 names this the modifier-lift-vs-plan-completion-momentum gap.

The legacy `Exhaustion` interrupt that was supposed to catch this never
fires because Foraging is in the disposition-replan exemption list at
the retired `disposition.rs:315-318`. Even at energy 0.0, Nettle was
shielded.

## Why the interrupt was vestigial

Same structural reason as 106's Starvation arm. The Hunting / Foraging
exemption was intended to protect cats already on the rest path — but
in the post-091 regime, cats are *always* on Hunt / Forage / Rest when
their energy gets low (because that's where the GOAP urgency arm at
`goap.rs:642-651` routes them). The exemption set IS the cat's
exhaustion-handling path; the interrupt arm was redundant by
construction.

## Decision

- **Ship the modifier inert** (defaults 0.0/0.0). Phase 1 wiring landed
  at c83de3cd alongside 106 + 110.
- **Skip Phase 3 hypothesize sweep.** The metric is at the noise floor.
- **Phase 4 retired the `Exhaustion` interrupt arm** as zero-risk
  structural cleanup — landed jointly with 106's Starvation retirement
  at 03f9f541. Verification soak post-retirement: 0% drift on every
  footer metric.
- **Activation of meaningful lifts is gated on ticket 118** (or a
  sibling for the energy axis). Until the plan-completion-momentum gap
  is fixed, raising `exhaustion_pressure_sleep_lift` above 0.0 wins L2
  but doesn't translate to L3 expression (Nettle's repro is the
  diagnostic case).

## Worked-example takeaway for distress-modifiers.md (ticket 113)

This ticket is the worked example of the **"lift wins L2, blocked by
plan-completion momentum"** branch of the substrate-over-override
outcome tree. Distinct from 106's "trigger rarely met" branch.

Both 106 and 107 share the deeper finding: **the legacy interrupt arm
was structurally vestigial because the exemption set IS the
need-handling path.** The substrate replacement is the right paradigm
even though it doesn't produce a behavioral change in the canonical
regime — it provides perception-richness and removes override-shaped
debt.
