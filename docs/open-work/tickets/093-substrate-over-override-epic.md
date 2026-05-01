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
| `src/steps/disposition/cook.rs:24-25` | `unwitnessed(Advance)` when no raw food; plans loop silently | return `Fail`; observability debt, not substrate axis | landed via 091 (2026-04-30) |
| `src/steps/disposition/retrieve_raw_food_from_stores.rs:24-25, 50-71` | three silent-advance paths | return `Fail` | landed via 091 (2026-04-30) |
| `src/steps/disposition/retrieve_from_stores.rs:21-65` | general retrieve silent-advance | return `Fail` | landed via 091 (2026-04-30) |
| `src/steps/disposition/feed_kitten.rs:28-62`, `mentor_cat.rs:62`, `mate_with.rs:62-93`, `groom_other.rs:111` | social steps silent-advance on missing target | return `Fail` | [027](027-mating-cadence-three-bug-cascade.md) (Bug 1 decoupling) + general |

### 4. Hard-coded planner shortcuts (L2↔L3 feasibility-language drift)

| Location | Hack | Lever | Ticket |
|---|---|---|---|
| `src/ai/planner/actions.rs:97-111` `resting_actions()` | `EatAtStores` required only `ZoneIs(Stores)`, not `HasStoredFood`; plans against empty stores | plumb `HasStoredFood` into `StatePredicate` (H1 fix). Tactical fix for one gap. | landed via 091 + 092 (2026-04-30) |
| `src/ai/planner/actions.rs:526, 656-777` | `actions_for_disposition(Resting, None, …)` expands to a fixed list without reachability check | gate Resting DSE on reachability via `EligibilityFilter`; or split into `RestedWithFood`/`RestedWithoutFood` | partially addressed via 091 (Resting goal drops `HungerOk` when stores empty, `goals.rs`); reachability gate not yet substrate-level |
| `src/ai/planner/mod.rs` `PlannerState` + `MarkerSnapshot` | **two parallel feasibility languages** — IAUS reads `MarkerSnapshot` via `EligibilityFilter`; GOAP reads `PlannerState` via `StatePredicate`. Each new gating fact requires manual sync; silent drift bug-producing. | **structural collapse** — `PlannerState` consumes `MarkerSnapshot` directly; `StatePredicate::HasMarker(MarkerKind)` becomes the GOAP-side primitive. One source of truth. | landed via 092 (2026-04-30 at `25439daf`); follow-ons [096](096-materials-available-substrate-split.md) / [097](097-non-cat-planner-substrate-audit.md) / [098](098-search-state-vs-substrate-doctrine.md) |
| `src/ai/planner/mod.rs` `PlannerState.materials_available` | hybrid field — entry-side mirrors world fact; search-side mutated by `StateEffect::SetMaterialsAvailable(true)`. Resists pure marker migration. | split — substrate-side `MaterialsAvailable` marker authored from per-site `materials_complete()`; new `PlannerState.materials_delivered_this_plan: bool` for the search side; `Construct` precondition becomes the disjunction. After this lands zero mirror fields remain on `PlannerState`. | **[096](096-materials-available-substrate-split.md)** (ready; 092 unblocked) |
| `src/ai/fox_planner/`, `src/ai/hawk_planner/`, `src/ai/snake_planner/` | each species planner implements `core::GoapDomain` for its own state struct; may carry the same parallel-feasibility-language smell 092 retired for cats | thread `MarkerSnapshot` through species `GoapDomain`; replace any mirror predicates with `HasMarker(...)` (or document audit-result if no mirrors exist). | **[097](097-non-cat-planner-substrate-audit.md)** (ready) |
| `src/systems/goap.rs:5539` `PlannerZone::Wilds` | resolver authored a parallel feasibility language for "where the wilds are" (`find_nearest_tile(...).or(Some(*pos))`), while IAUS Explore scored against `LandmarkAnchor::UnexploredFrontierCentroid`. The `.or(Some(*pos))` fallback stamped a degenerate self-target Travel that silently advanced. | consume `ExplorationMap::frontier_centroid()` directly; `find_nearest_tile` as no-frontier fallback; `None` (not `Some(*pos)`) when neither resolves. Anchor-shape analogue of 092's marker-shape cure. | **landed via [121](../landed/121-early-game-plan-churn-pre-kitchen.md)** (2026-05-01) |

### 5. Personality-gate overrides

| Location | Hack | Lever | Ticket |
|---|---|---|---|
| `src/ai/scoring.rs:1515-1553` `behavior_gate_check()` | five binary action overrides (Timid → not-Fight, Reckless → force-Fight, Shy → skip-Socialize, Compulsive Explorer → force-Explore, Compulsive Helper → force-Herbcraft) | each personality trait as a DSE-CP modifier; soft modulation, not post-scoring action swap | (no ticket; general hardening) |

### 6. Modifier over-breadth

| Location | Hack | Lever | Ticket |
|---|---|---|---|
| `src/ai/modifier.rs:526-583` Tradition | applies to every DSE regardless of action history | per-action keying or flat tile-familiarity ((a) or (b)) | **[058](058-tradition-unfiltered-loop-fix.md)** (parked 2026-04-30 — dormant in production with bonus = 0.0; design choice deferred to balance ticket) |

### 7. Coordinator-side override (parked) and last-resort modifier (parked)

| Location | Hack | Lever | Ticket |
|---|---|---|---|
| `src/systems/coordination.rs:788-862` `dispatch_urgent_directives()` | re-issues same directive every tick after cross-cat failures | `DirectiveFailureLedger` as colony-level failure memory axis; demotion modifier in §3.5.1 pipeline | **[081](081-coordination-directive-failure-demotion.md)** (parked) — re-evaluate as substrate-axis-shaped; unpark candidate |
| (not yet in code) | when recovery actions fail N times, no fallback | possibly the wrong shape — what's wanted may be fallback DSE always eligible at low score, not last-resort modifier | **[076](076-last-resort-promotion-modifier.md)** (parked) — re-frame; possibly close-and-replace |

### 8. Mating-cadence multi-bug cascade

| Location | Hack | Lever | Ticket |
|---|---|---|---|
| `src/ai/scoring.rs:916` (retired) + `socialize_target.rs:193` (retired by 078) | lifted-condition outer gate (Bug 2, retired); bias-pin for missing L2 layer (Bug 3, retired by 027b/082) | marker-based eligibility + L2 PairingActivity component | **027** (closed 2026-05-01 on structural verification); **[027b](027b-l2-pairing-activity.md)** (blocked-by 071, reactivation landed via 082) |

## Substrate prerequisites for hack retirement

The sequencing rule applied across the inventory:

| Hack to retire | Substrate prerequisite | Status |
|---|---|---|
| 047's `CriticalHealth` interrupt | [088](../landed/088-body-distress-modifier.md) (Body-distress Modifier) — must land first with sufficient magnitude | **088 done 2026-05-01** — `BodyDistressPromotion` registered in `default_modifier_pipeline` between Tradition and FoxTerritorySuppression; magnitude tuning deferred to 047 (consumer-driven) |
| 047's `Starvation`/`Exhaustion`/`CriticalSafety` interrupts | hunger_distress / exhaustion_distress / threat_proximity axes (extend 087's pattern; new sub-tickets) | not opened — open as 047 lands |
| 091's `EatAtStores` precondition gap | `HasStoredFood` plumbed into `StatePredicate` | landed (091, 2026-04-30) — generalized via 092's `HasMarker` |
| 091's silent-advance steps | `Fail` not `Advance` | landed (091, 2026-04-30) |
| 091's producer-side residual | `CanForage`/`PreyNearby` markers + reachable-zone substrate | landed (091, 2026-04-30) — `enforce_survival_floor` removed; `CarryingIs(Carrying::Nothing)` veto removed from `SearchPrey`/`ForageItem`; partial Resting goal when stores empty |
| L2↔L3 feasibility-language drift (general) | `StatePredicate::HasMarker(MarkerKind)` + `PlannerState` reads `MarkerSnapshot` directly | **landed (092, 2026-04-30 at `25439daf`) — the structural cure** |
| 092's hybrid `materials_available` follow-on | substrate-side `MaterialsAvailable` marker + per-plan `materials_delivered_this_plan` field | open under 096 (ready) |
| 092's non-cat-planner follow-on | thread `MarkerSnapshot` through fox/hawk/snake `GoapDomain` | open under 097 (ready) |
| 092's substrate-vs-search-state doctrine | §SubstrateVsSearchState in `docs/systems/ai-substrate-refactor.md` | open under 098 (ready) |
| 027 Bug 3's bias-pin | L2 PairingActivity component (027b) + 078 `target_pairing_intention` Consideration | 027b blocked-by 071 |
| 081's coordinator stuck-loop | `RecentTargetFailures` aggregate sensor | blocked-by 072 + 073 |

## Open child tickets — full roster

| Ticket | Status | Pattern role |
|---|---|---|
| ~~[027](../landed/027-mating-cadence-three-bug-cascade.md)~~ | done 2026-05-01 | multi-bug mating cascade (Bugs 1+2 landed; Bug 3 → 027b; closed on structural verification) |
| [027b](027b-l2-pairing-activity.md) | blocked-by 071 | L2 substrate retiring 027 Bug 3's bias-pin |
| [047](047-critical-health-interrupt-treadmill.md) | ready | **prototypical case** — interrupt → continuous IAUS axes |
| [058](058-tradition-unfiltered-loop-fix.md) | parked 2026-04-30 | over-broad modifier → per-action keyed history axis (deferred until balance ticket) |
| [076](076-last-resort-promotion-modifier.md) | parked | **re-evaluate with the lens** — possibly wrong shape |
| [081](081-coordination-directive-failure-demotion.md) | parked | colony-level failure memory as substrate axis |
| ~~[088](../landed/088-body-distress-modifier.md)~~ | done 2026-05-01 | **substrate prerequisite for 047** (landed; magnitude tuning deferred to 047) |
| [089](089-interoceptive-self-anchors.md) | ready | substrate expansion (spatial self-perception) |
| [090](090-self-perception-l4-l5.md) | ready | substrate expansion (L4/L5 perception coverage) |
| [096](096-materials-available-substrate-split.md) | ready (092 unblocked) | hybrid `PlannerState.materials_available` split — substrate-side marker + per-plan search field |
| [097](097-non-cat-planner-substrate-audit.md) | ready | apply 092's structural cure to fox/hawk/snake planners |
| [098](098-search-state-vs-substrate-doctrine.md) | ready | substrate-vs-search-state boundary doctrine in `docs/systems/ai-substrate-refactor.md` |
| ~~[121](../landed/121-early-game-plan-churn-pre-kitchen.md)~~ | done 2026-05-01 | anchor-shape analogue of 092 — `PlannerZone::Wilds` consumes `ExplorationMap::frontier_centroid`; sibling carveouts 122/123 unblocked |
| [122](122-socialize-dse-iaus-vs-gate-still-goal-mismatch.md) | ready | Socialize IAUS election vs OpenMinded gate `still_goal` proxy mismatch — sibling carveout from 121 |
| [123](123-recent-disposition-failures-cooldown.md) | ready | RecentDispositionFailures cooldown — per-cat failure-history substrate axis the planner lacks; sibling carveout from 121 |

**Total open: 13** (0 in-progress, 9 ready, 1 blocked, 3 parked) — after 027 closeout (2026-05-01), 088 land (2026-05-01), 099 (Modifier feature emission) opened blocked-by 047, and 121 landed (2026-05-01) with sibling carveouts 122 and 123 unblocked.

**Canonical exemplars (landed)**:
- **087** — interoceptive perception substrate (CriticalHealth interrupt → `pain_level` + `body_distress_composite` axes), landed 2026-04-30 at `fc4e1ab`.
- **091** — post-087 plan-execution collapse, landed 2026-04-30 (bundled into 092's commit per jj rebase). The **cautionary case** demonstrating the sequencing rule: partial substrate adoption causes collapse. Three hacks fell out under the lens (silent-advance step resolvers, planner H1 mirror, `enforce_survival_floor` post-hoc clamp).
- **092** — marker / state-predicate unification, landed 2026-04-30 at `25439daf`. The **structural cure** for L2↔L3 feasibility-language drift: `StatePredicate::HasMarker(&'static str)` + `PlanContext { markers, entity }` threaded through the cat planner. Opened follow-ons 096 (hybrid split), 097 (non-cat planner audit), 098 (boundary doctrine) per the new CLAUDE.md §Long-horizon coordination antipattern-migration rule.
- **094** — Eat-vs-Forage IAUS imbalance, landed 2026-04-30. The **natural-lever exemplar** for "publish a colony-state scalar → consume as a Modifier on the relevant DSE class": new `StockpileSatiation` Modifier in §3.5.1 mirroring `FoxTerritorySuppression`'s shape, multiplicative damp on Hunt/Forage when `food_fraction > 0.5`. One scoring-layer change cascaded — Resting/Socializing 4×, three never-fired social positives (`BondFormed`, `CourtshipInteraction`, `PairingIntentionEmitted`) started firing, courtship canary 0 → 210, ShadowFoxAmbush deaths 4 → 0, total deaths 8 → 1. **The case for the doctrine**: get the score landscape right and three orthogonal behaviors recover for free.

See `docs/open-work/landed/2026-04.md` for full landed entries.

## Out of scope

- **Per-ticket implementation work.** Each child ticket owns its own scope, verification, and log.
- **Balance threads.** Drift > ±10% on a characteristic metric follows the four-artifact methodology in `docs/balance/*.md`, not this epic.
- **Pre-existing issues** (`docs/open-work/pre-existing/*.md`) — tracked separately.
- **The substrate refactor itself.** This epic threads through the refactor (060) but doesn't replace it; it's a *cross-cutting design discipline*, not a competing program.

## Current state

Opened 2026-04-30. Inventory cataloged 11 child tickets initially. As of 2026-04-30 (post-091/092 land + reconciliation): **13 open** (1 in-progress, 7 ready, 2 blocked, 3 parked) plus the canonical exemplars 087 / 091 / 092. Recommended ordering:

1. ~~Close 091~~ (landed 2026-04-30, bundled into 092).
2. ~~Land 092~~ (landed 2026-04-30 at `25439daf` — structural cure for L2↔L3 sync drift).
3. ~~Promote 088~~ (unblocked 2026-05-01; 014 was landed at `453ea83` three days before 088 was opened — frontmatter `blocked-by: [014]` was stale at creation). 088 is the substrate prerequisite for 047.
4. ~~Land 088~~ (landed 2026-05-01) — `BodyDistressPromotion` Modifier reading 087's `body_distress_composite` into an additive lift on the six-DSE self-care class (Flee/Sleep/Eat/Hunt/Forage/GroomSelf). Magnitude default 0.20 at threshold 0.7; consumer-driven tuning deferred to 047. **Next:** tackle 047 (the prototypical case) with the lens explicit; verify the lift fires under focal trace and adjust magnitude before retiring `CriticalHealth` interrupt branches; per-disposition exemption lists fold in.
5. ~~058 (warm-up)~~ — parked 2026-04-30; revisit when a balance ticket opens for Tradition's bonus magnitude.
6. 027/027b/078 thread runs in parallel under 071.
7. Re-evaluate 076 and 081 with the lens before unparking.
8. Land 096 (materials_available split, 092-unblocked) and 097 (non-cat planner audit) to complete 092's structural-cure surface across all GOAP domains; 098 codifies the substrate-vs-search-state boundary doctrine in `docs/systems/ai-substrate-refactor.md`.
9. ~~Land 094 (Eat-vs-Forage IAUS imbalance)~~ — landed 2026-04-30. `StockpileSatiation` Modifier on Hunt/Forage; cascade unlocked Resting/Socializing/courtship.

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
- 2026-04-30: **Reconciliation pass.** 091 landed (bundled into 092's commit at `25439daf` per jj history; the standalone `fa0f3a84` SHA in 091's frontmatter was a hidden pre-rebase snapshot). 092 landed at `25439daf`, opening follow-ons 096 (materials_available hybrid split), 097 (non-cat planner audit), 098 (substrate-vs-search-state doctrine) per CLAUDE.md §Long-horizon coordination antipattern-migration-follow-ups rule. 058 parked — Tradition's unfiltered-loop smell is dormant in production (`tradition_location_bonus = 0.0`); design choice (a) per-action-keyed vs (b) flat tile-familiarity deferred to a balance ticket when someone wants the bonus turned on. 091's investigation surfaced a third hack falling out under the 093 lens (`enforce_survival_floor` post-hoc score clamp), which was removed as part of 091's land. Archived 012 / 024 / 091 to `landed/2026-04.md`. Roster delta: +094 (Eat-vs-Forage natural-lever follow-up surfaced by 091, `cluster: substrate-over-override`), +096, +097, +098, -091 (done), -092 (landed). Total open 11 → 13. New cautionary-and-cure exemplar pair (091 + 092) joins 087 as the canonical landed set.
- 2026-05-01: **027 closed on structural verification + 088 unblocked.** Ticket 027 (mating cadence three-bug cascade) closed at `e9efb4a6` — Bugs 1 + 2 landed at original commits; Bug 3 split into 027b → 082 (post-Wave-2 hardened substrate). The 2700s seed-42 closeout soak (`logs/tuned-42-027-closeout-2700s/`) confirmed every chain link upstream of `MatingOccurred` fires intact (PairingIntentionEmitted = 16740, CourtshipInteraction = 1154, BondFormed = 1) at 3× duration; terminal-tail rarity reframed as a chain property rather than a structural blocker. 088 (Body-distress Modifier) frontmatter `blocked-by: [014]` cleared — 014 had landed at `453ea83` (2026-04-27, three days before 088 was opened); 087 + the §L2.10 / §3.5.1 Modifier pipeline are also live. 088 is now `ready` and is the substrate prerequisite for 047 (`CriticalHealth` interrupt retirement). Roster delta: -027 (done), 088 blocked → ready. Total open 12 → 11 (0 in-progress, 8 ready, 0 blocked, 3 parked).
- 2026-04-30: **094 landed.** New `StockpileSatiation` Modifier in §3.5.1 mirroring `FoxTerritorySuppression`'s shape — multiplicative damp on Hunt and Forage scaled by `food_fraction` excess over a threshold (default 0.5) up to a max suppression scale (default 0.85). Two new `ScoringConstants` tunables; seven new unit tests. Verification on the seed-42 deep-soak: total deaths 8 → 1 (no starvations), `FoodEaten` 207 → 407 (2.0×), Hunting plans −57%, Foraging plans −85%, Resting plans 4×, Lark hunger end 0.20 → 0.89, Nettle alive. **Cascade observation**: damping the food-acquisition class freed election cycles for the rest of the catalog — three never-fired social positives (`BondFormed`, `CourtshipInteraction`, `PairingIntentionEmitted`) started firing, courtship canary 0 → 210, anxiety interrupts −59%, ShadowFoxAmbush deaths 4 → 0. The case for the doctrine: get the score landscape right and three orthogonal behaviors recover for free. Roster delta: -094 (done). Total open 13 → 12. 094 joins 087 / 091 / 092 as the canonical landed set — 094 is the **natural-lever exemplar** (additive substrate, no override to retire).
- 2026-05-01: **088 landed.** New `BodyDistressPromotion` Modifier in §3.5.1 — additive lift on the six-DSE self-care class (Flee/Sleep/Eat/Hunt/Forage/GroomSelf) when `body_distress_composite > 0.7`, ramping linearly to +0.20 at full distress. Companion to 094's `StockpileSatiation` (multiplicative damp on the food-acquisition class) — together they give the §L2.10 modifier substrate two production-stable consumers of 087's interoceptive perception scalars. Registers in `default_modifier_pipeline` between Tradition and FoxTerritorySuppression so the additive lift on Eat fires *before* StockpileSatiation's multiplicative damp on Hunt/Forage; under combined high stockpile + high body distress the IAUS contest tilts twice toward Eat. Two new `ScoringConstants` tunables; seven new unit tests; full lib suite 1659/1659. **Two scope deviations**: (a) self-care class is six DSEs not seven — no `Rest` DSE exists, Sleep covers the role; (b) `Feature::BodyDistressPromotionApplied` deferred — no existing Modifier emits a Feature, follow-on **099** opened (blocked-by 047) for the substrate-quality version covering all modifiers uniformly if 047's verification surfaces a need. Magnitude tuning at default 0.20 deferred to **047** (consumer-driven). Roster delta: -088 (done), +099 (blocked). Total open 11 → 11 (0 in-progress, 7 ready, 1 blocked, 3 parked).
- 2026-05-01: **121 promoted into the epic.** Cold-start "cats stand around for ~1500 ticks" symptom traced to `PlannerZone::Wilds` resolution authoring a parallel feasibility language vs IAUS Explore's `LandmarkAnchor::UnexploredFrontierCentroid`. Same shape as 092 but on the anchor surface instead of the marker surface. Substrate-aligned fix in-progress (resolver consumes `ExplorationMap::frontier_centroid()` with `find_nearest_tile` as no-frontier fallback; `.or(Some(*pos))` self-target removed; `None` surfaces as `no_plan_found` post-091). 1672/1672 lib tests green; soak verification on seed 42 pending. Roster delta: +121 (in-progress). Total open 11 → 12.
- 2026-05-01: **121 landed.** Substrate cure shipped; soak shows total event count 880k → 583k, `deaths_by_cause.Starvation` 2 → 0, `anxiety_interrupt_total` +23%. **Cold-start window itself unchanged** (first `BuildingConstructed` still at tick 1_201_490 in both runs) — the structural drift is closed but the symptom persists, validating the §Approach §2/§3 carveouts. Sibling tickets 122 (Socialize IAUS/gate mismatch) and 123 (RecentDispositionFailures cooldown) unblocked + tagged `cluster: substrate-over-override`. Roster delta: -121 (done), 122 blocked → ready, 123 blocked → ready, +122/+123 onto roster (they were carveouts not yet listed). Total open 12 → 13 (0 in-progress, 9 ready, 1 blocked, 3 parked). 121 joins the canonical landed set as the **anchor-shape analogue of 092's marker-shape structural cure** for L2↔L3 feasibility-language drift.
