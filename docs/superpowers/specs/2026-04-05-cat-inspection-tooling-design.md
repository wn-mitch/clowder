# Cat Inspection Tooling

## Purpose

Enable post-hoc inspection of individual cats to understand how their personality
drives their decisions. After a headless run, you should be able to ask "why did
Rowan spend 40% of the sim hunting while Ash barely hunted at all?" and get a
clear answer grounded in personality axes, needs state, relationships, and scored
action data.

## Components

### 1. New EventKind: `CatSnapshot`

Emitted every 100 ticks for each living cat. Captures the full state needed to
understand behavior without cross-referencing other data sources.

**Fields:**

```
CatSnapshot {
    cat: String,                          // Name
    position: (i32, i32),
    personality: PersonalitySnapshot,      // All 18 axes
    needs: NeedsSnapshot,                 // All 9 fields
    mood_valence: f32,                    // Current effective valence
    mood_modifier_count: usize,           // Active modifier count
    skills: SkillsSnapshot,              // All 6 skill values
    health: f32,                          // Current / max
    corruption: f32,
    magic_affinity: f32,
    current_action: Action,
    top_relationships: Vec<RelationshipEntry>,  // Top 3 by |fondness|
}
```

Where the sub-structs flatten to JSON naturally:

- `PersonalitySnapshot`: 18 named f32 fields (boldness, sociability, curiosity, ...)
- `NeedsSnapshot`: 9 named f32 fields (hunger, energy, warmth, safety, social, acceptance, respect, mastery, purpose)
- `SkillsSnapshot`: 6 named f32 fields (hunting, foraging, herbcraft, building, combat, magic)
- `RelationshipEntry`: `{ cat: String, fondness: f32, familiarity: f32, bond: Option<String> }`

**Emission site:** A new system `emit_cat_snapshots` that runs every 100 ticks,
iterates all living cats, and pushes to `EventLog`. Separate from `evaluate_actions`
to avoid the 16-param limit.

### 2. Expand `ActionChosen`

Add third-place action and score:

```
ActionChosen {
    cat: String,
    action: Action,
    score: f32,
    runner_up: Action,
    runner_up_score: f32,
    third: Action,           // NEW
    third_score: f32,        // NEW
}
```

### 3. `examples/inspect_cat.rs`

A cargo example binary that reads `logs/events.jsonl` and prints a formatted
report for a named cat.

**Usage:** `cargo run --example inspect_cat -- <cat-name> [--events <path>]`

Default event log path: `logs/events.jsonl`.

**Report sections:**

#### a. Personality Profile

Prints all 18 axes grouped by category with human-readable labels:

```
=== Rowan - Personality Profile ===

Drives:
  boldness      0.82  ████████░░  bold
  sociability   0.45  ████░░░░░░  moderate
  curiosity     0.31  ███░░░░░░░  incurious
  diligence     0.67  ██████░░░░  diligent
  warmth        0.55  █████░░░░░  moderate
  spirituality  0.22  ██░░░░░░░░  pragmatic
  ambition      0.71  ███████░░░  ambitious
  patience      0.48  ████░░░░░░  moderate

Temperament:
  anxiety       0.33  ███░░░░░░░  steady
  optimism      0.61  ██████░░░░  optimistic
  temper        0.74  ███████░░░  hot-tempered
  stubbornness  0.58  █████░░░░░  moderate
  playfulness   0.40  ████░░░░░░  moderate

Values:
  loyalty       0.80  ████████░░  loyal
  tradition     0.35  ███░░░░░░░  progressive
  compassion    0.50  █████░░░░░  moderate
  pride         0.65  ██████░░░░  proud
  independence  0.42  ████░░░░░░  moderate
```

Labels derived from thresholds: <0.3 = low label, 0.3-0.7 = "moderate", >0.7 = high label.

#### b. Action Distribution

Histogram from all ActionChosen events for this cat:

```
=== Action Distribution (1,247 decisions) ===

  Hunt       412  ██████████████████████  33.0%
  Build      289  ████████████           23.2%
  Eat        198  ████████               15.9%
  Sleep       95  ████                    7.6%
  Fight       78  ███                     6.3%
  ...

Personality correlation:
  boldness=0.82 -> Hunt (33%) + Fight (6%) = 39% combat-oriented
  diligence=0.67 -> Build (23%) + Forage (4%) = 27% work-oriented
```

#### c. Needs Timeline

From periodic CatSnapshot events, show min/max/final for each need:

```
=== Needs Timeline (386 snapshots over 1,247 days) ===

            min    max    final   critical dips
  hunger    0.02   0.95   0.71    3 (ticks 104200, 108900, 112400)
  energy    0.15   0.92   0.83    0
  warmth    0.31   0.95   0.88    0
  safety    0.00   1.00   0.45    12
  social    0.08   0.82   0.34    2
  ...
```

#### d. Relationships

From the most recent CatSnapshot's relationship data:

```
=== Relationships ===

  Wren      fondness: +0.72  familiarity: 0.85  [mate]
  Ash       fondness: +0.34  familiarity: 0.61
  Birch     fondness: -0.15  familiarity: 0.44
```

#### e. Key Decisions (last 20)

Most recent ActionChosen events with top-3 scores:

```
=== Recent Decisions ===

  tick 134200  Hunt (0.87)  > Build (0.62) > Eat (0.58)
  tick 134215  Eat (0.91)   > Hunt (0.74) > Groom (0.31)
  tick 134220  Sleep (1.42)  > Eat (0.55) > Groom (0.30)
  ...
```

#### f. Death Report (if applicable)

```
=== Death ===

  Died at tick 112631 (day 1127) from Injury
  Final needs: hunger=0.45 energy=0.12 safety=0.00
  Last 3 snapshots showed declining safety: 0.42 -> 0.18 -> 0.00
```

### 4. `just inspect` recipe

```
inspect name:
    cargo run --example inspect_cat -- {{name}}
```

## Files to modify

| File | Change |
|------|--------|
| `src/resources/event_log.rs` | Add `CatSnapshot` variant, expand `ActionChosen` with third place |
| `src/systems/ai.rs` | Update ActionChosen emission to include third-place action |
| `src/systems/ai.rs` or new `src/systems/snapshot.rs` | New `emit_cat_snapshots` system |
| `src/main.rs` | Register snapshot system in schedule |
| `examples/inspect_cat.rs` | New file: CLI report tool |
| `justfile` | Add `inspect` recipe |

## Files to read (not modify)

| File | Why |
|------|-----|
| `src/components/personality.rs` | Personality field names for snapshot struct |
| `src/components/physical.rs` | Needs field names |
| `src/components/skills.rs` | Skills field names |
| `src/components/mental.rs` | Mood, Memory types |
| `src/resources/relationships.rs` | Relationship lookup API |

## Verification

1. `just test` passes
2. `just check` (clippy) passes
3. Run headless 30s, then `just inspect Rowan` produces a formatted report
4. Report personality axes correlate visibly with action distribution
5. Event log file size stays reasonable (<50MB for a 60s run)
