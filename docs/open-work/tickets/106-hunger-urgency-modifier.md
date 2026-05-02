---
id: 106
title: HungerUrgency modifier — substrate axis for Starvation interrupt retirement
status: in-progress
cluster: ai-substrate
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

`InterruptReason::Starvation` (`src/systems/disposition.rs:311-313`) is the same
per-tick override pattern 047 retired on the health axis. It fires unconditionally
whenever `needs.hunger < d.starvation_interrupt_threshold` (default 0.15) AND the
cat's disposition is *not* `Resting | Hunting | Foraging`. The disposition
exemption list is the override-shaped tell: the interrupt only "works" because we
manually carve out the dispositions that already address hunger — exactly the
shape ticket 112 (per-disposition exemption-list retirement) targets. Substrate-
over-override (epic 093) wants a kind-specific modifier reading `hunger_urgency`
(already published at `src/ai/scoring.rs:403`, `(1 - needs.hunger).clamp(0,1)`)
that lifts the food-acquisition class enough for the IAUS contest to tilt
unprovoked, before the cat hits 0.15 hunger.

Sibling to 047's `AcuteHealthAdrenalineFlee` (sigmoid lurch on `health_deficit`)
and 088's `BodyDistressPromotion` (linear ramp on `body_distress_composite =
max(deficits)`). Distinct from both:

- vs 088: reads `hunger_urgency` directly rather than the max-flatten composite,
  so hunger alone fires it even when the cat's other axes are quiet — matching
  the late-soak slow-starvation regime where one need degrades first.
- vs 047: **pressure**, not **lurch**. Hunger is a gradual physiological build,
  not a phase transition. Linear ramp from threshold (mirroring 088's transform
  shape), no smoothstep. Pattern doctrine codified by ticket 113 in
  `docs/systems/distress-modifiers.md` — this ticket is one of the doctrine's
  worked examples.

## Scope

Five-phase methodology mirroring ticket 047. Ship the modifier inert (defaults
0.0) in Phase 1, verify behavioral effect with overrides in Phases 2-3, retire
the legacy interrupt only after the substrate is shown sufficient.

### Phase 1 — Modifier implementation (substrate-only, ships inert)

Files to touch:

- **`src/ai/modifier.rs`** — new `HungerUrgency` struct registered in
  `default_modifier_pipeline` between `AcuteHealthAdrenalineFlee` (047) and
  `FoxTerritorySuppression`. Same shape as 088's `BodyDistressPromotion`:
  linear ramp from threshold to 1.0, additive lift gated on `score > 0.0`
  (gated-boost contract). Add `const HUNGER_URGENCY: &str = "hunger_urgency";`
  near the existing `HEALTH_DEFICIT` / `BODY_DISTRESS_COMPOSITE` constants
  (~`modifier.rs:90`).

  Proposed signatures (mirroring `AcuteHealthAdrenalineFlee::new` / `apply`):

  ```rust
  pub struct HungerUrgency {
      threshold: f32,
      eat_lift: f32,
      hunt_lift: f32,
      forage_lift: f32,
  }

  impl HungerUrgency {
      pub fn new(sc: &ScoringConstants) -> Self { /* read four constants */ }
      fn ramp(&self, urgency: f32) -> f32 {
          if urgency <= self.threshold { return 0.0; }
          ((urgency - self.threshold) / (1.0 - self.threshold)).clamp(0.0, 1.0)
      }
  }

  impl ScoreModifier for HungerUrgency {
      fn apply(&self, dse_id: DseId, score: f32, ctx: &EvalCtx,
               fetch: &dyn Fn(&str, Entity) -> f32) -> f32 {
          let lift_scale = match dse_id.0 {
              EAT => self.eat_lift,
              HUNT => self.hunt_lift,
              FORAGE => self.forage_lift,
              _ => return score,
          };
          if score <= 0.0 { return score; }
          let urgency = fetch(HUNGER_URGENCY, ctx.cat).clamp(0.0, 1.0);
          let ramp = self.ramp(urgency);
          if ramp <= 0.0 { return score; }
          score + ramp * lift_scale
      }
      fn name(&self) -> &'static str { "hunger_urgency" }
  }
  ```

  Bump the `default_pipeline_registers_eleven_modifiers` test in `mod tests` to
  `_registers_twelve_` (matching the count comment that lists 047 as the
  eleventh).

- **`src/resources/sim_constants.rs::ScoringConstants`** — four new fields with
  `#[serde(default = "default_…")]` and rustdoc carrying the same threshold-
  aligns-with-disposition note 047's constants do (mirror lines 1341-1369 in
  that file). Default helpers added at the same site as
  `default_acute_health_adrenaline_*` (~line 2191).

  | Constant | Default (ship inert) | Hypothesize-spec value | Doc-comment anchor |
  |---|---|---|---|
  | `hunger_urgency_threshold` | 0.6 | 0.6 | "Mirrors `1 - starvation_interrupt_threshold` lifted slightly so the substrate engages *before* the legacy interrupt would have. `hunger_urgency` (= `1 - needs.hunger`) at 0.6 corresponds to `hunger = 0.4` — a hungry-but-not-starving cat." |
  | `hunger_urgency_eat_lift` | 0.0 | 0.40 | "Largest lift on the food-acquisition class — Eat is the direct solution; Hunt / Forage are upstream." |
  | `hunger_urgency_hunt_lift` | 0.0 | 0.20 | "Smaller than Eat — Hunt is upstream of Eat in the food chain; lift is enough to win the IAUS contest under hunger pressure but not enough to suppress Eat when stockpile is non-empty." |
  | `hunger_urgency_forage_lift` | 0.0 | 0.20 | "Symmetric to Hunt — both are food-acquisition fallbacks when stockpile is low." |

  All four serialize into the `events.jsonl` header per the comparability
  invariant (`src/plugins/headless_io.rs::emit_headless_header`).

- **Unit tests** in `mod tests` — mirror 047's `acute_health_adrenaline_*`
  test family (eight tests):
  - `hunger_urgency_no_lift_below_threshold`
  - `hunger_urgency_zero_lift_at_threshold` (boundary)
  - `hunger_urgency_lifts_above_threshold` (linear ramp midpoint)
  - `hunger_urgency_max_lift_at_full_urgency` (urgency = 1.0)
  - `hunger_urgency_targets_only_eat_hunt_forage` (assert all 14 non-target
    DSEs unchanged at urgency = 1.0)
  - `hunger_urgency_does_not_resurrect_zero_score` (gated-boost contract)
  - `hunger_urgency_composes_additively_with_stockpile_satiation` (full-pipeline
    regression: under combined high urgency + low stockpile, Eat lifts and
    Hunt/Forage lift then survive the multiplicative damp because food_scarcity
    is high → damp is small)
  - `hunger_urgency_starving_cat_late_soak_scenario` — pin a representative case
    (urgency = 0.90 ≈ hunger 0.10, just above starvation threshold) so future
    threshold tweaks don't silently break alignment with the legacy interrupt
    regime.

DSEs affected (read-only in this ticket — the modifier touches their scores
post-evaluation, doesn't modify their definitions):

- `src/ai/dses/eat.rs` (uses `"hunger_urgency"` already, line 56)
- `src/ai/dses/hunt.rs` (line 42)
- `src/ai/dses/forage.rs` (line 51)

Do not touch the predator hunger DSEs (`fox_hunting.rs`, `hawk_hunting.rs`,
`snake_*foraging.rs`, etc.) — they use `fox_ctx_scalars` / a separate scoring
pipeline; this modifier is registered in the cat pipeline only.

### Phase 2 — Focal-trace verification

Pin that the modifier fires at the right magnitude on a hunger-stressed cat
*before* running the multi-seed sweep.

- `just soak-trace 42 <cat>` with `CLOWDER_OVERRIDES` set to the proposed lift
  values from the table above. Focal cat: pick whichever cat hit
  `interrupts_by_reason.Starvation` highest in the canonical baseline
  (`logs/tuned-42`, the run pointed at by `logs/baselines/current.json`). Use
  `/logq events <run-dir>` to query the run, filtered to
  `kind=PlanInterrupted reason=Starvation` and grouped by cat (skill surface
  per CLAUDE memory — no raw `jq` / `grep` on logs/).

  **Override-fallback (mandatory in the post-091 regime).** The promoted
  baseline carries `interrupts_by_reason.Starvation == 0` as a hard gate, so
  there are no natural Starvation candidates to focal-trace. Generate a
  hunger-stressed scenario by **doubling the hunger drain rate**:
  `CLOWDER_OVERRIDES='{"needs":{"hunger_decay":0.2}}'` (default is 0.1 per
  in-game day, see `src/resources/sim_constants.rs:120` and the
  `RatePerDay` newtype at `src/resources/time_units.rs:50`). Doubling decay
  reproduces the slow-starvation regime the legacy interrupt was built for
  without compounding food-stockpile dynamics (which a per-meal-satiation
  knob would). Stack with the lift overrides into one JSON object and
  document the exact stack in the trace sidecar's commit notes.
- Verify in the trace's `modifier_deltas` rows: `hunger_urgency` fires on Eat /
  Hunt / Forage with the expected ramp = `((urgency - 0.6) / 0.4)`, multiplied
  by the per-DSE lift. Sanity: at `urgency = 0.85` (cat at hunger 0.15, the
  legacy interrupt threshold), ramp = 0.625; Eat lift = 0.625 × 0.40 = +0.25.
- Sanity-check that Eat's pre-modifier score plus the lift puts it above the
  competing Guarding / Crafting / Patrol scores in the IAUS contest. If not,
  *tune the lift upward before Phase 3* — same discipline 047 used (and
  abandoned, shipping inert) when its 0.50 / 0.60 lift turned out scoring-
  only-not-behavioral.

### Phase 3 — Hypothesize sweep

- **Spec file:** `docs/balance/106-hunger-urgency.yaml` — mirrors
  `docs/balance/047-acute-health-adrenaline.yaml` shape exactly:

  ```yaml
  hypothesis: "Adding a HungerUrgency pressure-modifier lift on Eat / Hunt /
    Forage above the 0.6 urgency threshold redirects hungry cats to food-
    acquisition before the Starvation interrupt fires, reducing
    interrupts_by_reason.Starvation in the same regime"

  constants_patch:
    scoring:
      hunger_urgency_threshold: 0.6
      hunger_urgency_eat_lift: 0.40
      hunger_urgency_hunt_lift: 0.20
      hunger_urgency_forage_lift: 0.20

  prediction:
    metric: interrupts_by_reason.Starvation
    direction: decrease
    rough_magnitude_pct: [40, 90]

  seeds: [42, 99, 7]
  reps: 3
  duration: 900
  ```

  Magnitude band [40, 90] is wider than 047's [30, 80] because the legacy
  Starvation interrupt fires *less often* than CriticalHealth in the post-091
  baseline (the seed-42 hard gate is `Starvation == 0`); single-seed variance
  dominates. Per CLAUDE.md "drift > ±10% needs hypothesis" rule, any
  direction-match within 2× of predicted magnitude band passes concordance.

- **Run:** `just hypothesize docs/balance/106-hunger-urgency.yaml`. The harness
  produces baseline + treatment sweeps under
  `logs/sweep-baseline-adding-a-hungerurgency-…/` and
  `logs/sweep-adding-a-hungerurgency-…-treatment/`, plus the four-artifact
  report in `docs/balance/106-hunger-urgency.md`.

- **Cross-metric read** (load-bearing per 047's lesson — the single-metric view
  often gives wrong-direction signal because the substrate's behavioral effect
  inflates per-tick interrupt counts as cats survive longer):
  `just sweep-stats logs/sweep-…-treatment --vs logs/sweep-baseline-…`.
  Specifically watch:
  - `deaths_by_cause.Starvation` — must stay 0 (hard gate per CLAUDE.md).
  - `food_fraction.{mean,median}` — should hold or rise (modifier promotes food
    acquisition; stockpile shouldn't collapse).
  - `Feature::FoodEaten`, `Feature::FoodCooked` — should rise.
  - Continuity canaries (grooming / play / mentoring / burial / courtship /
    mythic-texture) — each must stay ≥1 per soak.
  - `shadow_fox_spawn_total` — 047 surfaced this as the only `significant`-band
    drift; check it didn't compound.

### Phase 4 — Interrupt retirement (gated on Phase 3 verdict)

**Only proceed if Phase 3's cross-metric read shows the modifier is behaviorally
sufficient** — i.e., reduced reliance on the interrupt visible in the trace, no
canary regressions, no equilibrium shifts requiring a ticket-118-style follow-on.
If sufficiency is unclear (047's outcome — Sleep won the scoring layer 99.3% but
was selected only 1.4% of injured-window ticks), ship the modifier inert + open
a follow-on plan-completion-momentum ticket and **defer Phase 4 to that ticket**.

If proceeding:

- Remove `disposition.rs:311-313` (the Starvation arm of the interrupt branch).
  Leave the Resting/Hunting/Foraging exemption block in place — the Exhaustion
  arm at line 314-316 still uses it (ticket 107 retires that arm separately).
- **Wrapper cleanup (per landed-112's supersession Log):** if 107 has already
  landed at the time 106's Phase 4 ships, also delete the wrapping
  `if !matches!(disposition.kind, Resting | Hunting | Foraging)` block at
  `disposition.rs:305-317` — both arms inside it are gone, the wrapper is
  dead code. If 107 hasn't landed yet, leave the wrapper for 107's Phase 4
  to remove.
- Audit `goap.rs:615-625` (`accumulate_urgencies` Starvation arm) and lines
  627-636 (the critical-hunger override for Hunting/Foraging). These are
  separate consumers of `starvation_interrupt_threshold` and
  `critical_hunger_interrupt_threshold` — they feed the GOAP urgency vector,
  not the disposition replan. Decision per the 047 pattern: retire the
  *interrupt* arm here; leave the GOAP urgency-accumulation arm in place
  unless a Phase-3 finding shows it's also redundant under the modifier. If
  retiring it, also remove the `UrgencyKind::Starvation` enum variant if it
  has no remaining producers (`grep -rn "UrgencyKind::Starvation" src/`).
- Promote the four constants from 0.0 defaults to the swept-validated values in
  `default_hunger_urgency_*` helpers. Update doc-comments to remove the "ships
  inert" callout.
- The `InterruptReason::Starvation` enum variant itself: keep until ticket 107
  lands (Exhaustion's retirement). At that point an enum-pruning sweep in a
  follow-up retires both Starvation and Exhaustion variants together.

### Phase 5 — Documentation

- **Append** to `docs/balance/106-hunger-urgency.md` (drafted by
  `just hypothesize`) — fill in observation, concordance, decision sections per
  047's structure. Do **not** create a separate hunger-balance doc; follow
  CLAUDE.md "append iterations to the existing thread" rule.
- Update `docs/systems/distress-modifiers.md` (written by ticket 113 — if 113
  is still ready/blocked, coordinate sequencing) to add HungerUrgency to the
  pressure-modifier worked-examples table.
- `just wiki` if any change to `SimulationPlugin::build()` lands (unlikely —
  the modifier registers via `default_modifier_pipeline`, not the plugin).
- `just open-work-index` to regenerate `docs/open-work.md` on status-change
  commits.

## Verification

- **Phase 1 gate:** `just check && cargo nextest run --features all` — full lib
  suite passing including the eight new `hunger_urgency_*` tests, +1 to the
  modifier-pipeline-length assertion. No clippy warnings.
- **Phase 2 gate:** focal-trace sidecar shows the modifier firing on Eat / Hunt
  / Forage with the expected ramp values; the lift is enough to flip the IAUS
  contest under hunger pressure on the focal cat.
- **Phase 3 gate:** `just verdict logs/sweep-…-treatment/<seed>-1` exits 0 on
  at least 2 of 3 seeds; cross-metric read shows no canary regression. Hard
  gates: `deaths_by_cause.Starvation == 0`, `ShadowFoxAmbush ≤ 10`, six
  continuity canaries each ≥ 1, footer line written,
  `never_fired_expected_positives == 0` (or unchanged set, per 088's
  precedent). Expected directional shifts:
  `interrupts_by_reason.Starvation` decreases (or stays at noise floor 0);
  `food_fraction.mean` holds or rises; `Feature::FoodEaten` rises.
- **Phase 4 gate:** post-retirement seed-42 deep-soak — `just soak 42` then
  `just verdict logs/tuned-42-<sha>/` exits 0; same hard gates;
  `interrupts_by_reason.Starvation == 0` (the metric retires with the branch);
  no new entries in `deaths_by_cause` causally tied to slow starvation (e.g.
  cats dying with `hunger < 0.1` from causes other than Starvation).

## Hypothesis

Per CLAUDE.md "drift > ±10% on a characteristic metric requires a hypothesis"
rule:

> **Ecological fact:** A cat at `hunger = 0.4` (urgency = 0.6) is hungry enough
> that food-acquisition behavior should outrank Guarding / Crafting / Patrol
> in normal conditions; a cat at `hunger = 0.15` (urgency = 0.85) is in the
> slow-starvation regime where the legacy interrupt fires, and the substrate
> must already have re-ranked Eat / Hunt / Forage to the top of the contest.
> Pressure (linear ramp) is the right curve shape because hunger builds
> gradually — there's no phase-transition tick where the cat "becomes hungry";
> it's a continuous physiological state.
>
> **Predicted direction + magnitude:** with HungerUrgency active at the
> proposed magnitudes, `interrupts_by_reason.Starvation` decreases by 40-90%
> relative to baseline (band wide because baseline mean is near zero in the
> post-091 regime — single-seed variance dominates). Mechanism: cats hit the
> modifier's 0.6 threshold ~24 ticks of hunger drain *before* hitting the
> legacy interrupt's 0.85 threshold; the +0.40 Eat lift over those 24 ticks
> is enough to win the IAUS contest against non-food dispositions, redirecting
> the cat to a stockpile run before the interrupt would have fired.
> Concordance passes if direction matches and magnitude lands within ~2× of
> band (per repo doctrine — i.e., 20-180% accepted, anything outside requires
> a sub-investigation).

## Out of scope

- ExhaustionPressure (ticket 107) — same playbook on the energy axis.
- ThreatProximityAdrenaline (ticket 108) — perception-coupled lurch on threat
  distance.
- ThermalDistress (ticket 110).
- IntraspeciesConflictResponse (ticket 109).
- Per-disposition exemption-list retirement (ticket 112) — the Resting /
  Hunting / Foraging exemption in `disposition.rs:307-309` outlives this
  ticket; 112 retires the exemption pattern across all branches once each
  axis has its substrate.
- Fight / Freeze valences of acute-health adrenaline (tickets 102 / 105).
- BodyDistressPromotion retirement (ticket 111) — only meaningful once all
  per-axis modifiers are shipped active and the composite-distress path is
  superseded; this ticket leaves 088 in place.
- The plan-completion-momentum gap surfaced by 047's Phase 3 (ticket 118) —
  if Phase 3 of *this* ticket surfaces the same gap on the hunger axis, open
  a parallel ticket; do not fix in-flight here.
- Magnitude tuning beyond Phase 3's swept values — once Phase 4 promotes the
  swept values to defaults, further tuning is a separate balance iteration
  appended to `docs/balance/106-hunger-urgency.md`.

## Log

- 2026-05-01: Opened as the second of four substrate-axis follow-ons from
  ticket 047. The 047 ticket established the playbook; this ticket applies
  it to hunger.
- 2026-05-01: Expanded into a full five-phase workable spec following the
  047 playbook — concrete file paths, function signatures, constant defaults
  (ship-inert + proposed), hypothesize spec filename
  (`docs/balance/106-hunger-urgency.yaml`), focal-trace recipe (seed 42 +
  food-per-meal override fallback when the post-091 baseline carries 0
  Starvation interrupts), explicit cross-metric watch-list including the
  shadow-fox spawn coupling 047 surfaced, and the Phase 4 gating discipline
  that punted 047's interrupt retirement to ticket 119.
- 2026-05-02: **Phase 1 landed** at c83de3cd alongside 107/110 — modifier
  registered (pipeline 12 → 15), 4 ScoringConstants fields added with 0.0
  lift defaults (ships inert; bit-identical baseline), 8 unit tests pass.
  Phases 2-5 (focal trace + hypothesize sweep + interrupt retirement + docs)
  remain — pick up in a follow-up session per the 047 playbook.
- 2026-05-02: Phase 2 prep — corrected the override-fallback recipe in
  §Phase 2 from a non-existent `food.food_per_meal` knob to the real
  `needs.hunger_decay` rate (default 0.1/day at `sim_constants.rs:120`,
  doubled to 0.2/day reproduces the slow-starvation regime). Also flipped
  the focal-cat discovery recipe from raw `jq` to `/logq events` per the
  skill-surface rule. No source change.
