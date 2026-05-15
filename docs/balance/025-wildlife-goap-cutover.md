# Hawk + snake GOAP cutover — Phase 2 substrate shift (2026-05-15)

Ticket 025. Phase 1 (decision substrate: `HawkDomain` / `SnakeDomain`
planners, hawk/snake DSEs, softmax scoring) landed earlier. Phase 2 wires
the runtime: `HawkState` / `SnakeState` lifecycle components, dedicated
`hawk_goap` / `snake_goap` systems, GOAP plan resolution via step
resolvers, and a final atomic cutover that retires the legacy
`Circling` / `Waiting` `wildlife_ai` arms for hawks and snakes.
ShadowFoxes keep the legacy state machine.

## Hypothesis

Replacing the hardcoded `Circling` / `Waiting` movement loop with a
Maslow-structured GOAP loop shifts hawk and snake behavior from
species-agnostic loitering to disposition-driven predation. Hawks gain
Hunting / Soaring / Fleeing / Resting cycles; snakes gain Ambushing /
Foraging / Basking / Fleeing with thermoregulation.

**Causal chain:**

- Edge-spawn and initial-spawn paths now attach
  `HawkState` / `HawkAiPhase` / `HawkNeeds` / `HawkPersonality` (and snake
  equivalents) at spawn time.
- Each tick `hawk_evaluate_and_plan` (and snake equivalent) scores
  dispositions via the L2 DSE registry, softmax-picks one, and inserts a
  `HawkGoapPlan` (or `SnakeGoapPlan`). The resolver dispatches the
  current step (`SoarTo` / `SpotPrey` / `DiveAttack` / `Rest` / `FleeSky`
  for hawks; `SlideTo` / `SetAmbush` / `Strike` / `Bask` / `Retreat` for
  snakes) to its step resolver. Witnessed step outcomes record positive
  Features (`HawkSpottedPrey`, `HawkDiveLanded`, `SnakeStruckPrey`,
  `SnakeAmbushed`, `SnakeBasked`, …).
- Hunger and warmth decay each tick via `hawk_needs_tick` /
  `snake_needs_tick`; sustained `hunger >= 1.0` triggers
  `hawk_lifecycle_tick` / `snake_lifecycle_tick` to emit `HawkDied` /
  `SnakeDied` and mark the entity `Dead`.
- `predator_hunt_prey` was extended to apply species-specific satiation
  on a successful kill, gated on the predator's AiPhase. Kill-attribution
  Features remain in `predator_hunt_prey`; the *event* Features
  (`HawkDiveLanded` etc.) fire from the step resolvers regardless of
  kill outcome (so a missed dive still witnesses the dive).

## Prediction

| Field | Direction | Rough magnitude band |
|---|---|---|
| `deaths_by_cause.WildlifeCombat` | unclear ± | ±30% (within ±100% noise band) |
| `never_fired_expected_positives` | excludes 10 new positives | 0 (all stay opt-out at land) |
| Survival canaries (`deaths_by_cause.Starvation`, `ShadowFoxAmbush`) | unchanged | within ±10% |
| Continuity canaries (grooming · play · mentoring · courtship · mythic-texture) | unchanged | within ±10% |
| `MatingOccurred`, `KittenBorn` cascades | unchanged | within ±10% |

The cutover is **a comparability break by construction**: `SimConstants`
gains two `#[serde(default)]` sub-structs (`hawk_ecology` /
`snake_ecology`), the wildlife `.chain()` nests three per-species AI
sub-chains plus a lifecycle sub-chain, and spawn-time
`HawkPersonality::random` / `SnakePersonality::random` consume RNG. All
three shift seed-42 results. The new baseline is the first
post-cutover `just soak 42` archive.

## Observation

*Pending: run `just soak 42` after this commit lands, observe firing of
the four trunk positives, then promote `HawkSpottedPrey`,
`HawkDiveLanded`, `SnakeStruckPrey`, `SnakeAmbushed` to
`expected_to_fire_per_soak() => true` in a follow-on commit. Re-baseline
via `just promote logs/tuned-42 wildlife-goap-cutover`.*

## Concordance

*Pending: after observation, fill in the four-artifact concordance
check. If `deaths_by_cause.WildlifeCombat` drift exceeds ±30%, escalate
to `just hypothesize` for a multi-seed sweep.*

## Follow-ons

Per ticket §11, the following tickets open against the parent landing:

- **Balance iteration** — first tuning pass on
  `HawkEcologyConstants` / `SnakeEcologyConstants` after multi-seed
  sweep.
- **Perceptual-fact docs** — `docs/systems/hawk-ecology.md` and
  `docs/systems/snake-ecology.md` once behavior is observed.
- **Sensitivity-map rebuild** — `just rebuild-sensitivity-map` to
  populate `just explain`'s rho column for the 25 new constants.
- **Extract `shadow_fox_ai`** — pull the ShadowFox-only branches of
  `wildlife_ai` into a dedicated system; the function is the only
  legacy state-machine path post-cutover.
- **Narrative coverage** — narrative templates for `HawkSpottedPrey`,
  `HawkDiveLanded`, `SnakeStruckPrey`, `SnakeBasked`, `SnakeAmbushed`.
