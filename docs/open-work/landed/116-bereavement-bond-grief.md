---
id: 116
title: Bereavement — bond-specific grief modifier on partner/friend death
status: done
cluster: emotional-fidelity
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: 60c7c6ac
landed-on: 2026-05-01
---

## Why

When a bonded cat dies, there is no bond-specific behavioral consequence. The existing
proximity-based grief in `src/systems/death.rs:126-149` fires for ALL nearby cats
regardless of relationship depth. `MemoryType::Death` exists but has no relationship-
weighted behavioral consequence.

A cat's life partner dying should affect it deeply and lastingly, regardless of whether
it was physically nearby. The `FatedLove` system already handles mythic-bond deaths
(line 152); this ticket adds general-purpose bond grief for `Relationships` bonds
(Friends, Partners, Mates).

Grief is load-bearing for the mythic-texture narrative canary and the burial canary.

## Scope

- In `src/systems/death.rs`, add a bond-grief scan loop after the proximity-grief block.
- For each living cat with a `BondType` relationship to the deceased: push a `MoodModifier`
  with `kind: MoodSource::Grief` (ticket 114), duration scaled by bond type, amount scaled
  by fondness.
- New constants in `DeathConstants`: `bereavement_mates_intensity` (0.7),
  `bereavement_partners_intensity` (0.5), `bereavement_friends_intensity` (0.3), plus
  duration constants per bond type (in `DurationDays`).
- Add `Res<Relationships>` and `Entity` + `&Personality` to `check_death` query.
- Ships active (no 0.0 default) — grief is narrative-load-bearing with no survival risk.

## Verification

- `just soak 42` + `just verdict`: no survival canary regression.
- `just narrative`: burial events and sustained-negative-mood cats should correlate
  with bond-loss events in the same run.
- Mythic-texture canary: grief deepens the emotional register.

## Log

- 2026-05-01: Opened as tier-1 emotional-fidelity ticket from DSE flattening audit.
  Depends on ticket 114 for typed MoodSource::Grief.
