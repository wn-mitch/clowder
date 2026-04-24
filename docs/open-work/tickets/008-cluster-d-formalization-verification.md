---
id: 008
title: Formalization and verification (Cluster D)
status: ready
cluster: D
added: 2026-04-20
parked: null
blocked-by: []
supersedes: []
related-systems: [ai-substrate-refactor.md, refactor-plan.md]
related-balance: []
landed-at: null
landed-on: null
---

**Why this is a cluster:** D1–D3 are each half-day investigations that
formalize names for patterns Clowder likely already has. The payoff
is *vocabulary-as-engineering-leverage*: once `weather.rs` is labeled
as a Markov process, "add a rare unseasonal-warm-spell" becomes "add
a state + transition probabilities," not "figure out where in
`weather.rs` to add an if-else." Low urgency; no code changes expected
unless verification surfaces a bug.

### D1. Verify / label corruption spread as cellular automaton

Does `src/systems/magic.rs` corruption use local-rule propagation
(classic CA) or global scalars? If CA, label it as such in the
system's `docs/systems/*.md` stub. If not, consider whether
reaction-diffusion PDE or CA rules would produce better-looking
spread patterns.

**Preparation reading (shared with D2/D3):**
- Stephen Wolfram, *A New Kind of Science* ch. 2–3 (skim) — free
  online at <https://www.wolframscience.com/nks/> — CA classification
- Epstein & Axtell, *Growing Artificial Societies* — Sugarscape shows
  CA-style spread inside agent-based models; closest to Clowder's
  use case
- NetLogo CA model library (<http://ccl.northwestern.edu/netlogo/>) —
  runnable reference implementations of forest-fire and diffusion CAs,
  directly analogous to corruption spread

### D2. Verify / label mood dynamics as Markov process

Does `src/systems/mood.rs` implement explicit transition probabilities
between mood states? If yes, label as Markov. If transitions are
deterministic cascades, note the distinction.

**Preparation reading:**
- Any introductory probability textbook chapter on Markov chains
  (Grinstead & Snell, *Introduction to Probability* ch. 11, free
  Dartmouth PDF at <https://math.dartmouth.edu/~prob/prob/prob.pdf>)
- Marsella & Gratch, "Computationally modeling human emotion" (CACM
  2014) — depth on affect dynamics; probably overkill

### D3. Verify / label weather transitions as Markov process

Probably already obvious; confirm in `docs/systems/` stubs.

**Preparation reading:** same as D2.

**Exit criterion for cluster D:** `docs/systems/*.md` stubs carry the
formal pattern name where applicable.
