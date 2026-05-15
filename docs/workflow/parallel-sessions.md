# Parallel-session orchestration

Operator's guide for running multiple Claude Code sessions in parallel against the Clowder repo. Reference for `/work`, `/retag`, and the `[session]` / `[refinery]` / `[retag]` / `[block]` / `[ticket-query]` recipe groups landed in ticket 354.

## Mental model

You operate one "master" Claude Code session at `~/clowder`. Other sessions run in isolated `~/clowder-sessions/<slug>/` workspaces (jj workspaces sharing `.jj/repo` but with their own working copy + Rust `target/`). Each session owns one bookmark, `session/<slug>`, and never touches `main`. The master session runs `/work` to land child sessions into `main` via the refinery.

Three orchestration tracks govern how a session is treated:

| Track | Verdict cadence | Session lifetime | Polecat-eligible (Stage 2) |
|---|---|---|---|
| `substrate-sensitive` (default) | per-ticket, human-gated | short, careful | no |
| `coherent-block` | block-level at verdict-anchor | long, spans context windows | no |
| `swarm-safe` | per-ticket; refinery may auto | ephemeral | yes |

Every active ticket carries `orchestration: <track>` in its frontmatter. The default is `substrate-sensitive` (safest). Promote tickets to `swarm-safe` (faster cadence, polecat-eligible) or `coherent-block` (block-level orchestration) explicitly via `/retag` or `just retag <id> --track <name>`.

## Quickstart

**Daily entry:**
```
/work
```

**One-shot corpus tagging (Stage 0 ceremony):**
```
/retag
```

## Session lifecycle (step by step)

### 1. Create a session

```
just session-new <slug> --tickets <id1>[,<id2>] --track <name>
```

Creates `~/clowder-sessions/<slug>/` as a jj workspace, sets the `session/<slug>` bookmark at `main`, writes `.session-info.json`, and atomically claims the named tickets (writes `status: in-progress` per ticket under a `flock` on `docs/open-work/.claim-lock` — double-claiming is refused).

Pass `--print-prompt` to get a copy-pasteable starter prompt for a new Claude session in the new workspace path.

`--pick` auto-selects one ready ticket from `--track <name>` (mutually exclusive with `--tickets`).

### 2. Work the ticket in the new workspace

Open a new Claude Code session, `cd ~/clowder-sessions/<slug>`, paste the starter prompt. Build / test / commit as normal. Stay on the `session/<slug>` bookmark (jj will auto-snapshot edits to it).

When done, exit the session via `/handoff`, then push the bookmark:

```
jj git push --bookmark session/<slug> --allow-new
```

### 3. Master session lands the work

Back at `~/clowder`, run `/work`. It reads `just refinery --json` and surfaces the session as `landable-manual` (or `needs-rebase` / `conflict` if main has moved).

Land:

```
just refinery --land <slug>
```

The refinery:
- Rebases `session/<slug>` onto current `main` (if behind)
- Advances `main` to the session's head
- Forgets `session/<slug>`
- Calls `session-done.sh <slug> --no-release` to clean up the workspace (the tickets were already set to `done` via `just land` inside the session)

If the rebase has conflicts, the refinery aborts and names the conflict. Resolve in the session's workspace (`cd ~/clowder-sessions/<slug>`), commit, push, retry.

### 4. Abandoned session (no land needed)

```
just session-done <slug>
```

Releases the session's `in-progress` tickets back to `ready` (skips tickets already `done`), `cargo clean`s the workspace target, `jj workspace forget`s, removes the directory, forgets the bookmark.

Pass `--force` to skip the uncommitted-edits guard. Pass `--keep-bookmark` to preserve the bookmark for a later land.

## Three-track orchestration in detail

### substrate-sensitive (the default)

Use for: bugfix work, layer-walk required, balance-affecting changes, anything touching `src/ai/` or `src/components/` that needs the discipline named in CLAUDE.md §"Bugfix discipline".

**Verdict cadence:** per-ticket. The session runs its own soak + `just verdict` before landing. The refinery never auto-lands these.

**Sessions:** short, careful, one ticket at a time. The `/work` "Start a new session" flow suggests one ticket plus optionally one adjacent ticket (by `just similar`) — keeps batches small.

### coherent-block (epic construction)

Use for: epics where intermediate states are structurally unverifiable. Currently identified blocks:

| Block (initiative-id) | Anchor candidate | Member signal |
|---|---|---|
| `htn-method-composition` | 128 (epic) or a registry-enforcement gate ticket | `wires-method:` frontmatter present OR `blocked-by: 128` |
| `crafting-economy` (proposed) | 016 | manual — no auto-classifier signal yet |

**Verdict cadence:** block-level at the `verdict-anchor: true` ticket. Other tickets in the block land verdict-skipped (the substrate is partially-assembled and can't produce useful signal). The anchor's landing triggers the block-level verdict recipe (each anchor authors its own — generic `just verdict` doesn't answer "did the new substrate fire? did legs stay orthogonal?").

**Orthogonality precondition:** `verdict-anchor: true` is an *assertion* that the block's legs are designed orthogonally (per CLAUDE.md's "richer perception, better strategy" pillar — orthogonal axes, no single dominant scalar). If a block accidentally violates orthogonality, the safety property breaks and you must fall back to per-ticket cadence for that block. The anchor's authoring is a structural decision; the auto-classifier never picks anchors.

**Sessions:** long-lived. May span multiple context windows via `/handoff` artifacts. The session's commit stream lands the block's legs incrementally; only the anchor's land fires the block-level verdict.

**Per-block verdict pattern:** the anchor ticket scopes its own block-verdict recipe under `just block-verdict <initiative-id>` (currently a stub — each block authors its own composition of `just verdict` + block-specific Feature-fired checks + welfare deltas). Document the pattern in the anchor ticket's `## Verification` section so future readers see the signal shape.

### swarm-safe (the fast track)

Use for: docs, frontmatter migrations, mechanical refactors, atomic bugfixes with already-verified layer-walks, sweep-runner work, template adoption.

**Verdict cadence:** per-ticket. The refinery `--auto` flag (Stage 2; not yet implemented) lands these in batches when verdict-pass + no-conflict. The whitelist is **in code**, not by convention — `scripts/refinery.sh` refuses `--auto` on anything other than `swarm-safe`.

**Sessions:** ephemeral polecats (Stage 2). One ticket per session, push and exit. Stage 1 of the orchestration ships only the manual path; `/foreman` (Stage 2) ships the polecat dispatcher.

## Frontmatter invariants (enforced by `just check`)

```yaml
orchestration: <track>            # required on every active ticket
block: <initiative-id>            # required iff coherent-block
verdict-anchor: true              # optional, ≤1 per block
```

The enforcement script (`scripts/check_orchestration_frontmatter.py`) validates four invariants:

1. `orchestration:` present + one of substrate-sensitive | coherent-block | swarm-safe
2. `coherent-block` ⇒ `block:` present AND `block:` value appears in `initiative:` list
3. ≤1 `verdict-anchor: true` per `block:` value
4. `swarm-safe` ⇒ no `block:`, no `verdict-anchor:`

Run `just check` after editing any ticket frontmatter. Run `just retag-audit` for a corpus-wide rollup view.

## Discovering recipes

`just --list | grep '\[<tag>\]'` filters by recipe group:

```
just --list | grep '\[session\]'      # session lifecycle
just --list | grep '\[refinery\]'     # landing
just --list | grep '\[retag\]'        # corpus tagging
just --list | grep '\[block\]'        # block management
just --list | grep '\[ticket-query\]' # ticket introspection
```

The tag prefix is the durable handle — every new recipe in this subsystem carries one in its doc-comment.

## Troubleshooting

**"refinery: needs-rebase"**
The session diverged from main while you were working. Land path will attempt the rebase automatically. If it conflicts, resolve in the session's workspace (`cd ~/clowder-sessions/<slug>`), commit, push, retry `just refinery --land <slug>`.

**"session-new: ticket X is already in-progress"**
Another session has claimed this ticket. Check `just sessions` to find the holder; either wait for them to finish + land, or abandon their session (`just session-done <slug>`) to release the claim. Atomic claim prevents two sessions racing the same ticket.

**"orchestration-frontmatter: ... missing 'orchestration:'"**
A ticket is missing the field. Run `just retag-init` to backfill the default on all untagged tickets.

**"block 'foo' has 2 verdict-anchor:true tickets"**
Two tickets in the same block claim to be the verdict-anchor. Decide which one is canonical and unset the other: `just retag <id> --unset-anchor`.

**Disk pressure (sccache + workspace targets)**
Sessions share rustc unit-cache via `sccache` (configured in `~/.cargo/config.toml`). Each session has its own `target/`; `just session-done` runs `cargo clean` to reclaim. Monitor disk with `just sessions --disk` (shows per-session target sizes). If disk is binding, increase `SCCACHE_CACHE_SIZE` in the cargo config or reduce parallel-N.

**jj workspace stale**
If `jj status` reports "working copy is stale" in a session workspace, run `jj workspace update-stale`. Doesn't usually happen — sessions only contend on op-log writes, not working copies.

## Reference

- Ticket: [`docs/open-work/tickets/354-parallel-session-orchestration-work-skill-three-track-partition-refinery.md`](../open-work/tickets/354-parallel-session-orchestration-work-skill-three-track-partition-refinery.md)
- Plan: `~/.claude/plans/this-is-not-an-curried-hippo.md`
- Skills: [`.claude/skills/work/SKILL.md`](../../.claude/skills/work/SKILL.md), [`.claude/skills/retag/SKILL.md`](../../.claude/skills/retag/SKILL.md)
- CLAUDE.md addendum: §"Long-horizon coordination" / "Parallel-session orchestration"
- Heuristic classifier rules: `scripts/retag_suggest.py` docstring
