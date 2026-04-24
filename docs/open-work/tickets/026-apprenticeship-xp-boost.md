---
id: 026
title: Apprenticeship XP-boost on per-skill Skills component
status: blocked
cluster: null
added: 2026-04-24
parked: null
blocked-by: ["mastery-restoration iteration 2"]
supersedes: []
related-systems: [aspirations.md, crafting.md]
related-balance: [mastery-restoration.md]
landed-at: null
landed-on: null
---

## Why

Opened via `/rank-sim-idea` triage on 2026-04-24 while designing the
upper-need fillers. Score: **V=4 F=5 R=4 C=4 H=4 = 1280** — "cheap
win" bucket.

The existing `mentor_cat.rs:49` grows the *mentor's* mastery on
apprentice-teaching ticks, and `combat.rs:413` is the only site that
actually writes any value into the per-skill `Skills` component today.
That means apprentices gain neither mastery (they're not the mentor)
nor per-skill XP (nothing writes skills). A mentor-apprentice
relationship that doesn't accelerate the apprentice's actual
capability defeats the generational-knowledge axis of §5 sideways-
broadening.

## Design sketch

Extend `resolve_mentor` / `mentor_cat.rs` to write the apprentice's
weakest teachable skill every tick (or every N ticks) at a rate
scaled by the mentor-apprentice relationship strength. Pair with
mastery-restoration iter 2 (which restores the *need* for felt
competence); apprenticeship pulls up the *capability* so the need has
somewhere to recognize growth.

Load-bearing: the Skills write must land on the apprentice's
component (not the mentor's), which requires signature widening on
`unchained_skills: Query<&mut Skills, ...>` in the mentor effect
application block in `disposition.rs:~2840`. This was the same
signature collision that blocked iter-1 of acceptance-restoration;
mastery-restoration iter 2 landed without widening the resolver
signatures, so this ticket is the first one that actually has to do
the widening.

## Scoring justifications

- **V=4**: Lights generational-knowledge axis of §5; partial
  continuity-canary improvement (mentoring fires but produces no
  measurable skill differentiation today). Not 5 because mentoring is
  already counted as a canary event — this ticket deepens rather
  than creates.
- **F=5**: "High-mastery cats teach low-mastery cats" is the
  generational-knowledge thesis verbatim from
  `docs/systems/project-vision.md` §5.
- **R=4**: Extends existing `mentor_cat.rs:49` mentor-side hook;
  regression scope bounded to one resolver. Not 5 because it
  touches `Skills` which `combat.rs:413` already partially writes,
  and any write to `Skills` has second-order effects on DSE scoring.
- **C=4**: ~300–500 LOC. New per-tick apprentice-XP application in
  mentor resolver, plus tuning constants in
  `sim_constants.rs::disposition`. No new components; reuses
  existing `Mentor` relationship + `Skills`. Widening
  `unchained_skills` query is the main structural change.
- **H=4**: One or two new constants
  (`apprentice_xp_per_tick`, maybe `apprentice_xp_bond_scale`); no
  new canary required; doesn't destabilize existing axes. Isolated
  extension — H-source: structural tells all negative (no
  probabilistic rare-event; no feedback loop; no bespoke canary
  needed; tunables not cross-coupled).

Shadowfox comparison: zero structural tells fire. This is the
antithesis of a shadowfox-class addition.

## Dependency

Blocked on `docs/balance/mastery-restoration.md` iteration 2 landing
first so that the apprentice's per-skill XP growth feeds an
already-working mastery need pulse rather than a flatlined one. Once
iter 2 observation lands with the predicted 0.3–0.5 band, this ticket
is ready.

## Recommendation

"Pick up next session" — after mastery-restoration iter 2's
post-soak concordance confirms direction + magnitude.
