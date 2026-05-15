---
id: 341
title: Retarget 057 blocked-by from 126 to 128
status: ready
cluster: process-discipline
orchestration: substrate-sensitive
initiative: [smarter-cats]
added: 2026-05-14
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

128 epic process touchup. Ticket 057
(`coordinator-directive-intention-strategy-row`) is currently
`blocked-by: [126]`. With 126 landed and 128 now committed as
the next dependency (the HTN method-decomposition layer 057's
strategy-row writes through), 057's blocker retargets to 128.

Per `docs/open-work/landed/126-bdi-intention-substrate.md`
§Dependencies:

> 057 will write `HeldIntention { source:
> CoordinatorDirective(coord), .. }` ... on land of 126,
> retarget both to `blocked-by: [126]` since `HeldIntention` +
> `IntentionSource` is the actual prerequisite.

128 now strengthens that — 057's strategy row composes against
the method-decomposition layer 128 lands, so the dependency
edge moves up.

## Scope

- One-line frontmatter edit on
  `docs/open-work/tickets/057-coordinator-directive-intention-strategy-row.md`:
  `blocked-by: [126]` → `blocked-by: [128]`.
- Regenerate `docs/open-work.md` via `just open-work-index`.
- Add a `## Log` line in 057 noting the retarget.

## Out of scope

- Any substantive change to 057's body or scope.
- 057's actual implementation work (still waits on 128's land).

## Current state

128 promoted to epic 2026-05-14; child #23 of 25.
`status: ready` (no blockers) — can land any time during the
epic, but most natural as part of 128's epic-bootstrap commit
(this session).

## Approach

Simple frontmatter edit. The 060 row update happens in the same
commit (independent of 128's own row pointing at 057).

## Verification

- `just check` passes.
- `just open-work-index` regenerates cleanly.
- `just open-work-ready` shows 057 in the `ai-substrate` cluster
  with `blocked-by 128`.

## Log

- 2026-05-14: opened as 128 epic child #23 (Batch E cross-cutting;
  process touchup).
