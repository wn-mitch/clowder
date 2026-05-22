---
id: 446
title: Silent canaries — meat preservation positives sit at zero while drying side fires
status: done
cluster: items-crafting
initiative: []
added: 2026-05-21
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: pending
landed-on: 2026-05-21
---

## Why

The preservation pipeline classifies six features `Positive` and enrolls them in the per-soak canary at `src/resources/system_activation.rs:870-876`. Two sides:

- **Drying (fish) side** — `FoodLoadedOnDryingRack`, `FoodDried`: **both fire** in seed-42 soaks.
- **Smoking (meat) side** — `MeatLoadedOnSmokingRack`, `SmokingRackTended`, `MeatSmoked`: **none fire** in seed-42 soaks. They're enrolled in the canary as hypothesis-load-bearing completions (per the inline comment at `system_activation.rs:1395-1399`), and the hypothesis is being falsified every soak.

The "silent canary" framing matters because three canaries fail simultaneously and the verdict surfaces them as a single `never_fired_expected_positives` list — making it look like one defect, not three independent ones with a shared upstream cause. The fact that the drying side works is the **diagnostic clue**: the canary substrate is fine, the classification is intentional, the test harness is correctly enrolling these — the asymmetry is in the **meat-side chain itself** (probably the path from `RawOrgan`/meat-drop to `SmokeMeat` plan template to load-rack to smoke completion). Observed at `logs/tuned-42-40397a72/` (290 landing run) and `logs/tuned-42-53a6bd27/` (323 backfill, pre-340 pre-290) — predates the recent stack, lives in the 443-era smoking-chain substrate.

This ticket is the **diagnostic shape** of the smoking-chain gap; sibling ticket 444 carries the **fix shape**. Open as separate tickets because the diagnostic question ("where does drying succeed and smoking fail?") is independent of the fix question ("which structural option do we pick?") — closing one doesn't close the other, and the contrast frame is what tells us which fix to pick in 444.

## Current architecture (layer-walk audit)

Promoted via static code-read across both pipelines (this session) and behavioral query against `logs/tuned-42-40397a72/events.jsonl`. The static layers are structurally symmetric across all 8 rows; the asymmetry is locked at runtime — `SmokeMeat` never clears its eligibility filter under healthy seed-42 colony shape, so `score_dse_by_id` is never invoked and no L3 plan ever emits.

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L2 DSE definition | `src/ai/dses/dry_food.rs:43-173` vs `src/ai/dses/smoke_meat.rs:26-177` | Identical weighting `[0.32, 0.24, 0.24, 0.20]`, identical Maslow tier 2, parallel eligibility filters (`CanX::KEY` + `HasFunctionalXRack::KEY` + `HasXAccessible::KEY` + `forbid(Incapacitated)`). | `[verified-correct]` |
| DSE dispatcher | `src/ai/scoring.rs:1475-1482` | Registry-driven via `inputs.dse_registry.cat_dse(dse_id)`; no hand-written `score_dse_by_id` branches required since ticket 438. Both DSEs participate uniformly. | `[verified-correct]` |
| DSE registry | `src/plugins/simulation.rs:21,38`; `dry_food.rs:218-222`, `smoke_meat.rs:172-177` (linkme entries) | Both auto-registered via `linkme::distributed_slice` (order 3000 / 3100); no missing registration. | `[verified-correct]` |
| Plan template | `src/ai/planner/actions.rs:363-417` (drying) vs `438-483` (smoking) | Both follow the same `[DropItem?, RetrieveX, X]` shape and emit Goal labels that map to GoapPlan dispatch. | `[verified-correct]` |
| Resolver Feature emission | `load_drying_rack.rs:51` vs `load_smoking_rack.rs:45`; `tend_smoking_rack.rs:53,55` (per `goap.rs:7877-7894`); `preservation.rs:148-155` (drying completion) | Every smoking-side resolver passes the corresponding `Feature::*` to `record_if_witnessed`; no silent-advance antipattern. Both sides emit their three Features when the step actually executes. | `[verified-correct]` |
| Marker writers (static) | `src/systems/goap.rs:1916-1955` (snapshot build); `src/systems/buildings.rs:740-757` (functional-rack idle checks) | All required markers (`CanX`, `HasFunctionalXRack`, `HasXAccessible`) are authored into `MarkerSnapshot` before evaluation for both pipelines; not a ticket 209/084-class wiring gap. | `[verified-correct]` |
| Item availability | hunt-drop in `src/systems/disposition.rs:3715-3760` (carcass + organ); forage chain (fish, wood) | Code path for meat / organ / fish / wood production is present. RawOrgan is secondary-drop (roll-gated); meat is primary-drop. Per-event probing of stores not possible from `events.jsonl` (no `ItemStored` event), but behavioral L2 evidence (next row) supersedes. | `[verified-defect-not-here]` |
| Feature canary classification | `src/resources/system_activation.rs:870-876,1100,1395-1399` | All six preservation Features classified `Positive`; drying pair + smoking triple all return `expected_to_fire_per_soak() == true` (via bare `_ => true` default). Drying pair fires in healthy seed-42; smoking triple does not — classification is correct, the silence is downstream. | `[verified-correct]` |
| **Runtime DSE eligibility (behavioral)** | `last_scores` table in every `CatSnapshot` across `logs/tuned-42-40397a72/events.jsonl` | **Decisive finding.** Querying `[.results[].record.last_scores[][0]] \| unique` over the entire run: `DryFood` is present; **`SmokeMeat` is absent.** No cat ever scored `SmokeMeat`, which means the eligibility filter rejected every cat every tick. One (or more) of `CanSmoke::KEY` / `HasFunctionalSmokingRack::KEY` / `HasSmokeableAccessible::KEY` never held. Plan-creation evidence corroborates: `PlanCreated` shows `DryingFood` 3 times in the run, no smoking-side disposition appears at all. | `[verified-defect]` |
| **Composite gate shape** | `src/ai/dses/dry_food.rs:128` vs `src/ai/dses/smoke_meat.rs:87`; `goap.rs:1935-1955` (composite definitions) | Drying side requires `HasDryableAccessible = has_dryable \|\| (has_free_slot && has_dryable_in_stores)` — disjunction over substitutable raw inputs (`fish OR organ`). Smoking side requires `HasSmokeableAccessible = has_smokeable \|\| (has_free_slot && has_smokeable_in_stores)` where `smokeable` itself is a conjunction over meat AND fuel. The smoking conjunction never resolves under healthy seed-42 — meat and wood are produced by different subsystems (hunt vs forage), and co-presence in stores doesn't reliably occur. **This is the substrate-shape root of the silent canary.** | `[verified-defect]` |

## Fix candidates

This ticket's job is to **diagnose the asymmetry**, not to fix it. The structural-option menu is in 444; with the layer-walk now promoted, 444's R-selection rests on the bottom two rows.

- The eight static layers are clean — the chain is structurally complete, no silent-advance, no missing marker wiring, no registry gap.
- The behavioral defect is that `SmokeMeat` never clears its eligibility filter at runtime, because the meat-AND-fuel conjunction inside `HasSmokeableAccessible` doesn't hold under healthy seed-42 colony shape.
- This pushes **R4 off 444's menu** (the chain is already extended), strengthens **R3** (retire canary as honest signal — the substrate genuinely does not exercise under current colony shape), and identifies a future structural arc ("split the conjunction into sequential retrievals" or "add a fuel-acquisition DSE") that should be its own ticket with a balance hypothesis.

## Out of scope

- The fix itself (lives in 444).
- mythic-texture continuity canary at zero (sibling ticket 445; different gate, different root cause).
- 367's broader preservation epic — closing this ticket does not require the whole epic to land.

## Verification

Diagnostic outputs (read-only investigations; no soak required):

- A markdown table comparing drying-side vs smoking-side at each of the six layers above, with `[verified-correct]` / `[verified-defect]` per row, evidence (file:line or `logq` query) cited.
- Either a clear pointer at one layer's defect (informing 444's R-selection), or evidence that the asymmetry lives at the eligibility filter / marker / hunt-drop layer (in which case 444's R5 `scenario` approach becomes more attractive because the colony-level soak is genuinely the wrong tool).

## Log
- 2026-05-21: opened as 444's diagnostic-shape sibling. The "silent canary" framing surfaces because three meat features fail simultaneously while two drying-side features pass — the contrast is the bug shape, not the failure count.
- 2026-05-21: closed. Layer-walk promoted via static code-read (8 rows symmetric) + behavioral query of `logs/tuned-42-40397a72/events.jsonl` (decisive: `SmokeMeat` never appears in any `last_scores` table; `DryFood` does; `PlanCreated` shows 3 `DryingFood` plans, zero smoking-side plans). Root cause: the meat-AND-fuel conjunction inside `HasSmokeableAccessible` never resolves under healthy seed-42, while the fish-OR-organ disjunction inside `HasDryableAccessible` does. 444 will land R3 (retire smoking-side canary classification) as honest signal pending a substrate arc to split the conjunction into sequential retrievals (or add a fuel-acquisition DSE).
