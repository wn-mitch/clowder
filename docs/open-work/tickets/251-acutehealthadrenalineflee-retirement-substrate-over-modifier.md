---
id: 251
title: AcuteHealthAdrenalineFlee retirement — substrate-over-modifier
status: ready
cluster: ai-substrate
added: 2026-05-09
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

`AcuteHealthAdrenalineFlee` (`src/ai/modifier.rs`, ticket 047) is a §3.5
post-scoring modifier that lifts both `Flee` and `Sleep` scores when
`health_deficit ≥ acute_health_adrenaline_threshold` (default 0.4).
It was introduced as the substrate-correct retirement target for the
legacy `CriticalHealth` interrupt branch (per 119, 108, 107, 106).
Per the 047 rustdoc, *"Flee is filtered from the disposition softmax
... Sleep is the in-pool partner ... The Sleep lift is what flips
the disposition contest away from Guarding/Crafting under injury —
Sleep routes to a den, mechanically expressing retreat."*

The modifier is **structurally fragile**:

- **It is a §3.5 post-scoring modifier, not substrate.** Per CLAUDE.md's
  substrate-over-hacks pillar and the 093 epic, post-scoring modifiers
  that lift specific DSEs to mask scoring-layer gaps are antipatterns —
  the substrate-correct shape is for the DSE's own axes to encode the
  cat's belief / urgency directly. Sleep already has `injury_rest`
  and `pain_level` axes (per `src/ai/dses/sleep.rs:114-125`); the
  modifier's lift duplicates and overrides what those axes should be
  doing.
- **Its preempt rate is the single biggest contributor to the
  modifier-preemption metric in healthy seed-42 soaks** (~347/10kt
  post-247 baseline; this ticket's audit traces the historical
  trend pre-230 = 3,920 → post-247 = 347, with multiple tickets
  reducing it but never zeroing it out).
- **It interacts badly with the BDI commitment substrate (126).** With
  per-tick re-evaluation pre-126, cats responded to HP drops by
  re-picking optimal actions (no preempt event needed); post-126, HP
  drops trigger a §7.2 reconsideration preempt, recorded as
  `modifier_preemption(acute_health_adrenaline_flee)`. The metric's
  high baseline reflects commitment-window time spent in low-HP
  state — not a gameplay bug per se, but a recording artifact that
  signals the modifier path is fragile.
- **249's failed Sleep gate exposed how easily the lift's landing
  target can be starved.** Adding an `EligibilityFilter::require(...)`
  on Sleep regressed the preempt rate 11× back to pre-230 levels
  (~3,830/10kt). Any future eligibility refinement on Sleep risks
  the same regression unless the modifier itself retires.

The substrate-correct retirement: shift the injury-recovery urgency
into the Sleep DSE's existing `injury_rest` / `pain_level` axes (or
split into a new `health_deficit_urgency` axis if the existing axes
can't carry the load shape). The Sleep score becomes honestly high
when the cat is injured, the L3 softmax picks Sleep without needing
a post-scoring lift, and the modifier-preemption chain retires.

## Scope

1. **Audit the modifier's behavior under healthy soak.** Quantify
   how often the modifier's preempt actually flips the cat's
   intention (vs. the cat already on Sleep / Flee). 249's audit
   showed `FleeTargetPicked = 0` in healthy seed-42 (Flee never
   adopts), so the Sleep half of the lift is the load-bearing path.
2. **Re-shape Sleep DSE axes** (`src/ai/dses/sleep.rs`) so the
   `injury_rest` + `pain_level` axes carry an injury-recovery
   urgency *equivalent to* what the modifier currently delivers.
   May require a new `health_deficit_urgency` consideration with a
   sigmoid curve matching the modifier's `transition_width = 0.1`
   onset shape. Validate against the per-DSE applicability matrix
   in spec §3.5.2.
3. **Retire `AcuteHealthAdrenalineFlee` modifier** from
   `src/ai/modifier.rs` once the Sleep axes carry the load. Update
   spec §3.5.1 catalog.
4. **Verify the post-247 cliff is still gated** by 247's
   `intention_preempt_strength_regime_boundary` (R4) without the
   modifier in the pipeline.
5. **Document the substrate-vs-modifier transition** in spec §3.5
   alongside the existing 230 / 232 / 246 reduction history.

## Out of scope

- **Retiring the entire post-scoring-modifier layer.** Other
  modifiers (Pride, Independence, Patience, Tradition, Fox-suppression,
  Corruption-suppression, IntentionMomentum, the §3.5.5 cooldown,
  …) stay in place. This ticket is specifically the
  `AcuteHealthAdrenalineFlee` retirement.
- **Retiring `AcuteHealthAdrenalineFight`** (the cornered-cat fight
  lift). Different mechanism, different risks. Audit separately if
  needed.
- **The Fleeing disposition (230) adoption question.** That's
  ticket 252; 251's job is the modifier, not the disposition.

## Current state

- **Failed in 249's audit (2026-05-09).** Modifier-preempt rate
  regressed 11× under 249's Sleep gate; the gate was rolled back.
  The modifier's preempt rate has been progressively reduced by
  230 → 232 → 246 (3,920 → 1,613 → 662 → 347 per 10kt post-247
  baseline) but never retired entirely. 249 surfaced that the
  modifier is sensitive to any change in Sleep's eligibility / score
  shape.
- The legacy `CriticalHealth` interrupt branch retired by ticket 119
  is gone; the modifier is the substrate-correct replacement, but
  itself sits at the §3.5 modifier layer, not at the substrate-
  proper L1/L2 layer.
- Sleep DSE (`src/ai/dses/sleep.rs`) currently has six axes: `energy_deficit`,
  `day_phase`, `health_deficit` (injury_rest), `pain_level`,
  `sleep_spot_distance`, `safe_rest_distance`. The injury-recovery
  load is shared between `health_deficit` (via `injury_rest_bonus`)
  and `pain_level`, both Linear curves.

## Approach

1. **Phase A — preempt-flow audit.** For the post-247 baseline run,
   classify each `acute_health_adrenaline_flee` modifier-preemption
   into: (a) flipped cat from Hunt → Sleep, (b) flipped cat from
   Hunt → Flee (rare, since Flee filters out), (c) refined a
   cat already on Sleep, (d) other. Use `just q` queries against
   the existing baseline log. Determines the actual landing
   distribution.
2. **Phase B — Sleep axis re-shaping.** Draft a `health_deficit_urgency`
   consideration with sigmoid curve matching the modifier's onset
   shape. Add to Sleep DSE; rebalance composition weights to keep
   uninjured cats' Sleep score unchanged. Validate against scenario
   microexperiments and the existing modifier-preempt scenarios.
3. **Phase C — retire the modifier.** Remove from
   `compose_action_scores` / pipeline registration; delete the
   modifier struct. Update §3.5.1 catalog.
4. **Phase D — verification soak.** `just soak-trace 42 Mallow` +
   `just verdict logs/tuned-42 --baseline <post-247>`. Required:
   (a) `acute_health_adrenaline_flee` preempt rate = 0 (modifier
   is gone), (b) overall `ModifierPreemption` ≤ post-247 baseline
   rate (no other modifier picks up the load), (c) hard gates +
   continuity canaries hold, (d) injury healing rate maintained
   (cats still recover from injuries by sleeping at dens — verify
   via `InjuryHealed` Feature count per 10kt against baseline).

## Verification

- Modifier-preempt rate goes to 0 for `acute_health_adrenaline_flee`
  (modifier is retired).
- Total `ModifierPreemption` count does NOT regress vs post-247
  baseline (no other modifier compensates upward).
- `InjuryHealed` per 10kt within ±10% of baseline (Sleep-driven
  recovery still functions).
- Hard gates pass: deaths_starvation = 0, deaths_ShadowFoxAmbush ≤ 10,
  footer line written, never_fired_expected_positives = [].
- Continuity canaries hold (each ≥ 1, except burial per 250).
- `just check` clean (substrate-stub, step-resolver, IAUS-coherence
  all pass — the retirement is removing a modifier, not adding one).
- Frame-diff per-DSE drift on Sleep within concordance band (the
  re-shaped axes should produce equivalent score *shapes* under
  injury, not necessarily identical values).

## Log

- 2026-05-09: opened from 249's audit. The 11× modifier-preempt
  regression under 249's Sleep gate exposed how fragile the
  modifier path is — the substrate-correct retirement (axis-side
  load instead of modifier-side lift) is overdue. Cluster:
  ai-substrate. Plan-substrate exemplar: 119 (CriticalHealth
  retirement); this ticket follows the same shape one layer up
  (modifier → axis instead of interrupt → modifier).
