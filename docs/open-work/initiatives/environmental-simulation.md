# Initiative: environmental-simulation

**Outcome:** the world is *alive independent of cats* — weather, seasons, scent decay, terrain physics, ambient wildlife ecology, influence maps that update from non-cat causes. The world doesn't pause when cats sleep.

## What counts

- New weather / season / day-night cycle systems
- Influence-map producers that update from environmental causes (not from cat actions)
- Terrain physics (tremor, smoke, water flow)
- Ambient wildlife behavior — predators / prey that act when no cat is watching
- Per-species scent / pheromone decay rules
- Cross-system environmental coupling (weather affects scent decay affects predator detection, etc.)

## What doesn't count

- Predator AI driven by cat detection (that's `predator-prey-dynamics`)
- Cat-side perception of environment (that's `full-sensory-perception`)
- Building / structure placement (that's `world-richness`)

## Example tickets

- `283` Environmental quality — five influence maps for ambient spatial pressure
- `223` Prey-species split — per-species scent maps
- `274` Ambient predator/prey behavior-observation enrichment
- `175` Tremor map, Action::Stalk, and personality-driven hunt approach (environmental side of the split)

## Canary signal

No hard canary; ecological-magical-realist doctrine (CLAUDE.md "Architecture") names this as the load-bearing direction. A balance-doc thread that documents *the world behaving differently when cats aren't present* is the qualitative signal.
