---
id: 2026-04-22
title: "Phase 4b.2 MVP — §4 marker lookup foundation + `HasStoredFood` reference port"
status: done
cluster: null
landed-at: null
landed-on: 2026-04-22
---

# Phase 4b.2 MVP — §4 marker lookup foundation + `HasStoredFood` reference port

First end-to-end §4 marker port. `has_marker` moves from its
`|_, _| false` stub at `scoring.rs:435` to a real lookup against
a new `MarkerSnapshot` type threaded through `EvalInputs`.
`EatDse` gains `.require("HasStoredFood")`; the inline outer
`if ctx.food_available` gate at `score_actions` retires (both
the non-incapacitated and incapacitated code paths). The caller
populates `markers.set_colony("HasStoredFood", !food.is_empty())`
at the top of each scoring tick.

Pattern is now set for the remaining ~49 §4.3 markers: one
authoring-site line per marker in the caller, one `.require(...)`
row on the target DSE, optionally a per-tick system if the
predicate is expensive enough to cache. The canonical spec shape
(markers as ZST components on a `ColonyState` singleton) is a
drop-in refactor later — only the caller-side population logic
shifts; the evaluator-side surface stays identical.

Tests: 5 new `marker_snapshot_*` unit tests (empty pool, colony
scoping, entity scoping, clear semantics, clear-doesn't-nuke-peers).
`eat_dse_requires_has_stored_food` + `eat_dse_rejected_without_has_stored_food_marker`
replace the placeholder `eat_dse_has_no_eligibility_filter_today`
test (which named itself as a Phase 3d-to-flip placeholder).
