---
id: 394
title: rear_kitten arc Wean-failure churn — frame always created at sub_goal 0
status: done
cluster: ai-substrate
initiative: [smarter-cats, htn-method-composition]
orchestration: substrate-sensitive
added: 2026-05-16
parked: 2026-05-17
blocked-by: []
supersedes: []
related-systems: [htn-methods.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-17
---

## Why

After 364 landed the rear_kitten HTN dispatch closure, the seed-42
verification soak (`logs/tuned-42-attempt11`) introduced **2439
`"Wean: no dependent kitten in range/band"` plan failures** vs 0 in
the pre-364 baseline (`logs/tuned-42-d633bcc5`). Same archive,
ground-truthed via Phase A: footer shows 0 deaths, kittens_matured=1,
colony welfare/shelter actually IMPROVED — but Mocha's PlanStepFailed
jumped 780 → 2312 (+196%) and PlanReplanned jumped 24 → 1621
(+6655%). The substrate-cleanliness defect is real even though the
colony absorbs it (no health regression).

**Mechanism:** new frames are always created at `sub_goal_index = 0`
(Wean) — see `GoalFrame::new` at
`src/components/held_goal_stack.rs:100`. When the reactive emit
re-fires `kitten_reared` for a queen whose dependent kitten is past
the Wean band (maturity ≥ 0.33), the picker rejects every candidate
(no kitten matches the Wean band's filter), `dispatch_htn_kitten_primitive`
returns `Fail`, `htn_abandon_or_pop` pops the entire frame, and the
NEXT tick's reactive emit re-creates a frame at sub_goal_index=0
again. The loop only exits when KittenDependency is removed (which
requires Release to fire, which we never reach because we never get
past Wean).

The prior session's "starvation regression" framing was **not
grounded in the logs** — there is no Pebblekit-67 starvation, no
deaths at all, and Mocha's Caretake count is IDENTICAL between the
failing run (5) and the baseline (5). The actual signal is plan
churn, not Caretake displacement.

## Verified context (Phase A — existing archives, no new soak)

- **Failing run:** `logs/tuned-42-attempt11` (commit `b2ec4c9e`,
  ended tick 1325196, 125196 ticks). Verdict: CONCERN (constants
  drift + mythic-texture=0; survival passes; continuity fails only on
  burial=0 and mythic-texture=0, both expected-rare per ticket 250
  demotion).
- **Baseline:** `logs/tuned-42-d633bcc5` (commit `d633bcc5`, 127919
  ticks). Verdict: CONCERN (same continuity failures — not 364
  specific).
- **deaths_by_cause: 0** in BOTH runs. No starvation, no
  ShadowFoxAmbush.
- **Mocha's action distribution (failing vs baseline):** Cook 351 vs
  549 (-36%, displaced by Wean activity); GroomOther 209 vs 204
  (=); Hunt 56 vs 61 (=); **Caretake 5 vs 5 (=)**; **Wean 184 vs 0**
  (new — entirely 364-introduced).
- **Mocha's plan-churn signature (failing vs baseline):** PlanCreated
  9251 vs 8992 (+3%); PlanStepFailed 2312 vs 780 (+196%);
  **PlanReplanned 1621 vs 24 (+6655%)**.
- **plan_failures_by_reason (failing run, top 5):**
  - EngagePrey: lost prey during approach = 2887 (was 3146 in
    baseline; -8%, normal hunting churn)
  - **Wean: no dependent kitten in range/band = 2439 (NEW vs 0)**
  - TravelTo(HerbPatch): no path and stuck = 1595 (was 1738; -8%)
  - HandoffItem: no recipient on disposition = 717 (was 538; +33%)
  - ForageItem: nothing found = 627 (was 707; -11%)
- **colony_score delta (failing vs baseline):** aggregate 2609 vs
  2732 (-4.5%, within band); **welfare 0.60 vs 0.53 (+12%); shelter
  0.40 vs 0.17 (+140%); nourishment 0.68 vs 0.64 (+6%); fulfillment
  0.31 vs 0.28 (+10%)** — colony health IMPROVED. kittens_born 2 vs
  4 (-50%, within seed variance per registered baseline 095-phase-1a
  which also has 2). kittens_matured 1 vs 0 (NEW — substrate
  end-to-end works).
- **substrate Features fire:** KittenWeaned, SkillTaught,
  KittenReleased, SubGoalAdvanced all > 0 in the failing run
  (substrate end-to-end works for at least one kitten).

## Current architecture (layer-walk audit — verified)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | `src/components/markers.rs::Parent` | Written by `update_parent_markers` when any kitten carries `KittenDependency.mother == self`. Always true while queen has ≥1 dependent kitten. | `[verified-correct]` |
| Reactive emit predicate | `src/ai/methods/rear_kitten.rs:63-66 has_dependent_kitten` | Gates on Parent marker + alive only — no maturity-band check. So the emit fires every tick the queen has any dependent kitten, regardless of whether sub_goal 0 (Wean) is achievable. | `[verified-correct]` (intended design — the bug is downstream) |
| Frame creation | `src/components/held_goal_stack.rs:100 GoalFrame::new` | Hardcodes `sub_goal_index: 0`. Every new frame starts at Wean. | `[verified-defect-mechanism]` — the load-bearing defect |
| Adopt-hook gate | `src/systems/goap.rs::evaluate_and_plan ~line 2426` | Pins `chosen_action` to leaf primitive when frame is multi-step. Works correctly. | `[verified-correct]` |
| Dispatch arm | `src/systems/goap.rs:7163 dispatch_htn_kitten_primitive` | When picker returns None, returns `Fail` — does NOT differentiate "kitten in other band" vs "kitten gone". Triggers `htn_abandon_or_pop` which pops the whole frame. | `[verified-defect-handler]` |
| Picker | `src/ai/dses/dependent_kitten_target.rs::resolve_dependent_kitten_target` | Filters by `mother == self`, `maturity_in_band(action)`, and range ≤ 12. Returns None when no match. | `[verified-correct]` |
| Plan template | `src/ai/planner/actions.rs::htn_primitive_actions` | `[TravelTo(SocialTarget), Wean]` etc. Works correctly. | `[verified-correct]` |
| Resolver | `src/steps/disposition/{wean,teach,release}.rs` | Wean returns `unwitnessed(Advance)` when maturity already past — correct idempotent handling. Resolvers work. | `[verified-correct]` |
| Caretake-eligibility for hungry kittens | `src/ai/dses/caretake_target.rs` | Kinship-floor (0.6) + bloodline-override are intact. Mocha's Caretake count of 5 is the same in baseline (5) → no displacement effect. | `[verified-correct] — no displacement observed` |

The **two `[verified-defect-*]` rows together describe the churn
mechanism**: frame creation always starts at Wean, the dispatch's
Fail-on-no-target conflates "wrong band" with "kitten gone", and
together they form the loop.

## Fix candidates

**Structural options** (drafted per bugfix discipline):

- R5 (**extend** plan template) — prepend `FeedKitten` to each leaf
  plan. Doesn't address the churn; was sized for the (now-debunked)
  starvation framing. Multiplies plan-failure surface (5 steps vs 2)
  — RULED OUT.
- R9 (**extend** reactive predicate to check Wean band) — would
  break the substrate: predicate flips false after first Wean
  succeeds, so Teach / Release never fire. RULED OUT.
- R10 (**extend** frame creation to compute initial sub_goal_index
  from kitten maturity) — requires plumbing method-specific kitten
  state to the generic frame-creation site at `goap.rs:2840`. Larger
  blast radius (every method's frame creation needs to thread the
  same hook). Viable but heavyweight.
- **R11 (extend dispatch — Advance on band-mismatch vs Fail on
  kitten-gone) — RECOMMENDED.** When `dispatch_htn_kitten_primitive`'s
  picker returns None, check if the queen has any dependent kitten at
  all:
  - **Yes:** kitten exists but past this sub_goal's band → return
    `Advance` (no witness, no Feature emission). The advance hook
    bumps `sub_goal_index`. Next tick's dispatch resolves against
    the kitten's current band.
  - **No:** kitten despawned / orphaned → return `Fail` (real
    backtrack/abandon via `MethodFailure`).

R11 mirrors the same idiom the Wean resolver already uses internally
(`unwitnessed(Advance)` when maturity past — `wean.rs:36-41`); it
lifts that pattern up to the picker level so the band-mismatch case
is handled before the resolver runs.

**Parameter-level options** (no longer needed):

- R1 / R2 / R3 — threshold tuning, plan-cost shrink, cooldown — all
  premised on "arc is too aggressive". R11 fixes the mechanism; no
  parameter tuning required.

## Recommended direction

**R11.** One-line dispatch-side change: differentiate the two
picker-failure modes. Substrate-clean, localized, no new component or
method or DSE. Mirrors the existing resolver idempotency idiom.

R10 (frame init from kitten band) is the alternative if Phase D
verification shows R11 alone doesn't drive the Wean failure count to
~0 — that would mean there's a second mechanism (e.g.,
out-of-range-at-dispatch-time) that R10 would also need to address.

## Out of scope (open as follow-ons after R11 lands)

- **Plan-failure canary in `verdict`.** Add a verdict canary that
  fails when any `plan_failures_by_reason` key has rate ≥ 10× the
  baseline OR is new vs baseline with rate above some threshold. The
  Wean churn (0 → 2439, ~0.02/tick, new key) would have failed this
  canary. Tracked as a separate ticket after 394 lands (user-flagged
  during Phase A: "we should add those as failing canaries if it's
  logarithmically higher rate wise").
- Promoting `KittenWeaned` / `SkillTaught` / `KittenReleased` /
  `SubGoalAdvanced` to `expected_to_fire_per_soak() => true`.
  Deferred to Phase D of this ticket — flip after R11's verification
  soak confirms they still fire.
- Tuning the maturity threshold defaults (0.33 / 0.66 / 1.0).
  Defer until R11 + canary land; threshold tuning is a balance-doc
  iteration, not a substrate question.

## Verification

1. `cargo check --release` + `just check` + `cargo test --release`
   all pass.
2. Fresh `just soak-trace 42 Mocha` (or `42 Pebblekit-67` — the
   per-227 multi-focal convention picks a marker-gated cat) writing
   to a non-tuned-* path per `feedback_soak_trace_path_collision.md`.
3. `just verdict <new-run-dir>` returns pass; survival hard gates
   intact; continuity unchanged (burial=0 / mythic-texture=0 are
   pre-existing and not R11's responsibility).
4. **Wean failure count drops to ~0** in `plan_failures_by_reason`.
   This is the primary fix-verification metric.
5. **PlanReplanned for Mocha** drops back near baseline (~24, not
   ~1621). Secondary metric.
6. **Mocha's Wean action count** drops below 184 (less arc time
   wasted on adopt-fail-pop cycles). Tertiary.
7. After verification, flip the four Features to `true` in
   `src/resources/system_activation.rs::expected_to_fire_per_soak()`:
   `KittenWeaned`, `SkillTaught`, `KittenReleased`,
   `SubGoalAdvanced`. (`MethodBacktracked` stays `false` — no
   sibling method for `kitten_reared` yet.)
8. `just land 394 --commit "fix: 394 — R11 dispatch advances on
   band-mismatch" --log "Phase A debunked starvation framing;
   landed R11 + 4 Feature promotions"`.

## Log

- 2026-05-16: opened, blocked on 364. Original hot context preserved
  for one revision; superseded by Phase A findings.
- 2026-05-16: **Phase A reframe.** Existing archives queried (no new
  soak per user direction). Hot context's starvation claim is
  ungrounded — both runs have 0 deaths. Actual signal: 2439 Wean
  plan failures (new vs 0 in baseline); Mocha's PlanStepFailed
  +196%, PlanReplanned +6655%. Mocha's Caretake count is identical
  in both runs (5 → 5). Mechanism verified via code-read:
  `GoalFrame::new` always sets `sub_goal_index = 0`; `dispatch_htn_kitten_primitive`
  conflates "wrong band" vs "kitten gone" on picker None. R5/R9
  ruled out. R11 (dispatch case-split) recommended. Audit table
  rewritten with verified rows.
- 2026-05-16: R11 implemented at `src/systems/goap.rs:7163`. Cargo
  check + test suite pass. Verification soak (`logs/tuned-42-394-r11`)
  results: **Wean failures dropped 2439 → 9 (R11 works as
  designed)**, but **2 starvations appeared** (Pebblekit-67 died
  tick 1280417, Pebblekit-34 died tick 1312071). Mechanism: R11
  enables the arc to complete fast (Wean→Teach→Release within ~600
  ticks of birth); `resolve_release`'s `KittenDependency` removal
  fires at maturity 0.66 (the Release band threshold), but the
  kitten's natural maturity is still well below 1.0 — they fall out
  of the Caretake target pool but aren't physiologically capable of
  self-feeding. R11 reverted at `goap.rs:7163`. Substrate-stability
  pillar treats `Starvation > 0` as non-negotiable.
- 2026-05-16: **Parked, blocked on 395.** Opened
  `docs/open-work/tickets/395-rear-kitten-arc-decouple-release-from-kittendependency-removal.md`
  as the comprehensive fix (R11 + R13: dispatch case-split +
  Release's KittenDependency removal gated on natural maturity 1.0).
  394 carries the Phase A diagnostic record; 395 carries the fix.
- 2026-05-17: **Re-parked, blocked on 398** (`§7.M.2
  RaiseOffspringAspiration — kitten-rearing as nested-Intention
  aspiration`). Session dissection identified the 364 HTN
  frame-pin + wrap-site override as the wrong commitment layer
  per CLAUDE.md design pillar #4 (added 2026-05-17 — *"commitment
  is one mechanism, not two"*). 394's R11 dispatch-case-split is
  a patch on the override and retires with the override; 394's
  Phase A diagnostic record remains the source-of-truth for the
  Wean-churn mechanism. Plan at
  `/Users/will.mitchell/.claude/plans/let-s-start-tickets-394-397-snazzy-kahan.md`
  (local).
- 2026-05-17: Superseded by 398 — RaiseOffspringAspiration's L1 AspirationLift (Mother + Parent marker → Caretake +0.2) provides reliable Caretake score elevation across the full kitten-dependency window, addressing the queen-attention-diverted failure mode at the substrate layer rather than via per-tick lift compensations.
