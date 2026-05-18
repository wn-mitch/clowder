#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
Linear workspace audit — read-only.

Phase 1 of the Linear migration. Queries the workspace identified by
LINEAR_API_KEY (loaded from .env or env var) and reports:

- Authenticated viewer + organization
- Every team + current issue count + cycle settings
- Workflow states per team (status mapping inputs for Phase 2)
- Existing labels per team
- Projects
- A sample of existing issues per team (so the user can decide what to clear)
- Plan-tier capabilities (custom field availability)
- Rate-limit headers from the last response

Writes:
- logs/linear-audit-<UTC timestamp>.json — full JSON dump
- docs/migrations/linear-audit-summary.md — human-readable summary

This script makes only `query { … }` GraphQL calls. No mutations. The
import + clear-target scripts live elsewhere and run only after this
audit has informed Phase 2 design.
"""
from __future__ import annotations

import json
import sys
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parents[2]
ENV_PATH = REPO / ".env"
LOGS = REPO / "logs"
MIGRATIONS = REPO / "docs" / "migrations"
ENDPOINT = "https://api.linear.app/graphql"


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


def graphql(query: str, api_key: str, variables: dict[str, Any] | None = None) -> tuple[dict, dict]:
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
            rate_headers = {
                k: v for k, v in resp.headers.items()
                if "ratelimit" in k.lower() or "x-rate" in k.lower()
            }
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace")
        raise RuntimeError(f"HTTP {exc.code} from Linear: {detail}") from exc

    if "errors" in body and body["errors"]:
        raise RuntimeError(f"GraphQL errors: {json.dumps(body['errors'], indent=2)}")
    return body.get("data", {}), rate_headers


def query_identity(api_key: str) -> tuple[dict, dict]:
    return graphql(
        """
        query {
          viewer { id name email displayName }
          organization {
            id name urlKey
            createdAt
            subscription { type seats }
          }
        }
        """,
        api_key,
    )


def query_teams(api_key: str) -> tuple[dict, dict]:
    return graphql(
        """
        query {
          teams(first: 100) {
            nodes {
              id key name
              issueCount
              cyclesEnabled
              cycleDuration
              cycleCooldownTime
              defaultIssueState { name }
              triageEnabled
            }
          }
        }
        """,
        api_key,
    )


def query_workflow_states(api_key: str) -> tuple[dict, dict]:
    return graphql(
        """
        query {
          workflowStates(first: 250) {
            nodes {
              id name type position
              team { key name }
            }
          }
        }
        """,
        api_key,
    )


def query_labels(api_key: str) -> tuple[dict, dict]:
    return graphql(
        """
        query {
          issueLabels(first: 250) {
            nodes {
              id name color
              team { key name }
            }
          }
        }
        """,
        api_key,
    )


def query_projects(api_key: str) -> tuple[dict, dict]:
    return graphql(
        """
        query {
          projects(first: 100) {
            nodes { id name state startDate }
          }
        }
        """,
        api_key,
    )


def query_team_issues_sample(api_key: str, team_id: str) -> tuple[dict, dict]:
    return graphql(
        """
        query($teamId: String!) {
          team(id: $teamId) {
            id key
            issues(first: 25) {
              pageInfo { hasNextPage }
              nodes {
                id number title createdAt
                state { name }
                labels { nodes { name } }
              }
            }
          }
        }
        """,
        api_key,
        {"teamId": team_id},
    )


def render_summary(report: dict) -> str:
    org = report["identity"]["organization"]
    viewer = report["identity"]["viewer"]
    teams = report["teams"]
    states = report["workflow_states"]
    labels = report["labels"]
    projects = report["projects"]
    samples = report["team_issue_samples"]
    rate = report["rate_limit_last"]

    lines: list[str] = []
    lines.append("# Linear workspace audit — Phase 1 of ticket migration")
    lines.append("")
    lines.append(f"Generated: {report['generated_at']}")
    lines.append("")

    lines.append("## Workspace identity")
    lines.append("")
    lines.append(f"- Organization: **{org['name']}** ({org['urlKey']}, id {org['id']})")
    sub = org.get("subscription") or {}
    lines.append(f"- Subscription: type={sub.get('type', 'free?')} seats={sub.get('seats', '?')}")
    lines.append(f"- Created: {org.get('createdAt', '?')}")
    lines.append(f"- Viewer: **{viewer.get('displayName') or viewer['name']}** <{viewer['email']}> (id {viewer['id']})")
    lines.append("")

    lines.append("## Teams")
    lines.append("")
    if not teams:
        lines.append("_None found._")
    else:
        lines.append("| Key | Name | Issues | Cycles | Cycle len | Default state |")
        lines.append("| --- | --- | ---: | --- | ---: | --- |")
        for t in teams:
            cycles = "yes" if t.get("cyclesEnabled") else "no"
            default_state = (t.get("defaultIssueState") or {}).get("name") or "—"
            cycle_len = t.get("cycleDuration") or "—"
            lines.append(f"| {t['key']} | {t['name']} | {t.get('issueCount', '?')} | {cycles} | {cycle_len} | {default_state} |")
    lines.append("")

    lines.append("## Workflow states (per team)")
    lines.append("")
    by_team: dict[str, list[dict]] = {}
    for s in states:
        team_key = (s.get("team") or {}).get("key") or "(global)"
        by_team.setdefault(team_key, []).append(s)
    for team_key, st_list in sorted(by_team.items()):
        lines.append(f"### {team_key}")
        lines.append("")
        for s in sorted(st_list, key=lambda x: (x.get("type", ""), x.get("position", 0))):
            lines.append(f"- `{s['name']}` (type={s['type']}, position={s.get('position', '?')})")
        lines.append("")

    lines.append("## Existing labels")
    lines.append("")
    label_by_team: dict[str, list[str]] = {}
    for lbl in labels:
        team_key = (lbl.get("team") or {}).get("key") or "(workspace-global)"
        label_by_team.setdefault(team_key, []).append(lbl["name"])
    if not label_by_team:
        lines.append("_None._")
    else:
        for team_key, names in sorted(label_by_team.items()):
            lines.append(f"- **{team_key}**: " + ", ".join(f"`{n}`" for n in sorted(names)))
    lines.append("")

    lines.append("## Projects")
    lines.append("")
    if not projects:
        lines.append("_None._")
    else:
        for p in projects:
            lines.append(f"- `{p['name']}` (state={p.get('state', '?')}, start={p.get('startDate', '—')})")
    lines.append("")

    lines.append("## Existing issues sample (per team)")
    lines.append("")
    lines.append("Will has authorized clearing the chosen target team before bulk import. This section reports what's currently there so the choice is informed.")
    lines.append("")
    for team_key, sample in sorted(samples.items()):
        nodes = (sample.get("issues") or {}).get("nodes") or []
        has_more = (sample.get("issues") or {}).get("pageInfo", {}).get("hasNextPage")
        suffix = " (more present — sample only)" if has_more else ""
        lines.append(f"### {team_key}{suffix}")
        lines.append("")
        if not nodes:
            lines.append("_Empty — no existing issues._")
        else:
            for issue in nodes:
                state_name = (issue.get("state") or {}).get("name", "?")
                lines.append(f"- {team_key}-{issue['number']} **{issue['title']}** _({state_name})_")
        lines.append("")

    lines.append("## Rate limit (from last response)")
    lines.append("")
    if not rate:
        lines.append("_No rate-limit headers in response._")
    else:
        for k, v in sorted(rate.items()):
            lines.append(f"- `{k}`: {v}")
    lines.append("")

    lines.append("## Open questions for Phase 2")
    lines.append("")
    lines.append("- **Target team**: Which team hosts the migration? Will has authorized clearing existing issues in the chosen team — pick based on the issue inventory above.")
    lines.append("- **Plan tier capabilities**: Linear's free tier omits custom fields; Phase 2 field mapping needs confirmation that the chosen team's plan supports the `Legacy ID`, `Orchestration`, `Block`, `Verdict anchor`, `Wires method` custom fields. If not, those compress into labels.")
    lines.append("- **Pre-existing tickets**: `docs/open-work/pre-existing/{dead-features-in-activation-tracker,substrate-stub-catalogue}.md` — migrate as issues, projects, or stay as repo docs?")
    lines.append("- **Duplicate NNNs in landed/** (pre-existing, surfaced by the archaeology): IDs 001 (active + landed have different work), 014 (3 files, phase-numbered sub-tickets), 024 (2 files, warmth split phases), 072 (2 files, appear truly duplicated). Phase 2 must collapse these into single Linear issues or assign new IDs.")
    lines.append("- **Bulk import order**: With Linear ID == NNN as a hard constraint, the importer must run in strict numeric order 001..411 against an empty team. Decide whether to clear the existing target team (Will-authorized) or pick a different empty team.")
    lines.append("")

    return "\n".join(lines) + "\n"


def main() -> int:
    env = load_env(ENV_PATH)
    api_key = env.get("LINEAR_API_KEY") or ""
    if not api_key:
        print("error: LINEAR_API_KEY not found in .env or environment", file=sys.stderr)
        return 1

    LOGS.mkdir(parents=True, exist_ok=True)
    MIGRATIONS.mkdir(parents=True, exist_ok=True)

    print("Querying Linear API (read-only)...")

    identity, _ = query_identity(api_key)
    print(f"  ✓ identity: {identity['viewer']['email']} @ {identity['organization']['name']}")

    teams_data, _ = query_teams(api_key)
    teams = teams_data.get("teams", {}).get("nodes", []) or []
    print(f"  ✓ teams: {len(teams)}")

    states_data, _ = query_workflow_states(api_key)
    states = states_data.get("workflowStates", {}).get("nodes", []) or []
    print(f"  ✓ workflow states: {len(states)}")

    labels_data, _ = query_labels(api_key)
    labels = labels_data.get("issueLabels", {}).get("nodes", []) or []
    print(f"  ✓ labels: {len(labels)}")

    projects_data, _ = query_projects(api_key)
    projects = projects_data.get("projects", {}).get("nodes", []) or []
    print(f"  ✓ projects: {len(projects)}")

    team_issue_samples: dict[str, dict] = {}
    last_rate: dict[str, str] = {}
    for t in teams:
        sample, rate_headers = query_team_issues_sample(api_key, t["id"])
        team_issue_samples[t["key"]] = sample.get("team", {})
        last_rate = rate_headers or last_rate
        n = len(sample.get("team", {}).get("issues", {}).get("nodes", []) or [])
        print(f"  ✓ {t['key']} issue sample: {n}")

    now = datetime.now(timezone.utc)
    report = {
        "generated_at": now.isoformat(timespec="seconds"),
        "identity": identity,
        "teams": teams,
        "workflow_states": states,
        "labels": labels,
        "projects": projects,
        "team_issue_samples": team_issue_samples,
        "rate_limit_last": last_rate,
    }

    stamp = now.strftime("%Y%m%dT%H%M%SZ")
    json_path = LOGS / f"linear-audit-{stamp}.json"
    json_path.write_text(json.dumps(report, indent=2, default=str))
    print(f"  → {json_path.relative_to(REPO)}")

    summary_path = MIGRATIONS / "linear-audit-summary.md"
    summary_path.write_text(render_summary(report))
    print(f"  → {summary_path.relative_to(REPO)}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
