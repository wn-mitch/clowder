#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = [
#     "numpy",
#     "matplotlib",
# ]
# ///
"""
Balance report generator for Clowder simulation diagnostics.

Parses logs/events.jsonl, fits trend lines to key metrics, and produces
PNG charts in logs/charts/ plus a text summary.

Usage:
    uv run scripts/balance_report.py [--events PATH] [--out DIR]
"""

import argparse
import json
import os
import sys
from collections import defaultdict
from pathlib import Path

import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np


def parse_events(path: str) -> tuple[dict | None, list[dict]]:
    header = None
    events = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            obj = json.loads(line)
            if obj.get("_header"):
                header = obj
                continue
            events.append(obj)
    return header, events


def bucket_events(events: list[dict]) -> dict[str, list[dict]]:
    buckets: dict[str, list[dict]] = defaultdict(list)
    for e in events:
        buckets[e.get("type", "unknown")].append(e)
    return buckets


def fit_line(xs, ys):
    """Return (slope, intercept) for a linear fit, or (0, mean) if degenerate."""
    if len(xs) < 2:
        return (0.0, float(np.mean(ys)) if len(ys) else 0.0)
    coeffs = np.polyfit(xs, ys, 1)
    return (float(coeffs[0]), float(coeffs[1]))


# ---------------------------------------------------------------------------
# Chart 1: Per-cat respect trajectories
# ---------------------------------------------------------------------------

def plot_respect(snapshots: list[dict], out_dir: str):
    cats: dict[str, dict] = {}  # name -> {ticks, respect, pride}
    for s in snapshots:
        name = s["cat"]
        if name not in cats:
            cats[name] = {
                "ticks": [], "respect": [],
                "pride": s["personality"]["pride"],
            }
        cats[name]["ticks"].append(s["tick"])
        cats[name]["respect"].append(s["needs"]["respect"])

    fig, ax = plt.subplots(figsize=(14, 7))
    cmap = plt.cm.RdYlGn_r  # high pride = warm (red), low = cool (green)

    slopes = {}
    for name, data in sorted(cats.items()):
        xs = np.array(data["ticks"])
        ys = np.array(data["respect"])
        pride = data["pride"]
        color = cmap(pride)

        ax.plot(xs, ys, alpha=0.4, color=color, linewidth=1)

        slope, intercept = fit_line(xs, ys)
        slopes[name] = slope
        fit_ys = slope * xs + intercept
        ax.plot(xs, fit_ys, "--", color=color, linewidth=2,
                label=f"{name} (pride={pride:.2f}, slope={slope:.6f})")

    ax.set_xlabel("Tick")
    ax.set_ylabel("Respect")
    ax.set_title("Per-Cat Respect Trajectories with Trend Lines\n(color: red=high pride, green=low pride)")
    ax.set_ylim(-0.05, 1.05)
    ax.axhline(y=0.3, color="orange", linestyle=":", alpha=0.7, label="Wounded pride threshold (0.3)")
    ax.axhline(y=0.4, color="red", linestyle=":", alpha=0.5, label="Pride amplifier threshold (0.4)")
    ax.legend(fontsize=7, loc="upper right", ncol=2)
    ax.grid(True, alpha=0.3)
    fig.tight_layout()
    fig.savefig(os.path.join(out_dir, "respect_trajectories.png"), dpi=150)
    plt.close(fig)
    return slopes


# ---------------------------------------------------------------------------
# Chart 2: Per-cat mood trajectories + colony mean
# ---------------------------------------------------------------------------

def plot_mood(snapshots: list[dict], out_dir: str):
    cats: dict[str, dict] = {}
    for s in snapshots:
        name = s["cat"]
        if name not in cats:
            cats[name] = {"ticks": [], "mood": []}
        cats[name]["ticks"].append(s["tick"])
        cats[name]["mood"].append(s["mood_valence"])

    fig, ax = plt.subplots(figsize=(14, 7))
    colors = plt.cm.tab10.colors

    slopes = {}
    for i, (name, data) in enumerate(sorted(cats.items())):
        xs = np.array(data["ticks"])
        ys = np.array(data["mood"])
        color = colors[i % len(colors)]

        ax.plot(xs, ys, alpha=0.3, color=color, linewidth=1)
        slope, intercept = fit_line(xs, ys)
        slopes[name] = slope
        fit_ys = slope * xs + intercept
        ax.plot(xs, fit_ys, "--", color=color, linewidth=2,
                label=f"{name} (slope={slope:.6f})")

    # Colony mean
    tick_mood: dict[int, list[float]] = defaultdict(list)
    for s in snapshots:
        tick_mood[s["tick"]].append(s["mood_valence"])
    mean_ticks = sorted(tick_mood.keys())
    mean_vals = [np.mean(tick_mood[t]) for t in mean_ticks]
    ax.plot(mean_ticks, mean_vals, "k-", linewidth=2, alpha=0.6, label="Colony mean")
    if len(mean_ticks) >= 2:
        ms, mi = fit_line(mean_ticks, mean_vals)
        ax.plot(mean_ticks, ms * np.array(mean_ticks) + mi, "k--", linewidth=2, label=f"Mean trend ({ms:.6f})")

    ax.set_xlabel("Tick")
    ax.set_ylabel("Mood Valence")
    ax.set_title("Per-Cat Mood Valence Trajectories")
    ax.set_ylim(-1.05, 1.05)
    ax.axhline(y=0, color="gray", linestyle=":", alpha=0.5)
    ax.legend(fontsize=7, loc="upper right", ncol=2)
    ax.grid(True, alpha=0.3)
    fig.tight_layout()
    fig.savefig(os.path.join(out_dir, "mood_trajectories.png"), dpi=150)
    plt.close(fig)
    return slopes


# ---------------------------------------------------------------------------
# Chart 3: Food economy + prey populations
# ---------------------------------------------------------------------------

def plot_food_economy(food_events: list[dict], pop_events: list[dict], out_dir: str):
    fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(14, 10), sharex=True)

    # Food levels
    if food_events:
        ticks = [e["tick"] for e in food_events]
        fracs = [e["fraction"] for e in food_events]
        ax1.plot(ticks, fracs, "b-", linewidth=1.5, label="Food fraction")
        if len(ticks) >= 2:
            slope, intercept = fit_line(ticks, fracs)
            ax1.plot(ticks, slope * np.array(ticks) + intercept, "b--",
                     linewidth=2, label=f"Trend ({slope:.8f})")
        ax1.set_ylabel("Food Fraction (current/capacity)")
        ax1.set_title("Food Economy")
        ax1.set_ylim(-0.05, 1.05)
        ax1.legend(fontsize=8)
        ax1.grid(True, alpha=0.3)

    # Prey populations
    if pop_events:
        ticks = [e["tick"] for e in pop_events]
        for species in ["mice", "rats", "rabbits", "fish", "birds"]:
            counts = [e.get(species, 0) for e in pop_events]
            ax2.plot(ticks, counts, linewidth=1.5, label=species.capitalize())
        total = [e.get("mice", 0) + e.get("rats", 0) + e.get("rabbits", 0) + e.get("fish", 0) + e.get("birds", 0) for e in pop_events]
        ax2.plot(ticks, total, "k-", linewidth=2, alpha=0.5, label="Total")
        ax2.set_xlabel("Tick")
        ax2.set_ylabel("Count")
        ax2.set_title("Prey Populations")
        ax2.legend(fontsize=8)
        ax2.grid(True, alpha=0.3)

    fig.tight_layout()
    fig.savefig(os.path.join(out_dir, "food_economy.png"), dpi=150)
    plt.close(fig)


# ---------------------------------------------------------------------------
# Chart 4: Action distribution over time
# ---------------------------------------------------------------------------

def plot_action_distribution(action_events: list[dict], out_dir: str):
    if not action_events:
        return

    # Bin actions into time windows
    min_tick = min(e["tick"] for e in action_events)
    max_tick = max(e["tick"] for e in action_events)
    n_bins = min(50, max(10, (max_tick - min_tick) // 200))
    bin_edges = np.linspace(min_tick, max_tick + 1, n_bins + 1)

    all_actions = sorted(set(e["action"] for e in action_events))
    bins: dict[str, list[int]] = {a: [0] * n_bins for a in all_actions}

    for e in action_events:
        idx = int(np.searchsorted(bin_edges[1:], e["tick"]))
        idx = min(idx, n_bins - 1)
        bins[e["action"]][idx] += 1

    fig, ax = plt.subplots(figsize=(14, 7))
    x = (bin_edges[:-1] + bin_edges[1:]) / 2

    # Stacked area
    bottom = np.zeros(n_bins)
    colors = plt.cm.tab20.colors
    for i, action in enumerate(all_actions):
        vals = np.array(bins[action], dtype=float)
        ax.fill_between(x, bottom, bottom + vals, alpha=0.7,
                        color=colors[i % len(colors)], label=action)
        bottom += vals

    ax.set_xlabel("Tick")
    ax.set_ylabel("Action Count (per bin)")
    ax.set_title("Action Distribution Over Time")
    ax.legend(fontsize=7, loc="upper right", ncol=3)
    ax.grid(True, alpha=0.3)
    fig.tight_layout()
    fig.savefig(os.path.join(out_dir, "action_distribution.png"), dpi=150)
    plt.close(fig)


# ---------------------------------------------------------------------------
# Chart 5: Needs heatmap (colony average over time)
# ---------------------------------------------------------------------------

def plot_needs_heatmap(snapshots: list[dict], out_dir: str):
    need_names = ["hunger", "energy", "warmth", "safety", "social",
                  "acceptance", "respect", "mastery", "purpose"]
    tick_needs: dict[int, dict[str, list[float]]] = defaultdict(lambda: defaultdict(list))

    for s in snapshots:
        tick = s["tick"]
        for need in need_names:
            tick_needs[tick][need].append(s["needs"][need])

    ticks = sorted(tick_needs.keys())
    if not ticks:
        return

    matrix = np.zeros((len(need_names), len(ticks)))
    for j, tick in enumerate(ticks):
        for i, need in enumerate(need_names):
            vals = tick_needs[tick][need]
            matrix[i, j] = np.mean(vals) if vals else 0.0

    fig, ax = plt.subplots(figsize=(14, 5))
    im = ax.imshow(matrix, aspect="auto", cmap="RdYlGn", vmin=0, vmax=1,
                   interpolation="nearest")

    ax.set_yticks(range(len(need_names)))
    ax.set_yticklabels(need_names)

    # Thin out x-axis labels
    n_labels = min(20, len(ticks))
    label_indices = np.linspace(0, len(ticks) - 1, n_labels, dtype=int)
    ax.set_xticks(label_indices)
    ax.set_xticklabels([str(ticks[i]) for i in label_indices], rotation=45, fontsize=7)

    ax.set_xlabel("Tick")
    ax.set_title("Colony-Average Needs Over Time (green=satisfied, red=depleted)")
    fig.colorbar(im, ax=ax, shrink=0.6)
    fig.tight_layout()
    fig.savefig(os.path.join(out_dir, "needs_heatmap.png"), dpi=150)
    plt.close(fig)


# ---------------------------------------------------------------------------
# Summary text
# ---------------------------------------------------------------------------

def write_summary(snapshots, action_events, food_events, pop_events,
                  death_events, activation_events, header,
                  respect_slopes, mood_slopes, out_dir):
    lines = []
    lines.append("=" * 70)
    lines.append("CLOWDER BALANCE REPORT")
    lines.append("=" * 70)

    # Tick range
    if snapshots:
        ticks = [s["tick"] for s in snapshots]
        lines.append(f"\nTick range: {min(ticks)} - {max(ticks)}")
        lines.append(f"Snapshot count: {len(snapshots)}")

    # Cat count
    cat_names = sorted(set(s["cat"] for s in snapshots))
    lines.append(f"Cats: {len(cat_names)} ({', '.join(cat_names)})")

    # Deaths
    if death_events:
        lines.append(f"\nDEATHS ({len(death_events)}):")
        for d in death_events:
            lines.append(f"  tick {d['tick']}: {d['cat']} - {d.get('cause', 'unknown')}")
    else:
        lines.append("\nNo deaths.")

    # Respect analysis
    lines.append("\n" + "-" * 50)
    lines.append("RESPECT TRAJECTORIES")
    lines.append("-" * 50)
    for name in cat_names:
        slope = respect_slopes.get(name, 0)
        # Get final respect value
        cat_snaps = [s for s in snapshots if s["cat"] == name]
        if cat_snaps:
            final = cat_snaps[-1]["needs"]["respect"]
            initial = cat_snaps[0]["needs"]["respect"]
            pride = cat_snaps[0]["personality"]["pride"]
            flag = " ** SPIRAL **" if slope < -0.0000005 else ""
            lines.append(
                f"  {name:10s}  pride={pride:.2f}  "
                f"respect: {initial:.4f} -> {final:.4f}  "
                f"slope={slope:+.8f}{flag}"
            )

    # Mood analysis
    lines.append("\n" + "-" * 50)
    lines.append("MOOD TRAJECTORIES")
    lines.append("-" * 50)
    for name in cat_names:
        slope = mood_slopes.get(name, 0)
        cat_snaps = [s for s in snapshots if s["cat"] == name]
        if cat_snaps:
            final = cat_snaps[-1]["mood_valence"]
            initial = cat_snaps[0]["mood_valence"]
            lines.append(
                f"  {name:10s}  mood: {initial:+.4f} -> {final:+.4f}  "
                f"slope={slope:+.8f}"
            )

    # Food economy
    lines.append("\n" + "-" * 50)
    lines.append("FOOD ECONOMY")
    lines.append("-" * 50)
    if food_events:
        fracs = [e["fraction"] for e in food_events]
        lines.append(f"  Food fraction: min={min(fracs):.3f}  max={max(fracs):.3f}  "
                     f"final={fracs[-1]:.3f}  mean={np.mean(fracs):.3f}")
        ticks = [e["tick"] for e in food_events]
        slope, _ = fit_line(ticks, fracs)
        lines.append(f"  Food trend slope: {slope:+.8f}")

    # Prey populations
    if pop_events:
        final_pop = pop_events[-1]
        total = final_pop.get("mice", 0) + final_pop.get("rats", 0) + final_pop.get("rabbits", 0) + final_pop.get("fish", 0) + final_pop.get("birds", 0)
        lines.append(f"  Final prey: mice={final_pop.get('mice', 0)} rats={final_pop.get('rats', 0)} "
                     f"rabbits={final_pop.get('rabbits', 0)} fish={final_pop.get('fish', 0)} "
                     f"birds={final_pop.get('birds', 0)} total={total}")

    # Action distribution
    lines.append("\n" + "-" * 50)
    lines.append("ACTION DISTRIBUTION")
    lines.append("-" * 50)
    if action_events:
        action_counts: dict[str, int] = defaultdict(int)
        for e in action_events:
            action_counts[e["action"]] += 1
        total_actions = sum(action_counts.values())
        for action, count in sorted(action_counts.items(), key=lambda x: -x[1]):
            pct = 100 * count / total_actions
            lines.append(f"  {action:20s}  {count:6d}  ({pct:5.1f}%)")

    # Final needs snapshot (colony averages)
    lines.append("\n" + "-" * 50)
    lines.append("FINAL COLONY-AVERAGE NEEDS")
    lines.append("-" * 50)
    last_tick = max(s["tick"] for s in snapshots) if snapshots else 0
    final_snaps = [s for s in snapshots if s["tick"] == last_tick]
    need_names = ["hunger", "energy", "warmth", "safety", "social",
                  "acceptance", "respect", "mastery", "purpose"]
    if final_snaps:
        for need in need_names:
            vals = [s["needs"][need] for s in final_snaps]
            avg = np.mean(vals)
            mn = min(vals)
            mx = max(vals)
            bar = "#" * int(avg * 40) + "." * (40 - int(avg * 40))
            lines.append(f"  {need:12s}  [{bar}]  avg={avg:.3f}  min={mn:.3f}  max={mx:.3f}")

    # Constants hash
    if header and "constants" in header:
        import hashlib
        constants_json = json.dumps(header["constants"], sort_keys=True)
        constants_hash = hashlib.sha256(constants_json.encode()).hexdigest()[:16]
        lines.append("\n" + "-" * 50)
        lines.append("CONSTANTS")
        lines.append("-" * 50)
        lines.append(f"  Hash: {constants_hash}")
        lines.append(f"  Seed: {header.get('seed', '?')}")

    # System activation — split by feature valence.
    #
    # The old "features active: 29/72" ratio was misleading because deaths
    # and corruption counted toward it. Since schema v2 we emit three groups:
    # positive (colony-thriving wins), negative (adverse events), neutral
    # (ecology churn). Render each separately so readers see what's healthy
    # vs what's going wrong at a glance.
    if activation_events:
        lines.append("\n" + "-" * 50)
        lines.append("SYSTEM ACTIVATION")
        lines.append("-" * 50)
        last = activation_events[-1]

        positive = last.get("positive", {})
        negative = last.get("negative", {})
        neutral = last.get("neutral", {})

        def render_section(title: str, counts: dict, show_dead: bool):
            if not counts:
                return
            active = sum(1 for v in counts.values() if v > 0)
            total = len(counts)
            lines.append(f"\n  {title} ({active}/{total} firing)")
            for name, count in sorted(counts.items(), key=lambda x: -x[1]):
                marker = "" if count > 0 else " ** DEAD **"
                lines.append(f"    {name:35s}  {count:8d}{marker}")
            if show_dead:
                dead = [name for name, count in counts.items() if count == 0]
                if dead:
                    lines.append(f"    DEAD IN {title.upper()} ({len(dead)}):")
                    for name in sorted(dead):
                        lines.append(f"      - {name}")

        render_section("Positive (healthy signals)", positive, show_dead=True)
        render_section("Negative (adverse events)", negative, show_dead=False)
        render_section("Neutral (system activity)", neutral, show_dead=True)

    lines.append("\n" + "=" * 70)

    summary = "\n".join(lines)
    summary_path = os.path.join(out_dir, "summary.txt")
    with open(summary_path, "w") as f:
        f.write(summary)
    return summary


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Clowder balance report")
    parser.add_argument("--events", default="logs/events.jsonl",
                        help="Path to events.jsonl")
    parser.add_argument("--out", default="logs/charts",
                        help="Output directory for charts")
    args = parser.parse_args()

    if not os.path.exists(args.events):
        print(f"Error: {args.events} not found. Run a headless simulation first.", file=sys.stderr)
        sys.exit(1)

    os.makedirs(args.out, exist_ok=True)

    print(f"Parsing {args.events}...")
    header, events = parse_events(args.events)
    buckets = bucket_events(events)

    snapshots = buckets.get("CatSnapshot", [])
    food_events = buckets.get("FoodLevel", [])
    pop_events = buckets.get("PopulationSnapshot", [])
    action_events = buckets.get("ActionChosen", [])
    death_events = buckets.get("Death", [])
    activation_events = buckets.get("SystemActivation", [])

    print(f"  {len(snapshots)} snapshots, {len(food_events)} food events, "
          f"{len(action_events)} action events, {len(death_events)} deaths, "
          f"{len(activation_events)} activation snapshots")

    print("Generating charts...")
    respect_slopes = plot_respect(snapshots, args.out)
    mood_slopes = plot_mood(snapshots, args.out)
    plot_food_economy(food_events, pop_events, args.out)
    plot_action_distribution(action_events, args.out)
    plot_needs_heatmap(snapshots, args.out)

    print("Writing summary...")
    summary = write_summary(
        snapshots, action_events, food_events, pop_events,
        death_events, activation_events, header,
        respect_slopes, mood_slopes, args.out,
    )
    print(summary)
    print(f"\nCharts saved to {args.out}/")


if __name__ == "__main__":
    main()
