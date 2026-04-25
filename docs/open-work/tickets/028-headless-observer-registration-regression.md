---
id: 028
title: Headless build silently drops 4 personality-event observer cascades
status: ready
cluster: null
added: 2026-04-25
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Headless mode (the only path that produces dataset JSONL) silently
drops every personality-event cascade observer. The interactive
`SimulationPlugin::build()` registers four observers via
`systems::personality_events::register_observers(app)`
(`src/plugins/simulation.rs:21`); the headless `build_schedule`
in `src/main.rs` is a manual mirror per CLAUDE.md and the
registration was **never added** to it (per git history,
`register_observers` has zero hits in `src/main.rs` across all
commits).

Result: in headless, the trigger system `emit_personality_events`
runs each tick and calls `commands.trigger(...)`, but no observer is
listening. The events are dropped; no cascade fires. The bug is
silent — no error, no warning, just behaviorally invisible cats.

Affected cascades:

- `on_play_initiated` — playful socializing cats trigger
  `PlayInitiated`. Cascade should: boost mood of nearby cats,
  emit play-template narrative, **and** push
  `EventKind::PlayFired` into the event log so the `play`
  continuity canary tallies. None of this fires in headless.
- `on_temper_flared` — high-temper cat with bad mood + unmet
  needs triggers `TemperFlared`. Cascade should: fondness penalty
  on nearest cat, mood hit on target, narrative entry. Silent.
- `on_directive_refused` — stubborn cat with active directive
  triggers `DirectiveRefused`. Cascade should: relationship +
  narrative effects on the coordinator. Silent.
- `on_pride_crisis` — proud cat with low respect triggers
  `PrideCrisis`. Cascade should: narrative entry. Silent in
  headless. (The "presents it with visible pride" narratives
  visible in the dataset come from a *different* hunt-deliver
  path, not from this observer.)

This is the textbook headless-mirror-drift CLAUDE.md flags as a
recurring failure mode:

> `build_schedule()` in `src/main.rs` is a manual mirror of
> `SimulationPlugin::build()`. Change one, change both — they
> diverged silently before.

## Scope

### Fix 1 — register the observers in headless

`pub fn register_observers(app: &mut bevy::prelude::App)` takes an
`&mut App`, but the headless path uses raw `Schedule` + `World`
(no App). Two routes:

- **Preferred:** add a parallel
  `register_observers_world(world: &mut World)` in
  `personality_events.rs` that calls `world.add_observer(...)` on
  each handler, and invoke it from `setup_world` in `main.rs`. Keep
  the existing app-flavored `register_observers` for the plugin so
  the interactive build path is untouched.
- **Alternative:** refactor the single function to take `&mut World`
  and call it from both sites. This forces the plugin path to do an
  extra `app.world_mut()` access but reduces duplication.

### Fix 2 — push `EventKind::PlayFired` from `on_play_initiated`

Even with Fix 1, the `play` continuity canary will not tally,
because `on_play_initiated` only emits a narrative entry and a mood
modifier — it never calls
`event_log.push(time.tick, EventKind::PlayFired { ... })`.
Compare: `goap.rs:2602` (`GroomingFired`), `goap.rs:2825`
(`MentoringFired`), `event_log.rs:516` (the canary tally consumer
for `PlayFired`).

Add an `Option<ResMut<EventLog>>` parameter to `on_play_initiated`
and push a `PlayFired { cat, partner }` record per fired event.
Add the corresponding `EventKind::PlayFired` variant if it isn't
already present (it is — `event_log.rs:266`).

### Fix 3 — same for the other three cascades

Once Fix 1 is in place, audit the other three cascades for similar
canary/event-log gaps:

- `on_temper_flared` — should this push a `TemperFlaredFired` or
  similar? Currently emits narrative only.
- `on_directive_refused` — same question.
- `on_pride_crisis` — same question.

Whether they should push event-log records is a design call (they
may not need a continuity-canary class). At minimum, a session note
for each on what's intentional vs. missing.

### Fix 4 — add a headless-mirror-drift regression guard

The CLAUDE.md warning is a process control. Add a unit test or
shellcheck that scans both `register_observers` callsites and
fails if `src/main.rs` doesn't reference `register_observers`
(or its world-flavored equivalent). Cheap insurance against the
next time this drifts.

**Note on durability:** Fix 4 is interim — the structural fix is
[ticket 030](030-unify-headless-and-windowed-build-pipeline.md),
which replaces the manual-mirror architecture with a unified
plugin pipeline so this regression class disappears. Fix 4 buys
time until ticket 030 lands; once that ships, retire the regression
guard.

## Out of scope

- Designing a Play DSE / `DispositionKind::Playing` variant. The
  `PlayInitiated` event is intentionally *side-effectual* — it
  fires *while* a cat is socializing, not as its own disposition.
  Whether the §11 `Recreation & Grooming` system (rank 2 / score
  900 in `docs/systems-backlog-ranking.md`) should later promote
  Play to a first-class DSE is a separate design question
  unblocked by this ticket.
- Tuning the play trigger gates. Current trigger condition is
  `current.action == Action::Socialize && playfulness > 0.6 &&
  mood.valence > 0.0`, with `playfulness * 0.1` per-tick chance.
  In the dataset Socialize is 1.84% of CatSnapshot ticks, so even
  with the observer registered the firing rate is bounded. Tune
  later once the firing rate is observable.

## Current state

Diagnosed from `logs/baseline-2026-04-25/REPORT.md`:

- `continuity_tallies.play` = 0 across all 15 sweep runs.
- Narrative search: zero hits for any play-template phrasing
  (`"pinecone"`, `"thistlehead"`, `"chases their own tail"`, …)
  across all 15 narrative.jsonl files.
- 1.84% of CatSnapshots have `current_action == Socialize`
  (2,117 / 115,189). 50% of cats have `playfulness > 0.6`.
  Combined per-tick PlayInitiated trigger probability ≈ 0.03%,
  yielding ~25 expected events per 8-cat 900s soak — none of
  which fire.
- `git log --all -S register_observers -- src/main.rs` returns
  zero commits.

## Approach

Fix 1 → Fix 2 in one commit (smallest viable patch). Fix 3 + Fix 4
in a follow-on commit. Verification via the next baseline-dataset
run.

## Verification

After the patch:

1. `just baseline-dataset 2026-04-26-observers` — produces a fresh
   archive at the post-patch HEAD.
2. Expect `continuity_tallies.play > 0` in ≥ 80% of sweep runs.
3. Narrative search should now find play-template phrases in most
   runs.
4. Run the interactive build (`just run`) once and confirm play
   narrative still fires there — the plugin path should be
   untouched.

## Log

- 2026-04-25: Ticket opened from baseline-dataset diagnostic.
  User noted "play definitely had an implementation and regressed";
  investigation traced the gap to headless missing observer
  registration since the observer pattern was introduced.
