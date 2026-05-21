---
id: 440
title: migrate simple single-marker score_actions gates to EligibilityFilter
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

Ticket [[438]] landed the dispatcher retirement but routed *every* outer gate that existed pre-438 through the new `PRE_DISPATCH_GATES` `BTreeMap` table to preserve seed-42 byte-for-byte. Several of those gates are *not* composite / threshold / side-effect-bearing — they're plain single-marker checks against state the substrate writer already authors in `goap.rs::evaluate_and_plan`. Routing those through the pre-dispatch table is substrate-incorrect: it duplicates the gate intent across the DSE's `EligibilityFilter` surface and the dispatcher's wrap-site, and it costs an extra `BTreeMap::get` lookup per cat per tick. The substrate-canonical home for "this DSE is eligible iff marker M is set" is `EligibilityFilter::require(M::KEY)`, full stop.

## Scope

Migrate the following from `PRE_DISPATCH_GATES` (in `src/ai/scoring.rs::pre_dispatch_gates`) into each DSE's `EligibilityFilter`:

- `socialize` → add `.require(HasSocialTarget::KEY)` to `SocializeDse` (`src/ai/dses/socialize.rs:153`); drop the pre-dispatch entry.
- `groom_other` → add `.require(HasSocialTarget::KEY)` to `GroomOtherDse`; drop the pre-dispatch entry.
- `herbcraft_gather` → add `.require(HasHerbsNearby::KEY)` to `HerbcraftGatherDse`; drop the pre-dispatch entry.
- `magic_harvest` → add `.require(CarcassNearby::KEY)` to `HarvestDse`; drop the pre-dispatch entry (only after the `magic_outer` outer-gate question is settled — see Out of scope).
- `magic_commune` → add `.require(OnSpecialTerrain::KEY)` to `CommuneDse`; same caveat as `magic_harvest`.
- `bury` → BuryDse already requires `HasUnburiedCorpse::KEY`. The pre-dispatch entry is redundant; verify L3 softmax pool size is identical with the gate removed (per the 035 comment about pool stability) and drop it if so.

All writers (`HasSocialTarget`, `HasHerbsNearby`, `CarcassNearby`, `OnSpecialTerrain`, `HasUnburiedCorpse`) already exist in `src/systems/goap.rs::evaluate_and_plan` — `scripts/check_marker_snapshot_wiring.sh` keeps them aligned with `.require()` consumers.

## Out of scope

- The `magic_outer` outer-gate (`magic_affinity > X && magic_skill > Y`) is a continuous-threshold composite, not a single marker. It can't migrate to filter cleanly. Keep it in `PRE_DISPATCH_GATES` OR convert it to a `Consideration` curve that clamps to 0 below threshold (separate balance question — open a new ticket if pursuing).
- The composite OR / AND / threshold gates (Flee `has_threat_nearby OR safety<X`, Fight `has_threat_nearby AND allies>=N`, Patrol `safety<X`, Build `has_construction_site OR has_damaged_building`, HerbcraftPrepare `has_remedy_herbs AND colony_injury_count>0`, MagicCleanse `on_corrupted_tile AND tile_corruption>X`, MagicDurableWard `magic_skill>X`) stay in the table — `EligibilityFilter`'s required-AND-of-markers shape doesn't express OR / continuous-threshold / per-tick-int-count cleanly.
- Side-effect-bearing post-evaluation hooks (Cook's `wants_cook_but_no_kitchen` write, Caretake's durable-commitment lift, HerbcraftWard's siege bonus) stay in `POST_EVAL_HOOKS` — that's the right home for non-score mutations.

## Current state

438 landed `PRE_DISPATCH_GATES` carrying every pre-438 outer gate. Behavior was preserved byte-for-byte (mod the hide first-light activation). This ticket is the substrate-correct cleanup: the simple gates belong on the DSE-side filter, not on the dispatcher-side wrap.

## Approach

One commit per migration (~6 small commits, each touching one DSE file + one entry removal in `pre_dispatch_gates`):

1. For each simple-gate DSE: add `.require(M::KEY)` to its `EligibilityFilter::new()` chain.
2. Remove the corresponding `t.insert(DseId("..."), |ctx, _| ctx.<flag>)` line from `pre_dispatch_gates()`.
3. Run `cargo test --release --lib` to confirm the per-DSE unit tests still pass.
4. After all six migrations, run `just check` + a `just soak-trace 42 Simba` + `just verdict` to verify seed-42 didn't shift (the pre-dispatch gate and the filter are semantically equivalent for these cases — the RNG draw position is identical because the gate's truthiness pattern hasn't changed; what changes is WHERE the check lives).

The `check_substrate_stubs.sh` script will auto-validate that each new `.require(M::KEY)` has a matching writer in `evaluate_and_plan`. All six markers already do.

## Verification

- `just check` clean — including the substrate-stub and marker-snapshot-wiring scripts.
- `cargo test --release` — all 2379+ lib tests pass.
- `just soak-trace 42 Simba` + `just verdict logs/tuned-42-<sha>` — seed-42 behavior preserved relative to the 438 baseline (`logs/tuned-42-75deed49`). The migrations are semantically equivalent (filter rejects ⇔ pre-dispatch gate rejects) so no drift expected; if drift surfaces, investigate before landing.
- The dispatcher-invariant lint (`scripts/check_score_actions_dispatch.sh`) stays passing — exactly one `score_dse_by_id` call site in `score_actions`, every `impl CatDse` paired with a `CAT_DSE_REGISTRY` entry.

## Log
- 2026-05-21: opened as a follow-on to [[438]]. The dispatcher refactor preserved seed-42 by routing every existing outer gate through `PRE_DISPATCH_GATES`; this ticket migrates the substrate-correct ones (single-marker, no side effect, no composite) into their consumer DSEs' `EligibilityFilter`. Blocked-by edge dropped at open-time because 438 is already in `landed/`.
