---
id: 049
title: §9.2 faction overlay markers
status: done
cluster: null
landed-at: 384bf25
landed-on: 2026-04-27
---

# §9.2 faction overlay markers

**Landed:** 2026-04-27 | **Commit slate:** `384bf25` (1/5 KEY + TargetTakingDse::with_stance) · `56e6b3e` (2/5 filter_candidates_by_stance helper) · `396c0fd` (3/5 §9.3 binding into 3 cat target-taking DSEs + fox_raiding gate) · `1d254ff` (4/5 cat-on-cat banishment + Banished snapshot pop) · `fdc9cc0` (5/5 befriend_wildlife author + Visitor shim) · `3352366` (test: per-resolver prefilter integration tests)

**Why:** Ticket 014's §4 marker catalog large-fill (landed earlier today) closed all §4.3 markers except the four §9.2 faction overlay ZSTs (Visitor / HostileVisitor / Banished / BefriendedAlly). These required the §9.3 DSE filter binding plumbing — `resolve_stance` and `FactionRelations` were spec-built and well-tested in isolation but never called from production code; every target-taking DSE filtered candidates by distance only. Ticket 049 closes the §9.3 loop end-to-end.

**What landed:**

- **Marker `KEY` consts on all four §9.2 ZSTs.** Mirrors the §4.3 `HasCubs::KEY` pattern.
- **`TargetTakingDse::with_stance` builder + `Option<StanceRequirement>` field.** §9.3 stance requirements migrate from cat-action DSEs (where they were metadata-only since `passes_eligibility` doesn't enforce stance — see `eval.rs:404`) to their target-taking siblings. Migrated rows: `socialize_target` (Same|Ally), `fight_target` (Enemy|Prey), `hunt_target` (Prey). `flee.rs` and `fox_raiding.rs` keep their declarations (no target-taking sibling).
- **`filter_candidates_by_stance` helper** in `src/ai/faction.rs:243` — a free function callers invoke immediately before `evaluate_target_taking` to drop candidates whose resolved stance fails the requirement. Pure: caller supplies `species_of` and `overlays_of` closures.
- **§9.3 binding wired into 3 cat target-taking resolvers** (socialize / fight / hunt) and the fox_raiding colony-level gate. `ExecutorContext` (`goap.rs:260`) gained `faction_relations: Res<FactionRelations>` and a `faction_overlay_q` query for the four §9.2 markers, plus a `stance_overlays_of(e)` helper. The 8-line prefilter block at each callsite is identical across resolvers.
- **fox_raiding suppression on BefriendedAlly.** A fox carrying `BefriendedAlly` zeroes its raid drive (per-fox coarsening of the spec's per-pair befriending — flagged risk #1 in the plan).
- **Cat-on-cat `Banished` insert** in `combat.rs::resolve_combat`'s `pending_banishments` resolution loop. Branches on `cats.get(target).is_ok()`: if cat, insert `Banished` and continue (skipping the shadowfox-only despawn + corruption pushback + posse/witness boons). Today's only `pending_banishments` producer is the shadowfox HP/posse path, so the cat branch lights up only when a future system pushes a cat in (per "minimal real triggers" scope choice).
- **`MarkerSnapshot` mirror** for the four §9.2 markers in `goap.rs::evaluate_and_plan` per-cat loop. Bundled into a new `TargetMarkerQueries` SystemParam-derive struct (with the existing target-existence query) to keep `evaluate_and_plan` under Bevy's 16-param limit (was 17 → now 16).
- **`social::befriend_wildlife` author** — toggles `BefriendedAlly` on cat + wildlife pairs whose cross-species familiarity crosses `SocialConstants::befriend_familiarity_threshold` (default 0.6) with a 0.1 hysteresis band. No production system writes cat ↔ wildlife familiarity today, so the author runs as a no-op until trade or a non-hostile-contact accumulator lands (flagged risk #2).
- **`systems/visitors.rs`** carrying a `#[cfg(test)]` `spawn_visitor_cat` helper. The trade subsystem (Aspirational) is the production source — these markers are authoritative-on-arrival, not derived state, so no per-tick author system.
- **34 new tests:** 6 in `faction::filter_candidates_by_stance`, 7 in `social::befriend_wildlife`, 6 in `visitors`, 2 in `combat` (Banished marker persistence + resolve_stance integration), 3 per-resolver prefilter tests (one each for socialize/fight/hunt), 1 KEY-uniqueness, 1 fox_raiding suppression, plus the migrated stance-requirement tests on each target-taking sibling.

**Test-side change:** Two `fight_target` aggregation tests using `WildSpecies::Fox` candidates switched to `WildSpecies::ShadowFox` — `Cat→Fox = Predator` fails the §9.3 Enemy|Prey requirement (canonical matrix at `faction.rs:171`); ShadowFox = Enemy preserves the test intent (distance × threat scoring) while passing the prefilter.

**Verification (post-049 soak `logs/tuned-42/`, commit `3352366`, seed 42, --duration 900):**
- Survival canaries: **pass.** Starvation 0 (target == 0), ShadowFoxAmbush 0 (target ≤ 10), footer written, `never_fired_expected_positives = 3` (`MatingOccurred` / `GroomedOther` / `MentoredCat`) — pre-existing; same set in `logs/tuned-42-fcd13bd` (pre-049) and a strict subset of the registered baseline's 8.
- Continuity canaries: `mentoring=0`, `burial=0` — **pre-existing.** Same in baseline + pre-049 soak. Tracked in tickets 027 / 056 / downstream.
- Drift vs `logs/tuned-42-fcd13bd` (pre-049, commit `fcd13bd`, same seed/duration): deaths 8 → 2 (Starvation 1 → 0, ShadowFoxAmbush 3 → 0, WildlifeCombat 4 → 2); continuity courtship 0 → 1014, grooming 30 → 215, play 368 → 663, mythic-texture 3 → 23 — all positive direction; `anxiety_interrupt_total` 312 → 5436 (17×) is the one suspicious metric — needs separate investigation.
- Lib tests: 1424 → 1458 (+34).

**Hypothesis (per CLAUDE.md balance methodology):** §9.3 candidate-stance prefiltering changes which targets the disposition planner commits to per tick — particularly for `socialize_target` (where `Cat → Cat` resolves through the matrix path now, vs the prior distance-only filter) and `hunt_target` (where prey candidates flow through `from_sensory(SensorySpecies::Prey(...))` → `Cat → Prey = Prey` → kept). Direction: more eligible socialize targets → more `CourtshipInteraction` / `GroomingInteraction` / `play` events. Magnitude: 7–17× on continuity tallies. Concordance: shipping is positive-direction; the anxiety-interrupt 17× spike is *not* explained by the hypothesis and warrants a follow-on investigation (`just q events logs/tuned-42 --type=AnxietyInterrupt`-style drilldown). Survival gates and `Starvation == 0` / `ShadowFoxAmbush ≤ 10` invariants hold, so this lands.

**Out of scope (flagged risks → follow-ons):**
- **fox_raiding per-pair model.** The current per-fox `BefriendedAlly` gate suppresses raids on *all* colonies once the fox is befriended by *any* colony. Spec is per-pair; coarsened here per ticket plan D5.
- **Cat ↔ wildlife familiarity source.** No production system writes cat ↔ wildlife familiarity today. The `befriend_wildlife` author is correctly authored-on-threshold but unfeed in production until a non-hostile-contact accumulator (or trade) lands.
- **§4.3 sensing.rs `HasSocialTarget` overlay refinement.** `update_target_existence_markers` calls `resolve_socialize_target` with a no-op overlay closure (a banished cat is still counted as a "social target exists" for purposes of marker authoring). Harmless because the actual stance prefilter at the `goap.rs::dispatch_step_action` callsite uses real overlays. Refinement is a follow-on (see in-source comment at `sensing.rs:847`).
- **Anxiety-interrupt 17× drift.** Surfaced post-049 against pre-049 baseline. Likely benign (cats interrupting more = more agency under stance-aware target selection) but unconfirmed.

**Successor opportunities:**
- Per-pair fox-befriending data model.
- Cat ↔ wildlife familiarity write source.
- Trade subsystem `arrive_visitor` / `depart_visitor` author (replaces the `#[cfg(test)]` `visitors.rs` shim).
- `update_target_existence_markers` overlay closure refinement.
- Anxiety-interrupt 17× drift root-cause investigation.

---
