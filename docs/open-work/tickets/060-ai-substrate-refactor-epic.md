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
| Phase 4 follow-ons | §4 / §6.5 residue | 🔄 in flight | 049 ✅ landed (384bf25), 052 ✅ landed (acccdc7), 065 ✅ landed; in flight: [050](050-marker-predicate-refinements.md), [051](051-fox-dse-eligibility-migration.md) |
| Phase 5 | scattered sites + silent-advance audit | ✅ landed | (retired 005) |
| Phase 6a | §7 commitment gate | ✅ landed | (retired 005) |
| Phase 6b | §7.7 aspiration reconsideration | 🔄 in flight | [053](053-death-event-grief-emission.md), [054](054-fate-event-vocabulary-expansion.md), [055](055-mood-drift-threshold-detection.md), [056](056-aspiration-compatibility-matrix.md), [057](057-coordinator-directive-intention-strategy-row.md), [058](058-tradition-unfiltered-loop-fix.md) |
| Phase 6c | §8 softmax-over-Intentions | ✅ landed | (Phase 4a, retired 014) |
| Phase 6d | §7.W Fulfillment + axis-capture | ✅ landed | (retired 024 + 012) |
| Phase 7 | cleanup pass | 💤 parked | [059](059-phase-7-substrate-cleanup.md) |

### Adjacent / cluster work

These are not refactor phases per se but cluster directly into the
substrate's vocabulary and were tracked alongside it:

| Cluster | Spec | State | Owner |
|---|---|---|---|
| §7.M Mating | three-layer model | 🔄 in flight | [027](027-mating-cadence-three-bug-cascade.md) |
| Cluster C | deliberation layer (BDI / Versu / belief / coordinator) | 🔄 ready | [007](007-cluster-c-deliberation-layer.md) (narrative); **entry-point: [126](126-bdi-intention-substrate.md)** (BDI substrate); follow-ons [127](127-joint-intention-substrate.md) / [128](128-htn-method-composition.md) / [129](129-care-dses-perceivable-intentions.md) / [130](130-trust-weighted-coordinator-momentum.md) |
| Cluster D | formalization (corruption CA, mood Markov, weather Markov) | 🔄 ready | [008](008-cluster-d-formalization-verification.md) |
| Cluster E | world-gen pre-history fast-forward | 🔄 ready | [009](009-cluster-e-worldgen-richness.md) |

### Open child tickets — full roster

| Ticket | Status | Spec home | One-line scope |
|---|---|---|---|
| [007](007-cluster-c-deliberation-layer.md) | ready | Cluster C | Deliberation layer narrative (BDI / Versu / belief / coordinator) |
| [008](008-cluster-d-formalization-verification.md) | ready | Cluster D | Formalization vocabulary (CA / Markov / Markov) |
| [009](009-cluster-e-worldgen-richness.md) | ready | Cluster E | Pre-sim history fast-forward |
| [027](027-mating-cadence-three-bug-cascade.md) | in-progress | §7.M | Mating cascade — Bugs 1+2 landed, Bug 3 partial |
| [050](050-marker-predicate-refinements.md) | ready | §4 | Marker predicate refinements (3 promotions) |
| [051](051-fox-dse-eligibility-migration.md) | ready | §4 / fox | Fox DSE `.require()` / `.forbid()` cutover |
| [053](053-death-event-grief-emission.md) | blocked-by 007 | §7.7.b | Death-event grief emission |
| [054](054-fate-event-vocabulary-expansion.md) | ready | §7.7.c | Fate event vocabulary expansion |
| [055](055-mood-drift-threshold-detection.md) | blocked-by 056 | §7.7.d | Mood drift detection |
| [056](056-aspiration-compatibility-matrix.md) | ready | §7.7.1 | Aspiration compatibility matrix |
| [057](057-coordinator-directive-intention-strategy-row.md) | blocked-by 007 | §7.3 | Coordinator-directive Intention strategy row |
| [058](058-tradition-unfiltered-loop-fix.md) | parked | §3.5.3 | Tradition modifier unfiltered-loop fix |
| [059](059-phase-7-substrate-cleanup.md) | parked | Phase 7 | `ScoringContext` removal + §10 unblock + spec drift |
| [062](062-prey-species-split-maps.md) | ready | §5.6.3 #5 | Per-prey-species `PreyScentMap` split |
| [063](063-ward-strength-promotion.md) | ready | §5.6.3 #3 | Ward-strength as first-class spatial axis |
| [064](064-carcass-scent-consumer-cutover.md) | ready | §5.6.3 #6 | Carcass-scent consumer cutover (balance-affecting) |
| [126](126-bdi-intention-substrate.md) | ready | Cluster C | **BDI intention substrate — Cluster C entry-point** |
| [127](127-joint-intention-substrate.md) | blocked-by 126 | Cluster C | Joint-intention substrate for two-cat practices |
| [128](128-htn-method-composition.md) | blocked-by 126 | Cluster C | HTN method composition over `HeldIntention.goal` |
| [129](129-care-dses-perceivable-intentions.md) | blocked-by 126 | Cluster C | Care DSEs over perceivable intentions |
| [130](130-trust-weighted-coordinator-momentum.md) | blocked-by 126, 057 | Cluster C | Trust-weighted coordinator directive momentum |

**Total open: 21** (10 ready, 1 in-progress, 8 blocked, 2 parked).

### Critical path

**The structural spine is complete.** What remained at 060's
opening — `052 → 065 → 006 → 059` — has all landed except 059
(parked cleanup). The substrate is in production:

1. **052** ✅ landed 2026-04-28 (acccdc7). `SpatialConsideration`
   substrate with `LandmarkSource::{TargetPosition, Tile, Entity}`;
   all 9 cat target-taking DSEs cut over.
2. **065** ✅ landed. §L2.10.7 self-state DSE + fox disposition
   roster sweep — first production callers of `LandmarkSource::Entity`,
   aggregate-centroid resolution paths in place.
3. **006** ✅ landed 2026-04-27 (10989775). Cluster-B closeout —
   §5.6.3 producer-map catalog. Successors 062/063/064 (still open)
   carry the per-row consumer cutovers; 061 ✅ landed.
4. **059** 💤 parked. `ScoringContext` / `FoxScoringContext`
   removal; spec-vs-code drift reconciliation. Unblocked but not
   yet picked up.

**What's the largest remaining spend?** Cluster C — 126 (BDI
substrate) is the unblocked head; 127/128/129/130 unblock as 126
lands. The §7.7 aspiration cluster (053–058) is partial, gated
mostly on 007's narrative scope. The §5.6.3 follow-ons (062/063/064)
and §4 follow-ons (050/051) are independent ready work.

Other tickets parallelize off this. 027 is mating-specific and
runs independently. Cluster C/D/E (007–009) are large epics
themselves, with C now having a concrete implementation entry-point
in 126.

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

As of 2026-05-08 — the substrate spine (052/065/006) has
structurally landed; only 059's cleanup pass remains, and it's
parked with no blocker. Phase 6a/6b/6c/6d landed via separate
ticket threads. The remaining shape of the program is
*horizontal*, not *spinal*:

- **§5.6.3 follow-ons** — 062/063/064 (061 ✅ landed)
- **§4 follow-ons** — 050/051
- **§7.7 aspiration** — partial: 054/056 ready; 053/055/057 blocked; 058 parked
- **Cluster C (deliberation)** — entry-point 126 ready; 127/128/129/130 gated on it
- **Adjacent epics** — 007 (C narrative), 008 (D), 009 (E) all ready; 027 (§7.M mating) in progress

The unblocked set with the largest downstream payoff is **126**:
it's the substrate everything else in Cluster C composes against,
and it directly addresses the per-tick supply-chain failures
(`scoring-layer-second-order.md` framing #1) that today's
modifier-and-tenure stack only patches.

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
ticket status to match the epic.

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
