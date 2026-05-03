---
id: 151
title: Bugfix discipline — force a structural candidate in every fix-shape decision tree
status: done
cluster: process
added: 2026-05-03
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: pending
landed-on: 2026-05-03
---

## Why

While planning ticket 150 (cat starvation despite active food
production), my first pass enumerated three fix candidates — R1
(resolver patch in `resolve_engage_prey`), R2 (predicate polarity in
`eat.rs`), R3 (DSE scoring tweak) — all of them within the existing
DispositionKind / DSE / plan-template categorization. The user
flagged the gap with one sentence: *"eat should be separate from
resting no? This is multifaceted."* That surfaced **R5**, a
structural candidate that splits `Action::Eat` out of
`DispositionKind::Resting` so picking Eat at the L3 softmax doesn't
implicitly commit a hungry cat to Sleep + SelfGroom too.

R5 was load-bearing — the plan-duration cost asymmetry it removes is
invisible to the L3 softmax and was contributing to the structural
starvation pattern Bramble exhibited. **My first pass missed it
because I treated the existing DispositionKind enum as load-bearing-
and-correct rather than as a working hypothesis open to revision.**

The user names this as a class issue: *"This is a general problem
when we're bugfixing this project I'm noticing."* Existing categories
carry investigation-inertia. Each `DispositionKind` / `DSE` /
`Marker` / plan template is a frozen hypothesis from some prior
ticket that *somebody* believed was right. Bugs surface when a
hypothesis turns out wrong, but bugfix proposals are drafted *within*
the hypothesis-set rather than *against* it. The diagnostic toolchain
(`/logq`, `/inspect`, `/verdict`, `frame-diff`, `sweep-stats`,
`hypothesize`) all measure drift *within* the existing categorization
— there is no within-toolchain way to ask "is the categorization
itself wrong here?" So the path of least resistance is to score,
tune, or patch — never to revise the enum.

This is a process problem, not a tooling problem. The fix has to be
human-process: codified in `CLAUDE.md` and the ticket template,
treating any plan that doesn't include a structural-revision
candidate as incomplete by default.

## What this ticket lands

1. **`CLAUDE.md` doctrine update.** New section under "Long-horizon
   coordination" or adjacent to the existing substrate-vs-search-state
   rule:

   > **Bugfix discipline — structural candidates are non-optional.**
   > Every fix-shape decision tree in a ticket MUST list at least one
   > **structural** option (split / extend / rebind / retire an
   > existing DispositionKind, DSE, Marker, or plan template)
   > alongside parameter-level options. The structural option doesn't
   > have to win — it has to be drafted, named, and explicitly
   > considered. If you can't draft one, you haven't audited
   > `disposition.rs::from_action`, the plan templates in
   > `goap_plan.rs` / `ai/planner/`, or the completion proxies in
   > `commitment.rs` carefully enough.
   >
   > **Structural-option menu** (mirror in every fix tree):
   > - **split** — give the action its own DispositionKind / DSE /
   >   Marker variant.
   > - **extend** — keep the umbrella, branch the plan template /
   >   completion proxy / scoring shape on entry conditions so the
   >   umbrella varies by trigger. Modeled on the 148 distress →
   >   adrenaline-facet refactor.
   > - **rebind** — change the Action → Disposition (or sibling)
   >   mapping without inventing a new variant.
   > - **retire** — delete the variant entirely if the layer-walk
   >   shows it has no load-bearing job.
   >
   > **Layer-walk audit before fix candidates.** Walk L1 markers →
   > L2 DSE scores → L3 softmax selection → Action → Disposition
   > mapping → plan template → completion proxy → resolver. For each
   > layer, mark the relevant facts `[verified-correct]` or
   > `[suspect]` in the ticket's "Current architecture" section. A
   > plan that lists only resolver-level fixes against `[suspect]`
   > markers / mappings / templates has not been audited.
   >
   > Cite ticket 150 as the precedent — first plan undercaught R5
   > because it stayed inside the existing DispositionKind set.

2. **Ticket template update.** Update
   `scripts/scaffold_ticket.py` (or wherever the template lives) to
   include the layer-walk audit table skeleton and a
   "Structural option" slot in the fix-shape section.

3. **Memory note.** A `feedback_audit_l3_disposition_mapping.md` (or
   similarly-named) entry already exists in the global memory. Cross-
   reference from the CLAUDE.md doctrine so individual coding sessions
   pick it up via the auto-memory load.

## Proposed copy for CLAUDE.md (drop into a new section)

```
### Bugfix discipline

Every bugfix plan MUST include at least one structural-revision
candidate alongside parameter-level options. "Structural" means
split / extend / rebind / retire an existing DispositionKind, DSE,
Marker, or plan template. The structural candidate doesn't have to
ship — it has to be drafted, named, and explicitly considered.

Before listing fix candidates, walk every layer of the AI pipeline
(L1 markers → L2 DSE scores → L3 softmax → Action→Disposition
mapping → plan template → completion proxy → resolver) and tag each
load-bearing fact `[verified-correct]` or `[suspect]`. A plan that
lists only resolver-level fixes against `[suspect]` mappings has not
been audited.

Precedent: ticket 150 (cat starvation despite active food production).
The first plan listed R1 (resolver), R2 (predicate), R3 (scoring) —
all parameter-level. The user surfaced R5 (split Eat from Resting),
which turned out to be load-bearing. The lesson is now codified here.
```

## Out of scope

- The actual layer-walk audit on existing tickets (that's ticket 152
  for the tier-1-collapse pattern; future tickets for other classes).
- Tooling changes to `/logq` / `/diagnose-run` / etc. The discipline
  is human-process — no new tools needed.

## Log

- 2026-05-03: Opened as a 150-landing sibling per CLAUDE.md
  "antipattern migration follow-ups are non-optional" — 150's R5
  surfacing made it clear the category-revision discipline needs to
  be canonized before the next bugfix repeats the same mistake.
- 2026-05-03: Landed. CLAUDE.md gains a `## Bugfix discipline`
  section between "Long-horizon coordination" and "ECS rules" with
  the structural-option menu (split / extend / rebind / retire) and
  the layer-walk audit prescription. New
  `docs/open-work/tickets/_template_bugfix.md` embeds the layer-walk
  audit table and parameter-vs-structural fix-candidate split. The
  general `_template.md` stays lean for non-bugfix work. The
  existing user-global memory entry
  (`feedback_audit_l3_disposition_mapping.md`) is cited by name from
  the new doctrine; no new memory file. No tickets had `blocked-by:
  [151]`, so no dependents needed unblocking.
