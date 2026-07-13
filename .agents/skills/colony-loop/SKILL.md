---
name: colony-loop
description: Work ONE 3-5 ticket related cluster to a verified colony-score gate. Discovers a cluster off the top of the ready queue (just next + just similar), confirms a plan with the user BEFORE any work, works it track-aware (swarm-safe polecats / substrate-sensitive sessions), then gates on exactly one out-of-sample soak + just verdict + a separate-model skeptic audit. Use when the user types `/colony-loop`, or asks "run a colony-improvement chunk", "work a related ticket cluster", "advance the colony score", "pick a cluster and land it against the gate". One chunk per invocation; ad-hoc, never scheduled. Do NOT fire for ticket recommendation only (use `/next`), single-session orchestration (use `/work`), balance hypothesis testing (use `/hypothesize`), or run diagnosis (use `/diagnose-run`).
argument-hint: "[optional bias: --initiative <name>, focus text, or an approved PLAN to resume]"
---

# /colony-loop — related-cluster colony-improvement loop

Advance the colony's health by landing ONE cluster of 3-5 generally-related ready
tickets per invocation, under a plan the user approves, gated on the colony score.
This is the entry point for the forged loop spec in `docs/workflow/colony-loop-spec.md` — read
it for the full rationale; this file is the runnable procedure. **One chunk per
invocation.** A PASS presents the result and STOPS; the human re-invokes for the
next chunk.

**Arguments:** `$ARGUMENTS` — optional. `--initiative <name>` scopes discovery;
free text biases cluster selection; if it carries an already-approved PLAN, skip to
§3 Work.

## The done bar (embed — cold-checkable, no memory of any conversation)

> A 3-5 ticket related cluster is DONE only when: every ticket is landed on `main`;
> `just verdict logs/tuned-<oos-seed>` exits 0 against `logs/baselines/current.json`
> with no footer field regressed >=10%; every continuity canary
> (grooming/play/mentoring/burial/courtship/mythic-texture) > 0; the plan-declared
> target metric's Δ vs baseline is >= its threshold and outside the noise band per
> `just fingerprint`/`just frame-diff` (or, if flat, reported **UNMET** — not
> passed); exactly one soak ran (a 2nd only with a logged borderline justification);
> no test was deleted/`#[ignore]`/loosened; no substrate-sensitive ticket was
> auto-landed; and no baseline was promoted mid-chunk. A fresh skeptic (different
> model) re-runs verdict + `just check && just test` and re-reads each ticket's
> §Acceptance before granting pass; any failing check → not done, pause for the human.

## Two human gates (non-negotiable)

1. **Plan-confirm** — before ANY work. Present cluster + declared metric + per-ticket
   track + expected diff shape; user accepts / edits / replaces. NEVER work an
   unconfirmed cluster.
2. **Reject-pause** — on any gate reject (verdict `concern`/`fail`, OR any audit check
   failing): pause immediately, hand back the broken checks + strongest verified
   partial (labeled NOT done). **0 fix rounds** — never self-repair, never re-run the
   gate to fish for a pass.

## Workflow

### 0. Precondition — baseline present

Run first: `jq -r '.label // "none"' logs/baselines/current.json` and confirm the
baseline's events file exists (`test -f "$(jq -r .events_path logs/baselines/current.json)"`).
If missing / `no-baseline`: tier-1 regression degrades to canaries-only and the metric
Δ is uncomputable → objective auto-UNMET. **Pause and ask the human to
`just promote logs/tuned-<seed> <label>` a real baseline first.** The loop NEVER
promotes one itself.

### 1. Discover (read-only) — assemble the candidate cluster

Issue these in one message (independent reads):

- `just next` (add `--initiative <name>` from `$ARGUMENTS` if given) → top ready
  ticket off the queue. Take `results[0].id` as the **seed**.
- `just similar <seed-id> --corpus tickets` → relatedness-ranked neighbors.
- `just open-work-ready-filtered --track swarm-safe` and
  `... --track substrate-sensitive` → per-track ready pool + each ticket's
  `orchestration` track.

Grow the seed to **3-5 tickets**, keeping only `status: ready` tickets that share a
`cluster`/`initiative` OR clear the `similar` relatedness bar. A 3-ticket related
chunk beats a 5-ticket grab-bag — never force unrelated tickets in to hit 5. Read
each candidate's `orchestration` frontmatter for its track. Name **ONE** declared
target metric the cluster plausibly moves, from `docs/balance/healthy-colony.md`
(fall back to a footer field + `frame-diff` if it is not in the rubric).

### 2. Plan-confirm — interactive user gate (GATE 1)

Present the proposed `PLAN` compactly, then ask the user to accept / edit / replace:

```
PLAN
  cluster:  <cluster-or-initiative>          (relatedness: <similar scores>)
  tickets:  <id> [<track>] — <title>
            ... (3-5 rows)
  metric:   <declared_metric>  (threshold >=10%, outside noise band)
  oos-seed: <seed NOT used developing these tickets>
  shape:    <expected diff shape per ticket>
```

Ask the user to choose via your harness's interactive prompt (`AskUserQuestion` in
Claude Code, `ask` in omp): **Approve** / **Edit cluster** / **Edit metric** /
**Replace cluster**. Do not proceed to §3 until Approve. Record
`baselineLabelAtStart = jq -r .label logs/baselines/current.json` at approval time
(audit check 5 compares it later).

### 3. Work — track-aware, per-workspace isolation (orchestration primitives own it)

Do NOT add worktree logic here — `session-new`/`foreman-spawn` each create their own
`~/clowder-sessions/<slug>` jj workspace. `main` advances only via the refinery.

- **swarm-safe** tickets → `just foreman-spawn <N>` where N = swarm-safe count in the
  chunk (≤5). Master auto-lands them via `just refinery --auto` (whitelisted in code
  to swarm-safe) as polecats exit. Polecats run as the harness's worker-tier model.
- **substrate-sensitive / coherent-block** tickets → sequential, HUMAN-gated. For
  each: `just session-new <slug> --tickets <id> --track <name> --print-prompt`, emit
  the starter prompt verbatim for the user to run in the workspace, then land via
  `just refinery --land <slug>` (never `--auto`). Surface these for the master; the
  loop cannot bypass this gate.

Wait until every chunk ticket is landed on `main` before §4. The gate runs ONCE over
the combined result.

### 4. Verify — exactly ONE soak + separate-model skeptic (GATE 2 source)

- `just soak <oos-seed>` — one full 15-min soak on the out-of-sample seed. A 2nd soak
  ONLY if a check is borderline AND you log the justification to
  `logs/colony-loop-log.jsonl`.
- Launch a **fresh skeptic subagent** (`Task` in Claude Code, `task` in omp) — a
  **different, stronger model than the workers** (a top-tier reviewer model),
  defaulting to REJECT, that ACTS (re-runs,
  re-reads) rather than trusting reports. It runs `just verdict --rationale
  "colony-loop gate" logs/tuned-<oos-seed>`, `just check && just test`, reads each
  landed ticket's §Acceptance, and PASSES only if **all 10** hold:

  1. verdict re-run cold exits 0 vs `logs/baselines/current.json`, no footer regressed >=10%, `--no-history` NOT used
  2. soak is a full 15-min run on an out-of-sample seed (not used in development)
  3. continuity canaries grooming/play/mentoring/burial/courtship/mythic-texture all > 0
  4. declared metric Δ >= threshold AND band ≠ noise per fingerprint/frame-diff; flat ⇒ UNMET (concern), never pass
  5. baseline not promoted mid-chunk (`.label` unchanged since chunk start)
  6. each landed ticket §Acceptance actually satisfied by the diff (not frontmatter-only done)
  7. `just check && just test` green at chunk head; no test deleted/`#[ignore]`/loosened vs pre-chunk
  8. no `src/ai/` or `src/components/` ticket tagged swarm-safe or auto-landed
  9. the 3-5 tickets share cluster/initiative/high similar score (no grab-bag)
  10. every spawned follow-on ticket §Why is genuinely new work, not punted current scope

  High-stakes metric? Escalate to N skeptics reading the SAME one soak (majority-refute
  kills) — multiple verifier agents, NOT multiple soaks.

### 5. Present / Pause — 0 fix rounds

Append one record to `logs/colony-loop-log.jsonl`:
`{chunk_id, ticket_ids, cluster_or_initiative, declared_metric, baseline_label_at_start, verdict, objective_met, follow_on_ids, soaks_run}`.

- **All 10 hold → PASS.** Present: landed tickets on `main`, verdict envelope,
  target-metric Δ (or honest UNMET), follow-on ticket ids opened via
  `just open-ticket`. Then STOP. Never auto-`promote` a PASS chunk to baseline.
- **Any check fails → REJECT.** Hand back the exact broken checks + strongest verified
  partial (labeled NOT done) and pause for the human. Do not iterate.

## Conventions

- **Never modify `main` directly.** Every move to `main` goes through
  `just refinery --land <slug>` (manual) or `--auto` (swarm-safe only).
- **Generator ≠ evaluator.** Workers are the polecat / session agents; the skeptic
  is a different, stronger model. Never collapse them.
- **Caps by construction:** ≤5 tickets, ≤1 soak, 30m polecat wallclock, no fix loop.
  If budget is tight, pause before the expensive soak+verify phase rather than
  half-running it.
- **Never `just promote`** from inside this skill — baseline promotion is a separate
  human action.
- **Read state freshly** every invocation.

## Reference

- Full forged spec: `docs/workflow/colony-loop-spec.md` — the 8 loop-forge dimensions, all rationale.
- Rubric: `docs/balance/healthy-colony.md`; gate: `just verdict`; discovery: `just next`
  / `just similar`; orchestration: `just foreman-spawn` / `session-new` / `refinery`.
- Persistence: ticket corpus (`docs/open-work/`) is the registry;
  `logs/colony-loop-log.jsonl` is the per-chunk memory.
