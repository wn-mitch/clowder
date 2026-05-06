# Sub-agent prompt template

Use this checklist before dispatching every Explore / Plan / general-purpose
sub-agent. Sub-agents elaborate on the framing they're handed; they rarely
challenge a wrong premise presented as established. The prompt **is** their
perception layer — bad framing produces bad sense data, and the failure
propagates one layer up into your own reasoning.

The five slots below MUST be addressed in every sub-agent prompt that bears
on a non-trivial investigation. If a slot is genuinely N/A, say so in the
prompt — silence is indistinguishable from oversight.

Pairs with `_template_bugfix.md`'s discipline: that template forces the
audit on the human-author side; this one forces the audit at the prompt
boundary.

---

## Slot 1 — Mark load-bearing facts as hypotheses

Every load-bearing claim the agent will build on must be tagged exactly one
of:

- `[verified — see <query / file:line>]` — you've checked this yourself.
- `[HYPOTHESIS — please verify before proceeding]` — agent must confirm
  before any conclusion depends on it.
- (Omit ambiguous claims entirely.)

The agent's first task on a `[HYPOTHESIS]` row is to verify it via the
cheapest decisive query (source read, log scan, unit-test demonstration).

**Failure mode this prevents** (194 §F5, ticket 158): handing an agent the
unverified premise `"(38, 22) is peripheral to the colony"` as background
context produces a confident, well-organized R3 (Den-anchored spawn) plan
that elaborates 800 words of internal logic on a wrong foundation. The
agent didn't push back because background context reads as fact.

**Memory:** `feedback_subagents_inherit_premises.md`,
`feedback_agent_prompts_are_perception.md`.

---

## Slot 2 — Field-name validation step

When the prompt directs the agent to read a data-key path
(`footer.feature_counts.X`, `events[].kind == "Y"`, `header.constants.Z`),
require the agent to **confirm the field exists** in the actual file before
trusting any value it reads. `dict.get(key, default=0)` returns silently on
typos.

Phrasing: *"Before reading `<path>`, list one example record from the file
that confirms the path exists. If the path is absent, report that and stop
— don't return zero."*

**Failure mode this prevents** (194 §F1): I confidently reported
"ItemDropped=0, ItemTrashed=0, ItemHandedOff=0" because my code read
`footer['feature_counts'][...]` — a key path that never existed in the
schema. True values were `OverflowToGround` 467/10kt → 956/10kt (+104%).
A field-existence check at agent dispatch would have surfaced the regression
on day 1.

---

## Slot 3 — Alternative-mechanism slot

For "test this hypothesis" / "verify this claim" tasks, the prompt MUST
require the agent to enumerate ≥ 2 candidate mechanisms that could produce
the observation and design a discriminating query. Single-hypothesis framing
produces confirmation-vs-falsification, not divergent search across
mechanisms.

Phrasing: *"List ≥ 2 candidate mechanisms that could produce the observed
effect, then design a query whose result would *distinguish between them*
(not just support or falsify the named one)."*

**Failure mode this prevents** (194 §F5, F6, F9): when an Explore agent was
asked to verify "Guarding elections drove WildlifeCombat," the agent
dutifully tested it. The verdict was honest ("indeterminate at n=5"), but
the framing prevented divergent thinking about what ELSE might be driving
WildlifeCombat. Compounded by §F6: when v1 was falsified, the same evidence
base was treated as supporting v2 — but compatible-with-v2 ≠ exclusive-of-v3.

---

## Slot 4 — Skill-surface escape clause

If the prompt directs the agent into the skill surface (`/logq` /
`/sweep-stats` / `/inspect` / `/verdict` / etc.), include an explicit
escape: *"If no listed skill produces the analysis you need, write the
analysis directly using <Read / Grep / Bash / etc.> rather than forcing it
through a near-fit skill. Note the gap so the skill surface can grow."*

**Failure mode this prevents** (194 §F7): the bug that surfaced 193 was
found by writing a multi-row hunt-pipeline ratio table — there's no skill
for that. Reaching for `/logq footer --field=...` repeatedly never produced
the unified pipeline view; the user had to ask for it directly to break me
out of the skill-surface tunnel. Don't transmit that tunnel to the agent.

**Memory:** `feedback_use_skill_surface.md` — the rule itself, including
the "only escape hatch" line.

---

## Slot 5 — Ratio normalization for cross-run comparison

If the task involves comparing two runs (PRE vs POST, baseline vs treatment,
seed-A vs seed-B), the prompt MUST default to per-tick rates, not raw
counts — unless raw counts are explicitly meaningful (e.g., singleton
events that should fire exactly once).

Phrasing: *"Report all cross-run deltas as per-tick rates (count ÷
ticks_observed). If raw counts are more meaningful for a metric, name it
and justify."*

**Failure mode this prevents** (194 §F2): PRE soak survived 2 seasons (~42k
ticks); POST 1 season (~24k ticks). I compared raw counts and reported
metric drift in raw units until the user explicitly demanded ratios. Every
continuity tally, plan-failure count, and action share was systematically
misleading. Different-duration soaks are common when the SUT collapses
early — the wrong default is dangerous.

---

## Pre-dispatch checklist

Before sending the prompt, walk it once:

- [ ] Slot 1: every load-bearing fact tagged `[verified — …]` or
      `[HYPOTHESIS — please verify]`?
- [ ] Slot 2: every data-key path the agent will read has a field-existence
      check ahead of it?
- [ ] Slot 3: if this is a hypothesis-test task, has the agent been asked
      to enumerate ≥ 2 mechanisms and discriminate?
- [ ] Slot 4: if the prompt names skill-surface tools, is the escape hatch
      explicit?
- [ ] Slot 5: if the task spans runs, does the prompt default to per-tick
      rates?

A `[no]` on any slot is a perception gap that will propagate.

---

## Cross-references

- `feedback_subagents_inherit_premises.md` — the rule (Slot 1).
- `feedback_agent_prompts_are_perception.md` — the framing (this template's
  origin in 194 §F9).
- `feedback_promote_audit_rows_first.md` — companion discipline on the
  human-author side; pairs with Slot 1.
- `feedback_use_skill_surface.md` — the rule the Slot 4 escape clause
  excepts.
- `docs/open-work/landed/194-meta-analysis-189-diagnostic-delay.md` §F1 /
  §F2 / §F5 / §F7 / §F9 — the source incidents for each slot's worked
  example.
- `docs/open-work/tickets/_template_bugfix.md` — the parallel
  human-author-side template (audit table + structural-option slot).
