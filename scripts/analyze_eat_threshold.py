#!/usr/bin/env python3
"""Extract metrics for the eat_from_inventory_threshold balance pass.

Streams an events.jsonl file once, accumulates:
  - Header constants (for manual diff-constants sanity)
  - Footer: deaths_by_cause, features_activated
  - CatSnapshot hunger: count, mean, stddev, below-0.5 fraction, temporal bins
  - FoodLevel fraction over time (mean)
  - PreyKilled count per cat per 7000-tick week-bin (findability diagnostic)
  - ActionChosen counts by Action
  - PlanInterrupted counts by reason
  - PlanStepFailed counts by reason

Usage:
  scripts/analyze_eat_threshold.py LOGFILE [LABEL]
"""

from __future__ import annotations

import json
import math
import sys
from collections import Counter, defaultdict

# Leisure/variety actions vs. survival actions — used to summarize ActionChosen.
LEISURE_ACTIONS = {
    "Socialize", "Groom", "Mentor", "Wander", "Idle", "Explore",
    "Caretake", "Mate", "Cook", "Coordinate",
}
SURVIVAL_ACTIONS = {"Eat", "Sleep", "Hunt", "Forage", "Flee", "Fight"}
WORK_ACTIONS = {"Build", "Farm", "Herbcraft", "PracticeMagic", "Patrol"}

WEEK_TICKS = 7000  # 7 sim-days per week (ticks_per_day = 1000)


def analyze(path: str) -> dict:
    header: dict | None = None
    footer: dict | None = None
    hungers: list[float] = []
    # Hunger trajectory: bin by week, accumulate (sum, count) for mean per week.
    hunger_weekly: dict[int, tuple[float, int]] = defaultdict(lambda: (0.0, 0))
    foodlevels: list[float] = []
    prey_killed_by_week: Counter = Counter()
    prey_killed_by_cat_week: dict[tuple[str, int], int] = defaultdict(int)
    actions: Counter = Counter()
    action_chosen_total = 0
    plan_interrupts: Counter = Counter()
    plan_step_failures: Counter = Counter()
    search_timeouts = 0
    hunt_plans = 0

    with open(path, "r") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                ev = json.loads(line)
            except json.JSONDecodeError:
                # Event log has known ring-buffer flush corruption occasionally.
                continue
            if "_header" in ev:
                header = ev
                continue
            if "_footer" in ev:
                footer = ev
                continue
            t = ev.get("type")
            tick = ev.get("tick", 0)
            week = tick // WEEK_TICKS
            if t == "CatSnapshot":
                h = ev.get("needs", {}).get("hunger")
                if h is not None:
                    hungers.append(h)
                    s, c = hunger_weekly[week]
                    hunger_weekly[week] = (s + h, c + 1)
                ca = ev.get("current_action")
                if ca is not None:
                    actions[ca] += 1
                    action_chosen_total += 1
            elif t == "FoodLevel":
                foodlevels.append(ev.get("fraction", 0.0))
            elif t == "PreyKilled":
                prey_killed_by_week[week] += 1
                cat = ev.get("cat", "?")
                prey_killed_by_cat_week[(cat, week)] += 1
            elif t == "ActionChosen":
                action = ev.get("action")
                actions[action] += 1
                action_chosen_total += 1
            elif t == "PlanInterrupted":
                plan_interrupts[ev.get("reason", "?")] += 1
            elif t == "PlanStepFailed":
                reason = ev.get("reason", "?")
                plan_step_failures[reason] += 1
                if "timeout" in reason.lower() or "no scent" in reason.lower():
                    search_timeouts += 1
            elif t == "PlanCreated":
                if ev.get("disposition") == "Hunting":
                    hunt_plans += 1

    def stats(xs: list[float]) -> dict:
        if not xs:
            return {"count": 0}
        m = sum(xs) / len(xs)
        var = sum((x - m) ** 2 for x in xs) / len(xs)
        return {
            "count": len(xs),
            "mean": m,
            "stddev": math.sqrt(var),
            "below_0_5_frac": sum(1 for x in xs if x < 0.5) / len(xs),
            "below_0_3_frac": sum(1 for x in xs if x < 0.3) / len(xs),
            "min": min(xs),
            "max": max(xs),
        }

    result = {
        "path": path,
        "header_commit": header.get("commit_hash_short") if header else None,
        "header_dirty": header.get("commit_dirty") if header else None,
        "eat_from_inventory_threshold": (
            header.get("constants", {}).get("needs", {}).get("eat_from_inventory_threshold")
            if header else None
        ),
        "final_tick": footer.get("final_tick") if footer else None,
        "deaths_by_cause": footer.get("deaths_by_cause", {}) if footer else {},
        "features_activated": footer.get("features_activated", {}) if footer else {},
        "hunger_stats": stats(hungers),
        "hunger_weekly_mean": {
            int(w): s / c for w, (s, c) in sorted(hunger_weekly.items()) if c > 0
        },
        "foodlevel_stats": stats(foodlevels),
        "prey_killed_total": sum(prey_killed_by_week.values()),
        "prey_killed_by_week": dict(sorted(prey_killed_by_week.items())),
        "action_chosen_total": action_chosen_total,
        "action_counts": dict(actions.most_common()),
        "leisure_action_count": sum(actions[a] for a in LEISURE_ACTIONS),
        "survival_action_count": sum(actions[a] for a in SURVIVAL_ACTIONS),
        "work_action_count": sum(actions[a] for a in WORK_ACTIONS),
        "plan_interrupts": dict(plan_interrupts.most_common()),
        "plan_step_failures": dict(plan_step_failures.most_common()),
        "search_timeouts": search_timeouts,
        "hunt_plans": hunt_plans,
    }
    # Catches-per-cat-per-week (findability diagnostic).
    cats_by_week: dict[int, Counter] = defaultdict(Counter)
    for (cat, week), n in prey_killed_by_cat_week.items():
        cats_by_week[week][cat] = n
    result["catches_per_cat_by_week"] = {
        int(w): dict(c) for w, c in sorted(cats_by_week.items())
    }
    return result


def main() -> None:
    if len(sys.argv) < 2:
        print("usage: analyze_eat_threshold.py LOGFILE [LABEL]", file=sys.stderr)
        sys.exit(2)
    path = sys.argv[1]
    label = sys.argv[2] if len(sys.argv) > 2 else path
    r = analyze(path)
    r["label"] = label
    json.dump(r, sys.stdout, indent=2, default=str)
    print()


if __name__ == "__main__":
    main()
