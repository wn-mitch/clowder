# A1.2 §11 focal-cat replay instrumentation — kickoff

Session-startup artifact for the A1.2 / A5 sub-phase of the AI
substrate refactor. Pairs with
[`ai-substrate-refactor.md`](ai-substrate-refactor.md) §11 and
[`docs/open-work.md`](../open-work.md) #5 cluster-A entry A5.

A5 / A1.2 is the last cluster-A entry outstanding after the A1 trunk
+ A3 + A4 landed. Its job: expose the per-layer transforms the current
`CatSnapshot.last_scores` hides, so CLAUDE.md's Balance Methodology
(hypothesis → prediction → observation → concordance) can operate
against real per-layer evidence instead of "change it and see what
happens." Sidecar JSONL emitter + `scripts/replay_frame.py`
reconstructor; no changes to existing event + narrative logs.

Can proceed in parallel with A1.4 — disjoint file sets.

---

## Kickoff prompt

Paste into a new session. Self-contained — briefs cold.

````
You're picking up A5 from cluster A #5 — §11 focal-cat replay
instrumentation. This is the last cluster-A entry outstanding after
the A1/A2/A3/A4 substrate trunk landed. Its job: expose the per-layer
transforms that the current `CatSnapshot.last_scores` hides, so
CLAUDE.md's Balance Methodology (hypothesis → prediction →
observation → concordance) can operate against real per-layer
evidence instead of "change it and see what happens."

Read these first, in order:

  1. docs/systems/ai-substrate-refactor.md §11 in full (§11.1–§11.7,
     ~line 5851). This is the canonical specification — design
     principle, sampling strategy, record format, joinability
     invariant, scope rules, and out-of-scope items. §11.4
     (joinability) is the load-bearing invariant; read it twice.
  2. docs/open-work.md #5 cluster-A entry A5 — frames the
     "Curvature-at-every-layer traces" design and lists touch points.
  3. docs/open-work.md #14 — specifically the balance-tuning-deferral
     argument. A5 is what moves that deferral forward: without
     per-layer traces, refactor-substrate-stable balance iteration is
     guesswork.
  4. CLAUDE.md "Simulation Verification" + "Balance Methodology" —
     the canonical seed-42 deep-soak + constants-hash header mechanics
     + four-artifact hypothesis acceptance. A5 hooks into the same
     commit-hash header pattern.
  5. src/resources/event_log.rs (current sidecar emitter — model for
     the trace emitter's shape).
  6. src/ai/eval.rs — `evaluate_single` + `evaluate_all_cat_dses` are
     the sites that produce the L2 record data; `ScoredDse` already
     carries `per_consideration`, `raw_score`, `gated_score`,
     `final_score`. Most of the L2 record data is already computed;
     the task is primarily emission, not re-instrumentation.
  7. src/ai/scoring.rs — `score_actions` and `score_dse_by_id` are
     the scoring-layer call sites that will route to the new emitter.
  8. src/ai/modifier.rs — the `ModifierPipeline::apply` loop is where
     per-modifier deltas emit. Each modifier's pre/post score is the
     diff data §11.3 wants.
  9. src/main.rs — `parse_args` for the `--focal-cat NAME` flag
     plumbing + where a headless-only `FocalTraceTarget` resource
     gets inserted.

SCOPE FOR THIS COMMIT — the A5 / §11 trunk:

  - New `FocalTraceTarget` Bevy resource (headless-only). Stores the
    entity id of the cat whose decisions emit per-layer records.
    Resolved at startup from `--focal-cat NAME` against the name
    lookup; if the name matches no cat, print a warning and leave
    the resource unset (tracing silently disables).

  - New `--focal-cat NAME` CLI flag in `src/main.rs::parse_args`.
    Interactive build ignores the flag; headless runner inserts the
    resource only when the flag is present. Matches the existing
    `--log PATH` / `--event-log PATH` plumbing style.

  - Sidecar emitter — one new file alongside `src/resources/event_log.rs`,
    writing to `logs/trace-<focal>.jsonl` by default (path override
    via a new `--trace-log PATH` flag if simple to wire; otherwise
    defer). Line 1 is the commit-hash header per §11.4 — match the
    `event_log.rs` header format byte-for-byte on `commit_hash` /
    `commit_hash_short` / `commit_dirty` / `commit_time` / `seed` /
    `duration_secs` so trace and event log are joinable on those
    fields. Include the same `sim_config` block for tick → season /
    day-phase derivation.

  - Three record variants per §11.3:
      * **L1 record** — lazy emission, ONLY when an L2 consideration
        samples an influence map via `SpatialConsideration` on the
        focal cat. Fields: `tick`, `cat`, `map_key`, `position`,
        `sample_value`. Hook point: `score_consideration` in
        `src/ai/eval.rs` at the `Consideration::Spatial` branch.
        Skip emission when `ctx.cat != focal`.
      * **L2 record** — emit per-DSE per-tick for the focal cat.
        Fields: `tick`, `cat`, `dse_id`, `per_consideration: [{name,
        input, score}]`, `composition: {mode, weights,
        compensation_strength}`, `raw_score`, `maslow_tier`,
        `gated_score`, `modifier_deltas: [{name, pre, post}]`,
        `final_score`. Hook point: `evaluate_single` —
        `ScoredDse` already carries `raw_score/gated_score/final_score`;
        `per_consideration` is the per-score vector; modifier deltas
        need a small extension to `ModifierPipeline::apply` to capture
        pre/post per modifier when a trace sink is active.
      * **L3 record** — per-selection-tick, one record per
        `select_intention_softmax` call on the focal cat. Fields:
        `tick`, `cat`, `pool: [{dse_id, final_score, softmax_weight,
        softmax_probability}]`, `temperature`, `picked_dse_id`,
        `roll` (the RNG draw that resolved the pick — needs
        plumbing into `select_intention_softmax` since it's
        currently internal). Hook point: `select_intention_softmax`
        in `src/ai/eval.rs`.

  - The joinability invariant (§11.4) — every record carries `tick`
    and `cat`. `scripts/replay_frame.py --tick N --cat NAME` must
    reconstruct a single decision frame by joining L1 samples (0+
    records), L2 DSE evaluations (N_eligible_dses records), and L3
    selection (1 record) on (tick, cat). Implementation is
    line-by-line jq-style; tests grep for the fields.

  - New `scripts/replay_frame.py` — CLI tool per §11's exit
    criterion. Input: `--tick N --cat NAME --trace PATH`. Output:
    a pretty-printed reconstruction of the frame (L1 samples
    section, L2 evaluations table, L3 selection summary with
    softmax distribution). Pure Python, reads the JSONL lines.

  - `FocalTraceTarget` resource insertion wired only in the headless
    runner (`src/main.rs`'s headless path) — the interactive build
    does NOT insert it. Per §11.2, focal-cat tracing is a diagnostic
    mode, not a production mode. The sim behaves identically with
    or without the resource present; only emission side-effects
    change.

EXPLICIT NON-GOALS (each is its own later scope; don't scope-creep):

  - §11.6 out-of-scope items: general per-cat trace-sampling at
    lower cadence, in-memory ring buffers for live-debug overlays,
    event-driven rather than tick-driven emission, §11.3.1 backward
    compatibility with pre-A5 logs. All deferred.
  - No retroactive instrumentation of fox DSE evaluation. §11.2
    commits focal-cat as the sampling strategy; the equivalent
    for foxes is a separate scope if it's ever wanted.
  - No GUI / overlay consumer. `scripts/replay_frame.py` is the
    only consumer this commit ships.
  - No changes to `CatSnapshot.last_scores` or
    `logs/events.jsonl` format. Trace emission is a sidecar; the
    existing event + narrative logs stay byte-compatible.
  - No refactor of `event_log.rs`'s header generation into a shared
    helper — copy the pattern, don't consolidate. Consolidation is
    its own refactor.

DELIVERABLES:

  - `FocalTraceTarget` resource + new trace-emitter module + CLI
    flag plumbing.
  - Three record variants emitting per §11.3 schema exactly.
  - `scripts/replay_frame.py` reconstructs a frame from the
    sidecar.
  - Unit tests for the emitter: header-byte-compatibility with
    `event_log.rs` on commit-hash fields; record-shape tests for
    each of L1/L2/L3 variants; lazy-emission test confirming a
    non-focal cat's decisions produce zero trace lines.
  - Landing entry in `docs/open-work.md` Landed section with commit
    hash, a `just soak 42 --focal-cat <NAME>` sample-trace
    reconstruction, and confirmation that survival canaries hold
    (trace emission is side-effect-free by design; soak numbers
    must match a non-focal-cat run).

VERIFICATION:

  - `just check` (cargo check + clippy clean).
  - `just test` (all existing + new emitter tests pass).
  - `just soak 42` (baseline, no focal-cat flag) — survival
    canaries hold; record footer.
  - `just soak 42 --focal-cat <NAME>` (tracing enabled) — survival
    canaries hold; footer bytes match the baseline footer on
    everything except lossy log artifacts (trace sidecar produces
    a separate file; `events.jsonl` and `narrative.jsonl` should
    be byte-identical). Run `scripts/replay_frame.py` against a
    mid-run tick and confirm the output reconstructs a sensible
    decision frame. Commit an example frame reconstruction into
    the landing entry.
  - Per-§11.4 joinability test: manually (or in a small Python
    script) confirm that for at least 10 random (tick, cat=focal)
    pairs, the L2 record's `per_consideration` list matches the
    aggregate of any L1 records at the same (tick, cat), and the
    L3 record's `pool` covers every L2 record emitted that tick
    for the focal cat.

CONVENTIONS:

  - Bevy 0.18 (Messages not Events — see CLAUDE.md ECS Rules).
  - Conventional commit: `feat:` no scope.
  - No magic numbers — any new tunables (default trace path, etc.)
    live in `SimConstants`.
  - Headless-only emission — the production interactive build must
    not carry the trace-emission cost.
  - Additive, not destructive: no behavior changes to existing
    evaluator / modifier / softmax code paths. Emission hooks are
    `if let Some(sink) = focal_trace { sink.emit(…) }` short-
    circuits; when the sink is absent the cost is a single Option
    check.
  - The replay script is Python 3, stdlib-only (no new deps).

OUT OF SCOPE TO RAISE BEFORE IMPLEMENTING:

  - Whether to emit L3 records when the pool is length-1 (softmax
    degenerates to argmax) — yes, emit. §11's joinability argument
    requires every tick-selection to produce one L3 record.
  - Whether to emit L2 records for ineligible DSEs — no. §11.3
    explicitly scopes L2 emission to the eligible pool; the
    eligibility filter skip happens before scoring cost is incurred
    per §4, and ineligible DSEs don't have a `ScoredDse` output.
  - Whether to add a per-tick aggregation record (§11.6 calls this
    out as out-of-scope; don't reopen).
  - Whether to use MessagePack / CBOR instead of JSONL — no. JSONL
    matches `events.jsonl`'s shape and keeps jq-level diagnostics
    trivial.

If §11 ambiguity on record-field names or schema shape surfaces
during implementation, STOP and ask. The spec is load-bearing for
future balance work; alternatives need to be redone.
````

---

## Cross-refs

- Spec: [`ai-substrate-refactor.md`](ai-substrate-refactor.md) §11
  (§11.1 design principle → §11.7 cross-refs)
- Open-work: [`../open-work.md`](../open-work.md) #5 cluster-A entry
  A5 (touch points + rationale) + #14 (balance-tuning-deferral
  argument A5 unblocks)
- Parent kickoff: [`a1-iaus-core-kickoff.md`](a1-iaus-core-kickoff.md)
  phase table row A1.2 ("§11 focal-cat replay", parallelizable with
  A1.1/A1.4)
