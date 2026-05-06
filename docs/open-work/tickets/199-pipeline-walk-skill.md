---
id: 199
title: Hunt / production / consumption pipeline-walk skill (194 P4)
status: parked
cluster: process-discipline
added: 2026-05-06
parked: 2026-05-06
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Closes 194 F7 / P4. The bug that surfaced 193 was found by
writing a multi-row hunt-pipeline ratio table — there's no
existing skill for that. Reaching for `/logq footer
--field=...` repeatedly never produces the unified pipeline
view; the user had to ask for it directly to break me out of
the skill-surface tunnel.

A new `/pipeline-walk` skill would take a run-dir (or two
run-dirs for diff) and emit per-step rate-normalized counts
for the full hunt → kill → carry → cook → eat / dispose
chain. Each step labeled with its dispatch arm and failure
modes. Same shape for forage / herbcraft / build pipelines.

## Why parked

The 193 episode shows the *need* concretely (one episode), but
the skill is bespoke per-pipeline (hunt vs forage vs herbcraft
vs build) and the right normalization / failure-mode
classification differs across pipelines. Building a generic
funnel skill from a single instance risks over-design — the
right shape for hunt may not generalize cleanly.

**Defer until a second instance demands the per-pipeline
funnel view.** When a second investigation hits the same
"need a multi-step rate table no existing skill produces" wall,
we have two concrete examples to design from rather than one.
Until then, ad-hoc analysis (with the new
`feedback_use_skill_surface.md` escape clause from 197) is
sufficient.

## Direction (when unparked)

Sketched at 194 P4. Shape:

- New skill at `.claude/skills/pipeline-walk/SKILL.md`,
  patterned after `/inspect`'s envelope (query echo, scan
  stats, stable IDs, narrative, suggested next queries).
- Pipelines: hunt, forage, herbcraft, build, disposal — one
  per spec. Implementation can be a single dispatcher script
  that selects per-pipeline schema.
- Per-step rate normalization shares utilities with the P3
  verdict.py work landed inline with 194.

## Out of scope

(Until unparked.) Implementation. This ticket records the
intent and the parking rationale.

## Verification

(Until unparked.) None.

## Log

- 2026-05-06: opened from 194's closeout. Parked immediately
  per recommended triage — defer until a second instance
  demands the per-pipeline funnel view. Cluster
  `process-discipline`.
