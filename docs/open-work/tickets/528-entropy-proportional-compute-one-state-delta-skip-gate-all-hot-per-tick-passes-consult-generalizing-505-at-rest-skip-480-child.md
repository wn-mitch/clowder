---
id: 528
title: Entropy-proportional compute — one state-delta skip-gate all hot per-tick passes consult, generalizing 505 at-rest skip (480 child)
status: ready
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-07-09
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why
Ticket [505](../landed/505-integrate-beliefs-pass-b-decays-every-mental-model-facet-every-stagger-even-at-fixed-point-at-rest-skip-141-percent-self-at-post-504-flamegraph-480-child.md)
reclaimed 14.1% self CPU with a single observation: `integrate_beliefs` Pass B
was decaying every mental-model facet every stagger *even when the facet had
reached its fixed point* — re-deriving zero change. The fix was an at-rest skip.
Framed information-theoretically (the atlas Information-flow page), a cat sitting
at a fixed point carries ~0 bits of new state per tick, yet the hot passes spend
full cycles re-encoding it. 505 solved this for *one* pass. But the same
zero-entropy waste recurs across the hot-frame catalog: `passive_familiarity` /
`track_sustained_copresence` re-walk pairs whose relationship didn't move
(485/504/506 were all re-knives chasing exactly this), scoring re-evaluates DSEs
for cats whose relevant markers didn't change. Each got a *bespoke* fix. The
substrate-over-hacks pillar says: build the "did this cat's relevant state change
since last eval?" gate **once**, as one queryable signal all hot passes consult,
instead of re-deriving at-rest detection per system. This is the *skip*-scoped
cousin of a full incremental-view-maintenance layer — deliberately narrower
(skip stale work; do NOT try to incrementally *maintain* a materialized result),
because skip is behavior-preserving by construction while maintenance carries the
full determinism blast-radius that made 431 Stage B such a saga.

## Scope
- A per-cat **state-generation counter** (or dirty-set) — one substrate resource
  that increments a cat's generation when any hot-pass-relevant input changes
  (position via `CatMoved`, relationships via `RelationshipChanged`, the marker
  set, needs crossing a threshold). The set of tracked inputs is explicit and
  documented, not "everything."
- A **skip-gate API**: a hot pass records the generation it last processed a cat
  at; on the next tick it consults the counter and *skips* the cat when the
  generation is unchanged AND the pass is at its fixed point (505's condition,
  generalized).
- **Migrate 505's at-rest skip** onto the shared gate (proves the substrate
  subsumes the hand-rolled case without regressing it), then wire ≥1 further hot
  pass (`track_sustained_copresence` or scoring) as the second consumer.
- A **debug-only invariant assertion** (the 431 pattern): under
  `#[cfg(debug_assertions)]`, run the pass unconditionally and assert the skipped
  result equals the recomputed result every tick — this is what makes the skip
  provably behavior-preserving.

## Out of scope
- **Incremental maintenance** (recompute-avoidance by *updating* a cached result
  rather than *skipping*). That is the higher-blast-radius substrate; if it's ever
  wanted it is a separate ticket, and this skip-gate is a prerequisite building
  block, not a step toward it.
- **Approximation / LOD** — spending *fewer* cycles on a cat whose state DID change
  (coarse scoring for distant/off-screen cats). That is behavior-*changing* (a
  balance question) and — per the atlas Mythic page — likely conflicts with the
  "honest ecology, a world with its own weight" design pillar. Explicitly not this
  ticket.
- **The budget/priority scheduler** (deferring discretionary passes under a
  tick-time ceiling) — a different atlas page (Economic); separate ticket.

## Current state
Opened 2026-07-09 from an `/ideate` pass. Priced #1 of the atlas draw's new
candidates: it generalizes a proven win (505), stays behavior-preserving because
it only ever *skips* provably-stale work, and lands the substrate the
substrate-over-hacks pillar keeps asking for — without the determinism risk of full
IVM. Substrate already present to build on: `CatMoved` (431 Stage A),
`RelationshipChanged` (431 Stage D), `MarkerSnapshot`. Nothing built yet.

## Approach
1. Add the generation-counter resource keyed by `Entity`; increment on the tracked
   change-messages (reuse existing `CatMoved` / `RelationshipChanged` readers — do
   not add new per-tick writers; increment is event-driven per
   `project_per_tick_discipline_default_event_driven`).
2. Provide `SkipGate::should_skip(cat, last_gen, at_fixed_point) -> bool` and a
   per-pass `BTreeMap<Entity, u64>` of last-processed generations (BTreeMap for
   deterministic iteration order — the 431 trap).
3. Re-express 505's `integrate_beliefs` at-rest condition through the gate; confirm
   byte-identical `_footer` vs the current 505 binary (this is a refactor of an
   already-landed skip — determinism must hold exactly).
4. Wire the second consumer; flamegraph before/after per
   `feedback_perf_refactor_needs_flamegraph`.
5. Guard every consumer with the debug-only recompute-and-assert invariant.

## Verification
- `just soak-trace 42 <focal>` byte-identical `_footer` before/after (behavior-
  preserving — this is the hard gate for a perf refactor).
- Flamegraph confirms the targeted pass's self-% dropped; `just verdict logs/tuned-42`
  holds all survival + continuity canaries.
- Debug-assertion soak (≥2520 ticks) runs clean — skipped result ≡ recomputed
  result for every consumer.
- `just perf-fence` (if [527](527-perf-regression-land-time-fence-per-system-self-percent-budget-gate-so-per-tick-cost-cannot-creep-invisibly-480-meta-child.md)
  has landed) shows the budgeted ceilings improve, not regress.
- `just check && just test`.

## Log
- 2026-07-09: opened from `/ideate` (atlas Information-flow page). Skip-scoped
  generalization of 505; deliberately excludes incremental-maintenance and LOD to
  stay behavior-preserving. Paired with 527 as the low-risk perf track.
- 2026-07-09: RC flamegraph evidence (logs/flamegraphs/42-1d28ff6e54af, v0.4.0 release gate): `try_detect_cat` is now the #1 sim knife — 24.7% self / 25.6% inclusive of the whole profile (was 8.0% inclusive at the 2026-06-09 table), top child `equipment_effects::equipment_modifiers_for` (the 477 per-pair cloak/noise reads). Idle/grazing prey scanning every cat every tick is precisely the at-rest-skip shape this ticket generalizes; prey detection is a strong first candidate.
