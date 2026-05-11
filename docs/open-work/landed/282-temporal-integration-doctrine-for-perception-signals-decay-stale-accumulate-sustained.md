---
id: 282
title: Temporal-integration doctrine for perception signals — decay stale, accumulate sustained
status: done
cluster: ai-substrate
added: 2026-05-11
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-11
---

## Why

Surfaced during a perception-accuracy audit of `logs/tuned-42` for
ticket 273. Four separate detection tickets in flight (219 RecentAmbushMap,
234 damage_recency, 243/244 audible cue substrate, 283 fox-scent split)
are each re-deriving the same design question ad hoc: should the
perception scalar be a point-in-time read of current world state, or
should it integrate point events over time (decay stale signals,
accumulate sustained ones)? Each ticket's answer to that question
ends up similar in shape but slightly different in timescale,
authoring side (source vs perceiver), and decay function. This
ticket lands a **shared doctrine** that those four tickets can cite,
so the answer is consistent and the work of re-deriving it lives
in one place instead of four. No code change in this ticket — the
deliverable is a short design note that the instance tickets then
implement against.

## Scope

- Short design note. Either a new section in
  `docs/systems/ai-substrate-refactor.md` or its own file
  `docs/systems/perception-temporal-integration.md` — author's
  choice; the substrate-refactor doc already has a §4-style
  taxonomy that this would fit into.
- Names the load-bearing rule: **perception scalars driving safety
  / urgency DSEs must integrate over time, not sample point
  events**. Stale signals decay (threat memory, fox scent for
  short-timescale consumers, damage). Sustained signals accumulate
  (kitten cry, distress). Point sampling is reserved for state that
  IS instantaneous (own hunger, own energy, colony food stores).
- Names the four current detection channels that need it:
  ThreatSeen memory (no decay today) · fox scent for Patrol-class
  consumers (10-day half-life is wrong timescale) · kitten cry (no
  onset/offset smoothing, no duration weighting) · damage events
  (no `damage_recency` scalar exists).
- Names the design decisions each instance ticket must make:
  (a) authoring side — source-emits-pre-integrated vs perceiver-
  integrates-on-read; (b) decay function — exponential, linear,
  piecewise; (c) timescale — hours, days, or never (for genuinely
  persistent state); (d) interaction with existing point-sample
  consumers if the channel has more than one reader.
- Cross-references 219, 234, 243, 244, 283 as instances; 256 as the
  worked-around precedent that motivates the doctrine.

## Out of scope

- Implementing any of the four channel fixes. Each lives in its
  own ticket (219, 234, 243/244, 283).
- Retroactively tuning consumers that intentionally want point-
  sample semantics (e.g. cat's own `hunger_urgency` reads the
  current Needs value because that IS the ground truth, with no
  meaningful concept of "stale hunger").
- Memory-system mechanics (per-cat episodic memory has its own
  ticket lineage — see 207 / 258).

## Current state

Audit at `.claude/plans/let-s-work-273-dig-enchanted-wirth.md`
(2026-05-11) documents the four-channel pattern with citations:
- `src/ai/scoring.rs:1928` — `memory_proximity_sums()` (ThreatSeen
  with no decay)
- `src/systems/wildlife.rs:2383` and `src/resources/sim_constants.rs:4240-4247`
  (10-day scent half-life as territorial mark)
- `src/systems/growth.rs:162` (`KittenCryMap.clear()` per tick)
- (no file — `damage_recency` is absent)

The detection tickets exist independently; none cites the others.
This ticket is the missing rubric.

## Approach

Write the doctrine as a short note (~1 page). Structure:
1. Statement of the rule.
2. The point-sample antipattern (one concrete example: ThreatSeen-
   no-decay → permanent safety-deficit → patrol cascade).
3. The two integration shapes:
   - **Decay** (stale signals): exponential or linear half-life;
     applied at authoring time (source) for shared signals or at
     read time (perceiver) for per-cat memory.
   - **Accumulation** (sustained signals): integrate consecutive
     ticks of source-active state into a strength that ramps over
     duration; reset / decay when source goes inactive.
4. Authoring-side guidance: prefer source-side integration for
   colony-shared signals (one author, all readers see the same
   integrated value); per-cat integration only when the signal is
   genuinely per-cat (episodic memory, individual fatigue).
5. Channel checklist — each of the four detection channels gets a
   one-line entry naming the shape (decay vs accumulate), timescale,
   and authoring side.

## Verification

Doctrine ticket — no behavior change to test. Done when:
- The note lands in `docs/systems/`.
- Tickets 219, 234, 243, 244, 283 each get a `related-systems:`
  entry pointing at the doctrine file.
- `docs/wiki/systems.md` reflects the new doc if it's a standalone
  file.

## Log
- 2026-05-11: opened. Surfaced by the perception-accuracy audit
  for ticket 273. The audit identified four detection channels
  that each need temporal integration with slightly different
  shapes; this ticket lands the shared rubric so the instance
  tickets cite a common rule instead of re-deriving it.
- 2026-05-11: 2026-05-11: landed. Doctrine lives at docs/systems/ai-substrate-refactor.md §4.5.1; covers both temporal integration (219/234/244/283) and spatial range-summation (243).
