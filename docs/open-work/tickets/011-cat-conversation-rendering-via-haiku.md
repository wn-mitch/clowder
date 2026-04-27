---
id: 011
title: Cat-conversation rendering via Haiku (presenter over C3)
status: blocked
cluster: null
added: 2026-04-21
parked: null
blocked-by: [007, 010]
supersedes: []
related-systems: []
related-balance: []
landed-at: null
landed-on: null
---

**Why it matters:** Once C3 (§7 above) ships deterministic
facet-exchange records per Ryan, Mateas, Wardrip-Fruin 2016
*"Characters who speak their minds"* (AIIDE), Haiku renders the
prose of those exchanges into `logs/conversations/<tick>.md`. Belief
math stays in C3; LLM output never feeds back into sim state.

**Architectural contract:** same strict-presenter contract as #10.
C3 decides *what* beliefs got exchanged; the LLM only renders the
dialogue those exchanges would have produced.

**Cross-reference:** `docs/systems-backlog-ranking.md` rank 7 —
V=4/F=3/R=3/C=2/H=3 → **216** (earn the slot, after C3). Under the
**original in-loop framing** (LLM drives conversation → conversation
drives belief → belief drives scoring) the score is **4** —
shadowfox-worse, defer. The 216 only holds under strict presenter
discipline.

**Required hypothesis + prediction** (80–300 bucket per `CLAUDE.md`
Balance Methodology): *Adding presenter-rendered conversation prose
over C3's deterministic facet exchanges will not measurably alter
any canary (sim behavior is unchanged) but will measurably increase
time-to-comprehension when reading a seed-42 soak's social events.*
Null-direction sim prediction is unusual but correct here — this is
a rendering change, not a balance change.

**Dependencies:** gated on **A1** + **A3** + **C3** (above §§5 and
§7) and on **#10** landing first (reuses presenter-layer
infrastructure). Three-deep dependency chain; no rush.

**Risk surface to watch:** the soft aesthetic tax that LLM prose and
sim math can diverge — narratively-satisfying LLM prose subtly
drowning out the math's quieter truths. H=3 priced this in; vigilance
is the mitigation.

## Log

- 2026-04-27: dropped blocked-by 005 — cluster-A umbrella retired; A1 + A3 dependencies satisfied by landed work. Still blocked on 007 (cluster C / C3 belief modeling) and 010 (post-death biographies presenter).
