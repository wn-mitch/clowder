#!/usr/bin/env bash
# Foreman — the master-orchestrator that spawns child `claude` CLI processes
# (polecats) against pre-created swarm-safe workspaces. Stage 2.2 of ticket 354.
#
# Modes:
#   foreman.sh                            report polecats + ready queue (default)
#   foreman.sh --json                     machine-readable for /foreman skill
#   foreman.sh --spawn N [--wallclock M]  spawn N swarm-safe polecats
#                       [--dry-run]       (--dry-run plans without spawning)
#   foreman.sh --watch                    one-shot heartbeat: PIDs + bookmark state
#   foreman.sh --land                     run refinery --auto in this queue
#   foreman.sh --shutdown [--hard]        SIGTERM (or SIGKILL with --hard) all polecats
#   foreman.sh --log <slug>               tail one polecat's stream-json
#
# Spawn flow (per polecat):
#   1. Pick top ready swarm-safe ticket from open-work-ready-filtered
#   2. Compose slug `swarmpole-<id>` and call `just session-new <slug>` —
#      this atomically claims the ticket, creates the workspace + bookmark.
#   3. Compose the foreman-specific polecat prompt (heredoc; far more
#      constrained than `session-new --print-prompt` because the polecat
#      is headless and has no human to redirect).
#   4. Spawn the claude CLI subprocess via nohup with stream-json output
#      redirected to .polecat-stream.jsonl.
#   5. Spawn a wallclock sentinel: a background sleeper that SIGTERMs the
#      polecat if it's still alive after $wallclock seconds. (macOS doesn't
#      ship timeout(1); this is the portable workaround.)
#   6. After all polecats spawned, enter the watch+land poll loop: every 30s
#      check liveness; when all polecats are dead, run `just refinery --auto`.
#      Failed polecats (dead PID + bookmark not pushed) get session_done.sh
#      cleanup that releases the ticket claim back to ready.
#
# Artifacts per polecat under ~/clowder-sessions/<slug>/:
#   .session-info.json     — written by session_new.sh (slug/track/tickets/...)
#   .polecat.pid           — polecat PID
#   .polecat-watchdog.pid  — wallclock sentinel PID
#   .polecat-stream.jsonl  — stream-json from claude -p
#   .polecat-stderr.log    — stderr capture
#   .polecat-cmdline       — exact `claude` invocation (for post-mortem)
#   .polecat-deadline      — UNIX timestamp when wallclock fires
#   .polecat-exit          — exit code (written by spawn subshell on exit)

set -euo pipefail

REPO_ROOT=$(cd "$(dirname "$0")/.." && pwd)
SESSIONS_ROOT="$HOME/clowder-sessions"
DEFAULT_N=3
DEFAULT_WALLCLOCK_MIN=30
POLL_INTERVAL_SECS=30

mode="report"
spawn_n=""
wallclock_min="$DEFAULT_WALLCLOCK_MIN"
dry_run="false"
emit_json="false"
shutdown_hard="false"
log_slug=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --json) emit_json="true"; shift ;;
        --spawn) mode="spawn"; spawn_n="$2"; shift 2 ;;
        --wallclock) wallclock_min="$2"; shift 2 ;;
        --dry-run) dry_run="true"; shift ;;
        --watch) mode="watch"; shift ;;
        --land) mode="land"; shift ;;
        --shutdown) mode="shutdown"; shift ;;
        --hard) shutdown_hard="true"; shift ;;
        --log) mode="log"; log_slug="$2"; shift 2 ;;
        -h|--help) sed -n '/^# Modes:/,/^# Artifacts/p' "$0" | sed 's/^# \?//'; exit 0 ;;
        --*) echo "Unknown flag: $1" >&2; exit 2 ;;
        *) echo "Unexpected positional: $1" >&2; exit 2 ;;
    esac
done

# ─── helpers ──────────────────────────────────────────────────────────────────

list_polecat_workspaces() {
    # Emit one workspace dir per line for every session that has a .polecat.pid.
    if [[ -d "$SESSIONS_ROOT" ]]; then
        find "$SESSIONS_ROOT" -mindepth 2 -maxdepth 2 -name '.polecat.pid' \
            -exec dirname {} \; 2>/dev/null | sort
    fi
}

polecat_state() {
    # Print one tab-separated row describing a polecat's state.
    # Columns: slug · pid · alive · last_edit_seconds · deadline_secs_remaining · ticket
    local ws="$1"
    local slug; slug=$(basename "$ws")
    local pid="—" alive="dead" last_edit="—" deadline_remaining="—" tickets="—"

    if [[ -f "$ws/.polecat.pid" ]]; then
        pid=$(cat "$ws/.polecat.pid")
        if kill -0 "$pid" 2>/dev/null; then
            alive="alive"
        else
            alive="exited"
        fi
    fi

    if [[ -f "$ws/.polecat-stream.jsonl" ]]; then
        local mtime now
        mtime=$(stat -f %m "$ws/.polecat-stream.jsonl" 2>/dev/null || stat -c %Y "$ws/.polecat-stream.jsonl" 2>/dev/null || echo 0)
        now=$(date +%s)
        last_edit=$((now - mtime))
    fi

    if [[ -f "$ws/.polecat-deadline" ]]; then
        local deadline now
        deadline=$(cat "$ws/.polecat-deadline")
        now=$(date +%s)
        deadline_remaining=$((deadline - now))
    fi

    if [[ -f "$ws/.session-info.json" ]]; then
        tickets=$(python3 -c '
import json, sys
try:
    info = json.load(open(sys.argv[1]))
    print(",".join(str(t) for t in info.get("tickets", [])) or "—")
except Exception:
    print("—")
' "$ws/.session-info.json")
    fi

    printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$slug" "$pid" "$alive" "$last_edit" "$deadline_remaining" "$tickets"
}

pick_top_ready_swarm_safe() {
    # Emit the top ready swarm-safe ticket id, or empty string. Excludes any
    # tickets already in-progress (claimed) and any matching --exclude args.
    local excludes=("$@")
    uv run "$REPO_ROOT/scripts/open_work_filters.py" ready --track swarm-safe 2>/dev/null \
        | awk '/^    [0-9]+/{ print $1 }' \
        | while read -r tid; do
            local skip="false"
            for e in "${excludes[@]:-}"; do
                [[ "$tid" == "$e" ]] && skip="true" && break
            done
            [[ "$skip" == "false" ]] && echo "$tid" && break
        done
}

ready_swarm_safe_count() {
    uv run "$REPO_ROOT/scripts/open_work_filters.py" ready --track swarm-safe 2>/dev/null \
        | awk '/^    [0-9]+/{ print $1 }' | wc -l | tr -d ' '
}

ticket_path() {
    local tid="$1"
    local unpadded padded
    unpadded=$(echo "$tid" | sed 's/^0*//'); [[ -z "$unpadded" ]] && unpadded="0"
    padded=$(printf "%03d" "$unpadded" 2>/dev/null || echo "$tid")
    ls "$REPO_ROOT/docs/open-work/tickets/${padded}-"*.md 2>/dev/null | head -1
}

compose_polecat_prompt() {
    local slug="$1" tid="$2" tfile="$3" workspace="$4"
    local title
    title=$(awk -F': *' '/^title:/ { sub(/^title: */, ""); print; exit }' "$tfile")
    cat <<PROMPT
You are a headless polecat session working on ticket $tid ("$title"),
swarm-safe track. Workspace: $workspace. Bookmark: session/$slug.

Constraints (load-bearing):
- Headless mode — NEVER ask the user a question. If the task is ambiguous,
  abandon: print "polecat-abandoned: $slug <one-line reason>" to stdout and
  exit immediately. Do not push the bookmark.
- Stay on session/$slug. Never touch main; never edit other bookmarks.
- Swarm-safe scope only: docs / frontmatter / mechanical refactor / atomic
  work with an already-verified layer-walk. If you discover this work
  actually requires substrate-sensitive judgment (a layer-walk row marked
  [suspect], an ECS schedule edit, anything balance-affecting), ABANDON
  per the rule above — do not promote scope.
- \`just check && just test\` must pass before you push. If they don't,
  abandon — do not commit broken state.

Verifiability triage (do this FIRST, before reading any source code or
running \`cargo\`):
Read the ticket body below. Decide *how* you would prove correctness:
- UI-rendering work (windowed overlay, render-pipeline change, log-viewer
  chart) is NOT a reason to abandon. Pixels are programmatically assertable.
  Use the existing toolchain:
    - Bevy windowed surface → \`Screenshot::primary_window()\` + \`save_to_disk\`
      are wired in \`src/rendering/camera.rs\` (F5 + \`AutoScreenshot\` resource).
      Spawn headed binary, scrub to a known tick, capture, exit, assert
      against the saved PNG (pixel diff or OCR on stable text regions).
    - \`tools/narrative-editor\` (vite/npm web app) → \`npm run dev\` on a
      fixed port, drive with Playwright (\`npm install --save-dev
      @playwright/test\` if missing), assert against DOM and canvas pixels.
  Most UI tickets also split: do the headless plumbing (event payload, query,
  hit-test math, anchor constants, color tokens) verified by
  \`just check && just test\`, then add the screenshot/Playwright assertion
  for the rendered surface.
- If verification needs running a full \`just soak\` > 5 min and comparing
  footers / continuity canaries → abandon NOW:
    print "polecat-abandoned: $slug requires-long-soak"
- If verification needs substrate-sensitive judgment (promoting a
  [suspect] layer-walk row, ECS-schedule reasoning, balance call) →
  abandon NOW:
    print "polecat-abandoned: $slug requires-substrate-judgment"
- Reserve \`requires-gui\` abandons for rare cases of genuinely subjective
  aesthetic judgment ("does this color feel right", "is the easing curve
  pleasant") — NOT for any ticket that renders pixels.
- If verification is \`just check && just test\` + reading a deterministic
  \`just\` recipe output (\`just inspect\`, \`just q\`, \`just explain\`,
  \`just similar\`) → proceed to the exit ceremony.
Reason: abandoning at prompt-read time burns ~1 minute; abandoning after
\`cargo build\` burns 5-10 minutes. Triage cheap, then commit.

Exit ceremony (non-optional, in this exact order):
  1. Run \`just check && just test\` inside this workspace. If either fails,
     abandon (do NOT proceed to step 2).
  2. Commit your work with jj (jj describe -m "<conventional message>"
     references ticket $tid).
  3. Run \`just land $tid\` to flip the ticket to status: done and
     regenerate docs/open-work.md.
  4. \`jj git push --bookmark session/$slug --allow-new\` (the master
     refinery picks up bookmarks from here).
  5. Print "polecat-done: $slug ticket-$tid" to stdout and exit.

If you abandon at any point, print "polecat-abandoned: $slug <reason>"
and exit WITHOUT pushing the bookmark. The master foreman will detect
the missing push and call session_done.sh to release your ticket claim
back to status: ready.

Ticket body follows below the divider. Read it, do the work, exit cleanly.

─────────────────────────────────────────────────────────────────────────
$(cat "$tfile")
PROMPT
}

# ─── modes ────────────────────────────────────────────────────────────────────

report() {
    local workspaces=()
    while read -r ws; do
        [[ -n "$ws" ]] && workspaces+=("$ws")
    done < <(list_polecat_workspaces)

    local ready_n
    ready_n=$(ready_swarm_safe_count)

    if [[ "$emit_json" == "true" ]]; then
        echo "{"
        echo "  \"ready_swarm_safe\": $ready_n,"
        echo "  \"polecats\": ["
        local i n=${#workspaces[@]}
        for ((i=0; i<n; i++)); do
            local row
            row=$(polecat_state "${workspaces[$i]}")
            IFS=$'\t' read -r slug pid alive last_edit deadline_rem tickets <<< "$row"
            local comma=","
            (( i == n - 1 )) && comma=""
            printf '    {"slug":"%s","pid":"%s","alive":"%s","last_edit_secs":"%s","deadline_secs_remaining":"%s","tickets":"%s"}%s\n' \
                "$slug" "$pid" "$alive" "$last_edit" "$deadline_rem" "$tickets" "$comma"
        done
        echo "  ]"
        echo "}"
        return
    fi

    echo "foreman: ${#workspaces[@]} polecat(s) tracked · $ready_n ready swarm-safe ticket(s)"
    if (( ${#workspaces[@]} > 0 )); then
        printf '%-22s %-8s %-8s %-10s %-12s %s\n' "SLUG" "PID" "ALIVE" "LAST-EDIT" "DEADLINE-IN" "TICKETS"
        for ws in "${workspaces[@]}"; do
            local row; row=$(polecat_state "$ws")
            IFS=$'\t' read -r slug pid alive last_edit deadline_rem tickets <<< "$row"
            printf '%-22s %-8s %-8s %-10s %-12s %s\n' \
                "$slug" "$pid" "$alive" "${last_edit}s" "${deadline_rem}s" "$tickets"
        done
    fi
}

spawn_one_polecat() {
    # Picks one ready swarm-safe ticket, claims + workspaces it via
    # just session-new, then spawns a claude -p subprocess. Returns the slug
    # on success, empty on failure.
    local already_taken=("$@")
    local tid; tid=$(pick_top_ready_swarm_safe "${already_taken[@]:-}")
    if [[ -z "$tid" ]]; then
        echo "foreman: no ready swarm-safe tickets available (already claimed $((${#already_taken[@]})))" >&2
        return 1
    fi

    local tfile; tfile=$(ticket_path "$tid")
    if [[ -z "$tfile" ]]; then
        echo "foreman: no ticket file for id $tid" >&2
        return 1
    fi

    local slug="swarmpole-$tid"
    local workspace="$SESSIONS_ROOT/$slug"

    if [[ -e "$workspace" ]]; then
        echo "foreman: $workspace already exists; skipping ticket $tid" >&2
        return 1
    fi

    # Atomic claim + workspace + bookmark + .session-info.json
    if ! (cd "$REPO_ROOT" && just session-new "$slug" --tickets "$tid" --track swarm-safe >/dev/null 2>&1); then
        echo "foreman: just session-new $slug failed (race? check logs)" >&2
        return 1
    fi

    # Compose the headless prompt
    local prompt; prompt=$(compose_polecat_prompt "$slug" "$tid" "$tfile" "$workspace")

    local wallclock_secs=$((wallclock_min * 60))
    local deadline=$(($(date +%s) + wallclock_secs))
    local session_id; session_id=$(uuidgen)

    if [[ "$dry_run" == "true" ]]; then
        echo "[dry-run] would spawn polecat for ticket $tid in $workspace (wallclock ${wallclock_min}m)"
        # Roll back the session-new claim since we're not actually running.
        (cd "$REPO_ROOT" && just session-done "$slug" >/dev/null 2>&1) || true
        return 0
    fi

    # Record the planned invocation BEFORE forking so post-mortem is possible
    # even if spawn fails immediately.
    cat > "$workspace/.polecat-cmdline" <<CMDLINE
cd $workspace && \\
nohup claude \\
  --print \\
  --output-format stream-json \\
  --verbose \\
  --include-partial-messages \\
  --permission-mode bypassPermissions \\
  --model sonnet \\
  --name "polecat-$slug" \\
  --session-id $session_id \\
  --no-session-persistence \\
  "<polecat prompt — see .polecat-prompt>"
CMDLINE
    printf '%s' "$prompt" > "$workspace/.polecat-prompt"
    echo "$deadline" > "$workspace/.polecat-deadline"

    # Fork the polecat: nohup + & + disown so foreman can exit without taking
    # the polecat down. Stream-json to .polecat-stream.jsonl; stderr to .log.
    # Exit code captured to .polecat-exit on natural termination.
    (
        cd "$workspace"
        nohup bash -c "
            claude \
                --print \
                --output-format stream-json \
                --verbose \
                --include-partial-messages \
                --permission-mode bypassPermissions \
                --model sonnet \
                --name 'polecat-$slug' \
                --session-id '$session_id' \
                --no-session-persistence \
                \"\$(cat .polecat-prompt)\" \
                > .polecat-stream.jsonl 2> .polecat-stderr.log
            echo \$? > .polecat-exit
        " >/dev/null 2>&1 &
        echo $! > .polecat.pid
        disown 2>/dev/null || true
    )

    local polecat_pid; polecat_pid=$(cat "$workspace/.polecat.pid")

    # Wallclock sentinel: sleeps for the deadline, then SIGTERMs the polecat
    # if it's still alive (giving 10s for /handoff to flush), then SIGKILLs.
    (
        sleep "$wallclock_secs"
        if kill -0 "$polecat_pid" 2>/dev/null; then
            echo "[wallclock-sentinel] $slug exceeded ${wallclock_min}m, SIGTERM" >> "$workspace/.polecat-stderr.log"
            kill -TERM "$polecat_pid" 2>/dev/null || true
            sleep 10
            if kill -0 "$polecat_pid" 2>/dev/null; then
                echo "[wallclock-sentinel] $slug still alive after SIGTERM, SIGKILL" >> "$workspace/.polecat-stderr.log"
                kill -KILL "$polecat_pid" 2>/dev/null || true
            fi
        fi
    ) >/dev/null 2>&1 &
    echo $! > "$workspace/.polecat-watchdog.pid"
    disown 2>/dev/null || true

    echo "$slug"
    return 0
}

spawn_mode() {
    if [[ -z "$spawn_n" ]] || ! [[ "$spawn_n" =~ ^[0-9]+$ ]] || (( spawn_n < 1 )); then
        echo "ERROR: --spawn requires a positive integer (got '$spawn_n')" >&2
        exit 2
    fi

    local available; available=$(ready_swarm_safe_count)
    if (( available < spawn_n )); then
        echo "ERROR: only $available ready swarm-safe ticket(s) available (requested $spawn_n)." >&2
        echo "       Run 'just open-work-ready-filtered --track swarm-safe' to inspect." >&2
        exit 1
    fi

    # Pre-flight: refuse if master at REPO_ROOT has uncommitted edits on @.
    # (Don't ship polecats against a moving target.)
    if (cd "$REPO_ROOT" && jj status 2>/dev/null) | grep -qE '^[AM] '; then
        echo "WARN: master workspace at $REPO_ROOT has uncommitted edits on @." >&2
        echo "      Polecats branch from main, so this won't affect them — proceeding." >&2
    fi

    echo "foreman: spawning $spawn_n polecat(s) [wallclock ${wallclock_min}m/each]..."
    local spawned=() claimed_tickets=()
    for ((i=0; i<spawn_n; i++)); do
        local slug; slug=$(spawn_one_polecat "${claimed_tickets[@]:-}") || {
            echo "foreman: spawn $((i+1))/$spawn_n failed; continuing with already-spawned." >&2
            break
        }
        # Extract ticket id from slug `swarmpole-<id>`
        local tid="${slug#swarmpole-}"
        spawned+=("$slug")
        claimed_tickets+=("$tid")
        if [[ "$dry_run" == "true" ]]; then
            : # dry-run already printed the plan line
        else
            echo "  spawned: $slug (ticket $tid, PID $(cat "$SESSIONS_ROOT/$slug/.polecat.pid"))"
        fi
    done

    if [[ "$dry_run" == "true" ]]; then
        echo "foreman --spawn --dry-run: nothing spawned (claims rolled back)."
        return
    fi

    if (( ${#spawned[@]} == 0 )); then
        echo "foreman: spawned 0 polecats; nothing to watch." >&2
        return 1
    fi

    # Auto-poll-and-land loop (synchronous; blocks until all polecats are done).
    echo
    echo "foreman: entering poll-and-land loop (${POLL_INTERVAL_SECS}s interval; Ctrl-C is safe — polecats keep running)..."
    auto_poll_and_land "${spawned[@]}"
}

archive_abandoned_polecat() {
    # Copy a non-landing polecat's diagnostic artifacts to logs/polecat-abandoned/
    # BEFORE session_done.sh --force removes the workspace. Extracts the
    # `polecat-abandoned: <slug> <reason>` line (if present) and writes it to a
    # one-line REASON file. Echoes the reason on stdout for the caller to log
    # (empty if the polecat exited without printing the abandon line).
    local slug="$1"
    local workspace="$SESSIONS_ROOT/$slug"
    local stream="$workspace/.polecat-stream.jsonl"
    local stderr_log="$workspace/.polecat-stderr.log"
    local cmdline="$workspace/.polecat-cmdline"

    local stamp; stamp=$(date -u +"%Y%m%d-%H%M%S")
    local archive_dir="$REPO_ROOT/logs/polecat-abandoned/${stamp}-${slug}"
    mkdir -p "$archive_dir"

    [[ -f "$stream" ]] && cp "$stream" "$archive_dir/polecat-stream.jsonl"
    [[ -f "$stderr_log" ]] && cp "$stderr_log" "$archive_dir/polecat-stderr.log"
    [[ -f "$cmdline" ]] && cp "$cmdline" "$archive_dir/polecat-cmdline"

    # Extract the abandon reason. The polecat is supposed to print
    # `polecat-abandoned: <slug> <reason>` as its final stdout. In stream-json
    # mode that lands inside an assistant text event; grep -o pulls it out.
    local reason=""
    if [[ -f "$stream" ]]; then
        reason=$(grep -o "polecat-abandoned: $slug [^\"\\\\]*" "$stream" 2>/dev/null \
            | tail -1 | sed -E "s/^polecat-abandoned: $slug //")
    fi
    if [[ -z "$reason" && -f "$stderr_log" ]]; then
        reason=$(grep -o "polecat-abandoned: $slug .*" "$stderr_log" 2>/dev/null \
            | tail -1 | sed -E "s/^polecat-abandoned: $slug //")
    fi
    [[ -z "$reason" ]] && reason="(no abandon-line found — see polecat-stream.jsonl)"
    echo "$reason" > "$archive_dir/REASON"

    # Echo to stdout for caller (without the placeholder fallback)
    if [[ "$reason" == "(no abandon-line found"* ]]; then
        echo ""
    else
        echo "$reason"
    fi
}

auto_poll_and_land() {
    local watched=("$@")
    while true; do
        local any_alive="false"
        for slug in "${watched[@]}"; do
            local pid_file="$SESSIONS_ROOT/$slug/.polecat.pid"
            [[ -f "$pid_file" ]] || continue
            local pid; pid=$(cat "$pid_file")
            if kill -0 "$pid" 2>/dev/null; then
                any_alive="true"
                break
            fi
        done

        if [[ "$any_alive" == "false" ]]; then
            echo "foreman: all polecats exited; draining via 'just refinery --auto'..."
            (cd "$REPO_ROOT" && just refinery --auto)

            # For any session whose bookmark didn't advance (polecat abandoned
            # or died), archive the abandon reason + diagnostic artifacts first,
            # THEN release the ticket-claim back to ready via session_done.
            for slug in "${watched[@]}"; do
                if [[ -d "$SESSIONS_ROOT/$slug" ]]; then
                    local reason
                    reason=$(archive_abandoned_polecat "$slug")
                    if [[ -n "$reason" ]]; then
                        echo "foreman: polecat $slug abandoned — $reason; releasing claim..."
                    else
                        echo "foreman: polecat $slug did not land (no abandon-line found); releasing claim..."
                    fi
                    (cd "$REPO_ROOT" && bash scripts/session_done.sh "$slug" --force) || \
                        echo "  WARN: session_done.sh $slug failed; manual cleanup needed" >&2
                fi
            done
            return
        fi

        sleep "$POLL_INTERVAL_SECS"
    done
}

watch_mode() {
    # One-shot heartbeat — designed to be called repeatedly (or wrapped in
    # `watch` for live updates).
    report
}

land_mode() {
    (cd "$REPO_ROOT" && just refinery --auto)
}

shutdown_mode() {
    local signal="-TERM"
    [[ "$shutdown_hard" == "true" ]] && signal="-KILL"
    local count=0
    while read -r ws; do
        [[ -z "$ws" ]] && continue
        local pid_file="$ws/.polecat.pid"
        [[ -f "$pid_file" ]] || continue
        local pid; pid=$(cat "$pid_file")
        if kill -0 "$pid" 2>/dev/null; then
            kill "$signal" "$pid" 2>/dev/null && {
                echo "foreman: sent $signal to $(basename "$ws") (PID $pid)"
                count=$((count + 1))
            }
        fi
    done < <(list_polecat_workspaces)
    echo "foreman --shutdown: signaled $count polecat(s)"
}

log_mode() {
    [[ -z "$log_slug" ]] && { echo "ERROR: --log requires a slug" >&2; exit 2; }
    local stream="$SESSIONS_ROOT/$log_slug/.polecat-stream.jsonl"
    if [[ ! -f "$stream" ]]; then
        echo "ERROR: no stream log for $log_slug ($stream)" >&2
        exit 1
    fi
    tail -f "$stream"
}

case "$mode" in
    report) report ;;
    spawn) spawn_mode ;;
    watch) watch_mode ;;
    land) land_mode ;;
    shutdown) shutdown_mode ;;
    log) log_mode ;;
esac
