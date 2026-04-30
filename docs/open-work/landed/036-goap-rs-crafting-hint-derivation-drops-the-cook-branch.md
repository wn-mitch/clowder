---
id: 036
title: "`goap.rs` crafting_hint derivation drops the Cook branch"
status: done
cluster: null
landed-at: a879f43
landed-on: 2026-04-26
---

# `goap.rs` crafting_hint derivation drops the Cook branch

**Landed:** 2026-04-26 | **Tickets:** 036 (FoodCooked never fires — `goap.rs` `crafting_hint` derivation drops the Cook branch).

Restores the missing `CraftingHint::Cook` branch in the live disposition router. The legacy `src/systems/disposition.rs:947-969` path had the correct three-way comparison; the active `src/systems/goap.rs:1346-1363` path forked from it and was missing the Cook arm, so any time `select_disposition_via_intention_softmax` picked `Action::Cook` and returned `DispositionKind::Crafting`, the goap.rs caller routed the resulting plan to `PracticeMagic` (or `Herbcraft` as fallback) — `CraftingHint::Cook` was never produced by the live path even though `actions.rs:351` defines a complete RetrieveRawFood → Cook → DepositCookedFood action set for it.

**Hypothesis:** Adding the Cook branch routes Crafting-disposition cats to the cook chain whenever Cook score dominates Magic and Herbcraft ⇒ `Feature::FoodCooked` rises from 0 to ≥1 on the seed-42 deep-soak; the never-fired-expected canary clears.

**Concordance:** Mixed — structural fix landed, primary canary did not clear.

- **FoodCooked still 0.** The structural routing fix is necessary but not sufficient; the cook chain is failing somewhere downstream of routing (RetrieveRawFood, planner A* selection, or the runtime cook-step precondition). Tracked in ticket 039.
- **Reproduction loop unblocked.** Four previously-silent positives now fire on seed-42: `ItemRetrieved`, `KittenBorn`, `GestationAdvanced`, `KittenFed`. Strong evidence the routing fix is having a real downstream effect.
- **Disposition shift cascaded into other peer-groups.** `continuity_tallies.courtship` 804 → 0, `grooming` 71 → 19, `mythic-texture` 48 → 22. `BondFormed` and `CourtshipInteraction` newly silent. Likely cause: cats whose softmax decision shifted under the new routing now win different dispositions further downstream. Tracked in ticket 040.
- **Survival canaries hold.** Starvation 2 → 0, ShadowFoxAmbush 4 → 5, footer written. No regression on the hard-gate metrics.
- **Pre-fix baseline preserved** at `logs/tuned-42-a879f43-pre-cook-fix/` for reproducible diffing. Post-fix run at `logs/tuned-42/`.

**Decision to land despite mixed concordance:** the structural bug (port-from-legacy missing a branch) was real and worth fixing on its own merits regardless of whether FoodCooked clears. The downstream gaps are separate problems with separate causes. Bundling them would conflate three orthogonal investigations and make verification ambiguous.

**Deferred follow-ons:**
- Ticket 039 — FoodCooked still silent post-036; investigate Cook chain execution.
- Ticket 040 — Disposition shift caused continuity-canary regression on Courtship / Grooming / Mythic-texture; characterize and rebalance.

**Diagnostic trail (kept as worked example):** scoring-but-never-winning is invisible to `plan_failures_by_reason` (which is empty for Cook on the pre-fix soak), so the diagnostic chain ran through `last_scores` (Cook eligible at 0.51–0.69) versus the `current_action` histogram (Cook absent across 9645 snapshots). The bug was located by code-searching for `CraftingHint::Cook` and finding it produced only by the dead `disposition.rs` path. Useful template for the next "Feature::X never fires" investigation.

---
