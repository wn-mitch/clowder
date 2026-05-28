# Ticket lifecycle

The lifecycle is **script-driven**. The scripts exist precisely to absorb repetition; re-implementing them by hand burns tokens for zero added value and risks divergent frontmatter shapes. Same enforcement strength as "Substrate stubs are forbidden" — if the script can do it, the script does it.

## Indexes

Read before any new system / balance change / non-trivial refactor:

- `docs/open-work/tickets/<NNN>-<slug>.md` — frontmatter (`status`, `cluster`, `initiative`, `parked`, `blocked-by`) is source of truth; index at `docs/open-work.md`.
- `docs/open-work/pre-existing/*.md` — long-lived issues.
- `docs/open-work/landed/<NNN>-<slug>.md` — per-file landed archive, same layout as active tickets, with `landed-at` + `landed-on` frontmatter.
- `docs/open-work/clusters.md` — categorical bucket taxonomy.
- `docs/open-work/initiatives/*.md` — thematic outcomes.
- `docs/wiki/systems.md` — Built / Partial / Aspirational per system.
- `docs/balance/*.md` — append iterations to the existing thread.

## Two-axis ticket tagging

Every ticket carries exactly one `cluster:` (categorical — *where the work lives in code*, see `docs/open-work/clusters.md`) and zero-or-more `initiative:` tags (thematic — *what outcome it serves*, see `docs/open-work/initiatives/`).

A crafting ticket and a monument ticket carry different clusters (`items-crafting` vs `buildings-zones`) but can share `initiative: [world-richness]`.

**`--cluster` is required at open-time** (`just open-ticket "<title>" --cluster <name>` errors without it); `--initiative <a,b>` is optional. The index renders both axes: `## Ready by cluster` for categorical filtering, `## Ready by initiative` for thematic rollups.

Never reuse `cluster:` to express thematic outcomes — that conflation was the substrate-shape problem that motivated the split. Precedent: tickets 305 / 306 / 307.

## Before starting work

- `just open-work-active` — what's load-bearing right now.
- `just open-work-ready` / `open-work-wip` — match against existing tickets.
- `just open-work-ready-filtered --cluster <name>` or `--initiative <name>` — filter the ready queue.
- `just open-work-stale` — park-bankruptcy candidates.
- `just open-work-blocking <id>` — transitive blocker chains.
- `just initiatives` — thematic trajectory.
- `just open-work-epics --check` — flag orphan tickets and stale rosters.
- `just similar --centroid <initiative>` / `--not-tagged <initiative>` — initiative-scoped discovery.
- `just next --initiative <name>` — initiative-scoped recommendations.
- Check `docs/wiki/systems.md` if a system is named.

If no ticket matches, name whether the work advances `project-vision.md` §5 (broaden sideways) or a continuity canary, confirm with the user, then run `just open-ticket "<title>" --cluster <name>` (add `--bugfix`, `--initiative <a,b>`, or `--blocked-by <ids>` as needed) as the first commit — **never hand-write the file.**

If it advances an in-flight ticket, flip its `status: in-progress` and regenerate the index with `just open-work-index`. (This transition has no dedicated script yet — see "Coverage gaps" below.)

## Landing / deferring / surfacing

| Event | Action |
|---|---|
| Landed | `just land <id>` (see flags below) |
| Surfaced mid-session | `just open-ticket "<title>" --cluster <name>` |
| Deferred | Set `status: parked` + `parked: <date>` + a `## Log` line naming the blocker, then `just open-work-index`. |
| Trivial work without a ticket | Write a fresh `landed/NNN-<slug>.md` with the standard frontmatter, then `just open-work-index`. |
| Balance iteration | Append to the existing `docs/balance/*.md` thread. |
| `SimulationPlugin::build()` changed | Regenerate `docs/wiki/systems.md` (`just wiki`) in the same commit. |

### `just land <id>` flags

`land` rewrites frontmatter, moves `tickets/NNN.md` → `landed/NNN.md`, drops the id from every dependent's `blocked-by`, auto-promotes newly-unblocked tickets to `ready`, regenerates `docs/open-work.md`.

- `--commit "<msg>"` — bundle the jj landing (saves ~7 commands).
- `--sha <hex>` — backfill `landed-at: pending`.
- `--log "<entry>"` — append a `## Log` line.

### `just open-ticket` flags

Picks the next id, instantiates the template, fills frontmatter, and regens the index.

- `--bugfix` — selects the bugfix template.
- `--cluster <name>` — required.
- `--blocked-by <ids>` — sets `status: blocked` automatically.

> After every `open-ticket`, immediately Write the body. The script only fills frontmatter; the body stays as placeholder text. Don't batch multiple opens. Self-check: `grep -l 'One paragraph: what problem' docs/open-work/tickets/*.md`. (Memory: `feedback_open_ticket_needs_body_write`.)

## Coverage gaps (manual edits still required, no script yet)

(a) `ready → in-progress` flip on an existing ticket — edit `status:` line, then `just open-work-index`.

(b) `ready → parked` (and `parked → ready`) — edit `status:` + `parked:` + `## Log` line, then `just open-work-index`.

(c) Trivial work landed without ever opening a ticket — write a fresh `landed/NNN-<slug>.md` directly with the standard frontmatter (`status: done`, `landed-at`, `landed-on`), then `just open-work-index`.

All other transitions go through `just land` / `just open-ticket`.

## Antipattern migration follow-ups are non-optional

When a substrate-over-override or antipattern-migration ticket narrows scope, lists items in §Out of scope, or parks subscope ("park as a separate ticket," "follow-on if desired"), each parked item MUST be opened with `just open-ticket "<title>" [--blocked-by <parent>]` in the same commit that lands the parent ticket.

- `--blocked-by` auto-sets `status: blocked`.
- Omit it for `status: ready`.
- The opening commit's `## Why` references the parent's narrowing decision.
- The parent ticket's `## Log` lands-day line names the IDs opened with it.

The repo is large; "open as follow-on if desired" rots into lost context. This is the substrate-over-override discipline applied to the work-tracking layer itself: don't author parallel intent ("we should do X someday") in conversation memory when the index can hold it durably.

## Follow-ons inherit their parent's epic

When a ticket is a follow-on of an epic child (a tuning pass, a deferred sub-scope, a doctrine correction, a next-phase consumer), it belongs to the **same epic dashboard** as its parent — list it in that epic's body so the dashboard stays the complete "what's left in this layer?" read.

Example: 369 is 016 Phase 2b, so its descendants (461 / 462 / 463 / 476 / 477 / 478 / 479) are 016 children and live on the 016 dashboard, even though none is itself a numbered phase. Add a "follow-on clade" or "related work (not yet phased)" section rather than forcing non-phase tickets into the phase map.

**Caveat:** `just open-work-epics --check` detects orphans by **cluster match only** — it flags every same-cluster ticket missing from a given epic's roster, which over-reports when a cluster spans multiple epics (e.g., every `ai-substrate` ticket reads as a 128 orphan). Epic membership is curated by hand in the dashboard body, not auto-derived. Treat the check as a discovery aid, not ground truth.

## Major in-flight: AI substrate refactor

Spec: [`docs/systems/ai-substrate-refactor.md`](../systems/ai-substrate-refactor.md). §4 markers + §6 target-taking DSEs are load-bearing. **§4.7 substrate-vs-search-state is required reading before opening any substrate-migration ticket** — it names the boundary that 092 misclassified.

Status: [`docs/open-work/tickets/060-ai-substrate-refactor-epic.md`](../open-work/tickets/060-ai-substrate-refactor-epic.md). Read before any DSE port.

Balance-tuning on refactor-affected metrics is **deferred** until the substrate stabilizes. DSE registration: `populate_dse_registry` in `src/plugins/simulation.rs`. Exemplar port: `src/ai/dses/socialize_target.rs`.

## Parallel-session orchestration

See [`parallel-sessions.md`](parallel-sessions.md) for the operator surface (`/work` / `/retag` / `/foreman`), session lifecycle, three-track partition (substrate-sensitive / coherent-block / swarm-safe), polecat spawning, and the `refinery` gate.
