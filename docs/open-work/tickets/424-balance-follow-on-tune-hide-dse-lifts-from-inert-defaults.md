---
id: 424
title: Balance follow-on — tune Hide DSE lifts from inert defaults
status: ready
cluster: combat-threat
orchestration: substrate-sensitive
initiative: []
added: 2026-05-19
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

The 170+142+268 Hide-activation trio landed
2026-05-19 with **all lift constants and conditional-axis weights
defaulting to 0.0** — substrate wiring only, no behavioral change. This
balance ticket tunes the defaults out of inert so Hide DSE actually
fires under spec'd conditions ("remain still and hope" predator
response + subordinate-Freeze valence on social-distress + general-
threat C3 Belief/Affordance perception).

The inert defaults form a deliberate sequencing discipline (same as
ticket 263's Flee-affordance pattern): substrate lands first, the
balance pass that sweeps lift magnitudes to behavioral values lands
second. Lets us bisect substrate-correctness failures from balance
failures.

## Scope

Tune the following constants from 0.0 to behavioral values, using
`just hypothesize` with the four-artifact methodology (hypothesis ·
prediction · observation · concordance per CLAUDE.md "Verification"):

- **`intraspecies_conflict_freeze_hide_lift`** (142). Subordinate-
  Freeze valence lift on Hide for cats under `social_status_distress
  >= threshold`. Default 0.0; pressure-modifier-family magnitudes
  are ~0.30 (109 Phase A's Flight lift), with Hide-cap-at-0.5
  considered.
- **`hide_affordance_freeze_weight`** (268). Conditional axis weight
  on the `Affordance(Freeze, self, NearestThreat)` consideration.
  263's `flee_affordance_weight` tunes from 0.0 — mirror its
  hypothesis methodology.
- **`hide_recency_of_threat_cue_weight`** (268). Conditional axis
  weight on the recency-of-threat-cue MentalModel facet (from
  PredatorBeliefs OR ContextBeliefs[HereNow] max).
- **`hide_perceived_intent_clarity_weight`** (268). Conditional axis
  weight on the perceived-intent-clarity MentalModel facet. **Note:
  the inversion direction is a design choice** — the substrate
  surfaces raw clarity; the activation thread picks whether Hide
  scores higher under *unclear* intent (substrate-shape preferred —
  freeze when predator's commitment ambiguous) or under *clear*
  intent (less coherent semantically). This ticket settles the
  direction.

## Out of scope

- The substrate wiring itself — owned by 170 / 142 / 268
  (all landed).
- Production emitters for `AmbientShock` and
  `WitnessedConspecificStartle` events — those are spec'd in 268
  but not wired in v1. Open as separate follow-ons if the scenarios
  prove the consumer wiring needs production emission.
- Cover-availability-map performance refactor — owned by
  423.
- Promoting `Feature::HideFreezeFired` from rare-event class to
  `expected_to_fire_per_soak() => true`. Data-driven decision: after
  this ticket tunes the lifts and the seed-42 soak shows ≥1 fire/soak
  consistently, promote in a separate commit.

## Current state

- 170 / 142 / 268 landed alongside 2026-05-19.
- The 2026-05-15 baseline (`tuned-42-095-phase-1a-shadow`) is stale —
  promote a fresh baseline at HEAD (`just promote logs/tuned-42
  <label>`) before running the hypothesis sweeps, otherwise the
  drift signals will commingle 7+ intervening landings' cumulative
  effect with the lift activation.

## Approach

1. **Promote a fresh baseline** at HEAD (post-170+142+268-land
   commit) so this ticket's hypothesis sweeps measure pure
   activation-lift effect, not cumulative drift since 2026-05-15.
2. **Hypothesis specs** under `docs/balance/hide-activation/`:
   - One spec per constant being tuned, OR a combined spec sweeping
     all four together (recommended if the constants are designed
     to compose).
   - Hypothesis shape: `{ Hide DSE eligible cats under threat AND
     low-cover availability ⇒ actions.Hide.fraction lifts from 0%
     to ~2-5% per 10kt }`. Adjust target per the per-soak
     observability budget.
3. **Sweep** via `just hypothesize <spec.yaml>` — runs baseline +
   treatment + concordance check + draft balance doc.
4. **Verify continuity canaries hold** — Hide elevation could
   absorb L3 share from Flee / Combat; drift > ±10% on adjacent
   actions needs the four-artifact methodology per CLAUDE.md.
5. **Settle the intent-clarity inversion direction** — run a
   focal-cat scenario with two cats under identical threats, one
   with high `perceived_intent_clarity` and one with low; verify
   Hide wins under the lower-clarity cat (substrate-shape pick).

## Verification

- `just hypothesize` produces the four-artifact bundle for each
  tuned constant.
- `just soak-trace 42 Simba` + `just verdict` against the fresh
  baseline — survival canaries hold, drift on adjacent actions
  hypothesis-or-noise.
- The three 268 scenario microexperiments (`hide_fires_on_ambient_shock`,
  `hide_drops_when_threat_decays`, `hide_no_fire_on_clear_threat`)
  pass once the wiring + lifts are tuned.

## Log

- 2026-05-19: opened alongside 170 / 142 / 268 landings per
  CLAUDE.md "Antipattern migration follow-ups are non-optional."
