---
id: 363
title: polecat track-enforcement gap — coherent-block tickets reach polecat queue
status: ready
cluster: process-discipline
orchestration: swarm-safe
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

CLAUDE.md "Parallel-session orchestration" names a three-point
enforcement that should make polecat-eligibility **swarm-safe only**:

> "Polecat-eligibility is **swarm-safe only**, enforced in three places:
> `/foreman` skill refuses other tracks, `scripts/foreman.sh` only picks
> from the swarm-safe ready queue, `scripts/refinery.sh --auto` rejects
> non-swarm-safe rows even with explicit `--track <other>`."

Despite this, tickets #332 and #333 — both carrying
`orchestration: coherent-block` in frontmatter — were polecat-worked
(observed via the orphan trail in git log: `wip: ... (recovered from
crashed session)` + `feat: 332/333 ...` orphan pattern repeated across
multiple retries; substrate landed twice at `f3c72a06` and `00aa3636`,
both orphaned). Surfaced during #362's forensic investigation. One or
more of the three enforcement points is broken or bypassed.

**Impact.** Coherent-block tickets need substrate authoring +
intermediate verification that doesn't fit a 30m wallclock cap. When a
polecat with that constraint picks one up, it either (a) times out
mid-flight, or (b) ships only the verbally-easy parts (paperwork) and
skips the substrate. Both outcomes manifested for #332/#333 — verified
by inspecting their `land:` commits, which touched ONLY
`docs/open-work.md` + ticket file moves, with no source files.

## Current architecture (workflow-pipeline audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Skill-level filter | `/foreman` skill spec | Refuses to spawn polecats against non-swarm-safe tickets per CLAUDE.md doctrine. Implementation lives in the foreman skill file (`.claude/skills/foreman/SKILL.md` or similar). | `[suspect]` — needs verification |
| Foreman script filter | `scripts/foreman.sh::pick_ready_queue` | Filters `open-work-ready` output to swarm-safe orchestration only. Look for the orchestration-frontmatter read and the filter predicate. | `[suspect]` — needs verification |
| Refinery auto filter | `scripts/refinery.sh --auto` | Rejects bookmarks whose source ticket is non-swarm-safe even when `--track <other>` is passed. | `[suspect]` — needs verification |
| Ticket open-time | `just open-ticket --cluster ...` | Sets `orchestration: <track>` correctly on open. `scripts/check_orchestration_frontmatter.py` via `just check` enforces *presence* of the field, not the choice. | `[verified-correct]` (we read this in #362) |
| Frontmatter migration | `/retag` skill + `just retag` | Auto-classifies existing tickets into one of the three tracks. May have misclassified some coherent-block tickets as swarm-safe. | `[suspect]` |

## Reproduction

The investigation in #362 surfaced the symptom (orphan trail in git log
matching active ticket ids). To reproduce the broken enforcement
directly:

1. Open a test ticket with `orchestration: coherent-block`.
2. Run `just foreman` (default mode) and observe whether the ticket
   appears in the swarm-safe ready queue.
3. Run `scripts/refinery.sh --auto --dry-run` against a workspace whose
   bookmark cherry-picks a coherent-block ticket's land commit;
   observe whether it's rejected.

The expected behavior is "neither surfaces the coherent-block ticket
to polecat workflow." Observed behavior (in the #332/#333 case): at
least one of the points accepted it.

## Fix candidates

**Parameter-level options:**

- **R1 (add a final assertion in `scripts/foreman.sh::spawn_polecat`)** —
  before spawning each polecat, re-read the ticket's `orchestration:`
  frontmatter; if not `swarm-safe`, refuse to spawn and surface a clear
  error. This is the cheapest defense-in-depth — it doesn't rely on
  upstream filters being correct.

- **R2 (add an assertion in `scripts/refinery.sh --auto` per-row)** —
  for each bookmark `--auto` would land, look up the ticket's
  `orchestration:` from frontmatter; refuse if not `swarm-safe`. Same
  shape as R1 but at the landing gate.

- **R3 (audit + repair the existing filters)** — read each of the three
  enforcement points end-to-end, find the bug, fix at the source.
  Probably essential regardless of R1/R2.

**Structural options:**

- **R4 (split — separate swarm-safe queue file)** — instead of
  filtering the `open-work-ready` output at the consumer, maintain a
  separate `docs/open-work/.swarm-safe-ready` queue file regenerated
  by `just open-work-index`. Foreman + refinery read ONLY that file,
  not the broad ready set. Splits the swarm-safe surface from the
  general-ready surface entirely — no filter to bypass.

- **R5 (extend — orchestration tag → directory hierarchy)** — re-shape
  `docs/open-work/tickets/` into per-track subdirectories
  (`tickets/swarm-safe/`, `tickets/coherent-block/`,
  `tickets/substrate-sensitive/`). Foreman + refinery walk only the
  swarm-safe subdirectory by construction; enforcement becomes a
  filesystem fact, not a script-internal filter.

- **R6 (rebind — orchestration tag → CI gate)** — gate the foreman
  spawning at the CI level via a precondition check that runs
  `scripts/check_orchestration_frontmatter.py` against every claimed
  ticket. Rebinds the enforcement from "script-internal filter" to
  "git-level precondition" — same shape as the existing `just check`
  enforcements that block on misshapen frontmatter.

## Recommended direction

**R3 + R1 + R2** as a bundle.

- **R3** finds and fixes the root cause — without it, R1/R2 only
  paper-over the symptom and the broken filter remains as a footgun.

- **R1 + R2** add defense-in-depth at the two action sites (spawn,
  land) so a future regression in the upstream filter doesn't silently
  re-enable the bug.

Rejected:
- **R4** (separate queue file): scope creep — the bug isn't "filters
  are too distant from the consumer", it's "filters are buggy".
  Maintaining a separate queue file adds a sync invariant that itself
  needs enforcement.
- **R5** (directory hierarchy): would touch every ticket file on
  disk; massive churn for a small-bug fix. Save for a deliberate
  re-shape if the three-track model evolves.
- **R6** (CI gate): adds latency to spawn; doesn't help with the
  refinery side. The script-local assertion (R1/R2) is faster + same
  effect.

## Out of scope

- Wallclock-cap sizing for coherent-block work. Coherent-block tickets
  shouldn't be polecat-eligible at all (this ticket fixes that), so the
  cap is moot. If someone later wants polecats to handle coherent-block
  work, that's a separate design decision with its own implications.
- The "session_done.sh orphans unpushed bookmarks" defect (separate
  defect, fixed in #362). #363 ensures polecats stop reaching
  coherent-block tickets in the first place; #362 protects when
  polecat work does get orphaned regardless.
- Retroactive re-classification of mis-tagged tickets. Once the filter
  is fixed, `/retag` can re-audit the active set; out of scope here.

## Verification

- Read each of the three enforcement points; identify which one(s)
  accept coherent-block.
- After R3 fix: spawn a probe via `just foreman --dry-run --spawn 1`
  with a coherent-block ticket at the top of the ready queue; assert
  no polecat is spawned and the error message names the orchestration
  violation.
- After R1 fix: directly invoke `scripts/foreman.sh --spawn-against
  <coherent-block-id>` (or equivalent); assert immediate refusal with
  clear error.
- After R2 fix: stage a bookmark whose source ticket is coherent-block,
  run `scripts/refinery.sh --auto --dry-run`; assert the bookmark is
  reported as rejected with reason "orchestration != swarm-safe".

## Log

- 2026-05-15: opened as the follow-on to #362's §Out of scope item.
  #362 surfaced the orphan trail; this ticket fixes the *cause* of
  coherent-block work reaching polecats in the first place.
