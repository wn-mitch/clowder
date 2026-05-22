---
id: 451
title: Lift Without<KittenDependency> filter so kittens elect Begging at L3
status: ready
cluster: ai-substrate
initiative: [parenting-substrate]
added: 2026-05-22
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

Ticket [450] landed `Action::BegForFood` + `DispositionKind::Begging` + `BegForFoodDse` (two sibling registrations, NewbornKitten + EyesOpenKitten) + `resolve_beg_for_food` + plan template + dispatch arm + `Feature::KittenBegged` — the full substrate for kittens to elect Begging at L3. But the L2 evaluator `evaluate_and_plan` (`src/systems/goap.rs:1334-1392`) and the disposition-path evaluator `evaluate_dispositions` (`src/systems/disposition.rs:374-415`) both filter their cats queries with `Without<crate::components::KittenDependency>` — kittens are excluded from scoring entirely. The §Phase 5b filter was put in place because the post-loop `kitten_needs` query (`Without<GoapPlan>`) silently dropped kittens that had GoapPlan, so adults' `+0.5` hunger restoration from FeedKitten never landed on the kitten. Until that filter lifts (or a kitten-side scoring path is added), the 450 substrate is dormant: `Feature::KittenBegged` ships `expected_to_fire_per_soak() => false`, the cry-map retains its autonomic `Needs.hunger < threshold` predicate (a flip to the substrate-driven `CurrentAction.action == BegForFood` would permanently silence the cry-map in production), and the BegForFoodDse registry entries never reach a kitten because no kitten enters the evaluation pool.

## Scope

1. **Lift the `Without<KittenDependency>` filter** from `evaluate_and_plan` (`goap.rs:1391`) AND `evaluate_dispositions` (`disposition.rs:414`). Replace the filter with a per-cat eligibility check that allows kittens to enter scoring but constrains their L3 election to the narrow set their substrate supports (BegForFood / Sleep / Idle for Stage 1; BegForFood / Sleep / Idle / play-future for Stage 2; ForageDse / Mentor-mentee facet for Stage 3).
2. **Reshape `kitten_needs` query** (`goap.rs:634-639`) so it stays disjoint from the cats query while not excluding kittens entirely. Two candidate approaches:
   - (a) Drop the `Without<GoapPlan>` constraint and let `kitten_needs` match all live kittens — feasible if `cats.get_mut(kitten_entity)` is no longer attempted in the post-loop drain (since kittens now have GoapPlan but their own dispatch arm runs the resolver).
   - (b) Add a sibling query `kitten_with_plan: Query<&mut Needs, (With<KittenDependency>, With<GoapPlan>)>` for the FeedKitten drain to use when the recipient kitten has its own plan.
3. **Cry-map flip activation.** Once kittens elect at L3, flip `update_kitten_cry_map`'s predicate from raw `Needs.hunger < threshold` to `CurrentAction.action == BegForFood && Needs.hunger < threshold` (the compound predicate from the parked 450 attempt). Retire the autonomic path per the no-dual-emission rule.
4. **Promote `Feature::KittenBegged`** to `expected_to_fire_per_soak() => true` after the seed-42 verification soak observes ≥1 emission per healthy run.
5. **Promote the `kittenhood_stages` scenario's `expected_features`** to `&["KittenBegged"]` so the scenario harness gates the canary at structural verification time.
6. **Stage 1 special case.** Newborn kittens carry `Incapacitated` (450 substrate reuse). The Maslow gate + most DSE eligibility filters `.forbid(Incapacitated)` already restrict their L3 pool to Eat / Sleep / Idle. `BegForFoodDse` deliberately does NOT forbid Incapacitated so a Newborn can elect Begging — verify that the scoring path doesn't fast-path-zero an Incapacitated cat in a way that bypasses the BegForFood DSE entirely.
7. **Verify no FeedKitten regression.** Run a seed-42 soak post-fix; confirm `KittenFed` continues firing at the same rate (any drop indicates the kitten-Needs drain regressed). If it drops, the (b)-shape sibling query is required.

## Out of scope

- **Stage 3 (Juvenile) hunting solo.** Per 450's user-spec, Stage 3 kittens learn hunting via mentoring; they don't yet hunt solo. `CanHunt` stays gated on `is_adult || is_young`.
- **Kitten-side modifier pipeline tuning.** The §3.5 modifier catalog has implicit "adult-only" assumptions in some lifts (e.g. CommitmentTenure, IntentionMomentum). Tuning their kitten-side behavior is a balance follow-on, not part of unblocking L3 election.
- **Kitten target-taking DSE participation** (CaretakeTarget, MentorTarget mentee side, etc.). Kittens-as-targets is already wired via the parent-side DSEs; this ticket only opens the kitten-as-actor path.

## Current state

Opened 2026-05-22 alongside the 450 BegForFood substrate landing per CLAUDE.md "Antipattern migration follow-ups are non-optional" (parent 450 narrowed scope to the substrate-only landing; this ticket carries the kitten-scoring activation).

## Approach

Bugfix-discipline layer-walk: read `evaluate_and_plan`'s query (`goap.rs:1334-1392`), the `kitten_needs` post-loop drain (`goap.rs:634-639`), the FeedKitten dispatch arm + drain (`goap.rs:6575+`), and the §Phase 5b history (commit log around the filter's introduction). Then a small reproduction unit test that (a) spawns a hungry NewbornKitten with `KittenDependency`, (b) runs evaluate_and_plan once, (c) asserts `CurrentAction.action == BegForFood`. Once green, run `just scenario kittenhood_stages` and confirm `Feature::KittenBegged` fires.

## Verification

- New unit test in `src/systems/goap.rs::tests` for kitten-electing-BegForFood (per Approach above).
- `just scenario kittenhood_stages` — `expected_features: &["KittenBegged"]` promoted.
- `just soak-trace 42 Simba` + `just verdict logs/tuned-42` — no FeedKitten / KittenFed regression; `KittenBegged` fires ≥1; `mentoring` continuity canary stays ≥1 (450's `MentorableAge` gate already shipped).
- `just frame-diff logs/baselines/current/trace-Simba.jsonl logs/tuned-42/trace-Simba.jsonl` — adult-only DSE drift <10%.
- Optional: focal-trace a Stage 1 kitten (find one in seed-42 roster) and verify BegForFood appears in the chosen-plan trace.

## Log

- 2026-05-22: opened as a follow-on to [450]'s landing. 450 authored the full BegForFood substrate but discovered at verification time that kittens are filtered out of L2/L3 scoring; this ticket carries the production activation.
