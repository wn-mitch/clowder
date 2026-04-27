---
name: hypothesize
description: Run a balance hypothesis end-to-end (`just hypothesize <spec.yaml>`) — baseline + treatment sweeps, concordance check, draft balance doc. Use whenever the user wants to test a balance prediction with the four-artifact methodology (hypothesis · prediction · observation · concordance) that CLAUDE.md mandates for any drift > ±10% on a characteristic metric. Trigger phrases — "test if X causes Y", "validate the prediction that…", "run the four-artifact thing on…", "does increasing K shift the metric we expect", "I want to ship a balance change". Do NOT fire for — verdict checks on a single run (use `just verdict`), one-knob constant lookups (use `just explain`), per-DSE drift inspection (use `just frame-diff`), single-seed sanity checks (use `just soak` + `just verdict`).
---

# Balance Hypothesis Runner (`just hypothesize`)

`just hypothesize <spec.yaml>` is the one-call execution of the four-artifact balance methodology — runs a baseline sweep + treatment sweep with a `constants_patch`, evaluates concordance against the prediction, and drafts `docs/balance/<slug>.md` with all four artifacts pre-filled.

## When to fire

**Fire when:**

- User wants to test that a constants change actually moves a footer metric in the predicted direction (the entire point of the four-artifact discipline).
- User says "the prediction is X, let's verify" or similar.
- User has drafted a hypothesis YAML and wants to run it.
- Drift > ±10% on a characteristic metric is observed and per CLAUDE.md needs a hypothesis to ship.

**Do NOT fire when:**

- User wants survival/continuity verification on an existing run — that's `just verdict`.
- User wants to know what a single constant does — that's `just explain`.
- User wants per-DSE score-distribution drift between two focal traces — that's `just frame-diff`.
- User just wants to run a sweep without a hypothesis — call `just sweep` directly; reserve `hypothesize` for hypothesis-driven runs.
- User is iterating on the YAML itself (no need to run yet).

## The envelope

```jsonc
{
  "slug": "warmth-shift-courtship",
  "spec": {
    "hypothesis": "Increasing social_warmth_socialize_per_tick lifts courtship rate without starvation regression",
    "constants_patch": { "fulfillment.social_warmth_socialize_per_tick": 0.02 },
    "metric": "courtship_attempts_total",
    "direction": "increase",
    "rough_magnitude_pct": [10.0, 100.0],
    "seeds": [42, 99, 7],
    "reps": 3,
    "duration": 900
  },
  "baseline_dir":  "logs/sweep-baseline-warmth-shift-courtship",
  "treatment_dir": "logs/sweep-warmth-shift-courtship-treatment",
  "concordance": {
    "metric": "courtship_attempts_total",
    "predicted_direction": "increase",
    "predicted_magnitude_pct": [10.0, 100.0],
    "observed_direction": "increase",
    "observed_delta_pct": 32.4,
    "p_value": 0.0081,
    "effect_size": 0.74,
    "verdict": "concordant" | "off-magnitude" | "drift" | "wrong-direction" | "inconclusive"
  },
  "balance_doc": "docs/balance/warmth-shift-courtship.md",
  "next_steps": [
    "just verdict logs/sweep-warmth-shift-courtship-treatment/42-1",
    "edit docs/balance/warmth-shift-courtship.md and commit"
  ]
}
```

**Exit codes:** `0` concordant · `1` inconclusive / off-magnitude / drift · `2` wrong-direction or hard fail.

## Spec YAML shape

```yaml
hypothesis: "Increasing social_warmth_socialize_per_tick lifts courtship rate"
constants_patch:
  fulfillment.social_warmth_socialize_per_tick: 0.02
prediction:
  metric: courtship_attempts_total       # dotted path into _footer
  direction: increase                    # increase | decrease | unchanged
  rough_magnitude_pct: [10.0, 100.0]     # |Δ| band the prediction commits to
seeds: [42, 99, 7]                       # default if omitted
reps: 3                                  # per-seed reps (default 3)
duration: 900                            # seconds per soak (default 900)
```

`constants_patch` keys must match the dotted-path catalog from `just explain --list`. Misspellings will surface as `metric not found` after both sweeps complete (~30 min wasted) — verify with `just explain <patch.key>` before invoking.

## Concordance verdicts

- **concordant** — direction matched the prediction AND |observed Δ| ∈ predicted magnitude band.
- **off-magnitude** — direction right, magnitude outside the band (treat as a re-tune, not a refutation).
- **drift** — direction was *unchanged* but observed exceeded ±10% noise (or vice versa).
- **wrong-direction** — observed moved opposite to predicted (the hypothesis is refuted; do not ship).
- **inconclusive** — no measurable Δ; usually means duration too short or seeds too few.

## Background-safe execution

`hypothesize` writes `logs/hypothesize-<slug>/STATUS.json` after every phase (`starting → baseline-sweep → treatment-sweep → concordance → drafting-doc → done`). Re-running with the same slug **resumes** from the last completed phase — sweeps that already have `_footer` lines aren't re-run. Use `--skip-baseline` or `--skip-treatment` to force-reuse existing sweep dirs.

## Wall-clock cost

Default settings (3 seeds × 3 reps × 900s) → **two sweeps × 9 runs × 15 min ≈ 4.5 h serial** — but `just sweep` parallelizes 4-way, so closer to 70–80 min. Smoke a hypothesis first with `--seeds 42 --reps 1 --duration 60` (≈ 4 min total) before committing to the full run.

## Always pass `--rationale "<why>"` when called by an agent

A successful `hypothesize` run takes 70+ minutes; the rationale is what makes it findable later. Pass a one-sentence "what was I testing and why":

- `--rationale "ticket 042: testing whether warmth knob lifts courtship without starvation regression"`
- `--rationale "post-substrate-refactor sanity check on ward-decay prediction from balance doc"`

Skip the flag only when running by hand from the CLI.

## Examples

```bash
just hypothesize docs/balance/my-hypothesis.yaml                       # full run
just hypothesize docs/balance/my-hypothesis.yaml --text                # human-readable summary
just hypothesize docs/balance/my-hypothesis.yaml --slug custom-slug    # override doc filename
just hypothesize SPEC --duration 60 --seeds 42 --reps 1                # 4-min smoke test
just hypothesize SPEC --skip-baseline                                  # reuse existing baseline-<slug> sweep
just hypothesize SPEC --skip-treatment                                 # rerun analysis only
```

## Relationship to neighbouring tools

- **`just sweep`** — single-sweep primitive. `hypothesize` calls it twice. Use `sweep` directly only when you have no hypothesis (rare; CLAUDE.md prefers hypothesis-driven sweeps).
- **`just verdict`** — survival/continuity gate on a single run. Run `verdict` on one of the treatment-sweep runs after `hypothesize` finishes to confirm survival canaries didn't regress alongside the metric shift.
- **`just frame-diff`** — per-DSE focal-trace drift between two traces. Complementary to `hypothesize` (which works on footer metrics): use `frame-diff` to attribute a footer-metric shift to specific DSEs.
- **`docs/balance/*.md`** — `hypothesize` drafts a new file unless one already exists for the slug. **Never overwrites** an existing balance doc — re-running mid-iteration appends a `_treatment` sweep but leaves the doc alone.

## Non-goals

- Does not edit `src/resources/sim_constants.rs`. The `constants_patch` is applied at runtime via `CLOWDER_OVERRIDES`. Shipping the change is a separate manual step (edit + commit).
- Does not block on survival regressions. A wrong-direction concordance with passing canaries still exits 2 — verdict-canary checks must be run separately on a representative treatment run.
- Does not ladder up to multi-metric hypotheses. One metric per spec; combine with `just sweep-stats --vs` after the run if you need cross-metric comparison.
