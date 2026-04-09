# Cat Roster Panel

A persistent left-sidebar panel listing all living cats with at-a-glance status. Complements the existing cat inspect panel — the roster shows the colony overview, clicking a cat opens the detailed inspect view.

## Context

The current UI has a cat inspect panel (left side, triggered by click/Tab) that shows detailed info for one cat at a time. There's no way to see colony-wide health at a glance without clicking each cat individually. The roster fills this gap — a Rimworld-style colonist list showing every cat's mood, disposition, and critical needs in compact rows.

## Layout

- **Position**: top-left sidebar, `left: 8px, top: 8px`
- **Size**: `width: 220px, height: 70%` viewport
- **Toggle**: `R` key (new keybind)
- **Scroll**: vertical scroll when cats exceed panel height
- **Visibility**: shown by default, independent of inspect panel visibility

The roster and inspect panels coexist as separate panels. When a cat is selected, the inspect panel appears to the right of the roster (offset by roster width + gap) rather than overlapping it.

## Roster Row

Each cat gets a ~36px-tall row containing two lines:

**Line 1 (header):** mood dot, name (fur-colored), disposition label (right-aligned, dim)

**Line 2 (bars):** three mini horizontal bars for hunger, energy, safety

```
[mood dot] Bramble              Hunting
[■■■□□] [■■□□□] [■■■■□]
 hunger   energy   safety
```

### Mood Indicator

A small colored square (8x8px) conveying mood valence:
- `valence > 0.3` → green (`BAR_GREEN`)
- `valence > -0.3` → yellow (`BAR_YELLOW`)  
- `valence <= -0.3` → red (`BAR_RED`)

### Name

Cat's name text, colored using the existing `fur_color_to_bevy()` mapping so cats are visually distinguishable in the roster.

### Disposition

Current `DispositionKind` label (Resting, Hunting, Foraging, etc.) in `TEXT_DIM`, right-aligned on the header line.

### Need Bars

Three compact horizontal bars (4px tall, ~50px wide each) for the three physiological needs:
- **Hunger** — `needs.hunger`
- **Energy** — `needs.energy`  
- **Safety** — `needs.safety`

Color follows existing `bar_color()` logic: red < 0.2, yellow < 0.5, green above.

## Interaction

- **Click** a roster row → sets `InspectionMode::CatInspect(entity)`, opening the inspect panel
- **Selected cat** row gets a lighter background highlight (`PANEL_BG` with higher alpha)
- **Tab** cycling still works and highlights the corresponding roster row
- **Dead cats** are excluded from the roster query (`Without<Dead>`)

## Panel Header

"Colony (N)" where N is the count of living cats, using `TEXT_HIGHLIGHT` color.

## Ordering

Cats sorted by `Entity` for stable ordering, matching Tab-cycle behavior.

## Inspect Panel Repositioning

When the roster is visible, the cat inspect panel shifts right to avoid overlap:
- Roster visible: inspect panel `left: 236px` (220px roster + 16px gap)
- Roster hidden: inspect panel `left: 8px` (current behavior)

This requires the inspect panel's `Node.left` to be updated each frame based on roster visibility.

## Styling

Same dark semi-transparent styling as all other panels:
- Background: `PANEL_BG` (0.08, 0.08, 0.1, 0.85)
- Border: `PANEL_BORDER` (0.4, 0.35, 0.25, 0.9), 2px
- Text: `TEXT_COLOR`, `TEXT_DIM`, `TEXT_HIGHLIGHT` as appropriate

## Files

- **New**: `src/rendering/ui/cat_roster.rs` — roster panel setup and update system
- **Modify**: `src/rendering/ui/mod.rs` — register module, add `roster` to `PanelVisibility`, add `R` toggle
- **Modify**: `src/rendering/ui/cat_inspect.rs` — adjust `left` position based on roster visibility
- **Modify**: `src/rendering/ui/selection.rs` — no changes needed (existing click/Tab logic sets `InspectionMode` which both roster and inspect consume)

## Components

- `CatRoster` — marker for the roster panel container
- `CatRosterContent` — marker for the scrollable content area
- `RosterRow(Entity)` — marker on each row node, storing the cat's entity for click detection

## Systems

- `setup_cat_roster` (Startup) — spawn panel structure
- `update_cat_roster` (Update) — rebuild rows when cat count changes or data updates, handle click interaction, highlight selected cat
- `toggle_roster_visibility` — `R` key handler (can be folded into existing `toggle_panel_visibility`)

## Update Strategy

Rebuild roster content each frame only when something changed (cat spawned/died, tick advanced). Track a `last_tick: u64` in a `Local` to debounce rebuilds — only rebuild when simulation tick advances. This avoids per-frame entity spawning overhead while keeping the display current.
