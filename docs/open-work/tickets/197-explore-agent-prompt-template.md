---
id: 197
title: Explore-agent prompt template (194 P8, subsumes P6)
status: ready
cluster: process-discipline
added: 2026-05-06
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Closes 194 F5 / F9 / P6 / P8. Per 194 §F9, the prompts I write
for Explore / general-purpose sub-agents *are* my perception
layer — what I name to a sub-agent bounds what it can perceive,
and bad framing produces bad sense data, which produces bad
decisions one layer up.

The 189 cluster contains two concrete failures of this kind:

- **Wave-closeout-shape agent.** Asked for "the structural
  shape of the wave" with named files; agent returned a
  faithful description that included the load-bearing WRONG
  claim that PickingUp's plan template "already plans
  TravelTo(MaterialPile) → PickUpItemFromGround — eligibility
  gate alone enables the disposition." The agent inherited my
  premise that the wave was structurally sound; my prompt
  didn't ask "what could go wrong with this plan?".
- **Hypothesis-test agent.** I framed it as "test this single
  hypothesis." Single-hypothesis framing produces confirmation-
  vs-falsification, not divergent search across mechanisms.
  Memory `feedback_subagents_inherit_premises.md` already names
  this; the discipline of applying it isn't automatic.

This ticket builds a prompt template (or skill) that turns the
existing memory feedback into a checked discipline at the
prompt boundary — same way `_template_bugfix.md` turns bugfix
discipline into a fillable form.

This is the **largest-leverage process change** in the 194
catalog. Every future investigation that uses sub-agents
inherits its quality from this template.

## Direction

Build a checklist (or template skill) for sub-agent dispatch
prompts. Five required slots from 194 §P8:

1. **Load-bearing facts marked** with `[HYPOTHESIS — please
   verify]` — implements existing memory
   `feedback_subagents_inherit_premises.md` as a template gate
   rather than a hope.
2. **Field-name validation step** — agent must confirm any
   data-key path against actual file content before reading,
   not after. (Closes 194 F1 — the `feature_counts` silent-zero
   trap.)
3. **Alternative-mechanism slot** — for any "test this
   hypothesis" task, the prompt requires the agent to enumerate
   2+ candidate mechanisms and discriminate, not just confirm
   or falsify the named one. (Subsumes 194 P6.)
4. **Skill-surface escape clause** — explicit *"if `/logq` /
   `/sweep-stats` don't cover this, write the analysis directly"*
   permission, so the skill-surface preference doesn't become
   a tunnel. (Closes 194 F7.)
5. **Ratio normalization for cross-run comparison** — if the
   task involves comparing two runs, default to per-tick rates
   unless raw counts are explicitly meaningful. (Closes 194
   F2 — ties to the verdict.py P3 work landed inline with 194.)

### Implementation options

The template lives at one of:

- **`docs/open-work/_template_subagent_prompt.md`** — fill-in-
  the-blanks document. Cheap to land, but discipline is purely
  cultural.
- **`.claude/skills/agent-prompt/SKILL.md`** — a skill the user
  invokes that produces a structured prompt scaffold from a
  short brief. Costlier but enforces the slots at composition
  time.
- **CLAUDE.md addition under Bugfix discipline** — hardest to
  miss but blunt; doesn't structure the slot fill.

Recommend (A) + (C): land the document, link from CLAUDE.md.
Skip (B) initially — the template is text, not workflow; a
skill adds friction without clear payoff for a thing the
agent author writes once per investigation.

## Out of scope

- Retraining sub-agents themselves (their behavior is
  inherited from the prompt; this ticket fixes prompts, not
  agents).
- Auditing every existing sub-agent prompt in `.claude/agents/`
  for compliance — the template is the contract going forward.
- Automating prompt validation (would need a hook on the Agent
  tool; out of band for this ticket).

## Verification

- `_template_subagent_prompt.md` exists and is linked from
  CLAUDE.md "Bugfix discipline" section near the layer-walk /
  reframe-discipline paragraphs.
- Template includes all 5 slots above with worked examples
  drawn from 194's §F9 self-critique.
- Cross-reference: `feedback_subagents_inherit_premises.md`,
  `feedback_promote_audit_rows_first.md`, and
  `feedback_use_skill_surface.md` (the skill-surface tunnel
  exception) link from the template so the discipline is
  internally consistent.

## Log

- 2026-05-06: opened from 194's closeout. Subsumes P6
  (alternative-mechanism rule) as one of the five required
  template slots. Cluster `process-discipline`. Author flags
  this as the largest-leverage item in the 194 catalog —
  every future investigation using sub-agents inherits its
  quality from this template.
