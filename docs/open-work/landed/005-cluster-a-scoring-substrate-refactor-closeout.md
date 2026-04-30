---
id: 005
title: Cluster-A scoring substrate refactor closeout
status: done
cluster: null
landed-at: feedbac
landed-on: 2026-04-27
---

# Cluster-A scoring substrate refactor closeout

**Landed:** 2026-04-27 | **Closeout:** umbrella retirement (this commit)

**Why retired:** ticket 005 was a 469-line umbrella opened 2026-04-20 covering the cluster-A IAUS substrate refactor (A1–A5). Its scope and lifecycle predated the current ticket-process conventions (one ticket = one shippable unit; status flips per-ticket, not per sub-track). After ticket 014's 2026-04-27 closeout (§4 marker catalog large-fill), the substantive cluster-A work had landed, but the umbrella's `status: in-progress` flag couldn't reflect partial closure of its five sub-entries (A1–A5) and three internal tracks (A/B/C). The "Still outstanding" list was mostly struck through and one bullet (§4 marker authoring rollout) was outright stale. The umbrella is being decomposed: one new successor ticket for the surviving structural item, downstream cluster tickets unblocked, body archived here.

**What landed under 005's umbrella across its lifetime (2026-04-20 → 2026-04-27):**

- **A1 Track A — substrate infrastructure** (Phase 3a+3b, 2026-04-20 → 2026-04-21). `Curve` enum (7 variants + 7 named anchors), `Consideration` (Scalar/Spatial/Marker), `Composition` (CP / WS / Max with RtM / RtEO + 0.75 compensation), `Dse` trait, `EligibilityFilter`, `DseRegistry` + 6-method registration, `ModifierPipeline`, `evaluate_single`, `select_intention_softmax`. 30 cat+fox DSE factories under `src/ai/dses/`, all routed through `score_dse_by_id → evaluate_single`.
- **A1 Track B — per-axis curve migration** (§2.3 rows 1–6, completed 2026-04-23). `hangry()` / `sleep_dep()` / `loneliness()` / `scarcity()` / `flee_or_fight()` / `fight_gating()` / `piecewise()` day-phase axes / `inverted_need_penalty()` / `Composite`+`ClampMin`+`ClampMax` plus the five corruption-axis migrations.
- **A1 Track C — §4 marker authoring** (2026-04-22 → 2026-04-27, completed via ticket 014 closeout). All §4.3 markers except the §9.2 faction overlay now have author systems and consumer-side `.require()`/`.forbid()` cutovers (or are scheduled for cutover via ticket 051 for the fox-side residue).
- **A2 — `big-brain` evaluation** resolved as **build in-house**. The L2 substrate at `src/ai/eval.rs` is the outcome.
- **A3 — context-tag uniformity refactor.** Exit criterion ("at least one action migrates to a pure-tag-filter entry guard as proof-of-pattern") met; bulk per-marker ports landed via Track C.
- **A4 — target selection as inner optimization** (Phases 4b.3 + 4c.1–4c.7, 2026-04-22 → 2026-04-23). §6.3 `TargetTakingDse` foundation + all nine §6.5 per-DSE target-taking ports (Socialize / Mate / Mentor / Groom-other / Hunt / Fight / ApplyRemedy / Build / Caretake). `find_social_target` retired.
- **A5 — focal-cat replay instrumentation** (Phase A1.2). At-source L2/L3 capture through `evaluate_single_with_trace` / `ModifierPipeline::apply_with_trace` / `select_disposition_via_intention_softmax_with_trace`. Replay-frame joinability per §11.4.
- **§3.5 modifier port** (Phase 4a + follow-ons). All 10 §3.5 modifiers ported to first-class `ScoreModifier` impls in `src/ai/modifier.rs`. `default_modifier_pipeline` hands out all 10 passes in retiring-inline-order. Inline `score_actions:666–750` block deleted.
- **§13.1 retired-constants cleanup** (2026-04-23, two commits). Rows 1–3 (Incapacitated pathway) + rows 4–6 (corruption-emergency-bonus pathway) shipped as separate refactor commits; all 8 retired constants + 3 retired modifier impls gone.
- **§7 commitment strategies (§7.2 + §7.3)** (2026-04-23 → 2026-04-24, four sessions). Root cause was an LLVM optimization cliff resolved by splitting `resolve_goap_plans` (797 lines) + `dispatch_step_action` (1,275 lines, `#[inline(never)]`).
- **`Incapacitated` DSE consumer cutover** (2026-04-23). `.forbid("Incapacitated")` on every non-Eat/Sleep/Idle cat DSE + every fox DSE. Inline `is_incapacitated` branch at `scoring.rs:574–598` retired.
- **`resolve_disposition_chains` split** (LLVM cliff prevention). Extracted 875-line dispatch into `dispatch_chain_step` with `#[inline(never)]` + `ChainStepSnapshots` / `ChainStepAccumulators`.

**Successor tickets at retirement:**

- [049](../tickets/049-faction-overlay-markers.md) — §9.2 faction overlay markers (Visitor / HostileVisitor / Banished / BefriendedAlly).
- [050](../tickets/050-marker-predicate-refinements.md) — §4 marker predicate refinements: species-attenuated `HasThreatNearby`, truthful `WardNearbyFox`, event-driven `HasCubs` / `HasDen`.
- [051](../tickets/051-fox-dse-eligibility-migration.md) — fox DSE eligibility migration: `.require()` / `.forbid()` cutover for fox raiding / den-defense / feeding / dispersing.
- [052](../tickets/052-l2-10-7-plan-cost-feedback.md) — §L2.10.7 plan-cost feedback: `SpatialConsideration` curves on spatially-sensitive DSEs; unblocks 4 §6.5 deferred axes (`pursuit-cost`, `fertility-window` spatial, `apprentice-receptivity` spatial-pairing, `remedy-match` caretaker-distance).

**Stale-bullet note:** the body's "Still outstanding" list still carried `§4 marker authoring rollout (~43 markers). Life-stage, state (minus Incapacitated), capability, target-existence, and colony markers still unauthored.` That bullet was true on 2026-04-23 but false by 2026-04-27 after ticket 014's closeout. The umbrella's `status: in-progress` flag prevented routine landed-marking from back-porting closure into the body — exactly the staleness pattern that motivated this decomposition.

**Downstream tickets unblocked at retirement:** 006 (cluster B) / 007 (cluster C) / 009 (cluster E) had `blocked-by: [005]` and now have `blocked-by: []` (status flipped to `ready`). 011 / 018 / 021 had 005 plus other blockers; 005 dropped, other blockers remain. Every downstream ticket's cluster-A dependency was on **A1 (the IAUS refactor itself)**, which fully landed; none gated on the surviving spinoffs.

**Verification:** `just check` clean (no code changes — pure ticket-process). `just open-work-index` regenerates `docs/open-work.md` with In progress 5→4, Ready 21→25, Blocked 9→6.

---
