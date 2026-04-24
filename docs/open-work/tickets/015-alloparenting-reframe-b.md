---
id: 015
title: Alloparenting Reframe B — mama drops kitten at hearth near resting elder
status: parked
cluster: null
added: 2026-04-22
parked: 2026-04-22
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

**Deferred after Reframe A landed (Phase 4c.4, 2026-04-22).**
`KittenFed` is no longer zero (55 / 10 on seed-42 v3 soaks) — the
any-adult-feeds-any-hungry-kitten pattern lit up on the back of
(1) bond-weighted compassion, (2) the GOAP Caretake retrieve
step, and (3) target-entity persistence. See Landed Phase 4c.4
entry for the full bundle.

The canary Reframe B was meant to unblock — generational
continuity, a kitten reaching Juvenile in a soak — **is still
zero**, but the A-vs-B diagnosis in Reframe A's hypothesis has
shifted: more adults feeding kittens is no longer the bottleneck,
since A established that dozens of feedings happen per soak. The
gap between "kittens are fed" and "kittens reach Juvenile" is
downstream of feeding frequency — either growth-rate tuning or
the Phase 4c.3 literature anchor's milk-yield / nursing-quality
model. Adding an elder-hearth handoff mechanic doesn't help the
current bottleneck; it would add a communal-care texture without
shifting the canary.

**Resume when:** growth-tuning / milk-yield follow-ons have been
attempted and KittenMatured is still blocked on "no adult is
available to feed." Then B-elder's hypothesis becomes live again
— handoff-to-elder unlocks mother-mortality relief specifically.
Until then, defer.

**Originally specified shape preserved below for when it resumes.**

**Shape (~200–400 LOC):**

1. **Mama-side DSE extension.** Add a sub-mode to Caretake scoring
   (`src/ai/dses/caretake.rs`) that activates when mama has competing
   Action-level needs (Eat + Mate + Sleep debts ≥ some threshold)
   AND an eligible resting-elder is detectable near ColonyCenter.
   Sub-mode resolves to a GOAP plan: `MoveTo(hearth) +
   SettleKittens(near_elder) + release Caretake pressure`. Kittens
   follow via existing group-movement pathfinding (verify it exists;
   otherwise add a `FollowingMother` component that steers).
2. **Elder-side scoring boost.** Elders in Resting disposition gain
   a `near_kitten_at_hearth` urgency boost in their Caretake
   scoring. Reads the existing `resolve_caretake()` signal
   (`src/ai/caretake_targeting.rs` — post-4c.3). Elder doesn't
   actively pick up the handoff role; their existing Caretake
   scoring just gets pulled higher when a kitten is spatially
   present while they're resting at the hearth.
3. **Eligibility query.** Helper predicate:
   `find_resting_elder_at_hearth(&cats_query, &colony_center) ->
   Option<Entity>`. Three-line query over `With<Elder>`,
   `DispositionKind::Resting`, distance-to-ColonyCenter ≤
   `hearth_effect_radius`.
4. **Narrative emission.** Wire a narrative template for the
   hand-off event. `src/resources/narrative_templates.rs` already
   has a `"communal"` template under `Independence`; repurpose or
   add `ElderBabysit` tier.
5. **Continuity canary telemetry.** Add
   `continuity_tallies.elder_babysat_session` counter to the
   event-log footer (`src/resources/event_log.rs`). Same shape as
   grooming/play tallies — not a hard gate, just visibility.

**Hypothesis (if proceeding):**

> Post-Phase 4c.4 adults actively feed kittens (55/10 fed/soak)
> but no kitten reaches Juvenile yet. If the residual bottleneck
> is "mother's own Eat / Mate / Sleep debts drag her away from
> nursing when a non-mother alloparent isn't within feeding
> range", an elder-hearth handoff raises `KittenMatured` from 0
> to ≥1 per soak without regressing KittenFed or Starvation.

**Why not the full scruff-carry version.** That version scored
**675** (adds an inter-cat transport primitive to the codebase).
The physical-causality thesis favours it aesthetically — cats
carry kittens the way cats carry anything, by explicit effort — but
the carry primitive is new architecture with no current precedent
and no second use case on the near-term roadmap. If a second
carry-a-living-entity feature surfaces (corpse-handling per
`docs/systems/corpse-handling.md` is aspirational; wounded-retrieval
isn't stubbed), revisit the full version then. Until then, B-elder
pays for the ecological outcome without the architectural debt.

**Cross-reference:** `docs/systems-backlog-ranking.md` does not yet
carry an alloparenting entry. If B lands, file a stub at
`docs/systems/alloparenting.md` and add the ranked entry at the
same time.
