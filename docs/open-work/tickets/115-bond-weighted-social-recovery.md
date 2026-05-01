---
id: 115
title: Bond-weighted social recovery — fondness scales Needs.social inflow
status: ready
cluster: emotional-fidelity
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

`Needs.social` recovers at a flat rate regardless of who the cat is interacting with.
`src/steps/disposition/socialize.rs:63`: `needs.social += d.socialize_social_per_tick`
`src/steps/disposition/groom_other.rs:80`: `needs.social += d.groom_other_social_per_tick`

A stranger (fondness=0.0) and a life partner (fondness=1.0) give identical recovery. Cats
have no mechanical incentive to seek out bonded companions specifically — strangers are
equally satisfying, which is biologically dishonest.

`Fulfillment.social_warmth` was split from social by ticket 012, but neither resolver
weights recovery by the fondness of the specific interaction partner.

## Scope

- In `socialize.rs:63` and `groom_other.rs:80`, scale the social recovery rate by fondness.
- Formula: `recovery = base_rate * (1.0 + bond_social_fondness_scale * fondness.max(0.0))`
  — strangers give 1.0× base; bonded cats give up to (1.0 + scale)× base.
- `bond_social_fondness_scale: f32` default 0.5 in `SocializingConstants`.
- Hostile interactions (fondness < 0): `.max(0.0)` guard — no penalty, just no bonus.
- Access requirements: verify both resolvers have target entity ID and `&Relationships`
  resource in their call chain (may need to thread from `goap.rs` dispatch).

## Verification

- `just soak 42` + `just verdict`.
- `just hypothesize` sweep with scale values [0.0, 0.25, 0.5]: confirm bonded pairs trend
  toward higher social-need satisfaction. Watch MatingOccurred (learned from range=25
  regression: bond-weighted inflow should help not hurt mating cadence).

## Log

- 2026-05-01: Opened as tier-1 emotional-fidelity ticket from DSE flattening audit.
  Independent of ticket 114.
