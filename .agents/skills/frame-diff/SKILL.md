---
name: frame-diff
description: Per-DSE focal-cat trace diff (`just frame-diff <baseline> <new> [hypothesis.md]`) — ranks every DSE by |Δ mean(final_score)| between two trace sidecars and optionally classifies each row's concordance against a phase balance doc's hypothesis table. Use when comparing two focal-cat traces to attribute a behavior change to specific DSEs. Trigger phrases — "did the substrate refactor change DSE behavior", "compare focal traces between baselines", "which DSE moved between commits", "is the L1 invariant holding", "did Mate stop firing after the patch". Do NOT fire for — footer-field comparisons (use `just verdict` or `just sweep-stats --vs`), single-trace inspection (use `just q trace`), constants-only diffs (use `diff <(jq …)`).
---

# Per-DSE Trace Frame Diff (`just frame-diff`)

`just frame-diff BASELINE NEW [HYPOTHESIS]` walks two focal-cat trace JSONL files, joins L2 records by DSE, and reports the largest mean(`final_score`) deltas. With `--hypothesis PATH` it overlays predicted directions from a phase balance doc and classifies each row as `ok | drift | wrong-direction | untracked`.

## When to fire

**Fire when:**

- User has two trace sidecars (e.g. `logs/baseline-X/trace-Simba.jsonl` and `logs/tuned-42/trace-Simba.jsonl`) and wants to know which DSEs moved.
- Substrate-refactor phase exit gate: "did Phase N change DSE behavior beyond what the hypothesis table predicted?"
- Investigating a footer-metric shift surfaced by `verdict` or `hypothesize` and wanting to attribute it to specific DSE score distributions.

**Do NOT fire when:**

- The user has only one trace — that's `just q trace` (subtool `--layer=L2` or `--top-dses`).
- The user wants to compare *footer* fields, not DSE scores — that's `just verdict` (single run vs baseline) or `just sweep-stats --vs <other>` (sweep vs sweep).
- The traces come from different commits without a header match — `frame-diff` will warn and proceed advisorially, but interpret results with caution.
- Phase 2 invariant check (L1 must be tick-for-tick identical) — pass `--strict` to gate on any drift.

## Output shape

**Plain text, no JSON mode** (this is the only deliberate exception in the agent-tooling surface — the table is the contract). Stable rows:

```
headers match — traces are directly comparable
commit a1b2c3d  seed 42  focal_cat Simba

parsed 12 predictions from docs/balance/substrate-phase-3.md
  Mate: predicted up
  Farming: predicted up
  ...

top-15 per-DSE mean-score deltas:

  DSE                       baseline         new      Δ mean       rel     Δ count  concordance
  ----------------------------------------------------------------------------------------------
  Mate                        +0.420      +0.580      +0.160    +38.1%        +12  ok  rose (Δ mean +0.160)
  Farming                     +0.000      +0.310      +0.310       new        +47  ok  rose (Δ mean +0.310)
  Hunt                        +0.510      +0.490      -0.020     -3.9%         -2  drift  expected up; observed -0.020
  ...

concordance: ok — no unacknowledged drift on tracked DSEs
```

**Stable contract:**

- Header line: `headers match` or `header mismatch: <reason>` followed by per-key diff. **Mismatch means traces are not directly comparable** — diff still runs but treat as advisory.
- Column shape: `DSE | baseline | new | Δ mean | rel | Δ count | concordance | note`. Always 7 columns; `concordance` is `ok|drift|wrong-direction|untracked` and `note` is human-readable rationale.
- Footer line: one of `concordance: ok` | `concordance: drift` | `concordance: wrong-direction drift — investigate before phase exit`.

**Exit codes:** `0` no wrong-direction drift (always, unless `--strict`) · `1` drift detected with `--strict` · `2` wrong-direction drift OR a trace file missing.

## Hypothesis table format (from balance doc)

`--hypothesis docs/balance/<phase>.md` parses Markdown table rows shaped like:

```markdown
| **Mate (L3)** | CompensatedProduct | Logistic(...) | Gate-starvation resolved … firing count rises from ~0 to ≥3 … |
| **Farming**   | CompensatedProduct | Quadratic(2) food-scarcity | First-ever fire. |
```

Direction is parsed conservatively from prediction-cell keywords:

- **up** — `rise`, `rises`, `up`, `increase`, `grows`, `first-ever fire`, `non-zero`, `≥`
- **down** — `fall`, `falls`, `decrease`, `decline`, `down`, `retire`
- **flat** — `unchanged`, `no change`, `within noise`, `flat`

Rows whose predictions don't carry an obvious direction keyword are silently skipped (classified `untracked`).

## Concordance rules

| Predicted | Observed     | Status            |
|-----------|--------------|-------------------|
| up        | Δ > 0        | `ok`              |
| up        | Δ ≈ 0 (≤.01) | `drift`           |
| up        | Δ < 0        | `wrong-direction` |
| down      | Δ < 0        | `ok`              |
| down      | Δ ≈ 0        | `drift`           |
| down      | Δ > 0        | `wrong-direction` |
| flat      | rel ≤ ±10%   | `ok`              |
| flat      | rel > ±10%   | `drift`           |
| (none)    | any          | `untracked`       |

## Examples

```bash
# Bare diff — top 15 DSEs by |Δ mean|.
just frame-diff logs/baseline-pre-refactor/trace-Simba.jsonl logs/tuned-42/trace-Simba.jsonl

# With hypothesis overlay — phase exit gate.
just frame-diff logs/baseline-phase-2/trace-Simba.jsonl logs/tuned-42/trace-Simba.jsonl docs/balance/substrate-phase-3.md

# Phase 2 strict mode — any drift exits non-zero.
just frame-diff BASELINE NEW --hypothesis HYP --strict
```

## Caveats

- **L2 records only.** L1 (influence-map samples) and L3 (chosen actions) are not aggregated by this tool. For L3 distribution see `just q trace LOG_DIR CAT --layer=L3`.
- **Marker-gated DSEs.** A DSE that's silent in both traces because the focal cat lacks the marker (e.g., a non-Priestess against `Cleanse`) won't appear in the diff at all. That's correct — silence is by design, not drift.
- **`rel` column shows `new` for from-zero shifts.** A DSE that didn't fire in baseline but fires in new shows `rel = "new"`, not `+inf%`.
- **Header mismatch ≠ stop.** If `commit_hash`, `sim_config`, or `constants` differ between the two trace headers, the tool prints "(diff proceeds; results are advisory only)" and continues. Don't trust the numbers in that case — re-generate one of the traces against the other's commit.

## Relationship to neighbouring tools

- **`just q trace`** — single-trace inspection. Use it to drill into one tick of one DSE; use `frame-diff` to compare two traces in aggregate.
- **`just verdict`** — footer-metric drift. Complementary axis: `verdict` says "the run regressed at the colony level"; `frame-diff` says "the regression is concentrated in DSE X."
- **`just hypothesize`** — runs sweeps and computes footer-metric concordance. `frame-diff` is the per-DSE microscope to that telescope.
- **`scripts/frame_diff.py`** — implementation. Source of truth for column-shape changes.
