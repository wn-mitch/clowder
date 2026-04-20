# Scoring-layer tuning and second-order dynamics

**Source:** 2026-04-20 Clowder session, after iter-2 diagnostic on
`social_target_range` (see `docs/balance/social-target-range.report.md`
and commits `290a5d9` + `0e564b91`). Captured as a reference for a
concurrent design thread.

## The shape

The specific finding — Mate got starved because wider social range
diluted bond-building — is a symptom of a more general shape worth
naming before more constants get tuned one at a time.

**Scoring-layer tuning doesn't account for second-order dynamics.**

The utility-scoring model is a snapshot function. Given a cat's state
right now, it picks an action. But the state the cat will be in next
tick is a *consequence* of the action it picks now — and the state of
the colony an hour from now is a consequence of the aggregate of those
picks. Bonds, skills, mates, coordinator election, coalition formation,
colony knowledge — all of these are slow-building state that only
exists because a specific kind of interaction repeats on specific pairs
of cats over time.

When we tune a gate or a scale, we're adjusting what wins per-tick. The
second-order effect is what those picks accumulate into over the soak.
Our methodology (hypothesis + prediction + observation + concordance)
was designed for first-order effects — "lift this, that rises." It
caught the iter-1 second-order regression only because the effect was
big enough to show up in 15 minutes of sim. Smaller regressions, or
ones that only manifest over more sim-time, slip through.

## Three framings this unlocks

### 1. The scoring layer has no model of the economy it drives

Mate doesn't know bonds are formed by repeat Socialize interactions.
Socialize doesn't know it's the supply chain feeding Mate. Any tuning
to one action is invisible to the other until observable state (like
`bonds_formed`) drifts. A proper fix gives actions awareness of each
other via shared slow-state — either through the strategist-coordinator
task board already queued in `docs/open-work.md` follow-on #1 sub-3, or
through pair-level stickiness that carries across scoring decisions.

### 2. Instrumentation beat prediction

The iter-1 predictions doc passed every author's-intent check and still
got the mechanism wrong. What actually diagnosed it was adding
`last_scores` distribution capture, then noticing Mate appears zero
times. For the class of problems where one action's supply chain is
another action's output, the default should be instrumenting the
pipeline *before* tuning, not after a rejection.

### 3. The canary set is incomplete

`Starvation == 0`, `ShadowFoxAmbush ≤ 5`, `survives day 180` are all
survival canaries. The continuity canaries (kitten-to-adult,
grooming/play/mentoring/burial/courtship firing, mythic events) are
partially wired. But there are no canaries for the *infrastructure of
continuity* — bond progression, skill transfer, coordinator stability —
even though survival and continuity both rest on those. If
`bonds_formed` had been a hard canary, iter-1 would have been rejected
automatically, not diagnosed manually.

## Where this could go next

- **Infrastructure-of-continuity canaries.** Before more scoring-constant
  tuning, land a few: `bonds_formed` floor, Partners-or-Mates
  progression count, mates-count growth trajectory. Wire into
  `scripts/check_canaries.sh`.
- **A standard library of "why didn't this action happen" queries.**
  Treat the `last_scores` instrumentation as the first example of a
  pattern — gate-open rate, co-occurrence, margin, rank distribution —
  so the next time we suspect scoring competition we start with data,
  not a hypothesis. `scripts/analyze_score_competition.py` is the seed.
- **Re-evaluate the queued follow-ons with this lens.** Pair-stickiness
  and Mentor scale-raise are both local per-action tunes; the
  strategist-coordinator work (follow-on #1 sub-3) externalizes the
  "which goal is active" decision out of the per-tick scoring layer,
  which is exactly the layer that can't see slow state. It may be less
  risky than its scope suggests once you account for second-order
  regressions the local fixes keep surfacing.

## The one-line takeaway

Per-tick scoring is local in time; colony continuity is global in time.
Any balance change whose verisimilitude story runs through bonds,
skills, reputation, or reproduction is a change to a supply chain, not
to a score — and the methodology needs to treat it that way.
