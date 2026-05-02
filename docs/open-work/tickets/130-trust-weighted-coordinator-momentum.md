---
id: 130
title: Trust-weighted coordinator directive momentum
status: blocked
cluster: C
added: 2026-05-02
parked: null
blocked-by: [126, 057]
supersedes: []
related-systems: [ai-substrate-refactor.md, strategist-coordinator.md, scoring-layer-second-order.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Spun out of 126 (`## Out of scope`). 126 commits the
`IntentionSource` provenance field on `HeldIntention` and a
source-aware read-site on `IntentionMomentum`. 057 emits
coordinator-directive intentions. (081 — coordinator-side
failure demotion — was retired without implementation 2026-05-02;
130 no longer pairs with it.) 130 is the axis that closes the loop
— directive lift scales with recipient trust, so high-trust
coordinators' orders override marginal scores while low-trust
coordinators' orders mostly fail to adopt.

Enables observable good-vs-bad coordinator effects without an
out-of-fiction director. Per CLAUDE.md "no director" doctrine: a
coordinator's directive is perceivable substrate that recipients
score and may refuse — and *whether* they refuse depends on the
recipient's trust in that coordinator, which is itself
substrate-driven (built up from witnessed competence).

The shape this ticket has to commit:

- A `CoordinatorTrust` Component (or per-cat `BeliefsAbout<Entity>`
  facet, depending on whether C3 mental models has landed) holding
  recipient → coordinator → trust score in `[0, 1]`.
- Trust update rules: positive on `IntentionFulfilled` for
  directives sourced from that coordinator; negative on
  `IntentionAbandoned { reason: BecameImpossible | TargetInvalid }`
  attributed to the directive (the coordinator told you to do
  something that turned out to be wrong).
- Modifier extension: `IntentionMomentum` reads
  `HeldIntention.source`; for `CoordinatorDirective(coord)`,
  multiply the lift by `recipient.trust(coord)`. Default to 0.5
  for unknown coordinators (neutral start; first directive's
  outcome calibrates).
- Footer-line addition: `directive_compliance_by_coordinator` —
  per-coordinator adoption-rate, fulfillment-rate, and abandonment
  reasons. Substrate for post-hoc narrative ("Coordinator Whisker's
  directives were ignored 70% of the time after the failed cache
  raid"). (Originally framed as "substrate for 081's demotion
  logic"; 081 retired 2026-05-02, so the metric stands on its own
  narrative-and-tuning use.)
- Possibly a `coordinator_authority_axis` on the coordinator side
  (a coordinator with high authority issues directives that *all*
  recipients weight up, independent of personal trust). TBD —
  may be redundant with the recipient-side trust axis.

Hypothesis to validate: a colony with one high-trust coordinator
and one low-trust coordinator should exhibit *measurably*
different outcomes when both issue conflicting directives. The
high-trust coordinator's directives win adoption; the low-trust
coordinator's get ignored. This is the testable shape of "good
coordinator vs bad coordinator has an effect."

## Dependencies

- Blocked by 126 (`IntentionSource` substrate).
- Blocked by 057 (something must actually emit
  `CoordinatorDirective` intentions for the trust axis to lift).
- ~~Blocked by 081~~ — 081 retired without implementation
  2026-05-02. The directive-failure demotion consumer it would have
  provided no longer exists; the compliance-by-coordinator metric
  this ticket adds is justified on narrative + tuning grounds alone.
- Pairs with C3 (subjective belief) — if mental models have
  landed, trust is a facet on the recipient's belief about the
  coordinator; if not, it's a flat Component.

## Preparation reading

- `docs/systems/strategist-coordinator.md` (in repo).
- Wooldridge ch. 6 "Communication" — speech-act framing of
  directives; the trust-weighting math has lineage there.
- `docs/balance/scoring-layer-second-order.md` framing #1 — the
  per-tick churn motivator that 126/130 jointly address from the
  social-influence side.

## Log

- 2026-05-02: opened as 126 follow-on per CLAUDE.md
  antipattern-migration rule. Carries the user-requested
  good-vs-bad-coordinator-effects design.
- 2026-05-02: 081 retired without implementation; dropped from
  `blocked-by` (now `[126, 057]`). The directive-failure-demotion
  consumer this ticket originally paired with no longer exists; the
  compliance-by-coordinator metric stands on narrative + tuning
  grounds. No status change — 126 and 057 still block.
