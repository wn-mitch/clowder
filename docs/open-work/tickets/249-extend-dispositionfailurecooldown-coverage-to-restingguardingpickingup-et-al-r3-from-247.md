---
id: 249
title: Extend DispositionFailureCooldown coverage to Resting/Guarding/PickingUp et al. (R3 from 247)
status: parked
cluster: ai-substrate
initiative: []
orchestration: substrate-sensitive
added: 2026-05-08
parked: 2026-05-09
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

> **Status: closed without landing (2026-05-09).** This ticket was
> opened to close the H7 cooldown-coverage gap from 247, then reframed
> mid-session as a TargetExistence-marker fix (author `ColonyHasStores`,
> gate `SleepDse.eligibility` on it). Verification soak revealed the
> reframed gate caused an **11× regression** in
> `acute_health_adrenaline_flee` modifier-preemption rate vs the
> post-247 baseline — undoing 230's substrate-aware preempt-rate
> reduction by starving the 047 modifier's Sleep-lift landing target.
> Code rolled back without landing. **Architectural understanding
> *did* land** in
> `docs/systems/ai-substrate-refactor.md` §4.3 + §3.5.5 and in
> `src/ai/modifier.rs::DispositionFailureCooldown` rustdoc — the
> factual / aspirational TargetExistence distinction, the
> modifier-vs-marker boundary, the typed-failure-proxy temporary-proxy
> framing, and the caveat about modifier-lift landing-target
> starvation. Three follow-on tickets spawned (see §Log).
> H7 remains `[verified-defect]` per 247's audit but **not load-bearing**
> per 247's Phase D conclusion — the seed-42 cliff is gated by R4's
> `intention_preempt_strength_regime_boundary`, not by cooldown
> coverage.

## Why

247 promoted H7 to `[verified-defect]`: `DispositionFailureCooldown::signal_key`
in `src/ai/modifier.rs:2673-2686` covers Hunt, Forage, Cook,
HerbcraftGather, HerbcraftPrepare, HerbcraftWard, MagicScry,
MagicDurableWard, MagicCleanse, MagicColonyCleanse, MagicHarvest,
MagicCommune, Caretake, Build, Mate, Mentor — but NOT Resting,
Guarding, PickingUp, Discarding, Trashing, Handing, Socializing,
Exploring, Mating, Burying, Grooming, Coordinating. After a
planning failure on an uncovered disposition, the cat re-elects
the same disposition immediately, slamming the planner. 247's
collapsed run footer concentrated `Resting:GoalUnreachable=1172`
and `Guarding:GoalUnreachable=526` in the uncovered set; the cliff
was driven by a no-Stores cascade but the cooldown gap let the
cascade slam the planner without back-pressure.

R4 (247) resolved the trigger-3 churn that triggered the cascade,
so the cooldown gap is no longer load-bearing in the seed-42 soak.
247's Phase D log explicitly declined to open R3 as a defensive
fix. This ticket was opened anyway; the in-session audit then
showed the original framing was the wrong shape (extending a §3.5
post-scoring modifier outside the §12.3 belief-proxy architecture)
and reframed toward TargetExistence-marker authoring. The reframed
fix proved structurally too aggressive (see §Investigation).

## What we learned that DID land

Architectural insights confirmed and documented (independent of the
gate that was rolled back):

1. **`DispositionFailureCooldown` sits outside §12.3's belief-proxy
   architecture.** It is a §3.5 post-scoring modifier; the cat's
   *belief* that a disposition is plannable lives in §4.3
   TargetExistence markers + §12.3 proxies (`achievement_believed`,
   `achievable_believed`, `still_goal`), not in this modifier's
   per-disposition timer. Documented in `src/ai/modifier.rs`
   rustdoc + spec §3.5.5.
2. **TargetExistence markers come in two flavors: factual
   (percept-attenuated, sensing-derived) and aspirational
   (colony-scoped, categorical).** The factual / aspirational
   distinction is now named explicitly in spec §4.3 with the marker
   list under each flavor.
3. **Aspirational markers are categorical, not resource-state-aware.**
   "Does the colony have Stores?" (yes / no) vs "does that specific
   Stores have free capacity right now?" — the latter is per-resource
   belief revision, deferred to Talk-of-the-Town (§12.4 / cluster
   C3). The cooldown's surviving role is the band-aid for that
   categorical-vs-resource-state gap until ToT lands.
4. **`RecentDispositionFailures` is a temporary memory proxy.** §12.1
   names that the substrate has no general memory→scoring coupling
   today; the typed-failure-flavor components in tree
   (`RecentDispositionFailures`, `RecentTargetFailures`,
   `HuntingPriors::record_failed_search`, plus `RecentAmbushMap`
   proposed in 219) are one-off proxies that consolidate under
   ToT's unified `Memory` consumer at C3. New failure-flavors
   should not be added in this shape — landed as the C3
   "Typed-failure-proxy consolidation candidates" list in
   `tickets/007-cluster-c-deliberation-layer.md`.
5. **DSE-eligibility gates can starve §3.5 modifier-lift landing
   targets.** When a candidate DSE is the in-pool partner for a
   score-lift modifier (Sleep is the in-pool partner for
   `AcuteHealthAdrenalineFlee` because Flee is filtered from the
   disposition softmax), an eligibility filter at the DSE layer
   prevents the modifier from delivering its lift to a usable
   landing target. The cliff fix in such cases belongs at the
   plan-template / zone-resolution layer, not at DSE eligibility.
   Documented as the "Caveat from ticket 249's failed Sleep gate"
   in spec §4.3.

## Investigation — why the reframed fix failed

Verification soak (`logs/tuned-42` at commit `b0ecb0e6`, dirty)
vs post-247 baseline (`logs/tuned-42-post-247-0ee0e0cf`, commit
`0ee0e0cf`). Hard gates pass (deaths_starvation = 0,
deaths_ShadowFoxAmbush = 0, footer line written,
never_fired_expected_positives = []), all six continuity canaries
hold (grooming = 964, play = 5, mentoring = 233, courtship = 1644,
mythic-texture = 24, burial = 0 (post-250 demoted)). Welfare even
improved (+12% at 0.598 vs 0.534 baseline).

The verdict failed on three correlated metric drifts, all driven
by the same mechanism:

| Metric | Baseline | Current | Delta |
|---|---|---|---|
| `ModifierPreemption` cumulative | 4,228 | 32,902 | **+678%** |
| `ModifierPreemption` rate (per 10kt) | ~347 | ~3,830 | **11×** |
| `IntentionAdopted` per-tick | 0.34 | 0.70 | 2× |
| `IntentionFulfilled` cumulative | 15,797 | 10,000 | -37% |
| `deaths_injury` | 0 | 1 (Cedar — WildlifeCombat) | new |
| `duration_ticks` (15-min wall-clock) | 122,013 | 85,922 | -29.6% |

Historical context made the regression unambiguous:

| Run | mp / 10kt | Era |
|---|---|---|
| pre-230 | 3,920 | What 230 was meant to retire |
| pre-232 | 1,613 | 230's first reduction |
| pre-246 | 662 | Further reductions |
| post-247 baseline | 347 | Stable healthy state |
| **post-249 (rolled back)** | **3,830** | **Back to pre-230 levels** |

**Mechanism.** `AcuteHealthAdrenalineFlee` (047) is a §3.5
modifier that lifts BOTH Flee AND Sleep when
`health_deficit ≥ acute_health_adrenaline_threshold` (default 0.4).
Per the 047 rustdoc: *"Flee is filtered from the disposition softmax
... Sleep is the in-pool partner ... The Sleep lift is what flips
the disposition contest away from Guarding/Crafting under injury —
Sleep routes to a den, mechanically expressing retreat."* The
substrate-correct injury-recovery path is **modifier → Sleep → den
travel → recover → modifier stops firing**.

By gating Sleep at the DSE-eligibility layer when `ColonyHasStores`
is absent (cold-start window before the first Stores building
finishes, ~tick 1,201,600), the gate starved the modifier's lift in
that window. Cats with low HP couldn't recover via the substrate
path; the modifier kept firing per-tick (the exact pattern 230
retired); seed-42 trajectories diverged from baseline; the diverged
trajectories had cats farther from den-routes when Stores eventually
built, prolonging the low-HP state into mid- and late-game; one cat
(Cedar) died from WildlifeCombat as a downstream cascade.

**The fix shape was wrong at the layer.** The 247 cliff fires
because the *Resting plan template's* `ZoneIs(RestingSpot)`
precondition can't resolve when `stores_positions` is empty (per
`goap.rs:7766-7771`). That's a search-state failure
(per-replan-attempt, the planner discovers it) — and §4.7
substrate-vs-search-state classifier says search-state failures are
the planner's job, not the L1 marker layer's. Gating at the DSE
eligibility layer (L1) prevents Sleep from scoring at all, which
short-circuits the modifier-lift cascade at the wrong layer.

The substrate-correct fix shape is one of: (a) make `RestingSpot`
zone resolution honest about per-cat memory-based sleep spots, so
zone resolves even without Stores when memory has a positive-weight
spot; (b) trust §12.3 channel (b) — when `replan_count` cap fires
on Resting because RestingSpot is unresolvable, §7.2 drops the
intention; the residual cliff (immediate re-election) is exactly
what `DispositionFailureCooldown` was *originally* designed for, but
extending its match arms grows typed-failure surface area C3 will
have to retire. None of these are 249's scope; they spawn as
follow-ons.

## Out of scope (now that the gate is closed)

- **Authoring the Sleep gate.** Rolled back. The mechanism named
  above (DSE-eligibility-vs-modifier-lift starvation) is the
  reason; the structural alternatives are tracked as follow-ons.
- **Talk-of-the-Town belief revision.** §12.4 / C3 (ticket 007).
- **Renaming `DispositionFailureCooldown`.** The current name is
  accurate.
- **Extending the cooldown's `signal_key` match arms.** Rejected
  per the original reframe; this is the typed-failure-proxy
  surface-area expansion C3 will consolidate.

## Verification — what we did

1. Authored `ColonyHasStores` aspirational TargetExistence marker
   (`src/components/markers.rs`), authored from
   `update_colony_building_markers` (`src/systems/buildings.rs`),
   wired into `colony_state_query` in both `goap.rs` and
   `disposition.rs`, gated `SleepDse.eligibility` on it.
2. `cargo check` + `cargo clippy --all-targets -- -D warnings` clean.
3. `cargo test --lib` 2,037 passed (including new
   `sleep_eligibility_requires_colony_has_stores` regression test).
   `cargo test --tests` integration tests passed.
4. `just check` clean (step-resolver, time-units, iaus-coherence,
   substrate-stubs, items-are-real, InfluenceMap registry).
5. `just soak-trace 42 Mallow` — soak ran ~17 min wall-clock,
   reached tick 1,285,922 (85k sim ticks vs ~122k baseline; the
   29.6% wall-clock deficit reflects concurrent compute load on the
   user's box, not a sim slowdown by itself).
6. `just verdict logs/tuned-42 --baseline logs/tuned-42-post-247-0ee0e0cf/events.jsonl`
   reported `band: fail` driven by the three deltas above.
7. Historical comparison across `logs/tuned-42-*` runs surfaced the
   pre-230 → post-247 → post-249 progression that named the
   regression unambiguously.

## Related work

<!-- linkages:start -->
<!-- generated by `just similar-link-report` on 2026-05-17 — review and prune; pairs above threshold that aren't already cross-referenced. -->

- ✓ landed **152** (done, ai-substrate, score 0.92) — Tier-1 disposition-collapse audit — sweep for sibling Eat-into-Resting defects
- ✓ landed **189** (done, ai-substrate, score 0.92) — Post-178 food_available regression — layer-walk diagnosis
- ✓ landed ** 73** (done, ai-substrate, score 0.92) — Wave 2 substrate hardening

<!-- linkages:end -->
## Log

- 2026-05-08: opened from 247's §Out of scope. H7 row promoted in
  247 via code-side queries.
- 2026-05-09: **REFRAMED.** In-session audit (with the user)
  surfaced that the original "extend cooldown match arms" framing
  did an end-around on §12.3's belief-proxy architecture.
  Substrate-correct slot is §4.3 TargetExistence markers; the
  Sleep gap was the actual cliff trigger 247 documented.
- 2026-05-09: **IMPLEMENTED then ROLLED BACK.** Authored
  `ColonyHasStores` + gate; verification soak surfaced the 11× modifier-
  preempt regression diagnosed above. Code rolled back. The
  architectural understanding documented in spec §3.5.5 / §4.3 +
  modifier rustdoc *did* land. Three follow-on tickets opened:
  - **AcuteHealthAdrenalineFlee retirement (substrate-over-mod).**
    The 047 modifier's preempt-rate fragility under BDI commitment
    is itself the underlying issue; substrate-correct injury-urgency
    should live in the Sleep DSE's own axes (`injury_rest`,
    `pain_level`) carrying the lift, not in a post-scoring modifier.
  - **Fleeing disposition (230) adoption audit — why dead in seed-42 healthy.**
    `FleeTargetPicked` cumulative = 0 in every seed-42 healthy soak
    in `logs/tuned-42-*`. The substrate-aware Fleeing path 230
    designed never adopts; understand whether that's intended (the
    047 doc says Sleep is the in-pool partner because Flee is
    filtered from disposition softmax) or a regression.
  - **`RestingSpot` zone resolution as proper belief proxy.**
    Currently `goap.rs:7766-7771` resolves `RestingSpot` through
    `stores_positions.iter().min_by_key(...).map(...)` — yields
    `None` when no Stores exist, regardless of per-cat memory.
    Substrate-correct: fall through to `OwnSleepingSpot` /
    `OwnSafeRestSpot` from `interoception::own_safe_rest_spot` so
    cats with memory-based sleep spots can sleep without Stores.
    This is the cliff fix at the right layer (zone resolution, not
    DSE eligibility).
