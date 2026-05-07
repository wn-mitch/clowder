# Distress modifiers — lurch vs. pressure

## Statement

Acute distress (adrenaline, fight-or-flight, surprise) lands on the
IAUS as a **lurch** — sigmoid step at threshold, large magnitude,
possibly with valence-split context-gates.

Sustained pressure (hunger building, energy draining, cold creeping
in) lands as a **ramp** — graded linear lift, moderate magnitude,
single-direction targeting.

Picking the curve picks the semantic model. Picking wrong gives you
either a hair-trigger that fires on routine drift (lurch on a
pressure scalar) or a sluggish substrate that misses the
phase-transition (ramp on an acute scalar).

## When to use each

| Aspect | Lurch (sigmoid/smoothstep) | Pressure (linear ramp) |
|--------|----------------------------|------------------------|
| Curve shape | smoothstep / logistic | `((x - threshold) / (1 - threshold)).clamp(0,1)` |
| Magnitude | large (0.4–0.6 lift) | moderate (0.10–0.40 lift) |
| Threshold | high (0.7+) | mid (0.5–0.7) |
| Valence | possibly split (Flee/Fight/Freeze) | typically single direction |
| Scalar shape | acute change or threshold-crossing | gradual physiological build |
| Authoring sin | firing on routine drift (false-alarm adrenaline) | missing the phase-transition (sluggish reaction to acute danger) |

## Worked examples

| Ticket | Modifier | Shape | Scalar | DSEs lifted | Status |
|--------|----------|-------|--------|-------------|--------|
| 047 | `AcuteHealthAdrenalineFlee` | lurch (smoothstep) | `health_deficit` | Flee, Sleep | Landed; lifts active under 119 |
| 088 | `BodyDistressPromotion` | pressure (linear ramp) | `body_distress_composite` (`max(deficits)`) | Eat, Sleep, Hunt, Forage, Flee, GroomSelf | Landed |
| 106 | `HungerUrgency` | pressure | `hunger_urgency` (`1 - needs.hunger`) | Eat, Hunt, Forage | Ready |
| 107 | `ExhaustionPressure` | pressure | `energy_deficit` | Sleep, GroomSelf | Ready |
| 108 | `ThreatProximityAdrenalineFlee` | lurch | `threat_proximity_derivative` (rising change) | Flee, Sleep | Ready (blocked-by 103) |
| 110 | `ThermalDistress` | pressure | `thermal_deficit` | Sleep | Landed inert |

## Perception-richness pattern

More distress kinds = more modifiers, not bigger lift on one. The
088 composite-distress (`max`-flatten across all deficits) was a
deliberate first pass; 106/107/108/110 split it into per-axis
modifiers so a hungry cat and an injured cat don't get the same
"do anything self-care" lift. Each axis gets its own scalar, its
own threshold, its own DSE-class lift. Composing axes inside the
modifier pipeline gives richer behavior than tuning the lift on
one composite scalar. Ticket 111 retires the composite once the
per-axis modifiers are shipping active and cover its scope.

## Why this replaces interrupts

The cluster 042 / 043 / 047 each died from per-tick interrupt
branches that fired faster than their state could clear, churning
replans while damage accumulated. Substrate modifiers don't have
that failure mode — they raise scores in the IAUS contest, the
softmax economy resolves to the right disposition, and the cat
*picks* the response rather than being yanked into it. A modifier
that fires on every tick at the same magnitude is not a bug — it
just sets a baseline score lift the contest has to beat to pick
something else. See `docs/systems/ai-substrate-refactor.md`
§3.5.1 for the modifier catalog and pipeline registration.

Ticket 119 closed the substrate-over-override arc that 047 opened:
the legacy `CriticalHealth` interrupt was removed, 047's
`AcuteHealthAdrenalineFlee` lifts were promoted from 0.0 to the
spec-proposed 0.60 (Flee) / 0.50 (Sleep), and the substrate-driven
preempt path (ticket 118) replaced the interrupt's force-Flee
behavior. The modifier triggers on `health_deficit > 0.4` (HP < 60%)
— a wider trigger than the legacy 40% gate by design, since
adrenaline is a phase transition starting at moderate injury, not
a death-threshold check.

## Behavioral expression — `preempts_in_flight()`

Lurch shape on its own raises scores; without an additional gate,
those lifts only express behaviorally when the cat next re-elects.
Ticket 047 Phase 2 measured this gap on `AcuteHealthAdrenalineFlee`:
Sleep won the L2 softmax in 99.3% of injured-window ticks but was
the *chosen* action only 1.4% of them, because the cat was mid-plan
in Hunt / Forage / Patrol and those plans completed naturally
before the next softmax fired.

Ticket 118 closes this gap with a `preempts_in_flight()` method on
`ScoreModifier`. Default: `false` (pressure-class modifiers leave
it alone). Lurch modifiers override to return `true` when their
trigger scalar fires substantially — i.e., the smoothstep ramp is
non-zero AND the modifier's lift constant is non-zero (an inert
modifier with lift = 0 has nothing to redirect the softmax toward,
so claiming "behavioral expression demanded" would just oscillate).

The `check_modifier_preemption` system runs once per tick before
`evaluate_and_plan`. For each cat with an in-flight `GoapPlan`
(except Resting / Eating, which are already recovery
dispositions), it walks the modifier pipeline and on the first
`true` from `preempts_in_flight` drops the cat's plan and fires
`Feature::ModifierPreemption`. The cat re-elects on the next tick,
this time with the modifier's lift active in the score landscape,
so the lurch's behavioral demand is actually expressed.

| Modifier (ticket) | Class | preempts_in_flight |
|-------------------|-------|--------------------|
| 047 `AcuteHealthAdrenalineFlee` | lurch | yes (gated on `flee_lift > 0 \|\| sleep_lift > 0`) |
| 102 `AcuteHealthAdrenalineFight` | lurch | yes (gated on `fight_lift > 0`) |
| 105 `AcuteHealthAdrenalineFreeze` | lurch | yes (gated on `freeze_lift > 0`) |
| 108 `ThreatProximityAdrenalineFlee` | lurch | yes (gated on lift > 0; scalar Phase-1-stub at 0.0) |
| 088 `BodyDistressPromotion` | pressure | default `false` |
| 106 `HungerUrgency` | pressure | default `false` |
| 107 `ExhaustionPressure` | pressure | default `false` |
| 110 `ThermalDistress` | pressure | default `false` |
| 109 `IntraspeciesConflictResponseFlight` | pressure | default `false` |

## See also

- `docs/systems/ai-substrate-refactor.md` §3.5.1 — modifier catalog and pipeline order.
- `docs/open-work/landed/088-body-distress-modifier.md` — composite-distress first pass + the seed for the per-axis split.
- `docs/open-work/landed/047-critical-health-interrupt-treadmill.md` — the conversation that surfaced lurch-vs-pressure during Phase 1 design.
