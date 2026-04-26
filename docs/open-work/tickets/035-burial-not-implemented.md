---
id: 035
title: Burial — implement the §5 broaden-sideways capability so the continuity canary can pass
status: ready
cluster: null
added: 2026-04-26
parked: null
blocked-by: []
supersedes: []
related-systems: [death.md, project-vision.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

The continuity canary `burial` (`scripts/check_continuity.sh`, target `> 0`) fails on every soak because **burial has no producer in the codebase**. The event variant is defined and the tally is wired:

- `src/resources/event_log.rs:261` — `EventKind::BurialFired { .. }`
- `src/resources/event_log.rs:529-530` — increments `continuity_tallies["burial"]` on emission

But `grep -rn 'EventKind::BurialFired' src/` returns only those two definition sites — nothing constructs the event. The seed-42 soak at `a879f43` had 7 deaths (5 Injury, 2 Starvation) and 0 burials, which is the structurally inevitable outcome of "no system emits the event."

This isn't a regression — it's an unbuilt §5 capability. `docs/systems/project-vision.md §5` lists burial alongside grooming, play, mentoring, courtship, preservation, generational knowledge as the "broaden sideways" axes the colony's behavioral range should cover. Of those, grooming/play/mentoring/courtship have producers and tally non-zero in current soaks; burial alone is dead-wired.

## Scope

1. **Author the burial step resolver.** New `src/steps/disposition/bury.rs` following the GOAP step contract (5-heading rustdoc preamble, `StepOutcome<W>`, witness emission via `record_if_witnessed` with `Feature::BurialPerformed` (Positive) — add the feature variant to `src/resources/system_activation.rs` and classify it).
2. **Add the Bury DSE.** `src/ai/dses/bury.rs` — eligibility marker is "knows of an unburied colony-member corpse within sense range" (sight or scent channel). Composition: a small Maslow-level-3+ pull (belonging — caring for the dead is a community-belonging act) plus a strong proximity term so the cat closest to the corpse wins.
3. **Wire the chain.** Disposition → GOAP plan → step. The plan needs a "move-to-corpse" leg and a "bury" leg with a `bury_ticks` duration constant in `DispositionConstants`. Decide whether burial consumes the corpse (despawn) or marks it `Buried` (kept for memorial / monument tie-ins per ticket 021).
4. **Emit the event.** `EventKind::BurialFired { burier, deceased, tick }` from the resolver's witness path. Triggers continuity tally + can drive a Significant-tier narrative line.
5. **Verify.** Seed-42 soak should produce ≥1 burial (deaths happen reliably). Continuity canary flips from FAIL to pass.

## Non-goals

- Multi-cat funeral rites, gravesite landmarks, grief cascade tuning. All §5 expansion candidates but defer to follow-on tickets — burial-as-a-witnessed-action is the load-bearing minimum to clear the continuity canary and give the world a way to acknowledge death.

## Concordance prediction

A single `BurialFired` per death (or per accessible corpse, capped) ⇒ `continuity_tallies["burial"]` rises from 0 to ~5–10 per 15-min seed-42 soak (matches death count). No expected impact on survival canaries — burial is a post-death action.
