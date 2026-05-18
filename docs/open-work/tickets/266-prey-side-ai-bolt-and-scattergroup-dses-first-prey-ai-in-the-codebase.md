---
id: 266
title: Prey-side AI — Bolt and ScatterGroup DSEs (first prey AI in the codebase)
status: ready
cluster: wildlife
orchestration: substrate-sensitive
initiative: [predator-prey-dynamics]
added: 2026-05-10
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Prey (rabbits, mice, voles, etc. per `PreyKind` in `wildlife.rs`) currently have no AI — they exist as kill targets for cat/fox/hawk Hunt DSEs but don't make decisions of their own. This is acceptable when the substrate had no way to express prey-side perception, but the C3 spinout (258) + ActionAffordances (261) make prey AI cheap to add: with `Chase` enumerated as a perceivable action kind, prey can read `Affordance(Chase, perceiver=predator, target=me)` and decide. Two minimal v1 prey DSEs (`Bolt` for individual flight, `ScatterGroup` for herd flight) give prey honest threat-response without adding new substrate machinery.

The payoff: predation feels like predation. Prey that bolt at the right moment + scatter when grouped + freeze when stalking-affordance is high produce the predator-prey arc the third design pillar wants ("richer perception, better strategy" — prey perceiving Chase as an enumerated action means they can avoid it, not just be caught by it). Hunt's success rate becomes a function of *both* sides' affordance reads, not just the predator's.

## Scope

- **`Bolt` DSE** (new, under `src/ai/dses/prey_bolt.rs` or similar): individual prey flight. Reads `Affordance(Chase, predator, me)` + `MentalModel<Predator>(predator).recency_of_threat_cue`. Wins L3 election when affordance × belief crosses threshold. Resolver moves prey toward nearest cover.
- **`ScatterGroup` DSE** (new): herd-level flight. Reads herd density + cover distribution + per-predator affordance. When herd is dense and predator commits to Chase, scatter to break pursuit lock.
- **Prey-side `MentalModel<Predator>` reads**: prey carry mental models too. v1 facets: `recency_of_threat_cue`, `perceived_violence_capability` (instinct from species priors). No social facets (prey aren't socially complex enough at v1; revisit if prey behavior gets richer).
- **Prey eligibility for the AI tick loop**: prey entities currently aren't scored per tick. v1 adds them to the AI scoring loop with a minimal DSE roster (just Bolt + ScatterGroup); future tickets can extend (Forage, Rest, Mate at the prey level).
- **Performance budget**: prey are typically more numerous than cats. v1 must verify the AI tick budget doesn't blow up — if prey count × DSE count × per-tick cost exceeds budget, this ticket adds prey scoring with a longer cadence (every Nth tick) rather than every tick.

## Out of scope

- Prey reproduction / lifecycle / death-by-old-age (existing systems handle).
- Prey foraging behavior (future ticket — Forage DSE for prey reads grass/seed distribution).
- Prey social behavior (mating displays, territorial defense). Prey AI v1 is purely threat-response.
- Per-PreyKind specialization. v1 is uniform Bolt + ScatterGroup across all `PreyKind` variants. Per-species specialization (e.g. rabbits scatter, mice freeze-then-bolt, voles tunnel) is a follow-on.
- Hawk's aerial-pursuit-vs-prey-bolt-arc fidelity. Hawks Dive; prey Bolt or ScatterGroup. The interaction shape works without per-encounter choreography; richer choreography is a stretch goal.

## Current state

- Blocked-by 258 (Belief substrate) + 261 (ActionAffordances substrate). Prey need MentalModels and need to read `Affordance(Chase, ...)`.
- Existing `PreyKind` enum in `wildlife.rs` — variants per the layer-walk findings. Prey entities exist; they're just not AI-driven.
- Existing Hunt DSE on cats targets prey — verify post-substrate-wiring the Hunt-success rate doesn't crash to zero (predation should remain viable; prey just become competent at avoiding catastrophically-bad predator approaches).

## Approach

1. Confirm prey are entities with sensible spatial components (Position, etc.) but no AI components (no DSE roster, no Disposition).
2. Add minimal AI components to prey: per-cat scoring, per-tick L1/L2/L3 election (or per-Nth-tick if performance demands).
3. Land Bolt DSE as the simplest possible per-cat per-target-predator threat-response.
4. Land ScatterGroup DSE second; require proximity-to-conspecifics scoring component.
5. Verify Hunt-success rate stays viable post-prey-AI (this is the main risk — too-competent prey could starve cats).
6. Tune per-PreyKind in `SimConstants` if rabbits-vs-mice-vs-voles need different bolt thresholds.

## Verification

### Per-DSE scenario microexperiments

- `prey_bolt_at_chase_affordance_threshold` — predator approaches with rising Chase-affordance; verify prey Bolt fires at the threshold, not before.
- `prey_no_bolt_at_low_affordance` — predator nearby but Affordance(Chase) low (e.g. predator wounded, slow); verify prey doesn't bolt prematurely.
- `prey_scatter_breaks_pursuit_lock` — herd of 5 prey, predator chasing one; verify ScatterGroup fires and predator's pursuit-target switches mid-chase.

### Soak gates

- Hunt-success rate post-this-ticket within ~30% of pre-this-ticket baseline (predation viable).
- `cats_starvation == 0` (hard gate; if prey AI tanks predation success, cats will starve — this canary catches it).
- Per-tick simulation step time within budget (perf canary if exists).
- shadow_fox_spawn_total, ShadowFoxAmbush canaries unaffected (this ticket changes prey AI, not magic / threat / wildlife-on-cat axes).

### Frame-diff

`just frame-diff <pre-this-ticket> <post-this-ticket>` confirms cat Hunt DSE's per-tick scoring distribution stays similar (target-availability shifts, not target-scoring shape).

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed **263** (done, ai-substrate, score 0.89 (cross-cluster)) — 256-cluster DSE consumers wire belief + affordance axes (Flee, Patrol, Hunt wit…
- · **265** (ready, wildlife, score 0.87) — Wildlife symmetric DSE consumers wire belief + affordance (fox, hawk, snake, sh…
- ✓ landed **209** (done, balance, score 0.86 (cross-cluster)) — Positive colony_food_security axis on higher-tier DSEs

<!-- linkages:end -->
## Log

- 2026-05-10: opened sibling-to-258. First prey AI in the codebase. Enabled, not delivered, by the C3 + Affordance substrates landing in the same cluster lifecycle. Session plan: `~/.claude/plans/after-working-256-i-dreamy-fiddle.md`.
