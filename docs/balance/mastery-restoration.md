# Mastery restoration — iteration 1 (conservative signature-safe pass)

**Status:** landed alongside `acceptance-restoration.md` iteration 1.
Baseline at c49056f; treatment with `fight_mastery_gain = 0.03`,
`survey_mastery_gain = 0.02`. Most planned plug-points deferred — see
"Scope boundaries" below.

## Context

The colony-averaged Maslow chart (seed-42, 15 min soak, commit c49056f)
showed **mastery pinned at 0.0 across the run** after an initial decay
from ~0.4 in the first ~5000 ticks. Static analysis found only two
restorers:

- `src/steps/disposition/mentor_cat.rs:49` — grows the **mentor's**
  mastery on an apprentice-teaching tick. Requires a valid apprentice.
- `src/systems/aspirations.rs:569` — `+0.05` on a rare aspiration
  milestone trigger.

On seed-42 neither fires at a cadence that offsets drain
(`mastery_base_drain * (1 + diligence × 0.5)`, ~0.00002–0.00003/tick),
so mastery sinks and stays pinned.

This is structurally identical to the acceptance problem (see
`docs/balance/acceptance-restoration.md`): continuous drain, rare-event-
gated restoration → pinned to 0 → amplified through colony welfare via
Maslow esteem-tier suppression.

## Hypothesis

> Mastery models the **felt sense of growing competence** — a
> subjective skill-confidence axis distinct from the per-skill `Skills`
> numeric track (which itself largely doesn't grow today — only
> `combat.rs:413` actually increments a skill from banishment triumphs).
> Mastery should fire from action **success**, not just attempt. Wiring
> mastery gains to witnessed step completions gives cats a steady
> restoration signal that keeps the need off its floor, without changing
> the ecological rate of any action.

## Prediction

| Metric | Direction | Rough magnitude |
|---|---|---|
| Colony-averaged mastery | ↑ from 0.0 | 0.1–0.3 (conservative iter) |
| Mentor DSE scoring | ↓ slightly | less mastery deficit pressure |
| Continuity canary: mentoring | stays ≥ 1× | hard gate |
| Welfare composite | ↑ slightly | esteem term partially recovers |
| Survival canaries | unchanged | hard gates |

The "conservative iter" estimate reflects the deferred plug-points (see
below) — this iteration only catches two low-cadence plug-points, so
expect a small shift rather than a full recovery.

## Scope boundaries (iteration 1)

### In scope — resolvers that already take `&mut Needs`

Two existing resolvers carry `&mut Needs` in their signatures and have
a witnessed-success gate. Adding `needs.mastery += gain` in the Advance
arm is a zero-signature-change edit.

1. **`fight_mastery_gain = 0.03`** in `resolve_fight_threat`
   (`src/steps/disposition/fight_threat.rs:45-56`). Fires on completed
   fight engagement (`ticks ≥ fight_duration`, morale not broken).
   Parallels the existing combat skill growth on the same gate — felt-
   competence from "I held my ground."

2. **`survey_mastery_gain = 0.02`** in `resolve_survey`
   (`src/steps/disposition/survey.rs:31-43`). Fires on completed
   survey step (`ticks ≥ survey_duration`). Independent of discovery
   value — the skill is "I went and looked," not "I found something."

### Deferred — resolvers requiring signature changes

All the plug-points my plan identified as "high-frequency" don't take
`&mut Needs` today (`resolve_cook`, `resolve_construct`, `resolve_tend`,
`resolve_gather_herb`, `resolve_set_ward`, `resolve_cleanse_corruption`,
`resolve_harvest`, `resolve_repair`). Adding mastery there means adding
a `&mut Needs` parameter, which touches resolver signatures **and** the
dispatch call site. A parallel Claude Code session was mid-way through
an LLVM optimization-cliff split of `resolve_disposition_chains` during
this work; widening resolver signatures risked silent conflicts with
their in-flight structural changes.

Follow-up iteration (file a new open-work.md entry):

- Add `&mut Needs` to cook/construct/tend/harvest/gather/repair/
  set_ward/cleanse/prepare_remedy/apply_remedy. Dispatch call site
  plumbs actor needs (already available there as `&mut needs`).
- Constants: `mastery_per_successful_cook`, `mastery_per_build_tick`,
  `mastery_per_successful_tend`, `mastery_per_successful_hunt`,
  `mastery_per_magic_success`.
- Hunt is inline in the dispatch match (`StepKind::HuntPrey` at
  `disposition.rs`-ish line 2877 in the current file) rather than a
  standalone resolver — needs a different plug-point pattern.
- Expected magnitude after the full plug-point set lands: mastery
  colony-average in the 0.3–0.5 band. If iteration 1's conservative
  shift is measurable, that's a strong signal the full set will hit.

## Observation

Baseline: `logs/tuned-42-baseline-c49056f/` (c49056f pre-change).
Treatment: `logs/tuned-42/` (post-change, commit TBD).

Per-cat late-soak mastery (~tick 1.35M, 8 cats):

| Cat | Baseline mastery | Treatment mastery |
|---|---:|---:|
| Birch | 0.000 | 0.999 |
| Calcifer | 0.000 | 1.000 |
| Ivy | 0.000 | 0.999 |
| Lark | 0.000 | 0.999 |
| Mallow | 0.000 | 0.998 |
| Mocha | 0.000 | 0.999 |
| Nettle | 0.000 | 1.000 |
| Simba | 0.000 | 0.999 |

**Every cat saturates to 1.0.** `fight_mastery_gain` likely never
fired (`Feature::ThreatEngaged` doesn't appear in
`SystemActivation.positive` for either run); the saturation is driven
by `survey_mastery_gain = 0.02` on repeated `resolve_survey`
completions. `resolve_survey` emits `StepOutcome<()>` with no Feature,
so the emission count can't be read from `SystemActivation` — only the
mastery saturation itself confirms it fires.

Welfare composite (ColonyScore events, averaged over middle third of
soak):

| Metric | Baseline | Treatment | Δ |
|---|---:|---:|---:|
| Welfare (mid) | 0.552 | 0.586 | +6% |
| Welfare (end) | 0.537 | 0.640 | **+19%** |

End-of-soak welfare is notably higher in treatment. This matches the
mechanism hypothesis: mastery-at-ceiling unblocks the Maslow esteem-
tier suppression in `colony_score.rs:148-156`, allowing level-5
fulfillment to register.

Survival canaries:

| Metric | Baseline | Treatment | Result |
|---|---:|---:|---|
| Starvation deaths | 0 | 0 | **pass** |
| ShadowFoxAmbush deaths | 0 | 0 | **pass** |
| Footer written | yes | yes | **pass** |
| `never_fired_expected_positives` | 12 | 12 | **pass (same list)** |

## Concordance

**Direction:** match. Mastery rises; welfare rises; survival canaries
hold. Every cat reaches saturation, which is direction-correct but
**magnitude is out of band.**

**Magnitude:** rejected as "within 2×" for iteration 1. Predicted
0.1–0.3 colony-average; observed ~1.0 for every cat. That's ~3–10×
over the predicted upper bound.

**Mechanism correction for iteration 2:**

- Drop `survey_mastery_gain` from 0.02 → 0.002 (10× cut). Survey is
  evidently a more common resolver completion than my plan assumed —
  survey_duration is 50 ticks (vs groom_other's 80), and surveys don't
  get preempted the same way grooming does (no urgency-tier takeover).
- Keep `fight_mastery_gain` at 0.03 until we confirm
  `Feature::ThreatEngaged` actually fires. If it's also a never-fire,
  the gain is academic.
- Before re-soaking, investigate whether `Survey` saturation is
  "everyone surveys constantly" (natural ecology) vs "the DSE-scoring
  over-prefers Survey on seed-42" (balance regression risk). The
  baseline shows `AspirationSelected=8` in both runs — same aspiration
  pattern — so it's probably natural survey cadence, not a scoring
  shift.

**No regression.** Survival canaries hold. Welfare actually improves.
The overshoot is a tuning issue, not a structural problem.

## Iteration 2 — plan (not yet landed)

- `survey_mastery_gain`: 0.02 → 0.002.
- Instrument `Feature::Surveyed` or similar so future soaks can verify
  cadence directly rather than inferring it from saturation.
- Expand plug-point set: add `&mut Needs` to cook/construct/tend/
  harvest/gather/repair/magic resolvers once the parallel session's
  disposition.rs split settles, per the "Deferred" section above.

## Related work

- `docs/balance/acceptance-restoration.md` — sibling restoration
  pathway, landed same commit, same structural shape.
- `docs/open-work.md #12` (warmth split) — adjacent axis work.
- `docs/systems/ai-substrate-refactor.md` §13.1 — corruption-axis and
  other need-axis work relevant to the broader need-pathway audit.
