# ThermalDistress modifier — Phase 2 verdict: trigger rarely met AND lift outscored by sibling `groom_self`; ship inert (2026-05-02)

Authored after the ticket-110 Phase 2 focal-trace soak. No Phase 3
hypothesize sweep was run, and there is no Phase 4 — see the Decision
section for both.

## Hypothesis

Adding a `ThermalDistress` pressure-modifier lift on Sleep above the 0.7
`thermal_deficit` threshold pulls cold cats toward shelter (a den / hearth
tile) before their thermal state degrades into the body-distress regime.
Unlike 106 and 107, this ticket has **no legacy `InterruptReason::Thermal*`
arm to retire** — the thermal axis was always handled at the substrate
layer (via `body_distress_promotion` (088) on the composite scalar and via
`groom_self`'s IAUS, which encodes `thermal_deficit` as a
`CompensatedProduct` consideration per `src/ai/dses/groom_self.rs`).
ThermalDistress is purely a perception-richness lever — the "shake the
tree" pattern from 047's design — narrowing the composite into a per-axis
modifier so a cold cat and an injured cat don't get the same generic
"do anything self-care" lift.

**Constants patch (proposed; never promoted from defaults):**

```json
{
  "scoring": {
    "thermal_distress_threshold": 0.7,
    "thermal_distress_sleep_lift": 0.40
  }
}
```

## Prediction

| Field | Value |
|---|---|
| Metric | Sleep action share for cats that hit `thermal_deficit ≥ 0.7` |
| Direction | increase |
| Rough magnitude band | ±20–60% |

(No legacy interrupt → no `interrupts_by_reason.Thermal*` metric to
predict. The behavioral target is "cold cats route to Sleep more often
when in the modifier window.")

## Observation

**Phase 3 hypothesize sweep was skipped.** The thermal axis has no legacy
metric at the noise floor to compare against (unlike 106's `Starvation`
or 107's `Exhaustion`). Phase 2 substituted a focal-trace stress test:
`logs/tuned-trace-42-110-phase-2/`, 900s seed-42 release headless run
with focal cat Lark and the proposed Sleep lift active via
`CLOWDER_OVERRIDES='{"scoring":{"thermal_distress_sleep_lift":0.4}}'`.
No drain override was needed — natural seed-42 weather already produces
a small modifier window per cat.

CatSnapshot temperature trajectory survey across all 8 cats (`temperature
≤ 0.3` ⇔ `thermal_deficit ≥ 0.7` ⇔ in modifier window):

| Cat | n snapshots | min temp | max deficit | in-window (deficit ≥ 0.7) |
|---|---:|---:|---:|---:|
| Birch    | 1099 | 0.30 | 0.70 | 3 (0.27%) |
| Calcifer |  490 | 0.29 | 0.71 | 1 (0.20%) |
| Ivy      | 1364 | 0.30 | 0.70 | 2 (0.15%) |
| **Lark** | **1369** | **0.27** | **0.73** | **9 (0.66%)** |
| Mallow   |  955 | 0.29 | 0.71 | 2 (0.21%) |
| Mocha    | 1401 | 0.30 | 0.70 | 0 (0.00%) |
| Nettle   | 1389 | 0.28 | 0.72 | 3 (0.22%) |
| Simba    |  985 | 0.27 | 0.73 | 4 (0.41%) |
| **Total**| **9052** | — | — | **24 (0.27%)** |

The modifier window IS reached briefly across most cats, but each cat
spends ≤ 1% of cat-ticks in it. The thermal floor sits right at the
trigger threshold (every cat's `min temp` lands in 0.27–0.30) — winter
weather is enough to nudge cats just into the window, but not deep into
it. Bit-identical to the survey of the pre-existing `logs/tuned-42`
baseline (commit 9945e59, lift = 0.0): activating the lift produces zero
drift in the per-cat thermal trajectory, which is itself diagnostic — the
lift doesn't change which dispositions cats sit in.

Mechanical wiring verified: `thermal_distress` fires **52 times** in
Lark's L2 trace, exclusively on `dse:"sleep"` (matching v1 scope —
GroomSelf and Build are out of scope). Sample firing at tick 1234673
(thermal_deficit ≈ 0.74):

```json
"modifiers": [
  {"name": "body_distress_promotion", "delta": 0.036},
  {"name": "thermal_distress",         "delta": 0.054}
]
"final_score": 0.488
```

Both modifiers compose additively on Sleep when the cat enters the
combined-distress regime — the gated-boost contract is honored, no
double-counting, override propagation echoed in the trace header
(`applied_overrides.scoring.thermal_distress_sleep_lift = 0.4`).

## Concordance

**Verdict: trigger rarely met AND, when triggered, the lift is outscored
by `groom_self` (the existing thermal-axis-aware DSE).**

Key diagnostic finding: at Lark's deepest dip tick (1234673-1234674), the
L2 contest resolves as:

| DSE | avg score |
|---|---:|
| `groom_self` | **0.764** ← L2 winner |
| `sleep`      | 0.488 (with `thermal_distress` lift) |
| `eat`        | 0.446 |
| `wander`     | 0.119 |
| (others)     | < 0.1 |

L3 chooses Groom or Patrol — never Sleep — across the dip window. The
`thermal_distress` lift is mechanically correct but behaviorally
displaced: `groom_self`'s IAUS at `src/ai/dses/groom_self.rs:36` already
encodes `thermal_deficit` as a `CompensatedProduct` input ("the cat
settles and warms up by grooming when cold"), and that intrinsic
consideration outscores Sleep + ThermalDistress lift across the entire
modifier window.

This is **structurally different** from 106 ("trigger rarely met,
substrate is redundant safety") and 107 ("lift wins L2, blocked by
plan-completion momentum"). 110's outcome is a third branch:

> **L2-lift outscored by sibling DSE that already encodes the same
> scalar.** The substrate handles the axis correctly — just not via
> the lifted DSE.

The colony-level evidence: footer of the Phase 2 run is bit-identical to
the pre-existing baseline (8× ShadowFoxAmbush deaths, 999 courtship,
219 play, 194 grooming, 41 mythic-texture, 0 mentoring/burial,
43017 CriticalHealth interrupts, every plan-failure-by-reason count
identical). Lark's Sleep action share: 49/1369 = 3.6% — also identical
across baseline and Phase 2 runs. The lift produces zero behavioral drift
because Sleep is never the L2 winner at trigger ticks.

## Why the regime doesn't escalate

Three structural reasons keep `thermal_deficit` from going deeper than
~0.73 in canonical play:

1. **Den / hearth coverage.** `den_temperature_bonus = 3.0` and
   `hearth_temperature_bonus_cold = 3.0` (`buildings.rs:129-159`) restore
   `temperature` rapidly when a cat is on a warm tile — including
   passively while resting, not just by deliberate Sleep choice.
2. **`groom_self` already handles the axis.** Cold cats select
   `groom_self` at ~0.764 score, and the resolved behavior (settle +
   self-tend) routes them out of exposed terrain over a few ticks.
3. **Weather distribution.** Seed-42 winter weather pushes
   `temperature` down by a few percent per tick during Snow / Storm,
   not enough to overrun den/hearth recovery on a cat who's already
   settled.

The trigger is therefore reachable but rarely deep — the modifier window
is a thin sliver at `0.27 ≤ temp ≤ 0.30`, which the cat exits within
1–2 ticks via either weather change or warming infrastructure. Pure
substrate, no override.

## Decision

- **Ship the modifier inert** (defaults 0.0 for `thermal_distress_sleep_lift`).
  Phase 1 wiring landed at c83de3cd alongside 106 + 107.
- **Skip Phase 3 hypothesize sweep.** The behavioral metric (Sleep share
  during trigger windows) doesn't change between baseline and treatment
  — the lift is outscored by `groom_self`, so direction-of-effect is
  zero by construction.
- **No Phase 4.** ThermalDistress has no legacy `InterruptReason::Thermal*`
  arm to retire — unlike 106/107. The ticket's §Why explicitly opens
  with this asymmetry. Wiring stops at Phase 1; the modifier exists as
  perception-richness substrate, dormant in the canonical regime, ready
  to compose if a future regime (winter-bias seeds, climate-shift
  scenarios, Build-shelter DSE) needs the lever.
- **Build-shelter lift remains deferred** (per ticket §Out-of-scope).
  Activation needs a "BuildShelter" disposition variant first, which
  doesn't exist today. Open as a follow-on if/when the construction
  layer adds shelter as a buildable.
- **Existing thermal-axis substrate stays in place.** `groom_self` (via
  `CompensatedProduct(thermal_deficit, …)`) and `body_distress_promotion`
  (via composite max-deficit scalar) jointly handle the cold-cat
  response. Adding Sleep as a third axis is overlapping substrate, not
  a new behavioral driver.

## Worked-example takeaway for distress-modifiers.md (ticket 113)

This ticket is the worked example of a **third branch** of the
substrate-over-override outcome tree, distinct from 106 and 107:

> **"L2-lift outscored by sibling DSE that already encodes the same
> scalar."** The substrate handles the axis correctly — just not via
> the lifted DSE. The modifier is mechanically sound but behaviorally
> redundant; rebalancing would require either rebalancing the sibling
> DSE's IAUS or shifting the lift onto a different DSE class.

For the doctrine table at `docs/systems/distress-modifiers.md`:

- **106** = "trigger rarely met → substrate is redundant safety"
- **107** = "lift wins L2 contest, L3 expression blocked by plan-completion momentum"
- **110** = "trigger reachable but lift outscored by sibling DSE that already encodes the scalar"

All three branches share the deeper finding: in the post-091 colony, the
per-axis modifiers are **inert by default and correct in design** — they
provide perception-richness without imposing override-shaped per-tick
behavior. Activating any of them requires a co-evolving change (lift the
sibling DSE for 110; fix plan-completion momentum for 107; expose the
trigger to a stressed regime for 106).
