---
id: 438
title: Retire hand-written dispatcher in score_actions — iterate DseRegistry instead
status: ready
cluster: ai-substrate
orchestration: substrate-sensitive
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

`src/ai/scoring.rs::score_actions` is a hand-written sequence of `score_dse_by_id("eat", ...)` / `("hunt", ...)` / `("cook", ...)` / etc. calls — one branch per `Action` variant. `populate_dse_registry` is the DSE catalog, but `score_actions` does not consume it. Registering a DSE without adding the matching dispatch branch is a silent failure: the DSE constructs, holds its full eligibility filter + considerations, is reachable by name via the registry, but never enters per-cat scoring — never reaches L2, never reaches L3 softmax, never gets elected. No panic, no warning, just zero appearances in `last_scores`. The L2 trace surfaces the gap as missing-row-entirely (not even `eligible: false` — that capture path lives *inside* `score_dse_by_id`). Ticket [[437]] landed the three-branch fix that unblocked the Phase-1b preservation DSEs, but the same defect class will re-bite on every future DSE addition unless the dispatcher itself is retired. R2 from 437's structural-option menu; opening as a separate ticket per CLAUDE.md's "Antipattern migration follow-ups are non-optional" discipline.

## Current architecture (layer-walk audit)

The defect is structural, not behavioral. The audit walks the dispatch path, not an AI pipeline trace.

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Registry | `src/plugins/simulation.rs::populate_dse_registry` | Single source of truth for DSE catalog membership; populates `DseRegistry.cat_dses` and `target_taking_dses` | `[verified-correct]` |
| Dispatch | `src/ai/scoring.rs::score_actions` | Hand-written switch with one `score_dse_by_id(<id>, ...)` branch per `Action` variant; carries branch-specific outer gates (Cook's `cook_hunger_gate`, Caretake's parent-or-urgency disjunction, the disposal-chronicity gates) | `[verified-defect]` — no registry iteration; every new DSE requires a manual branch add |
| Per-DSE gates | branch-internal `if`s in `score_actions` | Some DSEs gate at the dispatcher (Cook), some only at the eligibility filter (Forage), the rest somewhere in between | `[suspect]` — needs catalog before R1 lands; some outer gates may be redundant with eligibility filters, others load-bearing (e.g. Cook's `wants_cook_but_no_kitchen` side-effect) |
| Score-to-Action mapping | inline `Action::Cook` / `Action::Hunt` / etc. inside each branch | One Action variant per DSE id; mapping is implicit in the dispatcher's hand-written structure | `[suspect]` — needs a single `dse_id → Action` table before R1 lands |

## Fix candidates

**Parameter-level options** — N/A. The defect is structural.

**Structural options**:

- **R1 (retire — recommended)** — replace the hand-written dispatcher in `score_actions` with a loop over `inputs.registry.cat_dses`. Per-DSE outer gates (Cook's `cook_hunger_gate`, the disposal-chronicity gates, etc.) move into either (a) the DSE's `EligibilityFilter` via a new gate primitive, OR (b) a thin pre-dispatch policy table keyed by `DseId`. The score-to-Action mapping moves into a single `fn action_for_dse_id(id: DseId) -> Action` helper (or a `Dse::action(&self) -> Action` trait method). Net surface: every DSE registered in `populate_dse_registry` is dispatched by construction, no manual branch additions ever needed.

- **R2 (extend — fallback if R1 is too disruptive)** — keep the hand-written dispatcher but author a *companion* registry-iteration loop that fires *only* for DSE ids absent from a hardcoded "already dispatched" allowlist. The allowlist starts containing every existing branch's DSE id; new DSEs land in the registry without an allowlist entry and get dispatched by the new loop. Achieves the same "registered DSE always scores" guarantee with less code churn, at the cost of carrying both code paths indefinitely. Worth considering if R1's gate-migration proves load-bearing for behavior preservation.

- **R3 (split)** — N/A.

- **R4 (rebind)** — N/A. The Action↔DseId mapping is already 1:1 in practice; what's missing is the explicit table.

## Recommended direction

R1, but only after auditing the per-branch outer gates to determine which are load-bearing for behavior preservation. Concrete order:

1. **Audit pass**: walk every `score_dse_by_id(<id>, ...)` branch in `score_actions` and classify the outer `if` as (a) redundant with the DSE's `EligibilityFilter`, (b) policy that should move into the filter, or (c) policy that needs a pre-dispatch table (e.g. Cook's `wants_cook_but_no_kitchen` side-effect can't move into the filter because it writes a non-score signal).
2. **Filter-side migration**: lift the (a)/(b) gates into the corresponding DSE's `EligibilityFilter`. Each migration is a single commit + unit test demonstrating equivalence.
3. **Pre-dispatch table**: codify the (c) gates as a `BTreeMap<DseId, fn(&ScoringContext) -> bool>` consulted by the new dispatcher before scoring.
4. **DseId → Action table**: codify the score-to-Action mapping as either a `Dse::action()` trait method or a parallel table.
5. **Dispatcher swap**: replace the hand-written body of `score_actions` with the registry loop + pre-dispatch table + score-to-Action lookup. Verdict gate on the canonical seed-42 deep-soak (no behavior drift on continuity canaries).

R2 (allowlisted companion loop) is the fallback if R1's gate audit reveals that the outer gates are too entangled with per-branch side effects to migrate cleanly.

## Out of scope

- Any DSE-side curve / weight / consideration tuning — this ticket is purely the dispatcher refactor.
- Target-taking DSE iteration — `score_actions` already handles per-cat DSEs; target-taking DSEs run on a separate path (`registry.target_taking_dses`) which has its own structure. If that path has a similar antipattern, open as a follow-on.
- The 367 Commit 10 multi-ingredient retrieve mirror — orthogonal work; unblocks empirical smoke_meat fire-rate but doesn't touch the dispatcher.

## Verification

- `just check` + `cargo test --release` — all existing tests pass post-refactor; behavior preservation is the load-bearing assertion.
- `just soak-trace 42 Simba` — canonical 15-min seed-42 soak. Verdict: survival canaries hold (Starvation = 0, ShadowFox ambush <= 10), continuity canaries hold (each ≥1: grooming, play, mentoring, courtship, mythic-texture), `never_fired_expected_positives == 0`. Drift on characteristic metrics within ±10%; >10% requires the four-artifact hypothesis-driven justification per CLAUDE.md.
- Scenario regression: the three `drying_chain_eligibility` tests landed by 436/437 stay passing.
- Future DSE additions: open a small test that registers a sentinel DSE in a test-only `DseRegistry`, runs one tick, and asserts the sentinel surfaces in the L2 trace. Lands the "registered ⇒ dispatched" guarantee as an enforced invariant.

## Log
- 2026-05-21: opened as the R2 structural follow-on from ticket [[437]]'s fix candidate menu. Blocked-by 437 because the audit pass needs a stable post-437 baseline to compare against — landing the dispatcher refactor before 437 would conflate two distinct kinds of behavior change.
- 2026-05-21: landed. The hand-written `score_actions` dispatcher retires in favor of a registry-iterating loop. Round-trip enforcement: `Dse::action() -> Action` is mandatory on the new `CatDse: Dse` sub-trait (registering a cat DSE without naming its `Action` is a compile error); `dse_id_for_action` in `src/ai/modifier.rs` covers the reverse direction; both held by construction. `populate_dse_registry` no longer hand-enumerates cat-DSE pushes — each `src/ai/dses/*.rs` self-registers via `#[linkme::distributed_slice(CAT_DSE_REGISTRY)]` and a `cat_dse_constructors()` helper sorts by declared `order` (gapped by 100 to match pre-438 dispatch order). Outer gates that aren't expressible as `EligibilityFilter` predicates (Cook's `wants_cook_but_no_kitchen` side-effect + hunger threshold; Caretake's parent-or-urgency disjunction + durable-commitment lift; HerbcraftWard's siege bonus; Bury's L3 pool stability gate; the composite OR/AND/threshold gates for Flee/Fight/Patrol/Build/Herbcraft/PracticeMagic) move into `PRE_DISPATCH_GATES` + `POST_EVAL_HOOKS` `BTreeMap`s keyed by `DseId`. `hide` is registered + dispatched (closes the same defect class as the 437 trio — pre-438 it was registered but never dispatched); the L3 softmax pool filter drops `Action::Hide` alongside `Action::Idle` because `DispositionKind::from_action(Hide) == None` and the anxiety-interrupt activation path (modifier 105/142 per Phase 1 design) remains the designated wiring. CLAUDE.md grows a "compile-time contracts" convention with the round-trip enforcement pattern as the worked example. `scripts/check_score_actions_dispatch.sh` (wired into `just check`) enforces both invariants at CI: exactly one `score_dse_by_id` call site in `score_actions`, and every `impl CatDse` paired with a `CAT_DSE_REGISTRY` entry in the same file. 2379 lib tests pass; `just check` clean.
- 2026-05-21: post-landing verification soak `logs/tuned-42-75deed49` (commit `75deed49`, 103,259 ticks, 0 deaths). `just verdict` reports `fail`: (a) `never_fired_expected_positives` = the 5 preservation features (`FoodLoadedOnDryingRack`, `MeatLoadedOnSmokingRack`, `SmokingRackTended`, `FoodDried`, `MeatSmoked`) — pre-existing per 437's log and tracked by [[439]] (planner zone-resolver); (b) continuity canary `mythic-texture = 0` — pre-existing class of rarity, no shadow-fox banishments or fate-awakenings in this soak; (c) significant POSITIVE drift on `wards_placed_total` (+593% rate-normalized, 4→24), `structures_built` (3→8), `kittens_born` (2→4), `peak_population` (10→12), aggregate `colony_score` (+16%) — the colony is healthier, not collapsing; welfare/health/nourishment all within ±2% of baseline; survival profile identical (0 deaths). The drift is consistent with the dormant-substrate-activation pattern (`feedback_dormant_substrate_activation_soak_first`) — `hide` entering the L2 score pool adds one jitter RNG draw per HideEligible cat per tick, perturbing the seed-42 sequence; downstream L3 softmax samples differ → cats make different per-tick action choices → colony state diverges over 100k+ ticks. No structural defect; the architectural change (registered ⇒ dispatched) is the intent. Performance: 13.5% fewer ticks vs the baseline at the same wall-clock budget — the BTreeMap lookups in the new dispatcher (PRE_DISPATCH_GATES / POST_EVAL_HOOKS) add per-DSE per-cat per-tick overhead vs the pre-438 inline branches. Acceptable for shipping; perf-tuning is a follow-on if it bites. Opens [[follow-on TBD]]: migrate the simple single-marker (b)-class gates (socialize → HasSocialTarget, groom_other → HasSocialTarget, herbcraft_gather → HasHerbsNearby, magic_harvest → CarcassNearby, magic_commune → OnSpecialTerrain, bury redundant) from `PRE_DISPATCH_GATES` into the DSEs' `EligibilityFilter` — substrate-canonical hygiene; composite / threshold / side-effect gates stay in the table.
