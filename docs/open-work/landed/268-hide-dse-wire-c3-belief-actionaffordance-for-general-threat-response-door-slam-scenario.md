---
id: 268
title: Hide DSE — wire C3 Belief + ActionAffordance for general threat-response (door-slam scenario)
status: done
cluster: combat-threat
orchestration: substrate-sensitive
initiative: []
added: 2026-05-10
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: c4c95b2d
landed-on: 2026-05-19
---

## Why

The session-plan grounding example (Simba sprinted behind couch when Cal startled at door slam, then resumed prowling when Cal didn't escalate) requires Hide DSE to fire from C3 Belief facets (`MentalModel<EnvironmentalContext>.recency_of_threat_cue` lifts) gated by ActionAffordances (`Affordance(Freeze, self, EnvCtx)` is high when nearby cover is available). Hide DSE already exists (ticket 104, landed) and serves the predator-avoidance "remain still and hope" valence; this ticket adds general-threat-response Belief/Affordance considerations so Hide also fires for ambient shocks and conspecific-as-sensor confirming-evidence cascades.

REFRAMED: original session plan called for "Freeze DSE (new)" as ticket ζ. Audit during ticket-opening surfaced 104 (Hide DSE, landed) + 142 (IntraspeciesConflictResponseFreeze Modifier on Hide). Per pillar-2 substrate-over-hacks doctrine: don't proliferate new DSEs when an existing one serves. This ticket is the Belief/Affordance consumer wiring on Hide; complements 142 (intraspecies-conflict Modifier) and 170 (HideEligible authoring system).

## Scope

- **Hide DSE consideration additions** (`src/ai/dses/hide.rs`):
  - `Affordance(Freeze, self, target=NearestThreat or EnvironmentalContext)` axis from substrate 261.
  - `MentalModel<X>(target).recency_of_threat_cue` axis from substrate 258 (X = whichever model the threat resolves to: Predator if a creature, EnvironmentalContext if ambient).
  - `MentalModel<X>(target).perceived_intent_clarity` axis — when a target's intent is *unclear* (not actively pursuing me but not clearly benign), Hide is the safest action; when intent is clear (definitely hunting OR definitely indifferent), Hide is wrong (Flee or resume normal behavior, respectively).
- **AmbientShock-driven elevation**: when a `WitnessableEvent::AmbientShock` is heard, the resulting belief lift on `EnvironmentalContext.recency_of_threat_cue` raises Hide's score (via the new axes); when no further evidence arrives, the fast-timescale decay drops Hide back below threshold and other DSEs reclaim L3.
- **Conspecific-as-sensor confirming evidence**: when belief integrator processes `WitnessedConspecificStartle` from another cat, the lift on `EnvironmentalContext.recency_of_threat_cue` (weighted by relay credibility per decision 16 in session plan) feeds into Hide's score the same way direct shock perception does.
- **Per-axis tunables** in `SimConstants`.

## Out of scope

- The Hide DSE itself (ticket 104 owns; landed).
- HideEligible authoring system (ticket 170 owns).
- IntraspeciesConflictResponseFreeze Modifier (ticket 142 owns; orthogonal Modifier on the same DSE).
- Other threat-response DSEs (Flee — ticket 263; Fight — sibling EngageThreat ticket 269).
- The Belief substrate (258).
- The ActionAffordances substrate (261).
- The cue substrates (242 body cues, 244 audible cues).

## Current state

- Blocked-by 258 (Belief substrate) + 261 (ActionAffordances substrate).
- 104 Hide DSE landed (per ticket 142's Log: "104 (Hide DSE) landed at 2a68f595 in the same Wave 1 batch").
- 142 (intraspecies-conflict Freeze Modifier on Hide) is `ready`, blocked on 109's substrate work. Complementary, not blocking.
- 170 (HideEligible authoring system) is `ready`. Adds the L1 marker that gates Hide; this ticket reads its output indirectly via the existing Hide DSE.
- The door-slam grounding example trace lives in ticket 258's body and the session plan; this ticket's verification scenarios are the ones that prove the trace works end-to-end.

## Approach

1. Read 104, 142, 170 to confirm the Hide DSE consideration shape and how 142's intraspecies Modifier composes.
2. Add new considerations on the existing Hide DSE struct.
3. Wire the considerations to read the Belief and Affordance APIs from 258 and 261.
4. Verify with door-slam scenario microexperiment (per session plan's `belief_ambient_shock_with_relay_confirmation` scenario, sketched in ticket 258).

## Verification

### Scenario microexperiments (≤ 3s, under `src/scenarios/`)

The canonical door-slam scenario is owned by ticket 258 (`belief_ambient_shock_with_relay_confirmation`) — substrate-side correctness lives there. This ticket adds DSE-side scenarios:

- `hide_fires_on_ambient_shock` — emit `WitnessableEvent::AmbientShock`; verify Hide DSE elevates, wins L3 when cover is nearby; resolves to nearest cover.
- `hide_drops_when_threat_decays` — same setup; advance 60+ ticks with no further shock; verify Hide drops below threshold and other DSE reclaims L3.
- `hide_no_fire_on_clear_threat` — predator clearly committed to Chase (high `perceived_intent_clarity`); verify Hide does NOT fire (Flee should be the right pick — testing the orthogonality between Hide and Flee under different intent-clarity regimes).

### Soak gates

- `just soak-trace 42 <focal>` + `just verdict` confirms no canary regression.
- Per `just q anomalies`, Hide doesn't absorb >10% of elections (it's a transient response, not a baseline behavior).

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- · **270** (ready, combat-threat, score 0.90) — EngageThreat split from Patrol DSE (256 R6 follow-on with Belief + ActionAfford…
- · **267** (ready, combat-threat, score 0.88) — Conflict-low DSEs — Threaten / Posture / Hiss escalation rungs (cheap pre-Fight…
- · **265** (ready, wildlife, score 0.87 (cross-cluster)) — Wildlife symmetric DSE consumers wire belief + affordance (fox, hawk, snake, sh…

<!-- linkages:end -->
## Log

- 2026-05-10: opened sibling-to-258. REFRAMED from original "Freeze DSE (new)" plan slot — Hide DSE already exists (104 landed); this is the Belief/Affordance consumer wiring on it. Complements 142 (intraspecies-conflict Modifier on same DSE). Session plan: `~/.claude/plans/after-working-256-i-dreamy-fiddle.md`.
- 2026-05-19: accuracy audit pass — no blockers; related infrastructure (104, 142, 170) confirmed landed or ready per stated design; hide.rs file verified; scenario stub structure confirmed.
