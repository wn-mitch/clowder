---
id: 446
title: Silent canaries — meat preservation positives sit at zero while drying side fires
status: ready
cluster: items-crafting
initiative: []
added: 2026-05-21
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

The preservation pipeline classifies six features `Positive` and enrolls them in the per-soak canary at `src/resources/system_activation.rs:870-876`. Two sides:

- **Drying (fish) side** — `FoodLoadedOnDryingRack`, `FoodDried`: **both fire** in seed-42 soaks.
- **Smoking (meat) side** — `MeatLoadedOnSmokingRack`, `SmokingRackTended`, `MeatSmoked`: **none fire** in seed-42 soaks. They're enrolled in the canary as hypothesis-load-bearing completions (per the inline comment at `system_activation.rs:1395-1399`), and the hypothesis is being falsified every soak.

The "silent canary" framing matters because three canaries fail simultaneously and the verdict surfaces them as a single `never_fired_expected_positives` list — making it look like one defect, not three independent ones with a shared upstream cause. The fact that the drying side works is the **diagnostic clue**: the canary substrate is fine, the classification is intentional, the test harness is correctly enrolling these — the asymmetry is in the **meat-side chain itself** (probably the path from `RawOrgan`/meat-drop to `SmokeMeat` plan template to load-rack to smoke completion). Observed at `logs/tuned-42-40397a72/` (290 landing run) and `logs/tuned-42-53a6bd27/` (323 backfill, pre-340 pre-290) — predates the recent stack, lives in the 443-era smoking-chain substrate.

This ticket is the **diagnostic shape** of the smoking-chain gap; sibling ticket 444 carries the **fix shape**. Open as separate tickets because the diagnostic question ("where does drying succeed and smoking fail?") is independent of the fix question ("which structural option do we pick?") — closing one doesn't close the other, and the contrast frame is what tells us which fix to pick in 444.

## Current architecture (layer-walk audit)

Rows are `[suspect]` until promoted via a fresh query that *distinguishes meat from fish at the named layer*. The discipline here is: at every layer, ask "what does this layer look like for drying-fish (works) vs. smoking-meat (silent)?"

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Item availability | hunt-drop in `src/steps/...` + `RawOrgan` from 367 Commit 6 | Drying side consumes fish (from forage chain); smoking side consumes meat/organ (from hunt chain). Does a healthy seed-42 colony produce hunt-drops at all? | `[suspect]` |
| Plan template | `src/ai/planner/actions.rs` (443) | Drying chain templates fire; smoking chain templates were added by 443 — do they reach `GoapPlan` emission ever? | `[suspect]` |
| L2 DSE eligibility | `src/ai/dses/smoke_meat.rs` vs `src/ai/dses/dry_food.rs` (if both exist) | Compare eligibility filters. The smoking DSE may require a marker the colony never sets. | `[suspect]` |
| Resolver | `src/steps/disposition/retrieve_smokeable_from_stores.rs` (443) + load/tend/complete steps | Drying side resolvers vs smoking side resolvers — same shape? Same Feature emission discipline? | `[suspect]` |
| Feature emit | resolver `record_if_witnessed` sites | Are the smoking-side resolvers actually emitting `Feature::MeatSmoked`/`SmokingRackTended`/`MeatLoadedOnSmokingRack` on success, or just advancing silently? | `[suspect]` — the never-fired-witness antipattern from CLAUDE.md GOAP Step Resolver Contract |
| Marker writers | `src/systems/goap.rs::evaluate_and_plan` snapshot | If a smoking-chain DSE has `.require(M::KEY)` where M isn't populated into `MarkerSnapshot`, the eligibility filter silently rejects every cat — same class as ticket 209/084. | `[suspect]` |

## Fix candidates

This ticket's job is to **diagnose the asymmetry**, not to fix it. The structural-option menu is in 444. Once a layer's `[suspect]` row promotes to a verified meat-vs-fish difference, 444's recommended-direction can be picked from evidence.

If the diagnostic surfaces a defect at a layer that's *not* already in 444's plan, this ticket may grow a sibling fix-shape ticket of its own.

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
