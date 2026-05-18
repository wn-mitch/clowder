---
id: 409
title: polecat exit-ritual + main-serialization: 3 silent failures + conflicted main from concurrent landings
status: done
cluster: process-discipline
orchestration: substrate-sensitive
initiative: []
added: 2026-05-18
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: b4a0f61c837f
landed-on: 2026-05-18
---

## Why

A single 8-polecat `/foreman` invocation produced **three silent failures** and a **conflicted local `main` bookmark** that required manual jj surgery to untangle. Of the eight polecats spawned across two batches (3+5), only two landed cleanly through the auto-loop; three reported `polecat-done` but their work was either never pushed (workspace nuked, ~30min + ~$2 each lost) or pushed but invisible to refinery (work survived on origin but main moved through divergent heads). The orchestration system gave no in-loop signal that anything was wrong — the operator surface (`/work`) and the auto-poll-and-land loop both reported success.

This violates a load-bearing invariant of the polecat track: the **headless contract** — operator MUST be able to trust that "polecat exited cleanly" means "work landed or polecat explicitly abandoned." When that contract holds, polecats are cheap; when it doesn't, every batch needs a post-mortem and the system burns money in the dark.

## Hot context

Concrete failure ledger from the 2026-05-17 `/foreman` invocation:

| Ticket | Polecat outcome | Push state | Real outcome | Cost |
|---|---|---|---|---|
| 307 | reported done, landed via refinery | ✓ on origin | landed cleanly | — |
| 338 | reported done, landed via refinery | ✓ on origin | landed cleanly | — |
| 337 run 1 | reported `polecat-done`, exited | ✗ never pushed | **work lost** (~30min, ~$2) | wallclock + compute |
| 337 run 2 | reported `polecat-done`, exited | ✓ on origin (a2fe3b2d) | refinery saw "no commits ahead of main" — work marooned, ticket released to ready, bookmark forgotten by my manual `refinery --land` | — work recovered via manual jj surgery |
| 339 | reported `polecat-done`, exited | ✗ never pushed | **work lost** (~30min, ~$2) | wallclock + compute |
| 350 | clean abandon: `requires-long-soak` | n/a | expected — sensitivity-map rebuild needs >30min | — |
| 353 | reported `polecat-done`, exited | ✓ on origin (50714222 + duplicate 7d19e170) | one of two pushed commits became a main head; refinery missed both; recovered manually | — |
| 363 | reported `polecat-done`, exited | ✓ on origin (8591658c) | recovered manually | — |

End-state before recovery: local `main` was conflicted with **7 divergent positions** across 4 logical commits; 3 of those were duplicates of 338 / 353 from concurrent `just land` calls in different polecat workspaces racing on the shared bookmark.

**Friction breadcrumbs** logged at `logs/agent-friction.jsonl`:
- `foreman --wallclock 30m` (with `m` suffix) parses as bash arithmetic and fails — SKILL.md said `30m`, script wants `30`. Major.
- `polecat-337` silent failure stream-json analysis (blocker).

Recovery path took ~45 minutes of manual `jj duplicate --onto main` + `just open-work-index` + `EDITOR=true jj squash --use-destination-message` per ticket. Linear chain rebuilt; tests green; pushed.

## Current architecture (orchestration-walk — adapted from AI layer-walk)

The bugfix template's L1/L2/L3 columns are AI-substrate-shaped and don't fit a tooling bug. Walking the orchestration pipeline instead:

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Polecat exit ritual | `scripts/foreman.sh::compose_polecat_prompt` (heredoc prompt) | Polecat is instructed to print `polecat-done: <slug> ticket-<id>` after work, but the prompt does NOT enforce "verify your bookmark advanced past main before printing done." Polecat-337 (run 1) and -339 both printed `polecat-done` with empty bookmarks. | `[verified-defect]` |
| Polecat push verification | `scripts/foreman.sh::wait_loop` | Foreman watches PIDs only; doesn't sample `session/<slug>` bookmark position vs main. A polecat that exited 0 with no push is indistinguishable from a successful one. | `[verified-defect]` |
| Refinery freshness | `scripts/refinery.sh::list_session_bookmarks` + `jj log -r main..bookmarks(...)` | Refinery reads local jj bookmark state without `jj git fetch` first. If a polecat's push reached origin but the master workspace's local view is stale (concurrent op-log writes, etc.), refinery sees 0 commits ahead and reports `no-changes`. | `[verified-defect]` |
| Refinery conflicted-main handling | `scripts/refinery.sh::ahead/behind counts` | `jj log -r "main..bookmarks(\"$bm\")"` is ambiguous when `main` is conflicted with multiple heads. Refinery silently picks one head; results are non-deterministic. | `[suspect]` |
| Main-bookmark serialization | `scripts/refinery.sh::land_one` (and parallel `just land` calls inside polecat workspaces) | No serialization on advancing `main`. Three polecats running `just land` concurrently each produced a child of the same parent, then each tried to advance `main` → divergent heads → conflict. | `[verified-defect]` |
| Foreman abandon parsing | `scripts/foreman.sh::detect_done_line` | Foreman only looks for `polecat-abandoned:` lines to detect graceful abandonment. `polecat-done` + empty bookmark + dead PID is parsed identically to "crashed silent" → claim released, workspace nuked, work lost. | `[verified-defect]` |
| Polecat workspace teardown | `scripts/session_done.sh` | When called with `--force` after foreman gives up, removes ALL workspace contents including any uncommitted polecat work. No reflog / staging area. | `[verified-correct]` (this is by design but enables the lossy failure mode) |

## Fix candidates

**Parameter-level options:**

- **R1** — Update `/foreman` SKILL.md to pass `--wallclock 30` not `30m`. (Trivial; one-line.)
- **R2** — Strip trailing `m` in `scripts/foreman.sh::parse_wallclock_arg` before arithmetic. (Also trivial; complementary to R1.)
- **R3** — Refinery `jj git fetch` before listing bookmarks. Adds 1-3s latency per `refinery --auto` call but eliminates the stale-bookmark-view class entirely.

**Structural options** (one MUST be drafted, even if not chosen):

- **R4 (extend)** — Extend the polecat exit ritual: after `jj git push --bookmark session/<slug>`, the polecat MUST verify `jj log -r "main..session/<slug>" --no-graph` returns ≥1 commit AND `jj git push --dry-run` reports "already pushed" / nothing-to-push. ONLY THEN print `polecat-done`. If verification fails, print `polecat-abandoned: <slug> push-failed` instead, archive workspace artifacts via session-done. This collapses "silent-done" to "explicit-abandon" — the operator-visible signal becomes truthful again.

- **R5 (split)** — Split refinery's status enum: `landed` / `no-changes-on-bookmark` / **`work-claimed-by-polecat-but-bookmark-stale`** (new). The third state fires `jj git fetch` and re-checks; if still stale after fetch, halts the batch and surfaces "manual inspection required" rather than silently releasing the claim. This makes refinery's silent-failure mode loud.

- **R6 (split)** — Split the main-bookmark advance into two phases: polecats push only to `session/<slug>`, NEVER advance `main` from inside a polecat workspace. The master orchestrator runs `refinery --auto` after ALL polecats have exited (existing behavior) AND holds an exclusive flock on a `main.lock` file during the multi-bookmark advance. This serializes the only conflict-producing operation.

- **R7 (rebind)** — Reroute the polecat prompt: instead of running `just land` inside the polecat workspace (which advances `main` locally and races with siblings), the polecat ONLY commits + pushes its work to `session/<slug>` and prints `polecat-done`. `just land` for the ticket runs in the master workspace as part of `refinery --auto` (which already does the fast-forward). This eliminates the cross-workspace `main` race by construction.

## Recommended direction

**R3 + R4 + R7**, sequenced:

1. R3 (refinery fetch) — cheapest, immediately makes the recovered-work case visible
2. R7 (move `just land` to master) — eliminates concurrent main advance entirely; this is the structural fix
3. R4 (polecat self-verifies push) — defense in depth so silent-failures become explicit abandons even if R7 fails to deploy fully

R5 is good but redundant if R3+R7 ship. R6 is a smaller version of R7. R1+R2 are independent one-liners and should ship in any commit that touches the area.

The structural candidate (R7) wins because the parameter-level fixes (R3 alone) leave the race condition in place — they just make the failure mode louder. The orchestration system needs the property "only one writer ever advances main per batch"; right now N polecats each think they're the writer.

## Out of scope

- **Lost work recovery** (339, 337-run-1): not recoverable; opening 410 to re-spawn 339 after R3+R4 ship.
- **Sensitivity-map rebuild (350)**: clean abandon, expected — needs `--wallclock 240` minimum or a different orchestration path. Track in 350's own ticket; this fix doesn't address it.
- **Backfill of `landed-at` for 307/338/353/363/337**: completed manually as part of recovery; commit `7769037c` carries the correction.

## Verification

- After R3 ships: re-run `/foreman` against a known-clean swarm-safe ticket; verify refinery reports `landed` (not `no-changes`) when work was pushed.
- After R7 ships: spawn 3+ polecats simultaneously against unrelated swarm-safe tickets; verify local `main` is never conflicted post-batch.
- After R4 ships: artificially break `jj git push` inside a polecat (e.g., revoke origin push perms mid-flight); verify polecat prints `polecat-abandoned: <slug> push-failed` and foreman archives to `logs/polecat-abandoned/`.

## Log

- 2026-05-18: opened after `/work` invocation surfaced 3 silent failures + conflicted main. Manual recovery via `jj duplicate --onto main` chain landed 307/338/353/363/337 cleanly; `logs/agent-friction.jsonl` has two breadcrumbs (foreman wallclock major, polecat silent-failure blocker). 339 work genuinely lost.
