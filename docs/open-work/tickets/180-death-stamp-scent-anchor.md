---
id: 180
title: Death-stamp / scent-anchor at kill sites (176 follow-on)
status: ready
cluster: world-ecology
added: 2026-05-05
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

User refinement during ticket 176 planning (2026-05-05): every
kill should leave a position-anchored stamp that persists after
the carcass is removed. Stage 2 of 176 changed `engage_prey` to
spawn a real carcass `Item` entity at the kill site (no more
silent orphan), but the *stamp* — the metaphysical-weight signal
that "X died here" — is a separate concept that should outlive
the carcass.

Honors the project-vision §Maslow ecological-magical-realism
doctrine: death is a *real event with metaphysical weight*, not
a transient state that disappears the instant the body is taken.

## Future consumers

- **Scavenger AI** — shadowfox / hawks drawn to recent kills
  even after the carcass is collected.
- **Corruption sensors** — stamps as a corruption-attractor
  signal in the existing corruption surface.
- **Cat phobic / mournful reactions** — cats with high
  spirituality / anxiety perceive stamps and shift mood.
- **Mythic events** — fate-line bookkeeping anchored to "where
  X fell."

## Direction

Substrate sketch:

- New component `DeathStamp { position: Position, intensity: f32, age_ticks: u64 }`
  on a stamp entity spawned when a creature dies (cats, prey,
  fox).
- Decay system that ticks intensity downward over time; despawns
  when intensity hits zero.
- Distinct from carcass entities — both can co-exist; the
  carcass is "the body lying there," the stamp is "the smell of
  death lingering." Stamps may outlive carcasses (carcass eaten
  → stamp persists for N ticks afterward).

## Out of scope

- Specific scavenger AI / corruption-sensor / mythic-event
  consumers — those are separate tickets each.
- Stamp-decay tuning constants — start with a placeholder
  decay rate in SimConstants (`death_stamp_decay_rate`).

## Verification

- Post-fix soak shows `DeathStamp` entities spawned 1:1 with
  cat / prey / fox deaths.
- Stamps decay correctly over time in unit tests.
- Existing survival hard-gates unchanged.

## Log

- 2026-05-05: opened from user direction during ticket 176
  planning. Project memory captured at
  `~/.claude/projects/-Users-will-mitchell-clowder/memory/project_death_smells.md`.
