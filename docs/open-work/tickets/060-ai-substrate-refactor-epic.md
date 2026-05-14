---
id: 060
title: AI substrate refactor — program epic
status: in-progress
cluster: ai-substrate
initiative: [smarter-cats]
added: 2026-04-27
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, refactor-plan.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

The AI substrate refactor (`docs/systems/ai-substrate-refactor.md`,
`docs/systems/refactor-plan.md`) is a multi-month program covering
seven phases and ~14 outstanding shippable units. Two umbrellas
(005 cluster-A, 013 spec-follow-on debts) were retired 2026-04-27
because their `status: in-progress` flags couldn't reflect partial
closure of sub-tracks — exactly the staleness antipattern this epic
must avoid.

This epic is **read-only over its child tickets** — it doesn't
own work, it owns *visibility*. Each shippable unit lives in its
own ticket per the post-2026-04-27 convention; this file is the
program-level dashboard that answers "what's left in the refactor?"
in one read. It updates when child tickets change status, not on
its own cadence.

## Scope

This epic tracks every shippable unit of the substrate refactor.
The unit of work is the child ticket; the unit of visibility is
this file.

### Phase coverage map

| Phase | Spec section | State | Owner ticket(s) |
|---|---|---|---|
| Phase 1 | §11 instrumentation | ✅ landed | (cluster-A umbrella, retired 005) |
| Phase 2 | §5 InfluenceMap substrate | ✅ landed (substrate + Cluster B closeout); 🔄 §5.6.3 follow-ons | 006 ✅ landed (10989775), 061 ✅ landed; in flight: [062](062-prey-species-split-maps.md), [063](063-ward-strength-promotion.md), [064](064-carcass-scent-consumer-cutover.md) |
| Phase 3a–3d | §2–§3 / §4 / §9 L2 substrate | ✅ landed | (retired 005) |
| Phase 4 | §6 target-taking DSEs | ✅ landed | (retired 014) |
| Phase 4 follow-ons | §4 / §6.5 residue | ✅ landed | 049 ✅ landed (384bf25), 050 ✅ landed (7dfa2796), 051 ✅ landed (238524ea), 052 ✅ landed (acccdc7), 065 ✅ landed |
| Phase 5 | scattered sites + silent-advance audit | ✅ landed | (retired 005) |
| Phase 6a | §7 commitment gate | ✅ landed | (retired 005) |
| Phase 6b | §7.7 aspiration reconsideration | 🔄 in flight | [056](056-aspiration-compatibility-matrix.md), [055](055-mood-drift-threshold-detection.md), [058](058-tradition-unfiltered-loop-fix.md); 053 ready ([053](053-death-event-grief-emission.md)); 057 gated on 128 ([057](057-coordinator-directive-intention-strategy-row.md)); 054 retired 2026-05-14 ([landed/054](../landed/054-fate-event-vocabulary-expansion.md)) |
| Phase 6c | §8 softmax-over-Intentions | ✅ landed | (Phase 4a, retired 014) |
| Phase 6d | §7.W Fulfillment + axis-capture | ✅ landed | (retired 024 + 012) |
| Phase 7 | cleanup pass | 💤 parked | [059](059-phase-7-substrate-cleanup.md) |
| **C3 belief substrate** | §C3 (post-2026-05 expansion) | ✅ landed (substrate); 🔄 retirement chain in flight | **258 ✅ landed** (mental-models / facets / evidence typology); **295 ✅ landed** (WitnessableEvent emit sites); retirement: [290](290-rdf-reader-cutover-sensorrs-reads-contextbeliefspredictability-instead-of-recentdispositionfailures-258-retirement-r3.md) / [291](291-colonyknowledge-restructure-promotion-via-mental-model-agreement-replaces-carrier-count-threshold-258-follow-on.md) / [292](292-recenttargetfailures-retirement-per-pair-failure-memory-moves-to-contextbeliefs-catbeliefs-predictability-258-follow-on.md) / [293](293-huntingpriors-retirement-per-location-belief-moves-to-locationbeliefsperceived-violence-capability-colony-absorption-rebuild-258-follow-on.md) / [294](294-recentambushmap-retirement-colony-resource-moves-to-per-cat-locationbeliefsrecency-of-threat-cue-258-follow-on.md) / [304](304-witnessableeventattack-emit-gated-on-cat-vs-cat-aggression-substrate.md) |
| **ActionAffordances substrate** | §261 (post-2026-05 expansion) | ✅ landed (substrate + consumer wiring); 🔄 follow-ons | **261 ✅ landed** (per-action success scalars + estimators); **263 ✅ landed** (Flee/Patrol/Hunt DSE consumers); follow-ons: [264](264-social-dse-consumers-wire-belief-affordance-axes-socialize-groomother-mate-mentor-care-feedkitten.md) / [265](265-wildlife-symmetric-dse-consumers-wire-belief-affordance-fox-hawk-snake-shadowfox.md) / [314](314-extend-actionaffordances-writer-to-cover-cat-vs-prey-stalkchasepounce-263-follow-on.md) / [315](315-activate-263-axes-with-four-artifact-methodology-fleepatrolhunt-resolver-bias.md) / [316](316-hunt-resolver-writes-stepphase-enum-for-focal-trace-visibility-263-follow-on.md) / [317](317-retire-flee-threat-distance-power-invert-if-frame-diff-shows-redundancy-with-affordanceflee.md) |

### Adjacent / cluster work

These are not refactor phases per se but cluster directly into the
substrate's vocabulary and were tracked alongside it:

| Cluster | Spec | State | Owner |
|---|---|---|---|
| §7.M Mating | three-layer model | ✅ landed | 027 ✅ landed (c182fad) |
| Cluster C narrative | deliberation layer doctrine | ✅ landed | **007 ✅ landed** (0b072423) |
| Cluster C substrate | BDI + JointIntention + HTN methods | ✅ landed (substrate); 🔄 sub-epic in flight | **126 ✅ landed** (6b0b8940, BDI intention substrate); **127 ✅ landed** (b5455647, joint-intention substrate); **128 🔄 in-progress** ([128](128-htn-method-composition.md) — HTN method composition sub-epic, 25 children #319-#343 opened 2026-05-14, design at [`docs/systems/htn-methods.md`](../../systems/htn-methods.md)); active sibling children: [129](129-care-dses-perceivable-intentions.md) / [130](130-trust-weighted-coordinator-momentum.md); 127 follow-ons: [274](274-co-mentoring-practice-on-jointintention-substrate.md) / [275](275-joint-cache-stocking-practice-on-jointintention-substrate.md) / [276](276-play-bout-practice-on-jointintention-substrate-play-continuity-canary-host.md) / [277](277-n2-joint-practices-group-hunting-kitten-circles-participants-hashsetentity-shape.md) / [278](278-asymmetric-courtship-roles-initiator-responder-on-jointintention.md) / [279](279-body-cue-driven-joint-adoption-compose-127-with-242-243.md) / [280](280-mental-model-of-partner-jointintention-compose-127-with-258-c3-mental-models.md) |
| Cluster D | formalization (corruption CA, mood Markov, weather Markov) | 🔄 ready | [008](008-cluster-d-formalization-verification.md) |
| Cluster E | world-gen pre-history fast-forward | 🔄 ready | [009](009-cluster-e-worldgen-richness.md) |

### Open child tickets — full roster

| Ticket | Status | Spec home | One-line scope |
|---|---|---|---|
| [008](008-cluster-d-formalization-verification.md) | ready | Cluster D | Formalization vocabulary (CA / Markov / Markov) |
| [009](009-cluster-e-worldgen-richness.md) | ready | Cluster E | Pre-sim history fast-forward |
| [053](053-death-event-grief-emission.md) | ready | §7.7.b | Death-event grief emission (007 landed, now unblocked) |
| [055](055-mood-drift-threshold-detection.md) | blocked-by 344 | §7.7.d | Mood drift detection |
| [056](056-aspiration-compatibility-matrix.md) | ✅ landed (3cc14e20d23b) | §7.7.1 | Aspiration compatibility matrix |
| [057](057-coordinator-directive-intention-strategy-row.md) | blocked-by 128 | §7.3 | Coordinator-directive Intention strategy row |
| [058](058-tradition-unfiltered-loop-fix.md) | parked | §3.5.3 | Tradition modifier unfiltered-loop fix |
| [059](059-phase-7-substrate-cleanup.md) | parked | Phase 7 | `ScoringContext` removal + §10 unblock + spec drift |
| [062](062-prey-species-split-maps.md) | ✅ landed (bdb35a8533fd) | §5.6.3 #5 | Per-prey-species `PreyScentMap` split |
| [063](063-ward-strength-promotion.md) | ready | §5.6.3 #3 | Ward-strength as first-class spatial axis |
| [064](064-carcass-scent-consumer-cutover.md) | ready | §5.6.3 #6 | Carcass-scent consumer cutover (balance-affecting) |
| [128](128-htn-method-composition.md) | 🔄 in flight | Cluster C | HTN method composition — epic (sub-epic, 25 children #319-#343 opened 2026-05-14) |
| [129](129-care-dses-perceivable-intentions.md) | ready | Cluster C | Care DSEs over perceivable intentions (126 landed) |
| [130](130-trust-weighted-coordinator-momentum.md) | blocked-by 057 | Cluster C | Trust-weighted coordinator directive momentum |
| [264](264-social-dse-consumers-wire-belief-affordance-axes-socialize-groomother-mate-mentor-care-feedkitten.md) | ready | §261 | Social DSE consumers wire belief/affordance axes |
| [265](265-wildlife-symmetric-dse-consumers-wire-belief-affordance-fox-hawk-snake-shadowfox.md) | ready | §261 | Wildlife DSE consumers (fox/hawk/snake/shadowfox) |
| [274](274-co-mentoring-practice-on-jointintention-substrate.md) | ready | Cluster C / 127 | Co-mentoring practice on JointIntention substrate |
| [275](275-joint-cache-stocking-practice-on-jointintention-substrate.md) | ready | Cluster C / 127 | Joint cache-stocking practice |
| [276](276-play-bout-practice-on-jointintention-substrate-play-continuity-canary-host.md) | ready | Cluster C / 127 | Play-bout practice (play continuity canary host) |
| [277](277-n2-joint-practices-group-hunting-kitten-circles-participants-hashsetentity-shape.md) | ready | Cluster C / 127 | N≥2 joint practices (group hunting / kitten circles) |
| [278](278-asymmetric-courtship-roles-initiator-responder-on-jointintention.md) | ready | Cluster C / 127 | Asymmetric courtship roles on JointIntention |
| [279](279-body-cue-driven-joint-adoption-compose-127-with-242-243.md) | ready | Cluster C / 127 | Body-cue-driven joint adoption (compose 127 with 242/243) |
| [280](280-mental-model-of-partner-jointintention-compose-127-with-258-c3-mental-models.md) | ready | Cluster C / 127 | Mental model of partner (compose 127 with 258) |
| [290](290-rdf-reader-cutover-sensorrs-reads-contextbeliefspredictability-instead-of-recentdispositionfailures-258-retirement-r3.md) | ready | §C3 / 258 | RDF reader cutover (`ContextBeliefs.predictability`) |
| [291](291-colonyknowledge-restructure-promotion-via-mental-model-agreement-replaces-carrier-count-threshold-258-follow-on.md) | ready | §C3 / 258 | ColonyKnowledge promotion via mental-model agreement |
| [292](292-recenttargetfailures-retirement-per-pair-failure-memory-moves-to-contextbeliefs-catbeliefs-predictability-258-follow-on.md) | ready | §C3 / 258 | RecentTargetFailures → ContextBeliefs.predictability |
| [293](293-huntingpriors-retirement-per-location-belief-moves-to-locationbeliefsperceived-violence-capability-colony-absorption-rebuild-258-follow-on.md) | ready | §C3 / 258 | HuntingPriors → LocationBeliefs.perceived_violence_capability |
| [294](294-recentambushmap-retirement-colony-resource-moves-to-per-cat-locationbeliefsrecency-of-threat-cue-258-follow-on.md) | ready | §C3 / 258 | RecentAmbushMap → per-cat LocationBeliefs.recency_of_threat_cue |
| [304](304-witnessableeventattack-emit-gated-on-cat-vs-cat-aggression-substrate.md) | ready | §C3 / 295 | WitnessableEvent::Attack emit gated on cat-vs-cat aggression |
| [314](314-extend-actionaffordances-writer-to-cover-cat-vs-prey-stalkchasepounce-263-follow-on.md) | ready | §261 / 263 | Affordances writer covers cat-vs-prey Stalk/Chase/Pounce |
| [315](315-activate-263-axes-with-four-artifact-methodology-fleepatrolhunt-resolver-bias.md) | blocked-by 314 | §261 / 263 | Activate 263 axes with four-artifact methodology |
| [316](316-hunt-resolver-writes-stepphase-enum-for-focal-trace-visibility-263-follow-on.md) | ready | §261 / 263 | Hunt resolver writes StepPhase enum for focal trace |
| [317](317-retire-flee-threat-distance-power-invert-if-frame-diff-shows-redundancy-with-affordanceflee.md) | blocked-by 315 | §261 / 263 | Retire `flee_threat_distance` if frame-diff shows redundancy |

**Total open: 33** (29 ready, 0 in-progress, 2 blocked, 2 parked).

Out-of-scope sub-rosters (substrate-adjacent but not refactor work):
body/audible cue substrate (170, 242–245, 262, 268), intraspecies-conflict
rungs (142–145, 267, 269), §270 EngageThreat split, §286–289 flee/threat
calibration, §283 fox-scent split, §282 temporal-integration doctrine —
all currently active but tracked separately rather than ballooning this
roster. See `just open-work-ready --cluster ai-substrate` for the full
ai-substrate cluster list.

### Critical path

**The structural spine is complete and Cluster C substrate is now
in production.** Original spine (`052 → 065 → 006 → 059`) all
landed except 059 (parked cleanup). Subsequent substrate landings
extended Cluster C and added two new substrate layers:

1. **052 / 065 / 006** ✅ landed (original spine — `SpatialConsideration`,
   §L2.10.7 fox roster, §5.6.3 producer-map catalog).
2. **126** ✅ landed (6b0b8940). **BDI intention substrate** — the
   Cluster C entry-point flagged 2026-05-08 as "the largest remaining
   spend." Now in production; 128/129 unblocked, 057 re-gated on 128.
3. **127** ✅ landed (b5455647). **JointIntention substrate** — codified
   body-language layer for two-cat practices. Opens follow-on roster
   274–280 (co-mentoring, joint cache-stock, play-bout, N≥2, asymmetric
   courtship, body-cue adoption, partner mental model).
4. **258** ✅ landed. **§C3 belief substrate** — `MentalModel`,
   `ContextBeliefs`, `LocationBeliefs`, evidence typology. Drives
   retirement chain 290–294 (RDF reader, ColonyKnowledge, RecentTargetFailures,
   HuntingPriors, RecentAmbushMap) + 304 (Attack emit).
5. **295** ✅ landed. WitnessableEvent emit sites (Attack / Mate / Care /
   FleeFrom / Hunt) — wires belief integrator on action-resolver
   completions per the 258 substrate.
6. **261 / 263** ✅ landed. **ActionAffordances substrate** —
   per-action success scalars + ActionKind enum + 21 heuristic
   estimators (261); Flee/Patrol/Hunt DSE consumers wired (263).
   Follow-ons 264/265/314–317 extend to social DSEs, wildlife,
   prey, and four-artifact activation.
7. **027** ✅ landed (c182fad). Mating cadence three-bug cascade
   closeout — adjacent §7.M work.
8. **050 / 051** ✅ landed 2026-05-14. §4 marker predicate truth
   + fox DSE eligibility migration (see Phase 4 follow-ons row).
9. **059** 💤 parked. `ScoringContext` / `FoxScoringContext`
   removal; spec-vs-code drift reconciliation. Unblocked but not
   yet picked up.

**What's the largest remaining spend?** Three parallel tracks of
roughly equal weight now that Cluster C substrate has landed:
- **§C3 retirement chain** — 290/291/292/293/294/304 (six ready
  tickets retiring the old per-cat / colony resources into
  `ContextBeliefs` / `LocationBeliefs` / `ColonyKnowledge` /
  WitnessableEvent). These are mechanical cutovers but touch many
  consumer sites.
- **Cluster C JointIntention practices** — 274–280 (seven ready
  tickets composing 127's substrate into specific two-cat
  practices). High narrative payoff per landing.
- **§261 affordance activation** — 314/315/316/317 (with 315 the
  load-bearing four-artifact activation pass).

Independent ready work: §5.6.3 follow-ons (062/063/064), §7.7
aspiration (053/056), Cluster D (008), Cluster E (009),
HTN/Care/Trust (128/129/130).

## Out of scope

- **Per-ticket implementation work.** Each child ticket owns its
  own scope, verification, and log. This file does not duplicate
  child-ticket bodies.
- **Balance threads.** Drift > ±10% on a characteristic metric
  follows the four-artifact methodology in `docs/balance/*.md`,
  not this epic.
- **Out-of-scope spec deferrals.** Body-zone epic, ToT epic,
  Calling subsystem, Trade subsystem — each is referenced from
  a child ticket but is not refactor work.
- **Pre-existing issues** (`docs/open-work/pre-existing/*.md`) —
  test-harness drift, dead activation features. Tracked separately.

## Current state

As of 2026-05-14 — every original spine-piece has landed
(052/065/006/126/127/258/261/263/295/027/049/050/051/052/065).
The substrate refactor has *successfully transitioned from "build
the substrate" to "drain the consumer-cutover and activation
queues."* What remains is wide rather than deep:

- **§C3 belief retirement chain** — 290/291/292/293/294 (RDF /
  ColonyKnowledge / RecentTargetFailures / HuntingPriors /
  RecentAmbushMap) + 304 (Attack emit). Mechanical but
  high-coverage; each retires a per-cat or colony resource into the
  258 substrate.
- **§261 affordance activation** — 264/265 (social + wildlife
  consumer wiring), 314/315/316/317 (writer extension + four-artifact
  activation + StepPhase enum + redundant-axis retirement).
- **Cluster C JointIntention practices** — 274/275/276/277/278/279/280
  (co-mentoring, joint cache-stock, play-bout, N≥2, asymmetric
  courtship, body-cue adoption, partner mental model) — all
  unblocked by 127.
- **§5.6.3 follow-ons** — 062/063/064 (prey scent split, ward
  strength promotion, carcass scent cutover).
- **§7.7 aspiration** — 053/056 ready (007 unblocked 053); 054
  retired 2026-05-14 (umbrella whose three children have divergent
  natural homes — see [landed/054](../landed/054-fate-event-vocabulary-expansion.md));
  055 still gated on 056; 057 now gated on 128 (was 007).
- **Cluster D + Cluster E** — 008, 009 (both ready, large epics).
- **HTN/Care/Trust** — 128/129 unblocked by 126; 130 still gated
  on 057.
- **Phase 7 cleanup** — 059 parked; unblocked but deferred.

The substrate-refactor program's narrative has shifted: the
"largest remaining spend" framing (126 BDI substrate) cited
2026-05-08 has been answered. Today the three parallel tracks
listed in the Critical path section carry roughly equal weight;
none is structurally privileged.

For the per-section coverage map, see the audit plan at
`/Users/will.mitchell/.claude/plans/trying-to-figure-out-luminous-charm.md`
(may be ephemeral; the source-of-truth tables above replicate its
findings).

## Approach

**Maintenance rule:** this epic is updated *only* when a child
ticket changes status. Updates happen in the same commit that
flips the child's status, not on a separate cadence. The Phase
coverage map and Open child tickets table are the load-bearing
sections; everything else can drift as long as the tables stay
honest.

**Anti-staleness measure:** if you find this file claiming a child
ticket is `ready` when the child file says otherwise, the child
file is the truth. Update the epic to match. Do not flip child
ticket status to match the epic. Run `just epic-children 060`
to surface this drift mechanically; `--fix` rewrites the roster
table to match. `just check` runs the audit on every commit
(ticket 318).

**When to retire this epic:** when every child ticket on the roster
is `landed` or `dropped`. At that point, move this file to
`docs/open-work/landed/YYYY-MM.md` as a `## Ticket 060 — AI
substrate refactor program closeout` entry summarizing the
program's outcome. Don't retire it just because Phase 6a or 6b
or any single phase landed — the whole program is the unit of
retirement.

## Verification

- Every child ticket on the roster exists and has the claimed
  status (verify via `just open-work-ready` / `just open-work-wip`
  / `just open-work` greps).
- `docs/open-work.md` Summary block: total open ≈ 16 epic children
  + non-refactor work (~28 other open).
- `just check` clean (no code changes in this epic file).
- Anyone asking "what's left in the substrate refactor?" can
  answer from this file alone in under 60 seconds.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-08 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed ** 71** (done, planning-substrate, score 0.92 (cross-cluster)) — Planning-substrate hardening — gird against the stuck-cat bug class (sub-epic)
- ✓ landed ** 72** (done, —, score 0.90) — "`plan_substrate` module extraction (refactor)"
- ✓ landed ** 73** (done, —, score 0.89) — Wave 2 substrate hardening

<!-- linkages:end -->
## Log

- 2026-04-27: opened from substrate-refactor audit. Cataloged 16
  open child tickets (12 ready, 1 in-progress, 2 blocked, 1
  parked) across 7 spec phases + 4 cluster threads. Marked
  `status: in-progress` because the program is, in fact, in
  progress — but body explicitly delegates work-tracking to
  children.
- 2026-05-08: dashboard audit. Promoted four roster entries to
  ✅ landed (006 cluster-B closeout 10989775; 049 §9.2 faction
  overlay markers 384bf25; 061 herb-location producer scaffold;
  065 §L2.10.7 self-state roster sweep). Flipped 058 ready →
  parked to match its child file. Added six new roster entries
  visible from the spine's landing — three §5.6.3 follow-ons
  (062/063/064) and five Cluster-C children (126 ready entry-point
  + 127/128/129/130 gated on 126; 130 also gated on 057).
  Roster grew 16 → 21 (10 ready, 1 in-progress, 8 blocked, 2
  parked). Critical-path section rewritten — the structural
  spine is complete; remaining shape is horizontal. Cluster-C
  cluster-table row points at 126 explicitly as the
  implementation entry-point so future readers don't have to
  re-derive it from 007's narrative body.
- 2026-05-14: §4 follow-ons closeout. Landed 050 (7dfa2796) and
  051 (238524ea) in tandem — every fox DSE eligibility now flows
  through the §4 marker substrate (`.require()` / `.forbid()`),
  every redundant `FoxScoringContext` boolean retired (seven
  fields), three predicate stubs promoted to truthful form
  (WardNearbyFox per-tick ward scan, HasDen pure event-driven,
  HasCubs hybrid event-driven + reconciliation, HasThreatNearby
  through `observer_sees_at`). Phase 4 follow-ons row flipped
  🔄 in flight → ✅ landed; 050/051 dropped from roster. Roster
  21 → 19 (9 ready, 1 in-progress, 7 blocked, 2 parked).
  Post-050 vs post-051 archive comparison: +141% wards placed,
  −80% ShadowFox ambush deaths, +661% colony health, 0→156
  shadow-fox ward-avoidance events — substrate alignment paid
  off in colony outcomes, not just structural cleanup.
- 2026-05-14: full-roster reconciliation. Manual frontmatter audit
  surfaced four landed-but-not-promoted roster entries (007 cluster-C
  narrative @0b072423; 027 mating cascade @c182fad; 126 BDI substrate
  @6b0b8940; 127 JointIntention substrate @b5455647) and four
  major substrate landings the dashboard never mentioned (258 §C3
  belief substrate; 261 ActionAffordances substrate; 263 DSE
  consumer wiring; 295 WitnessableEvent emit sites). Added two new
  Phase-coverage rows for §C3 and §ActionAffordances substrates,
  rewrote Cluster C row to reflect 126/127 landing + 274–280
  follow-ons unblocked. Added 16 new roster entries (264/265
  consumer wiring, 274–280 JointIntention practices, 290–294 §C3
  retirement chain, 304 Attack emit, 314–317 §261 follow-ons).
  Updated blockers: 053 (007 landed → ready); 055 (still 056);
  057 (now blocked-by 128, was 007); 128/129 (ready, 126 landed);
  130 (still blocked-by 057). Roster 19 → 33 (29 ready, 0
  in-progress, 2 blocked, 2 parked). The 2026-05-08 framing of
  "126 is the largest remaining spend" is now obsolete; rewrote
  Critical path + Current state to reflect the substrate
  refactor's shift from spinal-build to consumer-cutover-and-
  activation across three parallel tracks (§C3 retirement,
  §261 activation, Cluster C joint practices).
- 2026-05-14: friction logged — no `just ticket-status <ids>` or
  `just epic-children 060` query exists; the dashboard's
  Anti-staleness Measure rule (child file is the truth) goes
  unenforced because a child can land without notifying the epic.
  Hand-rolled bash audit over `docs/open-work/{tickets,landed}/`
  frontmatter is required. Tracked in `logs/agent-friction.jsonl`
  (severity: major). The dashboard staleness window between
  2026-05-08 and 2026-05-14 had 4 child-status promotions and
  4 major-substrate landings invisible to anyone reading 060
  alone — exactly the antipattern this file is meant to prevent.
- 2026-05-14: `epic-children --fix` touched 2 roster row(s) (315 status-mismatch; 317 status-mismatch). Auto-generated by scripts/epic_children.py.
- 2026-05-14: `epic-children --fix` touched 1 roster row(s) (062 landed-but-marked-active). Auto-generated by scripts/epic_children.py.
- 2026-05-14: `epic-children --fix` touched 2 roster row(s) (055 blocker-mismatch; 056 landed-but-marked-active). Auto-generated by scripts/epic_children.py.
