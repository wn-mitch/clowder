# Bugfix discipline

Every bugfix plan MUST include at least one **structural-revision candidate** alongside parameter-level options.

"Structural" means one of: **split / extend / rebind / retire** an existing `DispositionKind`, DSE, Marker, or plan template. The structural candidate doesn't have to ship — it has to be drafted, named, and explicitly considered. If you can't draft one, you haven't audited `src/components/disposition.rs::from_action`, the plan templates under `src/ai/planner/` (and `goap_plan.rs`), or the completion proxies in `src/components/commitment.rs` carefully enough.

## Structural-option menu

Mirror this in every fix-shape decision tree:

- **split** — give the action its own `DispositionKind` / DSE / Marker variant. (Precedent: ticket 150 R5a, `Eat` out of `Resting`.)
- **extend** — keep the umbrella, branch the plan template / completion proxy / scoring shape on entry conditions so the umbrella varies by trigger. (Precedent: ticket 148 distress → adrenaline-facet refactor.)
- **rebind** — change the Action → Disposition (or sibling) mapping without inventing a new variant.
- **retire** — delete the variant entirely if the layer-walk shows it has no load-bearing job.

## Layer-walk audit before listing fix candidates

Walk **L1 markers → L2 DSE scores → L3 softmax → Action→Disposition mapping → plan template → completion proxy → resolver.** For each layer, mark the relevant facts `[verified-correct]` or `[suspect]` in the ticket's "Current architecture" section. A plan that lists only resolver-level fixes against `[suspect]` mappings or templates has not been audited.

Use `just similar <ticket-id>` during the layer-walk to surface prior tickets and balance threads with the same behavioral signature — finds precedent for split / extend / rebind / retire decisions before the structural-option menu is drafted.

## Reframe discipline

When a hypothesis upgrades (v1→v2), `[verified-...]` rows promoted under the prior framing are **not transitively verified** — re-promote each via a fresh query that *distinguishes* v2 from v1 before any candidate depends on them. The same evidence pool can support compatible-but-incomplete framings; falsifying v1 doesn't license v2.

Precedent: ticket 189 v1→v2 reframe carried the schedule-edge row's verification across; the actual defect in 193 required different evidence to surface.

## Scenario microexperiment before a soak

Once the layer-walk identifies the suspect mapping/template/scoring, isolate the question with `just scenario <name>` (or define a new scenario under `src/scenarios/`) instead of running `just soak`. The harness preloads 1–5 cats with specific needs/personality/markers/positions and prints the focal cat's per-tick winning DSE + ranked L2 score table in ~3 seconds — the right tool for "given this state, which DSE wins?" triage.

Reach for `just soak` only when the bug genuinely requires whole-colony dynamics (continuity canaries, drift, multi-system interaction) — and state that explicitly in the ticket's investigation section so future readers see why the cheaper tool was skipped.

Ticket 162 ships the harness + 7 archetype scenarios.

## Sub-agent dispatch discipline

Before delegating any non-trivial investigation to an Explore / Plan / general-purpose sub-agent, walk [`docs/open-work/_template_subagent_prompt.md`](../open-work/_template_subagent_prompt.md) — five required slots:

1. Mark load-bearing facts as hypotheses.
2. Field-name validation.
3. Alternative-mechanism enumeration.
4. Skill-surface escape clause.
5. Ratio normalization for cross-run comparison.

The prompt IS the agent's perception layer; bad framing produces bad sense data, and the failure propagates one layer up. Precedent: ticket 194 §F9 — the 189-cluster diagnostic delay traces back to two Explore-agent prompts that inherited the wrong premise as established context.

## Open-time prefill for collapse tickets

When a session ends in a failing soak, open the bugfix ticket via `/ticket-from-session "<title>"` ([`.claude/commands/ticket-from-session.md`](../../.claude/commands/ticket-from-session.md)). The skill:

1. Detects the failing run.
2. Composes `just verdict`, `just fingerprint`, `just q run-summary`, `just q deaths`, `just q anomalies` into a hot-context payload.
3. Dispatches a Plan agent against the five-slot scaffolding.
4. Post-edits the new bugfix ticket to splice in `## Hot context` + a promoted layer-walk + a draft structural-option menu.

Promoting `[needs-promote]` → `[verified-*]` in the next session is non-optional and uses fresh queries (per Reframe discipline). The skill refuses to open a ticket if no failing run is present — it's a session-failure tool, not a generic shortcut. (Wired here per `feedback_diagnostic_tools_need_discipline_wiring`.)

## All multi-tick aspirations are HTN methods

Any ticket proposing new per-cat multi-step goal-shaped commitment substrate (a new Component carrying "I am pursuing X across N ticks") must either:

(a) Author an HTN method in `populate_method_registry`, OR
(b) Be a mirroring projection of an existing method (like `JointIntention` is of the Courtship method).

Naked aspiration Components — multi-tick goal state with no method-registry entry — are forbidden by the same enforcement strength as substrate stubs. Enforcement: `scripts/check_method_registry.sh` (lands with [319](../open-work/landed/319-method-registry-populate-no-stub-enforcement.md)). Design home: [`docs/systems/htn-methods.md`](../systems/htn-methods.md); epic dashboard: [128](../open-work/tickets/128-htn-method-composition.md).

## Every dormant method has a glue ticket

A method registered as `ApplicableWhen::PendingSubstrate { blocker }` must have its `blocker` field point to an **open** ticket in `docs/open-work/tickets/` whose frontmatter carries `wires-method: [<method-id>...]` referencing back. If the wiring ticket doesn't exist when authoring the dormant method, open it in the same commit.

The registry script enforces both directions: dormant method without glue ticket fails CI; glue ticket without matching method-id in frontmatter fails CI.

Without this discipline, design intent for arcs the sim *could* express rots — methods describe natural narrative trees that never sprout because nobody trips over the design intent in their work surface (`just open-work-ready` / `just next` / `just similar` don't surface registry-internal state).

Precedent: 128 epic's Tier-2 dormant methods (332/333/334) all carry `wires-method` frontmatter from open-time.

## Bugfix ticket template

Use [`docs/open-work/tickets/_template_bugfix.md`](../open-work/tickets/_template_bugfix.md) — embeds the layer-walk table and structural-option slot.

## Precedent

Ticket 150's first plan listed R1 (resolver) / R2 (predicate) / R3 (scoring), all parameter-level; the user surfaced R5 (split Eat from Resting), which was load-bearing. The same lesson lives in the auto-memory entry "Audit L3 Action→Disposition mapping when investigating Clowder AI defects" at the user-global layer.
