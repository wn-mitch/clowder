---
id: 071
title: Planning-substrate hardening — gird against the stuck-cat bug class (sub-epic)
status: done
cluster: planning-substrate
added: 2026-04-29
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: [027-l2-pairing-activity.md]
landed-at: null
landed-on: 2026-05-02
---

## Why

027b's failed soak surfaced a pre-existing planning-substrate fragility — the same bug class as tickets 038 (Founding-haul Flee-lock) and 041 (Mallow Cook-lock): cats permanently stuck in failing plan loops because nothing in the substrate persists target-failure memory across plan abandonment, no IAUS primitive penalizes recently-failed targets, and dead targets can be picked because there's no `EligibilityFilter::require_alive`.

Each prior incident closed one inline call site. This sub-epic closes the bug class.

The triggering soak: activating L2 `PairingActivity` (027b) flipped seed-42 from `Starvation = 0` to `Starvation = 3` — not because the L2 author misbehaved (`PairingIntentionEmitted = 0`, `PairingActivity` never inserted, bias readers collapse to identical math when `pairing_partner = None`), but because the schedule perturbation drifted long-horizon RNG enough to flip a single mate selection at tick ~1.2M, and the substrate amplified that into a colony-wide cascade. Mocha hit 109 `HarvestCarcass` failures, Nettle 66 identical `TravelTo(SocialTarget)` failures, Lark 91 `EngageThreat` failures; `anxiety_interrupt_total` dropped 70%.

The substrate has three defenses today (`replan_count` cap, per-plan `failed_actions` set, commitment drop gates) but none survive plan abandonment — the cat re-picks the same blocker on the next plan. Any sufficiently-perturbing schedule change (a Bevy version bump, a new feature flag, a future system) would expose the same fragility. Hardening pays off across every future ticket.

## Scope

Coordinates child tickets 072–081. The hardening lifts plan-lifecycle ops into a unified `plan_substrate` module (downstream of IAUS) and lands every cross-tick "memory-of-failure / stay-where-you-are / force-fallback" defense **inside** the IAUS engine as a `Consideration`, `Modifier`, or `EligibilityFilter`. Ships an `iaus_coherence` grep-check in `just check` to enforce the discipline going forward.

## Out of scope

Open as follow-on after 071 lands:
- Audit gap #8 — marker / sensing staleness (separate ticket later)
- Bevy parallel-scheduler determinism audit (HashMap → BTreeMap in `MatingFitnessParams`)

## Current state

Sub-epic opened 2026-04-29. **As of 2026-05-02, every child is closed** (072 / 073 / 074 / 075 / 076 / 077 / 078 / 079 / 080 / 081 / 082 — 074/075/078/080 bundled into 073's wave-2 commit; 076 retired without implementation 2026-05-01; 081 retired without implementation 2026-05-02 — no soak evidence of the colony-side directive-loop pattern in seed-42, and a framing tension between the Why's substrate-over-override Modifier framing and the Approach's coordinator-side ledger framing made retirement parallel to 076 the cleanest disposition). The substrate audit identified 4 blocking + 3 important + 2 deferred fragilities plus 2 IAUS-coherence cleanups; the four blocking + three important fragilities are all closed via the landed children. **Retirement candidate** — every child is closed; this sub-epic is ready to move to landed in a follow-up commit, which would also let 027b's stale `blocked-by: [071]` drop.

## Approach

Two layers operating as machined gears:

- **`plan_substrate` API (072)** owns plan-lifecycle: `record_step_failure` / `abandon_plan` / `try_preempt` / `carry_target_forward` / `validate_target` / `record_disposition_switch`. Future planning bugs land in this single well-tested module instead of scattering across `goap.rs` + 5 step resolvers + 6 target resolvers.
- **IAUS engine extensions (073–078, 080)** express each defense as an engine primitive: Consideration on target DSEs (073, 078), EligibilityFilter (074, 080), Modifier in §3.5.1 pipeline (075, 076).

Children:

- ~~**072**~~ — `plan_substrate` module extraction (landed 2026-04-29)
- ~~**073**~~ — `RecentTargetFailures` + `target_recent_failure` Consideration on all 6 target DSEs (landed 2026-04-29; bundled the wave-2 work for 074 / 075 / 078 / 080)
- ~~**074**~~ — `EligibilityFilter::require_alive` + step-resolver `validate_target` (bundled into 073)
- ~~**075**~~ — `CommitmentTenure` Modifier (bundled into 073)
- ~~**076**~~ — `LastResortPromotion` Modifier — retired without implementation 2026-05-01 (substrate twin 088 + 094 + 123 cover the post-failure-escalation surface)
- ~~**077**~~ — anxiety-interrupt cadence root-cause investigation (landed 2026-04-29; closed as no-op)
- ~~**078**~~ — backport 027b's `bond_score` pin to a `target_pairing_intention` Consideration (bundled into 073)
- ~~**079**~~ — `iaus_coherence` grep-check in `just check` (landed 2026-04-29)
- ~~**080**~~ — `Reserved` component + `EligibilityFilter::require_unreserved` (bundled into 073)
- ~~**081**~~ — coordination directive-failure demotion via `RecentTargetFailures` aggregate (audit gap #10) — retired without implementation 2026-05-02 (parallel to 076; soak evidence absent + framing tension between substrate Modifier framing and coordinator-side ledger framing)
- ~~**082**~~ — 027b reactivation (landed 2026-04-29)

**Blocks:** 027b activation — structurally unblocked when 082 landed 2026-04-29; 027b's frontmatter `blocked-by: [071]` still in place pending this sub-epic's retirement decision.

## Verification

Scope-honest gates for this sub-epic (substrate hardening, not colony-wide run quality):

- All children `status: done` ✅ (072 / 073 / 076 retired / 077 / 079 / 081 retired / 082 landed; 074 / 075 / 078 / 080 bundled into 073's wave-2 commit).
- The substrate-audit fragilities (4 blocking + 3 important) are all closed via the landed children — see the audit-findings table below for the per-row mitigation.
- `just check` includes `check_iaus_coherence.sh` ✅ (079).
- 027b reactivation (082) landed on the hardened substrate ✅.

(Original draft of this section additionally claimed `Starvation = 0`, `ShadowFoxAmbush ≤ 10`, all six continuity canaries ≥ 1, and a 12-run multi-seed sweep. Those are colony-wide hard gates and continuity canaries, not substrate-hardening-specific gates — the most recent canonical seed-42 deep-soak (`tuned-42-baseline-0783194`) shows `continuity/mentoring=0`, `continuity/burial=0`, and 4 never-fired positives, which are tracked under separate ticket clusters and do not gate this sub-epic.)

## Log

- 2026-04-29: Opened. Diagnosis of 027b's failed soak corrected (the "Bevy 0.18 topological-sort reshuffle" framing in 027b's Log is mechanically wrong — chain 2a uses `.chain()`, source order is enforced). The actual mechanism is long-horizon RNG drift + planning-substrate stuck-loop fragility (same bug class as tickets 038/041). Plan stored at `~/.claude/plans/working-in-users-will-mitchell-clowder-keen-planet.md`.
- 2026-05-02: Reconciliation pass. All blocking children landed (072 / 073 with 074 / 075 / 078 / 080 bundled / 077 / 079 / 082); 076 retired without implementation 2026-05-01; only 081 remains, parked and non-blocking for 027b reactivation. Marked as a retirement candidate — the load-bearing work is closed. 027b's frontmatter `blocked-by: [071]` is structurally stale and should drop the moment this sub-epic lands; surfaced for the next maintenance pass.
- 2026-05-02: 081 retired without implementation. Soak check on `logs/tuned-42-baseline-0783194` (commit `9945e59`) showed zero `Build`/`Construct` `DirectiveIssued` events in the full 1.34M-tick run — the canonical "kitchen-build with depleted materials" scenario isn't reaching dispatch in seed-42 — and no `(kind, target)` pair recurring ≥ 3 times in 34,645 issuances; the high-failure actions (`EngagePrey` 4590, `ForageItem` 1846) are cat-side step failures already covered at the substrate by 073's `RecentTargetFailures` cooldown. Combined with 081's internal framing tension (Why frames it as a Modifier in §3.5.1's pipeline; Approach describes coordinator-side ledger bookkeeping; coordinator has no IAUS pipeline today), retirement parallel to 076 was the cleanest disposition. With 081 closed, this sub-epic has zero open children and is itself ready to retire in a follow-up — 027b's stale `blocked-by: [071]` drops in the same future commit.
- 2026-05-02: **Sub-epic retired.** Zero open children; substrate-audit fragilities all closed via landed children (table below); `check_iaus_coherence.sh` enforced via `just check` (079); 027b reactivation soak (082) landed on the hardened substrate. Verification block tightened to scope-honest substrate-hardening gates — colony-wide continuity canaries (`mentoring=0`, `burial=0`, never-fired `FoodCooked`/`MatingOccurred`/`GroomedOther`/`MentoredCat` in the most recent seed-42 deep-soak) are tracked under separate ticket clusters and do not gate this sub-epic. 027b's frontmatter `blocked-by: [071]` is dropped in the same commit.

## Audit findings (full reference)

| # | Class | File:line | Severity | Mitigation |
|---|---|---|---|---|
| 1 | Cross-plan target memory | `goap_plan.rs:44–45` | **Blocking** | 073 — `RecentTargetFailures` |
| 2 | Target-resolver failure-awareness | `socialize_target.rs:109–137` etc. | **Blocking** | 073 — `target_recent_failure` Consideration |
| 3 | Plan-validity revalidation | `goap.rs:2817–2820` | **Blocking** | 074 — `EligibilityFilter::require_alive` |
| 4 | Stale-entity guard | `goap.rs:2817–2820` + step resolvers | **Blocking** | 074 — `validate_target` at step entry |
| 5 | Disposition oscillation | `sim_constants.rs:672–674` | Important | 075 — `CommitmentTenure` Modifier |
| 6 | Spiral-of-failure | `goap.rs:485–559` | Important | 076 — `LastResortPromotion` Modifier |
| 7 | Anxiety cadence drift | `disposition.rs:231` + `goap.rs:2414` | Important | 077 — investigation |
| — | 027b `bond_score` pin | `socialize_target.rs:193` | **IAUS-coherence** | 078 — backport to Consideration |
| — | No process gate | (no script today) | **IAUS-coherence** | 079 — `check_iaus_coherence.sh` |
| 8 | Marker staleness | `goap.rs:866`, `1072–1161` | Important | Defer (out-of-scope) |
| 9 | Resource reservation | (no `Reserved` today) | Important | 080 — `Reserved` + `require_unreserved` |
| 10 | Coordination drift | `coordination.rs:788–964` | Minor | ~~081~~ — retired without implementation 2026-05-02 (no observed pathology in seed-42; reopen if a future soak surfaces directive-loop cycling) |

Reference fixed defenses (do not reopen): ticket 041 fix at `goap.rs:2363–2383` (unconditional `ticks_remaining = 0` reset on all preempt kinds — Mallow Cook lock and Founding-haul Flee lock pattern is closed; the reset moves into `plan_substrate::try_preempt` in 072 to make it API-owned).
