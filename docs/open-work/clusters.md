# Cluster taxonomy

Clusters are the **categorical bucket** for a ticket — exactly one per ticket, required at open-time (`just open-ticket "<title>" --cluster <name>`). The cluster names the system or area where the work lives in code, not its thematic outcome (that's `initiative:` — see [`initiatives/`](initiatives/)).

A ticket's cluster decides:

- Which subsection it appears under in `## Ready by cluster` in [`docs/open-work.md`](../open-work.md)
- Which `just open-work-ready --cluster <name>` filter it answers
- Which neighbors `just similar` ranks higher when the seed shares a cluster

Two-axis rule: **cluster is categorical** (one per ticket, where the work lives), **initiative is thematic** (zero-or-more per ticket, what outcome it serves). A crafting ticket and a monument ticket carry different clusters (`items-crafting` vs. `buildings-zones`) but can share `initiative: [world-richness]`.

## The 11 clusters

| Cluster | Scope |
|---|---|
| `ai-substrate` | Markers, DSE catalog, scoring, softmax, modifier pipeline, target-taking, planner state. The L1/L2/L3 mechanism. |
| `belief-perception` | Interoception, witnessable events, mental models, location beliefs, scent maps, influence maps, audible cues. Where cats *sense* their environment. |
| `combat-threat` | Fight / flight / freeze / fawn / hide, engage-threat, IntraspeciesConflict, escape_viability. Cat-side threat response. |
| `wildlife` | Fox / hawk / snake / shadowfox planners, prey-side AI, predator behavior. Non-cat AI. |
| `social-coordination` | Coordinator directives, intention, JointIntention, BDI, courtship roles, mentoring, alloparenting practices. Multi-cat dynamics. |
| `items-crafting` | Recipes, stations, herbalism / witchcraft / cooking, slot inventory, materials, pickup pipeline. |
| `buildings-zones` | Monuments, gravesites, wards, kitchens, ColonyStores, planner zones, construction. |
| `life-cycle` | Mating → birth → kittens → growth → death → burial → grief → biographies. The full lifecycle arc. |
| `magic-mythic` | Calling, fate, ceremony, spirituality, corruption sites, ruin clearings, named events. |
| `tooling-diagnostics-ui` | `just` recipes, verdict / hypothesize / sweep-stats / similar / next / logdb, scenario harness, log viewer, windowed UI, rendering. |
| `process-discipline` | Substrate-stub lint, layer-walk audits, sub-agent prompt template, structural-option discipline, ticket-from-session, allowlists. |

## Retired / migrated clusters

These appear in the back-catalogue (or in active tickets still awaiting the Phase F tagging pass) but are not part of the current taxonomy:

- `C` / `D` / `E` — substrate-refactor phase tags; these are *phases* of work, not clusters. Tickets carrying them re-tag to `ai-substrate` (the categorical cluster) and keep their phase identity in the ticket title or `## Why` section.
- `substrate-migration` — folded into `ai-substrate`.
- `world-ecology` — folded into `initiative: world-richness` (this was thematic, not categorical).
- `emotional-fidelity` — folded into `initiative: welfare-fidelity` or `initiative: mythic-texture` depending on shape; not a categorical cluster.
- `docs` / `process` — generic catchall; folded into `process-discipline` or `tooling-diagnostics-ui`.
- `balance` — folded into the domain cluster (a ward-weight tune is `buildings-zones`; a hangry-curve tune is `life-cycle` or `welfare-fidelity`). The cluster names *where the code lives*, not *whether the work is tuning*.

## Adding a new cluster

If a ticket's work genuinely doesn't fit any of the 11 above, add a new cluster — but first ask:

1. Is this a *categorical* gap (a new system / area) or a *thematic* gap (a new outcome)? Thematic gaps belong in `initiative:`, not `cluster:`.
2. Will more than ~3 tickets carry this cluster? If only one ticket lives here, the cluster shouldn't exist — fold into the nearest categorical neighbor.
3. Update this doc in the same commit so future opens see the value.
