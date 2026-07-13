---
name: diagnose-collapse
description: Collapse overview for a failing Clowder soak — names the cluster pattern (e.g. "189-style schedule-edge"), surfaces the collapse trajectory, and suggests next-step drills. Use when the user asks "why did the colony collapse?", "what kind of failure is this?", or "does this look like a prior cluster?". Do NOT fire for — pass/concern/fail gating (that's `just verdict`), per-tick drilling (that's `just q`/`/logq`), or cross-run SQL (that's `just logdb-query`).
---

# Collapse Diagnosis (`/diagnose-collapse`)

`/diagnose-collapse` provides a structured overview of a colony collapse. It reads a failing run's death / anomaly / action signals and produces a cluster-labelled narrative — naming whether the collapse resembles a known prior cluster and suggesting drills to confirm.

## When to fire

**Fire when:**

- A soak failed (`just verdict` returned `fail`) and the user wants the full collapse overview rather than a single targeted drill.
- User asks "what kind of collapse is this?", "does this look like the 189 cluster?", "why did the colony die?"
- Used as the balance investigation entrypoint when the failing pattern is not yet named.

**Do NOT fire when:**

- User wants the pass/concern/fail gate — that's `just verdict`.
- User wants to drill into a specific tick, cat, or event — that's `just q` / `/logq`.
- User wants cross-run aggregate patterns — that's `just logdb-query`.

## Relationship to neighbouring tools

- **`just verdict`** — pass/concern/fail gate. Run `verdict` first; fire `/diagnose-collapse` when `verdict` returns `fail` and the `next_steps` suggest a full overview.
- **`just q` / `/logq`** — per-tick drill-down. `/diagnose-collapse` names the cluster; `just q` confirms the tick-level evidence.
- **`just similar`** — semantic retrieval across prose artifacts. After `/diagnose-collapse` names a cluster (e.g. "189-style schedule-edge"), use `just similar <prior-ticket-id>` to surface the prior fix, balance threads, and related tickets — e.g. `just similar 189 --corpus landed,balance`. This is the canonical "does this collapse pattern look like a prior cluster?" lookup.
- **`just logdb-query`** — cross-run SQL. `/diagnose-collapse` diagnoses one run; `logdb` compares patterns across many runs.
