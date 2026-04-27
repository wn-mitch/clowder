---
name: inspect
description: One-cat post-hoc report (`just inspect <name>`) — personality profile, action distribution, score breakdown, needs timeline, relationships, key decisions, and death info read from `events.jsonl`. Use when the user wants the *aggregate* picture of one cat across a run. Trigger phrases — "tell me about <cat>", "who is <cat>", "what kind of cat is <cat>", "how did <cat> spend the run", "why did <cat> behave that way overall", "show me <cat>'s personality". Do NOT fire for — per-tick decision drilling (use `just q trace` or `just q cat-timeline`), focal-cat L1/L2/L3 trace analysis (that's the `clowder-focal-cat` subagent), or colony-wide queries (use `just q anomalies`, `just q deaths`).
---

# Cat Inspection (`just inspect`)

`just inspect <name>` reads `logs/events.jsonl` (override with `--events <path>`) and prints a multi-section human-readable report for one cat: personality, action distribution, score breakdown, needs timeline, relationships, key decisions, and death info.

Implementation: `examples/inspect_cat.rs` (compiled to `target/release/examples/inspect_cat`). Outputs **plain text only — no JSON mode**.

## When to fire

**Fire when:**

- User asks "what kind of cat is <name>" / "tell me about <cat>" / "who is <cat>" — the orientation question for one cat across a whole run.
- User wants to understand a cat's overall behavioral pattern (e.g., did they spend the run socializing, foraging, fighting?).
- User wants relationship affinity/animosity overview before drilling into specific interactions.
- Cat just died and the user wants the macro view before investigating tick-level cause.

**Do NOT fire when:**

- User wants per-tick decisions, score landscape, or DSE eligibility for one tick — use `just q trace LOG_DIR <name> --layer=L2|L3` or the `clowder-focal-cat` subagent on a trace sidecar.
- User wants narrative beats mentioning the cat — use `just q narrative LOG_DIR --tier=…` then grep, or `just q cat-timeline LOG_DIR <name>`.
- The cat doesn't exist in the events log — `inspect` will print "No events found for cat '<name>' in <path>" and list available cat names. **Read that list and re-fire with the right name.**

## Output shape — stable section headers

The text output is a sequence of sections, each with a stable header. The contract is the headers and their column-shape, not the exact whitespace:

```
═══ Personality: <name> ═══
  drives:           <axis>: <value on -1..+1>  ...
  temperament:      <axis>: <value>
  values:           <axis>: <value>

═══ Action Distribution ═══
  Total actions: <N>
  <DSE>: <count> (<pct>%)  ████████░░  <bar>
  ...

═══ Score Breakdown ═══
  ticks <a>..<b>: top DSEs by mean score
  ...

═══ Needs Timeline ═══
  energy:    <ascii sparkline showing trajectory>
  hunger:    <sparkline>
  ...

═══ Relationships ═══
  <other-cat>: affinity=<value> animosity=<value>
  ...

═══ Key Decisions ═══
  tick <T>: chose <DSE> (score <s>) — context: <summary>
  ...

═══ Death (if cat died) ═══
  tick <T>: <cause>; final state: <snapshot>
```

**Stable contract:**

- Section order is fixed: Personality → Action Distribution → Score Breakdown → Needs Timeline → Relationships → Key Decisions → (optional) Death.
- Personality axes are one of: drives, temperament, values. Each row is `<axis>: <signed-float on -1..+1>`.
- Action distribution percentages always sum to ~100% (rounding). `Total actions: N` is the denominator.
- Death section appears iff the cat actually died.

**Exit code:** `0` always (even when the cat isn't found — error message goes to stderr, available cats go to stderr, and the program exits cleanly so downstream tools don't get a non-zero spurious failure).

## Default events path

Reads `logs/events.jsonl` (the un-prefixed default path) unless `--events <path>` is passed. Most callers want a specific run:

```bash
just inspect Simba --events logs/tuned-42/events.jsonl
```

The justfile recipe (`inspect name *ARGS`) passes `*ARGS` through, so flags after the name reach the binary.

## Examples

```bash
# Inspect against the default logs/events.jsonl (common after a `just headless` run).
just inspect Simba

# Inspect against a specific run.
just inspect Simba --events logs/tuned-42/events.jsonl

# Inspect a cat in a baseline run.
just inspect Calcifer --events logs/baseline-2026-04-25/sweep/42-1/events.jsonl
```

## Caveats

- **Default path predates the per-run convention.** `logs/events.jsonl` is the legacy single-run path (from `just headless` direct output). For runs from `just soak` / `just baseline-dataset`, you almost always want `--events logs/<run-dir>/events.jsonl`.
- **No tick-range slicing.** This is the macro view across the whole run. For tick-range drill-downs, use `just q cat-timeline <log_dir> <cat> --tick-range=N..M`.
- **Action distribution is from `ActionChosen` events, not from the L3 trace layer.** They should match in healthy runs; divergence means the trace sidecar may not be joinable with events.
- **Relationships are inferred from `CatSnapshot` affinity/animosity fields, not from interaction events.** A cat that ignored another cat all run will show neutral values; this is correct.

## Relationship to neighbouring tools

- **`just q cat-timeline <log_dir> <cat>`** — same input source, but emits structured events + narrative mentions interleaved by tick. Better when you want a chronological view; `inspect` is better when you want the aggregate picture.
- **`just q trace <log_dir> <cat> --layer=L3`** — chosen-action distribution from the trace sidecar. Confirm `inspect`'s action distribution against this if joinability is in question.
- **`clowder-focal-cat` subagent** — full L1/L2/L3 trace analysis. Use after `inspect` when the cat has a trace sidecar and you want the decision-landscape diagnostics.
- **`just q deaths <log_dir>`** — colony-wide death incidence. Use when the question is "which cats died" rather than "tell me about this one cat."

## Non-goals

- Does not read trace sidecars (`trace-<cat>.jsonl`). Those need `just q trace` or the `clowder-focal-cat` subagent.
- Does not write any output file. Pure stdout/stderr.
- Does not compare two cats. For dyadic comparison, run `inspect` twice and read the outputs side-by-side.
