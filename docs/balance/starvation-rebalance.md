# Starvation rebalance — ticket 032 thread

Single iteration thread per ticket §Approach. Append below; do not split into separate files.

## Iter 1 — 2026-05-02 — Investigation pass on existing logs

**Source:** `logs/tuned-42-baseline-0783194/` (post-substrate-refactor seed-42 soak, commit `9945e59` dirty, 1.34M ticks). All numbers below are from `just q run-summary` / `just q actions` / `just q events` per the skill-surface convention.

### Current colony state

```
deaths_by_cause           = { ShadowFoxAmbush: 8 }
deaths_by_cause.Starvation= 0          ← single draw within the 0–9 noise band
never_fired_expected      = [FoodCooked, MatingOccurred, GroomedOther, MentoredCat]
continuity_tallies.courtship = 999     ← attempts; not the bottleneck
continuity_tallies.grooming  = 194
continuity_tallies.mentoring = 0
wards_placed_total = 0
plan_failures.top         = EngagePrey:lost-prey 2983, ForageItem:nothing 1846, EngagePrey:re-seek 835
interrupts.top            = CriticalHealth 43017
```

### Reframe — the bottleneck has moved

The ticket §Why claimed `continuity_tallies.courtship` ≈ 0. **The current footer reads 999**. Courtship *attempts* are firing; what's not firing is the post-attempt completion — `MatingOccurred` is on the never-fired list. Item 3 (lower `breeding_hunger_floor` 0.6 → 0.4) targets the *attempt-side* gate, not the completion-side. So:

- The mating-gate veto picture is **less important** than the ticket assumed.
- The reproduction-collapse evidence shifts to: cats reach courtship but never close. Likely candidates: `resolve_mate_with` partner selection, fertility-phase windows, or the §7.M.7.6 viability hard-gate.
- This is a **scope adjustment, not a kill**. Item 3 is still cheap and worth running because the AND-gate may still be *partially* responsible (some courtship attempts may be self-eligible but partner-ineligible). But the predicted magnitude of `courtship ↑ [25, 300]%` from `032-3-breeding-floor.yaml` should be **read carefully** — the pre-existing 999 means even modest absolute gains will look like noise in the relative shift, and the real signal will be in `kittens_born_total` (currently 0) or `MatingOccurred` activation.

### Hunting-success audit (ticket §Scope item 4)

Per-discrete-attempt rate from existing-log skill-surface readout:

| Quantity (skill-surface source) | Count |
|---|---|
| `PreyKilled` events (success) | 835 |
| Hunt-action instances (`just q actions`) | 3266 |
| `EngagePrey: lost prey during approach` plan-failures | 2983 |
| `EngagePrey: seeking another target` plan-failures | 835 |

**Per-Hunt-action success rate: 835 / 3266 = 25.6%.**

Real-cat target per ticket §Real cat biology: **30–50% per attempt for a healthy adult.**

Sim is ~5 percentage points below the lower target bound. Not catastrophic, but **the sim runs ~25% leaner than realistic.** Two possible interpretations:

1. **Prey targeting needs work.** A higher fraction of Hunt actions end in `lost prey during approach` than real cats experience.
2. **The Hunt-action grouping conflates approach-restart with discrete attempt.** If "seeking another target" (835 events) means within-Hunt retargeting, the actual discrete-attempt success rate is closer to 835 / (3266 − 835) = 34.4%, **inside the 30–50% band.**

The skill surface doesn't expose enough granularity to disambiguate without per-step trace. **Recommendation:** open a follow-on ticket scoped to hunt-success disambiguation — either add a `HuntAttempt` event with start/outcome states, or instrument the existing approach-restart path. Ticket 032's item 4 closes inconclusive but with a concrete next-step.

### Mechanism trace, item 1 vs item 5

Question from the plan: does item 1's quadratic cliff change much, or is item 5's `body_condition` axis the load-bearing change?

Footer evidence: `Starvation == 0`, `CriticalHealth interrupts == 43017`. The colony spends a lot of time near the cliff (43017 CriticalHealth interrupts at ≤0.4 health is heavy) but rarely crosses into death. So:

- **Item 1's quadratic curve has a legitimate target.** Even if Starvation deaths are 0 on this seed, the panic-mode social/safety cascade is firing all the time (CriticalHealth interrupts proxy this). Softening the cliff smears that pressure across a wider hunger band, reducing the spike.
- **Item 5's `body_condition` axis is *additive*, not redundant.** It targets gate brittleness across hunger oscillations, not the cliff itself. Best run after items 1 + 2 land.

### Per-stage death distribution (ticket §Scope item 2)

Cannot be answered from a single seed-42 run with 0 starvation deaths and 8 shadow-fox-ambush deaths. **Item 2's stage-stratification justification depends on multi-seed sweeps** (which are deferred until 111+146 land per the operating constraints). The 2.0× / 1.3× / 1.0× / 1.5× multipliers are biology-motivated regardless; treatment-sweep verdicts will tell us whether the kitten-survival prediction holds.

### Welfare-axis means

The `colony_score` in `logs/baselines/post-session-2026-05-02.json` shows:

```
fulfillment   = 0.0     ← suspicious; possibly a measurement gap not a colony state
happiness     = 0.625
health        = 0.244
nourishment   = 0.824
shelter       = 0.0     ← also suspicious
```

`fulfillment = 0.0` and `shelter = 0.0` are likely measurement gaps (subsystems not contributing) rather than ambient state. Worth flagging as a separate concern outside 032's scope.

### Implications for the four hypothesis YAMLs

- **`032-3-breeding-floor.yaml`** — keep as drafted; widen the acceptance band's lower bound and add a *secondary* metric (`kittens_born_total ↑` or `MatingOccurred` count) that's the real signal. Re-evaluate after sweep against the fact that courtship is already firing 999 times.
- **`032-1-soften-cliff.yaml`** — keep as drafted; the quadratic curve targets the CriticalHealth-interrupt regime even when Starvation deaths are 0.
- **`032-2-stage-multipliers.yaml`** — primary metric should be `kittens_surviving / kittens_born` ratio (not just `kittens_surviving` count); needs multi-seed.
- **`032-5-body-condition.yaml`** — keep as drafted; expect modest shift on the courtship-cadence metric without item 1 + item 3 also engaged.

### Handoff state

Code scaffolding for items 1, 2, 3, 5 lands ship-inert in this commit. Item 4 audit closes inconclusive — open a follow-on ticket if hunt-success disambiguation is judged worth doing. Sweeps run later against a clean post-111/146 baseline; YAMLs in `docs/balance/032-{1,2,3,5}-*.yaml` are ready.

