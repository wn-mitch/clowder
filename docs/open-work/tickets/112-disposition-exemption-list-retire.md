---
id: 112
title: Retire per-disposition exemption lists (Resting/Hunting/Foraging/Guarding) — substrate replacement via commitment momentum
status: ready
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

`src/systems/disposition.rs:305-342` carries per-disposition exemption lists for the interrupt cascade (Resting/Hunting/Foraging exempt from hunger interrupts; Guarding exempt from threat interrupts). Same per-tick-override pattern as the interrupt branches 047 retired, in special-case clothing.

The 047 ticket text explicitly names this cluster: "the per-disposition exemption lists at `disposition.rs:305-342` are the same pattern in special-case clothing." Substrate-over-override wants a Rao-Georgeff §7.2 commitment/momentum modifier — a cat already pursuing Hunt has implicit momentum that resists Starvation interrupts because the planner hasn't yet succeeded in completing Hunt.

## Scope

Umbrella ticket. Sub-tickets per disposition if scope demands (likely it will):
- `112a-resting-exemption-retire.md` — Resting cats already in den shouldn't reroute to a different need-fulfillment disposition. Substrate: commitment momentum on Resting that decays only after rest_complete.
- `112b-hunting-exemption-retire.md` — Hunting cats are already pursuing food. Substrate: HungerUrgency (ticket 106) + commitment-tenure modifier on Hunting.
- `112c-foraging-exemption-retire.md` — same pattern.
- `112d-guarding-threat-exemption-retire.md` — Guarding's threat exemption is its own thing; the cat IS the threat-response. Substrate: gate the ThreatDetected interrupt on `disposition.kind != Guarding` from substrate, not from imperative match.

## Verification

- Each sub-ticket follows the 047 playbook: substrate first, focal-trace verify, hypothesize sweep, retire exemption.

## Out of scope

- Retiring the §7.2 CommitmentTenure modifier itself — it's the substrate replacement, not the override.

## Log

- 2026-05-01: Opened as cleanup follow-on from ticket 047. Same pattern (substrate-over-override) as the interrupt branches retired in 047 + 106 + 107 + 108.
