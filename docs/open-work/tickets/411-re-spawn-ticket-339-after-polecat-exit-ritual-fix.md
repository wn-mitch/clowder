---
id: 411
title: re-spawn ticket 339 after polecat exit-ritual fix
status: blocked
cluster: process-discipline
orchestration: substrate-sensitive
initiative: []
added: 2026-05-18
parked: null
blocked-by: [409]
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket 339's work was lost in the 2026-05-17 `/foreman` 8-polecat batch — the polecat reported `polecat-done` but never pushed its bookmark to origin, and foreman's `--force` workspace teardown nuked the uncommitted work (~30min + ~$2). Ticket 409 ships the fix (R3 fetch + R7 single-writer main + R4 polecat self-verifies push) that makes this failure mode either impossible (R7) or loud (R4 turns silent-done into explicit-abandon). Once 409 lands, re-spawn 339 with the same scope; it should now either land cleanly via the auto-loop or surface a `polecat-abandoned: <slug> push-failed` line that foreman can archive instead of lose.

## Scope

- Identify 339's current frontmatter state (status, blocked-by) and reset it to `ready` if it was released back to ready by session-done --force.
- Re-claim 339 via `/foreman` (next available swarm-safe batch, N≥1) once 409 is on main.
- Verify post-batch: local main is linear (no conflicted heads); 339 is in `landed/`; refinery's per-ticket landing commit pair exists on main.

## Out of scope

- Other lost / marooned work from the 2026-05-17 batch — those (337, 353, 363) were recovered manually; only 339 remained genuinely lost.
- Sensitivity-map rebuild (350) — separate ticket with its own wallclock requirement.

## Current state

Blocked on 409. 339 itself is unmodified since its pre-batch state.

## Approach

After 409 lands and `/foreman` is exercised cleanly once for verification, queue 339 in the next swarm-safe batch.

## Verification

- 339 transitions ready → in-progress → done within one `/foreman` invocation.
- Local main remains linear (`jj log -r main --no-graph -T 'change_id ++ "\n"' | head -5` shows a chain, not a conflict marker).
- 339's landed file carries `landed-at:` populated with the polecat's feature-commit sha, plus a separate `docs: backfill` commit from refinery (the R7 two-commit landing pair).

## Log

- 2026-05-18: opened after 409 to re-queue lost work from the 2026-05-17 silent-failure batch.
