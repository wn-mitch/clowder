---
id: 034
title: Time-anchor continuity rebalance — restore play/grooming/mythic-texture under fixed prey-scent
status: ready
cluster: null
added: 2026-04-26
parked: null
blocked-by: []
supersedes: []
related-systems: [time-anchor.md]
related-balance: [time-anchor-iteration-1.md]
landed-at: null
landed-on: null
---

## Why

Ticket 033 Phase 1 fixed a 200× scent-decay bug (prey scent decay
`20/day → 1/day`) and reconciled three "100-tick" cadence stragglers to
once-per-day. Survival canaries improved (`Starvation 2 → 0`,
`anxiety_interrupt_total 62260 → 3057`), and the colony is dramatically
more active (`wards_placed 78 → 190`, `EngagePrey:*` plan failures dropped
from ~2400 to 0). But continuity tallies dropped > 30%:

| Tally | Phase 0 | Phase 1 | Δ |
|-------|---------|---------|---|
| `grooming` | 44 | 21 | -52% |
| `play` | 348 | 109 | -69% |
| `mythic-texture` | 40 | 23 | -43% |

The hard gates (≥1 per tally per soak) still hold, but the magnitudes
moved enough to flag CLAUDE.md's >±30% scrutiny band. Hypothesis: cats
now spend their time-budget on successful hunting / ward placement /
threat engagement instead of idle social activity.

## Goal

Re-tune so the colony is both healthier (Phase 1's gain) *and* visibly
playful/social (Phase 0's level), without returning to the broken
prey-scent state.

## Approach (sketch)

Candidate levers, in roughly increasing order of blast radius:

- Adjust `mood`/`needs` thresholds that gate `play` / `grooming` DSE
  scoring — if `play` is being out-scored by hunting now that hunger
  resolves faster, the play DSE may need a higher floor.
- Tighten coordinator/aspiration directive issuance under the new
  cadence so cats aren't drowning in active directives.
- Re-examine `scent_detect_threshold` (`0.05`) and
  `scent_deposit_per_tick` (`0.1`) on the prey side — if H2 over-corrected
  by making prey scent persist *too* usefully, a shorter `RatePerDay`
  might preserve the bug fix without making hunting trivial.
- Add an explicit "leisure budget" floor to the `Socialize` /
  `RecreationPlay` DSE bundles when needs are sated.

## Out of scope

- Reverting Phase 1's value changes — the prey-scent bug fix and cadence
  reductions are right and stay.
- Multi-seed verification of Phase 1's pattern — single-seed first per
  CLAUDE.md.

## Dependencies

- Ticket 033 (Phase 1) lands first. This is the follow-on.

## Verification

Per CLAUDE.md balance methodology — hypothesis / prediction / observation
/ concordance against `docs/balance/time-anchor-iteration-1.md` (extend
or fork to `time-anchor-iteration-2.md`).

## Log

- 2026-04-26: Ticket opened as a follow-on to 033 Phase 1.
