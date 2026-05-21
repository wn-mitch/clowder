set positional-arguments := true

# Run the simulation
run *ARGS:
    cargo run -- {{ARGS}}

# Run with a specific seed
seed SEED:
    cargo run -- --seed {{SEED}}

# Load from autosave
load:
    cargo run -- --load saves/autosave.json

# Run headless simulation (default 60s)
headless *ARGS:
    cargo run -- --headless {{ARGS}}

# Stage H gate (ticket 431) — refuses if target/release/clowder's baked
# commit ≠ HEAD. build.rs already reruns on .git/HEAD changes, but a
# stale incremental cache can leave the binary baked at an older commit.
# The gate forces a `cargo build --release` and verifies the baked
# GIT_HASH via `clowder --print-build-info` before any soak commits to a
# long-running run.
_check-binary-fresh:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --release --quiet
    BAKED=$(target/release/clowder --print-build-info | sed -n 's/^commit=\([^ ]*\).*/\1/p')
    HEAD=$(git rev-parse HEAD)
    if [ "$BAKED" != "$HEAD" ]; then
      echo "REFUSED: target/release/clowder baked commit $BAKED ≠ HEAD $HEAD" >&2
      echo "  build.rs may not have rerun on the last commit; try:" >&2
      echo "    cargo clean -p clowder && cargo build --release" >&2
      exit 2
    fi

# Canonical 15-min deep-soak at a fixed seed. Release build, writes to
# logs/tuned-<seed>-<commit_short>/{events,narrative}.jsonl. See CLAUDE.md
# and docs/diagnostics/log-queries.md for verification.
#
# Archive directory naming convention is enforced (ticket 431 Stage H):
# the `<commit_short>` suffix makes `ls logs/` self-validating without
# `jq` on the header. Refuses to overwrite if the same seed × commit has
# already been soaked (genuine re-run case only).
soak SEED="42": _check-binary-fresh
    #!/usr/bin/env bash
    set -euo pipefail
    SHORT_HASH=$(git rev-parse --short HEAD)
    OUTDIR="logs/tuned-{{SEED}}-${SHORT_HASH}"
    if [ -s "${OUTDIR}/events.jsonl" ]; then
      echo "REFUSED: ${OUTDIR}/events.jsonl exists (same seed × commit)." >&2
      echo "  Either commit your changes (so commit_short advances) or" >&2
      echo "  rename the existing directory if it captures a different run:" >&2
      echo "    mv ${OUTDIR} ${OUTDIR}-<suffix>" >&2
      exit 2
    fi
    mkdir -p "${OUTDIR}"
    cargo run --release -- --headless --seed {{SEED}} --duration 900 \
      --log "${OUTDIR}/narrative.jsonl" \
      --event-log "${OUTDIR}/events.jsonl"

# Run the canary checks (starvation, shadowfox, activation, wipeout)
# against an events.jsonl. Exits non-zero on any failure. Default target
# is the latest tuned run.
check-canaries LOGFILE="logs/events.jsonl":
    scripts/check_canaries.sh {{LOGFILE}}

# Run the continuity-canary checks (grooming, play, mentoring, burial,
# courtship, mythic-texture) against an events.jsonl footer. Exits
# non-zero when any canary class fired zero times in the soak.
# Continuity canaries gate behavioural range — substrate refactor
# Phase 3+ requires them to strengthen (not just non-regress).
check-continuity LOGFILE="logs/events.jsonl":
    scripts/check_continuity.sh {{LOGFILE}}

# Query tools over a sim run's JSONL logs. Wraps the jq recipes in
# docs/diagnostics/log-queries.md as seven parameterized subtools with
# a consistent envelope (query echo, scan stats, stable IDs, narrative,
# next-query hints). Intended as an agent-friendly drill-down surface,
# complementing /diagnose-run's fixed-shape report.
#
# Subtools: run-summary | events | deaths | narrative | trace |
#           cat-timeline | anomalies
# See `just q <subtool> --help` for flags, or .claude/skills/logq/SKILL.md.
#
# Examples:
#   just q run-summary logs/tuned-42
#   just q deaths logs/tuned-42 --cause=Starvation
#   just q trace logs/tuned-42 Simba --layer=L3
#   just q anomalies logs/tuned-42
#   just q cat-timeline logs/tuned-42 Simba --tick-range=3800..4000
q *ARGS:
    @python3 scripts/logq/logq.py {{ARGS}}

# Run logq's envelope + subtool tests (stdlib unittest, no pytest dep).
# Runs the file directly because `unittest discover` requires the
# tests/ dir to be a Python package (which would mean adding an
# __init__.py and polluting the Rust test layout).
test-logq:
    python3 tests/logq/test_envelope.py -v

# Ticket 417: scripts/llm/claude_client — headless `claude` CLI wrapper
# for presenter-layer enrichment. All subprocess calls mocked; no live
# network. Same stdlib-unittest pattern as test-logq.
test-llm:
    python3 tests/llm/test_claude_client.py -v

# Ticket 125: verdict's colony_score_drift channel — bucket boundaries
# + escalation logic + per-field shape. Same stdlib-unittest pattern as
# test-logq. Ticket 396 adds the plan-failure canary test set.
test-verdict:
    python3 tests/verdict/test_colony_score_drift.py -v
    python3 tests/verdict/test_plan_failure_canary.py -v

# Ticket 229: similar.py chunkers + retrieval — pure-Python tests with
# a deterministic fake embedder so the suite runs without downloading
# the BGE-small model. Real-fastembed verification happens via
# `just similar-build` + smoke queries.
test-similar:
    python3 tests/similar/test_chunkers.py -v
    python3 tests/similar/test_retrieve.py -v

# Semantic retrieval over Clowder prose (tickets, landed, balance,
# system docs, DSE doc-comments). See `.claude/skills/similar/SKILL.md`.
# Three input shapes auto-detected:
#   just similar 189                          # ticket id (centroid query)
#   just similar tickets/175.md               # repo-relative file path
#   just similar "starvation cluster"         # free text
# Initiative-scoped modes (no positional input):
#   just similar --centroid world-richness    # centroid of tagged members → all neighbors
#   just similar --not-tagged world-richness  # same centroid → exclude tagged (discovery)
#   just similar 189 --initiative world-richness  # normal query, results filtered
similar *ARGS:
    @uv run scripts/similar/similar.py {{ARGS}}

# Build / refresh the embedding index used by `just similar`. Runs
# incrementally by default — only re-embeds files whose mtime exceeds
# the recorded one. Pass `--full` to force a from-scratch rebuild.
similar-build *ARGS:
    @uv run scripts/similar/index.py {{ARGS}}

# Surface ticket pairs that are conceptually adjacent but not formally
# linked — embedding-based discovery of cross-epic ticket relationships
# the `cluster:` field can't catch. Excludes pairs that already
# cross-reference each other in body / blocked-by / supersedes.
#   just similar-linkages                        # top 30 pairs, threshold 0.75
#   just similar-linkages --cross-cluster        # only cluster-A ↔ cluster-B
#   just similar-linkages --ticket 189           # all unlinked neighbors of 189
#   just similar-linkages --threshold 0.7        # lower bar, more candidates
similar-linkages *ARGS:
    @uv run scripts/similar/linkages.py {{ARGS}}

# Bulk linkage curation across all open tickets — writes a top-level
# report at `docs/open-work/_linkages.md` AND injects a `## Related
# work` section into each open ticket whose candidates clear the
# threshold. Re-runs are idempotent (the auto-marked block is
# replaced, prose outside it is preserved).
#   just similar-link-report                     # threshold 0.78, top-3 per ticket
#   just similar-link-report --report-only       # navigation aid only, no per-ticket edits
#   just similar-link-report --threshold 0.80    # tighter, fewer candidates
similar-link-report *ARGS:
    @uv run scripts/similar/link_report.py {{ARGS}}

# Embedding-based ready-ticket recommender. Surfaces a top-K of
# unblocked tickets adjacent to recent landings, current in-flight
# work, the AI-substrate refactor, or an ad-hoc seed. Reads
# `logs/.embeddings/` and auto-rebuilds the index when stale (cheap
# incremental refresh); pass `--no-auto-rebuild` to preserve strict
# read-only behavior. See `.claude/skills/next/SKILL.md`.
#   just next                                  # blend (momentum + wip + substrate)
#   just next --mode momentum                  # last-N landed centroid only
#   just next --mode wip                       # in-progress cohesion only
#   just next --mode substrate                 # AI-refactor alignment only
#   just next --mode seed --seed 256           # ticket-id seed
#   just next --mode seed --seed "starvation"  # free-text seed
#   just next --top 10 --text                  # widen + render text envelope
#   just next --no-auto-rebuild                # skip stale-index auto-rebuild
#   just next --initiative world-richness      # scope to one initiative
next *ARGS:
    @uv run scripts/similar/next.py {{ARGS}}

# Deep-soak with a focal-cat trace sidecar. Writes to
# logs/tuned-<seed>-<commit_short>/{events,narrative,trace-<focal>}.jsonl.
# Trace records decompose per-tick L1/L2/L3 state for one focal cat per
# §11 of docs/systems/ai-substrate-refactor.md. Archive naming + binary-
# freshness gate match `just soak` (ticket 431 Stage H).
soak-trace SEED="42" FOCAL_CAT="Simba" DURATION="900": _check-binary-fresh
    #!/usr/bin/env bash
    set -euo pipefail
    SHORT_HASH=$(git rev-parse --short HEAD)
    OUTDIR="logs/tuned-{{SEED}}-${SHORT_HASH}"
    if [ -s "${OUTDIR}/events.jsonl" ] || [ -s "${OUTDIR}/trace-{{FOCAL_CAT}}.jsonl" ]; then
      echo "REFUSED: ${OUTDIR}/ already has soak-trace output (same seed × commit)." >&2
      echo "  Either commit your changes (so commit_short advances) or" >&2
      echo "  rename the existing directory if it captures a different run:" >&2
      echo "    mv ${OUTDIR} ${OUTDIR}-<suffix>" >&2
      exit 2
    fi
    mkdir -p "${OUTDIR}"
    cargo run --release -- --headless --seed {{SEED}} --duration {{DURATION}} \
      --focal-cat {{FOCAL_CAT}} \
      --log "${OUTDIR}/narrative.jsonl" \
      --event-log "${OUTDIR}/events.jsonl" \
      --trace-log "${OUTDIR}/trace-{{FOCAL_CAT}}.jsonl"

# Ticket 430 — sampling profile via `samply` against the
# `profile.profiling` Cargo profile (release + embedded debug
# symbols via packed .dSYM). Captures `DURATION` seconds (default
# 60s) of CPU samples at 997 Hz; writes profile JSON + symbol
# sidecar to `logs/flamegraphs/<seed>-<commit>/`. Analyze with
# `python3 scripts/profiling/samply_top.py <profile-dir>` (or load
# in https://profiler.firefox.com).
#
# macOS notes: `cargo-flamegraph` 0.6.x requires full Xcode (uses
# xctrace); samply doesn't, but needs the dSYM bundle that
# `[profile.profiling]` with `split-debuginfo = "packed"` produces.
# No sudo needed.
#
# Usage:
#   just flamegraph                # seed=42, 60s
#   just flamegraph 17 30          # seed=17, 30s
flamegraph SEED="42" DURATION="60":
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v samply >/dev/null 2>&1; then
      echo "error: samply not installed. Run: brew install samply" >&2
      exit 1
    fi
    SHA=$(jj log -r '@-' --no-graph -T 'commit_id.short()' 2>/dev/null || git rev-parse --short HEAD)
    OUTDIR="logs/flamegraphs/{{SEED}}-${SHA}"
    mkdir -p "${OUTDIR}"
    echo "Building profile binary (cargo build --profile profiling)..."
    cargo build --profile profiling --quiet
    echo "Recording ${DURATION}s sample profile @ 997Hz..."
    samply record \
      --save-only --no-open \
      --output "${OUTDIR}/profile.json.gz" \
      --rate 997 \
      --unstable-presymbolicate \
      -- target/profiling/clowder \
         --headless --seed {{SEED}} --duration {{DURATION}} \
         --log /tmp/flamegraph.narrative.jsonl \
         --event-log /tmp/flamegraph.events.jsonl
    echo "wrote ${OUTDIR}/profile.json.gz + .syms.json"
    echo "  open in https://profiler.firefox.com or analyze via scripts/profiling/samply_top.py"

# Ticket 162 — scenario microexperiment harness. Runs a registered
# scenario (preset cats, preloaded needs/personality/markers) for a
# small number of ticks and prints per-tick winning DSE + final-tick
# ranked score table for the focal cat. Cheap (~3s) decision-landscape
# triage tool — preferred over `just soak` for hypothesis triage during
# bugfix loops (see CLAUDE.md "Bugfix discipline").
#
# Usage:
#   just scenario kitten_cry_basic
#   just scenario kitten_cry_basic --focal Pyre --ticks 20
#   just scenario list
scenario *ARGS:
    cargo run --release --bin scenario --quiet -- {{ARGS}}

# Pairwise frame-diff over two focal-cat traces, with optional
# hypothesis overlay from a phase balance doc. Emits per-DSE drift
# stats ranked by |Δ mean| and an overall concordance verdict.
#
# Example:
#   just frame-diff logs/baseline-pre-substrate-refactor/trace-Simba.jsonl \
#                   logs/tuned-42/trace-Simba.jsonl \
#                   docs/balance/substrate-phase-3.md
frame-diff BASELINE NEW HYPOTHESIS="" *EXTRA_ARGS="":
    #!/usr/bin/env bash
    if [[ -n "{{HYPOTHESIS}}" ]]; then
        uv run scripts/frame_diff.py {{BASELINE}} {{NEW}} --hypothesis {{HYPOTHESIS}} {{EXTRA_ARGS}}
    else
        uv run scripts/frame_diff.py {{BASELINE}} {{NEW}} {{EXTRA_ARGS}}
    fi

# One-call run validation. Composes check-canaries + check-continuity +
# diff-constants + footer-vs-baseline drift into a structured JSON
# envelope (or a human summary with --text). Exit codes:
#   0 = pass, 1 = concern, 2 = fail.
# Reads the active baseline from logs/baselines/current.json; falls back
# to logs/baseline-pre-substrate-refactor/events.jsonl. Pass --baseline
# to override.
#
# Examples:
#   just verdict logs/tuned-42
#   just verdict logs/tuned-42 --text
#   just verdict logs/tuned-42 --baseline logs/baseline-2026-04-25/events.jsonl
verdict *ARGS:
    @uv run scripts/verdict.py {{ARGS}}

# HTN method-registry audit surface (ticket 319 — 128 epic infrastructure).
# Lists every registered method (live + dormant) with its source file
# and, for PendingSubstrate methods, the open glue ticket it's waiting
# on. Composes scripts/check_method_registry.sh's --list-json side mode
# with a formatter — single parse source-of-truth.
#
# Examples:
#   just methods             # list all registered methods
#   just methods --pending   # only PendingSubstrate (dormant) methods
#   just methods --live      # only Live methods
#   just methods --json      # raw JSON pass-through
methods *ARGS:
    @uv run scripts/methods.py {{ARGS}}

# Audit an epic dashboard's child-ticket roster against frontmatter
# truth (ticket 318). Surfaces drift between the dashboard's claim and
# each child's actual `status` / `blocked-by` / landed sha. Default
# target is the substrate-refactor epic (060); pass `<id>` or a path
# to audit a different dashboard.
#
# Drift kinds: consistent · landed-but-marked-active · landed-but-sha-stale
# · status-mismatch · blocker-mismatch · missing-file · link-mismatch ·
# unparseable-status. The first four are mechanically rewritten by
# `--fix`; the rest need editorial attention.
#
# Exit codes: 0 consistent, 1 drift detected (or rewritten under --fix),
# 2 epic not found / parse error. Hooked into `just check` via
# `scripts/check_epic_children.sh`.
#
# Examples:
#   just epic-children                                # default: 060
#   just epic-children 060 --text                     # human summary
#   just epic-children 060 --fix                      # rewrite drift rows
#   just epic-children docs/open-work/tickets/060-...md
epic-children *ARGS:
    @uv run scripts/epic_children.py {{ARGS}}

# Run a balance hypothesis end-to-end: baseline + treatment sweeps,
# concordance check, draft balance doc. Formalizes the four-artifact
# methodology (hypothesis / prediction / observation / concordance).
# Treatment runs read CLOWDER_OVERRIDES env var; the binary's
# SimConstants::from_env() applies the patch at boot, no rebuild.
#
# Spec template: docs/balance/hypothesis-template.yaml.
# Exit codes: 0 concordant, 1 inconclusive/off-magnitude, 2 wrong-direction.
#
# Examples:
#   just hypothesize docs/balance/my-hypothesis.yaml
#   just hypothesize SPEC --duration 60 --seeds 42 --reps 1   # smoke test
#   just hypothesize SPEC --text                              # human summary
hypothesize *ARGS:
    @uv run scripts/hypothesize.py {{ARGS}}

# Per-metric "is this run in band?" verdict against
# docs/balance/healthy-colony.md. Pure sense-making companion to
# `just verdict` — emits per-field verdict (in-range / low / high /
# below-floor / above-cap) so a non-game-dev can read a soak.
#
# Exit codes: 0 every metric in band, 1 concerns, 2 failures (continuity
# tally at 0, starvation > 0, etc.).
#
# Examples:
#   just fingerprint logs/tuned-42
#   just fingerprint logs/tuned-42 --text
fingerprint *ARGS:
    @uv run scripts/fingerprint.py {{ARGS}}

# Explain a SimConstants field: doc comment, current value (read from a
# recent events.jsonl header), every read site in src/, and (if Tier 4.2
# sensitivity map exists) per-metric Spearman rho. Resolves dotted paths
# (e.g. `magic.ward_decay_per_tick`).
#
# Examples:
#   just explain magic.ward_decay_per_tick
#   just explain fulfillment.social_warmth_socialize_per_tick --text
#   just explain --list                                # every constant path
explain *ARGS:
    @uv run scripts/explain_constant.py {{ARGS}}

# Friction-log breadcrumb. Appends one JSON line to
# logs/agent-friction.jsonl when an agent (Claude or otherwise) hits a
# workflow snag — a tool's output didn't match its SKILL.md, two tries
# both failed, no `just` command served the user's intent. Periodic
# review of the log surfaces patterns worth encoding into CLAUDE.md or
# a new SKILL.md. See .claude/skills/agent-feedback/SKILL.md.
#
# Examples:
#   just agent-feedback "q events refused on sweep dir; narrative didn't suggest q deaths"
#   just agent-feedback "frame-diff column shape changed silently between runs" --severity major --tool frame-diff
agent-feedback *ARGS:
    uv run scripts/agent_feedback.py {{ARGS}}

# Promote a soak directory to a named first-class baseline. Writes
# logs/baselines/<label>.json + (unless --no-current) updates
# logs/baselines/current.json so `just verdict` auto-reads it as the
# active baseline. Run this after a soak you want as a checkpoint.
#
# Examples:
#   just promote logs/tuned-42 post-state-trio
#   just promote logs/tuned-42 post-state-trio --no-current
#   just promote logs/tuned-42 post-state-trio --force
promote *ARGS:
    bash scripts/promote.sh {{ARGS}}

# Find the commit that introduced a canary regression. Builds a test
# script that rebuilds + soaks + probes the metric at each candidate;
# the loop is run via `jj edit` (or `git bisect run`) until the
# offending commit is isolated. Defaults to a 60s probe at seed 42.
#
# Examples:
#   just bisect-canary deaths_by_cause.Starvation @
#   just bisect-canary deaths_by_cause.ShadowFoxAmbush @ --threshold 10
#   just bisect-canary wards_placed_total @ --threshold 50 --duration 300
bisect-canary *ARGS:
    bash scripts/bisect_canary.sh {{ARGS}}

# Build logs/sensitivity-map.json by perturbing every SimConstants leaf
# ±20% across a 3-seed sweep and recording Spearman rho between the knob
# and each footer metric. Costly (~5–10h wall) — run on a quiet weekend
# and commit the output; refresh quarterly. `just explain` reads it to
# show the top affected metrics per knob.
#
# Examples:
#   just rebuild-sensitivity-map
#   just rebuild-sensitivity-map magic.*       # only magic.* leaves
#   just rebuild-sensitivity-map --duration 30 # cheaper smoke variant
rebuild-sensitivity-map *ARGS:
    bash scripts/build_sensitivity_map.sh {{ARGS}}

# Multi-seed × multi-rep headless sweep for Phase 5b balance verification.
# Writes to logs/sweep-<label>/<seed>-<rep>/{narrative,events}.jsonl.
# Defaults: 5 seeds × 3 reps = 15 runs, 4-way parallel. Requires a release
# build (run `just release-build` first if needed).
#
# Usage:
#   just sweep baseline-5b                       # natural weather
#   just sweep fog-activation-1                  # natural weather, post-activation
#   just sweep forced-fog fog                    # --force-weather fog on every run
#   just sweep custom "" "42 99" 5               # 2 seeds × 5 reps, no override
sweep LABEL FORCE_WEATHER="" SEEDS="42 99 7 2025 314" REPS="3" DURATION="900" PARALLEL="4":
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ ! -x target/release/clowder ]]; then
      echo "Release binary not found — building..."
      cargo build --release
    fi
    base="logs/sweep-{{LABEL}}"
    mkdir -p "$base"
    force_arg=""
    if [[ -n "{{FORCE_WEATHER}}" ]]; then
      force_arg="--force-weather {{FORCE_WEATHER}}"
    fi
    # Emit one job per (seed, rep) pair, pipe to xargs for parallel exec.
    jobs=$(mktemp)
    trap 'rm -f "$jobs"' EXIT
    for seed in {{SEEDS}}; do
      for rep in $(seq 1 {{REPS}}); do
        dir="$base/${seed}-${rep}"
        mkdir -p "$dir"
        echo "./target/release/clowder --headless --seed ${seed} --duration {{DURATION}} ${force_arg} --log ${dir}/narrative.jsonl --event-log ${dir}/events.jsonl > ${dir}/stderr.log 2>&1 && echo DONE ${seed}-${rep}" >> "$jobs"
      done
    done
    total=$(wc -l < "$jobs" | tr -d ' ')
    echo "Sweep {{LABEL}}: $total runs, {{PARALLEL}}-way parallel, ${force_arg:-natural weather}"
    # -S bumps BSD xargs's default 255-char per-substitution buffer.
    xargs -P {{PARALLEL}} -I CMD -S 4096 bash -c CMD < "$jobs"
    echo "Sweep complete — outputs in $base/"

# Capture a versioned baseline dataset under logs/baseline-<LABEL>/.
# Five-phase orchestrator (probe → aggregate sweep → focal traces →
# conditional weather → REPORT.md). Designed to be backgroundable;
# writes STATUS.txt + STATUS.json after every phase. See
# scripts/run_baseline_dataset.sh for env-var overrides
# (SEEDS, REPS, DURATION, PROBE_DURATION, PARALLEL, ALLOW_DIRTY,
# SKIP_PHASE_4).
#
# Examples:
#   just baseline-dataset 2026-04-25
#   SEEDS="42 99" REPS=1 DURATION=60 just baseline-dataset smoke
#   nohup just baseline-dataset 2026-04-25 > /tmp/baseline.log 2>&1 &
baseline-dataset LABEL:
    bash scripts/run_baseline_dataset.sh {{LABEL}}

# Render REPORT.md from an existing logs/baseline-<LABEL>/ tree without
# re-running soaks. Useful after editing baseline_report.py or for
# inspecting partial datasets that crashed mid-run.
baseline-report LABEL:
    python3 scripts/baseline_report.py --baseline-dir logs/baseline-{{LABEL}}

# Diff tuning constants between two runs. Empty diff means the runs are
# behaviorally comparable.
diff-constants BASE NEW:
    #!/usr/bin/env bash
    diff <(jq -c 'select(._header) | .constants' {{BASE}}) \
         <(jq -c 'select(._header) | .constants' {{NEW}})

# Build
build:
    cargo build

# Run tests
test:
    cargo test

# Check + clippy + step-resolver contract lint + time-unit lint + IAUS-coherence lint + substrate-stub lint + marker-snapshot-wiring lint (ticket 217) + items-are-real lint + influence-map-registry lint + epic-children roster drift (ticket 318) + orchestration-frontmatter invariants (ticket 354) + score_actions dispatcher invariant (ticket 438)
check:
    cargo check --all-targets && cargo clippy --all-targets --all-features -- -D warnings && bash scripts/check_step_contracts.sh && bash scripts/check_time_units.sh && bash scripts/check_iaus_coherence.sh && bash scripts/check_substrate_stubs.sh && bash scripts/check_marker_snapshot_wiring.sh && bash scripts/check_item_transfers.sh && bash scripts/check_influence_map_registry.sh && bash scripts/check_method_registry.sh && bash scripts/check_epic_children.sh && bash scripts/check_orchestration_frontmatter.sh && bash scripts/check_score_actions_dispatch.sh

# [retag] Backfill `orchestration: substrate-sensitive` on every active ticket missing the field. Idempotent. Stage 0 step 4 of ticket 354. Run once at corpus rollout, then `just check` enforces invariants going forward.
retag-init:
    bash scripts/retag_init.sh

# [retag] Single-ticket orchestration retag. Sets/updates the orchestration / block / verdict-anchor / initiative fields on one ticket. Idempotent. Usage: just retag <id> --track <name> [--block <name>] [--anchor] [--initiative <a,b>]. Validated by `just check`.
retag ID *ARGS:
    bash scripts/retag.sh {{ID}} {{ARGS}}

# [retag] Heuristic auto-classifier — proposes orchestration tags for every untagged-or-default ticket. Read-only by default; --apply commits suggestions per-batch. --only <track> filters output; --json emits machine-readable for the /retag skill.
retag-suggest *ARGS:
    python3 scripts/retag_suggest.py {{ARGS}}

# [retag] Corpus rollup — counts per track / per status / per cluster, plus per-block anchor status. --json emits machine-readable. Complementary to `just check` (which gates) — audit reports state.
retag-audit *ARGS:
    python3 scripts/retag_audit.py {{ARGS}}

# [retag] Interactive per-ticket retag walk. Companion to `retag-suggest --apply` (batched): walks one suggestion at a time with [y]es/[n]o/[a]ll-remaining/[q]uit prompts. Composes retag-suggest --json for the heuristic + retag.sh for the edit, so the heuristic stays in one place. --track filters; --include-noop walks current==suggested rows; --dry-run prints without applying.
retag-walk *ARGS:
    python3 scripts/retag_walk.py {{ARGS}}

# [session] Create an isolated parallel-session workspace + bookmark + atomic ticket claim. Creates ~/clowder-sessions/<slug>/, sets bookmark session/<slug>, writes .session-info.json. Usage: just session-new <slug> [--tickets <ids>] [--track <name>] [--pick] [--print-prompt]. Ticket claim uses flock on docs/open-work/.claim-lock to prevent races.
session-new SLUG *ARGS:
    bash scripts/session_new.sh {{SLUG}} {{ARGS}}

# [session] Dashboard of all active sessions — slug, track, tickets, bookmark head, last edit, optional disk usage. --json for skill consumption.
session-list *ARGS:
    python3 scripts/session_list.py {{ARGS}}

# [session] Alias for session-list (the conversational shorthand).
sessions *ARGS:
    python3 scripts/session_list.py {{ARGS}}

# [session] Clean up a session after its work has landed (or been abandoned). cargo clean → jj workspace forget → rm -rf. Bookmark disposition (ticket 362): default `auto` forgets IFF the bookmark tip is on main, preserves it otherwise (prevents orphaning). --keep-bookmark preserves always; --forget-bookmark forces forget (pair with --orphan-ok if tip is NOT on main). Refuses on uncommitted edits unless --force.
session-done SLUG *ARGS:
    bash scripts/session_done.sh {{SLUG}} {{ARGS}}

# [session] Surface bookmarks + orphan commits whose work is NOT on main@origin. Walks every local session/* bookmark and the recent op-log for `feat:`/`fix:`/`land:` commits that match active ticket ids, reporting any whose ancestors don't reach main. Use to triage abandoned polecat work BEFORE running session-done on stale workspaces. Ticket 362.
orphan-scan *ARGS:
    bash scripts/orphan_scan.sh {{ARGS}}

# [session] Batch GC: finds sessions whose bookmark tip is on main@origin (landed) and runs session-done against each. --dry-run reports without acting; --yes skips per-session confirm; --json for skill consumption; --force passes through to session-done.
session-gc *ARGS:
    bash scripts/session_gc.sh {{ARGS}}

# [session] Suggest one or more next-session candidates {slug, tickets, track, rationale}. Composes open-work-by-track and groups by cluster (substrate-sensitive: pair adjacent; swarm-safe: atomic; coherent-block: prefer densest block). --track filters; --n caps count; --json for /work skill.
session-suggest *ARGS:
    python3 scripts/session_suggest.py {{ARGS}}

# [refinery] Gated lander. Walks all session/* bookmarks; reports rebase / conflict status. `just refinery` reports the table; `just refinery --json` for /work + /foreman skills; `just refinery --land <slug>` lands one session into main (any track, manual gate); `just refinery --auto [--dry-run]` drains swarm-safe queue gated on working-copy clean + `just check && just test` in each workspace. `--auto` is whitelisted in code to track=swarm-safe; coherent-block + substrate-sensitive always land via `--land <slug>`.
refinery *ARGS:
    bash scripts/refinery.sh {{ARGS}}

# [foreman] Report polecats + ready swarm-safe queue (default mode). `just foreman` for the text table; `--json` for /foreman skill consumption.
foreman *ARGS:
    bash scripts/foreman.sh {{ARGS}}

# [foreman] Spawn N swarm-safe polecats (default N=3, wallclock 30m/each). Atomically claims tickets + creates workspaces via session-new, spawns `claude -p` subprocesses, then enters the auto-poll-and-land loop that drains via `just refinery --auto` when all polecats exit. `--dry-run` plans without spawning. Subscription-billed; wallclock is the only runaway cap.
foreman-spawn N="3" *ARGS:
    bash scripts/foreman.sh --spawn {{N}} {{ARGS}}

# [foreman] One-shot heartbeat — PIDs, alive/exited, last edit, deadline remaining for every tracked polecat. Wrap in `watch -n 5 just foreman-watch` for live updates.
foreman-watch *ARGS:
    bash scripts/foreman.sh --watch {{ARGS}}

# [foreman] Tail one polecat's stream-json log. Usage: `just foreman-log swarmpole-301`.
foreman-log SLUG:
    bash scripts/foreman.sh --log {{SLUG}}

# [foreman] SIGTERM (or --hard SIGKILL) every tracked polecat. Workspace artifacts remain for post-mortem; call `just refinery --auto` afterwards to drain any that managed to push their bookmark.
foreman-shutdown *ARGS:
    bash scripts/foreman.sh --shutdown {{ARGS}}

# [block] List all coherent-block initiatives in the corpus — ticket count, verdict-anchor, status rollup. --json for skill consumption.
block-list *ARGS:
    python3 scripts/block_info.py list {{ARGS}}

# [block] Detail view of one coherent-block (initiative name) — all member tickets, anchor, status rollup, dependency graph hints. --json for skill consumption.
block-info BLOCK *ARGS:
    python3 scripts/block_info.py {{BLOCK}} {{ARGS}}

# [block] Set or clear the verdict-anchor on a coherent-block ticket. Refuses up-front if another ticket in the same block already carries the anchor (≤1 per block invariant; also enforced by `just check`). Composes scripts/retag.sh and regenerates the open-work index. Usage: just block-anchor <block> <ticket-id> [--clear].
block-anchor BLOCK TICKET *ARGS:
    python3 scripts/block_anchor.py {{BLOCK}} {{TICKET}} {{ARGS}}

# [block] Run `just verdict` against the soak log of a coherent-block's anchor session. Composes block-info + ticket-info to find the anchor's holding session, then runs verdict in that workspace. Refuses if the block has no anchor / the anchor is not in any session / the session has no soak log. Usage: just block-verdict <block> [seed=42].
block-verdict BLOCK *ARGS:
    bash scripts/block_verdict.sh {{BLOCK}} {{ARGS}}

# [ticket-query] Single-ticket frontmatter + status + which session (if any) currently holds it. --json for /work skill.
ticket-info ID *ARGS:
    python3 scripts/ticket_info.py {{ID}} {{ARGS}}

# [ticket-query] Ready-queue rollup grouped by orchestration track (substrate-sensitive / coherent-block / swarm-safe). Within coherent-block, sub-grouped by block. --json emits {"swarm-safe": [...], "substrate-sensitive": [...], "coherent-block": {"<block>": [...]}} for /work skill consumption.
open-work-by-track *ARGS:
    python3 scripts/open_work_by_track.py {{ARGS}}

# Generate a random template authoring prompt
template-prompt:
    cargo run --example template_prompt

# Audit narrative template coverage across action × mood × weather × season
template-audit:
    cargo run --example template_audit

# Inspect a cat's personality and decision history from the event log
inspect name *ARGS:
    cargo run --example inspect_cat -- {{name}} {{ARGS}}

# Per-metric statistical summary of a sweep directory, optionally
# comparing two sweeps with Welch's t / Cohen's d / effect-size bands.
# Replaces the retired `balance-report`, `score-diff`, and `sweep_compare.py`.
#
# Bands:
#   significant — |Δ| ≥ 30% AND p < 0.05 AND |d| > 0.5
#   drift       — 10% ≤ |Δ| < 30% (worth investigating)
#   noise       — |Δ| < 10%
#
# Examples:
#   just sweep-stats logs/sweep-baseline-5b
#   just sweep-stats logs/sweep-fog-activation-1 --vs logs/sweep-baseline-5b
#   just sweep-stats logs/sweep-X --text
#   just sweep-stats logs/sweep-X --charts          # opt-in matplotlib boxplots
sweep-stats *ARGS:
    uv run scripts/sweep_stats.py {{ARGS}}

# Cross-run log database — collate baseline + diagnostic archives for SQL
# queries against `logs/runs.duckdb`. Idempotent via mtime cache; re-running
# build is cheap. Schema reference: docs/diagnostics/logdb.md.
#
# Default ingest covers headers, footers, ColonyScore, CatSnapshots, and
# Death events. Heavy tables (cat_snapshot_scores, trace_l2/l3) are opt-in
# so the daily build stays under the 5-minute gate.
#
# Examples:
#   just logdb-build                              # ingest every logs/<dir>
#   just logdb-build baseline-2026-04-25          # ingest one archive
#   just logdb-build --rebuild                    # drop and recreate the DB
#   just logdb-build --with-scores                # +cat_snapshot_scores (~5x slower)
#   just logdb-build --with-traces                # +trace_l2/l3 from sidecars
logdb-build *ARGS:
    uv run scripts/logdb.py build {{ARGS}}

# One-shot read-only SQL against logs/runs.duckdb. Quote the SQL.
#   just logdb-query "SELECT COUNT(*) FROM runs"
logdb-query SQL:
    uv run scripts/logdb.py query "{{SQL}}"

# Interactive duckdb shell on logs/runs.duckdb (requires the duckdb CLI).
logdb-shell:
    uv run scripts/logdb.py shell

# Render a chart recipe to logs/charts/<recipe>-<ts>.html.
#   just logdb-chart colony-score-over-time
#   just logdb-chart colony-score-over-time --archive baseline-2026-04-25 --smooth 5
logdb-chart RECIPE *ARGS:
    uv run scripts/logdb.py chart {{RECIPE}} {{ARGS}}

# Generate game wiki and build mdBook site
wiki:
    uv run scripts/generate_wiki.py
    mdbook build docs/wiki

# Summary of open work by status (reads docs/open-work/tickets/ frontmatter)
open-work:
    #!/usr/bin/env bash
    for s in in-progress ready parked blocked; do
      n=$(rg -l "^status: $s\b" docs/open-work/tickets/ -g '!_*.md' 2>/dev/null | wc -l | tr -d ' ')
      printf "%-14s %s\n" "$s" "$n"
    done
    pe=$(ls docs/open-work/pre-existing/*.md 2>/dev/null | wc -l | tr -d ' ')
    printf "%-14s %s\n" "pre-existing" "$pe"

# List ready tickets with id and title
open-work-ready:
    #!/usr/bin/env bash
    for f in $(rg -l '^status: ready\b' docs/open-work/tickets/ -g '!_*.md' 2>/dev/null | sort); do
      id=$(rg '^id:' "$f" | head -1 | sed 's/id: *//')
      title=$(rg '^title:' "$f" | head -1 | sed 's/title: *//')
      printf "%-5s %s\n" "$id" "$title"
    done

# List in-progress tickets with id and title
open-work-wip:
    #!/usr/bin/env bash
    for f in $(rg -l '^status: in-progress\b' docs/open-work/tickets/ -g '!_*.md' 2>/dev/null | sort); do
      id=$(rg '^id:' "$f" | head -1 | sed 's/id: *//')
      title=$(rg '^title:' "$f" | head -1 | sed 's/title: *//')
      printf "%-5s %s\n" "$id" "$title"
    done

# Regenerate docs/open-work.md from per-ticket frontmatter
open-work-index:
    uv run scripts/generate_open_work.py

# Active focus: in-progress + ready blockers of active work + top-5 from `just next`.
# Mirrors the `## Active focus` section in docs/open-work.md without scrolling.
open-work-active:
    uv run scripts/open_work_filters.py active

# [ticket-query] Filter ready tickets by track / cluster / initiative. With no flags, lists all ready. Drop-in companion to `open-work-ready` (same output shape).
#
#   just open-work-ready                          # all ready
#   just open-work-ready --cluster ai-substrate   # one cluster
#   just open-work-ready --initiative world-richness  # one initiative
#   just open-work-ready --track swarm-safe       # filter by orchestration track
open-work-ready-filtered *ARGS:
    uv run scripts/open_work_filters.py ready {{ARGS}}

# List parked tickets older than N days (default 30). Also surfaces undated
# parks (parked: null) — they need backfilling before the staleness window can
# include them.
open-work-stale *ARGS:
    uv run scripts/open_work_filters.py stale {{ARGS}}

# Show all transitive blockers of a given ticket.
#
#   just open-work-blocking 305
open-work-blocking ID:
    uv run scripts/open_work_filters.py blocking {{ID}}

# List active initiatives with (open, landed) counts. Use when you want to see
# project trajectory per thematic outcome.
initiatives:
    uv run scripts/open_work_filters.py initiatives

# Per-epic progress (children done / open / blocked / parked, plus a bar).
# Reads each `*-epic.md`'s roster table; child status comes from frontmatter.
#
#   just open-work-epics                 # rollup table
#   just open-work-epics --epic 093      # one epic
#   just open-work-epics --detailed      # every child listed under each epic
#   just open-work-epics --json          # machine-readable
#   just open-work-epics --check         # lint: orphan tickets + stale rosters
open-work-epics *ARGS:
    uv run scripts/epic_progress.py {{ARGS}}

# Land a ticket. Three modes:
#
#   just land 197                                         # file-only: rewrite frontmatter,
#                                                         # move tickets/ -> landed/, drop
#                                                         # blocked-by from dependents,
#                                                         # regen docs/open-work.md.
#                                                         # User commits + backfills the sha
#                                                         # themselves.
#   just land 197 --sha 55b6e930                          # backfill landed-at: pending after
#                                                         # the commit lands.
#   just land 197 --commit "feat: 197 — short summary"    # full jj orchestration: bundles
#                                                         # current working copy + landing
#                                                         # diff into one feat commit, then
#                                                         # creates a docs sha-backfill
#                                                         # commit, then leaves @ empty.
#
# `--commit` saves ~7 jj commands per landing. Optionally pass `--log "..."` in
# any mode to append a `- <today>: <entry>` line to the ticket's Log section.
land *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    uv run scripts/land_ticket.py "$@"

# Open a new ticket: pick the next id, instantiate _template.md (or
# _template_bugfix.md with --bugfix), fill in id/title/added/cluster/
# blocked-by, and regenerate the index.
#
#   just open-ticket "<title>"
#   just open-ticket "<title>" --bugfix --cluster process-discipline
#   just open-ticket "<title>" --blocked-by 195,196
open-ticket *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    uv run scripts/create_ticket.py "$@"

# Render ticket dependency tree (blocked-by edges). Pass flags through.
#
# Examples:
#   just open-work-tree                        # default downward forest
#   just open-work-tree --upward               # invert: blocked → blockers
#   just open-work-tree --root 011             # subtree rooted at 011
#   just open-work-tree --format mermaid       # paste into docs
open-work-tree *ARGS:
    uv run scripts/open_work_tree.py {{ARGS}}

# Generate wiki and open in browser
wiki-serve:
    uv run scripts/generate_wiki.py
    mdbook serve docs/wiki --open

# Open the cat questionnaire in a browser
questionnaire:
    open tools/cat_questionnaire.html

# Start the Writer's Toolkit dev server (narrative editor, cat quiz,
# and simulation log dashboard). Deployed to GitHub Pages on merge to main.
narrative-editor:
    cd tools/narrative-editor && npm install --silent && npm run dev

# Open the simulation log dashboard straight to the #/logs route. Client-side
# only — drop events.jsonl / narrative.jsonl from disk to compare runs.
logs:
    cd tools/narrative-editor && npm install --silent && npm run dev -- --open /#/logs

# Open the focal-cat trace scrubber (#/trace). Drop a `trace-<name>.jsonl`
# produced by `just soak-trace <seed> <focal>` to step tick-by-tick through
# the IAUS L1/L2/L3 decision pipeline for that cat.
trace:
    cd tools/narrative-editor && npm install --silent && npm run dev -- --open /#/trace

# Build autotile atlases from Fan-tasy Tileset source images
atlas-build:
    python3 tools/build_grass_atlas.py

# Visual verification of blob autotile atlas coordinate mapping
atlas-test:
    python3 tools/verify_atlas.py

# Audit which Fan-tasy sprites are mapped vs unmapped
atlas-coverage:
    python3 tools/audit_sprites.py

# Replay-determinism gate: runs the integration test that proves two
# same-seed runs of the same binary produce a byte-identical events.jsonl.
# Release mode because the canonical sim is release; debug-mode determinism
# is not a gate this project cares about. Fast enough (~10s) to keep in
# every CI run — failures here mean a new HashMap-iteration or scheduler
# nondeterminism source has crept in.
check-determinism:
    cargo test --release --test integration simulation_is_deterministic

# Run all checks
ci: check test check-determinism

# Build optimized release binary
release-build:
    cargo build --release

# Tag and push a release (triggers GitHub Actions)
release VERSION:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Bumping to {{VERSION}}..."
    sed -i '' 's/^version = ".*"/version = "{{VERSION}}"/' Cargo.toml
    cargo check --quiet
    echo ""
    echo "Update CHANGELOG.md with the release date, then press Enter."
    read -r _
    jj commit -m "chore: release v{{VERSION}}"
    jj git push
    jj git export
    git tag "v{{VERSION}}"
    git push origin "v{{VERSION}}"
    echo "Pushed v{{VERSION}} — GitHub Actions will build and publish the release."
