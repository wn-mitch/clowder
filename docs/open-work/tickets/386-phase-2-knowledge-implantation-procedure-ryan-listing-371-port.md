---
id: 386
title: Phase-2 knowledge implantation procedure (Ryan Listing 37.1 port)
status: blocked
cluster: belief-perception
orchestration: coherent-block
block: worldgen-prehistory
initiative: [generational-continuity, smarter-cats, worldgen-prehistory]
added: 2026-05-16
parked: null
blocked-by: [385]
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

## Why

At the Phase-1 → Phase-3 boundary, every surviving cat needs subjective beliefs about kin, closest bonds, territory, and a salience-weighted subset of other entities. Ryan et al. § 37.3.10 / Listing 37.1 gives the pseudocode for a one-time bulk-insert into per-cat `MentalModel`s. All implanted beliefs are accurate at the moment of insertion; divergence emerges once Phase-3 runtime knowledge phenomena (observation, transference, confabulation, mutation) activate. This is the bridge procedure between Phase-1's ground-truth event log and Phase-3's subjective-belief substrate. Distinct from #388 (colony-shared ColonyKnowledge); this leg is *per-cat subjective*.

## Scope

- Port Listing 37.1 into Rust as a `KnowledgeImplanter` one-shot system
- Composes with the `MentalModel` substrate (258, landed) — uses the `Implant` evidence type to mark beliefs sourced from boundary insertion
- Implants over: kin (parents/grandparents reachable via #387), closest bonds (top-K by affinity), territory (dens / scent-marked perimeters from Phase-1), salience-weighted random subset of other entities (§ 37.3.10's probabilistic selection)
- Fires exactly once, at the Phase-1 termination boundary, before Phase-3 systems are scheduled
- Reads the Phase-1 ground-truth event log; doesn't re-run the sim

## Out of scope

- The sim-loop mode that produces the event log (#385)
- ColonyKnowledge (colony-shared) seeding — that's #388; this leg is per-cat subjective beliefs
- Phase-3 belief-mutation systems (already exist via 258)
- Sharpening the salience model — Listing 37.1's placeholder weighting ships first; refinement is a follow-on outside this block

## Current state

Aspirational — gated on `worldgen-prehistory` block activation (see [9]). 258 (landed) supplies the `MentalModel` substrate + `Implant` evidence type. Blocked-by [385] only (needs the Phase-1 event log to read from).

## Approach

Single-shot system scheduled exactly between Phase-1 termination and Phase-3 first tick. Walks surviving cats; for each, queries the Phase-1 event log + lineage graph (#387) + bond graph + spatial graph to materialize the beliefs. Uses the existing 258 `MentalModelFacet` types as targets. Salience weighting is a simple probability-by-co-occurrence-count for the first implementation.

## Verification

- After implantation, ∀ surviving cat: `MentalModel` non-empty for self + ≥1 kin + ≥1 closest-bond + ≥1 territory entity
- All implanted beliefs carry `Implant` as evidence type
- A focal-cat trace under `SimMode::Runtime` shows the implanted beliefs as inputs to L2 scoring on tick 1

## Log

- 2026-05-16: opened as leg of `worldgen-prehistory` coherent-block (see [9])
