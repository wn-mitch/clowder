---
id: 007
title: Deliberation-layer (Cluster C)
status: blocked
cluster: C
added: 2026-04-20
parked: null
blocked-by: [005]
supersedes: []
related-systems: [ai-substrate-refactor.md, refactor-plan.md, scoring-layer-second-order.md, strategist-coordinator.md]
related-balance: []
landed-at: null
landed-on: null
---

**Why this is a cluster:** C1–C4 all sit *above* the per-tick scoring
layer and add forms of persistence, commitment, and structure that
scoring alone cannot produce. Each addresses a specific gap named in
`docs/balance/scoring-layer-second-order.md`. All are gated on A1
because they add new scoring axes or consume slow-state the current
additive layer can't cleanly express.

### C1. BDI-style intention persistence

**Why it matters:** `docs/balance/scoring-layer-second-order.md` framing
#1 is a direct restatement of the BDI (Belief-Desire-Intention) thesis
(Rao & Georgeff 1991): *intentions are commitments that persist across
deliberation cycles*. Clowder's GOAP has *partial* intention persistence
(once a plan is picked, steps execute sequentially), but at the scoring
layer there is no commitment — each tick re-scores from scratch.
Result: flipper behavior near equal scores, and social/skill
accumulation supply chains that can't survive the per-tick churn.

**Current state:** Scoring is stateless per tick. GOAP adds per-plan
continuity but not per-goal continuity (a goal is re-deliberated every
time a plan completes or fails).

**Proposed approach:** add a lightweight intention layer between
scoring and GOAP — a per-cat `CurrentIntention` component carrying
a goal + commitment strength + expiry, scored with a momentum bonus
during deliberation. New plans inherit the intention if valid; new
deliberations only override if the alternative's margin exceeds the
commitment strength.

**Touch points:**
- New: `src/components/intention.rs` (or similar)
- `src/systems/goap.rs` — read intentions when picking plans
- `src/ai/scoring.rs` — momentum bonus as an axis (needs A1 for clean
  addition)

**Preparation reading:**
- Rao & Georgeff (1991), "Modeling Rational Agents within a
  BDI-Architecture" — KR 1991; canonical paper; Google Scholar PDF
  widely mirrored. Short, formal, readable.
- Michael Wooldridge, *An Introduction to MultiAgent Systems* (2nd
  ed., Wiley 2009) ch. 4 "Practical Reasoning Agents" — textbook BDI
  treatment with examples
- Jeff Orkin, "Three States and a Plan: The AI of F.E.A.R." (GDC
  2006) — free at <https://alumni.media.mit.edu/~jorkin/goap.html>;
  commitment and plan persistence in practical game AI
- `docs/balance/scoring-layer-second-order.md` (in repo) — framing #1
  is the BDI thesis in Clowder terms; re-read before starting

**Exit criterion:** seed-42 deep-soak shows reduced plan-preemption
rate without increased starvation or other canary degradation.

**Dependency:** gated on A1 (momentum is a scoring axis); pairs well
with A4 (target commitment and intention commitment interact).

---

### C2. Versu-style social practices

**Why it matters:** Evans & Short's Versu system models social
interactions as *practices* — multi-agent coordinated behaviors with
shared state (courtship, gossip, quarrel, greeting). A practice has
roles, stages, and invariants; agents *enter into* a practice
together, then its stages drive their actions until it completes.
Clowder's current social model is one-sided: one cat scores Socialize,
picks a target, emits an interaction, partner reacts. Practices would
let courtship, mentoring, and play be durable multi-stage structures
rather than per-tick score winners.

**Current state:** Social interactions are per-tick single-cat
decisions; partner cats react but don't co-commit. Bond/relationship
state accumulates but practice-level structure doesn't exist.

**Touch points:**
- `src/systems/social.rs` — currently one-sided interactions
- `src/systems/pregnancy.rs` (courtship should become a practice)
- New: `src/systems/practices.rs` or similar
- Relationship state in `src/components/social.rs`

**Preparation reading:**
- Richard Evans & Emily Short, "Versu — A Simulationist Storytelling
  System" (IEEE TCIAIG, 2014) — canonical Versu paper; defines
  practices, roles, stages, invariants
- Richard Evans, "The Sims 3" / "Imagination Engines" (GDC 2011,
  with Emily Short, GDC Vault) — shorter on-ramp; predecessor ideas
  at production scale
- Emily Short's two-part review of Ryan's dissertation
  (<https://emshort.blog/2019/05/21/curating-simulated-storyworlds-james-ryan/>)
  — Short was Evans' co-author; connects ToT practices to Versu
- James Ryan, *Curating Simulated Storyworlds* (UCSC 2018) ch. 5 on
  ToT — gossip and relationship practices; eScholarship PDF at
  <https://escholarship.org/uc/item/1340j5h2>
- James Ryan et al., Talk of the Town FDG 2015 paper (via
  <https://www.jamesryan.world/publications>) — dense, practical

**Exit criterion:** at least one practice (courtship is the natural
target — addresses the Mate supply-chain problem) implemented as a
two-agent multi-stage structure; partners co-commit rather than each
independently scoring Mate.

**Dependency:** gated on A1 (practices inform scoring axes); C1
(intentions are a natural substrate for practice participation);
potentially simpler post-A4.

---

### C3. Subjective knowledge / belief distortion

**Why it matters:** `src/systems/colony_knowledge.rs` models knowledge
as **democratic consensus** — memories held by ≥`promotion_threshold`
cats get promoted to `ColonyKnowledge.entries`; below that, they're
per-cat memories that decay. The model is elegant but structurally
prevents a whole class of emergent narrative: *the colony wrongly
believes X because one cat saw something misleading and panic
propagated faster than ground truth corrected*.

Ryan et al. (*Game AI Pro 3* ch. 37, 2017) give the canonical
alternative — a full architecture where per-character belief is
first-class, with explicit mechanisms for origination, propagation,
reinforcement, deterioration, and termination. The chapter is specific
enough to be a blueprint; this entry adopts its vocabulary.

**Current state:** `colony_knowledge.rs` tracks aggregated memories
only; `Memory.events` is assumed faithful to the ground-truth event;
no source attribution, no mutation, no candidate-belief tracking.

**Proposed architecture (scaled to cats, not ToT's 300–500 humans):**

*Ontological structure.* Each cat maintains an ontology of linked
mental models: one per other cat it knows of, one per notable location
(den, feeding spot, fox territory, fairy ring), one per non-cat entity
that matters (specific foxes/hawks). Models link to each other —
"Silverpaw's known den" points to a Location model. Keeps storage lean
and avoids cross-model inconsistency.

*Mental model facets.* Each model is a list of **belief facets** with
type + value + evidence + strength + accuracy flag. Clowder facet
types (vastly narrower than ToT's 24 human attributes):
- For cat mental models: **lineage** (parents/offspring, links to
  other cat models), **status** (alive/dead/banished), **role**
  (coordinator/healer/hunter), **bond** (affinity toward this cat),
  **last_seen_location**, **reputation** (recent aggressive/grooming
  events)
- For location mental models: **last_threat** (fox at this tile three
  days ago), **last_opportunity** (prey plentiful last visit),
  **affective_tag** (safe/fearful/sacred), **owner** (for dens — links
  to cat model)
- For predator mental models: **last_seen_location**, **last_seen_tick**,
  **scent_signature**, **known_victims** (links to cat models)

*Evidence typology* (Clowder-subset of ToT's eleven — skipping
Reflection (trivial) and Lie (cats don't linguistically lie)):
- **Observation** — direct witness. Origination for most beliefs.
- **Transference** — new entity reminds the cat of an old one (shared
  scent, territory, coat color) → beliefs copy. *This is how cats
  generalize fear of one fox to all foxes.*
- **Confabulation** — probabilistic invention weighted by the colony's
  distribution. "Where do foxes live?" confabulated = most-commonly-
  reported fox territory.
- **Implant** (at end of world-gen / E1 boundary) — starting cats
  know their parents' world because their parents told them. Handled
  in E1's implantation phase.
- **Declaration** (behavioral, not linguistic) — every time a cat
  *acts on* a belief (flees from a tile, approaches a preferred den),
  the belief is reinforced. Corollary: panic behavior can
  self-reinforce into conviction even if original evidence was weak.
- **Mutation** — probabilistic drift per tick scan; affected by the
  cat's `memory` personality attribute (inherited from parent) and
  the facet's salience.
- **Forgetting** — belief terminates when strength hits zero.

*Evidence metadata* — every piece carries: **source** (which cat told
me, if any), **location**, **tick**, **strength**. Source + location +
tick are exactly what's needed to surface "Whisker told me at the old
den three days ago" as a citable narrative line.

*Salience computation* — probability of observing / propagating /
forgetting weighted by:
- Character salience: kin > bonded > coordinator > stranger
- Attribute salience: `last_threat` ≫ `coat_color`; `lineage` ≫
  `last_visited`
- Existing belief strength (weak beliefs more likely to deteriorate)

*Belief revision with candidate tracking.* First evidence adopts.
Contradicting evidence weaker than current → adopt as *candidate*
belief, track separately; further reinforcing evidence strengthens the
candidate until/unless it exceeds the currently-held belief, at which
point they swap. Enables *belief oscillation* (the cat isn't sure yet)
and *slow conversion* (Whisker eventually accepts the fox moved dens,
but not on first encounter).

*Candidate narrative outputs* (free byproducts):
- "Whisker no longer believes the old den is safe" (candidate won)
- "The colony has forgotten Silverpaw" (all belief facets terminated)
- "Ember wrongly believes the fox returned last night" (ground-truth
  divergence available for diagnostic assertion)

**Touch points:**
- `src/systems/colony_knowledge.rs` — promotion becomes
  "high-agreement-across-mental-models" rather than simple carrier
  count; ColonyKnowledge may be derived rather than primary
- `src/components/mental.rs` `Memory` + `MemoryEntry` — becomes a
  collection of mental models, each a list of belief facets
- `src/systems/social.rs` — conversation-style knowledge exchange
  during co-location (salience-weighted topic selection, per-facet
  exchange probability)
- `src/systems/sensing.rs` — observation-as-evidence pipeline
- New: `src/resources/ground_truth_log.rs` (or reuse
  `logs/events.jsonl`) for accuracy assertions and divergence
  diagnostics

**Preparation reading:**
- **Ryan, Summerville, Mateas, Wardrip-Fruin (2017). "Simulating
  Character Knowledge Phenomena in Talk of the Town."** *Game AI
  Pro 3* ch. 37 (free PDF at
  <https://www.gameaipro.com/GameAIPro3/GameAIPro3_Chapter37_Simulating_Character_Knowledge_Phenomena_in_Talk_of_the_Town.pdf>)
  — the definitive treatment. Read § 37.3 front-to-back; § 37.3.5
  (evidence typology) and § 37.3.9 (belief revision) are load-bearing.
- **Ryan, Summerville, Mateas, Wardrip-Fruin (2015). "Toward
  characters who observe, tell, misremember, and lie."** Proc. 2nd
  Workshop on Experimental AI in Games, Nov 2015, UC Santa Cruz —
  earlier, denser version
- **Ryan, Mateas, Wardrip-Fruin (2016). "Characters who speak their
  minds: Dialogue generation in Talk of the Town."** AIIDE 2016 —
  how mental-model facets feed dialogue/narrative generation
- **James Ryan, *Curating Simulated Storyworlds*** (UCSC 2018) ch. 4
  — long-form treatment; covers Hennepin successor system
- **Shi Johnson-Bey, *Neighborly*** (ECS Python, archived 2026-04-07,
  <https://github.com/ShiJbey/neighborly>) — concrete ECS sketch of
  a ToT descendant; stable reference
- **Damián Isla, "Third Eye Crime: Building a stealth game around
  occupancy maps"** (AIIDE 2013) — cited in § 37.3; simpler cousin
  useful for calibration

**Exit criterion:** three demonstrable phenomena:
1. *Deliberate false belief:* plant a ground-truth-inconsistent
   observation in one cat, propagate via gossip, observe the false
   belief spreading with measurable divergence duration.
2. *Candidate revision:* construct a scenario where a cat holds a
   stale belief, expose them to weak counter-evidence twice, verify
   candidate tracking works and eventually flips.
3. *Transference:* introduce a second fox that shares features with a
   historically-known fox, verify the cat transfers fear and/or
   territory beliefs.

Add `belief_divergence_duration` and `belief_candidates_per_cat` as
diagnostic lines in `logs/events.jsonl`.

**Dependency:**
- Gated on **A1** (belief-strength becomes a scoring axis — fear
  scales with belief strength, not just raw danger).
- Gated on **A3** (mental models are entities with components — the
  context-tag refactor is a prerequisite for clean per-cat belief
  storage).
- Pairs with **C2** (gossip is a practice; co-location exchange is a
  practice).
- **Architecturally intertwined with E1** — world-gen runs without
  knowledge phenomena (too expensive), then knowledge-implantation at
  the E1 → runtime boundary seeds each cat's mental models (see
  § 37.3.10 for ToT's implantation procedure).

---

### C4. Strategist-coordinator task board

Existing entry: **this file, `#1 sub-3`** and design doc
`docs/systems/strategist-coordinator.md`. **Recontextualize under this
cluster** — it's the HTN-style hierarchical planning layer, sitting
above BDI intentions (C1), practices (C2), and belief modeling (C3).
Not duplicated here; see sub-task 3 of #1.

**Preparation reading** (for when the existing entry gets picked up):
- Dana Nau et al., "SHOP2: An HTN Planning System" (JAIR 2003) —
  canonical HTN reference; free PDF via Google Scholar
- Kallmann & Thalmann on hierarchical planning in game characters —
  shorter, more applied
- *Game AI Pro* chapters on hierarchical task networks and
  goal-oriented architectures — free at <http://www.gameaipro.com/>
- `docs/systems/strategist-coordinator.md` (in-repo) — the existing
  design stub
