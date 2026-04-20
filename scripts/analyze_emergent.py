#!/usr/bin/env python3
"""Analyze a 30-minute headless simulation for emergent behavior patterns.

Reads logs/events_30m.jsonl and logs/narrative_30m.jsonl and produces
a structured report covering:
  - Colony survival arc (population over time, deaths)
  - Food economy (stores, hunt success, Lotka-Volterra oscillations)
  - Social dynamics (relationships, bonds, coordinator elections)
  - Cat personality divergence (who thrives, who struggles, and why)
  - System activation (which features fired, which stayed dormant)
  - Emergent chain reactions (cross-system interactions)
"""

import json
import sys
from collections import defaultdict
from pathlib import Path

EVENT_LOG = Path("logs/events_30m_v3.jsonl")
NARRATIVE_LOG = Path("logs/narrative_30m_v3.jsonl")


def load_jsonl(path: Path) -> list[dict]:
    entries = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            obj = json.loads(line)
            if obj.get("_header"):
                continue
            entries.append(obj)
    return entries


def analyze_events(events: list[dict]) -> dict:
    """Extract structured data from the event log."""
    result = {
        "colony_scores": [],
        "food_levels": [],
        "population_snapshots": [],
        "cat_snapshots": defaultdict(list),
        "deaths": [],
        "coordinator_elections": [],
        "directives": [],
        "system_activations": [],
        "action_choices": [],
        "position_traces": defaultdict(list),
    }

    for e in events:
        t = e.get("type")
        tick = e.get("tick", 0)

        if t == "ColonyScore":
            result["colony_scores"].append(e)
        elif t == "FoodLevel":
            result["food_levels"].append(e)
        elif t == "PopulationSnapshot":
            result["population_snapshots"].append(e)
        elif t == "CatSnapshot":
            result["cat_snapshots"][e["cat"]].append(e)
        elif t == "Death":
            result["deaths"].append(e)
        elif t == "CoordinatorElected":
            result["coordinator_elections"].append(e)
        elif t == "DirectiveIssued":
            result["directives"].append(e)
        elif t == "SystemActivation":
            result["system_activations"].append(e)
        elif t == "ActionChosen":
            result["action_choices"].append(e)
        elif t == "PositionTrace":
            result["position_traces"][e["cat"]].append(e)

    return result


def print_header(title: str):
    print(f"\n{'=' * 60}")
    print(f"  {title}")
    print(f"{'=' * 60}")


def print_subheader(title: str):
    print(f"\n--- {title} ---")


def analyze_survival_arc(data: dict):
    print_header("COLONY SURVIVAL ARC")

    scores = data["colony_scores"]
    if not scores:
        print("  No colony score data found.")
        return

    first = scores[0]
    last = scores[-1]

    print(
        f"  Duration: tick {first['tick']} → {last['tick']} ({len(scores)} snapshots)"
    )
    print(
        f"  Living cats: {first.get('living_cats', '?')} → {last.get('living_cats', '?')}"
    )
    print(f"  Peak population: {last.get('peak_population', '?')}")
    print(f"  Seasons survived: {last.get('seasons_survived', '?')}")

    deaths = data["deaths"]
    if deaths:
        print(f"\n  Deaths ({len(deaths)}):")
        for d in deaths:
            day = d["tick"] // 1000 + 1
            print(f"    Day {day}: {d['cat']} — {d['cause']}")
    else:
        print(
            f"\n  Deaths: {last.get('deaths_starvation', 0)} starvation, "
            f"{last.get('deaths_old_age', 0)} old age, "
            f"{last.get('deaths_injury', 0)} injury (from final score)"
        )

    # Welfare trend
    print_subheader("Welfare Over Time")
    # Sample every 10th score for readability
    step = max(1, len(scores) // 15)
    print(
        f"  {'Day':>5}  {'Welfare':>8}  {'Nourish':>8}  {'Happy':>8}  {'Health':>8}  {'Cats':>5}"
    )
    for s in scores[::step]:
        day = s["tick"] // 1000 + 1
        print(
            f"  {day:>5}  {s.get('welfare', 0):>8.3f}  "
            f"{s.get('nourishment', 0):>8.3f}  "
            f"{s.get('happiness', 0):>8.3f}  "
            f"{s.get('health', 0):>8.3f}  "
            f"{s.get('living_cats', '?'):>5}"
        )


def analyze_food_economy(data: dict):
    print_header("FOOD ECONOMY")

    foods = data["food_levels"]
    if not foods:
        print("  No food level data.")
        return

    fractions = [f["fraction"] for f in foods]
    currents = [f["current"] for f in foods]
    positive = sum(1 for f in fractions if f > 0)

    print(f"  Snapshots: {len(foods)}")
    print(
        f"  Food positive: {positive}/{len(foods)} ({100 * positive / len(foods):.1f}%)"
    )
    print(f"  Avg food level: {sum(currents) / len(currents):.2f}")
    print(f"  Peak food: {max(currents):.2f}")
    print(f"  Min food: {min(currents):.2f}")

    # Food over time
    print_subheader("Food Stores Over Time")
    step = max(1, len(foods) // 15)
    print(f"  {'Day':>5}  {'Current':>8}  {'Fraction':>8}")
    for f in foods[::step]:
        day = f["tick"] // 1000 + 1
        print(f"  {day:>5}  {f['current']:>8.2f}  {f['fraction']:>8.3f}")


def analyze_prey_populations(data: dict):
    print_header("PREY POPULATION DYNAMICS")

    pops = data["population_snapshots"]
    if not pops:
        print("  No population data.")
        return

    species = ["mice", "rats", "rabbits", "fish", "birds"]

    print(f"  {'Day':>5}", end="")
    for s in species:
        print(f"  {s:>7}", end="")
    print(f"  {'Total':>7}")

    step = max(1, len(pops) // 20)
    for p in pops[::step]:
        day = p["tick"] // 1000 + 1
        total = sum(p.get(s, 0) for s in species)
        print(f"  {day:>5}", end="")
        for s in species:
            print(f"  {p.get(s, 0):>7}", end="")
        print(f"  {total:>7}")

    # Check for Lotka-Volterra oscillations
    print_subheader("Population Extremes")
    for s in species:
        vals = [p.get(s, 0) for p in pops]
        if vals:
            print(
                f"  {s:>8}: min={min(vals):>3}, max={max(vals):>3}, "
                f"range={max(vals) - min(vals):>3}, "
                f"final={vals[-1]:>3}"
            )


def analyze_cat_personalities(data: dict):
    print_header("CAT PERSONALITY & BEHAVIOR PROFILES")

    snapshots = data["cat_snapshots"]
    if not snapshots:
        print("  No cat snapshot data.")
        return

    for cat, snaps in sorted(snapshots.items()):
        print_subheader(cat)
        first = snaps[0]
        last = snaps[-1]

        # Personality summary
        p = last.get("personality", {})
        if p:
            drives = {
                k: v
                for k, v in p.items()
                if k
                in [
                    "boldness",
                    "sociability",
                    "curiosity",
                    "playfulness",
                    "temper",
                    "patience",
                    "diligence",
                    "independence",
                ]
            }
            values = {
                k: v
                for k, v in p.items()
                if k
                in ["loyalty", "compassion", "ambition", "spirituality", "tradition"]
            }

            strong_drives = [(k, v) for k, v in drives.items() if abs(v - 0.5) > 0.2]
            if strong_drives:
                traits = ", ".join(
                    f"{k}={v:.2f}"
                    for k, v in sorted(strong_drives, key=lambda x: -abs(x[1] - 0.5))
                )
                print(f"  Defining traits: {traits}")

            strong_values = [(k, v) for k, v in values.items() if abs(v - 0.5) > 0.2]
            if strong_values:
                vals = ", ".join(
                    f"{k}={v:.2f}"
                    for k, v in sorted(strong_values, key=lambda x: -abs(x[1] - 0.5))
                )
                print(f"  Values: {vals}")

        # Needs trajectory
        first_needs = first.get("needs", {})
        last_needs = last.get("needs", {})
        if first_needs and last_needs:
            print(f"  Needs trajectory:")
            for need in ["hunger", "energy", "safety", "social", "purpose"]:
                fn = first_needs.get(need, 0)
                ln = last_needs.get(need, 0)
                delta = ln - fn
                arrow = "↑" if delta > 0.05 else "↓" if delta < -0.05 else "→"
                print(f"    {need:>10}: {fn:.2f} → {ln:.2f} {arrow}")

        # Skills growth
        first_skills = first.get("skills", {})
        last_skills = last.get("skills", {})
        if first_skills and last_skills:
            growing = []
            for skill, val in last_skills.items():
                start = first_skills.get(skill, 0)
                if val > start + 0.01:
                    growing.append((skill, start, val))
            if growing:
                print(f"  Skill growth:")
                for skill, start, end in sorted(growing, key=lambda x: -(x[2] - x[1])):
                    print(
                        f"    {skill:>15}: {start:.3f} → {end:.3f} (+{end - start:.3f})"
                    )

        # Action frequency from last_scores
        actions = defaultdict(int)
        for snap in snaps:
            act = snap.get("current_action")
            if act:
                actions[act] += 1
        if actions:
            total = sum(actions.values())
            top = sorted(actions.items(), key=lambda x: -x[1])[:5]
            print(f"  Action distribution ({total} snapshots):")
            for act, count in top:
                print(f"    {act:>20}: {count:>3} ({100 * count / total:.0f}%)")

        # Relationships
        last_rels = last.get("relationships", [])
        if last_rels:
            close = [
                r
                for r in last_rels
                if r.get("fondness", 0) > 0.6 or r.get("bond") is not None
            ]
            if close:
                print(f"  Close relationships:")
                for r in close:
                    bond = f" [{r['bond']}]" if r.get("bond") else ""
                    print(
                        f"    → {r['cat']}: fondness={r['fondness']:.2f}, "
                        f"familiarity={r['familiarity']:.2f}{bond}"
                    )

        # Mood arc
        moods = [s.get("mood_valence", 0) for s in snaps]
        if moods:
            print(
                f"  Mood: avg={sum(moods) / len(moods):.3f}, "
                f"min={min(moods):.3f}, max={max(moods):.3f}"
            )

        # Health
        healths = [s.get("health", 1.0) for s in snaps]
        if healths and min(healths) < 0.95:
            print(f"  Health: min={min(healths):.3f}, final={healths[-1]:.3f}")


def analyze_coordination(data: dict):
    print_header("COORDINATION & SOCIAL STRUCTURE")

    elections = data["coordinator_elections"]
    if elections:
        print(f"  Coordinator elections: {len(elections)}")
        for e in elections:
            day = e["tick"] // 1000 + 1
            print(
                f"    Day {day}: {e['cat']} (social_weight={e.get('social_weight', 0):.3f})"
            )

    directives = data["directives"]
    if directives:
        kinds = defaultdict(int)
        by_coordinator = defaultdict(lambda: defaultdict(int))
        for d in directives:
            kinds[d.get("kind", "?")] += 1
            by_coordinator[d.get("coordinator", "?")][d.get("kind", "?")] += 1

        print(f"\n  Directive breakdown ({len(directives)} total):")
        for k, v in sorted(kinds.items(), key=lambda x: -x[1]):
            print(f"    {k:>25}: {v:>5}")

        print(f"\n  By coordinator:")
        for coord, dk in sorted(by_coordinator.items()):
            total = sum(dk.values())
            top = sorted(dk.items(), key=lambda x: -x[1])[:3]
            top_str = ", ".join(f"{k}={v}" for k, v in top)
            print(f"    {coord}: {total} directives ({top_str})")


def analyze_system_activation(data: dict):
    print_header("SYSTEM ACTIVATION")

    activations = data["system_activations"]
    if not activations:
        print("  No system activation data.")
        return

    # Schema v2: activations are split into positive/negative/neutral groups.
    # Merge each bucket across the run so we see lifetime totals per feature.
    merged = {"positive": defaultdict(int), "negative": defaultdict(int), "neutral": defaultdict(int)}
    for a in activations:
        for bucket in ("positive", "negative", "neutral"):
            for system, count in a.get(bucket, {}).items():
                merged[bucket][system] += count

    def print_group(title: str, counts: dict):
        if not counts:
            return
        total_firings = sum(counts.values())
        print(f"\n  {title} ({len(counts)} features, {total_firings} total firings):")
        for system, count in sorted(counts.items(), key=lambda x: -x[1]):
            marker = "" if count > 0 else "  ** DEAD **"
            print(f"    {system:>30}: {count:>6}{marker}")

    print_group("Positive (healthy signals)", merged["positive"])
    print_group("Negative (adverse events)", merged["negative"])
    print_group("Neutral (system activity)", merged["neutral"])

    # The "did a system go dead?" canary — positive dormancy is the real concern.
    scores = data["colony_scores"]
    if scores:
        last = scores[-1]
        pos_active = last.get("positive_features_active", 0)
        pos_total = last.get("positive_features_total", 0)
        neg_events = last.get("negative_events_total", 0)
        neu_active = last.get("neutral_features_active", 0)
        neu_total = last.get("neutral_features_total", 0)
        if pos_total > 0:
            print(f"\n  Positive activation: {pos_active}/{pos_total} ({100 * pos_active / pos_total:.0f}%)")
            dormant = pos_total - pos_active
            if dormant > 0:
                print(
                    f"  {dormant} positive features never fired — potential dead systems"
                )
        print(f"  Negative events: {neg_events} total")
        if neu_total > 0:
            print(f"  Neutral activity: {neu_active}/{neu_total} features firing")


def analyze_narrative(narratives: list[dict]):
    print_header("NARRATIVE HIGHLIGHTS")

    tiers = defaultdict(list)
    for n in narratives:
        tiers[n.get("tier", "?")].append(n)

    print(f"  Total entries: {len(narratives)}")
    for tier in ["Significant", "Action", "Micro"]:
        entries = tiers.get(tier, [])
        print(f"  {tier}: {len(entries)}")

    # Significant events are the most interesting
    significant = tiers.get("Significant", [])
    if significant:
        print_subheader("Significant Events")
        for n in significant[:30]:  # First 30
            day = n.get("day", "?")
            phase = n.get("phase", "?")
            print(f"  Day {day} ({phase}): {n['text']}")
        if len(significant) > 30:
            print(f"  ... and {len(significant) - 30} more")

    # Action tier narrative patterns
    action = tiers.get("Action", [])
    if action:
        print_subheader("Action Narrative Patterns")
        # Find most common action themes
        themes = defaultdict(int)
        for n in action:
            text = n.get("text", "")
            # Simple keyword extraction
            for keyword in [
                "hunt",
                "forage",
                "sleep",
                "socialize",
                "groom",
                "patrol",
                "build",
                "eat",
                "fight",
                "flee",
                "explore",
                "mate",
                "catch",
                "miss",
                "scent",
                "raid",
                "den",
                "prey",
                "starv",
                "injur",
            ]:
                if keyword in text.lower():
                    themes[keyword] += 1
        if themes:
            for theme, count in sorted(themes.items(), key=lambda x: -x[1])[:15]:
                print(f"    {theme:>12}: {count:>5} mentions")

    # Look for emergent chain reactions in narrative
    micro = tiers.get("Micro", [])
    if micro:
        print_subheader("Micro Behavior Patterns")
        # Count unique micro patterns
        patterns = defaultdict(int)
        for n in micro:
            text = n.get("text", "")
            # Normalize to find patterns
            patterns[text] += 1
        top = sorted(patterns.items(), key=lambda x: -x[1])[:10]
        for text, count in top:
            print(f"    [{count:>4}x] {text[:80]}")


def find_emergent_behaviors(data: dict, narratives: list[dict]):
    print_header("EMERGENT BEHAVIOR ANALYSIS")

    # 1. Lotka-Volterra oscillations
    print_subheader("Predator-Prey Oscillations")
    pops = data["population_snapshots"]
    foods = data["food_levels"]
    if pops and foods:
        # Check if prey populations oscillate with food levels
        total_prey = [
            sum(p.get(s, 0) for s in ["mice", "rats", "rabbits", "fish", "birds"])
            for p in pops
        ]
        food_vals = [f["current"] for f in foods]

        if len(total_prey) > 5:
            # Check for boom-bust cycles
            diffs = [
                total_prey[i + 1] - total_prey[i] for i in range(len(total_prey) - 1)
            ]
            sign_changes = sum(
                1 for i in range(len(diffs) - 1) if diffs[i] * diffs[i + 1] < 0
            )
            print(
                f"  Prey population sign changes: {sign_changes} (more = more oscillation)"
            )
            print(f"  Prey range: {min(total_prey)} → {max(total_prey)}")
            if sign_changes > 5:
                print(
                    "  ✓ DETECTED: Lotka-Volterra-style boom-bust cycles in prey populations"
                )
            elif max(total_prey) - min(total_prey) > 50:
                print(
                    "  ✓ DETECTED: Large prey population swings (possible predation pressure)"
                )
            else:
                print("  ✗ No significant oscillation detected")

    # 2. Social stratification
    print_subheader("Social Stratification")
    snapshots = data["cat_snapshots"]
    if snapshots:
        # Compare final needs across cats
        final_needs = {}
        for cat, snaps in snapshots.items():
            if snaps:
                final_needs[cat] = snaps[-1].get("needs", {})

        if final_needs:
            # Check hunger disparity
            hungers = {cat: n.get("hunger", 0.5) for cat, n in final_needs.items()}
            if hungers:
                best = max(hungers, key=hungers.get)
                worst = min(hungers, key=hungers.get)
                gap = hungers[best] - hungers[worst]
                print(
                    f"  Hunger disparity: {gap:.3f} "
                    f"(best: {best}={hungers[best]:.3f}, worst: {worst}={hungers[worst]:.3f})"
                )
                if gap > 0.3:
                    print("  ✓ DETECTED: Significant resource inequality among cats")

            # Check social need disparity
            socials = {cat: n.get("social", 0.5) for cat, n in final_needs.items()}
            if socials:
                best = max(socials, key=socials.get)
                worst = min(socials, key=socials.get)
                gap = socials[best] - socials[worst]
                if gap > 0.2:
                    print(
                        f"  ✓ DETECTED: Social isolation gradient "
                        f"({worst} at {socials[worst]:.3f} vs {best} at {socials[best]:.3f})"
                    )

    # 3. Personality-outcome correlation
    print_subheader("Personality-Outcome Correlations")
    if snapshots:
        personality_outcomes = []
        for cat, snaps in snapshots.items():
            if not snaps:
                continue
            last = snaps[-1]
            p = last.get("personality", {})
            n = last.get("needs", {})
            s = last.get("skills", {})
            if p and n:
                personality_outcomes.append(
                    {
                        "cat": cat,
                        "boldness": p.get("boldness", 0.5),
                        "sociability": p.get("sociability", 0.5),
                        "diligence": p.get("diligence", 0.5),
                        "curiosity": p.get("curiosity", 0.5),
                        "hunger": n.get("hunger", 0.5),
                        "social": n.get("social", 0.5),
                        "safety": n.get("safety", 0.5),
                        "mood": last.get("mood_valence", 0),
                        "health": last.get("health", 1.0),
                        "hunting_skill": s.get("hunting", 0) if s else 0,
                    }
                )

        if personality_outcomes:
            # Find who's thriving vs struggling
            by_mood = sorted(personality_outcomes, key=lambda x: -x["mood"])
            if len(by_mood) >= 2:
                top = by_mood[0]
                bot = by_mood[-1]
                print(f"  Happiest: {top['cat']} (mood={top['mood']:.3f})")
                print(
                    f"    personality: bold={top['boldness']:.2f}, social={top['sociability']:.2f}, "
                    f"diligent={top['diligence']:.2f}"
                )
                print(f"  Unhappiest: {bot['cat']} (mood={bot['mood']:.3f})")
                print(
                    f"    personality: bold={bot['boldness']:.2f}, social={bot['sociability']:.2f}, "
                    f"diligent={bot['diligence']:.2f}"
                )

            # Bold cats hunt more?
            bold_cats = [c for c in personality_outcomes if c["boldness"] > 0.6]
            timid_cats = [c for c in personality_outcomes if c["boldness"] < 0.4]
            if bold_cats and timid_cats:
                bold_hunt = sum(c["hunting_skill"] for c in bold_cats) / len(bold_cats)
                timid_hunt = sum(c["hunting_skill"] for c in timid_cats) / len(
                    timid_cats
                )
                if abs(bold_hunt - timid_hunt) > 0.01:
                    print(
                        f"  Bold cats avg hunting: {bold_hunt:.3f} vs timid: {timid_hunt:.3f}"
                    )
                    if bold_hunt > timid_hunt:
                        print(
                            "  ✓ DETECTED: Bold personality correlates with better hunting skill"
                        )

    # 4. Coordinator leadership style
    print_subheader("Leadership Dynamics")
    directives = data["directives"]
    elections = data["coordinator_elections"]
    if elections:
        for e in elections:
            coord = e["cat"]
            coord_directives = [d for d in directives if d.get("coordinator") == coord]
            if coord_directives:
                kinds = defaultdict(int)
                for d in coord_directives:
                    kinds[d.get("kind", "?")] += 1
                top = sorted(kinds.items(), key=lambda x: -x[1])[:3]
                style = top[0][0] if top else "unknown"
                print(
                    f"  {coord}: {len(coord_directives)} directives, "
                    f"primary focus: {style}"
                )
                # Check if coordinator personality matches leadership style
                if coord in snapshots and snapshots[coord]:
                    p = snapshots[coord][-1].get("personality", {})
                    if p:
                        print(
                            f"    personality: ambition={p.get('ambition', 0):.2f}, "
                            f"diligence={p.get('diligence', 0):.2f}, "
                            f"compassion={p.get('compassion', 0):.2f}"
                        )

    # 5. Cross-system chains
    print_subheader("Cross-System Interactions")
    # Check for narrative evidence of chain reactions
    chain_keywords = {
        "corruption → mood": ["corrupt", "mood", "dark"],
        "starvation → social": ["starv", "lonely", "isolat"],
        "hunt → prey decline": ["deplet", "scarce", "overhu"],
        "raid → food loss": ["raid", "stores", "stolen"],
        "weather → warmth": ["storm", "cold", "shiver"],
        "grief → mood cascade": ["grief", "mourn", "loss"],
    }
    for chain_name, keywords in chain_keywords.items():
        matches = 0
        for n in narratives:
            text = n.get("text", "").lower()
            if any(k in text for k in keywords):
                matches += 1
        if matches > 0:
            print(f"  {chain_name}: {matches} narrative mentions")

    # 6. Seasonal patterns
    print_subheader("Seasonal Patterns")
    scores = data["colony_scores"]
    if scores and len(scores) > 10:
        # Group by rough season (every ~20 days at 1000 ticks/day, 20000 ticks/season)
        season_welfare = defaultdict(list)
        for s in scores:
            tick = s["tick"]
            # 20000 ticks per season, 4 seasons
            season_idx = (tick // 20000) % 4
            season_name = ["Spring", "Summer", "Autumn", "Winter"][season_idx]
            season_welfare[season_name].append(s.get("welfare", 0))

        for season in ["Spring", "Summer", "Autumn", "Winter"]:
            vals = season_welfare.get(season, [])
            if vals:
                avg = sum(vals) / len(vals)
                print(f"  {season:>8}: avg welfare={avg:.3f} ({len(vals)} samples)")


def main():
    if not EVENT_LOG.exists():
        print(f"Error: {EVENT_LOG} not found. Run the simulation first.")
        sys.exit(1)

    print("Loading event log...")
    events = load_jsonl(EVENT_LOG)
    print(f"  {len(events)} events loaded")

    narratives = []
    if NARRATIVE_LOG.exists():
        print("Loading narrative log...")
        narratives = load_jsonl(NARRATIVE_LOG)
        print(f"  {len(narratives)} narrative entries loaded")

    data = analyze_events(events)

    analyze_survival_arc(data)
    analyze_food_economy(data)
    analyze_prey_populations(data)
    analyze_cat_personalities(data)
    analyze_coordination(data)
    analyze_system_activation(data)
    analyze_narrative(narratives)
    find_emergent_behaviors(data, narratives)

    print_header("END OF ANALYSIS")


if __name__ == "__main__":
    main()
