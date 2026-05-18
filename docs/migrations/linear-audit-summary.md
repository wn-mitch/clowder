# Linear workspace audit — Phase 1 of ticket migration

Generated: 2026-05-18T17:35:05+00:00

## Workspace identity

- Organization: **Clowder** (clowder, id aefc6e8f-b89d-4f8a-b07c-e8be432b4bdb)
- Subscription: type=free? seats=?
- Created: 2026-05-18T13:43:17.924Z
- Viewer: **will** <will@adversarial.com> (id 5b0b9221-76e5-4d88-9f9e-f660d1b1cfee)

## Teams

| Key | Name | Issues | Cycles | Cycle len | Default state |
| --- | --- | ---: | --- | ---: | --- |
| CLO | Clowder | 4 | no | 2 | Backlog |

## Workflow states (per team)

### CLO

- `Backlog` (type=backlog, position=0)
- `Canceled` (type=canceled, position=4)
- `Duplicate` (type=canceled, position=5)
- `Done` (type=completed, position=3)
- `In Progress` (type=started, position=2)
- `Todo` (type=unstarted, position=1)

## Existing labels

- **(workspace-global)**: `Bug`, `Feature`, `Improvement`

## Projects

_None._

## Existing issues sample (per team)

Will has authorized clearing the chosen target team before bulk import. This section reports what's currently there so the choice is informed.

### CLO

- CLO-4 **Import your data** _(Todo)_
- CLO-1 **Get familiar with Linear** _(Todo)_
- CLO-3 **Connect your tools** _(Todo)_
- CLO-2 **Set up your teams** _(Todo)_

## Rate limit (from last response)

- `x-ratelimit-complexity-limit`: 3000000
- `x-ratelimit-complexity-remaining`: 2998351
- `x-ratelimit-complexity-reset`: 1779129305688
- `x-ratelimit-requests-limit`: 2500
- `x-ratelimit-requests-remaining`: 2494
- `x-ratelimit-requests-reset`: 1779129305688

## Open questions for Phase 2

- **Target team**: Which team hosts the migration? Will has authorized clearing existing issues in the chosen team — pick based on the issue inventory above.
- **Plan tier capabilities**: Linear's free tier omits custom fields; Phase 2 field mapping needs confirmation that the chosen team's plan supports the `Legacy ID`, `Orchestration`, `Block`, `Verdict anchor`, `Wires method` custom fields. If not, those compress into labels.
- **Pre-existing tickets**: `docs/open-work/pre-existing/{dead-features-in-activation-tracker,substrate-stub-catalogue}.md` — migrate as issues, projects, or stay as repo docs?
- **Duplicate NNNs in landed/** (pre-existing, surfaced by the archaeology): IDs 001 (active + landed have different work), 014 (3 files, phase-numbered sub-tickets), 024 (2 files, warmth split phases), 072 (2 files, appear truly duplicated). Phase 2 must collapse these into single Linear issues or assign new IDs.
- **Bulk import order**: With Linear ID == NNN as a hard constraint, the importer must run in strict numeric order 001..411 against an empty team. Decide whether to clear the existing target team (Will-authorized) or pick a different empty team.

