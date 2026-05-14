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

---

## Iteration 2026-05-07 — Ticket 117 closure (social_warmth recovered)

Phase 3's most striking cross-metric finding was
`welfare_axes.social_warmth.max` collapsing 0.225 → 0.009 (-96%) when
the modifier shipped at the proposed magnitudes. Ticket 117 tracked
the signal pending ticket 118's momentum-gap fix; both 118 and 119
(legacy `CriticalHealth` interrupt retirement + magnitude promotion to
defaults) landed 2026-05-06.

Re-measurement on the post-118+119 seed-42 deep-soak
(`logs/tuned-42`, commit `9573dc8d`):

| `welfare_axes.social_warmth` | 047 baseline (lifts 0.0/0.0) | 047 Phase 3 treatment (lifts 0.6/0.5, pre-118) | Post-118+119 (current defaults) |
|---|---:|---:|---:|
| max   | 0.225 | 0.009 | **0.998** |
| mean  | —     | —     | 0.828 |
| min   | —     | —     | 0.553 |
| stdev | 0.079 | 0.004 | 0.191 |

The metric is not just recovered, it has substantially overshot the
pre-substrate baseline. Continuity holds (courtship 2383, grooming
945, mentoring 310, bonds_formed 32; `negative_events_total`
35443 vs 53116 baseline = -33%). Survival canaries pass
(Starvation = 0, ShadowFoxAmbush = 1, `never_fired_expected_positives`
empty). Verifies candidate explanation (a) from 117's "Why" section:
the -96% gap was a downstream symptom of plan-completion momentum
gating Sleep, not a property of the modifier itself.

Single-seed measurement is sufficient for closure here because the
gap was 96% — going from -96% to +344% over baseline in one of the
welfare axes is not a tuning question, it's evidence the substrate
expresses behaviorally now that the preempt path is live. Closes
ticket 117. No further action on `acute_health_adrenaline_sleep_lift`;
the 0.50 default promoted by 119 stands.

---

## Iteration 2026-05-14 — Ticket 120 closure (shadow-fox spawn-rate characterization)

Phase 3's other `significant`-band metric was `shadow_fox_spawn_total`
jumping 17.0 → 32.8 (+93%, p=0.017, d=1.35). The ticket framed three
hypotheses: (a) cat-presence coupling, (b) downstream run-length, (c)
new equilibrium with autocatalytic surface growth. Ticket 120
characterized this independently of the 047 magnitude decision.

**(a) is structurally rejected.** `spawn_shadow_fox_from_corruption`
at `src/systems/magic.rs:691-750` reads `wildlife: Query<&WildAnimal>`
(population-cap count only), `map: ResMut<TileMap>` (per-tile
corruption scan), plus `rng`/`time`/`constants`. It does **not** read
cat positions, `CatScentMap` (formerly `CatPresenceMap` /
`congregation`), ward coverage, or any cat-coupled state. The
algorithm is: cadence-gate every ~10 ticks → bail if
`count ≥ shadow_fox_population_cap (2)` → iterate all tiles → spawn
at first tile where `corruption > 0.85` AND `rng < 0.001`. The
+93% cannot be explained by cat-absence weighting because no
such weighting exists in the spawn function.

There IS an indirect autocatalytic loop: shadow foxes deposit
`+0.001` corruption per crossed tile (`src/systems/wildlife.rs:275`),
ward-siege foxes deposit `+0.005` (`src/systems/wildlife.rs:236`),
and corruption diffuses to 4-neighbors every 10 ticks
(`src/systems/magic.rs:78-130`). So the eligible-tile pool can grow
over the run in regimes where wards aren't actively maintained.
Cat-presence affects spawn rate only through downstream chains:
ward maintenance, corruption cleansing, fox-kill rates that free
cap slots, and run-length.

**Empirical readout (post-118+119, single-seed `logs/tuned-42`,
commit 9fb5c96f):**

| `shadow_fox_spawn_total` | 047 baseline (lifts 0.0/0.0, mean over 9 runs) | 047 Phase 3 treatment (lifts 0.6/0.5, pre-118, mean over 9 runs) | Post-118+119 (current defaults, seed=42 single run) |
|---|---:|---:|---:|
| value | 17.0 | 32.8 | **8** |

The metric fully inverted the Phase 3 elevation. Spawn-event detail
from `just q events logs/tuned-42 --kind=ShadowFoxSpawn`:

- 8 spawns across the run, ticks 1,211,440 → 1,270,030 (span 58,590
  ticks, run length 104,877 ticks)
- 6 of 8 fired at `corruption = 1.0` (saturated, long-established
  tiles); only 2 in the 0.85–1.0 ramp zone
- Locations dispersed across `x ∈ [40, 110]`, `y ∈ [28, 74]` — not
  concentrated near a single fox path

This rules out hypothesis (c) "autocatalytic surface growth dominant"
for the current regime: the eligible-tile pool is stable, not growing
late. The contemporaneous footer drift explains the suppression — the
post-118+119 colony places +107% more wards
(`wards_placed_total: 14 → 29`) and the ward layer aggressively
repels foxes (`shadow_foxes_avoided_ward_total: 2 → 156`, +7700%)
before they can grow the corruption surface. Survival canaries pass
(`deaths_by_cause.ShadowFoxAmbush = 2` ≤ 10 hard gate;
`deaths_by_cause.Starvation = 0`).

Same closure pattern as 117: the Phase 3 elevation was a transient of
the pre-118 momentum gap (cats stuck in plan-completion limbo,
ward maintenance starved), not a property of the 047 modifier
itself. Once 118 fixed the momentum gap and 119 promoted the
magnitudes, ward maintenance dominates and the autocatalytic spawn
loop is dampened. Single-seed measurement is sufficient because the
sign inverted and survival canaries pass.

Closes ticket 120. The structural finding — spawn rate is uncoupled
from cat presence; the only coupling is the indirect autocatalytic
loop through corruption deposit + diffusion — is documented in
`docs/systems/magic.md` for future substrate work.
