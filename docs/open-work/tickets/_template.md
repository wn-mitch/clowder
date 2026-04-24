---
id: NNN
title: Short title describing this ticket
status: ready              # ready | in-progress | parked | blocked | done | dropped
cluster: null              # A/B/C/D/E or null — matches substrate-refactor clusters
added: YYYY-MM-DD
parked: null               # YYYY-MM-DD date parked, or null
blocked-by: []             # list of other ticket ids that must land first
supersedes: []             # list of ticket ids or inline section refs this replaces
related-systems: []        # docs/systems/*.md filenames
related-balance: []        # docs/balance/*.md filenames
landed-at: null            # commit sha or null
landed-on: null            # YYYY-MM-DD or null
---

## Why
One paragraph: what problem does this ticket exist to solve.

## Scope
- Concrete deliverable 1
- Concrete deliverable 2

## Out of scope
- What this ticket explicitly does NOT cover.

## Current state
What's in flight, what's been landed toward this, what's next. Preserve context
from prior session notes here so a cold-session read gets up to speed fast.

## Approach
Implementation notes. For large tickets, link to `docs/systems/*.md` rather than
duplicating design content.

## Verification
How to prove this is done (tests, canaries, balance reports, focal-cat replays).

## Log
- YYYY-MM-DD: decision / observation / blocker
