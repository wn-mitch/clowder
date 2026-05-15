---
id: 353
title: splice just similar into peer skill cross-references + CLAUDE.md bugfix discipline
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

Phase 2 follow-on from [229](../landed/229-add-just-similar-semantic-retrieval-over-clowder-prose.md). 229 shipped the `just similar` tool itself; without discipline wiring it sits behind a discoverability cliff. Per CLAUDE.md auto-memory `feedback_diagnostic_tools_need_discipline_wiring` ("when adding an investigation/harness tool, update the workflow doc that names when to reach for it in the same PR; otherwise the tool exists but isn't used"), the next-session-me needs `just similar` to surface inside the workflows where it pays off: layer-walk audits, collapse triage, balance investigations.

229's `## Out of scope` parked this exact subscope: "Cross-skill docs integration — opens as a follow-on ticket blocked-by 229 once Phase 1 lands."

## Scope

- `.claude/skills/logq/SKILL.md` — add `just similar` to `Relationship to neighbouring tools` for "find adjacent runs / tickets" lookups.
- `.claude/skills/explain/SKILL.md` — add `just similar` for "what other constants / tickets are conceptually adjacent to this knob."
- `.claude/skills/verdict/SKILL.md` — add `just similar` for collapse-fingerprint matching against past failures.
- `.claude/skills/inspect/SKILL.md` — add `just similar` for "what tickets describe behavior like this cat's."
- `.claude/skills/diagnose-collapse/SKILL.md` — add `just similar` for "does this collapse pattern look like a prior cluster" lookups.
- `CLAUDE.md` "Bugfix discipline" section — one-line mention of `just similar <ticket-id>` as the surface to reach for during the layer-walk's "find precedent for split / extend / rebind / retire" question.

No code changes. Pure docs cross-linking.

## Out of scope

- Adding `just similar` to non-diagnostic skills (e.g. `commit`, `pr-comments`). Stays in the diagnostic family.
- Re-running embedder comparisons; SKILL.md::Identifier-only queries underdeliver already documents the v1 decision.

## Current state

Brand-new follow-on, opened in the same operation that lands 229. `just similar` itself is shipped; this ticket only adds the cross-references that make the tool reachable from peer-skill workflows.

## Approach

For each of the five peer-skill SKILL.md files, add a bullet under `## Relationship to neighbouring tools` (or equivalent section) pointing at `just similar` with a one-sentence note on what it adds vs. that skill. Mirror the corresponding "neighbouring tools" reverse-bullet that's already in `.claude/skills/similar/SKILL.md:128-136`.

For `CLAUDE.md` "Bugfix discipline," splice one line into the "Layer-walk audit before listing fix candidates" or "Sub-agent dispatch discipline" subsection — placed where it'll catch the reader during the layer-walk step.

## Verification

1. Each of the five peer SKILL.md files mentions `just similar` in its `Relationship to neighbouring tools` section.
2. CLAUDE.md "Bugfix discipline" mentions `just similar` once, in a clear "reach for this during the layer-walk" framing.
3. No code changes; `just check` clean.
4. Cross-references are bidirectional with `.claude/skills/similar/SKILL.md` (each peer is listed in `similar`'s relationships section, and vice versa).

## Log

- 2026-05-15: opened as 229's Phase 2 follow-on at landing time per CLAUDE.md "Antipattern migration follow-ups are non-optional"; ready immediately because 229 lands in the same operation.
