---
id: 288
title: EngageThreat morale_break must release the Guarding commitment so wounded cats can drop to Flee
status: ready
cluster: ai-substrate
added: 2026-05-11
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

<!--
Bugfix-shape ticket. Use this template (rather than `_template.md`) when the
work is a fix to observed defective behavior. The "Bugfix discipline" section
of CLAUDE.md REQUIRES at least one structural-revision candidate per fix-shape
decision tree — the slots below force that to be drafted, named, and considered.
-->

## Why

Cedar — boldness=0.34, anxiety=0.75, health=0.095 — died of
ShadowFoxAmbush at tick 1282037 in the post-271 verification soak
(`logs/tuned-42`, seed 42, commit `d013af9a8056`). One tick before
death (`1282036`) the trace shows `PlanStepFailed: disposition=Guarding,
step=EngageThreat, reason=morale_break` followed by `PlanReplanned:
new_steps=[TravelTo(PatrolZone), Survey]` — **still Guarding**. A cat
at 9.5% HP signalled "I cannot fight" and the planner replanned to
*more patrolling* inside the same disposition instead of releasing
commitment so L3 could elect Flee. Next tick the ShadowFox at (30, 31)
hit her for 0.18 damage; she died at 0.095 HP → 0.

This is not a Flee-scoring defect (271 already lifted the
boldness-invert curve; Cedar's curve is now 0.83 vs the old 0.66).
It's a **commitment-release defect**: `morale_break` is the substrate
saying "this cat has lost the will to engage" but the consequence is
landed wrong — the GOAP replan stays inside Guarding instead of
dropping back to L3 for re-election. Violates the survival hard gate
on `ShadowFoxAmbush` (post-271 footer: 1 death; pre-fix this ticket
takes it to 0 on seed 42).

## Hot context

- Run: `logs/tuned-42/` (post-271, commit `d013af9a8056`, seed 42)
- Focal cat trace: `logs/tuned-42/trace-Mocha.jsonl` (Mocha is the
  271 focal; Cedar's events live in `events.jsonl`)
- Smoking gun events (consecutive ticks):
  - `tick 1282036 PlanStepFailed disposition=Guarding step=EngageThreat reason=morale_break`
  - `tick 1282036 PlanReplanned disposition=Guarding new_steps=[TravelTo(PatrolZone), Survey]`
  - `tick 1282037 Ambush cat=Cedar predator=ShadowFox dmg=0.18 location=(30,31)`
  - `tick 1282037 Death cat=Cedar cause=Injury injury_source=ShadowFoxAmbush location=(31,30)`
- Cedar at tick 1282000: pos (39, 22), HP 0.09, safety 0.06, action
  Patrol. Flee score `0.007` (Flee in top-14 of L3 pool but
  non-competitive).
- Cedar's prior ambushes (5 total): (24, 4), (28, 31), (28, 32),
  (29, 23), (30, 31) — clustered around (28-30, 23-32) which has no
  ward coverage (closest ward 9 tiles NE at (33, 22)).
- Cumulative ambush damage ~0.91 with no healing between hits —
  Cedar stayed on Patrol instead of Sleep across the run (matches
  `project_l3_patrol_absorption_cascade.md`).
- Colony-wide: 8 of 10 cats clustered at (39, 21-23) on Patrol at
  tick 1282000. The patrol absorption cascade has the entire labor
  pool committed to threat-exposure patterns.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 perception | `src/systems/interoception.rs` | `safety_deficit`, `health_deficit`, `escape_viability` all author correctly for Cedar at the death tick. | `[verified-correct]` |
| L2 DSE base score | `src/ai/dses/flee.rs` | Post-271 R1a curve: Cedar's boldness=0.34 produces boldness_invert=0.83 (was 0.66 pre-271). Other axes near-saturated. CP raw should be near-competitive. | `[verified-correct]` |
| L2 modifier (lift) | `src/ai/modifier.rs::ThreatProximityAdrenalineFlee` | Gated on `escape_viability ≥ 0.4`. At (30, 31) with ShadowFox adjacent, viability ≈ 0 → modifier excluded. **Same gate exclusion as 286 names.** | `[verified-defect-when-cornered]` (per 271 audit promotion) |
| L3 softmax | `src/ai/scoring.rs` | Flee was in top-14 at score 0.007 (tick 1282000); Sleep was top at 0.76. Softmax sampling didn't favor Flee. | `[verified-correct]` — non-competitive Flee score is upstream of softmax |
| GOAP commitment | `src/ai/dses/...` Guarding DSE's `default_strategy()` | Guarding likely uses `CommitmentStrategy::Blind` (matching Fleeing's pattern). Blind prevents preemption by L3 re-election. | `[suspect]` (needs read) |
| Plan template — Guarding | `src/ai/planner/...` Guarding plan template (likely `[TravelTo(PatrolZone), Survey, EngageThreat, ...]` family) | The replan handler on `EngageThreat → morale_break` re-generates inside Guarding instead of releasing the disposition. **This is the load-bearing defect.** | `[verified-defect]` (smoking-gun trace at tick 1282036) |
| Resolver — EngageThreat | `src/steps/...engage_threat.rs` (path tbd) | Returns `StepResult::Fail` with reason `morale_break` when actor HP is below combat-effective threshold. | `[suspect]` (needs read for exact predicate) |
| Replan handler | `src/ai/planner/...` or `src/components/commitment.rs` | The handler that receives `PlanStepFailed` and decides whether to replan vs release commitment. Currently keeps Cedar in Guarding. | `[verified-defect]` (smoking-gun trace) |

## Fix candidates

**Parameter-level options:**
- **R1 — Lower the `morale_break` predicate threshold so EngageThreat
  refuses to ENTER the step when HP is too low.** Would shortcut the
  step earlier, but doesn't fix the commitment-release pattern — the
  cat still stays Guarding through subsequent replans.
- **R2 — Add a health-based eligibility filter on Guarding DSE.**
  Cats at HP < 0.3 can't elect Guarding. Doesn't help Cedar because
  she's already committed; eligibility filters apply at L2 election,
  not mid-plan.

**Structural options:**
- **R3 (rebind, RECOMMENDED) — `morale_break` releases the commitment
  rather than triggering an in-disposition replan.** When
  `PlanStepFailed { reason: morale_break }` fires, the handler clears
  the disposition (sets `current_disposition = None` or equivalent)
  and the next tick L3 re-elects. The cat's Sleep + Flee scores then
  compete on equal footing. Substrate-side: `morale_break` is already
  a first-class signal — currently it's wired to "stay in disposition,
  pick a softer step"; rebind to "drop disposition, re-elect."
- **R4 (split) — Carve `Guarding` → `Guarding` + `GuardingRetreat`.**
  New disposition variant for the morale-broken retreat state, with
  a plan template `[Flee, HoldUntilSafe]` that mirrors Fleeing.
  Useful if the Guarding disposition wants to preserve some context
  (location memory, ally coordination) that L3 re-election would
  drop. Heavier surface than R3.
- **R5 (extend) — Branch the Guarding plan template on entry
  condition: if `health_deficit > 0.7 at entry`, use a Fleeing-shaped
  template instead.** Doesn't fix mid-plan release (the cat could
  enter Guarding at high HP, take ambush damage, then be stuck);
  R3 covers the mid-plan case.

## Recommended direction

**R3 (rebind).** `morale_break` is the substrate's own signal
declaring "this cat has lost the will to engage." Currently the
consequence is bound to in-disposition replan; rebind to commitment
release. Wins because:

1. The signal already exists — no new marker, no new disposition,
   no new plan template. The fix is changing what one signal does.
2. Substrate-over-override per the design pillar — the existing
   substrate channel (`morale_break`) gets the correct semantic
   wiring instead of layering a new override.
3. Post-release, L3 re-election composes naturally: a wounded cat
   sees Sleep dominate (already does — Sleep 0.76 in Cedar's last_scores)
   or Flee if a threat is now visible. The substrate carries the
   replanning intent.
4. R4 (split) was considered but rejected: GuardingRetreat duplicates
   what Fleeing already provides; the structural cost (new variant
   in `disposition.rs::from_action`, new plan template, new
   completion proxy) doesn't earn its keep when R3 reuses existing
   L3 election.

## Out of scope

- **Lowering `threat_proximity_adrenaline_viability_threshold`.**
  That's ticket 286 — independently load-bearing for cornered
  cats. Either fix helps Cedar (R3 by releasing commitment; 286 by
  letting the Flee lift reach her). Both should ship.
- **Ward placement targeting the (28-30, 23-32) ambush hotspot.**
  Ticket 284 just activated the ambush/carcass anchor weights but
  the wards aren't moving into the hotspot. Separate balance
  follow-on (open as 289+ if 271's land hasn't already).
- **Patrol absorption cascade** (`project_l3_patrol_absorption_cascade.md`).
  R3 fixes the morale_break release but doesn't address why 5 cats
  elect Patrol simultaneously when wounded ones should be Sleeping.
  That's a Patrol DSE scoring question, separate ticket.
- **271's R3 structural (boldness-as-modifier)** — ticket 287.
  Orthogonal axis.

## Verification

- `cargo test` — no regressions on existing Guarding / commitment
  tests.
- New unit test on the replan handler: given a `PlanStepFailed
  { reason: morale_break }` from a Guarding disposition, the cat's
  `current_disposition` becomes `None` (or equivalent release).
- `just scenario` — write a new scenario `guarding_morale_break_releases`
  preloading a wounded cat (HP 0.1) inside a Guarding plan with an
  EngageThreat step pending; assert disposition clears at the
  morale_break tick, not in the replanned `[TravelTo, Survey]` shape.
- `just soak-trace 42 Cedar` — Cedar must survive past tick 1282037.
  At minimum: `deaths_by_cause.ShadowFoxAmbush == 0`.
- `just verdict logs/tuned-42-<commit>` — survival canary passes
  (hard gate). Continuity canaries unchanged. No new failure on
  Guarding-eligible scenarios.

## Log

- 2026-05-11: opened from post-271 Zapruder analysis of Cedar's
  death. Audit table promotes the replan-handler row to
  `[verified-defect]` via the smoking-gun `PlanStepFailed →
  PlanReplanned` trace at tick 1282036. R3 (rebind morale_break to
  commitment release) recommended; R4 (split GuardingRetreat) and
  R5 (extend plan template) considered and named in the audit
  trail.
