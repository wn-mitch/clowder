---
id: 450
title: Three-stage kittenhood — Newborn / Eyes-open / Juvenile with progressive capability gates
status: ready
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: []
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

Today's `Kitten` life-stage is monolithic: 0–3 seasons of maturity carry the same capability set (`src/components/identity.rs:24-68`, `src/systems/growth.rs::tick_kitten_growth`). Real kittens go through clear ethological sub-stages — *Newborn* (motionless, eyes closed, beg-and-sleep only); *Eyes-open* (mobile, can play and beg); *Juvenile* (the "mentorable" phase where they shadow adult hunters and start learning the colony's craft). The current monolithic stage flattens that arc and produces two specific defects: (1) early kittens with `Kitten` are eligible to be mentees of `Mentor` even when they can't yet absorb skills, inflating mentoring frequency on cats that gain nothing from it; (2) the autonomic cry-map fires whenever `hunger < threshold` regardless of stage, so kittens are perceived as "begging" even when they're physiologically incapable of beg-emission (Stage 1 newborns can't yet vocalize for food in any directed sense). Decomposing the stage into three sub-stages, each gated by progressive capability markers, gives parents and mentors an authentic ethological target and lets the substrate price "this kitten is helpless / partly able / nearly competent" as orthogonal axes rather than a single coarse flag.

This ticket is also load-bearing for [429]: 429's Phase 2 promotes `eat_from_inventory` to a planning-pool DSE with no Adult gate, and the cleanest expression of "kittens plan the same Eat aspiration as adults, but their means differ" lives in the HTN method registry — the substrate added here (sub-stage markers + the `[BegForFood]` method that decomposes Eat for empty-inventory kittens) is what 429's DSE-side absence-of-gate composes against.

## Scope

1. **Sub-stage markers** in `src/components/markers.rs`:
   - `NewbornKitten` — entity-scoped; authored when `Kitten ∧ KittenDependency.maturity < 0.33`.
   - `EyesOpenKitten` — entity-scoped; authored when `Kitten ∧ 0.33 ≤ maturity < 0.67`.
   - `JuvenileKitten` — entity-scoped; authored when `Kitten ∧ 0.67 ≤ maturity < 1.0`. (Maturity ≥ 1.0 retires `KittenDependency` + the `Kitten` life-stage marker per existing `tick_kitten_growth`.)
   - `MentorableAge` — entity-scoped; authored when `JuvenileKitten ∨ Young ∨ Adult`. Stage 1 and Stage 2 kittens cannot receive mentoring.
   - `HasFoodInInventory` — entity-scoped; authored from `inventory.has_food()`. (Used here by the `[BegForFood]` HTN method's `ApplicableWhen::Kitten ∧ ¬HasFoodInInventory`; consumed by [429] Phase 2 for the eat-from-own-inventory DSE eligibility.)
   - All markers ship with `set_entity(M::KEY, …)` populators in `src/systems/goap.rs::evaluate_and_plan` per CLAUDE.md's substrate-stub-forbid rule. `growth.rs::update_life_stage_markers` (already authors `Kitten`) extends to author the sub-stage markers from the same `KittenDependency.maturity` read.

2. **Reuse `Incapacitated` for Stage 1 motionlessness.** Newborn kittens are positioned, in substrate terms, the same way severely injured cats are: physiologically present, not eligible for self-directed action. `growth.rs::update_life_stage_markers` inserts the existing `Incapacitated` marker (`src/components/markers.rs:105-114`) when `NewbornKitten` and removes it on the Stage 1 → Stage 2 transition. Every existing `.forbid(Incapacitated::KEY)` filter already excludes them from fetch / forage / hunt / mate / cook / ward / mentor without any new gates — elegant substrate reuse rather than authoring a new `NonAmbulatory` marker. Verify in `src/systems/movement.rs` that movement is already gated on `Without<Incapacitated>` and add the filter if missing.

3. **Capability gates:**
   - `src/ai/dses/mentor_target.rs` — add `.require(MentorableAge::KEY)` on the mentee side. (`Incapacitated` already excludes Stage 1 from mentor-target candidacy via the existing forbid.)
   - `src/ai/capabilities.rs:117` (`CanForage`) — flip from `!is_kitten` to `is_adult || is_young || JuvenileKitten`. `CanHunt` stays `is_adult || is_young` (Stage 3 kittens learn hunting via mentoring; they don't yet hunt solo — confirm with user before flipping).
   - The bulk of the gating concentrates at the capabilities layer, not per-DSE: `.forbid(Incapacitated)` handles Stage 1, the `Kitten` marker still trips `!is_kitten` checks for Stages 1–2, and `JuvenileKitten` is the explicit Stage 3 lift.

4. **Begging as an HTN method of Eat, not a peer DSE.** Kittens have the same *want* (Eat) as everyone; what differs is the *means*. Per CLAUDE.md "All multi-tick aspirations are HTN methods," the registry in `src/ai/planner/` gains a new method that decomposes the Eat aspiration into begging when no fetch-method is applicable:
   - **Existing Eat methods (verify in `populate_method_registry`):**
     - `[TravelTo(Stores), EatAtStores]` — `ApplicableWhen::CanForage` (Adult, Young, or Stage 3 kitten via the updated capabilities gate).
     - `[EatFromInventory]` — `ApplicableWhen::HasFoodInInventory` (the method 429 Phase 2 adds).
   - **New `[BegForFood]` method:** `ApplicableWhen::Kitten ∧ ¬HasFoodInInventory` (the kitten can't fetch — `CanForage` already excludes pre-Stage-3 kittens — and has no food to eat, so the only remaining means is begging).
   - **`Action::BegForFood`** variant in `src/ai/mod.rs::Action`.
   - **`DispositionKind::Begging`** in `src/components/disposition.rs` with `from_action` + `constituent_actions` arms. (Distinct from `Eating` because the completion proxy differs — Eating completes on food consumed; Begging completes when another cat arrives with food and `feed_kitten` fires.)
   - **`resolve_beg_for_food`** in `src/steps/disposition/beg_for_food.rs`. Signature: `(ticks, kitten_entity, position) -> StepOutcome<Option<BegEmitted>>`. Witness `BegEmitted { kitten: Entity, position: Position }`. Real-world effect: registers the kitten in the cry-map (or amplifies its cry intensity at its tile). Five-heading rustdoc per CLAUDE.md GOAP Step Resolver Contract.
   - **`Feature::KittenBegged`** (Positive, classified `=> true`) in `src/resources/system_activation.rs`.
   - **Persistence is automatic.** The kitten's per-tick L2 scoring keeps Eat winning while hungry; the HTN method registry keeps selecting `[BegForFood]` while `ApplicableWhen` holds; `§7.4 persistence-bonus` smooths flicker. No commitment-substrate stub. The moment a parent feeds the kitten and `HasFoodInInventory` fires, the next planning tick selects `[EatFromInventory]` instead — "beg until they get it" emerges from method-applicability cascade without custom routing.
   - **L2-trace visibility.** The kitten's L2 trace shows `Action::Eat` winning (hunger-driven Eat DSE producing the score); the method choice (BegForFood vs EatFromInventory vs Stores-fetch) is recorded in the chosen plan emitted from HTN expansion. Cleanest version of "richer perception, better strategy": Eat is the want; means differs by capability.
   - **Cry-map routing.** Today's `update_kitten_cry_map` (`src/systems/growth.rs:123-150`) emits a spatial cry disc whenever `hunger < kitten_cry_hunger_threshold` automatically. After this ticket, the cry-map reads from witnessed `Feature::KittenBegged` activations rather than raw hunger. The parent-side `IsParentOfHungryKitten` marker continues to fire (consumes the same cry-map state); only the *source* of the cry-map data changes (autonomic-hunger → witnessed-begging). The autonomic path retires in the same commit — no dual emission.
   - **`scripts/check_method_registry.sh`** enforces that the new method has its `ApplicableWhen` wired and the corresponding `Action` variant exists — same enforcement strength as substrate-stubs. The method is *active* at landing (not `PendingSubstrate`), so no `wires-method` ticket is needed.

5. **Verification scenario:** `kittenhood_stages` (`src/scenarios/kittenhood_stages.rs`) — preset four cats (newborn, eyes-open, juvenile, adult). Assert sub-stage markers fire correctly at maturity boundaries; assert Stage 1 has `Incapacitated` and Stage 2 has it removed; assert mentor-target DSE only accepts `MentorableAge` mentees; assert `[BegForFood]` is the selected method for hungry-empty-inventory kittens at every stage and Stage 1 keeps begging while Stage 3 may compete with forage if both eligible.

## Out of scope

- **Ticket [429] itself** — this lands first as substrate; 429 builds on the markers in its Phase 2.
- **Tuning the maturity thresholds** (0.33 / 0.67) — these are starting points anchored to even-thirds; balance work happens after the substrate stabilizes.
- **A new `NonAmbulatory` marker** — explicitly rejected; `Incapacitated` covers Stage 1 motionlessness via existing forbid filters.
- **Stage 3 kittens hunting solo** — per user spec, Stage 3 learns hunting via mentoring but doesn't yet hunt solo. `CanHunt` stays `is_adult || is_young`.
- **Tuning mentoring rates after the gate-tightening** — if the `mentoring` continuity canary falls below threshold post-landing, that's a balance follow-on; first try the tighter gate.

## Current state

Opened 2026-05-22 as a sibling ticket to [429]. Per CLAUDE.md "Antipattern migration follow-ups are non-optional," opened in the same conceptual landing-stack as 429 (which will gain `blocked-by: [450]` to encode the dependency). User framing for the staging itself: "kittens spawn nearly helpless, motionless, only beg + sleep; at 1/3 maturity they open their eyes, can play + beg + sleep; at 2/3 they can additionally be mentored on hunting. They should still plan eating at every stage — they just can't fetch their own food until Stage 3." User framing for begging-as-method (not peer DSE): "Kittens want to eat, GOAP eligibility prevents it so food falls onto the 'beg until I get it' track."

## Approach

Phase A (this ticket):
1. Add markers + `HasFoodInInventory` to `src/components/markers.rs`; populate via `MarkerSnapshot` in `src/systems/goap.rs::evaluate_and_plan` so eligibility filters can read them.
2. Extend `src/systems/growth.rs::update_life_stage_markers` to author the four sub-stage markers + `MentorableAge` + insert/remove `Incapacitated` on the Stage 1 ↔ Stage 2 transition.
3. Author `Action::BegForFood`, `DispositionKind::Begging`, `resolve_beg_for_food`, `Feature::KittenBegged`, the `[BegForFood]` HTN method.
4. Swap `update_kitten_cry_map` to consume witnessed `Feature::KittenBegged` activations rather than raw hunger; retire the autonomic dual emission.
5. Update `src/ai/dses/mentor_target.rs` (`.require(MentorableAge)`) and `src/ai/capabilities.rs` (`CanForage` permit `JuvenileKitten`).
6. Author scenario + verify.

## Verification

- `just check` — substrate-stub / marker-snapshot-wiring / method-registry scripts pass for the new markers + method.
- `just test` — full unit suite plus new harness for `resolve_beg_for_food` witness shape.
- `just scenario kittenhood_stages` — assertions per Scope §5.
- `just soak-trace 42 Simba` + `just verdict logs/tuned-42` — no drift on adult-cat metrics (kitten gating shouldn't shift adult behavior). Compare against `logs/baselines/current.json`. Verify mentoring rates drop modestly (only Stage 3 kittens are now mentorable; pre-change all 0–3-season kittens were eligible). `mentoring` continuity canary must remain ≥1 per soak.
- `just frame-diff logs/baselines/current/trace-Simba.jsonl logs/tuned-42/trace-Simba.jsonl` — no per-DSE drift >10% on adult-only DSEs. Kitten-side: select a Stage 1 / Stage 2 / Stage 3 focal cat from the seed-42 roster and verify the `[BegForFood]` method appears in the chosen-plan trace for Stage 1–2 hungry empty-inventory ticks.

## Risks

- **Continuity-canary risk for mentoring.** Restricting mentor-target eligibility to `MentorableAge` will reduce mentoring frequency (pre-change all 0–3-season kittens were eligible mentees). The `mentoring` continuity canary must stay ≥1 per soak — verify before landing. If it falls below threshold, the fix is to widen `MentorableAge` to include `EyesOpenKitten` (Stage 2) as a balance call, but try the tighter gate first.
- **Cry-map routing change.** Switching `update_kitten_cry_map` from raw `hunger < threshold` to witnessed `Feature::KittenBegged` means the parent-side `IsParentOfHungryKitten` marker now depends on the HTN method `[BegForFood]` actually being selected when the kitten's Eat aspiration wins. Risk: if Sleep wins L2 because hunger is moderate but energy is low, the method cascade never runs, no Beg fires, no cry signal. Verify in soak that Stage 1 kittens spend most hungry ticks with Eat winning. If Sleep/Idle dominate during early hunger, the fix is *consideration-side* on the Eat DSE (steeper Logistic curve at low hunger so Eat wins earlier), NOT adding a separate Beg DSE — that re-introduces the peer-DSE pattern explicitly rejected here.
- **Cry-map dual-emission antipattern.** Today's autonomic cry is "raw hunger → spatial disc." After this ticket the cry is "witnessed BegForFood method execution → spatial disc." We must NOT keep both — that's belt-and-suspenders dual emission, which inflates the cry-map and breaks parent Caretake scoring. Per substrate-over-hacks, retire the autonomic path entirely; the verification soak proves the substrate-driven path covers it.
- **HTN method ordering / determinism.** Order of methods in `populate_method_registry` for the Eat aspiration is load-bearing for tie-breaking when multiple are applicable. Ensure the `ApplicableWhen` for `[BegForFood]` is precise enough (`Kitten ∧ ¬HasFoodInInventory`) that a Stage 3 kitten with food in inventory does not see both `[EatFromInventory]` and `[BegForFood]` as applicable. Verify against seed-42 determinism.

## Log

- 2026-05-22: opened as a sibling ticket to [429] per the kitten-staging + items-are-real plan stack. User framing surfaced during 429 plan review: kittens-still-plan-Eat-at-every-stage; means differ by capability via HTN methods; new sub-stage markers + Incapacitated reuse + BegForFood method.
