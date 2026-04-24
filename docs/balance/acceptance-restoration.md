# Acceptance restoration — iteration 1 (recipient-side plug-points)

**Status:** landed in the same commit as `mastery-restoration` iteration 1.
Baseline at c49056f (last tuned-42 soak pre-change); treatment with
`acceptance_per_groomed = 0.08`, `acceptance_per_kitten_fed = 0.10`.

## Context

The colony-averaged Maslow chart (seed-42, 15 min soak, commit c49056f)
showed **acceptance pinned at 0.0 across the run**. Static analysis
confirmed the mechanism: `src/systems/needs.rs:131-133` drains
acceptance every tick, but a full-repo grep for any `needs.acceptance +=`
/ `= (... +` found **zero restorers anywhere in the codebase**.
Acceptance is a structural one-way drain.

This matters beyond the chart: `src/systems/colony_score.rs:148-156`
computes colony welfare as an average of five axes, one of which
(`fulfillment`) applies Maslow level-suppression via
`belonging = (social + acceptance) / 2` cascading into level-5
suppression. Pinned-at-0 acceptance halves the belonging term, which
cascade-multiplies through esteem (level 4) into self-actualization
(level 5), dragging welfare down regardless of actual colony health.

## Hypothesis

> Acceptance models the **passive/received** side of social welcome —
> the felt sense of being cared for by the colony. It's distinguishable
> from `social` (active companionship drive) in that acceptance is
> topped up by *receiving* care, not by sending it. The current drain
> has no corresponding receiver-side event, so the need pins to 0. Wiring
> recipient-acceptance gains at the highest-cadence receiver-side events
> (being groomed, being fed as a kitten) lifts colony-averaged acceptance
> to a stable band above 0 on seed-42, without perturbing survival
> canaries.

This treats acceptance as a **fulfillment axis for care received**, not
a proxy for social activity. The asymmetry is load-bearing: groomer
gets `social` + `temperature` from grooming someone; groomee gets
`acceptance` from being groomed. Same behavioral arc, two different
need deltas — matches the ecological-magical-realist framing of
`docs/systems/project-vision.md`.

## Prediction

| Metric | Direction | Rough magnitude |
|---|---|---|
| Colony-averaged acceptance | ↑ from 0.0 | 0.3–0.6 steady band |
| Mood valence (colony) | ↑ slightly | belonging term recovers |
| Welfare composite | ↑ or sideways | amplified cascade unblocks |
| Starvation deaths | unchanged | stays 0 (hard gate) |
| ShadowFoxAmbush deaths | unchanged | stays ≤ 5 (hard gate) |
| Hunt/kill rates | unchanged | no supply-side change |

## Scope boundaries (iteration 1)

### In scope — receiver-side recipient bumps on witnessed effect

Both bumps fire on the same witness gate as the existing deferred
effect in the post-loop consumer. No resolver signatures changed.

1. **`acceptance_per_groomed = 0.08`** — applied to the groom-other
   recipient (the cat being groomed) when the `GroomOther` step
   completes. `src/systems/disposition.rs` post-loop `grooming_restorations`
   consumer. Fires once per `groom_other_duration = 80` ticks per
   completed session.

2. **`acceptance_per_kitten_fed = 0.10`** — applied to the kitten on
   successful `FeedKitten` witness (adult inventory took a food item).
   `src/systems/disposition.rs` post-loop `kitten_feedings` consumer.
   Fires once per witnessed feed event.

### Deferred

- **Apprentice-side acceptance on MentorCat.** Would require widening
  the `unchained_skills` query to `(&mut Skills, &mut Needs)` — a
  signature change that collided with a parallel session's in-progress
  LLVM split refactor of `resolve_disposition_chains`. Reframe once
  that lands: acceptance on mentor-session completion for apprentices,
  same witness as skill transfer.
- **Gossip-subject acceptance.** Diffuse effect, hard to gate on
  witness without new event infrastructure.
- **Gift-receipt acceptance.** Firing frequency unclear on seed-42;
  verify in a follow-up before wiring.

## Observation

Baseline: `logs/tuned-42-baseline-c49056f/` (c49056f pre-change soak).
Treatment: `logs/tuned-42/` (seed-42 --duration 900 --release after
this work landed, commit TBD).

Per-cat late-soak acceptance (~tick 1.35M, 8 cats):

| Cat | Baseline acceptance | Treatment acceptance |
|---|---:|---:|
| Birch, Calcifer, Ivy, Lark, Mallow, Mocha, Nettle, Simba | **0.000** all | **0.000** all |

Survival + never-fired footer:

| Metric | Baseline | Treatment | Result |
|---|---:|---:|---|
| Starvation deaths | 0 | 0 | **pass** |
| ShadowFoxAmbush deaths | 0 | 0 | **pass** |
| Footer written | yes | yes | **pass** |
| `never_fired_expected_positives` | 12 | 12 | **pass (no growth)** |
| Same list of never-fired features | — | — | **same 12** |

Critically: the 12 never-fired list on both runs includes
`Feature::GroomedOther` and `Feature::KittenFed`. **Both of the
witness gates my restorers hang on never fired during the 15-minute
soak on seed-42.** The post-loop `grooming_restorations` and
`kitten_feedings` vecs stayed empty, so neither acceptance bump
triggered.

Secondary symptom in the treatment footer: urgency interrupts
(`CriticalSafety` / `ThreatNearby` preempting active plans) jumped
from 3 in baseline to 34 in treatment — cats are abandoning long
duration steps (like the 80-tick `groom_other_duration`) before the
completion witness lands. The per-tick portion of `resolve_groom_other`
still fires and still lifts the groomer's `temperature`, but the
completion witness is structurally gated behind an uninterrupted
80-tick run.

## Concordance

**Direction:** inconclusive. My hypothesis predicted acceptance ↑; the
observed delta is zero. That's not a direction mismatch because the
underlying mechanism (witness firing) was never exercised.

**Mechanism correction:** the witness gates I chose are not the right
altitude for seed-42's behavior. Actions that *start* and then get
preempted never reach the completion witness, so a restorer tied to
completion is dormant. For iteration 2, consider a per-tick acceptance
accumulator (like the existing `groom_other_social_per_tick` already
does for the groomer's `social`) on the per-tick portion of
`resolve_groom_other` / `resolve_feed_kitten`, or diagnose and unblock
the completion-witness path itself.

**Survival canaries:** hold. Not a regression; the change is a no-op
at the observed metric level, not harmful.

## Iteration 2 — deferred (2026-04-24)

**Status:** new constants added (`acceptance_per_groom_other_per_tick`,
`acceptance_per_feed_kitten_per_tick`, `acceptance_per_mentor_per_tick`,
`acceptance_per_cleanse_per_tick`) but the per-tick recipient hooks are
not yet wired. The seed-42 v2 deep-soak that motivated this iteration
showed acceptance still flatlined at 0.012 mean / 95.3% zero — the
iter-1 receiver-side bumps remain dormant because their parent
actions (Groom 66 snapshots, Mentor 0, Caretake 0) almost never fire.

### Diagnosis

Acceptance restoration is **not the binding constraint**. The receiver-
side hooks are correctly designed; what's missing is the action firing
itself. Action firing is suppressed because `social` need is passively
saturated by `hearth_social_bonus = 0.001/tick` (10× the
`social_base_drain = 0.0001/tick`) plus `bond_proximity_social_rate =
0.0003/tick` from any nearby bonded companion. Cats never need to
actively socialize, so Socializing/Groom/Mentor lose DSE selection,
and the receiver-side acceptance hooks have nothing to fire on.

Per the CLAUDE.md "Balance Methodology" — drift > ±10% on a
characteristic metric requires a hypothesis that ties the cause to
verisimilitude. Wiring iteration 2's per-tick accumulator hooks
without first releasing the saturation lever would produce no
measurable change (the actions still won't fire), so the four-artifact
methodology can't tie out.

### Decision

Defer the per-tick wiring until the **saturation rebalance follow-on
ticket** lands (separate ticket, not yet opened). Once `social`
saturation is broken, Socialize/Groom/Mentor should re-enter DSE
competition; at that point iter-2's per-tick accumulator pattern
becomes meaningful and the receiver-side bumps stop being dormant.

The new constants are checked in with `serde(default = ...)` defaults
so a pre-saturation soak can wire and measure them without a
constants migration.

### Hypothesis (iter 2 — to be re-asserted after saturation rebalance)

> Acceptance flatline is a two-stage problem: (a) receiver-side
> witness hooks were dormant because actions didn't reach completion
> (iter 1 mechanism correction); (b) actions don't reach completion
> because they don't even start — `social` saturation suppresses the
> demand signal. Iteration 2 ships the per-tick mechanism but it
> needs the saturation lever release to take effect. Predicted
> magnitude post-release: 0.05–0.15 colony-mean. Without the release,
> 0 (the dormancy persists at a different witness altitude).

## Related work

- `docs/open-work.md #12` (warmth-split phase 3) — planned `social_warmth`
  axis that would be a sibling/alternative to acceptance for some of
  the same behaviors. This iteration does not touch that path.
- `docs/balance/mastery-restoration.md` — sibling restoration pathway
  for the other pinned-at-0 need (mastery), same structural shape,
  landed in the same commit.
- `docs/systems/colony_score.rs` (implementation, not yet a stub) —
  the cascade-suppression amplifier that made pinned-at-0 needs drag
  welfare down more than they otherwise would have.
