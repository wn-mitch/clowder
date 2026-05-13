# Initiative: world-richness

**Outcome:** the world has *stuff to see and find* — visible diversity that rewards looking around. Tickets contribute when they add to the inventory of things-in-the-world: items, recipes, monuments, civic structures, ruins, named landmarks, environmental detail, happy-paths trails worn into the terrain by use.

## What counts

- New item kinds or recipes that produce items
- Monument / civic / memorial structure variants
- Spatial features the player can discover (ruin clearings, named landmarks, scent signposts)
- Environmental textures that emerge from cat behavior (worn trails, scent-marked perimeters, midden zones)

## What doesn't count

- Tuning ward placement weights (that's `buildings-zones` cluster, no initiative — pure numeric tune)
- Adding a marker that drives a DSE but doesn't have visible expression (that's `full-sensory-perception`)
- Per-cat decoration / sprite work without world-side state (that's UI work)

## Example tickets

- `065` Crafting recipes from foraged herbs
- `180` Monument variant placement
- `253` Happy paths — usage-worn trails
- `260` Fox scent-marking signposts
- `301` evolve ward placement decision semantics

## Canary signal

No hard canary yet. A soft signal: the named-event vocabulary expands without proportional code growth (i.e., new content slots into existing substrate). If a ticket adds an item or structure that requires brand-new resolver work to be visible, it may belong primarily under a different initiative.
