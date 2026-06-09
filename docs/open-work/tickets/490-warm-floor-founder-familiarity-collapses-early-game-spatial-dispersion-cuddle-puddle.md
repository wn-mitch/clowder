---
id: 490
title: warm-floor founder familiarity collapses early-game spatial dispersion (cuddle puddle)
status: ready
cluster: social-coordination
orchestration: substrate-sensitive
initiative: []
added: 2026-05-30
parked: null
blocked-by: []
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

<!--
Bugfix-shape ticket. Use this template (rather than `_template.md`) when the
work is a fix to observed defective behavior. The "Bugfix discipline" section
of CLAUDE.md REQUIRES at least one structural-revision candidate per fix-shape
decision tree — the slots below force that to be drafted, named, and considered.
-->

## Why
Founders spawn and **huddle** — they cluster within ~4.7 tiles of their shared
centroid for the whole early game instead of fanning out across the map to
forage / hunt / build ("cuddle puddle"). It is a **spatial** defect:
colony-wide courtship/grooming *event counts* are flat across the regression
window (so the continuity canaries don't catch it), and `structures_built` is
unaffected (8–12). The signal only shows in founder spatial dispersion. There
is no existing canary for it — adding one is a candidate follow-on.

Surfaced on seed 42, windowed run + headless soak-traces. Diagnosed via an
A/B founder-dispersion scan (mean distance-to-centroid of the first-snapshot
founder set, bucketed by elapsed-tick window).

## Hot context (session-diagnosed; remove once picked up)

A/B, seed 42, 120s headless soaks, identical founder roster:

| founder spread (tiles, mean dist-to-centroid) | `ca5d59c4` (pre-cluster) | `b24d333b` (relationships only) | `a11a5afc` (full cluster) |
|---|---|---|---|
| +0..1500 ticks (spawn clump) | ~1.3 | ~1.4 | ~1.1 |
| +3000..6000 | **24.8** | 9.8 | **4.8** |
| +6000..12000 | **24.1** | 6.9 | **4.7** |

`structures_built`: 8 / 12 / 10 — building unaffected. courtship continuity
tally flat (~1550–1750) back to *before* `courtship_method` (323); grooming
drifts 402→653 only. So this is **not** a courtship/grooming-frequency
regression — it is a relationship-graph → spatial-attractor regression.

Reproduce: build at commit, `cargo run --release -- --headless --seed 42
--duration 120 --event-log <dir>/events.jsonl`, then compute mean
dist-to-centroid over founder `CatSnapshot` positions per elapsed-tick window
(one-off scan; not yet a wired tool).

## Current architecture (layer-walk audit)

| Layer | Component / file | Load-bearing fact | Status |
|---|---|---|---|
| Founder init | `src/resources/sim_constants.rs:1266` (`RelationshipsConstants`) | `founder_familiarity` ∈ **[0.4, 0.6)** — entirely **at/above** the 0.4 Friends gate, not straddling it. Set by `b24d333b`. | `[verified-correct]` (read + git-show) |
| Bond gate | `src/resources/sim_constants.rs:1210` + `src/systems/social.rs:555` | `friends_familiarity_threshold = 0.4`; pair graduates to `BondType::Friends` when `familiarity > 0.4 && fondness ≥ 0.3`. With fondness [0.1,0.4), ~⅓ of founder pairs clear both gates on the first bond-check. | `[verified-correct]` |
| Spatial attractor | `src/systems/fulfillment.rs::bond_proximity_social_warmth` (+488 `social_warmth` floor [0.85,1.0]) | Bonded founders gain passive fulfillment for staying near each other; combined with proximity-dependent groom/courtship targeting this is a co-location pull with no counter-pressure. | `[verified-correct]` (A/B: removing the bonds restores 24-tile dispersion) |
| L1 marker (amplifier) | `HasGroomCandidate` (487, `markers.rs`) | Universal founder bonding keeps a groom candidate perpetually in range → `GroomOther` perpetually eligible. Tightens the huddle (`b24d333b` 9.8/6.9 → `a11a5afc` 4.8/4.7) but is not the root. | `[verified-correct]` |
| L2/L3 scoring | `src/ai/dses/*`, `src/ai/scoring.rs` | Courtship/grooming DSE *win-share* and event counts are flat across the window — scoring shape is not the regression vector. | `[verified-correct]` (footer tallies flat) |

## Design tension (must be resolved by the chosen candidate)

`b24d333b` raised `founder_familiarity` **on purpose** — its doc comment
(`sim_constants.rs:1259`) names the *opposite* defect: at the old [0.1, 0.3),
the `socialize_target` **novelty** axis (`1 − familiarity`) was [0.7, 0.9),
judged an over-socializing "everyone is maximally novel" attractor. So the
commit traded a novelty-driven puddle for a bond-driven *spatial* puddle, and
verified only GroomOther win-share — never spatial dispersion. The levers are
in tension: **novelty** wants familiarity high; **dispersion** wants familiarity
below the 0.4 graduation gate. Whatever ships must not silently reintroduce the
novelty problem. (Note: at `ca5d59c4`, the old [0.1,0.3) familiarity *did*
disperse to 24 tiles with no spatial puddle — evidence the novelty concern was
about socialize win-share, not spatial behavior.)

## Fix candidates

**Parameter-level options:**
- R1 (**retune toward a true straddle**) — set `founder_familiarity` to a band
  that actually straddles 0.4, e.g. [0.3, 0.5), so only ~half of pairs graduate
  to Friends and novelty (`1 − familiarity` = [0.5, 0.7]) stays well below the
  old [0.7, 0.9]. Threads both needles; verify dispersion + socialize win-share.
- R2 (**soften the spatial attractor knobs**) — keep familiarity high (preserve
  the novelty fix) and dial back `bond_proximity_social_warmth` rate/range
  and/or the 488 `social_warmth` spawn floor so bonding no longer pins founders
  spatially.

**Structural option (required):**
- R3 (**decouple bond from forced co-location** — *rebind/extend*) — the root
  pathology is that `BondType::Friends` imposes a spatial proximity pull that
  outlasts productive need. Make affiliative proximity yield to work
  dispositions (forage/hunt/build) — i.e. founders can be warm friends *and*
  range the map during the working day, converging only when no productive
  pull exists. This fixes the puddle without re-litigating the familiarity
  band at all, and is the substrate-clean expression of "warm founders who
  still build."

## Recommended direction
Designer's call (encodes the social framework — see
[[project_clowder_substrate_encodes_morals]]). R1 is the cheapest verified-
restorable fix and realigns the constant with its own stated "a fraction
graduate" intent. R3 is the structurally correct fix if the design intent is
"founders are warm friends from day one *and* disperse to work" — it keeps the
warm-floor relationships and kills only the spatial pull. R2 is a fallback if
the novelty axis must stay maximally suppressed. R1 + R3 compose.

## Out of scope
- The materials-atlas / asset-path work in the same session (separate commits).
- Wiring a permanent founder-dispersion canary (follow-on if R-choice needs
  ongoing protection — the defect was invisible to every existing gate).

## Verification
No existing hard-gate/canary covers this. Verify via the founder-dispersion
A/B: rebuild + `--headless --seed 42 --duration 120`, confirm +3000..12000
spread recovers toward the pre-cluster ~24 tiles (or an agreed target band)
**without** regressing `socialize` L3 win-share in the first 5k ticks (the
metric `b24d333b` guarded). `just verdict` to confirm survival/continuity
canaries stay green and `structures_built` does not drop.

## Log
- 2026-05-30: opened. Root-caused via A/B founder-dispersion scan (seed 42):
  warm-floor `founder_familiarity` [0.4,0.6) over-graduates founder pairs to
  `BondType::Friends`, collapsing early-game dispersion 24→4.7 tiles; 487/488
  amplify. Courtship/grooming event counts and `structures_built` unaffected.
- 2026-06-09: designer's call resolved — **R1 + R3 compose**, plus the
  dispersion canary (pulled in from §Out-of-scope since the defect was
  invisible to every existing gate). Implemented:
  - **Canary**: `Founder` marker (spawn loop) + `FounderDispersionStats`
    resource sampled inside `emit_cat_snapshots` (no new system —
    schedule-edge discipline); footer `founder_dispersion` window blocks;
    `just verdict` absolute floor (mean dist < 10 tiles in any
    post-spawn ≥3000-elapsed window → concern).
  - **R1**: `founder_familiarity` → [0.3, 0.5) — a true straddle of the
    0.4 Friends gate (~half of pairs graduate); novelty axis reads
    [0.5, 0.7], below the old over-socializing [0.7, 0.9).
  - **R3**: `WorkPressureAffiliativeYield` registered ScoreModifier —
    multiplicative damp on {Socialize, GroomOther} keyed on
    `1 − phys_satisfaction` (threshold 0.5, scale 0.5), templated on
    StockpileSatiation. Driver-level (the 487 lesson: gate-only fixes
    shift bandwidth to Patrol); trace-visible; excludes Mate/Caretake;
    accrual systems untouched so founders stay warm Friends.
  Verification per §Verification pending: A/B dispersion recovery,
  socialize win-share guard, mating/bond canaries, Patrol-absorption
  check.
