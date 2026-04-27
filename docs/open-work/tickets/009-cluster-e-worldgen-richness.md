---
id: 009
title: World-generation richness (Cluster E)
status: ready
cluster: E
added: 2026-04-20
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, refactor-plan.md, fate.md, narrative.md]
related-balance: []
landed-at: null
landed-on: null
---

**Why this is a cluster:** Clowder currently starts every game at t=0
with fresh cats — no lineage, named past, seeded `ColonyKnowledge`, or
historical bonds. Emergent narrative is therefore a pure
forward-product. Talk of the Town and Dwarf Fortress both fix this
with the same architectural move: run the sim loop itself for
generations before the player arrives.

### E1. Pre-simulation history via same-loop fast-forward

**Why it matters:** ToT (*Game AI Pro 3* § 37.2.2) runs 140 sim-years
(1839–1979) using utility-based action selection (Mark 2009 — the
same substrate A1 refactors toward) and produces 300–500 NPCs with
full lineages, residences, daily routines, work networks, and
asymmetric unidirectional affinities as the *output* of that
fast-forward. There is no separate procedural-history algorithm;
history is what the runtime loop produces when you run it longer than
the player sees.

This is architecturally cheap: no new procedural system. It is
architecturally profound: the resulting past has the same causal
density as the present, because it *was* the present, a few generations
ago.

**Clowder analogue:** fast-forward ~3–5 cat generations (~15–30
sim-years) before the first player-visible tick. Output state at t=0:
- Starting cats have known parents, grandparents, great-grandparents
  referenced by name/lineage even when only the living cats are
  active entities
- `ColonyKnowledge` pre-seeded with named events produced during
  fast-forward ("the Long Winter of year 12," "the Fox that took
  Silverpaw")
- Asymmetric bonds already exist between starting cats because their
  ancestors' relationships propagated
- Starting cats carry implanted mental models (see C3) covering kin,
  territory, known predators, and colony history
- `fate.rs` carries prophecies/visions rooted in generated history
- `narrative.rs` templates can reference historical figures and events
  from tick 1

**Two-phase architecture (following § 37.2.2 + § 37.3.10):**

*Phase 1: fast-forward without knowledge phenomena.* Run the sim loop
for ~15–30 sim-years with all C3 belief machinery disabled. Cats still
interact, practices still run (C2), relationships still form, cats
still die and give birth, but no mental-model tracking, no belief
mutation, no gossip as knowledge propagation. Output: ground-truth
event log, lineage tree, surviving cat roster with relationships,
dens/territory/named features.

*Phase 2: knowledge implantation at the boundary.* Once fast-forward
terminates, run the implantation procedure: for each surviving cat,
populate mental models over their kin, closest bonds, territory, and
a probabilistically-selected subset of other entities weighted by
salience (§ 37.3.10, Listing 37.1 gives the pseudocode). All implanted
beliefs are accurate at t=0; divergence emerges once runtime knowledge
phenomena activate.

*Phase 3: runtime with knowledge.* From t=0 onward, full C3 machinery
is active: observation, transference, confabulation, mutation,
declaration-reinforcement, belief-revision, forgetting.

Phase 1 is cheap (sim loop minus rendering, maybe coarser ticks).
Phase 2 is a one-time bulk-insert. Phase 3 is the runtime system and
pays the ongoing cost.

**Current state:** `src/main.rs` spawn phase creates cats at t=0 with
fresh stats; `Memory.events` is empty; `ColonyKnowledge.entries` is
empty; no lineage depth.

**Touch points:**
- `src/main.rs` spawn phase — new pre-sim fast-forward phase
- `build_schedule()` in `src/main.rs` + `SimulationPlugin::build()` in
  `src/plugins/simulation.rs` — both need a "history-gen" mode that
  can run the sim loop without rendering, narrative emission (or at a
  tier filter), or per-tick diagnostic capture
- `src/resources/colony_knowledge.rs` — persist state across the
  fast-forward → runtime transition
- `src/components/mental.rs` — support inherited memories referencing
  ancestors
- `src/systems/social.rs` — asymmetric affinities survive the
  transition
- `src/systems/fate.rs` — seed prophecies from generated history
- `src/systems/narrative.rs` — templates reference historical state

**Performance sub-concern:** current headless runs ~15 min wall per
~10 sim-days. Fast-forward of 15 sim-years at that throughput ≈ 8
hours wall — too slow. A history-gen mode needs 10–100× throughput,
achievable by (a) skipping rendering + detailed sensing, (b) filtering
narrative emission to Significant tier only, (c) disabling per-axis
diagnostic capture, (d) possibly coarser tick resolution during
fast-forward. Benchmark and profile before scoping.

**Preparation reading:**
- **Ryan et al. *Game AI Pro 3* ch. 37 § 37.2.2 "World Generation"**
  — the 140-year loop architecture; "world-gen is the sim loop, run
  longer"
- **§ 37.3.10 "Knowledge Implantation"** (same chapter, Listing 37.1)
  — the bridge procedure from fast-forward to runtime knowledge
- **Ryan, Mateas, Wardrip-Fruin (2016). "A simple method for evolving
  large character social networks."** Proc. 5th Workshop on Social
  Believability in Games — "Ryan 16c"; the *how* to § 37.2.2's *what*
- **Tarn Adams (2015). "Simulation principles from Dwarf Fortress."**
  *Game AI Pro 2* pp. 519–522, CRC Press — DF's stance on
  world-generation-as-simulation; same intellectual tradition
- Tarn Adams + Tanya Short, GDC 2016 "Procedural Narrative and
  Mythology" — DF's version at larger scale
- James Ryan, *Curating Simulated Storyworlds* ch. 4–5 — long-form
  treatment
- Neighborly `neighborly/simulation/` — concrete ECS implementation
  of fast-forward over a ToT-like model
  <https://github.com/ShiJbey/neighborly>

**Exit criterion:** at the first player-visible tick, `ColonyKnowledge`
is non-empty, starting cats have ≥2-generation lineage referenceable
by name, ≥1 seeded historical event appears in a narrative line in
the first sim-week, and the "named events per sim year" continuity
canary passes from t=0 forward without relying on live-sim events.

**Dependency:**
- **Gated on A1 (IAUS refactor).** Running 15+ sim-years of
  linear-additive scoring bakes scoring pathologies into "canonical
  history" and produces an incoherent past.
- **Gated on performance work.** History-gen mode must run fast
  enough to be tolerable at build time; spec before implementation.
- Pairs with **C2 (Versu practices)** — courtship and mentoring
  practices during fast-forward produce the relationship graph at
  t=0.
- Pairs with **C3 (subjective knowledge)** — generated history is
  ground truth; per-cat beliefs are subjective views. Different
  starting cats know different versions of the generated past.
- Pairs with **C4 (strategist-coordinator)** — leadership patterns
  during fast-forward produce the current coordinator *and* the
  dynastic backstory explaining why they lead.

## Log

- 2026-04-27: dropped blocked-by 005 — cluster-A umbrella retired; A1 dependency satisfied by landed work. Status flipped blocked → ready.
