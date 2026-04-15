# Crepuscular Sleep Cycle

Cats are crepuscular — most active at dawn and dusk, sleeping through the night. The
current sleep model is purely deficit-based (energy level determines duration). A future
pass should make sleep time-of-day aware.

## Current model

`sleep_duration = base + (1.0 - energy) * multiplier`

Crepuscular behavior emerges indirectly: cats burn energy during active phases, enter
Night tired, and the deficit multiplier produces long sleeps. But there's no explicit
concept of "nighttime sleep" vs "midday nap."

## Future: DayPhase-aware sleep

- During Night phase, extend sleep duration to reach dawn (or a configurable wake hour).
- During Day phase, cap sleep to shorter naps (catnaps).
- Dawn/Dusk are peak activity — cats should resist sleeping unless critically exhausted.
- Pass `DayPhase` (or current tick + SimConfig) to the sleep step or scoring context.
- Sleep scoring could incorporate time-of-day: score Sleep higher at Night, lower at
  Dawn/Dusk.

## Prey animals

Prey species should also have activity cycles tuned to their real-world behavior:
- **Mice/rats**: nocturnal — most active at Night, den during Day
- **Rabbits**: crepuscular like cats — active Dawn/Dusk
- **Birds**: diurnal — active Day, roost at Night
- **Fish**: no strong cycle (underwater)

This creates natural predator-prey timing interactions: cats hunting at dusk encounter
crepuscular rabbits, nocturnal mice are available during Night hunts, birds require
daytime stalking.

## Dependencies

- Needs `DayPhase` accessible in sleep step and/or scoring context
- May want a per-species `ActivityCycle` component
- Prey grazing/denning behavior tied to activity cycles
