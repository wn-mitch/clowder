# Shadowfox four-drive motivation system — Phase A/B substrate shift (2026-05-14)

Ticket 023. Phase A wires `ShadowFoxDrives` + coherence decay/recovery. Phase B
adds the motivation softmax over four drives (Coherence, Resonance, Dread,
Entropy) selecting from `WildlifeAiState::{Reconstituting, Tending, Haunting,
Seeding}`.

## Hypothesis

Introducing the motivation tick shifts shadow-fox state distribution from
Patrolling-dominated to motivation-state-dominated, suppressing the
`Patrolling → Stalking → Ambush → Banishment` chain that drives the
mythic-texture continuity canary. Long-term Phase D balance will tune drive
weights to restore some banishment probability; Phase B confirms the substrate
fires, not the steady-state balance.

**Causal chain:**

- Motivation tick re-elects state every `shadow_fox_motivation_tick_cadence`
  (16) ticks.
- Coherence (recovery on corrupt tiles) keeps shadow-foxes alive longer →
  cumulative `shadow_fox_spawn_total` drops (the cap stays full).
- Reconstituting / Seeding / Tending all bypass `wildlife_ai`'s Patrolling
  branch, so the existing ward-siege and cat-scent avoidance lifts only fire
  when the pressure-floor fallback lets the patrol logic run.
- With shadow-foxes spending less time in Patrolling, the
  `predator_stalk_cats` system rarely promotes them to Stalking, so
  Ambush/Banishment events drop sharply.

## Prediction

| Field | Direction | Rough magnitude band |
|---|---|---|
| `continuity_tallies.mythic-texture` | decrease | 50–100% |
| `deaths_by_cause.ShadowFoxAmbush` | decrease | 30–100% |
| `shadow_fox_spawn_total` | decrease | 50–95% (cap stays full, no churn) |
| `shadow_foxes_avoided_ward_total` | increase | 100×+ (long-lived shadow-foxes ping-pong) |
| `colony_score.bonds_formed` | increase | 10–40% (less ambient threat) |
| `colony_score.kittens_born` | increase | 0–50% |
| `colony_score.happiness` | decrease | 10–30% (more ambient ward sieges) |

## Observation

Single-seed (42) verification soak, 900s release, post Phase B + pressure-floor
fallback fix. Substrate-fires gate is the primary acceptance bar; multi-seed
sweep deferred to Phase D per
[`feedback_chain_rare_events`](../../.claude/projects/-Users-will-mitchell-clowder/memory/feedback_chain_rare_events.md):
mythic-texture sits at the end of a long causal chain, so structural
verification + single longer soak suffice.

| Field | Baseline | Observed | Δ |
|---|---|---|---|
| `continuity_tallies.mythic-texture` | 43 | 0 | −100% |
| `deaths_by_cause.ShadowFoxAmbush` | 2 | 1 | −50% |
| `shadow_fox_spawn_total` | 30 | 2 | −93% |
| `shadow_foxes_avoided_ward_total` | 2 | 3485 | +174 150% |
| `colony_score.bonds_formed` | 29 | 38 | +31% |
| `colony_score.kittens_born` | 2 | 3 | +50% |
| `colony_score.happiness` | 0.896 | 0.616 | −31% |

Substrate-fires gate per Phase B acceptance criterion: all four motivation
Features fire ≥ 1 across the soak.

| Feature | Count |
|---|---|
| `ShadowFoxReconstitutingEntered` | 396 |
| `ShadowFoxTendingEntered` | 263 |
| `ShadowFoxHauntingEntered` | 25 |
| `ShadowFoxSeedingEntered` | 515 |

Run archive: `logs/tuned-42-023-phase-b-no-fallback/` (pre-fallback) and
`logs/tuned-42/` (post-fallback, current).

## Concordance

**Verdict: concordant (direction-match, magnitude-within-2×)**

- All seven predicted directions match the observed direction.
- All magnitudes fall inside the predicted band or within ~2× of its upper
  bound. The single overshoot — `shadow_foxes_avoided_ward_total` at
  +174 150% vs predicted "100×+" — is direction-correct and magnitude-
  consistent with the long-lived-shadow-fox prediction (only 2 shadow-foxes
  exist, but they spend the entire run bouncing off ward zones).
- Hard-gate canaries hold: survival pass, no Starvation, `ShadowFoxAmbush ≤ 10`.
- Soft-gate fail: `continuity.mythic-texture=0` — confirmed predicted.

## Implications for Phase C / Phase D

- **Phase C** deepens Dread to read cat mood/safety/ally counts. This should
  restore some shadow-fox → cat engagement once vulnerable cats become
  high-pressure Dread targets; expect mythic-texture to recover partially.
- **Phase D** tunes drive weights against the four-artifact methodology.
  Likely tuning: lift `shadow_fox_motivation_min_pressure` (currently 0.05)
  to reduce motivation-driven cycling; OR raise Dread's effective weight so
  shadow-foxes engage cats more aggressively; OR introduce a "patrol decay"
  signal that elevates Patrolling pressure when a shadow-fox has spent too
  many cadences in motivation states.
- The `+174 150%` `shadow_foxes_avoided_ward_total` is a tooling signal, not
  a balance issue — long-lived shadow-foxes simply trigger the avoidance
  feature every patrol step. Phase D should consider rate-limiting this
  Feature emission (record once per ward-avoidance entry, not per tick).
