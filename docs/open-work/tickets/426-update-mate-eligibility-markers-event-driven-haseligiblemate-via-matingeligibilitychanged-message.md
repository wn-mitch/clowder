---
id: 426
title: update_mate_eligibility_markers event-driven HasEligibleMate via MatingEligibilityChanged Message
status: ready
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
added: 2026-05-19
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Sibling perf hotspot to 423 / 205 / 425, surfaced by the 2026-05-19
O(N²) population-scaling survey. `src/ai/mating.rs::
update_mate_eligibility_markers` (lines 235-278) calls
`has_eligible_mate(...)` for every living cat each tick. The predicate
internally iterates `cat_positions: &[(Entity, Position)]` with
`.any()`, filtering by Partners/Mates bond + fertility + sated + happy
+ orientation. That's **O(N²) per tick**, recomputed exhaustively even
though the underlying preconditions (bond type, fertility phase,
seasonal fertility window) change rarely.

This site is the **strongest event-driven fit** of the O(N²) survey:
mating eligibility transitions on discrete events (bond change, season
change, fertility phase change, cat death). Per-tick rebuild is the
wrong shape — most ticks have zero eligibility changes, but we still do
N² work. The user's 2026-05-19 reframe ("some of these probably work
better as rust events that update the marker when it transitions a
boundary rather than a per tick update cadence") names this exact case.

## Scope

**New Message** — `src/messages/mating_eligibility.rs`:

```rust
#[derive(Message, Debug, Clone, Copy)]
pub struct MatingEligibilityChanged {
    pub entity: Entity,
}
```

Register in `SimulationPlugin::build` alongside the existing
`add_message::<>` block at `src/plugins/simulation.rs:517-550`.

**Emitter sites** — emit `MatingEligibilityChanged` from each precondition
mutation point. Locate via `rg '\.bond\s*=' src/` and the fertility/
season tick paths:

1. **Bond mutation** — sites that set `bond = Some(BondType::Partners |
   Mates)` or `bond = None` for an already-Partners/Mates pair:
   `src/ai/mating.rs:326, 699` (Partners), `src/components/joint_intention.rs:776, 796`
   (None / Mates), `src/systems/death.rs:538` (Mates), `src/systems/social.rs:307`
   (generic bond mutation — gate by checking new vs old value).
   Emit for **both** cats in the pair.
2. **Season transition** — find the season-tick site (`src/systems/time.rs::advance_time`
   or sibling). On `current_season` transition, emit for every living
   adult cat (one-shot batch). The season-fertility table changes per
   season, so the eligibility gate changes for everyone simultaneously.
3. **Fertility phase transition** — in `src/systems/fertility.rs`, emit
   when a cat's `FertilityPhase` flips (Estrus ↔ Diestrus ↔ Anestrus ↔
   Postpartum). One emit per affected cat.
4. **Cat death** — on a fresh `CatDied`-equivalent lifecycle event, emit
   `MatingEligibilityChanged` for the deceased's Partners/Mates so
   their markers re-evaluate without the dead bondee.
5. **Periodic full rescan** — every N=60 ticks (~1 game-minute) emit for
   every living cat. Catches mood + mating-need threshold crossings that
   v1 doesn't emit individually. The 60-tick cadence bounds staleness to
   one game-minute, well below the L3 decision horizon. Implement as
   `if time.tick % 60 == 0 { ... }`.

**New author** — `update_mate_eligibility_markers` rewritten to consume
`MessageReader<MatingEligibilityChanged>` + a `Local<bool> startup_done`:

```rust
pub fn update_mate_eligibility_markers(
    mut commands: Commands,
    mut messages: MessageReader<MatingEligibilityChanged>,
    mating: MatingFitnessParams,
    relationships: Res<Relationships>,
    constants: Res<SimConstants>,
    cats: Query<(Entity, &Needs, Has<HasEligibleMate>),
                (With<Species>, Without<Dead>)>,
    mut startup_done: Local<bool>,
) {
    let pending: HashSet<Entity> = if !*startup_done {
        *startup_done = true;
        cats.iter().map(|(e, _, _)| e).collect()  // full rescan on cold-start
    } else {
        messages.read().map(|m| m.entity).collect()
    };
    if pending.is_empty() { return; }
    // ... re-evaluate only pending cats via has_eligible_mate ...
}
```

**Tighten** `has_eligible_mate`'s internals to iterate Partners/Mates
relationships **first** (the K ≤ 5 short list from
`relationships.all_for(focal)`) instead of `cat_positions`. Even when
invoked, the per-call inner cost drops from O(N) to O(K). The outer
"iterate all cats" loop is now driven by Message volume, not population
size.

## Out of scope

- Tuning mating thresholds (`mating_interest_threshold`,
  `breeding_mood_floor`, season fertility curves) — owned by the mating
  balance thread.
- Bond mutation Message infrastructure beyond this ticket — if multiple
  consumers want bond-change events, generalize to a sibling
  `BondTypeChanged` Message in a follow-on. This ticket emits the
  coarser `MatingEligibilityChanged` because that's what the consumer
  needs.
- Cat-spatial-index work (ticket 205) — mating doesn't need spatial
  queries; partners are known by relationship, not proximity.

## Current state

Ticket independent of 205 — can land in any session. No blockers.

## Approach

1. Audit existing bond-mutation sites (5+ across joint_intention.rs,
   mating.rs, death.rs, social.rs). Each becomes an emit point. Sketch
   the emit site list in the layer-walk before coding.
2. Author the Message + register it. Wire the periodic 60-tick emitter
   first (covers mood/need crossings even if discrete emitters are
   incomplete).
3. Tighten `has_eligible_mate` to relationships-first iteration. This
   alone gives ~10× win at typical K=3-5 partners; the Message
   plumbing on top gets us to ~60× amortized.
4. Rewrite the author to consume `MessageReader` + cold-start full
   rescan via `Local<bool>`.
5. Run a startup sanity check: at tick 0, the cold-start rescan should
   evaluate every cat. After that, steady-state ticks evaluate only
   cats whose messages fire.

## Verification

- `just check` passes (existing Message registration + author wiring).
- **Unit test:** emit a synthetic `MatingEligibilityChanged` for a known
  cat with Partners + fertile state; assert the marker toggles on.
- `just soak-trace 42 Simba` → `just verdict`. `MatingOccurred` count
  unchanged from the pre-426 baseline (semantic preservation — the
  marker still flips at the same moments, just lazily).
- `just frame-diff <pre> <post> trace-Simba.jsonl` — `mate_target` DSE
  row mean-score delta ≈ 0.
- **Cost shape check:** instrument or grep events.jsonl for emit count;
  steady-state ticks should fire 0 emits, transition ticks fire 1-N. At
  most ~20N evaluations per game-day vs current 1200N — ~60× reduction.

## Log

- 2026-05-19: opened mid-session as a follow-on to 423's perf survey.
  Best event-driven fit of the four O(N²) survey hotspots.
