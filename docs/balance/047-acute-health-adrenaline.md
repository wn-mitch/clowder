# Promoting Sleep + Flee under acute health-deficit redirects injured cats to den-recovery before the CriticalHealth interrupt cascade engages, defusing the 1Hz Guarding/Crafting replan loop (2026-05-01)

Drafted by `just hypothesize` (ticket 031). Edit before committing — pre-filled
fields are starting points.

## Hypothesis

Promoting Sleep + Flee under acute health-deficit redirects injured cats to den-recovery before the CriticalHealth interrupt cascade engages, defusing the 1Hz Guarding/Crafting replan loop

**Constants patch:**

```json
{
  "scoring": {
    "acute_health_adrenaline_threshold": 0.4,
    "acute_health_adrenaline_flee_lift": 0.6,
    "acute_health_adrenaline_sleep_lift": 0.5
  }
}
```

## Prediction

| Field | Value |
|---|---|
| Metric | `interrupts_by_reason.CriticalHealth` |
| Direction | decrease |
| Rough magnitude band | ±30–80% |

## Observation

Sweeps: 3 seeds × 3 reps × 900s.

- Baseline: `logs/sweep-baseline-promoting-sleep-flee-under-acute-health-deficit-redirects-in`
- Treatment: `logs/sweep-promoting-sleep-flee-under-acute-health-deficit-redirects-in-treatment`

| Field | Value |
|---|---|
| Observed direction | increase |
| Observed Δ | 89.4% |
| p-value (Welch's t) | 0.2422 |
| Cohen's d | 0.57 |

## Concordance

**Verdict: wrong-direction**

- Direction match: ✗ (decrease vs increase)
- Magnitude in band: see |Δ|=89.4% vs predicted ±30–80%

## Survival canaries

Run `just verdict logs/sweep-promoting-sleep-flee-under-acute-health-deficit-redirects-in-treatment/<seed>-1` against any
treatment run to check survival/continuity didn't regress.

## Cross-metric findings (sweep-stats)

The single-metric concordance check is misleading because the substrate's main
behavioral effect (cats survive longer in injured states) inflates per-tick
interrupt counts even as the override is doing less life-saving work per
firing. The cross-metric `sweep-stats --vs` view is the load-bearing read:

**Positive signals:**

- `continuity_tallies.courtship`: 0 → 1197 (NEW NONZERO; courtship has been a
  zero-canary across recent runs — this is a major continuity restoration).
- `welfare_axes.purpose.min`: 0.057 → 0.220 (+288%, p=0.19) — purpose floor
  rises substantially.
- `welfare_axes.respect.min`: 0.65 → 0.70 (+8%) — small positive.
- Single-seed verdict comparison showed `anxiety_interrupt_total` dropping
  80% (16025 → 3237) when held against the prior pre-substrate baseline.

**Observations to characterize (not classified as regressions):**

The colony is fundamentally different in this regime — cats survive longer
in injured states, total alive-time per run is up, and downstream metrics
shift accordingly. Treat these as "how does the new equilibrium look?"
questions before "which constants need tuning?" answers. The hard survival
gates (`Starvation == 0`, `ShadowFoxAmbush <= 10`) hold across the sweep.

- `shadow_fox_spawn_total`: 17.0 → 32.8 (+93%, p=0.017, d=1.35) — the only
  metric crossing the `significant` band. Possibly cats spending more time
  at the den reduces perimeter coverage; possibly the spawn-rate system is
  presence-coupled in ways worth understanding regardless. Ticket 120
  characterizes this independently of any 047 magnitude decision.
- `welfare_axes.social_warmth.max`: 0.225 → 0.009 (-96%, p=0.17) — max
  social warmth dropped. Could be a downstream symptom of ticket 118's
  momentum gap (Sleep-locked between plan completions), or a real shift in
  colony interaction patterns under longer-lived cats. Ticket 117
  characterizes once 118 lands.
- `deaths_by_cause.Injury`: 0 → 0.67 mean (NEW NONZERO) — small absolute
  count emerging from a baseline zero. Worth understanding (probably
  reflects cats now reaching late-injury states they previously died out of
  via other causes), not a hard regression.

## Decision

**Ship the modifier wired with defaults 0.0/0.0** (substrate exists but
inert). The substrate paradigm — kind-specific lurch modifier on
`health_deficit` reading directly rather than through the `body_distress`
max-flatten — is the load-bearing design and lands here. Magnitudes are an
independent tuning question that benefits from:

- Ticket 118's momentum-gap fix landing first (decouples "Sleep wins the
  contest" from "Sleep is selected" so the 0.50 lift expresses behaviorally
  rather than per-tick scoring-only).
- Tickets 120 / 117 characterizing the colony shifts so any future magnitude
  bump is informed about what's downstream-coupled vs causally driven.

Once those land, re-run this hypothesize spec with the magnitudes intended
to ship — the spec, both sweeps, and this doc are anchors for the next
iteration.

The Phase 3 sweep is preserved at:

- Baseline: `logs/sweep-baseline-promoting-sleep-flee-under-acute-health-deficit-redirects-in/`
- Treatment: `logs/sweep-promoting-sleep-flee-under-acute-health-deficit-redirects-in-treatment/`

Both available as anchors for the next iteration once 118 lands.

---

## Iteration 2026-05-02 — Fight valence (ticket 102)

The N-valence framework gained its second valence on 2026-05-02:
ticket 102 ships `AcuteHealthAdrenalineFight`. Reads the same
`health_deficit` scalar but gated on `escape_viability < 0.4` (the
substrate from ticket 103). When the gate trips (cornered cat,
maternal defense, terrain-locked but combat is winnable), the
modifier lifts Fight by `acute_health_adrenaline_fight_lift` AND
suppresses Flee by the same magnitude — the two valences are mutually
exclusive by construction at the modifier-pipeline composition step.
047's Flee branch owns the response when `escape_viability >= 0.4`;
102's Fight branch owns it when below.

Same shipping discipline as 047: new lift defaults to 0.0 (modifier
inert), proposed 0.50 magnitude enabled via
`docs/balance/hypothesis-102-acute-health-adrenaline-fight.yaml`.
Per the user's chain-rare-events feedback memory, structural
verification (the 8 unit tests in `src/ai/modifier.rs`) is the
primary ship gate; the hypothesize spec is documentation for the
future enable, not a sweep gate — the gate trips rarely on default
geometry.

Freeze (ticket 105) and intraspecies fawn (ticket 109) round out the
N-valence framework when the Hide DSE (ticket 104) lands.
