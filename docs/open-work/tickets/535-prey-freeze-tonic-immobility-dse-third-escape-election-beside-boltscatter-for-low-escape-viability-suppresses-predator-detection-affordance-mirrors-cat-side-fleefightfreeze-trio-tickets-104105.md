---
id: 535
title: Prey freeze / tonic-immobility DSE — third escape election beside Bolt/Scatter for low-escape-viability, suppresses predator detection affordance (mirrors cat-side Flee/Fight/Freeze trio, tickets 104/105)
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
Prey have exactly one answer to a threat: run (Bolt / Scatter / BurstFlight).
But fleeing is the wrong move when escape viability is low — predator too close
or too fast, no cover to reach — where the real prey behavior is to hold
absolutely still (crypsis / tonic immobility) and drop out of the predator's
perception rather than bolt into a chase it will lose. Cats already have this:
the Flee / Fight / **Freeze** predator-response trio, `AcuteHealthAdrenalineFreeze`
(ticket 105) riding the `Hide` DSE (ticket 104), gated on a "cornered enough"
viability threshold (`src/ai/modifier.rs`). Prey have no equivalent. This ticket
gives prey the third response, lifting the cat-side 104/105 design as the
template.

## Scope
- New `PreyAiState::Frozen { from, ticks }` and a `Freeze` prey DSE registered
  alongside `prey_bolt` / `prey_scatter_group`.
- `Freeze` wins the 266 escape election when bolt viability is low (short head
  start + high believed lethality + poor escape-speed ratio / no reachable
  cover) — the inverse of the condition that favors Bolt.
- While Frozen, the prey suppresses its contribution to the predator's detection
  / target affordance (it is harder to notice), and does not move.
- Exit: predator leaves sensing range → `Idle`/`Grazing` with alertness reset;
  predator closes to strike range → the freeze has failed and normal resolution
  (or death) proceeds — freeze is a gamble, not a shield.
- Freeze-election threshold constant in `PreyConstants`.

## Out of scope
- Fish `Stationary` FleeStrategy is unrelated (fish never flee); this is an
  *elected* gamble for ground-flee species (`Standard` / `SeekCover`).
- Predator "flush the frozen prey" counter-behavior — later enrichment.
- Body-cue observation of a frozen prey by predators — that rides 245's channel.

## Current state
Nothing landed. The 266 escape election (`try_elect_escape` in
`src/systems/prey.rs`) is the insertion point — Freeze is a third arm in the same
one-argmax-per-(prey,threat) election. Cat-side 104/105 in `src/ai/modifier.rs`
is the reference for the viability gate and the "freeze beats fight/flee when
cornered" shape.

## Approach
1. Add the `Frozen` state variant + the `Freeze` DSE with its scalar
   (escape-viability inverted) — ship the dispatcher branch and `prey_ctx_scalars`
   entry in the same commit (silent-inert rule, memory
   `project_score_actions_dispatch_antipattern`).
2. Add the election arm in `try_elect_escape`; record a `Feature::PreyFreezeElected`
   (new Feature defaults `expected_to_fire_per_soak=false` until seed-42 observes
   ≥1 firing — memory `feedback_new_features_default_expected_false`).
3. Detection suppression: reduce the frozen prey's weight in whatever the predator
   reads to acquire/keep a target while Frozen.

## Verification
- Focal trace: a cornered prey (cat adjacent, no cover) elects `Freeze` instead
  of a losing Bolt; predator's target affordance on it drops; some frozen prey
  survive by not being noticed, some die when the predator closes anyway.
- `Feature::PreyFreezeElected` fires ≥1× on seed-42; lift its expected flag after.
- `just verdict`: `ShadowFoxAmbush <= 10` and `Starvation == 0` hold — freeze
  must not become a free survival cheat that starves predators.

## Log
- 2026-07-09: Opened from `/ideate` prey-ecology pass (idea #7). Confirmed
  cats have Freeze (104/105) and prey do not — user-flagged asymmetry. Cheapest
  and most isolated of the five prey tickets from this pass.
