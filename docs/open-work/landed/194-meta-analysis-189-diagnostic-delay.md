---
id: 194
title: Meta — why 189's root cause took three reframes and a full wave-closeout to surface
status: done
cluster: process-discipline
added: 2026-05-06
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: pending
landed-on: 2026-05-06
---

## Why

189 went through three reframes (RNG noise → scoring-substrate
expansion → 193's PickingUp/MaterialPile zone mismatch) and an
entire wave-closeout (179 + 185 + 188) before the actual root
cause was identified. The mechanism — a plan-template stub the
176 author flagged in plain English ("Reuses MaterialPile zone
until a more general TargetGroundItem zone lands. Default-zero
scoring keeps this dormant.") — was sitting in the source the
whole time and got missed because 185 lifted the curve without
re-reading the stub's caveat.

This won't be the last issue of this shape. The substrate
refactor has 80+ open tickets, many touching plan templates
that were stubbed out at one phase and assumed-stable at the
next. The cost of *each* round of mis-reframe is at least a
soak's worth of compute plus a wave of structural changes
shipped on a wrong premise. We need process changes that
shorten this loop next time.

This ticket is meta-analysis — process proposals, not code.

## Timeline of the 189 cluster

1. **178 lands** disposal substrate (Discarding/Trashing curve
   lift, HasMidden, etc.), wires Handing/PickingUp eligibility
   filters with `require(HasHandoffRecipient)` /
   `require(HasGroundCarcass)`. Allowlists those markers as
   spec'd-but-unwired.
2. **178 closeout soak** shows seed-42 regression: nourishment
   collapsed, food_available dropped 71% → 22%. Disposal
   Features fire 0× — substrate is correctly dormant.
3. **189 opened** to diagnose. v1 hypothesis: "schedule-edge
   perturbation from 178's `Has<HasMidden>` query expansion
   shifted seed-42 RNG roll." User rejected; reopened via
   user directive.
4. **189 Phase A** — diagnostic loops attempt to falsify
   alternative hypotheses. v2 conclusion lands: scoring-substrate
   expansion (new ctx_scalars + marker queries) shifts L3
   election landscape even when DSEs eligibility-reject.
5. **Wave-closeout planned and executed** on v2 premise: 179
   + 185 + 188 land consumers so the substrate fires "load-
   bearing" instead of "phantom." Plus `~/.claude/plans/...md`
   plan, plus 4 follow-on tickets opened (190–192).
6. **Post-wave seed-42 soak verdict: FAIL.** seasons_survived
   2 → 1; nourishment 0.52 → 0.22 (-57%); deaths_injury +33%;
   continuity (mythic-texture) collapses 4 → 0.
7. **User pushes harder**: "Why is nourishment so low? Break
   down EVERY step of the hunt pipeline pre/post — use ratios."
8. **Hunt-pipeline ratio walk** (per 10k ticks) surfaces:
   `PickingUp:GoalUnreachable` 0 → 1367/10kt (NEW). 185 lifted
   PickingUp's curve, but its plan template routes through
   `PlannerZone::MaterialPile`, which only resolves to ground
   items where `kind.material().is_some()` — Wood/Stone, NOT
   carcasses. The 176 stub flagged this; 185 ignored it.
9. **193 opened** with the actual mechanism + four structural
   fix candidates.

**Cost of the wrong reframes:** 4 commits (178+189, 179, 185,
188), 1 full 15-min soak, 1 multi-seed sweep, three follow-on
tickets opened on the wave-closeout premise (190/191/192).
Some of that work is still load-bearing (substrate is allowlist-
clean now), but the headline bug — the cause of the seed-42
collapse — was never the wave's substrate behavior; it was
185's missing zone, which existed as a known stub the whole
time.

## What slowed the diagnosis

These are the friction points that turned the 189 cluster into
a multi-day investigation when the answer was a one-line stub
comment. Each is a candidate for process change.

### F1. Wrong-key silent-zero in metric reads

Mid-investigation I confidently reported "ItemDropped=0,
ItemTrashed=0, ItemHandedOff=0, ItemRetrieved=0,
OverflowToGround=0 across all seeds" — used this to argue the
substrate wasn't firing. The actual key path is
`SystemActivation` events' `positive`/`negative` cumulative
maps; my code read `footer['feature_counts'][...]` which never
existed and silently returned 0 via `.get(default=0)`.

True values were: `OverflowToGround` 467/10kt PRE → 956/10kt POST
(+104%); `ItemRetrieved` 6 → 2 (-66%). Those numbers would have
pointed at the PickingUp regression on day 1 of 189.

**Process gap:** no sanity-check that the field names I was
reading actually existed in the data.

### F2. Tick-normalization not applied by default

PRE soak survived 2 seasons (~42k ticks); POST 1 season (~24k
ticks). I compared raw counts and reported metric drift in raw
units until the user explicitly demanded ratios. Raw-count
comparisons at unequal durations are misleading at every step
— continuity tallies, plan-failure counts, action shares.

**Process gap:** `just verdict` and informal soak comparisons
default to raw counts. Different-duration soaks are common
when the SUT collapses early.

### F3. Underpowered sweeps used as evidence basis

189 v1's "RNG noise" verdict came from a 5-seed × 300s sweep.
The 300s window cannot exercise the disposal substrate at all
— `ColonyStoresChronicallyFull` needs chronicity windowing,
`HasGroundCarcass` needs accumulated overflow, `HasHandoffRecipient`
needs kittens to be born. The sweep measured schedule-edge
RNG perturbation, not substrate behavior, and was treated as
authoritative for both.

**Process gap:** 300s is too short to verify any substrate
whose triggering conditions are rare. We need a "substrate-
firing precondition check" before claiming a sweep verdict.

### F4. Plan-template stub comment got missed twice

`src/ai/planner/actions.rs:274-285` carries a clear stub
warning:

> *"Reuses `MaterialPile` zone (the existing OnGround-item zone
> resolution) until a more general `TargetGroundItem` zone
> lands. Default-zero scoring keeps this dormant."*

185 (the wave-closeout step that lifted the curve) read this
file (had to — it edited the eligibility filter in
`src/ai/dses/picking_up.rs`) but didn't connect "default-zero
scoring keeps this dormant" with "we're about to make scoring
non-zero." The comment named the prerequisite (`TargetGroundItem`
zone) but didn't ENFORCE it.

**Process gap:** stub comments are documentation, not gates.
A comment that says "default-zero keeps this dormant" should
be a hard prerequisite — lift the curve only when the named
successor lands.

### F5. Sub-agents inherited my framing

When I asked an Explore agent to verify the user's "Guarding
elections drove WildlifeCombat" hypothesis, I framed it as a
hypothesis to test. The agent dutifully tested it. I didn't
ask "what other mechanism could produce these deltas?" — only
"is this mechanism present?" The agent's verdict ("indeterminate
at n=5") was honest but the framing prevented divergent thinking.

This is a known pattern — feedback memory `feedback_subagents_inherit_premises.md`
already names it. I had the memory and didn't apply it.

**Process gap:** following memory is non-automatic. Memory
exists; the discipline of applying it doesn't.

### F6. "Verified" treated as transitive across reframes

189 v2 (substrate-expansion) was an improvement on v1 (RNG
noise) but the same evidence base (5×300s sweep) was used to
support v2's framing. Because v1 was falsified, I treated v2's
acceptance as solid. The same evidence pool can support any
number of compatible-but-incomplete framings. The actual
falsification of v2 required different evidence (15-min soak +
hunt-pipeline ratio walk).

Memory feedback `feedback_promote_audit_rows_first.md` says:
*"`[suspect]` rows are not evidence; promote each to
`[verified-correct]`/`[verified-defect]` via a concrete query
before any candidate that depends on the row."*

189's audit table promoted the schedule-edge row to
`[verified-defect]` based on plan-divergence evidence, but
that evidence didn't distinguish v1 from v2 from v3. The
upgrade was premature.

**Process gap:** "verified" is single-mechanism; the evidence
needs to *exclude* alternatives, not just *support* the chosen
one.

### F7. Skill-surface preference vs ad-hoc analysis

The "use skill surface for log queries" rule
(`feedback_use_skill_surface.md`) is right for narrow drill-
down. But the bug that surfaced 193 was found by writing a
multi-row hunt-pipeline ratio table — there's no skill for
that. Reaching for `/logq footer --field=...` repeatedly never
produces the unified pipeline view; the user had to ask for it
directly to break me out of the skill-surface tunnel.

**Process gap:** the skill surface doesn't cover production-
pipeline / funnel-style cross-step analysis. A new skill (or
a published recipe) for "walk the hunt pipeline, normalize per
tick, list every step's failure rate" would have surfaced 193
in minutes.

### F9. Explore-agent prompts as the perception layer

User insight (2026-05-06): *"the instructions you gave the
explore agents — those are your way of experiencing the world.
just like how our cats need good senses, you need good context."*

Reviewing this session's two Explore-agent prompts:

- **Wave-closeout-shape agent** — I asked for "the structural
  shape of the wave," named the files to skim, and forbade
  proposing code or balance changes. The agent returned a
  faithful description of the wave including the load-bearing
  WRONG claim — that PickingUp's plan template "already plans
  TravelTo(MaterialPile) → PickUpItemFromGround — eligibility
  gate alone enables the disposition." That's the same mistake
  185's commit made. The agent inherited my premise that the
  wave was structurally sound; my prompt didn't ask "what could
  go wrong with this plan?" — it asked "what does this plan
  look like?"
- **Hypothesis-test agent** — I framed it as "test this single
  hypothesis." Single-hypothesis framing produces confirmation-
  vs-falsification, not divergent search across mechanisms.
  Memory `feedback_subagents_inherit_premises.md` already names
  this; I didn't apply it.

Cross-cutting prompt failures I owe the agents:
- **No field-name premise validation.** I assumed `feature_counts`
  was the disposal-Feature footer key. I never told the agents
  to verify field names against actual data, so my error
  propagated.
- **No alternative-mechanism slot.** I didn't say "list 2+
  candidate mechanisms that could produce the observation."
- **No "challenge load-bearing facts."** Memory says label load-
  bearing claims as `[HYPOTHESIS — please verify]`; I didn't.
- **Ratio-normalization not requested upfront.** Both pre/post
  comparisons were given to the agents in raw counts.
- **Skill-surface tunnel inherited.** I told the agents to go
  through `/logq` etc.; they had no escape hatch when those
  didn't fit. Same F7 trap, transmitted.

The pattern: **what I name to my sub-agents bounds what they
can perceive.** Bad framing produces bad sense data, which
produces bad decisions one layer up. The cats can't smell a
threat outside their range; my agents can't audit a premise I
present as background.

This shape will recur. Its proper treatment is its own ticket
— deferred (see P8).

### F8. Substrate-fire verification absent from wave landing gates

When 185 / 188 landed, the verification was: `just check`
clean, unit tests pass, allowlist drops. Nothing in the
landing flow checks that the substrate's *Feature events
actually fire ≥ 1×* in a representative scenario. The wave
shipped with PickingUp's L2/L3 path successfully reaching
plan-creation and then failing every plan unreachable —
invisible to landing gates.

The CLAUDE.md "never-fired canary" only fires AFTER a soak,
and only on positive Features expected to fire per soak (which
the disposal Features aren't, because they're conservative).

**Process gap:** wave-closeout shipping bar is "lint clean +
unit tests" instead of "substrate fires its Feature event in
at least one scenario." A scenario test that exercises the
end-to-end L2→L3→Plan→Step→Witness chain would have caught
the MaterialPile mismatch at land time.

## Process proposals

These are sketches, not specs. Each would warrant its own
ticket if accepted.

### P1. "Substrate-fires" landing gate

When a DSE curve is lifted from default-zero (or a new DSE is
registered), the same commit must include a deterministic
scenario test that asserts the corresponding `Feature::*`
emits ≥ 1× when conditions are set up to exercise it. The
curve shape + eligibility tests (which 185 / 188 included) are
necessary but insufficient.

Implementation: extend `scripts/check_substrate_stubs.sh`-style
lint to track DSE curve-non-zero status and require a sibling
scenario file under `src/scenarios/` that exercises the DSE,
referenced by `expected_features` metadata.

### P2. Plan-template-stub gate

Extend the substrate-stub lint to scan
`src/ai/planner/actions.rs` and `src/ai/planner/goap_plan.rs`
for plan-template comments containing strings like "stub,"
"default-zero keeps this dormant," or "until X lands." Each
match becomes an allowlist entry that retires when:
- The DSE that uses the plan-template moves to non-zero
  scoring, AND
- The named successor (e.g., `TargetGroundItem`) lands.

Failure mode this prevents: 185-shape regression where curve
lift ships without prerequisite plan-template work.

### P3. Tick-normalized verdict

`just verdict` already compares against a baseline footer.
Extend it to ALSO publish per-10kt rates for every metric, and
flag direction-of-drift on the rates rather than raw counts
when run-durations differ by more than ~10%.

Implementation: small change to
`scripts/verdict.py` (or wherever the verdict logic lives) to
divide by `final_tick - start_tick` per metric.

### P4. Hunt / production / consumption pipeline-walk skill

New `/pipeline-walk` skill (or extend `/logq` with a `funnel`
subcommand) that takes a run-dir (or two run-dirs) and emits:
the per-step rate-normalized counts for the full
hunt → kill → carry → cook → eat / dispose chain. Each step
labeled with its dispatch arm and failure modes. Same shape
for forage / herbcraft / build pipelines.

This would directly produce the kind of table that surfaced
193, on demand, without ad-hoc Python.

### P5. Reframe-discipline addition to CLAUDE.md

Add to "Bugfix discipline" section: when reframing a hypothesis
(v1→v2 etc.), the audit-table rows promoted to `[verified-...]`
under the prior framing must be RE-PROMOTED under the new
framing — same row, fresh query that distinguishes the new
framing from the old. "Verified for v1" is not "verified for
v2." This catches the F6 trap.

### P6. "What other mechanism could produce this?" sub-agent prompt

When asking a sub-agent to test a hypothesis, the prompt
template should require listing 2+ alternative mechanisms
that could produce the same observation, and the test should
attempt to discriminate. This is divergent thinking enforced
at the prompt boundary.

Connects to existing memory `feedback_subagents_inherit_premises.md`
but turns it into a checked discipline.

### P8. Explore-agent prompt template

Build a checklist (or template skill) for sub-agent dispatch
prompts. Required slots:
- **Load-bearing facts marked** with `[HYPOTHESIS — please
  verify]` — implements existing memory
  `feedback_subagents_inherit_premises.md` as a template gate
  rather than a hope.
- **Field-name validation step** — agent must confirm any
  data-key path against actual file content before reading,
  not after.
- **Alternative-mechanism slot** — for any "test this
  hypothesis" task, the prompt requires the agent to enumerate
  2+ candidate mechanisms and discriminate, not just confirm
  or falsify the named one.
- **Skill-surface escape clause** — explicit "if `/logq` /
  `/sweep-stats` don't cover this, write the analysis directly"
  permission, so the skill-surface preference doesn't become
  a tunnel.
- **Ratio normalization for cross-run comparison** — if the
  task involves comparing two runs, default to per-tick rates
  unless raw counts are explicitly meaningful.

Connects to F9 directly. This is the largest-leverage process
change in the catalog because every future investigation
that uses sub-agents inherits its quality from this template.

### P7. Soak-duration adequacy probe

Before declaring a sweep authoritative, run a quick "did the
substrate I'm investigating actually fire?" probe. If the
disposal substrate fires 0× across all seeds, the sweep cannot
verify or falsify hypotheses about disposal-DSE behavior. The
sweep can still verify schedule-edge / RNG-stream effects, but
those are downstream and noisy.

Implementation: extend `just verdict` or sweep-stats to
report which substrate Features fired ≥ 1× in any seed. If a
hypothesis names a Feature that fired 0× across all seeds, flag
it as "unprovable at this duration."

## Out of scope

- Implementing any of P1–P7 in this ticket. Each is its own
  follow-on if accepted. This ticket only documents the
  proposals.
- Reverting the wave-closeout work (179/185/188). The work
  itself was load-bearing for substrate hygiene; the regression
  it surfaced is the load-bearing finding from 193, not a
  reason to revert.
- Disciplining the 189 ticket body further — it now carries the
  v1→v2 reframe history; v3 lives in 193.

## Verification

This ticket is meta — verification is acceptance of the
process proposals (P1–P7) by the user, with follow-on tickets
opened for those that ship. No code, no soak.

## Log

- 2026-05-06: opened. The 189 cluster (178/189 closeout,
  179/185/188 wave-closeout, 193 root-cause ticket) finished
  with the actual defect identified — but the path to it cost
  4 commits and a wave of structural change shipped on the
  wrong premise. Eight friction patterns (F1-F8) are
  candidate process targets; seven proposals (P1-P7) sketch
  fixes. None of P1–P7 are mandatory; this ticket exists so
  the next time we hit a multi-reframe diagnostic, we have a
  list of disciplines to lean on rather than re-discovering
  these gaps.
- 2026-05-06: F9 + P8 added during user review — the Explore-
  agent prompt as a perception layer. Shipped inline with the
  closeout: P5 (CLAUDE.md "Bugfix discipline" reframe-
  discipline paragraph) and P3 (verdict.py per-tick rate
  normalization with `duration_drift_pct` + per-row `band_rate`,
  rate-band escalates `derive_overall` only when durations
  diverge >10%). Opened follow-ons under new cluster
  `process-discipline`: 195 (P2 plan-template stub-comment lint
  extension), 196 (P7 substrate-fired-≥1× probe), 197 (P8
  Explore-agent prompt template; subsumes P6), 198 (P1
  substrate-fires landing gate). Parked 199 (P4 pipeline-walk
  skill) — defer until a second instance demands the per-
  pipeline funnel view to avoid over-design from a single
  episode. Plan: `~/.claude/plans/work-194-functional-shannon.md`.
