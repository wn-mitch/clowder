# A*-first step_toward — ticket 493 (135 Phase 2c)

## Iteration 1 — wrapper landing (2026-07-05)

### Hypothesis
Making `step_toward` consult `find_path` first (greedy retained as the
A*-`None` fallback) removes concave-terrain stranding from per-tick
chase/travel steps. The ticket predicted "zero functional drift"; the
unit tests already falsified that optimistic claim at terrain
boundaries (A* picks the cheaper tile where greedy picked the
directional one), so the landing gate is verdict + drift attribution,
not byte-identity.

### Observation
Run `logs/tuned-42-07acc090` (900s soak-trace 42/Simba) vs the
post-506 baseline:
- Throughput 112.0 → **126.1 t/s (+12.6%)**; survival + continuity
  canaries pass; Starvation 0; ShadowFoxAmbush deaths 6 (≤10 gate).
- Hunting: 5/5 clean catches in the bone-snap scenario (previously
  mixed hit/miss) — chase competence up, consistent with the
  scenario re-timings (bird 300→800; bone-snap now forces misses).
- **Adverse drift**: ambush attempts 54 → 245; avoided-ward rate
  212 → 1032/10kt; 6 serial deaths at ONE box ([24-32, 58-64] — the
  shadowfox haunting ground), including the pregnant queen →
  kittens_born 0. /diagnose-collapse three-agent sweep confirmed the
  courtship→mating substrate is INTACT (a mating fired; the pregnancy
  and then the breeder pool were killed at the hotspot). Position
  scan: cat presence in the box 0.8% → 1.8% — optimal-corridor
  concentration is the amplifier.

### Concordance
Chase-competence and throughput predictions concordant. The
"zero functional drift" claim from the ticket was wrong in the
direction the unit tests already showed; the unpredicted second-order
effect is route concentration × threat-blind routing: cats have
threat memory (258 `LocationBeliefs`) that scoring reads but
**routing never consults** — no overlay prices "five colony-mates
died here" into A* edges. That substrate gap is ticket **508**
(ThreatBeliefOverlay), which lands immediately after this and BEFORE
the step-6 integrator (which would further concentrate routes via
smoothed corridors). Baseline deliberately NOT promoted on this run;
if 508's soak does not restore fertility (kittens ≥ 1) and cut
hotspot deaths, the 493+508 pair gets re-examined together
(archive-vs-archive attribution).

## Iteration 2 — 508 ThreatBeliefOverlay (2026-07-05)

### Observation (`logs/tuned-42-8e06c997`, vs post-506 baseline)
- ShadowFoxAmbush deaths **6 → 2** (predicted 0-2 ✓); FoxConfrontation 2.
- kittens_born **0 → 2** (predicted ≥1 ✓); welfare 0.316 → 0.444.
- ticks_per_sec 126.1 → **132.4** — the per-cat overlay costs nothing
  measurable (bucket lookups are O(log n) over tiny per-cat maps).
- Hotspot-box presence 1.8% → 2.3% (predicted <1% ✗): the overlay only
  reroutes cats whose OWN beliefs carry the cue — fewer ambushes means
  fewer witnesses, so presence rises while lethality falls. The
  prediction targeted the wrong proxy; deaths were the goal metric.
- Scoring-shape prediction (Patrol/Flee stable) NOT EVALUABLE by
  frame-diff: the comparison runs diverge at colony-survival level
  (one half-dead, one healthy), swamping any routing-layer signal.
  By construction the overlay writes A* edge costs only.

### Concordance
Core predictions concordant (deaths, fertility, canaries, perf); one
proxy prediction discordant with a mechanism-consistent explanation
(witness-gated subjectivity), one not evaluable. 508 lands; the
proxy lesson recorded: for subjective-belief substrate, gate on harm
metrics, not exposure metrics.
