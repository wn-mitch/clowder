---
id: 163
title: Migrate the 9 apply_*_bonus passes into §3.5.1 modifiers (full-batch)
status: ready
cluster: ai-substrate
added: 2026-05-04
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

`src/systems/goap.rs::evaluate_and_plan` (lines 1535–1604) runs **nine
post-`score_actions` passes** that mutate the per-Action `scores` Vec
*after* the L2 trace record is captured by `evaluate_single_with_trace`
and *before* the §L2.10.6 softmax reads it. The §3.5.1 catalog
(`docs/systems/ai-substrate-refactor.md` §3.5) classifies modifiers as
post-composition transforms registered in `default_modifier_pipeline`;
these nine are textbook §3.5.1 candidates that never got migrated.
They are the last surviving pre-substrate "imperative pass over the
score list" pattern in the cat-side AI loop.

This is the **§4.7 substrate-vs-search-state violation** named in the
refactor spec — the 9 functions read substrate state (`Memory`,
`ColonyKnowledge`, `Aspirations`, `Preferences`, `FatedLove` /
`FatedRival`, `RecentDispositionFailures`, `ColonyPriority`,
`ActiveDirective`, neighbor-cat actions) but apply the result through
legacy per-Action mutation rather than through the registered §3.5.1
modifier pipeline that already runs inside `evaluate_single_with_trace`
and is captured in the L2 trace's `modifier_deltas` field. Every other
post-composition transform (Pride, Independence solo/group, Patience,
Tradition, Fox/Corruption suppression, StockpileSatiation,
BodyDistressPromotion, the eight adrenaline / pressure / kitten-cry
modifiers) lands as a registered `ScoreModifier`; these nine are the
regression. CLAUDE.md "Antipattern migration follow-ups are
non-optional" — they should have been opened as a follow-on at ticket
014's land (`docs/open-work/landed/014-phase-4-follow-ons-closeout.md`,
2026-04-27). The 014 closeout names "§3.5 modifier pipeline" as
shipped but never enumerated the nine pre-existing imperative passes
as out-of-scope. This ticket is that follow-on, opened retroactively.

The user has rejected the earlier-drafted "extend the L2 trace with a
post_bonus_deltas slot, migrate later" sequencing. The trace surface
that has to go is the bonus pipeline itself — extending the trace
would just dignify the antipattern. **Full-batch migration in one
PR.**

## Soak verification (collected 2026-05-04, seed 42, focal Simba)

A 4-min seed-42 soak (`logs/163-l2-pool-verification/`) plus the
ticket-162 `kitten_cry_basic` scenario both reproduce the L2-vs-pool
divergence the §11.3 trace contract should make impossible:

| Tick | DSE | L2 final | Pool entry | Δ | Likely contributor |
|---|---|---|---|---|---|
| 1200003 | hunt | 0.606 | 0.907 | +0.30 | memory(ResourceFound) + cascading |
| 1200003 | forage | 0.636 | 0.958 | +0.32 | memory(ResourceFound) + cascading |
| 1212627 | hunt | 0.048 | 0.359 | +0.31 | memory(ResourceFound) ×7.5 |
| 1244383 | patrol | 0.105 | 0.470 | +0.37 | aspiration / cascading |
| 1244383 | cook | 0.556 | 0.208 | -0.35 | cooldown / no-kitchen-priority |
| (kitten_cry_basic) | caretake | 1.087 | 0.105 | -0.98 | unknown — re-investigate post-migration |

The substrate is wired — `score_actions` (`src/ai/scoring.rs:974`)
calls `score_dse_by_id` (line 819) which routes the focal cat through
`evaluate_single_with_trace` (`src/ai/eval.rs:508`); the same
`final_score` populates both the L2 capture and the per-Action
`scores` Vec the softmax reads. The divergence is exclusively the
work of the nine `apply_*` passes between scoring and softmax.

The kitten_cry_basic Caretake `1.087 → 0.105` collapse (~90%
reduction) is the most striking case. With substrate confirmed wired,
this is a bonus-layer regression — most likely
`apply_disposition_failure_cooldown` (the only multiplicative damp in
the chain — 0.1× at fresh failure could explain the magnitude) or a
combination of negative-delta layers. **This case will be
re-investigated post-migration with the trace truthful**; it is NOT
the migration's acceptance criterion.

## Migration plan

Each function in `goap.rs:1535–1604` ports to one or more registered
`ScoreModifier`s in `src/ai/modifier.rs` and is then deleted from its
current call site. The replacement modifiers register in
`default_modifier_pipeline` (`modifier.rs:~2159`) at positions
matching the legacy chain's intent (cooldown damp first as the legacy
chain runs it; additive bonuses next; multiplicative damps last per
the established convention).

| # | Legacy function | Source | Reads | Becomes (modifier name) | Trigger | Transform | Per-DSE list |
|---|---|---|---|---|---|---|---|
| 1 | `apply_disposition_failure_cooldown` | `src/systems/plan_substrate/sensors.rs` | `RecentDispositionFailures` component, current tick, `disposition_failure_cooldown_ticks` | `DispositionFailureCooldown` | per-disposition scalar `disposition_recent_failure_signal_<kind>` < 1.0 for the DSE's parent disposition | Multiplicative: `score *= signal`, mirroring existing `cooldown_curve()` exactly | Every DSE whose `DispositionKind::from_action(action)` falls in the failure-prone whitelist (Hunting, Foraging, Crafting, Caretaking, Building, Mating, Mentoring) — same set `is_failure_prone_disposition` already covers |
| 2 | `apply_memory_bonuses` (ResourceFound branch) | `src/ai/scoring.rs:1351` | `Memory.events`, cat position | `MemoryResourceFoundLift` | `memory_resource_found_proximity_sum > 0` (pre-aggregated proximity-weighted Σ over `events.iter().filter(ResourceFound).filter(within memory_nearby_radius)`) | Additive: `score += scalar × memory_resource_bonus` | hunt, forage |
| 3 | `apply_memory_bonuses` (Death branch) | same | same | `MemoryDeathPenalty` | `memory_death_proximity_sum > 0` | Subtractive: `score -= scalar × memory_death_penalty` | wander, idle |
| 4 | `apply_memory_bonuses` (ThreatSeen branch) | same | same | `MemoryThreatSeenSuppress` | `memory_threat_seen_proximity_sum > 0` | Subtractive: `score -= scalar × memory_threat_penalty` | wander, explore, hunt |
| 5 | `apply_colony_knowledge_bonuses` | `src/ai/scoring.rs` | `ColonyKnowledge.entries`, cat position | `ColonyKnowledgeLift` (one impl, two arms keyed by `entry.event_type`) | `colony_knowledge_resource_proximity > 0` ∨ `colony_knowledge_threat_proximity > 0` (two pre-aggregated scalars) | Additive: `score += scalar × colony_knowledge_bonus_scale` | resource arm: hunt, forage; threat arm: patrol |
| 6 | `apply_priority_bonus` | `src/ai/scoring.rs` | `ColonyPriority.active` | `ColonyPriorityLift` | `colony_priority_ordinal == <kind ordinal>` for the matching DSE set | Additive: `score += priority_bonus` | per-priority: Food→hunt+forage+farm; Defense→patrol+fight; Building→build; Exploration→explore |
| 7 | `apply_cascading_bonuses` | `src/ai/scoring.rs:1400` | per-cat `nearby_actions: HashMap<Action, usize>` (built from colony-wide `action_snapshot` scan in `goap.rs:~1042,1550–1557`) | `NeighborActionCascade` | `cascade_count_<action> > 0` (per-action scalar pre-published into `ctx_scalars` at `ScoringContext` build time, one f32 per Action variant ≠ Fight) | Additive: `score += scalar × cascading_bonus_per_cat` | every cat-DSE except `fight` (Fight has its own ally bonus inline; preserve exclusion) |
| 8 | `apply_aspiration_bonuses` | `src/ai/scoring.rs` | `Aspirations.active[i].domain.matching_actions()` | `AspirationLift` | `aspiration_action_<action> > 0` per-action scalar (count of active aspirations whose domain includes the action) | Additive: `score += count × aspiration_bonus` | dynamic per-cat — every action that appears in any active aspiration's domain |
| 9 | `apply_preference_bonuses` | `src/ai/scoring.rs` | `Preferences.get(action)` | `PreferenceLift` (Like arm) and `PreferencePenalty` (Dislike arm) — register two modifiers so the trace shows like-vs-dislike independently | `preference_for_<action> == 1.0` (Like) ∨ `preference_for_<action> == -1.0` (Dislike), per-action scalar | Like: `score += preference_like_bonus`; Dislike: `score -= preference_dislike_penalty` | dynamic per-cat — every action in the cat's `Preferences` map |
| 10 | `apply_fated_bonuses` (love arm) | `src/ai/scoring.rs` | `FatedLove` component + sensing visibility check at `goap.rs:1565–1577` | `FatedLoveLift` | `fated_love_visible == 1.0` scalar (already-computed at the call site) | Additive: `score += fated_love_social_bonus` | socialize, groom_other, mate |
| 11 | `apply_fated_bonuses` (rival arm) | same | `FatedRival` component + sensing check at `goap.rs:1578–1590` | `FatedRivalLift` | `fated_rival_nearby == 1.0` scalar | Additive: `score += fated_rival_competition_bonus` | hunt, patrol, fight, explore |
| 12 | `apply_directive_bonus` | `src/ai/scoring.rs:1423` (caller pre-computes magnitude at `goap.rs:1592–1605`) | `ActiveDirective` + `relationships.fondness` + `personality.{diligence,independence,stubbornness}` + `directive_*` constants | `ActiveDirectiveLift` | `active_directive_action_ordinal == <action ordinal>` ∧ `active_directive_bonus > 0` (pre-computed magnitude scalar; magnitude pre-multiplies all the personality/relationship factors caller-side as today) | Additive: `score += active_directive_bonus` | the single Action the directive targets |

### Cascading-bonus L1-map decision

Considered hoisting `nearby_actions` into a true L1
`NeighborActionMap` per §5 of the refactor spec. **Rejected** for
this ticket: the current behavior is a flat Manhattan-range scan
with no falloff curve, not a diffusion field; per-(tile × action)
storage is heavy for a count that's already O(cat-count) per cat
to compute; no other consumer exists. Pre-publishing 16 scalars
`cascade_count_<action>` into the existing `ctx_scalars` map
preserves semantics exactly. If a future ticket wants the L1-map
shape (e.g. for a falloff curve or for a non-cat reader), it carves
the lift independently.

### Cooldown-modifier scalar shape

The cooldown is the only multiplicative pass and the only one that
gates per-DSE on a *disposition* rather than per-action. **One
scalar per failure-prone disposition** —
`disposition_recent_failure_signal_hunting`, …, `_mentoring`. The
modifier switches on `DispositionKind::from_action(action)` to pick
which scalar to fetch. Mirrors the existing
`disposition_recent_failure_age_normalized` helper. Rejected
alternative: a single `recent_failure_signal_for_active_disposition`
scalar would make the trace less honest (no per-disposition signal
visible to the auditor).

## Layer-walk audit

| Layer | File / line | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | `src/components/markers.rs`, `src/systems/marker_authoring/*` | All §4.3 substrate is authored from observable state; mirror snapshot drives both eligibility and `StatePredicate::HasMarker` | `[verified-correct]` |
| L2 DSE composition | `src/ai/eval.rs:508` `evaluate_single_with_trace` | Per-DSE composition + Maslow pre-gate + registered §3.5.1 modifiers; output captured into `EvalTrace::modifier_deltas` per §11.3 | `[verified-correct]` |
| L2 → score-list bridge | `src/ai/scoring.rs:819, 974` (`score_dse_by_id`, `score_actions`) | `final_score` returned from `evaluate_single_with_trace` is pushed verbatim into the per-Action `(action, score)` tuple consumed downstream | `[verified-correct]` |
| Bonus pipeline (the 9) | `src/systems/goap.rs:1535–1604`; functions at `scoring.rs:1351,1400,1423,1435,1472,1502,1521,1543` and `plan_substrate/sensors.rs` | Nine `apply_*` passes mutate the score Vec post-L2-capture; their effects are invisible to the trace | `[verified-defect]` — this ticket retires |
| Softmax pool | `src/ai/scoring.rs:~1629` `select_disposition_via_intention_softmax_with_trace` | Reads post-bonus scores, applies Independence-penalty action-level transform on Coordinate / Socialize / Mentor (and Groom-other), Boltzmann-rolls at temperature `intention_softmax_temperature` | `[verified-correct]` |
| L3 capture | `src/ai/scoring.rs` + `src/systems/trace_emit.rs` `emit_focal_trace` | Surfaces the post-bonus pool + probabilities + roll into `FocalScoreCapture` | `[verified-correct]` |
| `chosen` field | resolved post `resolve_goap_plans` (`trace_emit.rs:234`) | First GOAP step (often e.g. `Action::Travel` for a Hunt plan); orthogonal to this defect | `[verified-correct, but visually misleading per existing 162 docstring]` |

## Sequencing

**Single-PR full migration.** All 12 modifier registrations + the 9
deletions land in one commit (or one squashed-commit-grade branch).
Justification:

- **The current chain is the regression.** Half-migrating leaves
  the trace incomplete on whichever `apply_*` is left behind,
  defeating the purpose of the migration. The user's framing
  ("otherwise all of the other balance stuff will continue being
  stupid filtered here") is correct: balance work upstream of any
  unmigrated bonus layer keeps suffering the same opacity.
- **No L1 prerequisite needed.** The cascading-bonus port reads
  from a new per-action scalar bundle in `ctx_scalars`, which the
  `ScoringContext` builder in `goap.rs::evaluate_and_plan` populates
  from the same `action_snapshot` it already builds at line ~1042.
  Same data flow, different consumer. No new substrate authoring
  system required, no `NeighborActionMap` introduction.
- **Existing modifier-pipeline saturating cap**
  (`max_additive_lift_per_dse`, ticket 146) automatically bounds the
  multi-modifier pile-up risk. Default 0.0 (disabled) → behavior
  matches today's un-bounded additive sum. Activating the cap is a
  separate balance decision; not in scope.
- **Modifier registration order matters within the additive
  section.** The legacy chain runs cooldown damp first, then
  additive bonuses (memory, colony-knowledge, priority, cascading,
  aspiration, preference, fated, directive). Modifier-pipeline
  insertion order in `default_modifier_pipeline` preserves this:
  cooldown damp prepended *before* the existing 20 modifiers (so
  Pride / Independence / Patience / etc. run on already-damped
  scores when the cat is in cooldown, matching today's order); then
  additive modifiers appended *after* the existing additive section
  but *before* the multiplicative damps (Fox / Corruption /
  Stockpile) so the cat-side score landscape sees the full additive
  lift before the spatial damps run. The exact insertion points are
  documented in the registration site's leading comment in the
  implementing PR.

### Behavior-preservation cross-check

Each new modifier emits a unit test asserting bit-equivalence to
the corresponding legacy function on a representative input. The
pipeline-level integration test asserts
`score_actions(...).then(legacy_apply_chain) == score_actions(...)`
with the chain retired and the modifiers registered, on a
synthetic cat with all nine triggers active. Both tests live in
`src/ai/modifier.rs` next to the existing per-modifier test suite.

### Rebuild order (within the single PR)

The implementer iterates in this order, asserting the locked
verification invariant after each step lands:

1. **`DispositionFailureCooldown`** — most mechanical (one
   disposition-ordinal scalar per kind, multiplicative damp).
   Smallest blast radius. Lands first.
2. **`MemoryResourceFoundLift` / `MemoryDeathPenalty` /
   `MemoryThreatSeenSuppress`** — three modifiers from one source
   function, all read pre-aggregated scalars. Same shape as
   `BodyDistressPromotion`'s additive lift over a class.
3. **`ColonyKnowledgeLift`** — two arms, mirrors Memory's shape
   but reads `ColonyKnowledge` instead of per-cat `Memory`.
4. **`ColonyPriorityLift`** — single ordinal scalar; mechanical
   per-DSE switch.
5. **`NeighborActionCascade`** — 16 new scalars in `ctx_scalars`;
   modifier reads its own action's count.
6. **`AspirationLift` / `PreferenceLift` / `PreferencePenalty`** —
   per-action scalars derived from per-cat components.
7. **`FatedLoveLift` / `FatedRivalLift`** — boolean scalars;
   cleanest.
8. **`ActiveDirectiveLift`** — depends on caller pre-computing the
   single magnitude scalar (lifts the existing personality + fondness
   + diligence pipeline at `goap.rs:1592–1604` *into a scalar
   producer* that lives next to other `ScoringContext` fields, then
   the modifier reads it through a single fetch).

The locked invariant (below) goes in the **first** commit so every
subsequent migration step either passes or surfaces a real bug.

## Verification

### Locked invariant (acceptance criterion)

> For every focal-cat tick across every scenario in
> `clowder::scenarios::ALL`, for every `Action` that appears in the
> L3 ranked pool,
> `|L2_record.final_score - pool_entry.score| < ε` (ε = 1e-4),
> with a carve-out for Independence-penalized actions
> (Coordinate / Socialize / Mentor / non-self Groom).

The Independence-penalty action-level transform applies inside
`select_disposition_via_intention_softmax_with_trace`; after
migration it is the only legitimate source of L2-vs-pool divergence
on Coordinate / Socialize / Mentor / non-self Groom. Out of scope:
moving the Independence penalty out of softmax and into a §3.5.1
modifier (separate ticket — see §Out of scope below).

The new `tests/scenarios.rs` test asserts the invariant for every
scenario × tick. Runtime: ~0.2s for 7 scenarios × ~10 ticks × ~16
DSEs. **Permanent CI invariant**: any regression to the "trace
silently incomplete" pattern fails the suite.

Test sketch:

```rust
#[test]
fn l2_final_matches_pool_entry_across_all_scenarios() {
    for scenario in scenarios::ALL {
        let report = runner::run(scenario, None, None, 42);
        for tick in &report.ticks {
            let pool: HashMap<&str, f32> = tick.ranked.iter()
                .map(|(name, score)| (name.as_str(), *score)).collect();
            for l2_row in &tick.l2 {
                if !l2_row.eligible { continue; }
                let action = action_for_dse(&l2_row.dse);
                if let Some(&p) = pool.get(action_name(action)) {
                    let independence_carveout =
                        matches!(action, Action::Coordinate | Action::Socialize | Action::Mentor)
                        || matches!((action, l2_row.dse.as_str()), (Action::Groom, "groom_other"));
                    if independence_carveout { continue; }
                    assert!((l2_row.final_score - p).abs() < 1e-4,
                        "scenario {} tick {} dse {}: L2 {} vs pool {}",
                        scenario.name, tick.tick, l2_row.dse,
                        l2_row.final_score, p);
                }
            }
        }
    }
}
```

### Soak verification

Re-run `just soak-trace 42 Simba 240` after the migration.
Spot-check ticks 1200003 / 1212627 / 1244383 against
`logs/163-l2-pool-verification/`. The trace now carries the
per-modifier deltas in `modifier_deltas` and the L2 `final_score`
matches the pool entry within ε. Hunt at tick 1200003 should show
a chain like: `composition=0.50 → maslow=0.50 → +pride=0.55 →
+independence_solo=0.60 → +memory_resource_found_lift=0.85 →
+neighbor_action_cascade=0.91 = pool 0.91`.

### Survival gates

`just soak 42 && just verdict logs/tuned-42-163` passes the
canonical seed-42 hard gates (`Starvation == 0`,
`ShadowFoxAmbush <= 10`, footer written,
`never_fired_expected_positives == 0`).

### Continuity canaries

All six (grooming / play / mentoring / burial / courtship /
mythic-texture) ≥ 1 per soak; mentoring within 2× the post-154
baseline of 1614. The migration is a substrate-shape change with no
behavior delta target — any movement on canaries is a real signal
worth investigating. If any canary regresses by >2× that's a
balance-methodology event (`docs/balance/*.md` thread).

### Post-merge follow-on

**The kitten_cry_basic Caretake collapse re-investigation** is a
*post-merge* task, not part of this ticket's acceptance. Once the
trace is truthful, the 1.087 → 0.105 path is readable directly from
the L2 `modifier_deltas` (the dominant contributor will be visible
as the largest negative delta). Open a follow-on bugfix ticket once
the contributor is identified — most likely
`DispositionFailureCooldown` damping Caretake at the scenario's
preset state, but won't know until the trace is honest.

## Out of scope

- **Changing softmax temperature, rolling shape, or distribution.**
  `intention_softmax_temperature`, the Boltzmann shape, the RNG
  roll — all preserved. This ticket relocates substrate reads, not
  softmax policy.
- **Re-tuning Independence-penalty placement.** The penalty
  currently applies inside the softmax helper as a per-Action
  transform on Coordinate / Socialize / Mentor / non-self Groom.
  Migrating it to a §3.5.1 modifier is the natural next step (and
  would close the Independence carve-out in the verification
  invariant), but it requires separating the Groom-self-vs-Groom-
  other routing from the modifier pipeline, which has its own
  complications (the `Groom` Action collapses two DSE ids;
  modifiers operate per-DSE-id). Open a sibling ticket once 163
  lands.
- **Changing DSE → Action collapse rules** (Groom self/other → max
  → `Action::Groom`; Herbcraft sub-mode max; PracticeMagic sub-mode
  max). Selection-layer concern, not a §3.5.1 modifier concern.
- **Tradition's unfiltered-loop fix** (ticket 058 parked).
  Independent semantic decision; doesn't compose with this
  migration.
- **Promoting the cascading-bonus read to an L1 `NeighborActionMap`**.
  Considered and rejected above; if a future caller wants spatial
  diffusion semantics, that's an InfluenceMap §5 ticket of its own.
- **The kitten_cry_basic Caretake 1.087 → 0.105 puzzle.**
  Re-investigated after this ticket lands and the trace is
  truthful; either resolves cleanly under the substrate-truthful
  trace or opens a tightly-scoped follow-on ticket. Either way,
  it's not 163's acceptance criterion.

## Never-fired-positive canary

This migration introduces **12 new `ScoreModifier`s** but **zero new
`Feature` emissions**. Reason: the existing 20 §3.5.1 modifiers
emit no `Feature` either; ticket 099 ("modifier feature emission")
is the substrate-quality follow-on tracking that gap uniformly
across all modifiers. The 12 new modifiers will inherit the same
Feature-emission gap and the same future fix. **No new Feature → no
`expected_to_fire_per_soak()` classification change.** If a
reviewer surfaces a per-modifier Feature requirement mid-migration,
defer to 099 rather than carve out a one-off here.

## Coordination notes

- **Ticket 081** (coordinator-side directive-failure demotion) —
  landed 2026-05-02 (commit `acb30b9d`, retired without
  implementation; lives at
  `docs/open-work/landed/081-coordination-directive-failure-demotion.md`).
  081's surface is coordinator-side dispatch policy; 163's
  `apply_directive_bonus` migration is the per-cat read of an
  already-issued directive. **Different surfaces; no overlap; no
  carve-out needed.**
- **Ticket 014** (`docs/open-work/landed/014-phase-4-follow-ons-closeout.md`) —
  the Phase-4 closeout shipped the §3.5 modifier pipeline framework
  but did not enumerate the 9 pre-existing imperative passes as
  out-of-scope. CLAUDE.md "Antipattern migration follow-ups are
  non-optional" implies this should have produced a follow-on at
  014's land. The implementer adds a one-line retro-note to 014's
  closeout in the same commit that lands 163, capturing the scoping
  miss for future audits without rewriting the closeout's load-
  bearing description of what landed.
- **Ticket 158** (kitten-cry seed-42 starvation) — stays
  blocked-by-161-only. 158's failure mode and structural fix
  operate at the eligibility / target-resolution layer (`scoring.rs`
  early-zero gate + `IsParentOfHungryKitten` marker authoring),
  upstream of where 163 operates. 163 lands independently.
- **Ticket 093** (substrate-over-override antipattern epic) —
  active at `docs/open-work/tickets/093-substrate-over-override-epic.md`.
  Add a row to its inventory naming the 9 `apply_*` chain as the
  bonus-pipeline-as-imperative-passes antipattern with a pointer
  to 163. The implementer updates 093 in the same commit.

## Implementer entry point

Start by reading `src/ai/modifier.rs` lines ~2120–2270 (the
`default_modifier_pipeline` registration helper and its surrounding
doc-comments naming what each modifier does and *why the registration
order matters*). That single block is the load-bearing surface to be
edited 12 times. Pair that with `src/ai/scoring.rs::ctx_scalars`
(line ~444) to see where new scalar producers slot in (each modifier
in this migration reads via `fetch_scalar`, and 16 of the new scalars
are per-action `cascade_count_*` keys plus a handful of per-action
`aspiration_action_*` / `preference_for_*` keys; producing them at
`ScoringContext` build time is the bulk of the work outside the
modifier impls themselves).

The first concrete migration task is **`DispositionFailureCooldown`**
— it is the only multiplicative pass and the most mechanical (a
single existing helper at `plan_substrate/sensors.rs` already
produces the per-disposition signal). Land it, watch the verification
invariant pass on the 7 scenarios for that one modifier (no other
change), then iterate the remaining 11 in the order listed in
§Sequencing's "Rebuild order". The locked invariant in
`tests/scenarios.rs` should be added in the **first** commit of the
migration with the Independence carve-out exception baked in — that
way every subsequent migration commit either passes the invariant or
surfaces a real bug. **Avoid the temptation to land all 12 modifiers
in one mega-commit before running the test; iterate.**

## Log

- 2026-05-04: opened. Replaces an earlier same-numbered draft that
  proposed an "extend L2 trace with `post_bonus_deltas`, migrate
  later" sequencing — rejected by the user in favor of full-batch
  migration. The earlier draft is preserved in git history as the
  pre-revision file. The substrate is wired
  (`score_actions → score_dse_by_id → evaluate_single_with_trace`,
  same path the L2 trace reads); the divergence comes exclusively
  from the 9 `apply_*` passes at `goap.rs:1535–1604` mutating
  per-Action scores after the L2 record is captured. Soak
  verification in `logs/163-l2-pool-verification/` (ticks
  1200003 / 1212627 / 1244383, focal Simba seed 42) confirms the
  divergence is substrate-state-driven, not a softmax bug.
  Migration ports each legacy `apply_*` to a registered §3.5.1
  modifier; cooldown damp → multiplicative; the 8 additive layers
  → additive lifts; cascading-bonus reads stay flat-range (no L1
  `NeighborActionMap` introduced). Acceptance: locked invariant
  `|L2.final_score - pool_entry| < 1e-4` for every focal-cat tick
  across every scenario, captured as a permanent
  `tests/scenarios.rs` assertion. Out-of-scope: softmax
  temperature, Independence-penalty migration, DSE→Action collapse
  changes, Tradition unfiltered-loop fix (058), and the
  kitten_cry_basic Caretake collapse (re-investigated post-
  migration with the trace truthful). Coordination: 081 already
  retired (no overlap); 014 closeout gets a one-line retro-note;
  158 stays blocked-by-161; 093 inventory gets a row pointing
  here.
