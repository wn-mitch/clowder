---
id: 258
title: C3 worked design — subjective belief substrate (mental models + facets + evidence typology)
status: done
cluster: C
added: 2026-05-10
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, collective-memory.md]
related-balance: []
landed-at: c3bce3500e6e
landed-on: 2026-05-11
---

## Why

The L3 patrol-absorption cascade (memory `project_l3_patrol_absorption_cascade`) showed that substrate axes elevating Patrol couldn't price *predator-exposure-likelihood-of-success* — Patrol won L3 bandwidth, exposed cats to ShadowFoxes, starvation followed 24k ticks later. Generalized: cats currently know "fox nearby" via `HasThreatNearby` + `NearestThreat` anchor, but they don't know "this fox's apparent disposition toward me" or "how reliable is my model of this individual." Real perception has two portions in tandem — raw sensory information AND the perceiver's read on the target's behavior, updated as evidence and decaying with staleness.

This ticket is the spinout of 007 cluster-C **C3 — Subjective knowledge / belief distortion**. The architectural framing is taken directly from C3's worked design (Ryan / Summerville / Mateas / Wardrip-Fruin, *Game AI Pro 3* ch. 37, 2017 — the canonical Talk of the Town treatment): per-cat **mental models** (one per known other cat / location / non-cat entity / environmental context), each a list of **belief facets** with type + value + evidence + strength + accuracy, updated via an **evidence typology** (Observation / Transference / Confabulation / Implant / Declaration / Mutation / Forgetting). Mirrors how 126 spun out C1 (BDI Intention substrate) — body retains the full design here while 007's C3 entry becomes a roadmap pointer.

Cluster role: C3 is the substrate that consumers (256-cluster DSEs, social DSEs, wildlife DSEs, Freeze, Fawn, EngageThreat, prey-side AI, conflict-low DSEs) score against. ActionAffordances (sibling ticket 261, novel beyond C3) consumes belief facets to compute per-action success scalars. C3's typed-failure-proxy consolidation list (RecentDispositionFailures, RecentTargetFailures, HuntingPriors, RecentAmbushMap — named in 007's 2026-05-09 Log entry) retires under this substrate.

## Scope

- **Mental model substrate** per cat per known entity. Four model families: `MentalModel<Cat>`, `MentalModel<Location>`, `MentalModel<Predator>`, `MentalModel<EnvironmentalContext>`. Wildlife symmetric (foxes, hawks, snakes, shadowfoxes carry their own mental models of cats and other creatures).

- **v1 facet list** (extends C3's facet types):

| Facet | C3 base | Timescale | Source | Range |
|---|---|---|---|---|
| `perceived_injury_level` | `status` (extended) | fast | body cues (242) | 0..1 |
| `perceived_intent_clarity` | derivable from strength + candidate tracking | fast | recency + variance of observed actions on me | 0..1 |
| `recency_of_threat_cue` | `last_seen_tick` of threat-flavored evidence | fast | last witnessed Attack/Hiss/Chase tick | 0..1 |
| `perceived_violence_capability` | `reputation` (extended) | slow | species prior (Implant) + per-pair witnessed-attack (Observation/Declaration) | 0..1 |
| `affiliation_history` | `bond` + `reputation` | slow | per-pair witnessed Groom + Mate + Care | -1..1 |
| `predictability` | function of evidence count + consistency | slow | per-pair observation count + variance | 0..1 |

  Per-facet tunables in `SimConstants`: `learning_rate`, `decay_rate_to_prior`. Plus `species_violence_priors` 5×5 table (Cat × Fox × Hawk × Snake × ShadowFox).

- **`MentalModel<Location>` and `MentalModel<Predator>` facets per C3's worked design** (§"For location mental models" / §"For predator mental models" in 007).

- **Evidence typology** (Clowder-subset of ToT's eleven, per 007):
  - `Observation` — direct witness; primary update path
  - `Observation` sub-variant **conspecific-as-sensor** (decision 16): `subject ≠ source` (relay's reaction is evidence about a *different* subject); credibility weight = `MentalModel<Relay>.perceived_perception_acuity × observed_stoicism × f(relay_state_at_cue)`. Affiliation and size/violence-capability are explicitly orthogonal to credibility.
  - `Transference` — feature-similarity copying (one fox reminds cat of another → fear copies)
  - `Confabulation` — probabilistic invention weighted by colony distribution
  - `Implant` — species priors at world-gen / E1 boundary
  - `Declaration` — acting on belief reinforces it (panic self-reinforces)
  - `Mutation` — probabilistic drift per tick; modulated by `memory` personality attribute
  - `Forgetting` — strength→0 terminates belief; passive decay toward prior

- **Update math: Bayes-flavored EMA, not strict Bayes.** Per-facet: `value ← value + lr × (observed − value) × confidence × recency_decay`, plus passive decay back to species prior. Maps onto C3 evidence types — EMA reinforcement = `Observation` + `Declaration`; passive decay = `Forgetting`; species prior init = `Implant`. Strict Bayes rejected (see Out of scope).

- **`WitnessableEvent` message** (`#[derive(Message)]` per CLAUDE.md ECS rules). Variants: `WitnessedAttack`, `WitnessedGroom`, `WitnessedMate`, `WitnessedCare`, `WitnessedFleeFrom`, `WitnessedHunt`, `WitnessedConspecificStartle`, `AmbientShock`, etc. Each action resolver in `src/steps/` emits the appropriate variant when producing an observable side-effect.

- **`belief_integrator` system** consumes `WitnessableEvent` messages, updates `(witness, actor)` facets per evidence typology. Plus per-tick passive-decay system that drifts axes toward species prior.

- **Evidence metadata per facet update**: `source` (which cat/system told me, if any), `location`, `tick`, `strength`. Enables citable narrative ("Whisker told me at the old den three days ago").

- **Candidate-belief tracking with revision rules** per 007 (first evidence adopts; contradicting evidence weaker than current → tracked as candidate; reinforcing strengthens until swap).

- **Salience computation** weighted by character salience (kin > bonded > coordinator > stranger), attribute salience (`last_threat` ≫ `coat_color`), existing belief strength.

- **Diagnostic lines** in `events.jsonl`: `belief_divergence_duration`, `belief_candidates_per_cat` (per 007 exit criteria).

- **Typed-failure-proxy retirement** (per 007 Log 2026-05-09): fold `RecentDispositionFailures`, `RecentTargetFailures`, `HuntingPriors::record_failed_search`, `RecentAmbushMap` into the unified mental-model substrate. Each retired component's reader rewires to read the relevant facet.

- **Update 007's C3 section** to point at this ticket as the worked-design spinout. Mirror of how C1 → 126.

## Out of scope

Adjacent work that belongs in sibling tickets, NOT here:

- **ActionAffordances substrate** (per-action success scalars) — sibling ticket 261. Consumes facets from this substrate; novel beyond C3.
- **DSE consumer wiring** — sibling tickets for 256-cluster, social, wildlife, Freeze, Fawn, EngageThreat, prey-side, conflict-low.
- **Versu social practices (C2)** — gossip transmission as a multi-stage practice. C3 receives the gossip via `Observation` evidence with `source = other_cat`; the practice itself is C2 territory (currently unspun-out from 007).
- **HTN strategist (C4)** — sits above belief modeling per 007.
- **LLM cat-conversation rendering** — ticket 011, blocked-on this ticket per `## Touch points`.

### Considered and rejected (rationale durably recorded here)

- **Strict Bayesian posteriors with hypothesis space** — clashes with orthogonal-scalar substrate vocabulary (and C3's facet typology), hard to read in trace logs, risks determinism with sampling. EMA captures the spirit (priors, evidence updates, decay) without the cost. Maps onto C3 evidence types directly.
- **Categorical 4Fs (fight/flee/freeze/fawn) as the Belief encoding** — collapses input (perception axes) with output (action choice); violates `feedback_single_axis_perception_scalars`. Orthogonal scalar facets are the encoding; 4Fs are one possible *consequence* a DSE computes from them.
- **Standalone "Talk of the Town" gossip ticket** — gossip = `Observation` evidence type with `source = other_cat`, transmission = C2 Versu practice. Already covered jointly by this ticket + future C2 spinout.
- **Standalone "Categorical Belief axes via Dirichlet-Multinomial" ticket** — C3's facet model supports `type + value` per facet, so categorical and scalar facets coexist in this substrate without needing a separate axis class.

## Current state

- 007's C3 section (lines 80–266) carries the original worked design — read it before this ticket.
- A1 + A3 dependencies satisfied per 007's 2026-04-27 Log entry.
- 126 (C1 BDI Intention) landed 2026-05-08 — precedent for perceivable per-cat ECS Component.
- Cue-source upstream tickets in flight: **242** (body-cue observable markers — limping, ear-flattening, hunched posture), **243** (target-side body-cue reads), **244** (audible cues — alarm calls, distress cries, hissing). These are the evidence sources that feed `Observation` updates. v1 of this ticket can land scaffolding without 242/243/244 wired (using `WitnessableEvent` from action resolvers as the primary evidence source) and integrate body/audible cues as those tickets land.
- Sibling tickets in cluster C (opened in same lifecycle as this one):
  - **261** ActionAffordances substrate (per-action success scalars + ActionKind enum + heuristic estimators)
  - **263** 256-cluster DSE consumers wire belief + affordance (Flee, Patrol, Hunt with Stalk/Chase/Pounce)
  - **264** Social DSE consumers (Socialize, GroomOther, Mate, Mentor, Care, FeedKitten)
  - **265** Wildlife symmetric DSE consumers (fox, hawk, snake, shadowfox)
  - **266** Prey-side AI (Bolt, ScatterGroup) — first prey AI in the codebase
  - **267** Conflict-low DSEs (Threaten, Posture, Hiss) — escalation rungs
  - **268** Hide DSE Belief+Affordance consumer wiring (reframed from "Freeze DSE (new)" — Hide DSE 104 already landed)
  - **269** Submit DSE Belief+Affordance consumer wiring + cross-species audit (reframed from "Fawn DSE (new)" — Submit DSE 145 in flight)
  - **270** EngageThreat split from Patrol (256 R6 follow-on) with Belief+Affordance reads
- Adjacent independent tickets opened in same lifecycle (NOT in cluster C):
  - **259** L1→L3 activation visualization in log viewer (devex)
  - **260** Fox scent-marking signposts (cross-species sensing)
  - **262** Audible-cue range falloff modeling (blocked-on 244)
- See 007's cluster table and the session plan `~/.claude/plans/after-working-256-i-dreamy-fiddle.md`.

## Approach

### Grounding example (ethological)

Observed (user, 2026-05-10): A car door slammed outside. Cal was sleeping (Simba was prowling nearby). Cal slightly startled awake. Simba sprinted behind the couch. Since Cal didn't jump down, Simba returned to prowling within seconds.

Useful context (NOT load-bearing): Cal and Simba get along well — affiliation is orthogonal to threat-signal credibility. Cal is a scaredy-cat by personality (low `stoicism`) — Cal's startle is a *low-credibility* relay signal, which is why Simba recovered fast. Cal is bigger than Simba but **size is incidental**; relay credibility is personality + state, not capability.

Trace through the proposed substrate:

| Tick | Event | Substrate effect |
|---|---|---|
| 0 | Car door slam | `WitnessableEvent::AmbientShock { intensity, source: External }` broadcast within audible range |
| 0 | Simba directly perceives the slam | Simba's `MentalModel<EnvironmentalContext>.recency_of_threat_cue` → high via direct Observation. **Primary driver.** |
| 0 | Cal's startle response (woken from sleep) | `BodyCueStartled { intensity, ttl }` marker authored on Cal by 242 system. Cal's `relay_state_at_cue = Sleeping` |
| 0–1 | Simba's sensing | 243 system reads `BodyCueStartled` on Cal; emits `WitnessedConspecificStartle { observed: Cal, context: concurrent_with_AmbientShock }` |
| 1 | Simba's belief integrator | Cal's startle treated as **secondary confirming evidence** for the EnvironmentalContext threat (conspecific-as-sensor, decision 16). Credibility weight is moderate-low because Cal's empirical stoicism is low and Cal was sleeping. Net effect: small additive lift on already-elevated `recency_of_threat_cue`. |
| 1–2 | Simba's DSE re-scoring | `Affordance(Freeze, Simba, EnvironmentalContext)` high (couch is nearby cover); Freeze DSE elevates → Simba sprints to cover |
| 2–30 | Cal returns to calm | No further `BodyCueStartled` (negative evidence — non-event informative) |
| 30+ | Passive decay | Simba's fast-timescale `recency_of_threat_cue` decays; absence of further cues is the dominant signal |
| 60+ | DSE re-election | Freeze drops below threshold, Patrol/Explore reclaims L3 → Simba resumes prowling |

This trace surfaced four design dimensions worth naming explicitly:

1. **Conspecific-as-sensor evidence subtype** with relay-credibility scaling = personality + state (NOT capability, NOT affiliation). See Scope's evidence typology.
2. **Negative evidence / non-events are informative** (Cal *not* escalating is the load-bearing signal). EMA passive decay handles this implicitly; design must not paper over with "must have positive counter-evidence to revise."
3. **Fast-timescale recovery is mechanically required** — `recency_of_threat_cue` decay must be tuned so brief startles don't lock cats into Freeze for game-minutes.
4. **`MentalModel<EnvironmentalContext>` joins the family** as a fourth model alongside Cat / Location / Predator (or folds into Location keyed on "here-now").

### Implementation order

1. New `src/messages/witnessable_event.rs` with v1 variants. Register in `SimulationPlugin::build()`.
2. Extend `src/components/mental.rs` with `MentalModel<T>` collection per cat. Fold existing `Memory` + `MemoryEntry` into `MentalModel<EnvironmentalContext>` or `MentalModel<Location>` (design choice during implementation).
3. Extend `src/resources/sim_constants.rs` with `BeliefAxisTunables` (per-facet `learning_rate`, `decay_rate_to_prior`) + `SpeciesViolencePriors` 5×5 table.
4. New `src/systems/belief_integrator.rs` — consumes `WitnessableEvent` (initially); updates facets via EMA per evidence typology; per-tick passive decay subsystem.
5. Wire body-cue and audible-cue reads (242/243/244) into the integrator as those tickets land.
6. Retire typed-failure proxies (RecentDispositionFailures, RecentTargetFailures, HuntingPriors, RecentAmbushMap) one at a time, repointing readers at the relevant facet.
7. Update `src/systems/colony_knowledge.rs` — promotion becomes "high-agreement-across-mental-models" rather than carrier count; ColonyKnowledge derived rather than primary.
8. Edit 007's C3 section to point here as the worked-design spinout.

### Reference / precedent files

- `src/components/relationships.rs:1–100` — per-pair symmetric resource keying precedent (this ticket extends asymmetric perceiver→target keying).
- `src/systems/sensing.rs` — perception system architecture precedent (chain of marker-author batches).
- `src/ai/dses/socialize_target.rs` — per-target DSE consideration shape (consumers will follow this pattern).
- `src/components/pairing.rs` (`PairingActivity`) — per-cat Component carrying a committed partner Entity (precedent for entity-keyed per-cat state).
- 126 (`landed/126-bdi-intention-substrate.md`) — C1 spinout precedent.
- `docs/systems/collective-memory.md` — pre-C3 simple promotion-threshold model (this ticket supersedes its design semantics; tunables remain for back-compat readers).

## Verification

### Per-ticket scenario microexperiments (≤ 3s each, under `src/scenarios/`)

Per CLAUDE.md "scenario microexperiment before a soak":

**007 C3 exit criteria (canonical):**
1. **Deliberate false belief** — plant a ground-truth-inconsistent observation in one cat, propagate via gossip, observe false belief spreading with measurable divergence duration.
2. **Candidate revision** — cat holds a stale belief, expose to weak counter-evidence twice, verify candidate tracking flips.
3. **Transference** — introduce a second fox sharing features with a known fox, verify cat transfers fear.

**Session-worked v1 scenarios:**

- `belief_witnessed_attack` — cat A attacks cat C with cat B in sensing range; verify `B.belief(A).perceived_violence_capability` lifts via EMA, `recency_of_threat_cue` maxes.
- `belief_decay_over_time` — preload elevated belief; fast-forward 10000 ticks no observations; verify decay toward species prior at expected rate.
- `belief_repeated_grooming` — cat A grooms cat B repeatedly; verify `B.belief(A).affiliation_history` increases monotonically with EMA learning curve.
- `species_prior_initialization` — fresh cat encounters fresh ShadowFox; verify `perceived_violence_capability` initializes from `species_violence_priors[Cat][ShadowFox]`, not zero.
- `belief_ambient_shock_with_relay_confirmation` (the door-slam grounding example): two cats with positive affiliation. Cal sleeping, Simba alert. Tick 0: emit `WitnessableEvent::AmbientShock` (heard by both) + author `BodyCueStartled` on Cal with `relay_state_at_cue = Sleeping`. Verify by tick 2: Simba's `MentalModel<EnvironmentalContext>.recency_of_threat_cue` lifts; tick 60+: recency decays back. **Personality assertion**: re-run with Cal as scaredy-cat (low stoicism) vs bold (high stoicism); bold-Cal variant produces *larger* secondary lift. **Affiliation orthogonality assertion**: re-run with Cal-Simba bonded vs strangers; identical Belief and DSE behavior. **Capability orthogonality assertion**: re-run with Simba bigger than Cal vs Cal bigger than Simba; identical Belief and DSE behavior.

### Headless soak gates

After scaffolding lands, `just soak-trace 42 <focal>` + `just verdict <run-dir>` confirms:

- All hard survival gates: `Starvation == 0`, `ShadowFoxAmbush ≤ 10`.
- All five continuity canaries hold (grooming, play, mentoring, courtship, mythic-texture).
- Action distribution: per `just q anomalies`, no DSE absorbs >40% of elections (256-cascade signature).
- Generational continuity: `KittenMatured ≥ 1 / sim year`.
- New diagnostic lines: `belief_divergence_duration`, `belief_candidates_per_cat` per `events.jsonl`.

### Trace inspection

`just q trace <run-dir> <cat> <tick>` confirms new MentalModel facets appear in L2 trace with named source contributions ("perceived_violence_capability lifted from X→Y by WitnessedAttack at tick Z, source = cat_<id>"). Per `feedback_use_skill_surface`, all log queries via skill surface.

### Cross-run drift detection

After scaffolding lands without consumers, sweep should produce **null behavioral drift** (substrate present but unconsumed → no per-DSE score change). Each consumer ticket then earns its own four-artifact methodology drift check per CLAUDE.md.

## Log

- 2026-05-10: opened from 007 cluster-C C3 expansion. Mirrors how 126 spun out C1. Session plan: `~/.claude/plans/after-working-256-i-dreamy-fiddle.md`. Sibling cluster tickets opened in same lifecycle: 261 (ActionAffordances), 263–267 (consumers + new DSEs: 256-cluster, social, wildlife, prey-side, conflict-low), 268–270 (Hide consumer, Submit consumer, EngageThreat split). Adjacent independent tickets opened in same batch: 259 (L1→L3 viz), 260 (fox scent-marking signposts), 262 (audible-cue range falloff). Reframes during opening: original "Freeze DSE (new)" became 268 (consumer-wiring on existing 104 Hide DSE); original "Fawn DSE (new)" became 269 (consumer-wiring on in-flight 145 Submit DSE) — pillar-2 substrate-over-hacks doctrine.
- 2026-05-11: Landed 2026-05-11. Scaffolding: src/components/beliefs.rs (4 newtype Components + MentalModel + Facet + EvidenceKind + LocationKey/EnvironmentalContextKey); src/messages/witnessable_event.rs (8 v1 variants); src/systems/belief_integrator.rs (Pass A Observation EMA + Pass B Implant + Forgetting decay, staggered); SimConstants::beliefs (per-facet lr+decay + 5x5 SpeciesViolencePriors). Wiring: WitnessableEvent::Groom from goap.rs grooming-completion site; WitnessableEvent::SelfPlanFailed dual-emit from goap.rs make_plan→None site (RDF write retained for IAUS cooldown). Verification: two seed-42 deep-soaks (scaffolding-only, dual-emit) both report verdict:pass with 0.0% drift across every footer field. 17 belief unit tests pass; just check clean. Follow-ons opened: 290 (RDF reader cutover, four-artifact balance change), 291 (ColonyKnowledge restructure), 292-294 (RecentTargetFailures / HuntingPriors / RecentAmbushMap retirements), 295 (Attack/Mate/Care/FleeFrom/Hunt emit sites). 007 cluster-C C3 pointer was already updated at ticket-open time.
