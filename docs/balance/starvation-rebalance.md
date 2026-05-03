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

## Iter 2 — 2026-05-03 — 032-3 sweep result + courtship-canary regression surfaced

**Run:** `just hypothesize docs/balance/032-3-breeding-floor.yaml` against post-148/150/152 main (commit `efb94e1a`).

### Concordance

```
metric: continuity_tallies.courtship
predicted_direction: increase
predicted_magnitude_pct: [25, 300]
observed_direction:    unchanged
observed_delta_pct:    +5.2%
p_value:               0.944
effect_size (Cohen d): 0.03
verdict:               wrong-direction
```

The breeding-floor reduction (0.6 → 0.4) produced statistically negligible movement (p=0.944). Iter 1's audit prediction is confirmed: lowering the gate floor doesn't change courtship-tally count because courtship attempts aren't gate-starved. **Item 3 closes wrong-direction.** The hypothesis is *valid as ecology* (lower hunger floors should make more cats eligible) but the in-sim signal is too small to reach magnitude band — the bottleneck lives elsewhere.

### Substrate-level regression: courtship collapsed since Iter 1

Iter 1 audit (against `logs/tuned-42-baseline-0783194/`, commit `9945e59`) read **`continuity_tallies.courtship = 999`**.

This sweep's runs (against main commit `efb94e1a`) read **`continuity_tallies.courtship = 0`** in BOTH baseline and treatment runs (per `just q run-summary` on `42-1` of each side).

The never-fired list also expanded:
- Iter 1: `[FoodCooked, MatingOccurred, GroomedOther, MentoredCat]`
- Iter 2: `[FoodCooked, MatingOccurred, GroomedOther, MentoredCat, CourtshipInteraction, PairingIntentionEmitted]`

`CourtshipInteraction` and `PairingIntentionEmitted` are new never-fired-positives. CriticalHealth interrupts also tripled: 43017 → 132255. Something in the 148/150/152 landing window (or the 111/146 ladder) broke courtship-attempt firing entirely — this was not 032-induced.

This is a continuity-canary regression that should have been gated by `verdict`. Surfacing as a separate concern (likely a ticket at 153+).

### Implications for the rest of the sweep chain

- **032-1 (`deaths_by_cause.Starvation`)** — primary metric is independent of courtship. Sweep continues; result will be load-bearing.
- **032-2 (`kittens_surviving`)** — depends on `MatingOccurred` to produce kittens at all. With MatingOccurred never-fired in current main, kitten-mortality stratification can't be tested. **Sweep will run but result will be non-actionable until courtship-canary is restored.**
- **032-5 (`continuity_tallies.courtship`)** — same broken-metric issue as 032-3. **Sweep deferred** until courtship-canary is restored.

### Sweep artifacts

- Baseline: `logs/sweep-baseline-lowering-breeding-hunger-floor-0-6-0-4-widens-the-and-gate-e/`
- Treatment: `logs/sweep-lowering-breeding-hunger-floor-0-6-0-4-widens-the-and-gate-e-treatment/`
- Hypothesize-generated balance doc: `docs/balance/lowering-breeding-hunger-floor-0-6-0-4-widens-the-and-gate-e.md` (auto-draft; this file is the ticket-named thread)

## Iter 3 — 2026-05-03 — 032-1 sweep result (graded cliff): broken / unshippable

**Run:** `just hypothesize docs/balance/032-1-soften-cliff.yaml` against post-148/150/152 main.

### Per-seed footer comparison (Starvation / total / CriticalHealth where readable)

```
side       run    starv  total
baseline   42-1     0      8
baseline   42-2     0      8
baseline   42-3     0      6
baseline   7-1      0      1
baseline   7-2      0      1
baseline   7-3      0      1
baseline   99-1     0      3
baseline   99-2     0      3
baseline   99-3     0      3
treatment  42-1     8      8
treatment  42-2     8      8
treatment  42-3     8      8
treatment  7-1      8      8
treatment  7-2      8      8
treatment  7-3      8      8
treatment  99-1     8      8
treatment  99-2     8      8
treatment  99-3     8      8
```

Treatment shows **exactly 8 Starvation deaths in every single run, regardless of seed**, and starvation = total in every case. That's the colony wiping out (peak_pop=8) and *every* death attributing to Starvation. CriticalHealth interrupts dropped 78% in 42-1 (132255 → 28987) — apparent welfare-axis improvement — but the cost is a complete-colony failure mode under graded mode.

### Concordance verdict

```
metric: deaths_by_cause.Starvation
predicted_direction: decrease
observed_direction:  unknown   ← baseline=0 across all runs, can't compute % change
verdict: wrong-direction
```

The script can't compute a percentage from a 0 baseline. By raw count: baseline mean ~0, treatment mean = 8.0. Direction is **opposite** of predicted (deaths went up, not down).

### Mechanism diagnosis — what went wrong

Two interacting effects I underestimated when writing the scaffolding:

1. **The discriminator threshold (`starvation_attribution_threshold: 0.1`) is too low.** Under graded mode, *every cat* accumulates > 0.1 in `total_starvation_damage` over a 15-minute run because the curve's quadratic is non-zero across the whole hunger range. So when a cat dies *for any reason* (ShadowFoxAmbush, MagicMisfire, WildlifeCombat), the discriminator sees `total_starvation_damage > 0.1` and labels the death `Starvation`. This explains why total deaths in treatment match baseline's count of *all* causes — it's the same cats dying of the same causes, just relabeled.

2. **But total deaths went up too.** Baseline 7-1/7-2/7-3 had 1 death each; treatment had 8 each. Baseline 99-* had 3 each; treatment had 8. So the graded cliff is *also* killing additional cats — not just relabeling. Likely mechanism: the quadratic curve at hunger=0.5 still drains health at 25% of legacy-cliff rate. Over a 15-minute soak that's enough to wear health down on cats who never fully recover (chronic mid-hunger).

The two effects compose: real graded-mortality + universal mis-attribution → 8 deaths-all-Starvation in every run.

### Why this isn't shippable as-is

- Treatment violates the **Starvation == 0 hard survival gate** in every seed.
- Death-cause attribution is meaningless under graded mode with the current threshold.
- The `0.1` threshold needs to scale with run duration or be raised to e.g. `0.5` (cat must lose > 50% of max health to starvation to attribute that way).
- The cliff exponent of `2.0` may be too aggressive — `(1 − hunger)²` at hunger=0.5 = 0.25, meaning 25% of full drain firing all the time. Need a higher exponent (e.g. `4.0` or `6.0`) so drain only kicks in below hunger ~0.3.

### What to do next

This sweep result must be acted on before 032 lands a ship-default change. Options (in order of cheapest):

1. **Re-sweep with higher exponent + higher threshold.** New hypothesis YAML: `starvation_cliff_exponent: 4.0`, `starvation_attribution_threshold: 0.5`. Predict Starvation deaths ↓ vs *legacy* baseline (not vs the failing-graded-mode treatment). This may close item 1 affirmatively without further code.
2. **Attribution semantic redesign.** Instead of accumulating monotonically, decay `total_starvation_damage` over time (e.g. half-life of 1000 ticks) so it tracks *recent* starvation, not lifetime. Larger code change but more defensible.
3. **Pause item 1.** Land scaffolding only (current state); leave item 1 deferred until a follow-on ticket explores the parameter space.

Recommendation: **option 1** — re-sweep with `exponent: 4.0` + `threshold: 0.5`. Cheap, no code change, fits 032's existing knob surface.

### Sweep artifacts

- Baseline: `logs/sweep-baseline-replacing-the-all-or-nothing-hunger-0-cliff-with-quadratic-g/`
- Treatment: `logs/sweep-replacing-the-all-or-nothing-hunger-0-cliff-with-quadratic-g-treatment/`

## Iter 4 — 2026-05-03 — Focal-cat trace + integrated-drain math: cliff curve shape is the bug

The Iter 3 "8 starvation deaths in every run" symptom needed a focal-cat root-cause, not a label.

### Focal-cat: Mocha (first death, treatment 42-1)

`just inspect Mocha logs/sweep-replacing-the-all-or-nothing-hunger-0-cliff-with-quadratic-g-treatment/42-1`:

```
Needs Timeline (60 snapshots over 1.2M ticks):
  hunger:      min=0.50  max=0.99  final=0.84   critical dips: 0
  energy:      min=0.31  max=0.75  final=0.59   critical dips: 0
  ...
Score Breakdown: "No Maslow violations detected (survival actions always won when hungry)"
```

**Mocha was well-fed throughout her life, made rational decisions, never had a hunger crisis — and died of "Starvation" at tick 1212507.** All 8 deaths cluster in a ~4000-tick window (1212507–1216317), the same way for every seed. They're not random food-pressure events — they're **synchronized colony-wide mortality** driven by a non-hunger mechanism.

### Root cause: integrated drain over a long run

The graded cliff formula `cliff_factor = (1 − hunger)²` is non-zero **for any hunger < 1.0**, not just near `hunger = 0`. Over 1.34M ticks (15 min × 1000 ticks/sec at default scale) the drain integrates:

```
hunger=0.30 → cliff_factor=0.49 → integrated drain over 1.34M ticks = 328 health units
hunger=0.50 → cliff_factor=0.25 → integrated drain over 1.34M ticks = 167 health units
hunger=0.70 → cliff_factor=0.09 → integrated drain over 1.34M ticks =  60 health units
hunger=0.84 → cliff_factor=0.026 → integrated drain over 1.34M ticks =  17 health units
hunger=0.95 → cliff_factor=0.003 → integrated drain over 1.34M ticks =   1.7 health units
```

Max health is `1.0`. **At `exponent=2`, even a perfectly-fed cat (hunger=0.84 like Mocha) loses ~17 health units to integrated graded drain over a single soak.** The cliff curve isn't an "occasional dip when hungry" mechanism — it's a constant-on bleed across the whole hunger range, and the integral over 1.34M ticks always overflows max health.

The synchronized death window is the integral catching up to all cats at roughly the same time.

### The exponent has to be much higher (or the formula different)

Re-running the integration math at hunger=0.7 (a reasonable colony-average) over 1.34M ticks:

```
exponent=2  → 60.3   health units drained   ← shipped (lethal)
exponent=4  →  5.4   health units drained   ← still over max
exponent=6  →  0.49  health units drained   ← survivable (~half max health)
exponent=8  →  0.044 health units drained   ← effectively only fires very near hunger=0
exponent=10 →  0.004 health units drained
```

`exponent=6` is the lower bound for survivability assuming avg colony hunger=0.7. For more headroom, `exponent=8`–`10`. But this is exponent-tuning around a curve-shape mistake — the cliff formula `(1 − hunger)^k` is a power-law tail, never truly zero. A cleaner shape:

```
cliff_factor = max(0, (threshold − hunger) / threshold)^k
              for hunger < threshold (e.g. 0.3); 0 above.
```

Truly zero above the threshold, monotonic ramp below. Matches the ticket's stated intent: "fasting cats lose body condition gradually" — they shouldn't be losing health when well-fed.

### Updated recommendation for next iteration

**Don't ship `exponent=2`.** Three viable next steps:

1. **Cheapest** — re-run 032-1 hypothesize with `starvation_cliff_exponent: 8.0` and `starvation_attribution_threshold: 0.5`. May produce a clean Starvation reduction without code change.
2. **Cleaner** — replace the curve shape with the thresholded form above. Add `starvation_cliff_threshold: 0.3` knob; gate the cliff factor on `hunger < threshold`. Small Rust change in `src/systems/needs.rs:110-131`.
3. **Punt** — close item 1 wrong-direction in the ticket; defer the curve-shape redesign to a follow-on ticket.

### Knock-on for the rest of the chain

- **032-2 (per-stage multipliers)** killed mid-run because it composes on `starvation_cliff_use_legacy: false` and would have produced identical wipeout. Cancelled by `pkill`. **Re-run after item 1's curve shape is fixed.**
- **032-5 (body_condition gate)** stays deferred — courtship metric broken in current main per Iter 2.

### Note on the `starvation_attribution_threshold` artifact

The 8/8 attribution pattern (every death labeled Starvation) is *also* real, but it's downstream of the actual integrated-drain bug. With `exponent=8`, total_starvation_damage stays well below `0.1` for healthy cats, and the threshold of `0.1` becomes a reasonable discriminator. **Do not raise the threshold without first fixing the curve shape** — at the wrong curve shape, *any* threshold under 1.0 still attributes incorrectly because the integrated drain is enormous.

