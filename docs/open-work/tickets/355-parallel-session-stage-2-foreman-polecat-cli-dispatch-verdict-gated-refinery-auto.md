---
id: 355
title: Parallel-session Stage 2: /foreman + polecat CLI dispatch + verdict-gated refinery --auto
status: ready
cluster: tooling-diagnostics-ui
orchestration: substrate-sensitive
initiative: []
added: 2026-05-15
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Stage 1 of 354 shipped the three-track partition + primitive recipes + `/work` and `/retag` skills. The remaining piece named in 354's "Out of scope" is the master-orchestrator pattern: a way for the master Claude session at `~/clowder` to spawn child `claude` CLI processes against pre-created swarm-safe workspaces, watch them, and auto-land the bookmarks they push. Without it, the parallel-session system requires a human to copy-paste a starter prompt into a new terminal for every session, defeating the throughput goal. Stage 2 closes that loop — but conservatively: polecats run swarm-safe-only (the safest track; layer-walks already verified, no balance impact), default N=3, wall-clock cap per child, and `refinery --auto` is whitelisted in code to swarm-safe rows (never substrate-sensitive or coherent-block, regardless of operator flag).

## Scope

- **`scripts/refinery.sh --auto`** (currently refuses with "verdict integration pending" at lines 40-42): real implementation. Per-bookmark filter to track==swarm-safe (whitelist enforced at flag parse AND inside the loop — two layers). Gate: working-copy clean + `just check && just test` exit-0 in the workspace. Reuse existing `land()` for the rebase + bookmark advance + cleanup. Per-row JSON outcome. `--dry-run` runs the gate but skips landing.
- **`scripts/foreman.sh`**: NEW. Modes — default report, `--spawn N [--wallclock M] [--dry-run]`, `--watch`, `--land`, `--shutdown [--hard]`, `--log <slug>`. Spawn loop: pick top ready swarm-safe ticket → `just session-new swarmpole-<id>` (composes the atomic-claim) → spawn `claude -p --output-format stream-json --include-partial-messages --permission-mode bypassPermissions --model sonnet --name polecat-<slug> --session-id $(uuidgen) --no-session-persistence` via `nohup` + `timeout`. Record PID, command line, stream-json output under `~/clowder-sessions/<slug>/`. Tail-of-spawn poll loop: every 30s check liveness; when all polecats are dead, run `just refinery --auto` to drain.
- **Polecat prompt template** (heredoc in `foreman.sh`): headless contract — no interactive questions; if ambiguity surfaces, abandon and log "polecat-abandoned: <reason>" via `/agent-feedback`, exit without pushing the bookmark; exit ceremony non-optional (`/handoff` → `just check && just test` → `jj git push --bookmark session/<slug> --allow-new` → final `polecat-done: <slug>` line).
- **`[foreman]` justfile recipe group**: `foreman`, `foreman-spawn`, `foreman-watch`, `foreman-log`, `foreman-shutdown`. All carry `[foreman]` doc-comment for discovery.
- **`/foreman` skill** (`.claude/skills/foreman/SKILL.md`): conversational entry mirroring `/work` shape. AskUserQuestion menu (spawn / watch / drain / shutdown / log). Guardrails: refuse non-swarm-safe spawn (whitelist also in code), refuse if master has uncommitted edits on main, confirm N>5 explicitly.
- **Docs**: `docs/workflow/parallel-sessions.md` gets a "Stage 2 — polecats" section + flips the orchestration table's "Polecat-eligible" column to "yes". `CLAUDE.md` gets a one-line addendum noting wall-clock-only caps (subscription-billed; no dollar budget).

## Out of scope

- **GUPP-style heartbeat / scheduled-wakeup workers** — deferred to Stage 3+. The auto-poll-and-land loop is synchronous (blocks on the master session); cadence-based async refinery sweeps are a separate decision.
- **Witness revival** (auto-restart of crashed polecats) — deferred. A dead polecat releases its ticket-claim back to ready via `session_done.sh`; the master decides whether to respawn.
- **Multi-account budgeting** — N/A under Claude Code team subscription (no per-API-key dollar budget). If we ever move to API-key billing, `--max-budget-usd` is a one-line addition.
- **`refinery --auto` for coherent-block / substrate-sensitive** — the whitelist is in code, not policy. Coherent-block needs anchor-verdict; substrate-sensitive needs soak + `just verdict`. Both are explicitly outside Stage 2.
- **`open-ticket` auto-populating `orchestration: substrate-sensitive`** — a Stage 1.7 coverage gap surfaced while opening this ticket. The field had to be added by hand. Worth fixing in a follow-on (one-line edit to `scripts/open_ticket.py` or wherever the recipe lives); not in scope here.

## Current state

- Stage 1 of 354 landed across 9 commits on the local stack (1.1 frontmatter axis + retag-init, 1.2 retag primitives, 1.3 session lifecycle, 1.4 refinery, 1.5 block + ticket-query, 1.6 /work + /retag skills, 1.7 CLAUDE.md addendum + operator's guide). Stage 0 step 4 (retag ceremony) ran in 1.6 — 156 tickets tagged across 132 substrate-sensitive / 10 coherent-block / 14 swarm-safe.
- `just check` enforces the four orchestration invariants. `scripts/refinery.sh` exists; the manual `--land <slug>` path works. The whole system is operable end-to-end through `/work` minus the auto-land + polecat spawn paths.
- Plan: [`~/.claude/plans/mighty-foraging-biscuit.md`](file:///Users/will.mitchell/.claude/plans/mighty-foraging-biscuit.md) (Stage 2 design — read first).
- 354 itself is still `status: ready`; it'll flip to `done` when Stage 2 lands (it covers both stages by scope) or stay open if we want a clean Stage-1-landed signal first. Probably the latter — 354 lands first, 355 lands next.

## Approach

See the plan at `~/.claude/plans/mighty-foraging-biscuit.md`. Build order, one commit per stage:

1. Stage 2.1 — `refinery --auto` (the prerequisite for everything else; auto-land path).
2. Stage 2.2 — `scripts/foreman.sh` (uses 2.1's `--auto` in its poll loop).
3. Stage 2.3 — `[foreman]` justfile group.
4. Stage 2.4 — `/foreman` skill.
5. Stage 2.5 — docs + CLAUDE.md addendum.

Existing primitives reused (not reimplemented): `session_new.sh` atomic-claim + workspace creation; `session_done.sh` cleanup + ticket-claim release (default release path for failed polecats; `--no-release` for landed ones); `refinery.sh::land()` for the actual bookmark advance.

User decisions baked in:
- Auto-poll-and-land (foreman owns the full spawn → watch → land lifecycle).
- No dollar budget (subscription-billed; wall-clock via `timeout(1)` is the only cap).
- Foreman composes its own polecat prompt (stricter than `session-new --print-prompt`; that one's human-facing).

## Verification

1. `just check` green (all 10 lints pass + four orchestration invariants).
2. `just foreman` reports no polecats + lists swarm-safe ready queue.
3. `just foreman-spawn 1` creates `~/clowder-sessions/swarmpole-<id>/` with `.session-info.json` + `.polecat.pid`; ticket flips to `in-progress`; PID alive.
4. Polecat completes; bookmark pushed; foreman's poll loop runs `just refinery --auto`; bookmark forgotten, workspace cleaned, ticket flips to `done`.
5. Negative — `just refinery --auto --track substrate-sensitive` refuses with explicit "swarm-safe only" message.
6. Negative — polecat dies (SIGKILL); foreman flags it; `session_done.sh` releases ticket back to `ready`.
7. Negative — `just foreman-spawn 2` invoked twice in parallel terminals: second invocation picks different tickets or aborts cleanly (flock prevents double-claim).

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- · **363** (ready, process-discipline, score 0.92 (cross-cluster)) — polecat track-enforcement gap — coherent-block tickets reach polecat queue
- ✓ landed **362** (done, process-discipline, score 0.90 (cross-cluster)) — session_done.sh orphans unpushed bookmarks — invert --keep-bookmark default + a…
- ✓ landed **356** (done, tooling-diagnostics-ui, score 0.89) — foreman polecat-abandon archive + early-abandon triage (close the silent-waste…

<!-- linkages:end -->
## Log

- 2026-05-15: opened. Blocked on 354 landing; the Stage-1 commits are still on the local stack as of open-time. Plan at `~/.claude/plans/mighty-foraging-biscuit.md`.
- 2026-05-19: accuracy audit pass — 354 landed (2026-05-15); blocked-by empty and status ready; all related-work linkages (362/356/363) verified landed or ready
