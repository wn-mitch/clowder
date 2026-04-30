---
id: 013
title: Spec-follow-on debts umbrella retirement
status: done
cluster: null
landed-at: null
landed-on: 2026-04-27
---

# Spec-follow-on debts umbrella retirement

**Landed:** 2026-04-27 | **Closeout:** umbrella retirement (this commit)

**Why retired:** ticket 013 was a 161-line umbrella opened 2026-04-21 that catalogued seven spec-follow-on debts (13.1–13.7) from the AI substrate refactor whose resolution lives in *other* systems (death.rs, fate.rs, mood.rs, coordination.rs, aspirations.rs) or in code (retired-constants cleanup). It exhibited the same staleness antipattern as ticket 005: an umbrella's `status: in-progress` flag couldn't reflect that 13.1 had landed (2026-04-23) while 13.2–13.7 each waited on different gates. Decomposing makes the surviving work visible per-ticket so a worker scanning the queue can see "how much work is left."

**What landed under 013's umbrella:**

- **13.1 Retired scoring constants + incapacitated branch cleanup** (2026-04-23, two commits). Rows 1–3 (Incapacitated pathway) + rows 4–6 (corruption-emergency-bonus pathway) shipped as separate `refactor:` commits via a three-way parallel fan-out. All eight retired constants + three retired modifier impls gone. See the `§13.1 …` and `§4.3 Incapacitated marker author` entries elsewhere in this archive.

**Successor tickets at retirement (one per surviving sub-task):**

- [053](../tickets/053-death-event-grief-emission.md) — §7.7.b death-event grief emission (formerly 13.2). Blocked on 007 (cluster C / C3 belief modeling).
- [054](../tickets/054-fate-event-vocabulary-expansion.md) — §7.7.c Fate event vocabulary expansion (formerly 13.3). Gated on Calling subsystem design — `docs/systems/the-calling.md` exists, no implementation ticket yet.
- [055](../tickets/055-mood-drift-threshold-detection.md) — §7.7.d mood drift-threshold detection (formerly 13.4). Blocked on 056.
- [056](../tickets/056-aspiration-compatibility-matrix.md) — §7.7.1 aspiration compatibility matrix (formerly 13.5). Ready.
- [057](../tickets/057-coordinator-directive-intention-strategy-row.md) — §7.3 coordinator-directive Intention strategy row (formerly 13.6). Blocked on 007 (cluster C / C4 strategist-coordinator).
- [058](../tickets/058-tradition-unfiltered-loop-fix.md) — §3.5.3 item 1 Tradition modifier unfiltered-loop fix (formerly 13.7). Ready; behavior-neutral land then balance-thread for non-zero bonus.

**Note on memory write-back:** 013's body called out a per-subtask memory tag pattern (`substrate-follow-on`, `{subsystem-name}`, `ai-substrate-refactor`). Successors carry that intent forward — each commits a memory entry on landing using the same tag pattern.

**Verification:** `just check` clean (no code changes — pure ticket-process). `just open-work-index` regenerates `docs/open-work.md` reflecting 013 retired and 053–058 added.

---
