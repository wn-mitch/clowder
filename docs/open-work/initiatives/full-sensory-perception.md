# Initiative: full-sensory-perception

**Outcome:** cats perceive their environment in good *chunks* — orthogonal axes of belief and observation, each encoding a distinct situation, not a louder single alarm. This is design pillar §3 ("Richer perception, better strategy") made concrete in substrate.

## What counts

- New `WitnessableEvent` variants and emitters
- New per-cat belief / mental-model fields (`LocationBeliefs`, `ContextBeliefs`, `CatBeliefs`, `MentalModel<Cat>`)
- New influence-map producers or perception scalars
- New body-cue / audible-cue / scent channels
- Migration of single-axis perception scalars into orthogonal axes (the §148 distress refactor pattern)

## What doesn't count

- Tuning weights on existing belief integrators (that's balance work in the relevant cluster)
- Adding a UI overlay for an existing belief (that's `tooling-diagnostics-ui`)
- Surfacing perception via L2 — perception that *exists* but isn't consumed yet still counts

## Example tickets

- `258` belief substrate (mental models + facets + evidence typology)
- `272` Interoceptive self-anchors — spatial self-perception
- `262` Audible cue substrate (alarm calls, distress cries, hissing)
- `267` Behavior-observation L1 channel (target-side body-cue + physical marker reads)
- `295` WitnessableEvent emit sites

## Canary signal

The §3 pillar's load-bearing memory: `feedback_single_axis_perception_scalars` — each scalar in `src/systems/interoception.rs` encodes one orthogonal axis; personality / phobias / ambient anxiety compose at the modifier layer, never folded into the perception. A ticket that *adds back* a single-axis scalar regresses this initiative.
