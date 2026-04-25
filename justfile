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

# Canonical 15-min deep-soak at a fixed seed. Release build, writes to
# logs/tuned-<seed>/{events,narrative}.jsonl. See CLAUDE.md and
# docs/diagnostics/log-queries.md for verification.
soak SEED="42":
    mkdir -p logs/tuned-{{SEED}}
    cargo run --release -- --headless --seed {{SEED}} --duration 900 \
      --log logs/tuned-{{SEED}}/narrative.jsonl \
      --event-log logs/tuned-{{SEED}}/events.jsonl

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
    python3 scripts/logq/logq.py {{ARGS}}

# Run logq's envelope + subtool tests (stdlib unittest, no pytest dep).
# Runs the file directly because `unittest discover` requires the
# tests/ dir to be a Python package (which would mean adding an
# __init__.py and polluting the Rust test layout).
test-logq:
    python3 tests/logq/test_envelope.py -v

# Deep-soak with a focal-cat trace sidecar. Writes to
# logs/tuned-<seed>/{events,narrative,trace-<focal>}.jsonl. Trace
# records decompose per-tick L1/L2/L3 state for one focal cat per §11
# of docs/systems/ai-substrate-refactor.md.
soak-trace SEED="42" FOCAL_CAT="Simba":
    mkdir -p logs/tuned-{{SEED}}
    cargo run --release -- --headless --seed {{SEED}} --duration 900 \
      --focal-cat {{FOCAL_CAT}} \
      --log logs/tuned-{{SEED}}/narrative.jsonl \
      --event-log logs/tuned-{{SEED}}/events.jsonl \
      --trace-log logs/tuned-{{SEED}}/trace-{{FOCAL_CAT}}.jsonl

# Pairwise frame-diff over two focal-cat traces, with optional
# hypothesis overlay from a phase balance doc. Emits per-DSE drift
# stats ranked by |Δ mean| and an overall concordance verdict.
#
# Example:
#   just frame-diff logs/baseline-pre-substrate-refactor/trace-Simba.jsonl \
#                   logs/tuned-42/trace-Simba.jsonl \
#                   docs/balance/substrate-phase-3.md
frame-diff BASELINE NEW HYPOTHESIS="":
    #!/usr/bin/env bash
    if [[ -n "{{HYPOTHESIS}}" ]]; then
        uv run scripts/frame_diff.py {{BASELINE}} {{NEW}} --hypothesis {{HYPOTHESIS}}
    else
        uv run scripts/frame_diff.py {{BASELINE}} {{NEW}}
    fi

# Full verification loop for a seed: soak, survival canaries,
# continuity canaries, constants diff against a baseline. Lands in
# Phase 1 of the substrate refactor; Phase 3+ extends with frame-diff
# against a baseline trace.
#
# Does NOT hard-abort on canary failures during pre-refactor runs —
# the baseline Starvation canary is already failing (see
# docs/balance/substrate-refactor-baseline.md). Prints a verdict line
# at the end; operator decides whether numbers are acceptable against
# the phase's balance doc. Exit code reflects worst observed status.
autoloop SEED="42" FOCAL_CAT="Simba":
    #!/usr/bin/env bash
    set -uo pipefail
    just soak-trace {{SEED}} {{FOCAL_CAT}}
    echo ""
    echo "== survival canaries =="
    just check-canaries logs/tuned-{{SEED}}/events.jsonl
    surv_rc=$?
    echo ""
    echo "== continuity canaries =="
    just check-continuity logs/tuned-{{SEED}}/events.jsonl
    cont_rc=$?
    baseline="logs/baseline-pre-substrate-refactor/events.jsonl"
    const_rc=0
    if [[ -f "$baseline" ]]; then
        echo ""
        echo "== constants diff vs baseline =="
        if just diff-constants "$baseline" logs/tuned-{{SEED}}/events.jsonl; then
            echo "(identical)"
        else
            const_rc=$?
            echo "(constants differ — phase's balance doc must explain)"
        fi
    fi
    echo ""
    echo "== autoloop verdict =="
    if [ "$surv_rc" -ne 0 ]; then
        echo "  survival canaries:  FAIL (exit $surv_rc) — pre-existing Starvation regression is inherited; see activation-1-status.md"
    else
        echo "  survival canaries:  pass"
    fi
    if [ "$cont_rc" -ne 0 ]; then
        echo "  continuity canaries: FAIL (exit $cont_rc) — expected at Phase 1 entry; refactor Phases 3+ must strengthen"
    else
        echo "  continuity canaries: pass"
    fi
    if [ "$const_rc" -ne 0 ]; then
        echo "  constants diff:     drift (exit $const_rc)"
    else
        echo "  constants diff:     clean"
    fi
    # Overall exit: 0 if survival passes; nonzero if survival regresses.
    # Continuity + constants surface in the verdict but don't block.
    exit "$surv_rc"

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

# Check + clippy + step-resolver contract lint
check:
    cargo check --all-targets && cargo clippy --all-targets --all-features -- -D warnings && bash scripts/check_step_contracts.sh

# Generate a random template authoring prompt
template-prompt:
    cargo run --example template_prompt

# Audit narrative template coverage across action × mood × weather × season
template-audit:
    cargo run --example template_audit

# Inspect a cat's personality and decision history from the event log
inspect name *ARGS:
    cargo run --example inspect_cat -- {{name}} {{ARGS}}

# Run multi-seed benchmark and record scores against current changeset
score-track *ARGS:
    uv run scripts/score_track.py {{ARGS}}

# Compare scores between recent changesets
score-diff *ARGS:
    uv run scripts/score_diff.py {{ARGS}}

# Generate balance report from most recent diagnostic run
balance-report *ARGS:
    uv run scripts/balance_report.py {{ARGS}}

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

# Run all checks
ci: check test

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
