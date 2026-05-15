---
id: 356
title: foreman polecat-abandon archive + early-abandon triage (close the silent-waste loop)
status: done
cluster: tooling-diagnostics-ui
orchestration: substrate-sensitive
initiative: []
added: 2026-05-15
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: pending
landed-on: 2026-05-15
---

## Why
First live foreman run (3 polecats against the swarm-safe queue, 2026-05-15) returned 0 / 3 landed — all three abandoned cleanly per protocol, but `session_done.sh --force` `rm -rf`'d each workspace before anyone could read the `polecat-abandoned: <slug> <reason>` line in `.polecat-stream.jsonl`. Result: we know polecats are abandoning, but not *why*, so we can't tune the picker or the prompt to stop wasting model time on tickets that headless workers can't possibly verify.

Two coupled gaps:
1. **Diagnostic black hole.** The abandon reason is the polecat's last stdout line; it goes to `.polecat-stream.jsonl`; the workspace is deleted before harvest.
2. **Late-stage abandon.** The current prompt only enumerates substrate-judgment as a reason. Polecats running `tooling-diagnostics-ui` tickets (windowed UI overlay 208, log-viewer activation chart 259, soak-running frame-diff parity 227) burn 5-10 min of cargo builds + check/test before discovering the work needs GUI verification or a long soak — then abandon. The fail-fast signal should fire at prompt-read time, not after cargo.

## Scope
- `scripts/foreman.sh`: insert `archive_abandoned_polecat()` that runs *before* `session_done.sh --force` in `auto_poll_and_land`. Copies `.polecat-stream.jsonl`, `.polecat-stderr.log`, `.polecat-cmdline` to `logs/polecat-abandoned/<YYYYMMDD-HHMMSS>-<slug>/`. Greps the stream for the `polecat-abandoned: <slug> <reason>` line and writes a one-line `REASON` file alongside.
- `scripts/foreman.sh`: extend `compose_polecat_prompt` with a **Verifiability triage** block listed before `just check && just test`. Names three canonical abandon reasons (`requires-gui`, `requires-long-soak`, `requires-substrate-judgment`) and tells the polecat to fire them at prompt-read time, not after work.
- Echo from `auto_poll_and_land`: after archiving, surface the abandon reason inline so the foreman log carries it: `foreman: polecat $slug abandoned — <reason>`.

## Out of scope
- A `polecat-suitable: bool` frontmatter field on tickets — leave that as a follow-on if archive data shows specific tickets fail repeatedly across distinct polecat invocations.
- The `session_new.sh` rollback gap (status flips inside flock; `jj workspace add` runs outside flock with no cleanup on failure) — separate ticket if it recurs.

## Approach
Both edits in `scripts/foreman.sh`. New function archives via `cp` (not `mv`) so cleanup remains intact; placed before the existing `bash scripts/session_done.sh "$slug" --force` call in the auto-land loop's per-slug branch. Triage block is a verbatim addition to the heredoc in `compose_polecat_prompt` — same structure as the existing "Constraints (load-bearing)" / "Exit ceremony" sections.

## Verification
- Re-spawn a foreman against the swarm-safe queue. The same UI-bound tickets will be picked. Expectation: each polecat abandons within ~1 minute (triage stage) with a named reason, not 5-10 minutes (post-cargo).
- `ls logs/polecat-abandoned/` after the drain shows one dir per abandoned slug with a non-empty `REASON` file.
- `just check && just test` still passes (no Rust touched).

## Log
- 2026-05-15: opened after first foreman drain returned 0 / 3 landed; abandon reasons unrecoverable because workspaces were removed before harvest.
- 2026-05-15: 2026-05-15: archived first-drain abandons `requires-gui/requires-long-soak/requires-substrate-judgment` are canonical reasons; archive copies stream/stderr/cmdline + REASON to logs/polecat-abandoned/<stamp>-<slug>/ before session_done --force
