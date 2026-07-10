---
id: 533
title: Prey graze real depletable regrowing vegetation resources (herb-regrowth model) — replace passive_hunger_relief hand-wave so prey_hunger and prey_population gain an honest ecological input
status: ready
cluster: wildlife
initiative: [predator-prey-dynamics]
added: 2026-07-09
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
Prey already carry a hunger scalar (`prey_hunger`, `src/systems/prey.rs`) and a
predation-suppressed breeding loop (`prey_population`), but the *food* those
loops feed on is fake. Mice and rats raid colony `Stores` (a real, spatial
interaction). Every other prey — and every non-raid tick — just does
`state.hunger -= p.passive_hunger_relief`: free satiety out of thin air.
`PreyAiState::Grazing` is cosmetic wandering, disconnected from that relief. So
grazing has no reason, foraging has no cost, and the "ecology of fear" breeding
suppression sits on top of a hunger economy with no honest input. This violates
**items-are-real / no-abstract-resources** (design pillar 1) at the prey layer.

Cats already forage against *real* resources: `CanForage` terrain clusters and
`Harvestable` herb plants that deplete and regrow via `herb_regrowth`
(`src/systems/magic.rs`), painted into `herb_location_map`. Prey should graze
against the same class of real, spatial, depletable-and-regrowing resource so
that grazing *means* something and the existing hunger/population machinery
finally has a load-bearing ecological driver.

## Scope
- A grazeable vegetation resource entity/marker (working name `Forageable` /
  grass-forage node), placed on prey-habitat terrain at world-gen, mirroring the
  `Harvestable` + `herb_location_map` + `herb_regrowth` shape.
- `PreyAiState::Grazing` resolution consumes from the nearest forage node in
  range: depletes the node, applies hunger relief keyed to what was actually
  eaten. Grazing movement biases toward non-depleted forage.
- A regrowth system (peer of `herb_regrowth`) that regenerates forage over time,
  seasonally modulated where sensible.
- Replace the unconditional `passive_hunger_relief` with relief gated on actually
  reaching + consuming forage. Preserve the mouse/rat store-raid path unchanged.
- New constants live in `PreyConstants` (`src/resources/sim_constants.rs`) — no
  inline magic numbers; the graze-relief / depletion / regrowth rates.

## Out of scope
- Predation-driven population dynamics — already shipped (`prey_population`
  fear-breeding suppression). This ticket only supplies the *food* input.
- Landscape-of-fear graze displacement (separate ticket — prey avoiding
  predator-sign tiles): composes with this but is independent.
- Sentinel/forage role division (separate ticket): also composes here.
- Cat-side foraging — untouched; this ticket only mirrors its resource model.

## Current state
Nothing landed toward this. `prey_hunger` and `prey_population` are live and
registered in `src/plugins/simulation.rs`. The `Harvestable` / `herb_regrowth` /
`herb_location_map` trio is the reference implementation to mirror.

## Approach
Follow the herb model closely to stay on the well-worn path:
1. Component/marker for a forage node + spawn at world-gen on prey-habitat
   terrain (reuse the Poisson/habitat machinery in `world_gen/prey_ecosystem.rs`
   where it fits).
2. Optional influence map (`forage_location_map`) if grazing needs to path
   toward density; otherwise nearest-in-range scan on the existing grazing tick.
3. Source/Transfer/Sink discipline for the eat: consumption passes a named gate,
   not an inline scalar mutation (memory `project_items_are_real_source_transfer_sink`).
4. Regrowth system registered alongside the prey systems; seasonal modulation via
   the same season plumbing `prey_population` already reads.
5. Swap `passive_hunger_relief` for consumption-gated relief; keep starvation
   drain intact so a forage-starved region still culls prey honestly.

## Verification
- Focal/inspect a prey animal: hunger falls only after it reaches and consumes a
  forage node, not every idle tick.
- Overgrazed region depletes and prey hunger/mortality rises there; forage
  regrows and the region recovers — a visible boom/bust in `clowder-stat-trends`.
- `just verdict` survival + continuity canaries hold; prey population does not
  collapse or explode on seed-42. Four-artifact balance framing for the
  hunger-economy shift (behavior-changing).

## Log
- 2026-07-09: Opened from `/ideate` prey-ecology pass (idea #4, reframed). Source
  audit confirmed `passive_hunger_relief` is the hand-wave to replace; herb model
  is the template. Composes with the sentinel (#1) and landscape-of-fear (#8)
  tickets opened in the same pass.
