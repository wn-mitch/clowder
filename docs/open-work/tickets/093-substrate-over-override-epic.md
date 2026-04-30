---
id: 093
title: Substrate-over-override — retire control-yanking hacks in favor of IAUS levers
status: in-progress
cluster: substrate-over-override
added: 2026-04-30
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Across the AI substrate refactor, a recurring pattern keeps surfacing: behavior is currently driven by a control-flow shortcut (interrupt, override, hard-coded gate, planner shortcut, silent-advance), and the right fix is a **substrate-side replacement** (DSE axis, consideration curve, marker, modifier, eligibility filter, jerk curve) that lets the existing score → intention → plan → execute loop arrive at the same answer naturally.

087 is the canonical success: the `CriticalHealth` interrupt yanked control whenever health crossed a threshold; replaced by `pain_level` and `body_distress_composite` feeding Sleep/Flee scoring as continuous IAUS axes, cats now prioritize self-care via the substrate without an interrupt.

Tickets 047, 058, 027, 027b, 081, 076, 088, 091, 092, 089, 090 all sit on this thread. Naming the thread converts the cascade pattern from "whack-a-mole" into "systematically retiring debt." This epic is the program-level dashboard.

This epic is **read-only over its child tickets** — same pattern as 060 (substrate refactor program) and 071 (planning-substrate hardening sub-epic). It owns visibility, not work. Updates when child tickets change status, in the same commit.

## The pattern, named: substrate-over-override

When fixing scoring or planning behavior, prefer substrate-side levers over control-flow shortcuts.

**Smell-test for "this is a hack"** — any of:
- The path bypasses `score_dse_by_id` / softmax / planner.
- The path forces a specific `Action` regardless of DSE rankings.
- The path is a binary gate where a continuous signal would be more honest.
- The path is a per-disposition exemption list ("Resting/Hunting/Foraging immune to hunger interrupts").
- The path silently advances or no-ops a step instead of failing visibly.
- The path applies a coefficient or modifier uniformly across DSEs when it should be action-matched.

**Critical sequencing constraint**: a hack can only be retired once its substrate replacement is expressive enough to do its job. 087 retired part of `CriticalHealth` (Sleep + Flee got the new axes) but didn't extend the pattern to Eat — and the colony food economy collapsed when interrupt telemetry zeroed (091). **Substrate axes land first; the corresponding hack retires second.**

## Inventory by category

The categories below are the surfaces where hack-shaped patterns live. Each row links the existing ticket (where one exists) and notes the IAUS lever underneath.

### 1. Interrupts (forced replan / forced action)

| Location | Hack | Lever | Ticket |
|---|---|---|---|
| `src/systems/disposition.rs:299-351` | `CriticalHealth`/`Starvation`/`Exhaustion`/`CriticalSafety` interrupts force per-tick replan; same disposition often re-picked while damage accumulates | continuous health/safety/hunger/energy deficits as DSE axes + jerk curves on Sleep/Eat/Flee | **[047](047-critical-health-interrupt-treadmill.md)** (ready, prototypical) |
| `src/systems/disposition.rs:254-276` | `ThreatDetected` forces `Action::Flee`, overriding higher-scoring Guarding | threat-proximity axis on Flee + threat-presence marker | 047 (related) |
| `src/systems/disposition.rs:192-276` | Six 1.0-multiplier hardcoded thresholds (binary gates) | inflection points on jerk curves, not switches | 047, 076 |

### 2. Per-disposition exemption lists (special-case smell)

| Location | Hack | Lever | Ticket |
|---|---|---|---|
| `src/systems/disposition.rs:305-317` | `Resting`/`Hunting`/`Foraging` exempt from hunger/energy interrupts | Rao-Georgeff §7.2 commitment/momentum modifier (folds into 047) | 047 |
| `src/systems/disposition.rs:319-342` | Guards exempt from threat interrupts | Guarding DSE's eligibility re-evaluates threat severity natively | 047 |

### 3. Silent advance / silent fail step resolvers

| Location | Hack | Lever | Ticket |
|---|---|---|---|
| `src/steps/disposition/cook.rs:24-25` | `unwitnessed(Advance)` when no raw food; plans loop silently | return `Fail`; observability debt, not substrate axis | **[091](091-post-087-action-collapse.md)** (audit scope) |
| `src/steps/disposition/retrieve_raw_food_from_stores.rs:24-25, 50-71` | three silent-advance paths | return `Fail` | 091 (audit) |
| `src/steps/disposition/retrieve_from_stores.rs:21-65` | general retrieve silent-advance | return `Fail` | 091 (audit) |
| `src/steps/disposition/feed_kitten.rs:28-62`, `mentor_cat.rs:62`, `mate_with.rs:62-93`, `groom_other.rs:111` | social steps silent-advance on missing target | return `Fail` | [027](027-mating-cadence-three-bug-cascade.md) (Bug 1 decoupling) + general |

### 4. Hard-coded planner shortcuts (L2↔L3 feasibility-language drift)

| Location | Hack | Lever | Ticket |
|---|---|---|---|
| `src/ai/planner/actions.rs:97-111` `resting_actions()` | `EatAtStores` required only `ZoneIs(Stores)`, not `HasStoredFood`; plans against empty stores | plumb `HasStoredFood` into `StatePredicate` (H1 fix). Tactical fix for one gap. | **091** (H1 fix in working tree) |
| `src/ai/planner/actions.rs:526, 656-777` | `actions_for_disposition(Resting, None, …)` expands to a fixed list without reachability check | gate Resting DSE on reachability via `EligibilityFilter`; or split into `RestedWithFood`/`RestedWithoutFood` | 091 |
| `src/ai/planner/mod.rs` `PlannerState` + `MarkerSnapshot` | **two parallel feasibility languages** — IAUS reads `MarkerSnapshot` via `EligibilityFilter`; GOAP reads `PlannerState` via `StatePredicate`. Each new gating fact requires manual sync; silent drift bug-producing. | **structural collapse** — `PlannerState` consumes `MarkerSnapshot` directly; `StatePredicate::HasMarker(MarkerKind)` becomes the GOAP-side primitive. One source of truth. | **[092](092-marker-state-predicate-unification.md)** (ready, blocked-by 091) |

### 5. Personality-gate overrides

| Location | Hack | Lever | Ticket |
|---|---|---|---|
| `src/ai/scoring.rs:1515-1553` `behavior_gate_check()` | five binary action overrides (Timid → not-Fight, Reckless → force-Fight, Shy → skip-Socialize, Compulsive Explorer → force-Explore, Compulsive Helper → force-Herbcraft) | each personality trait as a DSE-CP modifier; soft modulation, not post-scoring action swap | (no ticket; general hardening) |

### 6. Modifier over-breadth

| Location | Hack | Lever | Ticket |
|---|---|---|---|
| `src/ai/modifier.rs:526-583` Tradition | applies to every DSE regardless of action history | per-action keying or flat tile-familiarity ((a) or (b)) | **[058](058-tradition-unfiltered-loop-fix.md)** (ready) |

### 7. Coordinator-side override (parked) and last-resort modifier (parked)

| Location | Hack | Lever | Ticket |
|---|---|---|---|
| `src/systems/coordination.rs:788-862` `dispatch_urgent_directives()` | re-issues same directive every tick after cross-cat failures | `DirectiveFailureLedger` as colony-level failure memory axis; demotion modifier in §3.5.1 pipeline | **[081](081-coordination-directive-failure-demotion.md)** (parked) — re-evaluate as substrate-axis-shaped; unpark candidate |
| (not yet in code) | when recovery actions fail N times, no fallback | possibly the wrong shape — what's wanted may be fallback DSE always eligible at low score, not last-resort modifier | **[076](076-last-resort-promotion-modifier.md)** (parked) — re-frame; possibly close-and-replace |

### 8. Mating-cadence multi-bug cascade

| Location | Hack | Lever | Ticket |
|---|---|---|---|
| `src/ai/scoring.rs:916` (retired) + `socialize_target.rs:193` | lifted-condition outer gate (Bug 2, retired); bias-pin for missing L2 layer (Bug 3) | marker-based eligibility + L2 PairingActivity component | **027** (in-progress, Bugs 1+2 landed); **[027b](027b-l2-pairing-activity.md)** (blocked-by 071) |

## Substrate prerequisites for hack retirement

The sequencing rule applied across the inventory:

| Hack to retire | Substrate prerequisite | Status |
|---|---|---|
| 047's `CriticalHealth` interrupt | [088](088-body-distress-modifier.md) (Body-distress Modifier) — must land first with sufficient magnitude | 088 blocked-by 014 |
| 047's `Starvation`/`Exhaustion`/`CriticalSafety` interrupts | hunger_distress / exhaustion_distress / threat_proximity axes (extend 087's pattern; new sub-tickets) | not opened — open as 047 lands |
| 091's `EatAtStores` precondition gap | `HasStoredFood` plumbed into `StatePredicate` | H1 in working tree (091) |
| 091's silent-advance steps | `Fail` not `Advance` | H4 in working tree (091) |
| 091's producer-side residual | `CanForage`/`PreyNearby` markers + reachable-zone substrate | open under 091 (in-progress) |
| L2↔L3 feasibility-language drift (general) | `StatePredicate::HasMarker(MarkerKind)` + `PlannerState` reads `MarkerSnapshot` directly | **092 (ready, blocked-by 091) — the structural cure for the whole class** |
| 027 Bug 3's bias-pin | L2 PairingActivity component (027b) + 078 `target_pairing_intention` Consideration | 027b blocked-by 071 |
| 081's coordinator stuck-loop | `RecentTargetFailures` aggregate sensor | blocked-by 072 + 073 |

## Open child tickets — full roster

| Ticket | Status | Pattern role |
|---|---|---|
| [027](027-mating-cadence-three-bug-cascade.md) | in-progress | multi-bug mating cascade (Bugs 1+2 landed; Bug 3 → 027b) |
| [027b](027b-l2-pairing-activity.md) | blocked-by 071 | L2 substrate retiring 027 Bug 3's bias-pin |
| [047](047-critical-health-interrupt-treadmill.md) | ready | **prototypical case** — interrupt → continuous IAUS axes |
| [058](058-tradition-unfiltered-loop-fix.md) | ready | over-broad modifier → per-action keyed history axis |
| [076](076-last-resort-promotion-modifier.md) | parked | **re-evaluate with the lens** — possibly wrong shape |
| [081](081-coordination-directive-failure-demotion.md) | parked | colony-level failure memory as substrate axis |
| [088](088-body-distress-modifier.md) | blocked-by 014 | **substrate prerequisite for 047** |
| [089](089-interoceptive-self-anchors.md) | ready | substrate expansion (spatial self-perception) |
| [090](090-self-perception-l4-l5.md) | ready | substrate expansion (L4/L5 perception coverage) |
| [091](091-post-087-action-collapse.md) | in-progress | **cautionary case** — partial substrate adoption causes collapse |
| [092](092-marker-state-predicate-unification.md) | ready (blocked-by 091) | **structural cure** for L2↔L3 feasibility-language drift |

**Total open: 11** (1 in-progress, 5 ready, 3 blocked, 2 parked).

**Canonical exemplar (landed)**: 087 — interoceptive perception substrate (CriticalHealth interrupt → `pain_level` + `body_distress_composite` axes), landed 2026-04-30 at `fc4e1ab`. See `docs/open-work/landed/2026-04.md`.

## Out of scope

- **Per-ticket implementation work.** Each child ticket owns its own scope, verification, and log.
- **Balance threads.** Drift > ±10% on a characteristic metric follows the four-artifact methodology in `docs/balance/*.md`, not this epic.
- **Pre-existing issues** (`docs/open-work/pre-existing/*.md`) — tracked separately.
- **The substrate refactor itself.** This epic threads through the refactor (060) but doesn't replace it; it's a *cross-cutting design discipline*, not a competing program.

## Current state

Opened 2026-04-30. Inventory cataloged 11 child tickets (1 in-progress, 5 ready, 3 blocked, 2 parked) plus the canonical exemplar 087. Recommended ordering:

1. Close 091 (in-progress, user has a path).
2. Land 092 (structural cure for L2↔L3 sync drift — collapses the parallel feasibility languages). Unblocks the rest of 091's class-A gaps without per-fact tactical fixes.
3. Promote 088 (currently blocked-by 014; it's the substrate prerequisite for 047).
4. Tackle 047 (the prototypical case) with the lens explicit; per-disposition exemption lists fold in.
5. 058 (small, ready, high-confidence) as a warm-up between bigger moves.
6. 027/027b/078 thread runs in parallel under 071.
7. Re-evaluate 076 and 081 with the lens before unparking.

## Approach

**Maintenance rule:** this epic is updated *only* when a child ticket changes status. Updates happen in the same commit that flips the child's status. The Inventory by category and Substrate prerequisites tables are load-bearing; everything else can drift as long as the tables stay honest.

**Child-ticket convention:** each child carries a `## Substrate-over-override pattern` section near the top, populated with `Hack shape:` / `IAUS lever:` / `Sequencing:` / `Canonical exemplar:` lines. The convention is grep-discoverable: `rg '## Substrate-over-override pattern' docs/open-work/tickets/`.

**Discipline doc TODO**: write `docs/systems/substrate-over-override.md` once 2-3 children land cleanly with the lens applied (047 + 058 + one of 027b/091 closeout would be the natural inflection). Capture the smell-test, sequencing rule, 087 exemplar, and inventory-template for future tickets. Deferred sub-task; not blocking.

## Verification

- Every child ticket on the roster carries the `## Substrate-over-override pattern` callout.
- `rg '## Substrate-over-override pattern' docs/open-work/tickets/ | wc -l` matches child count (currently 11).
- `docs/open-work.md` Summary block reflects the new ticket.
- Anyone asking "what hacks remain?" can answer from the Inventory by category table alone in under 60 seconds.

**When to retire this epic:** when every child ticket on the roster is landed or dropped, and the discipline doc at `docs/systems/substrate-over-override.md` exists and codifies the smell-test + sequencing rule. At that point, move this file to `docs/open-work/landed/YYYY-MM.md` as a `## Ticket 093 — Substrate-over-override program closeout` entry.

## Log

- 2026-04-30: Opened from substrate-over-override pattern review session. Inventory enumerated 10 in-flight children plus canonical exemplar 087. Plan stored at `~/.claude/plans/looking-at-091-i-stateful-wand.md`. The pattern was implicitly being chased ticket-by-ticket; this epic is the explicit naming. The sequencing rule (substrate axes land before the corresponding hack retires) was extracted from the 087→091 cascade as a load-bearing discipline.
- 2026-04-30: Renumbered 092 → 093 to resolve collision with concurrent ticket 092 (marker / state-predicate unification). Added 092 itself as the 11th child — it's the structural cure for the L2↔L3 feasibility-language drift class, the most general substrate-over-override case in the inventory.
