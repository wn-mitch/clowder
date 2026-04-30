---
id: 038
title: "MaterialsDelivered routing gap → full Pickup/Carry/Deliver pipeline (infrastructure landed, founding spawn parked)"
status: done
cluster: null
landed-at: null
landed-on: 2026-04-26
---

# MaterialsDelivered routing gap → full Pickup/Carry/Deliver pipeline (infrastructure landed, founding spawn parked)

**Landed:** 2026-04-26 | **Tickets:** 038 (MaterialsDelivered Feature blind to the GOAP Construct path).

Investigation reframed the ticket. The original diagnosis ("`resolve_construct` absorbs material delivery as a side-effect and doesn't emit the Feature") was wrong: `resolve_construct` only checks `materials_complete()` as a precondition and never calls `site.deliver()`. The real cause is that **every production `ConstructionSite` is spawned via `coordination.rs:1127` using `ConstructionSite::new_prefunded`**, which clones `materials_needed` into `materials_delivered` at spawn time. `ConstructionSite::new()` is only called in tests. The legacy disposition-chain `Deliver` step is correctly wired and would emit the Feature, but no system produces those chains, and `GoapActionKind::DeliverMaterials` is a 20-tick no-op stub. The result: cats never haul, `site.deliver()` is unreachable, the Feature never fires.

The user's design intent: founding gets prefunded **but on the ground, then delivered as the first colony act, akin to Dwarf Fortress wagon dismantling.** The colony arrives carrying their materials, those drop on the ground next to the founding site, and the cats' first work is hauling them in.

**Infrastructure landed (B2 scope):**

- `ItemKind::Wood`, `ItemKind::Stone` variants + `decay_rate=0.0` + `material()` bridge to the construction `Material` enum (`src/components/items.rs`).
- `BuildMaterialItem` marker component for static-disjoint queries between food/herb item access (read-only) and build-material access (mutable). Without the marker, `Query<&Item>` and `Query<&mut Item>` overlap on the same entities and Bevy's borrow checker (B0001) rejects the system. Marker stamped at spawn; `items_query` filters `Without<BuildMaterialItem>`, `material_items` filters `With<BuildMaterialItem>`.
- `PlannerZone::MaterialPile` + `PlannerState.materials_available` + `StatePredicate::MaterialsAvailable` + `StateEffect::SetMaterialsAvailable`. Per-site `materials_available` authored from the cat's nearest `ConstructionSite`'s `materials_complete()` (read off the new `StepSnapshots::construction_materials_complete` map).
- `building_actions()` rewritten — emits `[TravelTo(MaterialPile), GatherMaterials, TravelTo(ConstructionSite), DeliverMaterials, Construct]` for unfunded sites with reachable piles; `Construct` short-circuits when `materials_available=true`.
- New `resolve_pickup_material` step resolver (`src/steps/building/pickup_material.rs`) — adjacent ground-item lookup, walks toward distant piles, flips `Item::location` from `OnGround` to `Carried(cat)`, calls `Inventory::add_item`. 6 unit tests covering adjacent / distant / full-inventory / wrong-kind / already-carried / missing-target.
- `resolve_deliver` reworked to consume one inventory unit per call instead of just bumping the site counter. 5 unit tests. Legacy chain shim (`deliver_legacy_chain_adapter`) preserves `task_chains.rs` Deliver-step semantics.
- GOAP dispatch wired (`src/systems/goap.rs`): `GatherMaterials` → `resolve_pickup_material` + `Feature::MaterialPickedUp`; `DeliverMaterials` → `resolve_deliver` + `Feature::MaterialsDelivered`. Both gated through `record_if_witnessed`.
- New `Feature::MaterialPickedUp` + restored Positive classification of `Feature::MaterialsDelivered`. Both currently demoted to `expected_to_fire_per_soak() → false` while founding spawn is parked.
- `ConstructionSite::new_with_custom_cost` constructor for the founding act's small (4-Wood) cost vs the blueprint default (10 Wood + 5 Stone for `Stores`).
- Founding spawn (`spawn_founding_construction_site` in `src/world_gen/colony.rs`) gated behind `CLOWDER_FOUNDING_HAUL` env var (default off). When activated, spawns one founding non-prefunded `Stores` site north of `colony_site` plus 4 ground Wood items south of it.

**Hypothesis:** Founding wagon-dismantling spawn ⇒ cats haul ground Wood to founding site ⇒ `MaterialPickedUp` and `MaterialsDelivered` Features fire 4× each (one per Wood item) ⇒ founding `Stores` completes via the new pipeline.

**Concordance (with founding spawn ON):** Direction match — Features did fire (4 each). Magnitude match — exactly 4 hauls. Mixed survival outcome — initial spawn-on soak showed Starvation 0 → 5 and anxiety_interrupt 6259 → 25051. Drilling the death events surfaced an orthogonal pre-existing bug: a `ThreatNearby`-preempted `GoapPlan` set `current.action = Action::Flee` and marked the plan exhausted, but did not remove `GoapPlan`. The trip-completion path then replanned (since `trips_done < target_trips`), so `is_exhausted()` flipped back to false and the cat carried the same plan indefinitely. `Action::Flee` is set-and-forget — no resolver releases it — and `evaluate_and_plan` filters `Without<GoapPlan>`, so the dispositioner never re-ran on the locked cat. Frozen `last_scores` for 5000+ ticks confirmed the freeze; deaths landed at days 122 / 167 with hunger=0 and the cat still in Flee.

**Flee-lock fix landed in same PR:** `goap.rs:2206` preempt path now also pushes the cat into `plans_to_remove` so the GoapPlan is removed this tick and `evaluate_and_plan` picks the cat up next tick (replacing `Action::Flee` with whatever the new top disposition is — typically Resting/Hunting given low hunger). This is genuinely orthogonal to the materials-delivery work but un-blocks the founding-spawn flow that exposed it. Surfaced via the focal trace pattern: starvation deaths at day 122/167 (NOT early-game), all in `Action::Flee`, all with frozen disposition scores.

**Bonus side-effects of the spawn-on flow (with Flee-lock fix):** continuity tallies surged in iter4 — `courtship 0 → 1254`, `mythic-texture 22 → 43`, `grooming 19 → 599`, `play 109 → 1840`, `BondFormed` and `CourtshipInteraction` (previously never-fired-expected) now fire. The new haul cycle changes cat positioning / disposition routing in ways that surface previously-suppressed behaviors. Whether the magnitude is realistic or over-firing is for ticket 041 to characterize.

**Decision to land:** Infrastructure is solid (1344/1344 tests pass, all canaries pass with spawn off). The Flee-lock fix is a real bug fix worth landing on its own merits regardless of 038's outcome. Founding-spawn-on tuning to fully clear the canary is deferred to ticket 041 — bundling it would mix infrastructure landing with multi-iteration balance work. Spawn gated behind `CLOWDER_FOUNDING_HAUL=1`; activate via env var to reproduce or use.

**Verification:**
- `just check` clean (all-targets clippy + step-contract preamble + time-units).
- `just test` — 1344 lib tests pass, including new `pickup_material` and `deliver` resolver tests + new planner action tests `building_haul_then_construct` and `building_construct_short_circuit_when_materials_already_available`.
- Default soak (`CLOWDER_FOUNDING_HAUL` unset, `logs/tuned-42-038-final/`) — survival canaries pass (Starvation 1 in scheduler-noise band per CLAUDE.md, ShadowFoxAmbush 2/10, footer written, never_fired_expected_positives unchanged from baseline).
- Spawn-on soak (`logs/tuned-42-038-iter3/`) — canary fails, captured for ticket 039 reproducer.

**Deferred follow-ons:**

- Ticket 041 — Founding wagon-dismantling balance tuning. Activate `CLOWDER_FOUNDING_HAUL`, identify the load-bearing cause (Maslow gating? founding building choice? pile placement?), tune until canaries hold, promote `MaterialsDelivered` + `MaterialPickedUp` back to `expected_to_fire_per_soak() → true`.

**Diagnostic trail (kept as worked example):** the original ticket diagnosis was wrong; the verification step was reading the actual code path of `resolve_construct` and grepping for non-test `ConstructionSite::deliver` call sites. That found a single legacy-only call and a `new_prefunded`-everywhere world spawn. The lesson: don't trust a ticket's diagnosis as a starting axiom — read the code paths it cites and confirm. The investigation was 3 Explore-agent recon passes plus targeted Read of the planner core, which was enough to refactor the ticket's scope before writing any code.

---
