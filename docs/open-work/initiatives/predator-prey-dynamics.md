# Initiative: predator-prey-dynamics

**Outcome:** the world has *danger* — predator AI, prey-side AI, hunting mechanics, scent-based avoidance, ambush dynamics, the cost of being in the wrong place. Predators are antagonistic ecology, distinct from passive environment.

## What counts

- Predator planners (fox / hawk / snake / shadowfox GOAP and DSE work)
- Prey-side AI (Bolt, ScatterGroup, herd / flock behavior)
- Hunt mechanics — stalk, chase, pounce, kill, drop-carcass
- Scent-based ambush avoidance (perceiving danger through scent / signposts)
- Predator territorial behavior (fox scent-marking, range overlap, retreat)
- WitnessableEvent::Attack and the cat-side response substrate

## What doesn't count

- Cat-side fight / flight / freeze / fawn decision (that's `combat-threat` cluster; tickets can share this initiative if they're predator-coupled)
- Body-zone injury from predator attacks (that's `welfare-fidelity`)
- Cat-on-cat aggression substrate (that's `combat-threat`, not predator-prey)

## Example tickets

- `025` Hawk and snake GOAP planner domains
- `213` Shadowfox motivations distinct from normal foxes
- `260` Fox scent-marking signposts — territorial boundaries without ward keying
- `269` Prey-side AI — Bolt and ScatterGroup DSEs
- `175` Tremor map, Action::Stalk, and personality-driven hunt approach

## Canary signal

**Hard survival gate**: `deaths_by_cause.ShadowFoxAmbush <= 10` on canonical seed-42 (CLAUDE.md). Predator-prey tickets that *raise* this count without commensurate substrate elsewhere have priced the predator-exposure tradeoff wrong.

Doctrine memory: `project_l3_patrol_absorption_cascade` — substrate axes need to price the predator-exposure cost of what they elevate, not just the cost of what they suppress. Predator-prey tickets that elevate exposure must also surface its cost.
