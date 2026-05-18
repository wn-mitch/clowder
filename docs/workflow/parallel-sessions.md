# Parallel-session orchestration

Operator's guide for running multiple Claude Code sessions in parallel against the Clowder repo. Reference for `/work`, `/retag`, and the `[session]` / `[refinery]` / `[retag]` / `[block]` / `[ticket-query]` recipe groups landed in ticket 354.

## Mental model

You operate one "master" Claude Code session at `~/clowder`. Other sessions run in isolated `~/clowder-sessions/<slug>/` workspaces (jj workspaces sharing `.jj/repo` but with their own working copy + Rust `target/`). Each session owns one bookmark, `session/<slug>`, and never touches `main`. The master session runs `/work` to land child sessions into `main` via the refinery.

Three orchestration tracks govern how a session is treated:

| Track | Verdict cadence | Session lifetime | Polecat-eligible |
|---|---|---|---|
| `substrate-sensitive` (default) | per-ticket, human-gated | short, careful | no |
| `coherent-block` | block-level at verdict-anchor | long, spans context windows | no |
| `swarm-safe` | per-ticket; `refinery --auto` whitelisted in code | ephemeral | **yes (Stage 2 live)** |

Every active ticket carries `orchestration: <track>` in its frontmatter. The default is `substrate-sensitive` (safest). Promote tickets to `swarm-safe` (faster cadence, polecat-eligible) or `coherent-block` (block-level orchestration) explicitly via `/retag` or `just retag <id> --track <name>`.

## Quickstart

**Daily entry:**
```
/work
```

**One-shot corpus tagging (Stage 0 ceremony):**
```
/retag
```

## Session lifecycle (step by step)

### 1. Create a session

```
just session-new <slug> --tickets <id1>[,<id2>] --track <name>
```

Creates `~/clowder-sessions/<slug>/` as a jj workspace, sets the `session/<slug>` bookmark at `main`, writes `.session-info.json`, and atomically claims the named tickets (writes `status: in-progress` per ticket under a `flock` on `docs/open-work/.claim-lock` — double-claiming is refused).

Pass `--print-prompt` to get a copy-pasteable starter prompt for a new Claude session in the new workspace path.

`--pick` auto-selects one ready ticket from `--track <name>` (mutually exclusive with `--tickets`).

### 2. Work the ticket in the new workspace

Open a new Claude Code session, `cd ~/clowder-sessions/<slug>`, paste the starter prompt. Build / test / commit as normal. Stay on the `session/<slug>` bookmark (jj will auto-snapshot edits to it).

When done, exit the session via `/handoff`, then push the bookmark:

```
jj git push --bookmark session/<slug> --allow-new
```

### 3. Master session lands the work

Back at `~/clowder`, run `/work`. It reads `just refinery --json` and surfaces the session as `landable-manual` (or `needs-rebase` / `conflict` if main has moved).

Land:

```
just refinery --land <slug>
```

The refinery:
- Runs `jj git fetch` to refresh origin bookmarks (ticket 409: prevents stale-local-view masking a polecat's push)
- Rebases `session/<slug>` onto current `main` (if behind)
- Advances `main` to the session's head
- Forgets `session/<slug>`
- Calls `session-done.sh <slug> --no-release` to clean up the workspace (the tickets were already set to `done` via `just land` inside the session)

If the rebase has conflicts, the refinery aborts and names the conflict. Resolve in the session's workspace (`cd ~/clowder-sessions/<slug>`), commit, push, retry.

### 4. Abandoned session (no land needed)

```
just session-done <slug>
```

Releases the session's `in-progress` tickets back to `ready` (skips tickets already `done`), `cargo clean`s the workspace target, `jj workspace forget`s, removes the directory, forgets the bookmark.

Pass `--force` to skip the uncommitted-edits guard. Pass `--keep-bookmark` to preserve the bookmark for a later land.

## Three-track orchestration in detail

### substrate-sensitive (the default)

Use for: bugfix work, layer-walk required, balance-affecting changes, anything touching `src/ai/` or `src/components/` that needs the discipline named in CLAUDE.md §"Bugfix discipline".

**Verdict cadence:** per-ticket. The session runs its own soak + `just verdict` before landing. The refinery never auto-lands these.

**Sessions:** short, careful, one ticket at a time. The `/work` "Start a new session" flow suggests one ticket plus optionally one adjacent ticket (by `just similar`) — keeps batches small.

### coherent-block (epic construction)

Use for: epics where intermediate states are structurally unverifiable. Currently identified blocks:

| Block (initiative-id) | Anchor candidate | Member signal |
|---|---|---|
| `htn-method-composition` | 128 (epic) or a registry-enforcement gate ticket | `wires-method:` frontmatter present OR `blocked-by: 128` |
| `crafting-economy` (proposed) | 016 | manual — no auto-classifier signal yet |

**Verdict cadence:** block-level at the `verdict-anchor: true` ticket. Other tickets in the block land verdict-skipped (the substrate is partially-assembled and can't produce useful signal). The anchor's landing triggers the block-level verdict recipe (each anchor authors its own — generic `just verdict` doesn't answer "did the new substrate fire? did legs stay orthogonal?").

**Orthogonality precondition:** `verdict-anchor: true` is an *assertion* that the block's legs are designed orthogonally (per CLAUDE.md's "richer perception, better strategy" pillar — orthogonal axes, no single dominant scalar). If a block accidentally violates orthogonality, the safety property breaks and you must fall back to per-ticket cadence for that block. The anchor's authoring is a structural decision; the auto-classifier never picks anchors.

**Sessions:** long-lived. May span multiple context windows via `/handoff` artifacts. The session's commit stream lands the block's legs incrementally; only the anchor's land fires the block-level verdict.

**Per-block verdict pattern:** the anchor ticket scopes its own block-verdict recipe under `just block-verdict <initiative-id>` (currently a stub — each block authors its own composition of `just verdict` + block-specific Feature-fired checks + welfare deltas). Document the pattern in the anchor ticket's `## Verification` section so future readers see the signal shape.

### swarm-safe (the fast track)

Use for: docs, frontmatter migrations, mechanical refactors, atomic bugfixes with already-verified layer-walks, sweep-runner work, template adoption.

**Verdict cadence:** per-ticket. The refinery `--auto` flag (Stage 2; not yet implemented) lands these in batches when verdict-pass + no-conflict. The whitelist is **in code**, not by convention — `scripts/refinery.sh` refuses `--auto` on anything other than `swarm-safe`.

**Sessions:** ephemeral polecats. One ticket per session, push and exit. The master foreman auto-lands their bookmarks via `just refinery --auto` (whitelisted in code to track==swarm-safe).

## Stage 2 — polecats (master-orchestrator)

`/foreman` is the master-orchestrator entry. It spawns headless child `claude` CLI processes (polecats) against pre-created swarm-safe workspaces, watches them, and auto-lands their bookmarks via `just refinery --auto` when they exit.

### Mental model

The master Claude session at `~/clowder` is the foreman. Each polecat is a `claude -p` subprocess in its own jj workspace under `~/clowder-sessions/swarmpole-<id>/`. The foreman:

1. **Picks** the top ready swarm-safe ticket(s).
2. **Claims + workspaces** via `just session-new swarmpole-<id> --tickets <id> --track swarm-safe` (atomic flock-gated claim; reuses Stage 1 primitive).
3. **Spawns** the polecat: `claude -p --output-format stream-json --permission-mode bypassPermissions --model sonnet --name polecat-<slug> --session-id $(uuidgen) --no-session-persistence`. Output streamed to `~/clowder-sessions/<slug>/.polecat-stream.jsonl`.
4. **Wall-clock sentinel**: a background sleeper that SIGTERMs the polecat if it's still alive after the deadline (default 30m; macOS doesn't ship `timeout(1)`, so this is the portable workaround).
5. **Poll loop**: every 30s checks if all polecats have exited; when they have, runs `just refinery --auto` to land bookmarks that passed the gate.

The polecat is given a stricter prompt than `session-new --print-prompt` (which is for human sessions). It's instructed to:
- Never ask the user a question. If anything's ambiguous, abandon and log via `/agent-feedback`, exit without pushing.
- Run `just check && just test` before pushing. If they fail, abandon — never commit broken state.
- Exit ceremony: `just check && just test` → `jj describe -m "..."` → `just land <id>` → `jj git push --bookmark session/<slug> --allow-new` → print `polecat-done: <slug>` and exit.

### The `[foreman]` recipe group

| Recipe | What it does |
|---|---|
| `just foreman` | Report polecats + ready queue (default) |
| `just foreman --json` | Machine-readable for `/foreman` skill |
| `just foreman-spawn N` | Spawn N polecats (default N=3, wallclock 30m); enters auto-poll-and-land loop |
| `just foreman-spawn N --dry-run` | Plans without spawning; rolls back its session-new claims |
| `just foreman-watch` | One-shot heartbeat — alive/exited, last-edit, deadline-remaining |
| `just foreman-log <slug>` | `tail -f` the polecat's stream-json |
| `just foreman-shutdown [--hard]` | SIGTERM (or SIGKILL with `--hard`) every tracked polecat |

Discover via `just --list | grep '\[foreman\]'`.

### Spawn → watch → land cycle

```text
just foreman-spawn 3
  ├─ pick 3 ready swarm-safe tickets (avoiding already-claimed)
  ├─ for each: just session-new swarmpole-<id> --tickets <id> --track swarm-safe
  │  └─ atomically claims, creates ~/clowder-sessions/swarmpole-<id>/, sets session/swarmpole-<id>
  ├─ spawn claude -p subprocess (PID → .polecat.pid; stream → .polecat-stream.jsonl)
  ├─ spawn wallclock sentinel (PID → .polecat-watchdog.pid; SIGTERMs after Mm)
  └─ enter poll loop:
       while any polecat alive:
         sleep 30s
       just refinery --auto       ← drains the queue: gate = working-copy clean + just check && just test
       for each polecat that didn't push:
         archive_abandoned_polecat ← cp stream/stderr/cmdline + REASON to logs/polecat-abandoned/<stamp>-<slug>/
         session_done.sh --force  ← releases the ticket claim back to ready (now safe — artifacts archived)
```

### Artifacts (per polecat, in `~/clowder-sessions/<slug>/`)

| File | Written by | Use |
|---|---|---|
| `.session-info.json` | `session_new.sh` | slug · track · tickets · bookmark · created_at |
| `.polecat.pid` | `foreman.sh::spawn_one_polecat` | PID of the `claude` child |
| `.polecat-watchdog.pid` | same | PID of the wallclock sentinel |
| `.polecat-stream.jsonl` | `claude --output-format stream-json` | full structured stream |
| `.polecat-stderr.log` | shell redirect | stderr from `claude` |
| `.polecat-cmdline` | `foreman.sh` | exact invocation (post-mortem) |
| `.polecat-prompt` | `foreman.sh` | the prompt sent to `claude` |
| `.polecat-deadline` | `foreman.sh` | UNIX timestamp when wallclock fires |
| `.polecat-exit` | spawn subshell | exit code on natural termination |
| `.refinery-gate.log` | `refinery.sh::auto_gate` | `just check && just test` output if the gate ran |

After a successful land, the workspace is removed via `session_done.sh --no-release`. After an abandoned/dead polecat, the foreman first copies the diagnostic artifacts to `logs/polecat-abandoned/<YYYYMMDD-HHMMSS>-<slug>/` (with a one-line `REASON` file extracted from the polecat's final `polecat-abandoned: <slug> <reason>` stdout), THEN runs `session_done.sh --force` to remove the workspace AND release the ticket-claim back to `ready`. Canonical abandon reasons (from the prompt's verifiability-triage block): `requires-gui` · `requires-long-soak` · `requires-substrate-judgment`.

### `refinery --auto` (the gate)

`refinery --auto` is the lander that foreman invokes after its polecats exit. Per session bookmark:

1. **Whitelist** — track must be `swarm-safe` (enforced at flag parse AND inside the loop; substrate-sensitive + coherent-block always require explicit `--land <slug>`).
2. **Fast-forward** — bookmark must be ahead of main with no commits behind. Conflicts → never auto-landed.
3. **Working-copy clean** — `jj status` in the workspace shows no `[AM]` edits.
4. **Gate** — `cd <workspace> && just check && just test` exits 0 for both.
5. **Land** — reuse the existing `land()` (rebase if needed → `jj bookmark set main` → forget bookmark → `session_done.sh --no-release`).

`--dry-run` runs steps 1-4 but skips landing. Outcomes per row: `landed` / `gate-pass` (dry-run) / `wrong-track` / `not-fast-forward` / `dirty-working-copy` / `check-fail` / `test-fail` / `no-changes` / `no-workspace` / `land-failed`.

### Recovering a stuck polecat

A "stuck" polecat is alive (PID up) but its bookmark hasn't advanced past main for >20min. Indicators in `just foreman-watch`: `alive` + `last-edit` growing large + `deadline-in` shrinking.

Options:
1. **Wait** — the wallclock sentinel will SIGTERM at the deadline.
2. **Tail the stream** — `just foreman-log <slug>` to see what `claude` is doing.
3. **Shutdown** — `just foreman-shutdown` (SIGTERM, gives `/handoff` a chance) or `just foreman-shutdown --hard` (SIGKILL, drops mid-flight state).
4. **Drain** — after shutdown, `just refinery --auto` lands any bookmarks that managed to push before death; failed ones get `session_done.sh --force` to release their ticket claims.

A "dead" polecat (PID gone) that didn't push its bookmark is handled automatically by the foreman's poll loop — it releases the ticket back to `ready` via `session_done.sh --force` after the `refinery --auto` step.

## Frontmatter invariants (enforced by `just check`)

```yaml
orchestration: <track>            # required on every active ticket
block: <initiative-id>            # required iff coherent-block
verdict-anchor: true              # optional, ≤1 per block
```

The enforcement script (`scripts/check_orchestration_frontmatter.py`) validates four invariants:

1. `orchestration:` present + one of substrate-sensitive | coherent-block | swarm-safe
2. `coherent-block` ⇒ `block:` present AND `block:` value appears in `initiative:` list
3. ≤1 `verdict-anchor: true` per `block:` value
4. `swarm-safe` ⇒ no `block:`, no `verdict-anchor:`

Run `just check` after editing any ticket frontmatter. Run `just retag-audit` for a corpus-wide rollup view.

## Discovering recipes

`just --list | grep '\[<tag>\]'` filters by recipe group:

```
just --list | grep '\[session\]'      # session lifecycle
just --list | grep '\[refinery\]'     # landing
just --list | grep '\[retag\]'        # corpus tagging
just --list | grep '\[block\]'        # block management
just --list | grep '\[ticket-query\]' # ticket introspection
```

The tag prefix is the durable handle — every new recipe in this subsystem carries one in its doc-comment.

## Troubleshooting

**"refinery: needs-rebase"**
The session diverged from main while you were working. Land path will attempt the rebase automatically. If it conflicts, resolve in the session's workspace (`cd ~/clowder-sessions/<slug>`), commit, push, retry `just refinery --land <slug>`.

**"session-new: ticket X is already in-progress"**
Another session has claimed this ticket. Check `just sessions` to find the holder; either wait for them to finish + land, or abandon their session (`just session-done <slug>`) to release the claim. Atomic claim prevents two sessions racing the same ticket.

**"orchestration-frontmatter: ... missing 'orchestration:'"**
A ticket is missing the field. Run `just retag-init` to backfill the default on all untagged tickets.

**"block 'foo' has 2 verdict-anchor:true tickets"**
Two tickets in the same block claim to be the verdict-anchor. Decide which one is canonical and unset the other: `just retag <id> --unset-anchor`.

**Disk pressure (sccache + workspace targets)**
Sessions share rustc unit-cache via `sccache` (configured in `~/.cargo/config.toml`). Each session has its own `target/`; `just session-done` runs `cargo clean` to reclaim. Monitor disk with `just sessions --disk` (shows per-session target sizes). If disk is binding, increase `SCCACHE_CACHE_SIZE` in the cargo config or reduce parallel-N.

**jj workspace stale**
If `jj status` reports "working copy is stale" in a session workspace, run `jj workspace update-stale`. Doesn't usually happen — sessions only contend on op-log writes, not working copies.

## Reference

- Ticket: [`docs/open-work/tickets/354-parallel-session-orchestration-work-skill-three-track-partition-refinery.md`](../open-work/tickets/354-parallel-session-orchestration-work-skill-three-track-partition-refinery.md)
- Plan: `~/.claude/plans/this-is-not-an-curried-hippo.md`
- Skills: [`.claude/skills/work/SKILL.md`](../../.claude/skills/work/SKILL.md), [`.claude/skills/retag/SKILL.md`](../../.claude/skills/retag/SKILL.md)
- CLAUDE.md addendum: §"Long-horizon coordination" / "Parallel-session orchestration"
- Heuristic classifier rules: `scripts/retag_suggest.py` docstring
