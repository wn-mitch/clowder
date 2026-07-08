# 314 — cat-vs-prey / wildlife-vs-prey / prey-perceiver writer rows: null-drift record

Ticket: `docs/open-work/landed/314-*.md`. Commit: 919ae1a8 (single
commit — the prey `PredatorBeliefs` implant writer and its reader, the
prey-perceiver `Bolt` heuristic, are coupled and ship together per the
substrate-stubs discipline). Gate soak: `tuned-42-919ae1a8` (900s).
References: `tuned-42-54e4d22e` (264 gate, Cooking family),
`tuned-42-04a9d223` / `tuned-42-d94c282f` (Coordinating family).

## Gate outcome

`just verdict` returns **concern** against the stale pre-Phase-III
baseline (`tuned-42-7c6a368a`): survival PASS, continuity PASS,
throughput PASS (−2.4%), with the familiar Cooking-family drift
channels (EngagePrey lost-prey canary 14.7×, kittens_born −50%,
fulfillment +73.7%, founder-dispersion windows, ward channels). Gate
was **null drift**, resolved by first-check byte-identity below.

## The null-mechanism proof

`tuned-42-919ae1a8` is **byte-identical to `tuned-42-54e4d22e`** — the
accepted 264-gate stream — past the header for the reference's entire
337,761-line body (the only "divergence" is the reference's footer
where the shorter run ends). Every drift channel in the verdict is an
already-attributed property of the Cooking trajectory family. The 314
writer rows, the prey `PredatorBeliefs` implants, and the new prey
query access on `affordance_writer` contribute **zero** mechanism
content. Gate judged **null-drift: satisfied**.

## Pivot bookkeeping (reversible-pivot model holds)

314 adds 5 f32 fields to `SpeciesViolencePriors` (prey-perceiver
rows). Per the 265 record's reversible-pivot model, the tick-1203745
Mallow election flipped again: the pre-314 stack (+22 fields total)
sat on Coordinating (= d94c282f); +5 more lands back on Cooking
(= 54e4d22e). Family ledger at the pivot so far:

| SimConstants growth vs pre-264 | Family |
|---|---|
| base | Coordinating |
| +11 (264) | Cooking |
| +22 (264+265) | Coordinating |
| +27 (264+265+314) | Cooking |

Both families are accepted; the amended methodology (byte-compare
against every accepted reference FIRST) resolved the gate in one
comparison, zero control soaks — versus three control soaks at step 18.

## Also surfaced

- The candidate diverges from the Coordinating references
  (`04a9d223` / `d94c282f`) at exactly line 6717, tick 1203745 — the
  same knife-edge election every constants-growth artifact has hit.
  The pivot's location is stable across four consecutive landings.
- New read-access schedule edges (`affordance_writer` ↔ prey systems'
  `PreyState` writes; `integrate_beliefs` prey `PredatorBeliefs`
  writes) are empirically byte-neutral, consistent with 265's
  control3 finding.

Run inventory kept under `logs/`: `tuned-42-919ae1a8` (900s gate
soak). No control runs were needed.
