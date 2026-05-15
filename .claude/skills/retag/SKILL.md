---
name: retag
description: One-shot corpus orchestration tagging ceremony. Walks the ticket corpus track-by-track (swarm-safe → coherent-block → substrate-sensitive) and applies `orchestration:` + optional `block:` / `verdict-anchor:` tags via the heuristic auto-classifier with per-batch user confirmation. Use when the user types `/retag` or asks to "tag the corpus", "apply orchestration tags", "promote tickets to swarm-safe / coherent-block", or "set up the three-track partition". Stage 0 step 4 of ticket 354; one-shot. Do NOT fire for single-ticket retags (use `just retag <id>` directly) or for new tickets (they tag at open-time via `just open-ticket`).
argument-hint: "[--start-from <track>]"
allowed-tools: ["Bash", "Read", "AskUserQuestion"]
---

# /retag — corpus orchestration tagging ceremony

Stage 0 step 4 of ticket 354. Walks the full ticket corpus and applies `orchestration: <track>` (+ optional `block:` / `verdict-anchor:` / `initiative:`) tags so the three-track partition (substrate-sensitive / coherent-block / swarm-safe) is fully populated.

**Run this once** after Stage 0 disk preconditions land + the orchestration enforcement script is wired. After that, `just check` enforces the invariants for every new ticket.

**Arguments:** `$ARGUMENTS` — optional `--start-from <track>` to skip ahead. Default order is swarm-safe → coherent-block → substrate-sensitive (lowest-risk-if-wrong first).

## Workflow

### 1. Read corpus state

```
just retag-audit
just retag-suggest --json
```

Synthesize a one-line summary:
```
Corpus: <N> active tickets · current: <substrate>/<coherent>/<swarm> · suggestions: <X> would change
```

### 2. Walk track by track

**Order matters.** Walk lowest-risk-if-wrong first; that way an early mistake is one revert, not a chain of substrate hacks.

#### Phase A — swarm-safe

```
just retag-suggest --only swarm-safe
```

Show the candidates as a single block (the list is small — currently ~15 tickets). Group by classifier reason. Ask the user via AskUserQuestion:

- **Apply all** — `just retag-suggest --only swarm-safe --apply` (single commit)
- **Walk one by one** — iterate per-row, `just retag <id> --track swarm-safe` per acceptance
- **Skip phase** — leave swarm-safe untagged for now

After applying, run `just retag-audit` and confirm counts shifted.

#### Phase B — coherent-block

```
just retag-suggest --only coherent-block
just block-list
```

The auto-classifier identifies HTN-block members via `wires-method:` frontmatter + `blocked-by: 128` (the HTN epic). Currently ~9 tickets.

For each block (initially just `htn-method-composition`):

1. Show the candidate members
2. Ask the user to identify the **verdict-anchor** (the ticket whose landing fires the block-level verdict). The auto-classifier doesn't pick anchors — that's a structural decision. Surface the highest-priority gate ticket (e.g., 128 itself; or a registry-enforcement ticket if one exists).
3. Apply: `just retag-suggest --only coherent-block --apply` (all members → coherent-block + block: htn-method-composition + initiative add)
4. Set anchor: `just retag <anchor-id> --anchor`
5. Verify: `just block-info htn-method-composition`

If the user wants to define a new block (e.g., a crafting block from 016/017/018), do it manually via:
- `just retag <id> --track coherent-block --block crafting-economy --initiative crafting-economy` per member
- `just retag <anchor-id> --anchor` for the chosen anchor

#### Phase C — substrate-sensitive

This is the safe default; every ticket not promoted to swarm-safe or coherent-block stays here. The retag-init step (Stage 0 commit) already set this for everyone. No further action unless a substrate-sensitive ticket was wrongly promoted to swarm-safe in Phase A — in which case revert per-ticket with `just retag <id> --track substrate-sensitive`.

### 3. Final verification

```
just retag-audit
just check     # runs scripts/check_orchestration_frontmatter.py
```

The audit shows the final corpus partition; the check confirms the four invariants hold. If invariants fail, the auditor's output names the violations.

## Conventions

- **One commit per phase, not per ticket** — keeps the revert surface manageable. If a phase is large (>30 tickets), split into 2-3 commits by classifier-reason group.
- **Never auto-set verdict-anchor** — that's a structural assertion ("the legs are orthogonal; this anchor's verdict means the block is whole") that requires human judgment.
- **Surface dissents** — if the heuristic classifier suggests `swarm-safe` for a ticket the user finds risky, accept the user's override silently. The heuristic is a starting point, not a gate.
- **This is one-shot** — once `just check` passes, the corpus is tagged. New tickets land tagged via `just open-ticket` (Stage 1.7 adds the default to the open-ticket flow).

## Reference

- Plan: `~/.claude/plans/this-is-not-an-curried-hippo.md`
- Ticket: `docs/open-work/tickets/354-parallel-session-orchestration-work-skill-three-track-partition-refinery.md`
- Heuristic rules: `scripts/retag_suggest.py` docstring
