---
id: 265
title: Wildlife symmetric DSE consumers wire belief + affordance (fox, hawk, snake, shadowfox)
status: blocked
cluster: C
added: 2026-05-10
parked: null
blocked-by: [258, 261]
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
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

## Log

- 2026-05-10: opened sibling-to-258. Wires Belief + Affordance into wildlife DSE catalog symmetrically. Species clash now happens because both sides perceive the substrate honestly. Session plan: `~/.claude/plans/after-working-256-i-dreamy-fiddle.md`.
