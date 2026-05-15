---
id: 318
title: just epic-children <id> query — read dashboard roster, print child status, enforce anti-staleness
status: ready
cluster: tooling-diagnostics-ui
orchestration: swarm-safe
initiative: []
added: 2026-05-14
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Epic-style tracker tickets (060 substrate refactor dashboard, future
similar trackers for life-cycle / belief / coordinator work) maintain
hand-curated rosters of child tickets with status / blockers / cluster
home. The dashboard's own **Anti-staleness Measure** rule — "if the
epic claims a child is `ready` when the child file says otherwise,
the child file is the truth; update the epic to match" — is **not
enforceable today** because no `just` recipe or script answers the
prerequisite question: *what is the current frontmatter status of
the children this dashboard names?*

Concrete pain (2026-05-14 060 audit): the dashboard had been correct
on 2026-05-08, but six days later carried four landed-but-not-marked
roster entries (007 / 027 / 126 / 127) and missed four major
substrate landings entirely (258 / 261 / 263 / 295). The audit that
surfaced these required a hand-rolled bash for-loop reading every
referenced ticket's frontmatter from `docs/open-work/{tickets,landed}/`.
Friction logged in `logs/agent-friction.jsonl` 2026-05-14 (severity:
major).

Linear / Jira / GitHub-projects all give "list children of this
epic with status" as a built-in view. Clowder's flat-file ticket
system does too — just not via a one-shot query. This ticket builds
the missing query.

## Scope

- `scripts/epic_children.py` or equivalent: parse a target epic's
  markdown body (default 060, but generic), extract every
  `(<id>)<ticket-NNN-…md>` link in the **Open child tickets** table
  + the **Phase coverage map**, look up each id under
  `docs/open-work/{tickets,landed}/`, and emit a structured report.
- `just epic-children <id-or-path>` recipe wrapping the script.
- Report shape (JSON envelope per `tooling-diagnostics-ui` convention):
  per-child `{ id, dashboard_claim, frontmatter_status, frontmatter_blocked_by, in_landed: bool, landed_at, drift_kind }`.
  `drift_kind` enum: `consistent` | `landed-but-marked-active` |
  `blocker-mismatch` | `status-mismatch` | `missing-file`.
- Exit codes: `0` = consistent, `1` = drift detected, `2` = epic
  not found / parse error.
- Optional `--fix` flag: rewrite the dashboard's roster rows to
  match frontmatter (status, blockers). Skips additions /
  promotions of unmentioned tickets (that's editorial work, not
  mechanical).

## Out of scope

- Auto-discovering child tickets not explicitly named in the
  dashboard body. Adding new children is editorial work that
  belongs in a human pass.
- Updating the **Critical path** / **Current state** prose sections
  — drift on those is a narrative concern, not mechanical.
- Notifying the dashboard when a child lands. That would need a
  hook in `just land` (or `git post-commit`); valuable but a
  separate ticket.
- Building a generic "epic schema." This ticket assumes 060's
  shape (roster table + phase coverage map); the script can be
  defensive but doesn't need to handle every conceivable layout.

## Current state

Open. The 060 dashboard had its first major reconciliation in
seven weeks land 2026-05-14 (commit `vvnsysox 7d88c74f`): 33-entry
roster, four landed promotions, four new substrate-row additions.
That pass was triggered by user prompt ("there are more updates to
make to match reality in 060"), not by any automated signal.

Memory entry: `feedback_epic_dashboard_needs_queryable_state.md`
(2026-05-14) captures the workflow-level friction.

## Approach

1. Parse the target dashboard's markdown body line-by-line for
   `[NNN](NNN-….md)` references. Bucket by section (Phase coverage
   map, Adjacent / cluster work, Open child tickets roster).
2. For each referenced id, locate the canonical file under
   `tickets/` first then `landed/`. Read frontmatter via existing
   `scripts/generate_open_work.py`'s YAML loader (factor out into
   `scripts/_ticket_frontmatter.py` if useful).
3. Cross-check dashboard claim vs frontmatter:
   - Dashboard says "ready" + child is in `landed/` → `drift_kind: landed-but-marked-active`.
   - Dashboard says "blocked-by 007" + frontmatter says `blocked-by: [128]` → `blocker-mismatch`.
   - Etc.
4. Emit JSON envelope; pretty-print to stderr when `--text` flag is
   passed (consistent with `just verdict` / `just q` envelopes).
5. `--fix` performs in-place rewrites of the roster table only,
   regenerates `docs/open-work.md`, leaves prose sections alone.
   Add a `## Log` entry noting the auto-fix with date + run id.

Reuse: `scripts/generate_open_work.py` already reads frontmatter
across all tickets; lift its YAML-load helper. The `--fix` path
parallels `just land`'s frontmatter rewrites — see
`scripts/land_ticket.py` for the mutation patterns.

## Verification

- Run against current 060 — should report `consistent` since the
  2026-05-14 reconciliation landed.
- Inject a deliberate stale row (mark 050 "ready" in 060's
  roster), re-run — should report `landed-but-marked-active` for
  050 with the landed-at sha in the envelope.
- `--fix` on the injected stale row should restore the row to
  `✅ landed (<sha>)` shape and exit 0.
- Add a CI hook (later — or in this ticket if cheap): `just check`
  could run `just epic-children 060 --quiet` and fail if drift is
  detected. Alternative: a weekly cron / `just diagnose-run`
  follow-on flag. Pick the lightest enforcement that catches the
  staleness window before it grows past a week.

## Log
- 2026-05-14: opened from the 060 full-roster reconciliation
  session. Pain quantified: six-day staleness window covered 4
  child landings + 4 major substrate landings invisible to anyone
  reading 060 alone. Friction breadcrumb in
  `logs/agent-friction.jsonl`. Cluster: `tooling-diagnostics-ui`.
