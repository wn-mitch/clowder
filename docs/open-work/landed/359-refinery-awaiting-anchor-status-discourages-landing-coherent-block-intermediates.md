---
id: 359
title: refinery awaiting-anchor status discourages landing coherent-block intermediates
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
landed-at: 11eb3efb2143
landed-on: 2026-05-15
---

## Why

`scripts/refinery.sh` overrides the report status for coherent-block sessions
from `landable-manual` to `awaiting-anchor` (lines 111-115). The override is
purely cosmetic — `land()` itself never gates on track or status — but
operators and `/work` read the status string as a gate and refuse to land
coherent-block intermediates, producing the bookmark-dangling antipattern.

CLAUDE.md explicitly says: *"Verdict fires at the anchor's landing;
intermediates land verdict-skipped."* Intermediates SHOULD land. The whole
point of the parallel-session workflow is to merge work back to main as it
completes; an "awaiting-anchor" string visible in `just refinery` and
`just sessions` reverses that intent and leaves coherent-block bookmarks
accumulating commits indefinitely (the htn-method-composition block has 7
ready siblings + 1 in-progress anchor + 1 blocked sibling — if every block
intermediate awaits the anchor, the bookmark would carry weeks of work
before any of it reaches main).

## Current state (tooling layer-walk)

| Layer | Component / file:line | Load-bearing fact | Status |
|---|---|---|---|
| Status compute | `scripts/refinery.sh:96-107` | Computes `rebase_state` + `action` ∈ {`already-on-main/forget-bookmark`, `clean-fast-forward/landable-manual`, `needs-rebase/landable-manual`} | `[verified-correct]` |
| Coherent-block override | `scripts/refinery.sh:111-115` | If `track == coherent-block` and `action == landable-manual`, **overwrites** action to `awaiting-anchor` | `[verified-suspect]` |
| `land()` enforcement | `scripts/refinery.sh:160-206` | Never reads `$track` or `$action`. Will land any bookmark with `--land <slug>` regardless of orchestration track. The "awaiting-anchor" string is report-only | `[verified-correct]` |
| `/work` skill UI | `.claude/skills/work/SKILL.md` | Presents the menu using refinery's status strings; an "awaiting-anchor" row gets demoted in the action list | `[verified-correct]` |
| CLAUDE.md doctrine | `CLAUDE.md` "Parallel-session orchestration" + "coherent-block" definition | *"Verdict fires at the anchor's landing; intermediates land verdict-skipped."* — intermediates ARE meant to land freely | `[verified-correct]` |

The doctrine and the enforcement layer agree. The misleading actor is the
report-string override.

## Fix candidates

**A. Remove the override entirely.** Delete the `if [[ "$track" ==
"coherent-block" ]]` block (lines 111-115). Coherent-block sessions report as
`landable-manual`, identical to substrate-sensitive. Operators land them
freely; the "verdict-skipped" semantics are documented in CLAUDE.md and in
the land commit messages (`(verdict-skipped per coherent-block)`), not
encoded in the refinery status.

**B. Rename to a non-gating signal.** Keep the override but change the string
to something like `landable-manual-verdict-skipped` or
`landable-coherent-block`. Communicates the verdict-skip semantics but
doesn't read as a gate. Cost: longer status strings break the table column
width budget at line 153.

**C. Move the signal out of the action column.** Add a separate "verdict"
column showing `verdict-required` / `verdict-skipped` / `verdict-anchor`.
Keeps the verdict semantics legible without contaminating the action.
Higher-cost change (column-width math, JSON shape change for downstream
consumers like `/work`).

**Structural option (rebind):** Move the verdict-skip annotation from
refinery status entirely to the land commit message convention (where it
already lives). Refinery only reports "can this land yes/no"; verdict
semantics are a per-ticket property surfaced via `just ticket-info`. Same
spirit as A but justifies the deletion: the verdict isn't refinery's
concern, it's the verdict tooling's concern. The verdict tooling (`just
verdict`) already runs against a run-dir, not a bookmark, so refinery has
no reason to comment on it.

## Recommended direction

**A.** The override exists because someone wanted to communicate "this
won't get a verdict on land" — but that semantic belongs to the verdict
tooling, not the refinery report. Deletion is the smallest change that
restores doctrine alignment.

If post-deletion operators miss the signal, C (separate column) is the
follow-on; it's a strictly additive change once the misleading override is
gone.

## Out of scope

- Changes to `just verdict` or how verdicts are recorded.
- Adding a verdict column to the report (deferred follow-on; see C).
- `/work` skill UI changes; the skill reads from refinery's JSON, so once
  refinery reports `landable-manual` for coherent-block sessions, `/work`
  presents them the same way it presents substrate-sensitive sessions.
- Auto-landing of coherent-block intermediates (still requires explicit
  `--land <slug>`; `--auto` remains swarm-safe whitelist).

## Verification

- A coherent-block session bookmark (e.g., a fresh test session) reports as
  `landable-manual` not `awaiting-anchor` in both text and JSON output.
- `just refinery --land <slug>` against a coherent-block bookmark succeeds
  (no change in behaviour; this was always supported, just unflagged).
- Grep `scripts/refinery.sh` for `awaiting-anchor` returns no matches.

## Log
- 2026-05-15: opened. Surfaced during recovery of the `grief-kitten-vocab`
  near-miss: while reviewing why a coherent-block session bookmark would
  "dangle until anchor lands," realized the gating was cosmetic and
  doctrine-contradicting.
