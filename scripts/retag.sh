#!/usr/bin/env bash
# Single-ticket orchestration retag. Sets or updates the
# `orchestration:` field and (optionally) `block:`, `verdict-anchor:`,
# and `initiative:` in the ticket's frontmatter.
#
# Idempotent — re-running with the same flags is a no-op.
# Validated by scripts/check_orchestration_frontmatter.py.
#
# Usage:
#   retag.sh <id> --track <substrate-sensitive|coherent-block|swarm-safe>
#                 [--block <name>]
#                 [--anchor]
#                 [--initiative <name1,name2>]   # appends to list (dedup)
#                 [--unset-anchor]
#                 [--unset-block]

set -euo pipefail

TICKETS_DIR="docs/open-work/tickets"

usage() {
    sed -n '/^# Usage:/,/^$/p' "$0" | sed 's/^# \?//'
    exit "${1:-2}"
}

ticket_id=""
track=""
block=""
anchor="false"
unset_anchor="false"
unset_block="false"
initiative_add=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --track) track="$2"; shift 2 ;;
        --block) block="$2"; shift 2 ;;
        --anchor) anchor="true"; shift ;;
        --unset-anchor) unset_anchor="true"; shift ;;
        --unset-block) unset_block="true"; shift ;;
        --initiative) initiative_add="$2"; shift 2 ;;
        -h|--help) usage 0 ;;
        --*) echo "Unknown flag: $1" >&2; usage ;;
        *) ticket_id="$1"; shift ;;
    esac
done

[[ -z "$ticket_id" ]] && { echo "ERROR: missing <id>" >&2; usage; }
[[ -z "$track" && -z "$unset_anchor" && -z "$unset_block" && -z "$initiative_add" && "$anchor" == "false" && -z "$block" ]] && {
    echo "ERROR: nothing to do (pass --track / --block / --anchor / --initiative)" >&2; usage; }

# Locate the file (id may have leading zeros; tolerate either form)
padded=$(printf "%03d" "$ticket_id" 2>/dev/null || echo "$ticket_id")
file=$(ls "$TICKETS_DIR/${padded}-"*.md 2>/dev/null | head -1)
if [[ -z "$file" ]]; then
    file=$(ls "$TICKETS_DIR/${ticket_id}-"*.md 2>/dev/null | head -1)
fi
[[ -z "$file" ]] && { echo "ERROR: no ticket file matches id '$ticket_id'" >&2; exit 1; }

# Validate track value if set
if [[ -n "$track" ]]; then
    case "$track" in
        substrate-sensitive|coherent-block|swarm-safe) ;;
        *) echo "ERROR: track must be one of substrate-sensitive|coherent-block|swarm-safe (got '$track')" >&2; exit 1 ;;
    esac
fi

# Helper: replace a single-line frontmatter field in place
set_field() {
    local fld="$1" val="$2"
    if grep -qE "^${fld}:" "$file"; then
        awk -v fld="$fld" -v val="$val" '
            BEGIN { in_fm=0; fm_count=0; done=0 }
            /^---$/ {
                fm_count++
                if (fm_count==1) { in_fm=1; print; next }
                if (fm_count==2) { in_fm=0; print; next }
            }
            in_fm && !done && $0 ~ "^" fld ":" {
                print fld ": " val
                done=1
                next
            }
            { print }
        ' "$file" > "$file.tmp" && mv "$file.tmp" "$file"
    else
        # Insert after cluster: (or after orchestration: if present)
        local anchor_line='orchestration:'
        grep -q "^${anchor_line}" "$file" || anchor_line='cluster:'
        awk -v fld="$fld" -v val="$val" -v anchor="$anchor_line" '
            BEGIN { ins=0 }
            $0 ~ "^" anchor && !ins { print; print fld ": " val; ins=1; next }
            { print }
        ' "$file" > "$file.tmp" && mv "$file.tmp" "$file"
    fi
}

# Helper: remove a frontmatter field
unset_field() {
    local fld="$1"
    awk -v fld="$fld" '
        BEGIN { in_fm=0; fm_count=0 }
        /^---$/ {
            fm_count++
            if (fm_count==1) { in_fm=1; print; next }
            if (fm_count==2) { in_fm=0; print; next }
        }
        in_fm && $0 ~ "^" fld ":" { next }
        { print }
    ' "$file" > "$file.tmp" && mv "$file.tmp" "$file"
}

# Helper: append to YAML inline list (e.g., initiative: [a, b])
append_to_list() {
    local fld="$1" val="$2"
    if ! grep -qE "^${fld}:" "$file"; then
        set_field "$fld" "[${val}]"
        return
    fi
    # Read current list, append if new, write back
    python3 - "$file" "$fld" "$val" <<'PY'
import re, sys
file, fld, val = sys.argv[1:4]
with open(file) as f:
    text = f.read()
pat = re.compile(rf"^{re.escape(fld)}:\s*\[([^\]]*)\]\s*$", re.MULTILINE)
m = pat.search(text)
if not m:
    sys.exit(0)
items = [s.strip().strip('"').strip("'") for s in m.group(1).split(",") if s.strip()]
new_items = items[:]
for v in val.split(","):
    v = v.strip()
    if v and v not in new_items:
        new_items.append(v)
new_line = f"{fld}: [{', '.join(new_items)}]"
text = pat.sub(new_line, text, count=1)
with open(file, "w") as f:
    f.write(text)
PY
}

if [[ -n "$track" ]]; then
    set_field "orchestration" "$track"
fi
if [[ -n "$block" ]]; then
    set_field "block" "$block"
fi
if [[ "$unset_block" == "true" ]]; then
    unset_field "block"
fi
if [[ "$anchor" == "true" ]]; then
    set_field "verdict-anchor" "true"
fi
if [[ "$unset_anchor" == "true" ]]; then
    unset_field "verdict-anchor"
fi
if [[ -n "$initiative_add" ]]; then
    append_to_list "initiative" "$initiative_add"
fi

echo "retag: $file ← track=${track:-_} block=${block:-_} anchor=${anchor} initiative+=${initiative_add:-_}"
