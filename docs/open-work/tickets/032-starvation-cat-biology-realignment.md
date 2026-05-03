---
id: 032
title: Starvation rebalance — align with IRL cat biology, interesting not cutthroat
status: in-progress
cluster: null
added: 2026-04-26
parked: null
blocked-by: []
supersedes: []
related-systems: [needs.md]
related-balance: [healthy-colony.md]
landed-at: null
landed-on: null
---

## Why

Two converging signals say the starvation pipeline needs a systematic look:

1. **Survival canary brittleness.** `deaths_by_cause.Starvation` is the project's hardest gate (target: 0). Recent seed-42 soaks have ranged 0–9 across re-runs of the same commit (Bevy parallel-scheduler variance). The gate fires often enough to mask a real regression underneath the noise — and survival-tier dominance is precisely what the §7.W fulfillment refactor and `social-target-range.report.md` flagged as the *cause* of the colony's narrow behavioral range.
2. **Reproduction collapse via hunger-floor gating.** `social-target-range.report.md` iter-2 documented that `Mate` is gate-starved on the scoring layer because `breeding_hunger_floor=0.6` is rarely satisfied — cats spend too much time hungry to compose the AND-gate of (hunger>0.6 ∧ energy>0.5 ∧ mood>0.2 ∧ Partners-bond ∧ ...). The cause is upstream: the colony lives in survival mode, never accumulates the welfare slack that makes higher-tier behaviors viable.

Result: starvation isn't a *narrative pressure* — it's an *attractor* the colony settles into. Per `docs/systems/project-vision.md` ("Honest world, no director"), real-world cat biology should drive realistic mortality without forcing every colony into the same brittle survival lock.

## Real cat biology — what we should be modeling

Quick reference (from veterinary literature, summarize before tuning):

- **Adult cats** survive ~1–2 weeks without food (pure starvation) but lose body condition fast — 5–10 days into a fast, fatty liver disease (hepatic lipidosis) becomes a serious risk and recovery requires intervention. **Kittens** survive far less — days, not weeks — and are far less robust to acute hunger.
- Cats are **obligate carnivores**: brief fasts followed by feeding cycles are normal, prolonged caloric deficit is not. They don't graze; they eat-rest cycles.
- **Hunting success** in the wild is ~30–50% per attempt for a healthy adult. Cubs/kittens fail far more.
- **Field cat mortality** is dominated by predation, disease, and accidents — *not* starvation. Starvation is a contributory factor (a cat in poor body condition is more vulnerable to all three) more than a primary cause.
- **Body condition** as a state matters more than discrete hunger. A cat at hunger=0.4 fed ad lib for two days returns to baseline; one at hunger=0.0 for two days is health-compromised even after feeding resumes.

Implication for the sim: starvation as a primary `deaths_by_cause` should be **rare and load-bearing** (a cat that starves is a story, not a statistic), and the *intermediate* states (mild caloric deficit, body-condition decline) should drive most of the welfare-tier knock-on effects — *not* the all-or-nothing `hunger == 0` cliff currently in `src/systems/needs.rs:92`.

## Scope

Each numbered item is a discrete tuning hypothesis. Land them as separate `just hypothesize` runs against a stable baseline.

1. **Soften the starvation cliff.** Today: `if needs.hunger == 0.0` triggers the full health-drain + safety-drain + persistent-mood-penalty cascade. Real biology: fasting cats lose body condition gradually, not at a single threshold. Replace the cliff with a graded `body_condition` axis (or scale `starvation_*_drain` by `(1 - hunger)^2` instead of `hunger == 0`). Predict: `deaths_by_cause.Starvation` *down* by 60–90%; `welfare_axes.acceptance.mean` *up* (cats not in panic mode); mating cadence *up* (hunger floor gate easier to satisfy intermittently).

2. **Stage-stratify mortality.** Kittens should be far more vulnerable to starvation than adults; elders also more vulnerable than prime adults. Today the constants are flat. Add per-life-stage multipliers to `starvation_health_drain` (kitten: 2.0×, juvenile: 1.3×, adult: 1.0×, elder: 1.5×). Predict: kitten mortality *up* during food-pressure events but adult survival *up* overall; the colony tells more stage-driven stories.

3. **Decouple survival from reproduction floor.** Today: `breeding_hunger_floor=0.6` × hunger-decay × starvation cascade collapses the mating window to near-zero on every soak. Lower the floor to 0.4 and observe whether mating cadence rises into the band predicted by `social-target-range`. Predict: `continuity_tallies.courtship` *up* by 50–200%; `kittens_born_total` *up*. Watch for unintended: cats trying to mate while too hungry for the encounter to succeed (gate is real, not arbitrary).

4. **Hunting success rate audit.** If real cats hit ~30–50% per attempt and the sim's `EngagePrey: lost prey during approach` averages 3675 ± 4990 plan failures per 15-min soak (per `healthy-colony.md`), the apparent failure rate is far higher than that — but only because each plan-step is a sub-attempt, not a discrete hunt. Validate: convert the failure tallies into per-discrete-attempt success rate via the events log, compare to the 30–50% target, decide whether prey-targeting needs any change.

5. **Body-condition welfare-axis.** Add a slow-moving `body_condition` welfare axis (akin to `Fulfillment.social_warmth`) that decays under hunger and recovers under feeding, and use it (not raw hunger) as the input to gates that should care about long-term health (mating, mentoring, ward-placement endurance). Predict: gates fire more reliably *across* hunger oscillations; less brittleness from per-tick hunger noise.

## Out of scope

- Any change to the food-economy production side (prey density, kill-yield, cooking).
- Any change to magic / corruption / shadowfox.
- New `deaths_by_cause` causes.
- Per-cat trait modifiers (e.g. "this cat has a slow metabolism").

## Approach

The five scope items above land as a **single ship-inert scaffolding commit**: knobs and code paths exist, but every default reproduces legacy behavior exactly so the change is a no-op until the matching `CLOWDER_OVERRIDES` is set in a hypothesize sweep. Same shape as ticket 111's `hunger_urgency_eat_lift: 0.0`. Sweep verification follows in a second pass once a clean post-111/146 baseline is promoted.

### Operating constraints

- 032 lives on a **fresh jj revision off `main`**, untouched by 111/146 WIP. No edits to `src/ai/modifier.rs`, `src/ai/scoring.rs`, the `NeedsConstants` modifier section of `sim_constants.rs`, `src/systems/disposition.rs`, `src/systems/goap.rs`, or `src/systems/interoception.rs` — those belong to 111.
- All knobs ship inert. `just check && just test` are no-ops. `events.jsonl` footer comparability holds against the pre-032 baseline.
- No fresh `just hypothesize` sweeps in this scaffolding pass — hypothesis YAMLs are drafted but not run. Sweeps wait for the clean post-substrate baseline.

### Execution sequence

#### Step 0 — Branch isolation
`jj new <main> -m "feat: 032 — starvation rebalance scaffolding"`. Verify `jj st` reports zero changes.

#### Step 1 — Item 3 doc-comment update
`src/resources/sim_constants.rs` — `breeding_hunger_floor` default stays at `0.6`. Doc-comment updated to reference 032 + treatment override path `0.4`, citing `docs/balance/social-target-range.report.md` finding 2 (bond progression as bottleneck, not just gate). No `src/ai/mating.rs` edit; it already reads the constant.

#### Step 2 — Item 1 graded drain
`src/systems/needs.rs:91–148` and `NeedsConstants`.

- New constant `starvation_cliff_exponent: f32` whose default reproduces the legacy `if hunger == 0.0` cliff exactly (sentinel value — `f32::INFINITY` works: `(1 − hunger).powf(∞)` is 0 for `hunger > 0` and 1 at `hunger == 0`). Treatment value: `2.0` (quadratic).
- Replace the `let starving = needs.hunger == 0.0` branch with a `cliff_factor` scalar that scales `starvation_health_drain`, `starvation_safety_drain`, and the `starvation_social_multiplier` lerp. Mood-modifier creation gates on `cliff_factor > starvation_mood_threshold` (new knob, default `1.0` — only fires at the legacy cliff) so brief dips don't spam mood modifiers.

**Death-cause discriminator** (`src/systems/death.rs:44–49`) — under graded drain, `health` may bottom while `hunger > 0`; today's discriminator would mis-attribute that to `Injury`. Add `total_starvation_damage` and `total_injury_damage` accumulators on `Health` (`src/components/physical.rs`); rule becomes `health <= 0 ∧ total_starvation_damage > total_injury_damage ⇒ Starvation`. Symmetric writes at injury sites (`src/systems/health.rs`). Legacy cliff still attributes correctly because `total_starvation_damage` is monotonic and only nonzero when `cliff_factor > 0`.

#### Step 3 — Item 2 per-life-stage multipliers
`NeedsConstants` gains four `starvation_drain_multiplier_{kitten,young,adult,elder}: f32` knobs (all default `1.0` ⇒ no behavior change). Treatment values: `2.0 / 1.3 / 1.0 / 1.5`. The `decay_needs` per-cat loop already binds `age` (line 99); look up `LifeStage` (`src/components/identity.rs:31–67`) and multiply `starvation_health_drain` and `starvation_safety_drain` by the matching constant (compounds with `cliff_factor` from Step 2).

#### Step 4 — Item 5 `body_condition` welfare axis (scaffold)
Mirror the `Fulfillment.social_warmth` pattern (`src/components/fulfillment.rs:20–26` + `src/systems/fulfillment.rs:24–58, 70–99`).

- `Fulfillment` gains `body_condition: f32` (default `1.0`, range `[0,1]`).
- `FulfillmentConstants` gains `body_condition_decay_per_unit_hunger_deficit` and `body_condition_recovery_per_unit_satiation` (both default `0.0` ⇒ axis ships flat at 1.0).
- New `body_condition_decay` system (run-if guard: `body_condition_decay_per_unit_hunger_deficit > 0.0`); new recovery hook on `Eat` / `Feed` step success.
- Gate-rewire is opt-in: `use_body_condition_for_breeding_gate: bool` (default `false`). When set, `is_sated_and_happy` (`src/ai/mating.rs:130–136`) reads `body_condition` instead of `hunger`. Same toggle pattern reserved for any other long-term-health gate (mentoring eligibility, ward-placement endurance) when those land.

#### Step 5 — Item 4 hunting-success audit (data only)
No code change. Use `/logq events <run-dir>` filtered to `EngagePrey` outcomes on existing `logs/tuned-42*` and `logs/sweep-*/42-*` directories. Group plan-step events into discrete hunt attempts (start = approach-begin, end = success / loss / abandon). Compute per-attempt success rate; compare to the 30–50% real-cat target. Findings into `docs/balance/starvation-rebalance.md` *Hunting-success audit* section. If in band, item 4 closes with no change. If out of band, open a follow-on sub-ticket naming the responsible step (likely under `src/steps/hunt/`).

#### Step 6 — Hypothesis YAMLs (drafted, not run)
Under `docs/balance/hypotheses/`:

- `032-3-breeding-floor.yaml` — `scoring.breeding_hunger_floor: 0.4`. Predict `continuity_tallies.courtship` ↑ `[25, 300]%` (band wider than the §Scope 50–200% because current is ~0).
- `032-1-soften-cliff.yaml` — `needs.starvation_cliff_exponent: 2.0`. Predict `deaths_by_cause.Starvation` ↓ `[60, 90]%`; `welfare_axes.acceptance.mean` ↑ `[10, 40]%`.
- `032-2-stage-multipliers.yaml` — kitten `2.0×`, young `1.3×`, adult `1.0×`, elder `1.5×`. Multi-metric prediction: adult `Starvation` deaths ↓; kitten mortality during food pressure ↑.
- `032-5-body-condition.yaml` — enable axis decay/recovery + `use_body_condition_for_breeding_gate: true`. Predict gate-firing variance ↓ across hunger oscillations.

Run order when sweeps fire (post-111/146 baseline): `3 → 1 → 2 → 5` — cheapest constant first, biggest refactor last.

#### Step 7 — Investigation pass on existing logs
Use the skill surface only (`/logq /inspect /verdict /fingerprint /explain`, per `feedback_use_skill_surface.md`). Six tasks consolidated into `docs/balance/starvation-rebalance.md`:

1. **Variance** — distribution of `Starvation`, `kittens_born`, `courtship`, welfare means across all `logs/tuned-42*` runs.
2. **Hunger trajectory** — how often / how long are cats at `hunger == 0.0`? Decides whether item 1's quadratic does meaningful work, or whether item 5 is the load-bearing change.
3. **Cliff vs chronic mortality** — for each `DeathCause::Starvation` cat, last-100-tick hunger / health trace.
4. **Mating-gate veto** — for cats scoring Mate but not firing, which AND-leg vetoed?
5. **Per-stage death distribution** — kitten / young / adult / elder mortality breakdown across soaks.
6. **Hunting-success rate** — Step 5's audit.

#### Step 8 — Verification (this scaffolding pass)
- `just check` — clippy + step-resolver linter pass.
- `just test` — unit tests pass (no test changes expected).
- `just headless` 30s smoke run — footer line written, no never-fired-positive regression, no crashes.
- `CLOWDER_OVERRIDES='{"needs":{"starvation_cliff_exponent":2.0}}' just headless` brief comparison run — confirms the override path wires through and graded drain visibly fires.
- `yq` parse on the four hypothesis YAMLs.

#### Step 9 — Ticket + index update + commit
- This ticket: `status: in-progress`, append `## Log` line dated 2026-05-02.
- `just open-work-index` regenerates `docs/open-work.md`.
- Single conventional commit: `feat: 032 — starvation rebalance scaffolding (ship-inert)`.

### Acceptance bar (per item, when sweeps run later)

Predicted-direction match + magnitude in band + no survival canary regression (`Starvation == 0` hard, `ShadowFoxAmbush ≤ 10`) + at least one continuity canary holding or improving.

## Verification (final, post-sweep)

- `just verdict` and `just fingerprint` both `pass` on the post-rebalance seed-42 soak.
- `deaths_by_cause.Starvation` mean ≤ 0.5 (below the noise band).
- `continuity_tallies.courtship` ≥ 5 (rising from current ~0).
- No survival canary regresses.
- Welfare-axis means (acceptance, respect, mastery, purpose) all *rise* — the test that the colony has slack to express higher-tier behaviors.

## Critical files

- `src/systems/needs.rs:91–148` — starvation cascade (Step 2 + Step 3).
- `src/systems/death.rs:44–49` — `DeathCause::Starvation` discriminator (Step 2 update).
- `src/components/physical.rs` — `Health`; new `total_starvation_damage` / `total_injury_damage` accumulators (Step 2).
- `src/systems/health.rs` — injury sites; symmetric writes for `total_injury_damage` (Step 2).
- `src/resources/sim_constants.rs` — `NeedsConstants` (Step 2 + Step 3); `FulfillmentConstants` (Step 4); `ScoringConstants` doc-comment (Step 1).
- `src/components/identity.rs:31–67` — `LifeStage` enum (Step 3 indexing).
- `src/components/fulfillment.rs` + `src/systems/fulfillment.rs` — exemplar pattern + new `body_condition` decay/recovery (Step 4).
- `src/ai/mating.rs:130–136` — `is_sated_and_happy`; reads new gate-toggle knob (Step 4).

## Log

- 2026-04-26: Ticket opened. Triggered by tooling pass surfacing two intersecting bugs: (a) `social.bond_proximity_social_rate` mis-spelled but `nearest` suggested the right path; (b) `fingerprint`'s silent-subsystem check now flags the ward-pipeline collapse on iter2 logs. Both in turn made the starvation-as-attractor pattern visible across the colony's whole continuity register.
- 2026-05-02: Approach hardened into an executable, ship-inert scaffolding plan (Steps 0–9). All five scope items implemented behind `CLOWDER_OVERRIDES`-friendly knobs at legacy-reproducing defaults; hypothesis YAMLs drafted (run order `3 → 1 → 2 → 5`); sweeps deferred until 111+146 land and a clean post-substrate baseline is promoted. 032 work lives on a fresh jj revision off `main`, untouched by the 111 WIP at `1f3153cf`.
- 2026-05-02: Scaffolding landed (this commit). `NeedsConstants` gains `starvation_cliff_use_legacy: true` (sentinel reproducing legacy cliff), `starvation_cliff_exponent: 2.0`, `starvation_mood_threshold: 0.0`, four `starvation_drain_multiplier_*` (all `1.0`), and `starvation_attribution_threshold: 0.1`. `Health` gains `total_starvation_damage` accumulator. `decay_needs` rewires the cascade through a unified `cliff_factor`; `check_death` discriminator gates on the legacy bool. `Fulfillment` gains `body_condition` axis; `FulfillmentConstants` gains decay/recovery/pivot knobs (all `0.0`) plus `use_body_condition_for_breeding_gate` toggle (`false`). `update_body_condition` system registered with run-if guard. Mating gate `is_sated_and_happy` reads `body_condition` when toggle on. Hypothesis YAMLs at `docs/balance/032-{1,2,3,5}-*.yaml`. Investigation thread opened at `docs/balance/starvation-rebalance.md` — surfaced **(a)** sim hunt success rate ≈25.6% vs 30–50% real-cat target (item 4 closes inconclusive, follow-on suggested for hunt-success disambiguation); **(b)** `continuity_tallies.courtship` is *not* zero (~999) — the reproduction-collapse bottleneck is on the *completion* side (`MatingOccurred` never-fired), not the gate. Item 3's predicted magnitude needs reframing accordingly. Default-config `just check`/`just test` clean (1798 unit tests pass). Override path validated via `CLOWDER_OVERRIDES='{"needs":{"starvation_cliff_use_legacy":false,"starvation_cliff_exponent":2.0}}'` smoke run — header `applied_overrides` confirms wire-up. **Next:** wait for clean post-111/146 baseline, then `just hypothesize docs/balance/032-3-breeding-floor.yaml` first.
