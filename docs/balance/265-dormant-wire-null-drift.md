# 265 — wildlife DSE dormant wire: null-drift record + pivot-reversal finding

Ticket: `docs/open-work/tickets/265-*.md` (dormant-wire half; activations
are plan step 21). Commits: 453cc3f7 (prey-affordance axes on
fox/hawk/snake hunting scorers), 04a9d223 (wildlife-side beliefs:
`#[require(CatBeliefs)]` on WildAnimal, integrator witness pass,
violence priors, dormant flee axes). Gate soak: `tuned-42-04a9d223`
(900s). References: `tuned-42-54e4d22e` (the 264 gate stream) and
`tuned-42-d94c282f` (the pre-264 Phase-III 291 gate stream).

## Gate outcome

`just verdict` returns **concern** against the stale pre-Phase-III
baseline (`tuned-42-7c6a368a`): survival PASS, continuity PASS,
throughput PASS (+12.1%), with the familiar drift channels
(kittens_born −50%, founder-dispersion windows, ward channels). The
gate for this landing was **null drift**, resolved by byte-comparison
below — every drift channel is shared with an already-accepted stream.

## The null-mechanism proof

`tuned-42-04a9d223` (main + 264 wire + 265 wire) is **byte-identical to
`tuned-42-d94c282f`** — the pre-264 Phase-III reference — past the
header for the full 194,000-line overlap (~98k ticks). The combined
264+265 dormant wires produce a stream indistinguishable from the world
before either landed. All behavioral difference previously visible in
the 264 gate stream (`54e4d22e`) was the ScoringConstants struct-growth
artifact, and 265's further growth cancels it. Zero mechanism content
in either wire. Gate judged **null-drift: satisfied**.

## The pivot is reversible, not saturating (supersedes the 264 record's note)

The 264 record predicted steps 18–19 "may byte-compare directly against
`tuned-42-54e4d22e`" because the artifact "saturates". Wrong. Control
ladder (all seed 42, byte-compared past the header; divergences all at
the same near-tie election — line 6717, tick 1203745, Mallow
Coordinating↔Cooking):

| Stream (on pre-265 main a917bef3 unless noted) | Trajectory at the pivot |
|---|---|
| `tuned-42-d94c282f` (pre-264 Phase-III main) | Coordinating (original) |
| `tuned-42-54e4d22e` (264 wire: +11 ScoringConstants fields) | Cooking |
| control1 `-dummy11`: +7 dummy ScoringConstants + 4 dummy SpeciesViolencePriors fields | Cooking (= 54e4d22e family) |
| control2 `-dummy11-require`: control1 + `#[require(CatBeliefs)]` on WildAnimal | Cooking |
| control3 `-dummy11-require-readaccess`: control2 + `Option<&CatBeliefs>` read access on the three wildlife evaluate queries | Cooking |
| **candidate `tuned-42-04a9d223`** (264 + 265 wires: +18 ScoringConstants / +4 priors fields total) | **Coordinating — byte-identical to d94c282f for 194k lines** |

Reading: the tick-1203745 election is a knife-edge pivot between (at
least) two trajectory families, flipped by SimConstants size/layout
thresholds — base → Coordinating, +11-ish → Cooking, +22 → Coordinating
again, *bit-exactly* recovering the original stream. Neither the
required-component archetype change nor the new query access sets
(schedule edges) move the pivot — three controls confirm it is purely
the constants-struct artifact. Dummy-field counts do NOT compose
predictably with real field layouts near the threshold (control1's 7+4
landed on Cooking while the candidate's real 7+4-on-top-of-264 landed
on Coordinating — same counts, same insertion points, different
neighbors in the struct).

## Methodology note (amends the 264 record)

When a null-drift gate meets a SimConstants schema change, byte-compare
the candidate against **every plausible accepted reference** (the
previous gate stream AND the pre-artifact original) before building
dummy controls — the pivot can snap *back*. If no reference matches,
run the dummy-control ladder as before; if that also fails to
reproduce, bisect structural layers (component, query access) as done
here. Byte-identity to ANY accepted-stream family is the null proof.

## Also surfaced

- The wildlife evaluate systems' `Option<&CatBeliefs>` read access
  (new conflict edges vs `belief_integrator`'s writes) is empirically
  schedule-neutral on seed 42 — control3 vs control2 identical.
- Deferred to step 21 (behavior-priced): the 505-flagged
  `FleeFrom→PredatorBeliefs` witness write; the live
  `Res<ActionAffordances>` borrows in the three wildlife evaluate
  systems; the `wildlife_species_clash` scenario.

Run inventory kept under `logs/`: `tuned-42-04a9d223` (900s gate soak),
`tuned-42-a917bef3-dummy11` (300s control1),
`tuned-42-a917bef3-dummy11-require` (90s control2),
`tuned-42-a917bef3-dummy11-require-readaccess` (90s control3).
