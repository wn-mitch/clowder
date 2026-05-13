---
id: 263
title: 256-cluster DSE consumers wire belief + affordance axes (Flee, Patrol, Hunt with Stalk/Chase/Pounce sub-actions)
status: blocked
cluster: ai-substrate
added: 2026-05-10
parked: null
blocked-by: [261]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Smallest-blast-radius first consumer of the C3 belief substrate (258) + ActionAffordances substrate (261). Wires the new MentalModel facets and per-action affordance reads into Flee, Patrol, and Hunt — the three DSEs at the heart of the L3 patrol-absorption cascade documented in memory `project_l3_patrol_absorption_cascade`. Decomposes Hunt's predation phase into observable sub-actions (Stalk/Chase/Pounce) so the affordance layer can differentiate "stalk this alerted prey (low affordance) vs chase it (high affordance)."

This is the consumer ticket where the substrate's payoff first appears in soak gates: ShadowFoxAmbush canary should hold or improve once Patrol can read `Affordance(Patrol-route, ..., Ambush-likelihood)` and route around predator-rich tiles, and Flee/Hunt can read `MentalModel<Predator>.recency_of_threat_cue` to score ground truth instead of binary presence.

## Scope

- **Flee DSE** (`src/ai/dses/flee.rs`): adds two new considerations:
  - `Affordance(Flee, self, target=NearestThreat)` — reads from substrate 261; supersedes/extends the existing `flee_threat_distance` Power-Invert curve.
  - `MentalModel<Predator>(target).perceived_violence_capability` — reads from substrate 258; gate on belief, not just presence.
- **Patrol DSE** (`src/ai/dses/patrol.rs`): adds:
  - `MentalModel<Location>(patrol_target).recency_of_threat_cue` and `.perceived_ambush_likelihood` (new location facet) — Patrol scoring is gated on perceived safety of the perimeter sector, not just route cost.
  - `Affordance(Patrol-route, self, target=PatrolSector)` — reads from substrate 261; routes through low-affordance sectors only when no high-affordance alternative exists.
- **Hunt DSE** (`src/ai/dses/hunt.rs`): decomposes predation into sub-actions:
  - Hunt's resolver chain (Approach → Pursue → Strike → Eat) is reframed as `Stalk → Chase → Pounce → Eat` with sub-action selection driven by per-sub-action affordance reads.
  - DSE consideration adds `Affordance(Stalk|Chase|Pounce, self, target=prey)` reads; the resolver picks the highest-affordance approach for the current `(self, prey)` state.
  - Belief facet read: `MentalModel<Prey>(target).perceived_intent_clarity` — a wary prey is harder to stalk; an oblivious one is easier.
- **Verification scenarios** (under `src/scenarios/`) per consumer.
- **256 R3+R4+R5 must land first** — Patrol's substrate (perimeter anchor, route-cost overlay, wildlife deterrent) is in flight in 256; this ticket adds Belief/Affordance axes ON TOP of that work.

## Out of scope

- Belief substrate itself (258 owns).
- ActionAffordances substrate itself (261 owns).
- Other DSE consumers (social, wildlife, Freeze, Fawn, EngageThreat, prey-side, conflict-low — all sibling tickets).
- Splitting EngageThreat from Patrol (separate ticket — the 256 R6 follow-on; this ticket consumes Affordance(Fight) in Patrol's existing combat resolver if 256 R6 hasn't landed yet, or in EngageThreat once it has).
- Hunt resolver chain refactor for sub-action selection — the Hunt resolver's sub-step structure may need a small refactor to support per-sub-action affordance gating; that refactor lives here, but the *external* GOAP step contract for Hunt should not change.

## Current state

- Blocked-by 256 (Patrol R3+R4+R5 substrate must land first; this ticket adds Belief/Affordance axes ON TOP), 258 (Belief substrate), 261 (ActionAffordances substrate).
- 256 LANDED (per jj log 2026-05-10) — initial open referenced 256 as a blocker; cleaned up post-open. The R3+R4+R5 substrate is in tree.
- Adjacent flee-substrate work to coordinate with: ticket 230 (`Carve DispositionKind::Fleeing + substrate-aware flee picker`, ready) — 230 carves the disposition; this ticket's Flee axes layer on top. Ticket 245 (`Ambient predator/prey behavior-observation enrichment`, blocked-on 243) — 245 supplies wildlife body-cue reads that this ticket's Flee/Hunt axes consume via the Belief integrator (258).
- Existing axes that need integration / supersession:
  - Flee's `flee_threat_distance` (Power-Invert curve, src/ai/dses/flee.rs:89–95) — Affordance(Flee) supersedes.
  - Flee's `health_deficit` (ticket 087 bonus lift, lines 112–115) — orthogonal to belief, retains.
  - Patrol's `safety_deficit`, `safety_upper_bound`, `patrol_perimeter_distance`, `patrol_route_cost` (post-256 5-axis composition) — Belief/Affordance axes layer on top.
  - Hunt's resolver-internal phase logic — this ticket decomposes via sub-action affordances.

## Approach

1. Read substrates 258 + 261 once they land; confirm read-API shapes match what the DSE consideration layer expects.
2. Land Flee consumer first (smallest surface; ticket 087's existing bonus-axis pattern as template).
3. Land Patrol consumer second (largest surface; coordinate with whatever 256 leaves in place).
4. Land Hunt consumer + sub-action decomposition third (most invasive — requires resolver-chain change).
5. Verify each consumer with focal-cat trace + frame-diff against pre-substrate baseline.

## Verification

### Per-consumer scenario microexperiments (≤ 3s, under `src/scenarios/`)

- `flee_belief_high_violence_capability` — cat encounters predator with high `perceived_violence_capability`; verify Flee score elevates beyond what raw distance alone would produce.
- `patrol_avoids_high_threat_sector` — patrol perimeter has two sectors; one with elevated `recency_of_threat_cue` from prior ambush; verify Patrol routes through the safer sector.
- `hunt_picks_stalk_for_oblivious_prey` — prey with `perceived_intent_clarity = 0` (unaware); verify Hunt sub-action selection picks Stalk over Chase.
- `hunt_picks_chase_for_alerted_prey` — same prey at same distance with `perceived_intent_clarity = 1` (alert); verify Hunt sub-action picks Chase over Stalk.

### Soak gates

`just soak-trace 42 <focal>` + `just verdict <run-dir>` confirms:
- Hard survival gates: `Starvation == 0`, `ShadowFoxAmbush ≤ 10` (this ticket should *improve* the ShadowFoxAmbush canary toward zero per the L3 patrol-absorption-cascade theory).
- All five continuity canaries hold.
- Per `just q anomalies`, no DSE absorbs >40% of elections (256-cascade signature) — this ticket's payoff condition.

### Frame-diff

`just frame-diff <pre-258-baseline> <post-this-ticket>` confirms:
- Flee/Patrol/Hunt drift is concordant with the substrate-addition hypothesis (these DSEs gain new considerations; their scoring distributions shift but stay within the eco-balance band).
- No wrong-direction drift on any other DSE during this ticket's wiring.

## Log

- 2026-05-10: opened sibling-to-258. Smallest-blast-radius first consumer of the new Belief + Affordance substrates. Session plan: `~/.claude/plans/after-working-256-i-dreamy-fiddle.md`.
