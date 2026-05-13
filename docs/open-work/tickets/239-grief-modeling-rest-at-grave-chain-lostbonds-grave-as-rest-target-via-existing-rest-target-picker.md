---
id: 239
title: Grief modeling + rest-at-grave chain — LostBonds + grave-as-rest-target via existing rest-target picker
status: ready
cluster: life-cycle
initiative: [welfare-fidelity, mythic-texture]
added: 2026-05-08
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

The load-bearing emotional payoff of the burial system: **a kitten
sleeping on its parent's grave**. Per the user's design redirect:
graves should NOT get a new `RestAtGrave` DSE — substrate-over-
override. Instead, `Grave` entities should become candidate rest
targets routed through the **existing rest-target-selection DSE**.

The chronic emotional layer: when a cat dies, surviving kin / bonded
peers carry a `LostBonds` entry that contributes a small `loneliness`
deficit. Visiting the grave (resting on it) discharges some of the
deficit. The behavior emerges from layered substrate: bond ↔ grave
↔ chronic deficit ↔ rest-target preference.

## Scope

- New `LostBonds` component on cats: `Vec<LostBond { deceased_name,
  fondness_at_death, last_grave_visit: Option<u64> }>`. Inserted
  on death in `check_death` for every survivor with
  `relationships.fondness(deceased) > threshold`.
- `loneliness` axis on `Fulfillment` (or aggregated through
  `social_warmth_deficit` — design choice in approach).
- Rest-target picker (existing DSE) gains per-target axes for graves:
  - `bond_to_deceased` (read from `LostBonds.fondness_at_death`)
  - `grave_anti_corruption_safety` (sample `GraveAuraMap` at the
    grave's tile)
  - `distance` (existing rest-target axis)
- Grave-rest visit decays the `LostBond` entry's contribution and
  updates `last_grave_visit`. Lifetime decay of entries is the
  open balance knob — recommend lifetime-persistent with
  diminishing pull.

## Out of scope

- Acute grief modification (already exists at `DeathConstants::
  grief_mood_penalty` — don't touch).
- New `RestAtGrave` DSE — replaced by routing graves through the
  existing rest-target picker.
- Anti-corruption tuning (240).

## Approach

Substrate-over-override. The kitten-on-parent's-grave behavior emerges
from: `LostBond` deficit pulls cat toward rest → rest-target picker
sees the parent's grave with high `bond_to_deceased` axis → cat
elects to rest at the grave → visit discharges some loneliness.
No special-case logic; the existing rest-target machinery does the
work once Graves enter its candidate pool.

## Verification

- Scenario: `kitten_rest_at_parent_grave` — kitten with high fondness
  to a recently-deceased parent has the parent's grave appear in
  the rest-target ranking with a non-zero score.
- Continuity: rest-at-grave is a new behavioral expression; emit a
  `RestAtGrave` Feature for the never-fired-canary surface.

## Log

- 2026-05-08: opened as 035 follow-on. User redirected from
  RestAtGrave-as-new-DSE to substrate routing through existing rest-
  target picker.
