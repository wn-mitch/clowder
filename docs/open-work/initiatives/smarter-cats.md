# Initiative: smarter-cats

**Outcome:** cats make *better decisions* — the AI substrate refactor's outcome layer. Closer alignment between observed environment, decided action, and welfare consequence. Better-priced tradeoffs at L2/L3, fewer plan-failure loops, more legible-from-the-trace reasoning.

## What counts

- DSE / scoring shape changes
- Modifier pipeline additions (§3.5)
- Softmax / temperature / commitment-momentum tuning that produces *more strategic* behavior
- Target-taking DSE work (§6.5)
- Coordinator-directive substrate (§7.3)
- HTN / GOAP planner enhancements
- Plan-failure-cooldown / disposition-failure substrate

## What doesn't count

- Pure perception substrate (that's `full-sensory-perception` — perception precedes decision)
- Multi-cat coordination dynamics (that's `social-coordination` cluster, may share this initiative for the AI-side)
- Pure UI for showing decisions (that's `tooling-diagnostics-ui`)

## Example tickets

- `060` AI substrate refactor — program epic
- `148` Courtship-chain fondness ceiling vs gate fragility
- `256` DSE consumers wire belief + affordance axes
- `163` §3.5 modifier port (full-batch)
- `247` planning-substrate hardening — gird against the stuck-cat bug class

## Canary signal

**Hard survival gates** (CLAUDE.md "Verification"): `deaths_by_cause.Starvation == 0`, `kittens_surviving > 0`. Smarter cats survive. A regression on the survival gates after a smarter-cats ticket implies the substrate change scored the wrong tradeoffs.

Doctrine memory: `project_l3_patrol_absorption_cascade` (substrate axes must price the predator-exposure cost of what they elevate, not just the cost of what they suppress).
