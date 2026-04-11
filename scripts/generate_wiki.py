#!/usr/bin/env python3
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""
Wiki generator for the Clowder game.

Parses Rust source to produce markdown reference pages in docs/wiki/.
Cross-references registered systems against design docs to classify each
as Built, Partial, or Aspirational.

Usage:
    uv run scripts/generate_wiki.py [--src SRC] [--docs DOCS] [--out OUT]
"""

import argparse
import os
import re
from dataclasses import dataclass, field
from pathlib import Path

# ---------------------------------------------------------------------------
# Data model
# ---------------------------------------------------------------------------


@dataclass
class RustType:
    name: str
    kind: str  # "struct" | "enum"
    category: str  # "Component" | "Resource" | "Message"
    file: str
    doc_comment: str
    fields: list[tuple[str, str]] = field(default_factory=list)
    variants: list[str] = field(default_factory=list)


@dataclass
class RegisteredSystem:
    module: str
    function: str
    section: str  # "chain1" | "chain1.5" | "chain2" | "chain3" | "chain4" | "disposition" | "standalone"


@dataclass
class SystemDoc:
    stem: str
    title: str
    purpose: str
    status: str = ""


@dataclass
class PreySpecies:
    name: str
    stats: dict[str, str] = field(default_factory=dict)


@dataclass
class EnumInfo:
    name: str
    file: str
    variants: list[str] = field(default_factory=list)
    extra: dict[str, dict[str, str]] = field(default_factory=dict)  # variant -> {col -> val}


# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

# Maps design doc stem -> system module names used in SimulationPlugin::build().
# Names must match the `systems::MODULE::function` paths.
DOC_TO_MODULES: dict[str, list[str]] = {
    "needs": ["needs"],
    "mood": ["mood"],
    "weather": ["weather", "wind"],
    "time": ["time"],
    "buildings": ["buildings"],
    "items": ["items"],
    "combat": ["combat"],
    "coordination": ["coordination"],
    "narrative": ["narrative"],
    "magic": ["magic"],
    "utility-ai": ["ai", "disposition"],
    "relationships": ["social", "personality_friction"],
    "collective-memory": ["memory", "colony_knowledge"],
    "world-gen": ["_worldgen"],  # special: code exists outside systems/
    "identity": ["_identity"],  # special: components only, no system module
    "skills": ["_skills"],  # special: components exist, used in scoring
    "activity-cascading": ["personality_events"],
    "mental-breaks": [],
    "disease": [],
    "recreation": [],
    "reproduction": [],
    "substances": [],
    "trade": [],
    "the-calling": [],
    "raids": [],
    "body-zones": ["_bodyzone"],  # partial: InjuryKind exists
    "corpse-handling": ["death"],
    "environmental-quality": [],
}

# Known cross-chain data flows for the system map.
KNOWN_INTERACTIONS: list[tuple[str, str, str]] = [
    ("time::advance_time", "weather::update_weather", "TimeState"),
    ("items::sync_food_stores", "needs::eat_from_inventory", "FoodStores"),
    ("needs::decay_needs", "mood::update_mood", "Needs"),
    ("mood::update_mood", "disposition::evaluate_dispositions", "Mood"),
    ("disposition::evaluate_dispositions", "disposition::disposition_to_chain", "Disposition"),
    ("disposition::disposition_to_chain", "task_chains::resolve_task_chains", "TaskChain"),
    ("combat::resolve_combat", "death::check_death", "Health/Injury"),
    ("social::passive_familiarity", "social::check_bonds", "Relationships"),
    ("coordination::assess_colony_needs", "disposition::evaluate_dispositions", "ColonyPriority"),
    ("prey::prey_population", "wildlife::predator_hunt_prey", "prey entities"),
    ("death::check_death", "death::cleanup_dead", "Dead marker"),
    ("magic::corruption_spread", "magic::spawn_shadow_fox_from_corruption", "TileMap corruption"),
]

# High-value enums to extract fully.
TARGET_ENUMS: list[str] = [
    "ItemKind",
    "DispositionKind",
    "Weather",
    "Terrain",
    "StructureType",
    "HerbKind",
    "WardKind",
    "ZodiacSign",
    "BondType",
    "DeathCause",
    "InjuryKind",
    "LifeStage",
    "Season",
    "DayPhase",
    "PreyKind",
    "FleeStrategy",
    "WildSpecies",
    "EventKind",
    "NarrativeTier",
    "PriorityKind",
    "DirectiveKind",
    "StepKind",
    "ZoneKind",
    "AspirationDomain",
]

SECTION_LABELS: dict[str, str] = {
    "chain1": "Chain 1: World Simulation",
    "chain1.5": "Chain 1.5: Items & Den Management",
    "chain2": "Chain 2: Needs, Mood & Decision-Making",
    "chain3": "Chain 3: Action Resolution",
    "chain4": "Chain 4: Social, Combat, Death & Narrative",
    "disposition": "Disposition Pipeline",
    "standalone": "Standalone Systems",
}


# ---------------------------------------------------------------------------
# Parsers
# ---------------------------------------------------------------------------


def parse_rust_types(src: Path) -> list[RustType]:
    """Scan src/components/ and src/resources/ for derive(Component|Resource|Message) types."""
    results: list[RustType] = []
    dirs = [src / "components", src / "resources"]
    for d in dirs:
        if not d.exists():
            continue
        for rs in sorted(d.glob("*.rs")):
            if rs.name == "mod.rs":
                continue
            results.extend(_parse_types_in_file(rs, src))
    # Also check src/species/mod.rs for SpeciesRegistry (Resource)
    species_mod = src / "species" / "mod.rs"
    if species_mod.exists():
        results.extend(_parse_types_in_file(species_mod, src))
    return results


def _parse_types_in_file(path: Path, src_root: Path) -> list[RustType]:
    """Extract derived types from a single .rs file."""
    results: list[RustType] = []
    lines = path.read_text().splitlines()
    rel = str(path.relative_to(src_root.parent))
    in_test = False
    i = 0
    while i < len(lines):
        line = lines[i]

        # Skip #[cfg(test)] blocks
        if re.match(r'\s*#\[cfg\(test\)\]', line):
            in_test = True
            i += 1
            continue
        if in_test:
            i += 1
            continue

        # Look for derive attributes
        derive_match = re.search(r'#\[derive\(([^)]*)\)\]', line)
        if not derive_match:
            # Handle multi-line derives
            if '#[derive(' in line and ')' not in line:
                combined = line
                while i + 1 < len(lines) and ')' not in combined:
                    i += 1
                    combined += ' ' + lines[i].strip()
                derive_match = re.search(r'#\[derive\(([^)]*)\)\]', combined)
            if not derive_match:
                i += 1
                continue

        derives = derive_match.group(1)
        category = None
        for cat in ("Component", "Resource", "Message"):
            if cat in derives or f"bevy_ecs::prelude::{cat}" in derives:
                category = cat
                break
        if not category:
            i += 1
            continue

        # Collect doc comments above the derive
        doc_lines: list[str] = []
        j = i - 1
        # Walk backwards past other attributes to find doc comments
        while j >= 0 and lines[j].strip().startswith('#['):
            j -= 1
        while j >= 0 and lines[j].strip().startswith('///'):
            doc_lines.insert(0, lines[j].strip().removeprefix('///').strip())
            j -= 1

        # Find the next pub struct/enum line
        i += 1
        while i < len(lines):
            stripped = lines[i].strip()
            if stripped.startswith('#['):
                i += 1
                continue
            type_match = re.match(r'pub\s+(struct|enum)\s+(\w+)', stripped)
            if type_match:
                break
            i += 1
            continue
        else:
            continue

        kind = type_match.group(1)
        name = type_match.group(2)

        # Collect fields (struct) or variants (enum)
        fields: list[tuple[str, str]] = []
        variants: list[str] = []
        brace_depth = 0
        if '{' in lines[i]:
            brace_depth = 1
        i += 1
        while i < len(lines) and brace_depth > 0:
            ln = lines[i].strip()
            brace_depth += ln.count('{') - ln.count('}')
            if brace_depth <= 0:
                break
            if kind == "struct":
                field_m = re.match(r'pub\s+(\w+)\s*:\s*(.+?),?\s*$', ln)
                if field_m:
                    fields.append((field_m.group(1), field_m.group(2).rstrip(',')))
            elif kind == "enum":
                var_m = re.match(r'(\w+)\s*[,{(]?', ln)
                if var_m and not ln.startswith('//'):
                    variants.append(var_m.group(1))
            i += 1

        results.append(RustType(
            name=name,
            kind=kind,
            category=category,
            file=rel,
            doc_comment=' '.join(doc_lines),
            fields=fields,
            variants=variants,
        ))
        i += 1
    return results


def parse_registered_systems(simulation_rs: Path) -> list[RegisteredSystem]:
    """Extract systems::module::function paths from SimulationPlugin::build()."""
    text = simulation_rs.read_text()
    results: list[RegisteredSystem] = []
    seen: set[tuple[str, str]] = set()

    # Known run conditions and setup functions (not per-tick systems).
    run_conditions: set[str] = {"not_paused", "register_observers"}

    # Determine section boundaries from comments and structure
    lines = text.splitlines()
    section = "chain1"
    for line in lines:
        stripped = line.strip()
        # Track section changes via comments
        if "Item pruning" in stripped or "food sync" in stripped:
            section = "chain1.5"
        elif "Chain 2" in stripped:
            section = "chain2"
        elif "Chain 3" in stripped:
            section = "chain3"
        elif "Chain 4" in stripped:
            section = "chain4"
        elif "Disposition systems" in stripped:
            section = "disposition"
        elif "Standalone systems" in stripped:
            section = "standalone"

        m = re.search(r'systems::(\w+)::(\w+)', stripped)
        if m:
            module, function = m.group(1), m.group(2)
            key = (module, function)
            if function in run_conditions or key in seen:
                continue
            seen.add(key)
            results.append(RegisteredSystem(
                module=module,
                function=function,
                section=section,
            ))
    return results


def parse_system_docs(docs_dir: Path) -> list[SystemDoc]:
    """Read docs/systems/*.md for title and purpose."""
    results: list[SystemDoc] = []
    systems_dir = docs_dir / "systems"
    if not systems_dir.exists():
        return results
    for md in sorted(systems_dir.glob("*.md")):
        text = md.read_text()
        title = ""
        purpose = ""
        title_m = re.search(r'^#\s+(.+)', text, re.MULTILINE)
        if title_m:
            title = title_m.group(1).strip()
        purpose_m = re.search(
            r'##\s+Purpose\s*\n(.*?)(?=\n##|\Z)',
            text,
            re.DOTALL,
        )
        if purpose_m:
            purpose = purpose_m.group(1).strip()
            # Take first paragraph only
            purpose = purpose.split('\n\n')[0].replace('\n', ' ')
        results.append(SystemDoc(stem=md.stem, title=title, purpose=purpose))
    return results


def parse_enums(src: Path) -> list[EnumInfo]:
    """Extract variants for high-value gameplay enums."""
    results: list[EnumInfo] = []
    found: set[str] = set()

    # Only search directories that contain gameplay types.
    search_dirs = [src / d for d in ("components", "resources", "species")]
    rs_files = sorted(rs for d in search_dirs if d.exists() for rs in d.glob("*.rs"))
    for rs in rs_files:
        text = rs.read_text()
        for enum_name in TARGET_ENUMS:
            if enum_name in found:
                continue
            # Find `pub enum EnumName {`
            pattern = rf'pub\s+enum\s+{re.escape(enum_name)}\s*\{{'
            m = re.search(pattern, text)
            if not m:
                continue
            found.add(enum_name)
            variants: list[str] = []
            start = m.end()
            depth = 1
            pos = start
            while pos < len(text) and depth > 0:
                ch = text[pos]
                if ch == '{':
                    depth += 1
                elif ch == '}':
                    depth -= 1
                pos += 1
            body = text[start:pos - 1]
            for line in body.splitlines():
                line = line.strip()
                if line.startswith('//'):
                    continue
                var_m = re.match(r'(\w+)\s*[,{(]?', line)
                if var_m:
                    variants.append(var_m.group(1))

            info = EnumInfo(
                name=enum_name,
                file=str(rs.relative_to(src.parent)),
                variants=variants,
            )
            # Scrape food_value and decay_rate for ItemKind
            if enum_name == "ItemKind":
                info.extra = _scrape_item_methods(text)
            results.append(info)

    return sorted(results, key=lambda e: TARGET_ENUMS.index(e.name) if e.name in TARGET_ENUMS else 999)


def _scrape_item_methods(text: str) -> dict[str, dict[str, str]]:
    """Scrape food_value() and decay_rate() match arms from items.rs."""
    extra: dict[str, dict[str, str]] = {}

    for method in ("food_value", "decay_rate"):
        m = re.search(rf'pub\s+fn\s+{method}\(self\).*?\{{(.*?)\n\s*\}}', text, re.DOTALL)
        if not m:
            continue
        body = m.group(1)
        # Parse match arms: Self::Variant | Self::Variant => value,
        for arm_m in re.finditer(
            r'((?:Self::\w+\s*\|?\s*)+)\s*=>\s*([^,}]+)',
            body,
        ):
            variants_str = arm_m.group(1)
            value = arm_m.group(2).strip().rstrip(',')
            for v in re.findall(r'Self::(\w+)', variants_str):
                extra.setdefault(v, {})[method] = value
    return extra


def parse_species(species_dir: Path) -> list[PreySpecies]:
    """Parse src/species/*.rs for PreyProfile trait impl values."""
    results: list[PreySpecies] = []
    if not species_dir.exists():
        return results
    for rs in sorted(species_dir.glob("*.rs")):
        if rs.name == "mod.rs":
            continue
        text = rs.read_text()
        name = rs.stem.capitalize()
        stats: dict[str, str] = {}
        # Extract simple fn returns: fn name(&self) -> Type { value }
        for m in re.finditer(
            r'fn\s+(\w+)\(&self\)\s*->\s*[^{]+\{\s*(.+?)\s*\}',
            text,
        ):
            fn_name = m.group(1)
            val = m.group(2).strip()
            # Skip complex bodies
            if '\n' in val or 'match' in val:
                continue
            stats[fn_name] = val
        # Extract seasonal breed modifier
        breed_m = re.search(
            r'fn\s+seasonal_breed_modifier.*?match\s+season\s*\{(.*?)\}',
            text,
            re.DOTALL,
        )
        if breed_m:
            seasons: list[str] = []
            for arm in re.finditer(r'Season::(\w+)\s*=>\s*([^,]+)', breed_m.group(1)):
                seasons.append(f"{arm.group(1)}: {arm.group(2).strip()}")
            stats["seasonal_breed_modifier"] = ", ".join(seasons)

        results.append(PreySpecies(name=name, stats=stats))
    return results


def parse_personality(personality_rs: Path) -> list[tuple[str, str, list[str]]]:
    """Extract personality layers and their axes from the Personality struct."""
    layers: list[tuple[str, str, list[str]]] = []
    if not personality_rs.exists():
        return layers
    text = personality_rs.read_text()
    # Find the struct body
    m = re.search(r'pub struct Personality\s*\{(.*?)\}', text, re.DOTALL)
    if not m:
        return layers
    body = m.group(1)
    current_layer = ""
    current_count = ""
    current_fields: list[str] = []
    for line in body.splitlines():
        line = line.strip()
        layer_m = re.match(r'//\s*---\s*(.+?)\s*\((\d+)\)\s*---', line)
        if layer_m:
            if current_layer:
                layers.append((current_layer, current_count, current_fields))
            current_layer = layer_m.group(1)
            current_count = layer_m.group(2)
            current_fields = []
            continue
        field_m = re.match(r'pub\s+(\w+)\s*:', line)
        if field_m:
            current_fields.append(field_m.group(1))
    if current_layer:
        layers.append((current_layer, current_count, current_fields))
    return layers


@dataclass
class NeedsLevel:
    label: str
    needs: list[str]


def parse_needs(physical_rs: Path) -> tuple[list[NeedsLevel], dict[str, str]]:
    """Extract Maslow levels, their needs, and default values from physical.rs."""
    levels: list[NeedsLevel] = []
    defaults: dict[str, str] = {}
    if not physical_rs.exists():
        return levels, defaults
    text = physical_rs.read_text()

    # Parse struct fields and level comments
    m = re.search(r'pub struct Needs\s*\{(.*?)\}', text, re.DOTALL)
    if m:
        body = m.group(1)
        current_level = ""
        current_fields: list[str] = []
        for line in body.splitlines():
            line = line.strip()
            level_m = re.match(r'//\s*Level\s+(\d+)\s*[—–-]\s*(.+)', line)
            if level_m:
                if current_level:
                    levels.append(NeedsLevel(current_level, current_fields))
                current_level = f"Level {level_m.group(1)}: {level_m.group(2).strip()}"
                current_fields = []
                continue
            field_m = re.match(r'pub\s+(\w+)\s*:', line)
            if field_m:
                current_fields.append(field_m.group(1))
        if current_level:
            levels.append(NeedsLevel(current_level, current_fields))

    # Parse Default impl for actual values
    default_m = re.search(
        r'impl\s+Default\s+for\s+Needs\s*\{.*?fn\s+default\(\).*?\{.*?Self\s*\{(.*?)\}',
        text,
        re.DOTALL,
    )
    if default_m:
        for assign in re.finditer(r'(\w+)\s*:\s*([0-9.]+)', default_m.group(1)):
            defaults[assign.group(1)] = assign.group(2)

    return levels, defaults


def _build_module_index(registered: list[RegisteredSystem]) -> dict[str, int]:
    """Pre-index registered system counts by module for O(1) lookup."""
    index: dict[str, int] = {}
    for r in registered:
        index[r.module] = index.get(r.module, 0) + 1
    return index


def determine_status(
    doc: SystemDoc,
    module_index: dict[str, int],
    src: Path,
) -> str:
    """Classify a system doc as Built, Partial, or Aspirational."""
    modules = DOC_TO_MODULES.get(doc.stem, [])
    if not modules:
        return "Aspirational"

    # Special-case markers (underscore prefix = check exists, not registered)
    specials = [m for m in modules if m.startswith("_")]
    regulars = [m for m in modules if not m.startswith("_")]

    # Count registered functions in regular modules
    reg_count = sum(module_index.get(m, 0) for m in regulars)

    # Check special modules (components/code that exists but isn't a system)
    special_exists = False
    for s in specials:
        tag = s.lstrip("_")
        if tag == "worldgen":
            special_exists = (src / "world_gen").exists() or (src / "world_gen.rs").exists()
        elif tag == "identity":
            special_exists = (src / "components" / "identity.rs").exists()
        elif tag == "skills":
            special_exists = (src / "components" / "skills.rs").exists()
        elif tag == "bodyzone":
            special_exists = (src / "components" / "physical.rs").exists()

    if reg_count >= 1:
        return "Built"
    elif special_exists:
        return "Partial"
    else:
        return "Aspirational"


# ---------------------------------------------------------------------------
# Generators
# ---------------------------------------------------------------------------

HEADER = "<!-- Auto-generated by scripts/generate_wiki.py — do not edit -->\n\n"


def generate_index(
    docs: list[SystemDoc],
    types: list[RustType],
    registered: list[RegisteredSystem],
    enums: list[EnumInfo],
    species: list[PreySpecies],
) -> str:
    components = [t for t in types if t.category == "Component"]
    resources = [t for t in types if t.category == "Resource"]
    messages = [t for t in types if t.category == "Message"]

    built = sum(1 for d in docs if d.status == "Built")
    partial = sum(1 for d in docs if d.status == "Partial")
    aspirational = sum(1 for d in docs if d.status == "Aspirational")
    modules = {r.module for r in registered}

    out = HEADER
    out += "# Clowder Game Wiki\n\n"
    out += "Auto-generated reference for the Clowder cat colony simulation.\n\n"
    out += "## Status Dashboard\n\n"
    out += "| Metric | Count |\n|--------|-------|\n"
    out += f"| Registered system functions | {len(registered)} |\n"
    out += f"| System modules | {len(modules)} |\n"
    out += f"| Component types | {len(components)} |\n"
    out += f"| Resource types | {len(resources)} |\n"
    out += f"| Message types | {len(messages)} |\n"
    out += f"| Prey species | {len(species)} |\n"
    out += f"| Gameplay enums | {len(enums)} |\n"
    out += "\n"
    out += "### Design Doc Status\n\n"
    out += f"| Status | Count |\n|--------|-------|\n"
    out += f"| Built | {built} |\n"
    out += f"| Partial | {partial} |\n"
    out += f"| Aspirational | {aspirational} |\n"
    out += "\n"
    out += "## Pages\n\n"
    out += "- [Systems](systems.md) — registered systems & design doc status\n"
    out += "- [System Map](system-map.md) — execution order & data flow diagram\n"
    out += "- [Components](components.md) — all ECS component types\n"
    out += "- [Resources](resources.md) — all ECS resource types\n"
    out += "- [Messages](messages.md) — inter-system message types\n"
    out += "- [Enums](enums.md) — key gameplay enumerations\n"
    out += "- [Species](species.md) — prey species profiles\n"
    out += "- [Personality](personality.md) — 18-axis personality system\n"
    out += "- [Needs](needs.md) — Maslow hierarchy of needs\n"
    return out


def generate_systems(
    docs: list[SystemDoc],
    registered: list[RegisteredSystem],
) -> str:
    out = HEADER
    out += "# Systems\n\n"
    out += "Status of each design document cross-referenced against `SimulationPlugin::build()`.\n\n"

    # Group by status
    for status in ("Built", "Partial", "Aspirational"):
        group = [d for d in docs if d.status == status]
        if not group:
            continue
        icon = {"Built": "**[Built]**", "Partial": "**[Partial]**", "Aspirational": "*[Aspirational]*"}[status]
        out += f"## {status}\n\n"
        out += "| System | Status | Registered Functions | Design Doc |\n"
        out += "|--------|--------|---------------------|------------|\n"
        for d in group:
            modules = DOC_TO_MODULES.get(d.stem, [])
            regulars = [m for m in modules if not m.startswith("_")]
            funcs = [f"`{r.module}::{r.function}`" for r in registered if r.module in regulars]
            funcs_str = ", ".join(funcs[:5])
            if len(funcs) > 5:
                funcs_str += f" (+{len(funcs) - 5} more)"
            if not funcs_str:
                funcs_str = "—"
            out += f"| {d.title} | {icon} | {funcs_str} | [doc](../systems/{d.stem}.md) |\n"
        out += "\n"

    # Registered functions not covered by any design doc
    doc_modules: set[str] = set()
    for modules in DOC_TO_MODULES.values():
        for m in modules:
            if not m.startswith("_"):
                doc_modules.add(m)
    orphan_modules = {r.module for r in registered} - doc_modules
    if orphan_modules:
        out += "## Undocumented Modules\n\n"
        out += "System modules with registered functions but no design doc:\n\n"
        for mod in sorted(orphan_modules):
            funcs = [r.function for r in registered if r.module == mod]
            out += f"- **{mod}**: {', '.join(funcs)}\n"
        out += "\n"

    return out


def generate_system_map(registered: list[RegisteredSystem]) -> str:
    out = HEADER
    out += "# System Execution Map\n\n"
    out += "How systems execute each tick in `SimulationPlugin::build()`.\n\n"
    out += "## Execution Order\n\n"
    out += "```mermaid\ngraph TD\n"

    # Group by section
    sections: dict[str, list[RegisteredSystem]] = {}
    for r in registered:
        sections.setdefault(r.section, []).append(r)

    for section_key in ("chain1", "chain1.5", "chain2", "chain3", "chain4", "disposition", "standalone"):
        systems = sections.get(section_key, [])
        if not systems:
            continue
        label = SECTION_LABELS.get(section_key, section_key)
        safe_key = section_key.replace(".", "_")
        out += f'    subgraph {safe_key}["{label}"]\n'
        if section_key != "standalone":
            for i, s in enumerate(systems):
                node_id = f"{s.module}_{s.function}"
                node_label = f"{s.module}::{s.function}"
                out += f'        {node_id}["{node_label}"]\n'
                if i > 0:
                    prev = systems[i - 1]
                    prev_id = f"{prev.module}_{prev.function}"
                    out += f"        {prev_id} --> {node_id}\n"
        else:
            for s in systems:
                node_id = f"{s.module}_{s.function}"
                node_label = f"{s.module}::{s.function}"
                out += f'        {node_id}["{node_label}"]\n'
        out += "    end\n"

    # Cross-chain arrows
    out += "\n    %% Cross-chain data flows\n"
    for src_sys, dst_sys, resource in KNOWN_INTERACTIONS:
        src_id = src_sys.replace("::", "_")
        dst_id = dst_sys.replace("::", "_")
        out += f'    {src_id} -.->|"{resource}"| {dst_id}\n'

    out += "```\n\n"

    out += "## Data Flow Summary\n\n"
    out += "| From | To | Via |\n|------|-----|-----|\n"
    for src_sys, dst_sys, resource in KNOWN_INTERACTIONS:
        out += f"| `{src_sys}` | `{dst_sys}` | {resource} |\n"

    return out


def _generate_type_page(types: list[RustType], category: str, title: str, derive: str) -> str:
    filtered = [t for t in types if t.category == category]
    out = HEADER
    out += f"# {title}\n\n"
    label = "component" if category == "Component" else "resource"
    out += f"{len(filtered)} {label} types derived from `#[derive({derive})]`.\n\n"

    by_file: dict[str, list[RustType]] = {}
    for t in filtered:
        by_file.setdefault(t.file, []).append(t)

    for file in sorted(by_file):
        out += f"## `{file}`\n\n"
        for t in by_file[file]:
            out += f"### {t.name} ({t.kind})\n\n"
            if t.doc_comment:
                out += f"> {t.doc_comment}\n\n"
            if t.fields:
                out += "| Field | Type |\n|-------|------|\n"
                for fname, ftype in t.fields:
                    out += f"| `{fname}` | `{ftype}` |\n"
                out += "\n"
            if t.variants:
                out += f"Variants: {', '.join(f'`{v}`' for v in t.variants)}\n\n"
    return out


def generate_components(types: list[RustType]) -> str:
    return _generate_type_page(types, "Component", "Components", "Component")


def generate_resources(types: list[RustType]) -> str:
    return _generate_type_page(types, "Resource", "Resources", "Resource")


def generate_messages(types: list[RustType]) -> str:
    messages = [t for t in types if t.category == "Message"]
    out = HEADER
    out += "# Messages\n\n"
    out += "Inter-system messages (Bevy 0.18 `#[derive(Message)]`).\n\n"
    if not messages:
        out += "*No message types found.*\n"
        return out
    out += "| Message | Fields | Source File |\n|---------|--------|------------|\n"
    for m in messages:
        fields = ", ".join(f"`{n}: {t}`" for n, t in m.fields) if m.fields else "—"
        out += f"| **{m.name}** | {fields} | `{m.file}` |\n"
    out += "\n"
    out += "### Details\n\n"
    for m in messages:
            out += f"#### {m.name}\n\n"
            if m.doc_comment:
                out += f"> {m.doc_comment}\n\n"
            if m.fields:
                out += "| Field | Type |\n|-------|------|\n"
                for fname, ftype in m.fields:
                    out += f"| `{fname}` | `{ftype}` |\n"
                out += "\n"
    return out


def generate_enums(enums: list[EnumInfo]) -> str:
    out = HEADER
    out += "# Gameplay Enums\n\n"
    out += f"{len(enums)} key enumerations defining game mechanics.\n\n"

    out += "## Overview\n\n"
    out += "| Enum | Variants | Source |\n|------|----------|--------|\n"
    for e in enums:
        out += f"| [{e.name}](#{e.name.lower()}) | {len(e.variants)} | `{e.file}` |\n"
    out += "\n---\n\n"

    for e in enums:
        out += f"## {e.name}\n\n"
        out += f"*Source: `{e.file}`*\n\n"
        if e.extra:
            # Rich table with extra columns
            extra_cols = set()
            for v_data in e.extra.values():
                extra_cols.update(v_data.keys())
            extra_cols_sorted = sorted(extra_cols)
            header = "| Variant | " + " | ".join(extra_cols_sorted) + " |\n"
            sep = "|---------|" + "|".join("-" * (len(c) + 2) for c in extra_cols_sorted) + "|\n"
            out += header + sep
            for v in e.variants:
                vals = e.extra.get(v, {})
                row = f"| `{v}` | " + " | ".join(vals.get(c, "—") for c in extra_cols_sorted) + " |\n"
                out += row
        else:
            # Simple variant list as table
            out += "| Variant |\n|---------|\n"
            for v in e.variants:
                out += f"| `{v}` |\n"
        out += "\n"
    return out


def generate_species(species: list[PreySpecies]) -> str:
    out = HEADER
    out += "# Prey Species\n\n"
    out += f"{len(species)} prey species implementing `PreyProfile`.\n\n"

    if not species:
        out += "*No species found.*\n"
        return out

    # Collect all stat keys
    all_keys: list[str] = []
    seen: set[str] = set()
    for s in species:
        for k in s.stats:
            if k not in seen:
                all_keys.append(k)
                seen.add(k)

    # Comparison table
    out += "## Comparison\n\n"
    out += "| Stat | " + " | ".join(s.name for s in species) + " |\n"
    out += "|------|" + "|".join("------" for _ in species) + "|\n"
    for key in all_keys:
        out += f"| `{key}` | " + " | ".join(
            s.stats.get(key, "—") for s in species
        ) + " |\n"
    out += "\n"

    # Individual profiles
    for s in species:
        out += f"## {s.name}\n\n"
        out += "| Stat | Value |\n|------|-------|\n"
        for k, v in s.stats.items():
            out += f"| `{k}` | {v} |\n"
        out += "\n"
    return out


def generate_personality(personality_rs: Path) -> str:
    layers = parse_personality(personality_rs)
    out = HEADER
    out += "# Personality System\n\n"
    total = sum(len(fields) for _, _, fields in layers)
    out += f"{total}-axis personality stored in {len(layers)} conceptual layers.\n\n"
    out += "All axes are `f32` in `[0.0, 1.0]`. Values near 0 represent the low end "
    out += "(e.g. cowardly, reclusive) and values near 1 the high end (e.g. bold, sociable). "
    out += "Generated with a 2-sample average biasing toward 0.5.\n\n"

    for layer_name, count, fields in layers:
        out += f"## {layer_name} ({count})\n\n"
        out += "| Axis | Range |\n|------|-------|\n"
        for f in fields:
            out += f"| `{f}` | 0.0 – 1.0 |\n"
        out += "\n"
    return out


def generate_needs(physical_rs: Path) -> str:
    levels, defaults = parse_needs(physical_rs)
    out = HEADER
    out += "# Maslow Hierarchy of Needs\n\n"
    total = sum(len(lv.needs) for lv in levels)
    out += f"{total} needs across {len(levels)} Maslow levels. All values `f32` in `[0.0, 1.0]`.\n\n"
    out += "Higher levels are multiplicatively suppressed when lower levels are unmet.\n\n"

    out += "## Suppression Formula\n\n"
    out += "```\n"
    out += "Level 1: always 1.0 (no suppression)\n"
    out += "Level 2: physiological_satisfaction\n"
    out += "Level 3: phys × safety_satisfaction\n"
    out += "Level 4: phys × safety × belonging_satisfaction\n"
    out += "Level 5: phys × safety × belonging × esteem_satisfaction\n"
    out += "```\n\n"
    out += "Each satisfaction uses `smoothstep` (Hermite curve) for gradual transitions.\n\n"

    for lv in levels:
        out += f"## {lv.label}\n\n"
        out += "| Need | Default |\n|------|---------|\n"
        for n in lv.needs:
            out += f"| `{n}` | {defaults.get(n, '?')} |\n"
        out += "\n"
    return out


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate Clowder game wiki")
    parser.add_argument("--src", default="src", help="Path to Rust source root")
    parser.add_argument("--docs", default="docs", help="Path to docs directory")
    parser.add_argument("--out", default="docs/wiki", help="Output directory")
    args = parser.parse_args()

    src = Path(args.src).resolve()
    docs = Path(args.docs).resolve()
    out_dir = Path(args.out).resolve()

    print(f"Parsing source in {src}")
    types = parse_rust_types(src)
    registered = parse_registered_systems(src / "plugins" / "simulation.rs")
    system_docs = parse_system_docs(docs)
    enums = parse_enums(src)
    species = parse_species(src / "species")

    print(f"  {len(types)} types, {len(registered)} registered systems, "
          f"{len(system_docs)} design docs, {len(enums)} enums, {len(species)} species")

    # Determine status for each design doc
    module_index = _build_module_index(registered)
    for doc in system_docs:
        doc.status = determine_status(doc, module_index, src)

    # Generate pages
    os.makedirs(out_dir, exist_ok=True)
    pages = [
        ("index.md", generate_index(system_docs, types, registered, enums, species)),
        ("systems.md", generate_systems(system_docs, registered)),
        ("system-map.md", generate_system_map(registered)),
        ("components.md", generate_components(types)),
        ("resources.md", generate_resources(types)),
        ("messages.md", generate_messages(types)),
        ("enums.md", generate_enums(enums)),
        ("species.md", generate_species(species)),
        ("personality.md", generate_personality(src / "components" / "personality.rs")),
        ("needs.md", generate_needs(src / "components" / "physical.rs")),
    ]

    for name, content in pages:
        path = out_dir / name
        path.write_text(content)
        print(f"  wrote {path.relative_to(Path.cwd())}")

    # Generate SUMMARY.md for mdBook navigation
    summary = "# Summary\n\n"
    summary += "[Overview](index.md)\n\n"
    summary += "---\n\n"
    summary += "# Systems\n\n"
    summary += "- [System Status](systems.md)\n"
    summary += "- [Execution Map](system-map.md)\n\n"
    summary += "# ECS Reference\n\n"
    summary += "- [Components](components.md)\n"
    summary += "- [Resources](resources.md)\n"
    summary += "- [Messages](messages.md)\n"
    summary += "- [Enums](enums.md)\n\n"
    summary += "# Game Mechanics\n\n"
    summary += "- [Prey Species](species.md)\n"
    summary += "- [Personality](personality.md)\n"
    summary += "- [Needs](needs.md)\n"
    (out_dir / "SUMMARY.md").write_text(summary)
    print(f"  wrote {(out_dir / 'SUMMARY.md').relative_to(Path.cwd())}")

    print(f"\nDone — {len(pages) + 1} pages in {out_dir.relative_to(Path.cwd())}")


if __name__ == "__main__":
    main()
