---
id: 537
title: Prey mob-and-harry — dense same-kind group elects a Mob DSE against a weak/injured predator, converges and applies deterrence push into predator retreat scoring (first symmetric prey->predator action; enriched by 245 body-cue reads)
status: ready
cluster: wildlife
initiative: [predator-prey-dynamics]
added: 2026-07-09
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
Every prey behavior in the sim is *reactive and asymmetric* — prey only ever
receive predation (Bolt / Scatter / BurstFlight). No prey ever acts *on* a
predator. But real prey do: songbirds mob an owl, crows dive a hawk, rats swarm
a cornered fox. When a group is dense enough and the predator is weak or
vulnerable (low base threat-power, grounded, or injured), the group's best move
is to converge and harass — not damage, but raise the predator's fear/retreat
pressure until it leaves. This is the **first symmetric prey→predator
interaction** in the codebase and the only idea from the `/ideate` prey pass that
opens the empty SYMMETRIC column of the cardinality×symmetry chart.

## Scope
- New `Mob` prey DSE alongside `prey_bolt` / `prey_scatter_group`, eligible only
  when: (same-kind group density ≥ threshold) AND (predator is weak/vulnerable).
- Weakness signal, cheapest-first: `WildSpecies::default_threat_power` low and/or
  predator `Health` low. Richer read — a *limping* or grounded predator — rides
  ticket 245's body-cue channel (`LimpingGait`), which is an enrichment, not a
  hard prerequisite (see Log).
- Resolution: mobbers move *toward* the predator and write a deterrence /
  retreat push into the predator's scoring (raise its retreat/flee score), not
  HP damage.
- `Feature::PreyMobElected` (+ narrative template) for observability.
- Knobs in `PreyConstants`: density threshold, predator-weakness threshold,
  deterrence-push magnitude, mob break-off condition.

## Out of scope
- Prey dealing actual damage / killing predators — deterrence only.
- Mobbing shadowfoxes (spectral, corruption-born) — start with fox/hawk/snake;
  shadowfox interaction is a separate balance question given its dedicated canary.
- Cross-species mobbing (mixed prey coalition) — single-species only.

## The expensive part (read before building)
This DSE **writes into predator scoring** — a genuine feedback loop between the
prey subsystem and the predator planners (fox/hawk/snake GOAP + retreat). Two
risks the `rank-sim-idea` triage flagged (priced R=2 / H=2):
- It can destabilize the `ShadowFoxAmbush <= 10` and predator-mortality balance
  even though shadowfoxes are out of scope — predators that retreat more change
  the whole hunt economy. Verify predator-side canaries, not just prey-side.
- The deterrence push must price the predator-exposure tradeoff it changes
  (memory `project_l3_patrol_absorption_cascade`): elevating prey boldness must
  surface its cost, not just suppress predator success.

## Current state
Nothing landed. Insertion point is the 266 election in `try_elect_escape`
(`src/systems/prey.rs`) — Mob is an alternative to Bolt/Scatter when the
eligibility gate is met. Predator retreat scoring lives in the fox/hawk/snake
DSEs / `systems/wildlife.rs`. 245 (body-cue channel, blocked-by 243) is the
enrichment source for the injured-predator read.

## Approach
1. Add the `Mob` DSE + scalar (group density × predator weakness); ship
   dispatcher branch + `prey_ctx_scalars` entry + election arm together
   (silent-inert rule, memory `project_score_actions_dispatch_antipattern`).
2. Deterrence write: a named push into predator retreat scoring — surface it in
   the predator's L2/resolver trace (substrate-over-hacks, design pillar 2), not a
   hidden side-channel.
3. `Feature::PreyMobElected` defaults `expected_to_fire_per_soak=false` until
   observed (memory `feedback_new_features_default_expected_false`).
4. Land the threat-power/health gate first (no 245 dependency); layer the
   `LimpingGait` body-cue read as a follow-on once 245/243 land.

## Verification
- Focal/narrative: a dense rat group harasses a low-health fox, which retreats;
  a healthy fox at the same density is NOT mobbed (eligibility gate holds).
- `Feature::PreyMobElected` fires ≥1× on seed-42; lift expected flag after.
- `just verdict`: `ShadowFoxAmbush <= 10`, `Starvation == 0`, predator
  populations do not crash from over-deterrence. Four-artifact balance framing.

## Log
- 2026-07-09: Opened from `/ideate` prey-ecology pass (idea #2). Kept `ready`
  rather than blocked-by 245: the core mob eligibility works off threat-power +
  health today; the injured-predator body-cue read is a 245 enrichment follow-on.
  Highest-scrutiny of the five prey tickets — writes into predator scoring.
