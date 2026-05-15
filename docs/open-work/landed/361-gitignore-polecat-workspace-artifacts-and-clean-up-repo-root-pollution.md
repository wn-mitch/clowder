---
id: 361
title: Gitignore polecat workspace artifacts and clean up repo-root pollution
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
landed-at: e903d6a3cc8b
landed-on: 2026-05-15
---

## Why

The swarmpole-227 polecat's land commit (`c16921591746` —
"docs: document multi-focal soak-trace convention for marker-gated DSE
tuning") accidentally committed eight workspace-internal runtime artifacts
into main:

- `.polecat-cmdline`
- `.polecat-deadline`
- `.polecat-prompt`
- `.polecat-stderr.log`
- `.polecat-stream.jsonl`
- `.polecat-watchdog.pid`
- `.polecat.pid`
- `.session-info.json`

These are foreman-managed runtime state per
[`scripts/foreman.sh:31-43`](../../../scripts/foreman.sh) — per-workspace PID
files, the polecat's own captured stream, and the wallclock-sentinel deadline.
They have no business being in version control: they leak the polecat's
session-id, are stale the moment the polecat exits, and pollute every clone
of the repo (the stream JSONL alone was 1124 lines in 227's case).

The root cause: `.gitignore` has no entry covering `.polecat-*` or
`.session-info.json`. When the polecat ran `just land 227`, the staging
process swept these files up alongside the legitimate docs changes.

## Scope

- Add to `.gitignore`:
  - `.polecat-*`
  - `.session-info.json`
- Remove the eight committed artifacts from the master workspace.
- Commit the removal in the same change that adds the gitignore lines.

## Out of scope

- Rewriting the history of commit `c16921591746` to scrub the polecat
  artifacts. The damage is already pulled across every clone; a force-push to
  fix it costs more than leaving the bytes. Future polecats won't repeat the
  mistake once gitignore is in place.
- Auditing `scripts/foreman.sh` / `scripts/session_done.sh` for other classes
  of workspace-internal files that could leak. The eight named above are the
  observed set; extend the pattern if more surface.

## Current state

- 2026-05-15: discovered during the foreman dispatch that produced 227's
  landing commit.

## Approach

Single-file edit to `.gitignore` (append a section labeled `# polecat
workspace artifacts (foreman.sh)`) + `rm` the eight files + commit.

## Verification

- `git status` shows no `.polecat-*` or `.session-info.json` files after
  the commit.
- `just check` passes.
- Future polecat runs do not introduce these files into their landing
  commits.

## Log
- 2026-05-15: opened — surfaced by inspection of commit c16921591746.
