---
id: 109
title: IntraspeciesConflictResponse — full four-valence (fight/flight/freeze/fawn) social response
status: in-progress
cluster: ai-substrate
added: 2026-05-01
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

The social analog of `AcuteHealthAdrenaline` (047). Predators don't accept appeasement; cats *do*. Intraspecies conflict (subordinate-vs-dominant context, mate competition, territorial dispute) has a full four-valence response repertoire — including **fawn** (the appeasement valence missing from predator response).

Reads a separate `social_status_distress` scalar (subordinate cat in dominant cat's space; mate competition with a stronger rival; territorial intrusion by a high-status cat). Distinct from physical body distress; distinct from predator threat.

## Scope

**Phase A** (this ticket): substrate scaffolding + Flee valence (subordinate retreat). Other valences are sub-tickets.

- New `social_status_distress` scalar in `interoception.rs`. Composition TBD — likely combines status differential vs nearest cat with proximity/intrusion factors.
- Publish via `ctx_scalars`.
- New `IntraspeciesConflictResponseFlight` modifier — lifts movement-away-from-dominant action (Flee or new "Withdraw" subaction; pick during impl).

**Phase B sub-tickets** (open during this work):
- `IntraspeciesConflictResponseFight` — territorial combat valence; lifts Fight against same-species rival.
- `IntraspeciesConflictResponseFreeze` — hold-position low-body-posture; reuses Hide/Freeze DSE from ticket 104.
- `IntraspeciesConflictResponseFawn` — belly-up, slow blink, appeasement gesture. **Requires new `Submit` gesture DSE** (or repurposes existing socialize-gesture machinery) — likely its own infrastructure ticket.

## Verification

- Phase A: focal-trace soak with subordinate cat near dominant cat shows withdrawal behavior over staying-put.
- Phase B: each sub-ticket gets its own focal-trace + hypothesize cycle.

## Out of scope

- Cross-species fawn (e.g. cat appeasing a fox) — ecologically incoherent; predator-response branches do not include fawn for that reason.
- Submit DSE infrastructure — likely opens as its own ticket alongside 109-Phase B.

## Log

- 2026-05-01: Opened as the social analog to ticket 047's AcuteHealthAdrenaline framework. Blocked by 104 (Hide/Freeze DSE) for the Freeze sub-valence; Phase A (Flee) can ship without it.
- 2026-05-02: 104 landed (2a68f595). **Phase A landed** at ca140e5d — `IntraspeciesConflictResponseFlight` modifier registered (pipeline +1), 2 ScoringConstants, 6 unit tests. The `social_status_distress` scalar is published as a 0.0 stub from `ctx_scalars`; v1 composition `(status_diff_to_nearest_cat × proximity_factor)` requires a defensible status-differential signal + per-cat nearest-cat resolution, which lands alongside lift activation. Phase B sub-tickets opened: 142 (Freeze), 143 (Fight), 144 (Fawn), 145 (Submit gesture DSE infrastructure).
