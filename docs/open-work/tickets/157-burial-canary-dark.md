---
id: 157
title: Burial continuity canary dark on post-154 soak — verify eligibility against new death distribution
status: ready
cluster: ai-substrate
added: 2026-05-03
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: [mentoring-extraction.md]
landed-at: null
landed-on: null
---

## Why

Ticket 154's land-day soak (`logs/tuned-42`, seed 42, commit
`bb189bc`, 8 sim years) lit `continuity_tallies.burial = 0`. The
CLAUDE.md continuity-canary set requires every named canary to fire
≥1 per soak (`grooming · play · mentoring · burial · courtship ·
mythic-texture`). The other five canaries lit cleanly (mentoring at
1614, courtship at 5008, grooming at 388, play at 142, mythic-texture
at 47). Only burial stayed dark.

The run had 5 deaths total (2 Starvation kittens at the same tile in
year 8; 3 ShadowFoxAmbush adults). Burial may have eligibility
predicates (witness-adult availability, non-fled state, distance to
corpse, personality threshold, kitten-vs-adult corpse, etc.) that
this specific death distribution doesn't match. The investigation
step is to confirm that — *not* to assume the burial system is
broken. Pre-154 baselines lit burial (per CLAUDE.md's continuity-set
expectation), so the system itself works; what's needed is to verify
why this run's specific death pattern slipped through.

The balance-layer narrative is at
`docs/balance/mentoring-extraction.md` Iter 1.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Death event emission | `src/systems/health.rs` (or wherever `EventKind::Death` fires) | corpse position + cause logged on death | `[verified-correct]` (deaths logged successfully) |
| Burial eligibility | unknown — likely `src/systems/burial.rs` or an aspiration DSE | unverified — what predicates gate "an adult buries this corpse"? | `[suspect]` |
| Burial step / chain | unknown | unverified — does a `BuryCorpse` step kind exist? Is it part of any plan template? | `[suspect]` |
| Continuity-tally increment | `src/resources/event_log.rs` | `continuity_tallies["burial"]` increments on which `EventKind`? | `[suspect]` |

## Investigation step

1. **Find the burial system.** `rg -n "burial\|bury_corpse\|BuryCorpse"` to locate the code path. Document where the increment happens and what event triggers it.
2. **Audit eligibility against this run's death pattern.** For each of the 5 deaths:
   - 2 Starvation kittens at (38,22) — were any adults nearby and idle? Are kitten corpses eligible for burial?
   - 3 ShadowFoxAmbush adults — were the ambushers still present when the cat died? Does combat-context block burial?
3. **Compare against a baseline run that lit burial ≥1.** Find a recent pre-154 soak with `continuity_tallies.burial > 0`, identify a death event that produced a burial, and compare its surrounding context (witnesses, distances, ambient threats, personalities) to the post-154 deaths.

The investigation step must finish before fix candidates are listed —
without knowing the eligibility shape, parameter-level fixes are
guesses.

## Fix candidates (placeholder; refine after investigation)

**Parameter-level:**

- R1 — relax a too-strict eligibility predicate (e.g. distance,
  witness count, post-death cooldown) if one is found to be cutting
  out reasonable cases.
- R2 — extend burial eligibility to cover kitten corpses if
  currently adult-only.

**Structural:**

- R3 (**extend**) — if burial currently rides on Caretaking or another
  disposition's chain, add a per-corpse marker (`UnburiedCorpse`)
  and have an aspiration-DSE author it; surfaces burial as substrate
  rather than a hidden chain post-condition.
- R4 (**split**) — if burial is currently a side-effect of some other
  resolver, give it its own `DispositionKind::Burying` (mirror the
  pattern of 150 R5a / 154's Mentoring split).

## Recommended direction

**Defer until investigation completes.** Likely R1 (eligibility relax)
unless the investigation surfaces a structural shape; R3 if burial
turns out to ride on another disposition's chain (substrate-vs-search-
state).

## Verification

- Post-fix `just soak 42` → `just verdict` reports
  `continuity_tallies.burial >= 1`.
- Other continuity canaries unchanged: `mentoring`, `grooming`,
  `play`, `courtship`, `mythic-texture` all stay ≥1.
- No new deaths introduced by the fix (sanity check).

## Out of scope

- **Kitten starvation localization** (ticket 156).
- **GroomedOther structural treatment** (ticket 158).
- **Burial-system rebalance** beyond restoring the canary.

## Log

- 2026-05-03: opened by ticket 154's land-day verdict.
