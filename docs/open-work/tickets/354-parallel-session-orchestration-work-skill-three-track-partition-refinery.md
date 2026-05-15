---
id: 354
title: Parallel-session orchestration: /work skill + three-track partition + refinery
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

153 open tickets, single-track landing cadence. Parallel session throughput is bottlenecked on five compounding pains: co-located sessions thrash (two Claudes in the same working tree spend cycles in "I didn't edit that" loops); jj-workspace ergonomics (staleness, bookmark ownership, accidental `main` moves); disk pressure (95%-full volume; per-session `target/` doesn't fit at N>2); cognitive overhead (no at-a-glance view of which session is doing what); landing serialization (sessions queue behind review attention). The autonomy model has converged on per-task bookmark + verdict-gated sweep-land; session shape aspires to 2-ticket batches but currently degrades to 1 because spin-up friction isn't amortized. This ticket builds the skill + recipe infrastructure that makes that aspirational shape cheap.

## Scope

- **Frontmatter axis**: `orchestration: substrate-sensitive | coherent-block | swarm-safe` (default `substrate-sensitive`), plus `block:` (required iff `coherent-block`) and `verdict-anchor: true` (≤1 per block). Enforced by `scripts/check_orchestration_frontmatter.sh` via `just check`.
- **`/work` skill** (`.claude/commands/work.md`): interactive entry — reads state (`just session-list`, `just open-work-by-track`, `just refinery --json`), presents menu, dispatches primitives.
- **`/retag` skill** (`.claude/commands/retag.md`): one-shot corpus tagging ceremony using heuristic auto-classifier + interactive review per batch.
- **Recipe set** with `[tag]` doc-comments (every recipe `--json`-capable for skill consumption):
  - `[session]`: `session-new`, `session-list`, `session-info`, `session-done`, `session-gc`, `session-suggest`
  - `[refinery]`: `refinery` (report / `--auto` whitelisted to `swarm-safe` rows in code / `--land <slug>` / `--block <id>` / `--track <name>`)
  - `[retag]`: `retag-init`, `retag-suggest`, `retag` (single ticket), `retag-audit`, `retag-walk`
  - `[ticket-query]`: `open-work-ready --track`, `open-work-by-track`, `ticket-info`
  - `[block]`: `block-list`, `block-info`, `block-anchor`, `block-verdict`
- **Workspace convention**: `~/clowder-sessions/<slug>/` jj workspaces; `session/<slug>` bookmark namespace owned by exactly one session; `main` read-only inside sessions; refinery is sole path to `main`.
- **Atomic ticket claim**: `session-new --tickets <ids>` writes `status: in-progress` per ticket via `flock` on `docs/open-work/.claim-lock` — prevents two sessions racing the same ticket.
- **CLAUDE.md addendum**: three-track partition, namespace, `/work` entry, refinery as sole path to `main`, polecat-eligibility is `swarm-safe`-only.
- **`docs/workflow/parallel-sessions.md`**: operator guide (deep dive, troubleshooting, why three tracks, block-verdict per-block pattern).

## Out of scope

- **Stage 2 (`/foreman`)**: master-orchestrator spawning child `claude` CLI sessions against swarm-safe queue. Follow-on ticket; `--blocked-by 354`.
- **GUPP-style heartbeat / scheduled-wakeup workers**: deferred (cadence-based refinery is sufficient at this maturity rung).
- **Witness revival logic** (auto-restart of crashed polecats): deferred.
- **Multi-account budgeting**: deferred (decision point, not engineering).
- **Refinery `--auto` for `coherent-block`**: not delivered. The whitelist in code is `swarm-safe`-only. Coherent-block landing always requires explicit user go.
- **Cloud-scheduled / `RemoteTrigger`-based workers**: Clowder requires local Rust builds; cloud workers can't run `just verdict`.

## Current state

- Stage 0 pre-conditions complete in this session (outside the ticket): `cargo clean` recovered ~190G; sibling workspaces (`clowder-002`, `clowder-test`, `incapacitated-wip`) culled; sibling directories (`~/clowder-002`, `~/clowder-incapacitated`, `~/workspace`) removed; `sccache 0.15.0` installed; `~/.cargo/config.toml` configured with `rustc-wrapper = "sccache"` and `SCCACHE_CACHE_SIZE = "40G"`; `wnmitch/002-hunt-approach` and `session-c-draft` pushed to origin as backup before bookmark cleanup; stale `worktree-agent-a*` and `wnmitch/028-030-pipeline` bookmarks forgotten.
- Disk: 262G free (from 46G at session start).
- Stage 0 step 4 (`/retag` ceremony against the 156-ticket corpus) waits on Stage 1 shipping the skill + recipes.

## Approach

Plan reference: `/Users/will.mitchell/.claude/plans/this-is-not-an-curried-hippo.md` carries the full design — three-track partition table, interaction sketches for `/work` and `/retag`, recipe surface organized by `[tag]`, frontmatter shape with the four invariants, workspace architecture, Gas Town comparison (steal: persistent work graph, Refinery as named role, ephemeral polecats, `/handoff` ceremony; reject: "some work gets lost", GUPP Nudge at rung 2, vibe coding).

Build order (incremental commits):

1. Frontmatter axis + enforcement (additive — every existing ticket falls under default `substrate-sensitive` after `retag-init`)
2. Retag primitives (`retag-init`, `retag-suggest`, `retag`, `retag-audit`, `retag-walk`)
3. Session lifecycle primitives (`session-new`, `session-list`, `session-info`, `session-done`, `session-gc`, `session-suggest`)
4. Refinery primitive (`refinery` with the swarm-safe `--auto` whitelist in code)
5. Block management primitives (`block-*`, `open-work-by-track`, `ticket-info`)
6. `/work` and `/retag` skills (compose the primitives)
7. CLAUDE.md addendum + `docs/workflow/parallel-sessions.md`

Each commit ships its own enforcement + tests where applicable. `just check` enforces the four invariants on the frontmatter axis (see Scope) starting commit 1; this means commit 1 must also run `retag-init` to keep `just check` green.

## Verification

After Stage 1 lands:

1. `just check` — green; the 4 frontmatter invariants enforced.
2. `/work` opens the menu against current state; shows ready queues by track, in-flight sessions (none yet), disk free, sccache stats.
3. `just session-new test-batch --tickets <id1>,<id2> --track swarm-safe` creates `~/clowder-sessions/test-batch/` jj workspace + `session/test-batch` bookmark + atomic claim on the tickets.
4. `cd ~/clowder-sessions/test-batch && just check` — isolated build works (sccache cache-hit visible on second invocation).
5. `cd ~/clowder && just sessions` — dashboard shows the new session.
6. `just refinery` — reports `test-batch` as `landable` or names the blocker.
7. `just refinery --auto` — lands swarm-safe verdict-pass rows; bookmark forgotten, workspace cleaned, disk reclaimed.
8. Negative test: `just refinery --auto --track substrate-sensitive` refuses (whitelist enforced in code).
9. Negative test: opening two `session-new` with overlapping `--tickets` — the second flocks-fails and aborts before creating its workspace.

Post-Stage-1: `/retag` ceremony tags all 156 tickets across the three tracks; HTN 128 children land as `coherent-block` + `block: htn-method-composition` with 319 as `verdict-anchor: true`.

## Log

- 2026-05-15: opened. Stage 0 pre-conditions completed in this session (disk recovery + sibling cull + sccache); Stage 1 build-out starts next commit.
