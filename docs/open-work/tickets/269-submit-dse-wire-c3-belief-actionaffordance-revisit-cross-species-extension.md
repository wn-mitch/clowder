---
id: 269
title: Submit DSE — wire C3 Belief + ActionAffordance + revisit cross-species extension
status: blocked
cluster: C
added: 2026-05-10
parked: null
blocked-by: [145, 261]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

The session-plan eureka identified Fawn (appeasement-grooming) as one of the four threat-response shapes (alongside Fight / Flee / Freeze) that should fire from the C3 belief substrate. Audit during ticket-opening surfaced 145 (Submit gesture DSE — ready) + 144 (IntraspeciesConflictResponseFawn Modifier — blocked-on 145). Per pillar-2 substrate-over-hacks doctrine: don't proliferate new DSEs when an existing one serves. This ticket is the Belief/Affordance consumer wiring on Submit DSE; complements 144 and revisits 144's "cross-species fawn (e.g. cat appeasing a fox) — ecologically incoherent" out-of-scope decision in light of the Belief substrate's ability to model honest cross-species perception.

REFRAMED: original session plan called for "Fawn DSE (new)" as ticket η. Revised to consumer-wire on existing 145 infrastructure once 145 lands.

Pre-existing parent framework: 109 (IntraspeciesConflictResponse — full four-valence, landed) names Fawn as one of the four threat-response valences, with intraspecies-only scope. This ticket honors 109's framing for v1 wiring and surfaces the cross-species question as a substrate-driven audit (see Scope below) rather than asserting a different framing.

## Scope

- **Submit DSE consideration additions** (`src/ai/dses/submit.rs`, after 145 lands):
  - `Affordance(Fawn, self, target)` axis from substrate 261.
  - `MentalModel<X>(target).perceived_hostility` axis from substrate 258.
  - `MentalModel<X>(target).recent_groom_history` axis (or `affiliation_history` as proxy if no recent_groom_history facet exists).
- **Cross-species extension audit** (independent sub-deliverable):
  - 144's claim is "predators don't accept appeasement (ecologically incoherent), which is why predator-response branches do not include Fawn." That's a design choice, not a fact. Real cats sometimes go low-body-submissive when cornered by a predator — this is buy-time defensive posture, not appeasement-of-a-bonded-other.
  - This ticket's audit: read 144's rationale, run a small scenario where a cat is cornered by a fox; confirm whether Submit (potentially with cross-species variant) produces ecologically-honest behavior or just confuses the substrate.
  - Decision outcome: either (a) confirm 144's intraspecies-only framing holds, document why; (b) extend Submit to cross-species with a separate cross-species sub-DSE or a CrossSpecies variant of Affordance(Fawn); (c) defer cross-species to a dedicated future ticket. v1 of this ticket lands the intraspecies wiring; the cross-species decision is the audit deliverable.

## Out of scope

- The Submit DSE itself (ticket 145 owns).
- The IntraspeciesConflictResponseFawn Modifier (ticket 144 owns; orthogonal Modifier on the same DSE).
- Other social DSE consumer wiring (ticket 264 owns).
- The Belief substrate (258).
- The ActionAffordances substrate (261).
- Cross-species combat semantics (separate concern from cross-species fawn).

## Current state

- Blocked-by 145 (Submit DSE infrastructure must land first), 258 (Belief substrate), 261 (ActionAffordances substrate).
- 144 (intraspecies-conflict Fawn Modifier on Submit) is `blocked` on 145. Complementary, not blocking.
- 145's verification scenarios test "subordinate cat near dominant cat" — this ticket extends with belief-driven scoring and a cross-species probe scenario.

## Approach

1. Wait for 145 to land (or co-design if 145 picks up post-258/261).
2. Add new considerations on the existing Submit DSE struct.
3. Wire the considerations to read Belief and Affordance APIs.
4. Run cross-species scenario audit; document the decision in a Log entry.
5. If decision is to extend cross-species, add the variant or sub-DSE in this ticket OR open a dedicated follow-on (depending on scope).

## Verification

### Scenario microexperiments

- `submit_fires_on_high_perceived_hostility_intraspecies` — cat A approaches cat B with elevated `perceived_hostility(A→B)`; verify B's Submit DSE elevates and resolves to belly-up gesture.
- `submit_no_fire_on_low_hostility` — cat A approaches cat B with neutral perceived_hostility; verify Submit doesn't fire.
- `cross_species_appeasement_audit` — cornered cat with fox closing in; check whether Submit fires (it should NOT under v1's intraspecies-only constraint), document the resulting behavior, decide if cross-species extension is warranted.

### Soak gates

- bonds_formed not regressed.
- Combat mortality canary stable.
- New `Feature::SubmitGestured` (per 145 scope) appears with reasonable frequency in soaks once Submit DSE actively fires.

## Log

- 2026-05-10: opened sibling-to-258. REFRAMED from original "Fawn DSE (new)" plan slot — Submit DSE infrastructure already in flight (145, ready). This ticket is the Belief/Affordance consumer wiring + cross-species extension audit. Session plan: `~/.claude/plans/after-working-256-i-dreamy-fiddle.md`.
