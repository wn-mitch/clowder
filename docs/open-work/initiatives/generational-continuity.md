# Initiative: generational-continuity

**Outcome:** the colony *persists across generations* — mating, kitten birth, alloparental care, maturation, and the lineage threads that connect founder cats to their descendants. The colony has a history, not just a roster.

## What counts

- Mating cadence / breeding-floor / fertility substrate
- Kitten cries, caretake plans, RetrieveFoodForKitten pipeline
- Alloparenting Reframes (mama drops kitten at hearth near resting elder, etc.)
- Apprenticeship / mentor relationships across maturity boundaries
- KittenMatured tracking and the continuity-canary surface
- Lineage / pedigree (parent-of, sibling-of relations as substrate)

## What doesn't count

- Adult social bonding (that's `social-coordination` cluster, no this initiative)
- Death and burial (that's `life-cycle` cluster shared with `mythic-texture` initiative)
- Body-zones for kittens specifically (that's `welfare-fidelity` — same substrate, different outcome)

## Example tickets

- `119` Mating cadence — three-bug cascade blocking MatingOccurred
- `212` Caretake plans complete but KittenFed never fires — kitten starvation chronic
- `222` Alloparenting Reframe B — mama drops kitten at hearth near resting elder
- `265` Apprenticeship XP-boost on per-skill Skills component
- `257` Body-cue-driven joint adoption (compose 127 with 242 + 243)

## Canary signal

**Hard survival gate**: `kittens_surviving > 0` (or equivalent KittenMatured event tally). A soak with zero kittens or zero matured kittens has demographic collapse — the colony doesn't persist.

Doctrine memory: `feedback_park_demographic_dependent_tuning` — when a DSE eligibility depends on a precondition the colony is currently failing (e.g. Caretake gate vs MatingOccurred-never-fired), park the tuning behind the substrate ticket. Generational-continuity tickets are upstream of most welfare and social-coordination tuning.
