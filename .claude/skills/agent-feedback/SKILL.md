---
name: agent-feedback
description: Friction-log breadcrumb (`just agent-feedback "<note>"`) — appends one JSON line to `logs/agent-friction.jsonl` capturing what was tried, where it broke, and how severe. Fire at the *moment of friction*, not as a TODO list. Trigger conditions — "you tried two things and they both failed", "a tool's output didn't match its SKILL.md description", "you can't figure out which `just` command serves the user's intent", "you misread an envelope and had to retry after a course-correction". Do NOT fire for — routine errors that resolve on the first retry, syntax mistakes the user corrects in stride, gaps in your codebase knowledge that aren't tooling gaps, or as a venting channel.
---

# Friction-Log Breadcrumb (`just agent-feedback`)

`just agent-feedback "<note>"` writes a single JSON line to `logs/agent-friction.jsonl` capturing the moment a workflow bonked. The corpus accumulates over weeks. Periodic review surfaces recurring patterns worth encoding into CLAUDE.md, a new SKILL.md, or a new tool.

This is the agent-side mirror of `logs/agent-call-history.jsonl` (which records *successful* tool calls and their rationales). Friction is the data the call-history doesn't capture.

## When to fire

**Fire when:**

- You tried two reasonable approaches and both failed — even though the user's request seemed in-scope for the available tooling.
- A tool's output shape didn't match what its SKILL.md described (and you wasted a turn parsing the disagreement).
- You couldn't pick the right `just` command from the available surface for the user's intent — i.e., the routing layer let you down.
- A SKILL.md's *Fire when* / *Do NOT fire when* block was wrong about a case that came up in practice.
- You hit a friction the user had to point out manually that should have been catchable from tool output alone.

**Do NOT fire when:**

- You made a mistake the user immediately corrected — that's not tool friction, it's you. Adjust and move on.
- A tool refused as designed (e.g., `q events` on a sweep dir refusing — the tool is *correct*; if its narrative pointed you at the right alternative, no friction occurred).
- A tool errored on bad input that's clearly the caller's fault (typos, malformed args).
- You were exploring and a tool said "no such file" — that's exploration, not friction.
- As a venting channel about CLAUDE.md being inconvenient or about not liking a constraint.
- Multiple times for the same friction in one session — one breadcrumb per friction is enough.

## Output

The tool writes to stderr `agent-feedback: recorded (<severity>) → logs/agent-friction.jsonl` and exits 0 on success. **No useful return value to the calling agent** — this is fire-and-forget breadcrumb-leaving.

## Schema

```jsonc
{
  "timestamp":   "<ISO8601 UTC>",
  "commit":      "<short hash or null>",
  "cwd":         "/Users/.../clowder",
  "note":        "<one-sentence summary>",
  "severity":    "minor" | "major" | "blocker",
  "tool":        "<name or null>",     // which tool the friction was around
  "what_tried":  "<text or null>",     // what you tried (often redundant with note)
  "where_stuck": "<text or null>"      // where things broke or became ambiguous
}
```

## Severity rubric

- **minor** — annoyance, didn't lose time. Default. Use generously; the corpus is the point.
- **major** — had to redo work, take a different path, or escalate to the user.
- **blocker** — couldn't proceed at all. Rare; this is the "this tool is broken or missing" signal.

## Examples

```bash
# Most common — minor severity, with tool tag.
just agent-feedback "q events refused on sweep dir; narrative didn't suggest q deaths" --tool q

# Major — wasted a turn parsing output mismatch.
just agent-feedback "frame-diff output table shape didn't match SKILL.md columns" --severity major --tool frame-diff

# Routing gap — couldn't pick a tool.
just agent-feedback "user wanted to compare two single runs (not sweeps); no obvious tool — verdict is single-run-vs-baseline; sweep-stats wants directories" --severity major

# Blocker — tool absent for a real workflow.
just agent-feedback "no way to query trace records by DSE name across multiple cats" --severity blocker --tool q
```

## Periodic review

The friction log is meant to be reviewed weekly or fortnightly. Suggested triage:

```bash
# Last week's friction.
jq -c 'select(.timestamp > "2026-04-20")' logs/agent-friction.jsonl

# Group by tool.
jq -c 'select(.tool)' logs/agent-friction.jsonl | jq -s 'group_by(.tool) | map({tool: .[0].tool, n: length})'

# Blockers only.
jq -c 'select(.severity == "blocker")' logs/agent-friction.jsonl
```

Recurring entries (2+ separate sessions on the same theme) are the strongest signal that the pattern deserves a fix — a CLAUDE.md note, a new SKILL.md, or a tool change.

## Hook safety

`logs/agent-friction.jsonl` is at the root of `logs/` and is **not** under the `logs/tuned-*` or `logs/baseline-*` prefixes that `.claude/hooks/no-log-overwrite.py` protects. Append-writes are allowed without prompting.

## Non-goals

- Does not page anyone, file an issue, or auto-create tickets. The corpus is a passive record.
- Does not include tool args or full context — this is for *your* friction signal, not for reproducing the situation. Pair with `logs/agent-call-history.jsonl` if cross-referencing is needed (match on commit + nearby timestamps).
- Does not have a "resolved" field. Once written, an entry stays. If a friction is fixed, that's evidence in the next periodic review's *delta*, not a state change on the old entry.
