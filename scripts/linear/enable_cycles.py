#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
Enable cycles on the Clowder Linear team.

Reads team key from arg (default: CLO). Issues a single teamUpdate
mutation setting cyclesEnabled: true. Prints the resulting cycle
settings.

This is the first Linear write — Phase 1 setup. Subsequent Phase 2 / 3
scripts (clear-target + bulk-import + label-create) live next to this
one. The audit script (audit_workspace.py) remains read-only as a
matter of principle.

Idempotent: re-running on a team where cycles are already enabled
returns success and re-prints the current settings.
"""
from __future__ import annotations

import json
import sys
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parents[2]
ENV_PATH = REPO / ".env"
ENDPOINT = "https://api.linear.app/graphql"
DEFAULT_TEAM_KEY = "CLO"


def load_env(path: Path) -> dict[str, str]:
    env: dict[str, str] = {}
    if not path.exists():
        return env
    for raw in path.read_text().splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        key, _, value = line.partition("=")
        env[key.strip()] = value.strip().strip('"').strip("'")
    return env


def graphql(query: str, api_key: str, variables: dict[str, Any] | None = None) -> dict:
    payload = json.dumps({"query": query, "variables": variables or {}}).encode("utf-8")
    req = urllib.request.Request(
        ENDPOINT,
        data=payload,
        headers={
            "Content-Type": "application/json",
            "Authorization": api_key,
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            body = json.loads(resp.read().decode("utf-8"))
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace")
        raise RuntimeError(f"HTTP {exc.code}: {detail}") from exc
    if body.get("errors"):
        raise RuntimeError(f"GraphQL errors: {json.dumps(body['errors'], indent=2)}")
    return body.get("data", {})


def find_team(api_key: str, key: str) -> dict:
    data = graphql(
        """
        query($key: String!) {
          teams(filter: { key: { eq: $key } }) {
            nodes {
              id key name
              cyclesEnabled
              cycleDuration
              cycleCooldownTime
              cycleStartDay
            }
          }
        }
        """,
        api_key,
        {"key": key},
    )
    nodes = data.get("teams", {}).get("nodes", []) or []
    if not nodes:
        raise RuntimeError(f"team with key '{key}' not found")
    return nodes[0]


def enable_cycles(api_key: str, team_id: str) -> dict:
    data = graphql(
        """
        mutation($id: String!, $input: TeamUpdateInput!) {
          teamUpdate(id: $id, input: $input) {
            success
            team {
              id key name
              cyclesEnabled
              cycleDuration
              cycleCooldownTime
              cycleStartDay
            }
          }
        }
        """,
        api_key,
        {
            "id": team_id,
            "input": {"cyclesEnabled": True},
        },
    )
    return data.get("teamUpdate", {})


def main() -> int:
    team_key = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_TEAM_KEY
    env = load_env(ENV_PATH)
    api_key = env.get("LINEAR_API_KEY") or ""
    if not api_key:
        print("error: LINEAR_API_KEY not found in .env or environment", file=sys.stderr)
        return 1

    team = find_team(api_key, team_key)
    print(f"team:    {team['key']} ({team['name']}) id={team['id']}")
    print(f"before:  cyclesEnabled={team['cyclesEnabled']} duration={team['cycleDuration']} cooldown={team['cycleCooldownTime']} startDay={team['cycleStartDay']}")

    if team["cyclesEnabled"]:
        print("cycles already enabled — no-op")
        return 0

    result = enable_cycles(api_key, team["id"])
    if not result.get("success"):
        print(f"error: teamUpdate did not return success: {result}", file=sys.stderr)
        return 2

    updated = result["team"]
    print(f"after:   cyclesEnabled={updated['cyclesEnabled']} duration={updated['cycleDuration']} cooldown={updated['cycleCooldownTime']} startDay={updated['cycleStartDay']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
