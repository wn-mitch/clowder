---
id: 091
title: "Post-087 seed-42 plan-execution collapse — Eat picks 62% but FoodEaten never witnesses; no Forage/Hunt; founder starvation cascade"
status: done
cluster: null
landed-at: 25439daf
landed-on: 2026-04-30
---

# Post-087 seed-42 plan-execution collapse — Eat picks 62% but FoodEaten never witnesses; no Forage/Hunt; founder starvation cascade

**Landed-at:** `25439daf` (HEAD-reachable; bundled into ticket 092's commit per jj history). The frontmatter recorded `fa0f3a84`, an earlier jj snapshot rewritten during rebase; multiple intermediate snapshots existed (`64f7abe8`, `c20e243`, `3f6db74`, `b00672d`, `e3c22d3`, `f47e8f1`, `ef5d3ea`) — all are hidden jj revisions of the same change, not divergent landings. The actual code (removed `enforce_survival_floor`, plumbed `HasStoredFood` into `StatePredicate` then collapsed via 092, removed Carrying-vetoes from Forage/Hunt, partial Resting goal) lives in HEAD inside the 092 commit.

**Why.** Surfaced by the 087 verification soak (`logs/tuned-42` at commit `fc4e1ab8`, vs. baseline `logs/tuned-42-pre-087/` at `e838bb7`). Substrate-and-DSE-adoption landed test-green (`just check` + `cargo test --lib` 1640/1640 pass) but the canonical seed-42 deep-soak showed a colony-action collapse in the logged tail-window (ticks 1.2M → 1.21M, the only ticks `just soak` writes events for). Reads as either a balance issue from the Sleep/Flee axis additions or a DSE→GOAP plan-execution disconnect — *not* a strict sim regression.

**Substrate-over-override role.** This ticket is the **cautionary case** demonstrating the sequencing rule: partial substrate adoption causes collapse. 087 retired part of `CriticalHealth` (gave Sleep + Flee new axes) but didn't extend the pattern to Eat. The substrate replacement wasn't expressive enough yet, and the colony food economy collapsed when `CriticalHealth` interrupt telemetry zeroed (19,382 → 0). Lesson for the rest of the 093 thread: **substrate axes land first; the corresponding hack retires second.**

Three hack classes were addressed:

- **Class A — L2↔L3 sync gap.** GOAP `EatAtStores` action required only `ZoneIs(Stores)`, not `HasStoredFood`. IAUS `EligibilityFilter` and GOAP `StatePredicate` were two parallel feasibility languages; the substrate refactor wired markers into IAUS but not into the planner. H1 fix plumbed `HasStoredFood` into `StatePredicate` as a substrate-side close. Generalized via 092's `StatePredicate::HasMarker(...)` structural cure.
- **Class B — silent-advance step resolvers.** `eat_at_stores` returned silent `unwitnessed(Advance)` on missing food. H4 fix converted to `Fail` so canaries see drift.
- **Class C — producer-side substrate gap (residual, post-H1+H4).** Forage/Hunt scored 0.97/0.89 in IAUS but 0% executed. Root cause turned out to be (a) `enforce_survival_floor` post-hoc clamp suppressing producer scores to 0.886, (b) `ForageItem`/`SearchPrey` requiring `CarryingIs(Carrying::Nothing)` — a permanent veto for cats carrying leftover herbs/forage.

**What landed (in order).**

1. **Telemetry hardening (Phase 1).** Added `EventKind::PlanningFailed { cat, disposition, reason, hunger, energy, temperature, food_available, has_stored_food }` and `EventLog.planning_failures_by_disposition: BTreeMap<String, u64>` footer field. Replaced silent `// If no plan found, cat stays idle` at `src/systems/goap.rs:1760` with an emit block. Footer wiring in `src/plugins/headless_io.rs` and `src/main.rs`. Unit test `planning_failed_increments_per_disposition_tally`. The post-Phase-1 footer immediately revealed `{Foraging: 20599, Hunting: 23580, Resting: 18340, Crafting: 3789}`, localizing the bug to the planner phase before reading a single trace line.
2. **Diagnosis (Phase 2/3).** Focal trace `logs/tuned-42/trace-Nettle.jsonl` showed Sleep=Hunt=Forage tied at 0.886 across the cascade window — a clamp signal, not natural scoring. Three root causes identified: `enforce_survival_floor` post-hoc clamp (Class C); `CarryingIs(Carrying::Nothing)` over-restrictive precondition on producer actions (Class C); H1's `HasStoredFood` precondition making the legacy three-need Resting goal unreachable when stores empty + hunger unmet (cascading Sleep regression).
3. **Removed `enforce_survival_floor` hack (Phase 4a).** Deleted function at `scoring.rs:1455`, four unit tests, two production call sites (`disposition.rs:1012`, `goap.rs:1501`), imports, and the `survival_floor_phys_threshold` field from `ScoringConstants` in `src/resources/sim_constants.rs`. The post-removal trace confirmed the IAUS layer was correctly identifying Forage as top action all along; the clamp had been hiding it. 1640/1640 lib pass after removal.
4. **Producer-side fix (Phase 4).** Removed `CarryingIs(Carrying::Nothing)` precondition from `SearchPrey` and `ForageItem` (`src/ai/planner/actions.rs:31-93`). The deposit chain still works: ForageItem sets `Carrying::ForagedFood` which DepositFood consumes; EngagePrey sets `Carrying::Prey` which DepositPrey consumes. New tests `foraging_with_carried_herbs_still_plans` and `hunting_with_carried_herbs_still_plans`.
5. **Sleep regression fix (Phase 5).** Changed `goal_for_disposition(kind, current_trips)` → `goal_for_disposition(kind, current_trips, has_stored_food)` (`src/ai/planner/goals.rs`). Resting goal drops `HungerOk` when `!has_stored_food`, leaving `[EnergyOk, TemperatureOk]` so a hungry-tired-cold cat with empty stores can still Sleep+SelfGroom and re-elect (Foraging/Hunting) on the next decision tick. Three call sites updated in `src/systems/goap.rs`. Test `resting_goal_drops_hunger_when_stores_empty`. The full→partial Resting goal restored baseline (e838bb7) Sleep behavior.

**Verification (Phase 6 final footer).**

| Metric | Pre-091 (post-H1 broken) | Post-091 (final) |
|---|---|---|
| Founder Starvation | 8 (cascade) | 1 (isolated, Nettle at tick 1304677) |
| PlanCreated{Foraging} | 0 | 57,954 |
| PlanCreated{Hunting} | 0 | 30,757 |
| PlanCreated{Resting} | 2,495 | 57,099 |
| PlanningFailed{Foraging} | 20,599 | 2,443 |
| PlanningFailed{Hunting} | 23,580 | 1,949 |
| FoodEaten | never | fires |
| grooming canary | 0 | 104 |
| mythic-texture | 0 | 25 |
| positive_features_active | 13/47 | 20/47 |

Cascade resolved (88% Starvation reduction, 8→1). Producer-side dispositions dominate planning. The 1 residual Starvation is a late-game isolated death (Nettle, tick 1304677, 5k+ ticks after her last successful PlanCreated), not a founder cascade — different shape, almost certainly a different root cause. `just verdict` still returns `fail` on the survival hard-gate (`Starvation == 0`) and continuity (`mentoring=0,burial=0,courtship=0` — pre-existing 087-era issues, not 091).

**Honest residual at land (not closed by 091).** User pointed out the contradiction in the verification footer: `FoodEaten` is still in `never_fired_expected_positives` even though only 1 cat starved. Investigation showed cats are *slowly* starving — they haul food but don't consume it. Lark at hunger 0.20: `last_scores = [Hunt 0.85, Forage 0.61, Groom 0.55, ...]`. Eat doesn't appear in top scores at all. Two natural-lever gaps:

- Forage's IAUS score outranks Eat's even when the cat is at Stores with `HasStoredFood=true`. The natural lever (Eat curve / stores-proximity multiplier / satiation modifier) doesn't express "if you're at the pantry and hungry, eat first."
- No preemption breaks a long Forage plan when hunger crosses a threshold mid-trip. The 087 interoceptive markers (`pain_level`, `body_distress_composite`) should produce that pressure but apparently don't reach interrupt strength.

These are substrate-over-override-shaped (093 thread) — IAUS levers, not a hack. Not in 091's scope, but the cascade-fix was a necessary precondition for surfacing them.

**Out-of-scope follow-ups identified during 091's investigation** (open as separate tickets if not already covered by 092/093):

- **Eat-vs-Forage IAUS scoring imbalance** — the food-loop bug above. Highest-priority follow-up.
- **Forage/Hunt commitment-preemption gap** — long producer-side plans don't break for Eat when hunger crosses a critical threshold.
- **Crafting Carrying-veto** — `GatherHerb` requires `CarryingIs(Carrying::Nothing)` (`actions.rs:270-281`, `292-297`); same pattern as the Forage/Hunt fix. Crafting planning failures jumped from 3,330 → 23,595 post-fix because cats now carry food/prey often, vetoing herb gathering.
- **Residual late-game Nettle starvation** — Nettle's last successful PlanCreated was at tick 1,299,505 (Resting), then no plans for 5,172 ticks until death.
- **Continuity regressions** — `mentoring=0`, `burial=0`, `courtship=0` (vs pre-087 baseline `courtship=764`, `mythic-texture=50`, `grooming=60`). Predates 091; 087-era social-stack regression that the 093 epic absorbs.

**Surprise.** The investigation found that `enforce_survival_floor` was itself a substrate-over-override hack — a third hack falling out under the 093 lens. Telemetry hardening (Phase 1) was load-bearing here: the PlanningFailed footer disambiguated planner-phase from execution-phase before any trace reading. And H1 (the IAUS-side close), which seemed like the obvious tactical fix, turned out to be necessary-but-not-sufficient: the producer-side gap and the Sleep-goal cascade had to be addressed in the same sweep. The structural cure (092's `StatePredicate::HasMarker`) absorbed H1 cleanly when it landed.
