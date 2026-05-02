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
| 047 | `AcuteHealthAdrenalineFlee` | lurch (smoothstep) | `health_deficit` | Flee, Sleep | Landed inert |
| 088 | `BodyDistressPromotion` | pressure (linear ramp) | `body_distress_composite` (`max(deficits)`) | Eat, Sleep, Hunt, Forage, Flee, GroomSelf | Landed |
| 106 | `HungerUrgency` | pressure | `hunger_urgency` (`1 - needs.hunger`) | Eat, Hunt, Forage | Ready |
| 107 | `ExhaustionPressure` | pressure | `energy_deficit` | Sleep, GroomSelf | Ready |
| 108 | `ThreatProximityAdrenalineFlee` | lurch | `threat_proximity_derivative` (rising change) | Flee, Sleep | Ready (blocked-by 103) |
| 110 | `ThermalDistress` | pressure | `thermal_deficit` | Sleep | Ready |

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

## See also

- `docs/systems/ai-substrate-refactor.md` §3.5.1 — modifier catalog and pipeline order.
- `docs/open-work/landed/088-body-distress-modifier.md` — composite-distress first pass + the seed for the per-axis split.
- `docs/open-work/landed/047-critical-health-interrupt-treadmill.md` — the conversation that surfaced lurch-vs-pressure during Phase 1 design.
