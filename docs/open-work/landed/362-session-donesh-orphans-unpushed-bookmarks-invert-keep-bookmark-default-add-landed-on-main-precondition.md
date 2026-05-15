---
id: 362
title: session_done.sh orphans unpushed bookmarks — invert --keep-bookmark default + add landed-on-main precondition
status: done
cluster: process-discipline
orchestration: swarm-safe
initiative: []
added: 2026-05-15
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: caa8e49b8beb
landed-on: 2026-05-15
---

## Why

`scripts/session_done.sh` line 109 forgets a workspace's jj bookmark
**by default** with no precondition that the bookmark's tip has been
pushed to remote or merged into `main`. Combined with `--force`
(line 47-60), which bypasses the already-weak safety check (only
inspects uncommitted *working-copy* edits, never committed-but-unpushed
work), polecats that exit before `jj git push --bookmark` completes
have their commits orphaned and eventually GC'd.

**Observed incident (the surfacing run).** Tickets #332 and #333 were
landed twice (orphans `f3c72a06` and `00aa3636`) with substrate work
(24 files, 663 insertions including a new `Mourning` Component, real
witness-typed resolvers, method-flips to Live, new `TargetHint` variants,
new `Feature::*` variants). Both attempts were orphaned. The eventual
`land:` commits on main (`771f7594` and `68c18c1c`) touched only
`docs/open-work.md` + the ticket file moves — **2 files each, no
source files**. Discovered while planning #357 (the dispatch follow-on
that depends on the substrate), surfaced via `git log --all --oneline |
grep -E '332|333|grief|mourn|kitten.rearing'` showing the
`wip: ... (recovered from crashed session)` + `feat:` orphan pattern
repeated across multiple polecat retries.

**Workflow canary violated:** "polecat substrate work lands or is
preserved for recovery." Today neither holds — the workspace cleanup
unconditionally destroys both the workspace AND the bookmark
reachability.

**Compounding factor (separate but related).** #332 / #333 carry
`orchestration: coherent-block`. Per CLAUDE.md *"Polecat-eligibility is
**swarm-safe only**, enforced in three places: /foreman skill refuses
other tracks, scripts/foreman.sh only picks from the swarm-safe ready
queue, scripts/refinery.sh --auto rejects non-swarm-safe rows"*. One
of those three enforcement points failed — coherent-block tickets
ended up being polecat-worked, which amplifies the orphaning risk
because coherent-block substrate authoring doesn't fit the 30m
wallclock cap. **Scope of this ticket: bookmark-orphaning. The track
enforcement gap is a sibling defect — out of scope here.**

## Current architecture (workflow-pipeline audit)

Workflow-shape bug, not AI-pipeline. Audit walks the polecat lifecycle
from spawn to cleanup. Each row marked `[verified-defect]` is read
from the live scripts.

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Polecat spawn | `scripts/foreman.sh` | Polecat is spawned against pre-created workspace; bookmark `session/<slug>` is created and tracks the working-copy commit. | `[verified-correct]` |
| Work authorship | (claude CLI inside workspace) | Polecat makes commits on the `session/<slug>` bookmark; advancing it as it works. | `[verified-correct]` |
| Wallclock cap | wallclock-sentinel subprocess | macOS lacks `timeout(1)`; 30m cap kills the polecat with `SIGKILL` if work outruns the cap. Polecat may be mid-`jj git push` when killed. | `[verified-correct]` |
| Bookmark push | `jj git push --bookmark session/<slug>` | Polecat pushes bookmark to remote before exiting via `/handoff`. **If killed mid-push or before reaching push, the bookmark exists only locally.** | `[verified-defect-trigger]` |
| Foreman auto-poll | `scripts/foreman.sh` poll loop | Every 30s checks PIDs; when all polecats exit, runs `just refinery --auto`. **No check that each dead polecat's bookmark made it to remote.** | `[verified-defect]` |
| Refinery --auto gate | `scripts/refinery.sh --auto` | Reads remote bookmarks. **A locally-only bookmark is invisible to refinery — it can't be landed and can't be flagged as "needs human attention".** | `[verified-defect]` |
| Cleanup precondition | `scripts/session_done.sh:47-60` | Safety check only inspects uncommitted **working-copy** edits (`jj status \| grep '^[AM] '`). Committed-but-unpushed work passes the check without complaint. | `[verified-defect]` |
| Bookmark forget | `scripts/session_done.sh:108-110` | `if [[ "$keep_bookmark" != "true" ]]; then jj bookmark forget "session/$slug" \|\| true; fi`. **Default is to forget. No precondition that the bookmark's tip is on main or remote.** | `[verified-defect]` |
| Workspace forget | `scripts/session_done.sh:99-104` | `jj workspace forget <slug>` + `rm -rf $workspace`. Workspace's local-only commits become unreachable when the bookmark is also forgotten. | `[verified-defect]` |
| jj GC | (jj internals) | Unreachable commits (no bookmark, no workspace, no ancestor of any branch) are eligible for garbage collection. The orphans `f3c72a06` and `00aa3636` are reachable today only via reflog + `--all` log; will be GC'd eventually. | `[verified-correct]` |

## Fix candidates

**Parameter-level options** (small flag flips, predicate additions):

- **R1 (refuse forget on unlanded bookmark)** — extend the safety check
  in `session_done.sh:47-60` to also reject if `session/<slug>`'s tip is
  not an ancestor of `origin/main`. Implementation: `jj log -r
  "session/<slug>..@" --no-graph` returns non-empty iff there are
  commits on the bookmark not on remote main. Refuse without `--force`
  AND a new `--orphan-ok` flag (two-flag gate; `--force` alone is not
  enough).

- **R2 (invert --keep-bookmark default)** — flip the default behavior
  so bookmark-forget requires explicit opt-in (`--forget-bookmark` or
  `--orphan-ok`). Keep-bookmark becomes the safe path; users (and
  foreman cleanup) get the safe behavior without thinking about it.
  Old `--keep-bookmark` flag becomes a no-op (kept for back-compat).

- **R3 (verify-push-before-cleanup hook in foreman.sh)** — before
  foreman calls `session_done.sh --force` on a dead polecat, run a
  `jj git push --bookmark session/<slug> --allow-new` to push any
  unpushed commits. If push fails (no remote tracking, or commit not
  acceptable), surface the bookmark for human triage instead of
  cleaning up.

**Structural options** (at least one MUST be drafted per CLAUDE.md
bugfix discipline):

- **R4 (split — orphan-rescue command)** — add `just orphan-scan`
  recipe that walks the jj op log for unreachable feat-commits matching
  active ticket numbers (regex on commit message). Surfaces orphans
  with a hint to `jj duplicate <hex> -d main` to rescue. Runs daily as
  part of the foreman lifecycle. **Splits the "what to do about
  orphans we already created" problem off from "stop creating orphans"
  — the latter is the prevention path (R1+R2), this is the recovery
  path.**

- **R5 (extend — push-before-forget contract in session_done.sh)** —
  rather than just refusing forget, attempt to push the bookmark first,
  flagged with a new `--push-before-forget` default. If push succeeds,
  the bookmark is preserved on remote and the local forget is safe. If
  push fails (e.g., diverged from main, needs rebase), refuse the
  cleanup and surface the workspace for human attention. **Extends
  session_done.sh's responsibility from "cleanup local state" to
  "ensure remote durability before destroying local state".**

- **R6 (rebind — bookmark namespace separation)** — move polecat
  bookmarks from `session/<slug>` to `polecat/<slug>` with a stricter
  retention policy (never auto-forgotten). The foreman lifecycle moves
  to landing-or-handoff explicit, no auto-cleanup. **Rebinds the
  problem from "is this polecat done?" (which the current default
  answers wrongly) to "is this polecat's work on main?" (which is
  observable).**

- **R7 (retire — delete --force flag for bookmark-forget path)** —
  `--force` retains for clearing dirty working-copies but no longer
  authorizes bookmark-forget. Bookmark-forget always requires the
  bookmark's tip be on main. **Retires the unsafe-by-default behavior
  entirely; `--force` keeps its other semantics.**

## Recommended direction

**R1 + R2 + R4** as a bundle.

- **R1** is the load-bearing precondition fix — it makes the orphaning
  path impossible by default. The two-flag gate (`--force --orphan-ok`)
  preserves an escape hatch for genuine cleanup-of-known-junk while
  blocking the silent-orphan case.

- **R2** flips the default to safe — `--keep-bookmark` becomes the
  implicit behavior. The foreman's `--force` cleanup path gains the new
  `--orphan-ok` only when surfacing a known-dead workspace whose work
  has already been pushed elsewhere.

- **R4** (`just orphan-scan`) ships alongside as the recovery path —
  not just for future orphans but to catalogue existing ones now. The
  scan output goes into a triage doc the operator can walk to either
  rescue (via `jj duplicate`) or explicitly mark abandoned.

Rejected:
- **R3** (push-before-cleanup in foreman): too coupled — pushing a
  half-done polecat's bookmark may push broken substrate. R1's "refuse
  cleanup" is the right shape because it stops the destruction without
  inferring intent.
- **R5** (push-before-forget in session_done): same concern as R3 — the
  cleanup script shouldn't decide remote durability. Refusing is safer
  than retrying.
- **R6** (namespace separation): scope is bigger than the bug warrants.
  The bug isn't "session bookmarks are misnamed"; it's "session
  bookmarks are forgotten unsafely". R1+R2 fix the unsafety without
  re-spelling the bookmark name.
- **R7** (retire --force for forget): too restrictive — `--force` is
  legitimately needed for genuine dirty-cleanup of abandoned workspaces
  whose work was already rescued. R1's two-flag gate preserves the
  escape hatch with explicit intent.

## Out of scope

- **Polecat-track-enforcement gap.** #332 / #333 carry
  `orchestration: coherent-block` but ended up polecat-worked. One of
  the three CLAUDE.md enforcement points (`/foreman` skill,
  `scripts/foreman.sh`, `scripts/refinery.sh --auto`) is broken or
  bypassed. **Separate ticket** — open as follow-on after this lands.
- Wallclock-cap sizing for coherent-block work — 30m is undersized.
  Coherent-block tickets shouldn't be polecat-eligible anyway (per the
  out-of-scope item above), so this is downstream of the
  track-enforcement fix.
- Existing orphan rescue (357 cherry-picks `00aa3636` directly as part
  of its substrate recovery). The `just orphan-scan` recipe (R4) covers
  *future* orphans plus retroactive cataloguing.

## Verification

- `bash -n scripts/session_done.sh` passes.
- `just check` passes (no new lint warnings).
- Unit test for the bookmark-on-main precondition: spin up a fixture
  workspace with a committed-but-unpushed bookmark, invoke
  `session_done.sh <slug>` (no flags), assert exit code != 0 and the
  bookmark still exists.
- Unit test for the new `--orphan-ok` flag: same fixture, invoke
  `session_done.sh <slug> --force --orphan-ok`, assert exit code == 0
  and the bookmark is forgotten.
- Smoke test for `just orphan-scan`: run against the current repo;
  assert it surfaces `f3c72a06` and `00aa3636` (the known existing
  orphans from #332/#333).

## Log

- 2026-05-15: opened. Surfacing run: planning #357 (HTN-driven action
  dispatch) revealed the substrate from #332/#333 was orphaned twice
  by this defect. Forensic trail in the #357 plan file and the chat
  thread.
