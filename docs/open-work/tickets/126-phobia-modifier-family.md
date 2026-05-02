---
id: 126
title: Phobia modifier family — Crusader-Kings-style trait modifiers on urge response
status: ready
cluster: ai-substrate
added: 2026-05-02
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Cats currently respond to perception scalars (`escape_viability`, `body_distress_composite`, threat species) with a single colony-wide intensity. Phobias are a *personality-trait layer* that modulates urge response per-cat — same world state, stronger panic in a phobic cat. Crusader-Kings-style: "claustrophobic cats freak out more if they can't find an escape." This is the design seam for adding *individual texture* to fight-or-flight behaviors without re-tuning the underlying perception, and the natural read-site for the single-axis perception substrate landed by tickets 087 / 088 / 103.

## Single-axis discipline (carry-over from 103)

Phobias must read perception axes that are **separate from** `escape_viability`. Claustrophobia is about *ambient* closed-space anxiety regardless of whether a threat is currently present; `escape_viability` is pure threat-coupled physics. Folding ambient anxiety into a threat-coupled scalar would make claustrophobia tuning leak into Fight (102) / Freeze (105) gating, which is a category error at the substrate layer. New phobia-facing axes get their own scalars in `src/systems/interoception.rs` (or a sibling `environment_perception.rs`).

## Sketch

1. **`Phobias` component** carrying a small enum-set with per-phobia intensity:
   - `Claustrophobia` — closed spaces
   - `Agoraphobia` — wide-open exposed spaces
   - `Monophobia` — being alone
   - `Lupophobia` — fox-shaped threats specifically (vs. generic threat fear)
   - …extensible

2. **New perception scalars** authored alongside (single-axis discipline):
   - `terrain_enclosure` — ambient closed-space pressure (1.0 = boxed in, 0.0 = open). Reuses 103's `count_walkable_tiles_in_box` helper, **decoupled from threat presence**. Consumed by Claustrophobia.
   - `terrain_exposure` — inverse of enclosure, for Agoraphobia.
   - `kin_proximity` — has any bonded relationship within radius. Consumed by Monophobia (high when alone, low when surrounded).
   - Lupophobia reads existing threat-species discriminator (`WildSpecies::Fox` etc.) — no new scalar.

3. **New modifier family** in `src/ai/modifier.rs`. Each phobia modifier multiplexes its perception axis × per-cat phobia intensity into either:
   - an *amplification* of an existing modifier lift (e.g. amplifies `AcuteHealthAdrenalineFlee`'s Flee lift when `Claustrophobia × terrain_enclosure` is high), OR
   - a *base anxiety lift* on `Hide` / `Freeze` even without acute threat (peacetime baseline anxiety in phobic cats).

## Out of scope at this stub

- **Catalog of phobias** — final list, naming, intensity ranges.
- **Distribution at cat-spawn** — which cats get phobias, base rates per personality.
- **Inheritance / acquisition** — whether phobias pass to kittens, are acquired via trauma (e.g. shadow-fox encounter creates Lupophobia), or are static traits.
- **Balance defaults** — magnitude of amplification, pre-soak tuning.

This stub records the design intent so it doesn't rot in conversation memory. User session note (work 103, 2026-05-02): *"introduce 'phobias' as modifiers akin to crusader kings that modify these urge responses. Like claustrophobic cats freak out more if they can't find an escape."*

## Log

- 2026-05-02: Opened as a follow-on of work 103 (escape_viability scalar). Independent of 103's landing — sits on its own perception axes per the single-axis discipline.
