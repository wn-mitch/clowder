---
id: 428
title: HandoffItem goap-path resolver reads empty kitten_snapshot — substrate-stub silent fail
status: done
cluster: ai-substrate
orchestration: substrate-sensitive
initiative: [generational-continuity]
added: 2026-05-20
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: pending
landed-on: 2026-05-20
---

## Why

`HandoffItem`'s goap-path resolver at `src/systems/goap.rs:7322-7344` falls back to a nearest-hungry-kitten pick from `snaps.kitten_snapshot` when `target_entity.is_none()`. That `Vec` is **statically initialized to `Vec::new()` at `goap.rs:3825` and never written elsewhere** — `grep -rn "snaps\.kitten_snapshot\|\.kitten_snapshot\.push\|\.kitten_snapshot\s*=" src/` shows only the empty initializer plus two readers (FeedKitten at 6310, HandoffItem at 7323). FeedKitten no-ops gracefully on the empty snapshot; HandoffItem hard-fails with `"handoff: no recipient on disposition (no dependent cat in colony)"`. The canary fired 177,190 times in `logs/afk-overnight-2026-05-19` (6h soak, 655k ticks, 0.27/tick rate) — paired with 22 starvations, 20 of which were kittens at den-clusters (20–22, 19–25) and (29, 26). Sibling-not-parent to ticket 273 (Caretake election crowded out by perception ratchet, parked behind upstream perception fixes); this defect surfaces inside the rare Caretake-election regime 273 describes — when election does happen and the plan re-enters the goap-path with a cleared `target_entity` (via one of the eight `disposition.rs` clear sites: 1765, 3593, 3609, 3754, 3821, 3853, 3858, 3870), the resolver can't recover. Violates the substrate-over-hacks pillar — a cat committed to Caretake should not silently fail to find a recipient when one exists in range.

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| L1 markers | `src/components/markers.rs:615` (`HasDependentCat`) + `src/systems/buildings.rs:649` (writer) | Authored by `update_colony_building_markers` from `!kittens.is_empty()`; copied into `MarkerSnapshot` by goap.rs:1516 and disposition.rs:511. Reader: Caretake `EligibilityFilter::require(HasDependentCat::KEY)` at `caretake.rs:127` | `[verified-correct]` — marker fires when kittens exist; 410 closed the no-kittens-globally case |
| L2 DSE | `src/ai/dses/caretake.rs` | 3-axis WeightedSum (kitten_urgency, compassion, parental_engagement) + 4th dormant lift axis (colony_food_security). Eligibility gate intact. | `[verified-correct]` — Caretake scores positively when kitten exists; election crowding is 273's domain, not this ticket's |
| L3 softmax | `src/ai/scoring.rs` | Caretake elects rarely under perception-ratchet conditions (273 measured 0.24% share during a kitten lifespan), but does fire — afk-overnight-2026-05-19 shows ~6% of action snapshots are Caretake/Handoff colony-wide | `[verified-correct]` — election path is functional; rate problem is 273 |
| Action→Disposition mapping | `src/components/disposition.rs:281,286,365,371` | `Action::Caretake → Caretaking`; `Caretaking → [Caretake]`. `Action::Handoff → Handing`; `Handing → [Handoff]`. | `[verified-correct]` |
| Plan template | `src/systems/disposition.rs::resolve_disposition_chains` builds Caretake chains with HandoffItem step and seeded `target_entity` | Disposition-chain path seeds `step_state[idx].target_entity` correctly at build time | `[verified-correct]` — chains created via the disposition-chain path start in a recoverable state |
| **Target-entity clears (mid-plan)** | `src/systems/disposition.rs:1765, 3593, 3609, 3754, 3821, 3853, 3858, 3870` | Eight sites set `step.target_entity = None` mid-execution (target out of range, search-for-another paths, etc.) — these are the entry points to the goap-path fallback | `[verified-defect-precondition]` — these clears are legitimate; the resolver's recovery path is what fails |
| **Goap-path snapshot wiring** | `src/systems/goap.rs:3819-3825` | `Snapshots.kitten_snapshot: Vec<KittenState>` is initialized `Vec::new()`. Comment claims "built from the main cats query itself" but the assignment is empty; the docstring rotted from the code. Only writer in the entire src tree is this initializer. | **`[verified-defect]`** — substrate-stub class. Marker authored + eligibility filter present + DSE elects + plan emits HandoffItem + resolver reads from this snapshot, but the snapshot the consumer reads is never populated. Same class as 209 / 084. |
| Resolver | `src/systems/goap.rs:7309-7356` (`GoapActionKind::HandoffItem` arm) | When `target_entity.is_none()`, attempts to refresh from `snaps.kitten_snapshot.iter().min_by(...)`; on empty vec → None → hard-fails with the canary string. **No alternative roster source.** | `[verified-defect]` — fallback structurally cannot succeed |
| **HandoffPending drain** | `src/systems/goap.rs:4917-4934` | The deferred-handoff drain calls `cats.get_many_mut([actor, recipient])` to grab both `&mut Inventory` borrows. `cats` is filtered `With<GoapPlan>` (adults only) because the per-cat loop holds `&mut GoapPlan`. **Kittens don't carry `GoapPlan`**, so `get_many_mut` Errs on every kitten recipient and the drain `continue`s. The slot never lands; the parent's `Inventory` is never touched. | **`[verified-defect]`** — substrate over-filtering (sister-defect to the resolver-side stub). Surfaced during fix verification on 2026-05-20: the resolver-side fix alone is structurally net-negative (R2b in isolation creates a no-op Handing loop — `inventory_excess` stays high, adult re-elects Handing every tick, drain silently drops). Items-are-real / kittens-are-cats: the kitten's `Inventory` is a real slot; the transfer must physically land. |
| Kitten autoconsume | `src/systems/needs.rs:301-314` (`eat_from_inventory`) | Query `(&mut Needs, &mut Inventory), Without<Dead>` — **no `With<GoapPlan>` filter**, so iterates adults AND kittens. When `needs.hunger < eat_from_inventory_threshold`, `inventory.take_food()` consumes one food slot and boosts hunger. Already substrate-complete pre-§428 — the kitten side of the chain works. | `[verified-correct]` — kittens-are-cats pillar honored at the eating layer; §428's drain fix completes the upstream link so food actually reaches Inventory in the first place |
| Verification harness | `src/scenarios/parenting_handoff_recipient_resolution.rs` | Pre-injects a `Handing` plan with `HandoffItem` step + `target_entity = None`, hungry kitten co-located, parent has food in inventory. Runs 2 FixedUpdate ticks (resolver fires, drain transfers, `eat_from_inventory` consumes within the same Update). Currently RED with `kitten_hunger=0.0499` (resolver fell through to canary); flips GREEN end-to-end after R2b + drain fix. | `[verified-defect-reproduces]` — added by this ticket as the failing-then-passing harness |

## Fix candidates

**Parameter-level options:**
- R1 — None applicable. This is a substrate-stub class defect, not a scoring / threshold / weight question. Listed only to make the absence explicit.

**Structural options:**

- R2 (**rebind**, RECOMMENDED) — Read from a populated kitten roster instead of the empty `snaps.kitten_snapshot`. The function `build_dependent_kitten_snapshot` already exists at `goap.rs:7666-7691`, builds from `ec.kitten_parentage` query, returns `Vec<DependentKittenState>`. Either:
  - **R2a** — change the resolver to read from `kittens` (the result of `build_dependent_kitten_snapshot`) when `kitten_snapshot` is empty, OR
  - **R2b** — populate `Snapshots.kitten_snapshot` itself from `ec.kitten_parentage` at construction (lines 3819-3825). Same data source, narrower diff at the resolver. Mirrors the existing pattern for `dead_cat_positions` / `cat_skills` / `cat_temperature` (lines 3793-3813) which all build read-only snapshots from queries disjoint from the outer mutable cats iteration. The `&mut Needs` conflict the rotted comment cites is misdirected — `ec.kitten_parentage` is `Without<GoapPlan>` so kittens already don't appear in the outer cats iteration with `&mut Needs`.

- R3 (**split**) — Give the goap-path `HandoffItem` resolver its own graceful-fallback shape (`DropAtNursery` step that drops food on the ground for the kitten to scavenge). Keeps the umbrella; the failure mode becomes "food placed where kittens can reach it" rather than "step fails, plan abandons." Lower priority than R2 — R2 actually fixes the resolver; R3 is defense-in-depth.

- R4 (**extend**) — Author a "HandoffItem onto goap-path" branch that pulls the recipient roster from `ec.kitten_parentage` directly inline (no Snapshots refactor). Equivalent in behavior to R2a but uglier — duplicates the populate logic at the read-site rather than centralizing it. Listed for completeness; R2 is cleaner.

- R5 (**retire**) — N/A. The goap-path resolver is load-bearing; it runs every time the disposition-chain re-enters with a cleared target. Retiring it would force every Caretake chain to abandon on the first target clear, which is strictly worse.

## Recommended direction

**R2b + drain rebind, landed together.** The two diffs are inseparable: R2b in isolation makes the canary go quiet but produces a no-op Handing loop (drain silently drops, adult's `inventory_excess` stays high, Handing re-elects every tick). Both fixes are required for the substrate-over-hacks pillar to hold.

**Diff 1 — R2b populate** at `src/systems/goap.rs:3819-3825`. Replace `Vec::new()` with a `KittenState` build that reads entity/pos/parentage from `ec.kitten_parentage` and looks up hunger via `ec.kitten_needs.get(entity)` (immutable accessor on the mut query). The "intentionally empty / avoid `&mut Needs` conflict" comment was misdirected: `kitten_parentage` is `Without<GoapPlan>`, disjoint from the cats query's `&mut Needs`. Mirror of the existing disposition-chain populate at `goap.rs:1473-1485`.

**Diff 2 — drain rebind** at `src/systems/goap.rs:4917-4986`. Add `pub kitten_inventory_q: Query<&mut Inventory, (Without<GoapPlan>, Without<Dead>, Without<Structure>)>` to `ExecutorContext` (mirror of existing `kitten_needs`). At the drain, route kitten recipients through it when `cats.get_many_mut` Errs:
- Adult→adult: `cats.get_many_mut([actor, recipient])` (existing).
- Kitten recipient: `cats.get_mut(actor)` + `ec.kitten_inventory_q.get_mut(recipient)`; both queries are statically disjoint by `With<GoapPlan>` vs `Without<GoapPlan>`, so Bevy permits the concurrent mut borrows.

Both branches call `resolve_handoff(actor_inv, recipient, recipient_inv)` — the transfer mechanic is identical; only the source of the kitten's `&mut Inventory` differs.

**The chain is complete.** Pre-existing `eat_from_inventory` (`src/systems/needs.rs:301`) iterates over all cats including kittens (no `GoapPlan` filter); the kitten consumes the slot and hunger rises on the same Update. R2b populates → resolver finds → drain transfers → autoconsume eats → hunger rises. The kitten-side substrate was already in place; §428 closes the upstream gap.

R3 (DropAtNursery defense-in-depth) is worth opening as a separate follow-on ticket but not landing here. The combined R2b + drain fix should drop the 177k canary count to near-zero on its own AND actually feed kittens.

## Out of scope

- **Caretake election rate** (273's domain — parked behind perception fixes 282 / 283 / 219 / 234 / 243 / 244 / 233). This ticket does not raise Caretake's L2 score, soften Patrol, or alter the softmax. The fix is purely the resolver-side recovery path.
- **DropAtNursery defense-in-depth** (R3) — open as a follow-on if R2's landing doesn't drive the 177k canary count to near-zero. The substrate work for "drop food on ground at nursery for kittens to scavenge" is non-trivial (path-walk to nursery, drop-on-tile mechanics, kitten-side `PickUp` from ground). Park behind verification of R2's impact.
- **Per-cat recipient picker** (ticket 192 — balance follow-on for multi-axis HandoffItem target selection). 192 is about *which* kitten to feed when several are hungry; this ticket is about *finding any* kitten when the original target cleared. Different layer, different ticket.
- **The `&mut Needs` borrow-checker comment** at `goap.rs:3819-3825` should be deleted along with the empty `Vec::new()` initializer — keeping it as docrot is worse than removing it. Include in the same commit as R2.
- **Source/Transfer/Sink contract for `eat_from_inventory`** — surfaced 2026-05-20 during fix verification: the kitten autoconsume path that completes §428's chain (`src/systems/needs.rs:301`) is a pre-substrate-era reflex that mutates Inventory outside any named gate. Works behaviorally, but violates the items-are-real contract. Tracked in **ticket 429** as the items-are-real Source/Transfer/Sink formalization. Park behind §428's landing; this ticket does not promote the autoconsume system, it relies on the existing one.

## Verification

**Scenario gate (immediate):** `cargo test --lib --release parenting_handoff_recipient_resolution` — the test `goap_path_resolver_finds_live_kitten_with_none_target` flips from RED to GREEN. Asserts both (a) kitten hunger > 0.05 (post-handoff feeding occurred) and (b) parent inventory drained to 0 slots (food actually transferred).

**Soak gate (full-run):** `just soak 42` followed by `just verdict logs/tuned-42`. Expected:
- `plan_failures_by_reason.HandoffItem: no recipient on disposition (no dependent cat in colony)` — drops from baseline 177k-class count to **near zero**. Some residual is expected (genuine race conditions where the original kitten target despawns between L2 eligibility sampling and resolver dispatch in the same tick), but the rate should fall by >99%.
- `deaths_by_cause.Starvation` — should drop, but **not necessarily to zero**. 273's perception ratchet still keeps Caretake rare; reducing the wasted-handoff rate frees Caretake cycles to be more productive, but the underlying election problem remains parked. Expected delta is partial improvement, not full closure.
- Continuity canaries (grooming, play, mentoring, burial, courtship, mythic-texture) must remain ≥ 1 each.

**No frame-diff requirement** — this is a substrate-stub fix, not a scoring change. The L1/L2/L3 trace shape should not shift. If `just frame-diff` shows any per-DSE drift, that's a signal R2b had an unintended side-effect (e.g., the new `kitten_snapshot` data is read by FeedKitten in a way that changes its targeting). Investigate before landing.

## Log

- 2026-05-20: opened. Sibling to 273 (parked, perception ratchet). Layer-walk surfaced the substrate-stub defect at `goap.rs:3825` during the afk-overnight-2026-05-19 soak audit. Scenario `parenting_handoff_recipient_resolution` ships RED in the same commit as the ticket, GREEN-when-R2-lands gate.
- 2026-05-20: scope expanded during fix verification. R2b alone passed compile + populated the snapshot correctly, but the test still failed — kitten Inventory stayed empty. Tracing revealed a sister-defect at the `HandoffPending` drain (`goap.rs:4917`): `cats.get_many_mut` over-filters by `With<GoapPlan>`, silently dropping every kitten-recipient transfer. Items-are-real + kittens-are-cats pillars apply: kitten `Inventory` is a real slot; the transfer must physically land. Added `ec.kitten_inventory_q` (mirror of `kitten_needs`) + a kitten-recipient branch to the drain. The existing `eat_from_inventory` at `systems/needs.rs:301` already iterates kittens — once the slot lands in kitten Inventory, autoconsume completes the chain. Test now passes end-to-end; soak verification next.
- 2026-05-20: soak 42 vs pre-428: HandoffItem canary 309 → 0 (>99% reduction, as predicted); zero deaths both runs; continuity canaries hold (courtship 1492 / grooming 1396 / mentoring 234 / play 3); Feature emissions (ItemHandedOff, KittenFed) track tick-count proportionally — no per-DSE shape drift. Follow-on 429 opened for items-are-real Source/Transfer/Sink contract formalization (out of scope here).
