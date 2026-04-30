# Magic

## Purpose

Models the world's fundamental force as a dual-natured current flowing through territory,
practitioners, objects, and living things. Magic is not a technology cats master — it is an
ecology they inhabit. The current has two aspects that cannot be separated, only managed: an
ordered, vital side and a wild, raw side. Cats call the wild side's residue **corruption**, but
it is not contamination — it is one half of the same force that wards channel and the Calling
touches. Skilled practitioners work in both aspects simultaneously. The art is shaping the ratio,
not achieving purity.

Three practice tiers:
- **Attunement** — perceiving and reading the current; passive at low levels; deliberate Scry and
  Commune actions strengthen the signal. Maslow tier 5.
- **Rites** — collective shaping of the current at the colony's thresholds (birth, death, hunt,
  season-turn). Require an officiating cat and consume crafted materials.
- **Imbued Craft** — fixing a portion of the current into material objects during crafting.
  Intentional, risky, ecologically constrained.

The **Dark Calling** — documented in `the-calling.md` §7.W.5 — is not a separate system. It is
what happens when a practitioner stops shaping the current and starts being shaped by it. It falls
out of the personal corruption mechanics below without requiring new triggers.

---

## The current: ordered and wild aspects

The magical force flows through territory continuously. It has two aspects that cannot be
disentangled, only managed in ratio:

| Aspect | What it is | How it manifests |
|--------|-----------|-----------------|
| **Ordered** | The vital, shaped portion of the current | Ward strength; herb potency at commune-echoed tiles; Named Objects produced by the Calling; rite effects; imbued items |
| **Wild** | The raw, unordered portion of the current | Tile corruption; personal corruption; misfire energy; shadow fox crystallization |

In code, the measurable quantity for the wild aspect is **corruption** — on tiles (`TileMap`) and
on cats (`Corruption` component). The ordered aspect has no direct measure; it is implied by the
*absence* of wild excess and the *presence* of wards, commune echoes, and rite effects.

**Shadow foxes** are what the wild aspect crystallizes into when territorial disorder goes
unmanaged. They are not invaders arriving from outside — they are the territory's own imbalance
made animate. Killing a shadow fox does not resolve the underlying disorder; it buys time.
Reordering the current at the territory level does.

**Wards shape the current; they do not block it.** A ward at the territory perimeter does not
prevent the wild aspect from entering. It organizes a gradient so the territory's interior sits in
the ordered portion of the current. When wards fail the gradient collapses and disorder spreads
inward. This is mechanically identical to the existing `corruption_spread` system — the territory
is not being invaded, it is going unmanaged.

---

## Personal corruption: exposure, not contamination

A cat with personal corruption is not sick. They have been *exposed* to the current — specifically
its wild aspect — and their sensitivity has changed. Low exposure sharpens magical perception. High
exposure overwhelms it. This is not contamination; it is a practitioner living close to the thing
they work with.

| Corruption range | Name | Effect |
|-----------------|------|--------|
| 0.0 – 0.15 | **Sensitized** | Effective affinity = `affinity * (1.0 + 0.2 * corruption)`. Scry range and commune echo strength improve. No mood penalty. Premonition trigger rate doubles. |
| 0.15 – 0.40 | **Exposed** | Mood jitter begins: `uniform(-corruption * 0.5, corruption * 0.5)`. Social penalty: `-0.05 * corruption`. Affinity bonus still applies at the low end of this range. |
| 0.40 – 0.70 | **Overwhelmed** | Existing scales apply: mood jitter = `uniform(-corruption, corruption)`, social penalty = `-0.1 * corruption`. No affinity bonus. |
| 0.70+ | **Dark Calling window** | `axis_capture` trigger conditions for dark variant become eligible; see `the-calling.md` §7.W.5. |

This creates a practitioner sweet spot at 0.10–0.20: enough to sharpen perception, not enough to
overwhelm. Cats who practice magic regularly will naturally sit in this range. Cats who push deeper
are making a choice with compounding costs.

**Natural decay:** `personal_corruption(t+1) = personal_corruption(t) * 0.999` per tick (halves
in ~700 ticks without intervention). Rest at a warded or commune-echoed tile accelerates recovery:
`* 0.995` per tick.

---

## Parameters

### Affinity and practice

| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Affinity — low tier | 80% of cats get 0.0–0.2 | Most cats feel the current but cannot shape it |
| Affinity — mid tier | 15% of cats get 0.3–0.6 | Hedge practitioners; rite-eligible |
| Affinity — high tier | 5% of cats get 0.7–1.0 | Full practitioners; Calling-eligible |
| Misfire threshold | `magic_skill < affinity * 0.8` | Underskilled for the force they're touching |
| Misfire probability | `max(0, (1 - magic_skill / affinity) * 0.5)` | Scales with skill gap; 0 when skilled enough |

### Wild aspect (tile corruption — unchanged implementation)

| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Corruption spread rate | 0.001/tick to adjacent tiles | Slow; requires sustained neglect before territory becomes dangerous |
| Spread rate within commune echo | 0.0005/tick | Shaped current resists disorder locally |
| Spread rate at natural ward (burial rite) | 0.0/tick | Permanently ordered site does not propagate disorder |
| Ward strength at creation | 1.0 | Full potency on placement |
| Ward decay — basic | 0.005/tick | ~200 ticks to failure |
| Ward decay — durable | 0.001/tick | ~1000 ticks |
| Shadow fox spawn threshold | tile corruption ≥ 0.8 | Crystallization point for wild-aspect density |

### Personal corruption tiers (new)

| Parameter | Initial Value | Rationale |
|-----------|--------------|-----------|
| Sensitized ceiling | 0.15 | Peak of affinity bonus window |
| Exposed ceiling | 0.40 | Significant impairment begins |
| Dark Calling floor | 0.70 | Axis-capture eligible |
| Natural decay factor | 0.999/tick | Slow without intervention |
| Accelerated decay (warded/echoed tile) | 0.995/tick | Rest in ordered territory hastens recovery |

### Herb effects (unchanged; extended by Potent flag)

| Preparation | Effect | Normal | Potent (echoed-tile herbs or imbued) |
|-------------|--------|--------|--------------------------------------|
| Healing poultice | Health restoration | +0.3 | +0.45 to +0.6 (scales with imbue strength) |
| Energy tonic | Energy restoration | +0.2 | +0.3 |
| Mood tonic | Mood valence boost | +0.3 | +0.45 |

---

## Tier 1 — Attunement

Attunement is how high-affinity cats perceive the current. It is always-on at low levels; deliberate
Scry and Commune actions strengthen the signal. Both step resolvers exist (`resolve_scry`,
`resolve_spirit_communion`). The following redesigns their effects; the infrastructure stays.

### Scrying (redesign of `resolve_scry`)

**Current behavior:** Writes a `MemoryEntry { ResourceFound, location: random_tile }` — the tile
is random, not based on any actual resource. Effectively noise. Skill grows.

**Redesigned behavior:** The scryer reads the current's local shape and returns genuine information.

On completion (`ticks >= scry_duration`):

1. **Herb reveal:** Scan all herb entities within `magic_skill * 15` tiles. For each that is not
   yet in the cat's memory, add a `MemoryEntry { ResourceFound, location: herb_position }`. The
   cat gains a genuine map of nearby herb patches — useful for the herbcraft gather DSE.

2. **Corruption gradient:** For all tiles within `magic_skill * 12` tiles, if tile corruption
   exceeds 0.3, mark those positions in a new `KnownCorruption` resource (a sparse map of
   corruption hotspot positions readable by the cleanse DSEs). The scryer doesn't have to have
   physically visited those tiles.

3. **Prey direction:** A probabilistic signal toward the nearest high-prey-density area. Accuracy
   = `0.5 + 0.5 * magic_skill`. On an accurate roll: apply a short-lived `PreyDirection` marker
   biasing the cat's hunt-target DSE toward the indicated area for ~50 ticks. On an inaccurate
   roll: the marker still fires with wrong information — low-skill high-affinity cats are erratic,
   not just weak.

**Personal corruption gain:** `+0.02`. Scrying at a tile with ambient corruption > 0.2 reads the
wild aspect more clearly (full gradient information even at lower skill), but costs `+0.05`
personal corruption instead — the practitioner is drawing on disorder to see it.

**Misfire:** Unchanged. Fizzle = `StepResult::Fail`. Other misfires apply their existing effects
before the step completes.

### Spirit Communion → Commune Echo (redesign of `resolve_spirit_communion`)

**Current behavior:** On completion, applies a mood modifier and grows skill. No territory effect.

**Redesigned behavior:** The communing cat shapes the local current toward order. On completion:

1. **Mood bonus:** Retained. `+spirit_communion_mood_bonus` for `spirit_communion_mood_duration`
   ticks. The cat sat in the current; that is genuinely nourishing at manageable exposure.

2. **Commune echo:** Spawn (or strengthen) a `CommuneEcho` component on the current tile.

   | Echo property | Value |
   |--------------|-------|
   | Strength at creation | `magic_skill * 0.6 + effective_affinity * 0.4` |
   | Decay rate | 0.003/tick (~330 ticks at full strength; slower than a ward — this is ambient shaping, not a placed barrier) |
   | Corruption spread suppression | Adjacent tiles spread at 0.5× rate while echo active |
   | Herb potency tag | Herbs foraged from this tile carry `Potent` flag while echo active; Herbalism & Remedy recipes produce Potent output from these inputs |
   | Calling preference | Calling trigger-check adds +50% weight when cat is at an echoed tile |

3. **Skill growth:** Retained.

**Personal corruption gain:** `+0.03`. The cat touched the current to shape it; contact has a cost.

**`on_special_terrain` scoring axis** in `CommuneDse` already gives preference to FairyRings and
similar sites — no change needed there. FairyRing sites should produce stronger echoes: multiply
echo strength by 1.5 at `Terrain::FairyRing`.

### Premonitions (new)

Premonitions are passive attunement events — impressions the world sends to high-affinity cats
without deliberate action. They are not infallible. They are the current's information leaking
through sensitivity that hasn't yet been fully controlled.

| Parameter | Value |
|-----------|-------|
| Eligible cats | `magic_affinity >= 0.4` |
| Base trigger rate | 0.01% per tick |
| Sensitized trigger rate | 0.02% per tick (doubles in 0.0–0.15 personal corruption range) |
| Premonition types | Weather shift (1 season ahead) · prey direction · danger vector (predator approach) · herb patch location |
| Accuracy | `0.5 + 0.5 * magic_skill` (0.0 skill = coin-flip; 1.0 skill = always correct) |
| Behavioral effect | Short-lived DSE score modifier (~50 ticks) biasing the cat toward the premonition's implied action |
| False premonitions | The modifier fires regardless of accuracy — wrong information still influences behavior |
| Event emission | `Significant`-tier event fires after-the-fact when a correctly-predicted event occurs within the window. Feeds the `naming.md` substrate. |

**Implementation note:** Premonitions require a new per-tick system checking affinity and firing
the trigger, plus a new marker type (e.g. `PreyDirectionHint`, `DangerWarning`) that the relevant
DSEs can read as a scoring axis. They do not require changes to existing step resolvers.

---

## Tier 2 — Rites

Rites are the colony's collective practice of shaping the current at life's thresholds. They are
not an optional bonus layer — they are what the colony does to maintain its relationship with the
land it occupies. A colony that neglects rites pays slowly: corruption creeps, grief lingers, hunts
go slightly worse, kittens are less settled. None of these are dramatic; they compound.

**Common requirements for all rites:**
- Officiating cat: `magic_affinity >= 0.4`; adult; not incapacitated
- Consuming a crafted item (type varies by rite; see below)
- Officiator gains personal corruption proportional to ambient tile corruption at the rite site:
  `corruption_cost = base_cost + tile_corruption * exposure_multiplier`
- Produces a `RitePerformed` Significant-tier event (feeds `naming.md` substrate)
- Other cats may attend; attendance is scored by a rite-attendance DSE (analogous to the burial
  attendance that already exists); attending cats receive reduced versions of rite effects

### Hunt-Blessing

**Purpose:** Shape the current in favor of the hunt's participants before they leave territory.

**Trigger:** Scored when a hunting party of 2+ cats is forming and no Hunt-Blessing has been
performed in the last ~300 ticks.

**Participants:** 1 officiator + 2–4 attending cats (voluntary).

**Location:** WardPost, FairyRing, or any ward-covered tile at territory boundary.

**Inputs:** 1 ward-herb bundle (Herbalism & Remedy output; herb type shapes the effect profile).

**Duration:** ~8 ticks.

**Effect:** All attending cats (and officiator) receive `HuntBlessed` marker for ~200 ticks. Hunt-
strike resolver reads `HuntBlessed` and applies a focus bonus: `+0.08` to strike accuracy. The
prey distribution does not change — the cats' attentiveness does.

**Herb profile variation:**
| Herb | Additional effect |
|------|-----------------|
| Thornbriar | Ward at the hunt departure tile strengthened by +0.1 |
| Moonpetal | Premonition accuracy +0.1 for blessed cats during hunt |
| Dreamroot | Detection noise threshold -5% for blessed cats (heightened awareness) |

**Officiator corruption cost:** `+0.03 + 0.05 * tile_corruption`.

---

### Burial Rite

**Purpose:** Transform what a death releases — grief becomes a protective force grounded in the land.

**Trigger:** Scored within ~20 ticks of a burial completing, when a high-affinity cat is within ~8
tiles of the burial site.

**Participants:** 1 officiator; family and colony members may attend (same social logic as burial).

**Location:** The burial tile.

**Inputs:** 1 carved bone token (Bone & Shell Craft output).

**Duration:** ~10 ticks.

**Effect:**
- The burial tile becomes a **natural ward**. Strength = `officiator.magic_skill * 0.8`. Does not
  decay — the death and the rite ground it permanently.
- Corruption spread from adjacent tiles is suppressed at this site permanently (`spread_rate = 0`).
- If tile corruption > 0.4 at time of rite: also emits `CorruptionPushback` centered on the tile.
  The wild aspect built up around death is redirected outward by the shaping act.

**Officiator corruption cost:** `+0.05 + 0.1 * tile_corruption`. Performing this rite in heavily
corrupted territory is costly. An officiator who performs burial rites repeatedly across multiple
deaths in dangerous areas accumulates real exposure — this is a meaningful arc for colony
practitioners, not just a routine maintenance action.

---

### Season-Opening Rite

**Purpose:** Negotiate with the current at the seasonal turn — the world's force shifts, and the
colony marks the change.

**Trigger:** Scored at the first day of spring and first day of autumn. Once per season.

**Participants:** 1 officiator + attending cats (more = stronger effect; cap at 5 for scaling).

**Location:** FairyRing preferred; any tile with commune echo strength ≥ 0.3.

**Inputs:** 1 Scent Censer (Phase 4 crafting output) + 1 in-season herb.

**Duration:** ~15 ticks.

**Effect:**
- For the current season, herb growth rate = `1.0 + 0.1 * min(participant_count, 5)`.
- Ambient corruption decay rate on all territory tiles `+0.0002/tick` for the season (stacks with
  ward effects — wards and rites compound each other, they are not alternatives).
- The Scent Censer used becomes permanently charged ("season-marked" flag) — it continues
  functioning as a Phase 4 decoration and becomes eligible for naming via `naming.md`.

**Officiator corruption cost:** `+0.04`. Low — the season-opening works *with* the natural current
at its most legible moment.

---

### Kitten-Blessing

**Purpose:** Welcome a new life into the current. The simplest rite; the most joyful.

**Trigger:** Scored when a kitten is born and a high-affinity cat is within ~8 tiles.

**Participants:** 1 officiator; parents may attend.

**Location:** Birth tile or nearest warm, warded tile.

**Inputs:** None. The kitten is the focus; the current needs no material anchor here.

**Duration:** ~5 ticks.

**Effect:**
- Kitten's `spirituality` starts `+0.05` above genetic baseline (permanent).
- Event fires as Significant tier → naming substrate trigger ("Bracken's Welcoming").
- Officiator receives `+0.1` mood for ~100 ticks.

**Officiator corruption cost:** `+0.01`. Blessing a birth goes *with* the current.

---

## Tier 3 — Imbued Craft

Imbuing an object means fixing a portion of the current into its material during crafting. The
object then acts as a persistent local organizer of the current around itself — not because it is
enchanted in isolation, but because a shaped portion of the force now inhabits the material.

**Design direction — intentional channeling.** Imbuing requires the crafter to deliberately draw
on ambient corruption during crafting. This is not a passive quality variation. A pristine cat at
a pristine tile cannot imbue — there is nothing to channel from. The crafter draws from:
- Ambient tile corruption at the crafting station (tile corruption > 0), OR
- Their own personal corruption (> 0.05), OR
- Both.

This makes ambient corruption a *resource* for practitioners, creating genuine colony-level tension:
do you cleanse the territory, or maintain some disorder for the practitioners who work there? The
answer is ecology-dependent — neither is universally correct.

**Imbued items** carry the ordered aspect fixed in material. **Corrupted items** (misfire outcome)
carry the wild aspect fixed in material. Corrupted items are not simply failures — they are objects
in resonance with the raw current. There are contexts where they are sought.

### Imbuing

At crafting time, a cat with `magic_affinity >= 0.3` may elect to channel. The crafter selects a
depth (shallow / moderate / deep), which determines output quality and personal risk:

| Depth | Imbue potential formula | Personal corruption gain |
|-------|------------------------|--------------------------|
| Shallow (0.1) | `magic_skill * 0.1 + affinity * 0.3` | `0.1 * (1.0 - magic_skill * 0.5)` |
| Moderate (0.3) | `magic_skill * 0.3 + affinity * 0.3` | `0.3 * (1.0 - magic_skill * 0.5)` |
| Deep (0.6) | `magic_skill * 0.6 + affinity * 0.3` | `0.6 * (1.0 - magic_skill * 0.5)` |

Skilled cats draw more cleanly — the `magic_skill * 0.5` term in the corruption formula means a
fully skilled cat gains half the corruption of an unskilled one at the same depth.

**Misfire check** applies if `magic_skill < affinity * 0.8`. On misfire: item becomes corrupted
rather than imbued.

### Imbued item effects (resolver-keyed, not item fields)

Effects live on action resolvers keyed to `item.imbue_strength`, not on numeric modifier fields on
the item type (same constraint as the rest of crafting — see `crafting.md` §design constraints).

| Crafted item | Imbued effect |
|--------------|---------------|
| Grooming Brush | Grooming action becomes emotionally resonant for both cats: fondness delta `+0.05 * imbue_strength` per session; recipient also gains a small personal corruption decay `+0.03 * imbue_strength` per tick during grooming |
| Reed Mat / Rug | Tile corruption spread rate reduced by `imbue_strength * 0.3`; cats sleeping at this tile have nightmares suppressed; kitten sleep at this tile counts as higher-quality rest |
| Tallow Lamp | Illumination radius `+ceil(imbue_strength)` tiles; corruption does not spread into illuminated tiles while lamp is lit — a warded pool of light |
| Bone-Tip Spear | Hunt-strike resolver applies `+0.05 * imbue_strength` focus bonus; snap risk unchanged (the current does not override ecology) |
| Remedy (any) | Potent variant: effect magnitude `× (1.0 + imbue_strength)` |
| Scent Censer | Contributes to tile commune echo at `imbue_strength * 0.3` strength per tick passively; stacks with live commune actions; effectively a slow permanent communer at that site |
| Play Bundle | Play action generates higher social-learning yield for kittens: `+0.1 * imbue_strength` to the learning transfer; the current in the object amplifies the bond formed during play |
| Hide Wrap / Bracers | Wearer's personal corruption decays at `+0.001 * imbue_strength` per tick additional — ordered-aspect cloth draws the wild aspect out slowly over time |

### Corrupted item effects (resolver-keyed)

A corrupted item carries the wild aspect fixed in form. The effects are real and sometimes
deliberately sought — particularly by cats already in the overwhelmed or Dark Calling window.

| Crafted item | Corrupted effect |
|--------------|-----------------|
| Grooming Brush | Grooming transmits `+0.02` personal corruption to recipient per session |
| Bone-Tip Spear | Successful hunt strike applies `+0.03` personal corruption to wielder; the wild aspect feeds back |
| Remedy (any) | Effect is weakened or reversed: healing poultice restores only `+0.1` health; also applies `+0.05` personal corruption to recipient |
| Tallow Lamp | Illumination radius unchanged; corruption spread rate *increases* `+0.001/tick` in illuminated tiles while lit — a beacon of disorder |
| Scent Censer | Emits wild-aspect gradient at the tile; slowly raises ambient tile corruption at `+0.0005/tick`; shadow fox spawn probability elevated near this tile |

A corrupted item is recognizable — a `MisfireEffect` event fires at crafting time. Cats in the
sensitized or exposed range may score *toward* using corrupted items; they are already in resonance
with the wild aspect. This behavioral signal is a pre-Dark Calling indicator readable in the event
log.

**Open question for implementation:** The depth-selection mechanic (shallow/moderate/deep) requires
either a new player-facing affordance or an AI decision at crafting time. Recommended: the crafter's
`magic_affinity` and `magic_skill` drive an automatic depth selection — high-affinity cats reach
deeper by nature; the skill deficit then determines whether they control it. This preserves the
personality-driven model without requiring explicit player input.

---

## The Dark Calling (cross-reference)

Documented in full in `the-calling.md` §7.W.5. Under this system's framing, the path to a Dark
Calling is accessible through ordinary practice pushed too far:

1. **Repeated deep imbuing** without recovery time between crafting sessions
2. **Performing burial rites** in heavily corrupted territory across multiple deaths
3. **False premonitions acted upon** — a low-skill high-affinity cat who repeatedly follows wrong
   impressions may wander into dangerous territory, accumulating corruption from the world itself

No new trigger logic is required. The Dark Calling is the same `axis_capture` mechanism as the
Calling with personal corruption ≥ 0.70 as the gate. This system's mechanics produce cats who
reach that threshold through identifiable, logged behavior — every step visible in `just q trace`.

---

## Formulas

```
// Personal corruption — sensitized affinity bonus
effective_affinity = affinity * (1.0 + 0.2 * clamp(personal_corruption, 0.0, 0.15))
// Note: bonus peaks at 0.15 and does not increase further

// Misfire (unchanged)
misfire_probability = max(0.0, (1.0 - magic_skill / affinity) * 0.5)
  (only evaluated when magic_skill < affinity * 0.8)

// Corruption spread (reframed, mechanics unchanged)
corruption(tile, t+1) = corruption(tile, t) + 0.001   // base spread per adjacent corrupted tile
                      × 0.5                            // within active commune echo
                      × 0.0                            // at natural ward (burial rite) site

// Ward strength decay (unchanged)
ward_strength(t+1) = ward_strength(t) - decay_rate

// Commune echo
echo_strength_at_creation = magic_skill * 0.6 + effective_affinity * 0.4
  (× 1.5 at Terrain::FairyRing)
echo(tile, t+1) = echo(tile, t) - 0.003

// Personal corruption decay
personal_corruption(t+1) = personal_corruption(t) * 0.999         // baseline
personal_corruption(t+1) = personal_corruption(t) * 0.995         // at warded or echoed tile

// Imbue potential at crafting
imbue_potential = magic_skill * depth + affinity * 0.3
personal_corruption_gain = depth * (1.0 - magic_skill * 0.5)

// Premonition accuracy
accuracy = 0.5 + 0.5 * magic_skill

// Mood effects from personal corruption (unchanged thresholds, new tiers)
mood_jitter = uniform(-corruption * 0.5, corruption * 0.5)   // [0.15, 0.40] Exposed
mood_jitter = uniform(-corruption, corruption)               // [0.40+]  Overwhelmed

// Social penalty (unchanged formula, new lower onset)
fondness_delta_modifier = fondness_delta * (1.0 - 0.1 * corruption_level)
```

---

## Integration with existing systems

**What changes in current code:**

- **`resolve_scry`** (`src/steps/magic/scry.rs`): Replace `random_tile` memory entry with actual
  herb-reveal scan + corruption gradient marking. Add `PreyDirectionHint` marker emission. Personal
  corruption gain on completion.
- **`resolve_spirit_communion`** (`src/steps/magic/spirit_communion.rs`): Retain mood bonus. Add
  `CommuneEcho` component spawn/strengthen on current tile. Add personal corruption gain.
- **`personal_corruption_effects`** (`src/systems/magic.rs`): Add sensitized-range affinity bonus
  branch. Currently only applies penalties; the 0.0–0.15 range should not apply penalties and
  should instead modify the cat's effective affinity (new scored input for scry/commune DSEs).
- **`corruption_tile_effects`** (`src/systems/magic.rs`): Add recovery bonus: at warded/echoed
  tiles, apply `× 0.995` decay factor to personal corruption each tick.

**What stays the same:** `corruption_spread`, `ward_decay`, `update_ward_coverage_map`,
`apply_misfire`, `check_misfire`, `apply_remedy_effects`, `spawn_shadow_fox_from_corruption` —
mechanics unchanged; semantics reframed by this document.

**What is new (not yet in code):**

- `CommuneEcho` tile component with strength and decay (analogous to `WardStrength`).
- Premonition system: per-tick trigger, marker emission, DSE scoring axis.
- Rite DSEs: one per rite type; each gates on `magic_affinity >= 0.4` and appropriate crafted-item
  availability. Rite step resolvers in `src/steps/magic/`.
- Imbued/corrupted item flags on `CraftedItem` (impl in crafting system, read by resolvers).

**Cross-system connections:**

- **Crafting — Herbalism & Remedy:** `Potent` remedy output when source herbs carry potency flag
  from echoed tile. Rite inputs (herb bundles, bone tokens) are Herbalism & Remedy and Bone & Shell
  Craft outputs respectively.
- **Crafting — Phase 4 (Scent Censer):** Imbued Scent Censer contributes passively to commune echo
  at its tile. Season-Opening Rite consumes a Scent Censer.
- **`the-calling.md`:** Commune echo tiles raise Calling trigger weight (+50%). Dark Calling falls
  out of corruption mechanics above; no new system.
- **`naming.md`:** `RitePerformed` events are Significant-tier; all four rite types are naming-
  eligible. Accurate premonition confirmations are Significant-tier events.
- **`collective-memory.md`:** Rite outcomes should propagate through social knowledge. A cat who
  witnesses a successful Hunt-Blessing followed by a good hunt learns the practice — same social-
  transmission logic as mentoring.
- **`corpse-handling.md`:** Burial Rite is a direct extension of the burial behavior chain; the
  rite DSE scores when a burial completes and a high-affinity cat is nearby.
- **`sensory.md`:** Premonitions are a fifth perceptual channel — not scent/sight/sound/tremor, but
  the current itself. Log as a distinct event type visible in `just q narrative`.

## Tuning Notes
_Record observations and adjustments here during iteration._
