# Open work

> **What this is:** the cross-thread index of open work. New sessions should
> consult this, `docs/wiki/systems.md`, and `docs/balance/*.md` before starting
> fresh. See `CLAUDE.md` §"Long-horizon coordination" for the request-time
> checklist and maintenance rules.

Living backlog of known-but-not-scoped work. Each entry is a pointer, not a
plan — the plan is written when the work is picked up.

---

## Uncommitted from the 2026-04-19 session

### Balance change: `eat_from_inventory_threshold: 0.05 → 0.4`

**Status:** validated, uncommitted. Commit strategy not chosen.

**Files:**
- `src/resources/sim_constants.rs:128` (one-line change)
- `docs/balance/eat-inventory-threshold.predictions.json` (new)
- `docs/balance/eat-inventory-threshold.report.md` (new)
- `scripts/analyze_eat_threshold.py` (new)

**Seed-42 15-min soak result:** starvation 2 → 1, below-0.3 hunger 1.06% →
0.50%, stores mean 85% → 92%, leisure action-time +18%, colony survives +2
sim-weeks. Canaries pass under the plan's "≤ baseline" criterion (the
absolute `check_canaries.sh` bar of `Starvation == 0` still fails at 1, but
the baseline already failed it at 2 — pre-existing).

**Archived baselines:**
- `logs/tuned-42-archive-apr17/` — pre-session snapshot
- `logs/tuned-42-baseline-eat-threshold/` — this session's baseline
- `logs/tuned-42/` — this session's treatment

**Commit options left on the table:** (A) `jj split` to isolate just these
files into their own commit, (B) bundle with the prior plan's docs reframe
(CLAUDE.md + `docs/systems/project-vision.md`) as two commits, (C) hand it
off, (D) bulk wip. No decision made.

### Docs reframe from the prior plan

Also uncommitted:
- `CLAUDE.md` — opening replaced with "A clowder of cats, living in a world
  with its own weight"; adds Systems inventory section; fixes
  `src/main.rs:346` line reference; adds continuity canaries.
- `docs/systems/project-vision.md` — new; full thesis + influences +
  design corollaries.

Plan-approved and edited to disk. Same commit-strategy question as above.

---

## Pre-existing issues (not from this session)

### Test harness drift

**Status:** pre-existing.

`cargo test` fails three integration tests with a Bevy "Resource does not
exist" panic:
- `cats_eat_when_hungry`
- `simulation_is_deterministic`
- `simulation_runs_1000_ticks_without_panic`

Reverting the 2026-04-19 balance change does not fix them — a system was
added to `build_schedule()` (in `src/main.rs` or `SimulationPlugin::build()`)
whose required Resource isn't inserted in `tests/integration.rs::setup_world`.

**`just check` (cargo check + clippy) passes green.** Only `cargo test` is
broken.

**To pick up:** enable a debug feature (or patch a local build of bevy_ecs)
to surface the actual system name and missing-Resource type, then add the
insertion to `setup_world`.

---

## Follow-on plans surfaced but not scoped

### 1. Explore dominance over targeted leisure

**Why it matters:** Explore claims 44–47% of all action-time in a seed-42
soak. Groom sits at 0.4–0.5%, Mentor / Caretake / Cook at exactly 0. The
user's "narrative leisure isn't happening" observation is real but it's a
target-availability problem, not a survival-lock problem.

**Root cause:** Explore has the loosest gate (just `unexplored_nearby > 0`).
Other leisure actions require specific targets (`has_social_target`,
apprentice, kitten, Kitchen, mate) that aren't consistently present.
Choosing Explore moves cats toward unexplored periphery → away from other
cats → `has_social_target` turns false → Explore wins again. Dispersion
feedback loop.

**Three directions agreed in the 2026-04-19 session** (ordered by blast
radius):

1. **Broaden `social_target_range`** (`src/resources/sim_constants.rs:1672`)
   from 10 → ~20–30 Manhattan tiles. Current 10 is combat-adjacent range,
   not cat-socializing range. In a 120×90 map with 8 cats, 10 is too
   tight for clustered-at-infrastructure moments to register.
2. **Saturation curve on Explore's weight.** Real cats don't explore
   indefinitely — past a local familiarity threshold it becomes
   indistinguishable from Wander. Current formula multiplies by
   `unexplored_nearby` linearly; at 50% locally explored, Explore still
   scores 0.5× its raw weight (enough to beat Wander's 0.08 floor).
   Target: sharp decay once local exploration fraction crosses ~0.7.
   Touch points: `src/ai/scoring.rs:302–309` and the radius/threshold
   args to `ExplorationMap::unexplored_fraction_nearby`.
3. **Strategist coordinator task board**
   (`docs/systems/strategist-coordinator.md`). The structural fix: a
   two-layer planner (strategic goal → tactical action) that gives cats
   a colony-level task board to align behavior against. Explore becomes
   "I have no better goal" rather than "I have no target." The doc itself
   gates this on the Cook loop firing end-to-end first — which is partly
   unblocked by the eat-threshold balance change above.

**Ordering:** (1) and (2) are small scoring-layer tunes with seed-42
A/B verification. (3) is real engineering and wants its own design pass.
Do them in order; (1) and (2) should make the strategist's value visible
before it's scoped.

### 2. Hunt-approach pipeline failures

**Why it matters:** 1,774 "lost prey during approach" failures in the
treatment soak vs. 9 "no scent found" search timeouts. Refines the
findability hypothesis: cats locate prey via scent fine, then lose it
during stalk/approach.

**Candidate levers:**
- Stalk speed (currently 1.0 tiles/tick, previously tuned up from 0.5)
- Approach speed (currently 3 tiles/tick)
- Prey detection-of-cat during approach phase (`try_detect_cat` in
  `src/systems/prey.rs`)
- Stall-out conditions — "stuck while stalking" fires 257–341× per soak,
  which is a separate failure mode from "lost"

**Catches-per-week trajectory** (seed-42, 17 weeks): week-0 boom (66),
weeks 1–3 settle (22/9/18), weeks 4+ oscillate 3–15. Not a flatline — the
local depletion → recovery cycle works. The issue is conversion: 1,981
Hunt plans created, ~11% convert to kills.

### 3. Mentor snapshot is never applied

**`src/steps/disposition/mentor_cat.rs:23–25`** — `resolve_mentor_cat`
clones the mentor's `Skills` into a snapshot for the apprentice; the
snapshot is never consumed. Mentoring fires (when a target exists) and
the mentor gains mastery/respect, but the apprentice learns nothing.

Orthogonal to everything else. Small fix. Primes follow-on #1 to actually
have an effect — if we lift Mentor's scoring but the action teaches
nothing, generational knowledge transfer stays cosmetic.

### 4. Magic hard-gated at scoring

**`src/ai/scoring.rs:483`** — `PracticeMagic` only scored if
`ctx.magic_affinity > 0.3 && ctx.magic_skill > 0.2`. ~60% of cats fall
below the affinity threshold and never see magic as a scoring option.

Contradicts `docs/systems/project-vision.md`'s framing of magic as an
ecological phenomenon — a kitten wandering into a FairyRing should feel
the pull whether or not it has "magic training." The misfire system
(`check_misfire`, `src/systems/magic.rs:919–940`) is the intended risk
gate for unskilled attempts; the scoring-level gate makes it unreachable.

Also touches `src/systems/disposition.rs:1675–1676, 1717–1718, 1748`
(redundant downstream gates that become dead once the scoring gate eases).

---

## Conventions

- When an item here becomes a plan, write the plan and leave a pointer in
  the entry (don't delete it until the plan lands).
- When an item lands, move the entry to a "Landed" section at the bottom
  with the commit hash, or just delete it if trivial.
- New entries go at the end of the relevant section, dated inline if the
  context is time-sensitive.
