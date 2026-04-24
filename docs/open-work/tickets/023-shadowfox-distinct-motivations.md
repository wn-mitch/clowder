---
id: 023
title: Shadowfox motivations distinct from normal foxes
status: ready
cluster: null
added: 2026-04-14
parked: null
blocked-by: []
supersedes: []
related-systems: [magic, wildlife, combat]
related-balance: []
landed-at: null
landed-on: null
---

## Why

Shadowfoxes currently run a flat hardcoded state machine (`Patrolling → Stalking → Ambush → Cooldown`) in `wildlife_ai` / `predator_stalk_cats`. They lack `FoxState`, bypass the fox GOAP pipeline, and feel like stalk-ambush machines rather than corruption made manifest. Their population cap is 0 (disabled). They need motivations that are alien — ecologically grounded with mythic weight, not biological drives with a reskin.

## Design (2026-04-24)

Replace the flat state machine with a **four-drive scored state machine**. Each tick, four pressure values (0.0–1.0) feed softmax selection; the winning drive maps to a `WildlifeAiState` variant.

### Drives

| Drive | What it does |
|-------|-------------|
| **Coherence** | Self-preservation via corruption. Sustained by high-corruption tiles, attenuated by clean ground. At 0 → spontaneous dissolution. Colonies can defeat shadowfoxes by cleansing their substrate. |
| **Resonance** | Corruption gardener. Drawn to corruption that is *losing ground* (adjacent to wards, recently cleansed). Patrols ward perimeters depositing corruption. Makes wards attract attention rather than merely block. |
| **Dread** | Psychic predation. Targets psychologically vulnerable cats (low mood, low safety, isolated). Primary mode is *Haunting* — pacing at detection edge, persistent safety/mood drain, no combat. Escalates to stalk/ambush after N ticks. Suppressed by grouped cats (2+ allies). |
| **Entropy** | Corruption expansion. Seeks the corruption frontier (boundary between corrupted and clean ground) and extends it. Probes for gaps in ward coverage. |

### New component

`ShadowFoxDrives { coherence, resonance, dread, entropy, age_ticks, origin_corruption }` — attached at spawn, no `FoxState`.

### New WildlifeAiState variants

`Reconstituting`, `Tending`, `Haunting`, `Seeding` — join existing `Patrolling`, `Stalking`, `EncirclingWard`, `Fleeing`.

### New systems

- `shadowfox_coherence_tick` — decay/recovery based on tile corruption
- `shadowfox_motivation_tick` — score four drives, select winner, transition state

### Emergent feedback loops

- Cleansing corruption → Resonance spikes (shadowfox defends) + Coherence drops (shadowfox weakens) → colony gets breathing room while it reconstitutes
- Haunting → cat mood/safety drop → suppresses positive behaviors → CorruptionPushback rate drops → corruption stabilizes → shadowfoxes strengthen
- Ward placement → blocks entry + draws Resonance to perimeter + Entropy probes for gaps → spatial puzzle, not point defense

### Key decisions

- **Haunting is primary threat mode** — psychological pressure first, combat is escalation
- **Both defeat paths valid** — coherence dissolution (slow, environmental) and combat banishment (fast, heroic)
- **All four drives ship together**, delivered in phases A–D
- **Inverted-ward attraction deferred** — not core to motivation system

### Phasing

- **A**: Foundation — `ShadowFoxDrives` component, coherence decay/recovery, dissolution, cap to 1, existing stalk/ambush unchanged
- **B**: Drive scoring + state selection — softmax over four drives, new AI states and movement logic, new features/events
- **C**: Behavioral depth — vulnerability targeting, frontier detection, ward-perimeter awareness, haunting drain wiring
- **D**: Population cap lift + balance — cap to 2, full soak sweep, canary verification, hypothesis documentation

### Critical files

`src/components/wildlife.rs`, `src/systems/wildlife.rs`, `src/resources/sim_constants.rs`, `src/systems/magic.rs`, `src/resources/system_activation.rs`, `src/resources/event_log.rs`, `src/plugins/simulation.rs`, `src/main.rs`

Full design: `.claude/plans/let-s-work-to-design-zippy-boot.md`
