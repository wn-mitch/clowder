---
name: foreman
description: Master-orchestrator skill for spawning swarm-safe polecats (Stage 2 of ticket 354). Reads in-flight polecat state + ready swarm-safe queue + disk, presents a conversational menu, dispatches to `just foreman-*` primitives. Use when the user types `/foreman`, or asks "spawn polecats", "drain the swarm-safe queue", "fire off N parallel sessions", "what are my polecats doing", "shut down all polecats", "tail polecat X". Polecats are headless child `claude` CLI processes — they run swarm-safe-only tickets (docs / frontmatter / atomic refactor / verified-layer-walk bugfix), exit via `/handoff` + `jj git push --bookmark`, and the master foreman auto-lands their bookmarks via `just refinery --auto`. Do NOT fire for substrate-sensitive or coherent-block work (those need human-gated landing — use `/work`).
argument-hint: "[free-text optional bias — 'spawn 2', 'drain', 'watch']"
allowed-tools: ["Bash", "Read", "AskUserQuestion"]
---

# /foreman — polecat dispatch + auto-land

Stage 2 of ticket 354. Master-orchestrator skill that composes `just foreman-spawn` + `just foreman-watch` + `just refinery --auto`. Polecats are ephemeral child `claude` CLI processes spawned via `claude -p --print` against pre-created jj workspaces.

**Hard constraints encoded here and in code:**
- Polecats run **swarm-safe only**. The whitelist is enforced in `scripts/refinery.sh` AND in `scripts/foreman.sh`; this skill is the third layer (refuses to dispatch substrate-sensitive / coherent-block work to a polecat regardless of how the user phrases the request).
- Subscription-billed; **no dollar caps**. Wall-clock via the foreman's wallclock-sentinel is the only runaway protection (default 30m/polecat).
- Master never edits `main` directly. Polecats push their own `session/<slug>` bookmark; the foreman drains via `just refinery --auto`.

**Arguments:** `$ARGUMENTS` — optional free-text bias ("spawn 3", "drain", "watch all"). If empty, present the standard menu.

## Workflow

### 1. Read state in parallel

Issue these Bash calls **in one message**:

- `just foreman --json` → in-flight polecats (slug, PID, alive, last-edit-secs, deadline-secs-remaining, tickets) + ready swarm-safe count
- `just refinery --json` → per-bookmark land status across all tracks (not just swarm-safe)
- `uv run scripts/open_work_filters.py ready --track swarm-safe | head -10` → top of ready queue for sample
- `df -h ~ | tail -1` → disk free
- `sccache --show-stats 2>/dev/null | head -8` → cache stats (sessions share rustc unit-cache via sccache)

### 2. Synthesize one short status block

Render at most 10 lines:

```
Polecats (N tracked):
  · swarmpole-<id>  alive  ticket-X  last-edit Xs  deadline Xm  ← actionable highlight if any
  · ...
Ready swarm-safe: Y ticket(s) · refinery queue: Z landable
Disk: <free>G · sccache <warm>%
```

Highlight order:
1. Stuck polecats (alive AND last-edit > 10min) → first
2. Landable bookmarks (refinery says clean) → second
3. Idle (no polecats running) → last

### 3. Apply free-text bias

- "spawn" / "fire off" / "start" → bias to Action 1 (Spawn)
- "watch" / "status" / "what's going on" → Action 2 (Watch)
- "drain" / "land" / "land what's done" → Action 3 (Drain)
- "shutdown" / "kill" / "stop" → Action 4 (Shutdown)
- "log" / "tail" / "what's polecat X doing" → Action 5 (Log)

Bias = list that action first in the menu and pre-highlight; never skip the menu without explicit confirmation.

### 4. Present the menu via AskUserQuestion

Standard five-option menu (omit non-applicable):

1. **Spawn N polecats** — sub-flow below
2. **Watch in-flight polecats** — `just foreman-watch` (one-shot). For live, instruct the user to run `watch -n 5 just foreman-watch` in another terminal.
3. **Drain landable swarm-safe queue now** — `just refinery --auto`. Confirm if there are landable rows; refuse if all rows are non-swarm-safe.
4. **Shutdown all polecats** — `just foreman-shutdown`. Ask `--hard?` first (default SIGTERM gives /handoff a chance; --hard SIGKILLs immediately).
5. **Tail one polecat's stream-json** — picker over `just foreman --json`, then `just foreman-log <slug>`.

### 5. Action 1 — Spawn sub-flow

AskUserQuestion sequence:

**Step 1: N (how many polecats).** Default 3.
- 1 — single polecat, easiest to verify the spawn loop
- 2 — light parallel
- 3 — standard (Recommended)
- 5+ — confirm explicitly ("yes I want N polecats") — N polecats = N parallel context windows; even subscription-billed this matters for queue-fairness

**Step 2: Wall-clock per polecat.** Default 30m.
- 30m (Recommended) — enough for layer-walk re-verification + apply + check/test + push on atomic swarm-safe work
- 20m — tighter; good for known-fast tickets (frontmatter migrations)
- 60m — looser; for swarm-safe with a substantive code touch

**Step 3: Dry-run first?** If unsure, run `just foreman-spawn N --dry-run` first to verify the spawn plan (which tickets would be picked, which workspaces would be created) without actually firing the children. The dry-run rolls back its session-new claims, so it's safe.

Then dispatch: `just foreman-spawn N --wallclock M` (or with `--dry-run`).

**Important:** after spawn, the recipe enters the auto-poll-and-land loop. It blocks until all polecats exit or wallclock-fire. Tell the user:
- Ctrl-C is safe — polecats keep running (they're detached via nohup); resume with `just foreman-watch` or `just refinery --auto`.
- Each polecat prints `polecat-done: <slug> ticket-<id>` on success or `polecat-abandoned: <slug> <reason>` on graceful abandonment (refuses to push broken state). Canonical reasons: `requires-gui` · `requires-long-soak` · `requires-substrate-judgment` · free-form for everything else.
- Abandoned polecats have their workspace artifacts copied to `logs/polecat-abandoned/<YYYYMMDD-HHMMSS>-<slug>/` (stream, stderr, cmdline, REASON) **before** `session_done.sh --force` removes the workspace. Read `REASON` first; fall back to `polecat-stream.jsonl` for the full trace.

### 6. Action 3 — Drain (refinery --auto)

`just refinery --auto` walks every `session/*` bookmark; lands only `track==swarm-safe` rows that pass `just check && just test` gate; reports skipped rows with their reason (wrong-track / not-fast-forward / dirty-working-copy / check-fail / test-fail / no-changes).

If `--dry-run` was requested, append the flag. Otherwise actually land.

After landing, summarize: which slugs landed → main, which were skipped + why. If any failed the gate (`check-fail` / `test-fail`), surface the `.refinery-gate.log` path in their workspace for inspection.

### 7. Action 5 — Tail one polecat's stream-json

`just foreman-log <slug>` does `tail -f` on the workspace's `.polecat-stream.jsonl`. The stream is JSONL — one JSON object per line, captured from `claude --output-format stream-json`. Useful for "what's polecat-301 thinking right now?" investigations.

## Conventions

- **Never spawn against a non-swarm-safe track.** The whitelist is enforced in code (`scripts/refinery.sh` refuses `--auto` for any non-swarm-safe row even with explicit `--track <other>`); this skill is the third defense. If the user asks to "spawn a polecat against ticket 145 (substrate-sensitive)", explain that polecats are headless and substrate-sensitive needs the layer-walk discipline — route to `/work` Action 4 (Start a new session) instead.
- **Never auto-confirm `--shutdown --hard`.** SIGKILL drops `/handoff` mid-flight; the polecat's session state may not flush. Always ask first.
- **Read state freshly** every invocation. Polecats can die between menu presentation and dispatch.
- **Always print where artifacts live.** When confirming a spawn or surfacing a failure, name the workspace path (`~/clowder-sessions/<slug>/`) and the relevant artifacts (`.polecat-stream.jsonl`, `.polecat-stderr.log`, `.polecat-cmdline`, `.refinery-gate.log`) so the user can post-mortem without asking. For abandoned polecats, the workspace is gone — point at `logs/polecat-abandoned/<stamp>-<slug>/REASON` (one-line cause) and `polecat-stream.jsonl` (full trace) under the repo root.

## Reference

- Plan: `~/.claude/plans/mighty-foraging-biscuit.md`
- Stage 1 plan (companion): `~/.claude/plans/this-is-not-an-curried-hippo.md`
- Tickets: `docs/open-work/tickets/354-...md` (Stage 1) + `docs/open-work/tickets/355-...md` (Stage 2)
- Operator's guide: `docs/workflow/parallel-sessions.md` (Stage 2 section)
- Sibling skills: [`.claude/skills/work/SKILL.md`](../work/SKILL.md) (manual daily-driver) + [`.claude/skills/retag/SKILL.md`](../retag/SKILL.md) (one-shot corpus ceremony)
