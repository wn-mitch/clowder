---
id: 021
title: Monuments — civic & memorial structures
status: blocked
cluster: null
added: 2026-04-22
parked: null
blocked-by: [020]
supersedes: []
related-systems: [monuments.md]
related-balance: []
landed-at: null
landed-on: null
---

**Why it matters:** Colonies leave physical structures that anchor
narrative across generations — burial mounds, coming-of-age stones,
defender's memorials, pact circles, founding stones. Monuments are
**built events**: the act of building is the narrative, the built
object is the artefact. Directly lights the **burial axis** of
ecological-variety canary (currently ~0 firings/year) and is the
strongest burial + generational-knowledge vehicle in the backlog.

**Design captured at:** `docs/systems/monuments.md` (Aspirational,
2026-04-22).

**Score:** V=4 F=5 R=3 C=3 H=3 = **540** — "worthwhile; plan
carefully" (300–1000 bucket).

**Five monument kinds at launch (load-bearing):** Burial Mound,
Coming-of-Age Stone, Defender's Memorial, Pact Circle, Founding
Stone. Each anchored to a specific Significant-tier triggering
event. Additions are a re-triage trigger.

**Scope discipline (load-bearing — keeps H=3):**
1. ≤4 monuments per sim-year (monument-spam guardrail).
2. All monuments are multi-cat (≥2 contributors).
3. No authored / player-directed monument placement.
4. No numeric modifiers on the cat passing a monument.
5. No Strange-Moods-analogue (the-Calling owns that mechanism).

**Build mechanic:** three phases — declaration (coordinator
directive posted on qualifying event), gathering (multi-cat
material transport to site), raising (simultaneous multi-cat action
emitting a Significant narrative event that self-names the
monument via #20).

**Canaries to ship in same PR (4 total):**
1. Burial-axis — ≥1 burial-axis firing per sim-year (currently ~0).
2. Monument-rate — 1–4 per sim-year (detects silence and spam).
3. Cross-kind diversity — ≥3 distinct kinds per 30-min soak.
4. Mortality-drift — `deaths_by_cause` within ±10% of baseline (no
   survival side-effects from monument-building).

**Dependencies:** hard-gated on #20 (naming substrate) and on A1
IAUS refactor (multi-cat GOAP coordination — same gate as
#18 ruin-clearings). Benefits from `coordination.rs` (Built) and
`fate.rs` (Built). Phase 3 needs colony-founding/splitting as a
legible event.

**Shadowfox watch:** no feedback loop in the adversarial direction,
no new mortality category. Main risk is the "monumentalism" gravity
well — pressure to add kinds over time creeping the launch-5 toward
15 and diluting each. Scope rule 1 is the brake.

**Resume when:** #20 naming substrate has landed, A1 IAUS refactor
has landed, and the #18 ruin-clearings multi-cat coordination
pattern is proven.

## Log

- 2026-04-27: dropped blocked-by 005 — cluster-A umbrella retired; A1 IAUS refactor landed. Still blocked on 020 (NamedLandmark substrate); #18's multi-cat coordination pattern is a soft sequencing dependency, not a hard blocker.
