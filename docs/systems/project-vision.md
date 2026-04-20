# Project vision

> **A clowder of cats, living in a world that has its own weight.**

## Thesis

The cats are the stars. Named, personality-driven, their lives accumulate
into stories we follow — kittens grow into elders, hunters earn their
reputations, pairs bond and grieve, rare cats witness or produce the strange.

The world around them is honest. Ecology that doesn't bend to their needs,
carrying a mythic undercurrent that doesn't bend to them either. Seasons
turn regardless. Prey oscillates regardless. Shadowfoxes stalk the fog
regardless. Corruption seeps regardless.

The cats' courage, craft, mentorship, mating, grief, and rare mystical
Calling are *responses* to a world that was here before them and will
continue after.

**The game is watching a clowder live in that world.** Dwarf Fortress is about
dwarves. Timberborn is about beavers. Clowder is about cats.

## Category

A colony sim starring cats, in the **ecological-magical-realist mode.** No
direct commercial precedent — Clowder sits in a gap between the DF/RimWorld
colony-sim tradition and the Watership-Down / Ghibli literary tradition.

## Influences

- **Watership Down** (Richard Adams) — the definitive literary match. Real
  ecology (warrens, predators, seasons, farmer politics) and mythic
  cosmology (Frith, El-ahrairah, the Black Rabbit of Inlé) share one world,
  interpenetrating rather than stacked. A rabbit fleeing a fox is fleeing
  death *and* the Black Rabbit simultaneously.
- **Timberborn** — structural peer. Beavers as a colony of protagonists
  navigating an indifferent ecosystem. The game is the world reacting,
  observed through the beavers you care about.
- **Ghibli** (*Princess Mononoke*, *My Neighbor Totoro*) — forest spirits
  and honest ecology occupy the same register. The supernatural *is* the
  natural, seen slant.
- **Pilgrim at Tinker Creek** (Annie Dillard) — the posture. The natural
  world is already strange enough; what's needed is mythic attention, not
  invented drama.
- **Endling: Extinction is Forever** — mother fox in a dying world. Real
  ecology with a mythic atmosphere, closer in tone than most colony sims.
- **Dwarf Fortress** — simulation depth that produces narrative from
  independent system collisions. The beer-cats-puke-depression spiral is
  the gold standard for emergence. DF is about *dwarves* in a weighty
  world — same structure, different creatures.

## Design corollaries

These fall out of the thesis. They're load-bearing for balance and feature
decisions.

### 1. Magic is ecology with metaphysical weight

Corruption, wards, the Calling, fate, prophecy — these are not a separate
"narrative layer" the cats reach once they're fed and housed. They are
**ecological phenomena** that happen to carry metaphysical weight.

- Corruption is an environmental hazard, like disease or pollution.
- Wards are territorial marks the world registers.
- The Calling is a rare psychological state in a world where minds can do
  that.
- Fate is a low-frequency pattern the simulation samples.

Tune them as part of the ecosystem, not as rewards.

### 2. No director, no storyteller

RimWorld has Cassandra/Phoebe/Randy — explicit event injectors that raise
drama when the colony is too stable. Clowder does not need one.

Seasons, weather, migration, predator-prey oscillation, corruption cycles,
and fate sampling *already are* the event generator. Trust them.

This is a real architectural simplification. An entire piece of work drops
off the roadmap.

### 3. The world doesn't bend to the cats

No difficulty scaling. No protagonist shield. No encounter weighting toward
"what the colony needs to see." A cat who wanders into a fog-bound
shadowfox ambush dies whether or not that cat was the player's favorite.
Prey populations collapse if over-hunted. Winter arrives on schedule.

Cats earn their stories by surviving a world that doesn't care.

### 4. Survival lock is a bug, not a difficulty level

A colony that only hunts, forages, and rests is failing to show its range.
Grooming, play, mentoring, courtship, burial, wandering, magic, the
Calling — these are ecological behaviors too, part of what a cat's life
*is*. A run where cats never have leisure is a run where the world has
become a treadmill, not a world.

This is what the **continuity canaries** in `CLAUDE.md` catch.

### 5. Broaden sideways, not deeper into predation

Predation is already well-developed (prey 1.5kloc, wildlife 2.5kloc, sensing
1.2kloc, GOAP 4kloc). The thin axes are the Watership-Down-shaped
behaviors:

- Grooming as a social and health economy
- Play (especially kitten play — social learning)
- Courtship rituals beyond the mating gate
- Corpse decay and burial cycles
- Seasonal food preservation (drying, smoking, cold storage)
- Generational knowledge transfer (what elders teach kittens)

Future balance work should favor these over more prey/sensing depth.

## Continuity canaries

(Also listed in `CLAUDE.md`; repeated here for context.) On seed-42
`--duration 900`:

- **Generational continuity:** ≥1 kitten reaches adulthood.
- **Ecological variety:** grooming, play, mentoring, burial, courtship each
  fire ≥1×.
- **Mythic texture:** ≥1 named event per sim year (Calling, banishment,
  visitor, named object).

All-zero on any of these is a regression of equal severity to
`Starvation > 0`.

## Related system docs

- [`magic.md`](magic.md) — herbcraft, wards, corruption.
- [`the-calling.md`](the-calling.md) — rare mystical trance producing named
  objects.
- [`collective-memory.md`](collective-memory.md) — social transmission of
  knowledge across cats and generations.
- [`raids.md`](raids.md) — organized external incursions as ecological
  events.
- [`trade.md`](trade.md) — visitors and barter.
- [`weather.md`](weather.md) — seasons, diurnal rhythm, weather types.
- [`sensory.md`](sensory.md) — four-channel perception (sight, hearing,
  scent, tremor).
- [`corpse-handling.md`](corpse-handling.md) — death's ecological and
  social footprint.
