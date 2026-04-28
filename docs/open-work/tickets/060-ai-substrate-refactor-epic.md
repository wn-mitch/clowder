---
id: 060
title: AI substrate refactor — program epic
status: in-progress
cluster: null
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
| Phase 2 | §5 InfluenceMap substrate | ✅ landed (substrate); 🔄 catalog rollout | [006](006-cluster-b-shared-spatial-slow-state.md) |
| Phase 3a–3d | §2–§3 / §4 / §9 L2 substrate | ✅ landed | (retired 005) |
| Phase 4 | §6 target-taking DSEs | ✅ landed | (retired 014) |
| Phase 4 follow-ons | §4 / §6.5 residue | 🔄 in flight | [049](049-faction-overlay-markers.md), [050](050-marker-predicate-refinements.md), [051](051-fox-dse-eligibility-migration.md), 052 ✅ landed (acccdc7), [065](065-l2-10-7-self-state-fox-roster-sweep.md) |
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
| Cluster C | deliberation layer (BDI / Versu / belief / coordinator) | 🔄 ready | [007](007-cluster-c-deliberation-layer.md) |
| Cluster D | formalization (corruption CA, mood Markov, weather Markov) | 🔄 ready | [008](008-cluster-d-formalization-verification.md) |
| Cluster E | world-gen pre-history fast-forward | 🔄 ready | [009](009-cluster-e-worldgen-richness.md) |

### Open child tickets — full roster

| Ticket | Status | Spec home | One-line scope |
|---|---|---|---|
| [006](006-cluster-b-shared-spatial-slow-state.md) | ready | Cluster B / §5.6.3 | Influence-map catalog completion (8 absent rows) |
| [007](007-cluster-c-deliberation-layer.md) | ready | Cluster C | Deliberation layer (BDI / Versu / belief / coordinator) |
| [008](008-cluster-d-formalization-verification.md) | ready | Cluster D | Formalization vocabulary (CA / Markov / Markov) |
| [009](009-cluster-e-worldgen-richness.md) | ready | Cluster E | Pre-sim history fast-forward |
| [027](027-mating-cadence-three-bug-cascade.md) | in-progress | §7.M | Mating cascade — Bugs 1+2 landed, Bug 3 partial |
| [049](049-faction-overlay-markers.md) | ready | §9.2 | Faction overlay markers (4 ZSTs) |
| [050](050-marker-predicate-refinements.md) | ready | §4 | Marker predicate refinements (3 promotions) |
| [051](051-fox-dse-eligibility-migration.md) | ready | §4 / fox | Fox DSE `.require()` / `.forbid()` cutover |
| [065](065-l2-10-7-self-state-fox-roster-sweep.md) | ready | §L2.10.7 | Self-state DSE + fox disposition roster (succeeds 052) |
| [053](053-death-event-grief-emission.md) | blocked-by 007 | §7.7.b | Death-event grief emission |
| [054](054-fate-event-vocabulary-expansion.md) | ready | §7.7.c | Fate event vocabulary expansion |
| [055](055-mood-drift-threshold-detection.md) | blocked-by 056 | §7.7.d | Mood drift detection |
| [056](056-aspiration-compatibility-matrix.md) | ready | §7.7.1 | Aspiration compatibility matrix |
| [057](057-coordinator-directive-intention-strategy-row.md) | blocked-by 007 | §7.3 | Coordinator-directive Intention strategy row |
| [058](058-tradition-unfiltered-loop-fix.md) | ready | §3.5.3 | Tradition modifier unfiltered-loop fix |
| [059](059-phase-7-substrate-cleanup.md) | parked | Phase 7 | `ScoringContext` removal + §10 unblock + spec drift |

**Total open: 16** (12 ready, 1 in-progress, 2 blocked, 1 parked).

### Critical path

The structural critical path is **052 ✅ → 065 → 006 → 059**:

1. **052** ✅ landed 2026-04-28 (acccdc7). The `SpatialConsideration`
   substrate is in production with `LandmarkSource::{TargetPosition,
   Tile, Entity}`; all 9 cat target-taking DSEs cut over to it; the
   four §6.5 deferred axes (`pursuit-cost`, `fertility-window` spatial,
   `apprentice-receptivity` spatial-pairing, `remedy-match` caretaker-
   distance) are unblocked. Cumulative drift across the entire
   refactor's 6 commits: ~zero on every characteristic metric.
2. **065** picks up the rest of §L2.10.7's roster — 12 cat self-state
   DSEs + 9 fox dispositions. First production callers of
   `LandmarkSource::Entity`; substrate decision needed for aggregate-
   centroid landmarks (Explore frontier, PracticeMagic corruption
   cluster, fox Hunting prey-belief centroid).
3. **006** completes the §5.6.3 absent-map catalog. Producer maps
   feed 065's centroid resolution paths and the influence-map
   sampling sites that ScoringContext currently owns.
4. **059** sweeps the residue: deletes `ScoringContext` /
   `FoxScoringContext` after 065's last consumer is written,
   reconciles spec-vs-code drift.

Other tickets parallelize off this spine. 027 is mating-specific
and runs independently. Cluster B/C/D/E (006–009) are large epics
themselves, gated only on the cluster-A landings.

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

As of 2026-04-27 — substrate is structurally complete through
Phase 6a. Phase 6b/6c/6d landed via separate ticket threads.
Phase 7 cleanup is parked pending 065 (which now owns 052's
remaining substrate consumers). The user's audit on 2026-04-27
confirmed every spec section maps to either landed work or an
open child ticket; founder-age regression (refactor pre-flight
gate 2) was confirmed quietly resolved with no ticket needed.

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

## Log

- 2026-04-27: opened from substrate-refactor audit. Cataloged 16
  open child tickets (12 ready, 1 in-progress, 2 blocked, 1
  parked) across 7 spec phases + 4 cluster threads. Marked
  `status: in-progress` because the program is, in fact, in
  progress — but body explicitly delegates work-tracking to
  children.
