---
id: 265
title: Wildlife symmetric DSE consumers wire belief + affordance (fox, hawk, snake, shadowfox)
status: done
cluster: wildlife
orchestration: substrate-sensitive
initiative: [predator-prey-dynamics]
added: 2026-05-10
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: 9eb6e7ed
landed-on: 2026-07-08
---

## Why

C3's Belief substrate (258) and ActionAffordances (261) are designed wildlife-symmetric from day one — `species_violence_priors` is a 5×5 table (Cat × Fox × Hawk × Snake × ShadowFox), and Belief Integrator runs per creature. This ticket wires the substrates into the wildlife DSE catalog (`fox_*`, `hawk_*`, `snake_*`, `shadowfox_*`) so the species-clash design pillar holds: cat decides "this fox will kill me if I don't fight" while fox decides "this cat will kill me if I don't fight," and the world resolves it. Both creatures' DSEs fire from incompatible Beliefs because both are reading the same substrate honestly.

Also wires species-specific predation actions (Dive on Hawk, Strike on Snake, Ambush on ShadowFox) into their respective DSEs as ActionAffordance consumers. Per-species action subsets means each wildlife DSE only consumes its own subset (Hawks consume Dive-affordance, never Pounce; Snakes consume Strike, never Dive).

## Scope

- **Fox DSEs** (`src/ai/dses/fox_*.rs`): each fox DSE adds Belief reads about cats (`MentalModel<Cat>(target).perceived_violence_capability`) and Affordance reads for fox actions (`Affordance(Stalk|Chase|Ambush|Flee|Fight, fox, target=cat-or-other-fox)`).
- **Hawk DSEs** (`src/ai/dses/hawk_*.rs` if present, otherwise stubs): Belief reads about prey (`MentalModel<Prey>(target).perceived_intent_clarity`) and Affordance reads for `Dive`, `Stalk` (aerial circling), `Flee`.
- **Snake DSEs** (similar): Affordance reads for `Strike`, `Stalk`, `Flee`.
- **ShadowFox DSEs** (`src/ai/dses/shadowfox_*.rs` if present): Affordance reads for `Ambush`, `Chase`, `Strike`, `Flee`. Cross-references ticket 023 (shadowfox motivations distinct from normal foxes).
- **`species_violence_priors` table population** for all 5×5 species pairs (this lives in 258's scope but populating the table for wildlife perceiver-rows is naturally a co-deliverable with this ticket).

## Out of scope

- Belief substrate (258).
- ActionAffordances substrate (261).
- Cat-side wildlife perception (handled by ticket 263 — 256-cluster DSE consumers).
- Prey-side AI (sibling ticket — first prey AI in the codebase; this ticket is for predators only).
- ShadowFox motivation distinctness (ticket 023 owns; this ticket consumes whatever 023 lands).
- Species-specific perception channel additions (e.g., snake tongue-flick chemoreception). v1 reads the same `WitnessableEvent` channel as cats; per-species perception extensions are future tickets.

## Current state

- Blocked-by 258 (Belief substrate) + 261 (ActionAffordances substrate).
- Existing wildlife DSEs (per layer-walk in session plan): `fox_patrolling.rs`, `fox_fleeing.rs`, `fox_raiding.rs`, `fox_hunting.rs`. Hawk/Snake/ShadowFox DSEs may or may not exist as dedicated files — check during implementation. ShadowFox currently triggers `ShadowFoxSpawn / ShadowFoxBanished` narrative events; whether it has dedicated DSEs vs reuses Fox DSEs is a TBD audit step.
- Species sensory profiles in `SimConstants.sensory_profiles` already keyed by `SensorySpecies` per layer-walk findings.

## Approach

1. Audit wildlife DSE catalog under `src/ai/dses/` to confirm which species have dedicated DSE files vs reuse Fox DSEs.
2. For each wildlife DSE, add Belief + Affordance considerations following the cat-side pattern from ticket 263 (consumer wiring template).
3. Populate `species_violence_priors` for wildlife perceiver-rows.
4. Verify per-species: focal-cat trace on a wildlife creature (`just soak-trace` may need extension to focal-wildlife — that's an engineering side-quest worth a separate small ticket if the trace harness doesn't already support it).

## Verification

### Per-species scenario microexperiments

- `fox_belief_high_violence_capability_cat` — fox encounters a cat with high `perceived_violence_capability` (e.g., a coordinator); verify fox Flee score elevates appropriately.
- `hawk_dive_affordance_aerial_cover` — hawk above prey with full sky vs prey under canopy; verify Dive-affordance is high when canopy absent, low when present.
- `shadowfox_ambush_concealment` — shadowfox in dense corruption vs open ground; verify Ambush-affordance scales with concealment quality.
- `wildlife_species_clash` — cat patrols toward fox, fox patrols toward cat; verify both sides' Beliefs fire and one (or both) backs off via the substrate (NOT via a director).

### Soak gates

- ShadowFoxAmbush canary holds (≤ 10).
- Wildlife combat mortality canary stable per healthy-colony.md.
- shadow_fox_spawn_total in band.

### Frame-diff

`just frame-diff <pre-substrate-baseline> <post-this-ticket>` confirms wildlife DSE drift is concordant with substrate-addition; cat-side DSE drift should be unchanged (this ticket changes wildlife scoring, not cat scoring).

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- · **269** (blocked, combat-threat, score 0.91 (cross-cluster)) — Submit DSE — wire C3 Belief + ActionAffordance + revisit cross-species extension
- ✓ landed **209** (done, balance, score 0.88 (cross-cluster)) — Positive colony_food_security axis on higher-tier DSEs
- · **314** (ready, ai-substrate, score 0.88 (cross-cluster)) — extend ActionAffordances writer to cover cat-vs-prey (Stalk/Chase/Pounce) — 263…

<!-- linkages:end -->
## Log

- 2026-05-10: opened sibling-to-258. Wires Belief + Affordance into wildlife DSE catalog symmetrically. Species clash now happens because both sides perceive the substrate honestly. Session plan: `~/.claude/plans/after-working-256-i-dreamy-fiddle.md`.
- 2026-05-19: accuracy audit pass — no blockers; wildlife DSE files (fox_*, hawk_*, snake_*, shadowfox_*) audit deferred to implementation per ticket scope; related work references verified.
- 2026-07-07: **dormant wire landed** (plan step 18; commits 453cc3f7 + 04a9d223). Audit result: fox has 9 dedicated DSEs; hawk 3 (hunting/fleeing/resting); snake 4 (ambushing/foraging/fleeing/basking); **shadowfox has no DSEs** (inline drives) — its slice moves to ticket 310 per the release plan. Commit 1: conditional prey-affordance axes at weight 0.0 on FoxHunting (`Stalk|Chase`), HawkHunting (`Dive|Chase`), SnakeAmbushing (`Strike`), SnakeForaging (`Stalk`) via new `best_affordance_over_targets` helper; wildlife-vs-prey writer rows arrive with 314 (step 19). Commit 2: `#[require(CatBeliefs)]` on WildAnimal (compile-time coverage of all spawn sites), belief_integrator wildlife witness pass (Attack: violence+threat-cue, hostility only when witness is the target; Hunt: violence), Pass-B implant from four new `SpeciesViolencePriors` perceiver rows (cat_perceived_by_fox 0.5 / hawk 0.3 / snake 0.65 / shadow_fox 0.2) + stagger decay, and dormant `perceived_cat_threat` axes on the three fleeing DSEs via the new `max_perceived_violence` sensor. Gate: null-drift **proven by byte-identity** — the 900s gate stream reproduces the pre-264 `tuned-42-d94c282f` reference bit-exactly for the full 194k-line overlap (`docs/balance/265-dormant-wire-null-drift.md`; the 264-record saturation claim is corrected there). Remaining for step 21 (activation, four-artifact): per-species weight lifts; live `Res<ActionAffordances>` borrows in the three wildlife evaluate systems (deferred schedule edge); the 505-flagged `FleeFrom→PredatorBeliefs` witness write (behavior-priced); `wildlife_species_clash` / `fox_belief_high_violence_capability_cat` / `hawk_dive_affordance_aerial_cover` scenarios. Affordance reads for wildlife Flee/Fight have no writer estimators yet — extend `write_wildlife_vs_cat` only if activation tuning needs them.
- 2026-07-08: **activations landed — ticket closed** (plan step 21; commits ea638840 fox / f72f4e32 hawk+snake / 9eb6e7ed FleeFrom→PredatorBeliefs + scenarios; record `docs/balance/265-wildlife-activation.md`). All 7 weights lifted to first-light 0.10; live `Res<ActionAffordances>` borrows in the three wildlife evaluate systems (byte-neutral read-edge class per the dormant-wire control ladder). Structural find during scenario work: the fox/hawk Fleeing outer gates (`health<0.5 || cats_nearby>=2`) silenced the belief axis for healthy lone-cat encounters — fixed with belief-eligibility clauses (`fox/hawk_flee_belief_eligibility_threshold` 0.75, inert at zeroed weight; snake's `>=1` gate needed none). The 505-flagged FleeFrom threat write landed in `PredatorBeliefs` at new `FLEE_CUE_OBSERVED_VALUE` 0.75 (wildlife-gated via component truth, third-party witnesses only — fleeing is not self-confirming; no new ballast class since Implant already models those entries). Three scenarios landed: `fox_belief_high_violence_capability_cat` (believer/skeptic twin geometry), `hawk_dive_affordance_aerial_cover` (live Ward, not setup stamp — coverage map rebuilds per tick), `wildlife_species_clash` (full observation channel, zero stamped beliefs; fox threat reads are range-gated ≤6 vs witness range 10 — distance 5 is the working band). Gate 1 mechanism signature: cat-side "lost prey during approach" −93% / "target Despawned" +55% — foxes now take prey with real stalk/chase affordance. Gate 3 tripped the never-fired canary (KnowledgePromoted 0× at 900s) — resolved as chain-rare re-timing: mechanism structurally verified (LocationBeliefs-only input, all tests + false-belief scenario green), fired on the 1800s window. Watch-item rolled to step-24 re-promote: KnowledgePromoted cadence under wildlife prey-competition. Wildlife Flee/Fight writer estimators not needed at first light (write_wildlife_vs_cat extension deferred until tuning demands it).
