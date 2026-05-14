#!/usr/bin/env bash
# Enforces the HTN method-registry contract (CLAUDE.md §"Bugfix
# discipline" / "Every dormant method has a glue ticket"). Spec:
# ticket 319, child #1 of the 128 epic.
#
# Bidirectional audit: every `ApplicableWhen::PendingSubstrate` method
# in `src/ai/methods/` must name an OPEN ticket in
# `docs/open-work/tickets/`, AND that ticket's frontmatter must carry
# `wires-method: [<method-id>, ...]` referencing the method back.
#
#   * Pass A: PendingSubstrate.blocker → docs/open-work/tickets/<id>-*.md
#             exists (and is not in docs/open-work/landed/).
#   * Pass B: that ticket's `wires-method:` frontmatter array contains
#             the method's id slug.
#
# Closes the regression vector flagged in CLAUDE.md: dormant methods
# leak into the codebase without glue tickets, and the "natural-trees-
# never-sprout" failure mode takes over because nobody trips over the
# design intent in their work surface.
#
# Allowlist format (mirrors scripts/influence_map_registry.allowlist):
#   <method-id> <ticket-id>     # rationale
# Comments after `#` ignored.
#
# Method-declaration convention (anchors the parser): every `Method`
# literal that uses `ApplicableWhen::PendingSubstrate` MUST sit in a
# file under `src/ai/methods/` (excluding `tests.rs`), with
#   id: MethodId("<slug>")
# and
#   blocker: "<ticket-id>"
# each on its own line. The parser walks `Method {` → matching `}`
# blocks; nested struct literals (`PendingSubstrate { … }`,
# `MethodId(…)`) are tolerated via brace counting.
#
# Side mode:
#   $0 --list-json
# emits a JSON array of {method_id, blocker, state, source} for every
# scanned method (live or pending) to stdout, exit 0 regardless of
# whether any are unwired. Used by `scripts/methods.py` (the
# `just methods` audit surface). Never gates CI.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

METHODS_DIR="src/ai/methods"
TICKETS_DIR="docs/open-work/tickets"
LANDED_DIR="docs/open-work/landed"
ALLOWLIST="scripts/methods.allowlist"

MODE="check"
if [ "${1:-}" = "--list-json" ]; then
    MODE="list-json"
fi

# Parse allowlist into a flat array of method-ids.
allowlist=()
if [ -f "$ALLOWLIST" ]; then
    while IFS= read -r line; do
        line="${line%%#*}"
        line="${line#"${line%%[![:space:]]*}"}"
        line="${line%"${line##*[![:space:]]}"}"
        if [ -n "$line" ]; then
            name="${line%% *}"
            allowlist+=("$name")
        fi
    done < "$ALLOWLIST"
fi

is_allowlisted() {
    local name="$1"
    for entry in "${allowlist[@]+"${allowlist[@]}"}"; do
        [ "$entry" = "$name" ] && return 0
    done
    return 1
}

# Files to scan: every .rs under src/ai/methods/ EXCEPT tests.rs.
# Test fixtures intentionally construct synthetic `PendingSubstrate`
# variants; the lint must not see them.
methods_files=()
if [ -d "$METHODS_DIR" ]; then
    while IFS= read -r f; do
        methods_files+=("$f")
    done < <(find "$METHODS_DIR" -type f -name '*.rs' ! -name 'tests.rs' | sort)
fi

# Walk `Method { … }` struct literals and emit one TSV record per
# method to stdout: <method-id>\t<state>\t<blocker_or_-->\t<source>
# state ∈ {Live, PendingSubstrate}. Returns nothing if no Method blocks
# found (the empty-registry case at 319 landing).
#
# Implemented with POSIX awk (BSD awk on macOS doesn't support GNU's
# match-with-capture); string extraction uses `index` + `substr`.
extract_methods() {
    local file="$1"
    awk -v FILE="$file" '
        function extract_quoted(s, key,    p, rest, q) {
            p = index(s, key)
            if (p == 0) return ""
            rest = substr(s, p + length(key))
            q = index(rest, "\"")
            if (q == 0) return ""
            rest = substr(rest, q + 1)
            q = index(rest, "\"")
            if (q == 0) return ""
            return substr(rest, 1, q - 1)
        }

        function flush_record() {
            if (in_method && method_id != "") {
                state = (saw_pending ? "PendingSubstrate" : "Live")
                b = (blocker != "" ? blocker : "-")
                printf "%s\t%s\t%s\t%s:%d\n", method_id, state, b, FILE, method_start_line
            }
            in_method = 0
            method_depth = 0
            method_id = ""
            blocker = ""
            saw_pending = 0
            method_start_line = 0
        }

        BEGIN {
            depth = 0
            in_method = 0
            method_depth = 0
            method_id = ""
            blocker = ""
            saw_pending = 0
            method_start_line = 0
        }

        {
            # Strip a trailing line comment (// ... to end of line).
            # Block comments are not handled — the convention forbids
            # them inside Method literals; rg/clippy will catch any
            # accidental use.
            line = $0
            sub(/\/\/.*$/, "", line)

            # Field extraction within a Method block. Each `id` /
            # `blocker` MUST live on its own line per the convention.
            if (in_method) {
                if (line ~ /id:[[:space:]]*MethodId\("[^"]*"\)/) {
                    v = extract_quoted(line, "MethodId(")
                    if (v != "") method_id = v
                }
                if (line ~ /ApplicableWhen::PendingSubstrate/) {
                    saw_pending = 1
                }
                if (saw_pending && line ~ /blocker:[[:space:]]*"[^"]*"/) {
                    v = extract_quoted(line, "blocker:")
                    if (v != "") blocker = v
                }
            }

            # Detect `Method {` (struct-literal start). Anchor on word
            # boundaries via a lookbehind-style class to avoid matching
            # identifiers like `MethodId` or `MethodFailure`. Defensive
            # guard: reject `struct Method {`, `enum Method {`,
            # `union Method {`, `trait Method {`, and `impl … for Method {`
            # — those are type-definition openings, not struct literals.
            # The convention writes literals at the top of a per-method
            # `pub fn …() -> Method { Method { … } }` or directly as
            # `registry.push(Method { … })`.
            if (!in_method && line ~ /(^|[^A-Za-z0-9_:])Method[[:space:]]*\{/) {
                is_typedef = (line ~ /(^|[^A-Za-z0-9_:])(struct|enum|union|trait)[[:space:]]+Method[[:space:]]*\{/)
                is_impl_target = (line ~ /(^|[^A-Za-z0-9_:])for[[:space:]]+Method[[:space:]]*\{/)
                if (!is_typedef && !is_impl_target) {
                    in_method = 1
                    method_start_line = NR
                    method_depth = depth
                }
            }

            # Count braces on this line (after stripping line comment).
            # `gsub` returns the substitution count; the substitution is
            # identity-mapping so `line` itself is unchanged.
            n_open = gsub(/\{/, "{", line)
            n_close = gsub(/\}/, "}", line)

            depth += n_open
            depth -= n_close

            # If we are inside a Method block and depth returned to
            # method_depth, the block has closed: emit and reset.
            if (in_method && depth <= method_depth) {
                flush_record()
            }
        }

        END {
            # Defensive: a Method block that never closed (unbalanced
            # braces) is a malformed file; flush anyway so the caller
            # sees the partial record.
            flush_record()
        }
    ' "$file"
}

# Lookup a ticket's frontmatter `wires-method:` array. Echoes one
# method-id per line. Empty output = no wires-method field (or field
# is empty `[]`).
extract_wires_method() {
    local file="$1"
    awk '
        BEGIN { in_fm = 0 }
        /^---[[:space:]]*$/ {
            in_fm = !in_fm
            next
        }
        in_fm && /^wires-method:/ {
            line = $0
            sub(/^wires-method:[[:space:]]*/, "", line)
            sub(/^\[/, "", line)
            sub(/\][[:space:]]*$/, "", line)
            n = split(line, parts, /,/)
            for (i = 1; i <= n; i++) {
                v = parts[i]
                gsub(/^[[:space:]]+|[[:space:]]+$/, "", v)
                gsub(/^"|"$/, "", v)
                if (v != "") print v
            }
        }
    ' "$file"
}

# Resolve a ticket id to a path in TICKETS_DIR. Echoes "" if absent.
# Refuses landed/ matches — Pass A requires the blocker to be open.
ticket_path() {
    local id="$1"
    local match
    match=$(find "$TICKETS_DIR" -maxdepth 1 -type f -name "${id}-*.md" 2>/dev/null | head -1 || true)
    if [ -n "$match" ]; then
        echo "$match"
        return 0
    fi
    return 1
}

ticket_landed() {
    local id="$1"
    local match
    match=$(find "$LANDED_DIR" -maxdepth 1 -type f -name "${id}-*.md" 2>/dev/null | head -1 || true)
    [ -n "$match" ]
}

# ---- Collect records from all files ----
records=()
for f in "${methods_files[@]+"${methods_files[@]}"}"; do
    while IFS= read -r rec; do
        if [ -n "$rec" ]; then
            records+=("$rec")
        fi
    done < <(extract_methods "$f")
done

# ---- --list-json side mode ----
if [ "$MODE" = "list-json" ]; then
    printf '['
    first=1
    for rec in "${records[@]+"${records[@]}"}"; do
        IFS=$'\t' read -r mid state blocker src <<< "$rec"
        if [ $first -eq 1 ]; then
            first=0
        else
            printf ','
        fi
        # Conservative JSON escape: backslashes and double-quotes only.
        # Field values are all derived from Rust identifiers / ticket
        # ids / file paths — no control chars expected.
        esc_mid=${mid//\\/\\\\}; esc_mid=${esc_mid//\"/\\\"}
        esc_blocker=${blocker//\\/\\\\}; esc_blocker=${esc_blocker//\"/\\\"}
        esc_src=${src//\\/\\\\}; esc_src=${esc_src//\"/\\\"}
        printf '{"method_id":"%s","state":"%s","blocker":"%s","source":"%s"}' \
            "$esc_mid" "$state" "$esc_blocker" "$esc_src"
    done
    printf ']\n'
    exit 0
fi

# ---- CI gate (default mode) ----
offenders=()
for rec in "${records[@]+"${records[@]}"}"; do
    IFS=$'\t' read -r mid state blocker src <<< "$rec"
    if [ "$state" != "PendingSubstrate" ]; then
        continue
    fi
    if is_allowlisted "$mid"; then
        continue
    fi
    # Pass A: blocker must be present and name an open ticket.
    if [ -z "$blocker" ] || [ "$blocker" = "-" ]; then
        offenders+=("${mid}|${src}|missing-blocker|")
        continue
    fi
    if ticket_landed "$blocker"; then
        offenders+=("${mid}|${src}|blocker-landed|${blocker}")
        continue
    fi
    tpath=$(ticket_path "$blocker" || true)
    if [ -z "$tpath" ]; then
        offenders+=("${mid}|${src}|blocker-missing|${blocker}")
        continue
    fi
    # Pass B: blocker ticket must carry `wires-method:` referencing mid.
    found=0
    while IFS= read -r wm; do
        if [ "$wm" = "$mid" ]; then
            found=1
            break
        fi
    done < <(extract_wires_method "$tpath")
    if [ $found -ne 1 ]; then
        offenders+=("${mid}|${src}|frontmatter-missing|${blocker}|${tpath}")
    fi
done

if [ "${#offenders[@]}" -ne 0 ]; then
    echo "HTN method-registry stub(s) detected:" >&2
    for o in "${offenders[@]}"; do
        IFS='|' read -r mid src reason blocker tpath <<< "$o"
        echo "  - method '${mid}' at ${src}:" >&2
        case "$reason" in
            missing-blocker)
                echo "    PendingSubstrate variant has no \`blocker:\` field." >&2
                ;;
            blocker-landed)
                echo "    blocker '${blocker}' points to a LANDED ticket" >&2
                echo "    (docs/open-work/landed/). Move the method to" >&2
                echo "    ApplicableWhen::Live (and remove the blocker field)," >&2
                echo "    or repoint to an open glue ticket." >&2
                ;;
            blocker-missing)
                echo "    blocker '${blocker}' names no ticket in ${TICKETS_DIR}." >&2
                echo "    Open one via \`just open-ticket\` (with the matching" >&2
                echo "    \`wires-method: [${mid}]\` frontmatter) or repoint." >&2
                ;;
            frontmatter-missing)
                echo "    blocker ticket ${tpath} exists but its frontmatter" >&2
                echo "    \`wires-method:\` does not include '${mid}'." >&2
                echo "    Edit the ticket's frontmatter to add the method id." >&2
                ;;
        esac
    done
    echo >&2
    echo "Fix: see CLAUDE.md §\"Bugfix discipline\" / \"Every dormant method has a" >&2
    echo "glue ticket.\" The discipline is bidirectional: dormant method → open" >&2
    echo "ticket exists → ticket frontmatter carries \`wires-method: [<id>...]\`." >&2
    echo "For follow-on work landing a method ahead of its glue ticket, add an" >&2
    echo "allowlist entry in ${ALLOWLIST} naming the wiring ticket." >&2
    exit 1
fi

count="${#records[@]}"
echo "HTN method registry: ${count} method(s), all registered."
exit 0
