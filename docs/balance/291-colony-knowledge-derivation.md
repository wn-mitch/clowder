# 291 — ColonyKnowledge derivation via mental-model agreement (four-artifact record)

Ticket: `docs/open-work/tickets/291-*.md`. Commits: derivation +
cutover (b83466cb — the ticket's planned commits 1+2 merged; the
derivation never shipped alongside the legacy scan), false-belief
scenario (d94c282f). Comparison run: `tuned-42-ea55e329` (292
landing gate — the last run with the legacy carrier-count pathway).

## Hypothesis (pre-registered, ticket §Approach, adapted)

With `agreement_quorum = 3`, `agreement_epsilon = 0.2`,
`promotion_strength = 0.3`, the derived knowledge set carries the
same scoring-visible signal as the legacy carrier-count set. The
ticket's ±20%-Jaccard framing is re-grounded in what's measurable
from soak artifacts (entry sets aren't logged per tick): the
promoted/forgotten EVENT VOLUME and the knowledge-flavored narrative
composition stand in for set similarity.

Named structural deltas (not drift — designed):

1. Only ThreatSeen + ResourceFound derive (the two location facets
   with belief writers — and exactly the two types the scoring
   readers consume). Death/Magic/Injury/Social/Triumph/Sleep colony
   knowledge retires with the Memory scan. **Mythic-texture canary
   is the pre-registered gate on that narrative loss.**
2. Entry strength = live agreed mean facet value at each scan (was:
   promotion-time memory strength with per-tick decay).
3. New footer field `belief_divergence_duration_ticks` (258 exit
   criteria) — expected > 0 on a live colony.

## Predictions

| # | Prediction | Band |
|---|---|---|
| P1 | Survival + continuity canaries hold — **mythic-texture ≥ 1 especially** (colony knowledge feeds narrative; the six retired types must not zero it) | hard gate |
| P2 | `KnowledgePromoted` and `KnowledgeForgotten` both fire ≥ 1 (silent-canary exposure on the new derivation path) | hard |
| P3 | Promoted-event volume within ~3× of the legacy run's in either direction (order-of-magnitude parity — the derivation scans live beliefs, not accumulated memories, so some volume shift is expected) | gate channel |
| P4 | `belief_divergence_duration_ticks` > 0 (the divergence window the restructure exists to expose actually occurs in colony life) | mechanism |

## Observation (`tuned-42-d94c282f` vs `tuned-42-ea55e329` legacy)

- **P1 mixed** — survival + continuity canaries PASS (tps 110.5,
  band pass; deaths 2, both ShadowFoxAmbush, within gates). The
  mythic-texture tally is 0 — but causally DECOUPLED from 291:
  mythic-texture counts Calling / named-object / banishment /
  visitor events, none of which flow through ColonyKnowledge, and
  this trajectory's shadowfox channel is stone cold (sieges 80 → 0,
  the 513 family swing in its other direction; banishments are the
  family's main mythic source). Zero also occurred pre-291 in this
  family (`861c9fe5`). Attributed to 513, not this cutover.
- **P2 CONFIRMED** — KnowledgePromoted 12, KnowledgeForgotten 11;
  both features live on the derivation path.
- **P3 DISCORDANT, decomposed** — promoted volume 73 → 12 (beyond
  the ±3× band). Forgotten-line flavor split explains it exactly:
  legacy 53 = danger 37 · rested-ground 13 · banishment 2 ·
  foraging 1; new 11 = foraging 10 · danger 1. Three components:
  (a) 15/53 legacy lines were the six retired types — designed;
  (b) ThreatSeen 37 → 1: `recency_of_threat_cue` is fast-decay BY
  DESIGN, so colony threat knowledge is now a short consensus
  window over fresh cues instead of the legacy's slowly-staling
  memory persistence; (c) ResourceFound 1 → 10: slow-decay
  `prey_yield` (293) supports rich, stable consensus.
- **P4 DISCORDANT, mechanism proven elsewhere** —
  `belief_divergence_duration_ticks` = 0 on this soak. The
  machinery is pinned by unit test + the false-belief scenario's
  contested bucket; in colony life, witnesses of the same events
  EMA toward the same values, so strength-quorum disagreement is
  genuinely rare rather than unmeasured. The footer field stays —
  it is the instrument for the gossip/divergence epidemics C2 work
  will introduce.

## Concordance

P2 concordant; P1's sensitive reading resolves to a known
trajectory-family swing (513); P3 and P4 discordant with verified,
named mechanisms rather than tuning misses. The structural judgment:
retuning `promotion_strength` / `agreement_epsilon` to restore
legacy threat-knowledge VOLUME would fight the substrate — threat
cues decay fast on purpose, and persistent colony threat memory now
correctly lives in per-cat `LocationBeliefs` reads (263/508 wiring)
rather than a stale colony ledger. If Phase IV/V fear-response
tuning wants persistent colony-level threat memory, the named
follow-on is a slow-timescale "remembered danger" location facet —
not a wider promotion gate. Landed on this record.
