---
name: work
description: Daily-driver parallel-session orchestration. Reads in-flight sessions + ready queue + disk, presents an interactive menu, dispatches to the right `just` primitive (session-new / refinery / session-done / block-info). Use when the user types `/work`, or asks "what should I do today", "let's start a session", "land what's ready", "show me what's in flight", "spawn a foreman against the swarm-safe queue". Composes the three-track partition (substrate-sensitive / coherent-block / swarm-safe) from ticket 354. Do NOT fire for ticket recommendation only (use `/next`), specific log queries (use `/logq`), or balance investigation (use `/diagnose-collapse`).
argument-hint: "[free-text optional bias]"
allowed-tools: ["Bash", "Read", "AskUserQuestion"]
---

# /work — parallel-session orchestration entry

This is the daily entry to the parallel-session system (ticket 354). It reads the current state of in-flight sessions, the ready queue partitioned by orchestration track, and the disk budget, then presents an interactive menu and dispatches to the right `just` primitive.

**Arguments:** `$ARGUMENTS` — optional free-text bias ("focus on swarm-safe", "land what's ready", "let's start an HTN block leg", etc.). If omitted, present the standard menu.

## Workflow

### 1. Read state in parallel

Issue these `Bash` calls **in a single message** for parallelism — they're independent reads:

- `just sessions --json` → in-flight sessions (slug, track, tickets, bookmark head, last edit)
- `just refinery --json` → per-bookmark land status (rebase / conflicts / behind / ahead)
- `just open-work-by-track` (or `just retag-audit --json`) → ready-queue rollup per track
- `just open-work-ready-filtered --track substrate-sensitive | head -8` — sample of ready substrate-sensitive
- `just open-work-ready-filtered --track swarm-safe | head -8` — sample of ready swarm-safe
- `just block-list --json` → coherent-block status
- `df -h /Users/will.mitchell | tail -1` → disk free
- `sccache --show-stats 2>/dev/null | head -10` → sccache stats

### 2. Synthesize one short status block

Render in this exact shape (target ≤12 lines total — scannable, not exhaustive):

```
Sessions (N in flight):
  · session/<slug>  (<tickets>)  <track>  <status>     ← actionable highlight if any
  · ...
Ready queue: <X> swarm-safe · <Y> substrate-sensitive · <Z> coherent-block (across <B> blocks)
Disk: <free>G free · <session-disk>G in sessions · sccache <warm>%
```

If a session is `landable-manual` (clean), call that out first — it's the most actionable item. If a session is `conflict`, surface the file path the conflict touched.

### 3. Apply free-text bias (if any)

If `$ARGUMENTS` contains words like "land" / "finish" / "land what's ready" → bias the menu toward Action 1 (Land). If "new" / "start" / "begin" / "fresh" → bias toward Action 4. If "block" / "HTN" / "crafting" → bias toward Action 6. Bias means listing that action first and pre-selecting it; never skip the menu without confirmation.

### 4. Present the menu via AskUserQuestion

Standard six-option menu (omit options that aren't applicable — e.g., omit "Land X" if no session is landable):

1. **Land session/<slug>** — refinery says clean + verdict-pass (manual confirm; swarm-safe may auto if `--auto` lands later)
2. **Resolve conflict on session/<slug>** — refinery says `needs-resolve`
3. **Continue session/<slug>** — open in its workspace
4. **Start a new session** — interactive picker (Action 4 sub-flow below)
5. **Spawn a foreman against swarm-safe queue** — Stage 2 (refuse with "not yet shipped" until /foreman exists)
6. **Show me block status** — calls `just block-list` + per-block detail; surfaces blocks ready for anchor verdict

### 5. Action 1 — Land

Confirm the land per session (substrate-sensitive and coherent-block always require explicit confirmation; swarm-safe may be batched with explicit "land these N rows"). Per session:

- `cd ~/clowder-sessions/<slug>` and run `just verdict logs/tuned-42` if the session had a soak. If no verdict has been run, surface that and ask the user to confirm landing without one. (Default: refuse to land without verdict for `substrate-sensitive`. Allow for `swarm-safe` if no test-affecting changes.)
- `cd ~/clowder && just refinery --land <slug>`
- On success: confirm bookmark dropped + workspace cleaned + tickets are `done` (via `just land`).

### 6. Action 4 — Start a new session

Use AskUserQuestion to choose:

1. **Track**: substrate-sensitive (default), coherent-block, swarm-safe
2. **Tickets**: pick from `open-work-ready --track <name>` output. For substrate-sensitive, suggest 1-2 tickets adjacent by cluster + file-touch (use `just similar <id>` against a recent landing if available). For coherent-block, scope to one block (`just block-info <name>`). For swarm-safe, suggest atomic single-ticket batches.

Compute a slug from the chosen tickets — preferred form: `<cluster>-<ticket-id-or-theme>` (e.g., `socialize-axes`, `htn-tier1`, `inventory-fix`). Validate `[a-z0-9][a-z0-9-]+`.

Then:

- `just session-new <slug> --tickets <id1>[,id2] --track <name> --print-prompt`
- Emit the starter prompt block verbatim — the user copy-pastes it into a new Codex session in the new workspace path.

### 7. Action 6 — Block status

`just block-list --json` → enumerate blocks. For each, surface:

- block name, ticket count, status counts (ready / in-progress / done)
- verdict-anchor (or `(none)` — flag as needing assignment)
- whether the block is `ready-to-anchor` (all non-anchor tickets done; anchor remains) — this is the most actionable per-block signal

If the user picks a block, run `just block-info <name>` and ask whether they want to start a session against it (route to Action 4 sub-flow with `--track coherent-block --initiative <block>`).

## Conventions

- **Never modify `main` directly** from inside this skill. Every move to `main` goes through `just refinery --land <slug>`.
- **Never auto-land** anything in the substrate-sensitive or coherent-block tracks. Even for swarm-safe, default to "show me first" — auto-land is a Stage 3+ feature requiring verdict integration.
- **Always print the starter prompt** for new sessions so the user can spawn a child Codex session manually — there's no automated child-spawning in Stage 1 (`/foreman` ships in Stage 2).
- **Read state freshly** every invocation. Don't cache the menu shape across calls.

## Reference

- Plan: `~/.Codex/plans/this-is-not-an-curried-hippo.md`
- Ticket: `docs/open-work/tickets/354-parallel-session-orchestration-work-skill-three-track-partition-refinery.md`
- Operator's guide: `docs/workflow/parallel-sessions.md` (Stage 1.7)
- Three-track table: AGENTS.md addendum (Stage 1.7)
