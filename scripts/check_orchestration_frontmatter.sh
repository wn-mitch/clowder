#!/usr/bin/env bash
# Wrapper around scripts/check_orchestration_frontmatter.py for the
# `just check` recipe. Python keeps the YAML-ish frontmatter parsing
# correct without taking a dependency on bash 4+ associative arrays.
#
# See the .py for the four invariants and rationale.

set -euo pipefail
exec python3 "$(dirname "$0")/check_orchestration_frontmatter.py" "$@"
