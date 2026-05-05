---
id: 170
title: HideEligible authoring system (Hide DSE Phase 2)
status: ready
cluster: ai-substrate
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

`HideEligible` (`src/components/markers.rs:230`) is *read* by the Hide
DSE — `src/ai/dses/hide.rs:94` declares
`eligibility: EligibilityFilter::new().require(markers::HideEligible::KEY)`
— but never *authored* by any system. As a result the Hide DSE has been
score-bit-identical to Phase-0 baseline since ticket 104 landed in late
April 2026: it's a dormancy contract, not a behavior. 105's adrenaline-freeze
modifier gates against the same dormancy.

160's substrate-stub lint flags `HideEligible` as `read-only` (half-wired).
This ticket implements the authoring system that lifts dormancy.

The predicate is spec'd in `src/ai/dses/hide.rs:8-19`:

> `HideEligible` fires iff the cat has a threat in sight AND a low-cover
> tile within sprint range (the "remain still and hope" predator-response
> valence is viable: fleeing is too risky, fighting unwinnable).

## Scope

1. **Implement `update_hide_eligible_markers`** — likely in
   `src/systems/sensing.rs` (consults the threat-perception path) or
   a new module if the surface is large.
   - Predicate: `Has<HasThreatNearby> AND ∃ low-cover tile within sprint range`.
     "Low-cover" is a TerrainKind property — check current overlay /
     terrain-kind decorations for the canonical name.
   - Insert/remove `HideEligible` on per-cat entities tick-by-tick.
2. **Confirm the Hide DSE actually starts firing** in soak runs.
   `Feature::HideFreezeFired` should now record ≥1 hit per soak in
   high-threat scenarios — promote it from rare-event class if it
   becomes frequent enough to be a continuity canary.
3. **Update `src/resources/system_activation.rs`** if `Feature::HideFreezeFired`
   rate changes — re-classify in `expected_to_fire_per_soak()` if needed.
4. **Drop the `HideEligible` allowlist entry** from
   `scripts/substrate_stubs.allowlist`.

## Out of scope

- Tuning the Hide DSE's curves. 104's bounded curve (caps at 0.5)
  and 105's adrenaline-freeze branch are the existing surface — this
  ticket lifts dormancy without retuning.
- Wider sensing-system refactors. If sensing isn't the right host for
  the author function, document the rationale and pick a sibling system,
  but don't rewrite sensing as part of this ticket.

## Current state

- Ticket 104 landed Phase 1 dormancy contract (April 2026).
- Ticket 105 lifted alongside.
- `src/ai/dses/hide.rs:8-19` carries the Phase-2 predicate spec.
- `src/ai/modifier.rs:1429-1453` describes the bit-identical-to-baseline
  invariant that this ticket *intentionally breaks*.

## Verification

1. After landing, `HideEligible` is inserted on per-cat entities under
   the spec'd predicate; soak runs show ≥1 `Feature::HideFreezeFired`
   in high-threat cohorts.
2. `just check` passes after dropping the allowlist entry.
3. `just soak` + `just verdict` — Hide DSE going from dormant to active
   will shift action distribution; treat as balance change. Drift
   on `actions.Hide.fraction` from 0% to non-zero is expected. Drift
   on adjacent actions (Flee, Combat) > ±10% needs hypothesis per
   CLAUDE.md "Verification" section.
4. Focal-cat trace via `just soak-trace` — confirm the L2 Hide DSE
   score lifts above 0 only on cats matching the predicate.

## Log

- 2026-05-05: opened in same commit as 160. Largest of the 160 follow-ons —
  touches sensing + introduces a new behavior into the action distribution.
