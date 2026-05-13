# Initiative: welfare-fidelity

**Outcome:** the **honest ecology** project pillar made concrete. IRL cat biology realism, anatomically-aware injury, body-distress modeling that maps to felt experience, starvation tuned to be *interesting not cutthroat*. The sim doesn't gamify suffering or trivialize biology.

## What counts

- Body zones / anatomical injury substrate
- Body-distress modifier family (Thermal / Hunger / ThreatProximityAdrenaline / etc.)
- Damage-recency / pain perception scalars
- Bond-weighted social recovery (welfare flowing through relationships)
- Starvation / hunger curve realignment with cat biology
- Health / medical resolver work that respects anatomy

## What doesn't count

- Hunting mechanics (that's `wildlife` cluster, may share `predator-prey-dynamics` initiative)
- Kitten-specific care substrate (that's `generational-continuity` — different lifecycle slice)
- Pure mental-state perception (that's `full-sensory-perception`)

## Example tickets

- `032` Starvation rebalance — align with IRL cat biology, interesting not cutthroat
- `095` Body zones — anatomical injury model for all animal species
- `115` Bond-weighted social recovery — fondness scales Needs.social inflow
- `088` BodyDistressPromotion (and its retirement once kind-specific modifiers cover the surface)

## Canary signal

**Hard survival gate**: `deaths_by_cause.Starvation == 0` on canonical seed-42 (CLAUDE.md). Welfare-fidelity tickets must not regress the gate. **Continuity canaries**: grooming, play, mentoring — all are welfare-bound; a colony where these go to zero has welfare collapse regardless of survival count.

Doctrine: design pillar §3 (Richer perception, better strategy) — welfare improves when cats understand their environment in good chunks, not when individual systems become more punitive.
