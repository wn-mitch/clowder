---
id: 358
title: refinery --dry-run silently ignored in --land mode, executes destructively
status: done
cluster: tooling-diagnostics-ui
orchestration: swarm-safe
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

`scripts/refinery.sh` accepts `--dry-run` in `--land <slug>` mode at flag-parse
time (line 49) but the `land()` function (lines 160-206) never checks
`$dry_run`. Result: `just refinery --land <slug> --dry-run` performs the full
destructive sequence (rebase onto main, move main bookmark forward, forget the
session bookmark, `rm -rf` the workspace via `session_done.sh`). Today this
nearly cost the `grief-kitten-vocab` session: a single `--dry-run` invocation
rebased the bookmark onto main with an unresolved `docs/open-work.md` conflict,
moved `main` to the conflicted commit, deleted the workspace (1.1GiB), and left
4 of the bookmark's 5 commits orphaned (the `-r` rebase only moves one commit;
the ancestor chain was unreachable until manually rescued from the jj op log).

The `--auto` codepath does honour `$dry_run` (line 284 gates landing on
`outcome == gate-pass && $dry_run == "false"`). Only `--land` is broken — and
the docstring at line 28 is silent on whether `--dry-run` applies there.

## Current state (tooling layer-walk)

| Layer | Component / file:line | Load-bearing fact | Status |
|---|---|---|---|
| Flag parse | `scripts/refinery.sh:49` | `--dry-run` accepted in any mode, sets `dry_run="true"` | `[verified-correct]` |
| Mode dispatch | `scripts/refinery.sh:333-337` | `case "$mode" in ... land) land "$target_slug" ;;` — `$dry_run` not threaded | `[verified-suspect]` |
| `auto()` body | `scripts/refinery.sh:284` | Checks `$dry_run` before calling `land()`, skips mutation when true | `[verified-correct]` |
| `land()` body | `scripts/refinery.sh:160-206` | **Never references `$dry_run`** — runs rebase + bookmark-move + forget + cleanup unconditionally | `[verified-suspect]` |
| Docstring | `scripts/refinery.sh:28` | "--dry-run runs the gate but skips steps 1-5" — describes only `--auto` semantics; silent on `--land` | `[verified-suspect]` |

## Fix candidates

**A. Implement `--dry-run` in `land()`.** Short-circuit the function with a
preview block: print "would rebase / would advance main / would forget
bookmark / would clean workspace" then `return` before any mutation. Aligns
with operator expectation that a parsed flag does what its name says.

**B. Reject `--dry-run` with `--land` at flag-parse time.** Error message like
"--dry-run is supported only with --auto; --land has no preview mode". Cleaner
contract but a regression in expected behaviour — the operator who reached for
`--dry-run` was hedging against the disaster that occurred; refusing the hedge
wouldn't make the underlying operation any safer.

**Structural option (extend):** The script's mode handling is procedural
(`case`-dispatch). A small refactor that threads a `--preview-only` boolean
through every mutating call site would generalize "preview mode" beyond
`--land`. Rejected as overkill — the only consumer of preview semantics today
is `--land`; candidate A localizes the fix to one function.

## Recommended direction

**A.** Two reasons: (1) the flag IS parsed in `--land` mode today, so an
operator reasonably expects it to work; (2) the change is ~10 lines and uses
the same `[[ "$dry_run" == "true" ]]` idiom that `auto()` already establishes
(line 284), so the codebase grows no new patterns.

## Out of scope

- `--auto --dry-run` (already works correctly; verified at line 284).
- Other refinery flags (`--track`, `--json`); no defects observed.
- Recovery tooling for orphaned commits (the jj op log already covers this).
- Renaming the flag, or splitting it into mode-specific variants.

## Verification

- `just refinery --land <slug> --dry-run` prints "would rebase / would advance
  / would forget / would clean" preview and exits 0.
- After the dry-run: `jj bookmark list session/<slug>` still shows the
  bookmark; main has not moved; workspace directory still exists.
- Docstring (or `--help` output via line 50) names `--dry-run` semantics in
  `--land` mode.

## Log
- 2026-05-15: opened after the `grief-kitten-vocab` near-miss. Recovery via
  `jj op log` + sanctioned `just land` re-runs (no work lost, ~15 min cost).
