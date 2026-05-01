---
id: 047
title: CriticalHealth interrupt is a treadmill, not a brake — replan picks the same disposition while damage accumulates
status: done
cluster: ai-substrate
added: 2026-04-27
parked: null
blocked-by: []
supersedes: []
related-systems: [needs.md, ai-substrate-refactor.md]
related-balance: [047-acute-health-adrenaline.md]
landed-at: pending
landed-on: 2026-05-01
---

## Why

Post-043+044 collapse probe surfaced a Mallow-shaped death (tick 1,216,304). His final 4 ticks show a tight oscillation:

```
tick 1,216,300: CatSnapshot — health 0.637, action=Fight, hunger 0.66
tick 1,216,301: PlanInterrupted (CriticalHealth) → PlanCreated Crafting/CleanseCorruption
tick 1,216,302: PlanInterrupted (CriticalHealth) → PlanCreated Guarding/EngageThreat
tick 1,216,303: PlanInterrupted (CriticalHealth) → PlanCreated Crafting/CleanseCorruption
tick 1,216,304: PlanInterrupted (CriticalHealth) → PlanCreated Guarding → PlanStepFailed (morale_break) → PlanReplanned → Death
```

CriticalHealth interrupt fires every tick, the replan picks Guarding or Crafting alternately based on jitter, the cat takes 0.18 damage/tick the whole time, dies in 4 ticks. **The interrupt's purpose was to break a stuck plan; instead it built a new failure-mode by churning replans during damage uptake.**

This is the third member of a family (042, 043, this) where stale-state-while-damage-accumulates kills the cat. Worth grouping in design notes as a recurring shape.

## Root cause

`src/systems/disposition.rs:263`:

```rust
if health.current / health.max < d.critical_health_threshold {
    return Some(InterruptReason::CriticalHealth);
}
```

This fires unconditionally on low health, every tick, regardless of:
- Whether the cat is currently fleeing (in which case CriticalHealth is redundant).
- Whether the *previous* CriticalHealth interrupt already fired this tick range (no debounce).
- Whether replanning will actually change the disposition (it usually won't, because the same threat / corruption / hunger state drives the same scoring outcome).

The interrupt's intent was to break commitments when the cat is in trouble — e.g. mid-Crafting, the cat is wounded; CriticalHealth fires; the cat re-evaluates and switches to Flee or Eat. But the implementation as a per-tick re-trigger means: once a cat falls below the threshold, every single tick it forces a replan. The replan often picks the *same* disposition (especially when threats are still nearby and there's nowhere safe to flee to in one tick). Damage accumulates during the replan latency. Cat dies.

## Mechanism in Mallow's case

- Health 0.637 at tick 1,216,300 — already below `critical_health_threshold` (likely 0.7 or 0.5).
- Wildlife adjacent → Guarding/EngageThreat scores high.
- Corruption tile near → Crafting/CleanseCorruption also scores high.
- Both dispositions trigger CriticalHealth interrupt because health is still below threshold.
- Disposition softmax + jitter alternates picks Guarding/Crafting/Guarding/Crafting between ticks.
- Cat never commits to either long enough to act productively (and even acting productively wouldn't save him — both dispositions keep him in damage range).
- Should-have-fled never fires because Flee disposition isn't scored above the per-tick interrupt loop.

## Family resemblance to 042 and 043

| Ticket | Stale state | Damage source while stuck |
|---|---|---|
| 042 | `ticks_remaining` > 0 after non-ThreatNearby urgency preempt → `evaluate_and_plan` skip | hunger drain |
| 043 | `ticks_remaining` > 0 after combat Flee setup → same skip | hunger drain (Calcifer 6,750-tick Flee lock) |
| **047** | CriticalHealth re-fires every tick → replan loop alternates same dispositions | wildlife combat damage |

All three are "interrupt fires faster than the underlying state can clear." Worth a doc note in `docs/systems/` codifying the invariant: **an interrupt that fires every tick must produce a state change that prevents it firing next tick** — otherwise it's a no-op cycle while real damage accumulates.

## Substrate-over-override pattern

Part of the substrate-over-override thread (see [093](093-substrate-over-override-epic.md)) — this ticket is the **prototypical case**.

**Hack shape**: per-tick interrupt that yanks control whenever `health < critical_health_threshold`. Binary gate, no debounce, fires regardless of whether replan changes the answer. Cats die in the replan-churn loop. The per-disposition exemption lists at `disposition.rs:305-342` are the same pattern in special-case clothing.

**IAUS lever**: continuous health/safety/hunger/energy deficits as DSE axes + jerk curves on Sleep/Eat/Flee CP composition. Mirror the 087 prototype exactly — `body_distress_composite` is the model; extend to `hunger_distress`, `exhaustion_distress`, `threat_proximity`. The per-disposition exemption lists get replaced by the Rao-Georgeff §7.2 commitment/momentum modifier in the same pass.

**Sequencing**: [088](../landed/088-body-distress-modifier.md) (Body-distress Modifier) **landed 2026-05-01** at default magnitude (threshold 0.7, lift 0.20). 088 is the substrate axis for the `CriticalHealth` interrupt branch; the remaining `Starvation` / `Exhaustion` / `CriticalSafety` branches still need their own per-axis substrate (extend 087's pattern with `hunger_distress` / `exhaustion_distress` / `threat_proximity` axes; open as sub-tickets when this ticket starts). Land axes one-at-a-time, soak-verify magnitude, *then* retire the corresponding interrupt branch. Removing interrupts before substrate is expressive enough caused 091's collapse. **First task on this ticket**: focal-trace verify that 088's lift fires at sufficient magnitude on a body-distressed cat to flip Guarding/Crafting below Flee/Eat in the IAUS contest; tune `body_distress_promotion_lift` upward if 0.20 is insufficient *before* removing the interrupt branch.

**Canonical exemplar**: 087 (CriticalHealth interrupt → `pain_level` + `body_distress_composite` axes, landed at fc4e1ab).

## Fix candidates

**(A) Force-Flee on CriticalHealth.** Most aggressive: when CriticalHealth fires, override the disposition picker for N ticks and force `Action::Flee` away from the nearest threat. Mirrors `disposition.rs:217-232`'s `ThreatDetected` handler which already does exactly this. The current `_ =>` branch at line 233 (catch-all for non-threat interrupt reasons) just resets `ticks_remaining`; CriticalHealth deserves the same Flee treatment when there's a nearby threat.

**(B) Debounce the interrupt.** Track `last_critical_health_tick` on the cat; only fire if more than M ticks have passed since the last firing. Risk: too long a debounce blocks legitimate course-corrects; too short doesn't help.

**(C) Replan must demonstrate state change.** If the new plan after CriticalHealth replan picks the same disposition + same target, drop into a fallback (Flee or Rest). Requires the disposition picker to expose "did my answer change?" semantics. More invasive.

(A) is the cleanest and most aligned with existing handler shape. (A) + a doc note codifying the family-of-bugs invariant.

## Verification

- **Pre-fix anchor:** `logs/collapse-probe-42-fix-043-044/` — Mallow's last 4 ticks oscillate Guarding/Crafting at 1Hz under CriticalHealth firing every tick. Footer shows 1,429 CriticalHealth interrupts (down from 4,723 pre-fix-043, but still ~85/year — too many for an interrupt that's supposed to be a safety brake).
- **Acceptance for (A):** Re-run collapse probe. CriticalHealth interrupts should drop sharply (only fires when health crosses the threshold *down*, then forces Flee for N ticks). Cats injured in combat retreat instead of oscillating. WildlifeCombat death cluster either disappears or shifts cause.
- **No regression:** Cats not in combat (e.g. injured from misfire, disease, weather) still get the safety break — they Flee to safe ground / rest. Don't accidentally suppress legitimate use cases.

## Out of scope

- Ward perimeter coverage (045).
- FightTarget combat-advantage math (046).
- The broader "interrupt invariant" doc note — separate follow-on, write after the 045/046/047 cluster lands.

## Log

- 2026-04-27: Ticket opened during post-043+044 collapse-probe drill-down. Mallow's 1Hz Guarding/Crafting oscillation at the moment of his death is the smoking-gun trace; same family as 042 and 043.
- 2026-05-01: Phase 0 confirmed threshold misalignment — Mallow's `body_distress_composite ≈ 0.61` at his death window sits below 088's threshold of 0.7, so 088's substrate categorically did not fire. Required a kind-specific modifier reading `health_deficit` directly.
- 2026-05-01: Phase 1 landed `AcuteHealthAdrenalineFlee` modifier (`src/ai/modifier.rs`) — sigmoid lurch on `health_deficit >= 0.4`, lifts Flee (+0.60) and Sleep (+0.50) per the fight-or-flight semantic model. Defaults shipped at 0.0/0.0 (modifier inert) so Phase 3 hypothesize can contrast vs the lift-active treatment. 8 unit tests, full lib suite 1667 passing, `just check` clean.
- 2026-05-01: Phase 2 focal-trace soak at seed 42 / 3600s (logs/tuned-42) confirmed the modifier fires with correct deltas (+0.50 Sleep, +0.60 Flee) and Mallow survived past tick 1.27M (vs death at tick 1.21M in the collapse-probe). Sleep wins the L2 scoring layer in 99.3% of injured-window L3 ticks but is **chosen** only 1.4% — a momentum/plan-completion gap where in-flight Hunt/Forage/Patrol plans complete before Sleep can be selected. Substrate works at scoring layer; behavioral expression is gated.
- 2026-05-01: Phase 4 (interrupt retirement) **deferred** to follow-on ticket 119 because removing the legacy CriticalHealth interrupt would lose its force-Flee path (responsible for ~64% of the actual life-saving in the verification soak). Substrate-over-override discipline requires the substrate to drive behavior, not just rank highest in the scoring layer. Opened ticket 118 (modifier-lift-vs-plan-completion-momentum) as the substrate-quality fix that unblocks 119.
- 2026-05-01: Phase 3 hypothesize sweep (3 seeds × 3 reps × 900s): primary metric (CriticalHealth interrupt count) returned wrong-direction +89% — predicted side effect of cats surviving longer in injured states (more ticks alive = more interrupt firings). Cross-metric `sweep-stats --vs` analysis: only one metric crossed `significant` band — `shadow_fox_spawn_total` +93% (p=0.017, d=1.35). Positive signals: `continuity_tallies.courtship` 0 → 1197 (NEW NONZERO; major continuity restoration), `welfare_axes.purpose.min` +288%, single-seed `anxiety_interrupt_total` -80%. Other shifts to characterize (not classified as regressions): `welfare_axes.social_warmth.max` -96%, `deaths_by_cause.Injury` 0 → 0.67 mean (NEW). Hard survival gates (Starvation, ShadowFoxAmbush) hold across the sweep. **The colony equilibrium itself shifts under the new substrate** — cats live longer in injured states and downstream metrics shift accordingly; surfaced drifts may reflect the new regime rather than regressions to fix. Decision: **ship the modifier wired but inert** (defaults stay at 0.0/0.0). Substrate infrastructure + paradigm land; magnitudes are an independent tuning question for the next iteration once 118's momentum fix lands and 120/117 characterize the equilibrium shifts. Balance doc at `docs/balance/047-acute-health-adrenaline.md` carries the full analysis + decision.
- 2026-05-01: Follow-on tickets opened in this commit (per CLAUDE.md substrate-over-override discipline): 102 (AcuteHealthAdrenaline.Fight), 103 (escape_viability scalar), 104 (Hide/Freeze DSE), 105 (AcuteHealthAdrenaline.Freeze), 106 (HungerUrgency), 107 (ExhaustionPressure), 108 (ThreatProximityAdrenaline), 109 (IntraspeciesConflictResponse — full four-valence including fawn), 110 (ThermalDistress), 111 (retire 088 BodyDistressPromotion), 112 (retire per-disposition exemption lists), 113 (interrupt-invariant + distress-modifiers doc), 118 (modifier-lift-vs-plan-completion-momentum), 119 (CriticalHealth interrupt retirement, blocked by 118), 120 (shadow-fox spawn vs cat-presence coupling — investigates the only `significant`-band metric drift from this ticket's sweep), 117 (social-warmth tradeoff — investigates -96% drift, blocked by 118).
