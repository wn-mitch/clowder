# Loop spec (forged): related-ticket-cluster colony-improvement loop

> **Status: SETTLED.** All 8 loop-forge dimensions interviewed and confirmed.
> This file is the forged loop specification — the deliverable. loop-forge does
> NOT launch it; §Approach carries the runnable rendering.

## Context

Forge a **loop specification** (not a running loop) for the Clowder repo that:
picks ready tickets off the top of the queue, groups **3–5 generally-related**
ones into a chunk, **confirms a plan with the user** (cluster + intended output
shape) before working, works them, spins up any new tickets discovered en
route, and gates on the **colony score** (`just soak` → `just verdict`). Ad-hoc
trigger (a human invokes it per chunk), not scheduled. The deliverable is the
forged spec + a runnable Claude Code rendering; loop-forge does not launch it.

## Classification (dimension 1 — archetype & artifact)

**Hybrid, convergent-dominant.** Each ticket is *convergent* — it has a
definition-of-done and `just verdict` returns a hard pass/concern/fail. The
per-chunk gate is therefore convergent (all tickets landed + verdict-pass +
colony score not regressed). The umbrella goal "improve the overarching colony
score" is *open/generative* — colony health is a rubric (`docs/balance/healthy-colony.md`,
`just fingerprint`), no single right answer — but it is realized one convergent
chunk at a time. So: **convergent verifier (`just verdict`) nested inside an
open score-improvement objective.** Artifact handed back per chunk: N landed
tickets on `main`, a verdict envelope proving no regression, and any follow-on
tickets opened via `just open-ticket`.

## Running synthesis of the loop
_(updated each interview turn — diff only)_

- **Objective:** advance the colony's health by landing related ticket clusters
  under a per-chunk plan the user approves, gated on `just verdict`.
- **Discovery (SETTLED):** `just next`-seeded top ready ticket + `just similar`
  neighbors; 3–5 related. Metric named to fit the cluster. See §Discovery.
- **Plan checkpoint:** per-chunk, before work — present cluster + intended
  output shape, user confirms. (Pinned by the request.)
- **Stop condition (SETTLED):** two tiers. Hard gate = all chunk tickets landed
  + `just verdict` = pass + no footer regression ≥ drift band vs baseline.
  Declared objective = per-chunk named target metric measured vs baseline;
  improvement ≥ threshold = win, held-but-flat = concern→human. See §Stop condition.
- **Does NOT count (SETTLED):** 10 near-misses. See §Does NOT count.
- **Adversarial audit (SETTLED):** separate skeptic, 1-soak budget, 10 checks +
  voting escalation. See §Adversarial audit.
- **Concurrency (SETTLED):** track-aware hybrid, mandatory per-workspace isolation.
- **Discipline (SETTLED):** 0 fix rounds (pause on reject); one chunk per
  invocation; plan-confirm + reject-pause gates; bounded by construction.

## Dimensions already pinned by the request

- **Dim 8 human checkpoint (partial):** a plan-confirm gate per chunk is
  mandatory — "always have a plan to ensure with the user this is the desired
  shape of the output." Confirming, will detail budget + landing gate later.
- **Dim 5 chunk size:** 3–5 tickets, generally related. Fixed.

## Stop condition (dimension 2 — settled)

Two tiers, both graded from artifacts a cold model can read:

1. **Hard gate (always blocking — governs "safe to land"):** every ticket in
   the chunk is landed on `main`; `just verdict logs/tuned-<seed>` exits `0`
   (pass); and no footer field regressed ≥ the drift band (10%) vs the active
   baseline (`logs/baselines/current.json`). A `concern`/`fail` verdict or any
   ≥10% regression blocks the chunk.
2. **Declared objective (per chunk — governs "score win", never silently
   waived):** the chunk's plan names ONE target metric from the healthy-colony
   rubric (`docs/balance/healthy-colony.md`). The loop measures its Δ vs
   baseline (`just fingerprint` + `just frame-diff`). Improvement ≥ the metric's
   threshold = objective met. Held-flat or noise = `concern` surfaced to the
   human checkpoint (accept / iterate / reopen) — the loop NEVER reports a
   colony-score win it did not measure.

A cold checker grades tier 1 from the verdict exit code + footer_drift array,
and tier 2 from the named metric's fingerprint band. No memory of the design
conversation is required.

## Does NOT count as done (dimension 3 — settled)

1. Verdict laundering — `concern`/`fail` treated as pass; lenient hand-picked
   `--baseline`; `verdict --no-history` to dodge the regression store.
2. Gate-gaming — a test deleted / `#[ignore]`'d / assertion loosened to green
   `just check && just test`.
3. Score-win from noise or one seed — Δ < 10% (in-band) called "improved"; a
   single lucky seed reported without sweep/rep confirmation.
4. Baseline ratcheting — the loop `just promote`-ing its own chunk run as the
   new baseline, hiding a regression from the next chunk's check.
5. Grab-bag chunk — 3–5 tickets not actually related (different cluster, no
   shared initiative, low `similar` score).
6. Frontmatter-only done — `done` flipped with §Acceptance unmet; substrate
   ticket reframed docs-only to dodge the layer-walk.
7. Follow-on dumping ground — `open-ticket` used to defer the current chunk's
   scope rather than capture genuinely new work.
8. Partial chunk as complete — some tickets abandoned, chunk reported done
   without explicit re-scope with the user.
9. Human-gate evasion — substrate-sensitive ticket mis-tagged `swarm-safe` so
   `refinery --auto` lands it unreviewed.
10. Collapse masked — verdict "pass" while a continuity canary sits at zero, or
    a sub-canonical soak that never surfaces a slow-burn collapse.

## Adversarial audit (dimension 4 — settled)

The verifier is a **different agent, ideally a different model** than any doer,
**defaults to REJECT**, and **acts** (re-runs, re-reads, re-derives) rather than
trusting the doer's report. Given a candidate chunk (landed diff + soak run dir
+ ticket list), it runs every check; **PASS only if all hold, else REJECT +
list each broken check**.

**Soak budget: exactly ONE soak per chunk by default.** The verifier reuses that
single gate run plus the *existing* baseline sweep corpus for variance context;
it does NOT spawn fresh soaks. A second soak is run ONLY when a check below is
borderline and the run log records the explicit justification.

1. **Re-run the gate cold** — run `just verdict logs/tuned-<seed>` itself (no new
   soak — same run dir); confirm exit 0, `--baseline` resolved to
   `logs/baselines/current.json` (not a hand-passed lenient path), and the
   `verdict-history.jsonl` record exists.
2. **Soak was canonical + out-of-sample** — footer shows a full 15-min soak
   (tick count/duration), AND its seed was NOT used while developing/tuning the
   chunk (the gate soak is out-of-sample by construction, so ONE run doubles as
   the anti-overfitting check — subsumes old check 11).
3. **Continuity canaries non-zero** — independently confirm grooming / play /
   mentoring / burial / courtship / mythic-texture > 0 in the footer.
4. **Declared metric moved for real** — `just fingerprint` + `just frame-diff`
   on the named target vs baseline: improvement ≥ threshold AND band ≠ noise
   (≥10%). Variance context comes from `just sweep-stats <baseline-sweep> --vs`
   (existing corpus, no new soak). Flat/in-noise ⇒ objective UNMET (concern).
   **Borderline** (Δ within one band of threshold, or metric flagged
   high-variance by `logs/sensitivity-map.json`) ⇒ the ONE justified extra soak
   on a second out-of-sample seed; log the justification.
5. **Baseline integrity** — the loop did NOT `just promote` its own run into
   `logs/baselines/current.json` mid-chunk; regression check is vs the pre-chunk
   baseline (compare the baseline label recorded at chunk start vs now).
6. **§Acceptance actually met** — open each `landed/NNN.md`, read §Acceptance,
   confirm the diff satisfies it (inspect the code/test that proves it) — reject
   `status: done` with unmet acceptance.
7. **Tests genuinely green, nothing gutted** — `just check && just test` cold at
   the chunk head; `jj diff` on `tests/` + test modules shows no deletion,
   `#[ignore]`, or loosened assertion vs pre-chunk.
8. **Layer-walk honored** — any ticket touching `src/ai/` or `src/components/`
   is NOT tagged `swarm-safe` and was NOT `refinery --auto`-landed; bugfix /
   substrate discipline followed.
9. **Cluster is real** — the 3–5 tickets share a `cluster` or `initiative` or
   high pairwise `similar` score (reject grab-bag).
10. **Follow-ons are new work** — each spawned `open-ticket`'s §Why references
    genuinely-discovered work, not a punted slice of the current chunk's scope.

Escalation: for a chunk whose declared metric is high-stakes, replace the single
verifier with N skeptics (adversarial voting, majority-refute to kill) — this is
multiple *verifier agents* reading the SAME one soak, not multiple soaks.

## Concurrency & isolation (dimension 7 — settled)

**Track-aware hybrid.** The loop reads each ticket's `orchestration` frontmatter:

- `swarm-safe` tickets → parallel **polecats** via `just foreman-spawn N`; the
  master lands them with `just refinery --auto` (whitelisted in code to
  swarm-safe) when they exit.
- `substrate-sensitive` / `coherent-block` tickets → **sequential**, one
  isolated session each via `just session-new <slug> --tickets <id> --track …`,
  human-gated landing via `just refinery --land <slug>`.

**Isolation is mandatory, not optional:** every file-mutating worker runs in its
own `~/clowder-sessions/<slug>/` jj workspace (own working copy + `target/`);
parallel edits never touch the same working tree. `main` is advanced only by the
refinery. The chunk gate (one soak + `just verdict`) runs once over the combined
landed result, after all the chunk's tickets are on `main`.

Concurrency cap: polecat fan-out ≤ the swarm-safe ticket count in the chunk
(≤5), well under the foreman default and the executor's parallelism ceiling.

## Discovery portfolio (dimension 5 — settled)

**next-seeded adjacency.** Assemble the chunk:

1. `just next` (optionally `--initiative <name>`) → the top ready ticket
   adjacent to recent landings / in-flight momentum. This is "off the top."
2. Grow to 3–5 with `just similar <seed-id> --corpus tickets` +
   `just open-work-ready-filtered --cluster/--initiative`, keeping only `ready`
   tickets that share a `cluster` or `initiative` or clear the `similar`
   relatedness bar (ties to audit check 9).
3. Name the ONE declared target metric the assembled cluster plausibly moves
   (from `docs/balance/healthy-colony.md`). Cluster chosen FIRST (off the top +
   related); metric named to fit it. Both surfaced in the plan-confirm.

Independence discipline: the loop does not pre-commit each ticket to a solution;
where a ticket is genuinely open, its worker session explores its own approach.
Cluster relatedness is the only coupling — never force unrelated high-priority
tickets in to hit 5. A 3-ticket related chunk beats a 5-ticket grab-bag.

## Persistence / registry (dimension 6 — settled)

The **ticket corpus is the registry** — no new schema invented:

- `docs/open-work/tickets/NNN.md` frontmatter (`status`, `blocked-by`,
  `cluster`, `initiative`, `orchestration`) = live open/blocked/ready state;
  `docs/open-work/landed/NNN.md` = done archive. `just land` / `just open-ticket`
  are the ONLY writers.
- **Dedup:** only `status: ready` tickets are pickable (never re-pick
  in-progress/done/blocked); never re-open a parked-bankrupt or wontfix ticket.
  This is dedup against the full corpus-state, not just "confirmed."
- `logs/verdict-history.jsonl` + `logs/agent-call-history.jsonl` = existing
  gate/history stores (always pass `verdict --rationale "<why>"`).
- The loop appends ONE per-chunk record to `logs/colony-loop-log.jsonl`:
  `{chunk_id, ticket_ids, cluster_or_initiative, declared_metric,
  baseline_label_at_start, verdict, objective_met, follow_on_ids, soaks_run}`.
  Cross-invocation memory: a later chunk sees what the last targeted and whether
  its metric moved.

## Discipline & guardrails (dimension 8 — settled)

- **Return contract, inverted toward the human — 0 fix rounds.** Any REJECT
  (verdict `concern`/`fail`, OR any audit check failing) **pauses immediately**
  and hands back the reject + each broken check + the strongest verified
  partial, explicitly labeled NOT done. The loop never self-repairs silently and
  never re-runs the gate to fish for a pass.
- **One chunk per invocation.** Ad-hoc: a PASS hands back the verified result +
  follow-on ticket ids and STOPS. The human re-invokes for the next chunk.
- **Human checkpoints (two):** (1) **plan-confirm** before any work — cluster +
  declared metric + intended output shape (which tickets, each ticket's track,
  expected diff shape); user accepts / edits / replaces. (2) **reject-pause** on
  any gate reject. Substrate-sensitive / coherent-block always land human-gated
  via `refinery --land`; only swarm-safe auto-lands via `refinery --auto` during
  the chunk. Even a PASS chunk is *presented*, never auto-`promote`d to baseline.
- **Budget, bounded by construction:** chunk ≤5 tickets; ≤1 soak (a 2nd only on a
  logged borderline justification); polecat wallclock 30m each; no inner fix loop
  ⇒ no unbounded iteration. A `budget.remaining()` floor guards the expensive
  soak+verify phase — under the floor, pause and hand back before soaking.

## Approach — the forged loop spec (execute this)

A fresh agent executes this section with no other context. Part A is portable;
Part B is the Claude Code binding. loop-forge does not launch it — present and stop.

### Part A — Prose core (portable)

**Objective.** Advance the Clowder colony's health by landing ONE cluster of 3–5
generally-related ready tickets per invocation, under a plan the user approves,
such that the colony-score gate (one out-of-sample soak → `just verdict`) passes
with no regression and the plan-declared target metric's movement is measured and
honestly reported. Artifact returned: the landed tickets on `main`, the verdict
envelope, the target-metric Δ, and any follow-on tickets opened via
`just open-ticket` — or, on reject, the broken checks + strongest partial.

- **Stop condition** — §Stop condition (two tiers: hard non-regression gate +
  declared-metric objective).
- **Does NOT count** — §Does NOT count (10 near-misses).
- **Adversarial audit** — §Adversarial audit (separate skeptic, 1-soak budget,
  10 checks; PASS only if all hold, else REJECT + list broken checks).
- **Discovery** — §Discovery portfolio (next-seeded adjacency).
- **Persistence** — §Persistence (corpus is the registry + `colony-loop-log.jsonl`).
- **Concurrency & isolation** — §Concurrency (track-aware hybrid; per-workspace).
- **Discipline** — §Discipline (0 fix rounds; one chunk/invocation; two human gates).

**Return contract.** Return the chunk as DONE only when the §Stop condition hard
gate passes AND it survives the §Adversarial audit AND the declared metric is
either improved ≥ threshold or explicitly reported UNMET (never silently). Do
NOT return: a partial chunk (some tickets abandoned) as complete; a `concern`/
`fail` verdict relabeled pass; a score "win" measured only in the noise band or
on a development seed; a chunk landed by loosening a test or auto-landing
substrate-sensitive work; a new promoted baseline. If the gate rejects, return
the strongest verified partial, the exact broken checks, and pause for the human.

### Part B — Claude Code rendering

#### B1 — the `/goal` stop line

```
/goal A 3–5 ticket related cluster is DONE only when: every ticket is landed on
main; `just verdict logs/tuned-<oos-seed>` exits 0 against logs/baselines/current.json
with no footer field regressed ≥10%; every continuity canary
(grooming/play/mentoring/burial/courtship/mythic-texture) > 0; the plan-declared
target metric's Δ vs baseline is ≥ its threshold and outside the noise band per
`just fingerprint`/`just frame-diff` (or, if flat, reported UNMET — not passed);
exactly one soak ran (a 2nd only with a logged borderline justification); no test
was deleted/#[ignore]/loosened; no substrate-sensitive ticket was auto-landed; and
no baseline was promoted mid-chunk. A fresh skeptic (different model) re-runs
verdict + `just check && just test` and re-reads each ticket's §Acceptance before
granting pass; any failing check → not done, pause for the human.
```

#### B2 — the `Workflow` skeleton

The chunk is worked by the `just` orchestration primitives (which own worker
isolation); the Workflow's job is to wire the ONE soak + the SEPARATE skeptic
audit + persistence, with caps. Adapt `PLAN` per chunk (it is the plan-confirm
output).

```javascript
export const meta = {
  name: 'colony-ticket-loop',
  description: 'Work one 3–5 ticket related cluster to a verified colony-score gate.',
  phases: [{ title: 'Discover' }, { title: 'Work' }, { title: 'Verify' }, { title: 'Present' }],
}

// One chunk per invocation. PLAN is the APPROVED plan-confirm output from the
// master session — the Workflow never works an unconfirmed cluster.
const PLAN = {
  clusterTickets: [312, 315, 318],                 // ≤5, related (audit check 9)
  trackByTicket:  { 312: 'swarm-safe', 315: 'substrate-sensitive', 318: 'substrate-sensitive' },
  declaredMetric: 'kitten_survival_rate',          // from healthy-colony rubric
  metricThreshold: 0.10,                           // ≥10%, outside noise band
  oosSeed: 91,                                     // NOT used developing these tickets
  baselineLabelAtStart: null,                      // captured in Discover
}

// The dimension-4 checklist IS the verifier's script.
const AUDIT = [
  'verdict re-run cold exits 0 vs logs/baselines/current.json, no footer regressed ≥10%, --no-history NOT used',
  'soak is a full 15-min run on an out-of-sample seed (not used in development)',
  'continuity canaries grooming/play/mentoring/burial/courtship/mythic-texture all > 0',
  'declared metric Δ ≥ threshold AND band ≠ noise per fingerprint/frame-diff; flat ⇒ UNMET (concern), never pass',
  'baseline not promoted mid-chunk (baseline label unchanged since chunk start)',
  'each landed ticket §Acceptance actually satisfied by the diff (not frontmatter-only done)',
  'just check && just test green at chunk head; no test deleted/#[ignore]/loosened vs pre-chunk',
  'no src/ai/ or src/components/ ticket tagged swarm-safe or auto-landed',
  'the 3–5 tickets share cluster/initiative/high similar score (no grab-bag)',
  'every spawned follow-on ticket §Why is genuinely new work, not punted current scope',
]
const VERDICT = {} // JSON Schema: { survives: boolean, brokenChecks: string[], reasons: string }
let soaksRun = 0

phase('Discover')
// Cluster chosen + CONFIRMED in the master session (plan-confirm gate). Record baseline label.
PLAN.baselineLabelAtStart = (await tool.bash({ command: 'jq -r .label logs/baselines/current.json 2>/dev/null || echo none' })).stdout.trim()

phase('Work')
if (budget.total && budget.remaining() < 200_000) { log('budget floor hit'); return { paused: 'budget', plan: PLAN } }
// swarm-safe → parallel polecats, each in its OWN ~/clowder-sessions workspace (isolation via session-new); refinery --auto lands on exit.
const swarm = PLAN.clusterTickets.filter(id => PLAN.trackByTicket[id] === 'swarm-safe')
if (swarm.length) await tool.bash({ command: `just foreman-spawn ${swarm.length}` })
// substrate-sensitive / coherent-block → sequential, HUMAN-gated: worked in ~/clowder-sessions/<slug>,
// landed via `just refinery --land <slug>`. The Workflow CANNOT bypass this gate — it surfaces them for the master.

phase('Verify')                                    // runs ONCE over the combined landed result — one soak.
await tool.bash({ command: `just soak ${PLAN.oosSeed}` }); soaksRun++
const judged = await agent(
  `Adversarially verify this landed chunk. Default to REJECT. Re-run/re-read; do not trust reports.\n` +
  `Run just verdict --rationale "colony-loop gate" logs/tuned-${PLAN.oosSeed}, just check && just test, and read each ticket's §Acceptance.\n` +
  `PASS only if ALL hold:\n${AUDIT.map((x,i)=>`${i+1}. ${x}`).join('\n')}\n\n` +
  `Chunk: ${JSON.stringify(PLAN)}. soaksRun=${soaksRun} (a 2nd soak allowed ONLY with a logged borderline justification).`,
  { phase: 'Verify', model: 'opus', effort: 'high', schema: VERDICT }   // DIFFERENT model than the sonnet polecats
)

phase('Present')                                   // 0 fix rounds: PASS → present + stop; REJECT → pause. Never auto-promote a baseline.
await tool.bash({ command: `printf '%s\\n' ${JSON.stringify(JSON.stringify({ ...PLAN, verdict: judged?.survives ? 'pass':'reject', soaksRun }))} >> logs/colony-loop-log.jsonl` })
return { survives: judged?.survives === true, brokenChecks: judged?.brokenChecks ?? [], plan: PLAN, soaksRun }
```

Carry into the rendering:
- **Generator ≠ evaluator.** Workers are sonnet polecats / session agents; the
  verifier is a different model (opus) that ACTS. Never collapse them.
- **Isolation is already handled** by `session-new` / `foreman-spawn` (each
  worker gets its own `~/clowder-sessions/<slug>` jj workspace). Do NOT add a
  second layer of `isolation: 'worktree'` in the Workflow — that double-nests.
- **Caps by construction:** ≤5 tickets, ≤1 soak, 30m polecat wallclock, no fix
  loop, `budget.remaining()` floor. These are the token-blowout breakers.
- **Human checkpoints:** plan-confirm precedes launch (master session);
  substrate-sensitive lands via `refinery --land`; a PASS chunk is presented,
  never auto-promoted.
- **Adversarial voting** (optional, high-stakes metric): replace the single
  `agent(...)` with N skeptics reading the SAME one soak; majority-refute kills.

#### B3 — worktree note

Workers mutate files, but each runs in its own `~/clowder-sessions/<slug>` jj
workspace created by `session-new` / `foreman-spawn` — isolation is handled by
the orchestration primitives. The read-only Discover scouts (`just next` /
`similar`) and the Verify skeptic need no worktree.

## Verification (prove the spec is executable)

Run these to confirm every primitive the spec leans on exists and behaves as
claimed (from `~/clowder`):

1. **Discovery yields a real related cluster:** `just next` then
   `just similar <top-id> --corpus tickets` — eyeball that 3–5 `ready` tickets
   share a cluster/initiative. NEW-behavior check: the assembled cluster is
   genuinely related, not grab-bag.
2. **Gate contract holds:** `just verdict <an-existing-run-dir>`; confirm the
   JSON envelope + exit code 0/1/2 (pass/concern/fail). Confirm baseline
   resolution: `jq . logs/baselines/current.json` (or note `no-baseline`).
3. **Polecat path dry-runs without landing:** `just foreman-spawn 1 --dry-run`
   (plans + rolls back the claim).
4. **Cold-checkability of the stop condition:** hand a fresh model ONLY the B1
   `/goal` line + an existing run dir and confirm it can grade pass/fail with no
   memory of this conversation. If it cannot, the stop condition is underspecified.
5. **Separation is real:** the Verify `agent` model differs from the workers'.

## Assumptions & contingencies

- **Metric ↔ rubric.** Assumes the declared metric appears in
  `docs/balance/healthy-colony.md` / `just fingerprint`. If it does not, fall
  back to the footer field + `just frame-diff` + `just sweep-stats <baseline-sweep>
  --vs`. If no baseline sweep exists for variance context, treat any borderline
  as REJECT (conservative) rather than spending a 2nd soak.
- **Baseline present.** Assumes `logs/baselines/current.json` exists. If verdict
  returns `no-baseline`, tier-1 degrades to canaries-only and tier-2 metric Δ is
  uncomputable → objective auto-UNMET (concern); pause for the human to
  `just promote` a baseline first. The loop never promotes one itself.
- **Plan-confirm before work.** Assumes the master session runs the plan-confirm
  gate before launching the Workflow. If driven headless, the first invocation
  MUST return the proposed `PLAN` and only proceed on a second invocation
  carrying approval — never work an unconfirmed cluster.
- **Per-DSE metrics.** If the declared metric is a per-DSE behavior, use
  `just soak-trace <oos-seed> <cat>` + `frame-diff` in place of plain `soak`
  (still ONE trace-soak) and pick an eligible focal cat per the multi-focal
  convention in `docs/workflow/commands.md`.

## Self-audit (loop-forge Phase 3)

- **Cold-checkable stop condition?** Yes — B1 `/goal` + exit code + footer_drift
  + fingerprint band; no conversation memory needed.
- **Verification separate & acting?** Yes — different model (opus vs sonnet),
  defaults REJECT, re-runs verdict/tests, re-reads §Acceptance.
- **Equivalent-strength trap?** Absent — the gate (mechanical verdict + audit)
  is strictly weaker than doing the tickets; it cannot secretly re-pose the task.
- **Anti-conditions non-empty?** Yes — 10, convergent-task requirement met.
- **Caps bounded by construction?** Yes — ≤5 tickets, ≤1 soak, 30m wallclock,
  no fix loop, budget floor. Riskiest baked-in assumption: that a related 3–5
  ticket cluster can plausibly move ONE named colony metric ≥10% in a single
  chunk — if chunks routinely land clean but flat, tighten cluster→metric
  selection (option "metric-targeted" in dim 5) rather than loosening the gate.

