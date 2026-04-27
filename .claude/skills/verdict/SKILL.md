---
name: verdict
description: One-call run validation for a Clowder soak. Use after running `just soak` or `just soak-trace`, or whenever the user asks "is this run OK?", "did I break the colony?", "verify this run", "check this directory". Composes survival canaries + continuity canaries + constants drift + footer-vs-baseline drift into a single JSON envelope with a pass/concern/fail verdict and `next_steps` hints. Replaces the retired `just autoloop`. Do NOT fire for drill-down queries (that's `just q`/`/logq`), overview reports (that's `/diagnose-run`), or balance hypothesis testing (that's `just hypothesize`).
---

# Run Verdict (`just verdict`)

`just verdict <run-dir>` is the one-call answer to "is this run OK?". It runs `check-canaries` + `check-continuity`, diffs constants against the active baseline, and ranks the top footer-field drifts — emitting a structured JSON envelope and an exit code.

## When to fire

**Fire when:**

- User just ran `just soak` / `just soak-trace` / a sweep and wants to know if the result is acceptable.
- User asks "is this run OK?", "did I break the colony?", "verify this run", "check `logs/tuned-...`".
- An agent loop needs a single-call gate before continuing (e.g., "after each substrate-refactor commit, verify").
- User hands you a run directory or `events.jsonl` and wants a status summary.

**Do NOT fire when:**

- User wants to drill into specifics ("why did Simba die at tick 12000?") — that's `just q` / `/logq`.
- User wants a long-form narrative report — that's `/diagnose-run`.
- User wants to test a balance hypothesis (predict-verify with treatment vs. baseline sweeps) — that's `just hypothesize`.
- User wants per-DSE focal-trace drift — that's `just frame-diff`.

## The envelope

```jsonc
{
  "run":     "logs/tuned-42",
  "verdict": "pass" | "concern" | "fail",
  "canaries": {
    "survival":   "pass" | "fail",
    "continuity": "pass" | "fail:play=0,burial=0,..."
  },
  "constants_drift_vs_baseline": "clean" | "drift" | "no-baseline",
  "footer_drift": [
    { "field": "deaths_by_cause.ShadowFoxAmbush",
      "baseline": 5, "observed": 9,
      "delta_pct": 80.0, "band": "significant" },
    /* ranked by |delta_pct|, top 20 */
  ],
  "baseline":  "logs/baseline-2026-04-25/events.jsonl",
  "commit":    "abc1234",
  "next_steps": ["just q deaths logs/tuned-42 --cause=ShadowFoxAmbush", ...]
}
```

**Exit codes:** `0` pass, `1` concern, `2` fail. The CLI invocation always emits the envelope on stdout — pipe to `jq` for one-field reads.

## Verdict rules

- `fail` if survival canaries fail (Starvation > 0, ShadowFoxAmbush > 10, footer missing, or never-fired-expected positives).
- `concern` if continuity canaries fail (any of grooming/play/mentoring/burial/courtship/mythic-texture at 0), constants drifted from baseline, or any footer field shifted ≥30% from baseline.
- `pass` otherwise.

## Drift bands

- `noise` — |Δ| < 10% (within measurement noise).
- `drift` — 10% ≤ |Δ| < 30% (worth investigating).
- `significant` — |Δ| ≥ 30% (gate-blocking).
- `new-nonzero` — baseline was 0 and observed > 0 (no percentage; surfaces silent-subsystem activations).

## Baseline resolution

Reads in this order:
1. `--baseline <path>` if explicitly passed.
2. `logs/baselines/current.json` if present (Tier 2.2 baseline registry).
3. Falls back to `logs/baseline-pre-substrate-refactor/events.jsonl`.
4. If none of the above exist, returns `constants_drift_vs_baseline: "no-baseline"` and an empty `footer_drift` — survival/continuity canaries still gate the verdict.

## History

Each verdict appends a record to `logs/verdict-history.jsonl` (one JSON line per invocation, with commit hash). Pass `--no-history` to skip. The history file is the regression-hunting backing store for `just bisect-canary` (Tier 2.1).

## Always pass `--rationale "<why>"` when called by an agent

Every invocation appends a record to `logs/agent-call-history.jsonl` (separate from the existing `verdict-history.jsonl` corpus). The rationale is the *why* — one short sentence describing what you were trying to figure out. The verdict itself records the *what*. Together they let later review surface patterns ("we keep verdict-checking after substrate-refactor commits → maybe a post-commit hook would help").

Good rationales:
- `--rationale "checking ticket 042 didn't regress starvation"`
- `--rationale "post-soak gate before promoting baseline"`
- `--rationale "user asked if the colony is OK after the magic-tuning patch"`

Skip the flag only when running by hand from the CLI.

## Examples

```bash
just verdict --rationale "post-soak gate, ticket 042" logs/tuned-42
just verdict logs/tuned-42 --text                                 # human-readable summary
just verdict logs/tuned-42 --baseline logs/baseline-2026-04-25/events.jsonl
just verdict logs/tuned-42 --no-history                           # don't append to verdict-history.jsonl

# Acting on the verdict in a script:
if ! just verdict logs/tuned-42 > /dev/null; then
  echo "run regressed; investigate"
fi
```
