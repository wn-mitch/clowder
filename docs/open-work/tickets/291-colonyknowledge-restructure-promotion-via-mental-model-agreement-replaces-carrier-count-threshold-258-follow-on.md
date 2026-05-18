---
id: 291
title: ColonyKnowledge restructure — promotion-via-mental-model-agreement replaces carrier-count threshold (258 follow-on)
status: ready
cluster: ai-substrate
orchestration: substrate-sensitive
added: 2026-05-11
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

`docs/systems/collective-memory.md` models the colony as democratic consensus: a memory held by ≥3 carriers at a bucketed location promotes to `ColonyKnowledge`; below threshold it's per-cat. The model is elegant but structurally precludes a load-bearing C3 narrative: *the colony wrongly believes X because one cat saw something misleading and panic propagated faster than ground truth corrected*. Now that 258 landed per-cat `MentalModel`s with `evidence_count` and `strength` per facet, "colony knowledge" can be derived as "high agreement across mental models" — a strict superset of the carrier-count threshold that admits divergence, gossip propagation, and false-belief epidemics as first-class phenomena. The ticket plan-agent for 258 flagged this restructure as "genuinely architecturally different from carrier-count promotion" and explicitly worth its own ticket so the four-artifact methodology can run against one behavior change at a time.

## Scope

- Replace `src/systems/colony_knowledge.rs::update_colony_knowledge` (lines 22–340) carrier-count promotion logic with mental-model agreement: aggregate facets across all cats' `CatBeliefs` + `LocationBeliefs` + `ContextBeliefs` per `(subject, facet_slot)`; promote when N≥threshold cats hold a facet within `agreement_epsilon` of each other AND strength ≥ `promotion_strength`.
- Add `ColonyKnowledgeConstants::agreement_epsilon`, `promotion_strength`, and `agreement_quorum` (default 3, matching legacy `promotion_threshold`) to `src/resources/sim_constants.rs`.
- Retain `ColonyKnowledge` Resource as the public type — narrative readers and `knowledge_description()` (`src/resources/colony_knowledge.rs:71`) keep working. Internal shape may add a `derivation_source: DerivedFrom { facet_slot, witnesses: Vec<Entity> }` field for citable narrative ("Whisker, Cedar, and Mallow all agree the den at (10,15) is dangerous").
- Drop or gracefully sunset the legacy `transmission_probability` constants — they're scaffolding-only in current doctrine (per `docs/systems/collective-memory.md:25`). Gossip transmission is C2 (Versu social practices) territory and a separate spinout.
- Update `knowledge_forgotten` narrative emit and `Feature::KnowledgePromoted` / `Feature::KnowledgeForgotten` activation hooks to fire on the new derivation path.

## Out of scope

- C2 Versu social practices for gossip transmission (separate cluster-C spinout from 007).
- LLM cat-conversation rendering of belief divergence (011).
- Diagnostic visualization of belief-divergence-duration (debatable — could ship here or land in a viz follow-on; default: include in `events.jsonl` footer field per 258's exit-criteria list).

## Current state

258 landed 2026-05-11 (commit `c3bce3500e6e`). The substrate side is in place:

- Every cat carries `CatBeliefs`, `LocationBeliefs`, `PredatorBeliefs`, `ContextBeliefs` with 6 facets per `MentalModel`, plus `strength` and `evidence_count` per facet.
- `belief_integrator` updates facets from `WitnessableEvent` observations and decays toward priors.
- `MemoryEntry` / `Memory` (legacy per-cat buffer) still exists alongside; 258 deliberately did NOT fold them in.

`ColonyKnowledge` still runs its pre-C3 promotion pathway untouched. No dual-write — clean cutover when this ticket lands.

## Approach

Three commits:

1. **Derivation function** — add `derive_colony_knowledge(world: &World, c: &ColonyKnowledgeConstants) -> Vec<KnowledgeEntry>` next to the existing system. Read-only over all cats' Belief Components. Doesn't modify state yet.
2. **Cutover** — replace `update_colony_knowledge` body with: `*colony_knowledge = derive_colony_knowledge(...)`, preserving the public `ColonyKnowledge` API (`has_entry`, `find_entry`, `knowledge_description`). Drop `Memory`-scanning logic. Drop carrier-count loop. Keep narrative-emit + Feature-activation paths on the new derivation.
3. **Hypothesize + tune** — `agreement_epsilon` and `promotion_strength` are new tunables. Run `just hypothesize docs/balance/291-colony-knowledge-derivation.yaml` against the legacy carrier-count behavior.

Hypothesis: with `agreement_quorum=3`, `agreement_epsilon=0.2`, `promotion_strength=0.3`, the derived `ColonyKnowledge` set matches the legacy carrier-count set within ±20% Jaccard similarity per soak. Stretch goal: false-belief test scenario — plant a divergent observation in one cat, verify it does NOT promote (strength × quorum gate); plant in three cats, verify it DOES promote and shows up in narrative with `derivation_source.witnesses` listing the three.

## Verification

- `just check` clean.
- `just soak 42` + `just verdict logs/tuned-42/` — continuity canaries hold (especially `mythic-texture` — colony knowledge feeds narrative).
- `just hypothesize docs/balance/291-colony-knowledge-derivation.yaml` — concordance pass on Jaccard-similarity vs legacy.
- New scenario `src/scenarios/colony_knowledge_false_belief.rs` — preload three cats with divergent beliefs; assert the wrong belief promotes (substrate alone doesn't gate truth) AND that the trace shows the witness chain.
- `events.jsonl` carries a `belief_divergence_duration` footer field per 258's exit criteria.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed **308** (done, belief-perception, score 0.88 (cross-cluster)) — Colony reserves belief — mental-model facet tracking thornbriar / remedy-herb s…
- ✓ landed **209** (done, balance, score 0.87 (cross-cluster)) — Positive colony_food_security axis on higher-tier DSEs
- · **  1** (in-progress, ai-substrate, score 0.87) — Explore dominance over targeted leisure

<!-- linkages:end -->
## Log

- 2026-05-11: opened as 258 follow-on. The pre-258 carrier-count promotion stays load-bearing until this lands; the per-cat substrate sits adjacent. Clean cutover (no dual-write window) per 258's plan-agent recommendation.
