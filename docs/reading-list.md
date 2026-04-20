# Reading list

The intellectual backlog that feeds the work queue in
`docs/open-work.md`. Where `docs/systems/project-vision.md` is the
*design thesis* (what kind of game Clowder is) and
`docs/open-work.md` is the *task queue* (what's next to build), this
file is the *reading queue* — the books, papers, talks, and code
references that prepare future-self to pick up each open-work entry.

Every substantive open-work entry carries a **Preparation reading**
section listing a focused subset of this file (3–5 items, typically).
If a task's prep list grows beyond ~5 items, that's a signal the task
is too big and should split.

## Update convention

Append freely. Keep entries tight (one-line relevance hook + a
cross-reference to an open-work entry where applicable). Use
`[watched]` or `[read]` to mark completion inline. When something is
clearly no longer relevant to Clowder, move it to **Not recommended**
rather than deleting — prevents future-self from re-evaluating.

---

## Completed

Already consumed, kept as a bibliography so future-self knows what
prompted what.

- **"Winding Road Ahead: Designing Utility AI with Curvature"** —
  Dave Mark, GDC 2018.
  <https://www.youtube.com/watch?v=TCf1GdRrerw> — prompted the
  realization that `src/ai/scoring.rs` is a partial IAUS
  implementation. Foundational for A1.
- **"Building a Better Centaur: AI at Massive Scale"** — Dave Mark,
  GDC 2015. <https://archive.org/details/GDC2015Mark> — established
  the utility × influence-map fusion that cluster A + B organize
  around. Foundational for A4 (target scoring) and B1 (influence
  maps).

---

## On deck

Next items to consume, ordered roughly by relevance to open-work
priority.

1. **"Modular Tactical Influence Maps"** — Dave Mark, *Game AI Pro 2*
   ch. 30 (free PDF at
   <http://www.gameaipro.com/GameAIPro2/GameAIPro2_Chapter30_Modular_Tactical_Influence_Maps.pdf>).
   Prepares **B1**.
2. **"Embracing the Dark Art of Mathematical Modeling in AI"** —
   Dave Mark, GDC 2013 (GDC Vault). Deepens **A1** curve work.
3. **Rao & Georgeff (1991), "Modeling Rational Agents within a
   BDI-Architecture"** (KR 1991; free PDF via Google Scholar).
   Prepares **C1**.
4. **Ryan et al. (2017), "Simulating Character Knowledge Phenomena
   in Talk of the Town"** — *Game AI Pro 3* ch. 37 (free PDF at
   <https://www.gameaipro.com/GameAIPro3/GameAIPro3_Chapter37_Simulating_Character_Knowledge_Phenomena_in_Talk_of_the_Town.pdf>).
   Prepares **C3** (§ 37.3 full) and **E1** (§ 37.2.2 + § 37.3.10).
   Load-bearing for two of the biggest cluster entries; don't skim.
5. **Ryan et al. (2015), "Toward characters who observe, tell,
   misremember, and lie."** (Proc. 2nd Workshop on Experimental AI
   in Games). Dense companion to § 37.3. Prepares **C3**.
6. **James Ryan, *Curating Simulated Storyworlds*** (UCSC 2018
   dissertation) — via Emily Short's two-part review
   (<https://emshort.blog/2019/05/21/curating-simulated-storyworlds-james-ryan/>)
   as on-ramp. Prepares **C2, C3, E1**.

---

## Utility AI & scoring (IAUS)

Cross-ref **open-work A1, A2, A3, A4**.

- Dave Mark, *Behavioral Mathematics for Game AI* (Cengage, 2009) —
  canonical IAUS text. Ch. 9–12 cover response curves and
  multi-consideration composition. Ch. 13 covers target selection.
  **A1, A4.**
- **"Winding Road Ahead: Designing Utility AI with Curvature"** —
  GDC 2018 [watched]. Curves applied to `scoring.rs`. **A1.**
- **"Building a Better Centaur: AI at Massive Scale"** — GDC 2015
  [watched]. Utility × influence maps at scale. **A4, B1.**
- **"Embracing the Dark Art of Mathematical Modeling in AI"** — GDC
  2013. Deeper curve treatment. **A1.**
- **"Architecture Tricks: Managing Behaviors in Time, Space, and
  Depth"** — Dave Mark + Kevin Dill, GDC 2012. Context tags as DSE
  filters. **A3.**
- **"Behavior is Brittle: Testing Game AI"** — Dave Mark et al., GDC
  2017. <https://www.youtube.com/watch?v=RO2CKsl2OmI> — introspection
  / telemetry framing. Connects to
  `docs/balance/scoring-layer-second-order.md` framing #2.
- *Game AI Pro* IAUS chapters (Mark's contributions in vols. 1 and
  2, free PDFs at <http://www.gameaipro.com/>) — published curve
  primitives + formulas in canonical form.
- Ian Millington, *AI for Games* (3rd ed., CRC Press) — ch. on
  decision-making / utility is the reference for invariant
  preservation during refactor. Ch. on debug/telemetry argues
  introspection tooling is load-bearing architecture, not overhead
  (connects to `scoring-layer-second-order.md` framing #2).

---

## Deliberative / long-horizon architectures

Cross-ref **open-work C1, C2, C3, C4**.

- **Rao & Georgeff (1991), "Modeling Rational Agents within a
  BDI-Architecture"** — KR 1991. Canonical BDI paper. Short, formal,
  readable. **C1.**
- **Michael Wooldridge, *An Introduction to MultiAgent Systems***
  (2nd ed., Wiley 2009) ch. 4 "Practical Reasoning Agents" —
  textbook BDI with examples. **C1.**
- **Jeff Orkin, "Three States and a Plan: The AI of F.E.A.R."** —
  GDC 2006. <https://alumni.media.mit.edu/~jorkin/goap.html>.
  Commitment and plan persistence in practical game AI. **C1.**
- **Richard Evans & Emily Short, "Versu — A Simulationist
  Storytelling System"** — IEEE TCIAIG, 2014. Canonical Versu paper;
  defines practices, roles, stages, invariants. **C2.**
- **Richard Evans + Emily Short, "The Sims 3 / Imagination Engines"**
  — GDC 2011. Shorter on-ramp; predecessor ideas at production scale.
  **C2.**
- **Emily Short's two-part review of Ryan's dissertation** —
  <https://emshort.blog/2019/05/21/curating-simulated-storyworlds-james-ryan/>.
  Connects ToT practices to Versu directly. **C2, C3.**
- **Dana Nau et al., "SHOP2: An HTN Planning System"** — JAIR 2003.
  Canonical HTN reference; free PDF via Google Scholar. **C4.**
- **Ryan, Summerville, Mateas, Wardrip-Fruin (2017), "Simulating
  Character Knowledge Phenomena in Talk of the Town"** — *Game AI
  Pro 3* ch. 37. The architectural blueprint for C3. § 37.2.2 is the
  blueprint for E1's world-gen phase. § 37.3.10 is the bridge between
  them. **C3, E1.**
- **Shi Johnson-Bey, *Neighborly*** — ECS Python, archived 2026-04-07.
  <https://github.com/ShiJbey/neighborly> + IEEE CoG 2022 paper.
  Stable ToT-descendant reference implementation. **C3, E1.**

---

## Influence maps & spatial reasoning

Cross-ref **open-work B1**.

- **"Modular Tactical Influence Maps"** — Dave Mark, *Game AI Pro 2*
  ch. 30 (free PDF). Definitive written reference.
- **"Lay of the Land: Smarter AI Through Influence Maps"** — Dave
  Mark, GDC 2014 (GDC Vault). Original pure-influence-maps talk.
- **"Spatial Knowledge Representation through Modular Scalable
  Influence Maps"** — Dave Mark, GDC 2018 (GDC Vault). Most recent
  full treatment with implementation details.
- **"Building a Better Centaur"** — GDC 2015 [watched]. Utility ×
  influence map fusion.
- **Nick Mercer, InfluenceMap** (Unity, open source) —
  <https://github.com/NickMercer/InfluenceMap>. Code reference for
  data structures and propagation.

---

## Emergent narrative & storyworlds

Cross-ref **open-work C2, C3, E1**.

- **Ryan, Summerville, Mateas, Wardrip-Fruin (2017), "Simulating
  Character Knowledge Phenomena in Talk of the Town"** — *Game AI
  Pro 3* ch. 37 (free PDF). The architectural blueprint for C3 and
  E1. **C3, E1.**
- **Ryan, Summerville, Mateas, Wardrip-Fruin (2015), "Toward
  characters who observe, tell, misremember, and lie"** — Proc. 2nd
  Workshop on Experimental AI in Games. Earlier, denser treatment.
  **C3.**
- **Ryan, Mateas, Wardrip-Fruin (2016), "A simple method for evolving
  large character social networks"** — Proc. 5th Workshop on Social
  Believability in Games, Abertay University. "Ryan 16c" — the
  algorithm for 300–500 NPC relationship graphs from utility-driven
  fast-forward. **E1.**
- **Ryan, Mateas, Wardrip-Fruin (2016), "Characters who speak their
  minds: Dialogue generation in Talk of the Town"** — AIIDE 2016.
  "Ryan 16d" — mental-model facets feeding narrative dialogue.
  Relevant once `narrative.rs` templates quote beliefs.
- **James Ryan, *Curating Simulated Storyworlds*** (UCSC 2018
  dissertation). Book-length treatment. eScholarship PDF at
  <https://escholarship.org/uc/item/1340j5h2>. Emily Short's two-part
  review is the on-ramp. **C2, C3, E1.**
- **Shi Johnson-Bey, *Neighborly***
  (<https://github.com/ShiJbey/neighborly>, archived 2026-04-07) +
  IEEE CoG 2022 paper. ECS-based ToT descendant, stable reference.
  **C3, E1.**
- **Tarn Adams (2015), "Simulation principles from Dwarf Fortress"**
  — *Game AI Pro 2* (Rabin, ed.), pp. 519–522. Short,
  authoritative. **E1.**
- **Tarn Adams + Tanya Short, GDC 2016 "Procedural Narrative and
  Mythology"** — DF's version of the world-gen-is-sim-loop move at
  larger scale. Maps directly onto `project-vision.md`'s stance on
  mythology/corruption/fate as ecology. **E1.**
- **Roguelike Celebration 2016, "Dwarf Fortress Design Inspirations"**
  — Tarn + Zach Adams. <https://www.youtube.com/watch?v=ZMRsScwdPcE>
  Foundational "how this family of games thinks."
- **DF talks YouTube playlist**
  <https://www.youtube.com/playlist?list=PLMNJXDpd1LHlfkDxQEr6iIk3fPo57gZmJ>
  — aggregator.
- **Tarn Adams, "Emergent Narrative in Dwarf Fortress"** — ch. 15 of
  *Procedural Storytelling in Game Design* (CRC Press 2018).
- **"Characterization and Emergent Narrative in Dwarf Fortress"** —
  ResearchGate 2021. Third-party academic analysis; mirror on DF's
  design.
- **James Ryan, Bad News papers** — "Ryan 16b" and follow-ons. The
  award-winning mixed-reality performance built on the same knowledge
  architecture. Secondary reading.
- **Damián Isla, "Third Eye Crime: Building a stealth game around
  occupancy maps"** — AIIDE 2013. Simpler cousin of the
  "what-does-the-agent-know" concern. **C3 calibration.**

---

## Agent-based modeling & systemic design

Broader systemic-design lineage, less directly task-driven.

- **Joshua Epstein & Robert Axtell, *Growing Artificial Societies:
  Social Science from the Bottom Up*** (MIT Press / Brookings, 1996)
  — foundational ABM text; Sugarscape shows CA-style spread + emergent
  culture/trade/disease from agent-level rules. Direct ancestor of
  Clowder's "honest ecology, no director" stance. **D1 calibration.**
- **Thomas Schelling, *Micromotives and Macrobehavior*** (W. W.
  Norton, 1978) — why simple agent rules produce social phenomena.
  Second tier; readable on-ramp to the ABM tradition.

---

## Bevy / Rust ecosystem

Cross-ref **open-work A2**.

- **big-brain** (Kat Marchán) — <https://github.com/zkat/big-brain> +
  <https://docs.rs/big-brain/latest/big_brain/>. Utility AI library
  for Bevy. Scorer/Action/Picker abstractions; composition primitives
  (`WinningScorer`, `ProductOfScorers`, `MeasuredScorer`). Not full
  IAUS out of the box; **A2** is the investigation entry.
- **Bevy official docs** — <https://bevy.org/learn/> — Components /
  Bundles / Tags chapter for **A3**.

---

## Reference texts

- **Ian Millington, *AI for Games*** (3rd ed., CRC Press). General
  reference; debug/telemetry chapter connects to
  `scoring-layer-second-order.md` framing #2. Utility-AI chapter is
  the reference for invariant preservation during A1.
- **Stephen Wolfram, *A New Kind of Science*** — free online at
  <https://www.wolframscience.com/nks/>. Ch. 2–3 (skim) for CA
  classification. **D1.**
- **Grinstead & Snell, *Introduction to Probability*** — free
  Dartmouth PDF at <https://math.dartmouth.edu/~prob/prob/prob.pdf>.
  Ch. 11 on Markov chains. **D2, D3.**
- **NetLogo model library** — <http://ccl.northwestern.edu/netlogo/>.
  Runnable reference CAs (forest-fire, diffusion). **D1.**

---

## Not recommended (for Clowder specifically)

Listed here to prevent re-evaluation by future-self. Each item has a
plausible-looking relevance that dissolves on closer inspection.

- **Behavior trees** (any treatment) — conflicts with Clowder's
  emergent, utility-scoring stance per `project-vision.md`. The whole
  design rejects authored branching structures in favor of scored
  dispositions. If BT material comes up again, refer to
  `project-vision.md` §5.
- **Apex Utility AI** (Unity Asset Store) — commercial Unity-only
  plugin. Wrong stack.
- **Dreamers Inc ECS-IAUS-sytstem** — Unity DOTS implementation of
  IAUS. Wrong stack; pattern is already captured via big-brain.
- **Curvature** (apoch/curvature) — abandoned WPF editor for IAUS,
  tied to a specific C++ runtime. Never portable; see A2/A1 plan.
- **Reynolds boids / flocking** — overkill for a non-swarm sim.
  Clowder has ~8 cats, not a swarm. Only revisit if a specific
  behavior (e.g. herd prey) actually requires it.

---

## Not yet categorized

Append new finds here if they don't obviously fit above; periodically
sort into the thematic sections when a pattern emerges.
